//! Derived parts and terminal counts for the Stage 12 documentation.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;
use ts_rs::TS;

use crate::{ComponentTypeId, ParameterId, Schematic, Unit, component_catalog};

#[derive(Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "MaterialParameter.ts")]
pub struct MaterialParameter {
    pub id: ParameterId,
    pub unit: Unit,
    pub value: Option<f64>,
}

#[derive(Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "MaterialRow.ts")]
pub struct MaterialRow {
    pub position: u32,
    pub quantity: u32,
    pub designators: Vec<String>,
    pub display_name_key: String,
    pub parameters: Vec<MaterialParameter>,
}

#[derive(Debug, PartialEq, Serialize, TS)]
#[ts(export, export_to = "TerminalStripRow.ts")]
pub struct TerminalStripRow {
    pub strip: String,
    pub quantity: u32,
    pub labels: Vec<String>,
}

#[derive(Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "BillOfMaterials.ts")]
pub struct BillOfMaterials {
    pub components: Vec<MaterialRow>,
    pub terminal_strips: Vec<TerminalStripRow>,
}

fn designator_key(name: &str) -> (&str, u32) {
    let letters = name.trim_end_matches(|character: char| character.is_ascii_digit());
    let number = name[letters.len()..].parse().unwrap_or(u32::MAX);
    (letters, number)
}

fn row_key((letters, number): (&str, u32)) -> (Option<u8>, u32, &str) {
    (letters.bytes().next(), number, letters)
}

/// Group the validated drawing by part type and the values of its catalog parameters.
pub fn bill_of_materials(schematic: &Schematic) -> BillOfMaterials {
    let catalog = component_catalog();
    let definitions: HashMap<_, _> = catalog
        .component_types
        .iter()
        .map(|definition| (definition.type_id, definition))
        .collect();
    let designators: HashMap<_, _> = schematic
        .documentation
        .designators
        .iter()
        .map(|entry| (&entry.component_id, &entry.designator))
        .collect();
    let mut groups = HashMap::<(ComponentTypeId, Vec<Option<u64>>), usize>::new();
    let mut rows = Vec::new();
    for component in &schematic.components {
        let definition = definitions[&component.type_id()];
        let values: HashMap<_, _> = component.parameter_values().into_iter().collect();
        let parameters: Vec<_> = definition
            .parameters
            .iter()
            .map(|parameter| MaterialParameter {
                id: parameter.id,
                unit: parameter.unit,
                value: values.get(&parameter.id).copied(),
            })
            .collect();
        let key = parameters
            .iter()
            .map(|parameter| parameter.value.map(f64::to_bits))
            .collect();
        let index = *groups.entry((definition.type_id, key)).or_insert_with(|| {
            rows.push((
                definition.designator_prefix.as_str(),
                MaterialRow {
                    position: 0,
                    quantity: 0,
                    designators: Vec::new(),
                    display_name_key: definition.display_name_key.clone(),
                    parameters,
                },
            ));
            rows.len() - 1
        });
        let row = &mut rows[index].1;
        row.quantity += 1;
        if let Some(designator) = designators.get(component.id()) {
            row.designators.push((*designator).clone());
        }
    }
    for (_, row) in &mut rows {
        row.designators
            .sort_by(|a, b| designator_key(a).cmp(&designator_key(b)));
    }
    rows.sort_by(|(fallback_a, a), (fallback_b, b)| {
        let a = a
            .designators
            .first()
            .map_or((*fallback_a, u32::MAX), |name| designator_key(name));
        let b = b
            .designators
            .first()
            .map_or((*fallback_b, u32::MAX), |name| designator_key(name));
        row_key(a).cmp(&row_key(b))
    });
    let components = rows
        .into_iter()
        .enumerate()
        .map(|(index, (_, mut row))| {
            row.position = index as u32 + 1;
            row
        })
        .collect();
    let mut strips = BTreeMap::<_, Vec<_>>::new();
    for terminal in &schematic.documentation.terminals {
        strips
            .entry(terminal.strip.clone())
            .or_default()
            .push(terminal.label.clone());
    }
    let terminal_strips = strips
        .into_iter()
        .map(|(strip, mut labels)| {
            labels.sort();
            TerminalStripRow {
                strip,
                quantity: labels.len() as u32,
                labels,
            }
        })
        .collect();
    BillOfMaterials {
        components,
        terminal_strips,
    }
}
