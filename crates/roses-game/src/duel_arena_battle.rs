//! The battle arena's attack (battle state 0x39 phases 5 and 6): the
//! attacker's close-up, its attack motion with the monster's own camera
//! path, the defender's answer, the loser's destroy orbit, the winner's
//! victory pose and the fade back to the versus cards.
//!
//! Traced in the project notes [F, emu footage 016-088 where
//! marked there]; this module follows its sub-phase table update by update:
//! - sub 0 waits for the stage path (run by `ArenaShot` before this starts);
//! - sub 1-2: polar (max(size, 50), -pi/4, pi/8) around the attacker's
//!   animated position, eye fixed, target follows, 60 updates;
//! - sub 3: wait for the attacker's idle loop (t < 2), then motion 2 and
//!   the monster CMOT (kind 8, 9 mirrored; one frame per update, frames - 1
//!   updates, rotated by the unit's yaw and placed at its position);
//! - sub 4: wait for the camera, then the outcome: the defender answers
//!   (5-9) or the destroy orbit (13/15);
//! - sub 14/15: orbit the loser (yaw ramping to -pi/128 per update,
//!   distance to max(20 + size, 50), pitch to pi/8), its motion 4, the
//!   destroy effect c621 and the 128-update fade;
//! - sub 16/17: the winner's motion 5 with its second CMOT;
//! - phase 6: yaw -pi/128 per update while both fades rise 4 per update
//!   (32 updates).
//!
//! The attack effects (BMODULE) run headless as hand ports
//! (`roses_fx::bmodule`); their return codes drive the target's reaction,
//! its hold and release, the module-end camera and the gates of sub 4 / 9
//! as 0x1F52E0 does. Without an effect program the effect is taken to end
//! with the camera (a destroyed target then starts motion 4 there).
//! With `tick_live` the effects run live (`duel_bmodule::EffectWorker`) and
//! their returns come from that run; their drawing is the renderer's.
//! Sound: each unit's model cues (0x1EEC80) before its motion step, timed
//! by the attack effect's task counter while one is attached (the effect's
//! own sounds; no module calls the SE player), c621's SECTL cue, and the
//! task killed by phase 7 after a skip [F; emu the project notes].
//! Not reproduced [H, reported by the renderer]: c621's drawing (its
//! timing is the footage's: result 2 after 33 updates, 180 updates long),
//! the motion blends (0x1F2760's 6 updates), the lights 2-5, and the module-end camera's length (30 updates; the original's is
//! max(30, the target's motion remainder)).
use crate::duel_arena::{Cmot, CmotCamera};
use crate::duel_models::LoadedModel;
use roses_client::monster_pose::{self, MotionClock, UnitObject};
use std::f64::consts::PI;
use std::sync::Arc;

/// What the attack phase needs of one unit.
#[derive(Debug)]
pub struct ArenaUnitData {
    pub card: u16,
    /// u16 0x2CEC28[card] (0x1DF4D0): the close-up distance.
    pub size: u16,
    /// u16 0x2CE6D0[card]: 0x8000 / 0x4000 random mirror of the attack /
    /// victory camera.
    pub flags: u16,
    pub model: LoadedModel,
    /// The slot's first and second CMOT (object +0x16C / +0x170).
    pub attack_camera: Option<Cmot>,
    pub victory_camera: Option<Cmot>,
    /// The attack effect's return codes, launch first (BMODULE port run
    /// headless, `roses_fx::bmodule`); `None`: not available (the effect is then
    /// taken to end with the camera).
    pub effect: Option<Vec<i32>>,
}

impl PartialEq for ArenaUnitData {
    fn eq(&self, other: &Self) -> bool {
        self.card == other.card
    }
}

/// The battle's result as the arena reads it (0x1DF530's three calls).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// F(A, D, flag 0): 4 the attacker destroys, 3 the defender wins, 6
    /// the defence holds.
    pub first: u8,
    /// F(A, D, flag 1).
    pub second: u8,
    /// F(D, A, flag 1).
    pub answer: u8,
}

impl Outcome {
    /// From what the engine resolved: `destroyed` bit 1 attacker, bit 0
    /// defender; the attack values decide the victory pose (F(D, A) is 4 or
    /// 3 by ATK). [H: the engine's ATK with terrain stands for 0x267A00.]
    pub fn from_battle(destroyed: u8, attack: i32, defender_attack: i32) -> Self {
        let answer = if defender_attack >= attack { 4 } else { 3 };
        match destroyed & 3 {
            1 => Self { first: 4, second: 4, answer },
            2 => Self { first: 3, second: 3, answer: 4 },
            3 => Self { first: 3, second: 4, answer: 4 },
            _ => Self { first: 6, second: 6, answer },
        }
    }
}

/// One arena unit's state (role 0 attacker, 1 defender).
#[derive(Clone, Debug)]
pub struct Unit {
    pub data: Arc<ArenaUnitData>,
    pub object: UnitObject,
    pub clock: MotionClock,
    /// obj+0x12F (0x80 opaque).
    pub alpha: u8,
    /// A motion change being blended (0x1F2770).
    pub blend: Option<MotionBlend>,
    /// obj+0x20D bit 0, set and cleared by attack effects (0x1F84D0): the
    /// motion step and the cue player skip the unit [F 0x1EE7B4 / 0x1EED60;
    /// emu battle-pine 036-043: B220 holds Orion's motion 4 at t 10 from
    /// call 121 to ~250].
    pub frozen: bool,
}

/// The previous motion during a blend: it keeps advancing; the current
/// motion's weight per draw is 0, 0, 1/n .. (n-2)/n, then 1 [F
/// the project notes for n = 6; n = 16 for the release, H].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionBlend {
    pub previous: MotionClock,
    pub updates: u8,
    pub step: u8,
}

impl MotionBlend {
    /// The current motion's weight for this draw.
    pub fn weight(&self) -> f32 {
        if self.step < 2 {
            0.0
        } else {
            f32::from(self.step - 1) / f32::from(self.updates)
        }
    }
}

impl PartialEq for Unit {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.object == other.object
            && self.clock == other.clock
            && self.alpha == other.alpha
            && self.blend == other.blend
            && self.frozen == other.frozen
    }
}

