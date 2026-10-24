use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Integer world-grid coordinates shared by the schematic and editor layers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "GridPoint.ts")]
pub struct GridPoint {
    pub x: i32,
    pub y: i32,
}
