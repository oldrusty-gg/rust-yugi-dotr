//! CMODULE c555 (0x22B), Mirror Force / Mirror Wall (event 0x36, handler
//! 0x26CDB0, one copy per target), as specified in
//! `claude-activate/NEWLY-ALLOWED.md` section 9 [F code; emu: the reference
//! model `tools/mf_c555_model.py` equals the interpreter's primitives to
//! 0.0625 px in four runs]. World-anchored at the target's card position T
//! (its cell centre, y 0), all additive (ALPHA 0x48):
//! - four ORBS start 150 out on the four sides of T, 50 up, and home in
//!   (0x103EC0, 0x800 per Euler axis); each flying orb draws its TRAIL (six
//!   tapering untextured quads over an 8-point history, the half-vector
//!   applied twice through its rotation, plus the original's end triangle
//!   next to the world origin) and its DART (a 4-point untextured shape
//!   turned to the screen direction of flight);
//! - all four arrive (< 20 from T) at u 7: one full-screen FLASH each;
//! - phase B from u 8 (drawn once): SMOKE (16 particles, TEXEFF 15
//!   (128..255, 192..255)) at orb 3's final position, 60 POINTS (x 1.1 per
//!   update) on a billboard at (T.x, 3, T.z), three cone RINGS growing 0.05/U.
//!
//! Colour (128, 80, 80). Returns 2 at u 7 (fixed age test), 8 at u 25. SE
//! bank 1 0x52 at u 1. 286 random values at init (rings 3 x 30, smoke 8 x
//! (x, y), points 60 x 3), 3 per smoke respawn (27 at u 15 and u 23). No
//! unit / card / light side effects.
//!
//! [H / not reproduced]: the port's random values (the caller's stream, same
//! call order and count); the sine table (see `duel_fxlib`); no OT keys (the
//! original keys world layers by mean w); a failed DART projection keeps the
//! last valid pair (the original reuses stale stack values).
use crate::duel_c621::FxTri;
use crate::duel_fxlib::{ang16, cosf, decay, dist, flash, homing, quad, rotm, sinf, tcos, tsin, uvn, vmul, Cam, Gs, PSys, Particle, SHEET_256};

/// Module life.
pub const UPDATES: u32 = 25;
/// The update returning 2 (Mirror Wall: c557 at u 8; Mirror Force: the
/// victim's c603 16 U later).
pub const RET2_U: u32 = 7;
/// The update the four orbs arrive (four full-screen flashes).
pub const ARRIVE_U: u32 = 7;
/// SE (SECTL bank 1) played at `SE_U`.
pub const SE_BANK: u8 = 1;
pub const SE_ID: u16 = 0x52;
pub const SE_U: u32 = 1;
/// Random values drawn at init.
pub const INIT_RAND: u32 = 286;
const RGB: [u8; 3] = [128, 80, 80];
const SHEET: u8 = 15;
/// The dart template (data 0x1E03380 x 10).
const DART: [[f32; 3]; 4] = [[-10.0, 10.0, 0.0], [0.0, -20.0, 0.0], [0.0, 0.0, 0.0], [10.0, 10.0, 0.0]];