impl Unit {
    pub fn new(data: Arc<ArenaUnitData>, role: usize) -> Self {
        let model = &data.model.model;
        let (position, yaw) = ARENA_POSES[role];
        let mut object = UnitObject::board_unit(model, 3, 3, 0);
        object.position = position;
        // Arena objects are not recentred (obj+0x1FF = 0) [emu battle-pine
        // 032 / 044 / 060: the skeleton node results match only with k = 0].
        object.recentre = 0;
        object.rotation[1] = yaw;
        let length = model.motion(0).map_or(1, |m| m.length);
        Self {
            object,
            clock: MotionClock::new(0, length),
            alpha: 0x80,
            blend: None,
            frozen: false,
            data,
        }
    }

    /// 0x1F2A40: play `motion` from t = 1 without a blend; a model without
    /// it keeps its motion.
    fn set_motion(&mut self, motion: u8) {
        if let Some(m) = self.data.model.model.motion(motion) {
            self.clock = MotionClock::new(motion, m.length);
            self.blend = None;
        }
    }

    /// 0x1F2760 / 0x1F2770: change to `motion` blending from the current
    /// one over `updates` updates.
    fn change_motion(&mut self, motion: u8, updates: u8) {
        let previous = self.clock;
        if let Some(m) = self.data.model.model.motion(motion) {
            self.clock = MotionClock::new(motion, m.length);
            self.blend = Some(MotionBlend { previous, updates, step: 0 });
        }
    }

    /// 0x1EEC80, run before the update's motion step (0x1AAD94, then
    /// 0x1EE6E0): the cue list obj+0x174 {u16 frame << 4 | key, u16 id}.
    /// With an attack effect attached (obj+0x1EC = its task, set by the
    /// launcher 0x1F1EE0, -1 again at its end 0x1EE6A0) the cues of key 2
    /// fire by the task's counter K (0x1F6530, the calls made so far): frame
    /// f in [K - 1, K); without, the cues of the current motion fire by the
    /// motion time t (obj+0x19C) in [t - 1, t), only while the motion runs
    /// forward (obj+0x1F7 bits 6-7 = 1). Ids keep bit 0x8000 (skip while
    /// playing) [F; emu `emu_se.py`: the footage battle's 57 SE requests].
    fn cues(&self, effect_counter: Option<usize>) -> Vec<u16> {
        if self.frozen {
            return Vec::new();
        }
        let (key, now) = match effect_counter {
            Some(k) => (2, k as f32),
            None if self.clock.direction == 1 => (self.clock.motion, self.clock.time),
            None => return Vec::new(),
        };
        // 0x1A90D0: the frames per update (0x2DAB86 = 1 in every state).
        let before = now - 1.0;
        self.data
            .model
            .model
            .sound_cues
            .iter()
            .filter(|c| c.motion == key && before <= f32::from(c.frame) && f32::from(c.frame) < now)
            .map(|c| c.sound)
            .collect()
    }

    /// 0x1EE6E0: one update; attack and victory motions fall back to idle
    /// (the queued motion 0, no blend) when they wrap; a blend's previous
    /// motion keeps advancing.
    fn advance(&mut self) {
        if !self.frozen {
            self.advance_clock();
        }
    }

    fn advance_clock(&mut self) -> Vec<u8> {
        if let Some(b) = &mut self.blend {
            b.previous.advance(1);
            b.step += 1;
            if b.step >= b.updates {
                self.blend = None;
            }
        }
        if self.clock.advance(1) && self.clock.motion != 0 {
            self.set_motion(0);
        }
        Vec::new()
    }

    /// The play mode (obj+0x1F7): 0 once motion 3 / 4 holds its last frame.
    fn playing(&self) -> bool {
        self.clock.direction != 0
    }

    /// obj+0x80, the animated world position: node 1's origin (node 0's
    /// without a node 1) under the object matrix, facing included, at the
    /// current motion time [emu: battle-pine 032 / 044 / 060, Kumootoko m0 t21
    /// (0, 50.370, 90.216), m2 t59 (0, 49.336, 82.948), Orion m2 t208 / m4 t12
    /// within 1e-4; `duel_bmodule` private test].
    pub fn position(&self) -> [f64; 3] {
        let model = &self.data.model.model;
        let sine = &self.data.model.sine;
        let world = monster_pose::object_matrix(model, &self.object, sine);
        let blend = self.blend.and_then(|b| {
            model.motion(b.previous.motion).map(|previous| monster_pose::PoseBlend {
                previous,
                time: b.previous.time,
                weight: b.weight(),
            })
        });
        let origin = monster_pose::evaluate_pose_blended(model, model.motion(self.clock.motion), self.clock.time, blend, sine)
            .ok()
            .and_then(|p| p.nodes.get(1).or(p.nodes.first()).copied())
            .map(|n| monster_pose::apply(&monster_pose::mul(&world, &n), [0.0, 0.0, 0.0, 1.0]))
            .unwrap_or_else(|| {
                let p = self.object.position;
                [f64::from(p[0]), f64::from(p[1]), f64::from(p[2]), 1.0]
            });
        [origin[0], origin[1], origin[2]]
    }

    fn yaw(&self) -> f64 {
        f64::from(self.object.rotation[1]) * 2.0 * PI / 65536.0
    }
}

/// The arena poses (0x1E64E0, table 0x291440): attacker (0, 50, 80) yaw
/// 0x8000, defender (0, 50, -80) yaw 0 [emu].
pub const ARENA_POSES: [([f32; 3], i32); 2] = [([0.0, 50.0, 80.0], 0x8000), ([0.0, 50.0, -80.0], 0)];

/// Camera 0 in the arena: eye, target, roll and projection distance; the
/// polar is derived from eye and target (CAMERA-TRACKS convention: eye =
/// target + dist (cos p cos y, sin p, cos p sin y)).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub eye: [f64; 3],
    pub target: [f64; 3],
    pub roll: f64,
    pub distance: f64,
    /// cam+0x8B0: 0 = distance at least 50 and pitch clamped, 1 = free.
    pub clamp_mode: u8,
}

const PITCH_LIMIT: f64 = 1.570_621;

impl Camera {
    pub fn view(&self) -> CmotCamera {
        CmotCamera {
            eye: self.eye,
            target: self.target,
            roll: self.roll,
            distance: self.distance,
        }
    }

