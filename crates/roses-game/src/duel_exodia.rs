//! The Exodia event (event 0x6D, handler 0x26DAA0) as the original plays
//! it when a side's turn starts with Exodia complete, before the result
//! (the duel ends with reason 2 once the event returns 0: 0x1D0EA0).
//! Bevy-free: the timeline of the handler as cues for the presentation;
//! the modules it launches run as original code elsewhere
//! (`duel_opening_fx::LeaderEffect` for c620, [`FinaleEffect`] for
//! c061..c063).
//!
//! TEMPORARY (Jim, 29 Oct 2026: the game is to stop running original
//! code): the handler 0x26DAA0 itself is hand-ported here ([`Exodia`],
//! plain Rust), but [`FinaleEffect`] (c061, c062, c063) and the c620s still
//! run the original CMODULE programs through the `roses_ee` interpreter.
//! They are to be hand-ported like the other `duel_c*.rs` modules, with
//! the interpreter kept only as the reference the ports are tested
//! against; then this runner goes.
//!
//! Sources: the PAL executable (EE addresses), `work/research/duel/
//! claude-exodia/FINDINGS.md` (private). Tags: [F] read in code, [emu]
//! checked against saved original states (the tutorial's scripted Exodia,
//! `claude-pine/session/182..187`), [H] hypothesis.
//!
//! - [F] The turn start 0x269570 phase 0x80 builds the record {0x6D, side,
//!   p2, 0} with p2 = slot(k0) << 9 | slot(k1) << 6 | slot(k2) << 3 |
//!   slot(k3), limb k = 0x26A010(k) = 0x38, 0x39, 0x36, 0x37 (0x269C38);
//!   code 1 (a scripted side: event only) or 5 (the win after the event).
//!   [emu] tutorial: {0x6D, 0, 0x463, 0}, C60 = 0xFF0001.
//! - [F] Init (a2 >= 0): camera snapshot 0x257280; music 0x217110(0x2B, 0);
//!   modules c061, c062, c063 and c620 requested (0x2573E0); unless
//!   scripted, Summoned Lord Exodia's model (list index 0x2E1 = card 0x29F)
//!   loaded into object 0x20 (0x1E99C0(0x20, -1, 0x2E1, 1)) at scale 0; the
//!   four limbs revealed (0x252FF0(side, slot, 1)); ctx+0 = 0x80; every unit
//!   of group (side == 0 ? 1 : 0), slots 1..10, revealed.
//! - [F] Two stage machines run every update, A (ctx+0x100) then B (ctx+4),
//!   sharing the counter ctx+0x102 (incremented first, compared before:
//!   a 25-wait is 26 updates):
//!   A0: wait 0x217150 idle; camera 0x262410(0, limb k3, 50, 180) (a cut);
//!       0x217110(0x2B, 1); the leader's card object made on its cell
//!       (0x1CED30, 0x1E2370) and hidden (card level target 0, 0x1E2BF0 /
//!       0x1E2DF0 / 0x1E2FF0); A1.
//!   A1..A3: after 25, once limb k2 / k1 / k0 has its model (0x1F70D0),
//!       the same cut to it.
//!   A4: after 25, once the leader has its model: 0x262410(0, leader, 200,
//!       180), blur 0x1A9010(6).
//!   A5: first update: SE 0x216EA0(0, 0x27), 0x262410(12, leader, 50, 180)
//!       (12 updates); from 12 blur 4; from 24 and the modules loaded
//!       (0x257470): blur 0, 0x2627D0(0, leader, 300, 180) (0x262410 with
//!       yaw base 0x3FFF and pitch 0x1000), B1, A6 (nothing).
//!   B1: c620 {0x26C, 0, cell(k0), 0}; B2..B4: after 25, c620 on k1, k2,
//!       k3; B5: after 25, the leader's card fades in (0x2519E0(side, 0, 1,
//!       1, 2)) and c620 on the leader's cell.
//!   B6: wait for every task to end. Scripted: board lights 0x2DF5B1 /
//!       0x2DF5B0 down by 4 per update, the event ends on the update both
//!       are 0 [emu: 44 at 185, 0 and the cursor state at 186]. Else: wait
//!       for object 0x20's model (0x1F2510), its effect textures as type 5
//!       (0x202150), camera tracks killed and the kind-8 track of its first
//!       CMOT (0x1F23C0(0x20, 0x13) = obj+0x16C, frames - 1 updates), then
//!       c061, c062, c063 {id, side, p2, 0}; B7.
//!   B7: objects 1, 5, 6's light +0x120 / +0x124 - 0.08 per update (floor
//!       0, 0x26E6B0(-1)); after 100, object 0x20's scale 1.0 (Summoned Lord
//!       Exodia shows); B8.
//!   B8: while a task returns 2 (0x1F64E0): the lights back to 0.8
//!       (0x26E6B0(1)), the four limbs off the board (0x1E0E30), object
//!       0x20 unloaded (0x1E90B0(0x20, -1, 999)), the leader's card object
//!       freed; when every task has ended the event returns 0.
//! - [F] Then 0x1D0EA0: card 0x29F marked seen in the winner's card list
//!   (0x216400(D54, 0x29F) with a2 = 1: record +0x49 bit 6), D50 = winner,
//!   D51 = 2, the result state 0x54.
//! - [H] The port's waits on the sound driver (0x217150), the disc
//!   (0x257470) and the model loads (0x1F70D0 / 0x1F2510) are 0 updates.
use roses_core::duel::{Duel, EXODIA_HEAD};

/// The event record's code.
pub const EVENT: u32 = 0x6D;
/// The limbs in the handler's order k = 0..3 (0x26A010).
pub const LIMB_ORDER: [u16; 4] = [0x38, 0x39, 0x36, 0x37];
/// The event's music (0x217110(0x2B, ..)).
pub const MUSIC_TRACK: u32 = 0x2B;
/// A5's sound effect 0x216EA0(0, 0x27).
pub const LEADER_SE: (u8, u8) = (0, 0x27);
/// Summoned Lord Exodia: list index 0x2E1 of 0x2CE024 is this card.
pub const LORD_CARD: u16 = 0x29F;
/// Object 0x20's place: (0, 0, +350) for side 0, (0, 0, -350) for side 1
/// (0x1F34C0), rotation y 0x8000 / 0 (0x1F3640) [F init of A0].
pub const LORD_Z: f32 = 350.0;
/// The object index the handler gives Summoned Lord Exodia.
pub const LORD_OBJECT: u8 = 0x20;
/// c620, the leaders' appearance effect, run on every limb and the leader.
pub const C620: u16 = 0x26C;
/// c061, c062, c063: the finale (non-scripted sides only).
pub const FINALE: [u16; 3] = [0x3D, 0x3E, 0x3F];
/// The waits between stages: the counter must have passed this value.
pub const STEP: u16 = 25;
/// B7's wait before Summoned Lord Exodia shows.
pub const LORD_SHOW: u16 = 100;
/// The board objects whose light 0x26E6B0 dims (1 = the arena's backdrop
/// ring, 5 / 6 = the windows) and its step / restored value.
pub const DIMMED_OBJECTS: [u8; 3] = [1, 5, 6];
pub const DIM_STEP: f32 = 0.08;
pub const DIM_RESTORED: f32 = 0.8;

