//! CMODULE c022 (0x16), Magical Neutralizing Force Field (event 0x1C,
//! handler 0x25F2F0), as specified in `claude-activate/C022-FORCE-FIELD.md`
//! [F code; emu: the reference model `tools/mf_c022_model.py` equals the
//! interpreter's primitives to 0.0625 px under two cameras]. World-anchored at
//! the world origin (the board centre), all additive (ALPHA 0x48):
//! - DISC (pool slot 0, u 1-66): a flat 12-sector fan at y 10, radius 200,
//!   turning +5 deg/U, untextured Gouraud, centre (R/2,G/2,B/2,A1) to rim
//!   (.., A2);
//! - CURTAINS (u 1-60): 4 rings spawned at u 1, 6, 13, 20, each 24 + 24
//!   sheared vertical quads of TEXEFF 15 (143,0)-(174,64) (the second
//!   sub-ring drawn twice), growing outward 8/U, turning +-5 / +-3 deg/U.
//!
//! Returns 0, then 8 at u 68 (never 2). SE bank 1 0x23 at u 1. 49 random
//! values at u 1, 6, 13 and 20 (196 in all), none at init.
//!
//! [H / not reproduced]: the port's random values (its own LCG, same call
//! order and count: per ring 24 x q, then the base angle, then 24 x q); no OT
//! keys / real Z (the original depth-tests with key = mean w; the port draws
//! after the scene in pool-slot order); host sinf vs newlib sinf (<= 1 ulp).
use crate::duel_c602::projector;
use crate::duel_c621::FxTri;

/// Module life (D+0x39F0 = 0x44).
pub const LIFE: i32 = 68;
/// The update returning 8 (done).
pub const END_U: u32 = 68;
/// Return code of the last update.
pub const RET_DONE: u32 = 8;
/// SE (SECTL bank 1, record 0x16) played at `SE_U`.
pub const SE_BANK: u8 = 1;
pub const SE_ID: u16 = 0x23;
pub const SE_U: u32 = 1;
/// Pool slots (D+0xF0, 0x40 bytes each).
pub const SLOTS: usize = 193;
/// Ring spawn updates (spawner 0x1E00080).
pub const RING_U: [u32; 4] = [1, 6, 13, 20];
/// TEXEFF sheet size (`claude-units/texeff/015.png`).
const SHEET: f32 = 256.0;

// ---------------------------------------------------------------- shared helpers

/// The ports' random source (the original: newlib rand), counting calls.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Rng {
    seed: u64,
    pub(crate) calls: u32,
}

impl Rng {
    pub(crate) fn new(seed: u32) -> Self {
        Self { seed: u64::from(seed) | 1, calls: 0 }
    }

    /// 0..0x7FFFFFFF.
    pub(crate) fn next(&mut self) -> i32 {
        self.calls += 1;
        self.seed = self.seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
        ((self.seed >> 32) & 0x7FFF_FFFF) as i32
    }
}

pub(crate) fn s16(x: i32) -> i32 {
    ((x + 0x8000) & 0xFFFF) - 0x8000
}

/// newlib sinf (0x1A7348) [H: host sin, rounded to f32].
pub(crate) fn sinf(x: f32) -> f32 {
    f64::from(x).sin() as f32
}

/// sinf(pi_f32 * d / 180) with f32 products (the module's own formula).
pub(crate) fn sin_deg(d: i32) -> f32 {
    sinf((std::f32::consts::PI * d as f32) / 180.0)
}

/// The degree table built at init: 91 x sinf(pi_f32 i / 180).
fn table() -> &'static [f32; 91] {
    use std::sync::OnceLock;
    static T: OnceLock<[f32; 91]> = OnceLock::new();
    T.get_or_init(|| std::array::from_fn(|i| sin_deg(i as i32)))
}

fn fold(d: i32) -> i32 {
    let mut d = s16(d);
    if !(-359..360).contains(&d) {
        d %= 360;
    }
    if d < 0 {
        d += 360;
    }
    d
}

/// 0x108910(table, d, 0).
pub(crate) fn dsin(d: i32) -> f32 {
    let t = table();
    let d = fold(d) as usize;
    match d {
        0..=89 => t[d],
        90..=179 => t[180 - d],
        180..=269 => -t[d - 180],
        _ => -t[360 - d],
    }
}

