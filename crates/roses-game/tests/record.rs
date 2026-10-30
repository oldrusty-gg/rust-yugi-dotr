//! The video recorder (`--record`, a tool) on the real game, headless, with
//! ROSES_MUTE=1 (the file still gets the game's sound). Ignored: they need
//! the user's own game import.
//!
//! `ROSES_RECORD_TEST_GAME=<game.json> [ROSES_RECORD_TEST_OUT=<dir>]
//! cargo test --features renderer --test record -- --ignored`
#![cfg(windows)]
use std::path::{Path, PathBuf};
use std::process::Command;

fn game() -> PathBuf {
    PathBuf::from(
        std::env::var_os("ROSES_RECORD_TEST_GAME").expect("ROSES_RECORD_TEST_GAME=<game.json>"),
    )
}

fn out_dir() -> PathBuf {
    let dir = std::env::var_os("ROSES_RECORD_TEST_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("record"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fresh(name: &str) -> PathBuf {
    let path = out_dir().join(name);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(platform_video::segment_sidecar(&path));
    path
}

/// The title screen, recorded headless and muted.
fn record(out: &Path, seconds: &str, keys: Option<&str>) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_roses-bevy-scene"));
    // The exact mixed sound beside the clip (ROSES_RECORD_WAV).
    let wav = out.with_extension("wav");
    let _ = std::fs::remove_file(&wav);
    cmd.env("ROSES_MUTE", "1")
        .env("ROSES_RECORD_WAV", &wav)
        .arg("--game")
        .arg(game())
        .args([
            "--start-stage",
            "title",
            "--game-seed",
            "1",
            "--record-seconds",
            seconds,
            "--record",
        ])
        .arg(out);
    if let Some(keys) = keys {
        cmd.args(["--record-keys", keys]);
    }
    let run = cmd.output().unwrap();
    let log =
        String::from_utf8_lossy(&run.stderr).to_string() + &String::from_utf8_lossy(&run.stdout);
    assert!(run.status.success(), "recorder failed:\n{log}");
    assert!(log.contains("Record: saved"), "{log}");
    log
}

#[test]
#[ignore = "needs ROSES_RECORD_TEST_GAME (the user's game import)"]
fn title_screen_ten_seconds() {
    let out = fresh("title-10s.mp4");
    record(&out, "10", None);
    let info = platform_video::mp4::inspect(&out).unwrap();
    let video = info.track("vide").unwrap();
    assert_eq!((video.codec.as_str(), video.samples), ("avc1", 600));
    assert_eq!(info.track("soun").unwrap().codec, "mp4a");
    // Read back through Media Foundation's source reader.
    let p = platform_video::probe(&out).unwrap();
    assert_eq!((p.width, p.height, p.fps), (1920, 1080, (60, 1)));
    assert_eq!(p.video_frames, 600);
    assert!(
        (p.video_end - p.video_start - 100_000_000).abs() <= 1,
        "video lasts {} (100 ns)",
        p.video_end - p.video_start
    );
    let sound_s = p.audio_frames as f64 / 48_000.0;
    assert!((sound_s - 10.0).abs() < 0.05, "sound lasts {sound_s} s");
    // ROSES_MUTE=1 silenced the speakers, not the file: the title music.
    let a = platform_video::decode_audio(&out).unwrap();
    let rms = (a.samples.iter().map(|&s| f64::from(s).powi(2)).sum::<f64>()
        / a.samples.len() as f64)
        .sqrt();
    assert!(rms > 200.0, "the title music is missing (rms {rms})");
    // The 4:3 picture with black bars at the sides.
    let mut checked = 0;
    platform_video::decode_video_luma(&out, |i, _, luma, stride| {
        if i == 300 {
            assert!(
                luma[540 * stride + 100] < 24 && luma[540 * stride + 1820] < 24,
                "side bars"
            );
            let inside = (240..1680)
                .map(|x| u32::from(luma[400 * stride + x]))
                .max()
                .unwrap();
            assert!(inside > 60, "picture");
            checked += 1;
        }
    })
    .unwrap();
    assert_eq!(checked, 1);
}

fn luma_frames(path: &Path) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    platform_video::decode_video_luma(path, |_, _, luma, stride| {
        // Every 4th pixel of every 4th row is plenty to see a change.
        frames.push(
            luma.chunks(stride)
                .step_by(4)
                .flat_map(|r| r[..1920].iter().step_by(4).copied())
                .collect(),
        );
    })
    .unwrap();
    frames
}

