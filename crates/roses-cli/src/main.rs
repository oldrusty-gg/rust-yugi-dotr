use roses_compare::pine::{MemoryWatch, PineClient, ReadWidth, DEFAULT_PORT};
use roses_compare::{compare, ComparisonStatus, SnapshotDocument};
use roses_disc::Disc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod duel_catalogue;
mod destiny_check;
mod duel_check;
mod duel_text;
mod experience_check;
mod hud_check;
mod hud_engine_check;
mod hud_queries_check;
mod import_campaign_map;
mod import_duel_arena;
mod import_duel_audio;
mod import_duel;
mod import_duel_board;
mod import_duel_end;
mod import_duel_models;
mod import_game;
mod import_opening;
mod import_opening_scene;
mod import_post_name;
mod import_story;
mod import_story_map;
mod import_story_red;
mod import_story_white;
mod story_faces;
mod practice_check;
mod refusal_sweep;
mod simulate_move;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const HELP: &str = "Roses 0.1 — local disc and comparison tools

roses inspect --iso <game.iso> [--out <new-report.json>]
roses import --iso <game.iso> --out <new-directory>
roses import-game --iso <game.iso> --out <new-directory>
roses verify-import --import <game-import-directory> | --data-root <directory>
roses import-opening --iso <game.iso> --out <new-directory>
roses import-story --iso <game.iso> --opening <manifest.json> --out <new-directory>
roses import-post-name --iso <game.iso> --opening <name-entry manifest.json> --out <new-directory>
roses import-story-map --iso <game.iso> --post-name <post-name manifest.json> --out <new-directory>
roses import-story-red --iso <game.iso> --story <story-to-map manifest.json> --out <new-directory>
roses import-duel --iso <game.iso> --out <new-directory>
roses import-duel-board --iso <game.iso> --out <new-directory>
roses import-duel-end --iso <game.iso> --out <new-directory>
roses import-campaign-map --iso <game.iso> --out <new-directory>
roses render-story --manifest <manifest.json> --out <new-directory>
roses export-duel-cards --iso <game.iso> --out <cards.json>
roses duel --iso <game.iso> [--seed <number>] [--player human|computer]
roses fusion-lookup --iso <game.iso> --first <0..853> --second <0..853> [--out <new-report.json>]
roses verify-catalog --iso <game.iso> [--port <28011>] [--out <new-report.json>]
roses compare --reference <snapshot.json> --candidate <snapshot.json> [--out <new-report.json>]
roses pine-info [--port <28011>] [--out <new-report.json>]
roses capture --watches <watches.json> [--port <28011>] [--out <new-report.json>]
roses capture-state --state <reference.p2s> --watches <watches.json> [--out <new-report.json>]
roses prepare-recording --sequence <sequence.json> --state <reference.p2s> --out <new-directory>
roses simulate-move --capture <saved.json> --request <request.json> --out <new-directory>

The movies are kept as they are on the disc and decoded by the built-in Rust
decoder (crates/roses-movie); no external program is used.
The input ISO is read-only. Reports never overwrite existing files.
compare exits 0 for observed equality, 1 for a difference, 3 for incomplete
observations, and 2 for invalid input or other errors. Synthetic equality is
only a tool check, not original-game parity.
capture requires PCSX2 to be paused and explicit physical EE RAM addresses.
It produces raw observations, not an automatically verified game-state mapping.
capture-state reads explicit watches from archived EE RAM without a running
emulator. It does not infer game identity or field meanings from a savestate.
simulate-move computes one explicitly permitted movement from an imported
baseline; it accepts no expected endpoint and does not establish gameplay parity.
";

fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("roses: {error}");
            2
        }
    };
    std::process::exit(code);
}

