//! CMODULE c026 (0x1A), Change of Heart board-wide (event 0x1A, handler
//! 0x25E300, launched at E+1), as specified in
//! the project notes section 3 [F code; emu: the
//! reference model `tools/mf_c026_model.py` equals the interpreter's
//! primitives to 0.0628 px in two runs]. Anchored at the camera's look-at
//! point T (re-read every update: always the screen centre), all additive,
//! **Z forced to 0xFFFFFF** (never occluded):
//! - GLOW (slot 0, u 1-125): the mirrored "eye" emblem, a billboard at T of 4
//!   quadrant quads (80 x 40 units) of TEXEFF 15 (176,0)-(235,31), colour C1
//!   (64,32,48) turning blue (20,32,192) from u 29;
//! - RINGS (u 8-111): 8 rings born at u 8 + 10 i, each up to 10 annular bands
//!   of 12 jittered 20-degree arcs of TEXEFF 15 (128,129)-(255,192), in the
//!   screen plane (node scale (0.7, 0.2, 0.7), rot = camera + 0x4000 on x),
//!   flowing inward 3/U, turning -6 deg/U (counter-clockwise on screen),
//!   shrinking from u 11 + 10 i, colour C2 (128,0,0) turning blue from u 29.
//!
//! Returns 0, then 8 at u 126 (never 2). SE bank 1 0x21 at u 1. 76 random
//! values (60 at u 1, 2 per ring birth).
//!
//! The pool is kept as in the original (50 slots, lowest free slot, data kept
//! after death, parent / child as slot indices followed even when stale), and
//! the draw's write of the parent's outer radius over the band's inner radius
//! (vertex scratch in the slot) is reproduced.
//!
//! [H / not reproduced]: the port's random values (its own LCG, same call
//! order and count); Z = 0xFFFFFF and OT key = mean w: the port draws it after
//! the scene, in slot order, over everything.
use crate::duel_c022::{dcos, dsin, project4, quad, s16, uv15, Rng};
use crate::duel_c602::{billboard_axes, projector};
use crate::duel_c621::FxTri;

/// Module life (0x7E).
pub const LIFE: i32 = 126;
pub const END_U: u32 = 126;
pub const RET_DONE: u32 = 8;
/// SE (SECTL bank 1, record 0x1A) played at `SE_U`.
pub const SE_BANK: u8 = 1;
pub const SE_ID: u16 = 0x21;
pub const SE_U: u32 = 1;
/// Pool slots (D+0x140, 0x80 bytes each).
pub const SLOTS: usize = 50;
/// Rings (scales D+0x1A78).
pub const RINGS: usize = 8;
/// The band radius limit Rmax (D+0x1B4E).
const RMAX: f32 = 130.0;
/// Ring node scale.
const NODE: [f32; 3] = [0.7, 0.2, 0.7];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Kind {
    #[default]
    Glow,
    Band,
}

