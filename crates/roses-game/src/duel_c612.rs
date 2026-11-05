//! CMODULE c612 (0x264), the crush (event 0x70, handler 0x274D90: Crush
//! terrain, Aqua Chorus's virus), as specified in
//! the project notes section 5 [F code; emu: the reference
//! model `tools/mf_c612_model.py` equals the interpreter's primitives to
//! 0.0625 px in three runs]. Anchored at the victim's model point (0x1F65C0(1),
//! (0, 0, 0) when no model is shown), both nodes billboards:
//! - DUST (u 1-53): four bursts of 256 tiny ochre specks (TEXEFF 15 (176, 64)
//!   32 x 32, rotated UVs, half size 0.3) spraying out in the screen plane,
//!   x = t m b[j], y = t m b[j + 1] from a 256-byte random table; burst k
//!   starts 2k updates late and restarts every 10 updates; colour
//!   (144, 112, 64, a), a -16/U from 128; additive (0x48);
//! - SMOKE (u 1-67): two c557-style emitters (16 positions x 1.01/U, 8 puffs,
//!   puff k from u 3k, 2-update frames over TEXEFF 15 (128..255, 192..255)),
//!   half size 12, colours (64, 32, 96) / (128, 64, 128) at alpha 32, in node D
//!   scaled 0.1, 0.3, 0.5, 0.7, 0.9, then 1.0; ALPHA 0x44 = (Cs - Cd) As + Cd
//!   (alpha-blended, As = a / 128), Z 0xFFFFFF and key 0: over everything.
//!
//! Returns 2 at u 65 (the handler removes the victim at u 66 = E+118) and 8 at
//! u 67. SE bank 1 0x50 at u 1. 320 random values at init (smoke A 16 x (x, y),
//! smoke B 16 x (x, y), 256 table bytes), none after.
//!
//! The module itself writes nothing to the victim. The handler around it
//! does, keyed to c612's updates (E+53 = u 1): the struggle [`struggle_frame`]
//! every update while c612 runs (motion 4 held in frames 11..16), the unit
//! fade 0x2519E0(g, s, -1, 1, 3) at u 34 (E+86), and the removal (to the
//! graveyard) when the 2 of u 65 is seen at u 66 (E+118); see
//! [`victim_level`] / [`victim_removed`].
//!
//! [H / not reproduced]: the port's random values (its own LCG, or a caller's
//! shared stream through [`C612::with_rng`], same call order and count); no
//! dust OT keys / real Z (the original depth-tests the dust with key = mean
//! w; the smoke is drawn over everything there as here); the fade speed of
//! 0x2519E0 (the port's -8/U).
use crate::duel_c602::{billboard_axes, projector};
use crate::duel_c621::FxTri;

/// Module life (0x43).
pub const UPDATES: u32 = 67;
/// The update returning 2 (the handler removes the victim at u 66).
pub const RET2_U: u32 = 65;
/// The update the handler starts the victim's fade (E+86).
pub const FADE_U: u32 = 34;
/// Dust is drawn while c = u - 1 < 53.
pub const DUST_LAST_U: u32 = 53;
/// SE (SECTL bank 1) played at `SE_U`.
pub const SE_BANK: u8 = 1;
pub const SE_ID: u16 = 0x50;
pub const SE_U: u32 = 1;
/// Random values drawn at init.
pub const INIT_RAND: u32 = 320;
/// TEXEFF record and its sheet size (the project notes).
pub const SHEET: u8 = 15;
const SHEET_SIZE: f32 = 256.0;
const SMOKE_RGB: [[u8; 3]; 2] = [[64, 32, 96], [128, 64, 128]];
const SMOKE_ALPHA: u8 = 32;
const SMOKE_HALF: f32 = 12.0;
const DUST_RGB: [u8; 3] = [144, 112, 64];
const DUST_HALF: f32 = 0.3;
const DUST_SPEED: f32 = 0.05;

/// The port's random source (the original: newlib rand), 0..0x7FFFFFFF.
fn lcg(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
    ((*seed >> 32) & 0x7FFF_FFFF) as u32
}

