use roses_compare::{
    compare, Checkpoint, ComparisonStatus, EvidenceKind, Provenance, SnapshotDocument,
    SnapshotMetadata, SCHEMA_VERSION,
};
use serde_json::json;

fn fixture() -> SnapshotDocument {
    SnapshotDocument {
        schema_version: SCHEMA_VERSION,
        scenario: "synthetic-observer-test".into(),
        profile: "synthetic-fields-v1".into(),
        metadata: SnapshotMetadata {
            disc_sha256: "0".repeat(64),
            executable_sha256: "1".repeat(64),
            observer_version: "test-1".into(),
            provenance: Provenance::Synthetic,
        },
        required_fields: vec!["/counter".into(), "/items".into()],
        checkpoints: vec![
            Checkpoint {
                id: "before".into(),
                observed: json!({"counter": 10, "items": [{"id": 4}]}),
            },
            Checkpoint {
                id: "after".into(),
                observed: json!({"counter": 11, "items": [{"id": 5}]}),
            },
        ],
    }
}

#[test]
fn equality_is_explicitly_scoped_and_synthetic() {
    let left = fixture();
    let encoded = serde_json::to_vec(&left).unwrap();
    let right = serde_json::from_slice(&encoded).unwrap();
    let report = compare(&left, &right).unwrap();
    assert_eq!(report.status, ComparisonStatus::Equal);
    assert_eq!(report.evidence, EvidenceKind::Synthetic);
    assert_eq!(report.checkpoints_compared, 2);
    assert_eq!(report.observed_leaves_compared, 4);
    assert!(report.first_difference.is_none());
    assert!(report.scope.contains("no claim of complete game parity"));
}

#[test]
fn reports_first_checkpoint_and_nested_json_pointer() {
    let left = fixture();
    let mut right = left.clone();
    right.checkpoints[0].observed["items"][0]["id"] = json!(99);
    right.checkpoints[1].observed["counter"] = json!(888);
    let report = compare(&left, &right).unwrap();
    assert_eq!(report.status, ComparisonStatus::Divergent);
    let diff = report.first_difference.unwrap();
    assert_eq!(diff.checkpoint_index, 0);
    assert_eq!(diff.checkpoint_id, "before");
    assert_eq!(diff.path, "/items/0/id");
    assert_eq!(diff.left, Some(json!(4)));
    assert_eq!(diff.right, Some(json!(99)));
}

#[test]
fn escapes_json_pointer_keys_and_checks_unrequired_observations() {
    let mut left = fixture();
    left.checkpoints[0].observed["a/b~c"] = json!({"value": 1});
    let mut right = left.clone();
    right.checkpoints[0].observed["a/b~c"]["value"] = json!(2);
    assert_eq!(
        compare(&left, &right)
            .unwrap()
            .first_difference
            .unwrap()
            .path,
        "/a~1b~0c/value"
    );
}

#[test]
fn unknown_on_both_sides_is_incomplete() {
    let mut left = fixture();
    left.checkpoints[0].observed["counter"] = json!(null);
    let report = compare(&left, &left).unwrap();
    assert_eq!(report.status, ComparisonStatus::Incomplete);
    assert_eq!(report.first_difference.unwrap().left, Some(json!(null)));
}

#[test]
fn missing_on_both_sides_is_incomplete() {
    let mut left = fixture();
    left.required_fields.push("/not_yet_understood".into());
    let report = compare(&left, &left).unwrap();
    assert_eq!(report.status, ComparisonStatus::Incomplete);
    let diff = report.first_difference.unwrap();
    assert_eq!(diff.path, "/not_yet_understood");
    assert_eq!(diff.left, None);
    assert_eq!(diff.right, None);
}

#[test]
fn unilateral_missing_observation_is_incomplete() {
    let left = fixture();
    let mut right = left.clone();
    right.checkpoints[0]
        .observed
        .as_object_mut()
        .unwrap()
        .remove("counter");
    assert_eq!(
        compare(&left, &right).unwrap().status,
        ComparisonStatus::Incomplete
    );
}

#[test]
fn empty_observation_cannot_pass() {
    let mut left = fixture();
    left.checkpoints[0].observed = json!({});
    assert_eq!(
        compare(&left, &left).unwrap().status,
        ComparisonStatus::Incomplete
    );
}