/// The uncompressed sound the recorder handed the encoder.
fn wav(path: &Path) -> Vec<i16> {
    let bytes = std::fs::read(path.with_extension("wav")).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    bytes[44..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect()
}

/// Per sound frame, the larger channel difference of two stereo tracks.
fn difference(a: &[i16], b: &[i16]) -> Vec<u32> {
    a.chunks_exact(2)
        .zip(b.chunks_exact(2))
        .map(|(x, y)| u32::from(x[0].abs_diff(y[0])).max(u32::from(x[1].abs_diff(y[1]))))
        .collect()
}

/// Start pressed at a known frame, against the same run without it: the
/// picture changes on the press's frame, the press's sound (SYSTEM0) starts
/// inside the frame the game handed it to the mixer, and the MP4's decoded
/// sound has it at the same moment as the mixer's own output (no shift
/// from the encoder), so it lands within one frame of its cue.
#[test]
#[ignore = "needs ROSES_RECORD_TEST_GAME (the user's game import)"]
fn sound_effect_lands_on_its_frame() {
    const PRESS: u64 = 120;
    const SPF: usize = 800;
    let pressed = fresh("sync-press.mp4");
    let plain = fresh("sync-plain.mp4");
    let log = record(&pressed, "4", Some(&format!("{PRESS}:s")));
    // The frame the game handed the press's sound to the mixer (the
    // recorder notes it; the title takes a press's sounds an update later).
    let cue = log
        .lines()
        .filter_map(|l| {
            l.strip_prefix("Record: frame ")?
                .split_once(": ")
                .filter(|(_, r)| r.contains("sound effect"))?
                .0
                .parse::<u64>()
                .ok()
        })
        .find(|&f| f >= PRESS)
        .expect("the press's sound effect");
    record(&plain, "4", None);
    // Picture: the first frame where the two clips differ.
    let (a, b) = (luma_frames(&pressed), luma_frames(&plain));
    assert_eq!((a.len(), b.len()), (240, 240));
    let diff = |i: usize| {
        a[i].iter()
            .zip(&b[i])
            .map(|(x, y)| u32::from(x.abs_diff(*y)))
            .sum::<u32>() as f64
            / a[i].len() as f64
    };
    let before: f64 = (0..PRESS as usize).map(diff).fold(0.0, f64::max);
    assert!(before < 0.5, "the clips differ before the press ({before})");
    let picture = (0..240)
        .find(|&i| diff(i) > 2.0)
        .expect("the press changes the picture");
    // The mixer's own output: the effect's first sample is in its cue frame.
    let raw = difference(&wav(&pressed), &wav(&plain));
    let raw_first = raw
        .iter()
        .position(|&v| v > 0)
        .expect("the effect in the mixed sound");
    assert_eq!(
        raw_first / SPF,
        cue as usize,
        "the effect starts in its cue frame's sound"
    );
    let raw_peak = *raw.iter().max().unwrap();
    let raw_onset = raw.iter().position(|&v| v * 4 >= raw_peak).unwrap();
    // The file: decoded AAC, the same onset rule.
    let (sa, sb) = (
        platform_video::decode_audio(&pressed).unwrap(),
        platform_video::decode_audio(&plain).unwrap(),
    );
    assert_eq!(sa.start, sb.start);
    let d = difference(&sa.samples, &sb.samples);
    let peak = *d.iter().max().unwrap();
    assert!(peak > 1000, "no sound effect ({peak})");
    let onset = d.iter().position(|&v| v * 4 >= peak).unwrap();
    let sound_s = (sa.start as f64 / 1e7) + onset as f64 / 48_000.0;
    let (picture_s, press_s, cue_s) = (
        picture as f64 / 60.0,
        PRESS as f64 / 60.0,
        cue as f64 / 60.0,
    );
    eprintln!(
        "press at frame {PRESS} ({press_s:.4} s): picture changes at frame {picture} ({picture_s:.4} s); sound cued at frame {cue} ({cue_s:.4} s), first mixed sample {:.4} s, onset mixed {:.4} s, in the file {sound_s:.4} s",
        raw_first as f64 / 48_000.0,
        raw_onset as f64 / 48_000.0
    );
    let frame = 1.0 / 60.0;
    assert_eq!(picture as u64, PRESS, "the press shows on its own frame");
    assert!(
        (sound_s - raw_onset as f64 / 48_000.0).abs() <= 0.001,
        "the encoder moved the sound"
    );
    assert!(
        sound_s >= cue_s && sound_s - cue_s <= frame,
        "sound {sound_s:.4} s vs its cue at {cue_s:.4} s"
    );
}