fn quad(out: &mut Vec<FxTri>, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], c: [u8; 4]) {
    out.push(FxTri { screen: [p[0], p[1], p[2]], uv: [uv[0], uv[1], uv[2]], colour: [c; 3], textured: true, sheet: SHEET });
    out.push(FxTri { screen: [p[1], p[3], p[2]], uv: [uv[1], uv[3], uv[2]], colour: [c; 3], textured: true, sheet: SHEET });
}

fn uv(p: [[i32; 2]; 4]) -> [[f32; 2]; 4] {
    p.map(|[u, v]| [u as f32 / SHEET_SIZE, v as f32 / SHEET_SIZE])
}

/// 0x262FA0's clamp of the victim's motion-4 frame, applied by the handler
/// every update while c612 runs: frame >= 16 -> 11; frame in (11, 16) and
/// rand % 3 == 0 -> frame - 2 (one rand only for frames in (11, 16), from the
/// shared stream); else unchanged.
pub fn struggle_frame(frame: i32, rng: &mut impl FnMut() -> u32) -> i32 {
    if frame >= 16 {
        11
    } else if frame > 11 && rng() % 3 == 0 {
        frame - 2
    } else {
        frame
    }
}

/// The victim's unit level (128 = shown) after c612 update `u`: 128 until
/// u 33, fading from u 34 (0x2519E0(g, s, -1, 1, 3), E+86) at the port's
/// -8/U [H on the speed].
pub fn victim_level(u: u32) -> u8 {
    if u < FADE_U { 128 } else { 128u32.saturating_sub(8 * (u - FADE_U + 1)) as u8 }
}

