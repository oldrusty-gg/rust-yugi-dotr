//! CMODULE c509 (0x1FD), the event-9 damage effect (Tremendous Fire,
//! Sparks, Hinotama, ...), as specified in the project notes
//! [F code; emu: the reference model `tools/mf_c509_model.py` equals the
//! interpreter's primitives to 0.0625 px]. World-anchored:
//! - three homing FIREBALLS from the card cell centre, 12 units per update
//!   (straight for 5 updates: up, and 45 degrees up to either side), then
//!   steering toward the victim leader's model point by at most 0xA00 per
//!   Euler axis, stopping when less than 30 units from it;
//! - per flying fireball: the shared 8-particle FLAME system (TEXEFF 1,
//!   rotated quads, additive) and a 10 x 10 CORE (TEXEFF 7, opaque 0x88);
//! - per arrived fireball: EXPLOSION A (TEXEFF 0, additive, alpha 96 -8/U)
//!   and EXPLOSION B (TEXEFF 15 smoke, subtractive, alpha 96 -2/U);
//! - per fireball: the TRAIL, a natural cubic spline through the last <= 8
//!   positions, four screen-space ribbons per segment (additive);
//! - from u_a + 1: the NUMBER ("-1000", c508's layout and rolling), opaque
//!   0x88, never fading.
//!
//! Returns 2 at the first arrival (u_a) and 8 at u_a + 48 (explosion B of the
//! first-arrived fireball runs out). With param bit 16 (the Goblin's Fan
//! path) it returns 8 at the first arrival and draws nothing else.
//!
//! [H / not reproduced]: the quarter-wave sine table is generated as
//! sin(i pi / 2048) in f32 (the game's table at 0x293430 is not embedded);
//! the port's random values (its own LCG, same call order and count); the
//! u-1 flame rotation is 0 (stale stack in the original, 90 / 91 / 73 / 121
//! in the runs); no OT keys / depth sorting (drawn after the scene); the
//! opaque list is meant as colour = texel x vertex / 128 with an alpha cut.
use crate::duel_c602::{billboard_axes, projector};
use crate::duel_c621::FxTri;

/// Safety cap of the original (never reached in the runs).
pub const LIFE: i32 = 180;
/// Updates from the first arrival to the end (explosion B: 96, -2 per U).
pub const TAIL: u32 = 48;

fn lcg(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
    ((*seed >> 32) & 0x7FFF_FFFF) as u32
}

// ---------------------------------------------------------------- EE trig

/// Quarter-wave sine table, 1025 f32 (0x293430) [H: sin(i pi / 2048)].
fn table() -> &'static [f32; 1025] {
    use std::sync::OnceLock;
    static T: OnceLock<[f32; 1025]> = OnceLock::new();
    T.get_or_init(|| std::array::from_fn(|i| (i as f64 * std::f64::consts::PI / 2048.0).sin() as f32))
}

/// 0x20ACC0: 16-bit angle, index a >> 4, linear over a & 15.
fn tsin(a: i32) -> f32 {
    let mut a = a & 0xFFFF;
    if a & 0x8000 != 0 {
        a -= 0x10000;
    }
    let mut s = 1.0f32;
    if a < 0 {
        s = -1.0;
        a = -a;
    }
    if a > 0x4000 {
        a = 0x8000 - a;
    }
    if a & 0x3FFF == 0 {
        return if a == 0x4000 { s } else { 0.0 };
    }
    let t = table();
    let (i, f) = ((a >> 4) as usize, a & 15);
    let v = t[i] * (16 - f) as f32 + t[i + 1] * f as f32;
    v * s / 16.0
}

/// 0x20ABF0.
fn tcos(a: i32) -> f32 {
    tsin((a + 0x4000) & 0xFFFF)
}

const TWO_PI: f32 = 6.283_185_5;

/// atan2f -> trunc(65536 a / 2 pi) & 0xFFFF (0x1A74D8; pi - 1 ulp at (0, -x)).
fn ang16(y: f32, x: f32) -> i32 {
    let mut a = f64::from(y).atan2(f64::from(x)) as f32;
    if y == 0.0 && x < 0.0 {
        a = 3.141_592_5;
    }
    ((65536.0f32 * (a / TWO_PI)) as i32) & 0xFFFF
}

/// 0x20AEA0.
fn sinf(x: f32) -> f32 {
    tsin((0.5f32 + (65536.0f32 * x) / TWO_PI) as i32)
}

/// 0x20AD90.
fn cosf(x: f32) -> f32 {
    tsin((16384.0f32 + (0.5f32 + (65536.0f32 * x) / TWO_PI)) as i32)
}

/// Shortest signed difference from -> to (0x103EC0).
fn sdiff(from: i32, to: i32) -> i32 {
    if from < to {
        let d = to - from;
        if d < 0x8001 { d } else { -(0x10000 - to + from) }
    } else if to < from {
        let d = from - to;
        if d < 0x8001 { -d } else { 0x10000 - from + to }
    } else {
        0
    }
}

fn clamp(v: i32, m: i32) -> i32 {
    if v >= m {
        m
    } else if -m < v {
        v
    } else {
        -m
    }
}