/// A limb on the board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub card: u16,
    pub slot: u8,
    pub cell: (u8, u8),
}

/// What the presentation does this update, in the handler's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// 0x217110(track, mode): `keep` = mode 1.
    Music { track: u32, keep: bool },
    /// 0x216EA0(slot, id).
    Se { slot: u8, id: u8 },
    /// 0x262410 (`low` false) / 0x2627D0 (`low` true)(frames, side << 8 |
    /// slot, add, cell distance).
    Camera { frames: u32, side: u8, slot: u8, add: i32, cell: i32, low: bool },
    /// 0x252FF0(side, slot, 1): a reveal (an 8-update flip when face down).
    Reveal { side: u8, slot: u8 },
    /// The leader's card object made on its cell, hidden (A0).
    LeaderCard,
    /// 0x2519E0(side, 0, 1, 1, 2): the leader's card fades in (B5).
    LeaderCardFadeIn,
    /// 0x1A9010(n): the previous frame drawn over at alpha 16 n.
    Blur(u8),
    /// c620 on a cell (record {0x26C, 0, row << 8 | column, 0}).
    LaunchC620 { cell: (u8, u8) },
    /// B6 (0xd4 = 0): the task list starts again.
    ClearTasks,
    /// B6: object 0x20's textures, the camera tracks killed and its CMOT
    /// camera started (kind 8), then c061..c063 launched.
    LordCamera,
    LaunchFinale,
    /// 0x26E6B0(-1) / 0x26E6B0(1).
    DimBackdrop,
    RestoreBackdrop,
    /// Object 0x20 at scale 1.0.
    ShowLord,
    /// B8: the limbs leave the board, object 0x20 and the leader's card
    /// object go.
    RemovePieces,
    /// Scripted B6: the board lights (0x2DF5B1, 0x2DF5B0).
    BoardLight([u8; 2]),
    /// The event returned 0.
    End,
}

/// The original's view of the running tasks, in launch order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TaskState {
    /// 0x1F63E0(task) != 0.
    pub running: bool,
    /// 0x1F64E0(task): the module's last return.
    pub ret: i32,
}

/// The event in progress.
#[derive(Clone, Debug, PartialEq)]
pub struct Exodia {
    pub side: u8,
    /// A scripted controller (0x280890): the event only, no Summoned Lord
    /// Exodia, no win.
    pub scripted: bool,
    pub leader_cell: (u8, u8),
    /// Limbs in the order k = 0..3.
    pub limbs: [Piece; 4],
    /// The record's p2.
    pub packed: u16,
    stage_a: u8,
    stage_b: u16,
    counter: u16,
    /// The board lights of the scripted ending (128 = full).
    pub lights: [u8; 2],
    /// Objects 1 / 5 / 6's dimmed light (+0x124; 0.8 at rest).
    pub backdrop: f32,
    /// Summoned Lord Exodia is loaded (object 0x20) / shown at scale 1.
    pub lord: bool,
    pub lord_shown: bool,
    /// The limbs and the leader's card have left (B8).
    pub removed: bool,
    pub ended: bool,
    /// Updates polled since the init.
    pub updates: u32,
}

impl Exodia {
    /// The event for `side`'s turn start (`None` without a complete
    /// Exodia): the limbs as 0x269F20 finds them (the first matching unit
    /// of the side in the leader's eight neighbours).
    pub fn new(duel: &Duel, side: u8, scripted: bool) -> Option<Self> {
        let leader = duel.units.iter().find(|u| u.side == side && u.slot == 0)?;
        if leader.card != EXODIA_HEAD {
            return None;
        }
        let at = leader.position;
        let near = |u: &roses_core::duel::DuelUnit| {
            u.position.column.abs_diff(at.column) <= 1 && u.position.row.abs_diff(at.row) <= 1 && u.position != at
        };
        let mut limbs = [Piece { card: 0, slot: 0, cell: (0, 0) }; 4];
        for (k, card) in LIMB_ORDER.into_iter().enumerate() {
            let u = duel.units.iter().find(|u| u.side == side && u.card == card && near(u))?;
            limbs[k] = Piece { card, slot: u.slot, cell: (u.position.column, u.position.row) };
        }
        let packed = (u16::from(limbs[0].slot & 7) << 9)
            | (u16::from(limbs[1].slot & 7) << 6)
            | (u16::from(limbs[2].slot & 7) << 3)
            | u16::from(limbs[3].slot & 7);
        Some(Self {
            side,
            scripted,
            leader_cell: (at.column, at.row),
            limbs,
            packed,
            stage_a: 0,
            stage_b: 0,
            counter: 0,
            lights: [128, 128],
            backdrop: DIM_RESTORED,
            lord: false,
            lord_shown: false,
            removed: false,
            ended: false,
            updates: 0,
        })
    }

    /// The group whose units the init reveals: `side == 0 ? 1 : 0`
    /// (0x26DC18: a0 = 1, movn a0, zero, side).
    pub fn revealed_group(&self) -> u8 {
        u8::from(self.side == 0)
    }

    /// The init call (0x2569E0(rec, ctx, 0)): `others`: the slots 1..10
    /// holding a unit of [`Self::revealed_group`].
    pub fn init(&mut self, others: &[u8]) -> Vec<Cue> {
        let mut out = vec![Cue::Music { track: MUSIC_TRACK, keep: false }];
        if !self.scripted {
            self.lord = true;
        }
        // 0x26DBAC..0x26DC04: bits 0-2, 3-5, 6-8, 9-11 (k3, k2, k1, k0).
        for k in (0..4).rev() {
            out.push(Cue::Reveal { side: self.side, slot: self.limbs[k].slot });
        }
        let group = self.revealed_group();
        for &slot in others.iter().filter(|s| (1..=10).contains(*s)) {
            out.push(Cue::Reveal { side: group, slot });
        }
        out
    }

    fn camera(&self, frames: u32, slot: u8, add: i32, low: bool) -> Cue {
        Cue::Camera { frames, side: self.side, slot, add, cell: 180, low }
    }

