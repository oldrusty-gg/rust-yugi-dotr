//! CMODULE c508 (0x1FC) "LP recovery", as traced in
//! `claude-activate/EFFECT-MODULES-2.md` 2 [F code; emu: the model matched
//! the interpreter (+600 and -1234) to 0.0625 px]. Screen anchored (nodes
//! at view (0, 0) on the depth-800 plane), Z max, over the board:
//! - SPARKLES: 64 twinkling TEXEFF 8 stars falling and spiralling out
//!   (node scale 15), global alpha 128, -16 from u 42;
//! - VIGNETTE: a 30-triangle white fan, alpha 0 at the centre and
//!   min(255, 16 (u - 1)) at radius 550 (255 - 20 (u - 73) from u 73);
//! - from u 42: the amount ("+600", the digits rolling in), sparks x 1.17,
//!   rings A / B, glow and rays in green (96, 176, 112) (red (176, 80, 96)
//!   for a loss). 85 updates; returns 2 at u 45.
//!
//! [H / not reproduced]: the port's random values; the number is drawn
//! additively where the original writes it opaquely (0x88) to u 72.
use crate::duel_c600::{decay, project_at, rotate};
use crate::duel_c621::FxTri;

pub const UPDATES: u32 = 85;

fn lcg(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
    ((*seed >> 32) & 0x7FFF_FFFF) as u32
}

#[derive(Clone, Debug, PartialEq)]
struct Sparkle {
    p: [f32; 3],
    alpha: u8,
    frame: u32,
}

/// Rim offsets of the vignette at 12-degree steps, j = 0..15 (x) and
/// 0..7 (y), as the game's float table truncates them [emu].
const RIM_X: [i32; 16] = [550, 537, 502, 444, 368, 274, 169, 57, -57, -169, -274, -368, -444, -502, -537, -550];
const RIM_Y: [i32; 8] = [0, 114, 223, 323, 408, 476, 523, 546];

fn rim(j: usize) -> [f32; 2] {
    let j = j % 30;
    let (x, y) = if j <= 15 {
        (RIM_X[j], RIM_Y[j.min(15 - j)])
    } else {
        let k = 30 - j;
        (RIM_X[k], -RIM_Y[k.min(15 - k)])
    };
    [256.0 + x as f32, 256.0 + y as f32]
}

#[derive(Clone, Debug, PartialEq)]
pub struct C508 {
    seed: u64,
    amount: i32,
    u: u32,
    sparkles: Vec<Sparkle>,
    sparkle_alpha: u8,
    rays: Vec<[i32; 3]>,
    sparks: Vec<[f32; 3]>,
    s: f32,
    rings: u8,
    glow: u8,
    rays_alpha: u8,
    /// The draws of the last update.
    pub tris: Vec<FxTri>,
}

impl C508 {
    pub fn new(amount: i32, seed: u32) -> Self {
        let mut seed = u64::from(seed) | 1;
        let sparkles = (0..64)
            .map(|_| {
                let frame = lcg(&mut seed) % 6;
                let a = f64::from(lcg(&mut seed) & 0xFFFF) * std::f64::consts::TAU / 65536.0;
                let rr = 0.1 * ((lcg(&mut seed) % 120) as f32 - 60.0);
                let y = 0.1 * ((lcg(&mut seed) % 400) as f32 - 200.0);
                Sparkle { p: [rr * a.sin() as f32, y, rr * a.cos() as f32], alpha: 0, frame }
            })
            .collect();
        let rays = (0..64).map(|_| std::array::from_fn(|_| (lcg(&mut seed) & 0xFFFF) as i32)).collect();
        let sparks = (0..60)
            .map(|_| {
                let x = (lcg(&mut seed) % 100) as f32 - 50.0;
                let z = (lcg(&mut seed) % 50) as f32;
                let y = (lcg(&mut seed) % 100) as f32 - 50.0;
                [x, y, z]
            })
            .collect();
        Self { seed, amount, u: 0, sparkles, sparkle_alpha: 128, rays, sparks, s: 0.1, rings: 32, glow: 96, rays_alpha: 32, tris: Vec::new() }
    }

    fn rgb(&self) -> [u8; 3] {
        if self.amount >= 0 { [96, 176, 112] } else { [176, 80, 96] }
    }

    fn quad(&mut self, s: f32, p: [[f32; 3]; 4], uv: [[f32; 2]; 4], c: [u8; 4], sheet: u8, size: f32) {
        let mut sc = [[0.0; 2]; 4];
        for i in 0..4 {
            match project_at([0.0, 0.0], s, p[i]) {
                Some(v) => sc[i] = v,
                None => return,
            }
        }
        self.sprite(sc, uv, c, sheet, size);
    }

