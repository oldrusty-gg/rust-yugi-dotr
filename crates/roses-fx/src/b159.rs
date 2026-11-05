//! BMODULE b159 (card 170), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E00CE0) [F]. The entry
//! takes (D, param). b064's family: a pool of 220 tasks of 0x40 bytes at
//! D+0x284 (alive +0x36, step +0x24 as a record address, steps by direct
//! `jal`), spawned by 0x1E01260 (b538's spawn on this record's offsets:
//! colour D+0x3B04, point D+0x80, +0x30 from D+0x3B60), the sines D+0x3B68.
//! Steps: 0x1E004C0 (b064's violet mote 0x1E017E0), 0x1E006E0 (b064's
//! grey puff 0x1E01A00 with +0x30 = rand % 360), 0x1E008E0 (b089's blue
//! puff 0x1E006E0 with +0x38 = 1), 0x1E00B30 (a dark violet ring turning
//! 12 a update, growing by 1 / 3 / 2 for the index +0x30 0 / 1 / other)
//! and a trail task 0x1E00A90 stepped inline in the entry (violet, alpha
//! 0x30, ten segments of the attacker's node 0x1D history D+0x3A60, ended
//! when the timer reads 50). Drawn twice each (mode 0 then 1): the motes
//! by a camera-facing quad (0x1E01520: cell 0x14, texture D+0xD8), the
//! puffs by its twin 0x1E01380 (cell 0x99, texture D+0x10C), the trail by
//! 0x1E016C0 (per segment twelve gouraud quads between two rings of the
//! segment's radius and that + 0.8, at the far Z) and the ring by
//! 0x1E01B30 (nine quads of 40 degrees between +0x38 and +0x38 - 10, cell
//! 0x96, texture D+0x10C). The emitter 0x1E00150: on 0x38..0x3A twenty
//! of each puff at node 0x1D; on 0x30 the trail; on 0x2E..0x41 two rings;
//! on 0x1F / 0x2D / 0x36 / 0x5D ten motes at node 0x38, on 0x19 / 0x25 /
//! 0x38 ten at node 0xD. From update 0x15 D+0xC0 / D+0xC8 creep up by
//! 0.01 below 0.5 (soft double compares), D+0xC4 = 0.1. Returns 1 on
//! update 0x2D, 4 on 0x41, 2 on 0x37, 8 once the 120-update timer D+0x3B00
//! has run out. The step 0x1E008D0 only returns (its draw case is never
//! reached).
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::spawn as lspawn;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0xDC;
const POOL: u32 = D + 0x284;
const POS: u32 = D + 0x80;
const HIST: u32 = D + 0x3A60;
const TIMER: u32 = D + 0x3B00;
const COLOUR: u32 = D + 0x3B04;
const OTW: u32 = D + 0x3B0C;
const MEAN: u32 = D + 0x3B3C;
const NODE: u32 = D + 0x3B50;
const ANGLE: u32 = D + 0x3B5A;
const INDEX: u32 = D + 0x3B60;
const COUNTER: u32 = D + 0x3B64;
const TAB: u32 = D + 0x3B68;

const MOTE: u32 = 0x01E0_04C0;
const PUFF: u32 = 0x01E0_06E0;
const IDLE: u32 = 0x01E0_08D0;
const PUFF_B: u32 = 0x01E0_08E0;
const TRAIL: u32 = 0x01E0_0A90;
const RING: u32 = 0x01E0_0B30;

const D05: u64 = 0x3FE0_0000_0000_0000;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x110);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x110);
    ret
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

/// 0x1A2E08's unpack of the double `x` at `p` (class +0, sign +4, exponent
/// +8, mantissa +0x10) (b089's).
fn unpack_d(fx: &mut Fx, x: u64, p: u32) {
    fx.mem.write_u32(p + 4, (x >> 63) as u32);
    let e = ((x >> 52) & 0x7FF) as u32;
    let m = x & 0xF_FFFF_FFFF_FFFF;
    if e == 0 {
        fx.mem.write_u32(p, 2);
    } else if e == 0x7FF {
        if m == 0 {
            fx.mem.write_u32(p, 4);
        } else {
            fx.mem.write_u32(p, u32::from(m & (1 << 51) != 0));
            fx.mem.write_u64(p + 0x10, m);
        }
    } else {
        fx.mem.write_u64(p + 0x10, (m << 8) | (1 << 60));
        fx.mem.write_u32(p + 8, e.wrapping_sub(0x3FF));
        fx.mem.write_u32(p, 3);
    }
}