/// 0x20BDA0 (rows).
fn rotm(ax: i32, ay: i32, az: i32) -> [[f32; 3]; 3] {
    let (cx, sx, cy, sy, cz, sz) = (tcos(ax), tsin(ax), tcos(ay), tsin(ay), tcos(az), tsin(az));
    [
        [cy * cz, cy * sz, -sy],
        [cz * (sx * sy) - cx * sz, (sx * sy) * sz + cx * cz, sx * cy],
        [cz * (cx * sy) + sx * sz, (cx * sy) * sz - sx * cz, cx * cy],
    ]
}

/// 0x20C540 row vector x matrix.
fn vmul(v: [f32; 3], m: [[f32; 3]; 3]) -> [f32; 3] {
    std::array::from_fn(|j| (v[1] * m[1][j] + v[0] * m[0][j]) + v[2] * m[2][j])
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    f64::from((dx * dx + dy * dy) + dz * dz).sqrt() as f32
}

/// 0x103EC0(pos, T, v, 0xA00): the new velocity.
fn homing(v: [f32; 3], d: [f32; 3]) -> [f32; 3] {
    let (a0, a1, a2) = (ang16(d[0], d[2]), ang16(d[2], d[1]), ang16(d[1], d[0]));
    let (b0, b1, b2) = (ang16(v[0], v[2]), ang16(v[2], v[1]), ang16(v[1], v[0]));
    let (rx, ry, rz) = (clamp(sdiff(b1, a1), 0xA00), clamp(sdiff(b0, a0), 0xA00), clamp(sdiff(b2, a2), 0xA00));
    vmul(v, rotm(rx & 0xFFFF, ry & 0xFFFF, rz & 0xFFFF))
}

// ---------------------------------------------------------------- camera

/// GS point: x, y in 1/16 px (window offset 1792 included, truncated), w =
/// view depth.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Gs {
    x: i32,
    y: i32,
    w: f32,
}

fn px(x: i32, y: i32) -> [f32; 2] {
    [x as f32 / 16.0 - 1792.0, y as f32 / 16.0 - 1792.0]
}

struct Cam<'a> {
    project: Option<&'a dyn Fn([f64; 3]) -> Option<[f32; 2]>>,
    eye: [f64; 3],
    forward: [f64; 3],
    axes: [[f64; 3]; 3],
}

impl Cam<'_> {
    fn gs(&self, p: [f64; 3]) -> Option<Gs> {
        let s = (self.project?)(p)?;
        let w = (0..3).map(|i| (p[i] - self.eye[i]) * self.forward[i]).sum::<f64>();
        Some(Gs { x: ((f64::from(s[0]) + 1792.0) * 16.0) as i32, y: ((f64::from(s[1]) + 1792.0) * 16.0) as i32, w: w as f32 })
    }

    fn gs32(&self, p: [f32; 3]) -> Option<Gs> {
        if p.iter().any(|v| !v.is_finite()) {
            return None;
        }
        self.gs(p.map(f64::from))
    }

    /// pos + lx right + ly up + lz toward the eye (0x1039A0 billboard node).
    fn bill(&self, pos: [f32; 3], l: [f32; 3]) -> [f64; 3] {
        let [r, u, t] = self.axes;
        std::array::from_fn(|i| f64::from(pos[i]) + f64::from(l[0]) * r[i] + f64::from(l[1]) * u[i] + f64::from(l[2]) * t[i])
    }
}

fn quad(out: &mut Vec<FxTri>, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], c: [[u8; 4]; 4], textured: bool, sheet: u8) {
    out.push(FxTri { screen: [p[0], p[1], p[2]], uv: [uv[0], uv[1], uv[2]], colour: [c[0], c[1], c[2]], textured, sheet });
    out.push(FxTri { screen: [p[1], p[3], p[2]], uv: [uv[1], uv[3], uv[2]], colour: [c[1], c[3], c[2]], textured, sheet });
}

// ---------------------------------------------------------------- particle systems

#[derive(Clone, Copy, Debug, PartialEq)]
struct Particle {
    x: f32,
    y: f32,
    z: f32,
    frame: i32,
    tick: i32,
}

/// A 0x150-byte particle system (0x1E009B0).
#[derive(Clone, Debug, PartialEq)]
struct System {
    row_major: bool,
    respawn: bool,
    rotated: bool,
    base: [f32; 2],
    cell: [i32; 2],
    uv_base: [i32; 2],
    sheet_size: [i32; 2],
    sheet: u8,
    half: f32,
    alpha: u8,
    hold: i32,
    frames: i32,
    range: i32,
    rot: i32,
    p: Vec<Particle>,
}

impl System {
    #[allow(clippy::too_many_arguments)]
    fn new(rng: &mut Rng, row_major: bool, respawn: bool, rotated: bool, base: [f32; 2], cell: [i32; 2], uv_base: [i32; 2], sheet_size: [i32; 2], sheet: u8, half: f32, alpha: u8, spacing: i32, frames: i32, range: i32) -> Self {
        let p = (0..8)
            .map(|i| {
                let x = base[0] + (rng.next() % 20) as f32 - 10.0;
                let y = base[1] + ((rng.next() & 15) as i32 - 4) as f32;
                Particle { x, y, z: 0.0, frame: 0, tick: -spacing * i }
            })
            .collect();
        Self { row_major, respawn, rotated, base, cell, uv_base, sheet_size, sheet, half, alpha, hold: 1, frames, range, rot: 0, p }
    }

