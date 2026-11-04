//! Offline checks of the Stage 7 reference data (issue #109).
//!
//! Nothing here runs the Engine or a simulator. The checked-in answers of the model equations
//! (`stage-7-analytical.expected.json`), the checked-in ngspice results, the manifest, the
//! netlists, and the fixture files of the contract (`stage-7-*.expected.json`) are read and
//! held against each other and against circuit laws that do not depend on the model: Kirchhoff's
//! current law at each node, the balance of power, and the mirror symmetry of the PNP. Later
//! tasks compare the Engine with the `contract` answers using the manifest's `engineTolerance`;
//! this file is what keeps the data they compare with honest.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden")
        .join(name)
}

fn read(name: &str) -> Value {
    let text = fs::read_to_string(golden(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// A number, or a decimal string (the manifest records circuit values as written in the
/// netlist, so that "0.6" stays "0.6").
fn num(value: &Value, what: &str) -> f64 {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .unwrap_or_else(|| panic!("{what} is not a number: {value}"))
}

fn text(value: &Value) -> &str {
    value.as_str().expect("a string")
}

/// Every numeric leaf of a quantities object, by a dotted path.
fn flatten(value: &Value, prefix: &str, out: &mut HashMap<String, f64>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                flatten(inner, &format!("{prefix}{key}."), out);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                flatten(
                    item,
                    &format!("{}[{index}].", prefix.trim_end_matches('.')),
                    out,
                );
            }
        }
        Value::Number(number) => {
            out.insert(
                prefix.trim_end_matches('.').to_owned(),
                number.as_f64().expect("finite"),
            );
        }
        _ => {}
    }
}

fn close(actual: f64, expected: f64, relative: f64, absolute: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= relative * actual.abs().max(expected.abs()) + absolute,
        "{what}: {actual} against {expected}"
    );
}

struct Reference {
    manifest: Value,
    analytical: Value,
    ngspice: Value,
}

fn reference() -> Reference {
    Reference {
        manifest: read("stage-7-manifest.json"),
        analytical: read("stage-7-analytical.expected.json"),
        ngspice: read("stage-7-ngspice.expected.json"),
    }
}

fn by_id<'a>(file: &'a Value, key: &str) -> HashMap<&'a str, &'a Value> {
    file[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} is not a list"))
        .iter()
        .map(|entry| (text(&entry["id"]), entry))
        .collect()
}