fn run() -> Result<i32> {
    let mut arguments = std::env::args_os().skip(1);
    let Some(command) = arguments.next() else {
        print!("{HELP}");
        return Ok(0);
    };
    if command == "--help" || command == "-h" || command == "help" {
        print!("{HELP}");
        return Ok(0);
    }
    let mut options = Options::parse(arguments)?;
    match command.to_str() {
        Some("inspect") => {
            let iso = options.required_path("--iso")?;
            let output = options.path("--out");
            options.finish()?;
            eprintln!("Reading ISO directory and computing source fingerprints…");
            let report = Disc::open(iso)?.inspect()?;
            emit_json(&report, output.as_deref())?;
            Ok(0)
        }
        Some("compare") => {
            let reference = options.required_path("--reference")?;
            let candidate = options.required_path("--candidate")?;
            let output = options.path("--out");
            options.finish()?;
            let reference: SnapshotDocument = read_json(&reference)?;
            let candidate: SnapshotDocument = read_json(&candidate)?;
            let report = compare(&reference, &candidate)?;
            let code = match report.status {
                ComparisonStatus::Equal => 0,
                ComparisonStatus::Divergent => 1,
                ComparisonStatus::Incomplete => 3,
            };
            emit_json(&report, output.as_deref())?;
            Ok(code)
        }
        Some("import") => {
            let iso = options.required_path("--iso")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_assets(&iso, &output)?;
            Ok(0)
        }
        Some("import-game") => {
            let iso = options.required_path("--iso")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_game::run(&iso, &output)?;
            Ok(0)
        }
        Some("verify-import") => {
            let import = options.path("--import");
            let data_root = options.path("--data-root");
            options.finish()?;
            let report = match (import, data_root) {
                (Some(import), None) => verify_import(&import),
                (None, Some(root)) => newest_import(&root),
                _ => return Err("give exactly one of --import <directory> or --data-root <directory>".into()),
            };
            println!("{}", report.line());
            Ok(0)
        }
        Some("import-opening") => {
            let iso = options.required_path("--iso")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_opening::run(&iso, &output)?;
            Ok(0)
        }
        Some("import-story") => {
            let iso = options.required_path("--iso")?;
            let opening = options.required_path("--opening")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_story::run(&iso, &opening, &output)?;
            Ok(0)
        }
        Some("import-post-name") => {
            let iso = options.required_path("--iso")?;
            let opening = options.required_path("--opening")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_post_name::run(&iso, &opening, &output)?;
            Ok(0)
        }
        Some("import-story-map") => {
            let iso = options.required_path("--iso")?;
            let post_name = options.required_path("--post-name")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_story_map::run(&iso, &post_name, &output)?;
            Ok(0)
        }
        // roses import-story-red --iso <game.iso> --story <story-to-map manifest.json> --out <new-directory>
        Some("import-story-red") => {
            let iso = options.required_path("--iso")?;
            let story = options.required_path("--story")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_story_red::run(&iso, &story, &output)?;
            Ok(0)
        }
        Some("import-duel") => {
            let iso = options.required_path("--iso")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_duel::run(&iso, &output)?;
            Ok(0)
        }
        Some("import-campaign-map") => {
            let iso = options.required_path("--iso")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_campaign_map::run(&iso, &output)?;
            Ok(0)
        }
        Some("import-duel-arena") => {
            let iso = options.required_path("--iso")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_duel_arena::run(&iso, &output)?;
            Ok(0)
        }
        Some("import-duel-audio") => {
            let iso = options.required_path("--iso")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_duel_audio::run(&iso, &output)?;
            Ok(0)
        }
        Some("import-duel-board") => {
            let iso = options.required_path("--iso")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_duel_board::run(&iso, &output)?;
            Ok(0)
        }
        // roses import-opening-scene --iso <game.iso> --out <new-directory>
        //   [--pan 0..100] [--level 0..4096] [--sky-frame <f>] [--world story-scene-0]
        //   [--verify-against <mesh-scene-simon.json> [--capture-root <dir>]]
        Some("import-opening-scene") => {
            let iso = options.required_path("--iso")?;
            let output = options.required_path("--out")?;
            let verify_against = options.path("--verify-against");
            let capture_root = options
                .path("--capture-root")
                .unwrap_or_else(|| PathBuf::from("."));
            let number = |options: &mut Options, key: &str| -> Result<Option<f64>> {
                match options.0.remove(key) {
                    None => Ok(None),
                    Some(value) => Ok(Some(
                        value
                            .to_str()
                            .ok_or("option must be a number")?
                            .parse::<f64>()?,
                    )),
                }
            };
            let mut pose = import_opening_scene::default_pose();
            if let Some(pan) = number(&mut options, "--pan")? {
                pose.pan = i8::try_from(pan as i64).map_err(|_| "pan must be 0..=100")?;
            }
            if let Some(level) = number(&mut options, "--level")? {
                pose.level = i16::try_from(level as i64).map_err(|_| "level must be 0..=4096")?;
            }
            if let Some(frame) = number(&mut options, "--sky-frame")? {
                pose.sky_frame = frame as f32;
            }
            let world = match options.0.remove("--world") {
                None => import_opening_scene::World::Opening,
                Some(w) if w == "story-scene-0" => import_opening_scene::World::StoryScene0,
                Some(_) => return Err("--world must be story-scene-0".into()),
            };
            options.finish()?;
            import_opening_scene::run(&import_opening_scene::Options {
                world,
                iso,
                output,
                pose,
                verify_against,
                capture_root,
            })
        }
        // roses import-duel-end --iso <game.iso> --out <new-directory>
        Some("import-duel-end") => {
            let iso = options.required_path("--iso")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_duel_end::run(&iso, &output)?;
            Ok(0)
        }
        // roses import-duel-models --iso <game.iso> --out <new-directory> [--cards first-duel|all]
        Some("import-duel-models") => {
            let iso = options.required_path("--iso")?;
            let output = options.required_path("--out")?;
            let cards = options.path("--cards");
            options.finish()?;
            import_duel_models::run(&iso, &output, cards.as_deref())?;
            Ok(0)
        }
        Some("render-story") => {
            let manifest = options.required_path("--manifest")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            import_story::render(&manifest, &output)?;
            Ok(0)
        }
        Some("verify-catalog") => {
            let iso = options.required_path("--iso")?;
            let port = options.port()?;
            let output = options.path("--out");
            options.finish()?;
            verify_catalog(&iso, port, output.as_deref())
        }
        Some("duel") => {
            let iso = options.required_path("--iso")?;
            let seed = options.path("--seed");
            let player = options.path("--player");
            options.finish()?;
            let computer = match player.as_deref().map(|p| p.to_string_lossy().to_string()) {
                None => false,
                Some(p) if p == "computer" => true,
                Some(p) if p == "human" => false,
                Some(_) => return Err("--player must be human or computer".into()),
            };
            play_duel(&iso, seed.as_deref(), computer)
        }
        Some("export-duel-cards") => {
            let iso = options.required_path("--iso")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            export_duel_cards(&iso, &output)
        }
        Some("fusion-lookup") => {
            let iso = options.required_path("--iso")?;
            let first = options.card_id("--first")?;
            let second = options.card_id("--second")?;
            let output = options.path("--out");
            options.finish()?;
            fusion_lookup(&iso, first, second, output.as_deref())?;
            Ok(0)
        }
        Some("pine-info") => {
            let port = options.port()?;
            let output = options.path("--out");
            options.finish()?;
            let mut client = PineClient::connect_local(port, Duration::from_secs(3))?;
            emit_json(&client.identify()?, output.as_deref())?;
            Ok(0)
        }
        Some("capture") => {
            let watch_path = options.required_path("--watches")?;
            let port = options.port()?;
            let output = options.path("--out");
            options.finish()?;
            let watches: Vec<MemoryWatch> = read_json(&watch_path)?;
            let mut client = PineClient::connect_local(port, Duration::from_secs(3))?;
            let before = client.identify()?;
            let capture = client.read_watches(&watches)?;
            let after = client.identify()?;
            if serde_json::to_value(&before)? != serde_json::to_value(&after)? {
                return Err(
                    "emulator identity or status changed during capture; discard this observation"
                        .into(),
                );
            }
            let report = serde_json::json!({
                "schema_version": "roses-raw-pine-1",
                "captured_unix_ms": SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
                "emulator": before,
                "capture": capture,
                "normalized_game_state": false,
            });
            emit_json(&report, output.as_deref())?;
            Ok(0)
        }
        Some("capture-state") => {
            let state = options.required_path("--state")?;
            let watches = options.required_path("--watches")?;
            let output = options.path("--out");
            options.finish()?;
            let watches: Vec<MemoryWatch> = read_json(&watches)?;
            let capture = roses_compare::state::capture_state(&state, &watches)?;
            emit_json(&capture, output.as_deref())?;
            Ok(0)
        }
        Some("prepare-recording") => {
            let sequence = options.required_path("--sequence")?;
            let state = options.required_path("--state")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            prepare_recording(&sequence, &state, &output)?;
            Ok(0)
        }
        Some("simulate-move") => {
            let capture = options.required_path("--capture")?;
            let request = options.required_path("--request")?;
            let output = options.required_path("--out")?;
            options.finish()?;
            simulate_move::run(&capture, &request, &output)?;
            Ok(0)
        }
        _ => Err(format!("unknown command {:?}; run roses --help", command).into()),
    }
}

