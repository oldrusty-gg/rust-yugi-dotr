//! A software mixer for the game's sound driver (`KCEJEAST.IRX`, traced in
//! the project notes): one music stream and 28 sound-effect
//! voices mixed to 48 kHz stereo.
//! - Music: 24 kHz stereo PCM, each frame output twice (the original's
//!   frame doubling for the SPU's 48 kHz), looping between its loop points
//!   or playing once; mode 0 (0x10BC90) fades the current track out by
//!   `speed` per field (1/50 s) and then starts the request at full volume;
//!   mode 1 (0x10BD60) keeps a track that already plays [F; the fade tick is
//!   H].
//! - Sound effects: ports 0..27; allocation (`SD_vPlayVAGOnAPort`): a
//!   request flagged "skip if playing" is dropped while the same entry
//!   plays; a voice-group low nibble stops the slot's voices of that group;
//!   a high nibble limits the entry's voices; otherwise the first free port,
//!   else the lowest priority below the new one, else the oldest [F; the two
//!   group helpers' exact rules are H]. Volume (vol << 7 | 0xFF) / 0x7FFF on
//!   both sides; pitch 0x1000 = one sample per output sample (linear
//!   interpolation; the SPU's Gaussian filter is not reproduced).
use std::sync::Arc;

pub const OUTPUT_RATE: u32 = 48_000;
pub const VOICES: usize = 28;
const FIELD_FRAMES: u32 = OUTPUT_RATE / 50;

/// A music track ready to play.
#[derive(Debug)]
pub struct MusicPcm {
    pub track: u32,
    /// Interleaved stereo at 24 kHz.
    pub samples: Arc<[i16]>,
    /// (loop start, loop end) frames, `None` = once.
    pub looped: Option<(usize, usize)>,
}

/// A sound effect ready to play (one bank entry).
#[derive(Clone, Debug)]
pub struct SeSample {
    pub samples: Arc<[i16]>,
    pub loop_start: Option<usize>,
    pub volume: u8,
    pub priority: u8,
    pub voice_group: u8,
    /// SPU pitch (0x1000 = 48 kHz).
    pub pitch: u32,
}

struct Music {
    pcm: Arc<MusicPcm>,
    frame: usize,
    second: bool,
    /// 0..=0x7FFF.
    volume: i32,
    done: bool,
}

struct Voice {
    se: SeSample,
    slot: u8,
    id: u8,
    /// Position in 1/0x1000 samples.
    pos: u64,
    age: u64,
}

/// A 48 kHz stereo stream played once at full volume beside the music and
/// effects (the intro/ending movies' sound track).
struct Stream {
    samples: Arc<[i16]>,
    frame: usize,
}

#[derive(Default)]
pub struct Mixer {
    stream: Option<Stream>,
    /// Frames of the current stream mixed so far.
    stream_mixed: u64,
    music: Option<Music>,
    next: Option<Arc<MusicPcm>>,
    /// Fade-out per field while set.
    fade: Option<i32>,
    fade_rest: u32,
    voices: Vec<Option<Voice>>,
    age: u64,
    /// Sound: Mono (the card block's option, 0x217200 -> 0x10A830(1)).
    mono: bool,
}

impl Mixer {
    pub fn new() -> Self {
        Self { voices: (0..VOICES).map(|_| None).collect(), ..Default::default() }
    }

    /// 0x217110(track, mode): mode 0 fades the current track out first
    /// (speed 0x100); mode 1 keeps a track that already plays.
    pub fn play_music(&mut self, pcm: Arc<MusicPcm>, keep_if_same: bool) {
        let same = self.music.as_ref().is_some_and(|m| m.pcm.track == pcm.track && !m.done);
        if same && keep_if_same {
            if let Some(m) = &mut self.music {
                m.volume = 0x7FFF;
            }
            self.fade = None;
            self.next = None;
            return;
        }
        if self.music.as_ref().is_some_and(|m| !m.done) {
            self.next = Some(pcm);
            self.fade = Some(0x100);
        } else {
            self.start(pcm);
        }
    }

    fn start(&mut self, pcm: Arc<MusicPcm>) {
        self.music = Some(Music { pcm, frame: 0, second: false, volume: 0x7FFF, done: false });
        self.fade = None;
    }

    /// 0x2171A0(speed): fade the music out (speed <= 0 stops it).
    pub fn fade_out_music(&mut self, speed: i32) {
        self.next = None;
        if speed <= 0 {
            self.music = None;
        } else {
            self.fade = Some(speed);
        }
    }

