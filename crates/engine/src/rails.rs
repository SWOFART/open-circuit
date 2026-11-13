//! Stage 8 drawn rail inputs and bounded validation; electrical normalization is #131.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Diagnostic, DiagnosticCode, DiagnosticReference, GridPoint, Schematic, StableId};

pub const MAX_RAIL_COUNT: usize = 256;
pub const MAX_RAIL_CONTACT_WORK: usize = 1_000_000;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "RailSpec.ts")]
pub struct RailSpec {
    pub id: StableId,
    pub name: String,
    pub start: GridPoint,
    pub end: GridPoint,
}

impl RailSpec {
    pub(crate) fn reference(&self) -> DiagnosticReference {
        DiagnosticReference::Rail {
            rail_id: self.id.clone(),
        }
    }
}

impl Schematic {
    pub(crate) fn rail_limit_diagnostic(&self) -> Diagnostic {
        let mut references = if self.rails.len() > MAX_RAIL_COUNT {
            Vec::new()
        } else {
            self.rails
                .iter()
                .map(RailSpec::reference)
                .collect::<Vec<_>>()
        };
        references.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
        Diagnostic::error(DiagnosticCode::RailLimitExceeded, references)
    }

    pub(crate) fn rail_work_is_bounded(&self, pins: usize) -> bool {
        let count = self.rails.len();
        if count > MAX_RAIL_COUNT {
            return false;
        }
        if count == 0 {
            return true;
        }
        let segments = self.wires.iter().try_fold(0usize, |sum, w| {
            sum.checked_add(w.points.len().saturating_sub(1))
        });
        let work = segments
            .and_then(|s| s.checked_add(pins))
            .and_then(|s| s.checked_add(self.junctions.len()))
            .and_then(|s| s.checked_add(self.reference_markers.len()))
            .and_then(|s| count.checked_mul(s))
            .and_then(|s| {
                count
                    .checked_mul(count - 1)
                    .and_then(|pairs| s.checked_add(pairs / 2))
            });
        work.is_some_and(|work| work <= MAX_RAIL_CONTACT_WORK)
    }

    pub(crate) fn validate_rails(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if !self.rail_work_is_bounded(0) {
            return vec![self.rail_limit_diagnostic()];
        }
        let mut names = BTreeMap::<&str, Vec<&RailSpec>>::new();
        for rail in &self.rails {
            let bytes = rail.name.as_bytes();
            if !(1..=32).contains(&bytes.len())
                || !bytes.first().is_some_and(u8::is_ascii_alphanumeric)
                || !bytes
                    .iter()
                    .all(|b| b.is_ascii_alphanumeric() || b"_+-".contains(b))
            {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidRailName,
                    vec![rail.reference()],
                ));
            }
            let length = i64::from(rail.end.y) - i64::from(rail.start.y);
            if rail.start.x != rail.end.x || !(2..=1_000_000).contains(&length) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidGeometry,
                    vec![rail.reference()],
                ));
            }
            names.entry(&rail.name).or_default().push(rail);
        }
        for conflicting in names.values().filter(|rails| rails.len() > 1) {
            let mut references = conflicting
                .iter()
                .map(|r| r.reference())
                .collect::<Vec<_>>();
            references.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::DuplicateRailName,
                references,
            ));
        }
        diagnostics
    }
}
