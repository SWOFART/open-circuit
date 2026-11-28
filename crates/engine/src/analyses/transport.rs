//! Transport-independent request and outcome shapes of the extra analyses
//! (docs/contracts/stage-17-extra-analyses.md section 5), and the function that runs a request.
//!
//! The application owns scheduling, cancellation and IPC; it sends these tagged values as JSON.
//! Step numbers and run identifiers that can exceed the JavaScript safe integer range cross as
//! decimal strings. Every numeric output is checked to be finite before it can be published.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    Diagnostic, DiagnosticCode, GeometryEnvelope, PinId, Schematic, StableId, Unit, netlist::PinRef,
};

use super::{
    AnalysisError, DcSweepRequest, FrequencyResponseRequest, FrequencySpacing, SpectrumCapture,
    SpectrumRequest, dc_sweep, frequency_response, harmonic_spectrum,
};

/// The largest request or response the transport carries, in bytes of JSON.
pub const MAX_ANALYSIS_PAYLOAD_BYTES: usize = 512 * 1024;

/// A pin of a component, by stable ID: a terminal of a frequency-response output.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "AnalysisTerminal.ts")]
pub struct AnalysisTerminal {
    pub component_id: StableId,
    pub pin_id: PinId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AnalysisSpacing.ts")]
pub enum AnalysisSpacing {
    Linear,
    Logarithmic,
}

/// Which analysis a request, a job or a result is.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AnalysisKind.ts")]
pub enum AnalysisKind {
    DcSweep,
    FrequencyResponse,
    Spectrum,
}

/// One analysis to run. Each request names its kind; the document revision names the drawing it
/// was made from, so a result can be told stale.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export, export_to = "AnalysisRequest.ts")]
pub enum AnalysisRequest {
    DcSweep {
        schematic: Box<Schematic>,
        geometry: Box<GeometryEnvelope>,
        document_revision: String,
        source_id: StableId,
        signal_key: String,
        start_volts: f64,
        stop_volts: f64,
        point_count: u32,
    },
    FrequencyResponse {
        schematic: Box<Schematic>,
        geometry: Box<GeometryEnvelope>,
        document_revision: String,
        source_id: StableId,
        output: [AnalysisTerminal; 2],
        start_hz: f64,
        stop_hz: f64,
        point_count: u32,
        spacing: AnalysisSpacing,
    },
    Spectrum {
        run_id: StableId,
        layout_version: u32,
        document_revision: String,
        signal_key: String,
        /// The inclusive last step, as a decimal string.
        end_step: String,
        sample_count: u32,
    },
}

impl AnalysisRequest {
    pub fn kind(&self) -> AnalysisKind {
        match self {
            Self::DcSweep { .. } => AnalysisKind::DcSweep,
            Self::FrequencyResponse { .. } => AnalysisKind::FrequencyResponse,
            Self::Spectrum { .. } => AnalysisKind::Spectrum,
        }
    }

    /// The drawing the request was made from.
    pub fn document_revision(&self) -> &str {
        match self {
            Self::DcSweep {
                document_revision, ..
            }
            | Self::FrequencyResponse {
                document_revision, ..
            }
            | Self::Spectrum {
                document_revision, ..
            } => document_revision,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "DcSweepPointDto.ts")]
pub struct DcSweepPointDto {
    pub source_volts: f64,
    pub reading: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "FrequencyResponsePointDto.ts")]
pub struct FrequencyResponsePointDto {
    pub frequency_hz: f64,
    pub magnitude: f64,
    /// Null for an exact zero magnitude.
    pub phase_degrees: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "SpectrumBinDto.ts")]
pub struct SpectrumBinDto {
    pub frequency_hz: f64,
    pub peak_amplitude: f64,
}

/// A finished analysis: the document revision and selection it answers, and its ordered points.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "AnalysisResult.ts")]
pub enum AnalysisResult {
    DcSweep {
        document_revision: String,
        source_id: StableId,
        signal_key: String,
        unit: Unit,
        points: Vec<DcSweepPointDto>,
    },
    FrequencyResponse {
        document_revision: String,
        source_id: StableId,
        output: [AnalysisTerminal; 2],
        points: Vec<FrequencyResponsePointDto>,
    },
    Spectrum {
        run_id: StableId,
        layout_version: u32,
        document_revision: String,
        signal_key: String,
        /// The first and the inclusive last captured step, as decimal strings.
        first_step: String,
        end_step: String,
        bin_spacing_hz: f64,
        unit: Unit,
        bins: Vec<SpectrumBinDto>,
    },
}

/// Why an analysis did not produce a result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AnalysisFailureReason.ts")]
pub enum AnalysisFailureReason {
    InvalidRequest,
    UnsupportedCircuit,
    UnsupportedSignal,
    StaleInput,
    UnavailableHistory,
    SolveFailed,
    /// The request or the complete response is larger than the transport carries.
    PayloadTooLarge,
}

