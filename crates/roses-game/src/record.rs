//! `--record <out.mp4>`: the in-engine video recorder. A tool for making
//! videos, not gameplay (docs/ADDITIONS.md): nothing in the game changes.
//!
//! - Off-screen like `--capture` (no window, no gamepads): every
//!   presentation camera draws into one 512x512 image that is read back
//!   from the GPU each frame (`Readback`), tagged with its update number in
//!   the render world, then scaled (nearest, 4:3 box with black bars) to
//!   NV12 and encoded H.264 by `platform_video::Recorder` (Media
//!   Foundation's Sink Writer).
//! - Sound: the game's one mixer is opened without an output device
//!   (`duel_board_render::NO_SOUND_OUTPUT`), so ROSES_MUTE=1 or not, the
//!   speakers stay silent; the recorder mixes exactly `48000 / fps` frames
//!   after each update, the sound that update asked for.
//! - Clock: game time advances by exactly `1 / fps` per update
//!   (`TimeUpdateStrategy::ManualDuration`), so the 25 Hz fixed updates and
//!   the sound follow the picture count, not wall time. Until every render
//!   pipeline is ready the clock is held and nothing is recorded, so a clip
//!   starts on a fully drawn first frame.
//! - `--record-keys 120:down,3s:x*10`: scripted keys at recorded frames
//!   (or seconds), held 3 frames unless `*N`.
use super::PresentationCamera;
use bevy::{
    camera::RenderTarget,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets,
        render_resource::{CachedPipelineState, PipelineCache, TextureFormat, TextureUsages},
        texture::GpuImage,
    },
    shader::ShaderCacheError,
    time::{TimeSystems, TimeUpdateStrategy},
};
use platform_video::{PixelLayout, Recorder, SegmentFormat, SourceSpec};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The framebuffer every scene presents (as `--capture`).
const SOURCE: u32 = 512;
/// Render frames with every pipeline ready before the clock starts.
const READY_FRAMES: u32 = 8;
/// Start anyway after this long (a pipeline that never compiles).
const READY_LIMIT: Duration = Duration::from_secs(120);

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptKey {
    /// Recorded frame of the press.
    pub frame: u64,
    pub hold: u64,
    pub code: KeyCode,
    pub logical: Key,
}

/// The `--record*` options.
#[derive(Clone, Debug, Default)]
pub struct RecordArgs {
    pub path: Option<PathBuf>,
    pub size: Option<(u32, u32)>,
    pub fps: Option<u32>,
    pub seconds: Option<f64>,
    pub keys: Option<String>,
    pub realtime: bool,
}

impl RecordArgs {
    /// Take the `--record*` options out of the arguments.
    pub fn take(args: Vec<std::ffi::OsString>) -> Result<(Self, Vec<std::ffi::OsString>), String> {
        let mut out = Self::default();
        let mut rest = Vec::new();
        let mut it = args.into_iter();
        while let Some(arg) = it.next() {
            let mut value = |name: &str| -> Result<String, String> {
                it.next()
                    .and_then(|v| v.into_string().ok())
                    .ok_or_else(|| format!("{name} needs a value"))
            };
            match arg.to_str() {
                Some("--record") if out.path.is_none() => {
                    out.path = Some(PathBuf::from(value("--record")?))
                }
                Some("--record-size") if out.size.is_none() => {
                    out.size = Some(SegmentFormat::parse_size(&value("--record-size")?)?)
                }
                Some("--record-fps") if out.fps.is_none() => {
                    out.fps = Some(
                        value("--record-fps")?
                            .parse()
                            .map_err(|_| "--record-fps needs a number")?,
                    )
                }
                Some("--record-seconds") if out.seconds.is_none() => {
                    let s: f64 = value("--record-seconds")?
                        .parse()
                        .map_err(|_| "--record-seconds needs a number")?;
                    if !(s > 0.0 && s <= 4.0 * 3600.0) {
                        return Err("--record-seconds must be above 0 and at most 4 hours".into());
                    }
                    out.seconds = Some(s);
                }
                Some("--record-keys") if out.keys.is_none() => {
                    out.keys = Some(value("--record-keys")?)
                }
                Some("--record-realtime") => out.realtime = true,
                Some(a) if a.starts_with("--record") => {
                    return Err(format!("Unknown or repeated argument: {a}"));
                }
                _ => rest.push(arg),
            }
        }
        if out.path.is_none()
            && (out.size.is_some()
                || out.fps.is_some()
                || out.seconds.is_some()
                || out.keys.is_some()
                || out.realtime)
        {
            return Err("--record-size, --record-fps, --record-seconds, --record-keys and --record-realtime need --record <out.mp4>".into());
        }
        Ok((out, rest))
    }

