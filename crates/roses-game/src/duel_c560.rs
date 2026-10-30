//! CMODULE c560 (0x230), the moth evolution (event 0x3C, 0x26ABF0), as
//! specified in `claude-activate/C560-EVOLUTION.md` [F code; emu: the
//! reference model `tools/mf_c560_model.py` equals the interpreter's
//! primitives to 0.166 px]. Every layer is screen-space (nodes at view
//! (0, 0, 800), the screen centre), additive, drawn over the scene:
//! - GLOW: the 4-quad fan of half size 100 on TEXEFF 15 (176,32)-(208,64),
//!   (128,128,64,a), scale 0.1 -> 1.1 then +0.005 per update;
//! - RAYS A / B: 64 Gouraud lines each (sent twice), (128,128,64,a) to
//!   black, length 80, moving 20 per update toward the centre, re-seeded
//!   every 11 updates (B 7 updates out of phase);
//! - BANDS: a chain of up to 20 annulus bands (10 quads of 50 degrees every
//!   36) on TEXEFF 14 (0,128)-(32,256), RGB (128,128,224), flowing inward 4
//!   per update and turning -7 degrees, node scale 0.1 -> 6.0;
//! - FLASH: the full screen, (128,128,128, 255 - 16 (u - 76)) from u 76;
//! - the target's card and model spin from u 12 (yaw += v, v += 0x40 to
//!   0x1000) and shrink from u 52 (card 0.01, model 0.1 per update); 2 at
//!   u 75 (the handler swaps the units), done after u 90.
//!
//! [H / not reproduced]: the port's random values (its own LCG); the spin's
//! start speed (stale task memory in the original, 0 here); lines drawn as
//! 1-pixel quads.
use crate::duel_c621::FxTri;

pub const UPDATES: u32 = 90;
/// The update c560 returns 2 (the handler swaps the old and new units).
pub const SWAP_U: u32 = 75;

fn lcg(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
    ((*seed >> 32) & 0x7FFF_FFFF) as u32
}

fn rise(a: i32, max: i32, step: i32) -> i32 {
    if a <= max - step { a + step } else { max }
}

fn decay(a: i32, min: i32, step: i32) -> i32 {
    if a >= min + step { a - step } else { min }
}

/// 91 floats sinf(pi_f32 * i / 180) (0x1E02720 init 6).
fn degree_table() -> [f32; 91] {
    let pi = f32::from_bits(0x4049_0FDB);
    std::array::from_fn(|i| ((pi * i as f32 / 180.0) as f64).sin() as f32)
}

fn norm(d: i32) -> usize {
    let mut d = if !(-359..360).contains(&d) { d % 360 } else { d };
    if d < 0 {
        d += 360;
    }
    d as usize
}

/// 0x1E019D0 sine of integer degrees.
fn dsin(t: &[f32; 91], d: i32) -> f32 {
    let d = norm(d);
    match d {
        0..=89 => t[d],
        90..=179 => t[180 - d],
        180..=269 => -t[d - 180],
        _ => -t[360 - d],
    }
}

/// 0x1E019D0 cosine of integer degrees.
fn dcos(t: &[f32; 91], d: i32) -> f32 {
    let d = norm(d);
    match d {
        0..=89 => t[90 - d],
        90..=179 => -t[d - 90],
        180..=269 => -t[270 - d],
        _ => t[d - 270],
    }
}

fn s16(x: i32) -> i32 {
    x as i16 as i32
}

/// A band link (0x5C bytes of the pool D+0xCD4); a freed link keeps its
/// fields until reused.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Link {
    a: f32,
    b: f32,
    ra: i32,
    rb: i32,
    ang: i32,
    rgb: [u8; 3],
    a1: i32,
    a2: i32,
    prev: Option<usize>,
    next: Option<usize>,
    spawner: i32,
}

/// The band system's template (D+0xAD8).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Template {
    last: Option<usize>,
    r44: f32,
    r48: f32,
    base: f32,
    ang: i32,
    spawner: i32,
    /// B, G, R, A bytes.
    colour: [u8; 4],
}

