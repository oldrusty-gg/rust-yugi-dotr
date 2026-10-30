//! Source-derived history → Simon handoff timeline (events 10020 → 10040).
//!
//! Traced from the PAL executable (EE addresses): event 10020's subphase
//! table at 0x2D2300 (handlers 0x21DF8C..0x21E1A8), the fade/camera-level
//! updater at 0x23A05C (mode 1) and the full-screen overlay drawer at
//! 0x225F84..0x226354. Plain Rust, no Bevy, no wall clock: one `step` is one
//! original main-loop update. The ordering of the three per-frame routines
//! inside one update is inferred, so any boundary may be off by one update.
//!
//! Camera poses derived from the level live in `camera.rs`; the Simon white
//! flash (c401 task) is modelled here; its light beam runs as original
//! code (`story_fx`, drawn by the scene). Not modelled: the disc-load stall
//! between events.

/// Global fade command byte 0x1ED0062: 2 = run the level down.
const LEVEL_FULL: i16 = 0x1000;
/// Event 10020 mode-1 level step (0x23A0B0).
const HISTORY_LEVEL_STEP: i16 = 0x10;
/// Overlay mode 3 step (0x226014) and mode 10 step (0x226098).
const OVERLAY_FADE_OUT_STEP: i16 = 0x10;
const OVERLAY_CLOSE_STEP: i16 = 0x400;
/// Event 10040 overlay mode 20 step (0x2260DC).
const SIMON_OVERLAY_STEP: i16 = 0x100;
/// Scene mode-2 level step (0x23A1EC).
const SIMON_LEVEL_STEP: i16 = 0x100;
/// Overlay mode 32 step (0x2261A0).
const SIMON_WHITE_STEP: i16 = 0x20;

/// Colour of the full-screen overlay quad (0x2261E0..0x226298).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayColor {
    /// Modes below 20 in these events: opaque navy (0,0,32) quad.
    Navy,
    /// Modes 20..29.
    Black,
}
impl OverlayColor {
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Self::Navy => [0, 0, 32],
            Self::Black => [0, 0, 0],
        }
    }
}

/// Output of one update: what the presentation should draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransitionFrame {
    pub update: u32,
    /// GS vertex alpha of the overlay quad, 0..=128 (`(value << 7) >> 12`).
    pub overlay_alpha: u8,
    pub overlay_color: OverlayColor,
    /// Camera level 0x1ED0064 (4096 → 0); drives the unimplemented fly-in.
    pub camera_level: i16,
    /// True from the update in which the cloud module sees command 2.
    pub cloud_exit_requested: bool,
    /// Original calls 0x217110(25, 0) once when the level drops below 2000.
    pub simon_music_cue: bool,
    /// Event 10040 at overlay mode 0x15 (0x21E2C8 / 0x21E2D4) plays SYSTEM0
    /// sound effects 0x22 then 0x23, the thunder, once.
    pub thunder: bool,
    /// Additive full-screen flash strength 0..=128 (c401 task, 0x1E001B0):
    /// the frontend adds (200,220,255)·a/128 per channel, clamped, over the
    /// 3D scene and beneath the letterbox bars.
    pub flash_alpha: u8,
    /// The event hands over to 10040 after this update.
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryExit {
    update: u32,
    subphase: u8,
    fade_command: u8,
    level: i16,
    overlay_mode: u8,
    overlay_value: i16,
    music_cued: bool,
}

impl Default for HistoryExit {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryExit {
    /// State when subphase 2 sees display phase 12 after the last page:
    /// navy overlay mode 2 at full value, level full, no fade command.
    pub fn new() -> Self {
        Self {
            update: 0,
            subphase: 3,
            fade_command: 0,
            level: LEVEL_FULL,
            overlay_mode: 2,
            overlay_value: LEVEL_FULL,
            music_cued: false,
        }
    }

