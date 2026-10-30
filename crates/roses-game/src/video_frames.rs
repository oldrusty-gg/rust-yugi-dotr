//! Video aid (a port addition): key frames of a long headless capture of
//! the connected game, for rehearsing a recorded online duel without a
//! window. `ROSES_VIDEO_FRAMES=<dir>` writes a PNG of the presented
//! 512x512 frame every `ROSES_VIDEO_FRAME_EVERY_S` seconds of the duel
//! (default 20) and twice after each big moment the paced bot presses (a
//! fusion, a big summon, an attack, an activation), named by the duel
//! clock (the bot's 25 Hz updates): `duel-MMmSSs-<label>.png`. Only while
//! the duel scene presents into the capture target (the menus write their
//! own frames with the online script's `shot:` steps).
//! `ROSES_CAPTURE_EXTRA_S` lengthens the capture's time limits (a whole
//! duel at a person's pace runs 15-30 minutes).

use super::*;

/// Seconds added to the headless capture's limits (`ROSES_CAPTURE_EXTRA_S`).
pub fn extra_seconds() -> u64 {
    std::env::var("ROSES_CAPTURE_EXTRA_S")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

#[derive(Resource)]
struct VideoFrames {
    dir: PathBuf,
    every: u64,
    next_periodic: u64,
    last_big: Option<u64>,
    due: Vec<(u64, String)>,
    written: usize,
}

const MAX_FRAMES: usize = 400;

pub fn add_systems(app: &mut App) {
    let Some(dir) = std::env::var_os("ROSES_VIDEO_FRAMES").map(PathBuf::from) else {
        return;
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("Video frames: cannot create {}: {e}", dir.display());
        return;
    }
    let every = std::env::var("ROSES_VIDEO_FRAME_EVERY_S")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(20u64)
        * 25;
    eprintln!(
        "Video frames: every {} s of the duel and after big moments into {}",
        every / 25,
        dir.display()
    );
    app.insert_resource(VideoFrames {
        dir,
        every: every.max(25),
        next_periodic: 0,
        last_big: None,
        due: Vec::new(),
        written: 0,
    })
    .add_systems(Update, take_frames);
}

fn take_frames(
    mut commands: Commands,
    mut frames: ResMut<VideoFrames>,
    state: Option<Res<CaptureState>>,
    recording: Option<Res<record::RecordTarget>>,
    flow: Option<Res<GameFlow>>,
    play: Option<Res<duel_board_render::PlayRuntime>>,
) {
    let in_duel = flow.as_ref().is_some_and(|f| f.scene == GameScene::Duel && f.next.is_none());
    if !in_duel {
        return;
    }
    // A capture's target, or the recorder's image (`--record`).
    let Some(target) = state.and_then(|s| s.target.clone()).or_else(|| recording.map(|r| r.0.clone())) else {
        return;
    };
    let Some((now, big)) = play.as_ref().and_then(|p| p.video_clock()) else {
        return;
    };
    if frames.written >= MAX_FRAMES {
        return;
    }
    if let Some(at) = big {
        if frames.last_big != Some(at) {
            frames.last_big = Some(at);
            frames.due.push((at + 40, "moment".into()));
            frames.due.push((at + 110, "after".into()));
        }
    }
    let mut label = None;
    if now >= frames.next_periodic {
        frames.next_periodic = now + frames.every;
        label = Some("every".to_string());
    }
    if let Some(i) = frames.due.iter().position(|(due, _)| *due <= now) {
        label = Some(frames.due.remove(i).1);
    }
    let Some(label) = label else { return };
    frames.written += 1;
    let path = frames.dir.join(format!(
        "duel-{:02}m{:02}s-{:04}-{label}.png",
        now / 25 / 60,
        now / 25 % 60,
        frames.written
    ));
    commands
        .spawn(Screenshot::image(target))
        .observe(move |event: On<ScreenshotCaptured>| {
            let result = (|| -> Result<(), String> {
                let image = event.image.clone().try_into_dynamic().map_err(|e| e.to_string())?;
                let mut encoded = Cursor::new(Vec::new());
                image
                    .write_to(
                        &mut encoded,
                        ImageFormat::Png.as_image_crate_format().ok_or("PNG encoder unavailable")?,
                    )
                    .map_err(|e| format!("PNG encode failed: {e}"))?;
                std::fs::write(&path, encoded.get_ref()).map_err(|e| e.to_string())
            })();
            if let Err(e) = result {
                eprintln!("Video frames: {}: {e}", path.display());
            }
        });
}
