//! The battle's return on the versus screen (C module 604 run again after
//! the arena): one or two flash passes centred on the cards, the second
//! carrying the damage number.
//!
//! Traced [F c604 0x1E03FD4.., 0x1E02BF0 rays, 0x1E03200 beam, 0x1E00460
//! glow, 0x1E01300 rings, 0x1E024B0 sparks; emu footage RAM 087-090 and
//! the GS dump dumps-battle-damage; notes
//! `claude-units/C604-RETURN-EFFECTS.md`]:
//! - Pass 1 is centred on the right card (view x +128), pass 2 on the left
//!   (-128). Each pass scales a node from 0.1 to 1.0 in 5 updates (+0.2),
//!   then grows it by 0.005 per update; from k = 5 every layer fades by its
//!   own step; the pass ends when the glow reaches 0 (k = 21).
//! - Layers, all from TEXEFF record 15, tinted (176, 80, 96), added
//!   (Cs * As + Cd): 64 rays (random orientation, +0x80 per update), a
//!   beam with a random colour, a four-quadrant glow, two ring bands at
//!   radius 120 and 60 sparks flying outward (x 1.17 per update).
//! - At the battle camera every local point projects in closed form:
//!   vx = 128 sigma + s (lx - ly), vy = -s (lx + ly), vz = 800 + s (ly - lz),
//!   X = 256 + 800 vx / vz, Y = 256 + 1074.2855 vy / vz; a quad with a
//!   vertex off the GS range is dropped.
//! - The random values (0x19AE88) are not reproduced bit for bit: this
//!   port uses its own generator [H].
use crate::duel_versus::DamageNumber;

/// Pass length: the glow fades 96 -> 0 by 6 from k = 5 (k = 21 ends it).
pub const PASS_UPDATES: u32 = 22;
/// c604's lifetime (+0x13E4): 60 updates from the fly-in, so pass 2 is cut
/// after 18 updates.
pub const SECOND_PASS_UPDATES: u32 = 18;
pub const TINT: [u8; 3] = [176, 80, 96];

/// 0x2760E0's mode: (damage != 0, event bit 28) -> 1 / 0 / 2 / 3.
/// Mode 0: pass 1 with the number on the right card; 1: both passes, the
/// number on the left in pass 2; 2: both passes, no number; 3: pass 1 only.
pub fn mode(damage: bool, bit28: bool) -> u8 {
    match (damage, bit28) {
        (true, true) => 1,
        (true, false) => 0,
        (false, true) => 2,
        (false, false) => 3,
    }
}

/// One textured quad in GS window coordinates (strip order) with texel UVs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectQuad {
    pub screen: [[f32; 2]; 4],
    pub uv: [[f32; 2]; 4],
}

/// A layer's quads and its vertex colour (alpha 128 = 1.0).
#[derive(Clone, Debug, PartialEq)]
pub struct EffectLayer {
    pub name: &'static str,
    pub colour: [u8; 4],
    pub quads: Vec<EffectQuad>,
}

/// A small LCG standing in for the executable's rand().
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (self.0 >> 16) & 0x7FFF
    }
}

