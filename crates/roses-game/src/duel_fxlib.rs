//! Shared pieces of the c555 / c505 / c546 / c507 / c533 / c537 ports, as
//! the reference models' `tools/mf_fxlib.py` (the project notes
//! sections 9-15): the EE's table trig (0x20ACC0 tsin, 0x20AD90 / 0x20AEA0
//! cosf / sinf, 0x1A74D8 atan2 -> 16-bit angle), 0x20BDA0 rotations, 0x20C540
//! row-vector products, 0x103EC0 homing, the GS 12.4 truncated projection of
//! 0x204D80 through the board camera, billboard nodes, the screen-anchored
//! depth-800 plane, the effect library's particle system 0x1E009B0 (+ the
//! frame animator / respawn 0x105260), newlib's rand / srand and the
//! decay / rise steps 0x103470 / 0x1032C0.
//!
//! [H / not reproduced]: the quarter-wave sine table is generated as
//! sin(i pi / 2048) in f32 (the game's table at 0x293430 is not embedded),
//! so table trig may differ from the game's by an ulp; no OT keys / real Z
//! (the original depth-sorts world layers by mean w; the port draws the
//! lists after the scene).
use crate::duel_c602::{billboard_axes, projector};
use crate::duel_c621::FxTri;

// ---------------------------------------------------------------- random

/// newlib rand (0x19AE88) / srand (0x19AE70): the port's random source for
/// the game-RNG modules ([H]: its own seed, same call order and count) and
/// the exact sequence for c533 (which seeds it itself).
#[derive(Clone, Debug, PartialEq)]
pub struct Rng {
    state: u64,
    /// Values drawn so far.
    pub calls: u32,
}

impl Rng {
    /// The port's stream (seed forced odd).
    pub fn new(seed: u32) -> Self {
        Self { state: u64::from(seed) | 1, calls: 0 }
    }

    /// srand(v): the state becomes v.
    pub fn srand(&mut self, v: u32) {
        self.state = u64::from(v);
    }

    /// The raw 64-bit state.
    pub fn state(&self) -> u64 {
        self.state
    }

    /// 0..0x7FFFFFFF.
    pub fn next(&mut self) -> u32 {
        self.calls += 1;
        self.state = self.state.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
        ((self.state >> 32) & 0x7FFF_FFFF) as u32
    }
}

/// C remainder of a non-negative rand value by `m` (either sign), as i32.
pub(crate) fn rmod(r: u32, m: i32) -> i32 {
    (r % m.unsigned_abs()) as i32
}

// ---------------------------------------------------------------- EE trig

/// Quarter-wave sine table, 1025 f32 (0x293430) [H: sin(i pi / 2048)].
fn table() -> &'static [f32; 1025] {
    use std::sync::OnceLock;
    static T: OnceLock<[f32; 1025]> = OnceLock::new();
    T.get_or_init(|| std::array::from_fn(|i| (i as f64 * std::f64::consts::PI / 2048.0).sin() as f32))
}

