//! The Engine's connectivity for drawings made with the Stage 2 editor tools.
//!
//! `tests/golden/stage-2-connectivity.json` lists edited drawings of the Stage 1
//! divider components together with the connectivity the Engine must derive. The
//! editor's own tests read the same file (src/editor/connectivity.test.ts), so the
//! drawings the editor produces and the rules it uses as hints are pinned to what
//! the Engine actually does (docs/contracts/stage-2-editor.md section 3.3).

use std::collections::BTreeSet;

use open_circuit_engine::{DiagnosticCode, GeometryEnvelope, Schematic, netlist::derive_netlist};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../tests/golden/stage-2-connectivity.json");
const SCHEMATIC: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");
const GEOMETRY: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("Stage 2 connectivity fixture should parse")
}

fn geometry() -> GeometryEnvelope {
    serde_json::from_str(GEOMETRY).expect("divider geometry should parse")
}

/// The divider's components with the case's wires, junctions, and markers.
fn schematic_for(case: &Value) -> Schematic {
    let mut schematic: Value = serde_json::from_str(SCHEMATIC).expect("divider should parse");
    for field in ["wires", "junctions", "referenceMarkers"] {
        schematic[field] = case[field].clone();
    }
    serde_json::from_value(schematic).expect("case schematic should deserialize")
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("expected an array")
        .iter()
        .map(|item| item.as_str().expect("expected a string").to_owned())
        .collect()
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

fn codes(diagnostics: &[open_circuit_engine::Diagnostic]) -> BTreeSet<String> {
    diagnostics
        .iter()
        .map(|diagnostic| format!("{:?}", diagnostic.code))
        .collect()
}

#[test]
fn engine_derives_the_expected_connectivity_for_each_edited_drawing() {
    let fixture = fixture();
    let names = fixture["componentNames"]
        .as_object()
        .expect("component names");
    let geometry = geometry();
    let cases = fixture["cases"].as_array().expect("cases");
    assert!(cases.len() >= 10, "the fixture lost cases");

    for case in cases {
        let name = case["name"].as_str().expect("case name");
        let expected = &case["expected"];
        let schematic = schematic_for(case);
        let expected_codes = strings(&expected["diagnostics"])
            .into_iter()
            .collect::<BTreeSet<_>>();

        match (
            expected["outcome"].as_str(),
            derive_netlist(&schematic, &geometry),
        ) {
            (Some("netlist"), Ok(netlist)) => {
                assert_eq!(
                    codes(&netlist.diagnostics),
                    expected_codes,
                    "{name}: diagnostics"
                );

                let pin_name = |pin: &open_circuit_engine::netlist::PinRef| {
                    format!(
                        "{}.{}",
                        names[pin.component_id.as_str()].as_str().expect("name"),
                        pin.pin_id.as_str()
                    )
                };

                // Every pin lies in exactly one node, so the joined groups plus
                // the lone pins account for all six pins of the three components.
                let total_pins: usize = netlist.nodes.iter().map(|node| node.pins.len()).sum();
                assert_eq!(total_pins, 6, "{name}: every pin belongs to one node");

                let mut joined = netlist
                    .nodes
                    .iter()
                    .filter(|node| node.pins.len() > 1)
                    .map(|node| sorted(node.pins.iter().map(&pin_name).collect()))
                    .collect::<Vec<_>>();
                joined.sort();

                let mut wanted = expected["joinedPins"]
                    .as_array()
                    .expect("joined pins")
                    .iter()
                    .map(|group| sorted(strings(group)))
                    .collect::<Vec<_>>();
                wanted.sort();

                assert_eq!(joined, wanted, "{name}: joined pins");
            }
            (Some("rejected"), Err(diagnostics)) => {
                assert_eq!(codes(&diagnostics), expected_codes, "{name}: diagnostics");
            }
            (outcome, result) => panic!(
                "{name}: expected {outcome:?} but the Engine returned {}",
                if result.is_ok() {
                    "a netlist"
                } else {
                    "diagnostics"
                }
            ),
        }
    }
}

#[test]
fn a_detached_marker_is_an_input_diagnostic_and_never_an_implicit_wire() {
    let fixture = fixture();
    let case = fixture["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["name"] == "detached-reference-marker")
        .expect("fixture case");

    let diagnostics = derive_netlist(&schematic_for(case), &geometry())
        .expect_err("a detached reference marker must be rejected");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::DetachedReferenceMarker)
    );
}
