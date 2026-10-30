//! CMODULE c059 (0x3B), the ritual presentation of event 0x38 (handler
//! 0x26D4B0), as specified in `claude-activate/C059-RITUAL.md` [F code; emu:
//! the reference model `tools/mf_c059_model.py` equals the interpreter's
//! primitives to 0.063 px, return codes, light bytes, rand calls and card
//! objects included]. 216 updates; returns 2 at u 134 (the handler removes
//! the three sacrifices) and 8 at u 216.
//!
//! A world-space particle engine anchored at the board centre: A braziers
//! on the corner cells (TEXEFF 11) with A' purple flames (TEXEFF 13), B smoke
//! wreath, C flame curtain and D red floor disc (TEXEFF 15), E fire ring
//! (TEXEFF 1), G glow discs behind the sacrifices (untextured), F
//! full-screen subtractive darkening, X two white flashes, Q falling stars
//! (TEXEFF 8), L lightning bolts and K spark rings (TEXEFF 15). The unit
//! choreography (section 5): the other cards fade and every 3D model
//! shrinks, the three sacrifices fly out, spiral to the centre and rise, the
//! result appears black at (0, 130, 0) at u 135, slides to its cell
//! (u 151-202) and tips flat (u 158-201).
//!
//! The port keeps the module's data area as a flat byte array with the
//! original offsets and raw EE pointers (as the model does), so the two
//! raw-memory quirks of the original come for free: the bolt death path's
//! `next + 0x30 = 0` write (into the following L slot, or K slot 0's x/y
//! after L slot 39), and the shared draw scratch (an L head segment's far
//! edge keeps the previous primitive's colours).
//!
//! [H / not reproduced]: the port's random values (its own LCG, the same
//! call order and count; seeding with the game's state reproduces the
//! game); `FxTri` has no sort key: each list is emitted in the original's
//! draw order (E slots, Q, L, K), the original's OT depth sort (key =
//! int(mean view w); G at the far bucket, F/X key 0) is not reproduced and
//! F lands in its own subtractive list; the 0x1F6B80 unit flags; the model
//! scale path (0x107A60 / 0x107C50) is from the code, not run.
use crate::duel_c602::projector;
use crate::duel_c621::FxTri;

/// Module length (life 0xD8).
pub const UPDATES: u32 = 216;
/// The update that returns 2 (the handler removes the sacrifices).
pub const REMOVE_U: u32 = 134;
/// The update that creates the result.
pub const RESULT_U: u32 = 135;

const DB: u32 = 0x1E1_0000;

// Function addresses, used as tags as the original stores them.
const UPD_A: u32 = 0x1E0_53B0;
const DRW_A: u32 = 0x1E0_2C10;
const UPD_D: u32 = 0x1E0_5440;
const DRW_D: u32 = 0x1E0_29A0;
const UPD_C: u32 = 0x1E0_5710;
const DRW_C: u32 = 0x1E0_2F40;
const UPD_B: u32 = 0x1E0_5A70;
const DRW_B: u32 = 0x1E0_31C0;
const UPD_G: u32 = 0x1E0_5E40;
const UPD_E: u32 = 0x1E0_63A0;
const DRW_EG: u32 = 0x1E0_6720;
const UPD_F: u32 = 0x1E0_6280;
const UPD_X: u32 = 0x1E0_62F0;
const DRW_FX: u32 = 0x1E0_6DC0;
const DRW_Q: u32 = 0x1E0_3B00;
const UPD_L: u32 = 0x1E0_5F90;
const DRW_L1: u32 = 0x1E0_3610;
const DRW_L2: u32 = 0x1E0_3D70;
const DRW_K: u32 = 0x1E0_40B0;
const UPD_K: u32 = 0x1E0_65D0;

// Pools (offsets from D).
const E0: usize = 0x290;
const EN: usize = 48;
const ES: usize = 0x58;
const Q0: usize = 0x1720;
const QN: usize = 27;
const QS: usize = 0x30;
const L0: usize = 0x1C30;
const LN: usize = 40;
const LS: usize = 0x30;
const K0: usize = 0x23B0;
const KN: usize = 10;
const KS: usize = 0x3C;
const A0: usize = 0x2E98;
const AN: usize = 35;
const AS: usize = 0x34;

/// Primitive families (draw functions), for [`C059::quad_counts`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Braziers (0x1E02C10), sent 4 times.
    A = 0,
    /// Brazier flames (0x1E02DE0).
    A2,
    /// Smoke wreath (0x1E031C0).
    B,
    /// Flame curtain (0x1E02F40).
    C,
    /// Floor disc (0x1E029A0).
    D,
    /// Fire ring (0x1E06720 / 0x1E063A0), sent twice.
    E,
    /// Glow discs (0x1E06720 / 0x1E05E40), sent twice.
    G,
    /// Darkening (0x1E06DC0 / 0x1E06280), subtractive.
    F,
    /// Flashes (0x1E06DC0 / 0x1E062F0).
    X,
    /// Falling stars (0x1E03B00).
    Q,
    /// Bolt segments 0x1E03610 (sent twice).
    L1,
    /// Bolt segments 0x1E03D70.
    L2,
    /// Spark rings (0x1E040B0).
    K,
}
pub const FAMILIES: usize = 13;

// ---------------------------------------------------------------- random

fn lcg(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
    ((*seed >> 32) & 0x7FFF_FFFF) as u32
}

/// The port's random source, counting calls (the original: newlib rand,
/// the same generator; seeding with the game's state replays the game).
#[derive(Clone, Debug, PartialEq)]
struct Rng {
    seed: u64,
    calls: u32,
}

impl Rng {
    fn next(&mut self) -> i32 {
        self.calls += 1;
        lcg(&mut self.seed) as i32
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
    fn zero(&mut self, o: usize, n: usize) {
        self.0[o..o + n].fill(0);
    }
    /// dec(a, st): a -= st if a >= lo.
    fn dec_a(&mut self, o: usize, st: i32, lo: u8) {
        if self.b(o) >= lo {
            self.sb(o, i32::from(self.b(o)) - st);
        }
    }
    /// inc(a, st, hi): a += st if a < hi (may overshoot).
    fn inc_a(&mut self, o: usize, st: i32, hi: u8) {
        if self.b(o) < hi {
            self.sb(o, i32::from(self.b(o)) + st);
        }
    }
    /// The "last spawned" pointer D+0x2A5C.
    fn last(&self) -> u32 {
        self.uw(0x2A5C)
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

// ---------------------------------------------------------------- trig

/// The 91-entry degree table sinf(pi_f32 i / 180) (0x1084F0).
fn degree_table() -> [f32; 91] {
    std::array::from_fn(|i| f64::from((std::f32::consts::PI * i as f32) / 180.0f32).sin() as f32)
}

fn norm_deg(d: i32) -> usize {
    let mut d = s16(d);
    if d >= 360 {
        d = s16(d % 360);
    }
    if d < -359 {
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

/// 0x109230 mode 0: about y by integer degrees.
fn rot_y(t: &[f32; 91], p: [f32; 3], a: i32) -> [f32; 3] {
    let (c, s) = (dcos(t, s16(a)), dsin(t, s16(a)));
    [p[0] * c + p[2] * -s, p[1], p[0] * s + p[2] * c]
}

/// 0x109230 mode 1: about x by integer degrees.
fn rot_x(t: &[f32; 91], p: [f32; 3], a: i32) -> [f32; 3] {
    let (c, s) = (dcos(t, s16(a)), dsin(t, s16(a)));
    [p[0], p[1] * c + p[2] * s, p[1] * -s + p[2] * c]
}

/// Row-vector 4 x 4 (world = [x y z 1] M).
type Mat = [[f64; 4]; 4];

/// 0x20BDA0 R(a, b, c), units of 65536 per turn.
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

/// 0x20C0B0 TRS(scale, rot, pos) (VP applied by the projector).
fn trs(scale: Option<[f64; 3]>, rot: Option<[i32; 3]>, pos: Option<[f64; 3]>) -> Mat {
    let mut m = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
    if let Some([a, b, c]) = rot {
        let r = rot16(a, b, c);
        for i in 0..3 {
            m[i][..3].copy_from_slice(&r[i]);
        }
    }
    if let Some(s) = scale {
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] *= s[i];
            }
        }
    }
    if let Some(p) = pos {
        m[3][..3].copy_from_slice(&p);
    }
    m
}

fn matmul(a: &Mat, b: &Mat) -> Mat {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..4).map(|k| a[i][k] * b[k][j]).sum()))
}

fn apply(m: &Mat, p: [f32; 3]) -> [f64; 3] {
    let v = [f64::from(p[0]), f64::from(p[1]), f64::from(p[2]), 1.0];
    std::array::from_fn(|j| (0..4).map(|k| v[k] * m[k][j]).sum())
}

fn ident() -> Mat {
    trs(None, None, None)
}

// ---------------------------------------------------------------- UVs

/// 0x106960(id, sub): (u, v, w, h).
fn uvrect(id: u32, s: i32) -> [i32; 4] {
    match id {
        0x9C => [128, 129, 127, 63],
        0x6E => [64 * s, 0, 63, 63],
        0x82 => [1 + 20 * (s % 4), 65 + 32 * (s / 4), 18, 31],
        0x97 => [143, 0, 31, 127],
        0x96 => [10, 0, 43, 127],
        0x50 => [1, 1 + 10 * s, 8, 8],
        0x0A => {
            if s < 5 {
                [1 + 24 * (s % 5), 1, 23, 31]
            } else {
                [1 + 24 * (s - 5), 33, 23, 31]
            }
        }
        _ => [0; 4],
    }
}

/// 0x108DE0.
fn uvset(r: [i32; 4], mode: u8) -> [[i32; 2]; 4] {
    let [u, v, w, h] = r;
    if mode == 0 { [[u, v], [u + w, v], [u, v + h], [u + w, v + h]] } else { [[u, v + h], [u, v], [u + w, v + h], [u + w, v]] }
}

/// TEXEFF sheet sizes (`claude-units/texeff/NNN.png`).
fn sheet_size(sheet: u8) -> [f32; 2] {
    match sheet {
        1 | 11 => [128.0, 64.0],
        8 | 13 => [128.0, 128.0],
        _ => [256.0, 256.0],
    }
}

/// The main loop's primitive scratch (shared by every draw).
#[derive(Clone, Debug, PartialEq)]
struct Prim {
    c: [[u8; 4]; 4],
    uv: [[i32; 2]; 4],
    g: bool,
    sheet: u8,
    subtract: bool,
}

impl Prim {
    /// 0x109B50: the template keeps the colours and UVs.
    fn template(&mut self) {
        self.g = false;
    }
    /// 0x108070: mode 0 all, 1 = v0 v1, 2 = v2 v3.
    fn colour(&mut self, rgba: [u8; 4], mode: u8) {
        let idx: &[usize] = match mode {
            0 => &[0, 1, 2, 3],
            1 => &[0, 1],
            _ => &[2, 3],
        };
        for &i in idx {
            self.c[i] = rgba;
        }
    }
}

// ---------------------------------------------------------------- inputs / outputs

/// One sacrifice (mask order), its card object at launch.
#[derive(Clone, Debug, PartialEq)]
pub struct Sacrifice {
    /// Group << 8 | slot.
    pub handle: u16,
    pub card_id: u16,
    /// The card object's position (its cell centre on the board).
    pub position: [f32; 3],
    /// The card object's colour (alpha is set to 128 at init).
    pub colour: [u8; 4],
    /// The unit's 3D model scale (face-up monsters), if it has one.
    pub model_scale: Option<f32>,
}

impl Sacrifice {
    /// A card on `cell` (col, row) of `side`'s `slot`: position = the cell
    /// centre (50 (c - 3), 0, 50 (r - 3)), colour (128, 128, 128, 128).
    pub fn on_cell(side: u8, slot: u8, card_id: u16, cell: (u8, u8), model: bool) -> Self {
        Self {
            handle: (u16::from(side) << 8) | u16::from(slot),
            card_id,
            position: cell_centre(cell),
            colour: [128; 4],
            model_scale: model.then_some(1.0),
        }
    }
}

/// Any other unit with a card object (leaders, monsters, spells).
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    pub handle: u16,
    pub alpha: u8,
    pub model_scale: Option<f32>,
}

/// The ritual's result.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultSpec {
    pub card_id: u16,
    /// The free monster slot 0x1E0800 takes (lowest free 1..5 of the active
    /// side); the handle group comes from the first sacrifice.
    pub slot: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RitualInput {
    /// The three sacrifices in mask order ([`mask_handles`]).
    pub sacrifices: [Sacrifice; 3],
    /// Every other unit with a card object (sacrifices excluded).
    pub others: Vec<Unit>,
    /// None when creation fails (no free monster slot).
    pub result: Option<ResultSpec>,
    /// The ritual card's cell (col, row): the result's cell.
    pub ritual_cell: (u8, u8),
}

/// A card object pose: position (world), rotation (units), colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardObj {
    pub position: [f32; 3],
    pub rotation: [i32; 3],
    pub colour: [u8; 4],
}