#[derive(Clone, Debug, PartialEq)]
struct Ring {
    ent: [([f32; 3], [f32; 3]); 31],
    a: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct C555 {
    t: [f32; 3],
    pos: [[f32; 3]; 4],
    vel: [[f32; 3]; 4],
    hist: [[[f32; 3]; 8]; 4],
    arrived: [bool; 4],
    stale: [Option<Gs>; 2],
    rings: [Ring; 3],
    ring_s: f32,
    pts: [[f32; 3]; 60],
    pts_a: u8,
    pts_s: f32,
    smoke: PSys,
    age: u32,
    life: i32,
    done: bool,
    flashes: usize,
    smoke_drawn: usize,
    drawn_pts_s: f32,
    drawn_ring_s: f32,
    rand_calls: u32,
    /// Additive (ALPHA 0x48) world triangles of the last update: trails
    /// (incl. the end triangles), darts, smoke, points, rings.
    pub additive: Vec<FxTri>,
    /// Kept empty (the convention of the other modules).
    pub subtractive: Vec<FxTri>,
    pub opaque: Vec<FxTri>,
    pub alpha_blend: Vec<FxTri>,
    /// Full-screen flashes of the last update (Z max, key 0: over
    /// everything), additive.
    pub flash: Vec<FxTri>,
}

impl C555 {
    /// Init: `t` = the target's card position (cell centre, y 0); `rng` = the
    /// caller's stream (286 draws: rings 3 x 30, smoke 8 x (x, y), points 60
    /// x 3).
    pub fn new(t: [f32; 3], rng: &mut impl FnMut() -> u32) -> Self {
        let mut n = 0u32;
        let mut r = || {
            n += 1;
            rng()
        };
        let rings = [(68.0f32, 56i32), (74.0, 42), (80.0, 28)].map(|(rr, h)| {
            let mut ent = [([0.0f32; 3], [0.0f32; 3]); 31];
            ent[0] = ([0.1, 0.0, 0.0], [rr, h as f32, 0.0]);
            for k in 1..31 {
                let a = 0.209_439_52f32 * k as f32;
                let j = ((r() % (2 * h) as u32) as i32 - h) >> 1;
                ent[k] = ([0.1 * cosf(a), 0.0, 0.1 * sinf(a)], [rr * cosf(a), h as f32 + j as f32, rr * sinf(a)]);
            }
            Ring { ent, a: 96 }
        });
        let mut p: Vec<Particle> = (0..8)
            .map(|i| {
                let x = (r() % 60) as f32 - 30.0;
                let y = (r() % 30) as f32;
                Particle { x, y, z: 0.0, f: 0, t: -i }
            })
            .collect();
        p.extend((0..8).map(|_| Particle { x: 0.0, y: 0.0, z: 0.0, f: 0, t: 0 }));
        let smoke = PSys {
            flags: 7,
            base: [0.0; 3],
            cell: [32, 32],
            uvb: [128, 192],
            per: 4,
            sheet: SHEET,
            sheet_size: SHEET_256,
            half: 20.0,
            rgb: RGB,
            a: 96,
            hold: 0,
            max_frame: 7,
            wrap: 8,
            rx: 30,
            ry: -30,
            p,
        };
        let pts = std::array::from_fn(|_| {
            let x = (r() % 50) as f32 - 25.0;
            let y = (r() % 25) as f32;
            let z = (r() % 50) as f32 - 25.0;
            [x, y, z]
        });
        let pos: [[f32; 3]; 4] = std::array::from_fn(|k| {
            let a = 0x4000 * k as i32;
            [t[0] + 150.0 * tcos(a), t[1] + 50.0, t[2] + 150.0 * tsin(a)]
        });
        let vel = std::array::from_fn(|k| {
            let prev = pos[(k + 3) % 4];
            std::array::from_fn(|i| (prev[i] - pos[k][i]) / 10.0)
        });
        Self {
            t,
            pos,
            vel,
            hist: pos.map(|p| [p; 8]),
            arrived: [false; 4],
            stale: [None; 2],
            rings,
            ring_s: 1.0,
            pts,
            pts_a: 128,
            pts_s: 0.1,
            smoke,
            age: 0,
            life: UPDATES as i32,
            done: false,
            flashes: 0,
            smoke_drawn: 0,
            drawn_pts_s: 0.1,
            drawn_ring_s: 1.0,
            rand_calls: n,
            additive: Vec::new(),
            subtractive: Vec::new(),
            opaque: Vec::new(),
            alpha_blend: Vec::new(),
            flash: Vec::new(),
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// Random values drawn at init (286).
    pub fn rand_calls(&self) -> u32 {
        self.rand_calls
    }

    /// The orbs' world positions.
    pub fn orbs(&self) -> [[f32; 3]; 4] {
        self.pos
    }

    pub fn arrived(&self, k: usize) -> bool {
        self.arrived[k]
    }

    /// Full-screen flashes sent in the last update (4 at u 7).
    pub fn flashes(&self) -> usize {
        self.flashes
    }

    /// Smoke quads sent in the last update.
    pub fn smoke_count(&self) -> usize {
        self.smoke_drawn
    }

    /// (points scale, ring scale) drawn in the last update.
    pub fn scales(&self) -> (f32, f32) {
        (self.drawn_pts_s, self.drawn_ring_s)
    }

    /// (smoke, points, rings) alphas now (after the last update's decays).
    pub fn alphas(&self) -> (u8, u8, u8) {
        (self.smoke.a, self.pts_a, self.rings[0].a)
    }

    /// The trail's half-vector at history point `i` of width `n`: (5n/8, 0,
    /// 0) through M twice (0x20C340 with axis angle 0 leaves M in place).
    fn half(h: &[[f32; 3]; 8], i: usize, n: f32) -> [f32; 3] {
        let d: [f32; 3] = std::array::from_fn(|j| h[i + 1][j] - h[i][j]);
        let yaw = ang16(d[0], d[2]);
        let m = rotm(ang16((d[0] * d[0] + d[2] * d[2]).sqrt(), d[1]), yaw, 0);
        let w = (5.0f32 / 8.0) * n;
        vmul(vmul([w, 0.0, 0.0], m), m)
    }

    /// 0x1E01D20: history shift, six quads, the end triangle.
    fn trail(&mut self, cam: &Cam, k: usize) {
        let h = &mut self.hist[k];
        h.copy_within(0..7, 1);
        h[0] = self.pos[k];
        let h = *h;
        for s in 0..6usize {
            let (a, t0) = (96i32, 12i32);
            let si = s as i32;
            let a01 = if s == 0 { 0 } else { (a - si * t0) as u8 };
            let a23 = (a - (si + 1) * t0) as u8;
            let r0 = Self::half(&h, s, (8 - s) as f32);
            let r1 = Self::half(&h, s + 1, (7 - s) as f32);
            let w = [
                std::array::from_fn(|j| f64::from(h[s][j] + r0[j])),
                std::array::from_fn(|j| f64::from(h[s][j] - r0[j])),
                std::array::from_fn(|j| f64::from(h[s + 1][j] + r1[j])),
                std::array::from_fn(|j| f64::from(h[s + 1][j] - r1[j])),
            ];
            if let Some(v) = cam.quad(w) {
                let c = |a: u8| [RGB[0], RGB[1], RGB[2], a];
                quad(&mut self.additive, v, [[0.0; 2]; 4], [c(a01), c(a01), c(a23), c(a23)], false, 0);
            }
        }
        // End triangle (flags 0x213, an original bug): vertices 0 and 1 are
        // the projection of the half-vector itself (next to the origin).
        let r = Self::half(&h, 6, 2.0);
        if let (Some(g0), Some(g2)) = (cam.gs32(r), cam.gs32(h[7])) {
            let c = |a: u8| [RGB[0], RGB[1], RGB[2], a];
            self.additive.push(FxTri { screen: [g0.px(), g0.px(), g2.px()], uv: [[0.0; 2]; 3], colour: [c(48), c(12), c(0)], textured: false, sheet: 0 });
        }
    }

    /// 0x1037C0: the screen angle of h[0] - h[1] (GS integer deltas >> 4).
    fn screen_angle(&mut self, cam: &Cam, k: usize) -> i32 {
        if let Some(g) = cam.gs32(self.hist[k][0]) {
            self.stale[0] = Some(g);
        }
        if let Some(g) = cam.gs32(self.hist[k][1]) {
            self.stale[1] = Some(g);
        }
        let (Some(g0), Some(g1)) = (self.stale[0], self.stale[1]) else {
            return 0;
        };
        let dx = ((g0.x & 0xFFFF) - (g1.x & 0xFFFF)) >> 4;
        let dy = ((g0.y & 0xFFFF) - (g1.y & 0xFFFF)) >> 4;
        ang16(dx as f32, dy as f32)
    }

    /// 0x1E02570: the dart, turned by `rot` about the view axis, billboard.
    fn dart(&mut self, cam: &Cam, k: usize, rot: i32) {
        let m = rotm(0, 0, rot);
        let w = DART.map(|p| cam.bill(self.pos[k], vmul(p, m), [1.0; 3]));
        if let Some(v) = cam.quad(w) {
            let c = |a: u8| [RGB[0], RGB[1], RGB[2], a];
            quad(&mut self.additive, v, [[0.0; 2]; 4], [c(32), c(128), c(32), c(32)], false, 0);
        }
    }

    fn points(&mut self, cam: &Cam, scale: f32) {
        let node = [self.t[0], 3.0, self.t[2]];
        let uv = [[0.8125, 0.25], [0.9375, 0.25], [0.8125, 0.375], [0.9375, 0.375]];
        let c = [RGB[0], RGB[1], RGB[2], self.pts_a];
        for i in 0..60 {
            self.pts[i] = self.pts[i].map(|v| v * 1.1);
            let [x, y, z] = self.pts[i];
            let h = 2.5f32;
            let l = [[x - h, y + h, z], [x + h, y + h, z], [x - h, y - h, z], [x + h, y - h, z]];
            if let Some(v) = cam.quad(l.map(|l| cam.bill(node, l, [scale; 3]))) {
                quad(&mut self.additive, v, uv, [c; 4], true, SHEET);
            }
        }
    }

    fn rings_draw(&mut self, cam: &Cam, scale: f32) {
        let uv = uvn([[142, 64], [142, 0], [174, 64], [174, 0]], SHEET_256);
        let t = self.t;
        let w = |p: [f32; 3]| -> [f64; 3] { std::array::from_fn(|i| f64::from(p[i] * scale + t[i])) };
        for g in 0..3 {
            let c = [RGB[0], RGB[1], RGB[2], self.rings[g].a];
            for s in 0..30 {
                let (i0, o0) = self.rings[g].ent[s];
                let (i1, o1) = if s < 29 { self.rings[g].ent[s + 1] } else { self.rings[g].ent[0] };
                if let Some(v) = cam.quad([w(i0), w(o0), w(i1), w(o1)]) {
                    quad(&mut self.additive, v, uv, [c; 4], true, SHEET);
                }
            }
            self.rings[g].a = decay(self.rings[g].a, 8);
        }
    }

    /// Module update `u` (1, 2, ..) with the caller's stream (smoke
    /// respawns) and the camera. Returns 0, 2 at u 7, 8 at u 25 (and every
    /// later call).
    pub fn update(&mut self, _u: u32, rng: &mut impl FnMut() -> u32, eye: [f64; 3], target: [f64; 3]) -> u32 {
        self.additive.clear();
        self.flash.clear();
        self.flashes = 0;
        self.smoke_drawn = 0;
        if self.done {
            return 8;
        }
        let cam = Cam::new(eye, target);
        let (sp, sr) = (self.pts_s, self.ring_s);
        let t = self.t;
        for k in 0..4 {
            if !self.arrived[k] {
                let d: [f32; 3] = std::array::from_fn(|i| t[i] - self.pos[k][i]);
                let nv = homing(self.vel[k], d, 0x800);
                self.pos[k] = std::array::from_fn(|i| self.pos[k][i] + nv[i]);
                self.vel[k] = nv;
                self.trail(&cam, k);
                let rot = self.screen_angle(&cam, k);
                self.dart(&cam, k, rot);
            } else if k == 0 {
                let origin = self.pos[3];
                self.smoke_drawn = self.smoke.draw(&cam, rng, origin, 1.0, None, &mut self.additive);
                self.smoke.a = decay(self.smoke.a, 8);
                if self.pts_s < 1.0 {
                    self.pts_s += 0.2;
                }
                self.points(&cam, sp);
                self.pts_a = decay(self.pts_a, 10);
                self.rings_draw(&cam, sr);
                self.ring_s += if self.ring_s < 1.0 { 0.2 } else { 0.05 };
                self.drawn_pts_s = sp;
                self.drawn_ring_s = sr;
            }
            if dist(t, self.pos[k]) < 20.0 {
                if !self.arrived[k] {
                    flash(&mut self.flash);
                    self.flashes += 1;
                }
                self.arrived[k] = true;
            }
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
    use crate::duel_fxlib::tests::trap;
    use crate::duel_fxlib::{cell_centre, Rng};

    fn run(col: i32, row: i32, side: u8, seed: u32) -> (C555, Rng, Vec<(u32, usize, usize, f32, f32, (u8, u8, u8), u32, usize)>) {
        let (eye, target) = trap(col, row, side);
        let mut rng = Rng::new(seed);
        let mut m = C555::new(cell_centre(col, row), &mut || rng.next());
        let mut v = Vec::new();
        for u in 1..=UPDATES {
            let n0 = rng.calls;
            let r = m.update(u, &mut || rng.next(), eye, target);
            let (ps, rs) = m.scales();
            v.push((r, m.flashes(), m.smoke_count(), ps, rs, m.alphas(), rng.calls - n0, m.additive.len()));
        }
        (m, rng, v)
    }

    #[test]
    fn rand_returns_and_flashes() {
        let (mut m, _, v) = run(3, 5, 0, 7);
        assert_eq!(m.rand_calls(), INIT_RAND);
        let rets: Vec<_> = v.iter().enumerate().filter(|(_, x)| x.0 != 0).map(|(i, x)| (i as u32 + 1, x.0)).collect();
        assert_eq!(rets, vec![(7, 2), (25, 8)]);
        for (i, x) in v.iter().enumerate() {
            assert_eq!(x.1, if i + 1 == 7 { 4 } else { 0 }, "u {}", i + 1);
        }
        let (eye, target) = trap(3, 5, 0);
        assert!(m.done());
        assert_eq!(m.update(26, &mut || 0, eye, target), 8);
        assert!(m.additive.is_empty() && m.flash.is_empty());
    }

    #[test]
    fn orb_path_and_arrival() {
        let t = cell_centre(3, 5);
        let mut rng = Rng::new(3);
        let mut m = C555::new(t, &mut || rng.next());
        let (eye, target) = trap(3, 5, 0);
        // Velocities (-15, 0, -15), (15, 0, -15), (15, 0, 15), (-15, 0, 15).
        assert_eq!(m.vel, [[-15.0, 0.0, -15.0], [15.0, 0.0, -15.0], [15.0, 0.0, 15.0], [-15.0, 0.0, 15.0]]);
        let d0 = [140.8f32, 121.6, 100.9, 79.7, 58.5, 37.3, 16.1];
        for u in 1..=7 {
            m.update(u, &mut || rng.next(), eye, target);
            let o = m.orbs()[0];
            if u == 1 {
                let rel = [o[0] - t[0], o[1] - t[1], o[2] - t[2]];
                for (a, b) in rel.iter().zip([133.33f32, 43.70, -11.50]) {
                    assert!((a - b).abs() < 0.02, "{rel:?}");
                }
            }
            assert!((dist(t, o) - d0[u as usize - 1]).abs() < 0.06, "u {u} {}", dist(t, o));
            assert_eq!((0..4).all(|k| m.arrived(k)), u == 7, "u {u}");
        }
        let ends = [[15.109f32, 4.718, -2.847], [3.802, 4.535, 14.359], [-15.109, 4.718, 2.847], [-3.802, 4.535, -14.359]];
        for k in 0..4 {
            for i in 0..3 {
                assert!((m.orbs()[k][i] - t[i] - ends[k][i]).abs() < 0.01, "orb {k} {:?}", m.orbs()[k]);
            }
        }
    }

    #[test]
    fn phase_b_counts_scales_and_alphas() {
        let (_, _, v) = run(3, 5, 0, 11);
        let at = |u: usize| &v[u - 1];
        // Smoke: 10 at u 8 rising to 16 at u 14; 27 rand at u 15 and u 23.
        assert_eq!(at(8).2, 10);
        assert_eq!(at(13).2, 15);
        for u in 14..=25 {
            assert_eq!(at(u).2, 16, "u {u}");
        }
        assert_eq!((at(15).6, at(23).6), (27, 27));
        // Else 3 per respawn: particle i (1..7) at u 15 + i, none before u 15.
        assert!(v[..14].iter().all(|x| x.6 == 0));
        assert_eq!((at(16).6, at(22).6, at(24).6), (3, 3, 3));
        // Points scale 0.1 at u 8, 1.1 at u 13; ring scale 1.0 + 0.05 (u - 8).
        assert!((at(8).3 - 0.1).abs() < 1e-6 && (at(13).3 - 1.1).abs() < 1e-5);
        assert!((at(8).4 - 1.0).abs() < 1e-6 && (at(20).4 - 1.6).abs() < 1e-4);
        // ptsA 8 drawn at u 20 (-> 0 after), 0 at u 21; ring A 0 drawn at u 20.
        assert_eq!(at(19).5 .1, 8);
        assert_eq!(at(20).5 .1, 0);
        assert_eq!(at(19).5 .2, 0);
        assert_eq!(at(18).5 .2, 8);
        // Nothing of phase B before u 8.
        assert!(v[..7].iter().all(|x| x.2 == 0));
    }

    #[test]
    fn trail_dart_and_layers_before_arrival() {
        let (eye, target) = trap(3, 5, 0);
        let mut rng = Rng::new(5);
        let mut m = C555::new(cell_centre(3, 5), &mut || rng.next());
        m.update(1, &mut || rng.next(), eye, target);
        // Per orb: six trail quads + the end triangle + the dart (all untextured).
        assert!(m.additive.iter().all(|t| !t.textured));
        assert!(m.additive.len() <= 4 * (6 * 2 + 1 + 2));
        // End triangle: alphas 48, 12, 0.
        assert!(m.additive.iter().any(|t| t.colour[0][3] == 48 && t.colour[1][3] == 12 && t.colour[2][3] == 0));
        // Dart alphas 32, 128, 32 (v0 v1 v2).
        assert!(m.additive.iter().any(|t| t.colour[0][3] == 32 && t.colour[1][3] == 128));
        // Phase B: textured TEXEFF 15, (128, 80, 80).
        for u in 2..=8 {
            m.update(u, &mut || rng.next(), eye, target);
        }
        assert!(m.additive.iter().filter(|t| t.textured).all(|t| t.sheet == 15 && t.colour[0][..3] == RGB));
        assert_eq!(m.additive.iter().filter(|t| t.textured).count() % 2, 0);
    }
}