/// (double) `v` < `b` (0x17BCA0) through 0x1A2C30 (from `ra1`), the
/// compare called from `ra2` with the caller's s0 `s0`, with the frames
/// they leave below the caller's sp (b089's `dcmp`) [F].
fn dlt(fx: &mut Fx, v: f32, b: u64, s0: u32, ra1: u32, ra2: u32) -> bool {
    let sp = fx.sp;
    fx.fix_residue(v, ra1);
    let f = sp - 0x30;
    let (class, sign, exp, mant) = (fx.mem.read_u32(f), fx.mem.read_u32(f + 4), fx.mem.read_u32(f + 8), fx.mem.read_u32(f + 0xC));
    let g = sp - 0x60;
    fx.mem.write_u32(g, class);
    fx.mem.write_u32(g + 4, sign);
    fx.mem.write_u32(g + 8, exp);
    fx.mem.write_u64(g + 0x10, u64::from(mant) << 30);
    fx.mem.write_u64(g + 0x20, 0x001A_2C60);
    let bits = v.to_bits();
    let a = if (bits >> 23) & 0xFF == 0 { u64::from(bits >> 31) << 63 } else { f64::from(v).to_bits() };
    fx.saved(sp - 0x10, &[(0, u64::from(ra2))]);
    let h = sp - 0x70;
    fx.mem.write_u64(h + 0x40, a);
    fx.mem.write_u64(h + 0x48, b);
    fx.mem.write_u64(h + 0x50, u64::from(s0));
    fx.mem.write_u64(h + 0x58, 0x0017_BCB0);
    unpack_d(fx, a, h);
    unpack_d(fx, b, h + 0x20);
    f64::from_bits(a) < f64::from_bits(b)
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3CE0);
    l2::sine_table(fx, TAB);
    fx.mem.set16(D + 0x3B62, param);
    fx.mem.set32(TIMER, 0x78);
    for o in [0xC0u32, 0xC4, 0xC8] {
        fx.mem.set32(D + o, 0);
    }
    for (k, node) in [0x1D, 0x38, 0xD].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0xD8, t);
    for k in 0xEu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x108 + 4 * (k - 0xE), t);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, or the history's end after its fill.
    let mut s1 = param as i64 as u64;
    if fx.mem.i16(COUNTER) == 0x2D {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        node_position_saving(fx, h, n, sp + 0x60, u64::from(D), Some(u64::from(HIST)), 0x01E0_0E04);
        for k in 0..9u32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
                fx.mem.write_u32(HIST + 0x10 * k + 4 * c, w);
            }
        }
        s1 = u64::from(HIST + 0x90);
    }
    fx.saved(sp - 0x70, &[(0x50, 0x01E0_0E40), (0x40, 0), (0x30, 0), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
    let n = fx.mem.i16(NODE);
    lspawn::node_history(fx, HIST + 0x90, 0xA, n);
    emit(fx, s1);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        let call = |fx: &mut Fx, size: u32, ra: u32, g: fn(&mut Fx, u32)| {
            let s = fx.enter(size);
            if size == 0x20 {
                fx.saved(s, &[(0x10, u64::from(ra)), (0, u64::from(D))]);
            } else {
                fx.saved(s, &[(0x20, u64::from(ra)), (0x10, u64::from(k)), (0, u64::from(D))]);
            }
            g(fx, t);
            fx.leave(size);
        };
        if f == MOTE {
            call(fx, 0x30, 0x01E0_0E80, mote);
        }
        if fx.mem.read_u32(t + 0x24) == PUFF {
            call(fx, 0x30, 0x01E0_0EA0, puff);
        }
        if fx.mem.read_u32(t + 0x24) == PUFF_B {
            call(fx, 0x30, 0x01E0_0EC0, puff_b);
        }
        if fx.mem.read_u32(t + 0x24) == TRAIL {
            l2::rgb(fx, t + 0x28, 0x1F, 0x10, 0x50);
            if fx.mem.i16(t + 0x36) == 1 {
                fx.mem.set16(t + 0x38, 5);
                fx.mem.set16(t + 0x36, 2);
            }
            let h = fx.self_handle();
            let n = fx.mem.i16(NODE);
            node_position_saving(fx, h, n, sp + 0x70, u64::from(D), Some(u64::from(k)), 0x01E0_0F20);
            fx.mem.write_u8(t + 0x2B, 0x30);
            fx.mem.set16(t + 0x32, 0xA);
            if fx.mem.i32(TIMER) == 0x32 {
                fx.mem.set16(t + 0x36, 0);
            }
        }
        if fx.mem.read_u32(t + 0x24) == RING {
            call(fx, 0x20, 0x01E0_0F68, ring);
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_0F8C);
    let z = ee::sub(fx.mem.f(sp + 0x58), 25.0);
    fx.mem.setf(sp + 0x58, z);
    let mut count = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        draw(fx, sp, t, k, count);
    }
    if fx.mem.i16(COUNTER) >= 0x15 {
        let v = fx.mem.f(D + 0xC0);
        if dlt(fx, v, D05, D, 0x01E0_1158, 0x01E0_1168) {
            let v = ee::add(fx.mem.f(D + 0xC0), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC0, v);
        }
        fx.mem.write_u32(D + 0xC4, 0x3DCC_CCCD);
        let v = fx.mem.f(D + 0xC8);
        if dlt(fx, v, D05, D, 0x01E0_11A0, 0x01E0_11B0) {
            let v = ee::add(fx.mem.f(D + 0xC8), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC8, v);
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x2D => 1,
        0x41 => 4,
        0x37 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw of one live task (index `k`, `count` the live tasks so
/// far: the entry's s3 / s2, s1 = D + 0x40 k): each draw twice (mode 0,
/// then 1) into the descriptor sp + 0x80.
fn draw(fx: &mut Fx, sp: u32, t: u32, k: u32, count: u32) {
    let d = sp + 0x80;
    let regs = [u64::from(D), u64::from(D + 0x40 * k), u64::from(count), u64::from(k)];
    // (frame size, the s0 slot, the ra slot, the saved registers past s3).
    let pair = |fx: &mut Fx, g: fn(&mut Fx, u32, u32, i32), lay: (u32, u32, u32, u32), ra: u32| {
        let (size, base, r, more) = lay;
        for (mode, ra) in [(0, ra), (1, ra + 0x14)] {
            let f = fx.sp - size;
            for (j, v) in regs.iter().enumerate() {
                fx.saved(f + base + 0x10 * j as u32, &[(0, *v)]);
            }
            for j in 0..more {
                fx.saved(f + base + 0x40 + 0x10 * j, &[(0, 0)]);
            }
            fx.saved(f, &[(r, u64::from(ra))]);
            g(fx, t, d, mode);
        }
    };
    const SPRITE: (u32, u32, u32, u32) = (0x110, 0, 0x50, 1);
    const TRAIL_F: (u32, u32, u32, u32) = (0x1F0, 0x10, 0xA0, 5);
    const RING_F: (u32, u32, u32, u32) = (0x110, 0, 0x60, 2);
    let s = fx.mem.read_u32(t + 0x24);
    if s == MOTE {
        pair(fx, sprite_mote, SPRITE, 0x01E0_0FDC);
    }
    if fx.mem.read_u32(t + 0x24) == PUFF {
        pair(fx, sprite_puff, SPRITE, 0x01E0_1018);
    }
    if fx.mem.read_u32(t + 0x24) == IDLE {
        pair(fx, sprite_puff, SPRITE, 0x01E0_1058);
    }
    if fx.mem.read_u32(t + 0x24) == PUFF_B {
        pair(fx, sprite_puff, SPRITE, 0x01E0_1098);
    }
    if fx.mem.read_u32(t + 0x24) == TRAIL {
        pair(fx, trail, TRAIL_F, 0x01E0_10D8);
    }
    if fx.mem.read_u32(t + 0x24) == RING {
        pair(fx, ring9, RING_F, 0x01E0_1118);
    }
}

/// 0x1E00150(D, 0) (frame 0x50: ra, the entry's s2 = 0, s1, s0 = D): the
/// spawn colour (0x80, 0, 0, 0), the update counter and the spawns. Its
/// s0 / s1 as the record leaves them are tracked for the frames below it
/// (the node calls', rand's and the spawn's).
fn emit(fx: &mut Fx, s1: u64) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_0E4C), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
    l2::rgb(fx, COLOUR, 0x80, 0, 0);
    fx.mem.write_u8(COLOUR + 3, 0);
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let at = |fx: &mut Fx, slot: u32, s0: u64, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let n = fx.mem.i16(slot);
        node_position_saving(fx, h, n, sp + 0x40, s0, Some(s1), ra);
    };
    let pos = |fx: &mut Fx| {
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    // The spawn's saves: the emitter's s0 / s1, s2 = D, s3 / s4 the
    // runner's 0.
    let go = |fx: &mut Fx, f: u32, s0: u64, s1: u64, ra: u32| {
        spawn(fx, f, [s0, s1, u64::from(D), 0, 0], ra);
    };
    let angle = |fx: &mut Fx, s0: u64, ra: u32| {
        let r = rmod(fx, s0 as u32, ra, 0x168);
        fx.mem.set16(ANGLE, r);
    };
    let (mut s0, mut s1) = (u64::from(D), s1);
    at(fx, NODE, s0, s1, 0x01E0_019C);
    at(fx, NODE, s0, s1, 0x01E0_01B4);
    s0 = 0;
    let c = fx.mem.i16(COUNTER);
    if (0x38..=0x3A).contains(&c) {
        while s0 < 0x14 {
            angle(fx, s0, 0x01E0_01E8);
            pos(fx);
            go(fx, PUFF, s0, s1, 0x01E0_0224);
            go(fx, PUFF_B, s0, s1, 0x01E0_0238);
            s0 += 1;
        }
    }
    at(fx, NODE, s0, s1, 0x01E0_0260);
    if fx.mem.i16(COUNTER) == 0x30 {
        s0 = 0;
        angle(fx, s0, 0x01E0_027C);
        pos(fx);
        go(fx, TRAIL, s0, s1, 0x01E0_02B8);
        s0 = 1;
    }
    let c = fx.mem.i16(COUNTER);
    if (0x2E..0x42).contains(&c) {
        s0 = 2;
        s1 = 0;
        while s1 < s0 {
            angle(fx, s0, 0x01E0_0310);
            pos(fx);
            fx.mem.set16(INDEX, 1);
            go(fx, RING, s0, s1, 0x01E0_0354);
            s1 += 1;
        }
    }
    at(fx, NODE + 2, s0, s1, 0x01E0_0380);
    s0 = 0;
    let c = fx.mem.i16(COUNTER);
    if [0x5D, 0x1F, 0x2D, 0x36].contains(&c) {
        while s0 < 0xA {
            angle(fx, s0, 0x01E0_03C0);
            pos(fx);
            go(fx, MOTE, s0, s1, 0x01E0_03FC);
            s0 += 1;
        }
    }
    at(fx, NODE + 4, s0, s1, 0x01E0_0428);
    s0 = 0;
    let c = fx.mem.i16(COUNTER);
    if [0x19, 0x25, 0x38].contains(&c) {
        while s0 < 0xA {
            angle(fx, s0, 0x01E0_045C);
            pos(fx);
            go(fx, MOTE, s0, s1, 0x01E0_0498);
            s0 += 1;
        }
    }
    fx.leave(0x50);
}

/// 0x1E01260(D, step, 0) (frame 0x60: the caller's s0..s4, `ra`): the
/// first free task of 220 cleared and set off at D+0x80 (as i16) with the
/// colour D+0x3B04 (twice), +0x30 = D+0x3B60.
fn spawn(fx: &mut Fx, f: u32, s: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, s[4]), (0x30, s[3]), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x40 * k + 0x36) == 0) {
        Some(k) => {
            let t = POOL + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn fade_in(fx: &mut Fx, t: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a < cap {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
    }
}

/// Alpha +0x2B down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// i16 `at` += `k` x (sine or cosine of i16 `deg`) through 0x1A2B80.
fn wobble(fx: &mut Fx, t: u32, at: u32, deg: u32, cos: i32, k: f32, ra: u32) {
    let a = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, TAB, a, cos);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(k, s));
    let r = fix(fx, f, ra);
    fx.mem.set16(t + at, r);
}