/// 0x108910(table, d, 1).
pub(crate) fn dcos(d: i32) -> f32 {
    let t = table();
    let d = fold(d) as usize;
    match d {
        0..=89 => t[90 - d],
        90..=179 => -t[d - 90],
        180..=269 => -t[270 - d],
        _ => t[d - 270],
    }
}

/// A 4-vertex strip (v0 v1 v2 / v1 v2 v3) as two triangles (v0 v1 v2), (v1 v3 v2).
pub(crate) fn quad(out: &mut Vec<FxTri>, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], c: [[u8; 4]; 4], textured: bool, sheet: u8) {
    out.push(FxTri { screen: [p[0], p[1], p[2]], uv: [uv[0], uv[1], uv[2]], colour: [c[0], c[1], c[2]], textured, sheet });
    out.push(FxTri { screen: [p[1], p[3], p[2]], uv: [uv[1], uv[3], uv[2]], colour: [c[1], c[3], c[2]], textured, sheet });
}

/// TEXEFF 15 pixel UVs normalised by the 256 x 256 sheet.
pub(crate) fn uv15(p: [[i32; 2]; 4]) -> [[f32; 2]; 4] {
    p.map(|[u, v]| [u as f32 / SHEET, v as f32 / SHEET])
}

/// Projects four world points; None when any fails the 0x204F70 cull.
pub(crate) fn project4(project: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, w: [[f32; 3]; 4]) -> Option<[[f32; 2]; 4]> {
    let a = project(w[0].map(f64::from))?;
    let b = project(w[1].map(f64::from))?;
    let c = project(w[2].map(f64::from))?;
    let d = project(w[3].map(f64::from))?;
    Some([a, b, c, d])
}

// ---------------------------------------------------------------- module

