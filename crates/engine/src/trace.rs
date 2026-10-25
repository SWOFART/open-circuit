//! Bounded per-run signal history and transport-independent trace queries.

use std::collections::HashSet;

use crate::{
    Diagnostic, DiagnosticCode, ResultLayout, SIMULATION_TIMESTEP_SECONDS, TraceBatch,
    TraceBatchMetadata, TracePoint, TraceQuery, TraceRequest, TraceSignal,
};

const MAX_TRACE_SIGNALS: usize = 8;
const MAX_TRACE_POINTS: u32 = 4096;
const MAX_HISTORY_SAMPLES: usize = 10_000;
const MAX_HISTORY_VALUE_BYTES: usize = 1 << 24;
const MAX_DOCUMENT_REVISION_BYTES: usize = 256;

/// A flat, preallocated ring of all result-layout values for one compiled run.
pub(super) struct TraceHistory {
    value_count: usize,
    capacity: usize,
    start: usize,
    len: usize,
    steps: Vec<u64>,
    revisions: Vec<String>,
    values: Vec<f64>,
}

impl TraceHistory {
    pub(super) fn new(value_count: usize) -> Option<Self> {
        if value_count > MAX_HISTORY_VALUE_BYTES / std::mem::size_of::<f64>() {
            return None;
        }
        let capacity = if value_count == 0 {
            MAX_HISTORY_SAMPLES
        } else {
            (MAX_HISTORY_VALUE_BYTES / (std::mem::size_of::<f64>() * value_count))
                .clamp(1, MAX_HISTORY_SAMPLES)
        };

        Self::with_capacity(value_count, capacity)
    }

    pub(super) fn with_capacity(value_count: usize, capacity: usize) -> Option<Self> {
        if value_count > MAX_HISTORY_VALUE_BYTES / std::mem::size_of::<f64>() {
            return None;
        }
        let max_capacity = if value_count == 0 {
            MAX_HISTORY_SAMPLES
        } else {
            (MAX_HISTORY_VALUE_BYTES / (std::mem::size_of::<f64>() * value_count))
                .clamp(1, MAX_HISTORY_SAMPLES)
        };
        if capacity == 0 || capacity > max_capacity {
            return None;
        }

        Some(Self {
            value_count,
            capacity,
            start: 0,
            len: 0,
            steps: vec![0; capacity],
            revisions: vec![String::new(); capacity],
            values: vec![0.0; capacity.saturating_mul(value_count)],
        })
    }

    pub(super) fn append(&mut self, step: u64, revision: &str, values: &[f64]) -> Result<(), ()> {
        if revision.len() > MAX_DOCUMENT_REVISION_BYTES
            || values.len() != self.value_count
            || values.iter().any(|value| !value.is_finite())
        {
            return Err(());
        }

        let physical = if self.len < self.capacity {
            let physical = (self.start + self.len) % self.capacity;
            self.len += 1;
            physical
        } else {
            let physical = self.start;
            self.start = (self.start + 1) % self.capacity;
            physical
        };
        self.write(physical, step, revision, values);
        Ok(())
    }

    pub(super) fn replace_latest(
        &mut self,
        step: u64,
        revision: &str,
        values: &[f64],
    ) -> Result<(), ()> {
        if self.len > 0 && self.step_at(self.len - 1) == Some(step) {
            if revision.len() > MAX_DOCUMENT_REVISION_BYTES
                || values.len() != self.value_count
                || values.iter().any(|value| !value.is_finite())
            {
                return Err(());
            }
            let physical = self.physical_index(self.len - 1);
            self.write(physical, step, revision, values);
            return Ok(());
        }

        self.append(step, revision, values)
    }

    pub(super) fn capacity(&self) -> usize {
        self.capacity
    }

    pub(super) fn append_history(&mut self, source: &Self) -> Result<(), ()> {
        if self.value_count != source.value_count
            || (0..source.len).any(|logical| {
                source
                    .revision_at(logical)
                    .is_none_or(|revision| revision.len() > MAX_DOCUMENT_REVISION_BYTES)
                    || source
                        .values_at(logical)
                        .is_none_or(|values| values.iter().any(|value| !value.is_finite()))
            })
        {
            return Err(());
        }

        for logical in 0..source.len {
            let step = source.step_at(logical).ok_or(())?;
            let revision = source.revision_at(logical).ok_or(())?;
            let values = source.values_at(logical).ok_or(())?;
            self.append(step, revision, values)?;
        }
        Ok(())
    }

    fn write(&mut self, physical: usize, step: u64, revision: &str, values: &[f64]) {
        self.steps[physical] = step;
        self.revisions[physical].clear();
        self.revisions[physical].push_str(revision);
        let value_start = physical * self.value_count;
        self.values[value_start..value_start + self.value_count].copy_from_slice(values);
    }

    fn physical_index(&self, logical: usize) -> usize {
        (self.start + logical) % self.capacity
    }

    fn step_at(&self, logical: usize) -> Option<u64> {
        (logical < self.len).then(|| self.steps[self.physical_index(logical)])
    }