/// A 0x80-byte slot (zeroed on allocation, kept after death).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Slot {
    active: bool,
    kind: Kind,
    /// h_in (+0x14 / +0x24), h_out (+0x34 / +0x44).
    h_in: f32,
    h_out: f32,
    /// Scaled radii +0x74, +0x76 (s16).
    s_in: i32,
    s_out: i32,
    /// +0x6E (s16 degrees).
    a: i32,
    ring: usize,
    /// Angle row (+0x0C, f32 in the original).
    row: usize,
    /// +0x78 "spawn the next band" flag.
    flag: bool,
    parent: Option<usize>,
    child: Option<usize>,
    /// RGBA +0x64 / +0x68.
    c1: [u8; 4],
    c2: [u8; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct C026 {
    rng: Rng,
    cnt: i32,
    life: i32,
    /// C1 glow colour (D+0x1BE8), C2 ring colour (D+0x1BEC), D+0x1AD0.
    c1: [u8; 3],
    c2: [u8; 3],
    c1_frozen: bool,
    /// Ring scales S[i] (160 - 5/U).
    scale: [f32; RINGS],
    /// Angle rows T[k][j] (D+0x1AD2).
    rows: [[i32; 12]; 5],
    /// D+0x1A50 "last spawned" (the next band's parent).
    last: Option<usize>,
    slots: Vec<Slot>,
    done: bool,
    counts: [usize; 2],
    /// Additive triangles of the last update (glow, then bands; Z max).
    pub additive: Vec<FxTri>,
}

/// Spawn template of a band (D+0x1A58.. / D+0x80..).
struct BandSpawn {
    s_in: i32,
    s_out: i32,
    h: f32,
    ring: usize,
    row: usize,
    a: i32,
}

impl C026 {
    /// Init (`entry(D, param)`; the param is saved, unused). Draws nothing.
    pub fn new(seed: u32) -> Self {
        Self {
            rng: Rng::new(seed),
            cnt: 0,
            life: LIFE,
            c1: [64, 32, 48],
            c2: [128, 0, 0],
            c1_frozen: false,
            scale: [160.0; RINGS],
            rows: [[0; 12]; 5],
            last: None,
            slots: vec![Slot::default(); SLOTS],
            done: false,
            counts: [0; 2],
            additive: Vec::new(),
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    pub fn rand_calls(&self) -> u32 {
        self.rng.calls
    }

    /// Quads sent in the last update: [glow, ring arcs].
    pub fn counts(&self) -> [usize; 2] {
        self.counts
    }

    pub fn prims(&self) -> usize {
        self.counts[0] + self.counts[1]
    }

    /// (C1 glow, C2 ring) colours.
    pub fn colours(&self) -> ([u8; 3], [u8; 3]) {
        (self.c1, self.c2)
    }

    /// Live bands per ring.
    pub fn bands(&self) -> [usize; RINGS] {
        let mut n = [0; RINGS];
        for s in self.slots.iter().filter(|s| s.active && s.kind == Kind::Band) {
            n[s.ring] += 1;
        }
        n
    }

    /// Ring scale S[i].
    pub fn scale(&self, i: usize) -> f32 {
        self.scale[i]
    }

    fn alloc(&mut self, s: Slot) -> Option<usize> {
        let i = self.slots.iter().position(|s| !s.active)?;
        self.slots[i] = Slot { active: true, ..s };
        Some(i)
    }

    /// 0x1E01270.
    fn spawn_band(&mut self, t: BandSpawn) {
        let s = Slot {
            kind: Kind::Band,
            c1: [128, 0, 0, 0],
            c2: [128, 0, 0, 0],
            s_in: t.s_in,
            s_out: t.s_out,
            a: t.a,
            parent: self.last,
            h_in: t.h.min(RMAX),
            h_out: (t.h + 18.0).min(RMAX),
            ring: t.ring,
            row: t.row,
            flag: true,
            ..Slot::default()
        };
        if let Some(i) = self.alloc(s) {
            if let Some(p) = self.last {
                self.slots[p].child = Some(i);
            }
            self.last = Some(i);
        }
    }

    /// 0x1E00080.
    fn spawner(&mut self) {
        self.cnt += 1;
        let c = self.cnt;
        if c == 1 {
            self.c1_frozen = false;
            self.c1 = [64, 32, 48];
            self.c2 = [128, 0, 0];
            self.alloc(Slot { kind: Kind::Glow, c1: [48, 48, 48, 0], c2: [48, 48, 48, 0], ..Slot::default() });
            self.last = Some(0);
            self.scale = [160.0; RINGS];
            const ADD: [i32; 12] = [30, 60, 90, 120, 150, 180, 210, 240, 270, 300, 330, 320];
            for k in 0..5 {
                for j in 0..12 {
                    self.rows[k][j] = self.rng.next() % 20 + ADD[j];
                }
            }
        }
        for i in 0..RINGS {
            if c >= 11 + 10 * i as i32 {
                self.scale[i] -= 5.0;
            }
        }
        for i in 0..RINGS {
            if c == 8 + 10 * i as i32 {
                self.last = None;
                let row = (self.rng.next() % 5) as usize;
                let a = self.rng.next() % 360;
                self.spawn_band(BandSpawn { s_in: 0, s_out: 0, h: 0.0, ring: i, row, a });
            }
        }
        if c >= 29 {
            if !self.c1_frozen {
                if self.c1[0] >= 21 {
                    self.c1[0] -= 1;
                }
                if self.c1[2] < 192 {
                    self.c1[2] += 2;
                } else {
                    self.c1_frozen = true;
                }
            }
            if self.c2[0] >= 21 {
                self.c2[0] -= 2;
            }
            if self.c2[2] < 192 {
                self.c2[2] += 2;
            }
        }
    }

    /// 0x1E00B10.
    fn glow_update(&mut self, i: usize) {
        let (cnt, life) = (self.cnt, self.life);
        let s = &mut self.slots[i];
        for a in [&mut s.c1[3], &mut s.c2[3]] {
            if cnt < 13 {
                if *a < 128 {
                    *a += 8;
                }
            } else if life < 6 {
                if *a >= 17 {
                    *a -= 16;
                }
            } else if *a < 176 {
                *a += 8;
            }
        }
        if life < 2 {
            s.active = false;
        }
    }

    /// 0x1E00BF0.
    fn band_update(&mut self, i: usize) {
        let (cnt, life) = (self.cnt, self.life);
        let mut s = self.slots[i];
        s.a = s16(s.a - 6);
        if s.a >= 361 {
            s.a %= 360;
        }
        let sc = self.scale[s.ring];
        let f = sc / RMAX;
        s.s_in = s16((f * s.h_in) as i32);
        if sc < 1.0 {
            s.active = false;
        }
        s.s_out = s16((f * s.h_out) as i32);
        if !(s.h_in < 0.0) {
            s.h_in -= 3.0;
            if !(s.h_out - s.h_in < 18.0) {
                s.h_out -= 3.0;
            }
        } else {
            s.h_out -= 3.0;
            if s.h_out < s.h_in {
                if let Some(c) = s.child {
                    self.slots[c].parent = None;
                }
                s.active = false;
            }
        }
        self.slots[i] = s;
        if s.flag && s.h_out < RMAX {
            self.last = Some(i);
            self.spawn_band(BandSpawn { s_in: s.s_in, s_out: s.s_out, h: s.h_out, ring: s.ring, row: s.row, a: s16(s.a + 15) });
            self.slots[i].flag = false;
        }
        let s = &mut self.slots[i];
        for a in [&mut s.c1[3], &mut s.c2[3]] {
            if cnt < 14 && *a < 128 {
                *a += 4;
            }
            if life < 6 {
                if *a >= 17 {
                    *a -= 16;
                }
            } else if *a < 128 {
                *a += 4;
            }
        }
        if life < 2 {
            s.active = false;
        }
    }

    /// Module update `u` (1, 2, ..) with the live camera (`eye`, `target`);
    /// the anchor is `target`. Returns 0, or 8 at u 126 (and every later
    /// call).
    pub fn update(&mut self, _u: u32, eye: [f64; 3], target: [f64; 3]) -> u32 {
        self.additive.clear();
        self.counts = [0; 2];
        if self.done {
            return RET_DONE;
        }
        self.spawner();
        // Slot order: a band spawned at a higher index is updated in this
        // pass (a newborn ring completes itself outward at once).
        for i in 0..SLOTS {
            if self.slots[i].active {
                match self.slots[i].kind {
                    Kind::Glow => self.glow_update(i),
                    Kind::Band => self.band_update(i),
                }
            }
        }
        if let Some(project) = projector(eye, target) {
            let project: &dyn Fn([f64; 3]) -> Option<[f32; 2]> = &project;
            let [r, up, back] = billboard_axes(eye, target);
            let glow = |l: [f32; 3]| -> [f32; 3] { std::array::from_fn(|k| (target[k] + f64::from(l[0]) * r[k] + f64::from(l[1]) * up[k] + f64::from(l[2]) * back[k]) as f32) };
            // Ring node: scale (0.7, 0.2, 0.7), rot = camera angles + 0x4000 on x:
            // world = T + 0.7 x right + 0.2 y back - 0.7 z up.
            let node = |l: [f32; 3]| -> [f32; 3] {
                let (x, y, z) = (f64::from(NODE[0] * l[0]), f64::from(NODE[1] * l[1]), f64::from(NODE[2] * l[2]));
                std::array::from_fn(|k| (target[k] + x * r[k] + y * back[k] - z * up[k]) as f32)
            };
            for i in 0..SLOTS {
                if !self.slots[i].active {
                    continue;
                }
                match self.slots[i].kind {
                    Kind::Glow => self.glow_draw(i, project, &glow),
                    Kind::Band => self.band_draw(i, project, &node),
                }
            }
        }
        self.life -= 1;
        if self.life <= 0 {
            self.done = true;
            RET_DONE
        } else {
            0
        }
    }

    /// 0x1E014F0: 0x109640(40, 20, q) quadrants, (C1, A), each sent once.
    fn glow_draw(&mut self, i: usize, project: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, at: &dyn Fn([f32; 3]) -> [f32; 3]) {
        let [r, g, b] = self.c1;
        let s = &mut self.slots[i];
        s.c1[..3].copy_from_slice(&self.c1);
        s.c2[..3].copy_from_slice(&self.c1);
        let (c01, c23) = ([r, g, b, s.c1[3]], [r, g, b, s.c2[3]]);
        let uv = uv15([[176, 0], [235, 0], [176, 31], [235, 31]]);
        for q in 0..4 {
            let x = if q % 2 == 0 { -40.0 } else { 40.0 };
            let y = if q < 2 { 20.0 } else { -20.0 };
            let Some(v) = project4(project, [[x, y, 0.0], [0.0, y, 0.0], [x, 0.0, 0.0], [0.0, 0.0, 0.0]].map(|l| at(l))) else {
                continue;
            };
            self.counts[0] += 1;
            quad(&mut self.additive, v, uv, [c01, c01, c23, c23], true, 15);
        }
    }

    /// 0x1E01860: 12 arcs of 20 degrees at the row's jittered steps; with a
    /// parent the inner edge is the parent's outer edge (written into the
    /// slot's h_in).
    fn band_draw(&mut self, i: usize, project: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, node: &dyn Fn([f32; 3]) -> [f32; 3]) {
        let c2 = self.c2;
        {
            let s = &mut self.slots[i];
            s.c1[..3].copy_from_slice(&c2);
            s.c2[..3].copy_from_slice(&c2);
        }
        let s = self.slots[i];
        let p = s.parent.map(|p| self.slots[p]);
        let c01 = p.map_or(s.c1, |p| p.c2);
        let c23 = s.c2;
        let uv = uv15([[128, 129], [255, 129], [128, 192], [255, 192]]);
        let arc = |r: i32, a: i32, y: f32| -> [f32; 3] { [r as f32 * dcos(a), y, r as f32 * dsin(a)] };
        let mut s4 = 0;
        for s5 in 0..12 {
            let (v0, v1) = match p {
                Some(p) => {
                    self.slots[i].h_in = p.h_out;
                    (arc(p.s_out, s16(s4 + p.a), p.h_out), arc(p.s_out, s16(s4 + p.a + 20), p.h_out))
                }
                None => (arc(s.s_in, s16(s4 + s.a), s.h_in), arc(s.s_in, s16(s4 + s.a + 20), s.h_in)),
            };
            let v2 = arc(s.s_out, s16(s4 + s.a), s.h_out);
            let v3 = arc(s.s_out, s16(s4 + s.a + 20), s.h_out);
            if let Some(v) = project4(project, [v0, v1, v2, v3].map(|l| node(l))) {
                self.counts[1] += 1;
                quad(&mut self.additive, v, uv, [c01, c01, c23, c23], true, 15);
            }
            s4 = self.rows[s.row][s5];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duel_c022::tests::{overview, trap};

    fn run(cam: ([f64; 3], [f64; 3]), seed: u32) -> (C026, Vec<(u32, [usize; 2], u32)>) {
        let mut c = C026::new(seed);
        let mut v = Vec::new();
        for u in 1..=140 {
            let n0 = c.rand_calls();
            let r = c.update(u, cam.0, cam.1);
            v.push((r, c.counts(), c.rand_calls() - n0));
            if r == 8 {
                break;
            }
        }
        (c, v)
    }

    #[test]
    fn counts_returns_and_rand_follow_the_doc() {
        for (cam, seed) in [(overview(), 1), (trap(2, 6), 5)] {
            let (c, v) = run(cam, seed);
            assert_eq!(v.len(), 126);
            assert!(c.done());
            assert!(v[..125].iter().all(|x| x.0 == 0) && v[125].0 == 8);
            let q = |u: usize| v[u - 1].1[0] + v[u - 1].1[1];
            assert_eq!(q(1), 4);
            assert_eq!(q(8), 112);
            assert_eq!(q(11), 124);
            assert_eq!(q(18), 232);
            assert_eq!(q(22), 244);
            assert_eq!(q(28), 352);
            assert_eq!(q(38), 472);
            for u in [41, 51, 61, 71, 81] {
                assert_eq!(q(u), 484, "u {u}");
            }
            assert_eq!(v.iter().map(|x| x.1[1]).max(), Some(480));
            for u in 112..=125 {
                assert_eq!(v[u - 1].1, [4, 0], "u {u}");
            }
            assert_eq!(q(126), 0);
            // 32,276 primitives: glow 500, rings 31,776.
            assert_eq!(v.iter().map(|x| x.1[0]).sum::<usize>(), 500);
            assert_eq!(v.iter().map(|x| x.1[1]).sum::<usize>(), 31_776);
            // 76 rand: 60 at u 1, 2 per ring birth.
            for (i, x) in v.iter().enumerate() {
                let u = i as i32 + 1;
                let want = if u == 1 { 60 } else if u >= 8 && (u - 8) % 10 == 0 && u <= 78 { 2 } else { 0 };
                assert_eq!(x.2, want, "u {u}");
            }
            assert_eq!(c.rand_calls(), 76);
            let mut c = c;
            assert_eq!(c.update(127, cam.0, cam.1), 8);
            assert!(c.additive.is_empty());
        }
    }

    #[test]
    fn ring_births_bands_and_lifetimes() {
        let (eye, target) = overview();
        let mut c = C026::new(3);
        for u in 1..=7 {
            c.update(u, eye, target);
            assert_eq!(c.bands(), [0; 8]);
        }
        c.update(8, eye, target);
        // Ring 0 builds itself outward in the birth update: 9 bands.
        assert_eq!(c.bands()[0], 9);
        let b: Vec<(f32, f32)> = c.slots.iter().filter(|s| s.active && s.kind == Kind::Band).map(|s| (s.h_in, s.h_out)).collect();
        assert_eq!(b, vec![(-3.0, 15.0), (15.0, 30.0), (30.0, 45.0), (45.0, 60.0), (60.0, 75.0), (75.0, 90.0), (90.0, 105.0), (105.0, 120.0), (120.0, 130.0)]);
        for u in 9..=112u32 {
            c.update(u, eye, target);
            for i in 0..8u32 {
                let alive = c.bands()[i as usize] > 0;
                assert_eq!(alive, (8 + 10 * i..=41 + 10 * i).contains(&u), "u {u} ring {i}");
            }
            if u == 41 {
                assert_eq!(c.bands().iter().sum::<usize>(), 40);
            }
        }
        assert_eq!(c.scale(0), 160.0 - 5.0 * 102.0);
    }

    #[test]
    fn colours_and_glow_alpha() {
        let (eye, target) = overview();
        let mut c = C026::new(3);
        for u in 1..=126u32 {
            c.update(u, eye, target);
            let (c1, c2) = c.colours();
            match u {
                1..=28 => assert_eq!((c1, c2), ([64, 32, 48], [128, 0, 0])),
                60 => assert_eq!(c2, [64, 0, 64]),
                _ => {}
            }
            if u >= 100 {
                assert_eq!(c1, [20, 32, 192]);
            }
            if u >= 124 {
                assert_eq!(c2, [20, 0, 192]);
            }
            if u == 83 {
                assert_eq!(c2[0], 20);
            }
            let a = match u {
                1..=12 => Some(8 * u as u8),
                13..=22 => Some(96 + 8 * (u as u8 - 12)),
                23..=121 => Some(176),
                122..=125 => Some(176 - 16 * (u as u8 - 121)),
                _ => None,
            };
            let glow: Vec<_> = c.additive.iter().filter(|t| t.uv[0][1] < 0.5).collect();
            match a {
                Some(a) => {
                    assert_eq!(glow.len(), 8, "u {u}");
                    assert!(glow.iter().all(|t| t.colour.iter().all(|x| *x == [c1[0], c1[1], c1[2], a])), "u {u}");
                }
                None => assert!(glow.is_empty()),
            }
            // Ring arcs carry C2.
            assert!(c.additive.iter().filter(|t| t.uv[0][1] >= 0.5).all(|t| t.colour[2][..3] == c2), "u {u}");
        }
    }

    #[test]
    fn anchored_at_the_look_at_point() {
        // The glow's centre (v3 of every quadrant) is the screen centre.
        for (eye, target) in [overview(), trap(2, 6)] {
            let mut c = C026::new(1);
            c.update(1, eye, target);
            for t in c.additive.iter().skip(1).step_by(2) {
                assert!((t.screen[1][0] - 256.0).abs() < 0.01 && (t.screen[1][1] - 256.0).abs() < 0.01, "{:?}", t.screen[1]);
            }
        }
    }
}
