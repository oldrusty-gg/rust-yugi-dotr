//! Moving emitters of bead beams of the effect library.
use super::arc::resample;
use super::beam::length3;
use super::exe::{aim_angles, countdown, first_free, node_source, set_counter4};
use super::trail::normalize3;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx};

/// 0 or a denormal, as the EE's c.eq.s with 0 sees it.
fn is_zero(v: f32) -> bool {
    v.to_bits() & 0x7F80_0000 == 0
}

/// e94d53479fde [15 B records; b157 0x1E03980, 340 words] [F]:
/// `bead_trails(g, _, n) -> 1` steps `n` emitter records (0x8C bytes from
/// `g`; a 0x2E0 frame) that leave bead-beam records (`beam::beads`, 0x58
/// bytes) along their path. Per record: countdown (0x101AF0), nothing on
/// its first count; position +0x2C += velocity +0x5C, from a node with +5
/// (0x101BA0) and then offset by +0x6C; flag 1: one segment from the last
/// position +0x3C, else four along the curve through +0x2C, +0x3C, +0x4C
/// (0x102550 on the path at sp + 0xA0, samples from sp + 0x104). Per
/// segment a free record of the pool *(+8) (+0x10 of them, 0x101C30),
/// counted +0x11 (0x101AD0): its direction (shorter than 0.3: + the drift
/// +0x7C; longer than 1: normalized, 0x20C7E0) gives length +0x14
/// (0x20C850, x +0x18 unless 0) and angles +0x48 (0x101940); +0x18 =
/// +0x1C, flags 4 (with +0xC = +0x16) / 8 from flags 2 / 4, +8 = +0x12,
/// sizes +0x1C / +0x20 = +0x20 / +0x24, +9 = rand >> 4, +0xA / +0xB / +0xD
/// = +0x13 / +0x14 / +0x15, +0x10 = +0xC, colour +0x24 = +0x28, origin
/// +0x28 = the segment's start. Then the history +0x4C = +0x3C, +0x3C =
/// +0x2C.
pub fn bead_trails(fx: &mut Fx, at: u32, g: u32, n: i32) -> i32 {
    bead_trails_at(fx, at + 0x3EC, g, n)
}

/// d475843933db [2 B records; b137 (card 166) 0x1E04F10, 324 words]
/// [F]: [`bead_trails`] rescheduled (the same 0x2E0 frame,
/// locals and stores); rand's return address at +0x3C0.
pub fn bead_trails_d(fx: &mut Fx, at: u32, g: u32, n: i32) -> i32 {
    bead_trails_at(fx, at + 0x3C0, g, n)
}

/// 8fbdb570917b [3 B records; card 107's 0x1E04980, 440 words] (helper
/// A18) [F]: [`bead_trails`] compiled without common-subexpression
/// elimination in a 0x2B0 frame (every local 0x30 lower, the same
/// caller-relative addresses: the path at sp + 0x70, the samples from +
/// 0xD4, the direction at + 0x2A0), n in s5; rand's return address at
/// +0x4DC.
pub fn bead_trails_e(fx: &mut Fx, at: u32, g: u32, n: i32) -> i32 {
    bead_trails_in(fx, 0x2B0, at + 0x4DC, g, n)
}

/// [`bead_trails`]' body; `rand_ra` the return address of its rand.
fn bead_trails_at(fx: &mut Fx, rand_ra: u32, g: u32, n: i32) -> i32 {
    bead_trails_in(fx, 0x2E0, rand_ra, g, n)
}

/// A compilation with a `frame`-byte frame (`sp` below is the 0x2E0-frame
/// base: the locals' caller-relative addresses).
fn bead_trails_in(fx: &mut Fx, frame: u32, rand_ra: u32, g: u32, n: i32) -> i32 {
    let sp = fx.enter(frame) + frame - 0x2E0;
    let mut g = g;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x8C;
        }
        one(fx, rand_ra, sp, g);
        i += 1;
    }
    fx.leave(frame);
    1
}

