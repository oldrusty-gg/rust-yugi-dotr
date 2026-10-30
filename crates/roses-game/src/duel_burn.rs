//! The end of the battle scene (C module 605): the destroyed card burns away
//! on its versus spot while the surviving card flies back to its board
//! cell, over the board seen through the battle camera.
//!
//! Traced [F c605 0x1E02E40 and its routines; emu: c605's update run in the
//! interpreter from footage state 089 matched the card fields of states
//! 090-093 exactly; frames dumps-battle-return 122-171; notes
//! `claude-units/C605-RETURN.md`]:
//! - Mode (launch parameter): bit 1 = card A (attacker, left) destroyed,
//!   bit 0 = card B (defender, right) destroyed. 50 updates; 36 when
//!   neither is destroyed.
//! - A destroyed card: drawn on a frame placed at its versus spot and
//!   turned to the camera, which through the battle camera's degenerate
//!   view comes out rotated, magnified and in perspective (the same closed
//!   form as the return flash). The card quad loses 18.4 local units from
//!   the bottom per update (Cs * As); a row of 16 flames (TEXEFF record 1,
//!   24 x 32 frames, additive, alpha 64, -8 per update from k = 21) rises
//!   16 per update to 208; 256 sparks start at the burn front at random
//!   delays and drift down, alpha 128 -> 8.
//! - A surviving card: the versus sprite until the other card's burn
//!   counter runs out (k = 23), then the 3D card returns in 20 linear
//!   steps to its board card pose; its board card shows again on landing.
//!   Without a destroyed card both return from k = 0.
//! - Random values (flame offsets and phases, spark spots) use this port's
//!   own generator [H].
use crate::duel_return::{EffectLayer, EffectQuad};
use crate::duel_versus::{CardPose, VersusCard};

pub const UPDATES: u32 = 50;
pub const UPDATES_NO_BURN: u32 = 36;
/// The burn counter: the survivor starts back in this update.
pub const SURVIVOR_START: u32 = 23;
pub const CUT_STEP: f32 = 18.4;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (self.0 >> 16) & 0x7FFF
    }
}

/// Closed-form projection of a frame-local point (scale 1) at view x =
/// 128 sigma through the battle camera (C604-RETURN-EFFECTS.md; the A frame
/// matrix of the footage reduces to it). Returns (x, y, depth).
fn project(sigma: f32, l: [f32; 3]) -> Option<[f32; 3]> {
    let vx = 128.0 * sigma + (l[0] - l[1]);
    let vy = -(l[0] + l[1]);
    let vz = 800.0 + (l[1] - l[2]);
    if vz < 1.0 {
        return None;
    }
    let x = 256.0 + 800.0 * vx / vz;
    let y = 256.0 + 1074.2855 * vy / vz;
    let ok = |v: f32| (-1792.0..=2303.0).contains(&v);
    (ok(x) && ok(y)).then_some([x, y, vz])
}

