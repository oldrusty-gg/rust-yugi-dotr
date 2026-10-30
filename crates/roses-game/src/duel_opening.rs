//! The duel's opening (input states 3..=0x12 of 0x1D00F0, before the first
//! turn's camera 0x1DAAA0 and state 0x15), played when the side that moves
//! first is pad-driven (side record +0x33C ≥ 100; a computer or a lesson
//! script skips it, 0x1D0298) [F]. Bevy-free.
//!
//! Source [F 0x1AE89C..0x1AE968, 0x1D02E4..0x1D0818] and timing [PCSX2,
//! `modes/older/intro`: the Weevil duel from session state 652-mup, every
//! main-loop iteration logged through PINE, update u from the first zoom
//! step]:
//! - duel init: camera target (0, 0, 0), polar (50, yaw of the player's
//!   side, 1.5706217 = 0x290358) — straight down from 50 —, every cell's
//!   height −128 (35 units sunk), the emblem scale d34 = 0, both leaders'
//!   alpha +0x53 = 0;
//! - state 3: distance +20 per update while below 600 (70 at u0 .. 610 at
//!   u27);
//! - states 4/5: the cells rise 8 per update (−120 at u28 .. 0 at u43);
//! - states 6/7/8: target and polar tracks (0x1FBAB0 kinds 3 / 4, 30
//!   updates) to the first side's leader cell at y 0 and distance 150
//!   (u44..u73), waiting while a track runs (0x1FB900 before the tracks
//!   step, 0x1FB430);
//! - state 9: the leader emblems grow, d34 +8 per update to 128 (u74..u89;
//!   0x1DDA00 draws them with half size 16.67·d34/128);
//! - states 0xA/0xB/0xC (u90): tracks to the leader at y 40 (+0x345),
//!   distance 100, yaw + π, pitch 0.5891 (0x3F16CBE4) over 30 updates, and
//!   the leaders' appearance effect c620 (0x1F5B00(0, 0x26C) per side) —
//!   when it reports status 2 (u129, 39 updates after its launch [PCSX2])
//!   the leader becomes visible (0x1E2E90 → +0x56 bit 2) and fades in +8
//!   per update (u129 .. 128 at u144);
//! - state 0xD: waits for the tracks and both leaders at alpha 128, then
//!   holds 30 updates (state 0xE, d35, u145..u174);
//! - states 0x10..0x12: overlay slot 0's slide c9c +8 to 64 (u175..u182),
//!   then −8 to 0 (u183..u190); at 0 the first turn's camera starts (u190).
//! c620 itself runs as original code (`duel_opening_fx`) when the game
//! import has CMODULE.MRG.

/// Distance the opening zooms out to (state 3 stops at ≥ 600).
pub const ZOOM_LIMIT: f64 = 600.0;
pub const ZOOM_STEP: f64 = 20.0;
/// The init polar (0x290350): distance 50, pitch 1.5706217.
pub const START_DISTANCE: f64 = 50.0;
pub const START_PITCH: f32 = 1.570_621_7;
pub const HEIGHT_START: i16 = -128;
pub const HEIGHT_STEP: i16 = 8;
pub const TRACK_FRAMES: u32 = 30;
pub const FIRST_DISTANCE: f64 = 150.0;
pub const SECOND_DISTANCE: f64 = 100.0;
/// 0x3F16CBE4.
pub const SECOND_PITCH_BITS: u32 = 0x3F16_CBE4;
/// Side record +0x345 (40 for both sides, 0x1AEA50).
pub const LEADER_HEIGHT: f64 = 40.0;
pub const EMBLEM_STEP: u8 = 8;
pub const FULL: u8 = 128;
/// Updates from c620's launch to its status 2 [PCSX2: u90 → u129].
pub const C620_SHOWS_AFTER: u32 = 39;
pub const LEADER_FADE_STEP: u8 = 8;
pub const HOLD_UPDATES: u8 = 30;
pub const SLIDE_STEP: i16 = 8;
pub const SLIDE_TOP: i16 = 64;

