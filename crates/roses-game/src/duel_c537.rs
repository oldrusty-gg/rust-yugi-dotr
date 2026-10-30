//! CMODULE c537 (0x219), Riryoku's equip-pair module (summon chain, handler
//! 0x272560), as specified in `claude-activate/NEWLY-ALLOWED.md` section 14
//! [F code; emu: the reference model `tools/mf_c537_model.py` equals the
//! interpreter's primitives (one GS step) in five runs]. From u 16, on the
//! screen-anchored node at view (-101, 8) on the depth-800 plane, Z max, OT
//! key 0 (over the HUD, `TOP_LAYER`), in draw order:
//! - the NUMBER ("+N" green (96, 176, 112) / "-N" red (176, 80, 96)):
//!   sign + 1-4 digit glyphs of TEXEFF 15 (10 x 16 texels at v = 8 + 16 g),
//!   30 x 48 px screen rectangles centred on x 155, y 240..288, never
//!   projected, no rolling; alpha 128, -4/U from u 22; ALPHA 0x88 (opaque
//!   modulate) at u 16..21, 0x44 (alpha blend) from u 22;
//! - c536's library in green (128, 224, 128): RING A 120 / 115 (a 32), RING
//!   B 120 / 130 (a 48), GLOW half 100 (a 96), 64 RAYS (x 15, a 32), node
//!   scale 0.1 .. 1.1 then +0.005, decays (rings 2, rays 2, glow 3) once
//!   past 1; additive.
//!
//! Returns 2 at u 22 (the handler applies +N ATK / DEF to the monster, -N to
//! the opponent's LP and sends the Riryoku card to the graveyard at u 23),
//! 8 at u 43. No SE of its own (the handler: 0x4C at its counter 1, 0x4D at
//! 15, 0x3B at 16). 192 random values at init (the rays), none after. The
//! card copies' choreography is the equip pair's (`duel_equip::b_alpha`).
//!
//! [H / not reproduced]: the port's random values (the caller's stream, same
//! count); host trig for the rays and rings (the original's library uses the
//! same formulas; matched to one GS step); the handler's d0 -> u mapping of
//! its sounds.
use crate::duel_c621::FxTri;
use crate::duel_fxlib::{decay, quad, rotm64, uvn, view_proj, SHEET_256};

/// Module life: returns 8 at u 43.
pub const UPDATES: u32 = 43;
/// The update returning 2.
pub const RET2_U: u32 = 22;
/// The update the handler applies the stats, the LP and the graveyard
/// (ret 2 seen).
pub const APPLY_U: u32 = 23;
/// The first drawing update.
pub const START_U: u32 = 16;
/// Drawn over everything (Z 0xFFFFFF, key 0), over the HUD.
pub const TOP_LAYER: bool = true;
/// The handler's sounds (bank 1) by its counter d0: (d0, id) [F; d0 -> u
/// as EQUIP-MODULES' mapping, H].
pub const HANDLER_SE: [(u32, u16); 3] = [(1, 0x4C), (15, 0x4D), (16, 0x3B)];
/// Random values drawn at init.
pub const INIT_RAND: u32 = 192;
/// The message box shown with it: 0x10D "Force!" at y -0xA0.
pub const MESSAGE: u16 = 0x10D;
const ANCHOR: [f64; 2] = [-101.0, 8.0];
const GREEN: [u8; 3] = [128, 224, 128];
const RAY: [[f64; 3]; 4] = [[-15.0, 300.0, 0.0], [15.0, 300.0, 0.0], [-7.5, 0.0, 0.0], [7.5, 0.0, 0.0]];

#[derive(Clone, Debug, PartialEq)]
pub struct C537 {
    amount: i32,
    glyphs: Vec<i32>,
    rgb: [u8; 3],
    number_a: u8,
    opaque_number: bool,
    s: f32,
    ring: [u8; 2],
    glow: u8,
    ray_a: u8,
    rays: [[i32; 3]; 64],
    u: u32,
    done: bool,
    rand_calls: u32,
    drawn: Option<(u8, bool, f32)>,
    /// Library triangles of the last update (rings, glow, rays), additive.
    pub additive: Vec<FxTri>,
    /// Kept empty (the convention of the other modules).
    pub subtractive: Vec<FxTri>,
    /// The number at u 16..21 (ALPHA 0x88: colour = texel x vertex / 128,
    /// alpha cut).
    pub opaque: Vec<FxTri>,
    /// The number from u 22 (ALPHA 0x44: (Cs - Cd) As + Cd, As = a / 128).
    pub alpha_blend: Vec<FxTri>,
}