    pub fn active(&self) -> bool {
        self.path.is_some()
    }

    /// The checked settings.
    pub fn settings(&self) -> Result<RecordSettings, String> {
        let path = self.path.clone().ok_or("--record needs an output .mp4")?;
        platform_video::check_output(&path)?;
        let (w, h) = self
            .size
            .unwrap_or((SegmentFormat::DEFAULT.width, SegmentFormat::DEFAULT.height));
        let format = SegmentFormat::new(w, h, self.fps.unwrap_or(SegmentFormat::DEFAULT.fps))?;
        let keys = match &self.keys {
            Some(text) => parse_keys(text, format.fps)?,
            None => Vec::new(),
        };
        Ok(RecordSettings {
            path,
            format,
            frames: self
                .seconds
                .map(|s| (s * f64::from(format.fps)).round().max(1.0) as u64),
            keys,
            realtime: self.realtime,
        })
    }
}

#[derive(Clone, Debug)]
pub struct RecordSettings {
    pub path: PathBuf,
    pub format: SegmentFormat,
    /// Stop after this many frames (else when the game exits).
    pub frames: Option<u64>,
    pub keys: Vec<ScriptKey>,
    pub realtime: bool,
}

fn key(name: &str) -> Option<(KeyCode, Key)> {
    let c = |s: &str| Key::Character(s.into());
    Some(match name {
        "up" => (KeyCode::ArrowUp, Key::ArrowUp),
        "down" => (KeyCode::ArrowDown, Key::ArrowDown),
        "left" => (KeyCode::ArrowLeft, Key::ArrowLeft),
        "right" => (KeyCode::ArrowRight, Key::ArrowRight),
        "enter" => (KeyCode::Enter, Key::Enter),
        "tab" | "select" => (KeyCode::Tab, Key::Tab),
        "x" | "cross" => (KeyCode::KeyX, c("x")),
        "z" | "circle" => (KeyCode::KeyZ, c("z")),
        "a" | "square" => (KeyCode::KeyA, c("a")),
        "d" | "triangle" => (KeyCode::KeyD, c("d")),
        "s" | "start" => (KeyCode::KeyS, c("s")),
        "q" | "l1" => (KeyCode::KeyQ, c("q")),
        "e" | "r1" => (KeyCode::KeyE, c("e")),
        "1" | "l2" => (KeyCode::Digit1, c("1")),
        "w" | "r2" => (KeyCode::KeyW, c("w")),
        "t" => (KeyCode::KeyT, c("t")),
        _ => return None,
    })
}

/// `120:down,2.5s:x*10`: a key at recorded frame 120, Cross at 2.5 s held
/// 10 frames (default 3, so a 25 Hz update always sees it).
pub fn parse_keys(text: &str, fps: u32) -> Result<Vec<ScriptKey>, String> {
    let mut keys = Vec::new();
    for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (at, rest) = item
            .split_once(':')
            .ok_or_else(|| format!("--record-keys: {item:?} is not FRAME:KEY"))?;
        let frame = match at.strip_suffix('s') {
            Some(s) => (s
                .parse::<f64>()
                .map_err(|_| format!("--record-keys: bad time {at:?}"))?
                * f64::from(fps))
            .round() as u64,
            None => at
                .parse::<u64>()
                .map_err(|_| format!("--record-keys: bad frame {at:?}"))?,
        };
        let (name, hold) = match rest.split_once('*') {
            Some((n, h)) => (
                n,
                h.parse::<u64>()
                    .map_err(|_| format!("--record-keys: bad hold {h:?}"))?
                    .max(1),
            ),
            None => (rest, 3),
        };
        let (code, logical) = key(&name.to_ascii_lowercase())
            .ok_or_else(|| format!("--record-keys: unknown key {name:?}"))?;
        keys.push(ScriptKey {
            frame,
            hold,
            code,
            logical,
        });
    }
    Ok(keys)
}