    /// One poll (0x2569E0(rec, ctx, -1)). `tasks`: the state of every task
    /// launched since the last [`Cue::ClearTasks`], in launch order.
    pub fn step(&mut self, tasks: &[TaskState]) -> Vec<Cue> {
        let mut out = Vec::new();
        if self.ended {
            return out;
        }
        self.updates += 1;
        // Stage A (ctx+0x100, table 0x2D33E0).
        match self.stage_a {
            0 => {
                // [H] 0x217150 idle at once.
                out.push(self.camera(0, self.limbs[3].slot, 50, false));
                out.push(Cue::Music { track: MUSIC_TRACK, keep: true });
                out.push(Cue::LeaderCard);
                self.stage_a = 1;
            }
            1..=3 => {
                let old = self.counter;
                self.counter += 1;
                if old >= STEP {
                    // [H] the limb's model is there (0x1F70D0).
                    let k = 3 - usize::from(self.stage_a);
                    out.push(self.camera(0, self.limbs[k].slot, 50, false));
                    self.counter = 0;
                    self.stage_a += 1;
                }
            }
            4 => {
                let old = self.counter;
                self.counter += 1;
                if old >= STEP {
                    out.push(self.camera(0, 0, 200, false));
                    out.push(Cue::Blur(6));
                    self.counter = 0;
                    self.stage_a = 5;
                }
            }
            5 => {
                let old = self.counter;
                self.counter += 1;
                if old == 0 {
                    out.push(Cue::Se { slot: LEADER_SE.0, id: LEADER_SE.1 });
                    out.push(self.camera(12, 0, 50, false));
                    return out;
                }
                if self.counter >= 12 {
                    out.push(Cue::Blur(4));
                }
                // [H] the modules are loaded (0x257470).
                if self.counter >= 24 {
                    out.push(Cue::Blur(0));
                    out.push(self.camera(0, 0, 300, true));
                    self.stage_b += 1;
                    self.counter = 0;
                    self.stage_a = 6;
                }
            }
            _ => {}
        }
        // Stage B (ctx+4, table 0x2D33B0).
        match self.stage_b {
            1 => {
                out.push(Cue::LaunchC620 { cell: self.limbs[0].cell });
                self.counter = 0;
                self.stage_b = 2;
            }
            2..=4 => {
                let old = self.counter;
                self.counter += 1;
                if old >= STEP {
                    let k = usize::from(self.stage_b) - 1;
                    out.push(Cue::LaunchC620 { cell: self.limbs[k].cell });
                    self.counter = 0;
                    self.stage_b += 1;
                }
            }
            5 => {
                let old = self.counter;
                self.counter += 1;
                if old >= STEP {
                    out.push(Cue::LeaderCardFadeIn);
                    out.push(Cue::LaunchC620 { cell: self.leader_cell });
                    self.counter = 0;
                    self.stage_b = 6;
                }
            }
            6 => {
                if tasks.iter().any(|t| t.running) {
                    return out;
                }
                if self.scripted {
                    // 0x26E2E4..0x26E384: s1 counts the lights already at 0.
                    let mut zero = 0;
                    for l in &mut self.lights {
                        if *l > 0 {
                            *l = l.saturating_sub(4);
                        } else {
                            zero += 1;
                        }
                    }
                    out.push(Cue::BoardLight(self.lights));
                    if zero == 2 {
                        self.ended = true;
                        out.push(Cue::End);
                    }
                    return out;
                }
                out.push(Cue::ClearTasks);
                // [H] object 0x20's model is loaded (0x1F2510).
                out.push(Cue::LordCamera);
                out.push(Cue::LaunchFinale);
                self.stage_b = 7;
            }
            7 => {
                self.backdrop = (self.backdrop - DIM_STEP).max(0.0);
                out.push(Cue::DimBackdrop);
                let old = self.counter;
                self.counter += 1;
                if old > LORD_SHOW {
                    self.lord_shown = true;
                    out.push(Cue::ShowLord);
                    self.stage_b = 8;
                    self.counter = 0;
                }
            }
            8 => {
                let mut busy = 0;
                for t in tasks {
                    if !t.running {
                        continue;
                    }
                    if t.ret == 2 && !self.removed {
                        self.backdrop = DIM_RESTORED;
                        out.push(Cue::RestoreBackdrop);
                        out.push(Cue::RemovePieces);
                        self.removed = true;
                        self.lord = false;
                        self.lord_shown = false;
                    }
                    busy += 1;
                }
                if busy == 0 {
                    self.ended = true;
                    out.push(Cue::End);
                }
            }
            _ => {}
        }
        out
    }

    /// The counter and stages, for tests and logs.
    pub fn stages(&self) -> (u8, u16, u16) {
        (self.stage_a, self.stage_b, self.counter)
    }

    /// A limb's or the leader's cell by slot (`None`: not part of it).
    pub fn cell_of(&self, slot: u8) -> Option<(u8, u8)> {
        if slot == 0 {
            return Some(self.leader_cell);
        }
        self.limbs.iter().find(|l| l.slot == slot).map(|l| l.cell)
    }

    /// Object 0x20's world position and Y rotation (16-bit) for the side.
    pub fn lord_pose(&self) -> ([f32; 3], i32) {
        lord_pose(self.side)
    }
}

/// Object 0x20's world position and Y rotation for `side` [F A0's init:
/// z = 350 for side 0, -350 otherwise; rot y 0x8000 for side 0, else 0].
/// N-player seats 2 / 3 (a port addition) stand behind their own edge.
pub fn lord_pose(side: u8) -> ([f32; 3], i32) {
    match side {
        0 => ([0.0, 0.0, LORD_Z], 0x8000),
        1 => ([0.0, 0.0, -LORD_Z], 0),
        // E seat (column 6): behind x = +350, facing -x; W seat behind -350.
        2 => ([LORD_Z, 0.0, 0.0], 0xC000),
        _ => ([-LORD_Z, 0.0, 0.0], 0x4000),
    }
}

/// The yaw (16-bit) and pitch of 0x2627D0's 3D branch: 0x262CA0 adds
/// 0x3FFF - turn (0x13FFE - turn when negative) to the model's facing and
/// 0x1000 to its pitch (0x262410 / 0x262C00: 0x47FF, 0x2000) [F].
pub fn low_angles(base: i32, turn: i32) -> (i32, i32) {
    let k = 0x3FFF - turn;
    (base + if k < 0 { 0x13FFE - turn } else { k }, 0x1000)
}

/// The finale's camera: object 0x20's first CMOT (kind 8) at `elapsed`
/// of `frames - 1` updates, rotated by the object's yaw and placed at its
/// position (0x1FCB50, as the arena's unit cameras).
pub fn lord_camera(cmot: &crate::duel_arena::Cmot, elapsed: u32, side: u8) -> crate::duel_arena::CmotCamera {
    let duration = cmot.frames.saturating_sub(1).max(1);
    let c = cmot.at(elapsed.min(duration), duration);
    let (p, yaw) = lord_pose(side);
    let (s, co) = (f64::from(yaw) * std::f64::consts::PI / 32768.0).sin_cos();
    let place = |v: [f64; 3]| [co * v[0] + s * v[2] + f64::from(p[0]), v[1] + f64::from(p[1]), -s * v[0] + co * v[2] + f64::from(p[2])];
    crate::duel_arena::CmotCamera { eye: place(c.eye), target: place(c.target), roll: c.roll, distance: c.distance }
}