    /// (dist, yaw, pitch) of the eye around the target.
    pub fn polar(&self) -> (f64, f64, f64) {
        let d: [f64; 3] = std::array::from_fn(|i| self.eye[i] - self.target[i]);
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if dist <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let yaw = d[2].atan2(d[0]).rem_euclid(2.0 * PI);
        let pitch = (d[1] / dist).clamp(-1.0, 1.0).asin();
        (dist, yaw, pitch)
    }

    fn set_polar(&mut self, dist: f64, yaw: f64, pitch: f64) {
        let (cp, sp) = (pitch.cos(), pitch.sin());
        self.eye = [
            self.target[0] + dist * cp * yaw.cos(),
            self.target[1] + dist * sp,
            self.target[2] + dist * cp * yaw.sin(),
        ];
    }

    /// 0x1FD420 mode 1 with t = 1: the target snaps to `position`, the eye
    /// stays.
    fn follow(&mut self, position: [f64; 3]) {
        self.target = position;
    }

    /// 0x1FC8B0(cam, 0, +1, dyaw, dpitch, ddist): the eye moves round the
    /// target; mode 0 keeps the distance at least 50 by moving the target
    /// back along the view, and the pitch within +-1.570621.
    fn nudge(&mut self, dyaw: f64, dpitch: f64, ddist: f64) {
        let (mut dist, yaw, pitch) = self.polar();
        let mut dpitch = dpitch;
        if self.clamp_mode != 1 {
            let min = if self.clamp_mode == 2 { 10.0 } else { 50.0 };
            if dist + ddist < min && dist > 0.0 {
                let need = min - ddist;
                let dir: [f64; 3] = std::array::from_fn(|i| (self.target[i] - self.eye[i]) / dist);
                self.target = std::array::from_fn(|i| self.eye[i] + dir[i] * need);
                dist = need;
            }
            dpitch = dpitch.clamp(-PITCH_LIMIT - pitch, PITCH_LIMIT - pitch);
        }
        dist += ddist;
        let yaw = (2.0 * PI + yaw + dyaw).rem_euclid(2.0 * PI);
        let pitch = (pitch + dpitch) % (2.0 * PI);
        self.set_polar(dist, yaw, pitch);
    }

    /// 0x1FCB50 for a unit's CMOT at `elapsed`: rotated by the unit's yaw
    /// and placed at its position (obj+0x00), mirrored in x when `mirror`.
    fn from_unit_cmot(&mut self, cmot: &Cmot, elapsed: u32, unit: &Unit, mirror: bool) {
        let c = cmot.at(elapsed, cmot.frames.saturating_sub(1));
        let (s, co) = unit.yaw().sin_cos();
        let p = unit.object.position.map(f64::from);
        let place = |v: [f64; 3]| {
            let x = if mirror { -v[0] } else { v[0] };
            [co * x + s * v[2] + p[0], v[1] + p[1], -s * x + co * v[2] + p[2]]
        };
        self.eye = place(c.eye);
        self.target = place(c.target);
        self.roll = if mirror { -c.roll } else { c.roll };
        self.distance = c.distance;
    }
}

/// A unit camera path in progress (track kind 8 / 9, tag 1).
#[derive(Clone, Debug, PartialEq)]
struct Track {
    role: usize,
    victory: bool,
    mirror: bool,
    elapsed: u32,
}

/// A running attack effect (a type-1 task slot).
#[derive(Clone, Debug, PartialEq)]
struct Task {
    role: usize,
    /// Calls made (0 = the launch).
    call: usize,
    /// The module-end camera was set (slot +3 bit 7).
    end_camera: bool,
}

/// The module-end camera (tag 2, set by the task executor when an attack
/// effect ends): projection distance to 800 and roll to 0 over `length`
/// updates, the target eased onto `role`'s unit (eye fixed) [emu linear d:
/// 687 / 754 at 3 / 30, 19 / 30].
#[derive(Clone, Debug, PartialEq)]
struct ModuleEnd {
    role: usize,
    elapsed: u32,
    length: u32,
    from_target: [f64; 3],
    from_distance: f64,
    from_roll: f64,
}

/// c621's result 2 and end (C621-DESTROY.md) [F/emu].
const C621_RESULT_TWO: u32 = crate::duel_c621::RESULT_TWO_AT;
const C621_LENGTH: u32 = crate::duel_c621::LENGTH;
/// The module-end camera's length [H: max(30, the target's remainder)].
const MODULE_END_UPDATES: u32 = 30;
/// Phase 6: 32 updates of +4 to both fades.
pub const EXIT_UPDATES: u32 = 32;
/// Phase 8: d3a 128 -> 0 by 4 under the cards [F, emu 085-088].
pub const RETURN_UPDATES: u32 = 32;

/// One call of a running attack effect: (the battle as it stands, the
/// attacker's role, the call: 0 = the launch, the state the module shares
/// with the game) -> its return (`None`: no effect program).
pub type EffectCall<'a> = dyn FnMut(&ArenaBattle, usize, usize, &mut FxShared) -> Option<i32> + 'a;

/// What a live module call reads and changes besides its return: the
/// caller runs the module on these and leaves them as the module did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FxShared {
    /// The game's newlib rand state (0x19AE88; one for the whole game: the
    /// module's draws advance it).
    pub rand: u64,
    /// obj+0x20D bit 0 of the battle's units (by battle role).
    pub frozen: [bool; 2],
}

/// Newlib rand draws (0x19AE88) the game makes every frame outside the
/// battle's own: 0x20A950 (ra 0x20A980) and 0x100820 (ra 0x100830, the main
/// loop's discarded draw), both before the battle logic [emu
/// the project notes 28`: 2 per frame in all 1,224 frames].
pub const FRAME_RAND_DRAWS: u32 = 2;

/// 0x1D151C / 0x1F5854's test: int(k * (float(r) / 2^31)) != 0 (cvt.s.w,
/// div.s, mul.s and 0x1A2B80's truncation; the EE rounds towards zero).
pub fn rand_scaled(r: i32, k: f32) -> i32 {
    fn trunc(x: f64) -> f32 {
        let y = x as f32;
        if f64::from(y).abs() > x.abs() {
            f32::from_bits(y.to_bits() - 1)
        } else {
            y
        }
    }
    let f = f64::from(trunc(f64::from(r))) / 2_147_483_648.0;
    trunc(f64::from(k) * f) as i32
}

