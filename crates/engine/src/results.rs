use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Diagnostic, GeometryEnvelope, ParameterId, Schematic, StableId, Unit};

pub const SIMULATION_TIMESTEP_SECONDS: f64 = 0.001;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
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
    PositiveRotationWithPositivePin1Current,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "RelayContactState.ts")]
pub enum RelayContactState {
    Released,
    Picked,
}

/// Runtime-only discrete device state carried atomically with its result snapshot.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "DeviceState.ts")]
pub enum DeviceState {
    Relay {
        component_id: StableId,
        state: RelayContactState,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        last_transition_time_seconds: Option<f64>,
    },
    Fuse {
        component_id: StableId,
        state: FuseState,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        last_transition_time_seconds: Option<f64>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "FuseState.ts")]
pub enum FuseState {
    Intact,
    Tripped,
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
    #[serde(default)]
    pub states: Vec<DeviceState>,
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
        internal_substeps: u32,
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
    pub expected_document_revision: String,
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
        internal_substeps: u32,
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

/// A bounded request for selected result-layout signals from an active run.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "TraceRequest.ts")]
pub struct TraceRequest {
    pub run_id: StableId,
    pub layout_version: u32,
    pub signal_keys: Vec<String>,
    pub query: TraceQuery,
    pub max_points: u32,
}

/// Selects a stored range or samples newer than the caller's previous cursor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "TraceQuery.ts")]
pub enum TraceQuery {
    History {
        #[ts(type = "number")]
        start_step: u64,
        #[ts(type = "number")]
        end_step: u64,
    },
    NewSamples {
        #[ts(type = "number")]
        after_step: u64,
        after_document_revision: String,
    },
}

/// The identity and SI unit of one selected result-layout signal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "TraceSignal.ts")]
pub struct TraceSignal {
    pub key: String,
    pub unit: Unit,
}

/// One decimated trace point. Each min/max vector follows `TraceBatch::signals` order.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "TracePoint.ts")]
pub struct TracePoint {
    pub simulation_step: u64,
    pub minimum_values: Vec<f64>,
    pub maximum_values: Vec<f64>,
}

/// Run and retention metadata attached to a bounded trace response.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "TraceBatchMetadata.ts")]
pub struct TraceBatchMetadata {
    pub run_id: StableId,
    pub document_revision: String,
    pub layout_version: u32,
    pub sample_interval_seconds: f64,
    pub history_samples: u32,
    pub oldest_sample_step: Option<u64>,
    pub latest_sample_step: Option<u64>,
    pub first_sample_step: Option<u64>,
    pub last_sample_step: Option<u64>,
    pub sample_count: u32,
    pub missed_samples: u64,
    pub replaced: bool,
}

/// Transport-independent trace data returned by the Engine.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "TraceBatch.ts")]
pub struct TraceBatch {
    pub metadata: TraceBatchMetadata,
    pub signals: Vec<TraceSignal>,
    pub points: Vec<TracePoint>,
}