    fn revision_at(&self, logical: usize) -> Option<&str> {
        (logical < self.len).then(|| self.revisions[self.physical_index(logical)].as_str())
    }

    fn value_at(&self, logical: usize, slot_offset: usize) -> Option<f64> {
        if logical >= self.len || slot_offset >= self.value_count {
            return None;
        }
        let physical = self.physical_index(logical);
        self.values
            .get(physical * self.value_count + slot_offset)
            .copied()
    }

    fn values_at(&self, logical: usize) -> Option<&[f64]> {
        if logical >= self.len {
            return None;
        }
        let physical = self.physical_index(logical);
        let start = physical.checked_mul(self.value_count)?;
        self.values.get(start..start + self.value_count)
    }

    fn logical_index_of_step(&self, step: u64) -> Option<usize> {
        (0..self.len).find(|&logical| self.step_at(logical) == Some(step))
    }

    pub(super) fn read(
        &self,
        request: &TraceRequest,
        run_id: crate::StableId,
        layout: &ResultLayout,
    ) -> Result<TraceBatch, Vec<Diagnostic>> {
        if request.signal_keys.is_empty()
            || request.signal_keys.len() > MAX_TRACE_SIGNALS
            || request.max_points == 0
            || request.max_points > MAX_TRACE_POINTS
        {
            return Err(invalid_trace_request());
        }

        let mut unique_keys = HashSet::with_capacity(request.signal_keys.len());
        let mut signals = Vec::with_capacity(request.signal_keys.len());
        let mut offsets = Vec::with_capacity(request.signal_keys.len());
        for key in &request.signal_keys {
            if key.len() > 256 || !unique_keys.insert(key) {
                return Err(invalid_trace_request());
            }
            let Some(slot) = layout.slots.iter().find(|slot| slot.key == *key) else {
                return Err(invalid_trace_request());
            };
            let offset = usize::try_from(slot.offset).map_err(|_| invalid_trace_request())?;
            if offset >= self.value_count {
                return Err(invalid_trace_request());
            }
            signals.push(TraceSignal {
                key: slot.key.clone(),
                unit: slot.unit,
            });
            offsets.push(offset);
        }

        let oldest_step = self.step_at(0);
        let latest_step = self.step_at(self.len.saturating_sub(1));
        let mut missed_samples = 0_u64;
        let mut replaced = false;
        let (first_step, last_step) = match (&request.query, oldest_step, latest_step) {
            (_, Some(oldest), Some(latest)) => {
                let (start, end) = match &request.query {
                    TraceQuery::History {
                        start_step,
                        end_step,
                    } => {
                        if end_step < start_step {
                            return Err(invalid_trace_request());
                        }
                        if *start_step < oldest {
                            let last_missed = (*end_step).min(oldest.saturating_sub(1));
                            if last_missed >= *start_step {
                                missed_samples =
                                    last_missed.saturating_sub(*start_step).saturating_add(1);
                            }
                        }
                        ((*start_step).max(oldest), (*end_step).min(latest))
                    }
                    TraceQuery::NewSamples {
                        after_step,
                        after_document_revision,
                    } => {
                        if after_document_revision.len() > 256 {
                            return Err(invalid_trace_request());
                        }
                        if *after_step < oldest {
                            missed_samples = oldest.saturating_sub(after_step.saturating_add(1));
                            (oldest, latest)
                        } else if let Some(logical) = self.logical_index_of_step(*after_step)
                            && self.revision_at(logical) != Some(after_document_revision.as_str())
                        {
                            replaced = true;
                            (*after_step, latest)
                        } else if *after_step >= latest {
                            (1, 0)
                        } else {
                            (after_step.saturating_add(1), latest)
                        }
                    }
                };
                (start, end)
            }
            _ => (1, 0),
        };

        let first_logical = if first_step <= last_step {
            self.logical_index_of_step(first_step)
        } else {
            None
        };
        let last_logical = if first_step <= last_step {
            self.logical_index_of_step(last_step)
        } else {
            None
        };
        let raw_count = match (first_logical, last_logical) {
            (Some(first), Some(last)) if last >= first => last - first + 1,
            _ => 0,
        };
        let max_points = request.max_points as usize;
        let bucket_size = raw_count.div_ceil(max_points).max(1);
        let mut points = Vec::with_capacity(raw_count.div_ceil(bucket_size));

        for bucket_start in (0..raw_count).step_by(bucket_size) {
            let bucket_end = (bucket_start + bucket_size).min(raw_count);
            let first_row =
                first_logical.expect("a non-empty range has a first row") + bucket_start;
            let sample_step = self
                .step_at(first_row)
                .expect("the selected first row is in the ring");
            let mut minimum_values = vec![f64::INFINITY; offsets.len()];
            let mut maximum_values = vec![f64::NEG_INFINITY; offsets.len()];
            for row in
                first_row..first_logical.expect("a non-empty range has a first row") + bucket_end
            {
                for (signal_index, offset) in offsets.iter().copied().enumerate() {
                    let value = self
                        .value_at(row, offset)
                        .ok_or_else(invalid_trace_request)?;
                    if !value.is_finite() {
                        return Err(invalid_trace_request());
                    }
                    minimum_values[signal_index] = minimum_values[signal_index].min(value);
                    maximum_values[signal_index] = maximum_values[signal_index].max(value);
                }
            }
            points.push(TracePoint {
                simulation_step: sample_step,
                minimum_values,
                maximum_values,
            });
        }

        let response_revision = last_logical
            .and_then(|logical| self.revision_at(logical))
            .or_else(|| self.revision_at(self.len.saturating_sub(1)))
            .unwrap_or_default()
            .to_owned();
        let first_sample_step = first_logical.and_then(|logical| self.step_at(logical));
        let last_sample_step = last_logical.and_then(|logical| self.step_at(logical));
        let sample_count = u32::try_from(raw_count).map_err(|_| invalid_trace_request())?;
        let history_samples = u32::try_from(self.capacity).map_err(|_| invalid_trace_request())?;

        Ok(TraceBatch {
            metadata: TraceBatchMetadata {
                run_id,
                document_revision: response_revision,
                layout_version: layout.layout_version,
                sample_interval_seconds: SIMULATION_TIMESTEP_SECONDS,
                history_samples,
                oldest_sample_step: oldest_step,
                latest_sample_step: latest_step,
                first_sample_step,
                last_sample_step,
                sample_count,
                missed_samples,
                replaced,
            },
            signals,
            points,
        })
    }
}

