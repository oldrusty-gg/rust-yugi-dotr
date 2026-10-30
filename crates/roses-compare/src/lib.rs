//! Compare explicitly observed checkpoint values. Equality is limited to the
//! supplied observations; it is never evidence of complete gameplay parity.

pub mod pine;
pub mod recording;
pub mod state;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Original,
    Native,
    Synthetic,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotMetadata {
    pub disc_sha256: String,
    pub executable_sha256: String,
    /// Version of the observer producing this file, not the game version.
    pub observer_version: String,
    pub provenance: Provenance,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDocument {
    pub schema_version: u32,
    pub scenario: String,
    /// Identifies a versioned definition of normalized fields and their meaning.
    pub profile: String,
    pub metadata: SnapshotMetadata,
    /// JSON pointers required at every checkpoint. Null means unknown.
    pub required_fields: Vec<String>,
    pub checkpoints: Vec<Checkpoint>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub id: String,
    /// An object containing only fields the observer actually understood.
    /// Omit an unavailable field or use null; neither is a successful match.
    pub observed: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonStatus {
    Equal,
    Divergent,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Synthetic,
    OriginalRepeatability,
    NativeRepeatability,
    OriginalVersusNative,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Difference {
    pub checkpoint_index: usize,
    pub checkpoint_id: String,
    /// JSON pointer within this checkpoint's observed object.
    pub path: String,
    /// Missing field is None; an explicitly unknown field is Some(Null).
    pub left: Option<Value>,
    pub right: Option<Value>,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub status: ComparisonStatus,
    pub evidence: EvidenceKind,
    pub checkpoints_compared: usize,
    pub observed_leaves_compared: usize,
    pub first_difference: Option<Difference>,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareError(pub String);

impl fmt::Display for CompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CompareError {}

fn invalid(message: impl Into<String>) -> CompareError {
    CompareError(message.into())
}

fn valid_pointer(pointer: &str) -> bool {
    if !pointer.starts_with('/') {
        return false;
    }
    let mut chars = pointer.chars();
    while let Some(ch) = chars.next() {
        if ch == '~' && !matches!(chars.next(), Some('0' | '1')) {
            return false;
        }
    }
    true
}

fn validate(document: &SnapshotDocument, side: &str) -> Result<(), CompareError> {
    if document.schema_version != SCHEMA_VERSION {
        return Err(invalid(format!(
            "{side}: unsupported schema_version {}; expected {SCHEMA_VERSION}",
            document.schema_version
        )));
    }
    for (name, value) in [
        ("scenario", &document.scenario),
        ("profile", &document.profile),
        ("observer_version", &document.metadata.observer_version),
    ] {
        if value.trim().is_empty() {
            return Err(invalid(format!("{side}: {name} must not be empty")));
        }
    }
    for (name, hash) in [
        ("disc_sha256", &document.metadata.disc_sha256),
        ("executable_sha256", &document.metadata.executable_sha256),
    ] {
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid(format!(
                "{side}: {name} must contain 64 hexadecimal digits"
            )));
        }
    }
    if document.required_fields.is_empty() {
        return Err(invalid(format!(
            "{side}: required_fields must explicitly declare coverage"
        )));
    }
    let mut fields = BTreeSet::new();
    for field in &document.required_fields {
        if !valid_pointer(field) || !fields.insert(field) {
            return Err(invalid(format!(
                "{side}: invalid or duplicate required JSON pointer {field:?}"
            )));
        }
    }
    if document.checkpoints.is_empty() {
        return Err(invalid(format!("{side}: checkpoints must not be empty")));
    }
    let mut ids = BTreeSet::new();
    for checkpoint in &document.checkpoints {
        if checkpoint.id.trim().is_empty() || !ids.insert(&checkpoint.id) {
            return Err(invalid(format!(
                "{side}: checkpoint IDs must be nonempty and unique"
            )));
        }
        if !checkpoint.observed.is_object() {
            return Err(invalid(format!(
                "{side}: checkpoint {:?} observed must be an object",
                checkpoint.id
            )));
        }
    }
    Ok(())
}

fn escape_pointer(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

struct FieldDifference {
    path: String,
    left: Option<Value>,
    right: Option<Value>,
    incomplete: bool,
    reason: &'static str,
}

fn field_difference(
    path: &str,
    left: Option<&Value>,
    right: Option<&Value>,
    count: &mut usize,
) -> Option<FieldDifference> {
    let fail = |incomplete, reason| FieldDifference {
        path: path.to_owned(),
        left: left.cloned(),
        right: right.cloned(),
        incomplete,
        reason,
    };
    let (Some(a), Some(b)) = (left, right) else {
        return Some(fail(true, "field is missing from one or both observations"));
    };
    if a.is_null() || b.is_null() {
        return Some(fail(true, "null denotes an unknown observation"));
    }
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            if a.is_empty() && b.is_empty() {
                return Some(fail(true, "empty objects contain no observed fields"));
            }
            let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
            for key in keys {
                let child = format!("{path}/{}", escape_pointer(key));
                if let Some(diff) = field_difference(&child, a.get(key), b.get(key), count) {
                    return Some(diff);
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for index in 0..a.len().min(b.len()) {
                let child = format!("{path}/{index}");
                if let Some(diff) = field_difference(&child, a.get(index), b.get(index), count) {
                    return Some(diff);
                }
            }
            if a.len() != b.len() {
                return Some(fail(false, "observed array lengths differ"));
            }
            if a.is_empty() && b.is_empty() {
                *count += 1;
            }
        }
        _ => {
            *count += 1;
            if a != b {
                return Some(fail(false, "observed values differ"));
            }
        }
    }
    None
}

/// Compare ordered checkpoints, then JSON fields in lexical order. Both the
/// required field set and every supplied observed field participate. Unknown or
/// missing observations stop with `Incomplete`, even when both sides omit them.
pub fn compare(
    left: &SnapshotDocument,
    right: &SnapshotDocument,
) -> Result<ComparisonReport, CompareError> {
    validate(left, "left")?;
    validate(right, "right")?;
    for (name, a, b) in [
        ("scenario", &left.scenario, &right.scenario),
        ("profile", &left.profile, &right.profile),
    ] {
        if a != b {
            return Err(invalid(format!("{name} mismatch: {a:?} versus {b:?}")));
        }
    }
    for (name, a, b) in [
        (
            "disc_sha256",
            &left.metadata.disc_sha256,
            &right.metadata.disc_sha256,
        ),
        (
            "executable_sha256",
            &left.metadata.executable_sha256,
            &right.metadata.executable_sha256,
        ),
    ] {
        if !a.eq_ignore_ascii_case(b) {
            return Err(invalid(format!("{name} mismatch: {a} versus {b}")));
        }
    }
    let required: BTreeSet<_> = left.required_fields.iter().collect();
    if required != right.required_fields.iter().collect() {
        return Err(invalid("required_fields coverage mismatch"));
    }
    if left.checkpoints.len() != right.checkpoints.len() {
        return Err(invalid(format!(
            "checkpoint count mismatch: {} versus {}",
            left.checkpoints.len(),
            right.checkpoints.len()
        )));
    }
    for (index, (a, b)) in left.checkpoints.iter().zip(&right.checkpoints).enumerate() {
        if a.id != b.id {
            return Err(invalid(format!(
                "checkpoint ID mismatch at index {index}: {:?} versus {:?}",
                a.id, b.id
            )));
        }
    }
    let evidence = match (left.metadata.provenance, right.metadata.provenance) {
        (Provenance::Synthetic, _) | (_, Provenance::Synthetic) => EvidenceKind::Synthetic,
        (Provenance::Original, Provenance::Original) => EvidenceKind::OriginalRepeatability,
        (Provenance::Native, Provenance::Native) => EvidenceKind::NativeRepeatability,
        _ => EvidenceKind::OriginalVersusNative,
    };
    let mut report = ComparisonReport {
        status: ComparisonStatus::Equal,
        evidence,
        checkpoints_compared: 0,
        observed_leaves_compared: 0,
        first_difference: None,
        scope: "Exact comparison of declared observations only; no claim of complete game parity. Provenance is supplied by the observer, not independently authenticated.".into(),
    };
    for (index, (a, b)) in left.checkpoints.iter().zip(&right.checkpoints).enumerate() {
        // Find the earliest observed discrepancy, while also catching fields
        // required by the profile but missing from both objects.
        let mut diff = field_difference(
            "",
            Some(&a.observed),
            Some(&b.observed),
            &mut report.observed_leaves_compared,
        );
        for path in &required {
            let mut ignored_count = 0;
            if let Some(required_diff) = field_difference(
                path,
                a.observed.pointer(path),
                b.observed.pointer(path),
                &mut ignored_count,
            ) {
                if diff
                    .as_ref()
                    .is_none_or(|current| required_diff.path < current.path)
                {
                    diff = Some(required_diff);
                }
            }
        }
        report.checkpoints_compared = index + 1;
        if let Some(diff) = diff {
            report.status = if diff.incomplete {
                ComparisonStatus::Incomplete
            } else {
                ComparisonStatus::Divergent
            };
            report.first_difference = Some(Difference {
                checkpoint_index: index,
                checkpoint_id: a.id.clone(),
                path: diff.path,
                left: diff.left,
                right: diff.right,
                reason: diff.reason.into(),
            });
            break;
        }
    }
    Ok(report)
}
