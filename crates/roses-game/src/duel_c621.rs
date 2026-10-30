//! CMODULE c621, the monster-destroyed effect ("big light"), as traced in
//! `claude-units/C621-DESTROY.md` [F/emu]: a fixed 180-update effect anchored
//! at the loser's animated position taken at launch, projected each update
//! through the live arena camera; everything additive (ALPHA 0x48), from
//! TEXEFF record 15 or untextured, base colour (112, 176, 224).
//!
//! Layers: A screen vignette, C1 ground glow disc (core alpha up to 255),
//! C2 camera-facing glow disc with a flickering colour, F eight scrolling
//! light tubes, D1 rising sparkles, D2 ring sparkles and B twinkles from
//! update 150; E (2 x 64 rays) is always sent with alpha 0 and is skipped.
//! Returns 2 at update 39 (the loser starts fading) and ends at 180.
//!
//! [H / not reproduced]: random values come from the port's generator
//! (the original's rand sequence includes other callers, so only the
//! distributions can match); B's per-element motion (only seen in output)
//! is a fixed scatter; the depth test of C1, D and F against the stage (the
//! port draws every layer without depth).
use crate::duel_arena::CmotCamera;
use std::f64::consts::PI;

pub const LENGTH: u32 = 180;
pub const RESULT_TWO_AT: u32 = 39;
const BASE: [u8; 3] = [112, 176, 224];

/// One triangle in buffer pixels (512 x 512), TEXEFF 15 texels when
/// `textured`, per-vertex GS colour (alpha 128 = 1.0, up to 255).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxTri {
    pub screen: [[f32; 2]; 3],
    pub uv: [[f32; 2]; 3],
    pub colour: [[u8; 4]; 3],
    pub textured: bool,
    /// TEXEFF record of a textured triangle (15, or 12 for C600's wings).
    pub sheet: u8,
}

