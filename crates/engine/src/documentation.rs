//! Stage 12 drawn documentation inputs and their bounded validation
//! (docs/contracts/stage-12-cabinet-documentation.md section 2). Documentation is saved with the
//! project but never changes a compile, a run or any result: it holds designators, wire numbers,
//! terminals, notes, a title block and sheets, all keyed by the stable IDs of the drawing. Nothing
//! derived is stored here.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Diagnostic, DiagnosticCode, DiagnosticReference, GridPoint, Schematic, StableId};

/// At most this many designators, wire numbers and terminals each (contract section 2.5).
pub const MAX_DESIGNATORS: usize = 4_096;
pub const MAX_WIRE_NUMBERS: usize = 4_096;
pub const MAX_TERMINALS: usize = 4_096;
pub const MAX_NOTES: usize = 200;
pub const MAX_SHEETS: usize = 32;

/// The identification letters a designator may start with (section 3.1). `I` and `O` are never
/// used, and `D`, `J`, `L`, `N`, `Y` and `Z` are reserved by the standard for later use.
pub const DESIGNATOR_FIRST_LETTERS: &str = "ABCEFGHKMPQRSTUVWX";

const NOTE_MAX_CHARS: usize = 500;
const NOTE_MAX_LINES: usize = 10;

/// A sheet is a frame of fixed size over the one drawing; one grid unit is 5 mm
/// (section 7.2): A4 landscape is 59 by 42 grid units, A3 landscape 84 by 59.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "SheetSize.ts")]
pub enum SheetSize {
    A4,
    A3,
}

impl SheetSize {
    /// The frame's width and height in grid units.
    pub const fn grid_units(self) -> (i64, i64) {
        match self {
            Self::A4 => (59, 42),
            Self::A3 => (84, 59),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "DesignatorEntry.ts")]
pub struct DesignatorEntry {
    pub component_id: StableId,
    pub designator: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "WireNumberEntry.ts")]
pub struct WireNumberEntry {
    pub wire_id: StableId,
    pub number: String,
}

/// A labelled marker on a pin or a wire, drawn as `strip:label`. It is not an electrical element.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "TerminalSpec.ts")]
pub struct TerminalSpec {
    pub id: StableId,
    pub strip: String,
    pub label: String,
    pub position: GridPoint,
}

/// Free text on the drawing.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "NoteSpec.ts")]
pub struct NoteSpec {
    pub id: StableId,
    pub position: GridPoint,
    pub text: String,
}

/// The one title block of a project; every field is text the user typed.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "TitleBlock.ts")]
pub struct TitleBlock {
    pub title: String,
    pub drawing_number: String,
    pub revision: String,
    pub author: String,
    pub company: String,
    pub date: String,
}

/// A frame over the one drawing: its top-left corner in grid units and its paper size.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "SheetSpec.ts")]
pub struct SheetSpec {
    pub id: StableId,
    pub name: String,
    pub size: SheetSize,
    pub origin: GridPoint,
}

/// Everything a project documents beyond the electrical drawing (docs/contracts/
/// stage-12-cabinet-documentation.md section 2.1). The entries are arrays, not maps, so that a
/// repeated key is a validation error and cannot be overwritten silently by the decoder.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "ProjectDocumentation.ts")]
pub struct ProjectDocumentation {
    pub designators: Vec<DesignatorEntry>,
    pub wire_numbers: Vec<WireNumberEntry>,
    pub terminals: Vec<TerminalSpec>,
    pub notes: Vec<NoteSpec>,
    pub title_block: TitleBlock,
    pub sheets: Vec<SheetSpec>,
}

impl ProjectDocumentation {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// True when nothing but designators is documented. Opening such a project while an
    /// assignment is open drops the designators instead of refusing it (section 2.4).
    pub fn holds_only_designators(&self) -> bool {
        Self {
            designators: Vec::new(),
            ..self.clone()
        }
        .is_empty()
    }
}

/// Whether `text` is a designator: one of the identification letters, up to two more capital
/// letters (never `I` or `O`), and a number from 1 to 9999 without a leading zero.
pub fn is_valid_designator(text: &str) -> bool {
    let bytes = text.as_bytes();
    let Some((&first, rest)) = bytes.split_first() else {
        return false;
    };
    if !DESIGNATOR_FIRST_LETTERS.as_bytes().contains(&first) {
        return false;
    }
    let letters = rest
        .iter()
        .take_while(|byte| byte.is_ascii_uppercase())
        .count();
    let (extra, digits) = rest.split_at(letters);

    letters <= 2
        && extra.iter().all(|byte| *byte != b'I' && *byte != b'O')
        && (1..=4).contains(&digits.len())
        && digits.iter().all(u8::is_ascii_digit)
        && digits[0] != b'0'
}

