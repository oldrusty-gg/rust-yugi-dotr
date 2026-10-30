//! Built-in movie playback for the disc's PSS files (crates/roses-movie):
//! no system codec, no external tool. A background thread decodes the
//! sound track first (it is small and fast), then video frames to RGBA in
//! display order into a short queue; the frontend calls [`NativeMovie::update`]
//! once per frame and uploads [`NativeMovie::frame`] when it changed.
//!
//! The clock is the sound track as mixed (`Mixer::stream_frames_mixed`)
//! when a mixer is given, else wall time (for example with ROSES_MUTE=1, when
//! no output is opened and nothing is audible).
use crate::sound_mix::Mixer;
use std::path::Path;
use std::sync::mpsc::{sync_channel, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Frames queued ahead of display (about a third of a second at 25 fps).
const QUEUE: usize = 8;
/// The mixer runs this far ahead of what is heard (output buffers) [H].
const OUTPUT_LATENCY_S: f64 = 0.06;

/// Lockstep (the video recorder, a tool): the mixer's sound is what is
/// heard (no output buffers), and `update` waits for the decoder instead of
/// showing a late frame, so a recording faster than real time keeps every
/// movie frame in step with its sound.
static LOCKSTEP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Movies opened from now on play in lockstep (see `LOCKSTEP`).
pub fn set_lockstep(on: bool) {
    LOCKSTEP.store(on, std::sync::atomic::Ordering::SeqCst);
}

pub struct MovieFrame {
    pub index: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

enum Msg {
    /// Interleaved stereo at the given rate, or none.
    Audio(Option<(Arc<[i16]>, u32, u32)>),
    Rate(u32, u32),
    Frame(MovieFrame),
    End(Result<(), String>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovieState {
    /// Decoding the sound track before the first frame.
    Starting,
    Playing,
    Ended,
}

enum Clock {
    Mixer(Arc<Mutex<Mixer>>),
    Wall(Instant),
}

pub struct NativeMovie {
    rx: Receiver<Msg>,
    mixer: Option<Arc<Mutex<Mixer>>>,
    clock: Option<Clock>,
    fps: (u32, u32),
    current: Option<MovieFrame>,
    pending: Option<MovieFrame>,
    decoded_all: bool,
    last_index: Option<u64>,
    stream_started: bool,
    lockstep: bool,
}

impl NativeMovie {
    /// Start decoding `path`. `mixer` receives the sound track; pass `None`
    /// when there is no sound output (muted).
    pub fn open(path: &Path, mixer: Option<Arc<Mutex<Mixer>>>) -> Result<Self, String> {
        let path = path.to_path_buf();
        // Fail early on a missing file.
        std::fs::metadata(&path).map_err(|e| format!("movie {}: {e}", path.display()))?;
        let (tx, rx) = sync_channel(QUEUE);
        std::thread::Builder::new()
            .name("movie decode".into())
            .spawn(move || {
                let result = decode(&path, &tx);
                let _ = tx.send(Msg::End(result));
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            rx,
            mixer,
            clock: None,
            fps: (25, 1),
            current: None,
            pending: None,
            decoded_all: false,
            last_index: None,
            stream_started: false,
            lockstep: LOCKSTEP.load(std::sync::atomic::Ordering::SeqCst),
        })
    }

    /// The frame to show now, if any has been reached.
    pub fn frame(&self) -> Option<&MovieFrame> {
        self.current.as_ref()
    }

    fn seconds(&self) -> f64 {
        match &self.clock {
            None => 0.0,
            Some(Clock::Wall(start)) => start.elapsed().as_secs_f64(),
            Some(Clock::Mixer(m)) => {
                let mixed = m.lock().map(|m| m.stream_frames_mixed()).unwrap_or(0);
                let latency = if self.lockstep { 0.0 } else { OUTPUT_LATENCY_S };
                (mixed as f64 / f64::from(crate::sound_mix::OUTPUT_RATE) - latency).max(0.0)
            }
        }
    }

    /// Advance to the current time. Returns the state and whether
    /// [`Self::frame`] changed since the last call.
    pub fn update(&mut self) -> Result<(MovieState, bool), String> {
        let mut changed = false;
        loop {
            // Before the clock starts only the sound track is awaited.
            if self.clock.is_none() {
                match self.next_msg() {
                    Ok(Msg::Audio(audio)) => self.start_clock(audio),
                    Ok(Msg::End(Err(e))) => return Err(e),
                    Ok(_) => return Err("movie decoder sent a frame before its sound".into()),
                    Err(TryRecvError::Empty) => return Ok((MovieState::Starting, false)),
                    Err(TryRecvError::Disconnected) => return Err("movie decoder stopped".into()),
                }
                continue;
            }
            let target = (self.seconds() * f64::from(self.fps.0) / f64::from(self.fps.1)) as u64;
            if let Some(p) = &self.pending {
                if p.index <= target {
                    self.current = self.pending.take();
                    changed = true;
                    continue;
                }
                break;
            }
            if self.decoded_all {
                break;
            }
            match self.next_msg() {
                Ok(Msg::Frame(f)) => {
                    self.last_index = Some(f.index);
                    self.pending = Some(f);
                }
                Ok(Msg::Rate(n, d)) => self.fps = (n.max(1), d.max(1)),
                Ok(Msg::End(Ok(()))) => self.decoded_all = true,
                Ok(Msg::End(Err(e))) => return Err(e),
                Ok(Msg::Audio(_)) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => self.decoded_all = true,
            }
        }
        let end = self.last_index.map_or(0, |i| i + 1) as f64 * f64::from(self.fps.1)
            / f64::from(self.fps.0);
        if self.decoded_all && self.pending.is_none() && self.seconds() >= end {
            return Ok((MovieState::Ended, changed));
        }
        Ok((MovieState::Playing, changed))
    }

    /// The decoder's next message: waits for it in lockstep.
    fn next_msg(&mut self) -> Result<Msg, TryRecvError> {
        if self.lockstep {
            self.rx.recv().map_err(|_| TryRecvError::Disconnected)
        } else {
            self.rx.try_recv()
        }
    }

    fn start_clock(&mut self, audio: Option<(Arc<[i16]>, u32, u32)>) {
        let playable = audio
            .as_ref()
            .is_some_and(|(_, rate, ch)| *rate == crate::sound_mix::OUTPUT_RATE && *ch == 2);
        if let (Some(mixer), true, Some((samples, _, _))) = (&self.mixer, playable, audio) {
            if let Ok(mut m) = mixer.lock() {
                m.play_stream(samples);
                self.stream_started = true;
            }
            self.clock = Some(Clock::Mixer(mixer.clone()));
        } else {
            if self.mixer.is_some() && !playable {
                eprintln!("Movie: sound track is not 48 kHz stereo; playing without sound");
            }
            self.clock = Some(Clock::Wall(Instant::now()));
        }
    }
}

impl Drop for NativeMovie {
    fn drop(&mut self) {
        // The decode thread ends when its next send fails.
        if self.stream_started {
            if let Some(m) = &self.mixer {
                if let Ok(mut m) = m.lock() {
                    m.stop_stream();
                }
            }
        }
    }
}

fn decode(path: &Path, tx: &std::sync::mpsc::SyncSender<Msg>) -> Result<(), String> {
    let open = || {
        std::fs::File::open(path)
            .map(|f| std::io::BufReader::with_capacity(1 << 20, f))
            .map_err(|e| format!("movie {}: {e}", path.display()))
    };
    let (header, pcm) = roses_movie::movie::decode_audio(open()?).map_err(|e| e.to_string())?;
    let audio = header.map(|h| (Arc::<[i16]>::from(pcm), h.sample_rate, h.channels));
    if tx.send(Msg::Audio(audio)).is_err() {
        return Ok(());
    }
    let mut reader = roses_movie::MovieReader::new(open()?);
    let mut rate_sent = false;
    while let Some(frame) = reader.next_frame().map_err(|e| e.to_string())? {
        let _ = reader.take_audio(); // already decoded above
        if !rate_sent {
            let (n, d) = reader.info().frame_rate;
            rate_sent = true;
            if tx.send(Msg::Rate(n, d)).is_err() {
                return Ok(());
            }
        }
        let mut rgba = vec![0u8; frame.width * frame.height * 4];
        frame.to_rgba(&mut rgba);
        let msg = Msg::Frame(MovieFrame {
            index: frame.display_index,
            width: frame.width as u32,
            height: frame.height as u32,
            rgba,
        });
        if tx.send(msg).is_err() {
            return Ok(()); // player dropped (skipped)
        }
    }
    if reader.video_stats().errors > 0 {
        eprintln!(
            "Movie: {} damaged slices concealed (last: {:?})",
            reader.video_stats().errors,
            reader.last_error()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs ROSES_PSS_DIR with DUMMY.PSS (11 s, 30 fps) from the user's disc.
    /// Nothing is played: the mixer is driven by hand, as the output thread
    /// would, and its samples are compared, never sent to a device.
    #[test]
    fn frames_follow_the_mixed_sound_track() {
        let Some(dir) = std::env::var_os("ROSES_PSS_DIR") else {
            eprintln!("ROSES_PSS_DIR not set; skipped");
            return;
        };
        let path = std::path::Path::new(&dir).join("DUMMY.PSS");
        let (_, expected) = roses_movie::movie::decode_audio(std::fs::File::open(&path).unwrap()).unwrap();
        let mixer = Arc::new(Mutex::new(Mixer::new()));
        let mut movie = NativeMovie::open(&path, Some(mixer.clone())).unwrap();
        let mut mixed: Vec<i16> = Vec::new();
        let mut buf = vec![0i16; 960 * 2];
        let mut steps = 0;
        let deadline = Instant::now() + std::time::Duration::from_secs(120);
        'play: loop {
            // The "device" plays 20 ms per step; before each step the decoder
            // may catch up (it is ~10x faster than real time in practice).
            let heard = mixer.lock().unwrap().stream_frames_mixed();
            let t = (heard as f64 / 48_000.0 - OUTPUT_LATENCY_S).max(0.0);
            let target = ((t * 30.0) as u64).min(329);
            loop {
                assert!(Instant::now() < deadline, "decoder stalled");
                let (state, _) = movie.update().unwrap();
                if state == MovieState::Ended {
                    break 'play;
                }
                if state == MovieState::Playing && movie.frame().is_some_and(|f| f.index >= target) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_micros(200));
            }
            assert_eq!(movie.frame().unwrap().index, target, "frame at {t:.3} s");
            mixer.lock().unwrap().mix(&mut buf);
            mixed.extend_from_slice(&buf);
            steps += 1;
        }
        assert_eq!(&mixed[..expected.len()], &expected[..], "streamed samples differ");
        assert_eq!(movie.frame().map(|f| f.index), Some(329));
        // Not before 11.0 s of video + output latency (553 steps of 20 ms);
        // in this compressed-time test the decoder's end notice may land a
        // few simulated steps later.
        assert!(steps >= 553, "{steps} steps");
        // The clock ran past the sound track's end to show the last frame.
        let heard = mixer.lock().unwrap().stream_frames_mixed();
        assert!(heard as f64 / 48_000.0 - OUTPUT_LATENCY_S >= 11.0);
    }
}
