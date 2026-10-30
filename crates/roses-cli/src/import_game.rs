//! One import for everything the connected game plays so far: the card library,
//! the opening through the story to the campaign map, the campaign map, and the
//! first duel (rules data, board, monster models, result screen and slot
//! machine textures), and the ending movies (MOVIE/EDMV_A6.PSS and
//! MOVIE/EDMV_C6.PSS, kept as the disc's PSS in the story part). Each part is produced by
//! its own importer exactly as when run separately; this only chains them and
//! records where each part is. Intermediate story imports are removed once the
//! final story import exists.
use crate::{emit_json, open_pal, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub const GAME_SCHEMA: &str = "roses-game-import-1";

#[derive(Serialize)]
struct Part {
    path: &'static str,
    manifest: &'static str,
    manifest_sha256: String,
}

/// An ending movie inside the story part (for readers of game.json; the
/// game itself takes it from the story manifest's asset).
#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct EndingMovieRef {
    pub asset: &'static str,
    pub disc_path: &'static str,
    /// Relative to the game import's folder.
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub skippable: bool,
}

/// The ending movies a story manifest holds (none in imports made before
/// 4 Oct 2026: the game then shows a stand-in).
pub(crate) fn ending_movies(story: &roses_client::Manifest, story_path: &str) -> Vec<EndingMovieRef> {
    roses_client::ending::ENDING_MOVIES
        .iter()
        .filter_map(|movie| {
            let asset = story.assets.iter().find(|a| a.id == movie.asset)?;
            Some(EndingMovieRef {
                asset: movie.asset,
                disc_path: movie.disc_path,
                path: format!("{story_path}/{}", asset.path),
                sha256: asset.sha256.clone(),
                bytes: asset.bytes,
                skippable: movie.skippable(),
            })
        })
        .collect()
}

#[derive(Serialize)]
struct GameManifest {
    schema_version: &'static str,
    importer_version: &'static str,
    iso_sha256: String,
    executable_sha256: Option<String>,
    library: Part,
    story: Part,
    campaign_map: Part,
    duel: Part,
    duel_board: Part,
    duel_models: Part,
    /// The result screens' and slot machine's textures (`roses import-duel-end`).
    duel_end: Part,
    /// Simon's scene derived from the disc (`roses import-opening-scene`).
    opening_scene: Part,
    /// The Stonehenge story events' world, MODEL.MRG pack 33
    /// (`roses import-opening-scene --world story-scene-0`).
    story_scene: Part,
    /// The history clouds' boot start: the two disc files it reads and a
    /// cloud source manifest (profile roses-disc-boot-c400-1).
    clouds: Part,
    /// The battle scene's arena stages and StageCam paths (`roses import-duel-arena`).
    duel_arena: Part,
    /// The duel's music and sound effects (`roses import-duel-audio`).
    duel_audio: Part,
    /// MOVIE/EDMV_A6.PSS (red) and MOVIE/EDMV_C6.PSS (white) in `story`.
    ending_movies: Vec<EndingMovieRef>,
    complete_game_import: bool,
    limitations: [&'static str; 4],
}

const LIMITATIONS: [&str; 4] = [
    "Only the opening, the story to the campaign map, the campaign map with both routes' story events (red: Bosworth, Stonehenge, the ending with its movie EDMV_A6; white: 12000, Mako 12040/12041, Stonehenge 12130..12156, the ending with its movie EDMV_C6) and the duel assets are imported; the other systems are not.",
    "Unit models: leaders and face-up monsters stand on the board (the cursor unit over it, as the original composites it); the first duel's decks with their fusion and ritual results are exported, other cards are decoded from the copied MONSTER.MRG when they appear. Only the idle motion plays: attacks, damage, destruction, movement between cells, summon grow-in and the near-camera fade are not reproduced.",
    "The history clouds start from the recorded boot schedule (1222 main-loop iterations before the history), not from the player's own dwell time; their update cadence (25 or 50 per second) is not settled.",
    "The duel's music and sound effects are imported (the opening and story music too; menus, map and deck editor sounds are not); of the battle scene the effect textures (TEXEFF.MRG), the arena stages with their StageCam orbits, the units' size and camera tables and the attack effect programs (BMODULE.MRG, run headless for their timing; their drawing is not reproduced) are imported, not the destroy effect (c621); the reward slot machine's line-flash visuals were not captured.",
];

fn part(output: &Path, path: &'static str, manifest: &'static str) -> Result<Part> {
    let bytes = fs::read(output.join(path).join(manifest))?;
    Ok(Part {
        path,
        manifest,
        manifest_sha256: format!("{:x}", Sha256::digest(&bytes)),
    })
}

fn step(number: usize, text: &str) {
    eprintln!("[{number}/12] {text}");
}

pub fn run(iso: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err(
            "game import output must be a new directory; existing imports are never overwritten"
                .into(),
        );
    }
    // Check the disc once before the long conversions start.
    let (_, report) = open_pal(iso)?;
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    let work = output.join("work");
    fs::create_dir(&work)?;

