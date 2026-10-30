//! The imported card library (`<import>/library`): the manifest, card
//! names, catalogue and rules metadata an import writes, checked against the
//! supported PAL disc's fingerprints and counts and joined into one card
//! list. The importer's `verify-import` command (what the launcher runs
//! after an import) and the developer viewer read imports through this.

use crate::{CARD_COUNT, CARD_NAME_FIRST_MESSAGE, FUSION_COUNT, MAP_COUNT, PAL_ENGLISH_MESSAGE_SHA256};
use roses_disc::{PAL_EXECUTABLE_SHA256, PAL_ISO_SHA256, PAL_PROFILE_ID};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const MAX_JSON_BYTES: u64 = 2 * 1024 * 1024;
const MAX_RULES_JSON_BYTES: u64 = 8 * 1024 * 1024;
const MAX_LIBRARY_DIRECTORIES: usize = 1024;
const REQUIRED_FILES: [&str; 8] = [
    "manifest.json",
    "disc.json",
    "catalog.json",
    "rules.json",
    "names.json",
    "card-art-preview.png",
    "picture-0000.png",
    "terrain-map-preview.svg",
];

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub profile: String,
    pub iso_sha256: String,
    pub message_sha256: String,
    pub catalog_records: usize,
    pub card_names: usize,
    pub card_name_language: String,
    pub fusion_records: usize,
    pub terrain_maps: usize,
    pub complete_game_import: bool,
    pub gameplay_validated: bool,
}

#[derive(Debug)]
pub struct Library {
    pub directory: PathBuf,
    pub manifest: Manifest,
    pub cards: Vec<Card>,
}

#[derive(Debug)]
pub struct Card {
    pub id: u16,
    pub name: String,
    pub kind: u8,
    pub attribute: u8,
    pub level: u8,
    pub attack: u16,
    pub defense: u16,
    pub deck_cost: u8,
}

#[derive(Deserialize)]
struct NameFile {
    schema_version: u32,
    profile_id: String,
    message_sha256: String,
    language: String,
    gameplay_validated: bool,
    names: Vec<Name>,
}
#[derive(Deserialize)]
struct Name {
    id: u16,
    message_id: u16,
    name: String,
}
#[derive(Deserialize)]
struct Catalog {
    schema_version: u32,
    profile_id: String,
    executable_sha256: String,
    gameplay_validated: bool,
    cards: Vec<Stats>,
}
#[derive(Deserialize)]
struct RulesMetadata {
    schema_version: u32,
    profile_id: String,
    executable_sha256: String,
    gameplay_validated: bool,
    fusions: Vec<serde::de::IgnoredAny>,
    maps: Vec<serde::de::IgnoredAny>,
}
#[derive(Deserialize)]
struct Stats {
    id: u16,
    kind: u8,
    attribute: u8,
    level: u8,
    attack: u16,
    defense: u16,
    deck_cost: u8,
}