/// Battle state 0x39 phases 5 and 6.
#[derive(Clone, Debug, PartialEq)]
pub struct ArenaBattle {
    pub units: [Unit; 2],
    pub camera: Camera,
    outcome: Outcome,
    /// d38: 5 or 6 (7 = finished).
    pub phase: u8,
    /// d39.
    pub sub: u8,
    d35: u32,
    d36: u32,
    /// Phase 6 fade (d3a / d3b), 0..=128.
    pub fade: u8,
    track: Option<Track>,
    /// The running attack effect (one at a time).
    task: Option<Task>,
    /// Whether the last attack had an effect program (else its end is
    /// timed by the camera).
    effect_ran: bool,
    module_end: Option<ModuleEnd>,
    /// handle[0..2]: c621 launched at the attacker / defender.
    c621: [Option<crate::duel_c621::C621>; 2],
    /// Sound cues of this update ((slot, id), drained by the duel).
    /// Ids keep bit 0x8000 (0x216EA0: dropped while the entry plays).
    pub sounds: Vec<(u8, u16)>,
    /// Sub 10's start (target, polar) for the 90-update move.
    move_from: Option<([f64; 3], (f64, f64, f64))>,
    /// Seed of the port's own effects (c621).
    seed: u32,
    /// The game's newlib rand state (*(0x28ED18) + 0xA8): the frame draws,
    /// the camera mirror (0x1D151C), the reaction hold (0x1F5854) and the
    /// live modules' draws share it [F, emu `emu_rand.py`]. Starts from the
    /// battle's seed (the game's state there is its whole history's).
    pub rand: u64,
    /// Updates run (the game's frame counter 0x2E963C moves with them).
    pub updates: u32,
    /// A live effect return differed from the cached headless run since
    /// the flag was last cleared (the battle's length must be recomputed).
    pub effect_diverged: bool,
}

impl ArenaBattle {
    /// At the end of the stage path (sub 0 done): `camera` is the stage
    /// path's last camera.
    pub fn new(units: [Arc<ArenaUnitData>; 2], outcome: Outcome, camera: CmotCamera, seed: u32) -> Self {
        let [a, d] = units;
        // Phase 2 re-poses both units in idle; they have idled through the
        // crossfade and the orbit since [H: the load wait is not counted];
        // the first update's step is this battle's own (t 52 at the end of
        // the update saved as battle-pine 028 [emu]).
        let mut units = [Unit::new(a, 0), Unit::new(d, 1)];
        for u in &mut units {
            for _ in 1..crate::duel_versus::ARENA_FADE_UPDATES + crate::duel_versus::ARENA_ORBIT_UPDATES {
                u.advance_clock();
            }
        }
        Self {
            units,
            camera: Camera {
                eye: camera.eye,
                target: camera.target,
                roll: camera.roll,
                distance: camera.distance,
                clamp_mode: 0,
            },
            outcome,
            phase: 5,
            sub: 1,
            d35: 0,
            d36: 0,
            fade: 0,
            track: None,
            task: None,
            effect_ran: false,
            module_end: None,
            c621: [None, None],
            sounds: Vec::new(),
            move_from: None,
            seed,
            rand: u64::from(seed),
            updates: 0,
            effect_diverged: false,
        }
    }

    /// The running attack effect: (attacker role, calls made).
    pub fn effect_task(&self) -> Option<(usize, usize)> {
        self.task.as_ref().map(|t| (t.role, t.call))
    }

    pub fn done(&self) -> bool {
        self.phase > 8
    }

    /// Phase 8: the cards rest over the board (the arena is gone).
    pub fn returning(&self) -> bool {
        self.phase == 8
    }

    /// The destroy effects launched so far (c621 at each loser).
    pub fn destroy_effects(&self) -> impl Iterator<Item = &crate::duel_c621::C621> {
        self.c621.iter().flatten()
    }

    /// Updates left until [`done`](Self::done) (a deterministic replay).
    pub fn length(&self) -> u32 {
        let mut copy = self.clone();
        let mut n = 0;
        while !copy.done() && n < 20_000 {
            copy.tick();
            n += 1;
        }
        n
    }

    /// One module call through `fx` with the shared state.
    fn call_effect(&mut self, fx: &mut EffectCall<'_>, role: usize, call: usize) -> Option<i32> {
        let mut shared = FxShared { rand: self.rand, frozen: [self.units[0].frozen, self.units[1].frozen] };
        let live = fx(&*self, role, call, &mut shared);
        self.rand = shared.rand;
        for (u, f) in self.units.iter_mut().zip(shared.frozen) {
            u.frozen = f;
        }
        live
    }

    /// newlib rand (0x19AE88): state * 0x5851F42D4C957F2D + 1, bits 32..62.
    fn rand(&mut self) -> i32 {
        self.rand = self.rand.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
        ((self.rand >> 32) & 0x7FFF_FFFF) as i32
    }

    fn size(&self, role: usize) -> f64 {
        f64::from(self.units[role].data.size)
    }

    /// The cached headless return of `role`'s attack effect for `call`
    /// (0 = the launch); `None` without an effect program.
    pub fn cached_return(&self, role: usize, call: usize) -> Option<i32> {
        self.units[role].data.effect.as_ref().map(|r| r.get(call).copied().unwrap_or(8))
    }

    /// 0x1F1EE0(role, 1, alt) (attack) or 0x1F2060 (victory, motion 5).
    fn attack(&mut self, role: usize, victory: bool, fx: &mut EffectCall<'_>) {
        let flag = if victory { 0x4000 } else { 0x8000 };
        // 0x1D1514: int(2 * rand / 2^31) != 0.
        let mirror = self.units[role].data.flags & flag != 0 && {
            let r = self.rand();
            rand_scaled(r, 2.0) != 0
        };
        let other = 1 - role;
        self.units[other].set_motion(0);
        self.units[role].change_motion(if victory { 5 } else { 2 }, 6);
        let has = if victory {
            self.units[role].data.victory_camera.is_some()
        } else {
            self.units[role].data.attack_camera.is_some()
        };
        self.track = has.then_some(Track { role, victory, mirror, elapsed: 0 });
        // 0x1F5B10: the effect's launch call (its return 8 would kill it
        // at once); the victory pose has none.
        self.effect_ran = false;
        if !victory {
            let live = self.call_effect(fx, role, 0);
            if live != self.cached_return(role, 0) {
                self.effect_diverged = true;
            }
            if let Some(first) = live {
                self.effect_ran = true;
                if first != 8 {
                    self.task = Some(Task { role, call: 0, end_camera: false });
                }
            }
        }
    }