/// 0x1E004C0 (D, task) (b064's 0x1E017E0): a violet mote drifting about
/// its point.
fn mote(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x3F, 0x30, 0x3F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        let r = rmod(fx, t, 0x01E0_0504, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, 0x01E0_0524, 0x1E);
        fx.mem.set16(t + 0x30, r - 0xF);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fade_in(fx, t, 0x60, 9);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let s = fx.mem.i16(t + 0x38);
        if s < 0x12 {
            fx.mem.set16(t + 0x38, s + 1);
        }
    }
    let n = fx.mem.i16(t + 0x32);
    if fx.div(n, 3).1 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    wobble(fx, t, 8, 0x3A, 0, 10.0, 0x01E0_064C);
    wobble(fx, t, 0xA, 0x30, 1, 4.0, 0x01E0_0684);
    wobble(fx, t, 0xC, 0x3A, 1, 10.0, 0x01E0_06BC);
}

/// 0x1E006E0 (D, task) (b064's 0x1E01A00 with +0x30 = rand % 360): a grey
/// puff (cells 0..7 every fifth update).
fn puff(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x3F, 0x3F, 0x3F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        let r = rmod(fx, t, 0x01E0_0724, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, 0x01E0_0744, 0x168);
        fx.mem.set16(t + 0x30, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 8 {
        fade_in(fx, t, 0xA0, 8);
    } else {
        fade_out(fx, t, 7, 5);
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38);
    if s < 0xD {
        fx.mem.set16(t + 0x38, s + 1);
    }
    let n = fx.mem.i16(t + 0x32);
    if fx.div(n, 5).1 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    wobble(fx, t, 8, 0x3A, 0, 13.0, 0x01E0_0844);
    wobble(fx, t, 0xA, 0x30, 1, 13.0, 0x01E0_087C);
    wobble(fx, t, 0xC, 0x3A, 1, 13.0, 0x01E0_08B4);
}

/// 0x1E008E0 (D, task) (b089's 0x1E006E0 with size 1): a blue puff.
fn puff_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 1);
        let v = fx.rand_from(t, 0x01E0_090C);
        fx.mem.set16(t + 0x34, v % 4);
        fx.rand_from(t, 0x01E0_0928);
        l2::rgb(fx, t + 0x28, 0x1F, 0x1F, 0xFF);
        let r = rmod(fx, t, 0x01E0_0944, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, t, 0x01E0_0960, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
    wobble(fx, t, 8, 0x3A, 0, 9.0, 0x01E0_09FC);
    wobble(fx, t, 0xA, 0x30, 1, 9.0, 0x01E0_0A34);
    wobble(fx, t, 0xC, 0x3A, 1, 9.0, 0x01E0_0A6C);
}

