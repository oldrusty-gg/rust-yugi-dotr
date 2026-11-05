//! CMODULE c507 (0x1FB), Harpie's Feather Duster's feather (event 7, handler
//! 0x25A080: one per target), as specified in
//! the project notes section 12 [F code; emu: the reference
//! model `tools/mf_c507_model.py` equals the interpreter's primitives to
//! 0.0625 px in four runs]. Anchored at the target's card position (cell
//! centre):
//! - FEATHER (u 1-60): the whole TEXEFF 12 sheet (one white feather), a
//!   60 x 30 quad turned -70 deg, offset (-10, -20), in a world node at
//!   (card x, y, card z) rotated R(rx, camera yaw, 0): it drops 5/U from
//!   y 120 while fading in (+8/U), settles at y 40 at u 17, sweeps +-22.5 deg
//!   (period 8) to u 41, then lifts 5/U to 100 and fades out (-16/U, 0 from
//!   u 49); additive (param 0) or ALPHA 0x88 (param != 0);
//! - DUST (u 17-40): 16 grey puffs (TEXEFF 15 (240, 16f)) rising at the card,
//!   positions x 1.01/U, alpha 80, -2/U from u 22; additive.
//!
//! Returns 2 at u 17 (the handler launches c603 param 1 on the unit at u 18
//! = E+18), 8 at u 60. SE bank 1 0x09 at u 17, 28, 34, 41. 32 random values
//! at init (dust 16 x (x, y)), 3 per dust respawn (u 32, 35, 38). It never
//! touches the card object (the card breaks through the handler's c603).
//!
//! [H / not reproduced]: the port's random values (the caller's stream, same
//! order and count); the sine table (see `duel_fxlib`); no OT keys; the
//! feather dips below the board (world y ~ -13 at y 40) and the scene
//! depth's effect on it.
use crate::duel_c621::FxTri;
use crate::duel_fxlib::{cosf, decay, quad, rise_u8, rotm, sinf, uvn, Cam, PSys, DEG, SHEET_128X64, SHEET_256};

/// Module life.
pub const UPDATES: u32 = 60;
/// The update returning 2 (the handler launches c603 param 1 on this unit
/// at u 18).
pub const RET2_U: u32 = 17;
/// The update the handler launches the card's shatter (c603 param 1).
pub const C603_U: u32 = 18;
/// SE (SECTL bank 1): (u, id).
pub const SE_BANK: u8 = 1;
pub const SE: [(u32, u16); 4] = [(17, 0x09), (28, 0x09), (34, 0x09), (41, 0x09)];
/// Random values drawn at init.
pub const INIT_RAND: u32 = 32;

#[derive(Clone, Debug, PartialEq)]
pub struct C507 {
    anchor: [f32; 3],
    y: f32,
    rx: i32,
    fa: u8,
    c: i32,
    dust: PSys,
    opaque_feather: bool,
    /// The feather's 4 local corners (12.3).
    loc: [[f32; 2]; 4],
    age: u32,
    life: i32,
    done: bool,
    rand_calls: u32,
    drawn: (f32, i32, u8),
    dust_drawn: usize,
    /// Additive (ALPHA 0x48) triangles of the last update: the feather (param
    /// 0), the dust.
    pub additive: Vec<FxTri>,
    /// Kept empty (the convention of the other modules).
    pub subtractive: Vec<FxTri>,
    /// ALPHA 0x88 triangles of the last update: the feather with param != 0.
    pub opaque: Vec<FxTri>,
    pub alpha_blend: Vec<FxTri>,
}

