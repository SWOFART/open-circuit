use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Diagnostic, GeometryEnvelope, ParameterId, Schematic, StableId, Unit};

pub const SIMULATION_TIMESTEP_SECONDS: f64 = 0.001;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ResultQuantity.ts")]
pub enum ResultQuantity {
    Voltage,
    Current,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ResultSignConvention.ts")]
pub enum ResultSignConvention {
    VoltagePin1MinusPin2,
    CurrentIntoPin1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ResultSlot.ts")]
pub struct ResultSlot {
    pub key: String,
    pub component_id: StableId,
    pub quantity: ResultQuantity,
    pub unit: Unit,
    pub sign_convention: ResultSignConvention,
    pub offset: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ResultLayout.ts")]
pub struct ResultLayout {
    pub layout_version: u32,
    pub value_count: u32,
    pub slots: Vec<ResultSlot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "FrameMetadata.ts")]
pub struct FrameMetadata {
    pub run_id: StableId,
    pub document_revision: String,
    pub layout_version: u32,
    pub simulation_step: u64,
    pub simulation_time_seconds: f64,
    pub frame_sequence: u64,
}

/// Values are ordered by `ResultLayout::slots`; the Tauri bridge chooses the byte transport.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ResultSnapshot.ts")]
pub struct ResultSnapshot {
    pub metadata: FrameMetadata,
    pub values: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "CompileRequest.ts")]
pub struct CompileRequest {
    pub schematic: Schematic,
    pub geometry: GeometryEnvelope,
    pub document_revision: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(
    tag = "status",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "CompileResult.ts")]
pub enum CompileResult {
    Success {
        run_id: StableId,
        layout: ResultLayout,
        initial_snapshot: ResultSnapshot,
        diagnostics: Vec<Diagnostic>,
    },
    Failure {
        diagnostics: Vec<Diagnostic>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "SetParameterRequest.ts")]
pub struct SetParameterRequest {
    pub run_id: StableId,
    pub document_revision: String,
    pub component_id: StableId,
    pub parameter_id: ParameterId,
    pub value: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(
    tag = "status",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "SetParameterResult.ts")]
pub enum SetParameterResult {
    Success {
        snapshot: ResultSnapshot,
        diagnostics: Vec<Diagnostic>,
    },
    Failure {
        diagnostics: Vec<Diagnostic>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "AdvanceRequest.ts")]
pub struct AdvanceRequest {
    pub run_id: StableId,
    pub step_count: u32,
}
