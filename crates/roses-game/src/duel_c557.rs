//! CMODULE c557 (0x22D) "Power decrease", as traced in
//! the project notes 5 [F code; emu: counts and
//! alphas equal to the interpreter, sparks and smoke to 0.063 px, the
//! speed lines within 0.38 px on screen]: 64 red speed lines out of the
//! victim (world axes, each sent twice), a subtractive dark purple puff
//! emitter and an additive red one, and three rising spark clouds from
//! u 13 / 16 / 19. 50 updates.
//!
//! [H / not reproduced]: the port's random values; lines drawn as 1-pixel
//! quads; no depth test.
use crate::duel_c602::{billboard_axes, projector};
use crate::duel_c621::FxTri;

pub const UPDATES: u32 = 50;

fn lcg(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
    ((*seed >> 32) & 0x7FFF_FFFF) as u32
}

#[derive(Clone, Debug, PartialEq)]
struct Emitter {
    points: Vec<[f32; 3]>,
    alpha: u8,
    step: u8,
    scale: f32,
    half: f32,
    rgb: [u8; 3],
    drawn_alpha: u8,
}

#[derive(Clone, Debug, PartialEq)]
struct Sparks {
    start: u32,
    points: Vec<[f32; 3]>,
    alpha: u8,
    sigma: f32,
    theta: i32,
    drawn: Option<(Vec<[f32; 3]>, u8, f32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct C557 {
    pub pos: [f64; 3],
    u: u32,
    lines: Vec<[f32; 3]>,
    drawn_lines: Vec<[f32; 3]>,
    line_scale: f32,
    drawn_scale: f32,
    emitters: [Emitter; 2],
    sparks: Vec<Sparks>,
}

impl C557 {
    pub fn new(pos: [f64; 3], seed: u32) -> Self {
        let mut seed = u64::from(seed) | 1;
        let lines = (0..64)
            .map(|_| {
                let x = 0.1 * ((lcg(&mut seed) % 480) as f32 - 240.0);
                let z = 0.1 * ((lcg(&mut seed) % 480) as f32 - 240.0);
                let y = 0.1 * (lcg(&mut seed) % 240) as f32;
                [x, y, z]
            })
            .collect::<Vec<_>>();
        let mut emitter = |base: f32, step: u8, scale: f32, half: f32, rgb: [u8; 3]| Emitter {
            points: (0..16)
                .map(|_| {
                    let x = (lcg(&mut seed) % 20) as f32 - 10.0;
                    let y = base + (lcg(&mut seed) & 15) as f32 - 4.0;
                    [x, y, 0.0]
                })
                .collect(),
            alpha: 96,
            step,
            scale,
            half,
            rgb,
            drawn_alpha: 96,
        };
        let a = emitter(5.0, 4, 1.02, 12.0, [64, 32, 96]);
        let b = emitter(10.0, 3, 1.03, 15.0, [128, 0, 0]);
        let sparks = [13, 16, 19]
            .into_iter()
            .map(|start| Sparks {
                start,
                points: (0..60)
                    .map(|_| {
                        let x = (lcg(&mut seed) % 50) as f32 - 25.0;
                        let y = (lcg(&mut seed) % 25) as f32;
                        let z = (lcg(&mut seed) % 50) as f32 - 25.0;
                        [x, y, z]
                    })
                    .collect(),
                alpha: 128,
                sigma: 0.1,
                theta: 0,
                drawn: None,
            })
            .collect();
        Self {
            pos,
            u: 0,
            drawn_lines: lines.clone(),
            lines,
            line_scale: 0.1,
            drawn_scale: 0.1,
            emitters: [a, b],
            sparks,
        }
    }

    pub fn update(&mut self, u: u32) {
        self.u = u;
        // Lines: drawn from p, then p += 3 n.
        self.drawn_lines = self.lines.clone();
        self.drawn_scale = self.line_scale;
        for p in &mut self.lines {
            let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-6);
            *p = [p[0] + 3.0 * p[0] / l, p[1] + 3.0 * p[1] / l, p[2] + 3.0 * p[2] / l];
        }
        self.line_scale = (self.line_scale + 0.3).min(1.0);
        for e in &mut self.emitters {
            for p in &mut e.points {
                *p = p.map(|v| v * e.scale);
            }
            e.drawn_alpha = e.alpha;
            e.alpha = e.alpha.saturating_sub(e.step);
        }
        for s in &mut self.sparks {
            if u < s.start {
                continue;
            }
            let t = f64::from(s.theta) * std::f64::consts::TAU / 65536.0;
            let v = [0.8 * t.sin() as f32, 0.8, 0.0];
            for p in &mut s.points {
                *p = [p[0] + v[0], p[1] + v[1], p[2]];
            }
            s.drawn = Some((s.points.clone(), s.alpha, s.sigma));
            s.alpha = s.alpha.saturating_sub(4);
            s.sigma = (s.sigma + 0.05).min(10.0);
            s.theta += 0x2000;
        }
    }

    fn line_alpha(&self) -> u8 {
        128u32.saturating_sub(4 * self.u.saturating_sub(1)) as u8
    }

    /// (additive, subtractive) triangles of the update last stepped.
    pub fn draw(&self, eye: [f64; 3], target: [f64; 3]) -> (Vec<FxTri>, Vec<FxTri>) {
        let (mut add, mut sub) = (Vec::new(), Vec::new());
        if self.u == 0 {
            return (add, sub);
        }
        let Some(project) = projector(eye, target) else {
            return (add, sub);
        };
        let [right, up, toward] = billboard_axes(eye, target);
        let bill = |l: [f32; 3], s: f32| -> [f64; 3] {
            std::array::from_fn(|i| {
                self.pos[i] + f64::from(s) * (f64::from(l[0]) * right[i] + f64::from(l[1]) * up[i] + f64::from(l[2]) * toward[i])
            })
        };
        // Speed lines (world axes), each sent twice.
        let a = self.line_alpha();
        let s = f64::from(self.drawn_scale);
        for p in &self.drawn_lines {
            let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-6);
            let q = [p[0] + 20.0 * p[0] / l, p[1] + 20.0 * p[1] / l, p[2] + 20.0 * p[2] / l];
            let w = |v: [f32; 3]| -> [f64; 3] { std::array::from_fn(|i| self.pos[i] + s * f64::from(v[i])) };
            let (Some(sa), Some(sb)) = (project(w(*p)), project(w(q))) else {
                continue;
            };
            let (dx, dy) = (sb[0] - sa[0], sb[1] - sa[1]);
            let len = (dx * dx + dy * dy).sqrt().max(1e-3);
            let (nx, ny) = (-dy / len * 0.5, dx / len * 0.5);
            let quad = [[sa[0] + nx, sa[1] + ny], [sa[0] - nx, sa[1] - ny], [sb[0] + nx, sb[1] + ny], [sb[0] - nx, sb[1] - ny]];
            let (ci, co) = ([0, 0, 0, a], [128, 48, 48, a]);
            for _ in 0..2 {
                add.push(FxTri { screen: [quad[0], quad[1], quad[2]], uv: [[0.0; 2]; 3], colour: [ci, ci, co], textured: false, sheet: 0 });
                add.push(FxTri { screen: [quad[1], quad[3], quad[2]], uv: [[0.0; 2]; 3], colour: [ci, co, co], textured: false, sheet: 0 });
            }
        }
        // Emitters: A subtractive, B additive; at most 8 puffs each.
        let u = self.u;
        for (n, e) in self.emitters.iter().enumerate() {
            for (k, p) in e.points.iter().enumerate().take(8) {
                let k = k as u32;
                if !(k == 0 || u >= 3 * k) {
                    continue;
                }
                let f = ((u.saturating_sub(1 + 3 * k) / 2) % 8) as f32;
                let h = e.half;
                let c = |dx: f32, dy: f32| project(bill([p[0] + dx, p[1] + dy, p[2]], 1.0));
                let (Some(a0), Some(a1), Some(a2), Some(a3)) = (c(-h, h), c(h, h), c(-h, -h), c(h, -h)) else {
                    continue;
                };
                let uv = [[240.0, 16.0 * f], [255.0, 16.0 * f], [240.0, 16.0 * f + 15.0], [255.0, 16.0 * f + 15.0]].map(|[x, y]: [f32; 2]| [x / 256.0, y / 256.0]);
                let col = [e.rgb[0], e.rgb[1], e.rgb[2], e.drawn_alpha];
                let out = if n == 0 { &mut sub } else { &mut add };
                out.push(FxTri { screen: [a0, a1, a2], uv: [uv[0], uv[1], uv[2]], colour: [col; 3], textured: true, sheet: 15 });
                out.push(FxTri { screen: [a1, a3, a2], uv: [uv[1], uv[3], uv[2]], colour: [col; 3], textured: true, sheet: 15 });
            }
        }
        // Spark clouds.
        for s in &self.sparks {
            let Some((points, alpha, sigma)) = &s.drawn else {
                continue;
            };
            if u < s.start {
                continue;
            }
            for p in points {
                let c = |dx: f32, dy: f32| project(bill([p[0] + dx, p[1] + dy, p[2]], *sigma));
                let (Some(a0), Some(a1), Some(a2), Some(a3)) = (c(-2.0, 2.0), c(2.0, 2.0), c(-2.0, -2.0), c(2.0, -2.0)) else {
                    continue;
                };
                let uv = [[176.0, 96.0], [207.0, 96.0], [176.0, 127.0], [207.0, 127.0]].map(|[x, y]: [f32; 2]| [x / 256.0, y / 256.0]);
                let col = [128, 32, 32, *alpha];
                add.push(FxTri { screen: [a0, a1, a2], uv: [uv[0], uv[1], uv[2]], colour: [col; 3], textured: true, sheet: 15 });
                add.push(FxTri { screen: [a1, a3, a2], uv: [uv[1], uv[3], uv[2]], colour: [col; 3], textured: true, sheet: 15 });
            }
        }
        (add, sub)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphas_and_spark_starts_follow_the_note() {
        let mut c = C557::new([0.0; 3], 5);
        for u in 1..=50 {
            c.update(u);
            let active = c.sparks.iter().filter(|s| s.drawn.is_some()).count();
            let want = [13, 16, 19].iter().filter(|&&s| u >= s).count();
            assert_eq!(active, want, "u {u}");
            if u == 2 {
                assert_eq!(c.line_alpha(), 124);
                assert_eq!((c.emitters[0].drawn_alpha, c.emitters[1].drawn_alpha), (92, 93));
            }
            if u == 33 {
                assert_eq!(c.line_alpha(), 0);
            }
        }
        // Each spark cloud: 128 at its first update, 0 after 32.
        assert_eq!(c.sparks[0].drawn.as_ref().unwrap().1, 0);
    }
}
