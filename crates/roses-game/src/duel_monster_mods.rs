//! The monster effects' modules, ported from the interpreter-matched models
//! in the project notes [F code; emu: every vertex of the original's
//! run, <= 0.13 px]:
//! - c529 (0x211, "Power increase", 30 U; `e2_c529_model.py`,
//!   EFFECT-MODULES-2.md 4): blue lines flowing into the target, a pulsing
//!   orb (u 1-23), then a crown ring and stars (u 22-30);
//! - c513 (0x201, Key Mace, 16 U; `mf_c513_model.py`): a green burst
//!   (sparks, rings, glow, rays) on a billboard at the target;
//! - c031 (0x1F, Man-Eater Bug's lock, 44 U; `mf_c031_model.py`): four red
//!   corners closing in, then a spinning cross, every quad sent 3 times.
//! All additive; nodes at the target's cell centre (y 0).
//!
//! [H / not reproduced]: the port's random values; no depth test (the
//! original keys most of these by depth).
use crate::duel_c600::rotate;
use crate::duel_c602::{billboard_axes, projector};
use crate::duel_c621::FxTri;

fn lcg(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
    ((*seed >> 32) & 0x7FFF_FFFF) as u32
}

fn decay(a: u8, s: u8) -> u8 {
    if a > s { a - s } else { 0 }
}

/// A quad in world space (strip order v0 v1 v2 v3) and its projection.
struct Out<'a, P: Fn([f64; 3]) -> Option<[f32; 2]>> {
    project: &'a P,
    tris: Vec<FxTri>,
}

impl<P: Fn([f64; 3]) -> Option<[f32; 2]>> Out<'_, P> {
    fn quad(&mut self, w: [[f64; 3]; 4], uv: [[f32; 2]; 4], c: [[u8; 4]; 4], sheet: u8, size: [f32; 2], copies: u32) {
        let (Some(p0), Some(p1), Some(p2), Some(p3)) = ((self.project)(w[0]), (self.project)(w[1]), (self.project)(w[2]), (self.project)(w[3])) else {
            return;
        };
        let uv = uv.map(|[u, v]| [u / size[0], v / size[1]]);
        let textured = sheet != 0;
        for _ in 0..copies {
            self.tris.push(FxTri { screen: [p0, p1, p2], uv: [uv[0], uv[1], uv[2]], colour: [c[0], c[1], c[2]], textured, sheet });
            self.tris.push(FxTri { screen: [p1, p3, p2], uv: [uv[1], uv[3], uv[2]], colour: [c[1], c[3], c[2]], textured, sheet });
        }
    }

    fn line(&mut self, a: [f64; 3], b: [f64; 3], ca: [u8; 4], cb: [u8; 4]) {
        let (Some(sa), Some(sb)) = ((self.project)(a), (self.project)(b)) else {
            return;
        };
        let (dx, dy) = (sb[0] - sa[0], sb[1] - sa[1]);
        let len = (dx * dx + dy * dy).sqrt().max(1e-3);
        let (nx, ny) = (-dy / len * 0.5, dx / len * 0.5);
        let q = [[sa[0] + nx, sa[1] + ny], [sa[0] - nx, sa[1] - ny], [sb[0] + nx, sb[1] + ny], [sb[0] - nx, sb[1] - ny]];
        self.tris.push(FxTri { screen: [q[0], q[1], q[2]], uv: [[0.0; 2]; 3], colour: [ca, ca, cb], textured: false, sheet: 0 });
        self.tris.push(FxTri { screen: [q[1], q[3], q[2]], uv: [[0.0; 2]; 3], colour: [ca, cb, cb], textured: false, sheet: 0 });
    }
}

/// A billboard point: t + s (l . [right, up, toward]).
fn bill(t: [f64; 3], axes: &[[f64; 3]; 3], l: [f32; 3], s: f32) -> [f64; 3] {
    std::array::from_fn(|i| t[i] + f64::from(s) * (f64::from(l[0]) * axes[0][i] + f64::from(l[1]) * axes[1][i] + f64::from(l[2]) * axes[2][i]))
}

