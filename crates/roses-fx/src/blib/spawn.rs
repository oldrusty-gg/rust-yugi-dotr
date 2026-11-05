//! Particle set-up routines of the effect library.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};

/// 0.1 as the code builds it (0x3DCCCCCD).
const TENTH: u32 = 0x3DCC_CCCD;

/// e7b7d82a9ae5 [54 B records; b036 0x1E01640] [F]: `scatter(g, life,
/// speed) -> 0` gives each of the group's g.i16(+0x2C) particles (16-byte
/// records from the pointer at g+0xC) a start: with flag 0x1D of g+4 a
/// flat ring (angle rand & 0xFFFF, radius (rand % k) / 10 with k =
/// 0x1A2B80(10 g.f(+0x28)); x = r sin, y = `speed`, z = r cos), else a
/// random direction of length `speed` (0x1043F0 into the stack local
/// sp + 0x80); then its life -(rand % `life`) at +0xE, the group's byte
/// g+0xB at +0xC and 0 at +0xD. A 0x90 frame.
pub fn scatter(fx: &mut Fx, g: u32, life: i32, speed: f32) -> i32 {
    let sp = fx.enter(0x90);
    let k = ee::fixsfsi(ee::mul(10.0, fx.mem.f(g + 0x28)));
    let mut off = 0u32;
    let mut i = 0;
    while i < fx.mem.i16(g + 0x2C) {
        let flags = fx.mem.read_u32(g + 4);
        if bit(flags, 0x1D) {
            let a = fx.rand() & 0xFFFF;
            let r = fx.rand();
            let (_, m) = fx.div(r, k);
            let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
            fx.mem.setf(e + 4, speed);
            let rad = ee::mul(f32::from_bits(TENTH), ee::from_i32(m));
            let c = fx.tcos(a);
            let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
            fx.mem.setf(e + 8, ee::mul(rad, c));
            let s = fx.tsin(a);
            let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
            fx.mem.setf(e, ee::mul(rad, s));
        } else {
            fx.random_dir(sp + 0x80, speed);
            for k in 0..3u32 {
                let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
                let v = fx.mem.f(sp + 0x80 + 4 * k);
                fx.mem.setf(e + 4 * k, v);
            }
        }
        let r = fx.rand();
        let (_, m) = fx.div(r, life);
        i += 1;
        let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
        fx.mem.set16(e + 0xE, m.wrapping_neg());
        let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
        let b = fx.mem.read_u8(g + 0xB);
        fx.mem.write_u8(e + 0xC, b);
        let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
        off += 0x10;
        fx.mem.write_u8(e + 0xD, 0);
    }
    fx.leave(0x90);
    0
}

/// a6cf8ad55488 [3 B records: b606 0x1E02750, b396 0x1E025D0, b533
/// 0x1E034D0; 108 words] [F]: `task400(D, step, arg) -> 1 /
/// 0`: the first free of 400 tasks (0x70 bytes at D+0x2A0, alive +0x58)
/// cleared and set off: alive 1, the colour D+0xB214 (bytes, 0x107FD0)
/// with its alpha +0xB217 at +0x60 and +0x64, D+0x250 as i16 at +0
/// (0x107390) and its xyz at +0x10, D+0xB2B0 at +0x50, `arg` +0x40,
/// `step` +0x44; 0 when none is free. `saves`: what its prologue (0x60:
/// s0 +0 .. s4 +0x40, ra +0x50) stores, the caller's registers.
pub fn task400(fx: &mut Fx, d: u32, f: u32, arg: u32, saves: &[(u32, u64)]) -> i32 {
    task400_slot(fx, d, f, arg, 0xB2B0, saves)
}

/// 8154ee20301e [3 B records; b646 0x1E02D70, 100 words]:
/// [`task400`] with +0x50 from D+0xB2B4 [F word-compare: only that
/// offset differs].
pub fn task400_b4(fx: &mut Fx, d: u32, f: u32, arg: u32, saves: &[(u32, u64)]) -> i32 {
    task70(fx, d, POOL_B4, f, arg, saves)
}