#[derive(Clone, Debug, PartialEq)]
struct Tube {
    r: f32,
    a: u8,
    rows: u16,
    scroll: u32,
    offset: [f64; 2],
    colour: [u8; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Particle {
    p: [f32; 3],
    alpha: u8,
    frame: u8,
    tick: i32,
}

/// What one update drew, in world or screen terms (projected by [`C621::draw`]).
#[derive(Clone, Debug, PartialEq)]
enum Item {
    Vignette(u8),
    GroundDisc(u8),
    Billboard { alpha: u8, scale: f32, colour: [u8; 3] },
    Tube { base: [f64; 3], r: f32, rows: u16, scroll: u32, alpha: u8, colour: [u8; 3] },
    Sparkle { at: [f64; 3], size: f32, frame: u8, alpha: u8 },
    Twinkle { offset: [f32; 2], frame: u8, alpha: u8 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct C621 {
    /// Updates done (u of the last update).
    pub u: u32,
    anchor: [f64; 3],
    radius: f64,
    a_a: u8,
    a_c1: u8,
    a_c2: u8,
    a_d1: u8,
    a_d2: u8,
    a_b: u8,
    g: u8,
    c2_scale: f32,
    c2_rg: [u8; 2],
    tubes: Vec<Tube>,
    d1: Vec<Particle>,
    d2: Vec<Particle>,
    twinkles: Vec<[f32; 2]>,
    items: Vec<Item>,
    seed: u32,
}

/// 0x1032C0: approach `max` by `step`.
fn approach(a: u8, max: u8, step: u8) -> u8 {
    if u16::from(a) + u16::from(step) <= u16::from(max) {
        a + step
    } else {
        max
    }
}
/// 0x103470: decay by `step` to 0.
fn decay(a: u8, step: u8) -> u8 {
    a.saturating_sub(step)
}

impl C621 {
    /// The launch: `anchor` = the loser's obj+0x80 now, `size` = its card's
    /// unit size (0x2CEC28).
    pub fn new(anchor: [f64; 3], size: u16, seed: u32) -> Self {
        let mut c = Self {
            u: 0,
            anchor,
            radius: (0.6 * f64::from(size)).max(25.0),
            a_a: 0,
            a_c1: 0,
            a_c2: 0,
            a_d1: 0,
            a_d2: 128,
            a_b: 128,
            g: 96,
            c2_scale: 0.1,
            c2_rg: [112, 176],
            tubes: Vec::new(),
            d1: Vec::new(),
            d2: Vec::new(),
            twinkles: Vec::new(),
            items: Vec::new(),
            seed: seed | 1,
        };
        for _ in 0..16 {
            let t = [(c.rand() % 20) as f32 - 10.0, (c.rand() % 20) as f32 - 10.0];
            c.twinkles.push(t);
        }
        for i in 0..128u8 {
            let tick = -((c.rand() % 12) as i32);
            let a = f64::from(c.rand() & 0xFFFF) * 2.0 * PI / 65536.0;
            let rr = 0.1 * ((c.rand() % 40) as f64 - 20.0);
            let y = -0.1 * (c.rand() % 30) as f64;
            c.d1.push(Particle {
                p: [(rr * a.sin()) as f32, y as f32, (rr * a.cos()) as f32],
                alpha: 0,
                frame: i & 1,
                tick,
            });
        }
        for i in 0..256u16 {
            let a = f64::from(c.rand() & 0xFFFF) * 2.0 * PI / 65536.0;
            let y = (c.rand() % 20) as f32;
            let tick = -((c.rand() % 34) as i32);
            c.d2.push(Particle {
                p: [(25.0 * a.sin()) as f32, y - 50.0, (25.0 * a.cos()) as f32],
                alpha: 128,
                frame: (i & 1) as u8,
                tick,
            });
        }
        for _ in 0..8 {
            let offset = c.ground_offset();
            c.tubes.push(Tube { r: 0.0, a: 0x60, rows: 0, scroll: 0, offset, colour: BASE });
        }
        c
    }

    fn rand(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (self.seed >> 1) & 0x7FFF_FFFF
    }

    /// A tube's random place on the ground: angle, then distance 0.05 *
    /// (rand % int(10 R)).
    fn ground_offset(&mut self) -> [f64; 2] {
        let a = f64::from(self.rand() & 0xFFFF) * 2.0 * PI / 65536.0;
        let n = ((10.0 * self.radius) as u32).max(1);
        let d = 0.05 * f64::from(self.rand() % n);
        [d * a.sin(), d * a.cos()]
    }

    pub fn done(&self) -> bool {
        self.u >= LENGTH
    }

    /// One update (0x1E04B24's order).
    pub fn update(&mut self) {
        if self.done() {
            // Killed after update 180: nothing more is drawn.
            self.items.clear();
            return;
        }
        self.u += 1;
        let u = self.u;
        self.items.clear();
        // A: vignette.
        self.items.push(Item::Vignette(self.a_a));
        self.a_a = if u < 160 { approach(self.a_a, 128, 8) } else { decay(self.a_a, 8) };
        // D1: rising sparkles.
        if u >= 20 {
            let a_d1 = self.a_d1;
            for i in 0..self.d1.len() {
                let p = self.d1[i];
                if p.tick >= 0 {
                    self.items.push(Item::Sparkle {
                        at: [
                            self.anchor[0] + f64::from(p.p[0]),
                            self.anchor[1] + f64::from(p.p[1]),
                            self.anchor[2] + f64::from(p.p[2]),
                        ],
                        size: 1.6,
                        frame: p.frame,
                        alpha: p.alpha.min(a_d1),
                    });
                }
                let p = &mut self.d1[i];
                p.tick += 1;
                p.frame ^= 1;
            }
            self.a_d1 = if u < 160 { approach(self.a_d1, 128, 8) } else { decay(self.a_d1, 8) };
            for i in 0..self.d1.len() {
                if self.d1[i].tick < 0 {
                    continue;
                }
                let p = &mut self.d1[i];
                p.p[1] += 2.0;
                p.alpha = decay(p.alpha, 8);
                if p.p[1] < 20.0 {
                    p.p[0] *= 1.2;
                    p.p[2] *= 1.2;
                } else {
                    let a = f64::from(self.rand() & 0xFFFF) * 2.0 * PI / 65536.0;
                    let rr = 0.1 * ((self.rand() % 160) as f64 - 80.0);
                    let y = -0.1 * (self.rand() % 120) as f64;
                    self.d1[i] = Particle {
                        p: [(rr * a.sin()) as f32, y as f32, (rr * a.cos()) as f32],
                        alpha: 128,
                        ..self.d1[i]
                    };
                }
            }
        }
        // C1: ground disc.
        self.items.push(Item::GroundDisc(self.a_c1));
        self.a_c1 = if u < 150 { approach(self.a_c1, 255, 16) } else { decay(self.a_c1, 16) };
        // C2: billboard disc.
        if u >= 10 {
            if u % 2 == 0 {
                self.c2_rg = [(self.rand() % 112) as u8, (self.rand() % 176) as u8];
            }
            self.items.push(Item::Billboard {
                alpha: self.a_c2,
                scale: self.c2_scale,
                colour: [self.c2_rg[0], self.c2_rg[1], 224],
            });
            self.a_c2 = if u < 160 { approach(self.a_c2, 128, 8) } else { decay(self.a_c2, 8) };
            self.c2_scale = (self.c2_scale + 0.05).min(1.0);
        }
        // B twinkles and D2 ring sparkles.
        if u >= 150 {
            for (i, t) in self.twinkles.iter().enumerate() {
                self.items.push(Item::Twinkle { offset: *t, frame: ((u as usize + i) & 1) as u8, alpha: self.a_b });
            }
            self.a_b = decay(self.a_b, 8);
            let a_d2 = self.a_d2;
            for i in 0..self.d2.len() {
                let p = self.d2[i];
                if p.tick >= 0 {
                    self.items.push(Item::Sparkle {
                        at: [
                            self.anchor[0] + f64::from(p.p[0]),
                            50.0 + f64::from(p.p[1]),
                            self.anchor[2] + f64::from(p.p[2]),
                        ],
                        size: 0.4,
                        frame: p.frame,
                        alpha: p.alpha.min(a_d2),
                    });
                }
                let p = &mut self.d2[i];
                p.tick += 1;
                p.frame ^= 1;
            }
            if u >= 160 {
                self.a_d2 = decay(self.a_d2, 8);
            }
            for i in 0..self.d2.len() {
                if self.d2[i].tick < 0 {
                    continue;
                }
                let step = [1.0, 2.0, 3.0][i % 3];
                let p = &mut self.d2[i];
                p.p[1] += step;
                if p.p[1] > 0.0 {
                    p.alpha = decay(p.alpha, 8);
                }
                if p.p[1] > 50.0 {
                    let a = f64::from(self.rand() & 0xFFFF) * 2.0 * PI / 65536.0;
                    self.d2[i].p = [(25.0 * a.sin()) as f32, -50.0, (25.0 * a.cos()) as f32];
                    self.d2[i].alpha = 128;
                }
            }
        }
        // F: light tubes.
        for i in 0..8 {
            let colour = [
                48 + (self.rand() & 63) as u8,
                112 + (self.rand() & 63) as u8,
                160 + (self.rand() & 63) as u8,
            ];
            self.tubes[i].colour = colour;
            if u > 4 * i as u32 + 10 {
                let t = self.tubes[i].clone();
                self.items.push(Item::Tube {
                    base: [self.anchor[0] + t.offset[0], 0.0, self.anchor[2] + t.offset[1]],
                    r: t.r,
                    rows: t.rows.min(29),
                    scroll: t.scroll,
                    alpha: t.a.min(0x60),
                    colour,
                });
                self.tubes[i].scroll += 1;
                self.tubes[i].r += 0.1;
                if self.tubes[i].r > 2.0 {
                    if self.tubes[i].a < 10 {
                        let offset = self.ground_offset();
                        let t = &mut self.tubes[i];
                        t.r = 0.0;
                        t.a = self.g;
                        t.rows = 0;
                        t.offset = offset;
                    } else {
                        self.tubes[i].a -= 10;
                    }
                }
                self.tubes[i].rows += 3;
            }
        }
        if u >= 150 {
            self.g = decay(self.g, 8);
            for t in &mut self.tubes {
                t.a = t.a.min(self.g);
            }
        }
    }

    /// The last update's draws through the arena camera.
    pub fn draw(&self, camera: &CmotCamera) -> Vec<FxTri> {
        let Some(view) = crate::camera::look_at(camera.eye, camera.target) else {
            return Vec::new();
        };
        let clip = crate::camera::mul(&crate::duel_arena::projection(camera.distance), &view);
        let aspect = crate::duel_board::GS_PROJECTION[1][1] / 800.0;
        // Buffer pixels and view depth of a world point (0x204D80; points
        // behind the camera are rejected).
        let project = |p: [f64; 3]| -> Option<([f32; 2], f64)> {
            let c: [f64; 4] =
                std::array::from_fn(|r| clip[0][r] * p[0] + clip[1][r] * p[1] + clip[2][r] * p[2] + clip[3][r]);
            if c[3] <= 1.0 {
                return None;
            }
            Some(([(c[0] / c[3] - 1792.0) as f32, (c[1] / c[3] - 1792.0) as f32], c[3]))
        };
        let mut out = Vec::new();
        let col = |rgb: [u8; 3], a: u8| [rgb[0], rgb[1], rgb[2], a];
        let fan = |out: &mut Vec<FxTri>, centre: [f32; 2], rim: &[[f32; 2]], c: [u8; 4], r: [u8; 4]| {
            for j in 0..rim.len() {
                let (p0, p1) = (rim[j], rim[(j + 1) % rim.len()]);
                out.push(FxTri { screen: [centre, p0, p1], uv: [[0.0; 2]; 3], colour: [c, r, r], textured: false, sheet: 0 });
            }
        };
        let quad = |out: &mut Vec<FxTri>, s: [[f32; 2]; 4], uv: [[f32; 2]; 4], c: [[u8; 4]; 4]| {
            out.push(FxTri { screen: [s[0], s[1], s[2]], uv: [uv[0], uv[1], uv[2]], colour: [c[0], c[1], c[2]], textured: true, sheet: 15 });
            out.push(FxTri { screen: [s[1], s[3], s[2]], uv: [uv[1], uv[3], uv[2]], colour: [c[1], c[3], c[2]], textured: true, sheet: 15 });
        };
        let ground = [self.anchor[0], -1.0, self.anchor[2]];
        for item in &self.items {
            match item {
                Item::Vignette(a) if *a > 0 => {
                    let rim: Vec<[f32; 2]> = (0..30)
                        .map(|j| {
                            let t = f64::from(j) * 2.0 * PI / 30.0;
                            [(256.0 + 550.0 * t.cos()) as f32, (256.0 + 550.0 * t.sin()) as f32]
                        })
                        .collect();
                    fan(&mut out, [256.0, 256.0], &rim, col(BASE, 0), col(BASE, *a));
                }
                Item::GroundDisc(a) if *a > 0 => {
                    let Some((c, _)) = project(ground) else { continue };
                    let rim: Option<Vec<[f32; 2]>> = (0..36)
                        .map(|j| {
                            let t = f64::from(j) * 2.0 * PI / 36.0;
                            project([ground[0] + self.radius * t.cos(), -1.0, ground[2] + self.radius * t.sin()]).map(|p| p.0)
                        })
                        .collect();
                    if let Some(rim) = rim {
                        fan(&mut out, c, &rim, col(BASE, *a), col(BASE, 0));
                    }
                }
                Item::Billboard { alpha, scale, colour } if *alpha > 0 => {
                    let Some((c, w)) = project(ground) else { continue };
                    let rx = self.radius * f64::from(*scale) * camera.distance / w;
                    let ry = rx * aspect;
                    let rim: Vec<[f32; 2]> = (0..36)
                        .map(|j| {
                            let t = f64::from(j) * 2.0 * PI / 36.0;
                            [c[0] + (rx * t.cos()) as f32, c[1] - (ry * t.sin()) as f32]
                        })
                        .collect();
                    fan(&mut out, c, &rim, col(*colour, *alpha), col(*colour, 0));
                }
                Item::Tube { base, r, rows, scroll, alpha, colour } if *rows > 0 => {
                    let ring = |k: u16| -> Option<Vec<[f32; 2]>> {
                        (0..=8)
                            .map(|j| {
                                let t = f64::from(j) * 8192.0 * 2.0 * PI / 65536.0;
                                project([
                                    base[0] + f64::from(*r) * t.sin(),
                                    base[1] + 3.0 * f64::from(k),
                                    base[2] + f64::from(*r) * t.cos(),
                                ])
                                .map(|p| p.0)
                            })
                            .collect()
                    };
                    let rings: Vec<Option<Vec<[f32; 2]>>> = (0..=*rows).map(ring).collect();
                    let a_of = |k: u16| if k == 0 || k == *rows { 0 } else { *alpha };
                    let v_of = |k: u16| (128.0 + f64::from((32 - (*scroll % 32) + u32::from(k)) % 32) + 0.5) as f32 / 256.0;
                    for k in 0..*rows {
                        let (Some(lo), Some(hi)) = (&rings[usize::from(k)], &rings[usize::from(k) + 1]) else { continue };
                        let (c0, c1) = (col(*colour, a_of(k)), col(*colour, a_of(k + 1)));
                        let u_ = 128.5 / 256.0;
                        for j in 0..8 {
                            quad(
                                &mut out,
                                [lo[j], hi[j], lo[j + 1], hi[j + 1]],
                                [[u_, v_of(k)], [u_, v_of(k + 1)], [u_, v_of(k)], [u_, v_of(k + 1)]],
                                [c0, c1, c0, c1],
                            );
                        }
                    }
                }
                Item::Sparkle { at, size, frame, alpha } if *alpha > 0 => {
                    let Some((c, w)) = project(*at) else { continue };
                    let half = (640.0 * f64::from(*size) / w / 2.0) as f32;
                    let v0 = 218.0 + 10.0 * f32::from(*frame);
                    let cc = col(BASE, *alpha);
                    quad(
                        &mut out,
                        [[c[0] - half, c[1] - half], [c[0] - half, c[1] + half], [c[0] + half, c[1] - half], [c[0] + half, c[1] + half]],
                        [[0.0, v0 / 256.0], [0.0, (v0 + 10.0) / 256.0], [10.0 / 256.0, v0 / 256.0], [10.0 / 256.0, (v0 + 10.0) / 256.0]],
                        [cc; 4],
                    );
                }
                Item::Twinkle { offset, frame, alpha } if *alpha > 0 => {
                    let Some((c, w)) = project([self.anchor[0], self.anchor[1], self.anchor[2]]) else { continue };
                    let k = (camera.distance / w) as f32;
                    let centre = [c[0] + offset[0] * k, c[1] - offset[1] * k * aspect as f32];
                    let v0 = 228.0 + 10.0 * f32::from(*frame);
                    let cc = col(BASE, *alpha);
                    let (hx, hy) = (1.5, 2.0);
                    quad(
                        &mut out,
                        [
                            [centre[0] - hx, centre[1] - hy],
                            [centre[0] - hx, centre[1] + hy],
                            [centre[0] + hx, centre[1] - hy],
                            [centre[0] + hx, centre[1] + hy],
                        ],
                        [[0.0, v0 / 256.0], [0.0, (v0 + 9.0) / 256.0], [9.0 / 256.0, v0 / 256.0], [9.0 / 256.0, (v0 + 9.0) / 256.0]],
                        [cc; 4],
                    );
                }
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_follow_the_traced_alphas() {
        let mut c = C621::new([0.0, 4.263, 102.054], 40, 7);
        // Footage RAM after update 6 (070: c 7): aA 48, aC1 96, aC2 0.
        for _ in 0..6 {
            c.update();
        }
        assert_eq!((c.a_a, c.a_c1, c.a_c2), (48, 96, 0));
        // 072 (c 40): A 128, C1 255, C2 128, D1 128, C2 scale 1.0.
        for _ in 6..39 {
            c.update();
        }
        assert_eq!((c.a_a, c.a_c1, c.a_c2, c.a_d1, c.g), (128, 255, 128, 128, 96));
        assert!((c.c2_scale - 1.0).abs() < 1e-6);
        // 080 (c 170): A 48, C1 0, C2 48, D1 48, B 0, g 0.
        for _ in 39..169 {
            c.update();
        }
        assert_eq!((c.a_a, c.a_c1, c.a_c2, c.a_d1, c.a_b, c.g), (48, 0, 48, 48, 0, 0));
        for _ in 169..200 {
            c.update();
        }
        assert!(c.done() && c.u == LENGTH);
    }

    #[test]
    fn draws_every_layer_through_a_camera() {
        let mut c = C621::new([0.0, 4.263, 102.054], 40, 7);
        for _ in 0..60 {
            c.update();
        }
        let cam = CmotCamera { eye: [40.0, 30.0, 140.0], target: [0.0, 4.263, 102.054], roll: 0.0, distance: 800.0 };
        let tris = c.draw(&cam);
        let flat = tris.iter().filter(|t| !t.textured).count();
        let textured = tris.len() - flat;
        eprintln!("flat {flat} textured {textured}");
        // A 30 + C1 36 + C2 36.
        assert_eq!(flat, 102);
        assert!(textured > 100);
    }
}