fn add3(fx: &mut Fx, a: u32, b: u32) {
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(a + 4 * k), fx.mem.f(b + 4 * k));
        fx.mem.setf(a + 4 * k, v);
    }
}

fn copy4(fx: &mut Fx, from: u32, to: u32) {
    for k in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn one(fx: &mut Fx, rand_ra: u32, sp: u32, g: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    if fx.mem.u8(g) == fx.mem.u8(g + 2) - 1 {
        return;
    }
    add3(fx, g + 0x2C, g + 0x5C);
    node_source(fx, g, g + 0x2C);
    if fx.mem.read_u8(g + 5) != 0 {
        add3(fx, g + 0x2C, g + 0x6C);
    }
    let segs = if fx.mem.u8(g + 4) & 1 != 0 {
        copy4(fx, g + 0x2C, sp + 0x104);
        copy4(fx, g + 0x3C, sp + 0x114);
        1
    } else {
        let p = sp + 0xA0;
        fx.mem.set32(p, 3);
        for (k, src) in [0x2Cu32, 0x3C, 0x4C].into_iter().enumerate() {
            for c in 0..3u32 {
                let w = fx.mem.read_u32(g + src + 4 * c);
                fx.mem.write_u32(p + 4 + 4 * k as u32 + 0x20 * c, w);
            }
        }
        resample(fx, p);
        4
    };
    let d = sp + 0x2D0;
    for k in 0..segs {
        let pool = fx.mem.read_u32(g + 8);
        let count = fx.mem.u8(g + 0x10);
        let i = first_free(fx, pool, 0x58, count);
        if i < 0 {
            continue;
        }
        let off = 0x58 * i as u32;
        let rec = |fx: &mut Fx| fx.mem.read_u32(g + 8).wrapping_add(off);
        let b = rec(fx);
        let c = fx.mem.u8(g + 0x11);
        set_counter4(fx, b, c, 0, 0xFF);
        let p0 = sp + 0x104 + 0x10 * k;
        for c in 0..3u32 {
            let v = ee::sub(fx.mem.f(p0 + 0x10 + 4 * c), fx.mem.f(p0 + 4 * c));
            fx.mem.setf(d + 4 * c, v);
        }
        let len = length3(fx, d);
        if ee_lt(len, f32::from_bits(0x3E99_999A)) {
            add3(fx, d, g + 0x7C);
        } else if !ee_le(len, 1.0) {
            normalize3(fx, d);
        }
        let len = length3(fx, d);
        let b = rec(fx);
        fx.mem.setf(b + 0x14, len);
        let s = fx.mem.f(g + 0x18);
        if !is_zero(s) {
            let b = rec(fx);
            let v = ee::mul(fx.mem.f(b + 0x14), s);
            fx.mem.setf(b + 0x14, v);
        }
        let w = fx.mem.read_u32(g + 0x1C);
        let b = rec(fx);
        fx.mem.write_u32(b + 0x18, w);
        fx.mem.write_u8(b + 4, 0);
        let flags = fx.mem.u8(g + 4);
        if flags & 2 != 0 {
            let b = rec(fx);
            let f = fx.mem.read_u8(b + 4) | 4;
            fx.mem.write_u8(b + 4, f);
            let v = fx.mem.read_u8(g + 0x16);
            fx.mem.write_u8(b + 0xC, v);
        }
        if fx.mem.u8(g + 4) & 4 != 0 {
            let b = rec(fx);
            let f = fx.mem.read_u8(b + 4) | 8;
            fx.mem.write_u8(b + 4, f);
        }
        let b = rec(fx);
        fx.mem.write_u8(b + 5, 0);
        let v = fx.mem.read_u8(g + 0x12);
        fx.mem.write_u8(b + 8, v);
        for (dst, src) in [(0x1Cu32, 0x20u32), (0x20, 0x24)] {
            let w = fx.mem.read_u32(g + src);
            let b = rec(fx);
            fx.mem.write_u32(b + dst, w);
        }
        let r = fx.rand_from(g, rand_ra);
        let b = rec(fx);
        fx.mem.set8(b + 9, r >> 4);
        for (dst, src) in [(0xAu32, 0x13u32), (0xB, 0x14), (0xD, 0x15)] {
            let v = fx.mem.read_u8(g + src);
            let b = rec(fx);
            fx.mem.write_u8(b + dst, v);
        }
        let w = fx.mem.read_u32(g + 0xC);
        let b = rec(fx);
        fx.mem.write_u32(b + 0x10, w);
        let b = rec(fx);
        for k in 0..4u32 {
            let v = fx.mem.read_u8(g + 0x28 + k);
            fx.mem.write_u8(b + 0x24 + k, v);
        }
        let b = rec(fx);
        copy4(fx, p0, b + 0x28);
        let b = rec(fx);
        aim_angles(fx, d, b + 0x48);
    }
    copy4(fx, g + 0x3C, g + 0x4C);
    copy4(fx, g + 0x2C, g + 0x3C);
}

/// b8b5b4f166a1 [4 B records; b368 0x1E050C0, 260 words] [F]:
/// `bead_trails_b(at, g, _, n) -> 1`: [`bead_trails`] for 0x7C-byte
/// emitters (a 0x2B0 frame, the path at sp + 0x70, samples from sp +
/// 0xD4) without the direction: each bead's +0x14 / +0x18 are the
/// emitter's +0x18 / +0x1C, +4 = 5 (| 8 with flag 2, +0xC = +0x16), and
/// its angles +0x48 aim along the stale sp + 0x2A0 (0x101940; never
/// written here: what the call before left at the entry's sp - 0x10).
pub fn bead_trails_b(fx: &mut Fx, at: u32, g: u32, n: i32) -> i32 {
    bead_trails_by(fx, at + 0x2B4, g, n)
}

/// 88444d92ecd4 [2 B records; b181 (card 138) 0x1E02DD0, 348 words]
///: [`bead_trails_b`] compiled without common-subexpression
/// elimination (each bead's address recomputed), the same stores in the
/// same order; rand's return address at +0x368 [F word-compare].
pub fn bead_trails_c(fx: &mut Fx, at: u32, g: u32, n: i32) -> i32 {
    bead_trails_by(fx, at + 0x368, g, n)
}

/// [`bead_trails_b`]'s body; `rand_ra` the return address of its rand.
fn bead_trails_by(fx: &mut Fx, rand_ra: u32, g: u32, n: i32) -> i32 {
    let sp = fx.enter(0x2B0);
    let mut g = g;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x7C;
        }
        one_b(fx, rand_ra, sp, g);
        i += 1;
    }
    fx.leave(0x2B0);
    1
}