fn quad(sigma: f32, points: [[f32; 2]; 4], uv: [[f32; 2]; 4]) -> Option<EffectQuad> {
    let mut screen = [[0.0; 2]; 4];
    for (i, p) in points.iter().enumerate() {
        let v = project(sigma, [p[0], p[1], 0.0])?;
        screen[i] = [v[0], v[1]];
    }
    Some(EffectQuad { screen, uv })
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Spark {
    x: f32,
    y: f32,
    alpha: u8,
    frame: u8,
    tick: i16,
}

/// One burning card.
#[derive(Clone, Debug, PartialEq)]
pub struct Burn {
    /// 0 = card A (left, view -128), 1 = card B (right).
    pub card: usize,
    pub cut: f32,
    flames: Vec<(f32, f32, u32)>,
    origin_y: f32,
    flame_alpha: u8,
    sparks: Vec<Spark>,
    spark_alpha: u8,
}

impl Burn {
    fn new(card: usize, rng: &mut Rng) -> Self {
        let flames = (0..16)
            .map(|i| {
                let y = -128.0 + ((rng.next() & 31) as f32 - 16.0);
                (-160.0 + 20.0 * i as f32, y, rng.next() % 10)
            })
            .collect();
        let sparks = (0..256)
            .map(|_| Spark {
                x: 0.1 * ((rng.next() % 2560) as f32 - 1280.0),
                y: -184.0 + 0.1 * (rng.next() % 920) as f32,
                alpha: 128,
                frame: 0,
                tick: -((rng.next() % 12) as i16),
            })
            .collect();
        Self {
            card,
            cut: 368.0,
            flames,
            origin_y: -128.0,
            flame_alpha: 64,
            sparks,
            spark_alpha: 128,
        }
    }

    fn sigma(&self) -> f32 {
        if self.card == 0 { -1.0 } else { 1.0 }
    }

    /// The burning card quad (corners and texel UVs on the 256 x 368 face),
    /// drawn Cs * As; `None` once burnt away.
    pub fn card_quad(&self) -> Option<EffectQuad> {
        if self.cut <= 0.0 {
            return None;
        }
        let bottom = 184.0 - self.cut;
        let v = (368.0 * self.cut / 368.0).trunc() - 1.0;
        quad(
            self.sigma(),
            [
                [-128.0, 184.0],
                [128.0, 184.0],
                [-128.0, bottom],
                [128.0, bottom],
            ],
            [[0.0, 0.0], [255.0, 0.0], [0.0, v], [255.0, v]],
        )
    }

    /// The flame row and the sparks drawn this update (TEXEFF record 1),
    /// with the update's flame motion and spark draw ticks applied as the
    /// original applies them before drawing.
    fn draw_effects(&mut self, k: u32, rng: &mut Rng) -> Vec<EffectLayer> {
        let sigma = self.sigma();
        // Flames: the row rises 16 while below 202, then fades by 8.
        let vel = if self.origin_y < 202.0 {
            16.0
        } else {
            self.flame_alpha = self.flame_alpha.saturating_sub(8);
            0.0
        };
        let (h, w) = (
            100.0 * std::f32::consts::FRAC_1_SQRT_2,
            0.75 * 100.0 * std::f32::consts::FRAC_1_SQRT_2,
        );
        let mut flames = Vec::new();
        for f in &mut self.flames {
            f.1 += vel;
            let frame = (f.2 + k / 2) % 10;
            let (u, v) = (24.0 * (frame / 2) as f32, 32.0 * (frame % 2) as f32);
            if let Some(q) = quad(
                sigma,
                [
                    [f.0 - w, f.1 + h],
                    [f.0 + w, f.1 + h],
                    [f.0 - w, f.1 - h],
                    [f.0 + w, f.1 - h],
                ],
                [[u, v], [u + 23.0, v], [u, v + 31.0], [u + 23.0, v + 31.0]],
            ) {
                flames.push(q);
            }
        }
        self.origin_y += vel;
        // Sparks: drawn when their tick is >= 0, as screen-aligned squares
        // of side 3840 / z.
        let mut by_alpha: std::collections::BTreeMap<u8, Vec<EffectQuad>> = Default::default();
        for s in &mut self.sparks {
            s.tick += 1;
            if s.tick > 0 && s.tick % 8 == 0 {
                s.frame += 1;
            }
            if s.tick >= 0 {
                if let Some(p) = project(sigma, [s.x, s.y, 0.0]) {
                    let half = 1920.0 / p[2];
                    let (u, v) = (32.0 * f32::from(s.frame % 4), 32.0 * f32::from(s.frame / 4));
                    let a = s.alpha.min(self.spark_alpha);
                    if a > 0 {
                        by_alpha.entry(a).or_default().push(EffectQuad {
                            screen: [
                                [p[0] - half, p[1] - half],
                                [p[0] + half, p[1] - half],
                                [p[0] - half, p[1] + half],
                                [p[0] + half, p[1] + half],
                            ],
                            uv: [[u, v], [u + 31.0, v], [u, v + 31.0], [u + 31.0, v + 31.0]],
                        });
                    }
                }
            }
        }
        // Move (after the draw).
        for (i, s) in self.sparks.iter_mut().enumerate() {
            if s.tick != 0 {
                s.y -= [4.0, 5.0, 6.0][i % 3];
                s.alpha = s.alpha.saturating_sub(8);
            } else {
                s.x = 0.1 * ((rng.next() % 2560) as f32 - 1280.0);
                s.y = (184.0 - self.cut) + 0.1 * (rng.next() % 920) as f32;
                s.alpha = 128;
                s.frame = 0;
            }
        }
        if self.cut <= 0.0 {
            self.spark_alpha = self.spark_alpha.saturating_sub(32);
        }
        let mut layers = vec![EffectLayer {
            name: "flames",
            colour: [128, 128, 128, self.flame_alpha],
            quads: flames,
        }];
        layers.extend(by_alpha.into_iter().map(|(a, quads)| EffectLayer {
            name: "sparks",
            colour: [128, 128, 128, a],
            quads,
        }));
        layers
    }
}

/// A surviving card on its way back.
#[derive(Clone, Debug, PartialEq)]
pub struct Survivor {
    pub card: VersusCard,
    /// 0 = A, 1 = B.
    pub index: usize,
    pub flying: bool,
    pub landed: bool,
}

/// c605's state.
#[derive(Clone, Debug, PartialEq)]
pub struct Return605 {
    pub mode: u8,
    pub k: u32,
    pub burns: Vec<Burn>,
    pub survivors: Vec<Survivor>,
    /// Effect layers drawn this update (flames and sparks).
    pub effects: Vec<(usize, Vec<EffectLayer>)>,
    rng: Rng,
    pub done: bool,
}

impl Return605 {
    /// `versus`: the two cards at rest (their poses are the versus poses);
    /// `board`: their board card poses (the return targets).
    pub fn new(mode: u8, versus: &[VersusCard; 2], board: [CardPose; 2], seed: u32) -> Self {
        let mut rng = Rng(seed);
        let destroyed = |i: usize| mode & (if i == 0 { 2 } else { 1 }) != 0;
        let burns = (0..2)
            .filter(|&i| destroyed(i))
            .map(|i| Burn::new(i, &mut rng))
            .collect();
        let survivors = (0..2)
            .filter(|&i| !destroyed(i))
            .map(|i| {
                let mut card = versus[i].clone();
                // The copy holds the versus pose (c605 init) and returns to
                // the board card pose (tilt 0, scale 0.1) in 20 steps.
                card.pose.spin = [0, 0x10000, 0x10000];
                card.target = CardPose {
                    tilt: [0, 0, 0],
                    scale: [0.1, 0.1],
                    ..board[i]
                };
                card.remaining = 20;
                Survivor {
                    card,
                    index: i,
                    flying: false,
                    landed: false,
                }
            })
            .collect();
        let mut s = Self {
            mode,
            k: 0,
            burns,
            survivors,
            effects: Vec::new(),
            rng,
            done: false,
        };
        s.update();
        s
    }

    pub fn length(mode: u8) -> u32 {
        if mode == 0 { UPDATES_NO_BURN } else { UPDATES }
    }

    /// The work of update `k` (draw-time state).
    fn update(&mut self) {
        let k = self.k;
        let mut effects = Vec::new();
        for b in &mut self.burns {
            effects.push((b.card, b.draw_effects(k, &mut self.rng)));
        }
        self.effects = effects;
        let start = if self.burns.is_empty() {
            0
        } else {
            SURVIVOR_START
        };
        for s in &mut self.survivors {
            if s.landed {
                continue;
            }
            if k >= start {
                s.flying = true;
                if !s.card.step() {
                    // Landing: the copy takes the board card's spin.
                    s.card.pose.spin = s.card.target.spin;
                    s.landed = true;
                }
            }
        }
    }

    /// Advance to the next update; then the burns' cut shrinks.
    pub fn tick(&mut self) {
        if self.done {
            return;
        }
        for b in &mut self.burns {
            b.cut = (b.cut - CUT_STEP).max(0.0);
        }
        self.k += 1;
        if self.k >= Self::length(self.mode) {
            self.done = true;
            return;
        }
        self.update();
    }

    /// Whether the board card of versus card `i` is shown again.
    pub fn board_card_shown(&self, i: usize) -> bool {
        self.survivors.iter().any(|s| s.index == i && s.landed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burn_corners_match_the_footage_frame() {
        // C605-RETURN.md (GS 1/16 px packets): the A frame maps local (-128, 184) to
        // (-101.75, 194.81) and (128, -184) to (494.94, 353.63).
        let a = project(-1.0, [-128.0, 184.0, 0.0]).unwrap();
        let b = project(-1.0, [128.0, -184.0, 0.0]).unwrap();
        assert!(
            (a[0] + 101.75).abs() < 0.07 && (a[1] - 194.81).abs() < 0.07,
            "{a:?}"
        );
        assert!(
            (b[0] - 494.94).abs() < 0.07 && (b[1] - 353.63).abs() < 0.07,
            "{b:?}"
        );
    }

    #[test]
    fn flame_quad_matches_the_footage_packet() {
        // k = 10 element 0 (x -160, y 36): corners (-139.06, 381.94),
        // (-45.5, 256.25), (-64.25, 603.75), (46.63, 454.88).
        let (h, w) = (
            100.0 * std::f32::consts::FRAC_1_SQRT_2,
            75.0 * std::f32::consts::FRAC_1_SQRT_2,
        );
        let q = quad(
            -1.0,
            [
                [-160.0 - w, 36.0 + h],
                [-160.0 + w, 36.0 + h],
                [-160.0 - w, 36.0 - h],
                [-160.0 + w, 36.0 - h],
            ],
            [[0.0; 2]; 4],
        )
        .unwrap();
        let want = [
            [-139.06, 381.94],
            [-45.5, 256.25],
            [-64.25, 603.75],
            [46.63, 454.88],
        ];
        for i in 0..4 {
            assert!(
                (q.screen[i][0] - want[i][0]).abs() < 0.1
                    && (q.screen[i][1] - want[i][1]).abs() < 0.1,
                "{:?}",
                q.screen
            );
        }
    }
}
