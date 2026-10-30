//! CMODULE c505 (0x1F9), Raigeki's lightning (event 5, handler 0x2593C0: one
//! bolt per struck cell every 12 U; Anti-Raigeki: 10 on its own cell), as
//! specified in `claude-activate/NEWLY-ALLOWED.md` section 10 [F code; emu:
//! the reference model `tools/mf_c505_model.py` equals the interpreter's
//! primitives to 0.0625 px in five runs]. The target is a CELL word: every
//! node sits at the cell centre T = (50 (col - 3), 0, 50 (row - 3)) (height
//! 0). All additive (ALPHA 0x48):
//! - three BOLTS (TEXEFF 14 (0|32, 128..256)) crawl down from 200 above the
//!   cell, one 40-unit segment per update (u 1-6), each led by a flickering
//!   ORB (TEXEFF 3) riding its tip (u 1-5); bolt alpha 128, -16/U from u 7,
//!   dim column (u0 32) once below 0x60 (u 9); from u 7 the bolt node
//!   shakes +-0.5 on the camera's screen-horizontal world axis;
//! - full-screen FLASHES at u 2, 4, 6;
//! - the strike from u 6: the 3-piece burst PANEL (TEXEFF 14, alpha 128 -32/U,
//!   x1.5 from u 9), the gold GLOW (u 6-9), the gold cone RING (alpha -32/U,
//!   scale +0.2/U), 60 gold POINTS (x 1.02, scale to 1.1) and gold SMOKE
//!   (8 particles, start frame -1).
//!
//! Returns 2 at u 6 (the handler starts the struck monster's fade at u 7), 8
//! at u 30. SE bank 1 0x07 at u 1. 256 random values at init (bolts 3 x 5 x
//! (x, z), ring 30, points 60 x 3, smoke 8 x (x, y)), 3 per smoke respawn
//! (from u 21). No unit / card / light side effects.
//!
//! [H / not reproduced]: the port's random values (the caller's stream, same
//! order and count); the sine table (see `duel_fxlib`); no OT keys.
use crate::duel_c621::FxTri;
use crate::duel_fxlib::{cell_centre, cosf, decay, flash, quad, sinf, uvn, Cam, PSys, Particle, SHEET_128X64, SHEET_256};

/// Module life.
pub const UPDATES: u32 = 30;
/// The update returning 2 (the handler starts the struck monster's fade at
/// u 7).
pub const RET2_U: u32 = 6;
/// The updates drawing a full-screen flash.
pub const FLASH_U: [u32; 3] = [2, 4, 6];
/// SE (SECTL bank 1) played at `SE_U`.
pub const SE_BANK: u8 = 1;
pub const SE_ID: u16 = 0x07;
pub const SE_U: u32 = 1;
/// Random values drawn at init.
pub const INIT_RAND: u32 = 256;
const WHITE: [u8; 3] = [128, 128, 128];
const GOLD: [u8; 3] = [128, 112, 64];
/// Panel points (x 30) and the glow template (c529's ORB x 15).
const PANEL: [[f32; 3]; 8] = [[-30.0, 0.0, 0.0], [-30.0, 30.0, 0.0], [0.0, 30.0, 0.0], [30.0, 30.0, 0.0], [30.0, 0.0, 0.0], [30.0, -9.0, 30.0], [0.0, -9.0, 30.0], [-30.0, -9.0, 30.0]];
const GLOW: [[f32; 3]; 6] = [[-15.0, 0.0, 0.0], [-15.0, 15.0, 0.0], [15.0, 15.0, 0.0], [15.0, 0.0, 0.0], [15.0, -4.5, 15.0], [-15.0, -4.5, 15.0]];
const ORB: [[f32; 2]; 6] = [[-12.5, 12.5], [-12.5, -12.5], [12.5, 12.5], [12.5, -12.5], [0.0, 12.5], [0.0, -12.5]];

