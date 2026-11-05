//! CMODULE C600's effect layers (the fusion), as traced in
//! the project notes [F/emu]: seven additive layers on
//! four billboard nodes at view (-101, 8) on the depth-800 plane, plus the
//! full-screen flash; base colour (160, 160, 128), everything additive and
//! never occluded. Closed-form projection: view = (-101 + s lx, 8 - s ly,
//! 800 - s lz), X = 256 + 800 vx / vz, Y = 256 + 1074.2857 vy / vz; a vertex
//! is rejected when vz <= 39.45 or >= 8188 or off the GS range, a quad when
//! any vertex is.
//!
//! [H / not reproduced]: random values come from the port's generator (only
//! the distributions can match); lines are drawn as 1-pixel quads. The
//! renderer draws the layers on the canvas over the HUD, as the original.
use crate::duel_c621::FxTri;

const BASE: [u8; 3] = [160, 160, 128];

/// 0x20BDA0 rotation (row vectors, 65536 per turn).
pub(crate) fn rotate(p: [f32; 3], r: [i32; 3]) -> [f32; 3] {
    let a = |v: i32| f64::from(v) * std::f64::consts::TAU / 65536.0;
    let (sa, ca) = a(r[0]).sin_cos();
    let (sb, cb) = a(r[1]).sin_cos();
    let (sc, cc) = a(r[2]).sin_cos();
    let m = [
        [cb * cc, cb * sc, -sb],
        [sa * sb * cc - ca * sc, sa * sb * sc + ca * cc, sa * cb],
        [ca * sb * cc + sa * sc, ca * sb * sc - sa * cc, ca * cb],
    ];
    let p = p.map(f64::from);
    std::array::from_fn(|c| (p[0] * m[0][c] + p[1] * m[1][c] + p[2] * m[2][c]) as f32)
}

pub(crate) fn project(s: f32, l: [f32; 3]) -> Option<[f32; 2]> {
    project_at([-101.0, 8.0], s, l)
}

/// [`project`] for a node at view (`anchor`) on the depth-800 plane.
pub(crate) fn project_at(anchor: [f32; 2], s: f32, l: [f32; 3]) -> Option<[f32; 2]> {
    let (vx, vy, vz) = (anchor[0] + s * l[0], anchor[1] - s * l[1], 800.0 - s * l[2]);
    if vz <= 39.45 || vz >= 8188.0 {
        return None;
    }
    let (x, y) = (256.0 + 800.0 * vx / vz, 256.0 + 1074.2857 * vy / vz);
    let ok = |v: f32| (-1792.0..=2303.0).contains(&v);
    (ok(x) && ok(y)).then_some([x, y])
}

fn approach(a: f32, max: f32, step: f32) -> f32 {
    if a <= max - step {
        a + step
    } else {
        max
    }
}

pub(crate) fn decay(a: u8, step: u8) -> u8 {
    a.saturating_sub(step)
}

#[derive(Clone, Debug, PartialEq)]
struct Lines {
    points: Vec<[f32; 3]>,
    alpha: u8,
    counter: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct C600 {
    seed: u64,
    s0: f32,
    s970: f32,
    s1f90: f32,
    glow: u8,
    ring: u8,
    rays_alpha: u8,
    feather_alpha: u8,
    flash: u8,
    rays: Vec<[i32; 3]>,
    sparks: Vec<[f32; 3]>,
    feather_p: Vec<[f32; 3]>,
    feather_a: Vec<[i32; 3]>,
    lines: [Lines; 2],
    /// The draws of the last update.
    pub tris: Vec<FxTri>,
}

impl C600 {
    pub fn new(seed: u32) -> Self {
        let mut c = Self {
            seed: u64::from(seed) | 1,
            s0: 0.1,
            s970: 0.1,
            s1f90: 1.0,
            glow: 96,
            ring: 32,
            rays_alpha: 64,
            feather_alpha: 128,
            flash: 255,
            rays: Vec::new(),
            sparks: Vec::new(),
            feather_p: Vec::new(),
            feather_a: Vec::new(),
            lines: [Lines { points: Vec::new(), alpha: 0, counter: 0 }, Lines { points: Vec::new(), alpha: 0, counter: 0 }],
            tris: Vec::new(),
        };
        for i in 0..2 {
            c.lines[i].points = (0..64).map(|_| c.line_point()).collect();
        }
        c.rays = (0..64).map(|_| std::array::from_fn(|_| (c.rand() & 0xFFFF) as i32)).collect();
        c.sparks = (0..60)
            .map(|_| [(c.rand() % 50) as f32 - 25.0, (c.rand() % 25) as f32, (c.rand() % 50) as f32 - 25.0])
            .collect();
        for _ in 0..60 {
            let a = f64::from(c.rand() & 0xFFFF) * std::f64::consts::TAU / 65536.0;
            let d = (c.rand() % 20) as f64;
            c.feather_p.push([(d * a.cos()) as f32, (d * a.sin()) as f32, d as f32]);
            let a: [i32; 3] = std::array::from_fn(|_| (c.rand() & 0xFFFF) as i32);
            c.feather_a.push(a);
        }
        c
    }