/// Shared with the render world.
#[derive(Resource, Clone, Default)]
struct RecordShared {
    /// Update numbers of the frames whose readback was requested, oldest
    /// first; each `ReadbackComplete` takes the front one.
    tags: Arc<Mutex<VecDeque<u64>>>,
    ready_streak: Arc<AtomicU32>,
}

/// This update, as the render world sees it (extracted).
#[derive(Resource, Clone, Copy, Default)]
struct RecordClock {
    frame: u64,
    recording: bool,
    target: Option<AssetId<Image>>,
}

/// The image every presentation camera draws into while recording.
#[derive(Resource, Clone)]
pub(crate) struct RecordTarget(pub Handle<Image>);

/// The outcome for `run` (the world is gone once the app returns).
#[derive(Resource, Clone, Default)]
pub struct RecordOutcome(pub Arc<Mutex<Option<Result<String, String>>>>);

#[derive(Resource)]
struct Recording {
    settings: RecordSettings,
    recorder: Option<Recorder>,
    target: Option<Handle<Image>>,
    shared: RecordShared,
    outcome: RecordOutcome,
    /// This update's number (from 0).
    frame: u64,
    started: Instant,
    /// Update number of recorded frame 0.
    first: Option<u64>,
    /// Sound mixed after each recorded update.
    audio: VecDeque<(u64, Vec<i16>)>,
    /// Readbacks waiting for the encoder: (update, RGBA).
    received: VecDeque<(u64, Vec<u8>)>,
    /// Next update to encode.
    next: Option<u64>,
    last: Option<Vec<u8>>,
    gaps: u64,
    held: Vec<(u64, KeyCode, Key)>,
    /// The mixer's started-effects count after the last update.
    se_seen: u64,
    /// `ROSES_RECORD_WAV=<file.wav>`: the exact sound handed to the
    /// encoder, uncompressed (checks, or a lossless track for an editor).
    wav: Option<Wav>,
    done: bool,
}

/// Add the recorder to a headless app (`run` has disabled the window).
pub fn add(app: &mut App, settings: RecordSettings) -> RecordOutcome {
    let shared = RecordShared::default();
    let outcome = RecordOutcome::default();
    eprintln!(
        "Record: {} at {}x{} {} fps (H.264 + AAC 48 kHz stereo){}; the game's sound is mixed into the file only (no output device)",
        settings.path.display(),
        settings.format.width,
        settings.format.height,
        settings.format.fps,
        settings
            .frames
            .map_or_else(|| ", until the game closes or ROSES_RECORD_STOP_FILE appears".to_string(), |f| format!(", {f} frames"))
    );
    super::duel_board_render::NO_SOUND_OUTPUT.store(true, Ordering::SeqCst);
    roses_client::movie_native::set_lockstep(true);
    app.insert_resource(Recording {
        settings,
        recorder: None,
        target: None,
        shared: shared.clone(),
        outcome: outcome.clone(),
        frame: u64::MAX,
        started: Instant::now(),
        first: None,
        audio: VecDeque::new(),
        received: VecDeque::new(),
        next: None,
        last: None,
        gaps: 0,
        held: Vec::new(),
        se_seen: 0,
        wav: std::env::var_os("ROSES_RECORD_WAV").and_then(|p| Wav::create(PathBuf::from(p))),
        done: false,
    })
    .insert_resource(RecordClock::default())
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
    .add_systems(Startup, start_recording)
    .add_systems(First, begin_frame.before(TimeSystems))
    .add_systems(PostUpdate, retarget_cameras.before(bevy::camera::CameraUpdateSystems))
    .add_systems(Last, (end_frame, pace_realtime).chain())
    .add_observer(on_readback);
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .insert_resource(shared)
            .add_systems(ExtractSchedule, extract_clock)
            .add_systems(Render, tag_frame.in_set(RenderSystems::Queue));
    }
    outcome
}

