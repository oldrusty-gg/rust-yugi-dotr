//! CMODULE c544 (0x220) "Spellbound", as traced in
//! the project notes 2 [F code; emu: the model matched
//! the interpreter's 3,732 vertices, alphas, UVs and the light curve to
//! 0.062 px]: a billboard node at the victim's cell centre carries a
//! TEXEFF 11 seal 160 units screen-up (far above the screen at the cell
//! camera) and 8 TEXEFF 15 lightning bolts that grow down from it one
//! segment per update, each with a 2-frame impact glow; the board light
//! 0x2DF5B1 dips 0x80 -> 0x20 by 16 (u 1-7) and comes back from u 38.
//! 46 updates, all additive.
//!
//! [H / not reproduced]: the port's random values (distributions only);
//! drawn over the board without a depth test (the original depth-sorts).
use crate::duel_c602::{billboard_axes, projector};
use crate::duel_c621::FxTri;

pub const UPDATES: u32 = 46;

#[derive(Clone, Debug, PartialEq)]
struct Bolt {
    /// Countdown before the bolt grows (5 + 2 i at the start).
    c: u8,
    alpha: u8,
    /// TEXEFF 15 strip: 10 (wide bolt), 54 once the alpha drops below 0x60.
    u0: f32,
    points: [[f32; 3]; 6],
}

