//! CMODULEs c531 (0x213, Shift's "vanish") and c532 (0x214, "appear"), event
//! 0x1F (handler 0x25FB20), as specified in the project notes
//! section 4 [F code; emu: the reference model `tools/mf_c531_model.py`
//! equals the interpreter's primitives to 0.0625 px in four runs]. One code
//! base, 20 updates each:
//! - three flat RINGS (shape type 1 of the effect library: 4 quadrant quads
//!   each) above the unit's cell at y 40 / 50 / 60 (slot 1..5) or 10 / 20 / 30,
//!   lying in the XZ plane (node rot x 0x4000), TEXEFF 6 cell 32 x 32 with a
//!   2-frame vertical flip-book, colour (128, 128, 128, a), additive (0x48);
//! - c531: scale 2.6 -> 1.0 (-0.3/U) while alpha 0 -> 128 (+32/U), held, then
//!   from u 14 shrinking -0.2/U and fading -32/U; returns 2 at u 17 (the
//!   handler moves the unit at u 18 = E+25) and 8 at u 20;
//! - c532: scale 0.1 -> 1.0 (+0.2/U) at alpha 128, held, then from u 14
//!   widening +0.3/U and fading -32/U; returns 8 at u 20 (never 2);
//! - the unit side effect (ring 2's pass): c531 card alpha 128 -> 80, 32, 0
//!   (u 13-15), c532 0 -> 48, 96, 128 (u 1-3); a shown model narrows into a
//!   beam (c531: x, z -0.3/U to 0, y +0.6/U to 3) or opens out (c532).
//!
//! SE bank 1: c531 0x27, c532 0x28, at u 1. Three random values at init
//! (ring half sizes), none after (the shared 0x19AE88 stream in the original).
//!
//! [H / not reproduced]: the port's random values (its own LCG, or a caller's
//! shared stream through [`C531::with_rng`], same call order and count); no OT
//! keys / real Z (key = trunc(mean w) in the original; the port draws after
//! the scene); 0x1F6B80(t, 1) at init is not modelled; the model-scale side
//! effect was not run in the interpreter (no shown model in the runs), and
//! c532's start scale (0, 3, 0) assumes c531 ran to its end on the same unit.
use crate::duel_c602::projector;
use crate::duel_c621::FxTri;

/// Module life (0x14).
pub const UPDATES: u32 = 20;
/// c531 only: the update returning 2 (the handler moves the unit at u 18).
pub const RET2_U: u32 = 17;
/// SE (SECTL bank 1) played at `SE_U`: c531 0x27, c532 0x28.
pub const SE_BANK: u8 = 1;
pub const SE_C531: u16 = 0x27;
pub const SE_C532: u16 = 0x28;
pub const SE_U: u32 = 1;
/// TEXEFF record and its sheet size (the project notes).
pub const SHEET: u8 = 6;
const SHEET_W: f32 = 128.0;
const SHEET_H: f32 = 64.0;
/// Shape type 1 template (data 0x1E01580).
const TPL: [(f32, f32); 8] = [(-1., 0.), (-1., 1.), (0., 1.), (1., 1.), (1., 0.), (1., -1.), (0., -1.), (-1., -1.)];

/// The port's random source (the original: newlib rand), 0..0x7FFFFFFF.
fn lcg(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
    ((*seed >> 32) & 0x7FFF_FFFF) as u32
}

/// 0x103540: step `x` down by `s` to `t` (f32).
fn dec(x: f32, t: f32, s: f32) -> f32 {
    if x >= t + s { x - s } else { t }
}

/// 0x103390: step `x` up by `s` to `t` (f32).
fn inc(x: f32, t: f32, s: f32) -> f32 {
    if x <= t - s { x + s } else { t }
}

/// A 4-vertex strip (v0 v1 v2 / v1 v2 v3) as two triangles (v0 v1 v2), (v1 v3 v2).
fn quad(out: &mut Vec<FxTri>, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], c: [u8; 4], sheet: u8) {
    out.push(FxTri { screen: [p[0], p[1], p[2]], uv: [uv[0], uv[1], uv[2]], colour: [c; 3], textured: true, sheet });
    out.push(FxTri { screen: [p[1], p[3], p[2]], uv: [uv[1], uv[3], uv[2]], colour: [c; 3], textured: true, sheet });
}