    /// 0x217150: a track plays.
    /// 0x217200(stereo): the output mode. Mono sends (L + R) / 2 to both
    /// channels [H: the IOP's own downmix is not traced].
    pub fn set_mono(&mut self, mono: bool) {
        self.mono = mono;
    }

    pub fn mono(&self) -> bool {
        self.mono
    }

    pub fn music_busy(&self) -> bool {
        self.music.as_ref().is_some_and(|m| !m.done) || self.next.is_some()
    }

    /// The track playing (or about to).
    pub fn music_track(&self) -> Option<u32> {
        self.next.as_ref().map(|n| n.track).or(self.music.as_ref().filter(|m| !m.done).map(|m| m.pcm.track))
    }

    /// 0x216EA0(slot, id): start a sound effect.
    pub fn play_se(&mut self, slot: u8, id: u8, se: &SeSample, skip_if_playing: bool) {
        let playing = |v: &Option<Voice>| v.as_ref().is_some_and(|v| v.slot == slot && v.id == id);
        if skip_if_playing && self.voices.iter().any(playing) {
            return;
        }
        let group = se.voice_group & 15;
        if group != 0 {
            for v in &mut self.voices {
                if v.as_ref().is_some_and(|v| v.slot == slot && v.se.voice_group & 15 == group) {
                    *v = None;
                }
            }
        }
        let limit = usize::from(se.voice_group >> 4);
        if limit != 0 {
            let mut same: Vec<usize> = (0..VOICES).filter(|&i| playing(&self.voices[i])).collect();
            if same.len() >= limit {
                same.sort_by_key(|&i| self.voices[i].as_ref().map_or(0, |v| v.age));
                self.voices[same[0]] = None;
            }
        }
        let port = (0..VOICES)
            .find(|&i| self.voices[i].is_none())
            .or_else(|| {
                (0..VOICES)
                    .filter(|&i| self.voices[i].as_ref().is_some_and(|v| v.se.priority < se.priority))
                    .min_by_key(|&i| self.voices[i].as_ref().map_or(0, |v| v.se.priority))
            })
            .or_else(|| (0..VOICES).min_by_key(|&i| self.voices[i].as_ref().map_or(0, |v| v.age)));
        if let Some(p) = port {
            self.age += 1;
            self.voices[p] = Some(Voice { se: se.clone(), slot, id, pos: 0, age: self.age });
        }
    }

    /// Sound effects started so far (the video recorder notes the frames).
    pub fn se_started(&self) -> u64 {
        self.age
    }

    /// 0x217050(slot, id): stop an entry (`None`: the whole slot).
    pub fn stop_se(&mut self, slot: u8, id: Option<u8>) {
        for v in &mut self.voices {
            if v.as_ref().is_some_and(|v| v.slot == slot && id.is_none_or(|i| v.id == i)) {
                *v = None;
            }
        }
    }

    /// Start a 48 kHz interleaved stereo stream (replacing any other). It
    /// plays once; afterwards silence is counted until `stop_stream`.
    pub fn play_stream(&mut self, samples: Arc<[i16]>) {
        self.stream = Some(Stream { samples, frame: 0 });
        self.stream_mixed = 0;
    }

    pub fn stop_stream(&mut self) {
        self.stream = None;
    }

    /// Stereo frames of the current (or last) stream mixed so far; the
    /// output device plays them a few buffers later.
    pub fn stream_frames_mixed(&self) -> u64 {
        self.stream_mixed
    }

