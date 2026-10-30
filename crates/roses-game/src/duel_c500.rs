//! CMODULE c500 (0x1F4), Nightmare Scorpion's "Spellbound!", ported from
//! the interpreter-matched model `claude-activate/tools/mf_c500_model.py`
//! (MONSTER-EFFECTS-PRESENTATION.md 3.1) [F code; emu: every vertex of the
//! original's run, <= 0.127 px]:
//! - u 1-11: a TEXEFF 3 seal (25 x 25 billboard, two mirrored halves, a
//!   2-frame flicker) falls from y 200 by 20 per update with a tapered
//!   4-fin trail (8-point history, (160, 160, 96)); it lands at u 11 with a
//!   one-frame full-screen flash;
//! - from u 12: 6 flickering TEXEFF 15 bolts from the target (re-randomised
//!   every update, growing one segment per update after a 1-6 update delay),
//!   60 purple sparks (x 1.1) on a billboard 3 above, three purple cones;
//!   all additive. 30 updates.
//!
//! [H / not reproduced]: the port's random values; no depth test.
use crate::duel_c600::rotate;
use crate::duel_c602::{billboard_axes, projector};
use crate::duel_c621::FxTri;

pub const UPDATES: u32 = 30;
const GOLD: [u8; 3] = [160, 160, 96];

fn lcg(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
    ((*seed >> 32) & 0x7FFF_FFFF) as u32
}

fn decay(a: u8, s: u8) -> u8 {
    if a >= s { a - s } else { 0 }
}

/// 65536-per-turn angle of an f32 radian value.
fn ang(x: f32) -> i32 {
    ((65536.0f32 * x / std::f32::consts::TAU) as i32) & 0xFFFF
}

#[derive(Clone, Debug, PartialEq)]
struct Bolt {
    delay: u32,
    alpha: u8,
    rot: i32,
}

/// What one update draws.
#[derive(Clone, Debug, PartialEq, Default)]
struct Drawn {
    /// Seal position and its UV column, the trail history (newest first).
    seal: Option<([f64; 3], f32, Vec<[f64; 3]>)>,
    flash: bool,
    /// Bolt quads (world corners, alphas top / bottom, u0).
    bolts: Vec<([[f64; 3]; 4], u8, u8, f32)>,
    sparks: Option<(Vec<[f32; 3]>, f32, u8)>,
    rings: Option<(f32, [u8; 3])>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct C500 {
    seed: u64,
    pub pos: [f64; 3],
    rings: Vec<Vec<([f32; 3], [f32; 3])>>,
    sparks: Vec<[f32; 3]>,
    bolts: Vec<Bolt>,
    y: f32,
    hist: Vec<[f64; 3]>,
    landed: bool,
    s_sparks: f32,
    a_sparks: u8,
    s_rings: f32,
    a_rings: [u8; 3],
    drawn: Drawn,
}

impl C500 {
    pub fn new(pos: [f64; 3], seed: u32) -> Self {
        let mut seed = u64::from(seed) | 1;
        let mut rings = Vec::new();
        for (r, h) in [(34.0f32, 28.0f32), (37.0, 21.0), (40.0, 14.0)] {
            let mut ent = vec![([0.1, 0.0, 0.0], [r, h, 0.0])];
            for k in 0..30 {
                let a = 0.209_439_52f32 * (k + 1) as f32;
                let j = (((lcg(&mut seed) % (2 * h as u32)) as i32) - h as i32) >> 1;
                ent.push(([0.1 * a.cos(), 0.0, 0.1 * a.sin()], [r * a.cos(), h + j as f32, r * a.sin()]));
            }
            rings.push(ent);
        }
        let sparks = (0..60)
            .map(|_| [(lcg(&mut seed) % 50) as f32 - 25.0, (lcg(&mut seed) % 25) as f32, (lcg(&mut seed) % 50) as f32 - 25.0])
            .collect();
        let mut bolts = Vec::new();
        for _ in 0..6 {
            for _ in 0..10 {
                lcg(&mut seed);
            }
            bolts.push(Bolt { delay: lcg(&mut seed) % 6 + 5, alpha: 128, rot: 0 });
        }
        for (i, b) in bolts.iter_mut().enumerate() {
            b.rot = i as i32 * 0x2AAA + (lcg(&mut seed) & 0x1FFF) as i32 - 0x1000;
        }
        let start = [pos[0], 200.0, pos[2]];
        Self {
            seed,
            pos,
            rings,
            sparks,
            bolts,
            y: 200.0,
            hist: vec![start; 8],
            landed: false,
            s_sparks: 0.1,
            a_sparks: 128,
            s_rings: 1.0,
            a_rings: [96; 3],
            drawn: Drawn::default(),
        }
    }