struct Options(BTreeMap<String, OsString>);

/// What the launcher says an import holds.
fn import_summary(library: &roses_assets::library::Library) -> String {
    format!(
        "{} cards, {} fusions, the story, campaign map and duels",
        library.manifest.catalog_records, library.manifest.fusion_records
    )
}

/// `verify-import --import <dir>`: the card library of one finished game
/// import (`<dir>/library`), for the launcher (platform_sdk::contract).
fn verify_import(import: &Path) -> platform_sdk::contract::ImportReport {
    use platform_sdk::contract::ImportReport;
    match roses_assets::library::load_library(&import.join("library")) {
        Ok(library) => ImportReport {
            ok: true,
            import_dir: Some(import.display().to_string()),
            summary: Some(import_summary(&library)),
            error: None,
        },
        Err(error) => ImportReport { ok: false, error: Some(error), ..ImportReport::default() },
    }
}

/// `verify-import --data-root <dir>`: the newest import whose library
/// checks out (older ones when newer ones do not; none yet is fine).
fn newest_import(data_root: &Path) -> platform_sdk::contract::ImportReport {
    use platform_sdk::contract::ImportReport;
    match roses_assets::library::latest_library(data_root) {
        Ok(Some(library)) => {
            let dir = &library.directory;
            let import = if dir.file_name().is_some_and(|n| n == "library") { dir.parent().unwrap_or(dir) } else { dir };
            ImportReport {
                ok: true,
                import_dir: Some(import.display().to_string()),
                summary: Some(import_summary(&library)),
                error: None,
            }
        }
        Ok(None) => ImportReport { ok: true, ..ImportReport::default() },
        Err(error) => ImportReport { ok: false, error: Some(error), ..ImportReport::default() },
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordingSequence {
    schema_version: u32,
    metadata: roses_compare::recording::RecordingMetadata,
    segments: Vec<roses_compare::recording::InputSegment>,
}

fn prepare_recording(sequence_path: &Path, state_path: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err("recording output must be a new directory".into());
    }
    let sequence: RecordingSequence = read_json(sequence_path)?;
    if sequence.schema_version != 1 {
        return Err("unsupported input sequence schema version".into());
    }
    let recording =
        roses_compare::recording::encode_recording(&sequence.metadata, &sequence.segments)?;
    let state = File::open(state_path)?;
    const MAX_STATE: u64 = 128 * 1024 * 1024;
    if state.metadata()?.len() > MAX_STATE {
        return Err("savestate exceeds 128 MiB".into());
    }
    let mut state_bytes = Vec::new();
    state.take(MAX_STATE + 1).read_to_end(&mut state_bytes)?;
    if state_bytes.len() as u64 > MAX_STATE || !state_bytes.starts_with(b"PK\x03\x04") {
        return Err("expected a bounded PCSX2 ZIP savestate; its internal compatibility is checked by PCSX2 at playback".into());
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    let movie_path = output.join("input.p2m2");
    let copied_state = roses_compare::recording::companion_state_path(&movie_path);
    recording.write_new(&movie_path)?;
    let mut state_output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&copied_state)?;
    state_output.write_all(&state_bytes)?;
    state_output.sync_all()?;
    emit_json(
        &serde_json::json!({
            "schema_version": "roses-recording-bundle-1",
            "input": recording.manifest(),
            "movie_file": "input.p2m2",
            "state_file": "input.p2m2_SaveState.p2s",
            "movie_sha256": format!("{:x}", Sha256::digest(recording.bytes())),
            "state_sha256": format!("{:x}", Sha256::digest(&state_bytes)),
            "replay_verified": false,
            "gameplay_validated": false,
            "scope": "Prepared input bytes and a copied savestate. This does not validate the savestate's game, complete replay, or establish repeatability. Replay a disposable copy: PCSX2 opens the movie writable and may switch to recording after the stored inputs.",
        }),
        Some(&output.join("manifest.json")),
    )?;
    eprintln!(
        "Prepared {} input frames in {}",
        recording.manifest().total_frames,
        output.display()
    );
    Ok(())
}

impl Options {
    fn parse(arguments: impl Iterator<Item = OsString>) -> Result<Self> {
        let mut arguments = arguments.peekable();
        let mut values = BTreeMap::new();
        while let Some(key) = arguments.next() {
            let key = key.to_str().ok_or("option names must be valid UTF-8")?;
            if !key.starts_with("--") {
                return Err(format!("expected an option, got {key:?}").into());
            }
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {key}"))?;
            if value.to_str().is_some_and(|v| v.starts_with("--")) {
                return Err(format!("missing value for {key}").into());
            }
            if values.insert(key.to_owned(), value).is_some() {
                return Err(format!("duplicate option {key}").into());
            }
        }
        Ok(Self(values))
    }

    fn path(&mut self, key: &str) -> Option<PathBuf> {
        self.0.remove(key).map(PathBuf::from)
    }

    fn required_path(&mut self, key: &str) -> Result<PathBuf> {
        self.path(key)
            .ok_or_else(|| format!("required option {key} is missing").into())
    }

    fn port(&mut self) -> Result<u16> {
        match self.0.remove("--port") {
            None => Ok(DEFAULT_PORT),
            Some(value) => Ok(value.to_str().ok_or("port must be a number")?.parse()?),
        }
    }

    fn card_id(&mut self, key: &str) -> Result<u16> {
        let value = self
            .0
            .remove(key)
            .ok_or_else(|| format!("required option {key} is missing"))?;
        let id: u16 = value
            .to_str()
            .ok_or("card IDs must be decimal integers")?
            .parse()
            .map_err(|_| {
                format!(
                    "{key} must be a zero-based decimal card ID in 0..{}",
                    roses_assets::CARD_COUNT - 1
                )
            })?;
        if usize::from(id) >= roses_assets::CARD_COUNT {
            return Err(format!(
                "{key} is outside zero-based card IDs 0..{}",
                roses_assets::CARD_COUNT - 1
            )
            .into());
        }
        Ok(id)
    }

    fn finish(self) -> Result<()> {
        if let Some(key) = self.0.keys().next() {
            return Err(format!("unrecognized option {key}").into());
        }
        Ok(())
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let file = File::open(path)?;
    if file.metadata()?.len() > 32 * 1024 * 1024 {
        return Err("JSON input exceeds 32 MiB".into());
    }
    Ok(serde_json::from_reader(BufReader::new(file))?)
}

fn emit_json(value: &impl Serialize, output: Option<&Path>) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    if let Some(path) = output {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        eprintln!("Saved {}", path.display());
    } else {
        io::stdout().lock().write_all(&bytes)?;
    }
    Ok(())
}

fn open_pal(path: &Path) -> Result<(Disc, roses_disc::DiscReport)> {
    eprintln!("Verifying the complete PAL ISO fingerprint…");
    let mut disc = Disc::open(path)?;
    let report = disc.inspect()?;
    if report.identification.status != roses_disc::IdentificationStatus::KnownRetailCandidate {
        return Err("This disc image doesn't match the supported PAL release (SLES_515.79). Modified or other-region images can't be imported.".into());
    }
    Ok((disc, report))
}

fn import_assets(iso: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err(
            "import output must be a new directory; existing data is never overwritten".into(),
        );
    }
    let (mut disc, report) = open_pal(iso)?;
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    let catalog = roses_assets::parse_pal_catalog(&executable)?;
    let rules = roses_assets::parse_pal_rules(&executable)?;
    let terrain_svg = terrain_preview(&rules.maps)?;
    let messages = disc.read_file("MSG/MSG_E.DAT", 1024 * 1024)?;
    let names = roses_assets::parse_pal_card_names(&messages)?;
    let pictures = disc.read_file("DATA/PICTURE.MRG", 32 * 1024 * 1024)?;
    if pictures.len() != roses_assets::PICTURE_RECORD_COUNT * roses_assets::PICTURE_RECORD_STRIDE {
        return Err("unexpected picture archive size for the supported PAL profile".into());
    }
    let mut previews = Vec::new();
    for index in 0..16 {
        previews.push(roses_assets::decode_picture(&pictures, index)?);
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    emit_json(&report, Some(&output.join("disc.json")))?;
    emit_json(&catalog, Some(&output.join("catalog.json")))?;
    emit_json(&rules, Some(&output.join("rules.json")))?;
    emit_json(&names, Some(&output.join("names.json")))?;
    let width = 4 * 128;
    let height = 4 * 128;
    let mut sheet = vec![0u8; (width * height * 4) as usize];
    for (index, preview) in previews.iter().enumerate() {
        if preview.width != 128 || preview.height != 128 {
            return Err("unexpected preview dimensions".into());
        }
        let tile_x = (index % 4) * 128;
        let tile_y = (index / 4) * 128;
        for row in 0..128 {
            let destination = ((tile_y + row) * width as usize + tile_x) * 4;
            let source = row * 128 * 4;
            sheet[destination..destination + 128 * 4]
                .copy_from_slice(&preview.pixels[source..source + 128 * 4]);
        }
    }
    write_png(&output.join("card-art-preview.png"), width, height, &sheet)?;
    write_png(
        &output.join("picture-0000.png"),
        128,
        128,
        &previews[0].pixels,
    )?;
    let mut terrain_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("terrain-map-preview.svg"))?;
    terrain_file.write_all(terrain_svg.as_bytes())?;
    terrain_file.sync_all()?;
    emit_json(
        &serde_json::json!({
            "schema_version": 1,
            "importer_version": env!("CARGO_PKG_VERSION"),
            "profile": roses_disc::PAL_PROFILE_ID,
            "iso_sha256": report.iso_sha256,
            "catalog_records": catalog.cards.len(),
            "card_names": names.names.len(),
            "card_name_language": names.language,
            "message_sha256": names.message_sha256,
            "fusion_records": rules.fusions.len(),
            "terrain_maps": rules.maps.len(),
            "picture_records_decoded": 16,
            "complete_game_import": false,
            "gameplay_validated": false,
            "fusion_behavior_validated": false,
            "map_orientation_verified": rules.map_orientation_verified,
            "map_opponent_mapping_verified": false,
            "scope": "Local card constants and English names, packed static fusion records, raw terrain maps and artwork previews. General dialogue and text rendering are not imported. Picture record order is retained. Terrain maps display source order as rows; in-game orientation is unverified. No complete artwork-to-card mapping, fusion-resolution behavior, terrain effects or other effect implementation is claimed.",
            "files": ["disc.json", "catalog.json", "rules.json", "names.json", "card-art-preview.png", "picture-0000.png", "terrain-map-preview.svg"]
        }),
        Some(&output.join("manifest.json")),
    )?;
    eprintln!("Imported {} card records, {} English names, {} static fusion records, {} terrain maps and a 16-picture preview into {}", catalog.cards.len(), names.names.len(), rules.fusions.len(), rules.maps.len(), output.display());
    Ok(())
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    writer.finish()?;
    Ok(())
}