/// A private test (`ROSES_EXODIA_TEST=<side>`, not gameplay): `side`'s
/// leader becomes Exodia's head and the four limbs stand face up around it
/// (the free neighbour cells nearest the board's centre, the side's first
/// free monster slots), so its next turn start wins by Exodia.
pub fn rig_for_test(duel: &mut Duel, side: u8) -> bool {
    use roses_core::duel::{DuelUnit, EXODIA_LIMBS};
    let Some(leader) = duel.units.iter_mut().find(|u| u.side == side && u.slot == 0) else { return false };
    leader.card = EXODIA_HEAD;
    let at = leader.position;
    let mut cells: Vec<(u8, u8)> = Vec::new();
    for dc in -1i32..=1 {
        for dr in -1i32..=1 {
            let (c, r) = (i32::from(at.column) + dc, i32::from(at.row) + dr);
            if (dc, dr) == (0, 0) || !(0..7).contains(&c) || !(0..7).contains(&r) {
                continue;
            }
            let cell = (c as u8, r as u8);
            if duel.units.iter().any(|u| (u.position.column, u.position.row) == cell) {
                continue;
            }
            cells.push(cell);
        }
    }
    cells.sort_by_key(|&(c, r)| (i32::from(c) - 3).abs() + (i32::from(r) - 3).abs());
    let slots: Vec<u8> = (1..=5u8).filter(|s| !duel.units.iter().any(|u| u.side == side && u.slot == *s)).collect();
    if cells.len() < 4 || slots.len() < 4 {
        return false;
    }
    for (k, limb) in EXODIA_LIMBS.into_iter().enumerate() {
        duel.units.push(DuelUnit {
            id: roses_core::UnitId(0xF000 + u16::from(side) * 8 + k as u16),
            card: limb,
            side,
            slot: slots[k],
            position: roses_core::GridPos { column: cells[k].0, row: cells[k].1 },
            face_up: true,
            defence: false,
            committed_face_up: true,
            committed_defence: false,
            acted: false,
            spellbind: 0,
            summon_order: 0,
            attack_bonus: 0,
            defense_bonus: 0,
            spellbind_fresh: false,
            evo_counter: 0,
            stolen: false,
            discount: 0,
            stolen_by: None,
        });
    }
    true
}

/// Summoned Lord Exodia as object 0x20: its model at [`lord_pose`], the
/// motion clock, its CMOT camera and effect textures.
pub struct Lord {
    pub scene: crate::duel_models::UnitScene,
    /// The first chained CMOT (obj+0x16C), the finale's camera.
    pub cmot: Option<crate::duel_arena::Cmot>,
    /// The kind-8 track's elapsed updates once it runs.
    pub camera_elapsed: Option<u32>,
    /// obj+0x30: 0 until B7 shows it.
    pub scale: f32,
    /// The model's effect textures (type 5 for the finale's modules).
    pub textures: Vec<Option<crate::duel_models::EffectTexture>>,
}

impl Lord {
    /// Load card 0x29F from the models import and place it for `side`
    /// at scale 0 (0x1F37E0(0x20, (0, 0, 0)) in the init).
    pub fn load(manifest: &std::path::Path, side: u8) -> crate::Result<Self> {
        let loaded = crate::duel_models::load_model(manifest, LORD_CARD)?;
        let mut scene = crate::duel_models::UnitScene::new(loaded, 0, (3, 3), side.min(1), 1.0)?;
        let (position, yaw) = lord_pose(side);
        scene.set_pose(position, yaw);
        scene.set_scale(0.0);
        let cmot = crate::duel_models::unit_cameras(manifest, LORD_CARD)?
            .0
            .map(|b| crate::duel_arena::Cmot::parse(&b))
            .transpose()?;
        let textures = crate::duel_models::effect_textures(manifest, LORD_CARD).unwrap_or_default();
        Ok(Self { scene, cmot, camera_elapsed: None, scale: 0.0, textures })
    }

    /// The model's (motion, frame count) list.
    pub fn motions(&self) -> Vec<(u8, u32)> {
        self.scene.loaded.model.motions.iter().map(|m| (m.motion, m.length)).collect()
    }

    /// One update of the motion clock (and the camera track).
    pub fn tick(&mut self) {
        self.scene.tick();
        if let Some(e) = &mut self.camera_elapsed {
            *e += 1;
        }
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
        self.scene.set_scale(scale);
    }

    /// A module's ChangeMotion (0x1F2770): the new motion from its time
    /// with the 6-update blend from the old one [F 0x1F2770; the blend's
    /// weights as the arena's H].
    pub fn change_motion(&mut self, motion: u8) {
        if motion == self.scene.motion {
            return;
        }
        let (old, time) = (self.scene.motion, self.scene.clock.time);
        if self.scene.set_motion(motion) {
            self.scene.blend = Some((old, time, 1.0 / 6.0));
        }
    }

    /// The camera this update (when the track runs).
    pub fn camera(&self, side: u8) -> Option<crate::duel_arena::CmotCamera> {
        Some(lord_camera(self.cmot.as_ref()?, self.camera_elapsed?, side))
    }

    /// Every skeleton node's world position at the current time (the node
    /// results obj+0xA0 + 16 slot the finale's 0x1F40D0 reads).
    pub fn nodes(&self) -> Vec<[f32; 3]> {
        use roses_client::monster_pose;
        let s = &self.scene;
        let model = &s.loaded.model;
        let world = monster_pose::object_matrix(model, &s.object, &s.loaded.sine);
        let Ok(pose) = monster_pose::evaluate_pose_blended(model, model.motion(s.motion), s.clock.time, None, &s.loaded.sine) else {
            return Vec::new();
        };
        pose.nodes
            .iter()
            .map(|n| {
                let p = monster_pose::apply(&monster_pose::mul(&world, n), [0.0, 0.0, 0.0, 1.0]);
                [p[0] as f32, p[1] as f32, p[2] as f32]
            })
            .collect()
    }

    /// Object 0x20 as the finale's modules read it.
    pub fn mirror(&self) -> roses_ee::bmodule::UnitMirror {
        let s = &self.scene;
        let mut frames = [0u32; 8];
        for (m, len) in self.motions() {
            if let Some(f) = frames.get_mut(usize::from(m)) {
                *f = len;
            }
        }
        let b = s.loaded.model.nodes.first().map_or([0.0; 4], |n| n.bounding_sphere);
        roses_ee::bmodule::UnitMirror {
            position: s.object.position,
            rotation: [0, s.object.rotation[1], 0],
            animated: s.object.position,
            motion: s.motion,
            time: s.clock.time,
            speed: 1.0,
            play_mode: 1,
            frozen: false,
            frames,
            alpha: 0x80,
            nodes: s.loaded.model.nodes.len().min(255) as u8,
            model: LORD_CARD,
            bounds: b,
        }
    }
}

/// What the finale's modules read each update: the board camera and
/// Summoned Lord Exodia (role 0, object 0x20).
pub struct LordWorld {
    pub camera: roses_ee::bmodule::CameraMirror,
    pub lord: roses_ee::bmodule::UnitMirror,
    pub nodes: Vec<[f32; 3]>,
}

impl roses_ee::bmodule::EffectWorld for LordWorld {
    fn camera(&self) -> roses_ee::bmodule::CameraMirror {
        self.camera
    }
    fn unit(&self, role: usize) -> roses_ee::bmodule::UnitMirror {
        if role == 0 {
            self.lord.clone()
        } else {
            roses_ee::bmodule::UnitMirror { alpha: 0x80, speed: 1.0, ..self.lord.clone() }
        }
    }
    fn node(&self, role: usize, node: u8) -> Option<[f32; 3]> {
        (role == 0).then(|| self.nodes.get(usize::from(node)).copied()).flatten()
    }
}