/// A machine-readable failure, with the diagnostics and identifiers that explain it.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AnalysisFailure.ts")]
pub struct AnalysisFailure {
    pub reason: AnalysisFailureReason,
    pub diagnostics: Vec<Diagnostic>,
    /// The components an unsupported circuit was refused for.
    pub component_ids: Vec<StableId>,
    /// The zero-based index of the point that failed to solve.
    pub point_index: Option<u32>,
}

impl AnalysisFailure {
    pub fn payload_too_large() -> Self {
        Self::of(AnalysisFailureReason::PayloadTooLarge)
    }

    fn of(reason: AnalysisFailureReason) -> Self {
        Self {
            reason,
            diagnostics: Vec::new(),
            component_ids: Vec::new(),
            point_index: None,
        }
    }
}

/// The failure an analysis error is published as. A cancellation is not a failure: it has its own
/// outcome, so it is not converted (`None`).
pub fn failure_from(error: AnalysisError) -> Option<AnalysisFailure> {
    Some(match error {
        AnalysisError::Cancelled => return None,
        AnalysisError::InvalidRequest(diagnostics) => AnalysisFailure {
            diagnostics,
            ..AnalysisFailure::of(AnalysisFailureReason::InvalidRequest)
        },
        AnalysisError::UnsupportedCircuit(component_ids) => AnalysisFailure {
            component_ids,
            ..AnalysisFailure::of(AnalysisFailureReason::UnsupportedCircuit)
        },
        AnalysisError::UnsupportedSignal => {
            AnalysisFailure::of(AnalysisFailureReason::UnsupportedSignal)
        }
        AnalysisError::StaleInput(code) => AnalysisFailure {
            diagnostics: vec![Diagnostic::error(code, Vec::new())],
            ..AnalysisFailure::of(AnalysisFailureReason::StaleInput)
        },
        AnalysisError::UnavailableHistory => {
            AnalysisFailure::of(AnalysisFailureReason::UnavailableHistory)
        }
        AnalysisError::SolveFailed {
            point_index,
            diagnostics,
        } => AnalysisFailure {
            diagnostics,
            point_index: Some(u32::try_from(point_index).unwrap_or(u32::MAX)),
            ..AnalysisFailure::of(AnalysisFailureReason::SolveFailed)
        },
    })
}

/// Where a job stands. Only a success carries points.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "AnalysisOutcome.ts")]
pub enum AnalysisOutcome {
    Pending,
    Running,
    Cancelled,
    Succeeded { result: AnalysisResult },
    Failed { failure: AnalysisFailure },
}

/// What a read of a job returns: which job and analysis it is, the drawing it was made from, and
/// where it stands. The job ID is a session-local number as a decimal string.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AnalysisJobReport.ts")]
pub struct AnalysisJobReport {
    pub job_id: String,
    pub analysis: AnalysisKind,
    pub document_revision: String,
    pub outcome: AnalysisOutcome,
}

/// The answer to starting a job: it started, another job of the window is still active (busy), or
/// the request was refused before anything ran.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "AnalysisStart.ts")]
pub enum AnalysisStart {
    Started {
        job_id: String,
        analysis: AnalysisKind,
    },
    Busy,
    Rejected {
        failure: AnalysisFailure,
    },
}

fn terminal(pin: &PinRef) -> AnalysisTerminal {
    AnalysisTerminal {
        component_id: pin.component_id.clone(),
        pin_id: pin.pin_id,
    }
}

/// A count from the request, which must fit the analysis's own bounds: a number too large to be a
/// count is refused as an invalid request.
fn count(value: u32) -> Result<usize, AnalysisError> {
    usize::try_from(value).map_err(|_| {
        AnalysisError::InvalidRequest(vec![Diagnostic::error(
            DiagnosticCode::InvalidParameter,
            Vec::new(),
        )])
    })
}

fn invalid() -> AnalysisError {
    AnalysisError::InvalidRequest(vec![Diagnostic::error(
        DiagnosticCode::InvalidParameter,
        Vec::new(),
    )])
}

impl AnalysisResult {
    /// The index of the first point with a number that is not finite, if there is one. Phase is
    /// allowed to be absent but not to be a non-finite number.
    pub fn first_non_finite_point(&self) -> Option<usize> {
        let finite = |values: &[f64]| values.iter().all(|value| value.is_finite());
        match self {
            Self::DcSweep { points, .. } => points
                .iter()
                .position(|point| !finite(&[point.source_volts, point.reading])),
            Self::FrequencyResponse { points, .. } => points.iter().position(|point| {
                !finite(&[point.frequency_hz, point.magnitude])
                    || point.phase_degrees.is_some_and(|phase| !phase.is_finite())
            }),
            Self::Spectrum {
                bins,
                bin_spacing_hz,
                ..
            } => {
                if !bin_spacing_hz.is_finite() {
                    return Some(0);
                }
                bins.iter()
                    .position(|bin| !finite(&[bin.frequency_hz, bin.peak_amplitude]))
            }
        }
    }
}