    /// 0x1F52E0's type-1 block for one return of the running effect
    /// (BMODULE-SCOPE.md 1.4): the target's reaction (motion 4, or 6 when the
    /// defence holds), its hold, its release to idle, the module-end camera
    /// and the end.
    fn executor(&mut self, fx: &mut EffectCall<'_>) {
        let Some(mut task) = self.task.take() else {
            return;
        };
        task.call += 1;
        let live = self.call_effect(fx, task.role, task.call);
        if live != self.cached_return(task.role, task.call) {
            self.effect_diverged = true;
        }
        let mut ret = live.unwrap_or(8);
        let target = 1 - task.role;
        // r = 0x1F8160(0): this attack's outcome (flag 0).
        let r = if task.role == 0 { self.outcome.first } else { self.outcome.answer };
        let m = self.units[target].clock.motion;
        if ret == 1 {
            ret = if r == 6 { 6 } else { 0 };
        }
        match ret {
            0 => {
                if r == 3 && m == 4 && self.units[target].clock.time >= 16.0 {
                    self.release(target);
                }
            }
            2 | 3 | 5 | 6 => {
                if !matches!(m, 3 | 4 | 6) {
                    self.units[target].change_motion(if r == 3 { 4 } else { r }, 6);
                }
                if ret == 3 {
                    self.end_camera(&mut task, target, r);
                }
                if ret == 6 {
                    let hold = self.units[target].playing() && self.units[target].clock.time >= 11.0;
                    if hold {
                        let t = self.units[target].clock.time;
                        // 0x1F5814..0x1F5890: t >= 16 holds, else one draw:
                        // int(3 * rand / 2^31) != 0.
                        if t >= 16.0 || {
                            let r = self.rand();
                            rand_scaled(r, 3.0) != 0
                        } {
                            self.units[target].clock.time = t - 2.0;
                        }
                    }
                }
            }
            4 => self.end_camera(&mut task, target, r),
            7 => {
                if matches!(m, 3 | 4) {
                    self.release(target);
                }
            }
            _ => {}
        }
        if ret == 8 {
            self.end_camera(&mut task, target, r);
            if r == 3 && m == 4 {
                self.release(target);
            }
            return;
        }
        self.task = Some(task);
    }

    /// The reaction's release: back to idle with a 16-update blend.
    fn release(&mut self, role: usize) {
        self.units[role].change_motion(0, 16);
    }

    /// The module-end camera, once per effect: over max(30, the target's
    /// frames of motion r - its time in it) updates.
    fn end_camera(&mut self, task: &mut Task, target: usize, r: u8) {
        if task.end_camera {
            return;
        }
        task.end_camera = true;
        let u = &self.units[target];
        let frames = u.data.model.model.motion(r).map_or(0, |m| m.length) as f32;
        let t = if u.clock.motion == r { u.clock.time } else { 0.0 };
        let length = (frames - t).max(30.0) as u32;
        self.module_end = Some(ModuleEnd {
            role: target,
            elapsed: 0,
            length,
            from_target: self.camera.target,
            from_distance: self.camera.distance,
            from_roll: self.camera.roll,
        });
    }

    /// The attack's end as the port times it: the camera has ended; a
    /// destroyed target starts its motion 4 and the module-end camera eases
    /// onto it.
    fn attack_end(&mut self, role: usize, target_destroyed: bool) {
        let target = 1 - role;
        if target_destroyed {
            self.units[target].set_motion(4);
        }
        self.module_end = Some(ModuleEnd {
            role: target,
            elapsed: 0,
            length: MODULE_END_UPDATES,
            from_target: self.camera.target,
            from_distance: self.camera.distance,
            from_roll: self.camera.roll,
        });
    }

    /// Phase 5 prologue: a human's skip press (pad 0x20) only starts the
    /// exit, phase 6 from the next update; nothing is killed
    /// [F ARENA-ATTACK.md 2]. True when it took effect.
    pub fn skip(&mut self) -> bool {
        if self.phase != 5 {
            return false;
        }
        self.to_exit();
        true
    }

    fn to_exit(&mut self) {
        self.phase = 6;
        self.d35 = 0;
    }

    /// One update: the logic (sub-phases), the camera tracks, then the
    /// units' motions (the original's frame order), with the attack effects'
    /// cached headless returns.
    pub fn tick(&mut self) {
        self.tick_live(&mut |b, role, call, _| b.cached_return(role, call));
    }

    /// [`tick`](Self::tick) with the attack effect's returns from `fx`
    /// (called at the launch inside the logic and once per update in the
    /// executor, after the units' motions, with the battle as it stands).
    pub fn tick_live(&mut self, fx: &mut EffectCall<'_>) {
        self.updates += 1;
        for _ in 0..FRAME_RAND_DRAWS {
            self.rand();
        }
        // The object step first (0x1AABB0: 0x1EEC80 cues, 0x1F5130, the
        // motion step 0x1EE6E0, then the mode update with the battle logic
        // 0x1D00F0; the camera tracks 0x1FB430; the executors 0x1F52E0 in the
        // draw 0x100850) [F main loop 0x100560; emu `emu_rand.py`: B125
        // launches the update Orion's idle loops, Kumootoko t 52 at the end
        // of state 028's update]: the logic sees the times after the step.
        let effect = self.task.as_ref().map(|t| (t.role, t.call));
        for (role, u) in self.units.iter_mut().enumerate() {
            let counter = effect.filter(|e| e.0 == role).map(|e| e.1);
            for id in u.cues(counter) {
                self.sounds.push((2 + role as u8, id));
            }
            u.advance();
        }
        // 0x1F5130: the C tasks' SECTL.MRG cues by their counter K (calls
        // made), frame f in [K - 1, K); c621's record 0x26D is (1, 0x57) at
        // frame 1 [F]: K = 2, two updates after its launch [emu `emu_se.py`
        // with `emu_rand.py`'s alignment: update 783 for a launch in 781].
        for c in self.c621.iter().flatten() {
            if c.u == 2 {
                self.sounds.push((1, 0x57));
            }
        }
        match self.phase {
            5 => self.phase5(fx),
            6 => {
                self.camera.nudge(-PI / 128.0, 0.0, 0.0);
                self.fade = (self.fade + 4).min(128);
                self.d35 += 1;
                if self.d35 >= EXIT_UPDATES {
                    // Phase 7 tears down in the same update; phase 8 waits
                    // for the board's loads (not counted [H]), then lowers
                    // d3a over 32 updates under the opaque cards.
                    self.phase = 8;
                    self.d35 = 0;
                }
            }
            8 => {
                self.d35 += 1;
                if self.d35 >= RETURN_UPDATES {
                    self.phase = 9;
                }
            }
            _ => {}
        }
        self.tracks();
        self.executor(fx);
        // Phase 7's teardown ends a task the skip left running: 32 calls
        // after the press (emu: counter 126 -> 158; `emu_se.py 28 .. 100`:
        // B220's cues stop 32 updates after the press, its last ones unplayed).
        if self.phase >= 8 {
            self.task = None;
        }
        for c in self.c621.iter_mut().flatten() {
            c.update();
        }
    }