    pub fn update(&mut self, u: u32) {
        let mut d = Drawn::default();
        let t = self.pos;
        if !self.landed {
            let p = [t[0], f64::from(self.y), t[2]];
            self.hist.insert(0, p);
            self.hist.truncate(8);
            d.seal = Some((p, if u % 2 == 1 { 64.0 } else { 96.0 }, self.hist.clone()));
            if self.y < 20.0 {
                self.y = 0.0;
                d.flash = true;
                self.landed = true;
            } else {
                self.y -= 20.0;
            }
        } else {
            for i in 0..self.bolts.len() {
                let mut p = Vec::new();
                for k in 0..5 {
                    let x = (lcg(&mut self.seed) % 10) as f32 - 5.0;
                    let yy = if k == 4 { (lcg(&mut self.seed) % 10) as f32 - 5.0 } else { (lcg(&mut self.seed) % 20) as f32 };
                    p.push([x, yy, 8.0 * (k + 1) as f32]);
                }
                let b = &self.bolts[i];
                let n = 5i32 - b.delay as i32;
                let a = b.alpha;
                let u0 = if a >= 0x60 { 10.0 } else { 54.0 };
                let w = |v: [f32; 3]| -> [f64; 3] {
                    let r = rotate(v, [0, b.rot, 0]);
                    [t[0] + f64::from(r[0]), t[1] + f64::from(r[1]), t[2] + f64::from(r[2])]
                };
                for s in 0..n.max(0) as usize {
                    let pv = if s == 0 { [0.0; 3] } else { p[s - 1] };
                    let cu = p[s];
                    let plus = |v: [f32; 3]| [v[0] + 13.75, v[1], v[2]];
                    let k0 = 16 * (5 - s as u32);
                    let k1 = 16 * (4 - s as u32);
                    let a0 = if u32::from(a) >= k0 { (u32::from(a) - k0) as u8 } else { 0 };
                    let a1 = if u32::from(a) >= k1 { (u32::from(a) - k1) as u8 } else { 0 };
                    d.bolts.push(([w(pv), w(cu), w(plus(pv)), w(plus(cu))], a0, a1, u0));
                }
                let b = &mut self.bolts[i];
                if b.delay > 0 {
                    b.delay -= 1;
                }
                b.alpha = decay(b.alpha, 2);
            }
            let s_draw = self.s_sparks;
            if self.s_sparks < 1.0 {
                self.s_sparks += 0.2;
            }
            for p in &mut self.sparks {
                *p = p.map(|v| v * 1.1);
            }
            d.sparks = Some((self.sparks.clone(), s_draw, self.a_sparks));
            self.a_sparks = decay(self.a_sparks, 8);
            let r_draw = self.s_rings;
            self.s_rings += if self.s_rings < 1.0 { 0.2 } else { 0.05 };
            d.rings = Some((r_draw, self.a_rings));
            self.a_rings = self.a_rings.map(|a| decay(a, 8));
        }
        self.drawn = d;
    }