/// [`task400_b4`]'s layout: 400 tasks at D+0x2A0, colour D+0xB214, point
/// D+0x250, +0x50 from D+0xB2B4.
pub const POOL_B4: Pool70 = Pool70 { base: 0x2A0, n: 400, colour: 0xB214, pos: 0x250, slot: 0xB2B4 };

/// The layout variants of 8154ee20301e [F fdiff: the same
/// 100 words, only the pool size (the immediates at +0x68 / +0x90), the
/// colour (lui +0xA4 / lbu +0xAC..) and the +0x50 slot (lui +0x130 / lh
/// +0x150) differ, and in card 254's c9381311efc0 the base D+0x5380 and
/// the point D+0x2D0 too; elsewhere D+0x2A0 / D+0x250]: (hash, layout).
pub const POOL70_B4: &[(&str, Pool70)] = &[
    ("8154ee20301e", POOL_B4),
    ("c7a311fb8c14", Pool70 { base: 0x2A0, n: 150, colour: 0xA904, pos: 0x250, slot: 0xAA0A }),
    ("808b8d306d7e", Pool70 { base: 0x2A0, n: 100, colour: 0x97C4, pos: 0x250, slot: 0x9860 }),
    ("6fd147a7dc0d", Pool70 { base: 0x2A0, n: 130, colour: 0x9FF4, pos: 0x250, slot: 0xE66E }),
    ("57042c7c1333", Pool70 { base: 0x2A0, n: 130, colour: 0x9FF4, pos: 0x250, slot: 0xA0AA }),
    ("558b8cebed3f", Pool70 { base: 0x2A0, n: 150, colour: 0xA904, pos: 0x250, slot: 0xAD56 }),
    ("f55f8bcd1468", Pool70 { base: 0x2A0, n: 130, colour: 0x9AD4, pos: 0x250, slot: 0x9DFA }),
    ("03c321c0da42", Pool70 { base: 0x2A0, n: 400, colour: 0xCB14, pos: 0x250, slot: 0xCBB0 }),
    ("780c0891d8d9", Pool70 { base: 0x2A0, n: 400, colour: 0xB354, pos: 0x250, slot: 0xB3F0 }),
    ("c9381311efc0", Pool70 { base: 0x5380, n: 320, colour: 0xEC84, pos: 0x2D0, slot: 0xECE6 }),
    ("ae03cf745317", Pool70 { base: 0x2A0, n: 150, colour: 0xA894, pos: 0x250, slot: 0xA8EA }),
    ("4ac5b35fe8b8", Pool70 { base: 0x2A0, n: 150, colour: 0xBC94, pos: 0x250, slot: 0xBCEA }),
    ("924e92a3a7b1", Pool70 { base: 0x2A0, n: 150, colour: 0xBD04, pos: 0x250, slot: 0xC20A }),
];

/// The layout of a [`POOL70_B4`] hash.
pub fn pool70_b4(hash: &str) -> Option<Pool70> {
    POOL70_B4.iter().find(|(h, _)| *h == hash).map(|&(_, p)| p)
}

fn task400_slot(fx: &mut Fx, d: u32, f: u32, arg: u32, slot: u32, saves: &[(u32, u64)]) -> i32 {
    task70(fx, d, Pool70 { base: 0x2A0, n: 400, colour: 0xB214, pos: 0x250, slot }, f, arg, saves)
}

/// A pool of 0x70-byte tasks (alive +0x58) at D + `base`, `n` tasks; the
/// spawn colour D + `colour`, the spawn point D + `pos`, +0x50 from the
/// i16 D + `slot` (D-relative offsets).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pool70 {
    pub base: u32,
    pub n: u32,
    pub colour: u32,
    pub pos: u32,
    pub slot: u32,
}