/// Whether `text` is a one to `max` character label of letters, digits and `+ . _ -`.
fn is_label(text: &str, max: usize) -> bool {
    (1..=max).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"+._-".contains(&byte))
}

fn is_strip(text: &str) -> bool {
    let bytes = text.as_bytes();
    (1..=8).contains(&bytes.len())
        && bytes[0].is_ascii_uppercase()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

fn plain_text(text: &str, max_chars: usize) -> bool {
    text.chars().count() <= max_chars && !text.chars().any(char::is_control)
}

fn is_note_text(text: &str) -> bool {
    let chars = text.chars().count();

    (1..=NOTE_MAX_CHARS).contains(&chars)
        && text.lines().count() <= NOTE_MAX_LINES
        && !text.chars().any(|c| c.is_control() && c != '\n')
}

fn sorted(mut references: Vec<DiagnosticReference>) -> Vec<DiagnosticReference> {
    references.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    references
}

impl Schematic {
    /// The documentation part of the document validation: bounded sizes first (a document over
    /// a limit is rejected whole, with no individual references), then each entry, then the
    /// relations between entries. Terminal attachment needs the drawing's geometry and is
    /// checked with it (issue #192).
    pub(crate) fn validate_documentation(&self) -> Vec<Diagnostic> {
        let documentation = &self.documentation;
        if documentation.designators.len() > MAX_DESIGNATORS
            || documentation.wire_numbers.len() > MAX_WIRE_NUMBERS
            || documentation.terminals.len() > MAX_TERMINALS
            || documentation.notes.len() > MAX_NOTES
            || documentation.sheets.len() > MAX_SHEETS
        {
            return vec![Diagnostic::error(
                DiagnosticCode::DocumentationLimitExceeded,
                Vec::new(),
            )];
        }

        let mut diagnostics = Vec::new();
        let components: HashSet<&str> = self.components.iter().map(|c| c.id().as_str()).collect();
        let wires: HashSet<&str> = self.wires.iter().map(|wire| wire.id.as_str()).collect();

        let mut designated = HashSet::new();
        let mut by_designator = BTreeMap::<&str, Vec<&StableId>>::new();
        for entry in &documentation.designators {
            let reference = DiagnosticReference::Component {
                component_id: entry.component_id.clone(),
            };
            if !components.contains(entry.component_id.as_str()) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::UnknownDocumentationTarget,
                    vec![reference],
                ));
                continue;
            }
            if !designated.insert(entry.component_id.as_str()) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::DuplicateId,
                    vec![reference.clone(), reference.clone()],
                ));
            }
            if !is_valid_designator(&entry.designator) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidDesignator,
                    vec![reference],
                ));
            }
            by_designator
                .entry(&entry.designator)
                .or_default()
                .push(&entry.component_id);
        }
        for ids in by_designator.values().filter(|ids| ids.len() > 1) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::DuplicateDesignator,
                sorted(
                    ids.iter()
                        .map(|id| DiagnosticReference::Component {
                            component_id: (*id).clone(),
                        })
                        .collect(),
                ),
            ));
        }

        let mut numbered = HashSet::new();
        for entry in &documentation.wire_numbers {
            let reference = DiagnosticReference::Wire {
                wire_id: entry.wire_id.clone(),
            };
            if !wires.contains(entry.wire_id.as_str()) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::UnknownDocumentationTarget,
                    vec![reference],
                ));
                continue;
            }
            if !numbered.insert(entry.wire_id.as_str()) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::DuplicateId,
                    vec![reference.clone(), reference.clone()],
                ));
            }
            if !is_label(&entry.number, 16) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidWireNumber,
                    vec![reference],
                ));
            }
        }

        let mut named = BTreeMap::<(&str, &str), Vec<&TerminalSpec>>::new();
        let mut positioned = BTreeMap::<(i32, i32), Vec<&TerminalSpec>>::new();
        for terminal in &documentation.terminals {
            let reference = DiagnosticReference::Terminal {
                terminal_id: terminal.id.clone(),
            };
            if !is_strip(&terminal.strip) || !is_label(&terminal.label, 8) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidTerminal,
                    vec![reference],
                ));
            }
            named
                .entry((&terminal.strip, &terminal.label))
                .or_default()
                .push(terminal);
            positioned
                .entry((terminal.position.x, terminal.position.y))
                .or_default()
                .push(terminal);
        }
        for terminals in named
            .values()
            .chain(positioned.values())
            .filter(|terminals| terminals.len() > 1)
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::DuplicateTerminal,
                sorted(
                    terminals
                        .iter()
                        .map(|terminal| DiagnosticReference::Terminal {
                            terminal_id: terminal.id.clone(),
                        })
                        .collect(),
                ),
            ));
        }

        for note in &documentation.notes {
            if !is_note_text(&note.text) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidNote,
                    vec![DiagnosticReference::Note {
                        note_id: note.id.clone(),
                    }],
                ));
            }
        }

        let block = &documentation.title_block;
        if !plain_text(&block.title, 100)
            || !plain_text(&block.drawing_number, 32)
            || !plain_text(&block.revision, 16)
            || !plain_text(&block.author, 64)
            || !plain_text(&block.company, 64)
            || !plain_text(&block.date, 32)
        {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::InvalidTitleBlock,
                Vec::new(),
            ));
        }

        for sheet in &documentation.sheets {
            let name = sheet.name.chars().count();
            if !(1..=32).contains(&name) || !plain_text(&sheet.name, 32) {
                diagnostics.push(Diagnostic::error(
                    DiagnosticCode::InvalidSheet,
                    vec![DiagnosticReference::Sheet {
                        sheet_id: sheet.id.clone(),
                    }],
                ));
            }
        }
        for (index, first) in documentation.sheets.iter().enumerate() {
            for second in &documentation.sheets[index + 1..] {
                if sheets_overlap(first, second) {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::OverlappingSheets,
                        sorted(vec![
                            DiagnosticReference::Sheet {
                                sheet_id: first.id.clone(),
                            },
                            DiagnosticReference::Sheet {
                                sheet_id: second.id.clone(),
                            },
                        ]),
                    ));
                }
            }
        }

        diagnostics
    }
}