fn world(t: [f64; 3], l: [f32; 3], s: f32) -> [f64; 3] {
    std::array::from_fn(|i| t[i] + f64::from(s) * f64::from(l[i]))
}

// ------------------------------------------------------------------ c529

#[derive(Clone, Debug, PartialEq)]
pub struct C529 {
    seed: u64,
    pub pos: [f64; 3],
    u: u32,
    points: Vec<[f32; 3]>,
    ring: Vec<([f32; 3], [f32; 3])>,
    lines: Vec<[f32; 3]>,
    a_lines: u8,
    a_orb: u8,
    s_orb: f32,
    a_ring: u8,
    s_ring: f32,
    a_points: u8,
    s_points: f32,
    /// What the last update drew: lines (positions, alpha), the orb (scale,
    /// alpha), the ring (scale, alpha), the points (positions, scale,
    /// alpha).
    d_lines: (Vec<[f32; 3]>, u8),
    d_orb: Option<(f32, u8)>,
    d_ring: Option<(f32, u8)>,
    d_points: Option<(Vec<[f32; 3]>, f32, u8)>,
}

impl C529 {
    pub const UPDATES: u32 = 30;

    pub fn new(pos: [f64; 3], seed: u32) -> Self {
        let mut seed = u64::from(seed) | 1;
        let points = (0..60)
            .map(|_| [(lcg(&mut seed) % 50) as f32 - 25.0, (lcg(&mut seed) % 25) as f32, (lcg(&mut seed) % 50) as f32 - 25.0])
            .collect();
        let a12 = 0.209_439_5f32;
        let mut ring = vec![([0.1, 2.0, 0.0], [30.0, 8.0, 0.0])];
        for k in 1..=30 {
            let a = a12 * k as f32;
            let y = 8 + (((lcg(&mut seed) % 12) as i32 - 6) >> 1);
            ring.push(([0.1 * a.cos(), 2.0, 0.1 * a.sin()], [30.0 * a.cos(), y as f32, 30.0 * a.sin()]));
        }
        let lines = (0..64).map(|_| Self::line(&mut seed)).collect();
        Self {
            seed,
            pos,
            u: 0,
            points,
            ring,
            lines,
            a_lines: 0,
            a_orb: 0,
            s_orb: 0.1,
            a_ring: 128,
            s_ring: 0.1,
            a_points: 128,
            s_points: 0.1,
            d_lines: (Vec::new(), 0),
            d_orb: None,
            d_ring: None,
            d_points: None,
        }
    }

    fn line(seed: &mut u64) -> [f32; 3] {
        let x = 0.1 * ((lcg(seed) % 480) as f32 - 240.0);
        let z = 0.1 * ((lcg(seed) % 480) as f32 - 240.0);
        let y = 0.1 * (lcg(seed) % 240) as f32;
        [x, y, z]
    }