/// [`task400`] on the layout `p`: the first free task of the pool cleared
/// and set off; 1, or 0 when none is free.
pub fn task70(fx: &mut Fx, d: u32, p: Pool70, f: u32, arg: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..p.n).find(|&k| fx.mem.i16(d + p.base + 0x70 * k + 0x58) == 0) {
        Some(k) => {
            let t = d + p.base + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            let c = d + p.colour;
            for o in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(c), fx.mem.read_u8(c + 1), fx.mem.read_u8(c + 2));
                crate::fxlib2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(c + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            crate::fxlib2::to_i16x4(fx, t, d + p.pos, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(d + p.pos + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(d + p.slot);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, arg);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 1f31920d0d56 [2 B records; b499 0x1E03760, b479 0x1E03490; 100 words]
/// [F]: [`task400`] on 300 tasks at D+0x35F0, the colour
/// D+0xBAB4, the point D+0x2D0, +0x50 from D+0xBB10.
pub fn task300(fx: &mut Fx, d: u32, f: u32, arg: u32, saves: &[(u32, u64)]) -> i32 {
    task70(fx, d, Pool70 { base: 0x35F0, n: 300, colour: 0xBAB4, pos: 0x2D0, slot: 0xBB10 }, f, arg, saves)
}

/// e8e1debea8f3 [2 B records; b499 0x1E035D0, b479 0x1E03300; 100 words]
/// [F]: `task200(D, step, arg) -> 1 / 0`: the first free of
/// 200 tasks (0x40 bytes at D+0x3F0, alive +0x36) cleared and set off:
/// alive 1, the colour D+0xBAB4 (bytes) with its alpha at +0x28 and +0x2C,
/// D+0x2D0 as i16 at +0 (0x107390), the words D+0x2F0..0x2F8 at +0x10, the
/// i16 D+0xBB08 at +0x3A and D+0xBB10 at +0x30, `arg` +0x20, `step` +0x24;
/// 0 when none is free. `saves` as [`task400`]'s.
pub fn task200(fx: &mut Fx, d: u32, f: u32, arg: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..200u32).find(|&k| fx.mem.i16(d + 0x40 * k + 0x426) == 0) {
        Some(k) => {
            let t = d + 0x3F0 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            let c = d + 0xBAB4;
            for o in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(c), fx.mem.read_u8(c + 1), fx.mem.read_u8(c + 2));
                crate::fxlib2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(c + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            crate::fxlib2::to_i16x4(fx, t, d + 0x2D0, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(d + 0x2F0 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(d + 0xBB08);
            fx.mem.set16(t + 0x3A, a);
            let a = fx.mem.i16(d + 0xBB10);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, arg);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}


/// a72bfb6bb70a [31 B records; b284 0x1E00080, 52 words]:
/// `node_history(p, n, slot) -> 0`: a history of `n` positions ending at
/// `p` (16-byte entries downwards, xyz only): entry k (at p - 0x10 k)
/// takes entry k + 1's, the last (p - 0x10 (n - 1)) the attacker's node
/// `slot` (0x1F6830 into its frame's sp + 0x60, read even when n <= 0). A
/// 0x70 frame [F].
pub fn node_history(fx: &mut Fx, p: u32, n: i32, slot: i32) -> i32 {
    let sp = fx.enter(0x70);
    let h = fx.self_handle();
    fx.node_position(h, slot, sp + 0x60);
    for k in 0..n.max(0) as u32 {
        let e = p.wrapping_sub(0x10 * k);
        let src = if k as i32 == n - 1 { sp + 0x60 } else { e.wrapping_sub(0x10) };
        for c in 0..3 {
            let w = fx.mem.read_u32(src + 4 * c);
            fx.mem.write_u32(e + 4 * c, w);
        }
    }
    fx.leave(0x70);
    0
}

/// 8a49c57fb3bb [50 B records; b073 0x1E00080, 36 words, a leaf] (helper
/// A7): `history(p, n, _, new) -> 0`: [`node_history`] with the newest
/// entry `new` (xyz) instead of a node: entry k (at p - 0x10 k) takes entry
/// k + 1's, the last (k = n - 1) `new`'s [F].
pub fn history(fx: &mut Fx, p: u32, n: i32, new: u32) -> i32 {
    let mut k = 0i32;
    while k < n {
        let e = p.wrapping_sub(0x10 * k as u32);
        let src = if n - 1 - k == 0 { new } else { e.wrapping_sub(0x10) };
        for c in 0..3 {
            let w = fx.mem.read_u32(src + 4 * c);
            fx.mem.write_u32(e + 4 * c, w);
        }
        k = (k + 1) as i16 as i32;
    }
    0
}

/// af061e92e92f [4 B records; b258 0x1E027A0, 56 words]: the
/// burning-disc step `(D, task) -> 0` at record address `at`: on its first
/// update size 430, colours (0xC0, 0x70, 0) / (0x20, 0x10, 0), alpha
/// +0x2F 0x10, a random angle (rand % 360, rand's frame from `at` + 0x54);
/// alpha +0x2B 0x80 for 5 updates, then down by 10 (the task ends under
/// 0x11); the age +0x32 counts up. A 0x20 frame [F].
pub fn disc_step(fx: &mut Fx, at: u32, t: u32) -> i32 {
    disc_step_with(fx, DISC, at, t)
}

/// The constants of a [`disc_step`] copy: size +0x38, colours +0x28 /
/// +0x2C, the ages holding alpha `alpha`, the fade step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Disc {
    pub size: i32,
    pub c1: (i32, i32, i32),
    pub c2: (i32, i32, i32),
    pub hold: i32,
    pub alpha: u8,
    pub fade: u8,
}

/// af061e92e92f's constants.
pub const DISC: Disc = Disc { size: 0x1AE, c1: (0xC0, 0x70, 0), c2: (0x20, 0x10, 0), hold: 5, alpha: 0x80, fade: 0xA };

/// The copies of af061e92e92f [F fdiff / cols.py: the same 56
/// words and calls (rand returning to +0x54) with other immediates; card
/// 140's 0x1E02BF0 schedules its grey colour's registers differently]:
/// (hash, constants).
pub const DISCS: &[(&str, Disc)] = &[
    ("af061e92e92f", DISC),
    ("78621d35edfb", Disc { size: 0xE6, c1: (0xC0, 0xC0, 0xC0), c2: (0x20, 0x10, 0), hold: 5, alpha: 0xC0, fade: 0xA }),
    ("5225257f1409", Disc { size: 0x14A, c1: (0xC0, 0x70, 0), c2: (0x20, 0x10, 0), hold: 5, alpha: 0x80, fade: 0xA }),
    ("c7cfb5b8be8e", Disc { size: 0x14A, c1: (0xC0, 0x70, 0), c2: (0x20, 0x10, 0), hold: 5, alpha: 0xC0, fade: 0xF }),
    ("38953db3edd0", Disc { size: 0x726, c1: (0xC0, 0x70, 0), c2: (0x20, 0x10, 0), hold: 0x78, alpha: 0x80, fade: 0xA }),
    ("eec8ab51b79f", Disc { size: 0x1F4, c1: (0x30, 0x30, 0x80), c2: (0x10, 0x10, 0x80), hold: 5, alpha: 0x80, fade: 0xA }),
    ("ede11d4434f2", Disc { size: 0x1AE, c1: (0xC0, 0x30, 0), c2: (0x40, 0x10, 0), hold: 5, alpha: 0x80, fade: 0xA }),
];

/// The constants of a [`DISCS`] hash.
pub fn disc(hash: &str) -> Option<Disc> {
    DISCS.iter().find(|(h, _)| *h == hash).map(|&(_, d)| d)
}

/// [`disc_step`] with the constants `k`.
pub fn disc_step_with(fx: &mut Fx, k: Disc, at: u32, t: u32) -> i32 {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, k.size);
        crate::fxlib2::rgb(fx, t + 0x28, k.c1.0, k.c1.1, k.c1.2);
        crate::fxlib2::rgb(fx, t + 0x2C, k.c2.0, k.c2.1, k.c2.2);
        let v = fx.rand_from(t, at + 0x54);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0x10);
    }
    if fx.mem.i16(t + 0x32) < k.hold {
        fx.mem.write_u8(t + 0x2B, k.alpha);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - k.fade);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
    0
}

/// rand % m (HI) from a step whose s0 is the task `t`, rand returning to
/// `ra`.
fn rand_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// 3e182ce461cf [3 B records; b297 0x1E01B70, 148 words] and its twin
/// 7a22a7e3f037 [2; b297 0x1E01DC0] [F]: `ring_rise(D, t) ->
/// 0`, the step of a rising ring task (ring_at_mt's, round the target):
/// on its first update size +0x38 45, frame +0x34 0, colour +0x28 grey `c`
/// (the immediate of the `addiu a1` at +0x1C: 0x80, 0x20 in the twin),
/// radius +0x30 130..429, angle +0x3A random, growth +0xE 0, row +8
/// 0..34, quads +0xC 2..4, life +6 15..24; then alpha +0x2B up by 0x12
/// (while at most 0x80) during the life, else down by 9 (the task ends
/// below 0xB); the size grows by 2 below 90 and again by 2, the frame
/// steps on odd ages below 5, the radius by +0xE, y +2 rises by 1, the
/// angles +0x10 / +0x14 / +0x18 random each update. A 0x20 frame (s0 +0,
/// ra +0x10: the caller writes them); rand's frames from s0 = t.
pub fn ring_rise(fx: &mut Fx, at: u32, t: u32) -> i32 {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        let c = (fx.mem.read_u32(at + 0x1C) & 0xFFFF) as i16 as i32;
        fx.mem.set16(t + 0x38, 0x2D);
        fx.mem.set16(t + 0x34, 0);
        crate::fxlib2::rgb(fx, t + 0x28, c, c, c);
        let v = rand_mod(fx, t, at + 0x44, 0x12C);
        fx.mem.set16(t + 0x30, v + 0x82);
        let v = rand_mod(fx, t, at + 0x64, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0xE, 0);
        let v = rand_mod(fx, t, at + 0x84, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_mod(fx, t, at + 0xA0, 3);
        fx.mem.set16(t + 0xC, v + 2);
        let v = rand_mod(fx, t, at + 0xC0, 0xA);
        fx.mem.set16(t + 6, v + 0xF);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x81 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x12));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 && fx.mem.i16(t + 0x38) < 0x5A {
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
    }
    if fx.mem.i16(t + 0x32).wrapping_rem(2) == 1 && fx.mem.i16(t + 0x34) < 5 {
        let f = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, f);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38) + 2;
        fx.mem.set16(t + 0x38, s);
    }
    let y = fx.mem.i16(t + 2) + 1;
    fx.mem.set16(t + 2, y);
    for (o, ra) in [(0x10u32, 0x1D8u32), (0x14, 0x1F4), (0x18, 0x210)] {
        let v = rand_mod(fx, t, at + ra, 0x168);
        fx.mem.set32(t + o, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
    0
}

/// e1f99f393722 [6 B records; b297 0x1E01110, 56 words; spawned only by
/// b499] [F]: `flare_step(D, t) -> 0`: on its first update
/// size +0x38 0x50 and frame +0x34 rand % 4 (C remainder); alpha +0x2B up
/// by 0x50 (below 0xA0) for two updates, then down by 0xF (the task ends
/// at 0x10 or below); colour +0x28 (0x80, 0x40, 0); +0xA += 6, the size
/// grows by 0x14. A 0x20 frame (s0 +0, ra +0x10: the caller writes them);
/// rand's frame from s0 = t.
pub fn flare_step(fx: &mut Fx, at: u32, t: u32) -> i32 {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x50);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v.wrapping_rem(4));
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 2 {
        if a < 0xA0 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x50));
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xF);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    crate::fxlib2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
    let v = fx.mem.i16(t + 0xA) + 6;
    fx.mem.set16(t + 0xA, v);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    let s = fx.mem.i16(t + 0x38) + 0x14;
    fx.mem.set16(t + 0x38, s);
    fx.leave(0x20);
    0
}

