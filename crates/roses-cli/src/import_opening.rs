//! Local opening import. Layout is a measured first pass, not a parity claim.
use crate::{emit_json, open_pal, write_png, Result};
use roses_assets::{
    decode_pal_opening_texture, inspect_pal_opening_archive, OpeningArchiveKind,
    MAX_OPENING_ARCHIVE_BYTES,
};
use roses_client::{
    Asset, AssetKind, Button, Canvas, Layer, Manifest, MenuAction, MenuEntry, Sampling, Source,
    Stage, MANIFEST_SCHEMA,
};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

const MOVIE_SHA256: &str = "649bbb1683c1d3525990d939f1420ccefff946e0380109ff9b4c5cb0150d0c52";

/// A PSS movie of the PAL disc and what the built-in decoder must get from
/// it (measured: `work/research/movie/claude-pss/NOTES.md` §1).
pub(crate) struct DiscMovie {
    pub disc_path: &'static str,
    pub sha256: &'static str,
    pub frames: u64,
    pub samples_per_channel: u64,
}
pub(crate) const OPENING_MOVIE: DiscMovie = DiscMovie {
    disc_path: "MOVIE/OPMV_EU.PSS",
    sha256: MOVIE_SHA256,
    frames: 2867,
    samples_per_channel: 5_504_768,
};
/// The red route's ending (0x1A8CD0(2, 1, 0)).
pub(crate) const ENDING_RED_MOVIE: DiscMovie = DiscMovie {
    disc_path: "MOVIE/EDMV_A6.PSS",
    sha256: "00658ce55b06f960077a5bd573822206cf57d236619ace1590046783a59e588f",
    frames: 3000,
    samples_per_channel: 5_760_000,
};
/// The white route's ending (0x1A8CD0(3, 1, 0)).
pub(crate) const ENDING_WHITE_MOVIE: DiscMovie = DiscMovie {
    disc_path: "MOVIE/EDMV_C6.PSS",
    sha256: "6c52c1d7059daf7d3273505131f6fa5a9911fccbdfc37f478fc6d932fb1eb97f",
    frames: 3000,
    samples_per_channel: 5_760_000,
};
/// Largest PSS on the disc is 146,751,492 bytes.
pub(crate) const MAX_MOVIE_BYTES: u64 = 160 * 1024 * 1024;

/// Strictly decode every frame and the whole sound track of a disc PSS
/// with the built-in decoder; the import fails if anything is off.
pub(crate) fn verify_native_movie(path: &Path, expected: &DiscMovie) -> Result<serde_json::Value> {
    let started = std::time::Instant::now();
    let mut reader =
        roses_movie::MovieReader::new(std::io::BufReader::new(fs::File::open(path)?));
    reader.set_strict(true);
    let mut frames = 0u64;
    let mut samples = 0usize;
    while reader.next_frame()?.is_some() {
        frames += 1;
        samples += reader.take_audio().len();
    }
    samples += reader.take_audio().len();
    let info = reader.info().clone();
    let audio = info.audio.ok_or_else(|| format!("{} has no audio", expected.disc_path))?;
    let per_channel = samples as u64 / u64::from(audio.channels);
    if !reader.ended_cleanly()
        || reader.video_stats().errors != 0
        || (info.width, info.height, info.frame_rate) != (640, 448, (25, 1))
        || (frames, per_channel) != (expected.frames, expected.samples_per_channel)
        || (audio.sample_rate, audio.channels) != (48_000, 2)
    {
        return Err(format!(
            "built-in movie decoder check failed for {}: {frames} frames, {per_channel} samples/channel, {:?}",
            expected.disc_path,
            reader.video_stats()
        )
        .into());
    }
    Ok(serde_json::json!({
        "disc_path":expected.disc_path,
        "decoder":"roses-movie (pure Rust MPEG-2 video + SShd PCM audio)",
        "frames":frames,"width":info.width,"height":info.height,"frame_rate":[25,1],
        "audio_sample_rate":audio.sample_rate,"audio_channels":audio.channels,
        "audio_samples_per_channel":per_channel,
        "check_seconds":started.elapsed().as_secs_f64()
    }))
}

