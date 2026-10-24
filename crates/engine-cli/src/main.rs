use open_circuit_engine::{
    CompileRequest, CompileResult, Engine, ParameterId, ResultLayout, ResultQuantity,
    ResultSnapshot, SetParameterRequest, SetParameterResult, StableId,
};
use serde_json::Value;

const SCHEMATIC: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");
const GEOMETRY: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");
const EXPECTED: &str = include_str!("../../../tests/golden/stage-1-divider.expected.json");
const SOURCE_ID: &str = "00000000-0000-4000-8000-000000000001";
const RESISTOR_1_ID: &str = "00000000-0000-4000-8000-000000000002";
const RESISTOR_2_ID: &str = "00000000-0000-4000-8000-000000000003";

fn main() {
    if let Err(error) = run() {
        eprintln!("Open Circuit Engine CLI failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let schematic = serde_json::from_str(SCHEMATIC).map_err(|error| error.to_string())?;
    let geometry = serde_json::from_str(GEOMETRY).map_err(|error| error.to_string())?;
    let expected: Value = serde_json::from_str(EXPECTED).map_err(|error| error.to_string())?;
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        diagnostics,
    } = engine.compile(CompileRequest {
        schematic,
        geometry,
        document_revision: "engine-cli-divider-1".to_owned(),
    })
    else {
        return Err("canonical divider failed to compile".to_owned());
    };

    let expected_equal = &expected["equalResistors"];
    verify_fixture_result(&layout, &initial_snapshot, expected_equal)?;
    println!("Stage 1 divider at simulation step 0:");
    print_values(&layout, &initial_snapshot)?;
    for diagnostic in diagnostics {
        println!(
            "Diagnostic: {:?} ({:?})",
            diagnostic.code, diagnostic.severity
        );
    }

    let SetParameterResult::Success {
        snapshot,
        diagnostics,
    } = engine.set_parameter(SetParameterRequest {
        run_id,
        expected_document_revision: "engine-cli-divider-1".to_owned(),
        document_revision: "engine-cli-divider-2".to_owned(),
        component_id: StableId::parse(RESISTOR_1_ID).map_err(|error| error.to_string())?,
        parameter_id: ParameterId::ResistanceOhms,
        value: 3000.0,
    })
    else {
        return Err("changing R1 to 3000 ohms failed".to_owned());
    };
    verify_fixture_result(&layout, &snapshot, &expected["resistor1At3000Ohms"])?;
    println!("\nAfter changing R1 to 3000 ohms:");
    print_values(&layout, &snapshot)?;
    for diagnostic in diagnostics {
        println!(
            "Diagnostic: {:?} ({:?})",
            diagnostic.code, diagnostic.severity
        );
    }
    Ok(())
}

fn verify_fixture_result(
    layout: &ResultLayout,
    snapshot: &ResultSnapshot,
    expected: &Value,
) -> Result<(), String> {
    let source_current = value(layout, snapshot, SOURCE_ID, ResultQuantity::Current)?;
    let resistor1_voltage = value(layout, snapshot, RESISTOR_1_ID, ResultQuantity::Voltage)?;
    let resistor2_voltage = value(layout, snapshot, RESISTOR_2_ID, ResultQuantity::Voltage)?;
    verify_close(
        source_current.abs(),
        expected["sourceCurrentAmps"].as_f64().unwrap_or(f64::NAN),
        1e-9,
        "source current",
    )?;
    verify_close(
        resistor1_voltage,
        expected["resistor1VoltageVolts"]
            .as_f64()
            .unwrap_or(f64::NAN),
        1e-6,
        "R1 voltage",
    )?;
    verify_close(
        resistor2_voltage,
        expected["resistor2VoltageVolts"]
            .as_f64()
            .unwrap_or(f64::NAN),
        1e-6,
        "R2 voltage",
    )
}

fn print_values(layout: &ResultLayout, snapshot: &ResultSnapshot) -> Result<(), String> {
    let source_current = value(layout, snapshot, SOURCE_ID, ResultQuantity::Current)?;
    let resistor1_voltage = value(layout, snapshot, RESISTOR_1_ID, ResultQuantity::Voltage)?;
    let resistor2_voltage = value(layout, snapshot, RESISTOR_2_ID, ResultQuantity::Voltage)?;
    println!("  source current into pin1: {source_current:.12} A");
    println!("  R1 voltage (pin1 - pin2): {resistor1_voltage:.12} V");
    println!("  R2 voltage (pin1 - pin2): {resistor2_voltage:.12} V");
    Ok(())
}

fn value(
    layout: &ResultLayout,
    snapshot: &ResultSnapshot,
    component_id: &str,
    quantity: ResultQuantity,
) -> Result<f64, String> {
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.component_id.as_str() == component_id && slot.quantity == quantity)
        .ok_or_else(|| format!("missing {quantity:?} result for {component_id}"))?;
    snapshot
        .values
        .get(slot.offset as usize)
        .copied()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("invalid result value for {component_id}"))
}

fn verify_close(actual: f64, expected: f64, tolerance: f64, name: &str) -> Result<(), String> {
    if !actual.is_finite() || !expected.is_finite() || (actual - expected).abs() > tolerance {
        return Err(format!(
            "{name} {actual:.12} is outside expected {expected:.12} ± {tolerance}"
        ));
    }
    Ok(())
}