/// 0x1E00B30 (D, task): a dark violet ring (0x30, 0x30, 0x60, alpha 0x80)
/// at random angles, radius +0x38 from 0 growing by 1 (index +0x30 0,
/// out by 16 from 11), 3 (1: to 50, out from 26) or 2 (else: to 30, out
/// from 21), its start angle +0x3A turning by 12 a update.
fn ring(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x30, 0x30, 0x60);
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, ra) in [(0x10u32, 0x01E0_0B6Cu32), (0x14, 0x01E0_0B88), (0x18, 0x01E0_0BA4)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0x80);
    }
    let r = fx.mem.i16(t + 0x38);
    match fx.mem.i16(t + 0x30) {
        0 => {
            if r >= 0xB {
                fade_out(fx, t, 0x11, 0x10);
            }
            let r = fx.mem.i16(t + 0x38);
            fx.mem.set16(t + 0x38, r + 1);
        }
        1 => {
            if r >= 0x1A {
                fade_out(fx, t, 0x11, 0x10);
            }
            let r = fx.mem.i16(t + 0x38);
            if r < 0x32 {
                fx.mem.set16(t + 0x38, r + 3);
            }
        }
        _ => {
            if r >= 0x15 {
                fade_out(fx, t, 0x11, 0x10);
            }
            let r = fx.mem.i16(t + 0x38);
            if r < 0x1E {
                fx.mem.set16(t + 0x38, r + 2);
            }
        }
    }
    let a = fx.mem.i16(t + 0x3A) + 0xC;
    fx.mem.set16(t + 0x3A, a);
}