fn terrain_preview(maps: &[roses_assets::TerrainMap]) -> Result<String> {
    const COLORS: [&str; 10] = [
        "#C7D3D4", "#7AAF72", "#D5AF70", "#8697C9", "#D28E9D", "#73B9CA", "#A48AC0", "#CFD888",
        "#EAA977", "#9DB39B",
    ];
    const COLUMNS: usize = 8;
    const TILE_WIDTH: usize = 140;
    const TILE_HEIGHT: usize = 162;
    const CELL: usize = 16;
    const LEFT: usize = 28;
    const TOP: usize = 156;
    if maps.is_empty() || maps.len() > roses_assets::MAP_COUNT {
        return Err("terrain preview requires a bounded, nonempty map list".into());
    }
    for map in maps {
        if usize::from(map.width) != roses_assets::MAP_SIDE
            || usize::from(map.height) != roses_assets::MAP_SIDE
            || map.cells.len() != roses_assets::MAP_SIDE * roses_assets::MAP_SIDE
            || map
                .cells
                .iter()
                .any(|code| usize::from(*code) >= COLORS.len())
        {
            return Err(
                "terrain preview encountered unsupported dimensions or terrain codes".into(),
            );
        }
    }
    let width = LEFT * 2 + COLUMNS * TILE_WIDTH;
    let height = TOP + maps.len().div_ceil(COLUMNS) * TILE_HEIGHT + 16;
    let mut svg = String::new();
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title description">"#
    )?;
    writeln!(
        svg,
        "<title id=\"title\">Imported terrain maps: raw stored order</title>"
    )?;
    writeln!(svg, "<desc id=\"description\">Locally imported terrain codes displayed in seven-cell rows. Board orientation, opponent assignments and terrain effects are unverified. Map numbers are zero-based table indices.</desc>")?;
    writeln!(
        svg,
        r##"<rect width="100%" height="100%" fill="#101B29"/><g font-family="Segoe UI, sans-serif" fill="#F0F3F7">"##
    )?;
    writeln!(
        svg,
        r#"<text x="28" y="42" font-size="24" font-weight="600">Terrain map catalog</text>"#
    )?;
    writeln!(
        svg,
        r##"<text x="28" y="68" font-size="13" fill="#B7C5D4">{} boards · 7 × 7 cells · zero-based map indices · numeric terrain codes</text>"##,
        maps.len()
    )?;
    writeln!(
        svg,
        r##"<text x="28" y="89" font-size="13" fill="#B7C5D4">Source order shown in rows. In-game orientation and opponent assignments remain unverified.</text>"##
    )?;
    for (code, color) in COLORS.iter().enumerate() {
        let x = LEFT + code * 105;
        writeln!(
            svg,
            r#"<rect x="{x}" y="111" width="20" height="20" rx="3" fill="{color}"/><text x="{}" y="126" font-size="12">Code {code}</text>"#,
            x + 29
        )?;
    }
    for (position, map) in maps.iter().enumerate() {
        let x = LEFT + (position % COLUMNS) * TILE_WIDTH;
        let y = TOP + (position / COLUMNS) * TILE_HEIGHT;
        writeln!(
            svg,
            r##"<g><rect x="{x}" y="{y}" width="128" height="148" rx="7" fill="#1C2A3C"/><text x="{}" y="{}" font-size="12" font-weight="600">Map {:03}</text>"##,
            x + 8,
            y + 20,
            map.index
        )?;
        for (index, code) in map.cells.iter().enumerate() {
            let cell_x = x + 8 + (index % roses_assets::MAP_SIDE) * CELL;
            let cell_y = y + 29 + (index / roses_assets::MAP_SIDE) * CELL;
            let color = COLORS[usize::from(*code)];
            writeln!(
                svg,
                r##"<rect x="{cell_x}" y="{cell_y}" width="15" height="15" fill="{color}"/><text x="{}" y="{}" font-size="9" text-anchor="middle" fill="#14202D">{code}</text>"##,
                cell_x + 7,
                cell_y + 11
            )?;
        }
        writeln!(svg, "</g>")?;
    }
    writeln!(svg, "</g></svg>")?;
    Ok(svg)
}