fn one_b(fx: &mut Fx, rand_ra: u32, sp: u32, g: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    if fx.mem.u8(g) == fx.mem.u8(g + 2) - 1 {
        return;
    }
    add3(fx, g + 0x2C, g + 0x5C);
    node_source(fx, g, g + 0x2C);
    if fx.mem.read_u8(g + 5) != 0 {
        add3(fx, g + 0x2C, g + 0x6C);
    }
    let segs = if fx.mem.u8(g + 4) & 1 != 0 {
        copy4(fx, g + 0x2C, sp + 0xD4);
        copy4(fx, g + 0x3C, sp + 0xE4);
        1
    } else {
        let p = sp + 0x70;
        fx.mem.set32(p, 3);
        for (k, src) in [0x2Cu32, 0x3C, 0x4C].into_iter().enumerate() {
            for c in 0..3u32 {
                let w = fx.mem.read_u32(g + src + 4 * c);
                fx.mem.write_u32(p + 4 + 4 * k as u32 + 0x20 * c, w);
            }
        }
        resample(fx, p);
        4
    };
    for k in 0..segs {
        let pool = fx.mem.read_u32(g + 8);
        let count = fx.mem.u8(g + 0x10);
        let i = first_free(fx, pool, 0x58, count);
        if i < 0 {
            continue;
        }
        let off = 0x58 * i as u32;
        let rec = |fx: &mut Fx| fx.mem.read_u32(g + 8).wrapping_add(off);
        let b = rec(fx);
        let c = fx.mem.u8(g + 0x11);
        set_counter4(fx, b, c, 0, 0xFF);
        for (dst, src) in [(0x14u32, 0x18u32), (0x18, 0x1C)] {
            let w = fx.mem.read_u32(g + src);
            let b = rec(fx);
            fx.mem.write_u32(b + dst, w);
        }
        let b = rec(fx);
        fx.mem.write_u8(b + 4, 5);
        if fx.mem.u8(g + 4) & 2 != 0 {
            let b = rec(fx);
            let f = fx.mem.read_u8(b + 4) | 8;
            fx.mem.write_u8(b + 4, f);
        }
        let v = fx.mem.read_u8(g + 0x16);
        let b = rec(fx);
        fx.mem.write_u8(b + 0xC, v);
        fx.mem.write_u8(b + 5, 0);
        let v = fx.mem.read_u8(g + 0x12);
        fx.mem.write_u8(b + 8, v);
        for (dst, src) in [(0x1Cu32, 0x20u32), (0x20, 0x24)] {
            let w = fx.mem.read_u32(g + src);
            let b = rec(fx);
            fx.mem.write_u32(b + dst, w);
        }
        let r = fx.rand_from(g, rand_ra);
        let b = rec(fx);
        fx.mem.set8(b + 9, r >> 4);
        for (dst, src) in [(0xAu32, 0x13u32), (0xB, 0x14), (0xD, 0x15)] {
            let v = fx.mem.read_u8(g + src);
            let b = rec(fx);
            fx.mem.write_u8(b + dst, v);
        }
        let w = fx.mem.read_u32(g + 0xC);
        let b = rec(fx);
        fx.mem.write_u32(b + 0x10, w);
        let b = rec(fx);
        for k in 0..4u32 {
            let v = fx.mem.read_u8(g + 0x28 + k);
            fx.mem.write_u8(b + 0x24 + k, v);
        }
        let b = rec(fx);
        copy4(fx, sp + 0xD4 + 0x10 * k, b + 0x28);
        let b = rec(fx);
        aim_angles(fx, sp + 0x2A0, b + 0x48);
    }
    copy4(fx, g + 0x3C, g + 0x4C);
    copy4(fx, g + 0x2C, g + 0x3C);
}