#[test]
fn the_manifest_the_answers_the_ngspice_results_and_the_netlists_name_the_same_cases() {
    let reference = reference();
    let manifest = by_id(&reference.manifest, "fixtures");
    let analytical = by_id(&reference.analytical, "cases");
    let ngspice = by_id(&reference.ngspice, "cases");
    let mut files = fs::read_dir(golden("stage-7-ngspice"))
        .unwrap()
        .map(|entry| {
            entry
                .unwrap()
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    files.sort();
    let mut ids = manifest
        .keys()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    ids.sort();

    assert_eq!(ids.len(), 18);
    assert_eq!(files, ids, "one netlist per fixture");
    assert_eq!(analytical.len(), ids.len());
    assert_eq!(ngspice.len(), ids.len());
    for id in &ids {
        assert!(analytical.contains_key(id.as_str()), "{id} has no answer");
        assert!(
            ngspice.contains_key(id.as_str()),
            "{id} has no ngspice result"
        );
    }

    // The roles the issue asks for are all present.
    let roles = manifest
        .values()
        .map(|entry| text(&entry["role"]))
        .collect::<Vec<_>>();
    for role in ["canonical", "difficult", "wrongVariant"] {
        assert!(roles.contains(&role), "no {role} fixture");
    }
    for kind in ["diode_dc", "diode_sweep", "bjt_dc", "rc_transient"] {
        assert!(
            manifest.values().any(|entry| text(&entry["kind"]) == kind),
            "no {kind} fixture"
        );
    }
}

#[test]
fn the_netlists_are_minimal_executable_and_aligned_with_the_manifest() {
    let reference = reference();
    let options = text(&reference.manifest["sources"]["ngspice"]["options"]);
    let transient_options = text(&reference.manifest["sources"]["ngspice"]["transientOptions"]);
    assert_eq!(text(&reference.ngspice["simulator"]), "ngspice-47");
    assert_eq!(
        text(&reference.manifest["sources"]["ngspice"]["simulator"]),
        "ngspice-47"
    );
    assert!(options.contains("temp=27") && options.contains("tnom=27"));
    assert!(
        options.contains("gmin=0"),
        "the contract's g_min is explicit"
    );

    for entry in reference.manifest["fixtures"].as_array().unwrap() {
        let id = text(&entry["id"]);
        let netlist = fs::read_to_string(golden(text(&entry["netlist"]))).unwrap();
        let lines = netlist.lines().collect::<Vec<_>>();
        assert!(
            lines[0].starts_with(&format!("* Stage 7 reference circuit: {id}")),
            "{id}: the title line"
        );
        assert!(
            netlist.contains(".control") && netlist.contains(".endc"),
            "{id}"
        );
        assert!(lines.last() == Some(&".end"), "{id}");
        assert!(netlist.contains(&format!("wrdata {id}.out")), "{id}");
        let wanted = if text(&entry["kind"]) == "rc_transient" {
            transient_options
        } else {
            options
        };
        assert!(
            netlist.contains(&format!(".options {wanted}")),
            "{id}: options"
        );
        // Every junction has the contract's 1 pS across it, as an explicit resistor.
        if text(&entry["kind"]) != "rc_transient" {
            assert!(
                netlist.contains("RGD ") || netlist.contains("RGBE "),
                "{id}"
            );
        }
        // Nothing in a netlist is generated from the Engine.
        assert!(!netlist.to_lowercase().contains("engine"), "{id}");
    }
}

#[test]
fn ngspice_agrees_with_the_equations_within_the_recorded_reference_error() {
    let reference = reference();
    let aligned = &reference.manifest["referenceError"]["aligned"];
    let contract = &reference.manifest["referenceError"]["contract"];
    let analytical = by_id(&reference.analytical, "cases");

    for (id, result) in by_id(&reference.ngspice, "cases") {
        let mut got = HashMap::new();
        let mut trimmed = result.clone();
        trimmed.as_object_mut().unwrap().remove("id");
        flatten(&trimmed, "", &mut got);

        for (label, key, bounds) in [
            ("aligned", "ngspiceConstants", aligned),
            ("contract", "contract", contract),
        ] {
            let mut want = HashMap::new();
            flatten(&analytical[id][key], "", &mut want);
            assert_eq!(
                got.keys().collect::<std::collections::BTreeSet<_>>(),
                want.keys().collect::<std::collections::BTreeSet<_>>(),
                "{id}: the same quantities"
            );
            for (path, ngspice_value) in &got {
                if path.ends_with("sourceVolts") {
                    continue;
                }
                let expected = want[path];
                let difference = (ngspice_value - expected).abs();
                if path.contains("Volts") {
                    let bound = if label == "aligned" && path.contains("capacitorVoltsAtSeconds") {
                        num(&bounds["transientVoltsAbsolute"], "bound")
                    } else {
                        num(&bounds["voltsAbsolute"], "bound")
                    };
                    assert!(
                        difference <= bound,
                        "{id} {path} ({label}): ngspice {ngspice_value} against {expected}"
                    );
                } else {
                    close(
                        *ngspice_value,
                        expected,
                        num(&bounds["ampsRelative"], "bound"),
                        num(&bounds["ampsAbsolute"], "bound"),
                        &format!("{id} {path} ({label})"),
                    );
                }
            }
        }
    }
}

#[test]
fn the_engine_tolerances_are_far_looser_than_the_reference_error() {
    let reference = reference();
    let aligned = &reference.manifest["referenceError"]["aligned"];
    let volts = num(&aligned["voltsAbsolute"], "volts");
    let transient = num(&aligned["transientVoltsAbsolute"], "transient");
    let relative = num(&aligned["ampsRelative"], "relative");

    for entry in reference.manifest["fixtures"].as_array().unwrap() {
        let id = text(&entry["id"]);
        let tolerance = &entry["engineTolerance"];
        let engine_volts = num(&tolerance["voltsAbsolute"], "voltsAbsolute");
        let reference_volts = if text(&entry["kind"]) == "rc_transient" {
            transient
        } else {
            volts
        };
        assert!(
            engine_volts >= 10.0 * reference_volts,
            "{id}: {engine_volts} V is not ten times the reference error {reference_volts} V"
        );
        if let Some(engine_relative) = tolerance.get("ampsRelative") {
            assert!(
                num(engine_relative, "ampsRelative") >= 10.0 * relative,
                "{id}"
            );
        }
    }
}

#[test]
fn the_answers_satisfy_kirchhoff_and_the_balance_of_power() {
    let reference = reference();
    let analytical = by_id(&reference.analytical, "cases");

    for entry in reference.manifest["fixtures"].as_array().unwrap() {
        let id = text(&entry["id"]);
        let circuit = &entry["circuit"];
        let answers = &analytical[id]["contract"];

        match text(&entry["kind"]) {
            "diode_dc" | "diode_sweep" => {
                let resistance = num(&circuit["resistance"], "resistance");
                let points: Vec<(f64, &Value)> = if text(&entry["kind"]) == "diode_dc" {
                    vec![(num(&circuit["source"], "source"), answers)]
                } else {
                    answers["points"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|point| (num(&point["sourceVolts"], "source"), point))
                        .collect()
                };
                for (source, point) in points {
                    let v = num(&point["diodeVoltageVolts"], "v");
                    let i = num(&point["diodeCurrentAmps"], "i");
                    // KVL around the loop: the source equals the resistor plus the diode.
                    close(
                        source,
                        resistance * i + v,
                        1e-12,
                        1e-15,
                        &format!("{id} KVL at {source}"),
                    );
                    // Power: the source delivers what the resistor and the diode absorb.
                    close(
                        source * i,
                        resistance * i * i + v * i,
                        1e-9,
                        1e-24,
                        &format!("{id} power at {source}"),
                    );
                    // The diode never delivers power.
                    assert!(v * i >= -1e-24, "{id}: the diode absorbs power at {source}");
                }
            }
            "bjt_dc" => {
                let vcc = num(&circuit["vcc"], "vcc");
                let rc = num(&circuit["rc"], "rc");
                let vbb = num(&circuit["vbb"], "vbb");
                let rb = num(&circuit["rb"], "rb");
                let swapped = circuit["swapped"].as_bool().unwrap();
                let vb = num(&answers["baseVoltageVolts"], "vb");
                let vl = num(&answers["loadNodeVoltageVolts"], "vl");
                let ic = num(&answers["collectorCurrentAmps"], "ic");
                let ib = num(&answers["baseCurrentAmps"], "ib");
                let ie = num(&answers["emitterCurrentAmps"], "ie");
                let load_current = if swapped { ie } else { ic };

                // Kirchhoff's current law at the base node and at the load node.
                close((vbb - vb) / rb, ib, 1e-9, 1e-17, &format!("{id} base node"));
                close(
                    (vcc - vl) / rc,
                    load_current,
                    1e-9,
                    1e-17,
                    &format!("{id} load node"),
                );
                // The device conserves current: the three terminal currents sum to zero.
                let scale = ic.abs().max(ib.abs()).max(ie.abs());
                assert!(
                    (ic + ib + ie).abs() <= 1e-12 * scale + 1e-24,
                    "{id}: terminal sum"
                );
                // Power: the two sources deliver what the resistors and the device absorb.
                let (vc_terminal, ve_terminal) = if swapped { (0.0, vl) } else { (vl, 0.0) };
                let delivered = vcc * (vcc - vl) / rc + vbb * (vbb - vb) / rb;
                let absorbed = rc * ((vcc - vl) / rc).powi(2)
                    + rb * ((vbb - vb) / rb).powi(2)
                    + vc_terminal * ic
                    + vb * ib
                    + ve_terminal * ie;
                // The node voltages are stored to 17 digits, so a small current through a large
                // voltage (the cutoff leakage) is known to about 1e-15 of vcc^2 / rc.
                close(
                    delivered,
                    absorbed,
                    1e-9,
                    1e-15 * vcc * vcc / rc,
                    &format!("{id} power"),
                );
            }
            "rc_transient" => {
                let times = answers["capacitorVoltsAtSeconds"].as_object().unwrap();
                let mut previous = 0.0;
                let mut sorted = times.iter().collect::<Vec<_>>();
                sorted.sort_by(|a, b| a.0.parse::<f64>().unwrap().total_cmp(&b.0.parse().unwrap()));
                for (time, volts) in sorted {
                    let v = num(volts, time);
                    let source = num(&circuit["source"], "source");
                    // The capacitor charges monotonically and never reaches the source.
                    assert!(v > previous && v < source, "{id}: {time} s");
                    previous = v;
                }
            }
            other => panic!("unknown kind {other}"),
        }
    }
}

#[test]
fn the_regions_and_the_polarity_are_what_the_cases_say_they_are() {
    let reference = reference();
    let analytical = by_id(&reference.analytical, "cases");
    let answer = |id: &str, key: &str| num(&analytical[id]["contract"][key], key);

    // Forward active: the collector current is beta_F times the base current, to the small
    // effect of the junction conductance.
    for id in ["npn-active", "pnp-active"] {
        close(
            answer(id, "collectorCurrentAmps") / answer(id, "baseCurrentAmps"),
            100.0,
            1e-4,
            0.0,
            id,
        );
    }
    // Saturation: beta_F falls short, and V_CE is below V_BE.
    for id in [
        "npn-saturation",
        "npn-saturation-high-current",
        "npn-saturation-edge",
    ] {
        let ratio = answer(id, "collectorCurrentAmps") / answer(id, "baseCurrentAmps");
        assert!(ratio < 100.0 && ratio > 1.0, "{id}: ratio {ratio}");
        assert!(
            answer(id, "loadNodeVoltageVolts") < answer(id, "baseVoltageVolts"),
            "{id}"
        );
    }
    assert!(answer("npn-saturation-edge", "loadNodeVoltageVolts") > 0.1);
    assert!(answer("npn-saturation-edge", "loadNodeVoltageVolts") < 0.3);
    // Cutoff: only leakage flows.
    for id in ["npn-cutoff", "pnp-cutoff"] {
        assert!(answer(id, "collectorCurrentAmps").abs() < 1e-9, "{id}");
        assert!(answer(id, "baseVoltageVolts").abs() < 1e-3, "{id}");
    }
    // Reverse active (the wrong wiring): the current that should enter the collector leaves it.
    assert!(answer("npn-reverse-active", "collectorCurrentAmps") < 0.0);
    assert!(answer("npn-reverse-active", "emitterCurrentAmps") > 0.0);
    // The beta-variation case follows its own beta.
    close(
        answer("npn-beta-variation", "collectorCurrentAmps")
            / answer("npn-beta-variation", "baseCurrentAmps"),
        30.0,
        1e-3,
        0.0,
        "beta_F = 30",
    );

    // PNP polarity: each PNP case is the NPN case with every voltage and current negated.
    for (pnp, npn) in [
        ("pnp-cutoff", "npn-cutoff"),
        ("pnp-active", "npn-active"),
        ("pnp-saturation", "npn-saturation"),
    ] {
        for key in [
            "baseVoltageVolts",
            "loadNodeVoltageVolts",
            "collectorCurrentAmps",
            "baseCurrentAmps",
            "emitterCurrentAmps",
        ] {
            close(
                answer(pnp, key),
                -answer(npn, key),
                1e-12,
                1e-30,
                &format!("{pnp} {key}"),
            );
        }
    }

    // The difficult diode cases are where they say they are.
    let low = answer("diode-low-current", "diodeCurrentAmps");
    assert!((1e-8..1e-6).contains(&low), "low current {low}");
    let high = answer("diode-high-current", "diodeCurrentAmps");
    assert!((0.3..0.5).contains(&high), "high current {high}");
    assert!(answer("diode-reverse", "diodeCurrentAmps") < 0.0);

    // The sweep passes through the polarity change: zero current at zero volts, the sign of the
    // current follows the sign of the source, and the current rises monotonically.
    let points = analytical["diode-sweep"]["contract"]["points"]
        .as_array()
        .unwrap();
    assert_eq!(points.len(), 11);
    let currents = points
        .iter()
        .map(|point| num(&point["diodeCurrentAmps"], "i"))
        .collect::<Vec<_>>();
    assert!(
        currents.windows(2).all(|pair| pair[0] < pair[1]),
        "monotonic"
    );
    assert!(currents[5].abs() < 1e-30, "zero at zero volts");
    assert!(currents[0] < 0.0 && currents[10] > 0.0);
}

#[test]
fn the_fixture_files_of_the_contract_hold_the_same_numbers_as_the_answers() {
    let reference = reference();
    let analytical = by_id(&reference.analytical, "cases");
    let contract = |id: &str, key: &str| num(&analytical[id]["contract"][key], key);

    // Diode.
    let diode = read("stage-7-diode-dc.expected.json");
    for case in diode["cases"].as_array().unwrap() {
        let id = match text(&case["name"]) {
            "forward" => "diode-forward",
            "reverse" => "diode-reverse",
            other => panic!("unknown case {other}"),
        };
        for (file_key, key) in [
            ("diodeVoltageVolts", "diodeVoltageVolts"),
            ("diodeCurrentAmps", "diodeCurrentAmps"),
        ] {
            close(
                num(&case[file_key], file_key),
                contract(id, key),
                1e-12,
                1e-30,
                id,
            );
        }
    }

    // Transistor, including the wrong variant.
    let bjt = read("stage-7-bjt-dc.expected.json");
    let rename = |name: &str| {
        let mut out = String::new();
        for (index, character) in name.chars().enumerate() {
            if character.is_uppercase() && index > 0 {
                out.push('-');
            }
            out.push(character.to_ascii_lowercase());
        }
        out
    };
    let cases = bjt["cases"]
        .as_array()
        .unwrap()
        .iter()
        .chain(bjt["wrongVariants"].as_array().unwrap());
    let mut checked = 0;
    for case in cases {
        let id = rename(text(&case["name"]));
        for (file_key, key) in [
            ("baseVoltageVolts", "baseVoltageVolts"),
            ("loadNodeVoltageVolts", "loadNodeVoltageVolts"),
            ("collectorCurrentAmps", "collectorCurrentAmps"),
            ("baseCurrentAmps", "baseCurrentAmps"),
            ("emitterCurrentAmps", "emitterCurrentAmps"),
        ] {
            close(
                num(&case[file_key], file_key),
                contract(&id, key),
                1e-12,
                1e-30,
                &id,
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 7);

    // Transient.
    let transient = read("stage-7-diode-rc-transient.expected.json");
    let charging = &analytical["rc-diode-charging"]["contract"]["capacitorVoltsAtSeconds"];
    for (step, time) in [(54, "0.054"), (104, "0.104"), (204, "0.204")] {
        let entry = transient["expected"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["step"] == step)
            .unwrap();
        close(
            num(&entry["capacitorVolts"], "v"),
            num(&charging[time], time),
            1e-12,
            1e-30,
            time,
        );
    }
    let reversed = &analytical["rc-diode-reversed"]["contract"]["capacitorVoltsAtSeconds"];
    for (step, time) in [(54, "0.054"), (104, "0.104"), (204, "0.204")] {
        let entry = transient["wrongVariants"][0]["expected"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["step"] == step)
            .unwrap();
        close(
            num(&entry["capacitorVolts"], "v"),
            num(&reversed[time], time),
            1e-9,
            1e-30,
            time,
        );
    }

    // The tolerances in the contract's files are the manifest's.
    let manifest = by_id(&reference.manifest, "fixtures");
    assert_eq!(
        num(
            &manifest["diode-forward"]["engineTolerance"]["voltsAbsolute"],
            "v"
        ),
        num(&diode["cases"][0]["tolerance"]["voltsAbsolute"], "v")
    );
    assert_eq!(
        num(
            &manifest["rc-diode-charging"]["engineTolerance"]["voltsAbsolute"],
            "v"
        ),
        num(&transient["expected"][2]["toleranceVolts"], "v")
    );
    assert_eq!(
        num(
            &manifest["rc-diode-reversed"]["engineTolerance"]["voltsAbsolute"],
            "v"
        ),
        num(&transient["wrongVariants"][0]["toleranceVolts"], "v")
    );
    assert_eq!(
        num(
            &manifest["npn-active"]["engineTolerance"]["ampsRelative"],
            "r"
        ),
        num(&bjt["tolerance"]["ampsRelative"], "r")
    );

    // The invalid fixtures name the structured diagnostic the contract defines.
    assert_eq!(
        text(&diode["failureCases"][0]["expected"]["diagnostic"]),
        "ModelRangeExceeded"
    );
    assert_eq!(
        text(&bjt["failureCases"][0]["expected"]["diagnostic"]),
        "ModelRangeExceeded"
    );
    assert_eq!(
        reference.manifest["failureFixtures"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