    pub fn step(&mut self) -> TransitionFrame {
        let mut music = false;
        let mut done = false;
        // Event 10020 subphase handler (one case per update).
        match self.subphase {
            3 => {
                self.fade_command = 2;
                self.subphase = 4;
            }
            4 => {
                self.overlay_mode = 3;
                self.subphase = 5;
            }
            5 => {
                if self.level < 2000 && !self.music_cued {
                    self.music_cued = true;
                    music = true;
                }
                if self.level < 80 {
                    self.overlay_mode = 10;
                    self.subphase = 6;
                }
            }
            _ => done = self.overlay_mode == 11,
        }
        // Level updater, mode-1 branch.
        if self.fade_command == 2 {
            self.level -= HISTORY_LEVEL_STEP;
            if self.level < 0 {
                self.level = 0;
                self.fade_command = 0;
            }
        }
        // Overlay drawer.
        match self.overlay_mode {
            3 if self.overlay_value >= 0 => {
                self.overlay_value -= OVERLAY_FADE_OUT_STEP;
                if self.overlay_value <= 0 {
                    self.overlay_value = 0;
                    self.overlay_mode = 0;
                }
            }
            10 => {
                self.overlay_value += OVERLAY_CLOSE_STEP;
                if self.overlay_value >= LEVEL_FULL {
                    self.overlay_value = LEVEL_FULL;
                    self.overlay_mode = 11;
                }
            }
            _ => (),
        }
        let frame = TransitionFrame {
            update: self.update,
            overlay_alpha: overlay_alpha(self.overlay_mode, self.overlay_value),
            overlay_color: OverlayColor::Navy,
            camera_level: self.level,
            cloud_exit_requested: self.fade_command == 2 || self.subphase > 3,
            flash_alpha: 0,
            simon_music_cue: music,
            thunder: false,
            done,
        };
        self.update += 1;
        frame
    }
}

/// Event 10040 opening: black overlay (mode 20) fading out before Simon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimonEntry {
    update: u32,
    overlay_mode: u8,
    overlay_value: i16,
    /// Camera level: sub1 issues command 2 once the black cover is gone and
    /// the mode-2 updater (0x23A1E8) lowers it by 0x100 per update.
    level: i16,
}

impl Default for SimonEntry {
    fn default() -> Self {
        Self::new()
    }
}

impl SimonEntry {
    /// Event 10040 subphase 0 sets mode 20 at full value (0x21E254..0x21E264).
    pub fn new() -> Self {
        Self {
            update: 0,
            overlay_mode: 20,
            overlay_value: LEVEL_FULL,
            level: LEVEL_FULL,
        }
    }
    /// Updates stepped so far (the last step's `TransitionFrame::update` + 1).
    pub fn updates(&self) -> u32 {
        self.update
    }
    pub fn step(&mut self) -> TransitionFrame {
        let mut thunder = false;
        if self.overlay_mode == 20 {
            self.overlay_value -= SIMON_OVERLAY_STEP;
            if self.overlay_value <= 0 {
                self.overlay_value = 0;
                self.overlay_mode = 21;
                // [H: the update mode 0x15 is first seen; the call sites are F.]
                thunder = true;
            }
        } else if self.level > 0 {
            self.level = (self.level - SIMON_LEVEL_STEP).max(0);
        } else {
            // Sub2..sub3: overlay modes 30 (+0x400) and 32 (-0x20) run their
            // course before dialogue. The drawer skips event 10040 from
            // subphase 2 (0x2262C4), so they are timing only, not drawn.
            match self.overlay_mode {
                21 => {
                    self.overlay_mode = 30;
                    self.overlay_value = 0;
                }
                30 => {
                    self.overlay_value += OVERLAY_CLOSE_STEP;
                    if self.overlay_value >= LEVEL_FULL {
                        self.overlay_value = LEVEL_FULL;
                        self.overlay_mode = 31;
                    }
                }
                31 => self.overlay_mode = 32,
                32 => {
                    self.overlay_value -= SIMON_WHITE_STEP;
                    if self.overlay_value <= 0 {
                        self.overlay_value = 0;
                        self.overlay_mode = 33;
                    }
                }
                _ => (),
            }
        }
        let frame = TransitionFrame {
            update: self.update,
            overlay_alpha: if self.overlay_mode == 20 {
                overlay_alpha(self.overlay_mode, self.overlay_value)
            } else {
                0
            },
            overlay_color: OverlayColor::Black,
            camera_level: self.level,
            cloud_exit_requested: false,
            flash_alpha: flash_alpha(self.update),
            simon_music_cue: false,
            thunder,
            // Sub4 starts dialogue and keeps track 25 after mode 33. The c401
            // flash is modelled here; its beam is `story_fx`.
            done: self.overlay_mode == 33,
        };
        self.update += 1;
        frame
    }
}

/// Flash colour of the c401 sprite (RGB 0xC8, 0xDC, 0xFF).
pub const FLASH_RGB: [u8; 3] = [200, 220, 255];
/// Updates from the start of the black cover fade to the flash (k = 0).
/// The flash starts at c401's task frame 32 (0x1E001B0) and the task runs
/// with the cover: task frame n = entry update + 1 [emu PCSX2 event 10040,
/// `modes/older/c401/emu`: n 7 with the cover value 0x900 (entry update 6),
/// n 22 with level 0xA00 (update 21), n 36 flash k 4 at level 0]. (An
/// earlier 18, read off dumps, put the flash before the camera's tilt.)
pub const FLASH_START_UPDATE: u32 = 31;
/// Flash frames k = 0..=48 with As = trunc(128·(48−k)/48) (0x1E001B0..0x1E00328).
fn flash_alpha(update: u32) -> u8 {
    match update.checked_sub(FLASH_START_UPDATE) {
        Some(k) if k <= 48 => (128 * (48 - k) / 48) as u8,
        _ => 0,
    }
}