    step(1, "Card library");
    crate::import_assets(iso, &output.join("library"))?;
    step(2, "Opening (intro movie: built-in decoder check)");
    crate::import_opening::run(iso, &work.join("opening"))?;
    // import-opening already carries the history, Simon's arrival and name entry.
    step(3, "Simon after the name");
    crate::import_post_name::run(
        iso,
        &work.join("opening").join("manifest.json"),
        &work.join("post-name"),
    )?;
    step(
        4,
        "Deck choice, Seto, side choice, Yugi, both routes' map story events and the ending movies (built-in decoder check)",
    );
    crate::import_story_map::run(
        iso,
        &work.join("post-name").join("manifest.json"),
        &work.join("story-map"),
    )?;
    crate::import_story_red::run(
        iso,
        &work.join("story-map").join("manifest.json"),
        &output.join("story"),
    )?;
    // The final story import holds copies of every asset it references.
    fs::remove_dir_all(&work)?;
    step(5, "Campaign map");
    crate::import_campaign_map::run(iso, &output.join("campaign-map"))?;
    step(6, "First duel rules and board");
    crate::import_duel::run(iso, &output.join("duel"))?;
    crate::import_duel_board::run(iso, &output.join("duel-board"))?;
    step(7, "Monster models for the first duel");
    crate::import_duel_models::run(iso, &output.join("duel-models"), None)?;
    step(8, "Result screens and slot machine textures");
    crate::import_duel_end::run(iso, &output.join("duel-end"))?;
    step(9, "Simon's scene and the Stonehenge story world");
    crate::import_opening_scene::run(&crate::import_opening_scene::Options {
        world: crate::import_opening_scene::World::Opening,
        iso: iso.to_path_buf(),
        output: output.join("opening-scene"),
        pose: crate::import_opening_scene::default_pose(),
        verify_against: None,
        capture_root: ".".into(),
    })?;
    crate::import_opening_scene::run(&crate::import_opening_scene::Options {
        world: crate::import_opening_scene::World::StoryScene0,
        iso: iso.to_path_buf(),
        output: output.join("story-scene"),
        pose: crate::import_opening_scene::default_pose(),
        verify_against: None,
        capture_root: ".".into(),
    })?;
    step(10, "History clouds");
    clouds(iso, &output.join("clouds"))?;
    step(11, "Battle arenas");
    crate::import_duel_arena::run(iso, &output.join("duel-arena"))?;
    step(12, "Duel music and sound effects");
    crate::import_duel_audio::run(iso, &output.join("duel-audio"))?;

    let story: roses_client::Manifest =
        serde_json::from_slice(&fs::read(output.join("story").join("manifest.json"))?)?;
    let ending_movies = ending_movies(&story, "story");
    if ending_movies.len() != roses_client::ending::ENDING_MOVIES.len() {
        return Err("the story import lacks the ending movies".into());
    }
    let manifest = GameManifest {
        schema_version: GAME_SCHEMA,
        importer_version: env!("CARGO_PKG_VERSION"),
        iso_sha256: report.iso_sha256.clone(),
        executable_sha256: report.executable_sha256.clone(),
        library: part(output, "library", "manifest.json")?,
        story: part(output, "story", "manifest.json")?,
        campaign_map: part(output, "campaign-map", "manifest.json")?,
        duel: part(output, "duel", "manifest.json")?,
        duel_board: part(output, "duel-board", "manifest.json")?,
        duel_models: part(output, "duel-models", "manifest.json")?,
        duel_end: part(output, "duel-end", "manifest.json")?,
        opening_scene: part(output, "opening-scene", "scene.json")?,
        story_scene: part(output, "story-scene", "scene.json")?,
        clouds: part(output, "clouds", "source.json")?,
        duel_arena: part(output, "duel-arena", "manifest.json")?,
        duel_audio: part(output, "duel-audio", "manifest.json")?,
        ending_movies,
        complete_game_import: false,
        limitations: LIMITATIONS,
    };
    emit_json(&manifest, Some(&output.join("game.json")))?;
    eprintln!(
        "Game import complete: {}",
        output.join("game.json").display()
    );
    Ok(())
}