    /// newlib's rand.
    fn rand(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
        ((self.seed >> 32) & 0x7FFF_FFFF) as u32
    }

    fn line_point(&mut self) -> [f32; 3] {
        [
            (self.rand() % 480) as f32 - 240.0,
            (self.rand() % 480) as f32 - 240.0,
            (self.rand() % 240) as f32 + 240.0,
        ]
    }

    fn quad(&mut self, s: f32, p: [[f32; 3]; 4], uv: [[f32; 2]; 4], colour: [u8; 4], sheet: u8, size: [f32; 2]) {
        let mut sc = [[0.0; 2]; 4];
        for i in 0..4 {
            match project(s, p[i]) {
                Some(v) => sc[i] = v,
                None => return,
            }
        }
        let uv = uv.map(|[u, v]| [u / size[0], v / size[1]]);
        self.tris.push(FxTri { screen: [sc[0], sc[1], sc[2]], uv: [uv[0], uv[1], uv[2]], colour: [colour; 3], textured: true, sheet });
        self.tris.push(FxTri { screen: [sc[1], sc[3], sc[2]], uv: [uv[1], uv[3], uv[2]], colour: [colour; 3], textured: true, sheet });
    }

    fn line(&mut self, a: [f32; 2], b: [f32; 2], ca: [u8; 4], cb: [u8; 4]) {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt().max(1e-3);
        let (nx, ny) = (-dy / len * 0.5, dx / len * 0.5);
        let q = [[a[0] + nx, a[1] + ny], [a[0] - nx, a[1] - ny], [b[0] + nx, b[1] + ny], [b[0] - nx, b[1] - ny]];
        self.tris.push(FxTri { screen: [q[0], q[1], q[2]], uv: [[0.0; 2]; 3], colour: [ca, ca, cb], textured: false, sheet: 0 });
        self.tris.push(FxTri { screen: [q[1], q[3], q[2]], uv: [[0.0; 2]; 3], colour: [ca, cb, cb], textured: false, sheet: 0 });
    }