/// The trans vector at `out`: the task's i16 position + i16 +8 / +0xA /
/// +0xC over 10 (integer division by the magic multiply).
fn trans_tenth(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3u32 {
        let off = fx.mem.i16(t + 8 + 2 * k);
        fx.mult(0x6666_6667, off);
        let hi = fx.hilo().1;
        let q = (hi >> 2).wrapping_add(((off as u32) >> 31) as i32);
        let v = fx.mem.i16(t + 2 * k).wrapping_add(q);
        fx.mem.setf(out + 4 * k, ee::from_i32(v));
    }
}

fn setup(fx: &mut Fx, d: u32, r: u32, c: u32, tex: u32) {
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, c, 0);
    l2::uv_strip3(fx, d, r, 0);
    let w = fx.mem.read_u32(tex);
    fx.desc_texture(d, w, 1);
}

/// 0x1E01380 / 0x1E01520 (D, task, desc, mode) (b064's 0x1E03D70): a
/// camera-facing quad of half size i16 +0x38 at the task's position (+
/// offsets / 10), UVs of sprite `id` cell +0x34, texture `tex`, keyed by
/// the mean w (0x109870).
fn sprite(fx: &mut Fx, t: u32, d: u32, mode: i32, id: i32, tex: u32, ra: u32) {
    let sp = fx.enter(0x110);
    let ot = OT;
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xB0, id, c);
        setup(fx, d, sp + 0xB0, t + 0x28, tex);
    } else {
        camera_angles_saving(fx, sp + 0x100, u64::from(ot), u64::from(d), ra);
        trans_tenth(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, ot);
    }
    fx.leave(0x110);
}