    fn tracks(&mut self) {
        if let Some(t) = &mut self.track {
            t.elapsed += 1;
            let t = t.clone();
            let unit = &self.units[t.role];
            let cmot = if t.victory {
                unit.data.victory_camera.as_ref()
            } else {
                unit.data.attack_camera.as_ref()
            };
            if let Some(cmot) = cmot {
                let duration = cmot.frames.saturating_sub(1).max(1);
                let cmot = cmot.clone();
                let unit = unit.clone();
                self.camera.from_unit_cmot(&cmot, t.elapsed.min(duration), &unit, t.mirror);
                if t.elapsed >= duration {
                    self.track = None;
                }
            } else {
                self.track = None;
            }
        }
        if let Some(m) = &mut self.module_end {
            m.elapsed += 1;
            let k = f64::from(m.elapsed.min(m.length)) / f64::from(m.length);
            let m = m.clone();
            let to = self.units[m.role].position();
            self.camera.target = std::array::from_fn(|i| m.from_target[i] + (to[i] - m.from_target[i]) * k);
            self.camera.distance = m.from_distance + (800.0 - m.from_distance) * k;
            self.camera.roll = m.from_roll * (1.0 - k);
            if m.elapsed >= m.length {
                self.module_end = None;
            }
        }
    }

    fn phase5(&mut self, fx: &mut EffectCall<'_>) {
        loop {
            match self.sub {
                1 => {
                    let t = self.units[0].position();
                    self.camera.follow(t);
                    self.camera.set_polar(self.size(0).max(50.0), 2.0 * PI - PI / 4.0, PI / 8.0);
                    self.d35 = 60;
                    self.sub = 2;
                }
                2 => {
                    let t = self.units[0].position();
                    self.camera.follow(t);
                    if self.d35 > 0 {
                        self.d35 -= 1;
                        return;
                    }
                    self.sub = 3;
                }
                3 | 8 => {
                    let role = usize::from(self.sub == 8);
                    if self.units[role].clock.time >= 2.0 {
                        return;
                    }
                    self.attack(role, false, fx);
                    self.sub += 1;
                }
                4 => {
                    if self.track.is_some() || self.task.is_some() {
                        return;
                    }
                    if !self.effect_ran {
                        self.attack_end(0, self.outcome.first == 4);
                    }
                    self.sub = if self.outcome.first == 3 { 5 } else { 13 };
                    return;
                }
                5 => {
                    let want = (20.0 + self.size(1)).max(50.0);
                    let delta = want - self.camera.polar().0;
                    let t = self.units[1].position();
                    self.camera.follow(t);
                    let dd = delta.clamp(-2.0, 2.0);
                    self.camera.nudge(0.0, 0.0, dd);
                    if dd > 1e-6 || self.units[1].clock.motion != 0 {
                        return;
                    }
                    self.sub = 6;
                }
                6 => {
                    self.d35 = 30;
                    self.sub = 7;
                }
                7 => {
                    let t = self.units[1].position();
                    self.camera.follow(t);
                    if self.d35 > 0 {
                        self.d35 -= 1;
                        return;
                    }
                    self.sub = 8;
                }
                9 => {
                    if self.track.is_some() || self.task.is_some() {
                        return;
                    }
                    let destroys = self.outcome.answer == 4;
                    if !self.effect_ran {
                        self.attack_end(1, destroys);
                    }
                    // 18 (answer 3) is a dead end in the original.
                    self.sub = if destroys { 12 } else { 18 };
                    return;
                }
                10 => {
                    // Kind 3 + 4 tracks over 90 updates [F, not in footage]:
                    // here a direct lerp of target and polar.
                    self.d35 = 0;
                    self.sub = 11;
                    self.move_from = Some((self.camera.target, self.camera.polar()));
                    return;
                }
                11 => {
                    self.d35 += 1;
                    let (from_t, (fd, fy, fp)) = self.move_from.unwrap_or((self.camera.target, self.camera.polar()));
                    let k = f64::from(self.d35.min(90)) / 90.0;
                    let to = self.units[1].position();
                    let (td, ty, tp) = (self.size(1).max(50.0), 3.0 * PI / 4.0, PI / 8.0);
                    let mut dy = ty - fy;
                    if dy > PI {
                        dy -= 2.0 * PI;
                    } else if dy < -PI {
                        dy += 2.0 * PI;
                    }
                    self.camera.target = std::array::from_fn(|i| from_t[i] + (to[i] - from_t[i]) * k);
                    self.camera.set_polar(fd + (td - fd) * k, fy + dy * k, fp + (tp - fp) * k);
                    if self.d35 < 90 {
                        return;
                    }
                    self.d36 = 0x80;
                    self.sub = 13;
                }
                12 | 13 => {
                    self.c621[usize::from(self.sub - 12)] = None;
                    self.d35 = 0;
                    self.sub += 2;
                }
                14 | 15 => {
                    self.destroy_orbit();
                    return;
                }
                16 => {
                    self.attack(0, true, fx);
                    if self.track.is_some() {
                        self.camera.clamp_mode = 0;
                    } else {
                        let t = self.units[0].position();
                        self.camera.follow(t);
                        self.camera.set_polar(self.size(0).max(80.0), 2.0 * PI - PI / 4.0, PI / 8.0);
                        self.camera.clamp_mode = 1;
                    }
                    self.sub = 17;
                }
                17 => {
                    if self.camera.clamp_mode == 1 {
                        let t = self.units[0].position();
                        self.camera.follow(t);
                    } else if self.track.is_some() {
                        return;
                    }
                    if self.units[0].clock.motion != 0 {
                        return;
                    }
                    self.camera.clamp_mode = 0;
                    self.to_exit();
                    return;
                }
                _ => return,
            }
        }
    }

