//! CMODULE c021 (0x15), Dark Hole (magic 699, event 4, handler 0x258C60:
//! stage 2 launches {0x15, 0, mask, 0} through 0x257350), as specified in
//! `claude-activate/DARK-HOLE-BLOCKER-GRAVEROBBER.md` [F code; emu: the
//! reference model `tools/mf_c021_model.py` equals the interpreter's
//! primitives in four runs (two savestates, three cameras) to 0.0625 px,
//! with every count, colour, UV, ALPHA, OT key, Z, return code, light byte,
//! rand count and unit-object write equal].
//!
//! A vortex anchored at the board centre, 65 object slots (D+0x120 + 0x94 k)
//! with update / draw callbacks that run only from the 9th update on:
//! - RINGS (0x1E03390): chains of 18-segment rings of TEXEFF 15 (143,0)-(174,64)
//!   / (143,63)-(174,127), (80,80,192) additive, around a 10-point twisting
//!   spine (heights 0..270, wobbling radius 2..15) in the node frame scale
//!   (1.5, 0.5, 1.5); every ring flows down by the speed (0.5, +0.1 every
//!   other update to 4.0) and shrinks, the chain head spawns the next ring at
//!   the top: a funnel;
//! - STREAKS (0x1E038A0): five 15-degree grey (64,64,64) wisps per object,
//!   chained the same way (TEXEFF 15 (143,0)-(174,127));
//! - SHADOW (0x1E03BA0): 8 x 45-degree purple (160,0,160) annulus segments at
//!   world y 10 around the board centre, **subtractive** (Cd - Cs As / 128),
//!   radii = the objects' heights, shrinking 4/U: the dark hole on the board;
//! - SPARKLES (0x1E03280): a camera-facing 4-frame star billboard every 3
//!   updates (u 1..133), spiralling in and falling;
//! - a full-screen quad (0x1E03EA0, subtractive (32,32,32)) whose alpha stays
//!   0 (its fade-in runs before the updates start): drawn, invisible.
//!
//! Units (22 handles, i < 11 -> g0 slot i, else g1 slot i - 11): u 1-7 every
//! 3D model shrinks -0.18/U (leaders and targets included) and every card
//! outside the mask fades -19/U (109 .. 0); the board light 116 .. 44. From
//! u 8 each target's card (not a leader's) rises spinning round the board
//! centre (height 100 sin(theta), theta += 1..6 to 100, then +1.2 to 200;
//! yaw +2..7 deg/U), is pulled in to the centre (x/z offsets -2/U), its
//! alpha -3/U once theta >= 100 and its scale 0.1 sin(theta) once > 110.
//! When no target card is visible any more, life := 30: the last 9 updates
//! bring the light (+16/U) and the other units back (alpha +16/U, models
//! +0.15/U, every card back on its cell). Returns 8 when life reaches 0
//! (u 112 with one target, 178 with eight in the runs); never 2.
//!
//! [H / not reproduced]: the port's random values (its own LCG, the same call
//! order and count: 88 at init, 12 at u 1, 8 per sparkle); no OT keys / real
//! Z (each list is emitted in slot order); the 0x1F6B80 unit flags; the card
//! scale restore 0x107DF0 reads an uninitialised stack value (taken as 0.1).
use crate::duel_c602::projector;
use crate::duel_c621::FxTri;

/// The update after which the targets are no longer drawn by the board
/// (the handler removes them when the module ends).
pub const SE_ID: u16 = 0x49;
/// TEXEFF sheet size (`claude-units/texeff/015.png`).
const SHEET: f32 = 256.0;
const DB: u32 = 0x1E1_0000;

const SL0: usize = 0x120;
const SLN: usize = 65;
const SLS: usize = 0x94;
const U0: usize = 0x26C0;
const US: usize = 0x70;
const SP0: usize = 0x3488;
const S0T: usize = 0x3750;
const C0T: usize = 0x37A0;
const S1T: usize = 0x37F0;
const C1T: usize = 0x3840;

const UPD_CTL: u32 = 0x1E0_0630;
const UPD_FS: u32 = 0x1E0_0AF0;
const UPD_SPK: u32 = 0x1E0_0B90;
const UPD_RING: u32 = 0x1E0_0E00;
const UPD_SUB: u32 = 0x1E0_13C0;
const UPD_STR: u32 = 0x1E0_16C0;
const DRW_SPK: u32 = 0x1E0_3280;
const DRW_RING: u32 = 0x1E0_3390;
const DRW_STR: u32 = 0x1E0_38A0;
const DRW_SUB: u32 = 0x1E0_3BA0;
const DRW_FS: u32 = 0x1E0_3EA0;

/// Primitive families, for [`C021::counts`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Sparkle = 0,
    Ring,
    Streak,
    Shadow,
    Screen,
}

// ---------------------------------------------------------------- random

#[derive(Clone, Debug, PartialEq)]
struct Rng {
    seed: u64,
    calls: u32,
}

impl Rng {
    fn next(&mut self) -> i32 {
        self.calls += 1;
        self.seed = self.seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
        ((self.seed >> 32) & 0x7FFF_FFFF) as i32
    }
}

// ---------------------------------------------------------------- data area

#[derive(Clone, Debug, PartialEq)]
struct Mem(Vec<u8>);