/// 0x20BDA0 rotation (row vectors, 65536 per turn).
fn rotate(p: [f32; 3], r: [i32; 3]) -> [f32; 3] {
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

/// The fading alpha: a0 up to k = 5, then a0 - step (k - 5), at least 0.
fn fade(a0: u32, step: u32, k: u32) -> u8 {
    a0.saturating_sub(step * k.saturating_sub(5)) as u8
}

/// One flash pass.
#[derive(Clone, Debug, PartialEq)]
pub struct FlashPass {
    /// +1 right card (pass 1), -1 left card (pass 2).
    pub sigma: f32,
    /// Update within the pass.
    pub k: u32,
    pub scale: f32,
    rays: Vec<[i32; 3]>,
    sparks: Vec<[f32; 3]>,
    rng: Rng,
}

impl FlashPass {
    pub fn new(right: bool, seed: u32) -> Self {
        let mut rng = Rng(seed);
        let rays = (0..64)
            .map(|_| std::array::from_fn(|_| 3 * (rng.next() as i32 & 0xFFFF)))
            .collect();
        let sparks = (0..60)
            .map(|_| {
                let x = (rng.next() % 100) as f32 - 50.0;
                let z = (rng.next() % 50) as f32;
                let y = (rng.next() % 100) as f32 - 50.0;
                // Grown once before the first draw.
                [x * 1.17, y * 1.17, z * 1.17]
            })
            .collect();
        Self {
            sigma: if right { 1.0 } else { -1.0 },
            k: 0,
            scale: 0.1,
            rays,
            sparks,
            rng,
        }
    }

    /// The closed-form projection of a local point with node scale `s`.
    fn project(&self, s: f32, l: [f32; 3]) -> Option<[f32; 2]> {
        let vx = 128.0 * self.sigma + s * (l[0] - l[1]);
        let vy = -s * (l[0] + l[1]);
        let vz = 800.0 + s * (l[1] - l[2]);
        if vz < 1.0 {
            return None;
        }
        let x = 256.0 + 800.0 * vx / vz;
        let y = 256.0 + 1074.2855 * vy / vz;
        let ok = |v: f32| (-1792.0..=2303.0).contains(&v);
        (ok(x) && ok(y)).then_some([x, y])
    }

    fn quad(&self, s: f32, points: [[f32; 3]; 4], uv: [[f32; 2]; 4]) -> Option<EffectQuad> {
        let mut screen = [[0.0; 2]; 4];
        for (i, p) in points.iter().enumerate() {
            screen[i] = self.project(s, *p)?;
        }
        Some(EffectQuad { screen, uv })
    }

    /// The layers drawn this update (in the original's draw order: rays,
    /// glow, ring B, ring A, sparks, beam).
    pub fn layers(&self) -> Vec<EffectLayer> {
        let (k, s) = (self.k, self.scale);
        let tint = |a: u8| [TINT[0], TINT[1], TINT[2], a];
        let streak = [[142.0, 0.0], [175.0, 0.0], [142.0, 63.0], [175.0, 63.0]];
        let mut layers = Vec::new();
        let rays = self
            .rays
            .iter()
            .filter_map(|r| {
                let p = [
                    [-15.0, 300.0, 0.0],
                    [15.0, 300.0, 0.0],
                    [-7.5, 0.0, 0.0],
                    [7.5, 0.0, 0.0],
                ];
                self.quad(s, p.map(|v| rotate(v, *r)), streak)
            })
            .collect();
        layers.push(EffectLayer {
            name: "rays",
            colour: tint(fade(32, 4, k)),
            quads: rays,
        });
        let p = |x: f32, y: f32| [100.0 * x, 100.0 * y, 0.0];
        let pts = [
            p(-1.0, 0.0),
            p(-1.0, 1.0),
            p(0.0, 1.0),
            p(1.0, 1.0),
            p(1.0, 0.0),
            p(1.0, -1.0),
            p(0.0, -1.0),
            p(-1.0, -1.0),
        ];
        let c = [0.0, 0.0, 0.0];
        let glow_uv = [[176.0, 32.0], [208.0, 32.0], [176.0, 64.0], [208.0, 64.0]];
        let glow = [
            [pts[1], pts[2], pts[0], c],
            [pts[3], pts[2], pts[4], c],
            [pts[5], pts[6], pts[4], c],
            [pts[7], pts[6], pts[0], c],
        ]
        .into_iter()
        .filter_map(|q| self.quad(s, q, glow_uv))
        .collect();
        layers.push(EffectLayer {
            name: "glow",
            colour: tint(fade(96, 6, k)),
            quads: glow,
        });
        for (name, outer, a0) in [("ring B", 130.0f32, 48), ("ring A", 115.0, 32)] {
            let ring = |r: f32, j: u32| {
                let a = (12.0 * j as f32).to_radians();
                [r * a.cos(), r * a.sin(), 0.0]
            };
            let quads = (0..30)
                .filter_map(|j| {
                    self.quad(
                        s,
                        [
                            ring(120.0, j),
                            ring(outer, j),
                            ring(120.0, j + 1),
                            ring(outer, j + 1),
                        ],
                        [[142.0, 64.0], [142.0, 0.0], [174.0, 64.0], [174.0, 0.0]],
                    )
                })
                .collect();
            layers.push(EffectLayer {
                name,
                colour: tint(fade(a0, 4, k)),
                quads,
            });
        }
        let sparks = self
            .sparks
            .iter()
            .filter_map(|[x, y, z]| {
                self.quad(
                    s,
                    [
                        [x - 2.5, y + 2.5, *z],
                        [x + 2.5, y + 2.5, *z],
                        [x - 2.5, y - 2.5, *z],
                        [x + 2.5, y - 2.5, *z],
                    ],
                    [[176.0, 64.0], [207.0, 64.0], [176.0, 95.0], [207.0, 95.0]],
                )
            })
            .collect();
        layers.push(EffectLayer {
            name: "sparks",
            colour: tint(fade(128, 4, k)),
            quads: sparks,
        });
        // The beam: its own node (scale 1), tip 100, 20, -60, -140, -200.
        let tip = [100.0, 20.0, -60.0, -140.0][k.min(4) as usize..]
            .first()
            .copied()
            .unwrap_or(-200.0);
        let beam_points = [
            [-20.0, 200.0, 0.0],
            [20.0, 200.0, 0.0],
            [-2.0, tip, 0.0],
            [2.0, tip, 0.0],
        ]
        .map(|v| rotate(v, [0, 0, -0x2000]));
        let mut rng = self.rng;
        let (r, g, b) = (rng.next() % 255, rng.next() % 255, rng.next() % 255);
        layers.push(EffectLayer {
            name: "beam",
            colour: [r as u8, g as u8, b as u8, fade(128, 16, k)],
            quads: self.quad(1.0, beam_points, streak).into_iter().collect(),
        });
        layers
    }

    /// After this update's draw: angles turn, sparks grow, the node scales.
    pub fn step(&mut self) {
        for r in &mut self.rays {
            for a in r.iter_mut() {
                *a += 0x80;
            }
        }
        for p in &mut self.sparks {
            for v in p.iter_mut() {
                *v *= 1.17;
            }
        }
        for _ in 0..3 {
            self.rng.next();
        }
        self.scale = if self.k < 4 {
            if self.scale <= 0.8 {
                self.scale + 0.2
            } else {
                1.0
            }
        } else if self.k == 4 {
            1.0
        } else {
            self.scale + 0.005
        };
        self.k += 1;
    }
}

/// The return: its mode, the running pass and the damage number.
#[derive(Clone, Debug, PartialEq)]
pub struct Return {
    pub mode: u8,
    pub pass: FlashPass,
    /// 0 or 1.
    pub pass_index: u8,
    pub number: Option<DamageNumber>,
    damage: i32,
    pub done: bool,
}

impl Return {
    /// `damage`: the LP change (negative), 0 for none.
    pub fn new(mode: u8, damage: i32, seed: u32) -> Self {
        let number = (mode == 0 && damage != 0).then_some(DamageNumber {
            card: 1,
            value: damage,
            frame: 0,
        });
        Self {
            mode,
            pass: FlashPass::new(true, seed),
            pass_index: 0,
            number,
            damage,
            done: false,
        }
    }

    /// One update (after this update's draw).
    pub fn tick(&mut self) {
        if self.done {
            return;
        }
        self.pass.step();
        if let Some(n) = &mut self.number {
            n.frame += 1;
        }
        let k = self.pass.k;
        if self.pass_index == 0 && k >= PASS_UPDATES {
            if matches!(self.mode, 1 | 2) {
                self.pass_index = 1;
                self.pass = FlashPass::new(false, self.pass.rng.next());
                self.number = (self.mode == 1 && self.damage != 0).then_some(DamageNumber {
                    card: 0,
                    value: self.damage,
                    frame: 0,
                });
            } else {
                self.done = true;
            }
        } else if self.pass_index == 1 && k >= SECOND_PASS_UPDATES {
            self.done = true;
        }
    }

    /// Updates the whole return lasts for `mode`.
    pub fn length(mode: u8) -> u32 {
        match mode {
            1 | 2 => PASS_UPDATES + SECOND_PASS_UPDATES,
            _ => PASS_UPDATES,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_follow_the_traced_scale_and_fades() {
        let mut p = FlashPass::new(true, 1);
        let mut scales = Vec::new();
        for _ in 0..8 {
            scales.push(p.scale);
            p.step();
        }
        assert!((scales[0] - 0.1).abs() < 1e-6 && (scales[4] - 0.9).abs() < 1e-5);
        assert_eq!(scales[5], 1.0);
        assert!((scales[7] - 1.01).abs() < 1e-5);
        // Fades from k = 5: rays 32 -> 0 at 13, glow 96 -> 0 at 21.
        assert_eq!(
            (fade(32, 4, 5), fade(32, 4, 6), fade(32, 4, 13)),
            (32, 28, 0)
        );
        assert_eq!((fade(96, 6, 20), fade(96, 6, 21)), (6, 0));
        assert_eq!(fade(128, 16, 13), 0);
    }

    #[test]
    fn modes_place_the_number_and_the_passes() {
        assert_eq!(mode(true, false), 0);
        let mut r = Return::new(1, -600, 7);
        assert!(r.number.is_none());
        for _ in 0..PASS_UPDATES {
            r.tick();
        }
        assert_eq!(r.pass_index, 1);
        assert_eq!(r.number.map(|n| n.card), Some(0));
        for _ in 0..SECOND_PASS_UPDATES {
            r.tick();
        }
        assert!(r.done);
        let r0 = Return::new(0, -300, 7);
        assert_eq!(r0.number.map(|n| n.card), Some(1));
    }

    #[test]
    fn beam_vertex_matches_the_footage_draw() {
        // dumps-battle-damage draw 1848 (pass 1): a beam vertex at (339.44,
        // -62.0) - the rotated top-left corner (-20, 200) at node scale 1.
        let p = FlashPass::new(true, 1);
        let v = p
            .project(1.0, rotate([-20.0, 200.0, 0.0], [0, 0, -0x2000]))
            .unwrap();
        assert!(
            (v[0] - 339.48).abs() < 0.05 && (v[1] + 61.98).abs() < 0.05,
            "{v:?}"
        );
    }
}