/// ((rand >> 4) % r - (r >> 1)) / 100 (rand called with s0 = `g` from
/// `ra`).
fn jitter(fx: &mut Fx, g: u32, ra: u32, r: i32) -> f32 {
    let x = fx.rand_from(g, ra) >> 4;
    let (_, h) = fx.div(x, r);
    ee::div(ee::from_i32(h - (r >> 1)), 100.0)
}

/// 4ae4ad38e3a6 [15 B records; b124 0x1E026A0, 224 words] [F]:
/// `segment_spawns(g, _, n) -> 1` steps `n` spawner records (0x3C bytes
/// from `g`; a 0x80 frame) that start 0x40-byte segment records in the
/// pool *(+8) (+0xC of them, 0x101C30). Per record: countdown (0x101AF0);
/// on counts where count & mask +3 is set only the position +0x24 moves by
/// +0x30; else a free record counted +0x11 (0x101AD0) takes flags +4 / +5,
/// +9 = +6, +8 / +0xB / +0xC / +0xD = +0xD / +0xE / +0xF / +0x10, sizes
/// +0x10 / +0x14 = +0x18 / +0x1C, colour +0x18 = +0x20, velocity +0x34 =
/// +0x30; with +5 the position from a node (0x101BA0 via sp + 0x70); a
/// jitter j = ((rand >> 4) % r - r / 2) / 100 per axis (r = i32 +0x14);
/// flag 2: the record's +0x1C = j, else its ends +0x1C = position + j and
/// +0x28 = position - j; without +5 the position then moves by +0x30.
pub fn segment_spawns(fx: &mut Fx, at: u32, g: u32, n: i32) -> i32 {
    segment_spawns_by(fx, [at + 0x1AC, at + 0x1F8, at + 0x244], g, n)
}