#[derive(Clone, Debug, PartialEq)]
pub struct C531 {
    /// The unit's cell (col, row).
    pub cell: (i32, i32),
    /// false = c531 (vanish), true = c532 (appear).
    pub appear: bool,
    /// Node positions (cell centre at the ring heights).
    pos: [[f32; 3]; 3],
    /// Ring half sizes (50 + rand % 10) / 2.
    half: [f32; 3],
    s: [f32; 3],
    a: [u8; 3],
    /// (scale, alpha) per ring as drawn in the last update.
    drawn: [(f32, u8); 3],
    /// The unit's card alpha (0x1F6FF0) after the last update.
    card: u8,
    /// The shown model's scale (x, y, z) after the last update.
    model: [f32; 3],
    /// D+0x470 (= updates done).
    c: u32,
    life: i32,
    rand_calls: u32,
    done: bool,
    /// Primitives (quads) sent in the last update.
    prims: usize,
    /// Additive (ALPHA 0x48) triangles of the last update.
    pub additive: Vec<FxTri>,
    /// Kept empty (the convention of the other modules).
    pub subtractive: Vec<FxTri>,
    pub opaque: Vec<FxTri>,
    pub alpha_blend: Vec<FxTri>,
}

impl C531 {
    /// Init with the port's own LCG: `cell` = the unit's cell, `slot` = the
    /// low byte of its ref (1..5 = monster zone: rings at 40 / 50 / 60, else
    /// 10 / 20 / 30), `appear` = c532.
    pub fn new(cell: (i32, i32), slot: u8, appear: bool, seed: u32) -> Self {
        let mut s = u64::from(seed) | 1;
        Self::with_rng(cell, slot, appear, &mut || lcg(&mut s))
    }

    /// Init drawing its three values (ring order) from a caller's stream
    /// (the original's shared 0x19AE88 stream).
    pub fn with_rng(cell: (i32, i32), slot: u8, appear: bool, rng: &mut impl FnMut() -> u32) -> Self {
        let base = [50.0 * (cell.0 - 3) as f32, 0.0, 50.0 * (cell.1 - 3) as f32];
        let mut half = [0.0f32; 3];
        let mut pos = [[0.0f32; 3]; 3];
        for i in 0..3 {
            half[i] = (50.0f32 + (rng() % 10) as f32) / 2.0;
            let y = if (1..=5).contains(&slot) { 10.0 * (i as f32 + 4.0) } else { 10.0 * (i as f32 + 1.0) };
            pos[i] = [base[0], y, base[2]];
        }
        let (s0, a0) = if appear { (0.1, 128) } else { (2.6, 0) };
        Self {
            cell,
            appear,
            pos,
            half,
            s: [s0; 3],
            a: [a0; 3],
            drawn: [(s0, a0); 3],
            card: if appear { 0 } else { 128 },
            model: if appear { [0.0, 3.0, 0.0] } else { [1.0; 3] },
            c: 0,
            life: UPDATES as i32,
            rand_calls: 3,
            done: false,
            prims: 0,
            additive: Vec::new(),
            subtractive: Vec::new(),
            opaque: Vec::new(),
            alpha_blend: Vec::new(),
        }
    }