    /// Mix `out.len() / 2` stereo frames.
    pub fn mix(&mut self, out: &mut [i16]) {
        for frame in out.chunks_exact_mut(2) {
            let (mut l, mut r) = (0i32, 0i32);
            if let Some(st) = &mut self.stream {
                // Past its end the stream is silence but keeps counting, so
                // a movie's clock runs on until the player stops it.
                let at = st.frame * 2;
                if at + 1 < st.samples.len() {
                    l += i32::from(st.samples[at]);
                    r += i32::from(st.samples[at + 1]);
                }
                st.frame += 1;
                self.stream_mixed += 1;
            }
            if let Some(m) = &mut self.music {
                if !m.done {
                    let at = m.frame * 2;
                    let (a, b) = (
                        i32::from(*m.pcm.samples.get(at).unwrap_or(&0)),
                        i32::from(*m.pcm.samples.get(at + 1).unwrap_or(&0)),
                    );
                    l += a * m.volume / 0x7FFF;
                    r += b * m.volume / 0x7FFF;
                    if m.second {
                        m.frame += 1;
                        match m.pcm.looped {
                            Some((start, end)) if m.frame >= end => m.frame = start,
                            None if m.frame * 2 >= m.pcm.samples.len() => m.done = true,
                            _ => {}
                        }
                    }
                    m.second = !m.second;
                }
            }
            if let Some(speed) = self.fade {
                self.fade_rest += 1;
                if self.fade_rest >= FIELD_FRAMES {
                    self.fade_rest = 0;
                    let mut finished = true;
                    if let Some(m) = &mut self.music {
                        m.volume = (m.volume - speed).max(0);
                        finished = m.volume == 0;
                    }
                    if finished {
                        self.fade = None;
                        match self.next.take() {
                            Some(next) => self.start(next),
                            None => self.music = None,
                        }
                    }
                }
            }
            for slot in &mut self.voices {
                let Some(v) = slot else { continue };
                let i = (v.pos >> 12) as usize;
                let s = &v.se.samples;
                if i >= s.len() {
                    *slot = None;
                    continue;
                }
                let f = (v.pos & 0xFFF) as i32;
                let next = s.get(i + 1).copied().unwrap_or(0);
                let x = (i32::from(s[i]) * (0x1000 - f) + i32::from(next) * f) >> 12;
                let vol = (i32::from(v.se.volume) << 7) | 0xFF;
                let y = x * vol / 0x7FFF;
                l += y;
                r += y;
                v.pos += u64::from(v.se.pitch);
                if (v.pos >> 12) as usize >= s.len() {
                    match v.se.loop_start {
                        Some(ls) => v.pos = (ls as u64) << 12,
                        None => *slot = None,
                    }
                }
            }
            if self.mono {
                let m = (l + r) / 2;
                l = m;
                r = m;
            }
            frame[0] = l.clamp(-32768, 32767) as i16;
            frame[1] = r.clamp(-32768, 32767) as i16;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_sends_the_mean_of_both_channels_to_both() {
        let mut m = Mixer::new();
        m.play_stream(vec![1000i16, -200, 300, 500].into());
        m.set_mono(true);
        let mut out = [0i16; 4];
        m.mix(&mut out);
        assert_eq!(out, [400, 400, 400, 400]);
    }

    #[test]
    fn music_frames_are_doubled_and_loop() {
        let pcm = Arc::new(MusicPcm { track: 1, samples: vec![1, 1, 2, 2, 3, 3].into(), looped: Some((1, 3)) });
        let mut m = Mixer::new();
        m.play_music(pcm, false);
        let mut out = vec![0i16; 2 * 8];
        m.mix(&mut out);
        let left: Vec<i16> = out.iter().step_by(2).copied().collect();
        assert_eq!(left, [1, 1, 2, 2, 3, 3, 2, 2]);
    }

    #[test]
    fn mode_zero_fades_then_starts_the_request() {
        let a = Arc::new(MusicPcm { track: 8, samples: vec![1000; 200_000].into(), looped: Some((0, 100_000)) });
        let b = Arc::new(MusicPcm { track: 17, samples: vec![2000; 200_000].into(), looped: Some((0, 100_000)) });
        let mut m = Mixer::new();
        m.play_music(a, false);
        m.play_music(b, false);
        assert_eq!(m.music_track(), Some(17));
        // 0x7FFF / 0x100 = 128 fields.
        let mut out = vec![0i16; 2 * 960 * 129];
        m.mix(&mut out);
        assert_eq!(out[out.len() - 2], 2000);
    }

    #[test]
    fn stream_plays_once_at_full_rate() {
        let mut m = Mixer::new();
        m.play_stream(vec![1, -1, 2, -2, 3, -3].into());
        let mut out = vec![0i16; 2 * 5];
        m.mix(&mut out);
        assert_eq!(out, [1, -1, 2, -2, 3, -3, 0, 0, 0, 0]);
        assert_eq!(m.stream_frames_mixed(), 5);
        m.stop_stream();
        m.mix(&mut out);
        assert_eq!(m.stream_frames_mixed(), 5);
    }

    #[test]
    fn skip_if_playing_drops_a_repeat() {
        let se = SeSample { samples: vec![1000; 100].into(), loop_start: None, volume: 127, priority: 1, voice_group: 0, pitch: 0x1000 };
        let mut m = Mixer::new();
        m.play_se(0, 4, &se, true);
        m.play_se(0, 4, &se, true);
        assert_eq!(m.voices.iter().filter(|v| v.is_some()).count(), 1);
        m.play_se(0, 4, &se, false);
        assert_eq!(m.voices.iter().filter(|v| v.is_some()).count(), 2);
    }
}