fn static_fusion_record(
    records: &[roses_assets::FusionRecord],
    first: u16,
    second: u16,
) -> Option<&roses_assets::FusionRecord> {
    let low = first.min(second);
    let high = first.max(second);
    records
        .iter()
        .find(|record| record.ingredient_low == low && record.ingredient_high == high)
}

/// A terminal duel against Weevil (development harness; see duel_text.rs).
fn play_duel(iso: &Path, seed: Option<&Path>, computer_player: bool) -> Result<i32> {
    let (mut disc, _) = open_pal(iso)?;
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    let messages = disc.read_file("MSG/MSG_E.DAT", 300_000)?;
    let names = roses_assets::parse_pal_card_names(&messages)?
        .names
        .into_iter()
        .map(|n| n.name)
        .collect();
    let seed = match seed {
        Some(s) => s
            .to_string_lossy()
            .parse::<u64>()
            .map_err(|_| "--seed must be a number")?,
        None => SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as u64,
    };
    println!("Terminal duel against Weevil (a rules-engine test harness, not the game's screen). Seed {seed}.");
    let prepared = duel_text::prepare(&executable, names, seed)?;
    duel_text::run_stdio(prepared, computer_player)
}

/// Write the duel card catalogue (cards, effect entries, power-up data and
/// fusion table) built from the user's disc, for a local client.
fn export_duel_cards(iso: &Path, output: &Path) -> Result<i32> {
    let (mut disc, _) = open_pal(iso)?;
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    let catalogue = duel_catalogue::build(&executable)?;
    fs::write(output, serde_json::to_vec(&catalogue)?)?;
    println!(
        "wrote {} cards and {} fusions to {}",
        catalogue.cards.len(),
        catalogue.fusions.len(),
        output.display()
    );
    Ok(0)
}