/// 6ffed6ec5529 [11 B records; b103's 0x1E00C70, 180 words]
/// [F]: `fan_ring(p)` spawns byte g+0x1D sparks of the group g = *p
/// round a ring (a 0x70 frame; rands with s0 = g from `at` + 0x34 / +
/// 0xFC / + 0x120). The start angle is 180 + (rand % 36000 - 18000) / 100
/// degrees, stepping 360 / g+0x1D (wrapped past 360). Per spark: a free
/// 0x28-byte record of the pool *(g+8) (0x101C30, byte g+0x1C of them;
/// its index into g.i16(+0xC + 2 k), -1 when none), counted p[4] after
/// rand & p[5] (0x101B40), +3 = (rand & 3) << 4; with c / s the libm
/// cosine / sine of the angle: position (p+0x14 + p+0x10 c, p+0x18, p+0x1C
/// + p+0x10 s), velocity (p+8 c, p+0xC, p+8 s), +0x1C / +0x24 = -velocity
/// x / z / p[4], +0x20 = 0.
pub fn fan_ring(fx: &mut Fx, at: u32, p: u32) {
    fx.enter(0x70);
    let g = fx.mem.read_u32(p);
    let r = fx.rand_from(g, at + 0x34) >> 4;
    let (_, m) = fx.div(r, 0x8CA0);
    let mut f20 = ee::add(180.0, ee::div(ee::from_i32(m - 0x4650), 100.0));
    let f21 = ee::div(360.0, ee::from_i32(i32::from(fx.mem.read_u8(g + 0x1D))));
    let mut k = 0u32;
    while (k as i32) < i32::from(fx.mem.read_u8(g + 0x1D)) {
        let (pool, cap) = (fx.mem.read_u32(g + 8), i32::from(fx.mem.read_u8(g + 0x1C)));
        let i = super::exe::first_free(fx, pool, 0x28, cap);
        if i < 0 {
            fx.mem.set16(g + 0xC + 2 * k, -1);
            k += 1;
            continue;
        }
        fx.mem.set16(g + 0xC + 2 * k, i);
        let s1 = fx.mem.read_u32(g + 8).wrapping_add(0x28 * i as u32);
        let r = fx.rand_from(g, at + 0xFC) >> 4;
        let delay = i32::from(fx.mem.read_u8(p + 5)) & r & 0xFF;
        let count = i32::from(fx.mem.read_u8(p + 4));
        super::exe::set_counter(fx, s1, count, delay);
        let r = fx.rand_from(g, at + 0x120) >> 4;
        fx.mem.set8(s1 + 3, (r & 3) << 4);
        f20 = ee::add(f20, f21);
        if !crate::exe::ee_le(f20, 360.0) {
            f20 = ee::sub(f20, 360.0);
        }
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), f20), 180.0);
        let sn = fx.libm_sinf(a);
        let cs = super::exe::libm_cosf(fx, a);
        let r10 = fx.mem.f(p + 0x10);
        let v = ee::add(fx.mem.f(p + 0x14), ee::mul(r10, cs));
        fx.mem.setf(s1 + 4, v);
        let r10 = fx.mem.f(p + 0x10);
        let v = ee::add(fx.mem.f(p + 0x1C), ee::mul(r10, sn));
        fx.mem.setf(s1 + 0xC, v);
        let w = fx.mem.read_u32(p + 0x18);
        fx.mem.write_u32(s1 + 8, w);
        let v8 = fx.mem.f(p + 8);
        fx.mem.setf(s1 + 0x10, ee::mul(v8, cs));
        let v8 = fx.mem.f(p + 8);
        fx.mem.setf(s1 + 0x18, ee::mul(v8, sn));
        let w = fx.mem.read_u32(p + 0xC);
        fx.mem.write_u32(s1 + 0x14, w);
        let n = ee::from_i32(i32::from(fx.mem.read_u8(p + 4)));
        let v = ee::neg(ee::div(fx.mem.f(s1 + 0x10), n));
        fx.mem.setf(s1 + 0x1C, v);
        let n = ee::from_i32(i32::from(fx.mem.read_u8(p + 4)));
        let v = ee::neg(ee::div(fx.mem.f(s1 + 0x18), n));
        fx.mem.setf(s1 + 0x24, v);
        fx.mem.write_u32(s1 + 0x20, 0);
        k += 1;
    }
    fx.leave(0x70);
}