pub fn load_library(directory: &Path) -> Result<Library, String> {
    for name in REQUIRED_FILES {
        let path = directory.join(name);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "Required library file {} is unavailable: {error}",
                path.display()
            )
        })?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(format!(
                "Required library file {} is not a nonempty regular file.",
                path.display()
            ));
        }
    }
    let manifest: Manifest = read_json(&directory.join("manifest.json"))?;
    if manifest.schema_version != 1 || manifest.profile != PAL_PROFILE_ID {
        return Err("This library uses an unsupported manifest version or disc profile.".into());
    }
    if manifest.iso_sha256 != PAL_ISO_SHA256
        || manifest.message_sha256 != PAL_ENGLISH_MESSAGE_SHA256
    {
        return Err(
            "This library's recorded source fingerprints do not match the supported PAL import."
                .into(),
        );
    }
    if manifest.catalog_records != CARD_COUNT
        || manifest.card_names != CARD_COUNT
        || manifest.fusion_records != FUSION_COUNT
        || manifest.terrain_maps != MAP_COUNT
        || manifest.card_name_language != "en"
        || manifest.complete_game_import
        || manifest.gameplay_validated
    {
        return Err("This library does not match the supported partial PAL import.".into());
    }
    let names: NameFile = read_json(&directory.join("names.json"))?;
    let catalog: Catalog = read_json(&directory.join("catalog.json"))?;
    let rules: RulesMetadata =
        read_json_bounded(&directory.join("rules.json"), MAX_RULES_JSON_BYTES)?;
    if names.schema_version != 1
        || names.profile_id != PAL_PROFILE_ID
        || names.message_sha256 != PAL_ENGLISH_MESSAGE_SHA256
        || names.language != "en"
        || names.gameplay_validated
        || catalog.schema_version != 1
        || catalog.profile_id != PAL_PROFILE_ID
        || catalog.executable_sha256 != PAL_EXECUTABLE_SHA256
        || catalog.gameplay_validated
        || rules.schema_version != 1
        || rules.profile_id != PAL_PROFILE_ID
        || rules.executable_sha256 != PAL_EXECUTABLE_SHA256
        || rules.gameplay_validated
    {
        return Err("Imported metadata has an unsupported or inconsistent schema, profile, language, fingerprint, or gameplay claim.".into());
    }
    if rules.fusions.len() != manifest.fusion_records || rules.maps.len() != manifest.terrain_maps {
        return Err("Imported fusion/map counts disagree with the manifest.".into());
    }
    if names.names.len() != manifest.card_names || catalog.cards.len() != manifest.catalog_records {
        return Err("Imported card/name counts disagree with the manifest.".into());
    }
    let mut by_id = BTreeMap::new();
    for name in names.names {
        if name.name.trim().is_empty()
            || name.name.chars().count() > 256
            || name.name.chars().any(char::is_control)
            || usize::from(name.id) >= manifest.card_names
            || usize::from(name.message_id) != CARD_NAME_FIRST_MESSAGE + usize::from(name.id)
            || by_id.insert(name.id, name.name).is_some()
        {
            return Err("Imported card names contain an invalid or duplicate record.".into());
        }
    }
    let mut cards = Vec::with_capacity(catalog.cards.len());
    for stats in catalog.cards {
        let name = by_id
            .remove(&stats.id)
            .ok_or("Imported card IDs are missing or duplicated.")?;
        if stats.level > 12
            || stats.attribute > 5
            || stats.attack > 8191
            || stats.defense > 8191
            || !matches!(stats.kind, 0..=20 | 32 | 64 | 96 | 128 | 160)
        {
            return Err("Imported card statistics are outside the supported ranges.".into());
        }
        cards.push(Card {
            id: stats.id,
            name,
            kind: stats.kind,
            attribute: stats.attribute,
            level: stats.level,
            attack: stats.attack,
            defense: stats.defense,
            deck_cost: stats.deck_cost,
        });
    }
    cards.sort_by_key(|card| card.id);
    Ok(Library {
        directory: directory.to_path_buf(),
        manifest,
        cards,
    })
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    read_json_bounded(path, MAX_JSON_BYTES)
}

