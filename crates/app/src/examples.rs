//! Bundled examples are indexed by ID and read through the project decoder.

use std::collections::HashSet;

use open_circuit_engine::Schematic;
use serde::Deserialize;

use crate::project::{decode_project_bytes, prepare_open_project};

const MANIFEST: &str = include_str!("../examples/manifest.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    entries: Vec<Example>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Example {
    id: String,
    title: String,
    description: String,
    path: String,
}

fn resource(path: &str) -> Option<&'static [u8]> {
    Some(match path {
        "divider.ocircuit" => include_bytes!("../examples/divider.ocircuit"),
        "rc.ocircuit" => include_bytes!("../examples/rc.ocircuit"),
        "relay-latch.ocircuit" => include_bytes!("../examples/relay-latch.ocircuit"),
        "start-stop.ocircuit" => include_bytes!("../examples/start-stop.ocircuit"),
        "rail-circuit.ocircuit" => include_bytes!("../examples/rail-circuit.ocircuit"),
        "three-phase-starter.ocircuit" => {
            include_bytes!("../examples/three-phase-starter.ocircuit")
        }
        _ => return None,
    })
}

fn catalog(source: &str) -> Result<Vec<Example>, String> {
    let manifest: Manifest = serde_json::from_str(source)
        .map_err(|error| format!("Invalid example catalog: {error}"))?;
    if manifest.schema_version != 1 || manifest.entries.len() > 64 {
        return Err("Unsupported example catalog version or size.".into());
    }

    let mut ids = HashSet::new();
    for entry in &manifest.entries {
        if !entry.id.split('-').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        }) || !ids.insert(&entry.id)
            || entry.title.trim().is_empty()
            || entry.description.trim().is_empty()
            || entry.path != format!("{}.ocircuit", entry.id)
            || resource(&entry.path).is_none()
        {
            return Err("The example catalog has an invalid entry.".into());
        }
        decode(entry)?;
    }
    Ok(manifest.entries)
}

fn decode(entry: &Example) -> Result<Schematic, String> {
    let bytes = resource(&entry.path).ok_or("Example resource is missing.")?;
    let schematic = decode_project_bytes(bytes)?;
    let original: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if original["schemaVersion"] != 3 {
        return Err("Example projects must use schema 3.".into());
    }
    Ok(schematic)
}

/// Returns entries in catalog order as ID, title, and description.
pub fn list_examples() -> Result<Vec<[String; 3]>, String> {
    Ok(catalog(MANIFEST)?
        .into_iter()
        .map(|entry| [entry.id, entry.title, entry.description])
        .collect())
}

/// Reads a baked resource without using the caller's ID as a filesystem path.
pub fn read_example(id: &str, assignment_open: bool) -> Result<Schematic, String> {
    let entry = catalog(MANIFEST)?
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or("Unknown example.")?;
    prepare_open_project(decode(&entry)?, assignment_open)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_rejects_invalid_version_duplicate_ids_and_missing_resources() {
        let mut manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
        manifest["schemaVersion"] = 2.into();
        assert!(catalog(&manifest.to_string()).is_err());
        manifest["schemaVersion"] = 1.into();
        let entries = manifest["entries"].as_array_mut().unwrap();
        entries.push(entries[0].clone());
        assert!(catalog(&manifest.to_string()).is_err());
        let entries = manifest["entries"].as_array_mut().unwrap();
        entries.pop();
        entries[0]["id"] = "missing".into();
        entries[0]["path"] = "missing.ocircuit".into();
        assert!(catalog(&manifest.to_string()).is_err());
        assert!(read_example("../divider", false).is_err());
        assert!(read_example("rail-circuit", true).is_err());
    }
}