    /// Draw at `pos` (node k), scaling the particles first when `mul`.
    fn draw(&mut self, cam: &Cam, rng: &mut Rng, pos: [f32; 3], mul: bool, out: &mut Vec<FxTri>) {
        let rgba = [128, 128, 128, self.alpha];
        let [w, h] = self.cell;
        let [sw, sh] = self.sheet_size;
        let (f20, f21) = if h < w { (1.0, h as f32 / w as f32) } else if w < h { (w as f32 / h as f32, 1.0) } else { (1.0, 1.0) };
        for i in 0..self.p.len() {
            let mut p = self.p[i];
            if mul {
                p.x *= 1.01;
                p.y *= 1.01;
                p.z *= 1.01;
            }
            let f = p.frame;
            let (u0, v0) = if self.row_major {
                (self.uv_base[0] + w * (f % (sw / w)), self.uv_base[1] + h * (f / (sw / w)))
            } else {
                (self.uv_base[0] + w * (f / (sh / h)), self.uv_base[1] + h * (f % (sh / h)))
            };
            let uv = [[u0, v0], [u0 + w - 1, v0], [u0, v0 + h - 1], [u0 + w - 1, v0 + h - 1]].map(|[a, b]| [a as f32 / sw as f32, b as f32 / sh as f32]);
            p.tick += 1;
            if p.tick > self.hold {
                p.tick = 0;
                p.frame += 1;
                if p.frame > self.frames - 1 {
                    if self.respawn {
                        p.x = self.base[0];
                        p.y = self.base[1];
                        if self.range != 0 {
                            p.x = self.base[0] + (rng.next() % (2 * self.range as u32)) as f32 - self.range as f32;
                            p.y = self.base[1] + (rng.next() % (2 * self.range as u32)) as f32 - self.range as f32;
                        }
                    }
                    p.frame -= self.frames;
                }
            }
            self.p[i] = p;
            let s = self.half;
            let l: [[f32; 3]; 4] = if self.rotated {
                [135, 45, 225, 315].map(|da| {
                    let r = 0.017_453_292f32 * ((self.rot + da) % 360) as f32;
                    [p.x + s * cosf(r) * f20, p.y + s * sinf(r) * f21, p.z]
                })
            } else {
                let (sx, sy) = (s * f20, s * f21);
                [[p.x - sx, p.y + sy, p.z], [p.x + sx, p.y + sy, p.z], [p.x - sx, p.y - sy, p.z], [p.x + sx, p.y - sy, p.z]]
            };
            if p.tick < 0 {
                continue;
            }
            let g = l.map(|c| cam.gs(cam.bill(pos, c)));
            let (Some(a), Some(b), Some(c), Some(d)) = (g[0], g[1], g[2], g[3]) else {
                continue;
            };
            quad(out, [a, b, c, d].map(|g| px(g.x, g.y)), uv, [rgba; 4], true, self.sheet);
        }
    }
}

/// The port's random source, counting calls (the original: newlib rand).
#[derive(Clone, Debug, PartialEq)]
struct Rng {
    seed: u64,
    calls: u32,
}

impl Rng {
    fn next(&mut self) -> u32 {
        self.calls += 1;
        lcg(&mut self.seed)
    }
}

// ---------------------------------------------------------------- trail

#[derive(Clone, Debug, PartialEq)]
struct Trail {
    p: [[f32; 3]; 8],
    n: usize,
}

