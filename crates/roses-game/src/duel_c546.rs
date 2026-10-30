//! CMODULE c546 (0x222), Widespread Ruin's per-cell blast (event 0x2E,
//! handler 0x2671B0: nine copies, the trap's own cell with param 1), as
//! specified in `claude-activate/NEWLY-ALLOWED.md` section 11 [F code; emu:
//! the reference model `tools/mf_c546_model.py` equals the interpreter's
//! primitives to 0.0625 px in seven runs]. The target is a CELL word: every
//! node sits at C = (50 (col - 3), 0, 50 (row - 3)). Layers in draw order:
//! - 60 orange POINTS (x 1.02, alpha 128 -10/U) on a billboard at C + (0, 3,
//!   0), scale 0.1 .. 1.1 (param 1: 1.1 .. 2.1), additive;
//! - eight grey-blue ROCKS (TEXEFF 7) thrown in arcs, tumbling 30 deg/U,
//!   drawn while above the node's plane, ALPHA 0x88 (opaque modulate);
//! - two brown-red cone RINGS (alpha 96 -16/U), world axes, additive;
//! - EMITTER A (TEXEFF 0 explosion, alpha 128 -8/U) and EMITTER B (TEXEFF 15
//!   smoke, positions x 1.01/U, alpha 96 -2/U), additive.
//! Param 1: node D scale 2, emitter A base (0, 20, 20), point half 1.5,
//! points / rings start at 1.1 with limit 2.0.
//!
//! Returns 2 at u 6 (the handler starts this cell's unit fade at u 7 =
//! E+44), 8 at u 60. No SE (the handler plays 0x31 once at its counter 1 and
//! four times at 5). 296 random values at init (rocks 24, emitter A 16, ring
//! A 30, ring B 30, points 180, emitter B 16), 2 per emitter respawn. Nine
//! instances launch in one update (centre param 1 first, then dc -1..1
//! outer, dr -1..1 inner), sharing the stream in that order.
//!
//! [H / not reproduced]: the port's random values (the caller's stream, same
//! order and count); the sine table (see `duel_fxlib`); no OT keys (the
//! original depth-sorts by mean w; the rocks would be depth-tested).
use crate::duel_c621::FxTri;
use crate::duel_fxlib::{cell_centre, cosf, decay, quad, rmod, sinf, uvn, Cam, PSys, DEG, SHEET_128X64, SHEET_256};