/// c061, c062 and c063 running as original code (one interpreter each:
/// in the original they share memory; here object 0x20 is mirrored into
/// each every update [H: state one module leaves for another is not
/// shared]). The unit records and card objects of the board are not
/// mirrored (c061 / c062 read them; reported).
pub struct FinaleEffect {
    effects: Vec<std::sync::Mutex<roses_ee::bmodule::AttackEffect>>,
    /// Each module's state as the handler polls it (launch order).
    pub tasks: Vec<TaskState>,
    /// The last update's primitives by module.
    pub prims: Vec<std::sync::Arc<Vec<roses_ee::bmodule::GsPrim>>>,
    /// The stage light 0x2DF5B2 c063 writes (0x80 = full).
    pub stage_light: u8,
    /// A motion a module gave object 0x20 this update.
    pub motion: Option<u8>,
    pub updates: u32,
}

impl FinaleEffect {
    /// Launch the three (init and the first update, as c620's launch).
    pub fn launch(executable: &[u8], cmodule: &[u8], texeff: Option<&[u8]>, lord: &Lord, side: u8, packed: u16, world: &LordWorld) -> Self {
        let textures = roses_ee::bmodule::EffectTextures {
            texeff: texeff.map(crate::duel_bmodule::texeff_records).unwrap_or_default(),
            own: lord.textures.iter().map(|t| t.as_ref().map(|t| t.rh2.clone())).collect(),
            other: Vec::new(),
        };
        let motions = lord.motions();
        let mut effects = Vec::new();
        for id in FINALE {
            match roses_ee::bmodule::AttackEffect::load_cmodule(executable, cmodule, id, [u16::from(side), packed], &textures) {
                Ok(Some(mut fx)) => {
                    fx.place_role(0, LORD_OBJECT, &motions);
                    effects.push(std::sync::Mutex::new(fx));
                }
                Ok(None) => eprintln!("Exodia: CMODULE program {id:#x} is not in the import"),
                Err(e) => eprintln!("Exodia: CMODULE program {id:#x} cannot load: {}", e.0),
            }
        }
        let n = effects.len();
        let mut fx = Self {
            effects,
            tasks: vec![TaskState { running: true, ret: 0 }; n],
            prims: vec![std::sync::Arc::new(Vec::new()); n],
            stage_light: 0x80,
            motion: None,
            updates: 0,
        };
        // The init call, then the launch update's first update.
        for e in &mut fx.effects {
            if let Ok(e) = e.get_mut() {
                let _ = e.update(world);
            }
        }
        fx.update(world);
        fx
    }

    /// One update of every module still running.
    pub fn update(&mut self, world: &LordWorld) {
        self.motion = None;
        self.updates += 1;
        for (i, e) in self.effects.iter_mut().enumerate() {
            let Ok(e) = e.get_mut() else { continue };
            if e.finished {
                self.tasks[i].running = false;
                self.prims[i] = std::sync::Arc::new(Vec::new());
                continue;
            }
            match e.update(world) {
                Ok(u) => {
                    self.tasks[i] = TaskState { running: !e.finished, ret: u.ret };
                    if i == 2 {
                        self.stage_light = u.stage_light;
                    }
                    self.prims[i] = std::sync::Arc::new(u.prims);
                    let (motion, _, _) = e.object_motion(0);
                    if motion != world.lord.motion {
                        self.motion = Some(motion);
                    }
                }
                Err(err) => {
                    eprintln!("Exodia: CMODULE program {:#x} stopped: {}", FINALE[i], err.0);
                    e.finished = true;
                    self.tasks[i].running = false;
                    self.prims[i] = std::sync::Arc::new(Vec::new());
                }
            }
        }
    }

    pub fn finished(&self) -> bool {
        self.tasks.iter().all(|t| !t.running)
    }
}

/// The event's presentation in a duel (`DuelPlay::exodia`): the handler's
/// cues applied to the port's board, the modules it launches.
pub struct ExodiaRun {
    pub event: Exodia,
    /// c620s launched since the last [`Cue::ClearTasks`].
    pub c620: Vec<crate::duel_opening_fx::LeaderEffect>,
    pub finale: Option<FinaleEffect>,
    pub lord: Option<Lord>,
    /// The leader's card object's level (+0x52; `None`: none made yet).
    pub leader_card: Option<u8>,
    leader_fading: bool,
    /// Units revealed and the updates since (the 8-update flip of a face-
    /// down card).
    pub reveals: Vec<((u8, u8), u8)>,
    /// 0x1A9010's alpha (16 n).
    pub blur: u8,
    /// The board camera this update (set by `drive_camera`).
    pub view: Option<([f64; 3], [f64; 3])>,
    /// A unit camera to start ([`Cue::Camera`]).
    pub camera: Option<Cue>,
    pub done: bool,
    reported: bool,
    /// Summoned Lord Exodia's effect texture images (type 5 of the
    /// finale's primitives).
    pub own: std::sync::Arc<Vec<Option<roses_assets::RgbaImage>>>,
}

fn mirror_of(c: crate::duel_arena::CmotCamera) -> roses_ee::bmodule::CameraMirror {
    roses_ee::bmodule::CameraMirror { eye: c.eye.map(|v| v as f32), target: c.target.map(|v| v as f32), distance: c.distance as f32, roll: c.roll as f32 }
}

impl ExodiaRun {
    /// The init call in the update the turn start reaches phase 0x80.
    pub fn start(play: &mut crate::duel_play::DuelPlay, side: u8, scripted: bool) -> Option<Box<Self>> {
        let duel = &play.session.duel;
        let mut event = Exodia::new(duel, side, scripted)?;
        let group = event.revealed_group();
        // N-player (a port addition): every other player's units.
        let others: Vec<(u8, u8)> = duel
            .units
            .iter()
            .filter(|u| u.slot != 0 && if duel.side_count() > 2 { u.side != side } else { u.side == group })
            .map(|u| (u.side, u.slot))
            .collect();
        let slots: Vec<u8> = others.iter().filter(|o| o.0 == group).map(|o| o.1).collect();
        let cues = event.init(&slots);
        let lord = if event.lord {
            let manifest = play.arena_provider.as_ref().map(|p| p.models_manifest.clone());
            match manifest.map(|m| Lord::load(&m, side)) {
                Some(Ok(l)) => Some(l),
                Some(Err(e)) => {
                    eprintln!("Exodia: Summoned Lord Exodia (card 0x29F) is not drawn: {e}");
                    None
                }
                None => {
                    eprintln!("Exodia: Summoned Lord Exodia is not drawn: no models import");
                    None
                }
            }
        } else {
            None
        };
        let mut run = Box::new(Self {
            event,
            c620: Vec::new(),
            finale: None,
            lord,
            leader_card: None,
            leader_fading: false,
            reveals: Vec::new(),
            blur: 0,
            view: None,
            camera: None,
            done: false,
            reported: false,
            own: std::sync::Arc::new(Vec::new()),
        });
        if let Some(l) = &run.lord {
            run.own = std::sync::Arc::new(l.textures.iter().map(|t| t.as_ref().and_then(|t| t.image.clone())).collect());
        }
        for (s, slot) in others.into_iter().filter(|o| o.0 != group) {
            run.reveals.push(((s, slot), 0));
        }
        run.apply(play, &cues);
        eprintln!(
            "Exodia: side {side}'s event 0x6D starts at duel update {} (p2 {:#05x}{})",
            play.frames,
            run.event.packed,
            if scripted { ", scripted: no win" } else { "" }
        );
        Some(run)
    }