impl C507 {
    /// `anchor` = the target's card position (cell centre, y 0); `param` !=
    /// 0 draws the feather with ALPHA 0x88; `rng` = the caller's stream (32
    /// draws: 16 x (x = r % 20 - 10, y = 5 + (r & 15))).
    pub fn new(anchor: [f32; 3], param: u8, rng: &mut impl FnMut() -> u32) -> Self {
        let mut n = 0u32;
        let mut r = || {
            n += 1;
            rng()
        };
        let dust = PSys::lib(&mut r, 0x85, [0.0, 5.0, 0.0], [16, 16], [240, 0], 16, 15, SHEET_256, 10.0, [128, 128, 128, 80], 16, 3, 1, 8, 30, -10, |v| (v & 15) as i32);
        let (hx, hy) = (30.0f32, 0.5f32 * 30.0);
        let th = DEG * -70.0;
        let (c, s) = (cosf(th), sinf(th));
        let (f26, f27, f24, f25) = (c * -hx, s * -hx, s * hy, c * hy);
        let (f28, f24b, f20, f21) = (c * hx, s * hx, s * -hy, c * -hy);
        let o = [-10.0f32, -20.0];
        let loc = [
            [o[0] + (f26 - f24), o[1] + (f27 + f25)],
            [o[0] + (f28 - f24), o[1] + (f24b + f25)],
            [o[0] + (f26 - f20), o[1] + (f27 + f21)],
            [o[0] + (f28 - f20), o[1] + (f24b + f21)],
        ];
        Self {
            anchor,
            y: 120.0,
            rx: 0,
            fa: 0,
            c: -4,
            dust,
            opaque_feather: param != 0,
            loc,
            age: 0,
            life: UPDATES as i32,
            done: false,
            rand_calls: n,
            drawn: (120.0, 0, 0),
            dust_drawn: 0,
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

    /// The feather as drawn in the last update: (node y, rx, alpha).
    pub fn feather(&self) -> (f32, i32, u8) {
        self.drawn
    }

    /// Dust puffs sent in the last update.
    pub fn dust_count(&self) -> usize {
        self.dust_drawn
    }

    /// The feather's local corners (v0..v3, before the node rotation).
    pub fn corners(&self) -> [[f32; 2]; 4] {
        self.loc
    }

    /// c507 never touches the card: its alpha / visibility stays the card's
    /// own (the handler's c603 param 1 breaks it from `C603_U`).
    pub fn card_touched(&self) -> bool {
        false
    }

    /// Module update `u` (1, 2, ..): `yaw16` = the camera yaw (0x205580 out[1]:
    /// 0 side 0, 0x8000 side 1), `rng` the caller's stream (dust respawns),
    /// (`eye`, `target`) the camera. Returns 0, 2 at u 17, 8 at u 60 (and
    /// every later call).
    pub fn update(&mut self, _u: u32, yaw16: u16, rng: &mut impl FnMut() -> u32, eye: [f64; 3], target: [f64; 3]) -> u32 {
        self.additive.clear();
        self.opaque.clear();
        self.dust_drawn = 0;
        if self.done {
            return 8;
        }
        let cam = Cam::new(eye, target);
        // FEATHER: node (card x, y, card z), rotation R(rx, yaw, 0).
        let m = rotm(self.rx & 0xFFFF, i32::from(yaw16), 0);
        let a = self.anchor;
        let y = self.y;
        self.drawn = (y, self.rx, self.fa);
        let w = self.loc.map(|[lx, ly]| {
            let (lx, ly) = (f64::from(lx), f64::from(ly));
            let t = [f64::from(a[0]), f64::from(y), f64::from(a[2])];
            std::array::from_fn(|j| t[j] + lx * f64::from(m[0][j]) + ly * f64::from(m[1][j]))
        });
        if let Some(v) = cam.quad(w) {
            let out = if self.opaque_feather { &mut self.opaque } else { &mut self.additive };
            quad(out, v, uvn([[0, 0], [128, 0], [0, 64], [128, 64]], SHEET_128X64), [[128, 128, 128, self.fa]; 4], true, 12);
        }
        if self.life >= 21 {
            if self.y >= 40.0f32 + 5.0 {
                self.y -= 5.0;
            } else {
                self.y = 40.0;
                if self.c < 0 {
                    self.rx -= 0x400;
                } else if self.c & 7 < 4 {
                    self.rx += 0x800;
                } else {
                    self.rx -= 0x800;
                }
                self.dust_drawn = self.dust.draw(&cam, rng, a, 1.0, Some(1.01), &mut self.additive);
                if (0..40).contains(&self.c) {
                    self.dust.a = decay(self.dust.a, 2);
                }
                self.c += 1;
            }
            self.fa = rise_u8(self.fa, 128, 8);
        } else {
            self.rx = if self.rx <= -0x800 { self.rx + 0x800 } else { 0 };
            self.y = if self.y <= 100.0 - 5.0 { self.y + 5.0 } else { 100.0 };
            self.fa = decay(self.fa, 16);
        }
        self.age += 1;
        self.life -= 1;
        if self.age == RET2_U {
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
    use crate::duel_fxlib::tests::{overview, trap};
    use crate::duel_fxlib::Rng;

    fn run(param: u8, seed: u32) -> (C507, Vec<(u32, (f32, i32, u8), usize, u32, usize, usize)>) {
        let (eye, target) = overview();
        let mut rng = Rng::new(seed);
        let mut m = C507::new([-50.0, 0.0, 150.0], param, &mut || rng.next());
        assert_eq!(rng.calls, INIT_RAND);
        let mut v = Vec::new();
        for u in 1..=UPDATES {
            let n0 = rng.calls;
            let r = m.update(u, 0, &mut || rng.next(), eye, target);
            v.push((r, m.feather(), m.dust_count(), rng.calls - n0, m.additive.len(), m.opaque.len()));
        }
        (m, v)
    }

    #[test]
    fn corners_follow_the_spec() {
        let mut rng = Rng::new(1);
        let m = C507::new([0.0; 3], 0, &mut || rng.next());
        let want = [[-6.166f32, 13.320], [14.356, -43.059], [-34.356, 3.059], [-13.834, -53.320]];
        for (a, b) in m.corners().iter().zip(want) {
            assert!((a[0] - b[0]).abs() < 2e-3 && (a[1] - b[1]).abs() < 2e-3, "{:?}", m.corners());
        }
    }

    #[test]
    fn returns_rand_and_feather_path() {
        let (mut m, v) = run(0, 7);
        let rets: Vec<_> = v.iter().enumerate().filter(|(_, x)| x.0 != 0).map(|(i, x)| (i as u32 + 1, x.0)).collect();
        assert_eq!(rets, vec![(17, 2), (60, 8)]);
        let f = |u: usize| v[u - 1].1;
        // y: 120 - 5 (u - 1) to 40 at u 17, 40 to u 41, 45 .. 100 (u 53 on).
        assert_eq!((f(1).0, f(17).0, f(41).0, f(42).0, f(53).0, f(60).0), (120.0, 40.0, 40.0, 45.0, 100.0, 100.0));
        // fa: 8 (u - 1) to 128 at u 17, 128 to u 41, 112 .. 16 (u 48), 0 from u 49.
        assert_eq!((f(1).2, f(17).2, f(41).2, f(42).2, f(48).2, f(49).2), (0, 128, 128, 112, 16, 0));
        // rx: 0 (u 1-17), -0x400 (u - 17) (u 18-21), +-0x1000 period 8, 0 at u 42.
        assert_eq!((f(17).1, f(18).1, f(21).1, f(25).1, f(29).1, f(33).1, f(37).1, f(41).1, f(42).1), (0, -0x400, -0x1000, 0x1000, -0x1000, 0x1000, -0x1000, 0x1000, 0));
        // Dust: 1 at u 17, 2 at u 19, 9 at u 40, 0 at u 41; respawns at u 32, 35, 38.
        assert_eq!((v[15].2, v[16].2, v[18].2, v[39].2, v[40].2), (0, 1, 2, 9, 0));
        let rr: Vec<u32> = v.iter().enumerate().filter(|(_, x)| x.3 > 0).map(|(i, _)| i as u32 + 1).collect();
        assert_eq!(rr, vec![32, 35, 38]);
        assert!(v.iter().all(|x| x.3 == 0 || x.3 == 3));
        assert_eq!(m.update(61, 0, &mut || 0, overview().0, overview().1), 8);
        assert!(!m.card_touched());
    }

    #[test]
    fn feather_blend_and_dust_alpha() {
        let (_, v) = run(0, 3);
        assert!(v.iter().all(|x| x.5 == 0));
        let (_, w) = run(1, 3);
        // Param 1: the feather is the ALPHA 0x88 list (one quad per update).
        assert!(w.iter().all(|x| x.5 == 2));
        // Dust alpha 80 for u 17-21, then 80 - 2 (u - 21), 42 at u 40.
        let (eye, target) = trap(2, 1, 0);
        let mut rng = Rng::new(5);
        let mut m = C507::new([-50.0, 0.0, -100.0], 0, &mut || rng.next());
        for u in 1..=40 {
            m.update(u, 0, &mut || rng.next(), eye, target);
            let d: Vec<u8> = m.additive.iter().filter(|t| t.sheet == 15).map(|t| t.colour[0][3]).collect();
            if u >= 17 {
                let want = if u <= 21 { 80 } else { 80 - 2 * (u - 21) as u8 };
                assert!(d.iter().all(|&a| a == want), "u {u} {d:?}");
            } else {
                assert!(d.is_empty());
            }
        }
    }
}
