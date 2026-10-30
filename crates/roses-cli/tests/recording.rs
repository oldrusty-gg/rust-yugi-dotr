//! CLI boundary tests use invented PK-prefixed bytes, deliberately NOT a valid
//! ZIP or PCSX2 savestate. They test preparation/copying, never emulator replay.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static CASE_ID: AtomicU64 = AtomicU64::new(0);
const SYNTHETIC_STATE: &[u8] = b"PK\x03\x04SYNTHETIC INVALID STATE: preparation test only";

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "roses recording cli test {} {stamp} {}",
            std::process::id(),
            CASE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("source state.p2s"), SYNTHETIC_STATE).unwrap();
        let fixture = Self { root };
        fixture.sequence(&valid_sequence());
        fixture
    }

    fn sequence(&self, value: &Value) {
        fs::write(
            self.root.join("sequence.json"),
            serde_json::to_vec(value).unwrap(),
        )
        .unwrap();
    }

    fn prepare(&self, directory: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_roses"))
            .arg("prepare-recording")
            .arg("--sequence")
            .arg(self.root.join("sequence.json"))
            .arg("--state")
            .arg(self.root.join("source state.p2s"))
            .arg("--out")
            .arg(self.root.join(directory))
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Delete only the exact files this fixture can create; no recursive
        // cleanup or traversal through caller-provided paths is used.
        for directory in ["bundle", "second-bundle"] {
            let directory = self.root.join(directory);
            for name in [
                "input.p2m2",
                "input.p2m2_SaveState.p2s",
                "manifest.json",
                "keep.txt",
            ] {
                let _ = fs::remove_file(directory.join(name));
            }
            let _ = fs::remove_dir(directory);
        }
        let _ = fs::remove_file(self.root.join("source state.p2s"));
        let _ = fs::remove_file(self.root.join("sequence.json"));
        let _ = fs::remove_dir(&self.root);
    }
}

fn valid_sequence() -> Value {
    json!({
        "schema_version": 1,
        "metadata": {
            "emulator_version": "PCSX2-v2.8.2",
            "author": "Synthetic CLI test",
            "game_name": "Synthetic game"
        },
        "segments": [
            {"frames": 2, "buttons": []},
            {"frames": 1, "buttons": ["cross"]},
            {"frames": 3, "buttons": []}
        ]
    })
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn preparation_preserves_inputs_and_hashes_exact_copied_bytes_without_replay_claim() {
    let fixture = Fixture::new();
    let original_sequence = fs::read(fixture.root.join("sequence.json")).unwrap();
    let output = fixture.prepare("bundle");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let directory = fixture.root.join("bundle");
    let movie = fs::read(directory.join("input.p2m2")).unwrap();
    let copied_state = fs::read(directory.join("input.p2m2_SaveState.p2s")).unwrap();
    let manifest: Value =
        serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["schema_version"], "roses-recording-bundle-1");
    assert_eq!(manifest["movie_sha256"], sha256(&movie));
    assert_eq!(manifest["state_sha256"], sha256(&copied_state));
    assert_eq!(manifest["state_file"], "input.p2m2_SaveState.p2s");
    assert_eq!(manifest["input"]["total_frames"], 6);
    assert_eq!(manifest["input"]["total_bytes"], movie.len());
    assert_eq!(manifest["replay_verified"], false);
    assert_eq!(manifest["gameplay_validated"], false);
    assert_eq!(manifest["input"]["replay_verified"], false);
    assert_eq!(
        manifest["input"]["segments"][1]["buttons"],
        json!(["cross"])
    );
    assert_eq!(copied_state, SYNTHETIC_STATE);
    assert_eq!(
        fs::read(fixture.root.join("source state.p2s")).unwrap(),
        SYNTHETIC_STATE
    );
    assert_eq!(
        fs::read(fixture.root.join("sequence.json")).unwrap(),
        original_sequence
    );
    assert_eq!(&movie[561..565], &[6, 0, 0, 0]);
    assert_eq!(movie.len(), 570 + 6 * 36);
}

#[test]
fn separate_bundles_are_deterministic_and_reusing_directory_cannot_overwrite_evidence() {
    let fixture = Fixture::new();
    assert!(fixture.prepare("bundle").status.success());
    assert!(fixture.prepare("second-bundle").status.success());
    for name in ["input.p2m2", "input.p2m2_SaveState.p2s", "manifest.json"] {
        assert_eq!(
            fs::read(fixture.root.join("bundle").join(name)).unwrap(),
            fs::read(fixture.root.join("second-bundle").join(name)).unwrap()
        );
    }
    let movie = fixture.root.join("bundle/input.p2m2");
    fs::write(&movie, b"preserved existing evidence").unwrap();
    let original_manifest = fs::read(fixture.root.join("bundle/manifest.json")).unwrap();
    let output = fixture.prepare("bundle");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(fs::read(&movie).unwrap(), b"preserved existing evidence");
    assert_eq!(
        fs::read(fixture.root.join("bundle/manifest.json")).unwrap(),
        original_manifest
    );
}

#[test]
fn existing_unrelated_directory_is_rejected_without_adding_files() {
    let fixture = Fixture::new();
    let directory = fixture.root.join("bundle");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("keep.txt"), b"unrelated user data").unwrap();
    assert_eq!(fixture.prepare("bundle").status.code(), Some(2));
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    assert_eq!(
        fs::read(directory.join("keep.txt")).unwrap(),
        b"unrelated user data"
    );
}

#[test]
fn incompatible_or_misspelled_sequence_schemas_fail_before_output_creation() {
    for change in 0..6 {
        let fixture = Fixture::new();
        let mut sequence = valid_sequence();
        match change {
            0 => sequence["schema_version"] = json!(2),
            1 => sequence["unexpected"] = json!(true),
            2 => sequence["metadata"]["game_namme"] = json!("typo"),
            3 => sequence["segments"][0]["framez"] = json!(2),
            4 => sequence["segments"][1]["buttons"] = json!(["not_a_button"]),
            _ => sequence["segments"][2]["buttons"] = json!(["start"]),
        }
        fixture.sequence(&sequence);
        let output = fixture.prepare("bundle");
        assert_eq!(output.status.code(), Some(2), "case {change}");
        assert!(!fixture.root.join("bundle").exists());
        assert_eq!(
            fs::read(fixture.root.join("source state.p2s")).unwrap(),
            SYNTHETIC_STATE
        );
    }
}

#[test]
fn missing_or_non_zip_state_cannot_leave_an_output_bundle() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.join("source state.p2s"),
        b"not a ZIP signature",
    )
    .unwrap();
    assert_eq!(fixture.prepare("bundle").status.code(), Some(2));
    assert!(!fixture.root.join("bundle").exists());
    fs::remove_file(fixture.root.join("source state.p2s")).unwrap();
    assert_eq!(fixture.prepare("bundle").status.code(), Some(2));
    assert!(!fixture.root.join("bundle").exists());
}