impl C537 {
    /// `param` = (amount << 16) | (sign << 8) | mode (0 or 3, alike): amount
    /// = param >> 16, negated if param & 0x100. `rng` = the caller's stream
    /// (192 draws: 64 rays x 3 angles, & 0xFFFF).
    pub fn new(param: u32, rng: &mut impl FnMut() -> u32) -> Self {
        let mut n = 0u32;
        let mut r = || {
            n += 1;
            rng()
        };
        let rays = std::array::from_fn(|_| std::array::from_fn(|_| (r() & 0xFFFF) as i32));
        let mut amount = (param >> 16) as i32;
        if param & 0x100 != 0 {
            amount = -amount;
        }
        let a = amount.abs();
        let digits = if a >= 1000 {
            vec![a / 1000, a % 1000 / 100, a % 100 / 10, a % 10]
        } else if a >= 100 {
            vec![a / 100, a % 100 / 10, a % 10]
        } else if a >= 10 {
            vec![a / 10, a % 10]
        } else {
            vec![a]
        };
        let mut glyphs = vec![if amount < 0 { 0xA } else { 0xB }];
        glyphs.extend(digits);
        Self {
            amount,
            glyphs,
            rgb: if amount < 0 { [176, 80, 96] } else { [96, 176, 112] },
            number_a: 128,
            opaque_number: true,
            s: 0.1,
            ring: [32, 48],
            glow: 96,
            ray_a: 32,
            rays,
            u: 0,
            done: false,
            rand_calls: n,
            drawn: None,
            additive: Vec::new(),
            subtractive: Vec::new(),
            opaque: Vec::new(),
            alpha_blend: Vec::new(),
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    pub fn rand_calls(&self) -> u32 {
        self.rand_calls
    }

    /// The signed amount shown.
    pub fn amount(&self) -> i32 {
        self.amount
    }

    /// Glyph indices: the sign (0xA '-', 0xB '+') then the digits.
    pub fn glyphs(&self) -> &[i32] {
        &self.glyphs
    }

    /// The number's RGB.
    pub fn rgb(&self) -> [u8; 3] {
        self.rgb
    }

    /// The last update's number: (alpha, opaque ALPHA 0x88, node scale);
    /// None before u 16.
    pub fn drawn(&self) -> Option<(u8, bool, f32)> {
        self.drawn
    }

    fn lib_quad(&mut self, l: [[f64; 3]; 4], uv: [[f32; 2]; 4], a: u8) {
        let s = f64::from(self.s);
        let mut p = [[0.0f32; 2]; 4];
        for i in 0..4 {
            match view_proj(ANCHOR, s, l[i]) {
                Some(v) => p[i] = v,
                None => return,
            }
        }
        quad(&mut self.additive, p, uv, [[GREEN[0], GREEN[1], GREEN[2], a]; 4], true, 15);
    }

    /// Module update `u` (1, 2, ..). Returns 0, 2 at u 22, 8 at u 43 (and
    /// every later call).
    pub fn update(&mut self, _u: u32) -> u32 {
        self.additive.clear();
        self.opaque.clear();
        self.alpha_blend.clear();
        self.drawn = None;
        if self.done {
            return 8;
        }
        self.u += 1;
        let u = self.u;
        if u >= START_U {
            // NUMBER (0x1E01FA0), screen pixels, first.
            self.drawn = Some((self.number_a, self.opaque_number, self.s));
            let n = self.glyphs.len() as i32;
            let x0 = 256 + ANCHOR[0] as i32 - 15 * n;
            let c = [self.rgb[0], self.rgb[1], self.rgb[2], self.number_a];
            for (i, &g) in self.glyphs.iter().enumerate() {
                let x = (x0 + 30 * i as i32) as f32;
                let v0 = 8 + 16 * g;
                let uv = uvn([[0, v0], [10, v0], [0, v0 + 16], [10, v0 + 16]], SHEET_256);
                let out = if self.opaque_number { &mut self.opaque } else { &mut self.alpha_blend };
                quad(out, [[x, 240.0], [x + 30.0, 240.0], [x, 288.0], [x + 30.0, 288.0]], uv, [c; 4], true, 15);
            }
            // RING A, RING B (inner 120).
            let ring = |r: f64, k: u32| {
                let a = f64::from(0.209_439_5f32) * f64::from(k);
                [r * a.cos(), r * a.sin(), 0.0]
            };
            let uv = uvn([[142, 64], [142, 0], [174, 64], [174, 0]], SHEET_256);
            for (outer, a) in [(115.0, self.ring[0]), (130.0, self.ring[1])] {
                for k in 0..30 {
                    self.lib_quad([ring(120.0, k), ring(outer, k), ring(120.0, k + 1), ring(outer, k + 1)], uv, a);
                }
            }
            // GLOW (type-1 shape, half 100).
            let p: [[f64; 3]; 8] = [(-1.0, 0.0), (-1.0, 1.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (1.0, -1.0), (0.0, -1.0), (-1.0, -1.0)].map(|(x, y)| [100.0 * x, 100.0 * y, 0.0]);
            let c0 = [0.0; 3];
            let uv = uvn([[176, 32], [208, 32], [176, 64], [208, 64]], SHEET_256);
            for q in [[p[1], p[2], p[0], c0], [p[3], p[2], p[4], c0], [p[5], p[6], p[4], c0], [p[7], p[6], p[0], c0]] {
                self.lib_quad(q, uv, self.glow);
            }
            // RAYS.
            let uv = uvn([[142, 0], [175, 0], [142, 63], [175, 63]], SHEET_256);
            for i in 0..64 {
                let m = rotm64(self.rays[i][0], self.rays[i][1], self.rays[i][2]);
                let w = RAY.map(|v| std::array::from_fn(|j| v[0] * m[0][j] + v[1] * m[1][j] + v[2] * m[2][j]));
                self.lib_quad(w, uv, self.ray_a);
            }
            if self.s <= 1.0 {
                self.s += 0.2;
            } else {
                self.s += 0.005;
                self.opaque_number = false;
                self.number_a = decay(self.number_a, 4);
                self.ray_a = decay(self.ray_a, 2);
                self.ring = self.ring.map(|a| decay(a, 2));
                self.glow = decay(self.glow, 3);
            }
            for r in &mut self.rays {
                *r = r.map(|v| v + 0x80);
            }
        }
        if u == UPDATES {
            self.done = true;
            8
        } else if u == RET2_U {
            2
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duel_fxlib::Rng;

    fn module(param: u32) -> C537 {
        let mut rng = Rng::new(4);
        let m = C537::new(param, &mut || rng.next());
        assert_eq!(rng.calls, INIT_RAND);
        m
    }

    #[test]
    fn returns_and_counts() {
        let mut m = module(0x01F4_0000);
        assert_eq!((m.amount(), m.glyphs()), (500, &[0xB, 5, 0, 0][..]));
        let mut rets = Vec::new();
        for u in 1..=UPDATES {
            let r = m.update(u);
            if r != 0 {
                rets.push((u, r));
            }
            if u < 16 {
                assert!(m.additive.is_empty() && m.opaque.is_empty() && m.drawn().is_none());
            }
            if u == 16 {
                // "+500": 4 glyphs + 60 rings + 4 glow + 64 rays = 132 quads.
                assert_eq!((m.opaque.len() + m.additive.len()) / 2, 132);
            }
        }
        assert_eq!(rets, vec![(22, 2), (43, 8)]);
        assert_eq!(m.update(44), 8);
        assert!(m.additive.is_empty() && m.alpha_blend.is_empty());
    }

    #[test]
    fn number_layout_alpha_and_blend() {
        let mut m = module(0x01F4_0000);
        let mut seen = Vec::new();
        for u in 1..=UPDATES {
            m.update(u);
            if let Some(d) = m.drawn() {
                seen.push((u, d));
            }
            if u == 16 {
                // X 95 / 125 / 155 / 185, Y 240..288; green.
                let xs: Vec<f32> = m.opaque.iter().step_by(2).map(|t| t.screen[0][0]).collect();
                assert_eq!(xs, vec![95.0, 125.0, 155.0, 185.0]);
                assert!(m.opaque.iter().step_by(2).all(|t| t.screen[0][1] == 240.0 && t.screen[2] == [t.screen[0][0], 288.0]));
                assert_eq!(m.opaque[0].colour[0], [96, 176, 112, 128]);
                // '+' at v 184, '5' at v 88.
                assert_eq!((m.opaque[0].uv[0][1] * 256.0, m.opaque[2].uv[0][1] * 256.0), (184.0, 88.0));
            }
            if u == 22 {
                assert!(m.opaque.is_empty() && m.alpha_blend.len() == 8);
            }
        }
        let at = |u: u32| seen.iter().find(|x| x.0 == u).unwrap().1;
        assert_eq!((at(16).0, at(21).0, at(22).0, at(43).0), (128, 128, 124, 40));
        assert_eq!((at(21).1, at(22).1), (true, false));
        assert!((at(16).2 - 0.1).abs() < 1e-6 && (at(21).2 - 1.1).abs() < 1e-5);
    }

    #[test]
    fn negative_and_short_amounts() {
        let mut m = module(0x01F4_0100);
        assert_eq!((m.amount(), m.rgb()), (-500, [176, 80, 96]));
        for u in 1..=16 {
            m.update(u);
        }
        // '-' at v 168.
        assert_eq!(m.opaque[0].uv[0][1] * 256.0, 168.0);
        let m = module(0x0007_0103);
        assert_eq!((m.amount(), m.glyphs()), (-7, &[0xA, 7][..]));
        let m = module(0x05DC_0000);
        assert_eq!(m.glyphs(), &[0xB, 1, 5, 0, 0][..]);
        let m = module(0);
        assert_eq!(m.glyphs(), &[0xB, 0][..]);
    }
}