fn read_json_bounded<T: serde::de::DeserializeOwned>(path: &Path, limit: u64) -> Result<T, String> {
    let file =
        File::open(path).map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
    if file.metadata().map_err(|error| error.to_string())?.len() > limit {
        return Err(format!(
            "{} exceeds the {} MiB limit.",
            path.display(),
            limit / (1024 * 1024)
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Library metadata grew beyond the size limit.".into());
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("Invalid {}: {error}", path.display()))
}

/// Load the newest valid completed library. Invalid newer imports are skipped;
/// when none validate, a bounded diagnostic lists the first three failures.
/// Incomplete directories without a manifest are ignored.
pub fn latest_library(data_root: &Path) -> Result<Option<Library>, String> {
    let entries = match fs::read_dir(data_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Cannot read library folder: {error}")),
    };
    let mut candidates = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index >= MAX_LIBRARY_DIRECTORIES {
            return Err("Too many entries in the library folder (limit 1,024).".into());
        }
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        // A game import's library counts only once the whole game import finished.
        let (library, marker) = if name.starts_with("game-") {
            (entry.path().join("library"), entry.path().join("game.json"))
        } else if name.starts_with("import-") {
            (entry.path(), entry.path().join("manifest.json"))
        } else {
            continue;
        };
        if let Ok(metadata) = marker.metadata() {
            if metadata.is_file() {
                candidates.push((metadata.modified().unwrap_or(UNIX_EPOCH), library));
            }
        }
    }
    candidates.sort_by(|left, right| right.cmp(left));
    let count = candidates.len();
    let mut failures = Vec::new();
    for (_, directory) in candidates {
        match load_library(&directory) {
            Ok(library) => return Ok(Some(library)),
            Err(error) if failures.len() < 3 => {
                let diagnostic = format!("{}: {error}", directory.display());
                let bounded: String = diagnostic.chars().take(2048).collect();
                failures.push(bounded);
            }
            Err(_) => {}
        }
    }
    if count == 0 {
        Ok(None)
    } else {
        Err(format!(
            "None of {count} completed library candidates could be loaded.\n{}",
            failures.join("\n")
        ))
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join("roses-library-tests").join(format!(
                "game-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn write(&self, name: &str, value: &serde_json::Value) {
            fs::write(self.0.join(name), serde_json::to_vec(value).unwrap()).unwrap();
        }
        fn valid_library(&self) {
            self.write(
                "manifest.json",
                &serde_json::json!({
                    "schema_version": 1, "profile": PAL_PROFILE_ID,
                    "iso_sha256": PAL_ISO_SHA256, "message_sha256": PAL_ENGLISH_MESSAGE_SHA256,
                    "catalog_records": 854, "card_names": 854, "card_name_language": "en",
                    "fusion_records": 26540, "terrain_maps": 46,
                    "complete_game_import": false, "gameplay_validated": false
                }),
            );
            self.write("names.json", &synthetic_names());
            self.write(
                "catalog.json",
                &serde_json::json!({"schema_version": 1, "profile_id": PAL_PROFILE_ID,
                    "executable_sha256": PAL_EXECUTABLE_SHA256, "gameplay_validated": false,
                    "cards": (0..854).rev().map(|id| serde_json::json!({
                "id": id, "kind": 1, "attribute": 2, "level": 3,
                "attack": 400, "defense": 500, "deck_cost": 6
            })).collect::<Vec<_>>()}),
            );
            self.write(
                "rules.json",
                &serde_json::json!({
                    "schema_version": 1, "profile_id": PAL_PROFILE_ID,
                    "executable_sha256": PAL_EXECUTABLE_SHA256, "gameplay_validated": false,
                    "fusions": vec![serde_json::json!({}); FUSION_COUNT],
                    "maps": vec![serde_json::json!({}); MAP_COUNT]
                }),
            );
            // These files are existence-checked by the cache loader, not decoded.
            for name in [
                "disc.json",
                "card-art-preview.png",
                "picture-0000.png",
                "terrain-map-preview.svg",
            ] {
                fs::write(self.0.join(name), b"synthetic existence fixture").unwrap();
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // This path is a fresh directory created by Fixture::new, never a supplied path.
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn synthetic_names() -> serde_json::Value {
        serde_json::json!({"schema_version": 1, "profile_id": PAL_PROFILE_ID,
            "message_sha256": PAL_ENGLISH_MESSAGE_SHA256, "language": "en",
            "gameplay_validated": false, "names": (0..854).map(|id| serde_json::json!({
            "id": id, "message_id": CARD_NAME_FIRST_MESSAGE + id, "name": format!("Invented test card {id}")
        })).collect::<Vec<_>>()})
    }

    #[test]
    fn library_joins_by_id_and_rejects_duplicate_or_missing_names() {
        let fixture = Fixture::new();
        fixture.valid_library();
        let library = load_library(&fixture.0).unwrap();
        assert_eq!(library.cards.len(), 854);
        assert_eq!(library.cards[12].id, 12);
        assert_eq!(library.cards[12].name, "Invented test card 12");
        assert_eq!(library.cards[12].attack, 400);
        let mut names = synthetic_names();
        names["names"][853]["id"] = serde_json::json!(0);
        fixture.write("names.json", &names);
        assert!(load_library(&fixture.0).unwrap_err().contains("duplicate"));
        names["names"].as_array_mut().unwrap().pop();
        fixture.write("names.json", &names);
        assert!(load_library(&fixture.0).unwrap_err().contains("counts"));
    }

    #[test]
    fn oversized_metadata_is_rejected_and_incomplete_import_is_ignored() {
        let fixture = Fixture::new();
        assert!(latest_library(&fixture.0).unwrap().is_none());
        fs::create_dir(fixture.0.join("import-incomplete")).unwrap();
        assert!(latest_library(&fixture.0).unwrap().is_none());
        fixture.valid_library();
        File::create(fixture.0.join("manifest.json"))
            .unwrap()
            .set_len(MAX_JSON_BYTES + 1)
            .unwrap();
        assert!(load_library(&fixture.0).unwrap_err().contains("2 MiB"));
    }

    #[test]
    fn rejects_mixed_cache_metadata_and_missing_source_fingerprints() {
        let fixture = Fixture::new();
        fixture.valid_library();
        for (file, field, bad) in [
            ("manifest.json", "iso_sha256", serde_json::json!("wrong")),
            (
                "manifest.json",
                "message_sha256",
                serde_json::json!("wrong"),
            ),
            ("catalog.json", "schema_version", serde_json::json!(2)),
            ("catalog.json", "profile_id", serde_json::json!("other")),
            (
                "catalog.json",
                "executable_sha256",
                serde_json::json!("wrong"),
            ),
            ("names.json", "schema_version", serde_json::json!(2)),
            ("names.json", "profile_id", serde_json::json!("other")),
            ("names.json", "language", serde_json::json!("xx")),
            ("names.json", "message_sha256", serde_json::json!("wrong")),
            ("rules.json", "profile_id", serde_json::json!("other")),
            (
                "rules.json",
                "executable_sha256",
                serde_json::json!("wrong"),
            ),
        ] {
            let original: serde_json::Value = read_json(&fixture.0.join(file)).unwrap();
            let mut changed = original.clone();
            changed[field] = bad;
            fixture.write(file, &changed);
            assert!(load_library(&fixture.0).is_err(), "accepted {file}.{field}");
            changed.as_object_mut().unwrap().remove(field);
            fixture.write(file, &changed);
            assert!(
                load_library(&fixture.0).is_err(),
                "accepted missing {file}.{field}"
            );
            fixture.write(file, &original);
        }
    }

    #[test]
    fn rejects_missing_outputs_and_rules_count_disagreement() {
        let fixture = Fixture::new();
        fixture.valid_library();
        let mut rules: serde_json::Value = read_json(&fixture.0.join("rules.json")).unwrap();
        rules["maps"].as_array_mut().unwrap().pop();
        fixture.write("rules.json", &rules);
        assert!(load_library(&fixture.0)
            .unwrap_err()
            .contains("fusion/map counts"));
        fixture.valid_library();
        for name in [
            "rules.json",
            "card-art-preview.png",
            "picture-0000.png",
            "terrain-map-preview.svg",
        ] {
            let path = fixture.0.join(name);
            let original = fs::read(&path).unwrap();
            fs::remove_file(&path).unwrap();
            assert!(load_library(&fixture.0).unwrap_err().contains(name));
            fs::write(&path, original).unwrap();
        }
    }

    #[test]
    fn recovers_older_valid_library_and_reports_when_all_candidates_fail() {
        let root = Fixture::new();
        let older = Fixture(root.0.join("import-older"));
        let newer = Fixture(root.0.join("import-newer"));
        fs::create_dir(&older.0).unwrap();
        fs::create_dir(&newer.0).unwrap();
        older.valid_library();
        newer.valid_library();
        for (library, seconds) in [(&older, 10), (&newer, 20)] {
            File::options()
                .write(true)
                .open(library.0.join("manifest.json"))
                .unwrap()
                .set_times(
                    fs::FileTimes::new()
                        .set_modified(UNIX_EPOCH + std::time::Duration::from_secs(seconds)),
                )
                .unwrap();
        }
        assert_eq!(latest_library(&root.0).unwrap().unwrap().directory, newer.0);
        fs::write(newer.0.join("names.json"), b"{truncated").unwrap();
        assert_eq!(latest_library(&root.0).unwrap().unwrap().directory, older.0);
        fs::write(older.0.join("catalog.json"), b"{truncated").unwrap();
        let error = latest_library(&root.0).unwrap_err();
        assert!(error.contains("None of 2"));
        assert!(error.contains("import-newer"));
        assert!(error.contains("import-older"));
    }

    #[test]
    #[ignore = "requires a private local import, supplied through ROSES_TEST_LIBRARY"]
    fn private_import_loads_all_cards() {
        let path = std::env::var_os("ROSES_TEST_LIBRARY")
            .expect("set ROSES_TEST_LIBRARY");
        let library = load_library(Path::new(&path)).unwrap();
        assert_eq!(library.cards.len(), 854);
        assert_eq!(library.cards[0].id, 0);
        assert_eq!(library.cards[853].id, 853);
    }

}