    fn sprite(&mut self, sc: [[f32; 2]; 4], uv: [[f32; 2]; 4], c: [u8; 4], sheet: u8, size: f32) {
        let uv = uv.map(|[u, v]| [u / size, v / size]);
        self.tris.push(FxTri { screen: [sc[0], sc[1], sc[2]], uv: [uv[0], uv[1], uv[2]], colour: [c; 3], textured: true, sheet });
        self.tris.push(FxTri { screen: [sc[1], sc[3], sc[2]], uv: [uv[1], uv[3], uv[2]], colour: [c; 3], textured: true, sheet });
    }

    /// Module update `u` (1..=85): draw, then step.
    pub fn update(&mut self, u: u32) {
        self.u = u;
        self.tris.clear();
        // SPARKLES (Ry(0x200) per update, node scale 15).
        let (st, ct) = (2.8125f32.to_radians().sin(), 2.8125f32.to_radians().cos());
        for i in 0..self.sparkles.len() {
            let mut k = self.sparkles[i].clone();
            k.p = [k.p[0] * ct + k.p[2] * st, k.p[1], k.p[2] * ct - k.p[0] * st];
            let (vx, vy, vz) = (15.0 * k.p[0], -15.0 * k.p[1], 800.0 - 15.0 * k.p[2]);
            if vz > 0.0 {
                let (x, y) = (256.0 + 800.0 * vx / vz, 256.0 + 1074.2855 * vy / vz);
                if (-256.0..768.0).contains(&x) && (-256.0..768.0).contains(&y) {
                    let h = 6400.0 / vz;
                    let sc = [[x - h, y - h], [x + h, y - h], [x - h, y + h], [x + h, y + h]].map(|[a, b]| [a.trunc(), b.trunc()]);
                    let f = 60.0 + 10.0 * k.frame as f32;
                    let a = k.alpha.min(self.sparkle_alpha);
                    self.sprite(sc, [[0.0, f], [10.0, f], [0.0, f + 10.0], [10.0, f + 10.0]], [128, 128, 128, a], 8, 128.0);
                }
            }
            k.frame = (k.frame + 1) % 6;
            // Move.
            k.p[1] -= 1.0;
            if k.alpha != 0 {
                k.alpha = if k.p[1] < 0.0 { decay(k.alpha, 8) } else { (u32::from(k.alpha) + 8).min(128) as u8 };
            }
            if k.p[1] > -20.0 {
                k.p[0] *= 1.05;
                k.p[2] *= 1.05;
            } else {
                let a = f64::from(lcg(&mut self.seed) & 0xFFFF) * std::f64::consts::TAU / 65536.0;
                let rr = 0.1 * ((lcg(&mut self.seed) % 120) as f32 - 60.0);
                k.p = [rr * a.sin() as f32, 20.0 - 0.1 * ((lcg(&mut self.seed) % 60) as f32 - 30.0), rr * a.cos() as f32];
                k.alpha = 1;
                k.frame = lcg(&mut self.seed) % 6;
            }
            self.sparkles[i] = k;
        }
        if u >= 41 {
            self.sparkle_alpha = decay(self.sparkle_alpha, 16);
        }
        // VIGNETTE.
        let va = if u <= 73 { (16 * (u - 1)).min(255) } else { 255u32.saturating_sub(20 * (u - 73)) } as u8;
        for j in 0..30 {
            let (p0, p1) = (rim(j), rim(j + 1));
            let (c0, c1) = ([255, 255, 255, 0], [255, 255, 255, va]);
            self.tris.push(FxTri { screen: [[256.0, 256.0], p0, p1], uv: [[0.0; 2]; 3], colour: [c0, c1, c1], textured: false, sheet: 0 });
        }
        if u >= 42 {
            let rgb = self.rgb();
            // NUMBER: sign and digits, the digits rolling in.
            let text = format!("{}{}", if self.amount >= 0 { '+' } else { '-' }, self.amount.unsigned_abs());
            let n = text.len() as f32;
            let na = if u <= 72 { 128 } else { 128u32.saturating_sub(10 * (u - 72)) } as u8;
            let k = u - 42;
            for (i, ch) in text.chars().enumerate() {
                let v_final = match ch {
                    '+' => 184,
                    '-' => 168,
                    d => 8 + 16 * d.to_digit(10).unwrap_or(0),
                };
                let v = if k < 4 * (i as u32 + 1) { (v_final + 8 * k) % 192 } else { v_final } as f32;
                let x = 256.0 - 15.0 * n + 30.0 * i as f32;
                self.sprite(
                    [[x, 232.0], [x + 30.0, 232.0], [x, 280.0], [x + 30.0, 280.0]],
                    [[0.0, v], [10.0, v], [0.0, v + 16.0], [10.0, v + 16.0]],
                    [rgb[0], rgb[1], rgb[2], na],
                    15,
                    256.0,
                );
            }
            let s = self.s;
            // SPARKS (x 1.17 per update).
            for i in 0..self.sparks.len() {
                self.sparks[i] = self.sparks[i].map(|v| v * 1.17);
                let [x, y, z] = self.sparks[i];
                self.quad(
                    s,
                    [[x - 2.5, y + 2.5, z], [x + 2.5, y + 2.5, z], [x - 2.5, y - 2.5, z], [x + 2.5, y - 2.5, z]],
                    [[176.0, 64.0], [207.0, 64.0], [176.0, 95.0], [207.0, 95.0]],
                    [rgb[0], rgb[1], rgb[2], 128],
                    15,
                    256.0,
                );
            }
            // RING A / B.
            for outer in [115.0f32, 130.0] {
                let ring = |r: f32, j: u32| {
                    let a = 0.209_439_5f32 * j as f32;
                    [r * a.cos(), r * a.sin(), 0.0]
                };
                for j in 0..30 {
                    let a = self.rings;
                    self.quad(s, [ring(120.0, j), ring(outer, j), ring(120.0, j + 1), ring(outer, j + 1)], [[142.0, 64.0], [142.0, 0.0], [174.0, 64.0], [174.0, 0.0]], [rgb[0], rgb[1], rgb[2], a], 15, 256.0);
                }
            }
            // GLOW.
            let p = |x: f32, y: f32| [100.0 * x, 100.0 * y, 0.0];
            let pts = [p(-1.0, 0.0), p(-1.0, 1.0), p(0.0, 1.0), p(1.0, 1.0), p(1.0, 0.0), p(1.0, -1.0), p(0.0, -1.0), p(-1.0, -1.0)];
            let c = [0.0, 0.0, 0.0];
            for q in [[pts[1], pts[2], pts[0], c], [pts[3], pts[2], pts[4], c], [pts[5], pts[6], pts[4], c], [pts[7], pts[6], pts[0], c]] {
                let a = self.glow;
                self.quad(s, q, [[176.0, 32.0], [208.0, 32.0], [176.0, 64.0], [208.0, 64.0]], [rgb[0], rgb[1], rgb[2], a], 15, 256.0);
            }
            // RAYS.
            for i in 0..64 {
                let r = self.rays[i];
                let t = [[-15.0, 300.0, 0.0], [15.0, 300.0, 0.0], [-7.5, 0.0, 0.0], [7.5, 0.0, 0.0]].map(|v| rotate(v, r));
                let a = self.rays_alpha;
                self.quad(s, t, [[142.0, 0.0], [175.0, 0.0], [142.0, 63.0], [175.0, 63.0]], [rgb[0], rgb[1], rgb[2], a], 15, 256.0);
            }
            for r in &mut self.rays {
                *r = r.map(|v| v + 0x80);
            }
            if self.s <= 1.0 {
                self.s += 0.2;
            } else {
                self.s += 0.005;
                self.rays_alpha = decay(self.rays_alpha, 2);
                self.rings = decay(self.rings, 2);
                self.glow = decay(self.glow, 4);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rim_table_and_phase_two_curves() {
        assert_eq!(rim(5), [256.0 + 274.0, 256.0 + 476.0]);
        assert_eq!(rim(15), [256.0 - 550.0, 256.0]);
        assert_eq!(rim(25), [256.0 + 274.0, 256.0 - 476.0]);
        assert_eq!(rim(30), rim(0));
        let mut c = C508::new(600, 3);
        let mut log = Vec::new();
        for u in 1..=85 {
            log.push((c.rings, c.glow, c.sparkle_alpha));
            c.update(u);
        }
        let at = |u: usize| log[u - 1];
        // Rings 32 to u 47, 0 at u 63; glow 96 - 4(u - 47); sparkles -16 from u 42.
        assert_eq!((at(47).0, at(48).0, at(63).0), (32, 30, 0));
        assert_eq!((at(48).1, at(71).1), (92, 0));
        assert_eq!((at(41).2, at(42).2, at(49).2), (128, 112, 0));
    }
}