    /// Sub 14 / 15 (ARENA-ATTACK.md 3.1).
    fn destroy_orbit(&mut self) {
        let s0 = usize::from(self.sub - 14);
        let (dist, _, pitch) = self.camera.polar();
        let want = (20.0 + self.size(s0)).max(50.0);
        let dd0 = want - dist;
        let dp0 = PI / 8.0 - pitch;
        let m = self.units[s0].clock.motion;
        let k = f64::from(self.d36.min(128));
        let t = self.units[s0].position();
        self.camera.follow(t);
        let dd = dd0.clamp(-k / 64.0, k / 64.0);
        let dp = dp0.clamp(-k * PI / 32768.0, k * PI / 32768.0);
        let dy = -2.0 * k * PI / 32768.0;
        self.camera.nudge(dy, dp, dd);
        if self.d36 < 255 {
            self.d36 += 1;
        }
        let busy_defender = s0 == 0 && self.outcome.second == 4 && self.units[1].clock.motion != 4;
        if busy_defender {
            if self.units[1].clock.motion == 0 {
                self.units[1].set_motion(4);
            }
        }
        let exit = if m != 4 {
            if self.d35 < 90 {
                self.d35 += 1;
                return;
            }
            true
        } else {
            false
        };
        if !exit {
            if self.units[s0].playing() {
                return;
            }
            let Some(launched) = self.c621[s0].as_ref().map(|c| c.u) else {
                let anchor = self.units[s0].position();
                let size = self.units[s0].data.size;
                let seed = self.seed ^ 0x621;
                self.c621[s0] = Some(crate::duel_c621::C621::new(anchor, size, seed));
                return;
            };
            if self.d35 == 0 {
                if launched >= C621_RESULT_TWO {
                    self.d35 = 1;
                }
                return;
            }
            if self.units[s0].alpha > 0 {
                self.units[s0].alpha -= 1;
                return;
            }
            if launched < C621_LENGTH {
                return;
            }
            if self.d35 < 30 {
                self.d35 += 1;
                return;
            }
            if busy_defender {
                return;
            }
        }
        if s0 == 1 {
            let (r1, r2) = (self.outcome.second, self.outcome.answer);
            if r1 != r2 && r1 == 4 {
                self.sub = 16;
                return;
            }
            self.to_exit();
        } else if self.outcome.second == 4 {
            if self.units[1].playing() {
                return;
            }
            self.sub = 10;
        } else {
            self.to_exit();
        }
    }
}

#[cfg(test)]
mod private_tests {
    use super::*;

    /// The footage battle (ARENA-ATTACK.md 7): Kumootoko (406) attacks
    /// Orion (370) and is destroyed by its counter. Needs private imports at
    /// `ROSES_MODELS_MANIFEST` and `ROSES_ARENA_MANIFEST`.
    #[test]
    #[ignore = "needs private imports"]
    fn footage_battle_follows_the_traced_path() {
        let models = std::path::PathBuf::from(std::env::var("ROSES_MODELS_MANIFEST").unwrap());
        let arena = crate::duel_arena::load_arena(std::path::Path::new(&std::env::var("ROSES_ARENA_MANIFEST").unwrap())).unwrap();
        let provider = crate::duel_arena::arena_provider(&arena, models).unwrap();
        let setup = (provider.setup)(0, [406, 370]).unwrap();
        let mut b = ArenaBattle::new(setup.units, Outcome::from_battle(2, 1200, 1400), setup.stage_end, 1);
        b.tick();
        let (dist, yaw, pitch) = b.camera.polar();
        eprintln!("close-up: eye {:?} target {:?} polar {dist:.3} {yaw:.4} {pitch:.4}", b.camera.eye, b.camera.target);
        assert!((dist - 50.0).abs() < 0.5 && (yaw - 5.4978).abs() < 0.01 && (pitch - 0.3927).abs() < 0.01);
        // Footage 024: eye (32.7, 69.2, 57.6), target (0, 50.1, 90.2) as it bobs.
        assert!((b.camera.target[2] - 90.2).abs() < 1.5 && (b.camera.eye[2] - 57.6).abs() < 1.5, "{:?}", b.camera);
        let mut subs = vec![(0u32, b.phase, b.sub)];
        let mut n = 1;
        while !b.done() && n < 5000 {
            b.tick();
            n += 1;
            if subs.last().map(|s| (s.1, s.2)) != Some((b.phase, b.sub)) {
                subs.push((n, b.phase, b.sub));
            }
        }
        eprintln!("path {subs:?}, {n} updates");
        let path: Vec<u8> = subs.iter().map(|s| if s.1 == 5 { s.2 } else { 100 + s.1 }).collect();
        assert_eq!(path, [2, 3, 4, 5, 7, 8, 9, 12, 14, 106, 108, 109]);
        assert!((b.units[0].alpha, b.units[1].alpha) == (0, 0x80));
        // With the imported effects: sub 4 lasts B220's 276 updates after its
        // launch, sub 9 B125's 340 (footage: 277 / 341 calls) [emu].
        if b.units[0].data.effect.is_some() {
            let at = |sub: u8| subs.iter().find(|s| s.1 == 5 && s.2 == sub).unwrap().0;
            assert_eq!((at(5) - at(4), at(12) - at(9)), (276, 340));
        }
    }
}

#[cfg(test)]
mod private_sound {
    use super::*;