    /// Update `u` (1..=30): what it draws, then the step.
    pub fn update(&mut self, u: u32) {
        self.u = u;
        let c = u - 1;
        self.d_lines = (self.lines.clone(), self.a_lines);
        for i in 0..self.lines.len() {
            let p = self.lines[i];
            let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-6);
            let mut p = [p[0] - 2.5 * p[0] / l, p[1] - 2.5 * p[1] / l, p[2] - 2.5 * p[2] / l];
            if p[1] <= 0.0 {
                p = Self::line(&mut self.seed);
            }
            self.lines[i] = p;
        }
        self.a_lines = if c < 16 { (u32::from(self.a_lines) + 32).min(160) as u8 } else { decay(self.a_lines, 32) };
        self.d_orb = None;
        if c < 23 {
            self.d_orb = Some((self.s_orb, self.a_orb));
            if self.s_orb < 1.0 {
                self.s_orb += 0.05;
                self.a_orb = (u32::from(self.a_orb) + 4).min(128) as u8;
            } else {
                self.s_orb = 1.0;
                self.a_orb = if c % 2 == 1 { 128 } else { 64 };
            }
        }
        self.d_ring = None;
        self.d_points = None;
        if c >= 21 {
            self.d_ring = Some((self.s_ring, self.a_ring));
            self.a_ring = decay(self.a_ring, 16);
            self.s_ring += if self.s_ring < 1.0 { 0.2 } else { 0.05 };
            for p in &mut self.points {
                *p = p.map(|v| v * 1.05);
            }
            self.d_points = Some((self.points.clone(), self.s_points, self.a_points));
            self.a_points = decay(self.a_points, 16);
            self.s_points += if self.s_points < 1.0 { 0.2 } else { 0.05 };
        }
    }

    pub fn draw(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<FxTri> {
        let Some(project) = projector(eye, target) else {
            return Vec::new();
        };
        let axes = billboard_axes(eye, target);
        let t = self.pos;
        let mut out = Out { project: &project, tris: Vec::new() };
        let (lines, a) = &self.d_lines;
        for p in lines {
            let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-6);
            let q = [p[0] + 20.0 * p[0] / l, p[1] + 20.0 * p[1] / l, p[2] + 20.0 * p[2] / l];
            out.line(world(t, *p, 1.0), world(t, q, 1.0), [128, 160, 255, *a], [0, 0, 0, *a]);
        }
        if let Some((s, a)) = self.d_orb {
            let p: [[f32; 3]; 6] = [[-15.0, 0.0, 0.0], [-15.0, 15.0, 0.0], [15.0, 15.0, 0.0], [15.0, 0.0, 0.0], [15.0, -4.5, 15.0], [-15.0, -4.5, 15.0]];
            let u0 = (178 + 32 * (((self.u - 1) / 2) % 2)) as f32;
            for (qq, uv) in [
                ([1usize, 2, 0, 3], [[u0, 96.0], [u0 + 31.0, 96.0], [u0, 111.0], [u0 + 31.0, 111.0]]),
                ([0, 3, 5, 4], [[u0, 111.0], [u0 + 31.0, 111.0], [u0, 127.0], [u0 + 31.0, 127.0]]),
            ] {
                out.quad(qq.map(|k| bill(t, &axes, p[k], s)), uv, [[96, 96, 128, a]; 4], 15, [256.0, 256.0], 1);
            }
        }
        if let Some((s, a)) = self.d_ring {
            for k in 0..30 {
                let (i0, o0) = self.ring[k];
                let (i1, o1) = self.ring[(k + 1) % 30];
                out.quad(
                    [world(t, i0, s), world(t, o0, s), world(t, i1, s), world(t, o1, s)],
                    [[142.0, 64.0], [142.0, 0.0], [174.0, 64.0], [174.0, 0.0]],
                    [[96, 128, 224, a]; 4],
                    15,
                    [256.0, 256.0],
                    1,
                );
            }
        }
        if let Some((points, s, a)) = &self.d_points {
            for &[x, y, z] in points {
                out.quad(
                    [[x - 1.5, y + 1.5, z], [x + 1.5, y + 1.5, z], [x - 1.5, y - 1.5, z], [x + 1.5, y - 1.5, z]].map(|l| bill(t, &axes, l, *s)),
                    [[176.0, 64.0], [207.0, 64.0], [176.0, 95.0], [207.0, 95.0]],
                    [[128, 160, 255, *a]; 4],
                    15,
                    [256.0, 256.0],
                    1,
                );
            }
        }
        out.tris
    }
}

// ------------------------------------------------------------------ c513

type C513Draw = (Vec<[f32; 3]>, f32, u8, Vec<[i32; 3]>, f32, [u8; 4]);