#[derive(Clone, Debug, PartialEq)]
struct Rays {
    points: Vec<[f32; 3]>,
    alpha: i32,
    counter: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct C560 {
    seed: u64,
    table: [f32; 91],
    life: i32,
    age: i32,
    glow_scale: f32,
    glow_alpha: i32,
    band_scale: f32,
    flash_alpha: i32,
    rays: [Rays; 2],
    links: [Link; 20],
    active: [bool; 20],
    template: Template,
    /// The target's added yaw (0x10000 = 360 degrees), its speeds and the
    /// card's (x, y) and model's scales.
    pub card_yaw: i32,
    pub model_yaw: i32,
    card_speed: i32,
    model_speed: i32,
    pub card_scale: f32,
    pub model_scale: f32,
    /// The last update's primitives.
    drawn: Vec<FxTri>,
    pub u: u32,
}

fn ray_set(seed: &mut u64) -> Vec<[f32; 3]> {
    (0..64)
        .map(|_| {
            let x = (lcg(seed) % 480) as f32 - 240.0;
            let y = (lcg(seed) % 480) as f32 - 240.0;
            let z = (lcg(seed) % 240) as f32 + 240.0;
            [x, y, z]
        })
        .collect()
}

/// Screen point of a local point at node scale `s` (view (s x, -s y,
/// 800 - s z), the board's GS projection on the 512 buffer).
fn proj(l: [f32; 3], s: f32) -> [f32; 2] {
    let (vx, vy, vz) = (f64::from(s) * f64::from(l[0]), -f64::from(s) * f64::from(l[1]), 800.0 - f64::from(s) * f64::from(l[2]));
    [(256.0 + 800.0 * vx / vz) as f32, (256.0 + 1_074.285_522_460_937_5 * vy / vz) as f32]
}

fn quad(out: &mut Vec<FxTri>, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], c: [[u8; 4]; 4], textured: bool, sheet: u8) {
    out.push(FxTri { screen: [p[0], p[1], p[2]], uv: [uv[0], uv[1], uv[2]], colour: [c[0], c[1], c[2]], textured, sheet });
    out.push(FxTri { screen: [p[1], p[3], p[2]], uv: [uv[1], uv[3], uv[2]], colour: [c[1], c[3], c[2]], textured, sheet });
}

impl C560 {
    pub fn new(seed: u32) -> Self {
        let mut seed = u64::from(seed) | 1;
        let rays = [
            Rays { points: ray_set(&mut seed), alpha: 0, counter: 0 },
            Rays { points: ray_set(&mut seed), alpha: 0, counter: 7 },
        ];
        let mut c = Self {
            seed,
            table: degree_table(),
            life: 90,
            age: 0,
            glow_scale: 0.1,
            glow_alpha: 0,
            band_scale: 0.1,
            flash_alpha: 255,
            rays,
            links: [Link::default(); 20],
            active: [false; 20],
            template: Template::default(),
            card_yaw: 0,
            model_yaw: 0,
            card_speed: 0,
            model_speed: 0,
            card_scale: 0.1,
            model_scale: 1.0,
            drawn: Vec::new(),
            u: 0,
        };
        // Five band heads, the template colour still 0 (invisible).
        for i in 0..5 {
            c.template.r44 = 30.0;
            c.template.r48 = 30.0;
            c.template.spawner = i32::from(i == 4);
            c.template.base = 0.0;
            c.template.ang = 15 * i;
            c.add();
        }
        c.template.colour = [0xE0, 0x80, 0x80, 0x80];
        c
    }

    /// 0x1E01C50: the lowest inactive slot, filled from the template.
    fn add(&mut self) -> bool {
        let Some(k) = (0..20).find(|&k| !self.active[k]) else {
            return false;
        };
        let t = self.template;
        let [b, g, r, a] = t.colour;
        let link = Link {
            a: t.base.min(50.0),
            b: (t.base + 13.0).min(50.0),
            ra: t.r44 as i32,
            rb: t.r48 as i32,
            ang: t.ang,
            rgb: [r, g, b],
            a1: i32::from(a),
            a2: i32::from(a),
            prev: t.last,
            next: None,
            spawner: t.spawner,
        };
        self.links[k] = link;
        self.active[k] = true;
        if let Some(p) = link.prev {
            self.links[p].next = Some(k);
        }
        self.template.last = Some(k);
        true
    }

    /// 0x1E02520.
    fn band_update(&mut self, k: usize) {
        let life = self.life;
        let mut l = self.links[k];
        l.ang = s16(l.ang - 7);
        l.ra = l.a as i32;
        l.rb = l.b as i32;
        if l.a > 0.0 {
            l.a -= 4.0;
            if l.b - l.a >= 13.0 {
                l.b -= 4.0;
            }
        } else {
            l.b -= 4.0;
            if l.b < l.a {
                l.b = l.a;
                if let Some(n) = l.next {
                    self.links[n].prev = None;
                }
                self.active[k] = false;
            }
        }
        self.links[k] = l;
        if l.spawner > 0 && l.b < 50.0 {
            self.template.last = Some(k);
            self.template.r44 = l.ra as f32;
            self.template.r48 = l.rb as f32;
            self.template.spawner = 1;
            self.template.base = l.b;
            self.template.ang = s16(l.ang + 12);
            self.template.colour[3] = 0;
            if self.add() {
                self.links[k].spawner = 0;
            }
        }
        let l = &mut self.links[k];
        if life < 10 {
            if l.a1 >= 17 {
                l.a1 -= 15;
            }
            if l.a2 >= 17 {
                l.a2 -= 15;
            }
        } else if l.a1 < 80 {
            l.a1 += 6;
        }
        l.a2 = 0;
    }