/// 0x20ACC0: 16-bit angle, index a >> 4, linear over a & 15.
pub(crate) fn tsin(a: i32) -> f32 {
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
pub(crate) fn tcos(a: i32) -> f32 {
    tsin((a + 0x4000) & 0xFFFF)
}

pub(crate) const TWO_PI: f32 = 6.283_185_5;
/// 0x3C8EFA35.
pub(crate) const DEG: f32 = 0.017_453_292;

/// 0x20AEA0.
pub(crate) fn sinf(x: f32) -> f32 {
    tsin((0.5f32 + (65536.0f32 * x) / TWO_PI) as i32)
}

/// 0x20AD90.
pub(crate) fn cosf(x: f32) -> f32 {
    tsin((16384.0f32 + (0.5f32 + (65536.0f32 * x) / TWO_PI)) as i32)
}

/// atan2f -> trunc(65536 a / 2 pi) & 0xFFFF (0x1A74D8; pi - 1 ulp at (0, -x)).
pub(crate) fn ang16(y: f32, x: f32) -> i32 {
    let mut a = f64::from(y).atan2(f64::from(x)) as f32;
    if y == 0.0 && x < 0.0 {
        a = 3.141_592_5;
    }
    ((65536.0f32 * (a / TWO_PI)) as i32) & 0xFFFF
}

/// Shortest signed difference from -> to (0x103EC0).
pub(crate) fn sdiff(from: i32, to: i32) -> i32 {
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

/// 0x20BDA0 with the table trig (rows, row vectors).
pub(crate) fn rotm(ax: i32, ay: i32, az: i32) -> [[f32; 3]; 3] {
    let (cx, sx, cy, sy, cz, sz) = (tcos(ax), tsin(ax), tcos(ay), tsin(ay), tcos(az), tsin(az));
    [
        [cy * cz, cy * sz, -sy],
        [cz * (sx * sy) - cx * sz, (sx * sy) * sz + cx * cz, sx * cy],
        [cz * (cx * sy) + sx * sz, (cx * sy) * sz - sx * cz, cx * cy],
    ]
}

/// 0x20BDA0 with host trig in f64 (the screen-anchored libraries' rays).
pub(crate) fn rotm64(a: i32, b: i32, c: i32) -> [[f64; 3]; 3] {
    let t = |v: i32| f64::from(v) * std::f64::consts::TAU / 65536.0;
    let (sa, ca) = t(a).sin_cos();
    let (sb, cb) = t(b).sin_cos();
    let (sc, cc) = t(c).sin_cos();
    [
        [cb * cc, cb * sc, -sb],
        [sa * sb * cc - ca * sc, sa * sb * sc + ca * cc, sa * cb],
        [ca * sb * cc + sa * sc, ca * sb * sc - sa * cc, ca * cb],
    ]
}

/// 0x20C540 row vector x matrix (f32).
pub(crate) fn vmul(v: [f32; 3], m: [[f32; 3]; 3]) -> [f32; 3] {
    std::array::from_fn(|j| (v[1] * m[1][j] + v[0] * m[0][j]) + v[2] * m[2][j])
}

/// 0x20C870.
pub(crate) fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    ((dx * dx + dy * dy) + dz * dz).sqrt()
}

/// 0x103EC0(pos, T, v, max): the new velocity (`d` = T - pos), each Euler
/// axis turned by at most `max`.
pub(crate) fn homing(v: [f32; 3], d: [f32; 3], max: i32) -> [f32; 3] {
    let (a0, a1, a2) = (ang16(d[0], d[2]), ang16(d[2], d[1]), ang16(d[1], d[0]));
    let (b0, b1, b2) = (ang16(v[0], v[2]), ang16(v[2], v[1]), ang16(v[1], v[0]));
    let (rx, ry, rz) = (clamp(sdiff(b1, a1), max), clamp(sdiff(b0, a0), max), clamp(sdiff(b2, a2), max));
    vmul(v, rotm(rx & 0xFFFF, ry & 0xFFFF, rz & 0xFFFF))
}

/// 0x103470(a, 0, st).
pub(crate) fn decay(a: u8, st: u8) -> u8 {
    a.saturating_sub(st)
}

/// 0x1032C0(a, max, st).
pub(crate) fn rise_u8(a: u8, max: u8, st: u8) -> u8 {
    if i32::from(a) <= i32::from(max) - i32::from(st) { a + st } else { max }
}

/// A cell's centre (0x1E2370(col, row, 0)).
pub fn cell_centre(col: i32, row: i32) -> [f32; 3] {
    [50.0 * (col - 3) as f32, 0.0, 50.0 * (row - 3) as f32]
}

// ---------------------------------------------------------------- camera

/// A GS vertex: 12.4 fixed point with the 1792 window offset, truncated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Gs {
    pub x: i32,
    pub y: i32,
}

impl Gs {
    pub fn px(self) -> [f32; 2] {
        [self.x as f32 / 16.0 - 1792.0, self.y as f32 / 16.0 - 1792.0]
    }
}

