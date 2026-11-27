use std::cell::Cell;

use open_circuit_engine::{
    GeometryEnvelope, Schematic, Unit,
    engine::analyses::{AnalysisError, DcSweepRequest, dc_sweep},
};

#[test]
fn divider_sweep_is_bounded_ordered_and_cancellable() {
    let schematic: Schematic = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-1-divider.schematic.json"
    ))
    .unwrap();
    let geometry: GeometryEnvelope = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-1-divider.geometry.json"
    ))
    .unwrap();
    let source_id = schematic.components[0].id();
    let signal_key = format!("{}:voltage", schematic.components[2].id().as_str());
    let request = DcSweepRequest {
        schematic: &schematic,
        geometry: &geometry,
        document_revision: "divider-1",
        source_id,
        signal_key: &signal_key,
        start_volts: 1000.0,
        stop_volts: -1000.0,
        point_count: 257,
    };
    let result = dc_sweep(DcSweepRequest { ..request }, || false).unwrap();
    assert_eq!(result.document_revision, "divider-1");
    assert_eq!(result.unit, Unit::Volt);
    assert_eq!(result.points.len(), 257);
    for (index, point) in result.points.iter().enumerate() {
        let volts = 1000.0 - 2000.0 * index as f64 / 256.0;
        assert_eq!(point.source_volts, volts);
        let expected = volts * 1000.0 / 2000.1;
        assert!((point.reading - expected).abs() <= 1e-8 + 1e-4 * expected.abs());
    }
    for (point_count, start_volts, stop_volts) in [
        (1, -1.0, 1.0),
        (258, -1.0, 1.0),
        (2, -1001.0, 1.0),
        (2, 1.0, 1.0),
        (2, f64::NAN, 1.0),
    ] {
        assert!(matches!(
            dc_sweep(
                DcSweepRequest {
                    point_count,
                    start_volts,
                    stop_volts,
                    ..request
                },
                || false
            ),
            Err(AnalysisError::InvalidRequest(_))
        ));
    }
    let checks = Cell::new(0);
    assert_eq!(
        dc_sweep(request, || {
            checks.set(checks.get() + 1);
            checks.get() == 3
        }),
        Err(AnalysisError::Cancelled)
    );
}