#[derive(Clone, Debug, PartialEq)]
struct SacState {
    handle: u16,
    card_id: u16,
    pos: [f32; 4],
    rot: [i32; 3],
    col: [u8; 4],
    model_scale: Option<f32>,
    removed: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct ResState {
    handle: u16,
    card_id: u16,
    obj: CardObj,
    save: CardObj,
}

/// The cell centre (50 (c - 3), 0, 50 (r - 3)).
pub fn cell_centre(cell: (u8, u8)) -> [f32; 3] {
    [50.0 * (f32::from(cell.0) - 3.0), 0.0, 50.0 * (f32::from(cell.1) - 3.0)]
}

/// The sacrifice handles of a mask in the module's order: bit (21 - i) for
/// i = 0..21, i < 11 -> g0 slot i, else g1 slot i - 11 (at most 3).
pub fn mask_handles(mask: u32) -> Vec<u16> {
    (0..22u16).filter(|&i| (mask >> (21 - i)) & 1 != 0).map(hnd).take(3).collect()
}

fn hnd(i: u16) -> u16 {
    if i < 11 { i } else { i + 0xF5 }
}

/// The camera angles 0x205580 returns (-pitch, 0x4000 - yaw, 0) from the
/// eye / target (polar as `duel_board::orbit_eye_polar`).
pub fn camera_angles(eye: [f64; 3], target: [f64; 3]) -> [i32; 3] {
    let d: [f64; 3] = std::array::from_fn(|i| eye[i] - target[i]);
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if dist <= 0.0 {
        return [0, 0x4000, 0];
    }
    let yaw = d[2].atan2(d[0]).rem_euclid(std::f64::consts::TAU);
    let pitch = (d[1] / dist).clamp(-1.0, 1.0).asin();
    crate::duel_versus::camera_angles([dist, yaw, pitch])
}

// ---------------------------------------------------------------- module

#[derive(Clone, Debug, PartialEq)]
pub struct C059 {
    m: Mem,
    rng: Rng,
    tab: [f32; 91],
    /// cos 0, sin 0, cos 30, sin 30 (0x108590 table entry 0).
    rt: [f32; 4],
    cam: [i32; 3],
    cam_override: Option<[i32; 3]>,
    prim: Prim,
    sc240: [f64; 3],
    pts: [[f32; 3]; 4],
    nodes: [[Mat; 5]; 3],
    sacs: Vec<SacState>,
    others: Vec<Unit>,
    result_spec: Option<ResultSpec>,
    ritual_cell: (u8, u8),
    result: Option<ResState>,
    light: u8,
    counter: u32,
    done: bool,
    counts: [u32; FAMILIES],
    /// Additive triangles of the last update in the original's draw order
    /// (everything but F).
    pub additive: Vec<FxTri>,
    /// Subtractive triangles (F: destination - source x alpha / 128).
    pub subtractive: Vec<FxTri>,
    /// Always empty (c059 has no opaque primitive); kept for the convention.
    pub opaque: Vec<FxTri>,
}

impl C059 {
    /// Init (`entry(D, param)`), `seed` = the LCG state (the game's rand
    /// state replays the game).
    pub fn new(input: RitualInput, seed: u64) -> Self {
        let tab = degree_table();
        let r30 = (std::f32::consts::PI * 30.0f32) / 180.0f32;
        let r0 = 0.0f32;
        let rt = [f64::from(r0).cos() as f32, f64::from(r0).sin() as f32, f64::from(r30).cos() as f32, f64::from(r30).sin() as f32];
        let mut c = Self {
            m: Mem(vec![0; 0x4000]),
            rng: Rng { seed, calls: 0 },
            tab,
            rt,
            cam: [0, 0, 0],
            cam_override: None,
            prim: Prim { c: [[0; 4]; 4], uv: [[0; 2]; 4], g: false, sheet: 15, subtract: false },
            sc240: [1.0; 3],
            pts: [[0.0; 3]; 4],
            nodes: [[ident(); 5]; 3],
            sacs: Vec::new(),
            others: input.others,
            result_spec: input.result,
            ritual_cell: input.ritual_cell,
            result: None,
            light: 128,
            counter: 0,
            done: false,
            counts: [0; FAMILIES],
            additive: Vec::new(),
            subtractive: Vec::new(),
            opaque: Vec::new(),
        };
        let m = &mut c.m;
        for i in 0..4 {
            m.sw(0x16D8 + 16 * i, 0x2000 * i as i32);
        }
        m.sf(0x2A88, 0.06);
        m.sh(0x2A9E, 0);
        let (col, row) = c.ritual_cell;
        m.sh(0x2D26, (i32::from(row) << 8) | i32::from(col));
        m.sw(0x2A50, 0xD8);
        for (k, t) in [[-150.0f32, 130.0, 0.0], [150.0, 130.0, 150.0], [150.0, 130.0, -150.0]].iter().enumerate() {
            for (i, v) in t.iter().enumerate() {
                m.sf(0x29B0 + 16 * k + 4 * i, *v);
            }
        }
        for (k, s) in input.sacrifices.iter().enumerate() {
            m.sw(0x2CAC + 4 * k, i32::from(s.handle));
        }
        for (k, s) in input.sacrifices.iter().enumerate() {
            let r = 0x2650 + 0x70 * k;
            let m = &mut c.m;
            m.sw(r + 0x64, 0);
            let v = c.rng.next();
            m.sf(r + 0x54, ((v % 26 + 9) / 6) as f32);
            let v = c.rng.next();
            m.sw(r + 0x28, (v & 7) + 5);
            let v = c.rng.next();
            m.sw(r + 0x34, v % 0x1800 + 0x100);
            let v = c.rng.next();
            m.sw(r + 0x38, (v & 0x7FF) - 0x400);
            let pos = [s.position[0], 0.0, s.position[2], 1.0];
            for (i, v) in pos.iter().enumerate() {
                m.sf(r + 4 * i, *v);
            }
            let tgt = 0x29B0 + 16 * k;
            m.sf(r + 0x50, (m.f(tgt) - (m.f(r) + m.f(r + 0x10))) / 35.0);
            m.sf(r + 0x58, (m.f(tgt + 8) - (m.f(r + 8) + m.f(r + 0x18))) / 35.0);
            m.sf(r + 0x54, (m.f(tgt + 4) - (m.f(r + 4) + m.f(r + 0x14))) / 35.0);
            m.sf(r + 0x5C, 0.0);
            let mut colour = s.colour;
            colour[3] = 0x80;
            m.0[r + 0x60..r + 0x64].copy_from_slice(&colour);
            c.sacs.push(SacState {
                handle: s.handle,
                card_id: s.card_id,
                pos,
                rot: [0, 0, 0],
                col: colour,
                model_scale: s.model_scale,
                removed: false,
            });
        }
        c.m.sh(0x2D20, 0x80);
        c
    }

    // ------------------------------------------------------------ accessors