#[test]
fn explicit_empty_array_is_observed_but_type_difference_fails() {
    let mut left = fixture();
    left.checkpoints[0].observed["items"] = json!([]);
    assert_eq!(
        compare(&left, &left).unwrap().status,
        ComparisonStatus::Equal
    );
    let mut right = left.clone();
    right.checkpoints[0].observed["counter"] = json!("10");
    assert_eq!(
        compare(&left, &right).unwrap().status,
        ComparisonStatus::Divergent
    );
}

#[test]
fn incompatible_documents_fail_before_field_comparison() {
    let left = fixture();
    type Mutation = Box<dyn Fn(&mut SnapshotDocument)>;
    let changes: Vec<(&str, Mutation)> = vec![
        ("schema_version", Box::new(|d| d.schema_version = 99)),
        (
            "scenario mismatch",
            Box::new(|d| d.scenario = "another".into()),
        ),
        (
            "profile mismatch",
            Box::new(|d| d.profile = "another".into()),
        ),
        (
            "disc_sha256 mismatch",
            Box::new(|d| d.metadata.disc_sha256 = "2".repeat(64)),
        ),
        (
            "executable_sha256 mismatch",
            Box::new(|d| d.metadata.executable_sha256 = "2".repeat(64)),
        ),
        (
            "coverage mismatch",
            Box::new(|d| d.required_fields.pop().map(|_| ()).unwrap()),
        ),
        (
            "checkpoint count mismatch",
            Box::new(|d| d.checkpoints.pop().map(|_| ()).unwrap()),
        ),
        (
            "checkpoint ID mismatch",
            Box::new(|d| d.checkpoints[0].id = "different".into()),
        ),
    ];
    for (expected, change) in changes {
        let mut right = left.clone();
        change(&mut right);
        right.checkpoints[0].observed["counter"] = json!(999);
        assert!(
            compare(&left, &right)
                .unwrap_err()
                .to_string()
                .contains(expected),
            "{expected}"
        );
    }
}

#[test]
fn observed_array_length_changes_are_divergent() {
    let left = fixture();
    let mut right = left.clone();
    right.checkpoints[0].observed["items"] = json!([{ "id": 4 }, { "id": 6 }]);
    let report = compare(&left, &right).unwrap();
    assert_eq!(report.status, ComparisonStatus::Divergent);
    assert_eq!(report.first_difference.unwrap().path, "/items");
}

#[test]
fn invalid_coverage_and_empty_or_duplicate_checkpoints_fail() {
    let left = fixture();
    for fields in [
        vec![],
        vec!["counter".into()],
        vec!["/bad~2key".into()],
        vec!["/counter".into(), "/counter".into()],
    ] {
        let mut right = left.clone();
        right.required_fields = fields;
        assert!(compare(&left, &right).is_err());
    }
    let mut right = left.clone();
    right.checkpoints.clear();
    assert!(compare(&left, &right).is_err());
    let mut right = left.clone();
    right.checkpoints[1].id = "before".into();
    assert!(compare(&left, &right).is_err());
}

#[test]
fn provenance_is_preserved_and_never_upgrades_synthetic_evidence() {
    for (a, b, evidence) in [
        (
            Provenance::Original,
            Provenance::Native,
            EvidenceKind::OriginalVersusNative,
        ),
        (
            Provenance::Native,
            Provenance::Original,
            EvidenceKind::OriginalVersusNative,
        ),
        (
            Provenance::Original,
            Provenance::Original,
            EvidenceKind::OriginalRepeatability,
        ),
        (
            Provenance::Native,
            Provenance::Native,
            EvidenceKind::NativeRepeatability,
        ),
        (
            Provenance::Synthetic,
            Provenance::Original,
            EvidenceKind::Synthetic,
        ),
        (
            Provenance::Native,
            Provenance::Synthetic,
            EvidenceKind::Synthetic,
        ),
    ] {
        let mut left = fixture();
        let mut right = fixture();
        left.metadata.provenance = a;
        right.metadata.provenance = b;
        assert_eq!(compare(&left, &right).unwrap().evidence, evidence);
    }
}

#[test]
fn required_field_order_and_hash_case_are_not_semantic_differences() {
    let mut left = fixture();
    left.metadata.disc_sha256 = "a".repeat(64);
    let mut right = left.clone();
    right.required_fields.reverse();
    right.metadata.disc_sha256.make_ascii_uppercase();
    right.metadata.observer_version = "different-observer".into();
    assert_eq!(
        compare(&left, &right).unwrap().status,
        ComparisonStatus::Equal
    );
}