    /// The footage battle's sound requests against the emulator's
    /// (the project notes 28`: every SE request sent to the IOP,
    /// with its frame; B220 launches at frame 95641). Needs private
    /// imports and `ROSES_SE_TRACE` = that json.
    #[test]
    #[ignore = "needs private imports"]
    fn footage_battle_sounds_match_the_emulator() {
        let models = std::path::PathBuf::from(std::env::var("ROSES_MODELS_MANIFEST").unwrap());
        let arena = crate::duel_arena::load_arena(std::path::Path::new(&std::env::var("ROSES_ARENA_MANIFEST").unwrap())).unwrap();
        let provider = crate::duel_arena::arena_provider(&arena, models).unwrap();
        let setup = (provider.setup)(0, [406, 370]).unwrap();
        let mut b = ArenaBattle::new(setup.units, Outcome::from_battle(2, 1200, 1400), setup.stage_end, 1);
        let mut port = Vec::new();
        let mut launch = None;
        let mut n = 0u32;
        while !b.done() && n < 5000 {
            b.tick();
            n += 1;
            if launch.is_none() && b.effect_task().is_some() {
                launch = Some(n);
            }
            if std::env::var_os("ROSES_SE_MOTIONS").is_some() {
                eprintln!(
                    "{n}: task {:?} motions {:?}",
                    b.effect_task(),
                    b.units.iter().map(|u| (u.clock.motion, u.clock.time, u.clock.direction)).collect::<Vec<_>>()
                );
            }
            for (slot, id) in b.sounds.drain(..) {
                port.push((n, slot, id));
            }
        }
        let launch = launch.unwrap();
        let trace: serde_json::Value =
            serde_json::from_slice(&std::fs::read(std::env::var("ROSES_SE_TRACE").unwrap()).unwrap()).unwrap();
        // Battle-scene requests up to the arena's end (bank 1 0x57 = the
        // destroy flash, the last one the battle makes).
        let emu: Vec<(u32, u8, u16)> = trace["se"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                let frame = e["frame"].as_u64().unwrap() as u32;
                (frame + launch - 95641, e["bank"].as_u64().unwrap() as u8, e["id"].as_u64().unwrap() as u16)
            })
            .take_while(|e| !(e.1 == 1 && e.2 == 0x41))
            .collect();
        let strip = |v: &[(u32, u8, u16)]| v.iter().map(|&(u, s, i)| (u, s, i & 0x7FFF)).collect::<Vec<_>>();
        let (p, e) = (strip(&port), strip(&emu));
        eprintln!("port launch {launch}; port {} requests, emulator {}", p.len(), e.len());
        for x in e.iter().filter(|x| !p.contains(x)) {
            eprintln!("emulator only {x:?}");
        }
        for x in p.iter().filter(|x| !e.contains(x)) {
            eprintln!("port only {x:?}");
        }
        // Exact through B220: the headless cache has neither the unit freeze
        // (obj+0x20D: B220 holds Orion at t 10, so B125 starts 1 update
        // early here) nor the modules' rand draws (0x1F52E0's ret-6 hold on
        // Kumootoko draws from another state: his motion-4 cues and c621's
        // (1, 0x57) come 5 early). The live run with the game's one rand is
        // exact (`duel_bmodule::private_tests::
        // footage_battle_rand_and_sounds_match_the_emulator`).
        let shift: Vec<_> = p.iter().map(|&(u, s, i)| (if u > 700 { u + 5 } else if u > 400 { u + 1 } else { u }, s, i)).collect();
        assert_eq!(shift, e);
    }
}

#[cfg(test)]
mod private_skip {
    use super::*;

    #[test]
    #[ignore = "needs private imports"]
    fn a_skip_press_starts_the_exit_from_the_next_update() {
        let models = std::path::PathBuf::from(std::env::var("ROSES_MODELS_MANIFEST").unwrap());
        let arena = crate::duel_arena::load_arena(std::path::Path::new(&std::env::var("ROSES_ARENA_MANIFEST").unwrap())).unwrap();
        let provider = crate::duel_arena::arena_provider(&arena, models).unwrap();
        let setup = (provider.setup)(0, [406, 370]).unwrap();
        let mut b = ArenaBattle::new(setup.units, Outcome::from_battle(2, 1200, 1400), setup.stage_end, 1);
        let full = b.length();
        for _ in 0..10 {
            b.tick();
        }
        assert!(b.skip());
        assert!(!b.skip(), "only phase 5 skips");
        // Phase 6 (32) then phase 8 (32) until done.
        assert_eq!(b.length(), EXIT_UPDATES + RETURN_UPDATES);
        assert!(10 + b.length() < full);
    }
}

#[cfg(test)]
mod private_trace {
    use super::*;

    /// Prints a battle's sub-phase path: `ROSES_ARENA_CARDS=attacker,defender,destroyed`.
    #[test]
    #[ignore = "research print; needs private imports"]
    fn print_battle_path() {
        let models = std::path::PathBuf::from(std::env::var("ROSES_MODELS_MANIFEST").unwrap());
        let arena = crate::duel_arena::load_arena(std::path::Path::new(&std::env::var("ROSES_ARENA_MANIFEST").unwrap())).unwrap();
        let v: Vec<u16> = std::env::var("ROSES_ARENA_CARDS").unwrap().split(',').map(|s| s.parse().unwrap()).collect();
        let provider = crate::duel_arena::arena_provider(&arena, models).unwrap();
        let setup = (provider.setup)(0, [v[0], v[1]]).unwrap();
        for u in &setup.units {
            let motions: Vec<u8> = u.model.model.motions.iter().map(|m| m.motion).collect();
            eprintln!("card {} motions {motions:?} effect {:?}", u.card, u.effect.as_ref().map(|e| e.len()));
        }
        let mut b = ArenaBattle::new(setup.units, Outcome::from_battle(v[2] as u8, 1900, 1400), setup.stage_end, 1);
        let mut last = (0, 0);
        for n in 0..3000 {
            if (b.phase, b.sub) != last {
                last = (b.phase, b.sub);
                eprintln!("{n}: phase {} sub {} motions {:?} c621 {:?}", b.phase, b.sub, b.units.iter().map(|u| u.clock.motion).collect::<Vec<_>>(), b.c621.iter().map(|c| c.as_ref().map(|c| c.u)).collect::<Vec<_>>());
            }
            if b.done() {
                break;
            }
            b.tick();
        }
    }
}