fn invalid_trace_request() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        DiagnosticCode::InvalidParameter,
        Vec::new(),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ResultQuantity, ResultSignConvention, ResultSlot, StableId, TraceQuery, TraceRequest, Unit,
    };

    #[test]
    fn capacity_is_bounded_by_value_count_and_history_limit() {
        assert_eq!(TraceHistory::new(0).unwrap().capacity(), 10_000);
        assert_eq!(TraceHistory::new(2).unwrap().capacity(), 10_000);
        assert_eq!(TraceHistory::new(209).unwrap().capacity(), 10_000);
        assert_eq!(TraceHistory::new(210).unwrap().capacity(), 9_986);
        assert_eq!(TraceHistory::new(65_536).unwrap().capacity(), 32);
        assert!(TraceHistory::new((1 << 21) + 1).is_none());
    }

    #[test]
    fn revisions_are_bounded_before_they_are_retained() {
        let mut history = TraceHistory::new(1).unwrap();
        let oversized_revision = "r".repeat(MAX_DOCUMENT_REVISION_BYTES + 1);

        assert!(history.append(0, &oversized_revision, &[0.0]).is_err());
        assert!(
            history
                .replace_latest(0, &oversized_revision, &[0.0])
                .is_err()
        );
        assert_eq!(history.len, 0);
    }

    #[test]
    fn ring_reports_overwritten_samples_and_decimates_with_peak_preservation() {
        let mut history = TraceHistory {
            value_count: 1,
            capacity: 3,
            start: 0,
            len: 0,
            steps: vec![0; 3],
            revisions: vec![String::new(); 3],
            values: vec![0.0; 3],
        };
        for step in 0..5 {
            history.append(step, "revision-1", &[step as f64]).unwrap();
        }
        let run_id = StableId::parse("00000000-0000-4000-8000-000000000001").unwrap();
        let layout = ResultLayout {
            layout_version: 1,
            value_count: 1,
            slots: vec![ResultSlot {
                key: "signal".to_owned(),
                component_id: run_id.clone(),
                quantity: ResultQuantity::Voltage,
                unit: Unit::Volt,
                sign_convention: ResultSignConvention::VoltagePin1MinusPin2,
                offset: 0,
            }],
        };
        let batch = history
            .read(
                &TraceRequest {
                    run_id: run_id.clone(),
                    layout_version: 1,
                    signal_keys: vec!["signal".to_owned()],
                    query: TraceQuery::History {
                        start_step: 0,
                        end_step: 4,
                    },
                    max_points: 2,
                },
                run_id.clone(),
                &layout,
            )
            .unwrap();
        assert_eq!(batch.metadata.missed_samples, 2);
        assert_eq!(batch.metadata.sample_count, 3);
        assert_eq!(batch.points.len(), 2);
        assert_eq!(batch.points[0].simulation_step, 2);
        assert_eq!(batch.points[0].minimum_values, vec![2.0]);
        assert_eq!(batch.points[0].maximum_values, vec![3.0]);
        assert_eq!(batch.points[1].simulation_step, 4);

        let incremental = history
            .read(
                &TraceRequest {
                    run_id: run_id.clone(),
                    layout_version: 1,
                    signal_keys: vec!["signal".to_owned()],
                    query: TraceQuery::NewSamples {
                        after_step: 0,
                        after_document_revision: "revision-1".to_owned(),
                    },
                    max_points: 8,
                },
                run_id,
                &layout,
            )
            .unwrap();
        assert_eq!(incremental.metadata.missed_samples, 1);
        assert_eq!(incremental.metadata.first_sample_step, Some(2));
        assert_eq!(incremental.metadata.last_sample_step, Some(4));
    }
}
