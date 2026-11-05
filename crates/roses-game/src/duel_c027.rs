//! CMODULE c027 (0x1B), Change of Heart on the target (event 0x1A, handler
//! 0x25E300, launched at E+128 after c026), as specified in
//! the project notes section 2 [F code; emu: the
//! reference model `tools/mf_c027_model.py` equals the interpreter's
//! primitives to 0.0626 px in four runs]. Billboards anchored at the target
//! unit's board position U (the cell centre, y 0), every local point at z 30
//! (toward the eye) and lifted 30 in screen-up:
//! - GLOW (slot 0, u 1-41): the mirrored "eye" emblem, 4 quadrant quads of
//!   TEXEFF 15 (176,0)-(235,31), 74 x 36 units, each sent 3 times; alpha
//!   16/U to 128, colour (64,32,48) turning violet (44,0,88) from u 13;
//! - BOLTS (u 10-38): 7 waves (u 10, 13, .. 28) of 6 lightning chains at a
//!   random screen angle, each a chain of up to 6 ribbon quads (TEXEFF 15
//!   (10,0)-(53,127)), growing 7/U, jittered every update, flickering
//!   (176,176,0) / (48,48,0).
//!
//! Returns 0, then 8 at u 44 (never 2). SE bank 1 0x22 at u 1. 2,675 random
//! values (5 at u 1, 1 per root, 2 per live bolt per update).
//!
//! The pool is kept as in the original (70 slots, lowest free slot, data kept
//! after death, parent / child as slot indices followed even when stale) so
//! the same-pass rule and the red first draws of low-slot children are exact.
//!
//! [H / not reproduced]: the port's random values (its own LCG, same call
//! order and count); the original depth-tests these quads with real Z (the
//! port draws them after the scene, in slot order, all additive).
use crate::duel_c022::{dcos, dsin, project4, quad, uv15, Rng};
use crate::duel_c602::{billboard_axes, projector};
use crate::duel_c621::FxTri;

/// Module life (0x2C).
pub const LIFE: i32 = 44;
pub const END_U: u32 = 44;
pub const RET_DONE: u32 = 8;
/// SE (SECTL bank 1, record 0x1B) played at `SE_U`.
pub const SE_BANK: u8 = 1;
pub const SE_ID: u16 = 0x22;
pub const SE_U: u32 = 1;
/// Pool slots (D+0xF0, 0x7C bytes each).
pub const SLOTS: usize = 70;