fn fusion_lookup(iso: &Path, first: u16, second: u16, output: Option<&Path>) -> Result<()> {
    let (mut disc, disc_report) = open_pal(iso)?;
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    let rules = roses_assets::parse_pal_rules(&executable)?;
    let record = static_fusion_record(&rules.fusions, first, second);
    emit_json(
        &serde_json::json!({
            "schema_version": "roses-static-fusion-lookup-1",
            "profile": rules.profile_id,
            "iso_sha256": disc_report.iso_sha256,
            "executable_sha256": rules.executable_sha256,
            "query": {"first_card_id": first, "second_card_id": second},
            "sorted_ingredient_ids": [first.min(second), first.max(second)],
            "status": if record.is_some() { "record_found" } else { "no_static_record" },
            "record": record,
            "gameplay_validated": false,
            "input_order_behavior_verified": false,
            "scope": "Lookup in the imported static fusion table using numerically sorted zero-based card IDs. A matching record is not a tested in-game outcome; no static record does not prove that no contextual behavior exists. Fusion priority, input-order behavior, effects and other runtime conditions remain unimplemented and unverified."
        }),
        output,
    )
}

fn verify_catalog(iso: &Path, port: u16, output: Option<&Path>) -> Result<i32> {
    let (mut disc, disc_report) = open_pal(iso)?;
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    let catalog = roses_assets::parse_pal_catalog(&executable)?;
    let mut client = PineClient::connect_local(port, Duration::from_secs(3))?;
    let before = client.identify()?;
    let game = before.game.as_ref().ok_or("no reference game is running")?;
    if game.serial != "SLES-51579" || !game.disc_crc.eq_ignore_ascii_case("cedf3740") {
        return Err("the running game does not match the PAL executable profile".into());
    }
    let mut watches = Vec::new();
    let mut expected = Vec::new();
    for card in &catalog.cards {
        let base = roses_assets::CARD_TABLE_EE_ADDRESS + u32::from(card.id) * 20;
        for (offset, field, value) in [
            (
                0,
                "kind_word",
                u16::from(card.kind) | (u16::from(card.kind_offset_raw) << 8),
            ),
            (
                2,
                "level_attribute_cost_word",
                u16::from((card.level << 4) | card.attribute) | (u16::from(card.deck_cost) << 8),
            ),
            (8, "attack_word", card.attack_word_raw),
            (10, "defense_word", card.defense_word_raw),
        ] {
            watches.push(MemoryWatch {
                name: format!("cards/{}/{field}", card.id),
                address: base + offset,
                width: ReadWidth::Bits16,
            });
            expected.push(u64::from(value));
        }
    }
    let capture = client.read_watches(&watches)?;
    let after = client.identify()?;
    if serde_json::to_value(&before)? != serde_json::to_value(&after)? {
        return Err("reference emulator changed during the comparison".into());
    }
    let mismatches: Vec<_> = capture.observations.iter().zip(&expected)
        .filter(|(actual, expected)| actual.value != **expected)
        .map(|(actual, expected)| serde_json::json!({"field":actual.name,"address":actual.address,"original":actual.value,"imported":expected}))
        .collect();
    let passed = mismatches.is_empty();
    emit_json(
        &serde_json::json!({
            "schema_version": "roses-catalog-memory-comparison-1",
            "status": if passed {"equal"} else {"divergent"},
            "profile": roses_disc::PAL_PROFILE_ID,
            "iso_sha256": disc_report.iso_sha256,
            "executable_sha256": catalog.executable_sha256,
            "emulator": before,
            "cards_compared": catalog.cards.len(),
            "words_compared": expected.len(),
            "mismatches": mismatches,
            "atomic_snapshot": capture.atomic,
            "gameplay_validated": false,
            "scope": "Imported kind, level, attribute, cost and attack/defense words compared against the paused original game's static card table. This validates data/memory mapping, not duel rules or effect behavior.",
            "capture": capture,
        }),
        output,
    )?;
    Ok(if passed { 0 } else { 1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_id_options_reject_out_of_range_or_nondecimal_inputs() {
        for (input, expected) in [
            ("0", Some(0)),
            ("853", Some(853)),
            ("854", None),
            ("-1", None),
            ("0x10", None),
            ("65536", None),
        ] {
            let arguments = [OsString::from("--first"), OsString::from(input)];
            let mut options = Options::parse(arguments.into_iter()).unwrap();
            assert_eq!(options.card_id("--first").ok(), expected);
        }
        let mut empty = Options::parse(std::iter::empty()).unwrap();
        assert!(empty.card_id("--first").is_err());
    }

    #[test]
    fn static_fusion_lookup_normalizes_pair_order_without_inventing_results() {
        let records = [roses_assets::FusionRecord {
            index: 0,
            ingredient_low: 3,
            ingredient_high: 12,
            result_id: 27,
            raw_word: (27 << 20) | (3 << 10) | 12,
        }];
        assert_eq!(static_fusion_record(&records, 12, 3), Some(&records[0]));
        assert_eq!(static_fusion_record(&records, 3, 12), Some(&records[0]));
        assert!(static_fusion_record(&records, 3, 3).is_none());
        assert!(static_fusion_record(&records, 12, 27).is_none());
    }
}
