use super::*;
use roses_compare::ComparisonStatus;
use serde_json::json;

fn synthetic_capture() -> Value {
    let observations: Vec<_> = roses_viewer::pal_watches().into_iter().map(|watch| {
        let value = if watch.name.ends_with("_card_id") { 999 }
            else if watch.name.starts_with("candidate_unit_") && (watch.name.ends_with("_col") || watch.name.ends_with("_row")) { 7 }
            else if watch.name.starts_with("candidate_unit_") && watch.name.ends_with("_side") { 2 }
            else if watch.name.starts_with("candidate_unit_") && watch.name.ends_with("_slot") { 11 }
            else if watch.name.ends_with("_lp") { 4321 }
            else if watch.name.ends_with("_terrain") { 7 } else { 0 };
        json!({"name":watch.name,"address":watch.address,"width":u8::from(watch.width),"value":value})
    }).collect();
    let mut capture = json!({"schema_version":"roses-saved-ee-memory-1", "state_sha256":"1".repeat(64),
        "ee_memory_sha256":"2".repeat(64), "state_bytes":1000, "ee_memory_bytes":33554432,
        "compression_method":"stored", "observations":observations, "atomic_saved_memory":true, "normalized_game_state":false});
    // Deliberately make raw side differ from array group and repeat a card ID.
    unit(&mut capture, 0, 2, 22, 1, 2, 2);
    unit(&mut capture, 0, 10, 22, 0, 5, 5);
    unit(&mut capture, 1, 0, 81, 0, 0, 6);
    capture
}

