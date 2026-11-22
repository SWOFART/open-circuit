//! Cross-layer contract check: the committed catalog fixture is what the
//! TypeScript symbol contract test (`src/symbols/contract.test.ts`) compares
//! against, so it must always equal the authoritative Rust catalog.

use open_circuit_engine::{CategoryId, ComponentCatalog, ComponentTypeId, component_catalog};

const CATALOG_FIXTURE: &str = include_str!("../../../tests/golden/stage-1-catalog.json");

#[test]
fn catalog_fixture_matches_the_rust_catalog() {
    let fixture: ComponentCatalog = serde_json::from_str(CATALOG_FIXTURE)
        .expect("tests/golden/stage-1-catalog.json must deserialize as a ComponentCatalog");

    assert_eq!(
        fixture,
        component_catalog(),
        "update tests/golden/stage-1-catalog.json to match the Rust catalog"
    );
}

fn name_segment(key: &str) -> &str {
    key.strip_prefix("component.")
        .and_then(|rest| rest.strip_suffix(".name"))
        .unwrap_or_else(|| panic!("`{key}` must look like component.<type>.name"))
}

#[test]
fn every_part_has_a_display_name_key_a_category_and_keyword_keys() {
    let catalog = component_catalog();
    for entry in &catalog.component_types {
        let segment = name_segment(&entry.display_name_key);
        assert!(!segment.is_empty(), "{:?}", entry.type_id);
        assert!(
            catalog.categories.contains(&entry.category),
            "{:?} has category {:?} missing from the listed categories",
            entry.type_id,
            entry.category
        );
        assert_eq!(
            entry.search_keyword_keys,
            vec![format!("component.{segment}.keywords")],
            "{:?}",
            entry.type_id
        );
    }
}

#[test]
fn categories_are_listed_once_in_display_order_and_never_empty() {
    let catalog = component_catalog();
    assert_eq!(
        catalog.categories,
        [
            CategoryId::Sources,
            CategoryId::Passive,
            CategoryId::Protection,
            CategoryId::Switching,
            CategoryId::Measurement,
            CategoryId::MotorsAndLoads,
            CategoryId::Semiconductors,
        ]
    );
    for category in &catalog.categories {
        assert!(
            catalog
                .component_types
                .iter()
                .any(|entry| entry.category == *category),
            "{category:?} is listed but has no part"
        );
    }
}

#[test]
fn parts_are_assigned_to_the_categories_of_the_stage_11_contract() {
    use ComponentTypeId::*;
    let expected = [
        (DcVoltageSource, CategoryId::Sources),
        (AcVoltageSource, CategoryId::Sources),
        (ThreePhaseSourceStar, CategoryId::Sources),
        (ThreePhaseSourceDelta, CategoryId::Sources),
        (Resistor, CategoryId::Passive),
        (Capacitor, CategoryId::Passive),
        (Inductor, CategoryId::Passive),
        (Fuse, CategoryId::Protection),
        (SwitchSpst, CategoryId::Switching),
        (PushButtonNo, CategoryId::Switching),
        (PushButtonNc, CategoryId::Switching),
        (RelaySpdt, CategoryId::Switching),
        (Contactor, CategoryId::Switching),
        (ContactorThreePole, CategoryId::Switching),
        (BreakerThreePole, CategoryId::Protection),
        (ThermalOverloadThreePole, CategoryId::Protection),
        (InductionMotorThreePhase, CategoryId::MotorsAndLoads),
        (TransformerThreePhase, CategoryId::MotorsAndLoads),
        (ThreePhaseLoadStar, CategoryId::MotorsAndLoads),
        (ThreePhaseLoadDelta, CategoryId::MotorsAndLoads),
        (Voltmeter, CategoryId::Measurement),
        (Ammeter, CategoryId::Measurement),
        (AcVoltmeter, CategoryId::Measurement),
        (AcAmmeter, CategoryId::Measurement),
        (Wattmeter, CategoryId::Measurement),
        (PmdcMotor, CategoryId::MotorsAndLoads),
        (Diode, CategoryId::Semiconductors),
        (BjtNpn, CategoryId::Semiconductors),
        (BjtPnp, CategoryId::Semiconductors),
        // Stage 13 (docs/contracts/stage-13-protection-and-safety.md sections 2 and 7).
        (EarthElectrode, CategoryId::Protection),
        (ExposedPart, CategoryId::Measurement),
        (RcdTwoPole, CategoryId::Protection),
        (RcdFourPole, CategoryId::Protection),
    ];
    let catalog = component_catalog();
    assert_eq!(catalog.component_types.len(), expected.len());
    for (type_id, category) in expected {
        let entry = catalog
            .component_types
            .iter()
            .find(|entry| entry.type_id == type_id)
            .unwrap_or_else(|| panic!("{type_id:?} is missing from the catalog"));
        assert_eq!(entry.category, category, "{type_id:?}");
    }
}
