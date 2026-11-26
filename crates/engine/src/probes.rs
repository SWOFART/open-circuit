//! Transport types for read-only measurements at a committed public endpoint.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Diagnostic, FrameMetadata, GridPoint, PinId, StableId, Unit};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ConductorSpan {
    pub conductor_id: StableId,
    pub start: GridPoint,
    pub end: GridPoint,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export)]
pub enum ProbeTarget {
    Voltage {
        positive: GridPoint,
        negative: GridPoint,
    },
    TerminalCurrent {
        component_id: StableId,
        pin_id: PinId,
    },
    ConductorCurrent {
        span: ConductorSpan,
    },
    Resistance {
        first: GridPoint,
        second: GridPoint,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ProbeRequest {
    pub run_id: StableId,
    pub document_revision: String,
    pub frame_sequence: u64,
    pub target: ProbeTarget,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ProbeResult {
    Reading {
        metadata: FrameMetadata,
        value: f64,
        unit: Unit,
    },
    OpenCircuit {
        metadata: FrameMetadata,
    },
    Unavailable {
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        metadata: Option<FrameMetadata>,
        diagnostic: Diagnostic,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ConductorSpanRequest {
    pub run_id: StableId,
    pub document_revision: String,
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConductorSpanPage {
    pub spans: Vec<ConductorSpan>,
    pub next_cursor: Option<String>,
}