fn sprite_puff(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    sprite(fx, t, d, mode, 0x99, D + 0x10C, 0x01E0_141C);
}

fn sprite_mote(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    sprite(fx, t, d, mode, 0x14, D + 0xD8, 0x01E0_15BC);
}

/// 0x1E016C0(D, task, desc, mode) (frame 0x1F0; both modes draw): for each
/// of the +0x32 - 1 segments of the history D+0x3A60.. a ring of twelve
/// gouraud quads (30 degrees each) between the segment's point at radius
/// r (0 for the first, + 1 a segment) and the next point at r + 0.8 (x by
/// the sine, y and z by the cosine), the task's colour with alpha falling
/// by alpha / +0x32 a segment, at the far Z (key 0) in the camera's
/// view-projection.
fn trail(fx: &mut Fx, t: u32, _d: u32, _mode: i32) {
    use ee::{add, mul};
    let sp = fx.enter(0x1F0);
    fx.mem.write_u32(sp + 0xFC, t);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 0xC8, OT);
    let s1 = sp + 0x158;
    let n = fx.mem.i16(t + 0x32);
    l2::desc_gouraud4(fx, s1, 0);
    l2::fullscreen4(fx, s1);
    fx.mem.set32(sp + 0xE0, n);
    fx.mem.set32(sp + 0xB0, 0);
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(sp + 0x1EC + k, b);
    }
    fx.mem.set32(sp + 0xD0, n - 1);
    let mut f20 = 0.0f32;
    let mut a = 0i32;
    let bump = |fx: &mut Fx, at: u32, r: f32, deg: i32, cos: i32| {
        let c = l2::sin_deg(fx, TAB, deg, cos);
        let v = add(fx.mem.f(at), mul(r, c));
        fx.mem.setf(at, v);
    };
    while a < n - 1 {
        let s3 = HIST + 0x10 * a as u32;
        l2::colour_strip4(fx, s1, sp + 0x1EC, 1);
        let al = i32::from(fx.mem.read_u8(t + 0x2B));
        let q = fx.div(al, n).0;
        let b = fx.mem.read_u8(sp + 0x1EF).wrapping_sub(q as u8);
        fx.mem.write_u8(sp + 0x1EF, b);
        l2::colour_strip4(fx, s1, sp + 0x1EC, 2);
        let mut s2 = 0i32;
        for _ in 0..0xC {
            for o in [0x110u32, 0x120] {
                for c in 0..3 {
                    let w = fx.mem.read_u32(s3 + 4 * c);
                    fx.mem.write_u32(sp + o + 4 * c, w);
                }
            }
            let s4 = (s2 + 0x1E) as i16 as i32;
            let r = f20;
            bump(fx, sp + 0x114, r, s2, 1);
            bump(fx, sp + 0x110, r, s2, 0);
            bump(fx, sp + 0x118, r, s2, 1);
            bump(fx, sp + 0x124, r, s4, 1);
            bump(fx, sp + 0x120, r, s4, 0);
            bump(fx, sp + 0x128, r, s4, 1);
            for o in [0x130u32, 0x140] {
                for c in 0..3 {
                    let w = fx.mem.read_u32(s3 + 0x10 + 4 * c);
                    fx.mem.write_u32(sp + o + 4 * c, w);
                }
            }
            let r2 = || add(f20, f32::from_bits(0x3F4C_CCCD));
            bump(fx, sp + 0x134, r2(), s2, 1);
            bump(fx, sp + 0x130, r2(), s2, 0);
            bump(fx, sp + 0x138, r2(), s2, 1);
            bump(fx, sp + 0x144, r2(), s4, 1);
            bump(fx, sp + 0x140, r2(), s4, 0);
            bump(fx, sp + 0x148, r2(), s4, 1);
            if l2::project(fx, sp + 0x110, SPR, 0, s1, sp + 0x100) {
                l2::far_z_desc(fx, s1, 1);
                let ot = fx.mem.read_u32(sp + 0xC8);
                fx.submit(s1, ot, 0);
            }
            s2 = s4;
        }
        f20 = add(f20, 1.0);
        a += 1;
        fx.mem.set32(sp + 0xB0, a);
    }
    fx.leave(0x1F0);
}