/// Module life.
pub const UPDATES: u32 = 60;
/// The update returning 2 (the handler starts the listed unit's fade at
/// u 7).
pub const RET2_U: u32 = 6;
/// Random values drawn at init.
pub const INIT_RAND: u32 = 296;
/// The handler's own SE (bank 1): once at E+39 (u 2), four times at E+43
/// (u 6) [F 0x26775C..0x267798].
pub const HANDLER_SE: u16 = 0x31;
const POINT_RGB: [u8; 3] = [128, 96, 64];
const ROCK_RGB: [u8; 3] = [96, 112, 112];
const RING_RGB: [u8; 3] = [160, 96, 80];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rock {
    spd: f32,
    sa: f32,
    ca: f32,
    sb: f32,
    cb: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct C546 {
    centre: [f32; 3],
    big: bool,
    rocks: [Rock; 8],
    rock_a: u8,
    rock_rot: i32,
    rock_t: i32,
    /// Ring A, B: (inner, outer) entries 0..30 (30 unused: segment 29 closes
    /// on 0).
    rings: [[([f32; 3], [f32; 3]); 31]; 2],
    ring_a: [u8; 2],
    pts: [[f32; 3]; 60],
    pt_half: f32,
    pt_a: u8,
    ea: PSys,
    eb: PSys,
    s_d: f32,
    s70: f32,
    se0: f32,
    lim: f32,
    age: u32,
    life: i32,
    done: bool,
    rand_calls: u32,
    /// (points, rocks, ring A, ring B, emitter A, emitter B) quads of the last
    /// update.
    counts: [usize; 6],
    drawn: (f32, f32, u8, u8, [u8; 2], u8, u8),
    /// Additive (ALPHA 0x48) triangles of the last update: points, rings,
    /// emitters.
    pub additive: Vec<FxTri>,
    /// Kept empty (the convention of the other modules).
    pub subtractive: Vec<FxTri>,
    /// ALPHA 0x88 triangles of the last update (colour = texel x vertex /
    /// 128, alpha cut): the rocks.
    pub opaque: Vec<FxTri>,
    pub alpha_blend: Vec<FxTri>,
}

impl C546 {
    /// `cell` = (col, row); `param` 1 = the trap's own cell. `rng` = the
    /// caller's stream (296 draws, order: rocks 8 x (speed, a, b), emitter A
    /// 8 x (x, y), ring A 30, ring B 30, points 60 x 3, emitter B 8 x (x, y)).
    pub fn new(cell: (u8, u8), param: u8, rng: &mut impl FnMut() -> u32) -> Self {
        let mut n = 0u32;
        let mut r = || {
            n += 1;
            rng()
        };
        let big = param == 1;
        let rocks = std::array::from_fn(|_| {
            let spd = 0.1f32 * (rmod(r(), 11) + 5) as f32;
            let a = DEG * rmod(r(), 360) as f32;
            let b = DEG * (rmod(r(), 50) + 40) as f32;
            Rock { spd, sa: sinf(a), ca: cosf(a), sb: sinf(b), cb: cosf(b) }
        });
        let ya = if big { 20.0 } else { 10.0 };
        let ea = PSys::lib(&mut r, 7, [0.0, ya, ya], [32, 32], [0, 0], 4, 0, SHEET_128X64, 20.0, [128, 128, 128, 128], 8, 3, 1, 8, 10, 10, |v| (v & 15) as i32 - 4);
        let mut ring = |rin: f32, rout: f32, y0: f32, h: f32| {
            let mut e = [([0.0f32; 3], [0.0f32; 3]); 31];
            e[0] = ([rin, y0, 0.0], [rout, y0 + h, 0.0]);
            for k in 1..31 {
                let ang = 0.209_439_5f32 * k as f32;
                let (c, s) = (cosf(ang), sinf(ang));
                let j = (rmod(r(), (2.0f32 * h) as i32) - h as i32) >> 1;
                e[k] = ([rin * c, y0, rin * s], [rout * c, (y0 + h) + j as f32, rout * s]);
            }
            e
        };
        let rings = [ring(0.1, 24.0, 0.0, 28.0), ring(5.0, 30.0, 2.0, 10.0)];
        let pts = std::array::from_fn(|_| {
            let x = rmod(r(), 50) - 25;
            let y = rmod(r(), 25);
            let z = rmod(r(), 50) - 25;
            [x as f32, y as f32, z as f32]
        });
        let eb = PSys::lib(&mut r, 0x85, [0.0, 20.0, 0.0], [16, 16], [240, 0], 16, 15, SHEET_256, 10.0, [128, 128, 128, 96], 8, 3, 1, 8, 30, 30, |v| (v & 15) as i32 - 4);
        Self {
            centre: cell_centre(i32::from(cell.0), i32::from(cell.1)),
            big,
            rocks,
            rock_a: 128,
            rock_rot: 0,
            rock_t: 0,
            rings,
            ring_a: [96, 96],
            pts,
            pt_half: if big { 1.5 } else { 1.0 },
            pt_a: 128,
            ea,
            eb,
            s_d: if big { 2.0 } else { 1.0 },
            s70: if big { 1.1 } else { 0.1 },
            se0: if big { 1.1 } else { 0.1 },
            lim: if big { 2.0 } else { 1.0 },
            age: 0,
            life: UPDATES as i32,
            done: false,
            rand_calls: n,
            counts: [0; 6],
            drawn: (0.0, 0.0, 0, 0, [0; 2], 0, 0),
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

    /// Param 1 (the trap's own cell).
    pub fn big(&self) -> bool {
        self.big
    }

    /// Quads of the last update: (points, rocks, ring A, ring B, emitter A,
    /// emitter B).
    pub fn counts(&self) -> [usize; 6] {
        self.counts
    }

    /// Drawn in the last update: (points scale, rings scale, points alpha,
    /// rocks alpha, rings alphas, emitter A alpha, emitter B alpha).
    pub fn drawn(&self) -> (f32, f32, u8, u8, [u8; 2], u8, u8) {
        self.drawn
    }

    /// Module update `u` (1, 2, ..) with the caller's stream (emitter
    /// respawns) and the camera. Returns 0, 2 at u 6, 8 at u 60 (and every
    /// later call).
    pub fn update(&mut self, _u: u32, rng: &mut impl FnMut() -> u32, eye: [f64; 3], target: [f64; 3]) -> u32 {
        self.additive.clear();
        self.opaque.clear();
        self.counts = [0; 6];
        if self.done {
            return 8;
        }
        let cam = Cam::new(eye, target);
        let c = self.centre;
        self.drawn = (self.s70, self.se0, self.pt_a, self.rock_a, self.ring_a, self.ea.a, self.eb.a);
        // POINTS: billboard at C + (0, 3, 0), scale s70.
        let o70 = [c[0], c[1] + 3.0, c[2]];
        let uv = uvn([[176, 64], [207, 64], [176, 95], [207, 95]], SHEET_256);
        let col = [POINT_RGB[0], POINT_RGB[1], POINT_RGB[2], self.pt_a];
        for i in 0..60 {
            self.pts[i] = self.pts[i].map(|v| v * 1.02);
            let [x, y, z] = self.pts[i];
            let h = self.pt_half;
            let l = [[x - h, y + h, z], [x + h, y + h, z], [x - h, y - h, z], [x + h, y - h, z]];
            if let Some(v) = cam.quad(l.map(|l| cam.bill(o70, l, [self.s70; 3]))) {
                quad(&mut self.additive, v, uv, [col; 4], true, 15);
                self.counts[0] += 1;
            }
        }
        self.pt_a = decay(self.pt_a, 10);
        // ROCKS (0x1E01B90): ballistic in node D's billboard frame.
        let grav = f32::from_bits(0x3EA7_40DA);
        let tf = self.rock_t as f32;
        let uv = uvn([[64, 0], [95, 0], [64, 31], [95, 31]], SHEET_128X64);
        let col = [ROCK_RGB[0], ROCK_RGB[1], ROCK_RGB[2], self.rock_a];
        for k in &self.rocks {
            let y = ((7.0f32 * tf) * k.sb) - ((grav * tf) * tf) / 2.0;
            let z = k.ca * ((7.0f32 * tf) * k.cb);
            let x = k.sa * ((7.0f32 * tf) * k.cb);
            let l = [0x87, 0x2D, 0xE1, 0x13B].map(|off| {
                let r = DEG * ((self.rock_rot + off) % 360) as f32;
                [x + k.spd * (2.5f32 * cosf(r)), y + k.spd * (2.5f32 * sinf(r)), z]
            });
            if y <= 0.0 {
                continue;
            }
            if let Some(v) = cam.quad(l.map(|l| cam.bill(c, l, [self.s_d; 3]))) {
                quad(&mut self.opaque, v, uv, [col; 4], true, 7);
                self.counts[1] += 1;
            }
        }
        self.rock_t += 1;
        self.rock_a = decay(self.rock_a, 3);
        self.rock_rot += 30;
        // RINGS (world axes, scale sE0).
        let uv = uvn([[142, 64], [142, 0], [174, 64], [174, 0]], SHEET_256);
        let s = self.se0;
        let w = |p: [f32; 3]| -> [f64; 3] { std::array::from_fn(|i| f64::from(c[i]) + f64::from(s) * f64::from(p[i])) };
        for j in 0..2 {
            let col = [RING_RGB[0], RING_RGB[1], RING_RGB[2], self.ring_a[j]];
            for k in 0..30 {
                let k1 = (k + 1) % 30;
                let (i0, o0) = self.rings[j][k];
                let (i1, o1) = self.rings[j][k1];
                if let Some(v) = cam.quad([w(i0), w(o0), w(i1), w(o1)]) {
                    quad(&mut self.additive, v, uv, [col; 4], true, 15);
                    self.counts[2 + j] += 1;
                }
            }
            self.ring_a[j] = decay(self.ring_a[j], 16);
        }
        self.se0 += if self.se0 < self.lim { 0.2 } else { 0.05 };
        if self.s70 < self.lim {
            self.s70 += 0.2;
        }
        // EMITTERS (node D billboard, scale sD).
        self.counts[4] = self.ea.draw(&cam, rng, c, self.s_d, None, &mut self.additive);
        self.ea.a = decay(self.ea.a, 8);
        self.counts[5] = self.eb.draw(&cam, rng, c, self.s_d, Some(1.01), &mut self.additive);
        self.eb.a = decay(self.eb.a, 2);
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

/// The nine launches of the handler at E+38: (cell, param) in launch order —
/// the trap's cell with param 1, then dc -1..1 (outer), dr -1..1 (inner),
/// centre skipped, off-board cells skipped [F 0x267478..0x267498].
pub fn launches(trap: (u8, u8)) -> Vec<((u8, u8), u8)> {
    let mut v = vec![(trap, 1)];
    for dc in -1i32..=1 {
        for dr in -1i32..=1 {
            if dc == 0 && dr == 0 {
                continue;
            }
            let (c, r) = (i32::from(trap.0) + dc, i32::from(trap.1) + dr);
            if (0..7).contains(&c) && (0..7).contains(&r) {
                v.push(((c as u8, r as u8), 0));
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duel_fxlib::tests::{overview, trap};
    use crate::duel_fxlib::Rng;

    fn run(cell: (u8, u8), param: u8, eye: [f64; 3], target: [f64; 3], seed: u32) -> (C546, Vec<(u32, [usize; 6], (f32, f32, u8, u8, [u8; 2], u8, u8), u32)>) {
        let mut rng = Rng::new(seed);
        let mut m = C546::new(cell, param, &mut || rng.next());
        assert_eq!(rng.calls, INIT_RAND);
        let mut v = Vec::new();
        for u in 1..=UPDATES {
            let n0 = rng.calls;
            let r = m.update(u, &mut || rng.next(), eye, target);
            v.push((r, m.counts(), m.drawn(), rng.calls - n0));
        }
        (m, v)
    }

    #[test]
    fn returns_rand_and_counts() {
        // (On the trap camera the grown rings leave the screen near the end.)
        let (eye, target) = overview();
        let (mut m, v) = run((3, 5), 0, eye, target, 7);
        let (te, tt) = trap(3, 5, 0);
        let (_, w) = run((3, 5), 0, te, tt, 7);
        assert!(w[..40].iter().all(|x| x.1[0] == 60 && x.1[2] + x.1[3] == 60));
        assert_eq!(m.rand_calls(), INIT_RAND);
        let rets: Vec<_> = v.iter().enumerate().filter(|(_, x)| x.0 != 0).map(|(i, x)| (i as u32 + 1, x.0)).collect();
        assert_eq!(rets, vec![(6, 2), (60, 8)]);
        // 60 point + 60 ring quads every update (all on screen here).
        for (i, x) in v.iter().enumerate() {
            assert_eq!(x.1[0], 60, "u {}", i + 1);
            assert_eq!(x.1[2] + x.1[3], 60, "u {}", i + 1);
        }
        // No rocks at u 1 (t = 0, y = 0).
        assert_eq!(v[0].1[1], 0);
        assert!(v[1].1[1] > 0);
        // Emitter puff counts 1, 1, 2 at u 1-3, 8 from u 21.
        assert_eq!((v[0].1[4], v[1].1[4], v[2].1[4]), (1, 1, 2));
        assert!(v[20..].iter().all(|x| x.1[4] == 8 && x.1[5] == 8));
        // Respawns draw 2 values each.
        assert!(v.iter().all(|x| x.3 % 2 == 0) && v.iter().any(|x| x.3 > 0));
        assert_eq!(m.update(61, &mut || 0, eye, target), 8);
        assert!(m.additive.is_empty() && m.opaque.is_empty());
    }

    #[test]
    fn alphas_and_scales() {
        let (eye, target) = overview();
        let (_, v) = run((2, 4), 0, eye, target, 3);
        let d = |u: usize| v[u - 1].2;
        // Points alpha 128 - 10 (u - 1), 0 from u 14; rocks 128 - 3 (u - 1).
        assert_eq!((d(1).2, d(13).2, d(14).2), (128, 8, 0));
        assert_eq!((d(2).3, d(43).3), (125, 2));
        // Rings 96 - 16 (u - 1), 0 from u 7; emitter A 0 from u 17, B 0 from u 49.
        assert_eq!((d(6).4, d(7).4), ([16, 16], [0, 0]));
        assert_eq!((d(16).5, d(17).5, d(48).6, d(49).6), (8, 0, 2, 0));
        // sE0 0.1 .. 1.1 (u 1-6), 1.15 at u 7; points 1.1 from u 6.
        assert!((d(1).1 - 0.1).abs() < 1e-6 && (d(6).1 - 1.1).abs() < 1e-5 && (d(7).1 - 1.15).abs() < 1e-5);
        assert!((d(60).1 - 3.8).abs() < 1e-3);
        assert!((d(6).0 - 1.1).abs() < 1e-5 && (d(30).0 - 1.1).abs() < 1e-5);
        // Param 1: points / rings from 1.1 with limit 2.0 (2.1 at u 6).
        let (_, v) = run((3, 5), 1, eye, target, 3);
        assert!((v[0].2 .1 - 1.1).abs() < 1e-6 && (v[5].2 .1 - 2.1).abs() < 1e-4 && (v[6].2 .1 - 2.15).abs() < 1e-4);
        assert!((v[59].2 .1 - 4.8).abs() < 2e-3);
    }

    #[test]
    fn rocks_are_opaque_texeff7_and_land() {
        let (eye, target) = overview();
        let (_, v) = run((3, 3), 0, eye, target, 9);
        assert!(v.iter().all(|x| x.1[1] <= 8));
        // Every rock is gone after t ~ 42.86 sin b (<= 43): none from u 45.
        assert!(v[44..].iter().all(|x| x.1[1] == 0));
        let mut rng = Rng::new(9);
        let mut m = C546::new((3, 3), 0, &mut || rng.next());
        m.update(1, &mut || rng.next(), eye, target);
        m.update(2, &mut || rng.next(), eye, target);
        assert!(m.opaque.iter().all(|t| t.sheet == 7 && t.colour[0] == [96, 112, 112, 125]));
    }

    #[test]
    fn nine_launches_in_order() {
        let l = launches((3, 5));
        assert_eq!(l.len(), 9);
        assert_eq!(l[0], ((3, 5), 1));
        assert_eq!(l[1], ((2, 4), 0));
        assert_eq!(l[2], ((2, 5), 0));
        assert_eq!(l[8], ((4, 6), 0));
        // A corner keeps the on-board cells only.
        assert_eq!(launches((0, 0)).len(), 4);
        // Shared stream: 9 x 296 draws at launch.
        let mut rng = Rng::new(1);
        let v: Vec<C546> = l.iter().map(|&(c, p)| C546::new(c, p, &mut || rng.next())).collect();
        assert_eq!(rng.calls, 9 * INIT_RAND);
        assert!(v[0].big() && !v[1].big());
    }
}