/// The cloud object's inputs from the disc (the boot executable for the sine
/// table, TEXEVE.MRG for the atlas) and a boot-start manifest. The schedule is
/// the recorded one (CLOUDS-START.md): the original's depends on how long the
/// player waits on the title and menu.
fn clouds(iso: &Path, output: &Path) -> Result<()> {
    let (mut disc, _) = open_pal(iso)?;
    fs::create_dir_all(output)?;
    for (disc_path, name) in [
        ("SLES_515.79", "SLES_515.79"),
        ("DATA/TEXEVE.MRG", "TEXEVE.MRG"),
    ] {
        let bytes = disc.read_file(disc_path, 16 * 1024 * 1024)?;
        fs::write(output.join(name), bytes)?;
    }
    emit_json(
        &serde_json::json!({
            "schema_version": 1,
            "profile": "roses-disc-boot-c400-1",
            "executable_path": "SLES_515.79",
            "texeve_path": "TEXEVE.MRG",
            "schedule": {"loop_ticks_before_init": 1222, "flips_before_init": 1223},
            "arithmetic": "pcsx2_positive_rounding"
        }),
        Some(&output.join("source.json")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use roses_client::ending::{RED_MOVIE, WHITE_MOVIE};

    fn manifest(assets: Vec<roses_client::Asset>) -> roses_client::Manifest {
        serde_json::from_value(serde_json::json!({
            "schema_version": roses_client::MANIFEST_SCHEMA,
            "source": {"profile": "p", "disc_sha256": "0", "executable_sha256": "0"},
            "canvas": {"width": 640, "height": 448, "aspect_width": 4, "aspect_height": 3},
            "assets": assets,
            "initial_stage": "intro",
            "stages": [],
        }))
        .unwrap()
    }

    fn video(id: &str, path: &str, sha: char) -> roses_client::Asset {
        roses_client::Asset {
            id: id.into(),
            kind: roses_client::AssetKind::Video,
            path: path.into(),
            sha256: sha.to_string().repeat(64),
            bytes: 146_751_492,
        }
    }

    #[test]
    fn game_json_lists_the_ending_movies_of_the_story_part() {
        let story = manifest(vec![
            video("opening-movie", "media/opening.pss", 'a'),
            video(RED_MOVIE.asset, RED_MOVIE.media_path, 'b'),
            video(WHITE_MOVIE.asset, WHITE_MOVIE.media_path, 'c'),
        ]);
        let movies = ending_movies(&story, "story");
        assert_eq!(movies.len(), 2);
        assert_eq!(movies[0].asset, "ending-movie-red");
        assert_eq!(movies[0].disc_path, "MOVIE/EDMV_A6.PSS");
        assert_eq!(movies[0].path, "story/media/ending-red.pss");
        assert_eq!(movies[0].sha256, "b".repeat(64));
        assert!(!movies[0].skippable && !movies[1].skippable);
        assert_eq!(movies[1].disc_path, "MOVIE/EDMV_C6.PSS");
        assert_eq!(movies[1].path, "story/media/ending-white.pss");
        let json = serde_json::to_value(&movies[0]).unwrap();
        for key in ["asset", "disc_path", "path", "sha256", "bytes", "skippable"] {
            assert!(json.get(key).is_some(), "{key}");
        }
        // An import from before the ending movies: none (the game's stand-in).
        let old = manifest(vec![video("opening-movie", "media/opening.pss", 'a')]);
        assert!(ending_movies(&old, "story").is_empty());
    }

    #[test]
    fn ending_movie_fingerprints_match_the_client_table() {
        use crate::import_opening::{ENDING_RED_MOVIE, ENDING_WHITE_MOVIE};
        assert_eq!(ENDING_RED_MOVIE.disc_path, RED_MOVIE.disc_path);
        assert_eq!(ENDING_WHITE_MOVIE.disc_path, WHITE_MOVIE.disc_path);
        for m in [&ENDING_RED_MOVIE, &ENDING_WHITE_MOVIE] {
            assert_eq!(m.sha256.len(), 64);
            assert_eq!((m.frames, m.samples_per_channel), (3000, 5_760_000));
        }
        assert_ne!(ENDING_RED_MOVIE.sha256, ENDING_WHITE_MOVIE.sha256);
    }
}