/// Whether two frames share any area (frames that only touch along an edge do not overlap).
fn sheets_overlap(a: &SheetSpec, b: &SheetSpec) -> bool {
    let (aw, ah) = a.size.grid_units();
    let (bw, bh) = b.size.grid_units();
    let (ax, ay) = (i64::from(a.origin.x), i64::from(a.origin.y));
    let (bx, by) = (i64::from(b.origin.x), i64::from(b.origin.y));

    ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah
}

/// Check wire labels against the same conductor connectivity used by the solver.
pub fn documentation_diagnostics(
    schematic: &Schematic,
    geometry: &crate::GeometryEnvelope,
) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
    let netlist =
        crate::netlist::derive_document_connectivity(schematic, geometry, &HashSet::new())?;
    let numbers: HashMap<_, _> = schematic
        .documentation
        .wire_numbers
        .iter()
        .map(|entry| (&entry.wire_id, entry.number.as_str()))
        .collect();
    let mut by_number = BTreeMap::<&str, BTreeMap<usize, Vec<DiagnosticReference>>>::new();
    let mut warnings = Vec::new();
    for node in &netlist.nodes {
        let mut labels = HashSet::new();
        let mut references = Vec::new();
        for wire in &node.wire_ids {
            let Some(number) = numbers.get(wire) else {
                continue;
            };
            let reference = DiagnosticReference::Wire {
                wire_id: wire.clone(),
            };
            labels.insert(*number);
            references.push(reference.clone());
            by_number
                .entry(number)
                .or_default()
                .entry(node.index)
                .or_default()
                .push(reference);
        }
        if labels.len() > 1 {
            warnings.push(Diagnostic::warning(
                DiagnosticCode::NetHasTwoWireNumbers,
                references.clone(),
            ));
        }
        if node.pins.is_empty() && !references.is_empty() {
            warnings.push(Diagnostic::warning(
                DiagnosticCode::WireNumberOnUnconnectedWire,
                references,
            ));
        }
    }
    for nodes in by_number.into_values().filter(|nodes| nodes.len() > 1) {
        warnings.push(Diagnostic::warning(
            DiagnosticCode::WireNumberOnDifferentNets,
            nodes.into_values().flatten().collect(),
        ));
    }
    Ok(warnings)
}

