//! Cross-layer contract check: the committed catalog fixture is what the
//! TypeScript symbol contract test (`src/symbols/contract.test.ts`) compares
//! against, so it must always equal the authoritative Rust catalog.

use open_circuit_engine::{ComponentCatalog, component_catalog};

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