/// 43c09cf2d9f4 [11 B records; b103's 0x1E00F40, 276 words] (helper
/// B10) [F]: `emitters(g, n) -> 0 / 1` (a 0xC0 frame). For each of eight
/// i16 update lists (pointers g+8.., 0 skips one) holding the update `n`
/// (u16): the self unit's node byte g+0x28+k (0x1F65B0 / 0x1F6830 into
/// sp + 0x80; none ends the list) starts a 0x38-byte emitter of the pool
/// *g (0x101C30, byte g+0x30 of them; none returns 0) counted 1 (kind 4,
/// sprite pool g+4, spec bytes g+0x31 / 4 / 1 / g+0x32 / 4, colour 0x90 x3
/// 0x30, sizes g+0x38 / g+0x40, rise -0.015, height the node's y) and
/// fans its sparks (the sibling [`fan_ring`] called at the `jal` +0x180,
/// on the spec at sp + 0x90: 0x20 counts after rand & 3, radius 0.25,
/// speeds 0.3 / 0.15, from the node). Then while g.i16(+0x34) <= n <=
/// g.i16(+0x36), node g+0x33 starts one more (two sparks of 0x20 counts
/// at the node jittered by (rand % 500 - 250) / 100 per axis, still,
/// sizes g+0x3C / g+0x40, rise -0.007). Returns 1, 0 when a node or a
/// pool fails.
pub fn emitters(fx: &mut Fx, at: u32, g: u32, n: i32) -> i32 {
    let sp = fx.enter(0xC0);
    // The prologue's `sq` of s0..s6 / ra (+0 .. +0x70): their upper
    // halves (0) stay below the caller's stack (a later quad's stale XYZ
    // reads them) [run b381].
    for k in 0..8u32 {
        fx.mem.write_u64(sp + 0x10 * k + 8, 0);
    }
    let n = n & 0xFFFF;
    let start = |fx: &mut Fx, kind_y: u32, rise: u32, sparks: u8, size: u32| -> Option<u32> {
        let (pool, cap) = (fx.mem.read_u32(g), i32::from(fx.mem.read_u8(g + 0x30)));
        let i = super::exe::first_free(fx, pool, 0x38, cap);
        if i < 0 {
            return None;
        }
        let r = fx.mem.read_u32(g).wrapping_add(0x38 * i as u32);
        super::exe::set_counter4(fx, r, 1, 0, 0xFF);
        fx.mem.write_u8(r + 4, 4);
        let w = fx.mem.read_u32(g + 4);
        fx.mem.write_u32(r + 8, w);
        let b = fx.mem.read_u8(g + 0x31);
        fx.mem.write_u8(r + 0x1C, b);
        fx.mem.write_u8(r + 0x1D, sparks);
        fx.mem.write_u8(r + 0x1E, 1);
        let b = fx.mem.read_u8(g + 0x32);
        fx.mem.write_u8(r + 0x1F, b);
        fx.mem.write_u8(r + 0x20, 4);
        for o in [0x22u32, 0x23, 0x24] {
            fx.mem.write_u8(r + o, 0x90);
        }
        fx.mem.write_u8(r + 0x25, 0x30);
        let w = fx.mem.read_u32(g + size);
        fx.mem.write_u32(r + 0x28, w);
        let w = fx.mem.read_u32(g + 0x40);
        fx.mem.write_u32(r + 0x2C, w);
        fx.mem.write_u32(r + 0x30, rise);
        fx.mem.write_u32(r + 0x34, kind_y);
        Some(r)
    };
    for k in 0..8u32 {
        let mut list = fx.mem.read_u32(g + 8 + 4 * k);
        if list == 0 {
            continue;
        }
        let mut ok = true;
        while fx.mem.i16(list) > 0 {
            if fx.mem.i16(list) != n {
                list += 2;
                continue;
            }
            let h = fx.self_handle();
            let slot = i32::from(fx.mem.read_u8(g + 0x28 + k));
            if !fx.node_position(h, slot, sp + 0x80) {
                ok = false;
                break;
            }
            let y = fx.mem.read_u32(sp + 0x84);
            let Some(r) = start(fx, y, 0xBC75_C28F, 4, 0x38) else {
                fx.leave(0xC0);
                return 0;
            };
            fx.mem.write_u32(sp + 0x90, r);
            fx.mem.write_u8(sp + 0x94, 0x20);
            fx.mem.write_u8(sp + 0x95, 3);
            fx.mem.write_u32(sp + 0x98, 0x3E80_0000);
            fx.mem.write_u32(sp + 0x9C, 0x3E99_999A);
            fx.mem.write_u32(sp + 0xA0, 0x3E19_999A);
            for c in 0..4u32 {
                let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
                fx.mem.write_u32(sp + 0xA4 + 4 * c, w);
            }
            let w = fx.mem.read_u32(at + 0x180);
            let fan = (at & 0xF000_0000) | ((w & 0x03FF_FFFF) << 2);
            fan_ring(fx, fan, sp + 0x90);
            list += 2;
        }
        let _ = ok;
    }
    let lo = fx.mem.i16(g + 0x34);
    if lo == 0 || n < lo || fx.mem.i16(g + 0x36) < n {
        fx.leave(0xC0);
        return 1;
    }
    let h = fx.self_handle();
    let slot = i32::from(fx.mem.read_u8(g + 0x33));
    if !fx.node_position(h, slot, sp + 0x80) {
        fx.leave(0xC0);
        return 0;
    }
    let Some(r) = start(fx, 0, 0xBBE5_6042, 2, 0x3C) else {
        fx.leave(0xC0);
        return 0;
    };
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(r + 0x1D)) {
        let (pool, cap) = (fx.mem.read_u32(g + 4), i32::from(fx.mem.read_u8(g + 0x31)));
        let i = super::exe::first_free(fx, pool, 0x28, cap);
        if i < 0 {
            fx.mem.set16(r + 0xC + 2 * k, -1);
            k += 1;
            continue;
        }
        fx.mem.set16(r + 0xC + 2 * k, i);
        let s1 = fx.mem.read_u32(g + 4).wrapping_add(0x28 * i as u32);
        super::exe::set_counter(fx, s1, 0x20, 0);
        let v = fx.rand_from(g, at + 0x310) >> 4;
        fx.mem.set8(s1 + 3, (v & 3) << 4);
        for (o, from, ra) in [(4u32, 0x80u32, 0x324u32), (0xC, 0x88, 0x364), (8, 0x84, 0x3A4)] {
            let v = fx.rand_from(g, at + ra) >> 4;
            let (_, m) = fx.div(v, 0x1F4);
            let f = ee::add(fx.mem.f(sp + from), ee::div(ee::from_i32(m - 0xFA), 100.0));
            fx.mem.setf(s1 + o, f);
        }
        for o in [0x24u32, 0x20, 0x1C, 0x18, 0x14, 0x10] {
            fx.mem.write_u32(s1 + o, 0);
        }
        k += 1;
    }
    fx.leave(0xC0);
    1
}