/// 0x1E01B30(D, task, desc, mode): a ring of nine quads (40 degrees each,
/// from +0x3A) between radius i16 +0x38 - 10 and +0x38 in the plane y =
/// 0, turned by the task's angles +0x10 at its i16 position, UVs of sprite
/// 0x96 cell +0x34, texture D+0x10C; each keyed by its mean w (D+0x3B3C)
/// on D+0x3B0C.
fn ring9(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x100, 0x96, c);
        setup(fx, d, sp + 0x100, t + 0x28, D + 0x10C);
    } else {
        for k in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0xB0 + 4 * k, v);
        }
        l2::xform_vp(fx, sp + 0xC0, 0, t + 0x10, sp + 0xB0);
        for o in [0x74u32, 0x84, 0x94, 0xA4] {
            fx.mem.set32(sp + o, 0);
        }
        let mut s1 = 0i32;
        for _ in 0..9 {
            for (deg, outer, inner) in [((fx.mem.i16(t + 0x3A) + s1) as i16 as i32, 0x90u32, 0x70u32), ((fx.mem.i16(t + 0x3A) + 0x28 + s1) as i16 as i32, 0xA0, 0x80)] {
                for (o, less) in [(outer, 0), (inner, 0xA)] {
                    for (cos, off) in [(1, 0u32), (0, 8)] {
                        let c = l2::sin_deg(fx, TAB, deg, cos);
                        let r = ee::from_i32(fx.mem.i16(t + 0x38) - less);
                        fx.mem.setf(sp + o + off, ee::mul(r, c));
                    }
                }
            }
            if l2::project(fx, sp + 0x70, SPR, sp + 0xC0, d, MEAN) {
                let key = ee::fixunssfsi(fx.mem.f(MEAN)) as i32;
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
            s1 = (s1 + 0x28) as i16 as i32;
        }
    }
    fx.leave(0x110);
}
