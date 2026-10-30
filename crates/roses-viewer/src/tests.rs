use super::*;
use serde_json::{json, Value};

fn synthetic_capture() -> Value {
    let observations: Vec<_> = pal_watches().into_iter().map(|watch| {
        let value = if watch.name.ends_with("_card_id") { 999 }
            else if watch.name.starts_with("candidate_unit_") && (watch.name.ends_with("_col") || watch.name.ends_with("_row")) { 7 }
            else if watch.name.starts_with("candidate_unit_") && watch.name.ends_with("_side") { 2 }
            else if watch.name.starts_with("candidate_unit_") && watch.name.ends_with("_slot") { 11 }
            else if watch.name.ends_with("_lp") { 4000 } else { 0 };
        json!({"name": watch.name, "address": watch.address, "width": u8::from(watch.width), "value": value})
    }).collect();
    json!({"schema_version":"roses-saved-ee-memory-1", "state_sha256":"1".repeat(64),
        "ee_memory_sha256":"2".repeat(64), "state_bytes":1000, "ee_memory_bytes":33554432,
        "compression_method":"stored", "observations":observations, "atomic_saved_memory":true, "normalized_game_state":false})
}
fn set(document: &mut Value, name: &str, value: u64) {
    document["observations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|item| item["name"] == name)
        .unwrap()["value"] = json!(value);
}
fn card(document: &mut Value, group: u8, slot: u8, id: u16, column: u8, row: u8) {
    let prefix = format!("candidate_unit_{group}_{slot}_");
    for (suffix, value) in [
        ("const_info_ptr", 0x2BE400 + u64::from(id) * 20),
        ("card_id", u64::from(id)),
        ("col", u64::from(column)),
        ("row", u64::from(row)),
        ("side", u64::from(group.min(1))),
        ("slot", u64::from(slot)),
    ] {
        set(document, &(prefix.clone() + suffix), value);
    }
}
fn parse(document: &Value) -> Result<BoardSnapshot> {
    from_saved_capture(&serde_json::to_vec(document).unwrap())
}

#[test]
fn reciprocal_links_create_units_but_unlinked_staging_does_not() {
    let mut capture = synthetic_capture();
    card(&mut capture, 0, 1, 12, 3, 5);
    card(&mut capture, 2, 1, 23, 3, 5);
    set(&mut capture, "candidate_cell_5_3_unit_ptr", 0x2DB9EC);
    let board = parse(&capture).unwrap();
    assert_eq!(board.document().units.len(), 1);
    assert_eq!(board.document().units[0].card_id, 12);
    assert_eq!(board.document().unlinked_candidates.len(), 1);
    assert_eq!(board.document().unlinked_candidates[0].card_id, 23);
    assert_eq!(
        board.cell(GridPos { column: 3, row: 5 }).unwrap().occupancy,
        Occupancy::Occupied { unit_key: 1 }
    );
    assert_eq!(
        board.cell(GridPos { column: 0, row: 0 }).unwrap().occupancy,
        Occupancy::Empty
    );
}

#[test]
fn missing_duplicate_unknown_and_mistyped_watches_are_errors() {
    let original = synthetic_capture();
    let mut changed = original.clone();
    changed["observations"].as_array_mut().unwrap().pop();
    assert!(parse(&changed).unwrap_err().0.contains("305"));
    let mut changed = original.clone();
    changed["observations"][304] = changed["observations"][0].clone();
    assert!(parse(&changed).unwrap_err().0.contains("Duplicate"));
    for (field, bad) in [
        ("name", json!("not_a_watch")),
        ("address", json!(1)),
        ("width", json!(8)),
        ("value", json!(65536)),
    ] {
        let mut changed = original.clone();
        changed["observations"][0][field] = bad;
        assert!(parse(&changed).is_err(), "accepted changed {field}");
    }
}

#[test]
fn invalid_coordinates_pointer_targets_and_card_identity_are_errors() {
    let mut original = synthetic_capture();
    card(&mut original, 0, 1, 12, 3, 5);
    set(&mut original, "candidate_cell_5_3_unit_ptr", 0x2DB9EC);
    for (name, bad) in [
        ("candidate_unit_0_1_col", 7),
        ("candidate_unit_0_1_row", 8),
        ("candidate_cell_5_3_unit_ptr", 0x2DB9ED),
        ("candidate_unit_0_1_const_info_ptr", 0),
        ("candidate_unit_0_1_card_id", 900),
        ("candidate_cell_5_3_terrain", 10),
    ] {
        let mut changed = original.clone();
        set(&mut changed, name, bad);
        assert!(parse(&changed).is_err(), "accepted changed {name}");
    }
    let mut duplicate = original;
    set(&mut duplicate, "candidate_cell_4_3_unit_ptr", 0x2DB9EC);
    assert!(parse(&duplicate).is_err());
}

#[test]
fn malformed_oversized_and_wrong_capture_metadata_are_errors() {
    assert!(from_saved_capture(b"{").is_err());
    assert!(from_saved_capture(&vec![b' '; MAX_INPUT_BYTES + 1]).is_err());
    for (field, bad) in [
        ("schema_version", json!("other")),
        ("state_sha256", json!("bad")),
        ("ee_memory_bytes", json!(1024)),
        ("state_bytes", json!(0)),
        ("atomic_saved_memory", json!(false)),
        ("normalized_game_state", json!(true)),
    ] {
        let mut changed = synthetic_capture();
        changed[field] = bad;
        assert!(parse(&changed).is_err());
    }
}

#[test]
fn explicit_snapshot_cannot_hide_missing_cells_or_orphan_units() {
    let original = demo().document().clone();
    assert_eq!(original.cells.len(), 49);
    assert_eq!(original.units.len(), 3);
    let mut changed = original.clone();
    changed.cells.pop();
    assert!(BoardSnapshot::new(changed).is_err());
    let mut changed = original.clone();
    changed.cells[48] = changed.cells[0].clone();
    assert!(BoardSnapshot::new(changed).is_err());
    let mut changed = original.clone();
    changed.units[0].position.column = 6;
    assert!(BoardSnapshot::new(changed).is_err());
    let mut changed = original;
    changed
        .cells
        .iter_mut()
        .find(|cell| matches!(cell.occupancy, Occupancy::Occupied { .. }))
        .unwrap()
        .occupancy = Occupancy::Empty;
    assert!(BoardSnapshot::new(changed).is_err());
}

fn native_document(checkpoint: &str) -> SnapshotDocument {
    let mut document = demo().document().clone();
    document.origin = ViewOrigin::NativeSimulation {
        source: OriginalCaptureReference {
            capture_json_sha256: "1".repeat(64),
            state_sha256: "2".repeat(64),
            ee_memory_sha256: "3".repeat(64),
            address_profile: roses_disc::PAL_PROFILE_ID.into(),
        },
        engine_version: "test-engine-1".into(),
        rules_profile: NATIVE_RULES_PROFILE.into(),
        request_sha256: "4".repeat(64),
        checkpoint: checkpoint.into(),
    };
    document.raw_candidate_fields = None;
    document.unlinked_candidates.clear();
    for unit in &mut document.units {
        unit.raw_location = None;
    }
    document
}

#[test]
fn native_checkpoints_preserve_initialization_references_and_reciprocity() {
    for checkpoint in ["before", "after"] {
        let document = native_document(checkpoint);
        let snapshot = from_snapshot_json(&serde_json::to_vec(&document).unwrap()).unwrap();
        assert!(!snapshot.is_synthetic());
        assert_eq!(snapshot.document().units.len(), 3);
        let ViewOrigin::NativeSimulation {
            source,
            checkpoint: actual,
            ..
        } = &snapshot.document().origin
        else {
            panic!("native provenance was lost");
        };
        assert_eq!(actual, checkpoint);
        assert_eq!(source.capture_json_sha256, "1".repeat(64));
        assert_eq!(source.state_sha256, "2".repeat(64));
        assert_eq!(source.ee_memory_sha256, "3".repeat(64));
        let mut broken = document;
        broken.units[0].position.column = 6;
        assert!(BoardSnapshot::new(broken).is_err());
    }
}

#[test]
fn native_origin_rejects_missing_invalid_or_unknown_provenance() {
    let original = serde_json::to_value(native_document("after")).unwrap();
    for (field, bad) in [
        ("request_sha256", json!("f".repeat(63))),
        ("checkpoint", json!("during")),
        ("engine_version", json!("\n")),
        ("engine_version", json!("a".repeat(129))),
        ("rules_profile", json!("unverified-other-profile")),
        ("unexpected_claim", json!(true)),
    ] {
        let mut changed = original.clone();
        changed["origin"][field] = bad;
        assert!(
            from_snapshot_json(&serde_json::to_vec(&changed).unwrap()).is_err(),
            "accepted {field}"
        );
    }
    for (field, bad) in [
        ("capture_json_sha256", json!("g".repeat(64))),
        ("state_sha256", json!("0".repeat(65))),
        ("ee_memory_sha256", json!("")),
        ("address_profile", json!("other")),
        ("gameplay_validated", json!(true)),
    ] {
        let mut changed = original.clone();
        changed["origin"]["source"][field] = bad;
        assert!(
            from_snapshot_json(&serde_json::to_vec(&changed).unwrap()).is_err(),
            "accepted source {field}"
        );
    }
    let mut missing = original;
    missing["origin"].as_object_mut().unwrap().remove("source");
    assert!(from_snapshot_json(&serde_json::to_vec(&missing).unwrap()).is_err());
}

#[test]
fn native_origin_rejects_stale_original_memory_diagnostics() {
    let original = native_document("after");
    let mut changed = original.clone();
    changed.units[0].raw_location = Some(RawUnitLocation {
        array_group: 0,
        array_slot: 0,
    });
    assert!(BoardSnapshot::new(changed)
        .unwrap_err()
        .0
        .contains("omit original"));
    let mut changed = original.clone();
    changed.raw_candidate_fields = Some(RawCandidateFields {
        selected_map_index: 0,
        active_side_byte: 0,
        unknown_resource_bytes: [0; 2],
        unknown_b8f2: 0,
        unknown_b8f3: 0,
        unknown_process_b8f7: 0,
    });
    assert!(BoardSnapshot::new(changed)
        .unwrap_err()
        .0
        .contains("omit original"));
    let mut changed = original;
    changed.unlinked_candidates.push(UnlinkedCandidate {
        raw_location: RawUnitLocation {
            array_group: 2,
            array_slot: 1,
        },
        card_id: 12,
        column_raw: 3,
        row_raw: 5,
        constant_info_pointer: 0x2BE400 + 12 * 20,
        side_field_raw: 0,
        slot_field_raw: 1,
    });
    assert!(BoardSnapshot::new(changed)
        .unwrap_err()
        .0
        .contains("omit original"));
}

#[test]
fn selection_geometry_respects_resize_bounds_and_outer_edges() {
    for tile in [1, 44, 91] {
        let layout = BoardLayout {
            left: 25,
            top: 80,
            tile,
        };
        assert_eq!(layout.hit(25, 80), Some(GridPos { column: 0, row: 0 }));
        assert_eq!(
            layout.hit(25 + tile * 7 - 1, 80 + tile * 7 - 1),
            Some(GridPos { column: 6, row: 6 })
        );
        assert_eq!(layout.hit(24, 80), None);
        assert_eq!(layout.hit(25 + tile * 7, 80), None);
    }
    assert!(BoardLayout {
        left: 0,
        top: 0,
        tile: 0
    }
    .hit(0, 0)
    .is_none());
}

#[test]
#[ignore = "requires private captures via ROSES_VIEWER_CAPTURE_ROOT"]
fn private_opening_and_summon_captures_have_two_and_three_occupants() {
    let root = std::path::PathBuf::from(
        std::env::var_os("ROSES_VIEWER_CAPTURE_ROOT").expect("set ROSES_VIEWER_CAPTURE_ROOT"),
    );
    for (file, count) in [
        ("practice-first-saved-20260927.json", 2),
        ("practice-summoned-saved-20260927.json", 3),
    ] {
        let snapshot = from_saved_capture(&read_bounded(&root.join(file)).unwrap()).unwrap();
        assert_eq!(snapshot.document().units.len(), count);
        assert_eq!(snapshot.document().lp_candidates_by_raw_group, [4000, 4000]);
        if count == 3 {
            let unit = snapshot.unit(1).unwrap();
            assert_eq!(unit.card_id, 156);
            assert_eq!(unit.position, GridPos { column: 3, row: 5 });
            assert_eq!(snapshot.document().unlinked_candidates.len(), 6);
        }
    }
}