    /// The update's triangles: (scene layer, full-screen flash).
    pub fn draw(&self, eye: [f64; 3], target: [f64; 3]) -> (Vec<FxTri>, Vec<FxTri>) {
        let mut out = Vec::new();
        let mut flash = Vec::new();
        let Some(project) = projector(eye, target) else {
            return (out, flash);
        };
        let axes = billboard_axes(eye, target);
        let bill = |c: [f64; 3], l: [f32; 3], s: f32| -> [f64; 3] {
            std::array::from_fn(|i| c[i] + f64::from(s) * (f64::from(l[0]) * axes[0][i] + f64::from(l[1]) * axes[1][i] + f64::from(l[2]) * axes[2][i]))
        };
        let quad = |out: &mut Vec<FxTri>, w: [[f64; 3]; 4], uv: [[f32; 2]; 4], c: [[u8; 4]; 4], textured: bool, sheet: u8| {
            let (Some(p0), Some(p1), Some(p2), Some(p3)) = (project(w[0]), project(w[1]), project(w[2]), project(w[3])) else {
                return;
            };
            out.push(FxTri { screen: [p0, p1, p2], uv: [uv[0], uv[1], uv[2]], colour: [c[0], c[1], c[2]], textured, sheet });
            out.push(FxTri { screen: [p1, p3, p2], uv: [uv[1], uv[3], uv[2]], colour: [c[1], c[3], c[2]], textured, sheet });
        };
        let d = &self.drawn;
        let gold = |a: u8| [GOLD[0], GOLD[1], GOLD[2], a];
        if let Some((p, u0, hist)) = &d.seal {
            // The seal: TEXEFF 3 (128 x 64), two mirrored halves.
            let pts: [[f32; 3]; 6] = [[-12.5, 12.5, 0.0], [-12.5, -12.5, 0.0], [12.5, 12.5, 0.0], [12.5, -12.5, 0.0], [0.0, 12.5, 0.0], [0.0, -12.5, 0.0]];
            let uv = [[*u0 / 128.0, 0.0], [(*u0 + 32.0) / 128.0, 0.0], [*u0 / 128.0, 1.0], [(*u0 + 32.0) / 128.0, 1.0]];
            for qq in [[0usize, 4, 1, 5], [2, 4, 3, 5]] {
                quad(&mut out, qq.map(|k| bill(*p, pts[k], 1.0)), uv, [[128, 128, 128, 128]; 4], true, 3);
            }
            // The trail: a tapered 4-fin tube along the history.
            let mut prev: Option<[[f64; 3]; 4]> = None;
            for c in (0..7).rev() {
                let dv = [hist[c + 1][0] - hist[c][0], hist[c + 1][1] - hist[c][1], hist[c + 1][2] - hist[c][2]];
                let b = ang((dv[0] as f32).atan2(dv[2] as f32));
                let a = ang(((dv[0] * dv[0] + dv[2] * dv[2]).sqrt() as f32).atan2(dv[1] as f32));
                let wd = 5.0 * (8 - c) as f32 / 8.0;
                let cor = [[wd, 0.0, 0.0], [-wd, 0.0, 0.0], [0.0, 0.0, wd], [0.0, 0.0, -wd]].map(|v| {
                    let r = rotate(v, [a, b, 0]);
                    [hist[c][0] + f64::from(r[0]), hist[c][1] + f64::from(r[1]), hist[c][2] + f64::from(r[2])]
                });
                for s in 0..4 {
                    if c == 6 {
                        if let (Some(a0), Some(a1), Some(a2)) = (project(hist[7]), project(hist[6]), project(cor[s])) {
                            out.push(FxTri { screen: [a0, a1, a2], uv: [[0.0; 2]; 3], colour: [gold(64), gold(64), gold(0)], textured: false, sheet: 0 });
                        }
                    } else if let Some(pr) = prev {
                        let inner = if c > 0 { 64 } else { 0 };
                        quad(&mut out, [hist[c + 1], hist[c], pr[s], cor[s]], [[0.0; 2]; 4], [gold(64), gold(inner), gold(0), gold(0)], false, 0);
                    }
                }
                prev = Some(cor);
            }
        }
        if d.flash {
            let c = [128, 128, 128, 128];
            let full = [[0.0, 0.0], [512.0, 0.0], [0.0, 512.0], [512.0, 512.0]];
            flash.push(FxTri { screen: [full[0], full[1], full[2]], uv: [[0.0; 2]; 3], colour: [c; 3], textured: false, sheet: 0 });
            flash.push(FxTri { screen: [full[1], full[3], full[2]], uv: [[0.0; 2]; 3], colour: [c; 3], textured: false, sheet: 0 });
        }
        for (w, a0, a1, u0) in &d.bolts {
            let uv = [[*u0, 0.0], [*u0, 128.0], [*u0 + 44.0, 0.0], [*u0 + 44.0, 128.0]].map(|[u, v]: [f32; 2]| [u / 256.0, v / 256.0]);
            quad(&mut out, *w, uv, [gold(*a0), gold(*a1), gold(*a0), gold(*a1)], true, 15);
        }
        if let Some((sparks, s, a)) = &d.sparks {
            let n = [self.pos[0], 3.0, self.pos[2]];
            let uv = [[0.8125, 0.25], [0.9375, 0.25], [0.8125, 0.375], [0.9375, 0.375]];
            for &[x, y, z] in sparks {
                quad(
                    &mut out,
                    [[x - 2.5, y + 2.5, z], [x + 2.5, y + 2.5, z], [x - 2.5, y - 2.5, z], [x + 2.5, y - 2.5, z]].map(|l| bill(n, l, *s)),
                    uv,
                    [[128, 64, 128, *a]; 4],
                    true,
                    15,
                );
            }
        }
        if let Some((s, alphas)) = &d.rings {
            let t = self.pos;
            let w = |l: [f32; 3]| -> [f64; 3] { std::array::from_fn(|i| t[i] + f64::from(*s) * f64::from(l[i])) };
            let uv = [[142.0, 64.0], [142.0, 0.0], [174.0, 64.0], [174.0, 0.0]].map(|[u, v]: [f32; 2]| [u / 256.0, v / 256.0]);
            for (ri, ent) in self.rings.iter().enumerate() {
                for k in 0..30 {
                    let (i0, o0) = ent[k];
                    let (i1, o1) = if k < 29 { ent[k + 1] } else { ent[0] };
                    quad(&mut out, [w(i0), w(o0), w(i1), w(o1)], uv, [[128, 64, 128, alphas[ri]]; 4], true, 15);
                }
            }
        }
        (out, flash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_falls_lands_at_11_with_a_flash() {
        let mut c = C500::new([0.0; 3], 4);
        let mut landed = None;
        for u in 1..=30 {
            c.update(u);
            if c.drawn.flash {
                landed = Some(u);
            }
            if u == 1 {
                assert_eq!(c.drawn.seal.as_ref().unwrap().0[1], 200.0);
            }
        }
        assert_eq!(landed, Some(11));
        // Rings: 96 - 8 per update from u 12 (0 at u 24).
        assert_eq!(c.drawn.rings.unwrap().1, [0; 3]);
    }
}