/// Return the new entries for nets whose wires currently have no number.
pub fn number_wires(
    schematic: &Schematic,
    geometry: &crate::GeometryEnvelope,
) -> Result<Vec<WireNumberEntry>, Vec<Diagnostic>> {
    let netlist =
        crate::netlist::derive_document_connectivity(schematic, geometry, &HashSet::new())?;
    let existing = &schematic.documentation.wire_numbers;
    let numbered: HashSet<_> = existing.iter().map(|entry| &entry.wire_id).collect();
    let used: HashSet<_> = existing.iter().map(|entry| entry.number.as_str()).collect();
    let mut origins: HashMap<_, _> = schematic
        .wires
        .iter()
        .map(|wire| {
            (
                &wire.id,
                wire.points
                    .iter()
                    .map(|p| (p.y, p.x))
                    .min()
                    .expect("validated wire"),
            )
        })
        .collect();
    origins.extend(schematic.rails.iter().map(|rail| {
        (
            &rail.id,
            (rail.start.y, rail.start.x).min((rail.end.y, rail.end.x)),
        )
    }));
    let mut nodes: Vec<_> = netlist
        .nodes
        .iter()
        .filter(|node| {
            !node.wire_ids.is_empty() && node.wire_ids.iter().all(|wire| !numbered.contains(wire))
        })
        .collect();
    nodes.sort_by_key(|node| {
        (
            node.wire_ids
                .iter()
                .chain(&node.rail_ids)
                .map(|id| origins[id])
                .min(),
            node.index,
        )
    });
    if existing.len() + nodes.iter().map(|node| node.wire_ids.len()).sum::<usize>()
        > MAX_WIRE_NUMBERS
    {
        return Err(vec![Diagnostic::error(
            DiagnosticCode::DocumentationLimitExceeded,
            Vec::new(),
        )]);
    }
    let mut serial = 1;
    let mut result = Vec::new();
    for node in nodes {
        let number = loop {
            let candidate = serial.to_string();
            serial += 1;
            if !used.contains(candidate.as_str()) {
                break candidate;
            }
        };
        result.extend(node.wire_ids.iter().map(|wire_id| WireNumberEntry {
            wire_id: wire_id.clone(),
            number: number.clone(),
        }));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbering_uses_conductor_geometry_and_reports_conflicting_labels() {
        let id = |n| StableId::parse(format!("00000000-0000-4000-8000-{n:012}")).unwrap();
        let wire = |n, y, x| crate::WireSpec {
            id: id(n),
            points: vec![GridPoint { x, y }, GridPoint { x: x + 2, y }],
        };
        let mut schematic = Schematic {
            schema_version: 3,
            components: vec![],
            wires: vec![wire(1, 10, 0), wire(2, 10, 2), wire(3, 5, 0), wire(4, 2, 0)],
            junctions: vec![],
            reference_markers: vec![],
            rails: vec![],
            documentation: ProjectDocumentation::default(),
        };
        let geometry = crate::GeometryEnvelope { components: vec![] };
        let entry = |n, number: &str| WireNumberEntry {
            wire_id: id(n),
            number: number.to_owned(),
        };
        schematic.documentation.wire_numbers.push(entry(4, "1"));
        let expected = vec![entry(3, "2"), entry(1, "3"), entry(2, "3")];
        assert_eq!(number_wires(&schematic, &geometry).unwrap(), expected);
        schematic.wires.reverse();
        assert_eq!(number_wires(&schematic, &geometry).unwrap(), expected);
        schematic
            .documentation
            .wire_numbers
            .extend([entry(1, "1"), entry(2, "2")]);
        let warnings = documentation_diagnostics(&schematic, &geometry).unwrap();
        for code in [
            DiagnosticCode::WireNumberOnDifferentNets,
            DiagnosticCode::NetHasTwoWireNumbers,
            DiagnosticCode::WireNumberOnUnconnectedWire,
        ] {
            assert!(warnings.iter().any(|diagnostic| diagnostic.code == code));
        }
    }

    #[test]
    fn designators_follow_the_contract_grammar() {
        for valid in [
            "Q1", "K2", "F1", "R10", "QF1", "QFA12", "M9999", "X1", "G7", "P1",
        ] {
            assert!(is_valid_designator(valid), "{valid}");
        }
        for invalid in [
            "", "1", "Q", "Q0", "Q01", "Q10000", "q1", "D1", "L1", "N1", "Y1", "Z1", "I1", "O1",
            "QI1", "QO1", "QFAB1", "Q1A", "Q 1", "Q-1", "QF", "Ä1",
        ] {
            assert!(!is_valid_designator(invalid), "{invalid}");
        }
    }
}