/// The board camera of one update: 0x204D80 projection and the billboard
/// axes (0x1039A0: screen right, screen up, toward the eye).
pub(crate) struct Cam {
    project: Option<Box<dyn Fn([f64; 3]) -> Option<[f32; 2]>>>,
    pub axes: [[f64; 3]; 3],
}

impl Cam {
    pub fn new(eye: [f64; 3], target: [f64; 3]) -> Self {
        let project = projector(eye, target).map(|p| Box::new(p) as Box<dyn Fn([f64; 3]) -> Option<[f32; 2]>>);
        Self { project, axes: billboard_axes(eye, target) }
    }

    pub fn gs(&self, p: [f64; 3]) -> Option<Gs> {
        if p.iter().any(|v| !v.is_finite()) {
            return None;
        }
        let s = (self.project.as_ref()?)(p)?;
        Some(Gs { x: ((f64::from(s[0]) + 1792.0) * 16.0) as i32, y: ((f64::from(s[1]) + 1792.0) * 16.0) as i32 })
    }

    pub fn gs32(&self, p: [f32; 3]) -> Option<Gs> {
        self.gs(p.map(f64::from))
    }

    /// Four world points -> pixels, None when any fails the cull.
    pub fn quad(&self, w: [[f64; 3]; 4]) -> Option<[[f32; 2]; 4]> {
        Some([self.gs(w[0])?.px(), self.gs(w[1])?.px(), self.gs(w[2])?.px(), self.gs(w[3])?.px()])
    }

    /// Billboard node at `pos`, local `l` scaled per axis by `s`.
    pub fn bill(&self, pos: [f32; 3], l: [f32; 3], s: [f32; 3]) -> [f64; 3] {
        let [r, u, t] = self.axes;
        let l: [f64; 3] = std::array::from_fn(|i| f64::from(l[i]) * f64::from(s[i]));
        std::array::from_fn(|i| f64::from(pos[i]) + l[0] * r[i] + l[1] * u[i] + l[2] * t[i])
    }
}

/// A node at view `anchor` on the depth-800 plane (0x103920), scale `s`:
/// view = (ax + s lx, ay - s ly, 800 - s lz), X = 256 + 800 vx / vz, Y = 256
/// + 1074.2855 vy / vz, GS 1/16 floor; None when 0x204D80 culls it.
pub(crate) fn view_proj(anchor: [f64; 2], s: f64, l: [f64; 3]) -> Option<[f32; 2]> {
    let (vx, vy, vz) = (anchor[0] + s * l[0], anchor[1] - s * l[1], 800.0 - s * l[2]);
    if vz <= 39.45 || vz >= 8188.0 {
        return None;
    }
    let x = ((256.0 + 800.0 * vx / vz) * 16.0).floor() / 16.0;
    let y = ((256.0 + 1_074.285_522_460_937_5 * vy / vz) * 16.0).floor() / 16.0;
    let ok = |v: f64| (-1792.0..=2303.0).contains(&v);
    (ok(x) && ok(y)).then_some([x as f32, y as f32])
}

// ---------------------------------------------------------------- primitives

/// A 4-vertex strip (v0 v1 v2 / v1 v2 v3) as two triangles.
pub(crate) fn quad(out: &mut Vec<FxTri>, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], c: [[u8; 4]; 4], textured: bool, sheet: u8) {
    out.push(FxTri { screen: [p[0], p[1], p[2]], uv: [uv[0], uv[1], uv[2]], colour: [c[0], c[1], c[2]], textured, sheet });
    out.push(FxTri { screen: [p[1], p[3], p[2]], uv: [uv[1], uv[3], uv[2]], colour: [c[1], c[3], c[2]], textured, sheet });
}

/// Texel UVs normalised by a sheet of `size` (the project notes).
pub(crate) fn uvn(p: [[i32; 2]; 4], size: [i32; 2]) -> [[f32; 2]; 4] {
    p.map(|[u, v]| [u as f32 / size[0] as f32, v as f32 / size[1] as f32])
}