/// Whether the handler has removed the victim (graveyard) after c612 update
/// `u`: the 2 of u 65 is seen at u 66 (E+118).
pub fn victim_removed(u: u32) -> bool {
    u > RET2_U
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Dust {
    t: i32,
    a: u8,
    seed: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct C612 {
    /// The victim's model point (node position of both nodes).
    pub anchor: [f64; 3],
    smoke: [[[f32; 2]; 16]; 2],
    tab: [i8; 256],
    dust: [Dust; 4],
    /// Node D's scale (smoke).
    sigma: f32,
    /// Node D's scale as drawn in the last update.
    drawn_sigma: f32,
    c: u32,
    life: i32,
    done: bool,
    rand_calls: u32,
    /// (dust quads, smoke quads) of the last update.
    counts: (usize, usize),
    /// Additive (ALPHA 0x48) triangles of the last update: the dust.
    pub additive: Vec<FxTri>,
    /// Kept empty (the convention of the other modules).
    pub subtractive: Vec<FxTri>,
    pub opaque: Vec<FxTri>,
    /// ALPHA 0x44 triangles of the last update, (Cs - Cd) As + Cd with
    /// As = a / 128, drawn over everything (Z 0xFFFFFF, key 0): the smoke
    /// (emitter A's puffs, then B's).
    pub alpha_blend: Vec<FxTri>,
}

impl C612 {
    /// Init with the port's own LCG; `anchor` = the victim's model point.
    pub fn new(anchor: [f64; 3], seed: u32) -> Self {
        let mut s = u64::from(seed) | 1;
        Self::with_rng(anchor, &mut || lcg(&mut s))
    }

    /// Init drawing its 320 values from a caller's stream (the original's
    /// shared 0x19AE88 stream): per emitter A, B, k = 0..15: x = rand % 20 -
    /// 10, y = (rand & 15) - 4; then 256 x b = (rand >> 4) as i8.
    pub fn with_rng(anchor: [f64; 3], rng: &mut impl FnMut() -> u32) -> Self {
        let mut n = 0u32;
        let mut r = || {
            n += 1;
            rng()
        };
        let mut smoke = [[[0.0f32; 2]; 16]; 2];
        for e in smoke.iter_mut() {
            for p in e.iter_mut() {
                let x = (r() % 20) as i32 - 10;
                let y = (r() & 15) as i32 - 4;
                *p = [x as f32, y as f32];
            }
        }
        let tab = std::array::from_fn(|_| ((r() >> 4) & 0xFF) as u8 as i8);
        Self {
            anchor,
            smoke,
            tab,
            dust: std::array::from_fn(|k| Dust { t: -2 * k as i32, a: 128, seed: k + 7 }),
            sigma: 0.1,
            drawn_sigma: 0.1,
            c: 0,
            life: UPDATES as i32,
            done: false,
            rand_calls: n,
            counts: (0, 0),
            additive: Vec::new(),
            subtractive: Vec::new(),
            opaque: Vec::new(),
            alpha_blend: Vec::new(),
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// Random values drawn (320, all at init).
    pub fn rand_calls(&self) -> u32 {
        self.rand_calls
    }

    /// (dust specks, smoke puffs) sent in the last update.
    pub fn counts(&self) -> (usize, usize) {
        self.counts
    }

    /// Node D's (smoke) scale as drawn in the last update.
    pub fn smoke_scale(&self) -> f32 {
        self.drawn_sigma
    }

    /// Burst k's (t, alpha) now (after the last update).
    pub fn burst(&self, k: usize) -> (i32, u8) {
        (self.dust[k].t, self.dust[k].a)
    }

    /// Module update `u` (1, 2, ..) with the camera (`eye`, `target`).
    /// Returns 0, 2 at u 65, or 8 at u 67 (and every later call).
    pub fn update(&mut self, _u: u32, eye: [f64; 3], target: [f64; 3]) -> u32 {
        self.additive.clear();
        self.alpha_blend.clear();
        self.counts = (0, 0);
        if self.done {
            return 8;
        }
        let u = self.c + 1;
        let project = projector(eye, target);
        let [right, up, _] = billboard_axes(eye, target);
        let t = self.anchor;
        let bill = |s: f64, x: f32, y: f32| -> [f64; 3] { std::array::from_fn(|i| t[i] + s * (f64::from(x) * right[i] + f64::from(y) * up[i])) };
        let proj4 = |w: [[f64; 3]; 4]| -> Option<[[f32; 2]; 4]> {
            let p = project.as_ref()?;
            Some([p(w[0])?, p(w[1])?, p(w[2])?, p(w[3])?])
        };
        // DUST (5.3), node D+0x310 (scale 1).
        if self.c < DUST_LAST_U {
            let duv = uv([[176, 64], [176, 95], [207, 64], [207, 95]]);
            for k in 0..4 {
                let mut d = self.dust[k];
                if d.t >= 10 {
                    d.t = 0;
                    d.a = 128;
                }
                if d.t > 0 {
                    let tf = d.t as f32;
                    let rgba = [DUST_RGB[0], DUST_RGB[1], DUST_RGB[2], d.a];
                    for i in 0..256 {
                        let m = DUST_SPEED * (0.1f32 * f32::from(self.tab[i].unsigned_abs()));
                        let j = (d.seed + i) & 0xFF;
                        let x = tf * (m * f32::from(self.tab[j]));
                        let y = tf * (m * f32::from(self.tab[(j + 1) & 0xFF]));
                        let l = [(x - DUST_HALF, y + DUST_HALF), (x + DUST_HALF, y + DUST_HALF), (x - DUST_HALF, y - DUST_HALF), (x + DUST_HALF, y - DUST_HALF)];
                        let Some(v) = proj4(l.map(|(a, b)| bill(1.0, a, b))) else {
                            continue;
                        };
                        self.counts.0 += 1;
                        quad(&mut self.additive, v, duv, rgba);
                    }
                }
                d.t += 1;
                d.a = d.a.saturating_sub(16);
                self.dust[k] = d;
            }
        }
        // SMOKE (5.4), node D (scale sigma), emitter A then B.
        let sigma = self.sigma;
        self.drawn_sigma = sigma;
        for e in 0..2 {
            for p in self.smoke[e].iter_mut() {
                *p = p.map(|v| v * 1.01f32);
            }
            let rgba = [SMOKE_RGB[e][0], SMOKE_RGB[e][1], SMOKE_RGB[e][2], SMOKE_ALPHA];
            for k in 0..8u32 {
                if k > 0 && u < 3 * k {
                    continue;
                }
                let f = ((u as i32 - 1 - 3 * k as i32).max(0) / 2) % 8;
                let (u0, v0) = (128 + 32 * (f % 4), 192 + 32 * (f / 4));
                let [px, py] = self.smoke[e][k as usize];
                let h = SMOKE_HALF;
                let l = [(px - h, py + h), (px + h, py + h), (px - h, py - h), (px + h, py - h)];
                let Some(v) = proj4(l.map(|(a, b)| bill(f64::from(sigma), a, b))) else {
                    continue;
                };
                self.counts.1 += 1;
                quad(&mut self.alpha_blend, v, uv([[u0, v0], [u0 + 31, v0], [u0, v0 + 31], [u0 + 31, v0 + 31]]), rgba);
            }
        }
        // 0x1033D0: sigma +0.2 to 1.0.
        self.sigma = if self.sigma <= 1.0f32 - 0.2f32 { self.sigma + 0.2 } else { 1.0 };
        self.c += 1;
        self.life -= 1;
        if self.c == RET2_U {
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

    /// The leader 0x000's model point in state 690 (the doc's r612_l run).
    const LEADER: [f64; 3] = [0.0, 45.863_189_697_265_625, 150.151_947_021_484_38];

    fn run(seed: u32) -> (C612, Vec<(u32, (usize, usize), f32, Vec<FxTri>, Vec<FxTri>)>) {
        let (eye, target) = trap(3, 6);
        let mut m = C612::new(LEADER, seed);
        let mut v = Vec::new();
        for u in 1..=UPDATES {
            let r = m.update(u, eye, target);
            v.push((r, m.counts(), m.smoke_scale(), m.additive.clone(), m.alpha_blend.clone()));
        }
        (m, v)
    }

    #[test]
    fn returns_rand_and_end() {
        let (mut m, v) = run(9);
        let rets: Vec<_> = v.iter().enumerate().filter(|(_, x)| x.0 != 0).map(|(i, x)| (i as u32 + 1, x.0)).collect();
        assert_eq!(rets, vec![(65, 2), (67, 8)]);
        assert!(m.done());
        assert_eq!(m.rand_calls(), 320);
        let (eye, target) = trap(3, 6);
        assert_eq!(m.update(68, eye, target), 8);
        assert!(m.additive.is_empty() && m.alpha_blend.is_empty());
        // Shared stream: exactly 320 draws.
        let mut n = 0u32;
        let m = C612::with_rng(LEADER, &mut || {
            n += 1;
            n * 7919
        });
        assert_eq!((n, m.rand_calls()), (INIT_RAND, INIT_RAND));
        // Table byte j = signed (rand >> 4) & 0xFF: draw 65 = 65 * 7919.
        assert_eq!(m.tab[0], (((65u32 * 7919) >> 4) & 0xFF) as u8 as i8);
        // Smoke A position 0 = (rand % 20 - 10, (rand & 15) - 4).
        assert_eq!(m.smoke[0][0], [(7919 % 20) as f32 - 10.0, ((2 * 7919) & 15) as f32 - 4.0]);
    }

    #[test]
    fn dust_counts_and_alphas() {
        let (_, v) = run(3);
        let dust = |u: usize| v[u - 1].1 .0;
        assert_eq!(dust(1), 0);
        // u 2: burst 0 at t 1, alpha 112, all 256 specks on screen.
        assert_eq!(dust(2), 256);
        assert!(v[1].3.iter().all(|t| t.colour[0] == [144, 112, 64, 112]));
        assert_eq!(v[1].3.len(), 512);
        // UVs rotated against XY: v0 (176, 64), v1 (176, 95), v2 (207, 64).
        assert_eq!(v[1].3[0].uv, [[176.0 / 256.0, 0.25], [176.0 / 256.0, 95.0 / 256.0], [207.0 / 256.0, 0.25]]);
        // u 3: burst 0 t 2 (96); burst 1 at t 0 (not drawn) until u 4.
        assert!(dust(3) <= 256 && dust(4) <= 512);
        assert!(dust(53) > 0);
        for u in 54..=67 {
            assert_eq!(dust(u), 0, "u {u}");
        }
        assert!(v.iter().all(|x| x.1 .0 <= 1018));
        // Burst 0's alphas at t 1..9: 112, 96, .., 16, 0, 0 (u 2-10), restart at u 12.
        let a0: Vec<u8> = (2..=10).map(|u| v[u - 1].3.first().map_or(255, |t| t.colour[0][3])).collect();
        assert_eq!(a0, vec![112, 96, 80, 64, 48, 32, 16, 0, 0]);
        assert_eq!(v[11].3[0].colour[0][3], 112);
    }

    #[test]
    fn dust_bursts_follow_the_schedule() {
        let mut m = C612::new([0.0; 3], 5);
        let (eye, target) = trap(3, 3);
        let mut drawn = [[false; 53]; 4];
        for u in 1..=53u32 {
            let before: [(i32, u8); 4] = std::array::from_fn(|k| m.burst(k));
            m.update(u, eye, target);
            for k in 0..4 {
                let t = if before[k].0 >= 10 { 0 } else { before[k].0 };
                drawn[k][u as usize - 1] = t > 0;
            }
        }
        // Burst k draws when (u - 1 - 2k) mod 10 != 0 (after its start at u 2k + 1).
        for k in 0..4 {
            for u in 1..=53usize {
                let want = u > 2 * k + 1 && (u - 1 - 2 * k) % 10 != 0;
                assert_eq!(drawn[k][u - 1], want, "k {k} u {u}");
            }
        }
    }

    #[test]
    fn smoke_puffs_uvs_scale_and_blend() {
        let (_, v) = run(3);
        let puffs = |u: usize| v[u - 1].1 .1;
        // Per emitter: 1 puff at u 1, 2 at u 3, .., 8 from u 21.
        assert_eq!(puffs(1), 2);
        assert_eq!(puffs(2), 2);
        assert_eq!(puffs(3), 4);
        assert_eq!(puffs(20), 14);
        for u in 21..=67 {
            assert_eq!(puffs(u), 16, "u {u}");
        }
        // Puff 0 at u 3: frame 1 -> UV (160, 192).
        let a = &v[2].4;
        assert_eq!(a[0].uv[0], [160.0 / 256.0, 0.75]);
        assert_eq!(a[0].uv[1], [191.0 / 256.0, 0.75]);
        // Emitter A then B, alpha 32, TEXEFF 15.
        assert_eq!(a[0].colour[0], [64, 32, 96, 32]);
        assert_eq!(a[4].colour[0], [128, 64, 128, 32]);
        assert!(v.iter().flat_map(|x| x.4.iter()).all(|t| t.sheet == 15 && t.textured && t.colour[0][3] == 32));
        // Node D scale 0.1, 0.3, 0.5, 0.7, 0.9, then 1.0.
        let s: Vec<f32> = (1..=7).map(|u| v[u - 1].2).collect();
        for (a, b) in s.iter().zip([0.1, 0.3, 0.5, 0.7, 0.9, 1.0, 1.0]) {
            assert!((a - b).abs() < 1e-6, "{s:?}");
        }
        // Puff 0 of emitter A at u 1: centre = anchor + 0.1 (1.01 p) on the billboard.
        let (eye, target) = trap(3, 6);
        let mut m = C612::new(LEADER, 3);
        let p0 = m.smoke[0][0].map(|x| x * 1.01f32);
        m.update(1, eye, target);
        let [r, up, _] = billboard_axes(eye, target);
        let w: [f64; 3] = std::array::from_fn(|i| LEADER[i] + f64::from(0.1f32) * (f64::from(p0[0] - 12.0) * r[i] + f64::from(p0[1] + 12.0) * up[i]));
        assert_eq!(m.alpha_blend[0].screen[0], projector(eye, target).unwrap()(w).unwrap());
    }

    #[test]
    fn victim_struggle_fade_and_removal() {
        assert_eq!((FADE_U, RET2_U, UPDATES, SE_ID), (34, 65, 67, 0x50));
        assert_eq!(victim_level(33), 128);
        assert_eq!(victim_level(34), 120);
        assert_eq!(victim_level(49), 0);
        assert!(!victim_removed(65) && victim_removed(66));
        let r = |v: u32| move || v;
        assert_eq!(struggle_frame(16, &mut r(0)), 11);
        assert_eq!(struggle_frame(20, &mut r(0)), 11);
        assert_eq!(struggle_frame(13, &mut r(3)), 11);
        assert_eq!(struggle_frame(13, &mut r(4)), 13);
        let mut n = 0;
        let mut count = || {
            n += 1;
            0
        };
        assert_eq!(struggle_frame(11, &mut count), 11);
        assert_eq!(struggle_frame(5, &mut count), 5);
        assert_eq!(n, 0);
    }
}