/// 0x102550 + 0x102390: chord-length natural cubic spline through the
/// points, 4 (N - 1) samples (the last point not sampled).
fn spline(p: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let n = p.len();
    let mut t = vec![0.0f32; n];
    for i in 1..n {
        t[i] = t[i - 1] + dist(p[i], p[i - 1]);
    }
    for i in 1..n {
        t[i] /= t[n - 1];
    }
    let mut mm = [vec![0.0f32; n], vec![0.0f32; n], vec![0.0f32; n]];
    for (ax, m) in mm.iter_mut().enumerate() {
        let y: Vec<f32> = p.iter().map(|q| q[ax]).collect();
        let h: Vec<f32> = (0..n - 1).map(|i| t[i + 1] - t[i]).collect();
        let mut dd = vec![0.0f32; n];
        for j in 1..n {
            dd[j] = (y[j] - y[j - 1]) / h[j - 1];
        }
        if n > 2 {
            m[1] = (dd[2] - dd[1]) - h[0] * m[0];
        }
        let mut diag = vec![0.0f32; n];
        diag[1] = 2.0 * (t[2] - t[0]);
        for j in 1..n.saturating_sub(2) {
            let w = h[j] / diag[j];
            m[j + 1] = (dd[j + 2] - dd[j + 1]) - w * m[j];
            diag[j + 1] = 2.0 * (t[j + 2] - t[j]) - w * h[j];
        }
        m[n - 1] = 0.0;
        m[n - 2] -= h[n - 2] * m[n - 1];
        for j in (1..=n - 2).rev() {
            m[j] = (m[j] - h[j] * m[j + 1]) / diag[j];
        }
    }
    let samples = 4 * (n - 1);
    let step = 1.0f32 / (4.0 * (n - 1) as f32);
    let mut s = 0.0f32;
    let mut out = Vec::with_capacity(samples);
    for _ in 0..samples {
        let (mut lo, mut hi) = (0usize, n - 1);
        while lo < hi {
            let mid = (lo + hi) / 2;
            if t[mid] < s {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo = lo.saturating_sub(1);
        out.push(std::array::from_fn(|ax| {
            let (y0, y1) = (p[lo][ax], p[lo + 1][ax]);
            let (m0, m1) = (mm[ax][lo], mm[ax][lo + 1]);
            let hh = t[lo + 1] - t[lo];
            let dx = s - t[lo];
            let acc = (y1 - y0) / hh - hh * (2.0 * m0 + m1);
            let inner = 3.0 * m0 + (dx * (m1 - m0)) / hh;
            y0 + dx * (acc + dx * inner)
        }));
        s += step;
    }
    out
}

impl Trail {
    /// 0x1E01EE0: history shift, spline, four ribbons per segment.
    fn draw(&mut self, cam: &Cam, pos: [f32; 3], out: &mut Vec<FxTri>) {
        if self.n == 0 {
            self.p[2] = self.p[0];
            self.p[0] = pos;
            self.p[1] = std::array::from_fn(|i| self.p[0][i] + (self.p[2][i] - self.p[0][i]) / 2.0);
        } else {
            self.p.copy_within(0..7, 1);
            self.p[0] = pos;
        }
        let n = self.n + 3;
        let nseg = 4 * (n - 1);
        let g: Vec<Option<Gs>> = spline(&self.p[..n]).into_iter().map(|q| cam.gs32(q)).collect();
        let hw: Vec<f32> = (0..nseg)
            .map(|s| {
                let w = g[s].map_or(1.0, |g| g.w);
                (28 - s as i32) as f32 * (0.0357f32 * (500.0f32 / (w / 256.0)))
            })
            .collect();
        for fp in 0..nseg - 1 {
            let (Some(pa), Some(pb)) = (g[fp], g[fp + 1]) else {
                continue;
            };
            let fi = fp as i32;
            let a2 = if fp == 0 { 0 } else { ((2 * (28 - fi)) & 0xFF) as u8 };
            let a3 = if fp == 0 { 0 } else { ((2 * (27 - fi)) & 0xFF) as u8 };
            for sub in 0..4 {
                let sg = if sub % 2 == 0 { 1.0f32 } else { -1.0 };
                let (v0, v1) = if sub < 2 {
                    ((pa.x, (pa.y as f32 + sg * hw[fp]) as i32), (pb.x, (pb.y as f32 + sg * hw[fp + 1]) as i32))
                } else {
                    (((pa.x as f32 + sg * hw[fp]) as i32, pa.y), ((pb.x as f32 + sg * hw[fp + 1]) as i32, pb.y))
                };
                if ![v0.0, v0.1, v1.0, v1.1].iter().all(|c| (24576..=40960).contains(c)) {
                    continue;
                }
                let c = |a| [128, 112, 80, a];
                quad(out, [px(v0.0, v0.1), px(v1.0, v1.1), px(pa.x, pa.y), px(pb.x, pb.y)], [[0.0; 2]; 4], [c(0), c(0), c(a2), c(a3)], false, 0);
            }
        }
        if n < 8 {
            self.n += 1;
        }
    }
}

// ---------------------------------------------------------------- module

#[derive(Clone, Debug, PartialEq)]
pub struct C509 {
    rng: Rng,
    fan: bool,
    /// F (D+0x10B0): bit 0 first arrival seen, bits 10..12 arrived, bit 31 fan end.
    flags: u32,
    age: i32,
    life: i32,
    count: i32,
    pos: [[f32; 3]; 3],
    vel: [[f32; 3]; 3],
    head: [[f32; 3]; 3],
    trails: [Trail; 3],
    expl_a: [System; 3],
    expl_b: [System; 3],
    flames: System,
    /// Number glyphs (sign 0xA / 0xB, digits), rolling v, draws so far.
    glyphs: Vec<i32>,
    roll: Vec<i32>,
    number_flags: u32,
    number_draws: u32,
    rgb: [u8; 3],
    u_a: Option<u32>,
    done: bool,
    /// Additive triangles of the last update (flames, explosion A, trails).
    pub additive: Vec<FxTri>,
    /// Subtractive triangles of the last update (explosion B).
    pub subtractive: Vec<FxTri>,
    /// Opaque ALPHA 0x88 triangles of the last update (cores, number).
    pub opaque: Vec<FxTri>,
}

impl C509 {
    /// Init (`entry(D, param)`): `cell` = the card's (col, row); `param` = the
    /// bare amount, bit 16 = the Fan path; `leader` = the victim leader's
    /// model point (object +0x80).
    pub fn new(cell: (i32, i32), param: u32, leader: [f64; 3], seed: u32) -> Self {
        let mut rng = Rng { seed: u64::from(seed) | 1, calls: 0 };
        let amount = (param & 0xFFFF) as i32;
        let s0 = -amount;
        let fan = param >> 16 != 0;
        let rgb = if s0 < 0 { [176, 80, 96] } else { [96, 176, 112] };
        let expl_a = std::array::from_fn(|_| System::new(&mut rng, true, true, false, [0.0, 15.0], [32, 32], [0, 0], [128, 64], 0, 20.0, 96, 3, 8, 10));
        let expl_b = std::array::from_fn(|_| System::new(&mut rng, false, true, false, [0.0, 20.0], [16, 16], [240, 0], [256, 256], 15, 10.0, 96, 3, 8, 30));
        // Three flame systems are initialised (rand order); only j = 0 is drawn.
        let fl: [System; 3] = std::array::from_fn(|_| System::new(&mut rng, false, false, true, [0.0, 0.0], [24, 32], [0, 0], [128, 64], 1, 20.0, 32, 1, 10, 0));
        let [flames, _, _] = fl;
        let c = [50.0 * (cell.0 - 3) as f32, 0.0, 50.0 * (cell.1 - 3) as f32];
        let t = leader.map(|v| v as f32);
        let yaw = ang16(t[0] - c[0], t[2] - c[2]);
        let (cy, sy) = (tcos(yaw), tsin(yaw));
        let roty = |v: [f32; 3]| [v[0] * cy + v[2] * sy, v[1], -(v[0] * sy) + v[2] * cy];
        let d = 8.485_281f32;
        let vel = [[0.0, 12.0, 0.0], roty([d, d, 0.0]), roty([-d, d, 0.0])];
        let a = amount;
        let g: Vec<i32> = if a >= 1000 {
            vec![a / 1000, a % 1000 / 100, a % 100 / 10, a % 10]
        } else if a >= 100 {
            vec![a / 100, a % 100 / 10, a % 10]
        } else if a >= 10 {
            vec![a / 10, a % 10]
        } else {
            vec![a % 10]
        };
        let mut glyphs = vec![if s0 < 0 { 0xA } else { 0xB }];
        glyphs.extend(g);
        let roll = glyphs.iter().map(|g| 8 + 16 * g).collect();
        Self {
            rng,
            fan,
            flags: 0,
            age: 0,
            life: LIFE,
            count: 4,
            pos: [c; 3],
            vel,
            head: [[0.0; 3]; 3],
            trails: std::array::from_fn(|_| Trail { p: [c; 8], n: 0 }),
            expl_a,
            expl_b,
            flames,
            glyphs,
            roll,
            number_flags: 0x40200,
            number_draws: 0,
            rgb,
            u_a: None,
            done: false,
            additive: Vec::new(),
            subtractive: Vec::new(),
            opaque: Vec::new(),
        }
    }

    /// The first-arrival update (the module returned 2), if seen.
    pub fn u_a(&self) -> Option<u32> {
        self.u_a
    }

    /// The final update (8 returned) once u_a is known (plain path).
    pub fn end_u(&self) -> Option<u32> {
        if self.fan { self.u_a } else { self.u_a.map(|u| u + TAIL) }
    }

    pub fn arrived(&self, k: usize) -> bool {
        self.flags & (1 << (10 + k)) != 0
    }

    /// The fireballs' world positions.
    pub fn positions(&self) -> [[f32; 3]; 3] {
        self.pos
    }

    /// Random values drawn so far (144 at init, then 2 per explosion respawn).
    pub fn rand_calls(&self) -> u32 {
        self.rng.calls
    }

    pub fn done(&self) -> bool {
        self.done
    }

    fn number(&mut self) {
        let n = self.glyphs.len();
        let x0 = 256 - 15 * n as i32;
        let [r, g, b] = self.rgb;
        let c = [r, g, b, 128];
        for i in 0..n {
            let v = if self.number_flags & (1 << 9) != 0 && self.number_flags & (1 << (10 + i)) == 0 {
                let mut v = self.roll[i];
                if v >= 192 {
                    self.roll[i] = 0;
                    v = 0;
                }
                self.roll[i] += 8;
                v
            } else {
                16 * self.glyphs[i] + 8
            } as f32;
            let x = (x0 + 30 * i as i32) as f32;
            let uv = [[0.0, v], [10.0, v], [0.0, v + 16.0], [10.0, v + 16.0]].map(|[a, b]| [a / 256.0, b / 256.0]);
            quad(&mut self.opaque, [[x, 232.0], [x + 30.0, 232.0], [x, 280.0], [x + 30.0, 280.0]], uv, [c; 4], true, 15);
        }
        self.number_draws += 1;
        if matches!(self.number_draws, 4 | 8 | 12 | 16 | 20) {
            self.number_flags |= 1 << (10 + self.number_draws / 4 - 1);
        }
    }

    fn core(cam: &Cam, pos: [f32; 3], out: &mut Vec<FxTri>) {
        let l = [[-5.0, 5.0, 0.0], [5.0, 5.0, 0.0], [-5.0, -5.0, 0.0], [5.0, -5.0, 0.0]];
        let g = l.map(|c| cam.gs(cam.bill(pos, c)));
        let (Some(a), Some(b), Some(c), Some(d)) = (g[0], g[1], g[2], g[3]) else {
            return;
        };
        let uv = [[0.0, 0.0], [64.0, 0.0], [0.0, 64.0], [64.0, 64.0]].map(|[u, v]: [f32; 2]| [u / 128.0, v / 64.0]);
        quad(out, [a, b, c, d].map(|g| px(g.x, g.y)), uv, [[128, 128, 128, 128]; 4], true, 7);
    }

    /// Module update `u` (1, 2, ..): `leader` = the victim leader's model
    /// point this update, (`eye`, `target`) the camera. Returns 0, 2 (first
    /// arrival) or 8 (done; also every later call).
    pub fn update(&mut self, u: u32, leader: [f64; 3], eye: [f64; 3], target: [f64; 3]) -> u32 {
        self.additive.clear();
        self.subtractive.clear();
        self.opaque.clear();
        if self.done {
            return 8;
        }
        let project = projector(eye, target);
        let project_dyn = project.as_ref().map(|p| p as &dyn Fn([f64; 3]) -> Option<[f32; 2]>);
        let fwd = {
            let f = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
            let l = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt().max(1e-12);
            f.map(|v| v / l)
        };
        let cam = Cam { project: project_dyn, eye, forward: fwd, axes: billboard_axes(eye, target) };
        let t = leader.map(|v| v as f32);
        // Loop 1: move, head, flame angle.
        for k in 0..3 {
            if !self.arrived(k) {
                if self.age < 5 {
                    self.pos[k] = std::array::from_fn(|i| self.pos[k][i] + self.vel[k][i]);
                    self.head[k] = std::array::from_fn(|i| t[i] + self.pos[k][i]);
                } else {
                    let d: [f32; 3] = std::array::from_fn(|i| t[i] - self.pos[k][i]);
                    let nv = homing(self.vel[k], d);
                    self.pos[k] = std::array::from_fn(|i| self.pos[k][i] + nv[i]);
                    self.vel[k] = nv;
                    self.head[k] = std::array::from_fn(|i| d[i] + self.pos[k][i]);
                }
            }
            if k == 0 {
                // 0x1037C0 on trail 0's newest two points; u 1: 0 [H].
                let ga = cam.gs32(self.trails[0].p[0]);
                let gb = cam.gs32(self.trails[0].p[1]);
                let (dx, dy) = match (ga, gb) {
                    (Some(a), Some(b)) => ((a.x - b.x) >> 4, (a.y - b.y) >> 4),
                    _ => (0, 0),
                };
                self.flames.rot = if u == 1 { 0 } else { (ang16(dx as f32, dy as f32) * 360) >> 16 };
            }
        }
        if self.flags & 1 != 0 {
            if self.count != 0 {
                self.count -= 1;
            } else {
                self.flags |= 0x1C00;
            }
        }
        if !self.fan && self.flags & 1 != 0 {
            self.number();
        }
        let mut ret = None;
        for k in 0..3 {
            if dist(self.head[k], self.pos[k]) < 30.0 {
                self.flags |= 1 << (10 + k);
                if self.fan && self.flags & 0x8000_0000 == 0 {
                    self.flags |= 0x8000_0000;
                    ret = Some(8);
                    break;
                }
            }
            let pos = self.pos[k];
            if self.arrived(k) {
                self.expl_a[k].draw(&cam, &mut self.rng, pos, false, &mut self.additive);
                self.expl_a[k].alpha = self.expl_a[k].alpha.saturating_sub(8);
                self.expl_b[k].draw(&cam, &mut self.rng, pos, true, &mut self.subtractive);
                if self.expl_b[k].alpha >= 2 {
                    self.expl_b[k].alpha -= 2;
                } else {
                    self.expl_b[k].alpha = 0;
                    ret = Some(8);
                    break;
                }
            } else {
                self.flames.draw(&cam, &mut self.rng, pos, false, &mut self.additive);
                Self::core(&cam, pos, &mut self.opaque);
            }
            self.trails[k].draw(&cam, pos, &mut self.additive);
        }
        let ret = match ret {
            Some(r) => r,
            None if !self.fan && self.flags & 1 == 0 && self.flags & 0x1C00 != 0 => {
                self.flags |= 1;
                self.u_a = Some(u);
                2
            }
            None => {
                self.age += 1;
                self.life -= 1;
                if self.life <= 0 { 8 } else { 0 }
            }
        };
        if ret == 8 {
            if self.fan && self.u_a.is_none() {
                self.u_a = Some(u);
            }
            self.done = true;
        }
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A high camera over the board (all of the board in view).
    fn cam() -> ([f64; 3], [f64; 3]) {
        ([0.0, 650.0, 520.0], [0.0, 0.0, 0.0])
    }

    const PLAYER: [f64; 3] = [0.0, 45.86, 150.15];
    const WEEVIL: [f64; 3] = [-100.0, 51.71, -151.25];

    fn run(cell: (i32, i32), param: u32, leader: [f64; 3]) -> (C509, Vec<u32>) {
        let (eye, target) = cam();
        let mut c = C509::new(cell, param, leader, 7);
        let mut rets = Vec::new();
        for u in 1..=200 {
            let r = c.update(u, leader, eye, target);
            rets.push(r);
            if r == 8 {
                break;
            }
        }
        (c, rets)
    }

    #[test]
    fn trig_and_angles() {
        assert_eq!(tsin(0x4000), 1.0);
        assert_eq!(tsin(0x8000), 0.0);
        assert_eq!(tcos(0), 1.0);
        assert!((tsin(0x2000) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        assert_eq!(ang16(1.0, 0.0), 0x4000);
        assert_eq!(ang16(0.0, -1.0), 0x7FFF);
        assert_eq!(sdiff(0xFF00, 0x100), 0x200);
        assert_eq!(sdiff(0x100, 0xFF00), -0x200);
    }

    #[test]
    fn rand_calls_144_at_init() {
        let c = C509::new((4, 1), 1000, PLAYER, 3);
        assert_eq!(c.rand_calls(), 144);
        assert_eq!(c.flames.p.iter().map(|p| p.tick).collect::<Vec<_>>(), (0..8).map(|i| -i).collect::<Vec<_>>());
        assert_eq!(c.expl_a[2].p[7].tick, -21);
    }

    #[test]
    fn fireball_flight_follows_the_note() {
        let (eye, target) = cam();
        let mut c = C509::new((4, 1), 1000, PLAYER, 1);
        let mut prev = c.positions();
        for u in 1..=40 {
            let r = c.update(u, PLAYER, eye, target);
            let p = c.positions();
            for k in 0..3 {
                if !c.arrived(k) {
                    // 12 units per update.
                    assert!((dist(p[k], prev[k]) - 12.0).abs() < 1e-3, "u {u} k {k}");
                }
            }
            if u <= 5 {
                // Straight: the velocities do not change.
                assert_eq!(c.vel[0], [0.0, 12.0, 0.0]);
                let d: [f32; 3] = std::array::from_fn(|i| p[1][i] - prev[1][i]);
                assert!((d[1] - 8.485_281).abs() < 1e-4);
            }
            if u == 1 {
                assert_eq!(p[0], [50.0, 12.0, -100.0]);
                assert!((p[1][0] - 58.3).abs() < 0.05 && (p[1][2] + 98.3).abs() < 0.05, "{:?}", p[1]);
            }
            if u == 5 {
                assert!((p[0][1] - 60.0).abs() < 1e-4);
                assert!((p[1][0] - 91.6).abs() < 0.06 && (p[1][1] - 42.4).abs() < 0.06 && (p[1][2] + 91.7).abs() < 0.06, "{:?}", p[1]);
            }
            if u == 10 {
                assert!((p[0][0] - 33.9).abs() < 0.06 && (p[0][1] - 91.9).abs() < 0.06 && (p[0][2] + 59.1).abs() < 0.06, "{:?}", p[0]);
            }
            if u == 27 {
                assert_eq!(r, 2);
                assert!((p[0][0] - 2.7).abs() < 0.06 && (p[0][1] - 48.7).abs() < 0.06 && (p[0][2] - 137.4).abs() < 0.06, "{:?}", p[0]);
                for k in 0..3 {
                    assert!(dist(p[k], PLAYER.map(|v| v as f32)) < 30.0);
                }
            }
            prev = p;
        }
        // The steering is capped at 0xA00 per Euler axis.
        let v = [5.0f32, 3.0, -10.0];
        let n = homing(v, [-v[0], -v[1], -v[2]]);
        let ang = |a: [f32; 3]| [ang16(a[0], a[2]), ang16(a[2], a[1]), ang16(a[1], a[0])];
        let (a, b) = (ang(v), ang(n));
        assert!((0..3).any(|i| sdiff(a[i], b[i]) != 0));
        // Each per-axis rotation is at most 0xA00 (14.06 degrees): the turn
        // of the vector is bounded by three of them.
        let cosang = (v[0] * n[0] + v[1] * n[1] + v[2] * n[2]) / (dist(v, [0.0; 3]) * dist(n, [0.0; 3]));
        assert!(cosang > (3.0 * 14.07f32).to_radians().cos(), "{cosang}");
        assert!((dist(n, [0.0; 3]) - dist(v, [0.0; 3])).abs() < 1e-3);
    }

    #[test]
    fn return_codes_for_the_runs() {
        // r_o: (4,1) -> the player's leader, u_a 27, end 75.
        let (c, rets) = run((4, 1), 1000, PLAYER);
        assert_eq!(c.u_a(), Some(27));
        assert_eq!(rets.len(), 75);
        assert_eq!(rets.iter().enumerate().filter(|&(_, &r)| r != 0).map(|(i, &r)| (i + 1, r)).collect::<Vec<_>>(), vec![(27, 2), (75, 8)]);
        // r_w: (3,4) -> Weevil's leader, u_a 24 (k 2 at 24, 0 and 1 at 25), end 72.
        let (eye, target) = cam();
        let mut c = C509::new((3, 4), 1000, WEEVIL, 9);
        for u in 1..=72 {
            let r = c.update(u, WEEVIL, eye, target);
            if u == 24 {
                assert_eq!((r, c.arrived(0), c.arrived(1), c.arrived(2)), (2, false, false, true));
            }
            if u == 25 {
                assert!(c.arrived(0) && c.arrived(1));
            }
            assert_eq!(r, if u == 24 { 2 } else if u == 72 { 8 } else { 0 }, "u {u}");
        }
        assert_eq!(c.end_u(), Some(72));
        // r_s: (0,5) -> Weevil, 500: u_a 27.
        let (c, rets) = run((0, 5), 500, WEEVIL);
        assert_eq!((c.u_a(), rets.len()), (Some(27), 75));
        assert_eq!(c.rand_calls(), 324);
    }

    #[test]
    fn explosion_alphas_and_number() {
        let (eye, target) = cam();
        let mut c = C509::new((3, 4), 1000, WEEVIL, 9);
        let alpha = |t: &[FxTri], sheet: u8| t.iter().find(|x| x.sheet == sheet && x.textured).map(|x| x.colour[0][3]);
        for u in 1..=72 {
            c.update(u, WEEVIL, eye, target);
            if u < 24 {
                assert!(c.subtractive.is_empty() && !c.opaque.iter().any(|t| t.sheet == 15));
                // Flames and cores while flying.
                assert!(c.opaque.iter().any(|t| t.sheet == 7));
            }
            if u >= 24 {
                let n = u - 24;
                // Fireball 2 arrived first: after its draw n, B = 96 - 2 (n + 1)
                // (drawn 96 .. 2, 0 at the final update), A = 96 - 8 (n + 1) to 0.
                assert_eq!(u32::from(c.expl_b[2].alpha), 94u32.saturating_sub(2 * n), "u {u}");
                assert_eq!(u32::from(c.expl_a[2].alpha), 88u32.saturating_sub(8 * n), "u {u}");
            }
            if u == 24 {
                // Only fireball 2 explodes: everything drawn at 96.
                assert!(!c.subtractive.is_empty() && c.subtractive.iter().all(|t| t.colour[0][3] == 96));
                assert!(c.additive.iter().filter(|t| t.textured && t.sheet == 0).all(|t| t.colour[0][3] == 96));
            }
            if u == 37 {
                // A of fireball 2 at 0 from its 13th draw, still sent.
                assert!(alpha(&c.additive, 0).is_some());
            }
            if u == 72 {
                assert_eq!(c.subtractive.iter().map(|t| t.colour[0][3]).min(), Some(0));
            }
            if u == 24 {
                assert!(c.opaque.iter().all(|t| t.sheet != 15));
            }
            if u > 24 {
                // "-1000": 5 glyphs, never fading, from u_a + 1.
                let num: Vec<_> = c.opaque.iter().filter(|t| t.sheet == 15).collect();
                assert_eq!(num.len(), 10, "u {u}");
                assert!(num.iter().all(|t| t.colour[0] == [176, 80, 96, 128]));
                assert_eq!(num[0].screen[0], [181.0, 232.0]);
            }
            if u == 24 + 1 + 20 {
                // All glyphs settled: '-' (v 168), '1', '0', '0', '0'.
                let v: Vec<f32> = c.opaque.iter().filter(|t| t.sheet == 15).step_by(2).map(|t| t.uv[0][1] * 256.0).collect();
                assert_eq!(v, vec![168.0, 24.0, 8.0, 8.0, 8.0]);
            }
        }
        assert!(c.done());
        // Explosion A system's alpha bottoms at 0.
        assert_eq!(c.expl_a[2].alpha, 0);
        assert_eq!(c.expl_b[2].alpha, 0);
    }

    #[test]
    fn trail_draws_and_vanishes_after_arrival() {
        let (eye, target) = cam();
        let mut c = C509::new((4, 1), 1000, PLAYER, 2);
        c.update(1, PLAYER, eye, target);
        let trail = |c: &C509| c.additive.iter().filter(|t| !t.textured).count();
        // u 1: N 3, 7 segments, 4 quads each (minus the culled), 3 fireballs.
        assert!(trail(&c) > 0 && trail(&c) <= 3 * 2 * 4 * 7);
        for u in 2..=27 {
            c.update(u, PLAYER, eye, target);
        }
        assert!(trail(&c) > 0);
        c.update(28, PLAYER, eye, target);
        assert_eq!(trail(&c), 0);
    }

    #[test]
    fn fan_path_ends_at_the_first_arrival() {
        let (c, rets) = run((4, 1), 1000 | 0x1_0000, PLAYER);
        assert_eq!(rets.len(), 27);
        assert_eq!(*rets.last().unwrap(), 8);
        assert!(rets[..26].iter().all(|&r| r == 0));
        assert!(c.subtractive.is_empty() && c.opaque.iter().all(|t| t.sheet != 15));
        assert_eq!(c.end_u(), Some(27));
        let mut c = c;
        let (eye, target) = cam();
        assert_eq!(c.update(28, PLAYER, eye, target), 8);
    }
}