/// 8d3dbc181755 [8 B records; b259 0x1E02380, 208 words] [F]:
/// [`segment_spawns`] scheduled differently (the same 0x80 frame); its
/// three rand calls return to +0x198 / +0x1D8 / +0x218.
pub fn segment_spawns_b(fx: &mut Fx, at: u32, g: u32, n: i32) -> i32 {
    segment_spawns_by(fx, [at + 0x198, at + 0x1D8, at + 0x218], g, n)
}

fn segment_spawns_by(fx: &mut Fx, ras: [u32; 3], g: u32, n: i32) -> i32 {
    let sp = fx.enter(0x80);
    let mut g = g;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x3C;
        }
        spawn_one(fx, ras, sp, g);
        i += 1;
    }
    fx.leave(0x80);
    1
}

fn spawn_one(fx: &mut Fx, ras: [u32; 3], sp: u32, g: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    if fx.mem.u8(g) & fx.mem.u8(g + 3) != 0 {
        add3(fx, g + 0x24, g + 0x30);
        return;
    }
    let pool = fx.mem.read_u32(g + 8);
    let count = fx.mem.u8(g + 0xC);
    let i = first_free(fx, pool, 0x40, count);
    if i < 0 {
        return;
    }
    let b = fx.mem.read_u32(g + 8).wrapping_add((i as u32) << 6);
    let c = fx.mem.u8(g + 0x11);
    set_counter4(fx, b, c, 0, 0xFF);
    for (dst, src) in [(5u32, 5u32), (4, 4), (9, 6), (8, 0xD), (0xB, 0xE), (0xC, 0xF), (0xD, 0x10)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(b + dst, v);
    }
    for (dst, src) in [(0x10u32, 0x18u32), (0x14, 0x1C)] {
        let w = fx.mem.read_u32(g + src);
        fx.mem.write_u32(b + dst, w);
    }
    for k in 0..4u32 {
        let v = fx.mem.read_u8(g + 0x20 + k);
        fx.mem.write_u8(b + 0x18 + k, v);
    }
    for k in 0..3u32 {
        let w = fx.mem.read_u32(g + 0x30 + 4 * k);
        fx.mem.write_u32(b + 0x34 + 4 * k, w);
    }
    if fx.mem.read_u8(g + 5) != 0 {
        node_source(fx, g, sp + 0x70);
        for k in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x70 + 4 * k);
            fx.mem.write_u32(g + 0x24 + 4 * k, w);
        }
    }
    let r = fx.mem.i32(g + 0x14);
    let jx = jitter(fx, g, ras[0], r);
    let r = fx.mem.i32(g + 0x14);
    let jy = jitter(fx, g, ras[1], r);
    let r = fx.mem.i32(g + 0x14);
    let jz = jitter(fx, g, ras[2], r);
    let j = [jx, jy, jz];
    if fx.mem.u8(g + 4) & 2 != 0 {
        for k in 0..3u32 {
            fx.mem.setf(b + 0x1C + 4 * k, j[k as usize]);
        }
    } else {
        for k in 0..3u32 {
            let v = ee::add(fx.mem.f(g + 0x24 + 4 * k), j[k as usize]);
            fx.mem.setf(b + 0x1C + 4 * k, v);
        }
        for k in 0..3u32 {
            let v = ee::sub(fx.mem.f(g + 0x24 + 4 * k), j[k as usize]);
            fx.mem.setf(b + 0x28 + 4 * k, v);
        }
    }
    if fx.mem.read_u8(g + 5) == 0 {
        add3(fx, g + 0x24, g + 0x30);
    }
}