/// What the opening asks of the board camera this update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OpeningCamera {
    /// State 3: the polar written directly (0x1FC570).
    Set { target: [f64; 3], polar: [f64; 3] },
    /// 0x1FBAB0 kinds 3 and 4 over `frames` updates.
    Track { target: [f64; 3], polar: [f64; 3], frames: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Zoom,
    Rise,
    Fly,
    WaitFly,
    Emblem,
    Summon,
    Settle,
    Hold,
    SlideUp,
    SlideDown,
    Done,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DuelOpening {
    /// Updates stepped (u = update − 1).
    pub update: u32,
    step: Step,
    /// The leader cell of the side that moves first.
    leader: (u8, u8),
    yaw: f64,
    distance: f64,
    /// Every cell's height (cell +0x14).
    pub height: i16,
    /// d34: the emblems' scale (128 = full).
    pub emblem: u8,
    /// Both leaders' alpha (unit +0x53).
    pub leader_alpha: u8,
    fading: bool,
    c620: Option<u32>,
    /// d35.
    pub hold: u8,
    /// c9c.
    pub slide: i16,
    /// Updates the last camera track still runs.
    track: u32,
    /// Set on the update c620 is launched (states 0xA/0xB, u90).
    pub launch_c620: bool,
}

impl DuelOpening {
    /// The opening for a duel whose first side's leader stands on `leader`;
    /// `yaw` is the player's side's (0x4000 or 0xC000 units, 0x1AE89C).
    pub fn new(leader: (u8, u8), player_side: u8) -> Self {
        let yaw = if player_side == 0 { std::f64::consts::FRAC_PI_2 } else { 3.0 * std::f64::consts::FRAC_PI_2 };
        Self {
            update: 0,
            step: Step::Zoom,
            leader,
            yaw: f64::from(yaw as f32),
            distance: START_DISTANCE,
            height: HEIGHT_START,
            emblem: 0,
            leader_alpha: 0,
            fading: false,
            c620: None,
            hold: 0,
            slide: 0,
            track: 0,
            launch_c620: false,
        }
    }

    /// The board camera before the first update (the duel init's).
    pub fn start_camera(&self) -> ([f64; 3], [f64; 3]) {
        ([0.0; 3], [START_DISTANCE, self.yaw, f64::from(START_PITCH)])
    }

    pub fn done(&self) -> bool {
        self.step == Step::Done
    }

    /// The input state the original is in (for the overlay's colour sets).
    pub fn state(&self) -> u8 {
        match self.step {
            Step::Zoom => 3,
            Step::Rise => 5,
            Step::Fly | Step::WaitFly => 8,
            Step::Emblem => 9,
            Step::Summon => 0xC,
            Step::Settle => 0xD,
            Step::Hold => 0xE,
            Step::SlideUp => 0x10,
            Step::SlideDown => 0x12,
            Step::Done => 0x15,
        }
    }

    fn cell_target(&self, y: f64) -> [f64; 3] {
        [
            (f64::from(self.leader.0) - 3.0) * 50.0,
            y,
            (f64::from(self.leader.1) - 3.0) * 50.0,
        ]
    }

    /// One update: the mode update (0x1D00F0, falling through the states
    /// as the original does), then the camera tracks step (0x1FB430) and the
    /// leaders fade. Returns the camera request of this update.
    pub fn step(&mut self) -> Option<OpeningCamera> {
        if self.done() {
            return None;
        }
        self.update += 1;
        self.launch_c620 = false;
        let busy = self.track > 0;
        let mut camera = None;
        loop {
            match self.step {
                Step::Zoom => {
                    if self.distance < ZOOM_LIMIT {
                        self.distance += ZOOM_STEP;
                        camera = Some(OpeningCamera::Set {
                            target: [0.0; 3],
                            polar: [self.distance, self.yaw, f64::from(START_PITCH)],
                        });
                        break;
                    }
                    self.step = Step::Rise;
                }
                Step::Rise => {
                    if self.height >= 0 {
                        self.step = Step::Fly;
                        continue;
                    }
                    self.height = (self.height + HEIGHT_STEP).min(0);
                    break;
                }
                Step::Fly => {
                    camera = Some(OpeningCamera::Track {
                        target: self.cell_target(0.0),
                        polar: [FIRST_DISTANCE, self.yaw, f64::from(START_PITCH)],
                        frames: TRACK_FRAMES,
                    });
                    self.track = TRACK_FRAMES;
                    self.step = Step::WaitFly;
                    break;
                }
                Step::WaitFly => {
                    if busy {
                        break;
                    }
                    self.step = Step::Emblem;
                }
                Step::Emblem => {
                    if self.emblem < FULL {
                        self.emblem = (self.emblem + EMBLEM_STEP).min(FULL);
                        break;
                    }
                    let tau = std::f64::consts::TAU;
                    let yaw = f64::from(((self.yaw as f32) + std::f32::consts::PI + std::f32::consts::TAU) % tau as f32);
                    camera = Some(OpeningCamera::Track {
                        target: self.cell_target(LEADER_HEIGHT),
                        polar: [SECOND_DISTANCE, yaw, f64::from(f32::from_bits(SECOND_PITCH_BITS))],
                        frames: TRACK_FRAMES,
                    });
                    self.track = TRACK_FRAMES;
                    self.c620 = Some(0);
                    self.launch_c620 = true;
                    self.step = Step::Summon;
                    break;
                }
                Step::Summon => {
                    // c620 runs; on its status 2 the leader shows and the
                    // state ends on the next update.
                    match self.c620 {
                        Some(n) if n + 1 >= C620_SHOWS_AFTER => {
                            self.c620 = None;
                            self.fading = true;
                        }
                        Some(n) => self.c620 = Some(n + 1),
                        None => {
                            self.step = Step::Settle;
                            continue;
                        }
                    }
                    break;
                }
                Step::Settle => {
                    if busy || self.leader_alpha < FULL {
                        break;
                    }
                    self.hold = HOLD_UPDATES;
                    self.step = Step::Hold;
                }
                Step::Hold => {
                    if self.hold > 0 {
                        self.hold -= 1;
                        break;
                    }
                    self.step = Step::SlideUp;
                }
                Step::SlideUp => {
                    self.slide = (self.slide + SLIDE_STEP).min(SLIDE_TOP);
                    if self.slide >= SLIDE_TOP {
                        self.step = Step::SlideDown;
                    }
                    break;
                }
                Step::SlideDown => {
                    self.slide = (self.slide - SLIDE_STEP).max(0);
                    if self.slide <= 0 {
                        self.step = Step::Done;
                    }
                    break;
                }
                Step::Done => break,
            }
        }
        self.track = self.track.saturating_sub(1);
        if self.fading {
            self.leader_alpha = (self.leader_alpha + LEADER_FADE_STEP).min(FULL);
        }
        camera
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The PCSX2 log's milestones (u from the first zoom step).
    #[test]
    fn the_opening_follows_the_logged_duel_start() {
        let mut o = DuelOpening::new((3, 6), 0);
        let mut log = Vec::new();
        while !o.done() {
            let cam = o.step();
            log.push((o.update - 1, o.state(), o.height, o.emblem, o.leader_alpha, o.hold, o.slide, cam));
            assert!(o.update < 400);
        }
        let at = |u: u32| log[u as usize];
        // Zoom: 70 at u0, 610 at u27.
        assert_eq!(at(0).7, Some(OpeningCamera::Set { target: [0.0; 3], polar: [70.0, f64::from(std::f32::consts::FRAC_PI_2), f64::from(START_PITCH)] }));
        assert!(matches!(at(27).7, Some(OpeningCamera::Set { polar, .. }) if polar[0] == 610.0));
        // Rise: −120 at u28, 0 at u43.
        assert_eq!((at(28).2, at(42).2, at(43).2), (-120, -8, 0));
        // First flight at u44 to the leader cell (3,6) at distance 150.
        assert_eq!(at(44).7, Some(OpeningCamera::Track { target: [0.0, 0.0, 150.0], polar: [150.0, f64::from(std::f32::consts::FRAC_PI_2), f64::from(START_PITCH)], frames: 30 }));
        // Emblems: 8 at u74, 128 at u89.
        assert_eq!((at(73).3, at(74).3, at(89).3), (0, 8, 128));
        // Second flight and c620 at u90.
        match at(90).7 {
            Some(OpeningCamera::Track { target, polar, frames: 30 }) => {
                assert_eq!(target, [0.0, 40.0, 150.0]);
                assert!((polar[1] - 4.712_389).abs() < 1e-5 && polar[0] == 100.0 && (polar[2] - 0.589_05).abs() < 1e-4);
            }
            other => panic!("{other:?}"),
        }
        // Leaders: 8 at u129, 128 at u144.
        assert_eq!((at(128).4, at(129).4, at(144).4), (0, 8, 128));
        // Hold: 29 at u145 .. 0 at u174; slide 8 at u175, 64 at u182, 0 at u190.
        assert_eq!((at(145).5, at(146).5, at(174).5), (29, 28, 0));
        assert_eq!((at(175).6, at(182).6, at(183).6, at(190).6), (8, 64, 56, 0));
        assert_eq!(log.len(), 191);
        assert_eq!(at(190).1, 0x15);
    }

    #[test]
    fn a_white_rose_player_starts_from_the_other_end() {
        let o = DuelOpening::new((3, 0), 1);
        let (_, polar) = o.start_camera();
        assert!((polar[1] - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-6);
    }
}