#[derive(Clone, Debug, PartialEq)]
pub struct C513 {
    pub pos: [f64; 3],
    sparks: Vec<[f32; 3]>,
    rays: Vec<[i32; 3]>,
    s_d: f32,
    s_s: f32,
    a_s: u8,
    a_ra: u8,
    a_rb: u8,
    a_g: u8,
    a_ray: u8,
    /// The last update's draw: sparks, spark scale and alpha, rays, node
    /// scale, alphas of rings A / B, glow and rays.
    drawn: Option<C513Draw>,
}

impl C513 {
    pub const UPDATES: u32 = 16;
    const RGB: [u8; 3] = [96, 208, 112];

    pub fn new(pos: [f64; 3], seed: u32) -> Self {
        let mut seed = u64::from(seed) | 1;
        let sparks = (0..60)
            .map(|_| [(lcg(&mut seed) % 50) as f32 - 25.0, (lcg(&mut seed) % 25) as f32, (lcg(&mut seed) % 50) as f32 - 25.0])
            .collect();
        let rays = (0..64).map(|_| std::array::from_fn(|_| (lcg(&mut seed) & 0xFFFF) as i32)).collect();
        Self { pos, sparks, rays, s_d: 0.1, s_s: 0.2, a_s: 128, a_ra: 32, a_rb: 48, a_g: 96, a_ray: 32, drawn: None }
    }

    pub fn update(&mut self) {
        for p in &mut self.sparks {
            *p = p.map(|v| v * 1.03);
        }
        self.drawn = Some((self.sparks.clone(), self.s_s, self.a_s, self.rays.clone(), self.s_d, [self.a_ra, self.a_rb, self.a_g, self.a_ray]));
        self.a_s = decay(self.a_s, 10);
        if self.s_s < 1.0 {
            self.s_s += 0.3;
        }
        if self.s_d <= 1.0 {
            self.s_d += 0.4;
        } else {
            self.s_d += 0.005;
            self.a_ray = decay(self.a_ray, 3);
            self.a_ra = decay(self.a_ra, 3);
            self.a_rb = decay(self.a_rb, 3);
            self.a_g = decay(self.a_g, 6);
        }
        for ray in &mut self.rays {
            *ray = ray.map(|v| v + 0x80);
        }
    }