    pub fn done(&self) -> bool {
        self.life <= 0
    }

    /// One update (u = 1 .. 90); returns the module's code (2 at u 75).
    pub fn update(&mut self) -> i32 {
        self.u += 1;
        let mut out = Vec::new();
        // Node matrices are built from the previous scales.
        let (glow_scale, band_scale) = (self.glow_scale, self.band_scale);
        if self.life >= 17 && self.age >= 11 {
            self.card_yaw += self.card_speed;
            self.card_speed = rise(self.card_speed, 0x1000, 0x40);
            self.model_yaw += self.model_speed;
            self.model_speed = rise(self.model_speed, 0x1000, 0x40);
            if self.age >= 51 {
                self.card_scale = if self.card_scale < 0.01 { 0.0 } else { self.card_scale - 0.01 };
                self.model_scale = if self.model_scale < 0.1 { 0.0 } else { self.model_scale - 0.1 };
            }
        }
        // GLOW.
        let g = [(-1.0, 0.0), (-1.0, 1.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (1.0, -1.0), (0.0, -1.0), (-1.0, -1.0)]
            .map(|(x, y): (f32, f32)| [100.0 * x, 100.0 * y, 0.0]);
        let centre = [0.0f32; 3];
        let colour = [128, 128, 64, self.glow_alpha.clamp(0, 255) as u8];
        let uv = [[176.0, 32.0], [208.0, 32.0], [176.0, 64.0], [208.0, 64.0]].map(|[x, y]: [f32; 2]| [x / 256.0, y / 256.0]);
        for q in [[g[1], g[2], g[0], centre], [g[3], g[2], g[4], centre], [g[5], g[6], g[4], centre], [g[7], g[6], g[0], centre]] {
            quad(&mut out, q.map(|v| proj(v, glow_scale)), uv, [colour; 4], true, 15);
        }
        if self.age < 30 {
            self.glow_alpha = rise(self.glow_alpha, 0x60, 4);
        }
        if self.glow_scale <= 1.0 {
            self.glow_scale += 0.2;
        } else {
            self.glow_scale += 0.005;
            self.glow_alpha = decay(self.glow_alpha, 0, 3);
        }
        // RAYS A, B.
        for s in 0..2 {
            let a = self.rays[s].alpha.clamp(0, 255) as u8;
            for i in 0..64 {
                let p = self.rays[s].points[i];
                let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
                let n = [p[0] / len, p[1] / len, p[2] / len];
                let q = [p[0] + n[0] * 80.0, p[1] + n[1] * 80.0, p[2] + n[2] * 80.0];
                let (sa, sb) = (proj(p, 1.0), proj(q, 1.0));
                let (dx, dy) = (sb[0] - sa[0], sb[1] - sa[1]);
                let l = (dx * dx + dy * dy).sqrt().max(1e-3);
                let (nx, ny) = (-dy / l * 0.5, dx / l * 0.5);
                let corners = [[sa[0] + nx, sa[1] + ny], [sa[0] - nx, sa[1] - ny], [sb[0] + nx, sb[1] + ny], [sb[0] - nx, sb[1] - ny]];
                let (ci, co) = ([128, 128, 64, a], [0, 0, 0, a]);
                for _ in 0..2 {
                    quad(&mut out, corners, [[0.0; 2]; 4], [ci, ci, co, co], false, 0);
                }
                let mut p = [p[0] - n[0] * 20.0, p[1] - n[1] * 20.0, p[2] - n[2] * 20.0];
                if p[2] >= 300.0 {
                    let x = 0.1 * ((lcg(&mut self.seed) % 480) as f32 - 240.0);
                    let y = 0.1 * ((lcg(&mut self.seed) % 480) as f32 - 240.0);
                    let z = 0.1 * (lcg(&mut self.seed) % 240) as f32;
                    p = [x, y, z];
                }
                self.rays[s].points[i] = p;
            }
        }
        for s in 0..2 {
            let life = self.life;
            let r = &mut self.rays[s];
            if life >= 17 {
                r.alpha = if r.counter < 5 { rise(r.alpha, 255, 64) } else { decay(r.alpha, 0, 32) };
            }
            if r.counter >= 11 {
                r.counter = 0;
                r.points = ray_set(&mut self.seed);
                r.alpha = 0;
            }
        }
        for r in &mut self.rays {
            r.counter += 1;
        }
        // BANDS: updates in slot order, then draws in slot order.
        for k in 0..20 {
            if self.active[k] {
                self.band_update(k);
            }
        }
        let t = &self.table;
        let uv = [[0.0, 128.0], [32.0, 128.0], [0.0, 256.0], [32.0, 256.0]].map(|[x, y]: [f32; 2]| [x / 256.0, y / 256.0]);
        for k in 0..20 {
            if !self.active[k] {
                continue;
            }
            let l = self.links[k];
            let prev = l.prev.map(|p| self.links[p]);
            for j in 0..10 {
                let s2 = 36 * j;
                let (v0, v1, a0) = match prev {
                    Some(p) => {
                        let a = s2 + p.ang;
                        let r0 = p.rb as f32;
                        self.links[k].a = p.b;
                        ([r0 * dsin(t, a), r0 * dcos(t, a), 0.0], [r0 * dsin(t, a + 50), r0 * dcos(t, a + 50), 0.0], p.a1)
                    }
                    None => {
                        let a = s2 + l.ang;
                        let r0 = l.ra as f32;
                        let v = [r0 * dsin(t, a), r0 * dcos(t, a), 0.0];
                        (v, v, l.a2)
                    }
                };
                let a = s2 + l.ang;
                let r1 = l.rb as f32;
                let v2 = [r1 * dsin(t, a), r1 * dcos(t, a), 0.0];
                let v3 = [r1 * dsin(t, a + 50), r1 * dcos(t, a + 50), 0.0];
                let [r, g, b] = l.rgb;
                let (ca, cb) = ([r, g, b, a0.clamp(0, 255) as u8], [r, g, b, l.a1.clamp(0, 255) as u8]);
                quad(&mut out, [v0, v1, v2, v3].map(|v| proj(v, band_scale)), uv, [ca, ca, cb, cb], true, 14);
            }
        }
        self.band_scale = if self.band_scale <= 6.0 - 0.1 { self.band_scale + 0.1 } else { 6.0 };
        if self.life == 16 {
            self.life = 15;
            self.drawn = out;
            return 2;
        }
        if self.life < 16 {
            let c = [128, 128, 128, self.flash_alpha.clamp(0, 255) as u8];
            quad(&mut out, [[0.0, 0.0], [512.0, 0.0], [0.0, 512.0], [512.0, 512.0]], [[0.0; 2]; 4], [c; 4], false, 0);
            self.flash_alpha = decay(self.flash_alpha, 0, 16);
        }
        self.age += 1;
        self.life -= 1;
        self.drawn = out;
        if self.life <= 0 { 8 } else { 0 }
    }

    /// The last update's triangles (screen space, additive).
    pub fn draw(&self) -> &[FxTri] {
        &self.drawn
    }

    /// The card's size factor (its (x, y) scale over the rest 0.1).
    pub fn card_size(&self) -> f32 {
        self.card_scale / 0.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_returns_and_spin_follow_the_spec() {
        let mut c = C560::new(1);
        let mut codes = Vec::new();
        let mut spin = Vec::new();
        while !c.done() {
            codes.push(c.update());
            spin.push((c.u, c.card_yaw, c.card_size(), c.model_scale));
        }
        assert_eq!(codes.len(), 90);
        assert_eq!((codes[74], codes[89]), (2, 8));
        assert!(codes.iter().enumerate().all(|(i, &r)| r == 0 || i == 74 || i == 89));
        // Spin from u 12: 0, 64, 128 ..; 124,992 units by u 74.
        assert_eq!((spin[10].1, spin[11].1, spin[12].1, spin[13].1), (0, 0, 64, 192));
        assert_eq!(spin[73].1, 124_992);
        // Shrink u 52 .. 61: the card and model are gone at u 61.
        assert!((spin[51].2 - 0.9).abs() < 1e-4 && spin[50].2 == 1.0);
        assert!(spin[60].2 < 1e-3 && spin[60].3 < 1e-4);
    }

    #[test]
    fn layers_counts_and_flash() {
        let mut c = C560::new(7);
        c.update();
        // u 1: 4 glow quads, 2 x 64 rays twice, 100 band quads (2 tris each).
        assert_eq!(c.draw().len(), 2 * (4 + 2 * 64 * 2 + 100));
        for _ in 1..76 {
            c.update();
        }
        // u 76: the flash at 255 is the last quad.
        let last = c.draw().last().unwrap();
        assert_eq!(last.colour[0], [128, 128, 128, 255]);
        assert!((degree_table()[30] - 0.5).abs() < 1e-6);
    }
}
