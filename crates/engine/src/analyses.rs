//! Bounded, transport-independent analyses of immutable circuit inputs.

use std::collections::HashSet;

use crate::{
    ComponentSpec, Diagnostic, DiagnosticCode, GeometryEnvelope, ResultQuantity, Schematic,
    StableId, Unit, devices::component_diagnostic, newton::SolverMemory,
};

use super::{
    SolveMode, build_result_layout, compute_circuit_with_sweep, has_errors, values_for_layout,
};

pub(super) const MAX_ANALYSIS_UNKNOWNS: usize = 128;

#[derive(Debug, PartialEq)]
pub enum AnalysisError {
    Cancelled,
    InvalidRequest(Vec<Diagnostic>),
    UnsupportedCircuit(Vec<StableId>),
    SolveFailed {
        point_index: usize,
        diagnostics: Vec<Diagnostic>,
    },
}

#[derive(Debug)]
pub struct DcSweepRequest<'a> {
    pub schematic: &'a Schematic,
    pub geometry: &'a GeometryEnvelope,
    pub document_revision: &'a str,
    pub source_id: &'a StableId,
    pub signal_key: &'a str,
    pub start_volts: f64,
    pub stop_volts: f64,
    pub point_count: usize,
}

#[derive(Debug, PartialEq)]
pub struct DcSweepPoint {
    pub source_volts: f64,
    pub reading: f64,
}

#[derive(Debug, PartialEq)]
pub struct DcSweepResult {
    pub document_revision: String,
    pub source_id: StableId,
    pub signal_key: String,
    pub unit: Unit,
    pub points: Vec<DcSweepPoint>,
}

/// Solve the authored circuit's operating points without accessing a simulation run.
pub fn dc_sweep(
    request: DcSweepRequest<'_>,
    is_cancelled: impl Fn() -> bool,
) -> Result<DcSweepResult, AnalysisError> {
    check_cancelled(&is_cancelled)?;
    validate_snapshot(
        request.schematic,
        request.geometry,
        request.document_revision,
    )?;
    if !(2..=257).contains(&request.point_count)
        || !(-1000.0..=1000.0).contains(&request.start_volts)
        || !(-1000.0..=1000.0).contains(&request.stop_volts)
        || request.start_volts == request.stop_volts
    {
        return Err(invalid_parameter());
    }

    let mut unsupported: Vec<_> = request
        .schematic
        .components
        .iter()
        .filter(|component| {
            !matches!(
                component,
                ComponentSpec::DcVoltageSource { .. }
                    | ComponentSpec::Resistor { .. }
                    | ComponentSpec::Capacitor { .. }
                    | ComponentSpec::Inductor { .. }
                    | ComponentSpec::Diode { .. }
                    | ComponentSpec::BjtNpn { .. }
                    | ComponentSpec::BjtPnp { .. }
                    | ComponentSpec::Ammeter { .. }
                    | ComponentSpec::Voltmeter { .. }
            )
        })
        .map(|component| component.id().clone())
        .collect();
    if !unsupported.is_empty() {
        unsupported.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        return Err(AnalysisError::UnsupportedCircuit(unsupported));
    }

    let mut schematic = request.schematic.clone();
    let source_index = schematic
        .components
        .iter()
        .position(|component| {
            matches!(component, ComponentSpec::DcVoltageSource { .. })
                && component.id() == request.source_id
        })
        .ok_or_else(|| {
            AnalysisError::InvalidRequest(vec![component_diagnostic(
                DiagnosticCode::InvalidParameter,
                request.source_id,
            )])
        })?;
    let layout = build_result_layout(&schematic).map_err(AnalysisError::InvalidRequest)?;
    let slot = layout
        .slots
        .iter()
        .find(|slot| {
            slot.key == request.signal_key
                && matches!(
                    slot.quantity,
                    ResultQuantity::Voltage | ResultQuantity::Current
                )
        })
        .ok_or_else(invalid_parameter)?;

    let mut points = Vec::with_capacity(request.point_count);
    let mut memory = SolverMemory::default();
    let open = HashSet::new();
    let last = request.point_count - 1;
    for point_index in 0..request.point_count {
        check_cancelled(&is_cancelled)?;
        let volts = if point_index == 0 {
            request.start_volts
        } else if point_index == last {
            request.stop_volts
        } else {
            request.start_volts
                + (request.stop_volts - request.start_volts) * (point_index as f64 / last as f64)
        };
        let ComponentSpec::DcVoltageSource { parameters, .. } =
            &mut schematic.components[source_index]
        else {
            unreachable!("validated sweep source");
        };
        parameters.voltage_volts = volts;
        let failed = |diagnostics| AnalysisError::SolveFailed {
            point_index,
            diagnostics,
        };
        let (results, _, accepted_memory) = compute_circuit_with_sweep(
            &schematic,
            request.geometry,
            &open,
            SolveMode::OperatingPoint,
            Some(&memory),
        )
        .map_err(failed)?;
        let values = values_for_layout(&layout, &results).map_err(failed)?;
        points.push(DcSweepPoint {
            source_volts: volts,
            reading: values[slot.offset as usize],
        });
        memory = accepted_memory;
    }
    check_cancelled(&is_cancelled)?;
    Ok(DcSweepResult {
        document_revision: request.document_revision.to_owned(),
        source_id: request.source_id.clone(),
        signal_key: request.signal_key.to_owned(),
        unit: slot.unit,
        points,
    })
}

fn validate_snapshot(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
    revision: &str,
) -> Result<(), AnalysisError> {
    if schematic.components.len() > 128
        || geometry.components.len() > 128
        || schematic.wires.len() > 512
        || schematic.rails.len() > 128
        || schematic
            .reference_markers
            .len()
            .saturating_add(schematic.junctions.len())
            > 128
        || revision.len() > 256
    {
        return Err(invalid_parameter());
    }
    let mut diagnostics = schematic.validate();
    diagnostics.extend(geometry.validate_for(schematic));
    if has_errors(&diagnostics) {
        return Err(AnalysisError::InvalidRequest(diagnostics));
    }
    Ok(())
}

fn invalid_parameter() -> AnalysisError {
    AnalysisError::InvalidRequest(vec![Diagnostic::error(
        DiagnosticCode::InvalidParameter,
        Vec::new(),
    )])
}

fn check_cancelled(is_cancelled: &impl Fn() -> bool) -> Result<(), AnalysisError> {
    if is_cancelled() {
        Err(AnalysisError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_memory_retains_the_accepted_ideal_source_branch_current() {
        let mut schematic: Schematic = serde_json::from_str(include_str!(
            "../../../tests/golden/stage-1-divider.schematic.json"
        ))
        .unwrap();
        let geometry = serde_json::from_str(include_str!(
            "../../../tests/golden/stage-1-divider.geometry.json"
        ))
        .unwrap();
        let ComponentSpec::DcVoltageSource { parameters, id, .. } = &mut schematic.components[0]
        else {
            panic!("divider source must be DC");
        };
        parameters.internal_resistance_ohms = 0.0;
        let id = id.clone();
        let (_, _, memory) = compute_circuit_with_sweep(
            &schematic,
            &geometry,
            &HashSet::new(),
            SolveMode::OperatingPoint,
            Some(&SolverMemory::default()),
        )
        .unwrap();
        assert!((memory.sweep_branch_currents[id.as_str()].abs() - 0.0025).abs() < 1e-12);
    }
}