    fn board_camera(&self) -> roses_ee::bmodule::CameraMirror {
        let (eye, target) = self.view.unwrap_or(([0.0, 300.0, 300.0], [0.0; 3]));
        crate::duel_opening_fx::board_camera(eye, target)
    }

    /// The camera the effects see this update: the Lord's CMOT once it
    /// runs, else the board's.
    pub fn effect_camera(&self) -> roses_ee::bmodule::CameraMirror {
        match self.lord.as_ref().and_then(|l| l.camera(self.event.side)) {
            Some(c) => mirror_of(c),
            None => self.board_camera(),
        }
    }

    fn apply(&mut self, play: &mut crate::duel_play::DuelPlay, cues: &[Cue]) {
        use crate::duel_audio::AudioCue;
        for &cue in cues {
            match cue {
                Cue::Music { track, keep } => play.audio.push(AudioCue::Music { track, keep }),
                Cue::Se { slot, id } => play.audio.push(AudioCue::Se { slot, id, skip: false }),
                Cue::Camera { .. } => self.camera = Some(cue),
                Cue::Reveal { side, slot } => {
                    if !self.reveals.iter().any(|r| r.0 == (side, slot)) {
                        self.reveals.push(((side, slot), 0));
                    }
                }
                Cue::LeaderCard => self.leader_card = Some(0),
                Cue::LeaderCardFadeIn => self.leader_fading = true,
                Cue::Blur(n) => self.blur = 16 * n,
                Cue::LaunchC620 { cell } => {
                    let program = play.arena_provider.as_ref().and_then(|p| p.cmodule.clone());
                    let view = self.board_camera();
                    match program.and_then(|c| crate::duel_opening_fx::LeaderEffect::launch(&play.data.executable, &c, play.data.texeff.as_deref(), cell, view)) {
                        Some(fx) => self.c620.push(fx),
                        None if !self.reported => {
                            self.reported = true;
                            eprintln!("Exodia: c620 is not drawn (the import has no CMODULE.MRG)");
                        }
                        None => {}
                    }
                }
                Cue::ClearTasks => self.c620.clear(),
                Cue::LordCamera => {
                    if let Some(l) = &mut self.lord {
                        if l.cmot.is_some() {
                            l.camera_elapsed = Some(0);
                        }
                    }
                }
                Cue::LaunchFinale => {
                    let program = play.arena_provider.as_ref().and_then(|p| p.cmodule.clone());
                    let side = self.event.side;
                    let packed = self.event.packed;
                    let fallback = self.board_camera();
                    if let (Some(cmodule), Some(lord)) = (program, self.lord.as_mut()) {
                        // The kind-8 track's first step runs before this
                        // update's render pass.
                        lord.tick();
                        let camera = lord.camera(side).map_or(fallback, mirror_of);
                        let world = LordWorld { camera, lord: lord.mirror(), nodes: lord.nodes() };
                        self.finale = Some(FinaleEffect::launch(&play.data.executable, &cmodule, play.data.texeff.as_deref(), lord, side, packed, &world));
                        eprintln!("Exodia: unsupported, not mirrored: the board's unit records and card objects the finale's c061 / c062 read (what they do to the cards is not drawn); the stage light 0x2DF5B2 c063 lowers; the previous-frame sprites (c062); the modules' sounds (SECTL schedule)");
                    } else {
                        eprintln!("Exodia: the finale c061..c063 is not drawn (no CMODULE.MRG or no Summoned Lord Exodia)");
                    }
                }
                Cue::DimBackdrop | Cue::RestoreBackdrop | Cue::BoardLight(_) => {}
                Cue::ShowLord => {
                    if let Some(l) = &mut self.lord {
                        l.set_scale(1.0);
                    }
                }
                Cue::RemovePieces => self.lord = None,
                Cue::End => {
                    self.done = true;
                    eprintln!("Exodia: the event ends after {} updates (duel update {}); the result follows", self.event.updates, play.frames);
                }
            }
        }
    }

    /// One update: the handler's poll with the tasks as the last render
    /// pass left them, then the modules' updates (the render pass).
    pub fn tick(&mut self, play: &mut crate::duel_play::DuelPlay) {
        let tasks: Vec<TaskState> = match &self.finale {
            Some(f) => f.tasks.clone(),
            None => self.c620.iter().map(|fx| TaskState { running: !fx.finished(), ret: fx.ret }).collect(),
        };
        let cues = self.event.step(&tasks);
        let launched = cues.contains(&Cue::LaunchFinale);
        self.apply(play, &cues);
        for r in &mut self.reveals {
            r.1 = r.1.saturating_add(1);
        }
        if self.leader_fading {
            if let Some(l) = &mut self.leader_card {
                *l = (*l + 8).min(128);
            }
        }
        if self.done {
            return;
        }
        if !launched {
            if let Some(l) = &mut self.lord {
                l.tick();
            }
        }
        let view = self.board_camera();
        for fx in &mut self.c620 {
            if !fx.finished() {
                fx.update(view);
            }
        }
        if launched {
            return;
        }
        let side = self.event.side;
        if let Some(f) = &mut self.finale {
            let world = match &self.lord {
                Some(lord) => LordWorld { camera: lord.camera(side).map_or(view, mirror_of), lord: lord.mirror(), nodes: lord.nodes() },
                // The pieces have gone: the modules run on to their end.
                None => LordWorld {
                    camera: view,
                    lord: roses_ee::bmodule::UnitMirror {
                        position: [0.0; 3],
                        rotation: [0; 3],
                        animated: [0.0; 3],
                        motion: 0,
                        time: 0.0,
                        speed: 1.0,
                        play_mode: 0,
                        frozen: false,
                        frames: [0; 8],
                        alpha: 0x80,
                        nodes: 0,
                        model: LORD_CARD,
                        bounds: [0.0; 4],
                    },
                    nodes: Vec::new(),
                },
            };
            f.update(&world);
            if let (Some(m), Some(lord)) = (f.motion, &mut self.lord) {
                lord.change_motion(m);
            }
        }
    }

    /// Every primitive list the event's modules drew this update (c620s,
    /// then the finale's c061, c062, c063).
    pub fn prims(&self) -> Vec<std::sync::Arc<Vec<roses_ee::bmodule::GsPrim>>> {
        let mut out: Vec<_> = self.c620.iter().map(|fx| fx.prims.clone()).collect();
        if let Some(f) = &self.finale {
            out.extend(f.prims.iter().cloned());
        }
        out
    }

    /// The board light: the scripted ending's 0x2DF5B0/1. (The finale's
    /// stage light 0x2DF5B2, which c063 lowers to 48, lights objects this
    /// port does not separate from the frame; not applied, reported.)
    pub fn light(&self) -> u8 {
        self.event.lights[0].min(self.event.lights[1])
    }