/// 0x107FD0 colour of the param (spawner at K = 1).
pub fn param_colour(param: i32) -> [u8; 3] {
    match param {
        1 => [128, 0, 0],
        2 => [0, 128, 0],
        3 => [0, 0, 128],
        4 => [128, 128, 0],
        5 => [128, 0, 128],
        6 => [0, 128, 128],
        7 => [128, 64, 128],
        8 => [128, 0, 255],
        9 => [128, 64, 0],
        _ => [128, 128, 128],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Disc,
    /// Ring 1 (spawner 0x1E00F10): violet foot with param 0.
    RingA,
    /// Rings 2-4 (0x1E01580).
    RingB,
}

/// A 0x40-byte pool slot.
#[derive(Clone, Copy, Debug, PartialEq)]
struct P {
    active: bool,
    layer: Layer,
    /// 1 or 2 (curtains).
    sub: u8,
    r: f32,
    r2: f32,
    h: f32,
    /// Top angle +0x34, foot angle +0x36, bob angle +0x3A (s16 degrees).
    a: i32,
    b: i32,
    q: i32,
    delay: i32,
    /// 0 (sub-ring 1), 1 (sub-ring 2), 100 (folding).
    mode: i32,
    spd: f32,
    c1: [u8; 3],
    c2: [u8; 3],
    a1: i32,
    a2: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct C022 {
    rng: Rng,
    param: i32,
    rgb: [u8; 3],
    /// K (D+0x3A48).
    k: i32,
    life: i32,
    pool: Vec<Option<P>>,
    done: bool,
    /// Primitives of the last update per layer: disc, A1, A2, B1, B2.
    counts: [usize; 5],
    /// Additive triangles of the last update (disc and curtains, slot order).
    pub additive: Vec<FxTri>,
}

impl C022 {
    /// Init (`entry(D, param)`): `param` = the 0x107FD0 colour index (the
    /// handler passes 0); `seed` for the port's LCG. Draws nothing.
    pub fn new(param: i32, seed: u32) -> Self {
        Self {
            rng: Rng::new(seed),
            param,
            rgb: [128; 3],
            k: 0,
            life: LIFE,
            pool: vec![None; SLOTS],
            done: false,
            counts: [0; 5],
            additive: Vec::new(),
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// Random values drawn so far (49 at each ring spawn, 196 in all).
    pub fn rand_calls(&self) -> u32 {
        self.rng.calls
    }

    /// Primitives (quads / disc sectors) sent in the last update, per layer:
    /// [disc, ring A sub 1, ring A sub 2, rings B sub 1, rings B sub 2].
    pub fn counts(&self) -> [usize; 5] {
        self.counts
    }

    /// Primitives sent in the last update (the doc's "quads per update").
    pub fn prims(&self) -> usize {
        self.counts.iter().sum()
    }

    /// The disc's (A1, A2) after the last update, if alive.
    pub fn disc_alpha(&self) -> Option<(i32, i32)> {
        self.pool.iter().flatten().find(|p| p.active && p.layer == Layer::Disc).map(|p| (p.a1, p.a2))
    }

    /// Live curtains (layer, sub-ring) with their (top, foot) colours.
    pub fn curtain_colours(&self) -> Vec<(Layer, u8, [u8; 3], [u8; 3])> {
        self.pool.iter().flatten().filter(|p| p.active && p.layer != Layer::Disc).map(|p| (p.layer, p.sub, p.c1, p.c2)).collect()
    }

    /// Live curtain radii (r, r2) in slot order, per sub-ring.
    pub fn curtain_radii(&self) -> Vec<(Layer, u8, f32, f32)> {
        self.pool.iter().flatten().filter(|p| p.active && p.layer != Layer::Disc).map(|p| (p.layer, p.sub, p.r, p.r2)).collect()
    }

    /// add = lowest inactive slot (zeroed).
    fn add(&mut self, p: P) {
        if let Some(s) = self.pool.iter_mut().find(|s| !s.is_some_and(|p| p.active)) {
            *s = Some(p);
        }
    }

    /// 0x1E00080.
    fn spawner(&mut self) {
        self.k += 1;
        let k = self.k;
        if k == 1 {
            self.rgb = param_colour(self.param);
            let h = self.rgb.map(|c| c >> 1);
            self.add(P { active: true, layer: Layer::Disc, sub: 0, r: 0.0, r2: 0.0, h: 0.0, a: 0, b: 0, q: 0, delay: 0, mode: 0, spd: 0.0, c1: h, c2: h, a1: 0, a2: 0 });
        }
        if !RING_U.contains(&(k as u32)) {
            return;
        }
        let [r, g, b] = self.rgb;
        let (layer, c1, c2) = if k == 1 { (Layer::RingA, [r >> 1, g >> 1, b >> 1], [r, g >> 1, b]) } else { (Layer::RingB, self.rgb, self.rgb) };
        let (s1, s2) = if k == 1 { (5.0, 3.0) } else { (-5.0, -3.0) };
        let curtain = |sub: u8, r: f32, a: i32, q: i32, spd: f32, mode: i32, delay: i32| P {
            active: true,
            layer,
            sub,
            r,
            r2: r,
            h: 70.0 + 25.0 * dsin(q),
            a,
            b: a,
            q,
            delay,
            mode,
            spd,
            c1,
            c2,
            a1: 0,
            a2: 0,
        };
        for i in 0..24 {
            let q = self.rng.next() % 360;
            self.add(curtain(1, 30.0, s16(15 * i), q, s1, 0, 0));
        }
        let base = self.rng.next() % 360;
        for i in 0..24 {
            let q = self.rng.next() % 360;
            self.add(curtain(2, 15.0, s16(base + 15 * i), q, s2, 1, i));
        }
    }

    /// 0x1E00750.
    fn curtain_update(p: &mut P, k: i32) {
        match p.mode {
            0 => {
                if p.r < 220.0 {
                    p.r += 8.0;
                }
                if p.r - p.r2 > 20.0 {
                    p.r2 += 8.0;
                }
            }
            100 => {
                if p.a1 >= 11 {
                    p.a1 -= 9;
                }
                if p.a2 >= 11 {
                    p.a2 -= 9;
                } else {
                    p.active = false;
                }
            }
            _ => {
                let grow = p.r <= 80.0 || p.delay != 0;
                if p.r > 80.0 && p.delay != 0 {
                    p.delay -= 1;
                }
                if grow {
                    if p.r < 220.0 {
                        p.r += 8.0;
                    }
                    if p.r - p.r2 > 20.0 {
                        p.r2 += 5.0;
                    }
                } else {
                    p.r2 += 8.0;
                    if p.r < p.r2 {
                        p.mode = 100;
                    }
                }
            }
        }
        p.a = s16((p.a as f32 + p.spd) as i32);
        p.b = s16((p.b as f32 + p.spd) as i32);
        p.q = s16(p.q + 5);
        p.h = 70.0 + 25.0 * sin_deg(p.q);
        p.q = s16(p.q + 15);
        if p.a >= 360 {
            p.a %= 360;
        }
        if p.b >= 360 {
            p.b %= 360;
        }
        if p.q >= 360 {
            p.a %= 360; // sic: writes +0x34; +0x3A is never wrapped
        }
        if k < 40 {
            if p.a1 < 64 {
                p.a1 += 15;
            }
            if p.a2 < 64 {
                p.a2 += 15;
            }
        }
        if p.r > 200.0 {
            if p.a1 >= 11 {
                p.a1 -= 9;
            }
            if p.a2 >= 11 {
                p.a2 -= 9;
            } else {
                p.active = false;
            }
        }
    }

    /// Inline disc update (= 0x1E00BA0).
    fn disc_update(p: &mut P, k: i32, life: i32) {
        p.a = s16(p.a + 5);
        if !(-359..360).contains(&p.a) {
            p.a %= 360;
        }
        if k < 8 {
            if p.a1 < 192 {
                p.a1 += 32;
            }
            if p.a2 < 17 {
                p.a2 += 16;
            }
        }
        if k >= 36 {
            if p.a1 >= 17 {
                p.a1 -= 15;
            }
            if p.a2 >= 17 {
                p.a2 -= 15;
            }
        }
        if life < 3 {
            p.active = false;
        }
    }

    /// Module update `u` (1, 2, ..; K = u) with the camera (`eye`,
    /// `target`). Returns 0, or 8 at u 68 (and every later call).
    pub fn update(&mut self, _u: u32, eye: [f64; 3], target: [f64; 3]) -> u32 {
        self.additive.clear();
        self.counts = [0; 5];
        if self.done {
            return RET_DONE;
        }
        self.spawner();
        let (k, life) = (self.k, self.life);
        for p in self.pool.iter_mut().flatten().filter(|p| p.active) {
            if p.layer == Layer::Disc {
                Self::disc_update(p, k, life);
            } else {
                Self::curtain_update(p, k);
            }
        }
        if let Some(project) = projector(eye, target) {
            let project: &dyn Fn([f64; 3]) -> Option<[f32; 2]> = &project;
            for p in self.pool.iter().flatten().filter(|p| p.active) {
                if p.layer == Layer::Disc {
                    Self::disc_draw(p, project, &mut self.additive, &mut self.counts);
                } else {
                    Self::curtain_draw(p, project, &mut self.additive, &mut self.counts);
                }
            }
        }
        self.life -= 1;
        if self.life <= 0 {
            self.done = true;
            RET_DONE
        } else {
            0
        }
    }

    /// 0x1E01850: 12 sectors (v0 = v1 = the centre: one triangle each).
    fn disc_draw(p: &P, project: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, out: &mut Vec<FxTri>, counts: &mut [usize; 5]) {
        let rgba = |c: [u8; 3], a: i32| [c[0], c[1], c[2], a as u8];
        let (ci, co) = (rgba(p.c1, p.a1), rgba(p.c2, p.a2));
        let rim = |s: i32| [200.0 * dcos(s), 10.0, 200.0 * dsin(s)];
        for k in 0..12 {
            let s = p.a + 30 * k;
            let ctr = [0.0, 10.0, 0.0];
            let Some(v) = project4(project, [ctr, ctr, rim(s), rim(s + 30)]) else {
                continue;
            };
            counts[0] += 1;
            // Strip v0 v1 v2 / v1 v2 v3: the first triangle is degenerate.
            out.push(FxTri { screen: [v[1], v[3], v[2]], uv: [[0.0; 2]; 3], colour: [ci, co, co], textured: false, sheet: 0 });
        }
    }

    /// 0x1E010E0: 1 quad (sub-ring 1) or 2 (sub-ring 2, the foot 2 units
    /// further in).
    fn curtain_draw(p: &P, project: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, out: &mut Vec<FxTri>, counts: &mut [usize; 5]) {
        let n = if p.mode == 0 { 1 } else { 2 };
        let top = [p.c1[0], p.c1[1], p.c1[2], p.a1 as u8];
        let foot = [p.c2[0], p.c2[1], p.c2[2], p.a2 as u8];
        let uv = uv15([[143, 0], [174, 0], [143, 64], [174, 64]]);
        let layer = match (p.layer, p.sub) {
            (Layer::RingA, 1) => 1,
            (Layer::RingA, _) => 2,
            (_, 1) => 3,
            _ => 4,
        };
        for k in 0..n {
            let rb = p.r2 - 2.0 * k as f32;
            let w = [
                [p.r * dcos(p.a - 8), p.h, p.r * dsin(p.a - 8)],
                [p.r * dcos(p.a - 23), p.h, p.r * dsin(p.a - 23)],
                [rb * dcos(p.b), 0.0, rb * dsin(p.b)],
                [rb * dcos(p.b - 15), 0.0, rb * dsin(p.b - 15)],
            ];
            let Some(v) = project4(project, w) else {
                continue;
            };
            counts[layer] += 1;
            quad(out, v, uv, [top, top, foot, foot], true, 15);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The overview camera (0x262060: target 0, distance 500, yaw 90 deg,
    /// pitch 33.75 deg).
    pub(crate) fn overview() -> ([f64; 3], [f64; 3]) {
        let p = 33.75f64.to_radians();
        ([0.0, 500.0 * p.sin(), 500.0 * p.cos()], [0.0; 3])
    }

    /// The trap camera on cell (c, r), side 0.
    pub(crate) fn trap(c: i32, r: i32) -> ([f64; 3], [f64; 3]) {
        let t = [50.0 * f64::from(c - 3), 0.0, 50.0 * f64::from(r - 3)];
        let p = 22.5f64.to_radians();
        (std::array::from_fn(|k| t[k] + 180.0 * [0.0, p.sin(), p.cos()][k]), t)
    }

    fn run(param: i32, cam: ([f64; 3], [f64; 3]), seed: u32) -> (C022, Vec<(u32, usize, [usize; 5], u32, u32)>) {
        let mut c = C022::new(param, seed);
        let mut v = Vec::new();
        for u in 1..=80 {
            let n0 = c.rand_calls();
            let r = c.update(u, cam.0, cam.1);
            v.push((r, c.prims(), c.counts(), c.rand_calls() - n0, c.additive.len() as u32));
            if r == 8 {
                break;
            }
        }
        (c, v)
    }

    #[test]
    fn trig() {
        assert_eq!(dsin(90), 1.0);
        assert_eq!(dcos(0), 1.0);
        assert_eq!(dsin(0), 0.0);
        assert_eq!(dsin(-90), -1.0);
        assert_eq!(dcos(360), 1.0);
        assert_eq!(dsin(450), 1.0);
        assert!((dsin(30) - 0.5).abs() < 1e-7);
        assert_eq!(dcos(180), -1.0);
    }

    #[test]
    fn counts_returns_and_rand_follow_the_doc() {
        for (cam, seed) in [(overview(), 1), (trap(3, 5), 7)] {
            let (c, v) = run(0, cam, seed);
            assert_eq!(v.len(), 68);
            assert!(c.done());
            let rets: Vec<_> = v.iter().enumerate().filter(|(_, x)| x.0 != 0).map(|(i, x)| (i + 1, x.0)).collect();
            assert_eq!(rets, vec![(68, 8)]);
            let q = |u: usize| v[u - 1].1;
            for u in 1..=5 {
                assert_eq!(q(u), 84);
            }
            for u in 6..=12 {
                assert_eq!(q(u), 156);
            }
            for u in 13..=19 {
                assert_eq!(q(u), 228);
            }
            for u in 20..=41 {
                assert_eq!(q(u), 300, "u {u}");
            }
            assert_eq!((42..=50).map(q).collect::<Vec<_>>(), vec![296, 284, 278, 272, 216, 122, 84, 56, 52]);
            assert_eq!(q(51), 34);
            assert_eq!(q(60), 14);
            for u in 61..=66 {
                assert_eq!(q(u), 12);
            }
            assert_eq!((q(67), q(68)), (0, 0));
            // 11,662 primitives: disc 792, A1 1,080, A2 2,146, B1 2,472, B2 5,172.
            assert_eq!(v.iter().map(|x| x.1).sum::<usize>(), 11_662);
            let per: Vec<usize> = (0..5).map(|l| v.iter().map(|x| x.2[l]).sum()).collect();
            assert_eq!(per, vec![792, 1_080, 2_146, 2_472, 5_172]);
            // Triangles: one per disc sector, two per curtain quad.
            assert_eq!(v[0].4 as usize, 12 + 2 * 72);
            // 49 random values at u 1, 6, 13, 20; 196 in all.
            for (i, x) in v.iter().enumerate() {
                let u = i as u32 + 1;
                assert_eq!(x.3, if RING_U.contains(&u) { 49 } else { 0 }, "u {u}");
            }
            assert_eq!(c.rand_calls(), 196);
            // Later calls: 8, nothing drawn.
            let mut c = c;
            assert_eq!(c.update(69, cam.0, cam.1), 8);
            assert!(c.additive.is_empty());
        }
    }

    #[test]
    fn disc_alphas() {
        let (eye, target) = overview();
        let mut c = C022::new(0, 3);
        for u in 1..=68u32 {
            c.update(u, eye, target);
            let a = c.disc_alpha();
            let want_a1 = match u {
                1..=6 => 32 * u as i32,
                7..=35 => 192,
                36..=47 => 192 - 15 * (u as i32 - 35),
                _ => 12,
            };
            let want_a2 = match u {
                1 => 16,
                2..=35 => 32,
                36 => 17,
                _ => 2,
            };
            if u <= 66 {
                assert_eq!(a, Some((want_a1, want_a2)), "u {u}");
                let disc: Vec<_> = c.additive.iter().filter(|t| !t.textured).collect();
                assert_eq!(disc.len(), 12);
                assert!(disc.iter().all(|t| t.colour[0] == [64, 64, 64, want_a1 as u8] && t.colour[1][3] == want_a2 as u8));
            } else {
                assert_eq!(a, None);
            }
        }
    }

    #[test]
    fn ring_spawns_colours_and_growth() {
        let (eye, target) = overview();
        let mut c = C022::new(0, 5);
        c.update(1, eye, target);
        let col = c.curtain_colours();
        assert_eq!(col.len(), 48);
        assert!(col.iter().all(|x| x.0 == Layer::RingA && x.2 == [64, 64, 64] && x.3 == [128, 64, 128]));
        // Sub-ring 1 starts at r 30 (+8 in the spawn update), sub-ring 2 at 15.
        let r = c.curtain_radii();
        assert!(r[..24].iter().all(|x| x.1 == 1 && x.2 == 38.0 && x.3 == 30.0));
        assert!(r[24..].iter().all(|x| x.1 == 2 && x.2 == 23.0 && x.3 == 15.0));
        // Alpha 15 after the first update, uv cell (143,0)-(174,64) / 256.
        let t = c.additive.iter().find(|t| t.textured).unwrap();
        assert_eq!(t.colour[0][3], 15);
        assert_eq!(t.uv[0], [143.0 / 256.0, 0.0]);
        assert_eq!(t.sheet, 15);
        for u in 2..=6 {
            c.update(u, eye, target);
        }
        let col = c.curtain_colours();
        assert_eq!(col.len(), 96);
        assert!(col[48..].iter().all(|x| x.0 == Layer::RingB && x.2 == [128; 3] && x.3 == [128; 3]));
        // Sub-ring 1 of ring 1 after 6 updates: r 78, r2 = r - 16 from n 3.
        assert!(c.curtain_radii()[..24].iter().all(|x| x.2 == 78.0 && x.3 == 62.0));
        // Sub-ring 2 spiral: particle i stops at r 87 + 8 i.
        let mut c = C022::new(0, 5);
        let mut max_r = [0.0f32; 24];
        for u in 1..=40 {
            c.update(u, eye, target);
            for (i, x) in c.curtain_radii().iter().filter(|x| x.0 == Layer::RingA && x.1 == 2).enumerate().take(24) {
                if c.curtain_radii().iter().filter(|x| x.0 == Layer::RingA && x.1 == 2).count() == 24 {
                    max_r[i] = max_r[i].max(x.2);
                }
            }
        }
        for (i, r) in max_r.iter().enumerate() {
            assert_eq!(*r, (87 + 8 * i as i32).min(223) as f32, "i {i}");
        }
        // Param 7: (64,32,64) / (128,32,128) for ring 1, (128,64,128) for rings 2-4.
        let mut c = C022::new(7, 5);
        for u in 1..=6 {
            c.update(u, eye, target);
        }
        let col = c.curtain_colours();
        assert_eq!((col[0].2, col[0].3), ([64, 32, 64], [128, 32, 128]));
        assert_eq!((col[60].2, col[60].3), ([128, 64, 128], [128, 64, 128]));
        assert_eq!(c.disc_alpha().map(|_| ()), Some(()));
        assert!(c.additive.iter().filter(|t| !t.textured).all(|t| t.colour[0][..3] == [64, 32, 64]));
    }
}