/// TEXEFF record sizes (texels).
pub(crate) const SHEET_128X64: [i32; 2] = [128, 64];
pub(crate) const SHEET_256: [i32; 2] = [256, 256];

/// The full-screen flash (0x1E002A0 / 0x1E00270): (0,0)-(512,512), Z max,
/// key 0, untextured, (128,128,128,128), additive.
pub(crate) fn flash(out: &mut Vec<FxTri>) {
    let c = [128, 128, 128, 128];
    quad(out, [[0.0, 0.0], [512.0, 0.0], [0.0, 512.0], [512.0, 512.0]], [[0.0; 2]; 4], [c; 4], false, 0);
}

// ---------------------------------------------------------------- particle system

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Particle {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub f: i32,
    pub t: i32,
}

/// The library particle system 0x1E009B0 (c509 / c545 / c546 / c507 / c555's
/// smoke): frame animator and respawn 0x105260.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PSys {
    /// bit 1 row-major frames, bit 2 respawn, bit 7 positions scaled.
    pub flags: u32,
    pub base: [f32; 3],
    pub cell: [i32; 2],
    pub uvb: [i32; 2],
    /// Cells per row (row-major) or per column (column-major).
    pub per: i32,
    pub sheet: u8,
    pub sheet_size: [i32; 2],
    pub half: f32,
    pub rgb: [u8; 3],
    pub a: u8,
    pub hold: i32,
    /// Respawn when the frame passes `max_frame`, then frame -= `wrap`.
    pub max_frame: i32,
    pub wrap: i32,
    pub rx: i32,
    pub ry: i32,
    pub p: Vec<Particle>,
}

impl PSys {
    /// The library init: per particle x = base.x + rand % 20 - 10, y =
    /// base.y + `yinit`(rand), z = base.z, frame `f0`, tick -spacing i.
    #[allow(clippy::too_many_arguments)]
    pub fn lib(
        rng: &mut dyn FnMut() -> u32,
        flags: u32,
        base: [f32; 3],
        cell: [i32; 2],
        uvb: [i32; 2],
        per: i32,
        sheet: u8,
        sheet_size: [i32; 2],
        half: f32,
        rgba: [u8; 4],
        n: usize,
        spacing: i32,
        hold: i32,
        frames: i32,
        rx: i32,
        ry: i32,
        yinit: impl Fn(u32) -> i32,
    ) -> Self {
        let p = (0..n as i32)
            .map(|i| {
                let x = base[0] + (rmod(rng(), 20) - 10) as f32;
                let y = base[1] + yinit(rng()) as f32;
                Particle { x, y, z: base[2], f: 0, t: -spacing * i }
            })
            .collect();
        Self { flags, base, cell, uvb, per, sheet, sheet_size, half, rgb: [rgba[0], rgba[1], rgba[2]], a: rgba[3], hold, max_frame: frames - 1, wrap: frames, rx, ry, p }
    }

    fn respawn(&self, p: &mut Particle, rng: &mut dyn FnMut() -> u32) {
        p.x = self.base[0];
        p.y = self.base[1];
        if self.rx != 0 {
            p.x = self.base[0] + (rmod(rng(), 2 * self.rx) - self.rx) as f32;
        }
        if self.ry != 0 {
            p.y = self.base[1] + (rmod(rng(), 2 * self.ry) - self.ry) as f32;
        }
        if self.ry < 0 {
            p.y = self.base[1] + rmod(rng(), self.ry) as f32;
        }
    }