    /// Module update `u` (1..=103): draw, then step (0x1E05240's order).
    pub fn update(&mut self, u: u32) {
        self.tris.clear();
        let tint = |a: u8| [BASE[0], BASE[1], BASE[2], a];
        if u >= 81 {
            // The flash: +A on every channel (colour 128, additive).
            let f = self.flash;
            let full = [[0.0, 0.0], [0.0, 512.0], [512.0, 0.0], [512.0, 512.0]];
            let c = [128, 128, 128, f];
            self.tris.push(FxTri { screen: [full[0], full[1], full[2]], uv: [[0.0; 2]; 3], colour: [c; 3], textured: false, sheet: 0 });
            self.tris.push(FxTri { screen: [full[1], full[3], full[2]], uv: [[0.0; 2]; 3], colour: [c; 3], textured: false, sheet: 0 });
            self.flash = decay(self.flash, 16);
        }
        if u < 18 {
            return;
        }
        // FEATHERS.
        for i in 0..60 {
            self.feather_p[i] = rotate(self.feather_p[i], [0, 0, 0x1000]);
            let (p, a) = (self.feather_p[i], self.feather_a[i]);
            let corners = [[-2.5, 1.25, 0.0], [2.5, 1.25, 0.0], [-2.5, -1.25, 0.0], [2.5, -1.25, 0.0]].map(|c| {
                let r = rotate(c, a);
                [r[0] + p[0], r[1] + p[1], r[2] + p[2]]
            });
            let alpha = self.feather_alpha;
            self.quad(self.s1f90, corners, [[0.0, 0.0], [127.0, 0.0], [0.0, 63.0], [127.0, 63.0]], [128, 128, 128, alpha], 12, [128.0, 64.0]);
            self.feather_a[i] = self.feather_a[i].map(|v| v + 0x1000);
        }
        if self.s1f90 < 10.0 {
            self.s1f90 = (self.s1f90 + 0.5).min(10.0);
        } else {
            self.feather_alpha = decay(self.feather_alpha, 4);
        }
        // RING A / B.
        let s0 = self.s0;
        for outer in [115.0f32, 130.0] {
            for j in 0..30 {
                let ring = |r: f32, j: u32| {
                    let a = (12.0 * j as f32).to_radians();
                    [r * a.cos(), r * a.sin(), 0.0]
                };
                let ring_alpha = self.ring;
                self.quad(
                    s0,
                    [ring(120.0, j), ring(outer, j), ring(120.0, j + 1), ring(outer, j + 1)],
                    [[142.0, 64.0], [142.0, 0.0], [174.0, 64.0], [174.0, 0.0]],
                    tint(ring_alpha),
                    15,
                    [256.0, 256.0],
                );
            }
        }
        // GLOW.
        let p = |x: f32, y: f32| [100.0 * x, 100.0 * y, 0.0];
        let pts = [p(-1.0, 0.0), p(-1.0, 1.0), p(0.0, 1.0), p(1.0, 1.0), p(1.0, 0.0), p(1.0, -1.0), p(0.0, -1.0), p(-1.0, -1.0)];
        let c = [0.0, 0.0, 0.0];
        for q in [[pts[1], pts[2], pts[0], c], [pts[3], pts[2], pts[4], c], [pts[5], pts[6], pts[4], c], [pts[7], pts[6], pts[0], c]] {
            let glow = self.glow;
            self.quad(s0, q, [[176.0, 32.0], [208.0, 32.0], [176.0, 64.0], [208.0, 64.0]], tint(glow), 15, [256.0, 256.0]);
        }
        // RAYS.
        for i in 0..64 {
            let r = self.rays[i];
            let t = [[-15.0, 300.0, 0.0], [15.0, 300.0, 0.0], [-7.5, 0.0, 0.0], [7.5, 0.0, 0.0]].map(|v| rotate(v, r));
            let a = self.rays_alpha;
            self.quad(self.s970, t, [[142.0, 0.0], [175.0, 0.0], [142.0, 63.0], [175.0, 63.0]], tint(a), 15, [256.0, 256.0]);
        }
        if u <= 80 {
            self.s0 = approach(self.s0, 1.0, 0.02);
        } else {
            for i in 0..60 {
                self.sparks[i] = self.sparks[i].map(|v| v * 1.3);
                let [x, y, z] = self.sparks[i];
                self.quad(
                    self.s970,
                    [[x - 3.0, y + 3.0, z], [x + 3.0, y + 3.0, z], [x - 3.0, y - 3.0, z], [x + 3.0, y - 3.0, z]],
                    [[176.0, 64.0], [207.0, 64.0], [176.0, 95.0], [207.0, 95.0]],
                    tint(128),
                    15,
                    [256.0, 256.0],
                );
            }
            self.s970 = approach(self.s970, 1.0, 0.3);
            self.s0 += 0.005;
            self.rays_alpha = decay(self.rays_alpha, 4);
            self.ring = decay(self.ring, 2);
            self.glow = decay(self.glow, 3);
        }
        for r in &mut self.rays {
            *r = r.map(|v| v + 0x80);
        }
        // LINES A / B: drawn twice, moved, respawned; the pulse.
        for d in 0..2 {
            let a = self.lines[d].alpha;
            for i in 0..64 {
                let p = self.lines[d].points[i];
                let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-6);
                let n = p.map(|v| v / len);
                let q = [p[0] + 80.0 * n[0], p[1] + 80.0 * n[1], p[2] + 80.0 * n[2]];
                if let (Some(sa), Some(sb)) = (project(1.0, p), project(1.0, q)) {
                    for _ in 0..2 {
                        self.line(sa, sb, tint(a), [0, 0, 0, a]);
                    }
                }
                let mut p = [p[0] - 20.0 * n[0], p[1] - 20.0 * n[1], p[2] - 20.0 * n[2]];
                if p[2] >= 300.0 {
                    p = [
                        0.1 * ((self.rand() % 480) as f32 - 240.0),
                        0.1 * ((self.rand() % 480) as f32 - 240.0),
                        0.1 * (self.rand() % 240) as f32,
                    ];
                }
                self.lines[d].points[i] = p;
            }
            let l = &mut self.lines[d];
            if u <= 80 {
                l.alpha = if l.counter < 5 { (u32::from(l.alpha) + 64).min(255) as u8 } else { decay(l.alpha, 32) };
            }
            if l.counter >= 11 {
                l.counter = 0;
                l.alpha = 0;
                let points = (0..64).map(|_| self.line_point()).collect();
                self.lines[d].points = points;
            }
            self.lines[d].counter += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_and_alphas_follow_the_traced_curves() {
        let mut c = C600::new(3);
        for u in 1..=62 {
            c.update(u);
        }
        assert!((c.s0 - 1.0).abs() < 1e-3, "{}", c.s0);
        assert_eq!(c.s1f90, 10.0);
        for u in 63..=82 {
            c.update(u);
        }
        // After u 82's step: s970 0.7, glow 96 - 6, rays 64 - 8.
        assert!((c.s970 - 0.7).abs() < 1e-6);
        assert_eq!((c.glow, c.rays_alpha, c.ring), (90, 56, 28));
        assert_eq!(c.flash, 255 - 32);
    }

    #[test]
    fn lines_pulse_as_captured() {
        let mut c = C600::new(5);
        let mut drawn = Vec::new();
        for u in 1..=30 {
            drawn.push(c.lines[0].alpha);
            c.update(u);
        }
        // Drawn at u 18..29: 0, 64, 128, 192, 255, 255, 223, 191, 159, 127, 95, 63.
        assert_eq!(&drawn[17..29], &[0, 64, 128, 192, 255, 255, 223, 191, 159, 127, 95, 63]);
    }
}