    /// The SE id played at u 1 (bank 1).
    pub fn se(&self) -> u16 {
        if self.appear { SE_C532 } else { SE_C531 }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// Random values drawn at init (always 3).
    pub fn rand_calls(&self) -> u32 {
        self.rand_calls
    }

    /// Ring half sizes (25.0 ..= 29.5).
    pub fn half_sizes(&self) -> [f32; 3] {
        self.half
    }

    /// (scale, alpha) of ring `i` as drawn in the last update.
    pub fn drawn(&self, i: usize) -> (f32, u8) {
        self.drawn[i]
    }

    /// Quads sent in the last update (12 when all are on screen).
    pub fn prims(&self) -> usize {
        self.prims
    }

    /// The card alpha the module has written to the unit after the last
    /// update: c531 128 until u 12, 80, 32, 0 at u 13-15, then 0; c532 48,
    /// 96, 128 at u 1-3 (0 before u 1), then 128.
    pub fn card_alpha(&self) -> u8 {
        self.card
    }

    /// A shown model's scale (x, y, z) after the last update [F, not run].
    pub fn model_scale(&self) -> [f32; 3] {
        self.model
    }

    /// Module update `u` (1, 2, ..) with the camera (`eye`, `target`).
    /// Returns 0, 2 (c531 at u 17), or 8 at u 20 (and every later call).
    pub fn update(&mut self, _u: u32, eye: [f64; 3], target: [f64; 3]) -> u32 {
        self.additive.clear();
        self.prims = 0;
        if self.done {
            return 8;
        }
        let c = self.c;
        let frame = 32.0 * (c % 2) as f32;
        let uv = [[0.0, frame], [32.0, frame], [0.0, frame + 32.0], [32.0, frame + 32.0]].map(|[u, v]: [f32; 2]| [u / SHEET_W, v / SHEET_H]);
        let project = projector(eye, target);
        for i in 0..3 {
            self.drawn[i] = (self.s[i], self.a[i]);
            if let Some(project) = project.as_ref() {
                let (s, h, pos) = (self.s[i], self.half[i], self.pos[i]);
                // local (x, y, 0) @ R(0x4000, 0, 0) = (x, 0, y), scaled by s (f32).
                let w = |k: usize| -> [f64; 3] {
                    let (tx, ty) = TPL[k];
                    let (x, y) = (tx * h, ty * h);
                    [f64::from(pos[0]) + f64::from(s) * f64::from(x), f64::from(pos[1]), f64::from(pos[2]) + f64::from(s) * f64::from(y)]
                };
                let centre = pos.map(f64::from);
                for k in 0..4 {
                    let v1 = if k < 2 { 2 } else { 6 };
                    let v2 = if k == 1 || k == 2 { 4 } else { 0 };
                    let ws = [w(2 * k + 1), w(v1), w(v2), centre];
                    let (Some(a), Some(b), Some(cc), Some(d)) = (project(ws[0]), project(ws[1]), project(ws[2]), project(ws[3])) else {
                        continue;
                    };
                    self.prims += 1;
                    quad(&mut self.additive, [a, b, cc, d], uv, [128, 128, 128, self.a[i]], SHEET);
                }
            }
            // After the draw: the update's changes.
            if c < 12 {
                if self.appear {
                    self.s[i] = inc(self.s[i], 1.0, 0.2);
                } else {
                    self.a[i] = (self.a[i] + 32).min(128);
                    self.s[i] = dec(self.s[i], 1.0, 0.3);
                }
            } else {
                self.s[i] = if self.appear { self.s[i] + 0.3 } else { self.s[i] - 0.2 };
                self.a[i] = self.a[i].saturating_sub(32);
            }
            if i == 2 {
                if !self.appear && c >= 12 {
                    self.model = [dec(self.model[0], 0.0, 0.3), inc(self.model[1], 3.0, 0.6), dec(self.model[2], 0.0, 0.3)];
                    self.card = self.card.saturating_sub(0x30);
                } else if self.appear && c < 12 {
                    self.model = [inc(self.model[0], 1.0, 0.3), dec(self.model[1], 1.0, 0.6), inc(self.model[2], 1.0, 0.3)];
                    self.card = (self.card + 0x30).min(0x80);
                }
            }
        }
        self.c += 1;
        self.life -= 1;
        if !self.appear && self.c == RET2_U {
            2
        } else if self.life <= 0 {
            self.done = true;
            8
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trap camera on cell (c, r), side 0.
    fn trap(c: i32, r: i32) -> ([f64; 3], [f64; 3]) {
        let t = [50.0 * f64::from(c - 3), 0.0, 50.0 * f64::from(r - 3)];
        let p = 22.5f64.to_radians();
        (std::array::from_fn(|k| t[k] + 180.0 * [0.0, p.sin(), p.cos()][k]), t)
    }

    fn run(appear: bool, slot: u8) -> Vec<(u32, [(f32, u8); 3], u8, usize, usize, f32)> {
        let (eye, target) = trap(3, 5);
        let mut m = C531::new((3, 5), slot, appear, 11);
        let mut v = Vec::new();
        for u in 1..=UPDATES {
            let r = m.update(u, eye, target);
            let v0 = m.additive.first().map_or(-1.0, |t| t.uv[0][1]);
            v.push((r, [m.drawn(0), m.drawn(1), m.drawn(2)], m.card_alpha(), m.prims(), m.additive.len(), v0));
        }
        assert!(m.done());
        assert_eq!(m.update(21, eye, target), 8);
        assert!(m.additive.is_empty());
        v
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn c531_scales_alphas_returns() {
        let v = run(false, 1);
        let s: Vec<f32> = v.iter().map(|x| x.1[0].0).collect();
        let want = [2.6, 2.3, 2.0, 1.7, 1.4, 1.1, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.8, 0.6, 0.4, 0.2, 0.0, -0.2, -0.4];
        for (u, (a, b)) in s.iter().zip(want).enumerate() {
            assert!(close(*a, b), "u {} scale {a} want {b}", u + 1);
        }
        let a: Vec<u8> = v.iter().map(|x| x.1[2].1).collect();
        assert_eq!(a, vec![0, 32, 64, 96, 128, 128, 128, 128, 128, 128, 128, 128, 128, 96, 64, 32, 0, 0, 0, 0]);
        assert_eq!(a[4], 128);
        assert_eq!(a[15], 32);
        let rets: Vec<_> = v.iter().enumerate().filter(|(_, x)| x.0 != 0).map(|(i, x)| (i as u32 + 1, x.0)).collect();
        assert_eq!(rets, vec![(17, 2), (20, 8)]);
        // Card alpha 128 to u 12, 80 / 32 / 0 at u 13-15.
        let card: Vec<u8> = v.iter().map(|x| x.2).collect();
        assert!(card[..12].iter().all(|&c| c == 128));
        assert_eq!(&card[12..16], &[80, 32, 0, 0]);
        assert!(card[15..].iter().all(|&c| c == 0));
    }

    #[test]
    fn c532_scales_alphas_returns() {
        let v = run(true, 1);
        let s: Vec<f32> = v.iter().map(|x| x.1[1].0).collect();
        let want = [0.1, 0.3, 0.5, 0.7, 0.9, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.3, 1.6, 1.9, 2.2, 2.5, 2.8, 3.1];
        for (u, (a, b)) in s.iter().zip(want).enumerate() {
            assert!(close(*a, b), "u {} scale {a} want {b}", u + 1);
        }
        assert!(close(s[5], 1.0) && close(s[13], 1.3));
        let a: Vec<u8> = v.iter().map(|x| x.1[0].1).collect();
        assert_eq!(a, vec![128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 96, 64, 32, 0, 0, 0, 0]);
        let rets: Vec<_> = v.iter().enumerate().filter(|(_, x)| x.0 != 0).map(|(i, x)| (i as u32 + 1, x.0)).collect();
        assert_eq!(rets, vec![(20, 8)]);
        let card: Vec<u8> = v.iter().map(|x| x.2).collect();
        assert_eq!(&card[..4], &[48, 96, 128, 128]);
        assert!(card[2..].iter().all(|&c| c == 128));
    }

    #[test]
    fn quads_frames_rand_and_geometry() {
        for appear in [false, true] {
            let v = run(appear, 1);
            // 12 quads (24 triangles) every update, alpha-0 ones included: 240 in all.
            assert!(v.iter().all(|x| x.3 == 12 && x.4 == 24));
            assert_eq!(v.iter().map(|x| x.3).sum::<usize>(), 240);
            // Flip-book: v = 0 at odd u, 32 (0.5 of the 64-texel sheet) at even u.
            for (i, x) in v.iter().enumerate() {
                assert_eq!(x.5, if (i + 1) % 2 == 0 { 0.5 } else { 0.0 });
            }
        }
        let m = C531::new((3, 5), 1, false, 11);
        assert_eq!(m.rand_calls(), 3);
        assert!(m.half_sizes().iter().all(|h| (25.0..=29.5).contains(h) && (h * 2.0).fract() == 0.0));
        assert_eq!((m.se(), C531::new((3, 5), 1, true, 1).se()), (0x27, 0x28));
        // Shared stream: three draws in ring order.
        let mut n = 0u32;
        let m = C531::with_rng((2, 1), 6, true, &mut || {
            n += 1;
            n + 3
        });
        assert_eq!(n, 3);
        assert_eq!(m.half_sizes(), [27.0, 27.5, 28.0]);
        // The quad corner v3 = the ring centre at y 40 / 50 / 60 (slot 1..5), 10 / 20 / 30 (slot 6).
        let (eye, target) = trap(3, 5);
        let project = projector(eye, target).unwrap();
        for (slot, ys) in [(1u8, [40.0, 50.0, 60.0]), (6, [10.0, 20.0, 30.0])] {
            let mut m = C531::new((3, 5), slot, false, 2);
            m.update(1, eye, target);
            for (i, y) in ys.iter().enumerate() {
                let c = project([0.0, *y, 100.0]).unwrap();
                assert_eq!(m.additive[8 * i + 1].screen[1], c, "slot {slot} ring {i}");
                // v0 of quad 0 = P1 = (-h, h) -> world (-2.6 h, y, 100 + 2.6 h).
                let h = f64::from(m.half_sizes()[i]);
                let s = f64::from(2.6f32);
                assert_eq!(m.additive[8 * i].screen[0], project([-s * h, *y, 100.0 + s * h]).unwrap());
            }
            // UVs normalised by the 128 x 64 sheet, TEXEFF 6, additive grey.
            assert_eq!(m.additive[0].uv, [[0.0, 0.0], [0.25, 0.0], [0.0, 0.5]]);
            assert!(m.additive.iter().all(|t| t.sheet == 6 && t.textured && t.colour[0] == [128, 128, 128, 0]));
        }
    }

    #[test]
    fn model_scale_side_effect() {
        let (eye, target) = trap(3, 5);
        let mut m = C531::new((3, 5), 1, false, 4);
        for u in 1..=20 {
            m.update(u, eye, target);
        }
        let s = m.model_scale();
        assert!(s[0] == 0.0 && s[2] == 0.0 && close(s[1], 3.0));
        let mut m = C531::new((3, 5), 1, true, 4);
        for u in 1..=20 {
            m.update(u, eye, target);
        }
        assert!(m.model_scale().iter().all(|v| close(*v, 1.0)));
    }
}