impl Mem {
    fn h(&self, o: usize) -> i32 {
        i32::from(i16::from_le_bytes([self.0[o], self.0[o + 1]]))
    }
    fn sh(&mut self, o: usize, v: i32) {
        self.0[o..o + 2].copy_from_slice(&(v as i16).to_le_bytes());
    }
    fn w(&self, o: usize) -> i32 {
        i32::from_le_bytes(self.0[o..o + 4].try_into().unwrap())
    }
    fn sw(&mut self, o: usize, v: i32) {
        self.0[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn uw(&self, o: usize) -> u32 {
        u32::from_le_bytes(self.0[o..o + 4].try_into().unwrap())
    }
    fn suw(&mut self, o: usize, v: u32) {
        self.0[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn f(&self, o: usize) -> f32 {
        f32::from_le_bytes(self.0[o..o + 4].try_into().unwrap())
    }
    fn sf(&mut self, o: usize, v: f32) {
        self.0[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn b(&self, o: usize) -> u8 {
        self.0[o]
    }
    fn sb(&mut self, o: usize, v: i32) {
        self.0[o] = v as u8;
    }
    fn col(&self, o: usize) -> [u8; 4] {
        [self.0[o], self.0[o + 1], self.0[o + 2], self.0[o + 3]]
    }
    fn setrgb(&mut self, o: usize, r: u8, g: u8, b: u8) {
        self.0[o] = r;
        self.0[o + 1] = g;
        self.0[o + 2] = b;
    }
    fn copy(&mut self, dst: usize, src: usize, n: usize) {
        self.0.copy_within(src..src + n, dst);
    }
}

fn ptr(o: usize) -> u32 {
    DB + o as u32
}

fn off(p: u32) -> Option<usize> {
    if p == 0 { None } else { Some(p.wrapping_sub(DB) as usize) }
}

fn s16(x: i32) -> i32 {
    i32::from(x as i16)
}

/// 0x1A2C70: f32 -> unsigned (0 below 0, saturating).
fn u32f(x: f32) -> u32 {
    if !(x >= 0.0) {
        0
    } else if x < 4_294_967_296.0 {
        x as u32
    } else {
        u32::MAX
    }
}

// ---------------------------------------------------------------- trig

fn degree_table() -> [f32; 91] {
    std::array::from_fn(|i| f64::from((std::f32::consts::PI * i as f32) / 180.0f32).sin() as f32)
}

fn norm_deg(d: i32) -> usize {
    let mut d = s16(d);
    if d >= 360 || d < -359 {
        d = s16(d % 360);
    }
    if d < 0 {
        d = s16(d + 360);
    }
    d as usize
}

fn dsin(t: &[f32; 91], d: i32) -> f32 {
    let d = norm_deg(d);
    if d < 90 {
        t[d]
    } else if d < 180 {
        t[180 - d]
    } else if d < 270 {
        -t[d - 180]
    } else {
        -t[360 - d]
    }
}

fn dcos(t: &[f32; 91], d: i32) -> f32 {
    let d = norm_deg(d);
    if d < 90 {
        t[90 - d]
    } else if d < 180 {
        -t[d - 90]
    } else if d < 270 {
        -t[270 - d]
    } else {
        t[d - 270]
    }
}

type Mat = [[f64; 4]; 4];

fn rot16(a: i32, b: i32, c: i32) -> [[f64; 3]; 3] {
    let cs = |x: i32| {
        let t = 2.0 * std::f64::consts::PI * f64::from(x) / 65536.0;
        (t.cos(), t.sin())
    };
    let ((ca, sa), (cb, sb), (cc, sc)) = (cs(a), cs(b), cs(c));
    [
        [cb * cc, cb * sc, -sb],
        [sa * sb * cc - ca * sc, sa * sb * sc + ca * cc, sa * cb],
        [ca * sb * cc + sa * sc, ca * sb * sc - sa * cc, ca * cb],
    ]
}

/// 0x20C0B0 TRS(scale, rot, pos) (the projector applies VP).
fn trs(scale: [f64; 3], rot: [i32; 3], pos: Option<[f64; 3]>) -> Mat {
    let r = rot16(rot[0], rot[1], rot[2]);
    let mut m = [[0.0; 4]; 4];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = r[i][j] * scale[i];
        }
    }
    m[3] = [0.0, 0.0, 0.0, 1.0];
    if let Some(p) = pos {
        m[3][..3].copy_from_slice(&p);
    }
    m
}

fn apply(m: &Mat, p: [f32; 3]) -> [f64; 3] {
    let v = [f64::from(p[0]), f64::from(p[1]), f64::from(p[2]), 1.0];
    std::array::from_fn(|j| (0..4).map(|k| v[k] * m[k][j]).sum())
}

/// 0x108DE0.
fn uvset(r: [i32; 4], mode: u8) -> [[i32; 2]; 4] {
    let [u, v, w, h] = r;
    if mode == 0 { [[u, v], [u + w, v], [u, v + h], [u + w, v + h]] } else { [[u, v + h], [u, v], [u + w, v + h], [u + w, v]] }
}

// ---------------------------------------------------------------- inputs / outputs

/// A unit as the module reads it (0x1F6D00 / 0x1F6DE0 / 0x1F6EB0 / 0x1F7050).
#[derive(Clone, Debug, PartialEq)]
pub struct UnitIn {
    /// Group << 8 | slot.
    pub handle: u16,
    /// The card object's position (its cell centre), w 1.
    pub position: [f32; 3],
    pub rotation: [i32; 3],
    pub colour: [u8; 4],
    /// A card object exists (0x1F6D00; leaders have none).
    pub card: bool,
    /// The unit shows a 3D model (0x1F70D0: leaders, face-up monsters).
    pub model: bool,
}

/// A card object's pose this update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardState {
    pub position: [f32; 3],
    pub rotation: [i32; 3],
    pub colour: [u8; 4],
    pub scale: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct UnitSt {
    present: bool,
    card: bool,
    pos: [f32; 4],
    rot: [i32; 3],
    col: [u8; 4],
    scale: f32,
    model: Option<f32>,
}

/// The module handle of index i (0..21): i < 11 -> g0 slot i, else g1 slot i - 11.
pub fn handle(i: usize) -> u16 {
    if i < 11 { i as u16 } else { i as u16 + 0xF5 }
}

/// The mask bit of (side, slot): bit 21 - (11 side + slot) (0x250C90 order).
pub fn mask_bit(side: u8, slot: u8) -> u32 {
    1 << (21 - (11 * u32::from(side) + u32::from(slot)))
}

#[derive(Clone, Debug, PartialEq)]
struct Prim {
    c: [[u8; 4]; 4],
    uv: [[i32; 2]; 4],
}

// ---------------------------------------------------------------- module

#[derive(Clone, Debug, PartialEq)]
pub struct C021 {
    m: Mem,
    rng: Rng,
    tab: [f32; 91],
    units: Vec<UnitSt>,
    prim: Prim,
    pts: [[f32; 3]; 4],
    cam: [i32; 3],
    cam_override: Option<[i32; 3]>,
    light: u8,
    counter: u32,
    done: bool,
    counts: [u32; 5],
    /// Additive triangles of the last update (slot order).
    pub additive: Vec<FxTri>,
    /// Subtractive triangles (Cd - Cs As / 128): the shadow annuli.
    pub subtractive: Vec<FxTri>,
}

impl C021 {
    /// Init (`entry(D, param)`, param 0 as the handler passes): `mask` = the
    /// record's unit mask, `units` = every unit on the board (absent handles
    /// read zeros), `seed` = the LCG state.
    pub fn new(mask: u32, units: &[UnitIn], seed: u64) -> Self {
        let tab = degree_table();
        let mut c = Self {
            m: Mem(vec![0; 0x3910]),
            rng: Rng { seed, calls: 0 },
            tab,
            units: Vec::new(),
            prim: Prim { c: [[0; 4]; 4], uv: [[0; 2]; 4] },
            pts: [[0.0; 3]; 4],
            cam: [-6144, 0, 0],
            cam_override: None,
            light: 128,
            counter: 0,
            done: false,
            counts: [0; 5],
            additive: Vec::new(),
            subtractive: Vec::new(),
        };
        let pi = std::f32::consts::PI;
        for i in 0..18 {
            let a = (pi * (20 * i) as f32) / 180.0f32;
            let a2 = (pi * (20 * i + 20) as f32) / 180.0f32;
            c.m.sf(S0T + 4 * i, f64::from(a).sin() as f32);
            c.m.sf(C0T + 4 * i, f64::from(a).cos() as f32);
            c.m.sf(S1T + 4 * i, f64::from(a2).sin() as f32);
            c.m.sf(C1T + 4 * i, f64::from(a2).cos() as f32);
        }
        c.m.sh(0x38BA, 0);
        c.m.sw(0x3578, 0xCC);
        c.m.sh(0x38B6, 0);
        for i in 0..22 {
            let hn = handle(i);
            let u = units.iter().find(|u| u.handle == hn);
            let r = U0 + US * i;
            let v = c.rng.next();
            c.m.sf(r + 0x54, ((v % 33 + 6) / 6) as f32);
            let v = c.rng.next();
            c.m.sw(r + 0x28, v % 6 + 2);
            let v = c.rng.next();
            c.m.sw(r + 0x34, v % 0x1800 + 0x100);
            let v = c.rng.next();
            c.m.sw(r + 0x38, (v & 0x7FF) - 0x400);
            let (pos, rot, col, card, model) = match u {
                Some(u) => ([u.position[0], 0.0, u.position[2], 1.0], u.rotation, u.colour, u.card, u.model),
                None => ([0.0, 0.0, 0.0, 1.0], [0; 3], [0; 4], false, false),
            };
            for k in 0..4 {
                c.m.sf(r + 4 * k, pos[k]);
            }
            c.m.sw(r + 0x24, 0);
            for k in 0..3 {
                c.m.sw(0x31B0 + 16 * i + 4 * k, rot[k]);
            }
            let mut colour = col;
            colour[3] = 0x80;
            c.m.0[r + 0x60..r + 0x64].copy_from_slice(&colour);
            for k in 0..3 {
                c.m.sf(r + 0x40 + 4 * k, 0.1);
            }
            c.units.push(UnitSt { present: u.is_some(), card, pos, rot, col, scale: 0.1, model: model.then_some(1.0) });
        }
        c.m.sh(0x38AE, 0x80);
        c.m.sw(0x38A0, mask as i32);
        c
    }

    // ------------------------------------------------------------ accessors

    pub fn done(&self) -> bool {
        self.done
    }

    /// The update counter (= u of the last update).
    pub fn counter(&self) -> u32 {
        self.counter
    }

    /// The board light written this update (0x1F7F00).
    pub fn light(&self) -> u8 {
        self.light
    }

    pub fn rand_calls(&self) -> u32 {
        self.rng.calls
    }

    /// Life left (D+0x3578): the module ends when it reaches 0.
    pub fn life(&self) -> i32 {
        self.m.w(0x3578)
    }

    /// Quads per [`Layer`] in the last update.
    pub fn counts(&self) -> [u32; 5] {
        self.counts
    }

    /// The vortex speed (D+0x3898).
    pub fn speed(&self) -> f32 {
        self.m.f(0x3898)
    }

    /// Force the billboard angles (0x205580) instead of the eye / target's.
    pub fn set_camera_angles(&mut self, a: Option<[i32; 3]>) {
        self.cam_override = a;
    }

    /// The card object of `handle` (None for a handle without one).
    pub fn card(&self, handle: u16) -> Option<CardState> {
        let i = (0..22).find(|&i| self::handle(i) == handle)?;
        let u = &self.units[i];
        (u.present && u.card).then_some(CardState { position: [u.pos[0], u.pos[1], u.pos[2]], rotation: u.rot, colour: u.col, scale: u.scale })
    }

    /// The 3D model scale of `handle` (None without a model).
    pub fn model_scale(&self, handle: u16) -> Option<f32> {
        let i = (0..22).find(|&i| self::handle(i) == handle)?;
        self.units[i].model
    }

    fn bit(&self, i: usize) -> bool {
        (self.m.w(0x38A0) >> (21 - i)) & 1 != 0
    }

    // ------------------------------------------------------------ spawners

    fn alloc(&mut self) -> Option<usize> {
        for k in 0..SLN {
            let o = SL0 + SLS * k;
            if self.m.h(o + 0x84) == 0 {
                self.m.0[o..o + SLS].fill(0);
                self.m.sh(o + 0x84, k as i32 + 1);
                return Some(o);
            }
        }
        None
    }

    fn colours_from_spawn(&mut self, o: usize, alpha: i32) {
        let m = &mut self.m;
        let (r, g, b) = (m.b(0x357C), m.b(0x357D), m.b(0x357E));
        m.setrgb(o + 0x7C, r, g, b);
        m.sb(o + 0x7F, alpha);
        m.setrgb(o + 0x80, r, g, b);
        m.sb(o + 0x83, alpha);
    }

    /// 0x1E02820: the spine controller.
    fn spawn_ctl(&mut self) {
        if let Some(o) = self.alloc() {
            self.m.suw(o + 0x74, UPD_CTL);
        }
    }

    /// 0x1E028F0: the full-screen quad.
    fn spawn_fs(&mut self) {
        if let Some(o) = self.alloc() {
            self.colours_from_spawn(o, 0);
            self.m.suw(o + 0x74, UPD_FS);
            self.m.suw(o + 0x70, DRW_FS);
        }
    }

    /// 0x1E029F0: a sparkle (5 random values).
    fn spawn_spk(&mut self) {
        let Some(o) = self.alloc() else {
            return;
        };
        self.colours_from_spawn(o, 0);
        let m = &mut self.m;
        m.copy(o + 0x30, 0xC0, 0x10);
        m.sb(o + 0x86, u32f(m.f(0x3590)) as i32);
        m.sb(o + 0x87, u32f(m.f(0x3594)) as i32);
        m.sf(o + 0x78, m.f(0x3598));
        let r = self.rng.next();
        let m = &mut self.m;
        m.sh(o + 0x8E, r & 3);
        m.sw(o + 0x24, m.w(0x3344));
        let r = self.rng.next();
        self.m.sh(o + 0x8A, r % 10 + 5);
        self.m.suw(o + 0x74, UPD_SPK);
        self.m.suw(o + 0x70, DRW_SPK);
        self.rng.next();
        let r = self.rng.next();
        self.m.sf(o + 0x54, (r % 9 + 3) as f32);
        let r = self.rng.next();
        self.m.sw(o + 0x24, s16(r % 360) * 182);
    }

    fn spawn_common(&mut self, o: usize) {
        let a = i32::from(self.m.b(0x357F));
        self.colours_from_spawn(o, a);
        let m = &mut self.m;
        m.copy(o + 0x30, 0xC0, 0x10);
        m.sb(o + 0x86, u32f(m.f(0x3590)) as i32);
        m.sb(o + 0x87, u32f(m.f(0x3594)) as i32);
        m.sf(o + 0x78, m.f(0x3598));
        m.sh(o + 0x8C, m.h(0x38A6));
        m.suw(o + 0x68, m.uw(0x3588));
        m.sh(o + 0x88, m.h(0x38B4));
    }

    /// 0x1E02BE0: a ring (copies the previous link's centre).
    fn spawn_ring(&mut self) {
        let Some(o) = self.alloc() else {
            return;
        };
        self.spawn_common(o);
        let m = &mut self.m;
        if matches!(m.h(o + 0x88), 0 | 1) {
            let y0 = m.f(0x3598);
            let y1 = y0 + 18.0;
            m.sf(o + 0x60, y0);
            m.sf(o + 0x64, if y1 <= 180.0 { y1 } else { 180.0 });
        }
        m.sh(o + 0x92, m.h(0x38C4));
        m.suw(o + 0x68, m.uw(0x3588));
        if let Some(p) = off(m.uw(o + 0x68)) {
            m.copy(o, p, 0x10);
            m.copy(o + 0x40, p + 0x40, 0x10);
            m.suw(p + 0x6C, ptr(o));
        }
        m.suw(o + 0x70, DRW_RING);
        m.suw(o + 0x74, UPD_RING);
        m.sh(o + 0x8E, m.h(0x38B0));
        m.sh(o + 0x90, 1);
        m.suw(0x3588, ptr(o));
    }

    /// 0x1E02E40 (shadow, clamp 200) / 0x1E03060 (streak, clamp 180).
    fn spawn_chain(&mut self, upd: u32, drw: u32, clamp: f32) {
        let Some(o) = self.alloc() else {
            return;
        };
        self.spawn_common(o);
        let m = &mut self.m;
        let y0 = m.f(0x3598);
        m.sf(o + 0x60, y0);
        m.sf(o + 0x64, y0 + 18.0);
        if clamp < m.f(o + 0x60) {
            m.sf(o + 0x60, clamp);
        }
        if clamp < m.f(o + 0x64) {
            m.sf(o + 0x64, clamp);
        }
        m.sh(o + 0x92, m.h(0x38C4));
        m.suw(o + 0x68, m.uw(0x3588));
        if let Some(p) = off(m.uw(o + 0x68)) {
            m.suw(p + 0x6C, ptr(o));
        }
        m.suw(o + 0x70, drw);
        m.suw(o + 0x74, upd);
        m.sh(o + 0x8E, m.h(0x38B0));
        m.suw(0x3588, ptr(o));
    }

    fn toggle_c4(&mut self) {
        let v = if self.m.h(0x38C4) == 0 { 1 } else { 0 };
        self.m.sh(0x38C4, v);
    }

    /// 0x1E00080: the counter, the first update's set-up, the speed and the
    /// sparkles.
    fn spawner(&mut self) {
        self.m.setrgb(0x357C, 0x30, 0x30, 0x30);
        let cnt = self.m.h(0x38BA) + 1;
        self.m.sh(0x38BA, cnt);
        if cnt == 1 {
            for i in 0..12 {
                self.m.setrgb(0x38D0 + 4 * i, 0xA0, 0, 0xA0);
                self.m.sb(0x38D3 + 4 * i, 0x80);
            }
            self.m.sf(0x3898, 0.5);
            for (o, base) in [(0x38C6, 50), (0x38C8, 130), (0x38CA, 210), (0x38CC, 270)] {
                let r = self.rng.next();
                self.m.sh(o, r % 60 + base);
            }
            self.spawn_ctl();
            let mut y = 0.0f32;
            self.m.sw(SP0 + 4, 0);
            for k in 1..10 {
                y += 30.0;
                self.m.sf(SP0 + 0x18 * k + 4, y);
            }
            for (k, r) in [(0, 3), (1, 3), (2, 2), (3, 2), (4, 2), (5, 2)] {
                self.m.sh(SP0 + 0x18 * k + 0x16, r);
            }
            self.m.sf(0x3590, 10.0);
            self.m.sw(0x3588, 0);
            for k in 0..10 {
                self.toggle_c4();
                self.m.sf(0x3590, (1 + 11 * k) as f32);
                self.m.sf(0x3594, (1 + 12 * k) as f32);
                self.m.sh(0x38B0, i32::from(k == 9));
                self.m.sf(0x3598, (18 * k) as f32);
                self.m.sh(0x38B4, 1);
                self.m.setrgb(0x357C, 0x50, 0x50, 0xC0);
                self.m.sb(0x357F, 0);
                self.zero_c0();
                self.m.sh(0x38A6, 10 * k as i32);
                self.spawn_ring();
            }
            self.m.setrgb(0x357C, 0x40, 0x40, 0x40);
            self.m.sw(0x3588, 0);
            for k in 0..15 {
                self.m.sf(0x3590, (1 + 10 * k) as f32);
                self.m.sf(0x3594, (1 + 11 * k) as f32);
                self.m.sh(0x38B0, i32::from(k == 14));
                self.m.sf(0x3598, (13 * k) as f32);
                self.m.sh(0x38B4, 1);
                self.m.sb(0x357F, 0);
                self.zero_c0();
                self.m.sh(0x38A6, 15 * k as i32);
                self.spawn_chain(UPD_STR, DRW_STR, 180.0);
            }
            self.m.setrgb(0x357C, 0x10, 0x10, 0x10);
            self.m.sw(0x3588, 0);
            for k in 0..5 {
                self.m.sf(0x3590, 30.0);
                self.m.sf(0x3594, 30.0);
                self.m.sh(0x38B0, i32::from(k == 4));
                self.m.sf(0x3598, (25 * k) as f32);
                self.m.sh(0x38B4, 1);
                self.zero_c0();
                self.m.sb(0x357F, 0);
                self.m.sh(0x38A6, 15 * k as i32);
                self.toggle_c4();
                self.spawn_chain(UPD_SUB, DRW_SUB, 200.0);
            }
            self.m.setrgb(0x357C, 0x20, 0x20, 0x20);
            self.m.sb(0x357F, 0);
            self.spawn_fs();
        }
        let cnt = self.m.h(0x38BA);
        if cnt >= 0 && cnt & 1 == 1 && self.m.f(0x3898) < 4.0 {
            let s = self.m.f(0x3898) + 0.1;
            self.m.sf(0x3898, s);
        }
        if cnt % 3 == 1 && cnt < 0x87 {
            self.m.setrgb(0x357C, 0x80, 0x80, 0x80);
            let r = self.rng.next();
            self.m.sf(0xC0, (r % 70 + 80) as f32);
            let r = self.rng.next();
            self.m.sf(0xC4, (r % 50 + 60) as f32);
            self.m.sf(0xC8, 0.0);
            let r = self.rng.next();
            self.m.sh(0x38A6, r % 360);
            self.m.sw(0x3344, 0);
            self.m.sb(0x357F, 0);
            self.spawn_spk();
        }
    }

    fn zero_c0(&mut self) {
        self.m.sf(0xC0, 0.0);
        self.m.sf(0xC4, 0.0);
        self.m.sf(0xC8, 0.0);
    }

    // ------------------------------------------------------------ updates

    /// 0x1E00630: the spine wobble (entry 5's wrap test reads entry 0).
    fn upd_ctl(&mut self, o: usize) {
        let cnt = self.m.h(0x38BA);
        let spec: [(usize, i32, Option<i32>, bool); 6] =
            [(0, 3, Some(15), false), (1, -8, Some(14), false), (2, 11, Some(14), false), (3, -12, Some(13), true), (4, 18, Some(13), false), (5, -9, None, true)];
        for (k, da, cap, reset) in spec {
            let e = SP0 + 0x18 * k;
            let a = self.m.h(e + 0x14);
            let r = self.m.h(e + 0x16) as f32;
            let (c, s) = (dcos(&self.tab, a), dsin(&self.tab, a));
            self.m.sf(e, c * r);
            self.m.sf(e + 8, s * r);
            let a = s16(a + da);
            self.m.sh(e + 0x14, a);
            match k {
                0 if a >= 360 => self.m.sh(e + 0x14, a % 360),
                5 => {
                    let a0 = self.m.h(SP0 + 0x14);
                    if a0 < -359 {
                        self.m.sh(SP0 + 0x14, a0 % 360);
                    }
                }
                1..=4 if a < -359 => self.m.sh(e + 0x14, a % 360),
                _ => {}
            }
            if reset {
                self.m.sh(e + 0x16, 10);
            }
            if let Some(cap) = cap {
                if cnt % 14 == 1 && self.m.h(e + 0x16) < cap {
                    let v = self.m.h(e + 0x16) + 1;
                    self.m.sh(e + 0x16, v);
                }
            }
        }
        if self.m.w(0x3578) < 16 {
            self.m.sh(o + 0x84, 0);
        }
    }

    /// 0x1E00AF0 (its fade-in never runs: the updates start at counter 9).
    fn upd_fs(&mut self, o: usize) {
        let m = &mut self.m;
        if m.w(0x3578) < 0x12 {
            for k in [0x7F, 0x83] {
                if m.b(o + k) >= 0x11 {
                    m.sb(o + k, i32::from(m.b(o + k)) - 0xF);
                }
            }
        }
        if m.w(0x3578) < 0xB {
            m.sh(o + 0x84, 0);
        }
    }

    /// 0x1E00B90: a sparkle.
    fn upd_spk(&mut self, o: usize) {
        let m = &mut self.m;
        let v = (m.w(o + 0x24) as f32 - m.f(o + 0x54)) as i32;
        m.sw(o + 0x24, v);
        if !(v < 361) || !(v >= -360) {
            m.sw(o + 0x24, v % 360);
        }
        m.sf(o + 0x34, m.f(o + 0x34) - 1.0);
        if !(m.f(o + 0x30) <= 0.0) {
            m.sf(o + 0x30, m.f(o + 0x30) - 2.0);
        }
        let a0 = o + 0x7F;
        let a1 = o + 0x83;
        if m.w(0x3578) < 0xD {
            if m.b(a0) >= 0x1A {
                m.sb(a0, i32::from(m.b(a0)) - 0x19);
            }
            if m.b(a1) >= 0x1A {
                m.sb(a1, i32::from(m.b(a1)) - 0x19);
            } else {
                m.sh(o + 0x84, 0);
            }
        } else if m.f(o + 0x34) < 50.0 || m.f(o + 0x30) < 8.0 {
            if m.b(a0) >= 0x11 {
                m.sb(a0, i32::from(m.b(a0)) - 0xA);
            }
            if m.b(a1) >= 0x11 {
                m.sb(a1, i32::from(m.b(a1)) - 0xA);
            } else {
                m.sh(o + 0x84, 0);
            }
        } else {
            if m.b(a0) < 0x60 {
                m.sb(a0, i32::from(m.b(a0)) + 5);
            }
            if m.b(a1) < 0x60 {
                m.sb(a1, i32::from(m.b(a1)) + 5);
            }
        }
        if m.w(0x3578) < 0xB {
            m.sh(o + 0x84, 0);
        }
    }

    /// 0x10A1B0 against the spine segment above the object's y0: the
    /// object's centre (+0x40, copied to +0x00). Returns the segment.
    fn project_line(&mut self, o: usize) -> Option<usize> {
        let m = &mut self.m;
        let qy = m.f(o + 0x60);
        for s0 in 1..8 {
            let b = SP0 + 0x18 * s0;
            if m.f(b + 4) <= qy {
                continue;
            }
            let a = SP0 + 0x18 * (s0 - 1);
            let (px, pz) = (m.f(SP0), m.f(SP0 + 8));
            let f7 = m.f(a) + px;
            let f9 = m.f(a + 8) + pz;
            let f8 = m.f(a + 4);
            let f3 = (m.f(b) + px) - f7;
            let f4 = m.f(b + 4) - f8;
            let f5 = (m.f(b + 8) + pz) - f9;
            let acc = f64::from(f3 * f3) + f64::from(f4 * f4);
            let l2 = (acc + f64::from(f5) * f64::from(f5)) as f32;
            let (x, y, z) = if l2 == 0.0 {
                (0.0, m.f(o + 0x44), m.f(o + 0x48))
            } else {
                let acc = f64::from(f3 * (0.0 - f7)) + f64::from(f4 * (qy - f8));
                let num = (acc + f64::from(f5) * f64::from(0.0 - f9)) as f32;
                let t = num / l2;
                (f7 + f3 * t, 0.0, f9 + f5 * t)
            };
            let w = m.f(o + 0x4C);
            for (c, v) in [x, y, z, w].into_iter().enumerate() {
                m.sf(o + 0x40 + 4 * c, v);
                m.sf(o + 4 * c, v);
            }
            return Some(s0);
        }
        None
    }

    fn shrink_radii(&mut self, o: usize, s0: Option<usize>) {
        let m = &mut self.m;
        if let Some(s0) = s0 {
            if m.h(0x38BA) >= 8 {
                if usize::from(m.b(o + 0x86)) >= 10 * s0 + 5 {
                    m.sb(o + 0x86, i32::from(m.b(o + 0x86)) - 3);
                }
                if usize::from(m.b(o + 0x87)) >= 11 * s0 + 5 {
                    m.sb(o + 0x87, i32::from(m.b(o + 0x87)) - 3);
                }
            }
        }
        let sp = m.f(0x3898);
        for k in [0x86, 0x87] {
            if m.b(o + k) >= 4 {
                if sp < 2.0 {
                    m.sb(o + k, i32::from(m.b(o + k)) - 1);
                } else {
                    m.sb(o + k, u32f(f32::from(m.b(o + k)) - sp) as i32);
                }
            }
        }
    }

    fn flow(&mut self, o: usize, gap: f32, step: f32) {
        let m = &mut self.m;
        if !(m.f(o + 0x60) <= 0.0) {
            m.sf(o + 0x60, m.f(o + 0x60) - step);
            if !(m.f(o + 0x64) - m.f(o + 0x60) < gap) {
                m.sf(o + 0x64, m.f(o + 0x64) - step);
            }
        } else {
            m.sf(o + 0x64, m.f(o + 0x64) - step);
            if m.f(o + 0x64) < m.f(o + 0x60) {
                m.sf(o + 0x64, m.f(o + 0x60));
                if let Some(n) = off(m.uw(o + 0x6C)) {
                    m.suw(n + 0x68, 0);
                }
                m.sh(o + 0x84, 0);
            }
        }
    }

    fn fade_alpha(&mut self, o: usize, up: i32, cap: u8) {
        let m = &mut self.m;
        if m.w(0x3578) < 0x12 {
            for k in [0x7F, 0x83] {
                if m.b(o + k) >= 0x11 {
                    m.sb(o + k, i32::from(m.b(o + k)) - 0x10);
                }
            }
        } else {
            for k in [0x7F, 0x83] {
                if m.b(o + k) < cap {
                    m.sb(o + k, i32::from(m.b(o + k)) + up);
                }
            }
        }
        if m.w(0x3578) < 0xB {
            m.sh(o + 0x84, 0);
        }
    }

    fn thin_fade(&mut self, o: usize) {
        let m = &mut self.m;
        if m.b(o + 0x87) < 6 {
            for k in [0x7F, 0x83] {
                if m.b(o + k) >= 0x11 {
                    m.sb(o + k, i32::from(m.b(o + k)) - 0x10);
                }
            }
        }
    }

    /// 0x1E00E00: a ring.
    fn upd_ring(&mut self, o: usize) {
        let a = s16(self.m.h(o + 0x8C) - 4);
        self.m.sh(o + 0x8C, a);
        if a < -359 {
            self.m.sh(o + 0x8C, a % 360);
        }
        let s0 = self.project_line(o);
        self.shrink_radii(o, s0);
        let sp = self.m.f(0x3898);
        self.flow(o, 18.0, sp);
        self.thin_fade(o);
        if self.m.h(o + 0x8E) > 0 && self.m.f(o + 0x64) < 180.0 {
            let m = &mut self.m;
            m.suw(0x3588, ptr(o));
            m.sf(0x3594, 155.0);
            m.sf(0x3590, 155.0);
            let (r, g, b) = (m.b(o + 0x7C), m.b(o + 0x7D), m.b(o + 0x7E));
            m.setrgb(0x357C, r, g, b);
            m.sb(0x357F, 10);
            m.sh(0x38B0, 1);
            m.sf(0x3598, m.f(o + 0x64));
            m.sh(0x38B4, 1);
            m.copy(0x3350, o, 0x10);
            m.sh(0x38A6, m.h(o + 0x8C));
            m.sh(0x38C4, i32::from(m.h(o + 0x92) == 0));
            self.spawn_ring();
            self.m.sh(o + 0x8E, 0);
        }
        self.fade_alpha(o, 5, 0x60);
    }

    /// 0x1E013C0: a shadow annulus (radii = its heights).
    fn upd_sub(&mut self, o: usize) {
        let a = s16(self.m.h(o + 0x8C) - 1);
        self.m.sh(o + 0x8C, a);
        if a < -359 {
            self.m.sh(o + 0x8C, a % 360);
        }
        let m = &mut self.m;
        m.sb(o + 0x86, u32f(m.f(o + 0x60)) as i32);
        m.sb(o + 0x87, u32f(m.f(o + 0x64)) as i32);
        self.flow(o, 18.0, 4.0);
        if self.m.h(o + 0x8E) > 0 && self.m.f(o + 0x64) < 200.0 {
            let m = &mut self.m;
            m.suw(0x3588, ptr(o));
            m.sf(0x3590, f32::from(m.b(o + 0x86)));
            m.sf(0x3594, f32::from(m.b(o + 0x87)));
            m.sb(0x357F, 0);
            let (r, g, b) = (m.b(o + 0x7C), m.b(o + 0x7D), m.b(o + 0x7E));
            m.setrgb(0x357C, r, g, b);
            m.sh(0x38B0, 1);
            m.sf(0x3598, m.f(o + 0x64));
            m.sh(0x38B4, 1);
            self.zero_c0();
            let m = &mut self.m;
            m.sh(0x38A6, m.h(o + 0x8C) + 15);
            m.sh(0x38C4, i32::from(m.h(o + 0x92) == 0));
            self.spawn_chain(UPD_SUB, DRW_SUB, 200.0);
            self.m.sh(o + 0x8E, 0);
        }
        self.fade_alpha(o, 4, 0x80);
    }

    /// 0x1E016C0: a streak.
    fn upd_str(&mut self, o: usize) {
        let a = s16(self.m.h(o + 0x8C) - 4);
        self.m.sh(o + 0x8C, a);
        if a >= 361 {
            self.m.sh(o + 0x8C, a % 360);
        }
        let s0 = self.project_line(o);
        self.shrink_radii(o, s0);
        let sp = self.m.f(0x3898);
        self.flow(o, 13.0, sp);
        self.thin_fade(o);
        if self.m.h(o + 0x8E) > 0 && self.m.f(o + 0x64) < 180.0 {
            let m = &mut self.m;
            m.suw(0x3588, ptr(o));
            m.sf(0x3594, 160.0);
            m.sf(0x3590, 160.0);
            m.sb(0x357F, i32::from(m.b(o + 0x7F)));
            let (r, g, b) = (m.b(o + 0x7C), m.b(o + 0x7D), m.b(o + 0x7E));
            m.setrgb(0x357C, r, g, b);
            m.sh(0x38B0, 1);
            m.sf(0x3598, m.f(o + 0x64));
            m.sh(0x38B4, 1);
            m.copy(0x3350, o, 0x10);
            self.zero_c0();
            let m = &mut self.m;
            m.sh(0x38A6, m.h(o + 0x8C) + 15);
            m.sh(0x38C4, i32::from(m.h(o + 0x92) == 0));
            if m.h(0x38BA) < 0xB4 {
                self.spawn_chain(UPD_STR, DRW_STR, 180.0);
            }
            self.m.sh(o + 0x8E, 0);
        }
        self.fade_alpha(o, 10, 0x50);
    }

    // ------------------------------------------------------------ draws

    fn emit(&mut self, sc: [[f32; 2]; 4], layer: Layer, subtract: bool, textured: bool) {
        self.counts[layer as usize] += 1;
        let p = &self.prim;
        let uv = if textured { p.uv.map(|[u, v]| [u as f32 / SHEET, v as f32 / SHEET]) } else { [[0.0; 2]; 4] };
        let c = p.c;
        let out = if subtract { &mut self.subtractive } else { &mut self.additive };
        out.push(FxTri { screen: [sc[0], sc[1], sc[2]], uv: [uv[0], uv[1], uv[2]], colour: [c[0], c[1], c[2]], textured, sheet: 15 });
        out.push(FxTri { screen: [sc[1], sc[3], sc[2]], uv: [uv[1], uv[3], uv[2]], colour: [c[1], c[3], c[2]], textured, sheet: 15 });
    }

    fn quad(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, mx: Option<&Mat>, layer: Layer, subtract: bool) {
        let mut sc = [[0.0f32; 2]; 4];
        for i in 0..4 {
            let p = self.pts[i];
            let w = match mx {
                Some(m) => apply(m, p),
                None => [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])],
            };
            match pj(w) {
                Some(v) => sc[i] = v,
                None => return,
            }
        }
        self.emit(sc, layer, subtract, true);
    }

    fn polar(&mut self, i: usize, ang: i32, rad: u8) {
        let r = f32::from(rad);
        self.pts[i][0] = r * dcos(&self.tab, s16(ang));
        self.pts[i][2] = r * dsin(&self.tab, s16(ang));
    }

    fn add4(&mut self, src: usize) {
        let v = [self.m.f(src), self.m.f(src + 4), self.m.f(src + 8)];
        for p in &mut self.pts {
            for c in 0..3 {
                p[c] += v[c];
            }
        }
    }

    /// 0x1086B0: points i0, i0 + 1: x/z += a - b.
    fn add2diff(&mut self, i0: usize, a: usize, b: usize) {
        let dx = self.m.f(a) - self.m.f(b);
        let dz = self.m.f(a + 8) - self.m.f(b + 8);
        for i in [i0, i0 + 1] {
            self.pts[i][0] += dx;
            self.pts[i][2] += dz;
        }
    }

    fn colours_chain(&mut self, o: usize, p: Option<usize>) {
        let top = match p {
            Some(p) => self.m.col(p + 0x7C),
            None => self.m.col(o + 0x80),
        };
        let bottom = if off(self.m.uw(o + 0x6C)).is_some() {
            self.m.col(o + 0x80)
        } else {
            let c = self.m.col(o + 0x7C);
            [c[0], c[1], c[2], 10]
        };
        self.prim.c = [top, top, bottom, bottom];
    }

    /// 0x1E03280: a sparkle billboard.
    fn drw_spk(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, o: usize, mx: &Mat) {
        let n = self.m.h(o + 0x8E);
        let (c0, c1) = (self.m.col(o + 0x7C), self.m.col(o + 0x80));
        self.prim.c = [c0, c0, c1, c1];
        self.prim.uv = uvset([177 + 32 * (n & 1), 65 + 32 * (n >> 1), 30, 30], 0);
        let sz = self.m.h(o + 0x8A);
        let a = (s16(sz) >> 1) as f32;
        let b = sz as f32;
        self.pts = [[-a, b, 0.0], [a, b, 0.0], [-a, -b, 0.0], [a, -b, 0.0]];
        self.quad(pj, Some(mx), Layer::Sparkle, false);
    }

    /// 0x1E03390: a ring of 18 segments.
    fn drw_ring(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, o: usize, mx: &Mat) {
        self.prim.uv = uvset(if self.m.h(o + 0x92) == 0 { [143, 0, 31, 64] } else { [143, 63, 31, 64] }, 0);
        let p = off(self.m.uw(o + 0x68));
        let (y0, y1) = (self.m.f(o + 0x60), self.m.f(o + 0x64));
        self.pts[0][1] = y0;
        self.pts[1][1] = y0;
        self.pts[2][1] = y1;
        self.pts[3][1] = y1;
        for i in 0..18 {
            let (c0, s0, c1, s1) = (self.m.f(C0T + 4 * i), self.m.f(S0T + 4 * i), self.m.f(C1T + 4 * i), self.m.f(S1T + 4 * i));
            match p {
                Some(p) => {
                    let r = f32::from(self.m.b(p + 0x87));
                    self.pts[0][0] = c0 * r;
                    self.pts[0][2] = s0 * r;
                    self.pts[1][0] = c1 * r;
                    self.pts[1][2] = s1 * r;
                    let y = self.m.f(p + 0x64);
                    self.m.sf(o + 0x60, y);
                    self.pts[0][1] = y;
                    self.pts[1][1] = y;
                    self.add2diff(0, p + 0x40, o + 0x40);
                }
                None => {
                    let r = f32::from(self.m.b(o + 0x86));
                    self.pts[0][0] = c0 * r;
                    self.pts[0][2] = s0 * r;
                    self.pts[1][0] = c1 * r;
                    self.pts[1][2] = s1 * r;
                }
            }
            let r1 = f32::from(self.m.b(o + 0x87));
            self.pts[2][0] = c0 * r1;
            self.pts[2][2] = s0 * r1;
            self.pts[3][0] = c1 * r1;
            self.pts[3][2] = s1 * r1;
            self.add2diff(2, o + 0x40, o);
            self.add4(o + 0x40);
            self.colours_chain(o, p);
            self.quad(pj, Some(mx), Layer::Ring, false);
        }
    }

    /// 0x1E038A0: five 15-degree streaks.
    fn drw_str(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, o: usize, mx: &Mat) {
        self.prim.uv = uvset([143, 0, 31, 127], 1);
        let p = off(self.m.uw(o + 0x68));
        let y1 = self.m.f(o + 0x64);
        self.pts[2][1] = y1;
        self.pts[3][1] = y1;
        let mut s4 = 0;
        for k in 0..5 {
            let ang = self.m.h(o + 0x8C);
            match p {
                Some(p) => {
                    let (pa, pr) = (self.m.h(p + 0x8C), self.m.b(p + 0x87));
                    self.polar(0, s16(s4 + pa), pr);
                    self.polar(1, s16(s4 + pa + 15), pr);
                    let y = self.m.f(p + 0x64);
                    self.m.sf(o + 0x60, y);
                    self.pts[0][1] = y;
                    self.pts[1][1] = y;
                    self.add2diff(0, p + 0x40, o + 0x40);
                }
                None => {
                    let r0 = self.m.b(o + 0x86);
                    self.polar(0, s16(s4 + ang), r0);
                    self.polar(1, s16(s4 + ang + 15), r0);
                    let y = self.m.f(o + 0x60);
                    self.pts[0][1] = y;
                    self.pts[1][1] = y;
                }
            }
            let r1 = self.m.b(o + 0x87);
            self.polar(2, s16(s4 + ang), r1);
            self.polar(3, s16(s4 + ang + 15), r1);
            self.add4(o + 0x40);
            self.add2diff(2, o + 0x40, o);
            self.colours_chain(o, p);
            self.quad(pj, Some(mx), Layer::Streak, false);
            s4 = self.m.h(0x38C6 + 2 * k);
        }
    }

    /// 0x1E03BA0: the shadow annulus (world space, no node).
    fn drw_sub(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, o: usize) {
        let subtract = self.m.h(0x38B6) == 0;
        self.prim.uv = uvset([129, 129, 127, 30], 0);
        let p = off(self.m.uw(o + 0x68));
        for q in &mut self.pts {
            q[1] = 10.0;
        }
        let mut s4 = 0;
        for k in 0..8 {
            let ang = self.m.h(o + 0x8C);
            match p {
                Some(p) => {
                    let (pa, pr) = (self.m.h(p + 0x8C), self.m.b(p + 0x87));
                    self.polar(0, s16(s4 + pa), pr);
                    self.polar(1, s16(s4 + pa + 45), pr);
                    let y = self.m.f(p + 0x64);
                    self.m.sf(o + 0x60, y);
                }
                None => {
                    let r0 = self.m.b(o + 0x86);
                    self.polar(0, s16(s4 + ang), r0);
                    self.polar(1, s16(s4 + ang + 45), r0);
                }
            }
            let r1 = self.m.b(o + 0x87);
            self.polar(2, s16(s4 + ang), r1);
            self.polar(3, s16(s4 + ang + 45), r1);
            let ce = 0x38D0 + 4 * k;
            let a0 = match p {
                Some(p) => self.m.b(p + 0x83),
                None => self.m.b(o + 0x7F),
            };
            self.m.sb(ce + 3, i32::from(a0));
            let top = self.m.col(ce);
            self.m.sb(ce + 3, i32::from(self.m.b(o + 0x83)));
            let bottom = self.m.col(ce);
            self.prim.c = [top, top, bottom, bottom];
            self.quad(pj, None, Layer::Shadow, subtract);
            s4 = s16(s4 + 45);
        }
    }

    // ------------------------------------------------------------ units

    fn model_shrink(&mut self, i: usize) {
        if let Some(s) = &mut self.units[i].model {
            *s = (*s - 0.18).max(0.0);
        }
    }

    /// 0x1E01F60: a target card spirals into the hole.
    fn unit_update(&mut self, i: usize) {
        self.model_shrink(i);
        let hn = handle(i);
        if !self.bit(i) || !self.units[i].present || !self.units[i].card || hn == 0 || hn == 0x100 {
            return;
        }
        let r = U0 + US * i;
        let m = &mut self.m;
        let mut q = [m.f(r), m.f(r + 4), m.f(r + 8)];
        if m.f(r + 4) < 100.0 {
            m.sf(r + 4, m.f(r + 4) + m.f(r + 0x54));
        } else {
            m.sw(r + 0x64, 1);
            if m.f(r + 4) < 200.0 {
                m.sf(r + 4, m.f(r + 4) + 1.2);
            }
            for (ax, offs) in [(0, 0x10), (8, 0x18)] {
                if !(m.f(r + ax) + m.f(r + offs) <= 5.0) {
                    m.sf(r + offs, m.f(r + offs) - 2.0);
                }
                if m.f(r + ax) + m.f(r + offs) < -5.0 {
                    m.sf(r + offs, m.f(r + offs) + 2.0);
                }
            }
            q[0] += m.f(r + 0x10);
            q[2] += m.f(r + 0x18);
        }
        q[1] = 100.0 * dsin(&self.tab, s16(m.f(r + 4) as i32));
        let yaw = m.w(r + 0x24) + s16(m.w(r + 0x28));
        m.sw(r + 0x24, yaw);
        let a = s16(yaw);
        let (c, s) = (dcos(&self.tab, a), dsin(&self.tab, a));
        let (x, z) = (q[0], q[2]);
        q[0] = x * c + z * -s;
        q[2] = x * s + z * c;
        let (dy, dz) = (m.w(r + 0x34), m.w(r + 0x38));
        if m.w(r + 0x64) != 0 {
            let a = m.b(r + 0x63);
            m.sb(r + 0x63, if a >= 6 { i32::from(a) - 3 } else { 0 });
        }
        if m.b(r + 0x63) != 0 {
            let v = m.h(0x38AC) + 1;
            m.sh(0x38AC, v);
        }
        let col = m.col(r + 0x60);
        let theta = m.f(r + 4);
        let life = m.w(0x3578);
        let u = &mut self.units[i];
        u.pos = [q[0], q[1], q[2], u.pos[3]];
        u.rot = [u.rot[0], u.rot[1].wrapping_add(dy), u.rot[2].wrapping_add(dz)];
        u.col = col;
        if theta <= 110.0 && life >= 10 {
            return;
        }
        if !(self.m.f(r + 0x40) <= 0.0) {
            let s = (0.1 * f64::from(dsin(&self.tab, s16(theta as i32)))) as f32;
            self.m.sf(r + 0x40, s);
        }
        let s = self.m.f(r + 0x40);
        self.m.sf(r + 0x44, s);
        self.m.sf(r + 0x48, s);
        self.units[i].scale = s;
    }

    /// 0x1E01E10: the last 9 updates put every unit back.
    fn unit_restore(&mut self, i: usize) {
        if self.m.w(0x3578) >= 10 {
            return;
        }
        let bit = self.bit(i);
        if !bit {
            if let Some(s) = &mut self.units[i].model {
                *s = (*s + 0.15).min(1.0);
            }
        }
        if !self.units[i].present || !self.units[i].card {
            return;
        }
        let r = U0 + US * i;
        let rot = [self.m.w(0x31B0 + 16 * i), self.m.w(0x31B0 + 16 * i + 4), self.m.w(0x31B0 + 16 * i + 8)];
        self.m.sf(r + 4, 0.0);
        let pos = [self.m.f(r), 0.0, self.m.f(r + 8), self.m.f(r + 12)];
        let u = &mut self.units[i];
        u.rot = rot;
        u.pos = pos;
        if !bit {
            let a = (u32::from(u.col[3]) + 0x10) & 0xFF;
            u.col[3] = if a < 0x80 { a as u8 } else { 0x80 };
            self.m.0[r + 0x60..r + 0x64].copy_from_slice(&u.col);
        }
        u.scale = 0.1;
    }

    // ------------------------------------------------------------ the entry

    /// One update (`entry(D, -1)`), returns 0 or 8 (done).
    pub fn update(&mut self, eye: [f64; 3], target: [f64; 3]) -> u32 {
        self.additive.clear();
        self.subtractive.clear();
        self.counts = [0; 5];
        if self.done {
            return 8;
        }
        self.cam = self.cam_override.unwrap_or_else(|| crate::duel_c059::camera_angles(eye, target));
        let project = projector(eye, target);
        let none = |_: [f64; 3]| -> Option<[f32; 2]> { None };
        let pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]> = match project.as_ref() {
            Some(p) => p,
            None => &none,
        };
        self.spawner();
        self.counter = self.m.h(0x38BA) as u32;
        if self.m.h(0x38BA) >= 9 {
            for k in 0..SLN {
                let o = SL0 + SLS * k;
                if self.m.h(o + 0x84) == 0 {
                    continue;
                }
                match self.m.uw(o + 0x74) {
                    UPD_CTL => self.upd_ctl(o),
                    UPD_FS => self.upd_fs(o),
                    UPD_SPK => self.upd_spk(o),
                    UPD_RING => self.upd_ring(o),
                    UPD_SUB => self.upd_sub(o),
                    UPD_STR => self.upd_str(o),
                    _ => {}
                }
            }
        }
        for k in 0..SLN {
            let o = SL0 + SLS * k;
            if self.m.h(o + 0x84) == 0 {
                continue;
            }
            let mx = if self.m.uw(o + 0x74) != UPD_SPK {
                trs([1.5, 0.5, 1.5], [self.m.w(o + 0x20), self.m.w(o + 0x24), self.m.w(o + 0x28)], None)
            } else {
                let p = [self.m.f(o + 0x30), self.m.f(o + 0x34), self.m.f(o + 0x38)];
                let a = s16(self.m.w(o + 0x24));
                let (c, s) = (dcos(&self.tab, a), dsin(&self.tab, a));
                let p = [p[0] * c + p[2] * -s, p[1], p[0] * s + p[2] * c];
                trs([1.5, f64::from(0.3f32), 1.5], self.cam, Some(p.map(f64::from)))
            };
            match self.m.uw(o + 0x70) {
                DRW_SPK => self.drw_spk(pj, o, &mx),
                DRW_RING => self.drw_ring(pj, o, &mx),
                DRW_STR => self.drw_str(pj, o, &mx),
                DRW_SUB => self.drw_sub(pj, o),
                // The full-screen quad: Cd - Cs x 0 (its alpha never rises).
                DRW_FS => self.counts[Layer::Screen as usize] += 1,
                _ => {}
            }
        }
        if self.m.h(0x38BA) < 8 {
            for i in 0..22 {
                self.model_shrink(i);
                if !self.bit(i) && self.units[i].present && self.units[i].card {
                    let a = self.units[i].col[3];
                    self.units[i].col[3] = if a >= 21 { a - 19 } else { 0 };
                }
            }
            let l = (self.m.h(0x38AE) - 12).max(0);
            self.m.sh(0x38AE, l);
            self.m.sh(0x38AC, 1);
        } else if self.m.w(0x3578) >= 11 {
            for i in 0..22 {
                self.unit_update(i);
            }
        } else {
            for i in 0..22 {
                self.unit_restore(i);
            }
            let l = (self.m.h(0x38AE) + 16).min(0x80);
            self.m.sh(0x38AE, l);
        }
        self.light = self.m.b(0x38AE);
        if self.m.h(0x38AC) == 0 && self.m.h(0x38AA) == 0 {
            self.m.sh(0x38AA, 1);
            if self.m.w(0x3578) >= 31 {
                self.m.sw(0x3578, 30);
            }
        }
        self.m.sh(0x38AC, 0);
        if self.m.w(0x3578) != 0 {
            let l = self.m.w(0x3578) - 1;
            self.m.sw(0x3578, l);
        }
        if self.m.w(0x3578) <= 0 {
            self.done = true;
            8
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EYE: [f64; 3] = [0.0, 277.785_117, 415.734_806];
    const TARGET: [f64; 3] = [0.0, 0.0, 0.0];

    fn board() -> Vec<UnitIn> {
        // 765-f3-after's board as the module reads it: the leaders (no card
        // object), g0 s1 at (2,6), g1 s1..s7.
        let card = |side: u8, slot: u8, cell: (u8, u8)| UnitIn {
            handle: (u16::from(side) << 8) | u16::from(slot),
            position: crate::duel_c059::cell_centre(cell),
            rotation: [0x4000, 0, 0],
            colour: [128, 128, 128, 128],
            card: true,
            model: false,
        };
        let mut v = vec![
            UnitIn { handle: 0, position: [0.0, 0.0, 150.0], rotation: [0; 3], colour: [128; 4], card: false, model: true },
            UnitIn { handle: 0x100, position: [-100.0, 0.0, -150.0], rotation: [0; 3], colour: [128; 4], card: false, model: true },
            card(0, 1, (2, 6)),
        ];
        for (s, cell) in [(1, (2, 4)), (2, (3, 2)), (3, (2, 3)), (4, (1, 3)), (5, (0, 3)), (6, (4, 1)), (7, (2, 1))] {
            v.push(card(1, s, cell));
        }
        v
    }

    fn run(mask: u32, seed: u64) -> (C021, Vec<(u32, [u32; 5], u8, u32)>) {
        let mut c = C021::new(mask, &board(), seed);
        c.set_camera_angles(Some([-6144, 0, 0]));
        let mut log = Vec::new();
        let mut calls = c.rand_calls();
        for u in 1..400 {
            let r = c.update(EYE, TARGET);
            log.push((u, c.counts(), c.light(), c.rand_calls() - calls));
            calls = c.rand_calls();
            if r == 8 {
                break;
            }
        }
        (c, log)
    }

    #[test]
    fn init_draws_88_random_values_and_the_first_update_12() {
        let c = C021::new(0x1003F8, &board(), 0x1234_5678);
        assert_eq!(c.rand_calls(), 88);
        let (_, log) = run(0x1003F8, 0x1234_5678);
        // u 1: 4 offsets + a sparkle (3 + 5); then a sparkle every 3 U to u 133.
        assert_eq!(log[0].3, 12);
        assert_eq!((log[1].3, log[2].3, log[3].3), (0, 0, 8));
        assert_eq!(log[132].3, 8);
        assert_eq!(log[135].3, 0, "no sparkle from u 135");
    }

    #[test]
    fn the_first_update_draws_ten_rings_fifteen_streaks_five_shadows() {
        let (_, log) = run(0x1003F8, 7);
        // [EMU] u 1: 180 ring quads, 75 streaks, 40 shadow, 1 screen, 1 sparkle.
        assert_eq!(log[0].1, [1, 180, 75, 40, 1]);
        // u 9: the first update pass spawns the next ring (198 quads) and the
        // shadow chain (96).
        assert_eq!((log[8].1[1], log[8].1[3]), (198, 96));
    }

    #[test]
    fn the_light_dims_to_44_then_returns_at_the_end() {
        let (c, log) = run(0x1003F8, 7);
        let lights: Vec<u8> = log.iter().map(|l| l.2).collect();
        assert_eq!(&lights[..8], &[116, 104, 92, 80, 68, 56, 44, 44]);
        let n = lights.len();
        assert_eq!(&lights[n - 6..], &[124, 128, 128, 128, 128, 128]);
        assert!(c.done());
    }

    #[test]
    fn the_module_ends_thirty_updates_after_the_last_target_card_vanishes() {
        for seed in [1u64, 99, 0xDEAD] {
            let mut c = C021::new(0x1003F8, &board(), seed);
            c.set_camera_angles(Some([-6144, 0, 0]));
            let mut last_visible = 0;
            let mut end = 0;
            for u in 1..400 {
                let r = c.update(EYE, TARGET);
                let visible = (0..22).any(|i| {
                    c.bit(i) && c.card(handle(i)).is_some_and(|k| k.colour[3] > 0) && u >= 8
                });
                if visible {
                    last_visible = u;
                }
                if r == 8 {
                    end = u;
                    break;
                }
            }
            assert_eq!(end, last_visible + 30, "seed {seed}");
        }
    }

    #[test]
    fn replays_the_interpreter_run_on_765() {
        // [emu] r021_765 (765-f3-after, mask 0x1003F8, overview): the game's
        // rand state at the launch, quads per layer and the light byte.
        let (c, log) = run(0x1003F8, 0x289E_0CD0_0814_92B6);
        let at = |u: usize| (log[u - 1].1, log[u - 1].2);
        assert_eq!(at(1), ([1, 180, 75, 40, 1], 116));
        assert_eq!(at(9), ([3, 198, 75, 96, 1], 44));
        assert_eq!(at(30), ([10, 198, 75, 112, 1], 44));
        assert_eq!(at(50), ([15, 216, 80, 112, 1], 44));
        assert_eq!(at(100), ([14, 216, 80, 104, 1], 44));
        assert_eq!(at(150), ([7, 216, 80, 112, 1], 44));
        assert_eq!(at(165), ([4, 216, 80, 104, 1], 44));
        assert_eq!(at(170), ([0, 0, 0, 0, 0], 76));
        assert_eq!(log.len(), 178);
        assert_eq!(at(178).1, 128);
        assert!(c.done());
    }

    #[test]
    fn others_fade_and_models_shrink_then_come_back() {
        let mut c = C021::new(mask_bit(1, 1), &board(), 3);
        c.set_camera_angles(Some([-6144, 0, 0]));
        let mut alphas = Vec::new();
        let mut leader = Vec::new();
        loop {
            let r = c.update(EYE, TARGET);
            alphas.push(c.card(0x102).unwrap().colour[3]);
            leader.push(c.model_scale(0).unwrap());
            if r == 8 {
                break;
            }
        }
        assert_eq!(&alphas[..7], &[109, 90, 71, 52, 33, 14, 0]);
        assert!((leader[5] - 0.0).abs() < 1e-6 && leader[4] > 0.0);
        assert_eq!(*alphas.last().unwrap(), 128);
        assert_eq!(*leader.last().unwrap(), 1.0);
        // The target's card was carried off and faded out.
        let t = c.card(0x101).unwrap();
        assert_eq!(t.colour[3], 0);
    }

    #[test]
    fn a_target_card_rises_on_a_sine_while_turning_round_the_centre() {
        let mut c = C021::new(mask_bit(0, 1), &board(), 5);
        c.set_camera_angles(Some([-6144, 0, 0]));
        for _ in 0..7 {
            c.update(EYE, TARGET);
        }
        let start = c.card(1).unwrap().position;
        assert_eq!(start, [-50.0, 0.0, 150.0]);
        c.update(EYE, TARGET);
        let p = c.card(1).unwrap().position;
        let r = U0 + US;
        let speed = c.m.f(r + 0x54);
        let theta = c.m.f(r + 4);
        assert_eq!(theta, speed);
        assert!((p[1] - 100.0 * dsin(&c.tab, theta as i32)).abs() < 1e-4);
        // Same distance from the centre (no pull before theta 100).
        let d = |q: [f32; 3]| (q[0] * q[0] + q[2] * q[2]).sqrt();
        assert!((d(p) - d(start)).abs() < 0.01);
    }
}