fn start_recording(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut state: ResMut<Recording>,
) {
    let mut target = Image::new_target_texture(SOURCE, SOURCE, TextureFormat::Rgba8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = images.add(target);
    commands.spawn((
        Name::new("Video recorder readback"),
        Readback::texture(target.clone()),
    ));
    // The recorded image, for `video_frames`' key frames of a recording.
    commands.insert_resource(RecordTarget(target.clone()));
    state.target = Some(target);
    let source = SourceSpec {
        width: SOURCE,
        height: SOURCE,
        layout: PixelLayout::Rgba,
        display_aspect: (4, 3),
    };
    match Recorder::start(
        &state.settings.path,
        state.settings.format,
        source,
        "roses-bevy-scene",
    ) {
        Ok(r) => state.recorder = Some(r),
        Err(e) => fail(&mut state, e),
    }
}

fn fail(state: &mut Recording, error: String) {
    eprintln!("Record: {error}");
    state.recorder = None;
    state.done = true;
    *state.outcome.0.lock().unwrap() = Some(Err(error));
}

fn begin_frame(
    mut state: ResMut<Recording>,
    mut strategy: ResMut<TimeUpdateStrategy>,
    mut clock: ResMut<RecordClock>,
    mut keys: MessageWriter<KeyboardInput>,
) {
    let state = &mut *state;
    state.frame = state.frame.wrapping_add(1);
    if state.first.is_none() && !state.done {
        let streak = state.shared.ready_streak.load(Ordering::SeqCst);
        let late = state.started.elapsed() > READY_LIMIT;
        if streak >= READY_FRAMES || late {
            if late {
                eprintln!(
                    "Record: render pipelines still compiling after {} s; recording anyway",
                    READY_LIMIT.as_secs()
                );
            }
            eprintln!(
                "Record: everything drawn after {} updates; recording starts (game clock 1/{} s per frame)",
                state.frame, state.settings.format.fps
            );
            state.first = Some(state.frame);
        }
    }
    let recording = state.first.is_some() && !state.done;
    *strategy = TimeUpdateStrategy::ManualDuration(if recording {
        Duration::from_nanos(1_000_000_000 / u64::from(state.settings.format.fps))
    } else {
        Duration::ZERO
    });
    *clock = RecordClock {
        frame: state.frame,
        recording,
        target: state.target.as_ref().map(|t| t.id()),
    };
    let Some(first) = state.first.filter(|_| recording) else {
        return;
    };
    let at = state.frame - first;
    let mut events = Vec::new();
    state.held.retain(|(until, code, logical)| {
        let keep = *until > at;
        if !keep {
            events.push((*code, logical.clone(), ButtonState::Released));
        }
        keep
    });
    for k in state.settings.keys.iter().filter(|k| k.frame == at) {
        events.push((k.code, k.logical.clone(), ButtonState::Pressed));
        eprintln!(
            "Record: frame {at}: key {:?} down for {} frames",
            k.code, k.hold
        );
    }
    let new: Vec<_> = state
        .settings
        .keys
        .iter()
        .filter(|k| k.frame == at)
        .map(|k| (at + k.hold, k.code, k.logical.clone()))
        .collect();
    state.held.extend(new);
    for (key_code, logical_key, s) in events {
        keys.write(KeyboardInput {
            key_code,
            logical_key,
            state: s,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
}

/// Every scene's presentation camera draws into the recorded image.
/// Runs before Bevy's `camera_system` (`CameraUpdateSystems`): that system
/// computes a camera's target size only when the camera is new or its
/// projection or target image changed, so a scene camera spawned later
/// (Online Play, the deck editor, the duel) and retargeted after it had
/// run kept no target size and drew nothing. Touching the projection
/// makes it compute the image's size.
fn retarget_cameras(
    mut commands: Commands,
    state: Res<Recording>,
    // The scene switch's fade (`mode_fade`, cameras 100/101 over the scene)
    // is recorded too.
    mut cameras: Query<
        (Entity, Option<&RenderTarget>, Option<&mut Projection>),
        Or<(With<PresentationCamera>, With<super::mode_fade::Overlay>)>,
    >,
) {
    let Some(target) = &state.target else { return };
    for (entity, current, projection) in &mut cameras {
        let ok = matches!(current, Some(RenderTarget::Image(i)) if i.handle == *target);
        if !ok {
            commands
                .entity(entity)
                .insert(RenderTarget::Image(target.clone().into()));
            if let Some(mut p) = projection {
                p.set_changed();
            }
        }
    }
}

fn on_readback(event: On<ReadbackComplete>, mut state: ResMut<Recording>) {
    let tag = state.shared.tags.lock().unwrap().pop_front();
    if let Some(tag) = tag {
        state.received.push_back((tag, event.data.clone()));
    }
}

/// After the update: its sound, then encode what the GPU has returned.
fn end_frame(
    mut state: ResMut<Recording>,
    sound: Option<Res<super::duel_board_render::DuelSound>>,
    mut exit: ResMut<Messages<AppExit>>,
) {
    let state = &mut *state;
    if state.done {
        return;
    }
    // The game itself is closing (an exit written this update before Last).
    let closing = !exit.is_empty();
    let format = state.settings.format;
    let spf = format.samples_per_frame() as usize;
    if state.first.is_some() {
        let mut pcm = vec![0i16; spf * 2];
        if let Some(mixer) = sound.as_ref().and_then(|s| s.mixer()) {
            if let Ok(mut m) = mixer.lock() {
                // Effects this update started (each begins at this
                // frame's first sound frame).
                let started = m.se_started();
                if started > state.se_seen {
                    if let Some(first) = state.first {
                        eprintln!(
                            "Record: frame {}: {} sound effect(s) start",
                            state.frame - first,
                            started - state.se_seen
                        );
                    }
                }
                state.se_seen = started;
                m.mix(&mut pcm);
            }
        }
        state.audio.push_back((state.frame, pcm));
    }
    let row = SOURCE as usize * 4;
    let padded = row.div_ceil(256) * 256;
    while let Some((tag, mut data)) = state.received.pop_front() {
        let Some(recorder) = state.recorder.as_mut() else {
            break;
        };
        if state.first.is_none_or(|f| tag < f) || state.next.is_some_and(|n| tag < n) {
            continue;
        }
        if padded != row && data.len() == padded * SOURCE as usize {
            data = data
                .chunks_exact(padded)
                .flat_map(|r| r[..row].to_vec())
                .collect();
        }
        let next = *state.next.get_or_insert(tag);
        // A frame the GPU never returned repeats the last picture (keeps
        // the clock; reported).
        let mut result = Ok(());
        for n in next..=tag {
            let pixels = if n == tag {
                data.clone()
            } else {
                state.last.clone().unwrap_or_else(|| data.clone())
            };
            if n != tag {
                state.gaps += 1;
            }
            while state.audio.front().is_some_and(|(f, _)| *f < n) {
                state.audio.pop_front();
            }
            let pcm = match state.audio.front() {
                Some((f, _)) if *f == n => {
                    state.audio.pop_front().map(|(_, p)| p).unwrap_or_default()
                }
                _ => vec![0; spf * 2],
            };
            if let Some(wav) = state.wav.as_mut() {
                wav.write(&pcm);
            }
            result = recorder.audio(pcm).and_then(|()| recorder.frame(pixels));
            if result.is_err() {
                break;
            }
        }
        if let Err(e) = result {
            fail(state, e);
            exit.write(AppExit::error());
            return;
        }
        state.next = Some(tag + 1);
        state.last = Some(data);
        if state
            .settings
            .frames
            .is_some_and(|f| state.recorder.as_ref().is_some_and(|r| r.frames() >= f))
        {
            finish(state);
            exit.write(
                if state
                    .outcome
                    .0
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_some_and(|r| r.is_ok())
                {
                    AppExit::Success
                } else {
                    AppExit::error()
                },
            );
            return;
        }
    }
    // The game itself is closing: keep what was recorded.
    if closing {
        finish(state);
        return;
    }
    // `ROSES_RECORD_STOP_FILE=<path>`: a recording of unknown length (a
    // whole duel) ends cleanly once that file exists (checked each second).
    // Killing the process instead leaves an MP4 without its index.
    if state.frame % 60 == 0
        && std::env::var_os("ROSES_RECORD_STOP_FILE").is_some_and(|p| std::path::Path::new(&p).exists())
    {
        eprintln!("Record: ROSES_RECORD_STOP_FILE exists; finishing");
        finish(state);
        let ok = state.outcome.0.lock().unwrap().as_ref().is_some_and(|r| r.is_ok());
        exit.write(if ok { AppExit::Success } else { AppExit::error() });
    }
}

/// `--record-realtime`: each recorded frame ends no earlier than its time on
/// the wall clock (frame n at start + n / fps), so the game (whose clock
/// moves exactly 1 / fps per frame) keeps pace with the network and the
/// other players. A slow frame is caught up by the next ones (no sleep
/// while behind); only when more than 2 s behind does the schedule restart
/// from now (reported), as the game cannot skip frames.
fn pace_realtime(state: Res<Recording>, mut base: Local<Option<(Instant, u64)>>, mut lagged: Local<u32>) {
    if !state.settings.realtime || state.done {
        return;
    }
    let Some(first) = state.first else { return };
    let n = state.frame.saturating_sub(first) + 1;
    let (t0, n0) = *base.get_or_insert((Instant::now(), n));
    let due = t0 + Duration::from_secs_f64((n - n0) as f64 / f64::from(state.settings.format.fps));
    let now = Instant::now();
    if due > now {
        std::thread::sleep(due - now);
    } else if now - due > Duration::from_secs(2) {
        *lagged += 1;
        if *lagged <= 5 {
            eprintln!("Record: --record-realtime is {:.1} s behind the wall clock at frame {n}; pacing from here", (now - due).as_secs_f64());
        }
        *base = Some((now, n));
    }
}

fn finish(state: &mut Recording) {
    state.done = true;
    if let Some(wav) = state.wav.take() {
        wav.close();
    }
    let Some(recorder) = state.recorder.take() else {
        return;
    };
    let fps = recorder.format().fps;
    let result = recorder.finish().map(|s| {
        format!(
            "Record: saved {} ({} frames, {:.2} s, {} sound frames, {} encoder{})",
            state.settings.path.display(),
            s.frames,
            s.frames as f64 / f64::from(fps),
            s.audio_frames,
            if s.hardware {
                "hardware-allowed"
            } else {
                "software"
            },
            if state.gaps > 0 {
                format!("; {} frames repeated for missing readbacks", state.gaps)
            } else {
                String::new()
            }
        )
    });
    match &result {
        Ok(line) => println!("{line}"),
        Err(e) => eprintln!("Record: {e}"),
    }
    *state.outcome.0.lock().unwrap() = Some(result);
}

/// A 48 kHz stereo 16-bit WAV written as the recording goes.
struct Wav {
    file: std::io::BufWriter<std::fs::File>,
    path: PathBuf,
    frames: u64,
}

impl Wav {
    fn create(path: PathBuf) -> Option<Self> {
        let file = std::fs::File::create(&path)
            .map_err(|e| eprintln!("Record: ROSES_RECORD_WAV {}: {e}", path.display()))
            .ok()?;
        let mut w = Self {
            file: std::io::BufWriter::new(file),
            path,
            frames: 0,
        };
        w.header();
        Some(w)
    }

    fn header(&mut self) {
        use std::io::Write;
        let data = (self.frames * 4) as u32;
        let mut h = Vec::with_capacity(44);
        h.extend_from_slice(b"RIFF");
        h.extend_from_slice(&(36 + data).to_le_bytes());
        h.extend_from_slice(b"WAVEfmt ");
        h.extend_from_slice(&16u32.to_le_bytes());
        h.extend_from_slice(&[1, 0, 2, 0]);
        h.extend_from_slice(&48_000u32.to_le_bytes());
        h.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
        h.extend_from_slice(&[4, 0, 16, 0]);
        h.extend_from_slice(b"data");
        h.extend_from_slice(&data.to_le_bytes());
        let _ = self.file.write_all(&h);
    }

    fn write(&mut self, pcm: &[i16]) {
        use std::io::Write;
        let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
        let _ = self.file.write_all(&bytes);
        self.frames += pcm.len() as u64 / 2;
    }

    fn close(mut self) {
        use std::io::{Seek, SeekFrom, Write};
        let _ = self.file.flush();
        if self.file.seek(SeekFrom::Start(0)).is_ok() {
            self.header();
        }
        let _ = self.file.flush();
        eprintln!("Record: sound also written to {}", self.path.display());
    }
}

fn extract_clock(mut commands: Commands, clock: Extract<Res<RecordClock>>) {
    commands.insert_resource(**clock);
}

/// Render world: readiness (every pipeline compiled, the image prepared)
/// and the update number of each frame whose readback is requested.
fn tag_frame(
    clock: Option<Res<RecordClock>>,
    shared: Res<RecordShared>,
    images: Res<RenderAssets<GpuImage>>,
    cache: Res<PipelineCache>,
    readbacks: Query<&Readback>,
) {
    let Some(clock) = clock else { return };
    let prepared = clock.target.is_some_and(|t| images.get(t).is_some()) && !readbacks.is_empty();
    let compiled = cache.pipelines().all(|p| match &p.state {
        CachedPipelineState::Ok(_) => true,
        CachedPipelineState::Err(
            ShaderCacheError::ShaderNotLoaded(_) | ShaderCacheError::ShaderImportNotYetAvailable,
        ) => false,
        // A pipeline that failed for good never becomes ready.
        CachedPipelineState::Err(_) => true,
        _ => false,
    });
    if prepared && compiled {
        shared.ready_streak.fetch_add(1, Ordering::SeqCst);
    } else if !clock.recording {
        shared.ready_streak.store(0, Ordering::SeqCst);
    }
    // Every requested readback is tagged (also before recording starts),
    // so the ones in flight when it starts keep the queue aligned.
    if prepared {
        shared.tags.lock().unwrap().push_back(clock.frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_options_and_key_script() {
        let args = |v: &[&str]| v.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
        let (r, rest) = RecordArgs::take(args(&[
            "--game",
            "g.json",
            "--record",
            "a.mp4",
            "--record-fps",
            "30",
            "--record-seconds",
            "2",
        ]))
        .unwrap();
        assert_eq!(rest, args(&["--game", "g.json"]));
        assert_eq!((r.fps, r.seconds), (Some(30), Some(2.0)));
        assert!(RecordArgs::take(args(&["--record-fps", "30"])).is_err());
        assert!(RecordArgs::take(args(&["--record", "a.mp4", "--record-bogus"])).is_err());
        let keys = parse_keys("120:down, 2.5s:x*10", 60).unwrap();
        assert_eq!(
            (keys[0].frame, keys[0].hold, keys[0].code),
            (120, 3, KeyCode::ArrowDown)
        );
        assert_eq!(
            (keys[1].frame, keys[1].hold, keys[1].code),
            (150, 10, KeyCode::KeyX)
        );
        assert!(parse_keys("5:nokey", 60).is_err());
        assert!(parse_keys("five:x", 60).is_err());
    }
}