/// Copy one disc movie into an import as is (`root/relative`), after its
/// fingerprint; then the strict full decode check. Returns the manifest
/// asset (kind video, sha256 of the written file) and the check report.
pub(crate) fn import_disc_movie(
    disc: &mut roses_disc::Disc,
    expected: &DiscMovie,
    root: &Path,
    id: &str,
    relative: &str,
) -> Result<(Asset, serde_json::Value)> {
    let movie = disc.read_file(expected.disc_path, MAX_MOVIE_BYTES)?;
    if format!("{:x}", Sha256::digest(&movie)) != expected.sha256 {
        return Err(format!("unsupported {} fingerprint", expected.disc_path).into());
    }
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_new(&path, &movie)?;
    drop(movie);
    eprintln!("Checking {} with the built-in decoder…", expected.disc_path);
    let check = verify_native_movie(&path, expected)?;
    eprintln!("Built-in decoder check passed: {check}");
    let asset = asset(root, id, AssetKind::Video, relative)?;
    if asset.sha256 != expected.sha256 {
        return Err(format!("{relative} changed while it was checked").into());
    }
    Ok((asset, check))
}

pub fn run(iso: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err(
            "opening output must be a new directory; existing files are never overwritten".into(),
        );
    }
    let (mut disc, report) = open_pal(iso)?;
    let movie = disc.read_file(OPENING_MOVIE.disc_path, MAX_MOVIE_BYTES)?;
    if format!("{:x}", Sha256::digest(&movie)) != MOVIE_SHA256 {
        return Err("unsupported opening movie fingerprint".into());
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    fs::create_dir(output.join("textures"))?;
    fs::create_dir(output.join("media"))?;
    let mut assets = Vec::new();
    let mut archives = Vec::new();
    for (kind, prefix) in [
        (OpeningArchiveKind::TexSys, "texsys"),
        (OpeningArchiveKind::TexEve, "texeve"),
        (OpeningArchiveKind::TexEtc, "texetc"),
    ] {
        let bytes = disc.read_file(kind.iso_path(), MAX_OPENING_ARCHIVE_BYTES as u64)?;
        let archive = inspect_pal_opening_archive(&bytes, kind)?;
        for record in &archive.records {
            let image = decode_pal_opening_texture(&bytes, kind, usize::from(record.id))?;
            let id = format!("{prefix}-{:02}", record.id);
            let relative = format!("textures/{id}.png");
            write_png(
                &output.join(&relative),
                image.width,
                image.height,
                &image.pixels,
            )?;
            assets.push(asset(output, &id, AssetKind::Png, &relative)?);
        }
        archives.push(archive);
    }
    write_new(&output.join("media/opening.pss"), &movie)?;
    let audio = roses_assets::demux_pal_opening_movie(&movie)?;
    emit_json(&audio.metadata, Some(&output.join("movie-source.json")))?;
    drop(movie);
    // The PSS is kept as is and decoded in Rust at playback (crates/roses-movie).
    eprintln!("Checking the opening movie with the built-in decoder…");
    let native_check = verify_native_movie(&output.join("media/opening.pss"), &OPENING_MOVIE)?;
    eprintln!("Built-in decoder check passed: {native_check}");
    let movie_path = "media/opening.pss";
    assets.push(asset(output, "opening-movie", AssetKind::Video, movie_path)?);
    let mut manifest = opening_manifest(
        assets,
        Source {
            profile: roses_disc::PAL_PROFILE_ID.into(),
            disc_sha256: report.iso_sha256.clone(),
            executable_sha256: report
                .executable_sha256
                .clone()
                .ok_or("missing executable fingerprint")?,
        },
    );
    let story_report = crate::import_story::append(&mut disc, output, &mut manifest)?;
    emit_json(&story_report, Some(&output.join("story-import.json")))?;
    roses_client::validate_manifest(&manifest)?;
    emit_json(&report, Some(&output.join("disc.json")))?;
    emit_json(&archives, Some(&output.join("texture-archives.json")))?;
    emit_json(
        &serde_json::json!({
            "schema_version":"roses-opening-import-1",
            "source_movie_sha256":MOVIE_SHA256,
            "movie_conversion":"none: the user-provided PSS is decoded in Rust at playback",
            "native_movie_check":native_check,
            "complete_game_import":false,
            "opening_parity_validated":false,
            "limitations":["New Game connects to 15 historical pages, 12 Simon dialogue pages and interactive name entry. Following dialogue and the first duel are not connected. Continue, Custom Duel and Trade are unavailable.","The original 3D backdrop and animation, title idle/attract behavior, native screen transitions, selection animation and menu audio remain incomplete.","Layout and input rules remain a partial implementation; movie synchronization has not been compared frame by frame. Native story music still needs an audible check."]
        }),
        Some(&output.join("import-report.json")),
    )?;
    let pending_path = output.join("manifest.pending.json");
    emit_json(&manifest, Some(&pending_path))?;
    roses_client::Loaded::open(&pending_path)?;
    let manifest_path = output.join("manifest.json");
    emit_json(&manifest, Some(&manifest_path))?;
    eprintln!("Opening imported: {}", manifest_path.display());
    Ok(())
}

pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
pub(crate) fn hash_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn asset(root: &Path, id: &str, kind: AssetKind, relative: &str) -> Result<Asset> {
    let path = root.join(relative);
    Ok(Asset {
        id: id.into(),
        kind,
        path: relative.into(),
        sha256: hash_file(&path)?,
        bytes: fs::metadata(path)?.len(),
    })
}
fn layer(asset: &str, source: Option<[u32; 4]>, destination: [u32; 4], brightness: u8) -> Layer {
    Layer {
        asset: asset.into(),
        source,
        destination,
        tint: [brightness; 3],
        opacity: 255,
        flip_x: false,
        filter: Sampling::Linear,
    }
}
fn selected_frame(y: u32) -> Vec<Layer> {
    let mut layers = Vec::new();
    // The original atlas supplies the frame and side ornaments. Diamond motion
    // is a separate draw and is not represented by a guessed replacement.
    for (sx, sw, dx, dw) in [(0, 8, 140, 10), (8, 16, 150, 340), (24, 8, 490, 10)] {
        for (sy, sh, dy, dh) in [(208, 8, y, 7), (216, 16, y + 7, 42), (232, 8, y + 49, 7)] {
            layers.push(layer(
                "texetc-01",
                Some([sx, sy, sw, sh]),
                [dx, dy, dw, dh],
                255,
            ));
        }
    }
    layers.push(layer(
        "texetc-01",
        Some([216, 192, 40, 64]),
        [100, y, 50, 56],
        255,
    ));
    let mut right = layer("texetc-01", Some([216, 192, 40, 64]), [490, y, 50, 56], 255);
    right.flip_x = true;
    layers.push(right);
    layers
}
fn opening_manifest(assets: Vec<Asset>, source: Source) -> Manifest {
    let entries = [
        ("new-game", "New Game"),
        ("continue", "Continue"),
        ("custom-duel", "Custom Duel"),
        ("trade", "Trade"),
    ]
    .iter()
    .enumerate()
    .map(|(i, (id, label))| {
        let y = 28 + i as u32 * 98;
        let normal = layer(
            "texetc-01",
            Some([0, i as u32 * 48, 256, 48]),
            [160, y, 320, 42],
            128,
        );
        let selected = layer(
            "texetc-01",
            Some([0, i as u32 * 48, 256, 48]),
            [160, y, 320, 42],
            255,
        );
        let mut selected_layers = vec![selected];
        selected_layers.extend(selected_frame(y - 8));
        MenuEntry {
            id: (*id).into(),
            label: (*label).into(),
            normal_layers: vec![normal],
            selected_layers,
            action: MenuAction::Unavailable,
            campaign_side: None,
        }
    })
    .collect();
    Manifest {
        schema_version: MANIFEST_SCHEMA.into(),
        source,
        canvas: Canvas {
            width: 640,
            height: 448,
            aspect_width: 4,
            aspect_height: 3,
        },
        assets,
        initial_stage: "intro".into(),
        stages: vec![
            Stage::Movie {
                id: "intro".into(),
                asset: "opening-movie".into(),
                on_end: "title".into(),
                skip_buttons: vec![Button::Start],
                on_skip: Some("title".into()),
            },
            Stage::Screen {
                id: "title".into(),
                layers: vec![
                    layer("texetc-00", None, [0, 0, 640, 448], 255),
                    layer(
                        "texetc-01",
                        Some([0, 192, 184, 16]),
                        [205, 360, 230, 14],
                        255,
                    ),
                ],
                music: None,
                advance_buttons: vec![Button::Start],
                duration_ms: None,
                next: "main-menu".into(),
            },
            Stage::Menu {
                id: "main-menu".into(),
                layers: vec![layer("texetc-00", None, [0, 0, 640, 448], 128)],
                music: None,
                entries,
                initial_selection: 0,
                confirm_buttons: vec![Button::Cross],
                cancel_target: None,
                wrap_navigation: false,
            },
        ],
        layer_sets: Default::default(),
    }
}