    /// The counter (= u of the last update).
    pub fn counter(&self) -> u32 {
        self.counter
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// The board light byte written this update (0x1F7F00).
    pub fn light(&self) -> u8 {
        self.light
    }

    /// Random values drawn so far (12 at init, 55 after u 1).
    pub fn rand_calls(&self) -> u32 {
        self.rng.calls
    }

    /// Quads submitted in the last update per [`Family`] (repeats counted).
    pub fn quad_counts(&self) -> [u32; FAMILIES] {
        self.counts
    }

    pub fn count(&self, f: Family) -> u32 {
        self.counts[f as usize]
    }

    /// Billboard angles used in the last update.
    pub fn angles(&self) -> [i32; 3] {
        self.cam
    }

    /// Force the camera angles instead of deriving them from eye / target
    /// (the camera object's own polar, when known exactly).
    pub fn set_camera_angles(&mut self, a: Option<[i32; 3]>) {
        self.cam_override = a;
    }

    /// Sacrifice `k`'s handle and card id.
    pub fn sacrifice_card(&self, k: usize) -> (u16, u16) {
        (self.sacs[k].handle, self.sacs[k].card_id)
    }

    /// Sacrifice `k`'s card object (None once removed, after u 134).
    pub fn sacrifice_pose(&self, k: usize) -> Option<CardObj> {
        let s = &self.sacs[k];
        (!s.removed).then_some(CardObj { position: [s.pos[0], s.pos[1], s.pos[2]], rotation: s.rot, colour: s.col })
    }

    /// Sacrifice `k`'s model scale (shrinks -0.18/U at u 1-9; never back).
    pub fn sacrifice_model_scale(&self, k: usize) -> Option<f32> {
        self.sacs[k].model_scale
    }

    /// Sacrifice `k`'s phase (0 out, 1 spiral, 2 rise).
    pub fn sacrifice_phase(&self, k: usize) -> i32 {
        self.m.w(0x2650 + 0x70 * k + 0x64)
    }

    /// The other units' card alpha and model scale.
    pub fn others(&self) -> &[Unit] {
        &self.others
    }

    /// The result's handle (group of the first sacrifice + slot) and card
    /// object, from u 135.
    pub fn result_pose(&self) -> Option<(u16, CardObj)> {
        self.result.as_ref().map(|r| (r.handle, r.obj))
    }

    pub fn result_card(&self) -> Option<u16> {
        self.result.as_ref().map(|r| r.card_id)
    }

    // ------------------------------------------------------------ spawners

    fn ecol(&mut self, o: usize) {
        let m = &mut self.m;
        let (r, g, b, a) = (m.b(0x2A54), m.b(0x2A55), m.b(0x2A56), m.b(0x2A57));
        m.setrgb(o + 0x44, r, g, b);
        m.sb(o + 0x47, i32::from(a));
        m.setrgb(o + 0x40, r, g, b);
        m.sb(o + 0x43, i32::from(a));
        for c in 0..4 {
            let v = m.f(0x2A20 + 4 * c);
            m.sf(o + 0x10 + 4 * c, v);
        }
    }

    fn ealloc(&mut self) -> Option<usize> {
        for i in 0..EN {
            let o = E0 + ES * i;
            if self.m.h(o + 0x48) == 0 {
                self.m.zero(o, ES);
                self.m.sh(o + 0x48, i as i32 + 1);
                return Some(o);
            }
        }
        None
    }

    fn ecommon(&mut self, o: usize) {
        let m = &mut self.m;
        m.sh(o + 0x4E, m.f(0x2A40) as i32);
        m.sh(o + 0x50, m.f(0x2A44) as i32);
        m.sh(o + 0x4C, m.f(0x2A48) as i32);
        m.sh(o + 0x4A, m.h(0x2A90));
        m.suw(o + 0x30, m.last());
        m.sh(o + 0x56, m.h(0x2A9C));
        m.sh(o + 0x52, m.h(0x2A96));
    }

    /// 0x1E01BB0.
    fn spawn_bb0(&mut self, upd: u32, drw: u32) {
        let Some(o) = self.ealloc() else { return };
        self.ecol(o);
        self.m.suw(o + 0x38, drw);
        self.m.suw(o + 0x3C, upd);
    }

    /// 0x1E01CE0 (clamp 200) / 0x1E01EE0 (clamp D+0x2CD0).
    fn spawn_ce0(&mut self, upd: u32, drw: u32, clampv: i32) {
        let Some(o) = self.ealloc() else { return };
        self.ecol(o);
        self.ecommon(o);
        let m = &mut self.m;
        m.sh(o + 2, m.f(0x2A48) as i32);
        m.sh(o + 0xA, m.h(o + 2) + 20);
        if m.h(o + 2) > clampv {
            m.sh(o + 2, clampv);
        }
        if m.h(o + 0xA) > clampv {
            m.sh(o + 0xA, clampv);
        }
        m.suw(o + 0x30, m.last());
        if let Some(p) = off(m.last()) {
            m.suw(p + 0x34, ptr(o));
        }
        m.suw(o + 0x38, drw);
        m.suw(o + 0x3C, upd);
        m.suw(0x2A5C, ptr(o));
    }

    fn spawn_ee0(&mut self, upd: u32, drw: u32) {
        let c = self.m.h(0x2CD0);
        self.spawn_ce0(upd, drw, c);
    }

    /// 0x1E020E0 (draw forced to 0x1E031C0).
    fn spawn_0e0(&mut self, upd: u32) {
        let Some(o) = self.ealloc() else { return };
        self.ecol(o);
        self.ecommon(o);
        let m = &mut self.m;
        if matches!(m.h(o + 0x56), 0 | 1) {
            m.sh(o + 2, m.f(0x2A48) as i32);
            m.sh(o + 0xA, m.h(o + 2) + 28);
            if m.h(o + 0xA) > 180 {
                m.sh(o + 0xA, 180);
            }
        }
        m.suw(o + 0x30, m.last());
        if let Some(p) = off(m.last()) {
            m.0.copy_within(p + 0x20..p + 0x30, o + 0x20);
            m.suw(p + 0x34, ptr(o));
        }
        m.suw(o + 0x38, DRW_B);
        m.suw(o + 0x3C, upd);
        m.sh(o + 0x54, 1);
        m.suw(0x2A5C, ptr(o));
    }

    /// 0x1E02460.
    fn spawn_460(&mut self, upd: u32, drw: u32) {
        let Some(o) = self.ealloc() else { return };
        self.ecol(o);
        self.ecommon(o);
        self.m.sh(o + 0x54, 0);
        self.m.suw(o + 0x38, drw);
        self.m.suw(o + 0x3C, upd);
    }

    /// 0x1E025E0.
    fn spawn_5e0(&mut self, upd: u32, drw: u32) {
        let Some(o) = self.ealloc() else { return };
        self.ecol(o);
        self.ecommon(o);
        self.m.suw(o + 0x38, drw);
        self.m.suw(o + 0x3C, upd);
    }

    /// 0x1E02310: a bolt segment.
    fn spawn_l(&mut self, upd: u32, drw: u32) {
        let m = &mut self.m;
        let Some(o) = (0..LN).map(|i| L0 + LS * i).find(|&o| m.h(o + 0x28) == 0) else { return };
        m.zero(o, LS);
        m.sh(o + 0x28, ((o - L0) / LS) as i32 + 1);
        let (r, g, b) = (m.b(0x2A54), m.b(0x2A55), m.b(0x2A56));
        m.setrgb(o + 0x24, r, g, b);
        m.sb(o + 0x27, 0);
        m.setrgb(o + 0x20, r, g, b);
        m.sb(o + 0x23, 0);
        m.sh(o + 0x2A, m.h(0x2A96));
        m.sh(o + 0x2E, m.h(0x2A9C));
        m.suw(o + 0x10, m.last());
        if let Some(p) = off(m.last()) {
            m.suw(p + 0x14, ptr(o));
            m.sh(o + 8, m.h(p + 8));
            m.sh(o, m.h(p + 8));
            m.sh(o + 0xA, m.h(p + 0xA));
            m.sh(o + 2, m.h(p + 0xA));
            m.sh(o + 0xC, m.h(p + 0xC));
            m.sh(o + 4, m.h(p + 0xC));
        }
        m.sh(o + 0x2C, 5);
        m.suw(o + 0x18, drw);
        m.suw(o + 0x1C, upd);
    }

    /// 0x107390: 4 floats -> 4 s16.
    fn i16vec(&mut self, dst: usize, src: usize) {
        for c in 0..4 {
            let v = self.m.f(src + 4 * c) as i32;
            self.m.sh(dst + 2 * c, v);
        }
    }

    /// 0x1E02750: a falling star.
    fn spawn_q(&mut self) {
        let Some(o) = (0..QN).map(|i| Q0 + QS * i).find(|&o| self.m.h(o + 0x28) == 0) else { return };
        self.m.zero(o, QS);
        self.m.sh(o + 0x28, ((o - Q0) / QS) as i32 + 1);
        let (r, g, b, a) = (self.m.b(0x2A54), self.m.b(0x2A55), self.m.b(0x2A56), self.m.b(0x2A57));
        self.m.setrgb(o + 0x24, r, g, b);
        self.m.sb(o + 0x27, i32::from(a));
        self.i16vec(o, 0x2A20);
        self.rng.next();
        let m = &mut self.m;
        m.sh(o + 0x2A, m.f(0x2A40) as i32);
        let v = self.rng.next();
        self.m.sh(o + 0x2C, v % 2);
        self.i16vec(o + 8, 0x2A10);
        let m = &mut self.m;
        for c in 0..3 {
            m.sf(o + 0x10 + 4 * c, 1.0);
        }
        m.sh(o + 0x2E, m.h(0x2A9C));
        m.suw(o + 0x20, DRW_Q);
    }

    /// 0x1E02890: a spark ring.
    fn spawn_k(&mut self) {
        let Some(o) = (0..KN).map(|i| K0 + KS * i).find(|&o| self.m.h(o + 0x2E) == 0) else { return };
        self.m.zero(o, KS);
        self.m.sh(o + 0x2E, ((o - K0) / KS) as i32 + 1);
        self.i16vec(o, 0x2A20);
        let m = &mut self.m;
        m.sh(o + 0x30, m.f(0x2A40) as i32);
        m.sh(o + 0x32, m.h(0x2A90));
        m.sh(o + 0x28, m.h(0x2A9C));
        self.i16vec(o + 8, 0x2A10);
        self.m.suw(o + 0x20, DRW_K);
        self.m.suw(o + 0x24, UPD_K);
    }

    fn set_pos(&mut self, x: f32, y: f32, z: f32) {
        self.m.sf(0x2A20, x);
        self.m.sf(0x2A24, y);
        self.m.sf(0x2A28, z);
    }

    // ------------------------------------------------------------ director 0x1E04420

    fn director(&mut self) {
        self.m.setrgb(0x2A54, 0x80, 0x80, 0x80);
        self.m.sb(0x2A57, 0);
        self.m.sh(0x2AAC, 20);
        self.m.sh(0x2AA2, self.m.h(0x2AA2) + 1);
        let cnt = self.m.h(0x2AA2);
        if cnt == 1 {
            let m = &mut self.m;
            m.setrgb(0x2D28, 0x40, 0x40, 0x40);
            m.setrgb(0x2D2C, 0x50, 0x20, 0x20);
            for o in [0x2D30, 0x2D34, 0x2D38] {
                m.setrgb(o, 0x80, 0x80, 0x80);
            }
            for i in 0..10 {
                let v = if i == 0 { 0.0 } else { 20.0f32 + m.f(0x2ACC + 4 * i) };
                m.sf(0x2AD0 + 4 * i, v);
            }
            m.sh(0x2CC8, 30);
            m.sh(0x2CD0, 150);
            m.sh(0x2CD8, 180);
            let v = self.rng.next() % 200 + 40;
            self.m.sh(0x2D14, v);
            let v = self.rng.next() % 200 + 160;
            self.m.sh(0x2D16, v);
            let v = self.rng.next() % 120 + 200;
            self.m.sh(0x2D18, v);
            self.m.suw(0x2A5C, 0);
            for i in 0..8 {
                self.m.sf(0x2A60 + 4 * i, 160.0);
            }
            self.m.setrgb(0x2A54, 0x80, 0x80, 0x80);
            // A: four braziers on the corner cells.
            for (x, z) in [(150.0, 150.0), (-150.0, 150.0), (150.0, -150.0), (-150.0, -150.0)] {
                self.set_pos(x, 0.0, z);
                self.spawn_bb0(UPD_A, DRW_A);
            }
            // B: the wreath, a 3-link chain.
            let (mut s4, mut s0, mut s2, mut s3) = (70i32, 106.0f32, 113.0f32, 126.0f32);
            self.m.suw(0x2A5C, 0);
            for s5 in 7..10 {
                let m = &mut self.m;
                m.sh(0x2A9A, if m.h(0x2A9A) == 0 { 1 } else { 0 });
                m.sf(0x2A40, s0);
                m.sf(0x2A44, s2);
                m.sh(0x2A96, i32::from(s5 == 9));
                m.sf(0x2A48, s3);
                m.sh(0x2A9C, 1);
                self.set_pos(0.0, 0.0, 0.0);
                let m = &mut self.m;
                m.sb(0x2A57, 0);
                m.sh(0x2A90, s4);
                m.sh(0x2AA8, 7);
                m.setrgb(0x2A54, 0x50, 0x50, 0x50);
                self.spawn_0e0(UPD_B);
                s4 = s16(s4 + 10);
                s0 += 15.0;
                s2 += 16.0;
                s3 += 18.0;
            }
            // C: the curtain.
            let m = &mut self.m;
            m.setrgb(0x2A54, 0x70, 0x70, 0x70);
            m.suw(0x2A5C, 0);
            m.sf(0x2A40, 41.0);
            m.sf(0x2A44, 45.0);
            m.sh(0x2A96, 1);
            m.sf(0x2A48, 60.0);
            m.sh(0x2A9C, 1);
            self.set_pos(0.0, 0.0, 0.0);
            let m = &mut self.m;
            m.sb(0x2A57, 0);
            m.sh(0x2A90, 40);
            m.sh(0x2AA8, 7);
            m.sh(0x2A8E, 3);
            self.spawn_ee0(UPD_C, DRW_C);
            // D: the floor disc.
            let m = &mut self.m;
            m.suw(0x2A5C, 0);
            m.sf(0x2A40, 0.0);
            m.sf(0x2A44, 0.0);
            m.sh(0x2A96, 1);
            m.sf(0x2A48, 0.0);
            m.sh(0x2A9C, 1);
            self.set_pos(0.0, 0.0, 0.0);
            let m = &mut self.m;
            m.sb(0x2A57, 0);
            m.sh(0x2A90, 0);
            m.sh(0x2AA8, 7);
            m.sh(0x2AC4, 0);
            self.spawn_ce0(UPD_D, DRW_D, 200);
            // E: the fire ring.
            self.m.sf(0x2A40, 20.0);
            self.set_pos(0.0, 50.0, 0.0);
            self.spawn_460(UPD_E, DRW_EG);
            // F: the darkening.
            self.m.setrgb(0x2A54, 0x10, 0x10, 0x10);
            self.spawn_5e0(UPD_F, DRW_FX);
            self.m.sb(0x2A57, 0);
        }
        let m = &mut self.m;
        if cnt >= 21 && m.f(0x2A84) < 180.0 {
            m.sf(0x2A84, m.f(0x2A84) + 1.0);
        }
        if m.h(0x2CC8) < 180 {
            m.sh(0x2CC8, m.h(0x2CC8) + 3);
        }
        m.sh(0x2A94, m.h(0x2A94) + 6);
        if m.h(0x2A94) >= 360 {
            m.sh(0x2A94, m.h(0x2A94) % 360);
        }
        // A bolt anchor and its head segment.
        if cnt & 1 != 0 && m.w(0x2A50) >= 0x33 {
            m.suw(0x2A5C, 0);
            m.sw(0x2A40, 0);
            m.sh(0x2A8C, 35);
            for i in 0..AN {
                let a = A0 + AS * i;
                if m.h(a + 0x22) == 0 {
                    m.zero(a, AS);
                    m.sh(a + 0x22, 1);
                    m.sh(0x2A8C, i as i32);
                    break;
                }
            }
            if m.h(0x2A8C) < 35 {
                let i = m.h(0x2A8C) as usize;
                let a = A0 + AS * i;
                m.setrgb(0x2A54, 0x30, 0x30, 0);
                m.sh(0x2A9C, i as i32);
                m.sh(0x2A96, 0);
                let drw = if self.rng.next() % 10 < 6 { DRW_L2 } else { DRW_L1 };
                self.spawn_l(UPD_L, drw);
                let v = self.rng.next() % 100 + 120;
                self.m.sh(a + 0x24, v);
                let x = (self.rng.next() % 50 - 25) as f32;
                let z = (self.rng.next() % 50 - 25) as f32;
                self.m.sf(a, x);
                self.m.sf(a + 4, 0.0);
                self.m.sf(a + 8, z);
                let v = self.rng.next() & 0xFFFF;
                self.m.sw(a + 0x14, v);
                let v = (self.rng.next() & 0x7F) + 0x20;
                self.m.sh(a + 0x26, v);
                self.m.sh(a + 0x28, 7);
                let v = self.rng.next() % 60 + 170;
                self.m.sh(a + 0x2A, v);
                let v = self.rng.next() & 7;
                self.m.sf(a + 0x30, (0.6 + 0.1 * f64::from(v)) as f32);
            }
        }
        let m = &mut self.m;
        for i in 0..AN {
            let a = A0 + AS * i;
            if m.h(a + 0x22) == 0 {
                continue;
            }
            if m.h(a + 0x28) != 0 {
                m.sh(a + 0x28, m.h(a + 0x28) - 1);
            } else {
                m.sw(a + 0x10, m.w(a + 0x10).wrapping_add(m.h(a + 0x26)));
            }
            if m.h(a + 0x20) < 100 {
                m.sh(a + 0x20, m.h(a + 0x20) + 3);
            }
        }
        // A falling star.
        if cnt & 1 != 0 {
            let x = (self.rng.next() % 300 - 150) as f32;
            let z = (self.rng.next() % 300 - 150) as f32;
            self.set_pos(x, 100.0, z);
            self.m.sb(0x2A57, 0);
            const PAL: [[u8; 3]; 10] = [
                [255, 255, 255],
                [255, 0, 0],
                [0, 255, 0],
                [0, 0, 255],
                [255, 255, 0],
                [255, 0, 255],
                [0, 255, 255],
                [255, 128, 255],
                [128, 0, 255],
                [255, 128, 0],
            ];
            let [r, g, b] = PAL[(self.rng.next() % 10) as usize];
            self.m.setrgb(0x2A54, r, g, b);
            let v = self.rng.next() % 12;
            let m = &mut self.m;
            m.sh(0x2A9C, v);
            m.sf(0x2A14, -6.0);
            m.sf(0x2A10, -m.f(0x2A20) / 14.0);
            m.sf(0x2A18, -m.f(0x2A28) / 14.0);
            let v = self.rng.next() % 100 + 50;
            self.m.sf(0x2A40, v as f32);
            self.m.suw(0x2A5C, 0);
            if self.m.w(0x2A50) >= 0x12 {
                self.spawn_q();
            }
        }
        // G: the glow discs behind the sacrifices.
        if cnt == 1 {
            let v = self.rng.next() % 10 + 15;
            self.m.sf(0x2A40, v as f32);
            self.m.sb(0x2A57, 0);
            let v = self.rng.next() % 120;
            self.m.sh(0x2A90, v);
            self.m.sw(0x2A48, 0);
            let v = self.rng.next() % 12;
            self.m.sh(0x2A96, v);
            self.m.setrgb(0x2A54, 0x70, 0x70, 0x70);
            for k in 0..3 {
                self.m.sh(0x2A9C, k as i32);
                let r = 0x2650 + 0x70 * k;
                let (x, y, z) = (self.m.f(r), self.m.f(r + 4), self.m.f(r + 8));
                self.set_pos(x, y, z);
                self.spawn_460(UPD_G, DRW_EG);
            }
        }
        // K: a spark ring every 7 U.
        if cnt % 7 == 1 {
            self.set_pos(0.0, 30.0, 0.0);
            self.m.sb(0x2A57, 10);
            self.m.sh(0x2A9C, 0);
            let v = self.rng.next() % 360;
            self.m.sh(0x2A90, v);
            self.m.sw(0x2A2C, 0);
            self.spawn_k();
        }
        // The node matrices round each sacrifice (or the result).
        let ca = self.cam;
        for k in 0..3 {
            let m = &mut self.m;
            let r = 0x2650 + 0x70 * k;
            let mut p = [m.f(r) + m.f(r + 0x10), m.f(r + 4) + m.f(r + 0x14), m.f(r + 8) + m.f(r + 0x18)];
            p = rot_y(&self.tab, p, m.w(r + 0x24));
            if cnt >= 0x88 && m.w(0x2AC8) != 0 {
                p = [m.f(0x28F0), m.f(0x28F4), m.f(0x28F8)];
            }
            for j in 0..5 {
                let q = 0x16D0 + 16 * j;
                m.sw(q, ca[0] / 10);
                m.sw(q + 8, m.w(q + 8).wrapping_add(0x400));
                m.sw(q + 4, ca[1]);
                let t1 = trs(Some([0.9; 3]), None, Some(p.map(f64::from)));
                let t2 = trs(Some([0.9; 3]), Some([m.w(q), m.w(q + 4), m.w(q + 8)]), None);
                self.nodes[k][j] = matmul(&t2, &t1);
            }
        }
        if cnt == 100 || cnt == 130 {
            self.m.setrgb(0x2A54, 0xFF, 0xFF, 0xFF);
            self.m.sb(0x2A57, 0);
            self.m.sh(0x2A9C, 0);
            self.spawn_5e0(UPD_X, DRW_FX);
        }
        if self.m.h(0x2AB0) < 200 {
            self.m.sh(0x2AB0, self.m.h(0x2AB0) + 4);
        }
    }

    // ------------------------------------------------------------ emitter updates

    fn unlink_next(&mut self, o: usize) {
        if let Some(n) = off(self.m.uw(o + 0x34)) {
            self.m.suw(n + 0x30, 0);
        }
    }

    /// A braziers.
    fn upd_a(&mut self, o: usize) {
        let m = &mut self.m;
        if m.h(0x2AA2) < 10 {
            m.inc_a(o + 0x47, 16, 0x80);
            m.inc_a(o + 0x43, 16, 0x80);
        }
        if m.w(0x2A50) < 10 {
            m.dec_a(o + 0x47, 16, 17);
            m.dec_a(o + 0x43, 16, 17);
        }
    }

    /// Shared link motion of D / C / B: a -= st, b follows; remove when done.
    fn link_step(&mut self, o: usize, st: i32, gap: i32) {
        let m = &mut self.m;
        if m.h(o + 2) > 0 {
            m.sh(o + 2, m.h(o + 2) - st);
            if m.h(o + 0xA) - m.h(o + 2) >= gap {
                m.sh(o + 0xA, m.h(o + 0xA) - st);
            }
        } else {
            m.sh(o + 0xA, m.h(o + 0xA) - st);
            if m.h(o + 0xA) < m.h(o + 2) {
                m.sh(o + 0xA, m.h(o + 2));
                self.unlink_next(o);
                self.m.sh(o + 0x48, 0);
            }
        }
    }

    /// D floor disc.
    fn upd_d(&mut self, o: usize) {
        let m = &mut self.m;
        m.sh(o + 0x4A, m.h(o + 0x4A) - 4);
        if m.h(o + 0x4A) < -359 {
            m.sh(o + 0x4A, m.h(o + 0x4A) % 360);
        }
        m.sh(o + 0x4E, m.h(o + 2));
        m.sh(o + 0x50, m.h(o + 0xA));
        self.link_step(o, 4, 20);
        let m = &mut self.m;
        if m.h(o + 0xA) < m.h(0x2CC8) - 10 && m.h(o + 0x52) > 0 {
            m.suw(0x2A5C, ptr(o));
            m.sf(0x2A40, m.h(o + 0x4E) as f32);
            m.sf(0x2A44, m.h(o + 0x50) as f32);
            m.setrgb(0x2A54, 0x50, 0x50, 0x50);
            m.sb(0x2A57, 0);
            m.sh(0x2A96, 1);
            m.sf(0x2A48, m.h(o + 0xA) as f32);
            m.sh(0x2A9C, 1);
            self.set_pos(0.0, 0.0, 0.0);
            self.m.sh(0x2A90, self.m.h(o + 0x4A) + 15);
            self.spawn_ce0(UPD_D, DRW_D, 200);
            self.m.sh(o + 0x52, 0);
        }
        let m = &mut self.m;
        m.setrgb(o + 0x44, 0x40, 0, 0);
        m.setrgb(o + 0x40, 0x40, 0, 0);
        if m.h(0x2AA2) < 8 {
            m.inc_a(o + 0x47, 13, 0x30);
            m.inc_a(o + 0x43, 13, 0x30);
        } else if m.w(0x2A50) < 10 || m.h(o + 0x4E) < 15 {
            m.dec_a(o + 0x47, 16, 17);
            m.dec_a(o + 0x43, 16, 17);
        } else {
            m.inc_a(o + 0x47, 13, 0x80);
            m.inc_a(o + 0x43, 13, 0x80);
        }
    }

    /// C curtain.
    fn upd_c(&mut self, o: usize) {
        let m = &mut self.m;
        m.sh(o + 0x4A, m.h(o + 0x4A) - 5);
        if m.h(o + 0x4A) < -359 {
            m.sh(o + 0x4A, m.h(o + 0x4A) % 360);
        }
        m.sh(o + 0x4E, (1.3 * f64::from(m.h(o + 2))) as i32);
        m.sh(o + 0x50, (1.3 * f64::from(m.h(o + 0xA))) as i32);
        if m.h(o + 0x4E) < 10 {
            m.sh(o + 0x4E, 10);
        }
        if m.h(o + 0x50) < 10 {
            m.sh(o + 0x50, 10);
        }
        self.link_step(o, 5, 16);
        let m = &mut self.m;
        if m.h(o + 0xA) < m.h(0x2CD0) && m.h(o + 0x52) > 0 {
            m.suw(0x2A5C, ptr(o));
            m.sf(0x2A40, m.h(o + 0x4E) as f32);
            m.sf(0x2A44, m.h(o + 0x50) as f32);
            m.setrgb(0x2A54, 0x70, 0x70, 0x70);
            m.sb(0x2A57, 0);
            m.sh(0x2A96, 1);
            m.sf(0x2A48, m.h(o + 0xA) as f32);
            m.sh(0x2A9C, 1);
            self.set_pos(0.0, 0.0, 0.0);
            self.m.sh(0x2A90, self.m.h(o + 0x4A) + 10);
            if self.m.w(0x2A50) >= 3 {
                self.spawn_ee0(UPD_C, DRW_C);
            }
            self.m.sh(o + 0x52, 0);
        }
        let m = &mut self.m;
        if m.h(0x2AA2) < 8 {
            m.inc_a(o + 0x47, 12, 0x40);
            m.inc_a(o + 0x43, 12, 0x40);
        } else if m.h(o + 0xA) < 35 || m.w(0x2A50) < 10 {
            m.dec_a(o + 0x47, 10, 17);
            m.dec_a(o + 0x43, 10, 17);
        } else {
            m.inc_a(o + 0x47, 12, 0x80);
            if m.uw(o + 0x34) != 0 {
                m.inc_a(o + 0x43, 12, 0x80);
            }
        }
        if m.w(0x2A50) == 2 {
            m.sh(o + 0x48, 0);
        }
    }

    /// B wreath.
    fn upd_b(&mut self, o: usize) {
        let m = &mut self.m;
        m.sh(o + 0x4A, m.h(o + 0x4A) - 4);
        if m.h(o + 0x4A) >= 361 {
            m.sh(o + 0x4A, m.h(o + 0x4A) % 360);
        }
        let mut a1 = 1;
        while a1 < 8 {
            if m.f(0x2AD0 + 4 * a1) <= m.h(o + 2) as f32 {
                a1 += 1;
                continue;
            }
            if m.h(0x2AA2) >= 8 {
                let n = a1 as i32;
                if m.h(o + 0x4E) >= 10 * n + 5 {
                    m.sh(o + 0x4E, m.h(o + 0x4E) - 5);
                }
                if m.h(o + 0x50) >= 11 * n + 5 {
                    m.sh(o + 0x50, m.h(o + 0x50) - 5);
                }
            }
            break;
        }
        self.link_step(o, 5, 28);
        let m = &mut self.m;
        if m.h(o + 0x50) < 20 {
            m.dec_a(o + 0x47, 16, 17);
            m.dec_a(o + 0x43, 16, 17);
        }
        if m.h(o + 0xA) < m.h(0x2CD8) && m.h(o + 0x52) > 0 {
            m.suw(0x2A5C, ptr(o));
            m.sf(0x2A44, 155.0);
            m.sf(0x2A40, 155.0);
            m.setrgb(0x2A54, 0x50, 0x50, 0x50);
            m.sb(0x2A57, 10);
            m.sh(0x2A96, 1);
            m.sf(0x2A48, m.h(o + 0xA) as f32);
            m.sh(0x2A9C, 1);
            self.set_pos(0.0, 0.0, 0.0);
            self.m.sh(0x2A90, self.m.h(o + 0x4A));
            if self.m.w(0x2A50) >= 17 {
                self.spawn_0e0(UPD_B);
            }
            self.m.sh(o + 0x52, 0);
        }
        let m = &mut self.m;
        if m.h(0x2AA2) < 8 {
            let st = if m.h(o + 0xA) >= 121 { 6 } else { 15 };
            if m.uw(o + 0x30) != 0 {
                m.inc_a(o + 0x47, st, 0x60);
            }
            m.inc_a(o + 0x43, st, 0x60);
        } else if m.w(0x2A50) < 10 {
            m.dec_a(o + 0x47, 16, 17);
            m.dec_a(o + 0x43, 16, 17);
        } else if m.h(o + 0xA) >= 81 {
            if m.uw(o + 0x30) != 0 {
                m.inc_a(o + 0x47, 5, 0x60);
            }
            m.inc_a(o + 0x43, 5, 0x60);
        } else {
            m.dec_a(o + 0x47, 5, 17);
            m.dec_a(o + 0x43, 5, 17);
        }
    }

    /// G glow discs.
    fn upd_g(&mut self, o: usize) {
        let m = &mut self.m;
        let k = m.h(o + 0x56) as usize;
        let r = 0x2650 + 0x70 * k;
        for c in 0..3 {
            m.sf(o + 0x10 + 4 * c, m.f(r + 4 * c) + m.f(r + 0x10 + 4 * c));
        }
        if m.h(0x2AA2) >= 0x88 && m.w(0x2AC8) != 0 {
            for c in 0..3 {
                m.sf(o + 0x10 + 4 * c, m.f(0x28F0 + 4 * c));
            }
        }
        m.sb(o + 0x47, 0);
        m.setrgb(o + 0x44, 0x50, 0x50, 0xFF);
        if m.w(0x2A50) >= 11 {
            if m.b(o + 0x43) < 0xB0 {
                m.sb(o + 0x43, i32::from(m.b(o + 0x43)) + 16);
            }
        } else if m.b(o + 0x43) >= 17 {
            m.sb(o + 0x43, i32::from(m.b(o + 0x43)) - 15);
        } else {
            m.sh(o + 0x48, 0);
        }
        m.setrgb(o + 0x40, 0xB0, 0xB0, 0xBF);
        if m.h(o + 0x4C) < 60 {
            m.sh(o + 0x4C, m.h(o + 0x4C) + 4);
        }
        m.sf(o + 0x24, m.f(o + 0x24) + 2048.0);
    }

    /// F darkening.
    fn upd_f(&mut self, o: usize) {
        let m = &mut self.m;
        m.sb(o + 0x47, 0);
        if m.w(0x2A50) < 10 {
            if m.b(o + 0x43) >= 33 {
                m.sb(o + 0x43, i32::from(m.b(o + 0x43)) - 21);
            } else {
                m.sh(o + 0x48, 0);
            }
        } else if m.b(o + 0x43) < 0xF0 {
            m.sb(o + 0x43, i32::from(m.b(o + 0x43)) + 60);
        }
        m.setrgb(o + 0x40, 0x20, 0, 0x40);
    }

    /// X flashes.
    fn upd_x(&mut self, o: usize) {
        let m = &mut self.m;
        let s = m.h(o + 0x56);
        if s == 0 {
            if m.b(o + 0x47) < 0xE0 {
                m.sb(o + 0x47, i32::from(m.b(o + 0x47)) + 0x30);
            }
            if m.b(o + 0x43) < 0xE0 {
                m.sb(o + 0x43, i32::from(m.b(o + 0x43)) + 0x30);
            } else {
                m.sh(o + 0x56, 1);
            }
        } else if s < 4 {
            m.sh(o + 0x56, s + 1);
        } else {
            if m.b(o + 0x47) >= 20 {
                m.sb(o + 0x47, i32::from(m.b(o + 0x47)) - 16);
            }
            if m.b(o + 0x43) >= 20 {
                m.sb(o + 0x43, i32::from(m.b(o + 0x43)) - 16);
            } else {
                m.sh(o + 0x48, 0);
            }
        }
    }

    /// E fire ring.
    fn upd_e(&mut self, o: usize) {
        let m = &mut self.m;
        m.sh(o + 0x52, m.h(o + 0x52) + 1);
        if m.h(o + 0x52) >= 10 {
            m.sh(o + 0x52, 0);
        }
        if m.h(o + 0x56) == 0 {
            m.sh(o + 0x4E, m.h(o + 0x4E) + 2);
            if m.h(o + 0x4E) >= 61 {
                m.sh(o + 0x56, 1);
            }
        } else {
            m.sh(o + 0x4E, m.h(o + 0x4E) - 2);
            if m.h(o + 0x4E) < 20 {
                m.sh(o + 0x56, 0);
            }
        }
        if m.w(0x2A50) < 10 {
            if m.b(o + 0x43) >= 17 {
                m.sb(o + 0x43, i32::from(m.b(o + 0x43)) - 16);
            } else {
                m.sh(o + 0x48, 0);
            }
        } else if m.b(o + 0x43) < 0x80 {
            m.sb(o + 0x43, i32::from(m.b(o + 0x43)) + 9);
        }
    }

    fn upd_e_slot(&mut self, o: usize, f: u32) {
        match f {
            UPD_A => self.upd_a(o),
            UPD_D => self.upd_d(o),
            UPD_C => self.upd_c(o),
            UPD_B => self.upd_b(o),
            UPD_G => self.upd_g(o),
            UPD_F => self.upd_f(o),
            UPD_X => self.upd_x(o),
            UPD_E => self.upd_e(o),
            _ => {}
        }
    }

    /// 0x1E05F90: a bolt segment.
    fn upd_l(&mut self, o: usize) {
        let m = &mut self.m;
        let a = A0 + AS * m.h(o + 0x2E) as usize;
        let c = m.h(o + 0x2C);
        if c >= 2 {
            if m.h(o + 0xA) < m.h(a + 0x24) {
                m.sh(o + 0xA, m.h(o + 0xA) + 8);
            }
            m.sh(o + 0x2C, c - 1);
        } else if c == 1 {
            if m.h(o + 0xA) < m.h(a + 0x24) {
                m.sh(0x2A96, 0);
                m.sb(0x2A57, 0);
                if m.uw(o + 0x18) == DRW_L1 {
                    m.setrgb(0x2A54, 0x30, 0x30, 0);
                } else {
                    m.setrgb(0x2A54, 0x30, 0x10, 0);
                }
                self.set_pos(0.0, 0.0, 0.0);
                let m = &mut self.m;
                m.suw(0x2A5C, ptr(o));
                m.sh(0x2A9C, m.h(o + 0x2E));
                let d = m.uw(o + 0x18);
                self.spawn_l(UPD_L, d);
            } else {
                m.sh(a + 0x22, 0);
            }
            self.m.sh(o + 0x2C, 0);
        } else {
            m.dec_a(o + 0x27, 16, 17);
            if m.b(o + 0x23) >= 20 {
                m.sb(o + 0x23, i32::from(m.b(o + 0x23)) - 16);
            } else {
                // Quirk 1: `next + 0x30 = 0` as if next were an E slot.
                if let Some(n) = off(m.uw(o + 0x14)) {
                    m.suw(n + 0x30, 0);
                }
                m.sh(o + 0x28, 0);
            }
        }
        let v = self.rng.next() % 40 - 20;
        let m = &mut self.m;
        m.sh(o + 0xE, v);
        m.setrgb(o + 0x24, 0x80, 0x80, 0);
        m.setrgb(o + 0x20, 0x80, 0x80, 0);
        m.sh(o + 0x2A, if m.h(o + 0x2A) != 0 { 0 } else { 1 });
        if m.w(0x2A50) < 10 {
            m.dec_a(o + 0x27, 16, 17);
            if m.b(o + 0x23) >= 17 {
                m.sb(o + 0x23, i32::from(m.b(o + 0x23)) - 16);
            } else {
                m.sh(o + 0x28, 0);
            }
        } else if m.h(o + 0x2C) != 0 {
            if m.b(o + 0x27) < 0xB0 {
                m.sb(o + 0x27, 0xB0);
            }
            if m.b(o + 0x23) < 0xB0 {
                m.sb(o + 0x23, 0xB0);
            }
        }
    }

    /// 0x1E06470: a falling star.
    fn upd_q(&mut self, o: usize) {
        let m = &mut self.m;
        m.sh(o + 2, m.h(o + 2) + m.h(o + 0xA));
        if m.h(o + 2) < 0 {
            m.sh(o + 0x28, 0);
        }
        m.sh(o, m.h(o) + m.h(o + 8));
        m.sh(o + 4, m.h(o + 4) + m.h(o + 0xC));
        let (x, y, z) = (m.h(o), m.h(o + 2), m.h(o + 4));
        let d = f64::from((x * x + y * y + z * z) as f32).sqrt() as f32;
        if d < 35.0 {
            for c in [0x10, 0x18, 0x14] {
                m.sf(o + c, m.f(o + c) - 0.35);
            }
            if m.f(o + 0x18) < 0.0 {
                m.sh(o + 0x28, 0);
            }
        }
        if m.h(o + 0x2C) == 0 {
            for c in 0..3 {
                m.sb(o + 0x24 + c, i32::from(m.b(o + 0x24 + c)) - 0x30);
            }
            m.sh(o + 0x2C, 1);
        } else {
            for c in 0..3 {
                m.sb(o + 0x24 + c, i32::from(m.b(o + 0x24 + c)) + 0x30);
            }
            m.sh(o + 0x2C, 0);
        }
        if m.b(o + 0x27) < 0x80 {
            m.sb(o + 0x27, i32::from(m.b(o + 0x27)) + 0x12);
        }
    }

    /// 0x1E065D0: a spark ring.
    fn upd_k(&mut self, o: usize) {
        let m = &mut self.m;
        m.sh(o + 2, m.h(o + 2) + 9);
        m.sh(o + 0x32, m.h(o + 0x32) + 5);
        if m.h(o + 0x32) >= 360 || m.h(o + 0x32) < -359 {
            m.sh(o + 0x32, m.h(o + 0x32) % 360);
        }
        let y = m.h(o + 2);
        if y >= 0x83 {
            m.dec_a(o + 0x37, 15, 17);
            if m.b(o + 0x3B) >= 17 {
                m.sb(o + 0x3B, i32::from(m.b(o + 0x3B)) - 15);
            } else {
                m.sh(o + 0x2E, 0);
            }
        } else if y >= 0x29 {
            if m.w(0x2A50) >= 11 {
                m.inc_a(o + 0x37, 0x13, 0x80);
                m.inc_a(o + 0x3B, 0x13, 0x80);
            } else {
                m.dec_a(o + 0x37, 15, 17);
                m.dec_a(o + 0x3B, 15, 17);
            }
        }
        if m.h(o + 2) >= 0xA0 {
            m.sh(o + 0x2E, 0);
        }
        m.setrgb(o + 0x34, 0x80, 0x40, 0x80);
        m.setrgb(o + 0x38, 0x80, 0x40, 0x80);
    }

    // ------------------------------------------------------------ draws

    fn emit(&mut self, sc: [[f32; 2]; 4], n: u32, fam: Family) {
        self.counts[fam as usize] += n;
        let p = &self.prim;
        let (uv, textured, sheet) = if p.g {
            ([[0.0f32; 2]; 4], false, 0u8)
        } else {
            let [sw, sh] = sheet_size(p.sheet);
            (p.uv.map(|[u, v]| [u as f32 / sw, v as f32 / sh]), true, p.sheet)
        };
        let c = p.c;
        let out = if fam == Family::F { &mut self.subtractive } else { &mut self.additive };
        for _ in 0..n {
            out.push(FxTri { screen: [sc[0], sc[1], sc[2]], uv: [uv[0], uv[1], uv[2]], colour: [c[0], c[1], c[2]], textured, sheet });
            out.push(FxTri { screen: [sc[1], sc[3], sc[2]], uv: [uv[1], uv[3], uv[2]], colour: [c[1], c[3], c[2]], textured, sheet });
        }
    }

    /// 0x1098D0 (+ repeated submits): drop the quad if any vertex fails.
    fn quad(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, mx: &Mat, n: u32, fam: Family) -> bool {
        let mut sc = [[0.0f32; 2]; 4];
        for i in 0..4 {
            match pj(apply(mx, self.pts[i])) {
                Some(v) => sc[i] = v,
                None => return false,
            }
        }
        self.emit(sc, n, fam);
        true
    }

    fn set_pts(&mut self, p: [[f32; 3]; 4]) {
        self.pts = p;
    }

    fn polar(&mut self, i: usize, ang: i32, rad: i32) {
        let r = rad as f32;
        self.pts[i][0] = r * dcos(&self.tab, s16(ang));
        self.pts[i][2] = r * dsin(&self.tab, s16(ang));
    }

    fn add4(&mut self, v: [f32; 3]) {
        for p in &mut self.pts {
            for c in 0..3 {
                p[c] += v[c];
            }
        }
    }

    /// 0x1086B0: v0, v1 xz += src - dst.
    fn off_ab(&mut self, src: usize, dst: usize) {
        let dx = self.m.f(src) - self.m.f(dst);
        let dz = self.m.f(src + 8) - self.m.f(dst + 8);
        for i in 0..2 {
            self.pts[i][0] += dx;
            self.pts[i][2] += dz;
        }
    }

    /// A braziers + A' flames.
    fn drw_a(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, o: usize) {
        let a1 = self.m.b(o + 0x47);
        self.m.0[0x2D2B] = a1;
        self.prim.colour(self.m.col(0x2D28), 1);
        self.m.0[0x2D2B] = self.m.b(o + 0x43);
        self.prim.colour(self.m.col(0x2D28), 2);
        self.prim.uv = uvset(uvrect(0x6E, 1), 0);
        self.prim.sheet = 11;
        let bm = trs(None, Some(self.cam), Some([self.m.f(o + 0x10), self.m.f(o + 0x14), self.m.f(o + 0x18)].map(f64::from)));
        self.set_pts([[-20.0, 40.0, 0.0], [20.0, 40.0, 0.0], [-20.0, 0.0, 0.0], [20.0, 0.0, 0.0]]);
        self.quad(pj, &bm, 4, Family::A);
        // 0x1E02DE0 (.., 0): set-up, one rand.
        self.rng.next();
        self.m.0[0x2D2F] = self.m.b(o + 0x47);
        self.prim.colour(self.m.col(0x2D2C), 1);
        self.m.0[0x2D2F] = self.m.b(o + 0x43);
        self.prim.colour(self.m.col(0x2D2C), 2);
        self.prim.sheet = 13;
        // (.., 1): geometry, one rand + 3 frames.
        self.rng.next();
        self.set_pts([[-20.0, 120.0, 0.0], [20.0, 120.0, 0.0], [-20.0, 30.0, 0.0], [20.0, 30.0, 0.0]]);
        for _ in 0..3 {
            let f = self.rng.next() % 6;
            self.prim.uv = uvset(uvrect(0x82, f), 0);
            self.quad(pj, &bm, 1, Family::A2);
        }
    }

    /// D floor disc.
    fn drw_d(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, o: usize) {
        self.prim.uv = uvset(uvrect(0x9C, 0), 1);
        self.prim.sheet = 15;
        let pv = off(self.m.uw(o + 0x30));
        let y0 = 10.0f32 * dsin(&self.tab, self.m.h(0x2A94) + 4 * self.m.h(o + 2));
        self.pts[0][1] = y0;
        self.pts[1][1] = y0;
        let y1 = 10.0f32 * dsin(&self.tab, self.m.h(0x2A94) + 4 * self.m.h(o + 0xA));
        self.pts[2][1] = y1;
        self.pts[3][1] = y1;
        let mut s2 = 0;
        let id = ident();
        for _ in 0..8 {
            let ang = self.m.h(o + 0x4A);
            if let Some(p) = pv {
                let (pa, pr) = (self.m.h(p + 0x4A), self.m.h(p + 0x50));
                self.polar(0, s2 + pa, pr);
                self.polar(1, s2 + pa + 35, pr);
                self.m.sh(o + 2, self.m.h(p + 0xA));
            } else {
                let r = self.m.h(o + 0x4E);
                self.polar(0, s2 + ang, r);
                self.polar(1, s2 + ang + 35, r);
            }
            let r = self.m.h(o + 0x50);
            self.polar(2, s2 + ang, r);
            self.polar(3, s2 + ang + 35, r);
            let c0 = match pv {
                None => self.m.col(o + 0x44),
                Some(p) => self.m.col(p + 0x40),
            };
            self.prim.colour(c0, 1);
            self.prim.colour(self.m.col(o + 0x40), 2);
            self.quad(pj, &id, 1, Family::D);
            s2 = s16(s2 + 45);
        }
    }

    /// C curtain.
    fn drw_c(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, o: usize) {
        self.prim.uv = uvset(uvrect(0x97, 0), 1);
        self.prim.sheet = 15;
        let pv = off(self.m.uw(o + 0x30));
        if let Some(p) = pv {
            let (pa, pr) = (self.m.h(p + 0x4A), self.m.h(p + 0x50));
            self.polar(0, pa, pr);
            self.polar(1, pa + 15, pr);
            let y = self.m.h(p + 0xA) as f32;
            self.pts[0][1] = y;
            self.pts[1][1] = y;
            self.off_ab(p + 0x20, o + 0x20);
        } else {
            let (a, r) = (self.m.h(o + 0x4A), self.m.h(o + 0x4E));
            self.polar(0, a, r);
            self.polar(1, a + 15, r);
            let y = self.m.h(o + 2) as f32;
            self.pts[0][1] = y;
            self.pts[1][1] = y;
        }
        let (a, r) = (self.m.h(o + 0x4A), self.m.h(o + 0x50));
        self.polar(2, a, r);
        self.polar(3, a + 15, r);
        let y = self.m.h(o + 0xA) as f32;
        self.pts[3][1] = y;
        self.pts[2][1] = y;
        self.add4([self.m.f(o + 0x20), self.m.f(o + 0x24), self.m.f(o + 0x28)]);
        let mut c = self.m.col(o + 0x44);
        c[3] = match pv {
            None => self.m.b(o + 0x47),
            Some(p) => self.m.b(p + 0x43),
        };
        self.prim.colour(c, 1);
        c[3] = self.m.b(o + 0x43);
        self.prim.colour(c, 2);
        let mut ry = 0i32;
        for kk in 0..3 {
            let mx = trs(Some(self.sc240), Some([0, ry, 0]), None);
            self.quad(pj, &mx, 1, Family::C);
            ry = self.m.h(0x2D14 + 2 * kk).wrapping_mul(0xB6);
        }
    }

    /// B wreath.
    fn drw_b(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, o: usize) {
        self.prim.colour(self.m.col(o + 0x40), 2);
        self.prim.uv = uvset(uvrect(0x9C, 0), 0);
        self.prim.sheet = 15;
        let [c0, s0, c30, s30] = self.rt;
        let pv = off(self.m.uw(o + 0x30));
        let rr = match pv {
            Some(p) => self.m.h(p + 0x50),
            None => self.m.h(o + 0x4E),
        } as f32;
        self.pts[0][0] = c0 * rr;
        self.pts[0][2] = s0 * rr;
        self.pts[1][0] = c30 * rr;
        self.pts[1][2] = s30 * rr;
        if let Some(p) = pv {
            self.m.sh(o + 2, self.m.h(p + 0xA));
            let y = self.m.h(p + 0xA) as f32;
            self.pts[1][1] = y;
            self.pts[0][1] = y;
            self.off_ab(p + 0x20, o + 0x20);
        }
        let rr = self.m.h(o + 0x50) as f32;
        self.pts[2][0] = c0 * rr;
        self.pts[2][2] = s0 * rr;
        self.pts[3][0] = c30 * rr;
        self.pts[3][2] = s30 * rr;
        let y = self.m.h(o + 0xA) as f32;
        self.pts[3][1] = y;
        self.pts[2][1] = y;
        self.add4([self.m.f(o + 0x20), self.m.f(o + 0x24), self.m.f(o + 0x28)]);
        let y = 30.0f32 + 150.0f32 * dcos(&self.tab, self.m.h(o + 2) - 110);
        self.pts[0][1] = y;
        self.pts[1][1] = y;
        let y = 30.0f32 + 150.0f32 * dcos(&self.tab, self.m.h(o + 0xA) - 110);
        self.pts[2][1] = y;
        self.pts[3][1] = y;
        let c = match pv {
            None => self.m.col(o + 0x44),
            Some(p) => self.m.col(p + 0x40),
        };
        self.prim.colour(c, 1);
        let mut ry = 0i32;
        for _ in 0..12 {
            let mx = trs(Some(self.sc240), Some([0, ry, 0]), None);
            self.quad(pj, &mx, 1, Family::B);
            if self.m.h(o + 2) < 100 {
                let mx = trs(Some([1.0, 1.8, 1.0]), Some([0, ry, 0]), None);
                self.quad(pj, &mx, 1, Family::B);
            }
            ry += 0x1554;
        }
    }

    /// E fire ring / G glow discs (0x1E06720).
    fn drw_eg(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, o: usize) {
        let up = self.m.uw(o + 0x3C);
        if up == UPD_E {
            self.prim.template();
            self.prim.uv = uvset(uvrect(0x0A, self.m.h(o + 0x52)), 0);
            let (c4, c0) = (self.m.col(o + 0x44), self.m.col(o + 0x40));
            self.prim.c = [c4, c4, c0, c0];
            self.prim.sheet = 1;
            let mx = trs(None, Some([0, self.cam[1], 0]), None);
            let ax = (-self.cam[0]) / 180;
            let mut s1 = 0;
            for _ in 0..12 {
                let y = 70.0f32 * dsin(&self.tab, 90);
                let pts = [[-30.0, y, 0.0], [30.0, y, 0.0], [-30.0, 0.0, 0.0], [30.0, 0.0, 0.0f32]].map(|p| rot_x(&self.tab, p, ax));
                let r = self.m.h(o + 0x4E) as f32;
                let a = self.m.h(o + 0x4A) + s1;
                let (dx, dz) = (r * dcos(&self.tab, a), r * dsin(&self.tab, a));
                self.set_pts(pts.map(|p| [p[0] + dx, p[1], p[2] + dz]));
                self.quad(pj, &mx, 2, Family::E);
                s1 += 30;
            }
        } else if up == UPD_G {
            self.prim.g = true;
            let (c0, c4) = (self.m.col(o + 0x40), self.m.col(o + 0x44));
            self.prim.c = [c0, c0, c4, c4];
            let k = self.m.h(o + 0x56) as usize;
            let r = 0x2650 + 0x70 * k;
            self.pts[0] = [0.0; 3];
            self.pts[1] = [0.0; 3];
            self.pts[2][2] = 0.0;
            self.pts[3][2] = 0.0;
            for p in &mut self.pts {
                p[2] += -10.0;
            }
            for p in &mut self.pts {
                p[2] += 0.0;
            }
            let mut p = [self.m.f(o + 0x10), self.m.f(o + 0x14), self.m.f(o + 0x18)];
            if self.m.h(0x2AA2) < 0x87 {
                p = rot_y(&self.tab, p, self.m.w(r + 0x24));
            }
            let mx = trs(None, Some(self.cam), Some(p.map(f64::from)));
            for fp in 0..6 {
                let (s7, s1) = (30 * fp, 30 + 30 * fp);
                for sg in [1, -1] {
                    let rr = self.m.h(o + 0x4C) as f32;
                    self.pts[2][0] = rr * dcos(&self.tab, sg * s7);
                    self.pts[2][1] = rr * dsin(&self.tab, sg * s7);
                    self.pts[3][0] = rr * dcos(&self.tab, sg * s1);
                    self.pts[3][1] = rr * dsin(&self.tab, sg * s1);
                    self.quad(pj, &mx, 2, Family::G);
                }
            }
        }
    }

    /// F darkening / X flashes: full-screen Gouraud quads.
    fn drw_fx(&mut self, o: usize) {
        self.prim.g = true;
        let (c4, c0) = (self.m.col(o + 0x44), self.m.col(o + 0x40));
        self.prim.c = [c4, c4, c0, c0];
        let up = self.m.uw(o + 0x3C);
        self.prim.subtract = !(up == UPD_E || up == UPD_X);
        let fam = if self.prim.subtract { Family::F } else { Family::X };
        self.emit([[0.0, 0.0], [512.0, 0.0], [0.0, 512.0], [512.0, 512.0]], 1, fam);
    }

    /// Q falling star.
    fn drw_q(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, o: usize) {
        self.prim.template();
        self.prim.uv = uvset(uvrect(0x50, self.m.h(o + 0x2E)), 0);
        let c = self.m.col(o + 0x24);
        self.prim.colour(c, 1);
        self.prim.colour(c, 2);
        self.prim.sheet = 8;
        let ax = (-self.cam[0]) / 180;
        let mx = trs(None, Some([0, self.cam[1], 0]), None);
        for s2 in 0..2 {
            let pts = [[-3.0f32, 3.0, 0.0], [3.0, 3.0, 0.0], [-3.0, -3.0, 0.0], [3.0, -3.0, 0.0]].map(|p| rot_x(&self.tab, p, ax));
            let m = &self.m;
            let (ox, oz) = if s2 == 0 { (m.h(o) as f32, m.h(o + 4) as f32) } else { (-m.h(o) as f32, -m.h(o + 4) as f32) };
            let oy = m.h(o + 0x2A) as f32 * dsin(&self.tab, m.h(o + 2));
            let sc = [m.f(o + 0x10), m.f(o + 0x14), m.f(o + 0x18)];
            self.set_pts(pts.map(|p| [p[0] * sc[0] + ox, p[1] * sc[1] + oy, p[2] * sc[2] + oz]));
            self.quad(pj, &mx, 1, Family::Q);
        }
    }

    /// L bolt segment (0x1E03610 / 0x1E03D70).
    fn drw_l(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, o: usize, kind: u32) {
        let pv = off(self.m.uw(o + 0x10));
        self.prim.template();
        self.prim.uv = uvset(uvrect(0x96, 0), 0);
        self.prim.sheet = 15;
        let mut c = self.m.col(o + 0x24);
        if self.m.uw(o + 0x14) == 0 {
            c[3] = 0;
        }
        self.prim.colour(c, 1);
        if self.m.h(o + 0x2C) == 0 {
            let mut c = if kind == DRW_L1 { self.m.col(o + 0x20) } else { self.m.col(o + 0x24) };
            c[3] = 0;
            self.prim.colour(c, 2);
        } else if let Some(p) = pv {
            self.prim.colour(self.m.col(p + 0x24), 2);
        }
        // else quirk 2: v2, v3 keep the previous primitive's colours.
        let m = &self.m;
        let a = A0 + AS * m.h(o + 0x2E) as usize;
        let x0 = m.h(o + 8) + m.h(o + 0xE);
        self.pts[0][0] = (x0 - 6) as f32;
        self.pts[1][0] = (x0 + 6) as f32;
        let x1 = match pv {
            Some(p) => m.h(p + 8) + m.h(p + 0xE),
            None => m.h(o),
        };
        self.pts[2][0] = (x1 - 6) as f32;
        self.pts[3][0] = (x1 + 6) as f32;
        let far_ha = match pv {
            Some(p) => m.h(p + 0xA),
            None => m.h(o + 2),
        };
        let (scale, n, fam) = if kind == DRW_L1 {
            let sc = m.f(a + 0x30);
            let ph = m.h(a + 0x2A) as f32;
            let len = m.h(a + 0x24) as f32;
            let an = (ph + m.h(o + 0xA) as f32 * sc) as i32;
            let y = 50.0f32 + len * dcos(&self.tab, an);
            self.pts[0][1] = y;
            self.pts[1][1] = y;
            let an = (ph + far_ha as f32 * sc) as i32;
            let y = 50.0f32 + len * dcos(&self.tab, an);
            self.pts[2][1] = y;
            self.pts[3][1] = y;
            let depth = m.h(a + 0x20) as f32;
            let z = depth * dcos(&self.tab, m.h(o + 0xA) - 110);
            self.pts[0][2] = z;
            self.pts[1][2] = z;
            let z = if let Some(p) = pv { depth * dcos(&self.tab, m.h(p + 0xA) - 110) } else { 0.0 };
            self.pts[2][2] = z;
            self.pts[3][2] = z;
            ([1.0, 0.5, 1.0], 2, Family::L1)
        } else {
            let y = m.h(o + 0xA) as f32;
            self.pts[0][1] = y;
            self.pts[1][1] = y;
            let y = far_ha as f32;
            self.pts[2][1] = y;
            self.pts[3][1] = y;
            let z = 90.0f32 * dsin(&self.tab, m.h(o + 0xA));
            self.pts[0][2] = z;
            self.pts[1][2] = z;
            let z = 90.0f32 * dsin(&self.tab, far_ha);
            self.pts[2][2] = z;
            self.pts[3][2] = z;
            ([1.0, 1.0, 1.0], 1, Family::L2)
        };
        let rot = [m.w(a + 0x10), m.w(a + 0x14), m.w(a + 0x18)];
        let pos = [m.f(a), m.f(a + 4), m.f(a + 8)].map(f64::from);
        self.set_pos(150.0, 0.0, 150.0);
        let mx = trs(Some(scale), Some(rot), Some(pos));
        for _ in 0..n {
            self.quad(pj, &mx, 1, fam);
        }
    }

    /// K spark ring round each sacrifice node.
    fn drw_k(&mut self, pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, o: usize) {
        self.prim.template();
        self.prim.uv = uvset(uvrect(0x96, 0), 0);
        let (c4, c8) = (self.m.col(o + 0x34), self.m.col(o + 0x38));
        self.prim.c = [c4, c4, c8, c8];
        self.prim.sheet = 15;
        let y = self.m.h(o + 2);
        let r0 = s16((65.0f32 * dsin(&self.tab, y)) as i32) as f32;
        let r1 = s16((65.0f32 * dsin(&self.tab, y - 5)) as i32) as f32;
        let y0 = (y - 90) as f32;
        let y1 = y0 - 12.0;
        let (mut s7, mut s4) = (0, 0x48);
        for _ in 0..5 {
            let t = &self.tab;
            let (c7, n7, c4, n4) = (dcos(t, s7), dsin(t, s7), dcos(t, s4), dsin(t, s4));
            self.set_pts([[c7 * r0, y0, n7 * r0], [c4 * r0, y0, n4 * r0], [c7 * r1, y1, n7 * r1], [c4 * r1, y1, n4 * r1]]);
            for j in 0..4 {
                for k in 0..3 {
                    let mx = self.nodes[k][j];
                    self.quad(pj, &mx, 1, Family::K);
                }
            }
            s7 += 0x48;
            s4 += 0x48;
        }
    }

    // ------------------------------------------------------------ units

    /// 0x1E006D0: sacrifice k's card object.
    fn sac_update(&mut self, k: usize) {
        let cam = self.cam;
        let m = &mut self.m;
        let r = 0x2650 + 0x70 * k;
        let (px, pz) = (m.f(r) + m.f(r + 0x10), m.f(r + 8) + m.f(r + 0x18));
        let dist = f64::from(px * px + pz * pz).sqrt() as f32;
        let mut sp = [m.f(r), m.f(r + 4), m.f(r + 8)];
        let mut s1 = 0;
        let ph = m.w(r + 0x64);
        if ph == 0 {
            let tgt = 0x29B0 + 16 * k;
            let dx = (m.f(tgt) - (m.f(r) + m.f(r + 0x10))).abs();
            let dz = (m.f(tgt + 8) - (m.f(r + 8) + m.f(r + 0x18))).abs();
            if dx > 5.0 {
                m.sf(r + 0x10, m.f(r + 0x10) + m.f(r + 0x50));
            } else {
                s1 += 1;
                m.sw(0x2CBC, m.w(0x2CBC) + 1);
            }
            if dz > 5.0 {
                m.sf(r + 0x18, m.f(r + 0x18) + m.f(r + 0x58));
            } else {
                s1 += 1;
                m.sw(0x2CBC, m.w(0x2CBC) + 1);
            }
            if m.f(r + 4) < 30.0 {
                m.sf(r + 4, m.f(r + 4) + m.f(r + 0x54) / 5.0);
            } else if s1 == 2 {
                m.sw(0x2CBC, m.w(0x2CBC) + 1);
                m.sf(r + 0x50, -(m.f(r) + m.f(r + 0x10)) / 35.0);
                m.sf(r + 0x58, -(m.f(r + 8) + m.f(r + 0x18)) / 35.0);
            }
        } else if ph == 2 {
            if m.f(r + 4) < 130.0 {
                m.sf(r + 4, m.f(r + 4) + m.f(r + 0x54));
            }
            if dist < 45.0 {
                let a = m.b(r + 0x63);
                m.sb(r + 0x63, if a >= 9 { i32::from(a) - 7 } else { 0 });
                self.sacs[k].col = m.col(r + 0x60);
            }
        }
        sp = [sp[0] + m.f(r + 0x10), sp[1], sp[2] + m.f(r + 0x18)];
        if m.w(r + 0x64) == 1 {
            if dist > 7.0 {
                m.sf(r + 0x10, m.f(r + 0x10) + m.f(r + 0x50) / 2.0);
                m.sf(r + 0x18, m.f(r + 0x18) + m.f(r + 0x58) / 2.0);
            } else {
                m.sw(r + 0x64, 2);
            }
            sp = rot_y(&self.tab, sp, m.w(r + 0x24));
            m.sw(r + 0x24, m.w(r + 0x24) + 6);
        }
        let s = &mut self.sacs[k];
        s.pos[..3].copy_from_slice(&sp);
        if m.w(r + 0x64) > 0 {
            s.rot = cam;
        } else {
            if m.w(r + 0x34) < cam[1] {
                m.sw(r + 0x34, m.w(r + 0x34) + 0x800);
            } else {
                m.sw(r + 0x34, m.w(r + 0x34) - 0x800);
                m.sw(r + 0x3C, 1);
            }
            if m.w(r + 0x30) < cam[0] {
                m.sw(r + 0x30, m.w(r + 0x30) + 0x300);
            } else {
                m.sw(r + 0x30, m.w(r + 0x30) - 0x300);
            }
            s.rot = [m.w(r + 0x30), m.w(r + 0x34), m.w(r + 0x38)];
        }
        if m.h(0x2AA2) >= 0x68 {
            m.sb(r + 0x63, 0);
            s.col = m.col(r + 0x60);
        }
    }

    /// 5.3: create (u 135), slide (u 151+) and tip (u 158+).
    fn result_update(&mut self, cnt: i32) {
        let cam = self.cam;
        if cnt == 0x87 {
            let Some(spec) = self.result_spec.clone() else { return };
            let m = &mut self.m;
            m.sh(0x2D22, if m.w(0x2CAC) < 0xB { 0 } else { 0x100 });
            let handle = u16::from(spec.slot) + m.h(0x2D22) as u16;
            let g = handle >> 8;
            let cell = cell_centre(self.ritual_cell);
            // The fresh card object (0x1CE7F0) [emu]; rot.y for group 0 [H].
            let save = CardObj { position: cell, rotation: [0x4000, if g == 1 { 0 } else { 0x8000 }, 0], colour: [0, 0, 0, 128] };
            m.sw(0x2CB8, i32::from(handle));
            m.sw(0x2AC8, 1);
            for (i, v) in [cell[0], cell[1], cell[2], 1.0].iter().enumerate() {
                m.sf(0x2880 + 4 * i, *v);
            }
            m.sf(0x28F0, 0.0);
            m.sf(0x28F4, 130.0);
            m.sf(0x28F8, 0.0);
            m.sw(0x2910, cam[0]);
            m.sw(0x2914, cam[1]);
            m.sw(0x2918, cam[2]);
            m.sf(0x28D0, m.f(0x2880) / 52.0);
            m.sf(0x28D4, (m.f(0x2884) - m.f(0x28F4)) / 52.0);
            m.sf(0x28D8, m.f(0x2888) / 52.0);
            let obj = CardObj { position: [0.0, 130.0, 0.0], rotation: cam, colour: [0, 0, 0, 0x80] };
            self.result = Some(ResState { handle, card_id: spec.card_id, obj, save });
            return;
        }
        let Some(res) = self.result.as_mut() else { return };
        let m = &mut self.m;
        if cnt >= 0x97 {
            let (dx, dy, dz) = (m.f(0x2880) - m.f(0x28F0), m.f(0x2884) - m.f(0x28F4), m.f(0x2888) - m.f(0x28F8));
            let d = f64::from((dx * dx + dy * dy) + dz * dz).sqrt() as f32;
            if d > 3.0 {
                for c in 0..3 {
                    m.sf(0x28F0 + 4 * c, m.f(0x28F0 + 4 * c) + m.f(0x28D0 + 4 * c));
                }
            } else {
                for c in 0..4 {
                    m.sf(0x28F0 + 4 * c, m.f(0x2880 + 4 * c));
                }
            }
        }
        res.obj.position = [m.f(0x28F0), m.f(0x28F4), m.f(0x28F8)];
        if cnt >= 0x9E {
            let mut rx = m.w(0x2910);
            if rx >= 0x4201 {
                rx -= 0x200;
            } else if rx < 0x3E00 {
                rx += 0x200;
            } else {
                rx = 0x4000;
            }
            m.sw(0x2910, rx);
            m.sw(0x2914, if m.w(0x2914) > 0 { m.w(0x2914) - 0x200 } else { 0 });
        }
        res.obj.rotation = [m.w(0x2910), m.w(0x2914), m.w(0x2918)];
    }

    // ------------------------------------------------------------ update

    /// Module update `u` (1..=216; the counter is the module's own) with
    /// the camera (`eye`, `target`). Returns 0, 2 (u 134: remove the
    /// sacrifices) or 8 (u 216: done; also every later call).
    pub fn update(&mut self, u: u32, eye: [f64; 3], target: [f64; 3]) -> u32 {
        self.additive.clear();
        self.subtractive.clear();
        self.opaque.clear();
        self.counts = [0; FAMILIES];
        if self.done {
            return 8;
        }
        debug_assert_eq!(u, self.counter + 1, "c059 updates must be consecutive");
        self.cam = self.cam_override.unwrap_or_else(|| camera_angles(eye, target));
        let project = projector(eye, target);
        let none = |_: [f64; 3]| -> Option<[f32; 2]> { None };
        let pj: &dyn Fn([f64; 3]) -> Option<[f32; 2]> = match project.as_ref() {
            Some(p) => p,
            None => &none,
        };
        self.director();
        let cnt = self.m.h(0x2AA2);
        self.counter = cnt as u32;
        for i in 0..EN {
            let o = E0 + ES * i;
            let f = self.m.uw(o + 0x3C);
            if self.m.h(o + 0x48) != 0 && f != 0 {
                self.upd_e_slot(o, f);
            }
        }
        for i in 0..QN {
            let o = Q0 + QS * i;
            if self.m.h(o + 0x28) != 0 {
                self.upd_q(o);
            }
        }
        for i in 0..LN {
            let o = L0 + LS * i;
            if self.m.h(o + 0x28) != 0 && self.m.uw(o + 0x1C) != 0 {
                self.upd_l(o);
            }
        }
        for i in 0..KN {
            let o = K0 + KS * i;
            if self.m.h(o + 0x2E) != 0 && self.m.uw(o + 0x24) != 0 {
                self.upd_k(o);
            }
        }
        // Draws: E slots, Q, L, K (the original's submit order).
        for i in 0..EN {
            let o = E0 + ES * i;
            if self.m.h(o + 0x48) == 0 {
                continue;
            }
            let d = self.m.uw(o + 0x38);
            self.sc240 = match d {
                DRW_B => [1.2, 0.1, 1.2],
                DRW_D => [1.3, 0.1, 1.3],
                _ => [1.0; 3],
            };
            self.prim.template();
            match d {
                DRW_A => self.drw_a(pj, o),
                DRW_D => self.drw_d(pj, o),
                DRW_C => self.drw_c(pj, o),
                DRW_B => self.drw_b(pj, o),
                DRW_EG => self.drw_eg(pj, o),
                DRW_FX => self.drw_fx(o),
                _ => {}
            }
        }
        for i in 0..QN {
            let o = Q0 + QS * i;
            if self.m.h(o + 0x28) != 0 && self.m.uw(o + 0x20) != 0 {
                self.prim.template();
                self.drw_q(pj, o);
            }
        }
        for i in 0..LN {
            let o = L0 + LS * i;
            let d = self.m.uw(o + 0x18);
            if self.m.h(o + 0x28) != 0 && d != 0 {
                self.prim.template();
                self.drw_l(pj, o, d);
            }
        }
        for i in 0..KN {
            let o = K0 + KS * i;
            if self.m.h(o + 0x2E) != 0 && self.m.uw(o + 0x20) != 0 {
                self.prim.template();
                self.drw_k(pj, o);
            }
        }
        // Units (section 5). The handler removed the sacrifices after the 2.
        if cnt >= 0x87 {
            for s in &mut self.sacs {
                s.removed = true;
            }
        }
        let life = self.m.w(0x2A50);
        if cnt < 10 {
            for unit in &mut self.others {
                unit.alpha = if unit.alpha >= 21 { unit.alpha - 19 } else { 0 };
                if let Some(s) = unit.model_scale.as_mut() {
                    *s = (*s - 0.18).max(0.0);
                }
            }
            for s in &mut self.sacs {
                if let Some(v) = s.model_scale.as_mut() {
                    *v = (*v - 0.18).max(0.0);
                }
            }
        }
        if cnt < 0x87 {
            for k in 0..3 {
                self.sac_update(k);
            }
            if self.m.w(0x2CBC) >= 9 {
                for k in 0..3 {
                    self.m.sw(0x2650 + 0x70 * k + 0x64, 1);
                }
            }
            self.m.sw(0x2CBC, 0);
        }
        if cnt < 8 {
            self.m.sh(0x2D20, (self.m.h(0x2D20) - 9).max(0));
        }
        if life < 10 {
            for unit in &mut self.others {
                unit.alpha = (u16::from(unit.alpha) + 16).min(0x80) as u8;
                if let Some(s) = unit.model_scale.as_mut() {
                    *s = (*s + 0.15).min(1.0);
                }
            }
            if let Some(r) = self.result.as_mut() {
                r.obj.colour[3] = (u16::from(r.obj.colour[3]) + 16).min(0x80) as u8;
            }
            self.m.sh(0x2D20, (self.m.h(0x2D20) + 16).min(0x80));
        }
        self.light = self.m.b(0x2D20);
        if cnt >= 0x87 {
            self.result_update(cnt);
        }
        if life == 1 {
            if let Some(r) = self.result.as_mut() {
                r.obj = r.save;
            }
        }
        if life != 0 {
            self.m.sw(0x2A50, life - 1);
        }
        let ret = if cnt == 0x86 {
            2
        } else if self.m.w(0x2A50) <= 0 {
            8
        } else {
            0
        };
        if ret == 8 {
            self.done = true;
        }
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The port's overview camera (target the board centre, distance 500,
    /// yaw 90 deg, pitch 33.75 deg).
    fn overview() -> ([f64; 3], [f64; 3]) {
        let t = [0.0; 3];
        (crate::duel_board::orbit_eye_polar(t, [500.0, std::f64::consts::FRAC_PI_2, 33.75f64.to_radians()]), t)
    }

    /// r1-like: Weevil's g1 s1 / s3 / s5 (mask 0x2A0), result on (4, 1).
    fn input() -> RitualInput {
        let h = mask_handles(0x2A0);
        assert_eq!(h, vec![0x101, 0x103, 0x105]);
        // r1's card objects: (-50, 0, 50), (-50, 0, 0), (-150, 0, 0).
        let cells = [(2u8, 4u8), (2, 3), (0, 3)];
        let sacrifices = std::array::from_fn(|k| Sacrifice::on_cell(1, (h[k] & 0xFF) as u8, 425, cells[k], k == 0));
        let others = vec![
            Unit { handle: 0x000, alpha: 128, model_scale: Some(1.0) },
            Unit { handle: 0x100, alpha: 128, model_scale: Some(1.0) },
            Unit { handle: 0x002, alpha: 128, model_scale: None },
        ];
        RitualInput { sacrifices, others, result: Some(ResultSpec { card_id: 0x1A6, slot: 1 }), ritual_cell: (4, 1) }
    }

    fn run_all(seed: u64) -> (C059, Vec<(u32, u32)>, Vec<u8>) {
        let (eye, target) = overview();
        let mut c = C059::new(input(), seed);
        let mut rets = Vec::new();
        let mut light = Vec::new();
        for u in 1..=UPDATES {
            let r = c.update(u, eye, target);
            light.push(c.light());
            if r != 0 {
                rets.push((u, r));
            }
        }
        (c, rets, light)
    }

    #[test]
    fn camera_angles_of_the_overview() {
        let (eye, target) = overview();
        assert_eq!(camera_angles(eye, target), [-6144, 0, 0]);
    }

    #[test]
    fn return_codes() {
        let (mut c, rets, _) = run_all(5);
        assert_eq!(rets, vec![(134, 2), (216, 8)]);
        assert!(c.done());
        let (eye, target) = overview();
        assert_eq!(c.update(217, eye, target), 8);
        assert!(c.additive.is_empty() && c.subtractive.is_empty());
    }

    #[test]
    fn light_timeline() {
        let (_, _, light) = run_all(9);
        assert_eq!(&light[..8], &[119, 110, 101, 92, 83, 74, 65, 65]);
        assert!(light[7..207].iter().all(|&l| l == 65));
        assert_eq!(&light[206..], &[65, 81, 97, 113, 128, 128, 128, 128, 128, 128]);
    }

    #[test]
    fn rand_calls_and_u1_counts() {
        let (eye, target) = overview();
        let mut c = C059::new(input(), 77);
        assert_eq!(c.rand_calls(), 12);
        c.update(1, eye, target);
        assert_eq!(c.rand_calls(), 12 + 43);
        let n = |f| c.count(f);
        assert_eq!((n(Family::A), n(Family::A2), n(Family::B), n(Family::C), n(Family::D)), (16, 12, 36, 21, 16));
        assert_eq!((n(Family::E), n(Family::G), n(Family::F), n(Family::X), n(Family::Q), n(Family::K)), (24, 72, 1, 0, 2, 60));
        assert!(matches!((n(Family::L1), n(Family::L2)), (2, 0) | (0, 1)));
        assert_eq!(c.subtractive.len(), 2);
        assert!(c.opaque.is_empty());
        // F: subtractive, top (16,16,16,0), bottom (32,0,64,60).
        assert_eq!(c.subtractive[0].colour[0], [16, 16, 16, 0]);
        assert_eq!(c.subtractive[0].colour[2], [32, 0, 64, 60]);
        // Additive tris = 2 x the quads except F.
        let q: u32 = c.quad_counts().iter().sum::<u32>() - n(Family::F);
        assert_eq!(c.additive.len() as u32, 2 * q);
    }

    #[test]
    fn sacrifice_path_checkpoints() {
        let (eye, target) = overview();
        let mut c = C059::new(input(), 3);
        let r_xz = |p: [f32; 3]| (p[0] * p[0] + p[2] * p[2]).sqrt();
        let mut prev_r = [0.0f32; 3];
        for u in 1..=REMOVE_U {
            c.update(u, eye, target);
            for k in 0..3 {
                let p = c.sacrifice_pose(k).unwrap();
                let t = [[-150.0f32, 0.0], [150.0, 150.0], [150.0, -150.0]][k];
                let to_t = ((p.position[0] - t[0]).powi(2) + (p.position[2] - t[1]).powi(2)).sqrt();
                if u <= 41 {
                    // Flying out toward the targets (1/35 of the offset per
                    // U), rising to ~30.5.
                    assert_eq!(c.sacrifice_phase(k), 0, "u {u}");
                    assert!(to_t <= prev_r[k] + 1e-3 || u == 1, "u {u} k {k}");
                    assert_ne!(p.rotation, c.angles());
                }
                if u == 41 {
                    // y as drawn (before this update's rise): 40 x 0.743.
                    assert!((p.position[1] - 29.71).abs() < 0.01, "{:?}", p.position);
                    assert!((p.position[0] - t[0]).abs() < 6.0 && (p.position[2] - t[1]).abs() < 6.0, "{:?}", p.position);
                }
                if u == 42 {
                    assert!((p.position[1] - 30.46).abs() < 0.01, "{:?}", p.position);
                }
                if (42..110).contains(&u) {
                    // Phase 1 from the end of u 42: facing the camera from
                    // u 43, circling 6 deg/U from u 44.
                    assert_eq!(c.sacrifice_phase(k), 1, "u {u}");
                    if u >= 43 {
                        assert_eq!(p.rotation, c.angles());
                    }
                }
                if u == 112 {
                    assert_eq!(c.sacrifice_phase(k), 2);
                    assert!(r_xz(p.position) < 12.0, "{:?}", p.position);
                }
                if u == 134 {
                    // 115.9 (rising from u 111) or 112.2 (from u 112).
                    let y = p.position[1];
                    assert!((y - 115.886).abs() < 0.01 || (y - 112.171).abs() < 0.01, "{:?}", p.position);
                }
                assert_eq!(p.colour[3], if u >= 104 { 0 } else { 128 }, "u {u}");
                prev_r[k] = to_t;
            }
        }
        c.update(RESULT_U, eye, target);
        assert!((0..3).all(|k| c.sacrifice_pose(k).is_none()));
    }

    #[test]
    fn result_slide_and_tip() {
        let (eye, target) = overview();
        let mut c = C059::new(input(), 11);
        let cell = cell_centre((4, 1));
        for u in 1..=UPDATES {
            c.update(u, eye, target);
            let r = c.result_pose();
            if u < RESULT_U {
                assert!(r.is_none());
                continue;
            }
            let (h, o) = r.unwrap();
            assert_eq!(h, 0x101);
            if u < 216 {
                assert_eq!(o.colour, [0, 0, 0, 128]);
            }
            if u <= 150 {
                assert_eq!(o.position, [0.0, 130.0, 0.0]);
            }
            if u == 151 {
                assert!((o.position[0] - 0.96).abs() < 0.01 && (o.position[1] - 127.5).abs() < 0.01 && (o.position[2] + 1.92).abs() < 0.01);
            }
            if (151..202).contains(&u) {
                let k = (u - 150) as f32;
                assert!((o.position[1] - (130.0 - 2.5 * k)).abs() < 0.01, "u {u} {:?}", o.position);
            }
            if u == 202 {
                // 52 steps of the f32 velocity land within rounding of the cell.
                assert!((0..3).all(|i| (o.position[i] - cell[i]).abs() < 1e-3), "{:?}", o.position);
            }
            if (203..216).contains(&u) {
                assert_eq!(o.position, cell);
            }
            if u <= 157 {
                assert_eq!(o.rotation, [-6144, 0, 0]);
            }
            if (158..=201).contains(&u) {
                assert_eq!(o.rotation[0], -6144 + 0x200 * (u as i32 - 157), "u {u}");
            }
            if (201..216).contains(&u) {
                assert_eq!(o.rotation, [0x4000, 0, 0]);
            }
            if u == 216 {
                assert_eq!(o, CardObj { position: cell, rotation: [0x4000, 0, 0], colour: [0, 0, 0, 128] });
            }
        }
    }

    #[test]
    fn other_units_fade_and_models_shrink() {
        let (eye, target) = overview();
        let mut c = C059::new(input(), 2);
        for u in 1..=UPDATES {
            c.update(u, eye, target);
            let o = &c.others()[0];
            match u {
                1 => assert_eq!(o.alpha, 109),
                7..=207 => assert_eq!(o.alpha, 0),
                208 => assert_eq!(o.alpha, 16),
                215 | 216 => assert_eq!(o.alpha, 128),
                _ => {}
            }
            let s = o.model_scale.unwrap();
            if (6..208).contains(&u) {
                assert_eq!(s, 0.0, "u {u}");
            }
            if u == 1 {
                assert!((s - 0.82).abs() < 1e-6);
            }
            if u == 213 {
                assert!((s - 0.9).abs() < 1e-5, "{s}");
            }
            if u >= 214 {
                assert_eq!(s, 1.0);
            }
            if u >= 6 {
                assert_eq!(c.sacrifice_model_scale(0), Some(0.0));
            }
            assert_eq!(c.others()[2].model_scale, None);
        }
    }

    #[test]
    fn flashes_and_bolt_quirk_paths_run() {
        let (eye, target) = overview();
        let mut c = C059::new(input(), 1234);
        let mut x = Vec::new();
        for u in 1..=UPDATES {
            c.update(u, eye, target);
            if c.count(Family::X) > 0 {
                x.push(u);
            }
        }
        // Flash 1 u 100-122, flash 2 u 130-152.
        let want: Vec<u32> = (100..=122).chain(130..=152).collect();
        assert_eq!(x, want);
    }

    /// Cross-check against `tools/mf_c059_model.py` (run by hand): env
    /// C059_IN = "seed s / sac handle x y z r g b a / result slot id / cell
    /// c r" lines, C059_OUT = the dump written.
    #[test]
    #[ignore]
    fn dump_for_the_model() {
        let (Ok(inp), Ok(out)) = (std::env::var("C059_IN"), std::env::var("C059_OUT")) else { return };
        let text = std::fs::read_to_string(inp).unwrap();
        let (mut seed, mut sacs, mut result, mut cell) = (0u64, Vec::new(), None, (0u8, 0u8));
        for l in text.lines() {
            let w: Vec<&str> = l.split_whitespace().collect();
            match w.first().copied() {
                Some("seed") => seed = w[1].parse().unwrap(),
                Some("sac") => {
                    let f: Vec<f32> = w[2..].iter().map(|v| v.parse().unwrap()).collect();
                    sacs.push(Sacrifice {
                        handle: w[1].parse().unwrap(),
                        card_id: 0,
                        position: [f[0], f[1], f[2]],
                        colour: [f[3] as u8, f[4] as u8, f[5] as u8, f[6] as u8],
                        model_scale: None,
                    });
                }
                Some("result") => result = Some(ResultSpec { slot: w[1].parse().unwrap(), card_id: w[2].parse().unwrap() }),
                Some("cell") => cell = (w[1].parse().unwrap(), w[2].parse().unwrap()),
                _ => {}
            }
        }
        let sacrifices: [Sacrifice; 3] = sacs.try_into().unwrap();
        let mut c = C059::new(RitualInput { sacrifices, others: Vec::new(), result, ritual_cell: cell }, seed);
        let (eye, target) = overview();
        let mut s = String::new();
        use std::fmt::Write;
        for u in 1..=UPDATES {
            let r = c.update(u, eye, target);
            writeln!(s, "U {u} {r} {} {}", c.light(), c.rand_calls()).unwrap();
            for (tag, list) in [("A", &c.additive), ("S", &c.subtractive)] {
                for q in list.chunks(2) {
                    let v = [q[0].screen[0], q[0].screen[1], q[0].screen[2], q[1].screen[1]];
                    let col = [q[0].colour[0], q[0].colour[1], q[0].colour[2], q[1].colour[1]];
                    let uv = [q[0].uv[0], q[0].uv[1], q[0].uv[2], q[1].uv[1]];
                    write!(s, "{tag} {} {}", q[0].textured as u8, q[0].sheet).unwrap();
                    for i in 0..4 {
                        write!(s, " {} {} {} {} {} {} {} {}", v[i][0], v[i][1], col[i][0], col[i][1], col[i][2], col[i][3], uv[i][0], uv[i][1]).unwrap();
                    }
                    s.push('\n');
                }
            }
            for k in 0..3 {
                if let Some(p) = c.sacrifice_pose(k) {
                    writeln!(s, "K {k} {} {} {} {} {} {} {}", p.position[0], p.position[1], p.position[2], p.rotation[0], p.rotation[1], p.rotation[2], p.colour[3]).unwrap();
                }
            }
            if let Some((_, p)) = c.result_pose() {
                writeln!(s, "R {} {} {} {} {} {} {}", p.position[0], p.position[1], p.position[2], p.rotation[0], p.rotation[1], p.rotation[2], p.colour[3]).unwrap();
            }
        }
        std::fs::write(out, s).unwrap();
    }

    #[test]
    #[ignore = "research print"]
    fn print_coverage() {
        let (eye, target) = overview();
        let mut c = C059::new(input(), 1);
        for u in 1..=60 {
            c.update(u, eye, target);
            if u % 10 == 0 {
                let area = |l: &[FxTri]| -> (usize, f32, f32) {
                    let mut tot = 0.0f32;
                    let mut big = 0.0f32;
                    for t in l {
                        let [a, b, cc] = t.screen;
                        let ar = ((b[0] - a[0]) * (cc[1] - a[1]) - (cc[0] - a[0]) * (b[1] - a[1])).abs() / 2.0;
                        tot += ar * f32::from(t.colour[0][3]) / 128.0;
                        big = big.max(ar);
                    }
                    (l.len(), tot / (512.0 * 512.0), big)
                };
                eprintln!("u {u}: add {:?} sub {:?} light {}", area(&c.additive), area(&c.subtractive), c.light());
            }
        }
    }
}