/// One textured quad in node-local coordinates.
#[derive(Clone, Debug, PartialEq)]
struct Quad {
    local: [[f32; 3]; 4],
    uv: [[f32; 2]; 4],
    /// RGBA per vertex.
    colour: [[u8; 4]; 4],
    sheet: u8,
    size: [f32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct C544 {
    seed: u64,
    pub pos: [f64; 3],
    life: u32,
    light: u8,
    seal_alpha: u8,
    bolts: Vec<Bolt>,
    glow_frame: bool,
    /// The board light passed this update (0x1F7F00).
    pub light_out: u8,
    quads: Vec<Quad>,
}

fn decay(a: u8, min: u8, s: u8) -> (u8, bool) {
    if a >= min.saturating_add(s) { (a - s, true) } else { (min, false) }
}

fn rise(a: u8, max: u8, s: u8) -> (u8, bool) {
    if a <= max.saturating_sub(s) { (a + s, true) } else { (max, false) }
}

impl C544 {
    pub fn new(pos: [f64; 3], seed: u32) -> Self {
        let mut c = Self {
            seed: u64::from(seed) | 1,
            pos,
            life: UPDATES,
            light: 0x80,
            seal_alpha: 0,
            bolts: Vec::new(),
            glow_frame: false,
            light_out: 0x80,
            quads: Vec::new(),
        };
        for i in 0..8u8 {
            let mut points = [[0.0, 160.0, 0.0]; 6];
            for j in 1..=5 {
                let (x, z) = if j < 5 {
                    ((c.rand() % 30) as f32 - 15.0, (c.rand() % 30) as f32 - 15.0)
                } else {
                    ((c.rand() % 10) as f32 - 5.0, (c.rand() % 10) as f32 - 5.0)
                };
                points[j] = [x, 160.0 - 32.0 * j as f32, z];
            }
            c.bolts.push(Bolt { c: 5 + 2 * i, alpha: 128, u0: 10.0, points });
        }
        c
    }

    fn rand(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
        ((self.seed >> 32) & 0x7FFF_FFFF) as u32
    }

    /// One module update (0x1E01F60's order).
    pub fn update(&mut self) {
        self.quads.clear();
        self.light_out = self.light;
        // The seal: the TEXEFF 11 quarter mirrored into a round seal.
        let ea = self.seal_alpha;
        for (cx, cy) in [(-25.0f32, 185.0f32), (25.0, 185.0), (25.0, 135.0), (-25.0, 135.0)] {
            self.quads.push(Quad {
                local: [[cx, cy, 0.0], [0.0, cy, 0.0], [cx, 160.0, 0.0], [0.0, 160.0, 0.0]],
                uv: [[0.0, 0.0], [64.0, 0.0], [0.0, 64.0], [64.0, 64.0]],
                colour: [[128, 128, 128, ea]; 4],
                sheet: 11,
                size: [128.0, 64.0],
            });
        }
        if self.life >= 37 {
            self.light = decay(self.light, 0x20, 16).0;
        }
        if self.life < 10 {
            self.seal_alpha = decay(self.seal_alpha, 0, 16).0;
            self.light = rise(self.light, 0x80, 16).0;
        } else {
            let (ea, moving) = rise(self.seal_alpha, 0x80, 16);
            self.seal_alpha = ea;
            if !moving {
                for i in 0..self.bolts.len() {
                    if self.bolts[i].alpha < 0x60 {
                        self.bolts[i].u0 = 54.0;
                    }
                    let b = self.bolts[i].clone();
                    if b.c == 0 {
                        let [x5, _, z5] = b.points[5];
                        let centre = [x5, 5.5 - z5, z5];
                        let f = if self.glow_frame { 208.0 } else { 176.0 };
                        self.glow_frame = !self.glow_frame;
                        let at = |dx: f32, dy: f32| [centre[0] + dx, centre[1] + dy, centre[2]];
                        self.quads.push(Quad {
                            local: [at(-10.0, 10.0), at(10.0, 10.0), at(-10.0, -10.0), at(10.0, -10.0)],
                            uv: [[f, 64.0], [f + 31.0, 64.0], [f, 95.0], [f + 31.0, 95.0]],
                            colour: [[96, 96, 176, b.alpha]; 4],
                            sheet: 15,
                            size: [256.0, 256.0],
                        });
                    }
                    if b.c <= 4 {
                        for s in 0..=(4 - b.c) as usize {
                            let (p, q) = (b.points[s], b.points[s + 1]);
                            let top = b.alpha.saturating_sub(16 * (5 - s as u8));
                            let bottom = b.alpha.saturating_sub(16 * (4 - s as u8));
                            let w = |v: [f32; 3]| [v[0] + 11.0, v[1], v[2]];
                            let rgb = |a: u8| [96, 96, 176, a];
                            self.quads.push(Quad {
                                local: [p, q, w(p), w(q)],
                                uv: [[b.u0, 0.0], [b.u0, 127.0], [b.u0 + 43.0, 0.0], [b.u0 + 43.0, 127.0]],
                                colour: [rgb(top), rgb(bottom), rgb(top), rgb(bottom)],
                                sheet: 15,
                                size: [256.0, 256.0],
                            });
                        }
                    }
                    if b.c > 0 {
                        self.bolts[i].c -= 1;
                    } else {
                        let (a, moving) = decay(b.alpha, 0, 16);
                        self.bolts[i].alpha = a;
                        if !moving {
                            self.bolts[i].c = 5;
                            self.bolts[i].alpha = 128;
                            for j in 1..=5 {
                                let (x, z) = if j < 5 {
                                    ((self.rand() % 40) as f32 - 20.0, (self.rand() % 30) as f32 - 15.0)
                                } else {
                                    ((self.rand() % 30) as f32 - 15.0, (self.rand() % 30) as f32 - 15.0)
                                };
                                self.bolts[i].points[j] = [x, 160.0 - 32.0 * j as f32, z];
                            }
                        }
                    }
                }
            }
        }
        self.life = self.life.saturating_sub(1);
    }

    /// The update's triangles through the board camera (billboard node).
    pub fn draw(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<FxTri> {
        let mut out = Vec::new();
        let Some(project) = projector(eye, target) else {
            return out;
        };
        let [right, up, toward] = billboard_axes(eye, target);
        for q in &self.quads {
            let mut sc = [[0.0f32; 2]; 4];
            let mut ok = true;
            for (k, l) in q.local.iter().enumerate() {
                let w: [f64; 3] = std::array::from_fn(|i| {
                    self.pos[i] + f64::from(l[0]) * right[i] + f64::from(l[1]) * up[i] + f64::from(l[2]) * toward[i]
                });
                match project(w) {
                    Some(v) => sc[k] = v,
                    None => ok = false,
                }
            }
            if !ok {
                continue;
            }
            let uv = q.uv.map(|[u, v]| [u / q.size[0], v / q.size[1]]);
            let c = q.colour;
            out.push(FxTri { screen: [sc[0], sc[1], sc[2]], uv: [uv[0], uv[1], uv[2]], colour: [c[0], c[1], c[2]], textured: true, sheet: q.sheet });
            out.push(FxTri { screen: [sc[1], sc[3], sc[2]], uv: [uv[1], uv[3], uv[2]], colour: [c[1], c[3], c[2]], textured: true, sheet: q.sheet });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_curve_and_counts_follow_the_note() {
        let mut c = C544::new([0.0; 3], 9);
        let mut light = Vec::new();
        let mut counts = Vec::new();
        for _ in 0..UPDATES {
            c.update();
            light.push(c.light_out);
            counts.push(c.quads.len());
        }
        // L(u) = max(0x20, 0x80 - 16(u - 1)) to u 38, then +16 to 0x80 at u 44.
        assert_eq!(&light[..8], &[0x80, 0x70, 0x60, 0x50, 0x40, 0x30, 0x20, 0x20]);
        assert_eq!((light[37], light[38], light[43], light[45]), (0x20, 0x30, 0x80, 0x80));
        // 4 seal quads u 1-9; bolt 0 grows from u 10; 4 again from u 38.
        assert_eq!((counts[0], counts[8], counts[9]), (4, 4, 5));
        assert_eq!((counts[37], counts[45]), (4, 4));
        assert!(counts[27..36].iter().all(|&n| n <= 44) && counts.iter().max() > Some(&30));
    }
}