/// The board position of cell (col, row) (0x1F6DE0: cell centre, y 0).
pub fn cell_centre(col: i32, row: i32) -> [f64; 3] {
    [50.0 * f64::from(col - 3), 0.0, 50.0 * f64::from(row - 3)]
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Kind {
    #[default]
    Glow,
    Bolt,
}

/// A 0x7C-byte slot (zeroed on allocation, kept after death).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Slot {
    active: bool,
    kind: Kind,
    /// Start point (+0, +4, +8) and tip (+0x10, +0x14 = r, +0x18).
    start: [f32; 3],
    tip: [f32; 3],
    /// Jitters +0x1C (tip, rand % 40 - 20) and +0x0C (root foot, rand % 20 - 10).
    j1: f32,
    j2: f32,
    /// +0x48 (degrees).
    angle: i32,
    parent: Option<usize>,
    child: Option<usize>,
    /// RGBA +0x60 / +0x64.
    c1: [u8; 4],
    c2: [u8; 4],
    /// +0x78 growth counter, +0x76 flicker toggle.
    counter: i32,
    toggle: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct C027 {
    rng: Rng,
    cnt: i32,
    life: i32,
    /// D+0x2368: set once the glow reaches 128.
    gate: bool,
    /// D+0x2372 glow colour.
    g: [u8; 3],
    /// D+0x22E8 "last spawned" (the next bolt's parent).
    last: Option<usize>,
    /// D+0x234E angle of the next bolt.
    next_angle: i32,
    slots: Vec<Slot>,
    done: bool,
    counts: [usize; 2],
    /// Additive triangles of the last update (glow 3 x 4 quads, bolts).
    pub additive: Vec<FxTri>,
}

impl C027 {
    /// Init (`entry(D, param)`; the param is saved, unused). Draws nothing.
    pub fn new(seed: u32) -> Self {
        Self {
            rng: Rng::new(seed),
            cnt: 0,
            life: LIFE,
            gate: false,
            g: [64, 32, 48],
            last: None,
            next_angle: 0,
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

    /// Quads sent in the last update: [glow (12 = 4 x 3), bolts].
    pub fn counts(&self) -> [usize; 2] {
        self.counts
    }

    pub fn prims(&self) -> usize {
        self.counts[0] + self.counts[1]
    }

    /// Glow colour (D+0x2372).
    pub fn glow_colour(&self) -> [u8; 3] {
        self.g
    }

    /// Live bolts.
    pub fn bolts(&self) -> usize {
        self.slots.iter().filter(|s| s.active && s.kind == Kind::Bolt).count()
    }

    fn alloc(&mut self, s: Slot) -> Option<usize> {
        let i = self.slots.iter().position(|s| !s.active)?;
        self.slots[i] = Slot { active: true, ..s };
        Some(i)
    }

    /// 0x1E00AA0: a bolt with parent = last.
    fn spawn_bolt(&mut self) {
        let mut s = Slot { kind: Kind::Bolt, c1: [128, 0, 0, 0], c2: [128, 0, 0, 0], parent: self.last, angle: self.next_angle, counter: 4, ..Slot::default() };
        if let Some(p) = self.last {
            s.tip = self.slots[p].tip;
            s.start = s.tip;
        }
        if let Some(i) = self.alloc(s) {
            if let Some(p) = self.last {
                self.slots[p].child = Some(i);
            }
        }
    }

    /// 0x1E00080.
    fn spawner(&mut self) {
        self.cnt += 1;
        if self.cnt == 1 {
            self.gate = false;
            self.g = [64, 32, 48];
            // 0x1E008B0: 5 rands (+0x6C, +0x6E angles overwritten with 360
            // every update; three +-30.0 never used).
            for _ in 0..5 {
                self.rng.next();
            }
            self.alloc(Slot { kind: Kind::Glow, c1: [128, 0, 0, 0], c2: [128, 0, 0, 0], ..Slot::default() });
        }
        if self.cnt % 3 == 1 && self.life >= 17 && self.gate {
            for _ in 0..6 {
                self.last = None;
                self.next_angle = self.rng.next() % 360;
                self.spawn_bolt();
            }
        }
    }

    /// 0x1E00290.
    fn glow_update(&mut self, i: usize) {
        let s = &mut self.slots[i];
        if self.life < 6 {
            if s.c1[3] >= 49 {
                s.c1[3] -= 48;
            }
            if s.c2[3] >= 49 {
                s.c2[3] -= 48;
            } else {
                s.active = false;
            }
        } else {
            if s.c1[3] < 128 {
                s.c1[3] += 16;
            }
            if s.c2[3] < 128 {
                s.c2[3] += 16;
            }
            if s.c2[3] >= 128 {
                self.gate = true;
            }
        }
        if self.cnt >= 13 {
            if self.g[0] >= 49 {
                self.g[0] -= 10;
            }
            self.g[1] = 0;
            if self.g[2] < 80 {
                self.g[2] += 10;
            }
        }
        if self.life < 2 {
            s.active = false;
        }
    }

    /// 0x1E003A0.
    fn bolt_update(&mut self, i: usize) {
        let k = self.slots[i].counter;
        if k >= 2 {
            let s = &mut self.slots[i];
            if s.tip[1] < 120.0 {
                s.tip[1] += 7.0;
            }
            s.counter = k - 1;
        } else if k == 1 {
            if self.slots[i].tip[1] < 120.0 {
                self.last = Some(i);
                self.next_angle = self.slots[i].angle;
                self.spawn_bolt();
            }
            self.slots[i].counter = 0;
        } else if k == 0 {
            if self.slots[i].c1[3] >= 33 {
                self.slots[i].c1[3] -= 32;
            } else {
                if let Some(c) = self.slots[i].child {
                    self.slots[c].c1 = self.slots[i].c2;
                    self.slots[c].parent = None;
                }
                self.slots[i].active = false;
            }
            if self.slots[i].c2[3] >= 33 {
                self.slots[i].c2[3] -= 32;
            }
        }
        let j1 = (self.rng.next() % 40 - 20) as f32;
        let j2 = (self.rng.next() % 20 - 10) as f32;
        let life = self.life;
        let s = &mut self.slots[i];
        s.j1 = j1;
        s.j2 = j2;
        let rgb = if s.toggle { [48, 48, 0] } else { [176, 176, 0] };
        s.toggle = !s.toggle;
        s.c1[..3].copy_from_slice(&rgb);
        s.c2[..3].copy_from_slice(&rgb);
        if life < 10 {
            if s.c1[3] >= 17 {
                s.c1[3] -= 16;
            }
            if s.c2[3] >= 17 {
                s.c2[3] -= 16;
            } else {
                s.active = false;
            }
        } else if s.counter != 0 {
            if s.c1[3] < 176 {
                s.c1[3] = s.c1[3].wrapping_add(58);
            }
            if s.c2[3] < 176 {
                s.c2[3] = s.c2[3].wrapping_add(58);
            }
        }
    }

    /// Module update `u` (1, 2, ..): `unit` = the target's board position
    /// (`cell_centre`), (`eye`, `target`) the live camera. Returns 0, or 8 at
    /// u 44 (and every later call).
    pub fn update(&mut self, _u: u32, unit: [f64; 3], eye: [f64; 3], target: [f64; 3]) -> u32 {
        self.additive.clear();
        self.counts = [0; 2];
        if self.done {
            return RET_DONE;
        }
        self.spawner();
        // Slot order: a slot spawned at a higher index is updated in this pass.
        for i in 0..SLOTS {
            if self.slots[i].active {
                match self.slots[i].kind {
                    Kind::Glow => self.glow_update(i),
                    Kind::Bolt => self.bolt_update(i),
                }
            }
        }
        if let Some(project) = projector(eye, target) {
            let project: &dyn Fn([f64; 3]) -> Option<[f32; 2]> = &project;
            let [r, up, back] = billboard_axes(eye, target);
            let at = |l: [f32; 3]| -> [f32; 3] { std::array::from_fn(|k| (unit[k] + f64::from(l[0]) * r[k] + f64::from(l[1]) * up[k] + f64::from(l[2]) * back[k]) as f32) };
            for i in 0..SLOTS {
                let s = self.slots[i];
                if !s.active {
                    continue;
                }
                match s.kind {
                    Kind::Glow => self.glow_draw(&s, project, &at),
                    Kind::Bolt => self.bolt_draw(&s, project, &at),
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

    /// 0x1E00C50: 4 quadrants (angles 360: no rotation, no offset), y + 30,
    /// z 30; each sent 3 times.
    fn glow_draw(&mut self, s: &Slot, project: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, at: &dyn Fn([f32; 3]) -> [f32; 3]) {
        const Q: [[[f32; 2]; 4]; 4] = [
            [[-37.0, 18.0], [0.0, 18.0], [-37.0, 0.0], [0.0, 0.0]],
            [[37.0, 18.0], [0.0, 18.0], [37.0, 0.0], [0.0, 0.0]],
            [[-37.0, -18.0], [0.0, -18.0], [-37.0, 0.0], [0.0, 0.0]],
            [[37.0, -18.0], [0.0, -18.0], [37.0, 0.0], [0.0, 0.0]],
        ];
        let [r, g, b] = self.g;
        let (c1, c2) = ([r, g, b, s.c1[3]], [r, g, b, s.c2[3]]);
        let uv = uv15([[176, 0], [235, 0], [176, 31], [235, 31]]);
        for q in Q {
            let Some(v) = project4(project, q.map(|[x, y]| at([x, y + 30.0, 30.0]))) else {
                continue;
            };
            for _ in 0..3 {
                self.counts[0] += 1;
                quad(&mut self.additive, v, uv, [c1, c1, c2, c2], true, 15);
            }
        }
    }

    /// 0x1E01930: one ribbon quad from the parent's tip (or the start point)
    /// to this tip, rotated by the bolt's angle (0x109230 mode 2).
    fn bolt_draw(&mut self, s: &Slot, project: &dyn Fn([f64; 3]) -> Option<[f32; 2]>, at: &dyn Fn([f32; 3]) -> [f32; 3]) {
        let rgb = [s.c1[0], s.c1[1], s.c1[2]];
        let top = if s.child.is_some() { s.c1 } else { [rgb[0], rgb[1], rgb[2], 0] };
        let p = s.parent.map(|p| self.slots[p]);
        let bot = p.map_or([rgb[0], rgb[1], rgb[2], 10], |p| p.c2);
        let (x0, x1) = (s.j1 + (s.tip[0] - 6.0), s.j1 + (s.tip[0] + 6.0));
        let (x2, x3, yb) = match p {
            Some(p) => (p.j1 + (p.tip[0] - 6.0), p.j1 + (p.tip[0] + 6.0), p.tip[1].trunc()),
            None => (s.j2 + (s.start[0] - 6.0), s.j2 + (s.start[0] + 6.0), s.start[1].trunc()),
        };
        let yt = s.tip[1];
        let (c, sn) = (dcos(s.angle), dsin(s.angle));
        let rot = |x: f32, y: f32| at([x * c + y * sn, (x * -sn + y * c) + 30.0, 30.0]);
        let w = [rot(x0, yt), rot(x1, yt), rot(x2, yb), rot(x3, yb)];
        let Some(v) = project4(project, w) else {
            return;
        };
        self.counts[1] += 1;
        let uv = uv15([[10, 0], [53, 0], [10, 127], [53, 127]]);
        quad(&mut self.additive, v, uv, [top, top, bot, bot], true, 15);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duel_c022::tests::{overview, trap};

    const UNIT: [f64; 3] = [0.0, 0.0, 100.0];

    fn run(cam: ([f64; 3], [f64; 3]), unit: [f64; 3], seed: u32) -> (C027, Vec<(u32, [usize; 2], u32, [u8; 3])>) {
        let mut c = C027::new(seed);
        let mut v = Vec::new();
        for u in 1..=60 {
            let n0 = c.rand_calls();
            let r = c.update(u, unit, cam.0, cam.1);
            v.push((r, c.counts(), c.rand_calls() - n0, c.glow_colour()));
            if r == 8 {
                break;
            }
        }
        (c, v)
    }

    #[test]
    fn counts_returns_and_rand_follow_the_doc() {
        for (cam, unit, seed) in [(trap(3, 5), UNIT, 1), (trap(2, 6), cell_centre(2, 6), 9), (overview(), cell_centre(2, 6), 4)] {
            let (c, v) = run(cam, unit, seed);
            assert_eq!(v.len(), 44);
            assert!(c.done());
            assert!(v[..43].iter().all(|x| x.0 == 0) && v[43].0 == 8);
            let q = |u: usize| v[u - 1].1;
            for u in 1..=41 {
                assert_eq!(q(u)[0], 12, "u {u}");
            }
            for u in 42..=44 {
                assert_eq!(q(u), [0, 0]);
            }
            for u in 1..=9 {
                assert_eq!(q(u)[1], 0);
            }
            // Timeline: 18 at u 10, 30 / 48 at u 13 / 16, max 81 at u 26, 12 from u 39.
            assert_eq!(q(10)[0] + q(10)[1], 18);
            assert_eq!(q(13)[0] + q(13)[1], 30);
            assert_eq!(q(16)[0] + q(16)[1], 48);
            assert_eq!(q(26)[1], 69);
            assert_eq!(v.iter().map(|x| x.1[1]).max(), Some(69));
            for u in 39..=41 {
                assert_eq!(q(u)[1], 0);
            }
            // 1,689 primitives (glow 492, bolts 1,197); 2,675 rand.
            assert_eq!(v.iter().map(|x| x.1[0]).sum::<usize>(), 492);
            assert_eq!(v.iter().map(|x| x.1[1]).sum::<usize>(), 1_197);
            assert_eq!(c.rand_calls(), 2_675);
            assert_eq!(v[0].2, 5);
        }
    }

    #[test]
    fn glow_alpha_and_colour() {
        let (eye, target) = trap(3, 5);
        let mut c = C027::new(2);
        for u in 1..=44u32 {
            c.update(u, UNIT, eye, target);
            let glow: Vec<_> = c.additive.iter().filter(|t| t.uv[0][0] >= 176.0 / 256.0).collect();
            let a = match u {
                1..=8 => Some(16 * u as u8),
                9..=39 => Some(128),
                40 => Some(80),
                41 => Some(32),
                _ => None,
            };
            let col = match u {
                1..=12 => [64, 32, 48],
                13 => [54, 0, 58],
                14 => [44, 0, 68],
                15 => [44, 0, 78],
                _ => [44, 0, 88],
            };
            match a {
                Some(a) => {
                    assert_eq!(glow.len(), 24, "u {u}");
                    assert!(glow.iter().all(|t| t.colour.iter().all(|c| *c == [col[0], col[1], col[2], a])), "u {u}");
                }
                None => assert!(glow.is_empty()),
            }
        }
    }

    #[test]
    fn glow_is_centred_above_the_unit() {
        // The quadrants meet at U + 30 up + 30 toward the eye.
        let (eye, target) = trap(3, 5);
        let mut c = C027::new(2);
        c.update(1, UNIT, eye, target);
        let [_, up, back] = billboard_axes(eye, target);
        let centre: [f64; 3] = std::array::from_fn(|k| UNIT[k] + 30.0 * up[k] + 30.0 * back[k]);
        let want = projector(eye, target).unwrap()(centre).unwrap();
        // v3 of every quadrant (second triangle's vertex 1) is the centre.
        for t in c.additive.iter().skip(1).step_by(2) {
            assert!((t.screen[1][0] - want[0]).abs() < 0.01 && (t.screen[1][1] - want[1]).abs() < 0.01);
        }
    }

    #[test]
    fn bolts_grow_flicker_and_chain() {
        let (eye, target) = trap(3, 5);
        let mut c = C027::new(3);
        for u in 1..=10 {
            c.update(u, UNIT, eye, target);
        }
        // Wave 1: 6 roots, r 7 after the spawn update, alpha 58, bright.
        let roots: Vec<Slot> = c.slots.iter().filter(|s| s.active && s.kind == Kind::Bolt).copied().collect();
        assert_eq!(roots.len(), 6);
        assert!(roots.iter().all(|s| s.tip[1] == 7.0 && s.c1 == [176, 176, 0, 58] && s.parent.is_none() && s.counter == 3));
        c.update(11, UNIT, eye, target);
        assert!(c.slots.iter().filter(|s| s.active && s.kind == Kind::Bolt).all(|s| s.c1 == [48, 48, 0, 116] && s.tip[1] == 14.0));
        c.update(12, UNIT, eye, target);
        c.update(13, UNIT, eye, target);
        // u 13: wave 2 (6 new roots) and the first children (r 21 -> start at the parent's tip).
        let kids: Vec<Slot> = c.slots.iter().filter(|s| s.active && s.parent.is_some()).copied().collect();
        assert_eq!(kids.len(), 6);
        assert!(kids.iter().all(|s| s.start[1] == 21.0));
        assert_eq!(c.bolts(), 18);
        // Chains end at 6 segments (tip radius 126).
        let mut c = C027::new(3);
        let mut max_r = 0.0f32;
        for u in 1..=44 {
            c.update(u, UNIT, eye, target);
            max_r = c.slots.iter().filter(|s| s.active && s.kind == Kind::Bolt).map(|s| s.tip[1]).fold(max_r, f32::max);
        }
        assert_eq!(max_r, 126.0);
        assert_eq!(c.update(45, UNIT, eye, target), 8);
        assert!(c.additive.is_empty());
    }
}
