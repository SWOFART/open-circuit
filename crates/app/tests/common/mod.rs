//! Shared helpers of the Stage 3 acceptance tests: a real Engine run through
//! `SimulationCore` against a fake clock, and a reader for the trace envelope.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use open_circuit_app_lib::frame::decode_frame;
use open_circuit_app_lib::simulation::{
    FrameSink, SimulationConfig, SimulationCore, SinkClosed, TraceSink,
};
use open_circuit_engine::{
    CompileRequest, CompileResult, ResultLayout, ResultQuantity, ResultSnapshot, StableId,
    TraceQuery, TraceRequest,
};
use serde_json::Value;

#[derive(Clone, Default)]
pub struct Capture(Arc<Mutex<Vec<Vec<u8>>>>);

impl Capture {
    pub fn take(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut *self.0.lock().expect("lock"))
    }
}

impl FrameSink for Capture {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed> {
        self.0.lock().expect("lock").push(frame);
        Ok(())
    }
}

impl TraceSink for Capture {
    fn send(&mut self, response: Vec<u8>) -> Result<(), SinkClosed> {
        self.0.lock().expect("lock").push(response);
        Ok(())
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: {actual} is not within {tolerance} of {expected}"
    );
}

pub fn slot_key(layout: &ResultLayout, component: &str, quantity: ResultQuantity) -> String {
    layout
        .slots
        .iter()
        .find(|slot| slot.component_id.as_str() == component && slot.quantity == quantity)
        .unwrap_or_else(|| panic!("no {quantity:?} slot for {component}"))
        .key
        .clone()
}

pub fn value_of(
    layout: &ResultLayout,
    snapshot: &ResultSnapshot,
    component: &str,
    quantity: ResultQuantity,
) -> f64 {
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.component_id.as_str() == component && slot.quantity == quantity)
        .expect("a slot for the component and quantity");

    snapshot.values[slot.offset as usize]
}

/// What a trace response says, read straight from the bytes of the envelope in
/// docs/contracts/stage-1.md section 8.3.
pub struct Trace {
    pub header: Value,
    /// One entry per point: its step, then a (minimum, maximum) pair for each signal.
    pub points: Vec<(u64, Vec<(f64, f64)>)>,
}

pub fn parse_trace(bytes: &[u8]) -> Trace {
    assert_eq!(&bytes[..4], b"OCT1");
    let header_length = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let header: Value = serde_json::from_slice(&bytes[8..8 + header_length]).unwrap();
    let signals = header["signals"].as_array().unwrap().len();
    let count = header["pointCount"].as_u64().unwrap() as usize;
    let mut offset = (8 + header_length).next_multiple_of(8);
    let mut points = Vec::new();

    for _ in 0..count {
        let step = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let mut values = Vec::new();

        for _ in 0..signals {
            let minimum = f64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
            let maximum = f64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().unwrap());
            offset += 16;
            values.push((minimum, maximum));
        }

        points.push((step, values));
    }

    assert_eq!(offset, bytes.len(), "the response has no trailing bytes");

    Trace { header, points }
}

/// A compiled run that streams against a fake clock.
pub struct Run {
    pub core: SimulationCore,
    pub run_id: StableId,
    pub layout: ResultLayout,
    pub initial_snapshot: ResultSnapshot,
    pub compile_result: CompileResult,
    frames: Capture,
    traces: Capture,
    now: Instant,
}

impl Run {
    /// Compiles `request` with the real Engine and opens the frame stream.
    pub fn start(request: CompileRequest) -> Self {
        let mut core = SimulationCore::new(SimulationConfig::default());
        let compile_result = core.compile(request);
        let CompileResult::Success {
            run_id,
            layout,
            initial_snapshot,
            ..
        } = &compile_result
        else {
            panic!("the circuit must compile: {compile_result:?}");
        };
        let (run_id, layout, initial_snapshot) =
            (run_id.clone(), layout.clone(), initial_snapshot.clone());
        let frames = Capture::default();
        let now = Instant::now();

        core.start(run_id.clone(), Box::new(frames.clone()), now)
            .expect("start streaming");

        Self {
            core,
            run_id,
            layout,
            initial_snapshot,
            compile_result,
            frames,
            traces: Capture::default(),
            now,
        }
    }

    /// Moves the fake clock by `milliseconds` in 1 ms ticks, acknowledging every frame.
    /// Returns the newest step a frame reported.
    pub fn advance(&mut self, milliseconds: u32) -> u64 {
        let mut newest = 0;

        for _ in 0..milliseconds {
            self.now += Duration::from_millis(1);
            self.core.tick(self.now);

            for frame in self.frames.take() {
                let snapshot = decode_frame(&frame).expect("every delivered frame decodes");
                newest = newest.max(snapshot.metadata.simulation_step);
                self.core.acknowledge(snapshot.metadata.frame_sequence);
            }
        }

        newest
    }

    /// One bounded history read, parsed. The response is acknowledged so the next read
    /// is admitted. Also returns the raw bytes and the request.
    pub fn read(
        &mut self,
        signal_keys: &[String],
        start_step: u64,
        end_step: u64,
    ) -> (Trace, Vec<u8>, TraceRequest) {
        let request = TraceRequest {
            run_id: self.run_id.clone(),
            layout_version: self.layout.layout_version,
            signal_keys: signal_keys.to_vec(),
            query: TraceQuery::History {
                start_step,
                end_step,
            },
            max_points: 2048,
        };
        let sequence = self
            .core
            .read_traces(request.clone(), Box::new(self.traces.clone()), self.now)
            .expect("the trace request is served");
        self.core.acknowledge_trace(&self.run_id, sequence);
        let responses = self.traces.take();

        assert_eq!(responses.len(), 1, "one response per request");

        (parse_trace(&responses[0]), responses[0].clone(), request)
    }

    /// Every public step from `first` to `last`, exactly, in windows the envelope can
    /// carry. Each entry holds the step and one value per requested signal.
    pub fn samples(
        &mut self,
        signal_keys: &[String],
        first: u64,
        last: u64,
    ) -> Vec<(u64, Vec<f64>)> {
        let mut samples = Vec::new();
        let mut start = first;

        while start <= last {
            let end = (start + 1999).min(last);
            let (trace, _, _) = self.read(signal_keys, start, end);

            assert_eq!(
                trace.points.len() as u64,
                end - start + 1,
                "an undecimated window holds one point for each step"
            );
            for (step, values) in trace.points {
                assert!(
                    values.iter().all(|(minimum, maximum)| minimum == maximum),
                    "an undecimated point is one sample"
                );
                samples.push((step, values.iter().map(|(value, _)| *value).collect()));
            }
            start = end + 1;
        }

        samples
    }
}