/// Runs one request.
///
/// A spectrum needs samples of the active run, which only the simulation worker can read, so the
/// caller gives `capture`: it receives the spectrum request and returns the bounded, copied
/// samples (or why there are none). Everything else runs on the data in the request. The
/// cancellation flag is checked at the points the analyses and this function define; a result that
/// has a number that is not finite is a solve failure and is never returned.
pub fn run_analysis(
    request: &AnalysisRequest,
    capture: impl FnOnce(SpectrumRequest<'_>) -> Result<SpectrumCapture, AnalysisError>,
    is_cancelled: impl Fn() -> bool,
) -> Result<AnalysisResult, AnalysisError> {
    if is_cancelled() {
        return Err(AnalysisError::Cancelled);
    }

    let result = match request {
        AnalysisRequest::DcSweep {
            schematic,
            geometry,
            document_revision,
            source_id,
            signal_key,
            start_volts,
            stop_volts,
            point_count,
        } => {
            let result = dc_sweep(
                DcSweepRequest {
                    schematic,
                    geometry,
                    document_revision,
                    source_id,
                    signal_key,
                    start_volts: *start_volts,
                    stop_volts: *stop_volts,
                    point_count: count(*point_count)?,
                },
                &is_cancelled,
            )?;
            AnalysisResult::DcSweep {
                document_revision: result.document_revision,
                source_id: result.source_id,
                signal_key: result.signal_key,
                unit: result.unit,
                points: result
                    .points
                    .into_iter()
                    .map(|point| DcSweepPointDto {
                        source_volts: point.source_volts,
                        reading: point.reading,
                    })
                    .collect(),
            }
        }
        AnalysisRequest::FrequencyResponse {
            schematic,
            geometry,
            document_revision,
            source_id,
            output,
            start_hz,
            stop_hz,
            point_count,
            spacing,
        } => {
            let positive = PinRef {
                component_id: output[0].component_id.clone(),
                pin_id: output[0].pin_id,
            };
            let negative = PinRef {
                component_id: output[1].component_id.clone(),
                pin_id: output[1].pin_id,
            };
            let result = frequency_response(
                FrequencyResponseRequest {
                    schematic,
                    geometry,
                    document_revision,
                    source_id,
                    output: [&positive, &negative],
                    start_hz: *start_hz,
                    stop_hz: *stop_hz,
                    point_count: count(*point_count)?,
                    spacing: match spacing {
                        AnalysisSpacing::Linear => FrequencySpacing::Linear,
                        AnalysisSpacing::Logarithmic => FrequencySpacing::Logarithmic,
                    },
                },
                &is_cancelled,
            )?;
            AnalysisResult::FrequencyResponse {
                document_revision: result.document_revision,
                source_id: result.source_id,
                output: [terminal(&result.output[0]), terminal(&result.output[1])],
                points: result
                    .points
                    .into_iter()
                    .map(|point| FrequencyResponsePointDto {
                        frequency_hz: point.frequency_hz,
                        magnitude: point.magnitude,
                        phase_degrees: point.phase_degrees,
                    })
                    .collect(),
            }
        }
        AnalysisRequest::Spectrum {
            run_id,
            layout_version,
            document_revision,
            signal_key,
            end_step,
            sample_count,
        } => {
            // Only plain decimal digits are a step: no sign, space or exponent.
            if end_step.is_empty()
                || end_step.len() > 20
                || !end_step.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(invalid());
            }
            let end_step: u64 = end_step.parse().map_err(|_| invalid())?;
            let captured = capture(SpectrumRequest {
                run_id,
                layout_version: *layout_version,
                document_revision,
                signal_key,
                end_step,
                sample_count: count(*sample_count)?,
            })?;
            if is_cancelled() {
                return Err(AnalysisError::Cancelled);
            }
            let result = harmonic_spectrum(&captured, &is_cancelled)?;
            AnalysisResult::Spectrum {
                run_id: result.run_id,
                layout_version: result.layout_version,
                document_revision: result.document_revision,
                signal_key: result.signal_key,
                first_step: result.first_step.to_string(),
                end_step: result.end_step.to_string(),
                bin_spacing_hz: result.bin_spacing_hz,
                unit: result.unit,
                bins: result
                    .bins
                    .into_iter()
                    .map(|bin| SpectrumBinDto {
                        frequency_hz: bin.frequency_hz,
                        peak_amplitude: bin.peak_amplitude,
                    })
                    .collect(),
            }
        }
    };

    if let Some(point_index) = result.first_non_finite_point() {
        return Err(AnalysisError::SolveFailed {
            point_index,
            diagnostics: vec![Diagnostic::error(
                DiagnosticCode::NonFiniteState,
                Vec::new(),
            )],
        });
    }
    // The last check is the one that matters: a cancel accepted before publication discards it.
    if is_cancelled() {
        return Err(AnalysisError::Cancelled);
    }
    Ok(result)
}