    /// Draws every particle at `origin` + (local x `scale`) on the billboard;
    /// `mul` scales the positions first (flags bit 7). Returns the quads sent.
    pub fn draw(&mut self, cam: &Cam, rng: &mut dyn FnMut() -> u32, origin: [f32; 3], scale: f32, mul: Option<f32>, out: &mut Vec<FxTri>) -> usize {
        let rgba = [self.rgb[0], self.rgb[1], self.rgb[2], self.a];
        let [w, h] = self.cell;
        let (f20, f21) = if h < w { (1.0, h as f32 / w as f32) } else if w < h { (w as f32 / h as f32, 1.0) } else { (1.0, 1.0) };
        let mut n = 0;
        for i in 0..self.p.len() {
            let mut p = self.p[i];
            if let Some(m) = mul {
                if self.flags & 0x80 != 0 {
                    p.x *= m;
                    p.y *= m;
                    p.z *= m;
                }
            }
            let f = p.f;
            let (u0, v0) = if self.flags & 2 != 0 {
                (self.uvb[0] + w * (f % self.per), self.uvb[1] + h * (f / self.per))
            } else {
                (self.uvb[0] + w * (f / self.per), self.uvb[1] + h * (f % self.per))
            };
            let uv = uvn([[u0, v0], [u0 + w - 1, v0], [u0, v0 + h - 1], [u0 + w - 1, v0 + h - 1]], self.sheet_size);
            p.t += 1;
            if p.t > self.hold {
                p.t = 0;
                p.f += 1;
                if p.f > self.max_frame {
                    if self.flags & 4 != 0 {
                        self.respawn(&mut p, rng);
                    }
                    p.f -= self.wrap;
                }
            }
            self.p[i] = p;
            let (sx, sy) = (self.half * f20, self.half * f21);
            let l = [[p.x - sx, p.y + sy, p.z], [p.x + sx, p.y + sy, p.z], [p.x - sx, p.y - sy, p.z], [p.x + sx, p.y - sy, p.z]];
            if p.t < 0 {
                continue;
            }
            let Some(v) = cam.quad(l.map(|c| cam.bill(origin, c, [scale; 3]))) else {
                continue;
            };
            quad(out, v, uv, [rgba; 4], true, self.sheet);
            n += 1;
        }
        n
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The trap camera on cell (c, r) (`coh_run` trap:c,r[,side]): target the
    /// cell centre, polar (180, 90 / 270 deg, 22.5 deg).
    pub(crate) fn trap(c: i32, r: i32, side: u8) -> ([f64; 3], [f64; 3]) {
        let t = [50.0 * f64::from(c - 3), 0.0, 50.0 * f64::from(r - 3)];
        let p = 22.5f64.to_radians();
        let z = if side == 0 { p.cos() } else { -p.cos() };
        ([t[0], t[1] + 180.0 * p.sin(), t[2] + 180.0 * z], t)
    }

    /// The overview (target 0, polar 500 / 90 deg / 33.75 deg).
    pub(crate) fn overview() -> ([f64; 3], [f64; 3]) {
        let p = 33.75f64.to_radians();
        ([0.0, 500.0 * p.sin(), 500.0 * p.cos()], [0.0; 3])
    }

    #[test]
    fn trig_rand_and_projection() {
        assert_eq!(tsin(0x4000), 1.0);
        assert_eq!(tcos(0), 1.0);
        assert_eq!(ang16(1.0, 0.0), 0x4000);
        assert_eq!(ang16(0.0, -1.0), 0x7FFF);
        // -70 deg through the table (NEWLY-ALLOWED 12.3).
        let th = DEG * -70.0;
        assert!((cosf(th) - 0.342_030_05).abs() < 1e-6 && (sinf(th) + 0.939_655_9).abs() < 1e-6);
        // newlib srand / rand.
        let mut r = Rng::new(0);
        r.srand(1);
        let a = r.next();
        assert_eq!(a, ((1u64.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1) >> 32) & 0x7FFF_FFFF) as u32);
        assert_eq!(rmod(17, -10), 7);
        // Screen-anchored centre.
        assert_eq!(view_proj([0.0, 0.0], 1.0, [0.0, 0.0, 0.0]), Some([256.0, 256.0]));
        assert_eq!((decay(5, 8), rise_u8(124, 128, 8), rise_u8(120, 128, 8)), (0, 128, 128));
    }
}