fn set(capture: &mut Value, name: &str, value: u64) {
    capture["observations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|observation| observation["name"] == name)
        .unwrap()["value"] = json!(value);
}

fn unit(capture: &mut Value, group: u8, slot: u8, card: u16, side: u8, column: u8, row: u8) {
    for (suffix, value) in [
        ("const_info_ptr", 0x2BE400 + u64::from(card) * 20),
        ("card_id", u64::from(card)),
        ("side", u64::from(side)),
        ("slot", u64::from(slot)),
        ("col", u64::from(column)),
        ("row", u64::from(row)),
    ] {
        set(
            capture,
            &format!("candidate_unit_{group}_{slot}_{suffix}"),
            value,
        );
    }
    set(
        capture,
        &format!("candidate_cell_{row}_{column}_unit_ptr"),
        0x2DB990 + u64::from(group) * 0x3F4 + u64::from(slot) * 0x5C,
    );
}

fn valid_request() -> Value {
    json!({"schema_version":REQUEST_SCHEMA,"scenario":"synthetic isolated movement","profile":PROFILE,
        "metadata":{"disc_sha256":"A".repeat(64),"executable_sha256":"b".repeat(64)},
        "command":{"kind":"move_unit","unit":2,"destination":{"column":2,"row":1}},
        "context":{"kind":"restricted","eligible_unit":2,"direction":"decreasing_row","permission":"available"}})
}

fn execute(capture: &Value, request: &Value) -> Result<Simulation> {
    simulate(&serde_json::to_vec(capture)?, &serde_json::to_vec(request)?)
}

#[test]
fn adapter_preserves_raw_side_and_projection_order_is_explicit() {
    let simulation = execute(&synthetic_capture(), &valid_request()).unwrap();
    assert_eq!(simulation.before.unit(UnitId(2)).unwrap().side, 1);
    assert_eq!(simulation.before.unit(UnitId(11)).unwrap().side, 0);
    let before = &simulation.snapshot.checkpoints[0].observed;
    let after = &simulation.snapshot.checkpoints[1].observed;
    assert_eq!(
        before["occupied_units"]
            .as_array()
            .unwrap()
            .iter()
            .map(|unit| unit["unit_key"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["0:10", "0:2", "1:0"]
    );
    assert_eq!(
        before["occupied_cells"],
        json!([
            {"column":2,"row":2,"unit_key":"0:2"},
            {"column":5,"row":5,"unit_key":"0:10"},
            {"column":0,"row":6,"unit_key":"1:0"},
        ])
    );
    let mut expected = before.clone();
    expected["occupied_units"][1]["row"] = json!(1);
    expected["occupied_cells"][0]["row"] = json!(1);
    assert_eq!(after, &expected);
    assert_eq!(before["terrain_row_major"].as_array().unwrap().len(), 49);
    assert_eq!(simulation.snapshot.metadata.provenance, Provenance::Native);
    assert_eq!(simulation.snapshot.metadata.disc_sha256, "a".repeat(64));
    assert_eq!(simulation.trace["source_identity_authenticated"], false);
    assert_eq!(simulation.trace["reference_endpoint_used"], false);
}

#[test]
fn request_rejects_missing_context_unbounded_metadata_and_expected_endpoints() {
    for (field, value) in [
        ("schema_version", json!("other")),
        ("profile", json!("guessed_profile")),
        ("scenario", json!("x".repeat(121))),
        ("scenario", json!("bad\nscenario")),
    ] {
        let mut changed = valid_request();
        changed[field] = value;
        assert!(execute(&synthetic_capture(), &changed).is_err());
    }
    for bad in ["short".to_string(), "z".repeat(64)] {
        let mut changed = valid_request();
        changed["metadata"]["disc_sha256"] = json!(bad);
        assert!(execute(&synthetic_capture(), &changed).is_err());
    }
    let mut changed = valid_request();
    changed.as_object_mut().unwrap().remove("context");
    assert!(execute(&synthetic_capture(), &changed).is_err());
    let mut changed = valid_request();
    changed["context"] = json!({"kind":"unknown"});
    assert!(execute(&synthetic_capture(), &changed).is_err());
    let mut changed = valid_request();
    changed["expected_endpoint"] = json!({"row":1});
    assert!(execute(&synthetic_capture(), &changed).is_err());
    assert!(request(&vec![b' '; MAX_REQUEST_BYTES + 1]).is_err());
}

#[test]
fn capture_adapter_and_core_rejections_propagate() {
    let mut capture = synthetic_capture();
    capture["observations"].as_array_mut().unwrap().pop();
    assert!(execute(&capture, &valid_request()).is_err());
    let mut capture = synthetic_capture();
    set(&mut capture, "candidate_unit_0_2_side", 3);
    assert!(execute(&capture, &valid_request()).is_err());
    let mut capture = synthetic_capture();
    set(&mut capture, "candidate_cell_1_2_terrain", 0);
    assert!(execute(&capture, &valid_request()).is_err());
    let mut request = valid_request();
    request["command"]["destination"] = json!({"column":5,"row":5});
    assert!(execute(&synthetic_capture(), &request).is_err());
}

#[test]
fn replay_is_byte_stable_and_viewer_outputs_drop_original_raw_state() {
    let first = execute(&synthetic_capture(), &valid_request()).unwrap();
    let second = execute(&synthetic_capture(), &valid_request()).unwrap();
    assert_eq!(
        serde_json::to_vec(&first.snapshot).unwrap(),
        serde_json::to_vec(&second.snapshot).unwrap()
    );
    assert_eq!(first.trace, second.trace);
    let report = roses_compare::compare(&first.snapshot, &second.snapshot).unwrap();
    assert_eq!(report.status, ComparisonStatus::Equal);
    for (state, checkpoint) in [
        (&first.before, "before"),
        (&first.transition.state, "after"),
    ] {
        let view = viewer_snapshot(&first, state, checkpoint).unwrap();
        assert!(view.document().raw_candidate_fields.is_none());
        assert!(view.document().unlinked_candidates.is_empty());
        assert!(view
            .document()
            .units
            .iter()
            .all(|unit| unit.raw_location.is_none()));
        let ViewOrigin::NativeSimulation {
            source,
            request_sha256,
            checkpoint: actual,
            ..
        } = &view.document().origin
        else {
            panic!("wrong origin")
        };
        assert_eq!(source.state_sha256, "1".repeat(64));
        assert_eq!(request_sha256, &first.request_sha256);
        assert_eq!(actual, checkpoint);
        assert_eq!(
            view.unit(2).unwrap().position.row,
            if checkpoint == "before" { 2 } else { 1 }
        );
    }
}

#[test]
fn output_directory_is_new_and_invalid_actions_write_nothing() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "roses simulate move {} {stamp}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let capture = root.join("capture.json");
    let request = root.join("request.json");
    fs::write(&capture, serde_json::to_vec(&synthetic_capture()).unwrap()).unwrap();
    fs::write(&request, serde_json::to_vec(&valid_request()).unwrap()).unwrap();
    let output = root.join("output");
    run(&capture, &request, &output).unwrap();
    let snapshot = fs::read(output.join("native-snapshot.json")).unwrap();
    assert!(run(&capture, &request, &output).is_err());
    assert_eq!(
        fs::read(output.join("native-snapshot.json")).unwrap(),
        snapshot
    );
    let mut invalid = valid_request();
    invalid["context"] = json!({"kind":"unknown"});
    fs::write(&request, serde_json::to_vec(&invalid).unwrap()).unwrap();
    let rejected = root.join("rejected");
    assert!(run(&capture, &request, &rejected).is_err());
    assert!(!rejected.exists());
    // Only exact files created by this fixture are removed; no recursive cleanup.
    for name in [
        "native-snapshot.json",
        "trace.json",
        "viewer-before.json",
        "viewer-after.json",
    ] {
        fs::remove_file(output.join(name)).unwrap();
    }
    fs::remove_dir(output).unwrap();
    fs::remove_file(capture).unwrap();
    fs::remove_file(request).unwrap();
    fs::remove_dir(root).unwrap();
}