#[derive(Clone, Debug, PartialEq)]
struct Bolt {
    p: [[f32; 3]; 6],
    cnt: u32,
    a: u8,
    u0: i32,
    orb_f: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct C505 {
    t: [f32; 3],
    bolts: [Bolt; 3],
    ring: [([f32; 3], [f32; 3]); 31],
    ring_a: u8,
    ring_s: f32,
    pts: [[f32; 3]; 60],
    pts_a: u8,
    pts_s: f32,
    smoke: PSys,
    panel_a: u8,
    panel_tick: u32,
    /// Glow (tick, frame, done).
    glow: (u32, i32, bool),
    scale_1c0: bool,
    /// The bolt node's position for the next draw (shaken from u 7).
    node150: [f32; 3],
    delay: u32,
    flagged: bool,
    life: i32,
    age: u32,
    done: bool,
    rand_calls: u32,
    // Last-update observations.
    drawn_node150: [f32; 3],
    drawn_panel: Option<(u8, bool, i32)>,
    drawn_glow: Option<i32>,
    drawn_ring: Option<(u8, f32)>,
    drawn_pts: Option<(u8, f32)>,
    drawn_bolts: [(usize, u8, i32); 3],
    drawn_orbs: [Option<f32>; 3],
    smoke_drawn: usize,
    flashes: usize,
    /// Additive (ALPHA 0x48) world triangles of the last update: bolts,
    /// orbs, panel, glow, ring, points, smoke.
    pub additive: Vec<FxTri>,
    /// Kept empty (the convention of the other modules).
    pub subtractive: Vec<FxTri>,
    pub opaque: Vec<FxTri>,
    pub alpha_blend: Vec<FxTri>,
    /// Full-screen flashes of the last update (Z max: over everything).
    pub flash: Vec<FxTri>,
}

impl C505 {
    /// `cell` = (col, row) (a cell word, never a unit ref); `rng` = the
    /// caller's stream (256 draws: bolts 3 x 5 x (x, z), ring 30, points
    /// 180, smoke 8 x (x, y)).
    pub fn new(cell: (u8, u8), rng: &mut impl FnMut() -> u32) -> Self {
        let mut n = 0u32;
        let mut r = || {
            n += 1;
            rng()
        };
        let t = cell_centre(i32::from(cell.0), i32::from(cell.1));
        let bolts = std::array::from_fn(|_| {
            let mut p = [[0.0, 200.0, 0.0]; 6];
            for j in 0..5 {
                let rr: i32 = if j < 4 { 15 } else { 5 };
                let x = (r() % (2 * rr) as u32) as i32 - rr;
                let z = (r() % (2 * rr) as u32) as i32 - rr;
                p[j + 1] = [x as f32, 200.0 - 40.0 * (j + 1) as f32, z as f32];
            }
            Bolt { p, cnt: 5, a: 128, u0: 0, orb_f: 0 }
        });
        let mut ring = [([0.0f32; 3], [0.0f32; 3]); 31];
        ring[0] = ([8.0, 0.0, 0.0], [24.0, 30.0, 0.0]);
        for k in 1..31 {
            let a = 0.209_439_52f32 * k as f32;
            let j = ((r() % 60) as i32 - 30) >> 1;
            ring[k] = ([8.0 * cosf(a), 0.0, 8.0 * sinf(a)], [24.0 * cosf(a), 30.0 + j as f32, 24.0 * sinf(a)]);
        }
        let pts = std::array::from_fn(|_| {
            let x = (r() % 50) as f32 - 25.0;
            let y = (r() % 25) as f32;
            let z = (r() % 50) as f32 - 25.0;
            [x, y, z]
        });
        let p = (0..8)
            .map(|i| {
                let x = (r() % 60) as f32 - 30.0;
                let y = (r() % 30) as f32;
                Particle { x, y, z: 0.0, f: -1, t: -i }
            })
            .collect();
        let smoke = PSys {
            flags: 7,
            base: [0.0; 3],
            cell: [32, 32],
            uvb: [128, 192],
            per: 4,
            sheet: 15,
            sheet_size: SHEET_256,
            half: 20.0,
            rgb: GOLD,
            a: 128,
            hold: 1,
            max_frame: 6,
            wrap: 8,
            rx: 30,
            ry: -30,
            p,
        };
        Self {
            t,
            bolts,
            ring,
            ring_a: 128,
            ring_s: 1.0,
            pts,
            pts_a: 128,
            pts_s: 0.1,
            smoke,
            panel_a: 128,
            panel_tick: 0,
            glow: (0, 0, false),
            scale_1c0: false,
            node150: t,
            delay: 5,
            flagged: false,
            life: UPDATES as i32,
            age: 0,
            done: false,
            rand_calls: n,
            drawn_node150: t,
            drawn_panel: None,
            drawn_glow: None,
            drawn_ring: None,
            drawn_pts: None,
            drawn_bolts: [(0, 0, 0); 3],
            drawn_orbs: [None; 3],
            smoke_drawn: 0,
            flashes: 0,
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

    pub fn rand_calls(&self) -> u32 {
        self.rand_calls
    }

    /// The cell centre T.
    pub fn centre(&self) -> [f32; 3] {
        self.t
    }

    /// The bolt node's offset from T in the last draw (the shake).
    pub fn shake(&self) -> [f32; 3] {
        std::array::from_fn(|i| self.drawn_node150[i] - self.t[i])
    }

    /// Bolt i in the last update: (segments drawn, alpha, u0).
    pub fn bolt(&self, i: usize) -> (usize, u8, i32) {
        self.drawn_bolts[i]
    }

    /// Bolt i's orb in the last update: its tip height (None: not drawn).
    pub fn orb(&self, i: usize) -> Option<f32> {
        self.drawn_orbs[i]
    }

    /// The panel in the last update: (alpha, x1.5, u0 base 96 / 176).
    pub fn panel(&self) -> Option<(u8, bool, i32)> {
        self.drawn_panel
    }

    /// The glow's u0 in the last update.
    pub fn glow(&self) -> Option<i32> {
        self.drawn_glow
    }

    /// (alpha, scale) of the ring / points in the last update.
    pub fn ring(&self) -> Option<(u8, f32)> {
        self.drawn_ring
    }

    pub fn points(&self) -> Option<(u8, f32)> {
        self.drawn_pts
    }

    pub fn smoke_count(&self) -> usize {
        self.smoke_drawn
    }

    pub fn flashes(&self) -> usize {
        self.flashes
    }

    /// Module update `u` (1, 2, ..): `yaw16` = the camera yaw (0x205580
    /// angle 1: 0 side 0, 0x8000 side 1) for the shake axis, `rng` the
    /// caller's stream (smoke respawns), (`eye`, `target`) the camera.
    /// Returns 0, 2 at u 6, 8 at u 30 (and every later call).
    pub fn update(&mut self, _u: u32, yaw16: u16, rng: &mut impl FnMut() -> u32, eye: [f64; 3], target: [f64; 3]) -> u32 {
        self.additive.clear();
        self.flash.clear();
        self.flashes = 0;
        self.smoke_drawn = 0;
        self.drawn_panel = None;
        self.drawn_glow = None;
        self.drawn_ring = None;
        self.drawn_pts = None;
        self.drawn_orbs = [None; 3];
        if self.done {
            return 8;
        }
        let cam = Cam::new(eye, target);
        let t = self.t;
        let (sp, sr) = (self.pts_s, self.ring_s);
        let n1c0 = [t[0], 3.0, t[2]];
        let s1c0 = if self.scale_1c0 { [1.5, 1.5, 1.0] } else { [1.0; 3] };
        let node150 = self.node150;
        self.drawn_node150 = node150;
        if self.glow.1 > 0 {
            self.scale_1c0 = true;
        }
        self.node150 = t;
        if self.delay == 0 {
            let y = i32::from(yaw16);
            let xs = (0x2000..0x6000).contains(&y) || (0xA000..0xE000).contains(&y);
            let d = if self.life % 2 == 0 { 0.5 } else { -0.5 };
            if xs {
                self.node150[0] += d;
            } else {
                self.node150[2] += d;
            }
        }
        for i in 0..3 {
            if self.bolts[i].a < 0x60 {
                self.bolts[i].u0 = 32;
            }
            self.bolt_draw(&cam, i, node150);
            if self.bolts[i].cnt != 0 {
                self.orb_draw(&cam, i, node150);
                self.bolts[i].cnt -= 1;
            }
        }
        if self.delay == 0 {
            self.panel_draw(&cam, n1c0, s1c0);
            self.panel_tick += 1;
            self.panel_a = decay(self.panel_a, 32);
            if !self.glow.2 {
                self.glow_draw(&cam, n1c0, s1c0);
            }
            self.ring_draw(&cam, sr);
            self.ring_a = decay(self.ring_a, 32);
            self.ring_s += 0.2;
            if self.pts_s < 1.0 {
                self.pts_s += 0.2;
            }
            self.points_draw(&cam, sp);
            self.pts_a = decay(self.pts_a, 8);
            for b in &mut self.bolts {
                b.a = decay(b.a, 16);
            }
            self.smoke_drawn = self.smoke.draw(&cam, rng, t, 1.0, None, &mut self.additive);
            self.smoke.a = decay(self.smoke.a, 8);
        }
        if self.delay % 2 == 0 && !self.flagged {
            flash(&mut self.flash);
            self.flashes += 1;
        }
        if self.delay == 0 {
            self.flagged = true;
        }
        if self.delay > 0 {
            self.delay -= 1;
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

    /// 0x1E02CA0: segments s < 5 - cnt, ribbon +10 on the node's x.
    fn bolt_draw(&mut self, cam: &Cam, i: usize, node: [f32; 3]) {
        let b = self.bolts[i].clone();
        let n = 5 - b.cnt as usize;
        self.drawn_bolts[i] = (n, b.a, b.u0);
        let uv = uvn([[b.u0, 128], [b.u0, 256], [b.u0 + 32, 128], [b.u0 + 32, 256]], SHEET_256);
        for s in 0..n {
            let a0 = b.a.saturating_sub((16 * (5 - s)) as u8);
            let a1 = b.a.saturating_sub((16 * (4 - s)) as u8);
            let (p0, p1) = (b.p[s], b.p[s + 1]);
            let l = [p0, p1, [p0[0] + 10.0, p0[1], p0[2]], [p1[0] + 10.0, p1[1], p1[2]]];
            if let Some(v) = cam.quad(l.map(|l| cam.bill(node, l, [1.0; 3]))) {
                let c = |a: u8| [WHITE[0], WHITE[1], WHITE[2], a];
                quad(&mut self.additive, v, uv, [c(a0), c(a1), c(a0), c(a1)], true, 14);
            }
        }
    }

    /// 0x1E00AE0: two mirrored halves around the tip, TEXEFF 3, u0 64 / 96.
    fn orb_draw(&mut self, cam: &Cam, i: usize, node: [f32; 3]) {
        let b = &mut self.bolts[i];
        let q = b.p[5 - b.cnt as usize];
        let (cx, cy) = (5.0 + q[0], q[1]);
        let u0 = 64 + 32 * b.orb_f;
        b.orb_f ^= 1;
        self.drawn_orbs[i] = Some(cy);
        let uv = uvn([[u0, 0], [u0 + 32, 0], [u0, 64], [u0 + 32, 64]], SHEET_128X64);
        for qq in [[0usize, 4, 1, 5], [2, 4, 3, 5]] {
            let l = qq.map(|k| [cx + ORB[k][0], cy + ORB[k][1], 0.0]);
            if let Some(v) = cam.quad(l.map(|l| cam.bill(node, l, [1.0; 3]))) {
                quad(&mut self.additive, v, uv, [[WHITE[0], WHITE[1], WHITE[2], 128]; 4], true, 3);
            }
        }
    }

    /// 0x1E03090: three 80 x 80 cells of TEXEFF 14.
    fn panel_draw(&mut self, cam: &Cam, node: [f32; 3], s: [f32; 3]) {
        let v = if self.panel_tick % 2 == 0 { 80 } else { 0 };
        let (u0, u1) = (v + 96, v + 175);
        self.drawn_panel = Some((self.panel_a, s[0] == 1.5, u0));
        let p = PANEL;
        let c0 = [0.0; 3];
        let c = [WHITE[0], WHITE[1], WHITE[2], self.panel_a];
        for (q, (va, vb)) in [[p[0], c0, p[7], p[6]], [p[1], p[2], p[0], c0], [p[2], p[3], c0, p[4]]].into_iter().zip([(0, 79), (80, 159), (160, 239)]) {
            if let Some(w) = cam.quad(q.map(|l| cam.bill(node, l, s))) {
                quad(&mut self.additive, w, uvn([[u0, va], [u1, va], [u0, vb], [u1, vb]], SHEET_256), [c; 4], true, 14);
            }
        }
    }

    /// 0x1E003D0: c529's ORB, 2 frames x 2 U, once.
    fn glow_draw(&mut self, cam: &Cam, node: [f32; 3], s: [f32; 3]) {
        let u0 = 176 + 32 * self.glow.1;
        self.drawn_glow = Some(u0);
        let p = GLOW;
        let c = [GOLD[0], GOLD[1], GOLD[2], 128];
        for (q, (va, vb)) in [[p[1], p[2], p[0], p[3]], [p[0], p[3], p[5], p[4]]].into_iter().zip([(64, 79), (79, 95)]) {
            if let Some(w) = cam.quad(q.map(|l| cam.bill(node, l, s))) {
                quad(&mut self.additive, w, uvn([[u0, va], [u0 + 31, va], [u0, vb], [u0 + 31, vb]], SHEET_256), [c; 4], true, 15);
            }
        }
        self.glow.0 += 1;
        if self.glow.0 > 1 {
            self.glow.0 = 0;
            self.glow.1 += 1;
            if self.glow.1 > 1 {
                self.glow.2 = true;
            }
        }
    }

    /// 0x1E02660: world axes at T, scale `s`.
    fn ring_draw(&mut self, cam: &Cam, s: f32) {
        self.drawn_ring = Some((self.ring_a, s));
        let t = self.t;
        let w = |p: [f32; 3]| -> [f64; 3] { std::array::from_fn(|i| f64::from(p[i] * s + t[i])) };
        let uv = uvn([[142, 64], [142, 0], [174, 64], [174, 0]], SHEET_256);
        let c = [GOLD[0], GOLD[1], GOLD[2], self.ring_a];
        for k in 0..30 {
            let (i0, o0) = self.ring[k];
            let (i1, o1) = if k < 29 { self.ring[k + 1] } else { self.ring[0] };
            if let Some(v) = cam.quad([w(i0), w(o0), w(i1), w(o1)]) {
                quad(&mut self.additive, v, uv, [c; 4], true, 15);
            }
        }
    }

    /// 0x1E01DC0: x 1.02 per draw, quads +-1.5 on the billboard at T.
    fn points_draw(&mut self, cam: &Cam, s: f32) {
        self.drawn_pts = Some((self.pts_a, s));
        let uv = uvn([[176, 64], [207, 64], [176, 95], [207, 95]], SHEET_256);
        let c = [GOLD[0], GOLD[1], GOLD[2], self.pts_a];
        let t = self.t;
        for i in 0..60 {
            self.pts[i] = self.pts[i].map(|v| v * 1.02);
            let [x, y, z] = self.pts[i];
            let h = 1.5f32;
            let l = [[x - h, y + h, z], [x + h, y + h, z], [x - h, y - h, z], [x + h, y - h, z]];
            if let Some(v) = cam.quad(l.map(|l| cam.bill(t, l, [s; 3]))) {
                quad(&mut self.additive, v, uv, [c; 4], true, 15);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duel_fxlib::tests::trap;
    use crate::duel_fxlib::Rng;

    #[derive(Debug)]
    struct Obs {
        ret: u32,
        flashes: usize,
        bolts: [(usize, u8, i32); 3],
        orbs: [Option<f32>; 3],
        shake: [f32; 3],
        panel: Option<(u8, bool, i32)>,
        glow: Option<i32>,
        ring: Option<(u8, f32)>,
        pts: Option<(u8, f32)>,
        smoke: usize,
        rand: u32,
    }

    fn run(col: u8, row: u8, side: u8, seed: u32) -> (C505, Vec<Obs>) {
        let (eye, target) = trap(i32::from(col), i32::from(row), side);
        let yaw = if side == 0 { 0 } else { 0x8000 };
        let mut rng = Rng::new(seed);
        let mut m = C505::new((col, row), &mut || rng.next());
        assert_eq!(rng.calls, INIT_RAND);
        let mut v = Vec::new();
        for u in 1..=UPDATES {
            let n0 = rng.calls;
            let ret = m.update(u, yaw, &mut || rng.next(), eye, target);
            v.push(Obs {
                ret,
                flashes: m.flashes(),
                bolts: std::array::from_fn(|i| m.bolt(i)),
                orbs: std::array::from_fn(|i| m.orb(i)),
                shake: m.shake(),
                panel: m.panel(),
                glow: m.glow(),
                ring: m.ring(),
                pts: m.points(),
                smoke: m.smoke_count(),
                rand: rng.calls - n0,
            });
        }
        (m, v)
    }

    #[test]
    fn returns_flashes_and_rand() {
        let (mut m, v) = run(3, 5, 0, 7);
        assert_eq!(m.rand_calls(), INIT_RAND);
        let rets: Vec<_> = v.iter().enumerate().filter(|(_, x)| x.ret != 0).map(|(i, x)| (i as u32 + 1, x.ret)).collect();
        assert_eq!(rets, vec![(6, 2), (30, 8)]);
        let fl: Vec<u32> = v.iter().enumerate().filter(|(_, x)| x.flashes > 0).map(|(i, _)| i as u32 + 1).collect();
        assert_eq!(fl, FLASH_U.to_vec());
        assert!(v.iter().all(|x| x.flashes <= 1));
        // Smoke respawns (3 rand) only from u 21.
        assert!(v[..20].iter().all(|x| x.rand == 0));
        assert!(v[20..].iter().any(|x| x.rand > 0) && v.iter().all(|x| x.rand % 3 == 0));
        let (eye, target) = trap(3, 5, 0);
        assert_eq!(m.update(31, 0, &mut || 0, eye, target), 8);
        assert!(m.additive.is_empty());
    }

    #[test]
    fn bolts_orbs_and_shake() {
        let (_, v) = run(3, 5, 0, 3);
        for u in 1..=30usize {
            let o = &v[u - 1];
            // Segments u - 1 (u 1-6) then 5.
            assert_eq!(o.bolts[0].0, (u - 1).min(5), "u {u}");
            // Orbs ride the tips: y 200 - 40 (u - 1) at u 1..5.
            for i in 0..3 {
                assert_eq!(o.orbs[i], if u <= 5 { Some(200.0 - 40.0 * (u - 1) as f32) } else { None }, "u {u}");
            }
            // Bolt alpha 128 to u 6, then 128 - 16 (u - 6); u0 32 from u 9.
            assert_eq!(o.bolts[1].1, 128u32.saturating_sub(16 * (u as u32).saturating_sub(6)) as u8, "u {u}");
            assert_eq!(o.bolts[2].2, if u >= 9 { 32 } else { 0 }, "u {u}");
            // Shake on z (yaw 0) from u 7: -0.5 odd u, +0.5 even u.
            let want = if u >= 7 { if u % 2 == 1 { -0.5 } else { 0.5 } } else { 0.0 };
            assert_eq!(o.shake, [0.0, 0.0, want], "u {u}");
        }
        assert_eq!(v[6].bolts[0].1, 112);
        // x shake for yaw 0x4000.
        let (eye, target) = trap(3, 5, 0);
        let mut rng = Rng::new(1);
        let mut m = C505::new((3, 5), &mut || rng.next());
        for u in 1..=7 {
            m.update(u, 0x4000, &mut || rng.next(), eye, target);
        }
        assert_eq!(m.shake(), [-0.5, 0.0, 0.0]);
    }

    #[test]
    fn strike_layers() {
        let (_, v) = run(3, 5, 0, 5);
        let at = |u: usize| &v[u - 1];
        assert!(v[..5].iter().all(|x| x.panel.is_none() && x.ring.is_none() && x.smoke == 0));
        // Panel u 6..30: alpha 128, 96, 64, 32, 0; u0 176 at even tick (u 6, 8, ..); x1.5 from u 9.
        let pa: Vec<u8> = (6..=11).map(|u| at(u).panel.unwrap().0).collect();
        assert_eq!(pa, vec![128, 96, 64, 32, 0, 0]);
        assert_eq!((at(6).panel.unwrap().2, at(7).panel.unwrap().2, at(8).panel.unwrap().2), (176, 96, 176));
        assert_eq!((at(8).panel.unwrap().1, at(9).panel.unwrap().1, at(30).panel.unwrap().1), (false, true, true));
        // Glow u 6..9: u0 176, 176, 208, 208.
        let g: Vec<Option<i32>> = (5..=10).map(|u| at(u).glow).collect();
        assert_eq!(g, vec![None, Some(176), Some(176), Some(208), Some(208), None]);
        // Ring alpha 128, 96, 64, 32, 0 from u 10; scale 1.0 + 0.2 (u - 6).
        let r: Vec<u8> = (6..=10).map(|u| at(u).ring.unwrap().0).collect();
        assert_eq!(r, vec![128, 96, 64, 32, 0]);
        assert!((at(11).ring.unwrap().1 - 2.0).abs() < 1e-5);
        // Points alpha 128 - 8 (u - 6), 0 from u 22; scale 0.1 .. 1.1 (u 6-11).
        assert_eq!((at(6).pts.unwrap().0, at(21).pts.unwrap().0, at(22).pts.unwrap().0), (128, 8, 0));
        assert!((at(6).pts.unwrap().1 - 0.1).abs() < 1e-6 && (at(11).pts.unwrap().1 - 1.1).abs() < 1e-5 && (at(12).pts.unwrap().1 - 1.1).abs() < 1e-5);
        // Smoke count 2 at u 6 rising to 8 at u 12 (hold 1, start frame -1).
        assert_eq!((at(6).smoke, at(12).smoke), (1 + 1, 8));
    }

    #[test]
    fn smoke_start_frame_uses_c_remainder() {
        // Frame -1 -> UV (96, 192) (C: -1 % 4 = -1, -1 / 4 = 0).
        let (eye, target) = trap(3, 3, 0);
        let mut rng = Rng::new(2);
        let mut m = C505::new((3, 3), &mut || rng.next());
        for u in 1..=6 {
            m.update(u, 0, &mut || rng.next(), eye, target);
        }
        let first = m.additive.iter().find(|t| t.colour[0][..3] == GOLD && t.uv[0][1] == 0.75).unwrap();
        assert_eq!(first.uv[0], [96.0 / 256.0, 0.75]);
    }
}