/// Add the flash to one byte-space RGB pixel, as the GS additive blend
/// Cd + Cs·As/128 with clamping.
pub fn apply_flash(rgb: [u8; 3], alpha: u8) -> [u8; 3] {
    std::array::from_fn(|i| {
        let add = u32::from(FLASH_RGB[i]) * u32::from(alpha.min(128)) / 128;
        (u32::from(rgb[i]) + add).min(255) as u8
    })
}

fn overlay_alpha(mode: u8, value: i16) -> u8 {
    if mode == 0 {
        0
    } else {
        ((i32::from(value) << 7) >> 12).clamp(0, 128) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(exit: &mut HistoryExit) -> Vec<TransitionFrame> {
        let mut frames = Vec::new();
        loop {
            let frame = exit.step();
            frames.push(frame);
            if frame.done {
                return frames;
            }
            assert!(frames.len() < 1000, "history exit must terminate");
        }
    }

    #[test]
    fn history_exit_follows_traced_level_and_overlay_steps() {
        let frames = run(&mut HistoryExit::new());
        // Command 2 on the first update; cloud exit begins there.
        assert!(frames[0].cloud_exit_requested);
        assert_eq!(frames[0].camera_level, 0x1000 - 0x10);
        assert_eq!(frames[0].overlay_alpha, 128);
        // Navy overlay fades by 16/4096 per update from the second update.
        assert_eq!(frames[2].overlay_alpha, 127);
        // Music cue exactly once, the update after the level passes below 2000.
        let cues: Vec<_> = frames.iter().filter(|f| f.simon_music_cue).collect();
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].update, 132);
        // Level < 80 → overlay closes at +0x400 per update, then handover.
        let close = frames.iter().position(|f| f.camera_level < 80).unwrap();
        assert_eq!(close, 251);
        let last = frames.last().unwrap();
        assert_eq!((last.update, last.overlay_alpha), (256, 128));
        assert!(frames.iter().all(|f| f.overlay_color == OverlayColor::Navy));
    }

    #[test]
    fn simon_entry_thunder_plays_once_before_the_flash() {
        let mut entry = SimonEntry::new();
        let thunder: Vec<u32> = (0..120).map(|_| entry.step()).filter(|f| f.thunder).map(|f| f.update).collect();
        // The black cover's 16 steps end at update 15; the flash starts at 31.
        assert_eq!(thunder, [15]);
    }

    #[test]
    fn simon_entry_flash_matches_traced_law() {
        let mut entry = SimonEntry::new();
        let flashes: Vec<u8> = (0..80).map(|_| entry.step().flash_alpha).collect();
        let f = FLASH_START_UPDATE as usize;
        assert!(flashes[..f].iter().all(|&a| a == 0));
        assert_eq!(flashes[f], 128);
        assert_eq!(flashes[f + 24], 64);
        assert_eq!(flashes[f + 47], 2);
        assert_eq!(flashes[f + 48], 0);
        assert!(flashes[f + 49..].iter().all(|&a| a == 0));
        // Pixel check from the dumps: a dark pixel gains (200, 220→216 in
        // 5-bit output, 255) at k = 0 and (96, 112, 128) at k = 24.
        assert_eq!(apply_flash([0, 0, 0], 128), [200, 220, 255]);
        assert_eq!(apply_flash([0, 0, 0], 64), [100, 110, 127]);
        assert_eq!(apply_flash([100, 100, 100], 128), [255, 255, 255]);
    }

    #[test]
    fn simon_entry_black_fade_then_camera_tilt() {
        let mut entry = SimonEntry::new();
        let frames: Vec<_> = (0..16).map(|_| entry.step()).collect();
        assert_eq!(frames[0].overlay_alpha, 120);
        assert_eq!(frames[14].overlay_alpha, 8);
        assert!(!frames[14].done);
        assert_eq!(frames[15].overlay_alpha, 0);
        assert_eq!(frames[15].camera_level, 0x1000);
        assert!(!frames[15].done && frames[15].overlay_color == OverlayColor::Black);
        // Then the camera tilts down over 16 updates before dialogue.
        let tilt: Vec<_> = (0..16).map(|_| entry.step()).collect();
        assert_eq!(tilt[0].camera_level, 0xF00);
        assert_eq!(tilt[15].camera_level, 0);
        // Undrawn white overlay timing: 1 + 4 + 1 + 128 updates, then dialogue.
        let mut rest = 0;
        loop {
            let f = entry.step();
            rest += 1;
            assert_eq!(f.overlay_alpha, 0);
            if f.done {
                break;
            }
            assert!(rest < 200);
        }
        assert_eq!(rest, 134);
    }
}