    pub fn draw(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<FxTri> {
        let (Some((sparks, s_s, a_s, rays, s_d, [a_ra, a_rb, a_g, a_ray])), Some(project)) = (&self.drawn, projector(eye, target)) else {
            return Vec::new();
        };
        let axes = billboard_axes(eye, target);
        let t = self.pos;
        let [r, g, b] = Self::RGB;
        let mut out = Out { project: &project, tris: Vec::new() };
        for &[x, y, z] in sparks {
            out.quad(
                [[x - 1.5, y + 1.5, z], [x + 1.5, y + 1.5, z], [x - 1.5, y - 1.5, z], [x + 1.5, y - 1.5, z]].map(|l| bill(t, &axes, l, *s_s)),
                [[176.0, 64.0], [207.0, 64.0], [176.0, 95.0], [207.0, 95.0]],
                [[r, g, b, *a_s]; 4],
                15,
                [256.0, 256.0],
                1,
            );
        }
        let ring = |rad: f32, k: u32| {
            let a = 0.209_439_5f32 * k as f32;
            [rad * a.cos(), rad * a.sin(), 0.0]
        };
        for (ro, a) in [(28.0f32, *a_ra), (32.0, *a_rb)] {
            for k in 0..30 {
                out.quad(
                    [ring(30.0, k), ring(ro, k), ring(30.0, k + 1), ring(ro, k + 1)].map(|l| bill(t, &axes, l, *s_d)),
                    [[142.0, 64.0], [142.0, 0.0], [174.0, 64.0], [174.0, 0.0]],
                    [[r, g, b, a]; 4],
                    15,
                    [256.0, 256.0],
                    1,
                );
            }
        }
        let p = |x: f32, y: f32| [25.0 * x, 25.0 * y, 0.0];
        let pts = [p(-1.0, 0.0), p(-1.0, 1.0), p(0.0, 1.0), p(1.0, 1.0), p(1.0, 0.0), p(1.0, -1.0), p(0.0, -1.0), p(-1.0, -1.0)];
        let c = [0.0, 0.0, 0.0];
        for q in [[pts[1], pts[2], pts[0], c], [pts[3], pts[2], pts[4], c], [pts[5], pts[6], pts[4], c], [pts[7], pts[6], pts[0], c]] {
            out.quad(q.map(|l| bill(t, &axes, l, *s_d)), [[176.0, 32.0], [208.0, 32.0], [176.0, 64.0], [208.0, 64.0]], [[r, g, b, *a_g]; 4], 15, [256.0, 256.0], 1);
        }
        for ray in rays {
            let w = [[-4.0, 80.0, 0.0], [4.0, 80.0, 0.0], [-2.0, 0.0, 0.0], [2.0, 0.0, 0.0]].map(|v| bill(t, &axes, rotate(v, *ray), *s_d));
            out.quad(w, [[142.0, 0.0], [175.0, 0.0], [142.0, 63.0], [175.0, 63.0]], [[r, g, b, *a_ray]; 4], 15, [256.0, 256.0], 1);
        }
        out.tris
    }
}

// ------------------------------------------------------------------ c031

#[derive(Clone, Debug, PartialEq)]
struct Mover {
    p: [f32; 2],
    v: [f32; 2],
    settled: bool,
    alpha: u32,
    fresh: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Spin {
    n_a: i32,
    n_c: i32,
    a: [i32; 3],
    v: [i32; 3],
    alpha: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct C031 {
    seed: u64,
    pub pos: [f64; 3],
    movers: Vec<Mover>,
    spin: Option<Spin>,
    life: u32,
    /// The last update drew (life >= 2).
    drawing: bool,
}

/// 0x108910: integer degrees through a 91-entry sine table.
fn sind(a: i32) -> f32 {
    let a = a.rem_euclid(360);
    let t = |k: i32| (std::f64::consts::PI * f64::from(k) / 180.0).sin() as f32;
    match a {
        0..=90 => t(a),
        91..=180 => t(180 - a),
        181..=270 => -t(a - 180),
        _ => -t(360 - a),
    }
}

impl C031 {
    pub const UPDATES: u32 = 44;
    const SIGNS: [(f32, f32); 4] = [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];

    pub fn new(pos: [f64; 3], seed: u32) -> Self {
        let movers = Self::SIGNS
            .iter()
            .map(|&(sx, sy)| Mover { p: [100.0 * sx, 100.0 * sy], v: [-10.0 * sx, -10.0 * sy], settled: false, alpha: 0, fresh: false })
            .collect();
        Self { seed: u64::from(seed) | 1, pos, movers, spin: None, life: Self::UPDATES, drawing: false }
    }

    pub fn update(&mut self) {
        self.drawing = false;
        if self.life < 2 {
            return;
        }
        for i in 0..4 {
            let m = &mut self.movers[i];
            let moving = if i == 0 || i == 2 { m.p[0] < 0.0 } else { m.p[0] > 0.0 };
            if !m.settled && moving {
                m.p = [m.p[0] + m.v[0], m.p[1] + m.v[1]];
            } else if !m.settled {
                m.settled = true;
                m.fresh = true;
            }
            if !m.settled {
                if m.alpha < 0xF0 {
                    m.alpha += 13;
                }
            } else if std::mem::take(&mut m.fresh) && i == 1 && self.spin.is_none() {
                let r: Vec<u32> = (0..7).map(|_| lcg(&mut self.seed)).collect();
                self.spin = Some(Spin {
                    n_a: (r[0] % 3 + 3) as i32,
                    n_c: (r[1] % 3 + 3) as i32,
                    a: [0, (r[2] % 180) as i32, (r[3] % 360) as i32],
                    v: [r[4], r[5], r[6]].map(|x| if x & 1 == 1 { -30 } else { 30 }),
                    alpha: 0,
                });
            }
            let m = &mut self.movers[i];
            if self.life < 6 && m.alpha >= 0x31 {
                m.alpha -= 0x30;
            }
        }
        if let Some(s) = &mut self.spin {
            for k in 0..3 {
                let a = s.a[k];
                s.a[k] = if (-359..360).contains(&a) { a + s.v[k] } else { 360 };
            }
            s.alpha = if self.life >= 6 { 0xFF } else if s.alpha >= 0x31 { s.alpha - 0x30 } else { s.alpha };
        }
        self.drawing = true;
        self.life -= 1;
    }

    pub fn draw(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<FxTri> {
        let (true, Some(project)) = (self.drawing, projector(eye, target)) else {
            return Vec::new();
        };
        let axes = billboard_axes(eye, target);
        let t = self.pos;
        let mut out = Out { project: &project, tris: Vec::new() };
        for (i, m) in self.movers.iter().enumerate() {
            let (sx, sy) = Self::SIGNS[i];
            let (x, y) = (m.p[0], m.p[1]);
            let loc = [[x + 20.0 * sx, y + 20.0 * sy + 10.0, 30.0], [x, y + 20.0 * sy + 10.0, 30.0], [x + 20.0 * sx, y + 10.0, 30.0], [x, y + 10.0, 30.0]];
            let a = m.alpha.min(255) as u8;
            out.quad(loc.map(|l| bill(t, &axes, l, 1.0)), [[64.0, 1.0], [127.0, 1.0], [64.0, 64.0], [127.0, 64.0]], [[255, 0, 0, a]; 4], 9, [128.0, 64.0], 3);
        }
        if let Some(s) = &self.spin {
            let a = s.a[2];
            let (ca, sa) = (sind(a + 90), sind(a));
            let ox = sind(s.a[0]) * s.n_a as f32;
            let oy = 10.0 + sind(s.a[1]) * s.n_c as f32;
            let sg = -1.0f32;
            for (sx, sy) in Self::SIGNS {
                let base = [[20.0 * sx, 20.0 * sy], [0.0, 20.0 * sy], [20.0 * sx, 0.0], [0.0, 0.0]];
                let loc = base.map(|[bx, by]| [bx * ca - sg * by * sa + ox, sg * bx * sa + by * ca + oy, 30.0]);
                let alpha = s.alpha.min(255) as u8;
                out.quad(loc.map(|l| bill(t, &axes, l, 1.0)), [[0.0, 1.0], [63.0, 1.0], [0.0, 64.0], [63.0, 64.0]], [[255, 0, 0, alpha]; 4], 9, [128.0, 64.0], 3);
            }
        }
        out.tris
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam() -> ([f64; 3], [f64; 3]) {
        let t = [-50.0, 0.0, -100.0];
        let p = 22.5f64.to_radians();
        (std::array::from_fn(|k| t[k] + 180.0 * [0.0, p.sin(), p.cos()][k]), t)
    }

    #[test]
    fn c031_corners_close_in_and_the_cross_spawns() {
        let (eye, target) = cam();
        let mut c = C031::new(target, 3);
        for u in 1..=10 {
            c.update();
            assert_eq!(c.movers[0].alpha, 13 * u, "u {u}");
        }
        assert!(c.spin.is_none());
        c.update();
        assert!(c.spin.is_some());
        // 4 corners + 4 cross quads, each 3 times, 2 triangles.
        assert_eq!(c.draw(eye, target).len(), (4 + 4) * 3 * 2);
        for _ in 12..=44 {
            c.update();
        }
        assert!(c.draw(eye, target).is_empty());
    }

    #[test]
    fn c529_and_c513_draw_on_the_cell_camera() {
        let (eye, target) = cam();
        let mut a = C529::new(target, 5);
        for u in 1..=30 {
            a.update(u);
        }
        assert!(!a.draw(eye, target).is_empty());
        let mut b = C513::new(target, 5);
        b.update();
        assert!(b.draw(eye, target).len() > 100);
    }
}