    /// A revealed unit's flip (+0x54: 0x20 per update to 0xFF; `None` once
    /// done) [F/emu FLIP-AND-VICTIMS.md 1].
    pub fn reveal(&self, side: u8, slot: u8) -> Option<Option<u8>> {
        let r = self.reveals.iter().find(|r| r.0 == (side, slot))?;
        let p = u32::from(r.1) * 0x20;
        Some((p < 0xFF).then_some(p as u8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roses_core::duel::{DuelUnit, EXODIA_LIMBS};
    use roses_core::{GridPos, UnitId};

    fn unit(id: u16, card: u16, side: u8, slot: u8, column: u8, row: u8) -> DuelUnit {
        DuelUnit {
            id: UnitId(id),
            card,
            side,
            slot,
            position: GridPos { column, row },
            face_up: true,
            defence: false,
            committed_face_up: true,
            committed_defence: false,
            acted: false,
            spellbind: 0,
            summon_order: 0,
            attack_bonus: 0,
            defense_bonus: 0,
            spellbind_fresh: false,
            evo_counter: 0,
            stolen: false,
            discount: 0,
            stolen_by: None,
        }
    }

    /// The tutorial's board (lesson 2): the limbs 57, 56, 55, 54 at
    /// (2,6), (4,6), (2,5), (4,5) around the leader at (3,6) [O 172]; the
    /// record's p2 was 0x463 in the original [emu 183].
    fn tutorial_duel() -> Duel {
        let mut units = vec![unit(1, EXODIA_HEAD, 0, 0, 3, 6), unit(2, 0x12, 1, 0, 3, 0)];
        // Slots as the original's lesson gave them: 0x38 in 2, 0x39 in 1,
        // 0x36 in 4, 0x37 in 3 (p2 = 2 << 9 | 1 << 6 | 4 << 3 | 3).
        for (id, card, slot, c, r) in [(3u16, 0x39u16, 1u8, 2u8, 6u8), (4, 0x38, 2, 4, 6), (5, 0x37, 3, 2, 5), (6, 0x36, 4, 4, 5)] {
            units.push(unit(id, card, 0, slot, c, r));
        }
        assert!(EXODIA_LIMBS.iter().all(|l| units.iter().any(|u| u.card == *l)));
        let mut d = roses_core::duel::Duel::new(
            roses_core::duel::DuelSetup {
                terrain: vec![0; 49],
                leaders: [EXODIA_HEAD, 0x12],
                decks: [vec![], vec![]],
                first: 0,
                leader_abilities: [vec![], vec![]],
                controllers: [100, 100],
            },
            &test_cards(),
        )
        .unwrap()
        .0;
        d.units = units;
        d
    }

    fn test_cards() -> Vec<roses_core::duel::CardInfo> {
        (0..0x40u16)
            .map(|id| roses_core::duel::CardInfo {
                id,
                kind: 0,
                level: 1,
                attribute: 0,
                attack: 100,
                defense: 100,
                effects: [false; 5],
                toon: false,
                kind_index: 0,
                entries: [None; 5],
                equip_bits: [0; 7],
                power_up: None,
            })
            .collect()
    }

    fn run(e: &mut Exodia, finale_two_at: Option<u32>, finale_len: u32) -> Vec<(u32, Cue)> {
        // The tasks: c620 runs 72 updates (ret 2 on its 39th) [emu c620
        // run1]; the finale's three modules `finale_len`.
        let mut tasks: Vec<(u32, u32, Option<u32>)> = Vec::new(); // (launched, length, ret-2 update)
        let mut log = Vec::new();
        let mut n = 0u32;
        while !e.ended && n < 5000 {
            n += 1;
            // A task's update k runs in the render pass of update launch +
            // k - 1 (the launch runs its first): the handler sees it the
            // update after.
            let state: Vec<TaskState> = tasks
                .iter()
                .map(|&(at, len, two)| {
                    let age = n - at;
                    TaskState { running: age < len, ret: if two.is_some_and(|t| age >= t) { 2 } else { 0 } }
                })
                .collect();
            for cue in e.step(&state) {
                match cue {
                    Cue::LaunchC620 { .. } => tasks.push((n, 72, Some(39))),
                    Cue::ClearTasks => tasks.clear(),
                    Cue::LaunchFinale => {
                        for _ in 0..3 {
                            tasks.push((n, finale_len, finale_two_at));
                        }
                    }
                    _ => {}
                }
                log.push((n, cue));
            }
        }
        log
    }

    #[test]
    fn the_record_packs_the_limb_slots_like_the_tutorial() {
        let d = tutorial_duel();
        let e = Exodia::new(&d, 0, true).unwrap();
        assert_eq!(e.packed, (2 << 9) | (1 << 6) | (4 << 3) | 3);
        assert_eq!(e.limbs.map(|l| l.card), LIMB_ORDER);
        assert_eq!(e.revealed_group(), 1);
        assert!(Exodia::new(&d, 1, false).is_none(), "side 1's leader is not Exodia");
    }

    /// The handler's timeline: A0 cuts to limb k3 on the first poll, a
    /// limb every 26 updates (k2, k1, k0), the leader at 105, its close-up
    /// at 106 with the SE, the low camera at 129 with c620 on k0, then
    /// c620 every 26 updates and on the leader at 233 with the card's
    /// fade-in; the scripted side's lights fall 4 per update from the
    /// last c620's end and the event ends on the update both are 0.
    #[test]
    fn the_scripted_timeline_follows_the_handler() {
        let d = tutorial_duel();
        let mut e = Exodia::new(&d, 0, true).unwrap();
        let init = e.init(&[]);
        assert_eq!(init[0], Cue::Music { track: 0x2B, keep: false });
        assert_eq!(init[1], Cue::Reveal { side: 0, slot: 3 });
        let log = run(&mut e, None, 0);
        let at = |want: &dyn Fn(&Cue) -> bool| log.iter().filter(|(_, c)| want(c)).map(|(n, _)| *n).collect::<Vec<_>>();
        let cams = at(&|c| matches!(c, Cue::Camera { .. }));
        assert_eq!(cams, vec![1, 27, 53, 79, 105, 106, 129]);
        assert_eq!(log.iter().find(|(n, c)| *n == 1 && matches!(c, Cue::Camera { .. })).unwrap().1, Cue::Camera { frames: 0, side: 0, slot: 3, add: 50, cell: 180, low: false });
        assert_eq!(at(&|c| matches!(c, Cue::Se { id: 0x27, .. })), vec![106]);
        let mut blur: Vec<u32> = vec![105];
        blur.extend(117..=129);
        blur.push(129);
        assert_eq!(at(&|c| matches!(c, Cue::Blur(_))), blur);
        let c620 = at(&|c| matches!(c, Cue::LaunchC620 { .. }));
        assert_eq!(c620, vec![129, 155, 181, 207, 233]);
        assert_eq!(at(&|c| matches!(c, Cue::LeaderCardFadeIn)), vec![233]);
        // The leader's c620 ends 72 updates after 233; 32 steps of 4 from
        // 128, the end on the 33rd.
        let lights = at(&|c| matches!(c, Cue::BoardLight(_)));
        assert_eq!(lights.first(), Some(&(233 + 72)));
        assert_eq!(at(&|c| matches!(c, Cue::End)), vec![233 + 72 + 32]);
        assert!(at(&|c| matches!(c, Cue::LaunchFinale | Cue::ShowLord)).is_empty(), "no finale for a scripted side");
    }

    /// [emu] The original's saved states agree: 183 (B2, counter 24) to 184
    /// (B6) is 87 updates (here 26 + 26 + 26 + 2 = 80 to B6 at least); the
    /// lights read 44 at 185, 21 decrements after the last c620 ended.
    #[test]
    fn the_scripted_lights_match_the_saved_states() {
        let d = tutorial_duel();
        let mut e = Exodia::new(&d, 0, true).unwrap();
        e.init(&[]);
        let log = run(&mut e, None, 0);
        let first = log.iter().find(|(_, c)| matches!(c, Cue::BoardLight(_))).unwrap().0;
        let after = log.iter().find(|(n, c)| *n == first + 20 && matches!(c, Cue::BoardLight(_))).unwrap().1;
        assert_eq!(after, Cue::BoardLight([44, 44]));
    }

    /// A real side: after the last c620, c061..c063 with Summoned Lord
    /// Exodia's camera; the backdrop dims for 10 updates; the Lord shows
    /// after 101; a module's return 2 removes the pieces; the event ends
    /// when the three have ended.
    #[test]
    fn the_finale_shows_the_lord_and_ends_with_the_modules() {
        let d = tutorial_duel();
        let mut e = Exodia::new(&d, 0, false).unwrap();
        e.init(&[1, 2]);
        assert!(e.lord);
        let log = run(&mut e, Some(870), 954);
        let at = |want: &dyn Fn(&Cue) -> bool| log.iter().filter(|(_, c)| want(c)).map(|(n, _)| *n).collect::<Vec<_>>();
        let finale = at(&|c| matches!(c, Cue::LaunchFinale))[0];
        assert_eq!(finale, 233 + 72);
        assert_eq!(at(&|c| matches!(c, Cue::LordCamera)), vec![finale]);
        assert_eq!(at(&|c| matches!(c, Cue::ShowLord)), vec![finale + 102]);
        assert_eq!(at(&|c| matches!(c, Cue::DimBackdrop)).len(), 102);
        assert_eq!(at(&|c| matches!(c, Cue::RemovePieces)), vec![finale + 870]);
        assert_eq!(at(&|c| matches!(c, Cue::End)), vec![finale + 954]);
        assert_eq!(e.backdrop, DIM_RESTORED);
        assert!(!e.lord && e.removed);
    }

    #[test]
    fn the_low_camera_turns_a_quarter_from_the_close_up() {
        assert_eq!(low_angles(0x8000, 0), (0xBFFF, 0x1000));
        assert_eq!(low_angles(0, 0x5000), (0x13FFE - 0x5000, 0x1000));
        assert_eq!(lord_pose(0), ([0.0, 0.0, 350.0], 0x8000));
        assert_eq!(lord_pose(1), ([0.0, 0.0, -350.0], 0));
    }
}

#[cfg(test)]
mod private_tests {
    use super::*;

    fn game() -> Option<std::path::PathBuf> {
        std::env::var_os("ROSES_TEST_GAME_DIR").map(std::path::PathBuf::from)
    }

    /// Summoned Lord Exodia's model, CMOT camera and the finale's modules
    /// run headless with the Lord mirrored [run, exploration]: what they
    /// return and draw per update.
    #[test]
    #[ignore = "requires ROSES_TEST_GAME_DIR (a game import)"]
    fn private_the_finale_runs_with_the_lord_mirrored() {
        let dir = game().expect("ROSES_TEST_GAME_DIR");
        let manifest = dir.join("duel-models/manifest.json");
        let mut lord = Lord::load(&manifest, 0).expect("card 0x29F");
        eprintln!("motions {:?}", lord.motions());
        eprintln!("nodes {}, radius {:?}", lord.scene.loaded.model.nodes.len(), lord.scene.loaded.model.nodes.first().map(|n| n.bounding_sphere));
        let cmot = lord.cmot.clone().expect("a CMOT");
        eprintln!("CMOT frames {} eye {:?} target {:?} fov {:?}", cmot.frames, cmot.eye.first(), cmot.target.first(), cmot.fov);
        eprintln!("effect textures {:?}", lord.textures.iter().map(|t| t.is_some()).collect::<Vec<_>>());
        for e in [1u32, 14, 54, 100, 184, 290, 400, 560, 700, 900] {
            let c = lord_camera(&cmot, e, 1);
            eprintln!("camera side 1 at {e}: eye {:?} target {:?} distance {:.1} roll {:.3}", c.eye, c.target, c.distance, c.roll);
        }
        let exe = std::fs::read(dir.join("duel/SLES_515.79")).unwrap();
        let cmodule = std::fs::read(dir.join("duel-arena/CMODULE.MRG")).unwrap();
        let texeff = std::fs::read(dir.join("duel/TEXEFF.MRG")).unwrap();
        lord.camera_elapsed = Some(0);
        let world = |lord: &Lord| {
            let c = lord.camera(0).unwrap();
            LordWorld {
                camera: roses_ee::bmodule::CameraMirror { eye: c.eye.map(|v| v as f32), target: c.target.map(|v| v as f32), distance: c.distance as f32, roll: c.roll as f32 },
                lord: lord.mirror(),
                nodes: lord.nodes(),
            }
        };
        let w = world(&lord);
        let started = std::time::Instant::now();
        let mut fx = FinaleEffect::launch(&exe, &cmodule, Some(&texeff), &lord, 0, 0x463, &w);
        let mut last = String::new();
        let mut n = 1;
        while !fx.finished() && n < 3000 {
            if n == 102 {
                lord.set_scale(1.0);
            }
            lord.tick();
            let w = world(&lord);
            fx.update(&w);
            if let Some(m) = fx.motion {
                eprintln!("u{n}: object 0x20 motion -> {m}");
                lord.change_motion(m);
            }
            if n == 2 || n == 200 {
                for (m, prims) in fx.prims.iter().enumerate() {
                    for p in prims.iter().take(3) {
                        eprintln!("u{n} module {m}: kind {} flags {:#x} tex {:?} alpha {:x?} verts {:?}", p.kind, p.flags, p.texture, p.alpha, p.vertices.iter().map(|v| (v.x, v.y, v.rgba)).collect::<Vec<_>>());
                    }
                }
            }
            let line = format!("{:?} prims {:?} light {}", fx.tasks.iter().map(|t| (t.running, t.ret)).collect::<Vec<_>>(), fx.prims.iter().map(|p| p.len()).collect::<Vec<_>>(), fx.stage_light);
            if line != last {
                eprintln!("u{n}: {line}");
                last = line;
            }
            n += 1;
        }
        eprintln!("finale ended after {n} updates in {:?}", started.elapsed());
        assert!(fx.finished());
    }
}
