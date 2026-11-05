//! BMODULE b258 (card 71), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03A00) [F]. The entry
//! takes (D, param). A task pool of 150 tasks of 0x40 bytes at D+0x2F0
//! (alive +0x36, step +0x24 as a record address), spawned by 0x1E04E70,
//! drawn with the second library: 18-strip bands turned by the task's
//! angles (05dcbcde46f4 at 0x1E00C90, `quads4::band18_turned`), rings of
//! quads with a mode (383a1b86ead2 at 0x1E01510, `quads4::ring_at_m`),
//! 36-strip bands (ad71810de1d8 at 0x1E010A0, `quads4::ring_band_t`), the
//! record's own turned sprite (0x1E009A0) and Lagrange-smoothed tubes along
//! four 50-point trails (0x1E04FE0; the trails kept by c769f66b5e38 at
//! 0x1E00080, `spawn::history`). Four orbs gather at the attacker's nodes
//! 0xF / 0x16 / 0x1E / 0x26, drift to the target from update 178 and are
//! thrown out from 290; the stage dims (0x1E03830) and the units' flag
//! +0x20D bit 0 is set at the start (0x1F8470(1)) and cleared every
//! update (0x1F8470(0)). Returns 1 on update 285, 4 on 405, 2 on 395, 6 on
//! 296..389, 8 once the 410-update timer D+0x8970 has run out.
//!
//! The steps 0x1E027A0 (af061e92e92f, `spawn::disc_step`), 0x1E02880,
//! 0x1E036B0 (a camera slot through 0x1F7E20) and 0x1E03900 (an object op
//! through 0x1F8380) are never spawned [F: the spawn `jal`s name the steps;
//! only 0x1E04E70 writes +0x24]: they are not ported.
use crate::blib::{quads4, spawn, spline, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0x96;
const POOL: u32 = D + 0x2F0;
const POS: u32 = D + 0x250;
/// The four orbs (16 bytes each).
const ORBS: u32 = D + 0x28A0;
/// The orbs' throw targets (16 bytes each).
const GOALS: u32 = D + 0x2920;
/// The trails' newest points (16 bytes each).
const HEADS: u32 = D + 0x29A0;
/// The trails (0x320 bytes each, 50 points).
const TRAILS: u32 = D + 0x2B80;
const TIMER: u32 = D + 0x8970;
const COLOUR: u32 = D + 0x8974;
const OTW: u32 = D + 0x897C;
const NODES: u32 = D + 0x8998;
const THROWN: u32 = D + 0x89B8;
const INDEX: u32 = D + 0x89C4;
const FACING: u32 = D + 0x89C6;
const COUNTER: u32 = D + 0x89CA;
const BYTES: u32 = D + 0x89CC;
const TAB: u32 = D + 0x8DCC;

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn orb(i: i32) -> u32 {
    (ORBS as i32 + 0x10 * i) as u32
}

fn trail(j: i32) -> u32 {
    (TRAILS as i32 + 0x320 * j) as u32
}

/// 0x1F8470(flag) (a runner boundary run as original code [F]): bit 0 of
/// +0x20D of every unit object of kinds 4, 5, 2, 3 (0x1F8560 -> 0x1F3B90
/// / 0x1F4500) and bit 0 of byte 1 of the 8 entries of 0x2DF5D0 in use
/// (0x1FB980(-1, -1, flag)) = `flag`.
pub(crate) fn unit_flags(fx: &mut Fx, flag: i32, a: [u32; 3]) {
    fx.boundary(0x001F_8470, "camera track 0x1F8470", [flag as u32, a[0], a[1], a[2]]);
    for kind in [4i32, 5, 2, 3] {
        for i in 0..0x54u32 {
            let o = fx.mem.read_u32(crate::world::OBJECT_TABLE + 4 * i);
            if o == 0 || i32::from(fx.mem.read_u8(o + 0x1F3) as i8) != kind {
                continue;
            }
            let f = fx.mem.read_u8(o + 0x20D);
            fx.mem.write_u8(o + 0x20D, (f & !1) | (flag as u8 & 1));
        }
    }
    for k in 0..8u32 {
        let e = 0x002D_F5D0 + 0x30 * k;
        if fx.mem.read_u8(e) & 1 != 0 {
            let f = fx.mem.read_u8(e + 1);
            fx.mem.write_u8(e + 1, (f & !1) | u8::from(flag != 0));
        }
    }
}

/// The entry 0x1E03A00 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x120);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x120);
    ret
}

/// The attacker's yaw (rotation y / 180, 0x1A2B80 from `ra`) through the
/// local `out`.
fn yaw(fx: &mut Fx, out: u32, ra: u32) -> i32 {
    let h = fx.self_handle();
    fx.unit_rotation(h, out);
    let y = ee::div(ee::from_i32(fx.mem.i32(out + 4)), 180.0);
    fx.fix_residue(y, ra);
    ee::fixsfsi(y)
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x8F40);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x89C8, param);
    fx.mem.set32(TIMER, 0x19A);
    for (k, (n, clear)) in [(0xF, 1), (0x16, 0), (0x1E, 0), (0x26, 0), (0x1F, 0), (0x34, 0), (0x36, 0), (0x37, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(NODES + 0x10, r);
    // 0x1E03950(0, D): textures.
    for k in 0..3u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x2B0 + 4 * k, t);
    }
    for k in 0xDu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x2E4 + 4 * (k - 0xD), t);
    }
    for k in 0..5u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x8950 + 4 * k, t);
    }
    let y = yaw(fx, sp + 0xD0, 0x01E0_3BE4);
    fx.mem.set16(FACING, y);
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_3BF8, 0x1E);
        fx.mem.set8(BYTES + k, r - 0xF);
    }
    // Three more byte tables: 0..99 cleared, 100..179 rand % m, 180..255
    // signed by the yaw.
    let tables: [(u32, u32, i32); 3] = [(0x8ACC, 0x01E0_3C50, 5), (0x8BCC, 0x01E0_3D64, 6), (0x8CCC, 0x01E0_3E74, 3)];
    for (n, &(at, ra, m)) in tables.iter().enumerate() {
        for k in 0..0x64u32 {
            fx.mem.write_u8(D + at + k, 0);
        }
        for k in 0x64..0xB4u32 {
            let r = rmod(fx, D, ra, m);
            fx.mem.set8(D + at + k, r);
        }
        match n {
            0 => {
                let s = yaw(fx, sp + 0xE0, 0x01E0_3CB4);
                for k in 0xB4..0x100u32 {
                    let v = if s != 0 { fx.rand_from(D, 0x01E0_3CCC) % 4 } else { -(fx.rand_from(D, 0x01E0_3CFC) % 4) };
                    fx.mem.set8(D + at + k, v);
                }
            }
            1 => {
                let s = yaw(fx, sp + 0xF0, 0x01E0_3DC8);
                for k in 0xB4..0x100u32 {
                    let v = if s != 0 { rmod(fx, D, 0x01E0_3DE0, 0x14) + 5 } else { -(rmod(fx, D, 0x01E0_3E10, 0x14) + 5) };
                    fx.mem.set8(D + at + k, v);
                }
            }
            _ => {
                for k in 0xB4..0x100u32 {
                    let v = fx.rand_from(D, 0x01E0_3EA8) % 2 - 1;
                    let a = D + at + k;
                    fx.mem.set8(a, v);
                    let s = yaw(fx, sp + 0x100, 0x01E0_3F08);
                    let b = i32::from(fx.mem.read_u8(a) as i8);
                    fx.mem.set8(a, if s != 0 { b + 2 } else { b - 2 });
                }
            }
        }
    }
    // a1 = 0x1F7370's `out`; a2 = a3 = the last rand's __muldi3 (0x1A0EA8)
    // low product word: the new state's low word - 1 [F 0x1A0EA8, run].
    let a2 = (fx.rand_state() as u32).wrapping_sub(1);
    unit_flags(fx, 1, [sp + 0x100, a2, a2]);
    0
}


/// The steps (their addresses, also the tasks' +0x24) and frame sizes; the
/// entry's return address after step i's `jal` is 0x1E03F90 + 0x20 i
/// (0x1E027A0, 0x1E02880 and 0x1E036B0 are never spawned).
const STEPS: [(u32, u32); 14] = [
    (0x01E0_2370, 0x30),
    (0x01E0_2500, 0x30),
    (0x01E0_2650, 0x30),
    (0x01E0_27A0, 0x20),
    (0x01E0_2880, 0x30),
    (0x01E0_2A60, 0x20),
    (0x01E0_2C10, 0x20),
    (0x01E0_2D80, 0x20),
    (0x01E0_2F40, 0x40),
    (0x01E0_32B0, 0x20),
    (0x01E0_33B0, 0x20),
    (0x01E0_34A0, 0x30),
    (0x01E0_36B0, 0x30),
    (0x01E0_3830, 0x10),
];

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        for (i, &(a, size)) in STEPS.iter().enumerate() {
            if fx.mem.read_u32(t + 0x24) == a {
                let ra = 0x01E0_3F90 + 0x20 * i as u32;
                let s = fx.enter(size);
                match size {
                    0x10 => fx.saved(s, &[(0, u64::from(ra))]),
                    0x20 => fx.saved(s, &[(0x10, u64::from(ra)), (0, u64::from(D))]),
                    0x40 => fx.saved(s, &[(0x30, u64::from(ra)), (0x20, u64::from(D + 0x40 * k)), (0x10, 0), (0, u64::from(D))]),
                    _ => fx.saved(s, &[(0x20, u64::from(ra)), (0x10, 0), (0, u64::from(D))]),
                }
                run_step(fx, a, t, s);
                fx.leave(size);
            }
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_41A4), (0, u64::from(D))]);
    let mut count = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        draw(fx, sp, t, k, count);
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x11D {
        1
    } else if c == 0x195 {
        4
    } else if c == 0x18B {
        2
    } else if (0x128..0x186).contains(&c) {
        6
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

fn run_step(fx: &mut Fx, a: u32, t: u32, sp: u32) {
    match a {
        0x01E0_2370 => orb_glow(fx, t),
        0x01E0_2500 => orb_core(fx, t, 0x01E0_2548, 0xFF, 4, 0x186),
        0x01E0_2650 => orb_core(fx, t, 0x01E0_2698, 0x80, 0, 0xAA),
        0x01E0_2A60 => sparks(fx, t, false),
        0x01E0_2C10 => sparks(fx, t, true),
        0x01E0_2D80 => flecks(fx, t),
        0x01E0_2F40 => throw(fx, t),
        0x01E0_32B0 => flash(fx, t, 0x32, (0xF0, 0x80, 0x10), 0x01E0_3304, false),
        0x01E0_33B0 => flash(fx, t, 0xDC, (0x80, 0x40, 0x10), 0x01E0_3404, true),
        0x01E0_34A0 => ring(fx, t),
        0x01E0_3830 => dim(fx, t),
        _ => {
            let _ = sp;
        }
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8, end: bool) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else if end {
        fx.mem.set16(t + 0x36, 0);
    }
}

fn odd(n: i32) -> bool {
    n % 2 == 1
}

/// The task's angles +0x10 = the camera's (0x205580 with its frames: the
/// step's s0 = the task, s1, return address `ra`), x + 0x4000.
fn face(fx: &mut Fx, t: u32, s1: u32, ra: u32) {
    crate::b494::camera_angles_saving(fx, t + 0x10, u64::from(t), u64::from(s1), ra);
    let x = fx.mem.i32(t + 0x10).wrapping_add(0x4000);
    fx.mem.set32(t + 0x10, x);
}

/// 0x1E02370 (D, task): an orb's orange glow band (size 50, growing by 5 to
/// 100 from update 176), flickering 0x80 / 0x70 until update 390.
fn orb_glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x32);
        l2::rgb(fx, t + 0x28, 0xC0, 0x70, 0);
        l2::rgb(fx, t + 0x2C, 0x20, 0x10, 0);
        let r = rmod(fx, t, 0x01E0_23CC, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, 0x01E0_23E8);
        fx.mem.set16(t + 0x34, v % 8);
        fx.mem.set16(t + 0x36, 2);
    }
    fx.mem.set16(t + 6, 0);
    let n = fx.mem.i16(t + 0x32);
    if n <= 0 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else if fx.mem.i16(COUNTER) < 0x186 {
        fx.mem.write_u8(t + 0x2B, if odd(n) { 0x80 } else { 0x70 });
    } else {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
    }
    if fx.mem.i16(COUNTER) >= 0xB0 {
        let s = fx.mem.i16(t + 0x38);
        if s < 0x64 {
            fx.mem.set16(t + 0x38, s + 5);
        }
    }
    age(fx, t);
    face(fx, t, D, 0x01E0_24D8);
}

/// 0x1E02500 / 0x1E02650 (D, task): an orb's core sprite (white size 10 /
/// grey size 8, cell +0x30 + `cell`), flickering 0x80 / 0x60 until update
/// `until`, growing by 2 to 30 from update 168.
fn orb_core(fx: &mut Fx, t: u32, ra: u32, grey: i32, cell: i32, until: i32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, if grey == 0xFF { 0xA } else { 8 });
        l2::rgb(fx, t + 0x28, grey, grey, grey);
        let r = rmod(fx, t, ra, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let c = fx.mem.i16(t + 0x30) + cell;
        fx.mem.set16(t + 0x34, c);
        fx.mem.set16(t + 0x36, 2);
    }
    let n = fx.mem.i16(t + 0x32);
    if n <= 0 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else if fx.mem.i16(COUNTER) < until {
        fx.mem.write_u8(t + 0x2B, if odd(n) { 0x80 } else { 0x60 });
    } else {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
    }
    if fx.mem.i16(COUNTER) >= 0xA8 {
        let s = fx.mem.i16(t + 0x38);
        if s < 0x1E {
            fx.mem.set16(t + 0x38, s + 2);
        }
    }
    age(fx, t);
}

/// 0x1E02A60 / 0x1E02C10 (`white`) (D, task): sparks about a thrown orb:
/// the radius +0x30 kept in +0xA, then 10..29 (30..179) growing by 5..14
/// (32..41), row 0..34, life 2..6; in by 0x40, out by 0xB; 0x2A60's grow
/// by 3 on odd updates.
fn sparks(fx: &mut Fx, t: u32, white: bool) {
    let ra = |a: u32, b: u32| if white { b } else { a };
    if fx.mem.i16(t + 0x36) == 1 {
        if white {
            fx.mem.set16(t + 0x38, 5);
            fx.mem.set16(t + 0x34, 0);
            l2::rgb(fx, t + 0x28, 0xFF, 0xFF, 0x40);
        } else {
            fx.mem.set16(t + 0x38, 0xF);
            fx.mem.set16(t + 0x34, 3);
            l2::rgb(fx, t + 0x28, 0x80, 0x80, 8);
        }
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        let (m, add) = if white { (0x96, 0x1E) } else { (0x14, 0xA) };
        let x = rmod(fx, t, ra(0x01E0_2AAC, 0x01E0_2C58), m);
        fx.mem.set16(t + 0x30, x + add);
        let x = rmod(fx, t, ra(0x01E0_2ACC, 0x01E0_2C78), 0x168);
        fx.mem.set16(t + 0x3A, x);
        let x = rmod(fx, t, ra(0x01E0_2AE8, 0x01E0_2C94), 0xA);
        fx.mem.set16(t + 0xE, x + if white { 0x20 } else { 5 });
        let x = rmod(fx, t, ra(0x01E0_2B08, 0x01E0_2CB4), 0x23);
        fx.mem.set16(t + 8, x);
        let x = rmod(fx, t, ra(0x01E0_2B24, 0x01E0_2CD0), 5);
        fx.mem.set16(t + 6, x + 2);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x81 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else {
        fade_out(fx, t, 0x2B, 0xD, 0xB, true);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if !white && odd(fx.mem.i16(t + 0x32)) {
        let s = fx.mem.i16(t + 0x38) + 3;
        fx.mem.set16(t + 0x38, s);
    }
    age(fx, t);
}

/// 0x1E02D80 (D, task): grey flecks about a thrown orb's trail start.
fn flecks(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        let x = rmod(fx, t, 0x01E0_2DA4, 0x1E);
        fx.mem.set16(t + 0x38, x + 0x28);
        let v = fx.rand_from(t, 0x01E0_2DC4);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        let x = rmod(fx, t, 0x01E0_2DFC, 0xC8);
        fx.mem.set16(t + 0x30, x + 0x64);
        let x = rmod(fx, t, 0x01E0_2E1C, 0x168);
        fx.mem.set16(t + 0x3A, x);
        let x = rmod(fx, t, 0x01E0_2E38, 5);
        fx.mem.set16(t + 0xE, x + 5);
        let x = rmod(fx, t, 0x01E0_2E58, 0x23);
        fx.mem.set16(t + 8, x);
        let v = fx.rand_from(t, 0x01E0_2E74);
        fx.mem.set16(t + 6, v % 2 + 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0xC0);
    } else {
        fade_out(fx, t, 0x2B, 0x1F, 0x1D, true);
    }
    if odd(fx.mem.i16(t + 0x32)) {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    age(fx, t);
}

/// 0x1E02F40 (D, task): a thrown orb: trail j = +0x30 (of orb j % 4)
/// filled with the orb's point, the head D+0x29A0 + 0x10 j; +6 (the tube's
/// points) up to 10 for 7 updates, then down; on update 7 the flash, four
/// fleck sets and 15 sparks at the trail's start.
fn throw(fx: &mut Fx, t: u32) {
    let j = fx.mem.i16(t + 0x30);
    let n = (j % 4) as i16 as i32;
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 6, 1);
        fx.mem.write_u8(t + 0x2B, 0);
        let src = orb(n);
        let base = trail(fx.mem.i16(t + 0x30));
        for k in 0..0x31u32 {
            copy3(fx, base + 0x10 * k, src);
        }
        let j = fx.mem.i16(t + 0x30);
        copy4(fx, (HEADS as i32 + 0x10 * j) as u32, src);
        fx.mem.set16(t + 0x38, 0x1E);
        fx.mem.set16(t + 0x36, 2);
    }
    let p = fx.mem.i16(t + 6);
    if fx.mem.i16(t + 0x32) < 7 {
        if p < 0xA {
            fx.mem.set16(t + 6, p + 1);
        }
    } else if p > 0 {
        fx.mem.set16(t + 6, p - 1);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fx.mem.write_u8(t + 0x2B, 0xC0);
    } else {
        fade_out(fx, t, 0x2B, 0xB, 9, true);
        let s = fx.mem.i16(t + 0x38);
        if s > 0 {
            fx.mem.set16(t + 0x38, s - 3);
        }
    }
    let g = if fx.mem.i16(t + 0x32) % 2 != 0 { 0xF0 } else { 0x80 };
    l2::rgb(fx, t + 0x28, g, g, g);
    if fx.mem.i16(t + 0x32) < 7 {
        let j = fx.mem.i16(t + 0x30);
        spawn::history(fx, trail(j) + 0x310, 0x32, (HEADS as i32 + 0x10 * j) as u32);
    }
    if fx.mem.i16(t + 0x32) == 7 {
        let j = fx.mem.i16(t + 0x30);
        fx.mem.set16(INDEX, j);
        spawn_task(fx, 0x01E0_33B0);
        for _ in 0..4 {
            spawn_task(fx, 0x01E0_2D80);
        }
        let j = fx.mem.i16(t + 0x30);
        copy3(fx, POS, trail(j));
        for _ in 0..0xF {
            spawn_task(fx, 0x01E0_2C10);
        }
    }
    age(fx, t);
}

/// 0x1E032B0 / 0x1E033B0 (`burst`) (D, task): a white / orange flash band
/// facing the camera: size `size`, outer colour `outer`, alpha +0x2F 0;
/// 0x32B0 in by 0x60 (under 0xC) for 5 updates then out by 0xF, 0x33B0 at
/// 0xA0 for 3 updates then out by 0x1D.
fn flash(fx: &mut Fx, t: u32, size: i32, outer: (i32, i32, i32), ra: u32, burst: bool) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, size);
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x2C, outer.0, outer.1, outer.2);
        let r = rmod(fx, t, ra, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if burst {
        if fx.mem.i16(t + 0x32) < 3 {
            fx.mem.write_u8(t + 0x2B, 0xA0);
        } else {
            fade_out(fx, t, 0x2B, 0x1F, 0x1D, true);
        }
    } else if fx.mem.i16(t + 0x32) < 5 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0xC {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x60));
        }
    } else {
        fade_out(fx, t, 0x2B, 0x11, 0xF, true);
    }
    if burst {
        face(fx, t, 0, 0x01E0_346C);
    } else {
        face(fx, t, 0, 0x01E0_3380);
    }
    age(fx, t);
}

/// 0x1E034A0 (D, task): an orb's thrown ring band (size 32), white /
/// grey flicker by the update counter's parity.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x20);
        l2::rgb(fx, t + 0x2C, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let r = rmod(fx, t, 0x01E0_34FC, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, 0x01E0_3518);
        fx.mem.set16(t + 0x34, v % 8);
        fx.mem.set16(t + 0x36, 2);
    }
    fx.mem.set16(t + 6, 0);
    let n = fx.mem.i16(t + 0x32);
    if n < 5 {
        let a = fx.mem.read_u8(t + 0x2F);
        fx.mem.write_u8(t + 0x2F, a.wrapping_add(0x20));
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x20));
    } else if fx.mem.i16(COUNTER) < 0x186 {
        fx.mem.write_u8(t + 0x2F, if odd(n) { 0x80 } else { 0x70 });
    } else {
        fade_out(fx, t, 0x2F, 0xB, 9, true);
        fade_out(fx, t, 0x2B, 0xB, 9, false);
    }
    if odd(fx.mem.i16(COUNTER)) {
        l2::rgb(fx, t + 0x2C, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    } else {
        l2::rgb(fx, t + 0x2C, 0xCF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x40);
    }
    age(fx, t);
    face(fx, t, D, 0x01E0_3688);
}

/// 0x1E03830 (D, task): the stage light +0x30 (0x1F7F10): 0x80 down by
/// 10 to 0x30 while the timer is at least 31, then back up to 0x80.
fn dim(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x30, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    let v = fx.mem.i16(t + 0x30);
    let v = if fx.mem.i32(TIMER) >= 0x1F {
        if v < 0x31 {
            0x30
        } else {
            v - 0xA
        }
    } else if v < 0x80 {
        v + 0xA
    } else {
        0x80
    };
    fx.mem.set16(t + 0x30, v);
    let l = fx.mem.read_u8(t + 0x30);
    fx.set_stage_light(i32::from(l));
}

/// 0x1E04E70(D, step, 0): a new task (first free of 150) with the spawn
/// colour D+0x8974 (both colours), at D+0x250, index D+0x89C4.
fn spawn_task(fx: &mut Fx, step: u32) {
    fx.enter(0x60);
    let mut task = None;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            task = Some(t);
            break;
        }
    }
    if let Some(t) = task {
        for at in [0x28u32, 0x2C] {
            let c: [i32; 3] = std::array::from_fn(|k| i32::from(fx.mem.read_u8(COLOUR + k as u32)));
            l2::rgb(fx, t + at, c[0], c[1], c[2]);
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + at + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let i = fx.mem.i16(INDEX);
        fx.mem.set16(t + 0x30, i);
        fx.mem.set32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// a2 / a3 at the emitter's 0x1F8470 `jal` (the entry's, as the runner
/// leaves them) [run].
const STEP_FLAG_ARGS: [u32; 2] = [0, 0];

/// 0x1E01A70(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter,
/// the units' flag and the orbs (frame 0xA0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0xA0);
    fx.saved(sp, &[(0x70, 0x01E0_3F5C), (0x30, param as i64 as u64), (0x20, 0), (0x10, u64::from(D))]);
    l2::rgb(fx, COLOUR, 0x80, 0, 0);
    fx.mem.write_u8(COLOUR + 3, 0);
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    unit_flags(fx, 0, [0, STEP_FLAG_ARGS[0], STEP_FLAG_ARGS[1]]);
    if fx.mem.i16(COUNTER) == 1 {
        spawn_task(fx, 0x01E0_3830);
    }
    let unit = |fx: &mut Fx, target: bool, out: u32, ra: u32, s0: u32| {
        let h = if target { fx.target_handle() } else { fx.self_handle() };
        fx.unit_position(h, out);
        fx.saved(sp - 0x20, &[(0x10, u64::from(ra)), (0, u64::from(s0))]);
    };
    unit(fx, true, sp + 0x80, 0x01E0_1B30, D);
    let s0 = sp + 0x90;
    unit(fx, false, sp + 0x90, 0x01E0_1B48, s0);
    for (k, ra) in [0x01E0_1B5Cu32, 0x01E0_1B70, 0x01E0_1B84, 0x01E0_1B98].into_iter().enumerate() {
        unit(fx, false, D + 0x2960 + 0x10 * k as u32, ra, s0);
    }
    for k in 0..4u32 {
        copy4(fx, D + 0x28E0 + 0x10 * k, sp + 0x80);
    }
    for k in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x90 + 4 * k), fx.mem.f(sp + 0x80 + 4 * k)), 73.0);
        fx.mem.setf(sp + 0x80 + 4 * k, v);
    }
    let c = fx.mem.i16(COUNTER);
    if c < 0xB2 {
        for k in 0..4u32 {
            let h = fx.self_handle();
            let n = fx.mem.i16(NODES + 2 * k);
            fx.node_position(h, n, orb(k as i32));
        }
    } else if c < 0xF0 {
        for k in 0..4u32 {
            let a = orb(k as i32) + 8;
            let v = ee::sub(fx.mem.f(a), fx.mem.f(sp + 0x88));
            fx.mem.setf(a, v);
        }
    } else {
        if c == 0xF0 {
            unit(fx, true, sp + 0x80, 0x01E0_1DA8, s0);
            for k in 0..4u32 {
                copy4(fx, GOALS + 0x10 * k, sp + 0x80);
            }
            for (o, w) in [(0u32, 0x4170_0000u32), (0x10, 0xC170_0000), (0x20, 0x4170_0000), (0x30, 0xC170_0000)] {
                fx.mem.write_u32(GOALS + o, w);
            }
            for (o, w) in [(4u32, 0x42A0_0000u32), (0x14, 0x42A0_0000), (0x24, 0x4248_0000), (0x34, 0x4248_0000)] {
                fx.mem.write_u32(GOALS + o, w);
            }
        }
        for k in 0..4u32 {
            for c in 0..2u32 {
                let (g, p) = (GOALS + 0x10 * k + 4 * c, orb(k as i32) + 4 * c);
                let v = ee::add(fx.mem.f(p), ee::div(ee::sub(fx.mem.f(g), fx.mem.f(p)), 30.0));
                fx.mem.setf(p, v);
            }
        }
    }
    if fx.mem.i16(COUNTER) < 2 {
        for i in [2, 3, 0, 1] {
            fx.mem.set16(INDEX, i);
            for s in [0x01E0_2650, 0x01E0_2500, 0x01E0_2370] {
                spawn_task(fx, s);
            }
        }
    }
    if fx.mem.i16(COUNTER) == 0x118 {
        for i in [2, 3, 0, 1] {
            fx.mem.set16(INDEX, i);
            spawn_task(fx, 0x01E0_34A0);
        }
    }
    if fx.mem.i16(COUNTER) < 0x187 {
        for k in 0..4 {
            copy3(fx, POS, orb(k));
        }
    }
    let c = fx.mem.i16(COUNTER);
    if c >= 0x122 {
        let a = fx.mem.i16(THROWN);
        if a < 0x1E && fx.div(c, 3).1 == 1 {
            fx.mem.set16(INDEX, a);
            spawn_task(fx, 0x01E0_2F40);
            spawn_task(fx, 0x01E0_32B0);
            let j = fx.mem.i16(THROWN) % 4;
            copy3(fx, POS, orb(j));
            for _ in 0..3 {
                spawn_task(fx, 0x01E0_2A60);
            }
            let a = fx.mem.i16(THROWN) + 1;
            fx.mem.set16(THROWN, a);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x123 {
        let h = fx.target_handle();
        let n = fx.mem.i16(NODES + 0x10);
        fx.node_position(h, n, sp + 0x80);
        let mut k = 0i32;
        while k < fx.mem.i16(THROWN) {
            let o = orb(k % 4);
            let head = (HEADS as i32 + 0x10 * k) as u32;
            let s0 = (D as i32 + 0x10 * k) as u32;
            let dx = ee::div(ee::sub(fx.mem.f(sp + 0x80), fx.mem.f(o)), 6.0);
            let r = fx.rand_from(s0, 0x01E0_2270) & 0xF;
            let v = ee::add(fx.mem.f(head), ee::sub(ee::add(dx, ee::from_i32(r)), 8.0));
            fx.mem.setf(head, v);
            let r = fx.rand_from(s0, 0x01E0_22B0) & 0xF;
            let dy = ee::div(ee::sub(fx.mem.f(sp + 0x84), fx.mem.f(o + 4)), 6.0);
            let v = ee::add(fx.mem.f(head + 4), ee::sub(ee::add(dy, ee::from_i32(r)), 8.0));
            fx.mem.setf(head + 4, v);
            let dz = ee::div(ee::sub(fx.mem.f(sp + 0x88), fx.mem.f(o + 8)), 6.0);
            let v = ee::add(fx.mem.f(head + 8), dz);
            fx.mem.setf(head + 8, v);
            k += 1;
        }
    }
    fx.leave(0xA0);
}

fn rect_d(fx: &mut Fx, local: u32, id: i32, sub: i32) {
    l2::rect(fx, local, id, sub);
    copy4(fx, D + 0x94, local);
}

/// The entry's draw of one live task (k its index, `count` the live tasks
/// so far: the entry's s3 / s1).
fn draw(fx: &mut Fx, sp: u32, t: u32, k: u32, count: u32) {
    let s = fx.mem.read_u32(t + 0x24);
    let _ = (k, count);
    let pos = sp + 0x50;
    let at_orb = |fx: &mut Fx| {
        let i = fx.mem.i16(t + 0x30);
        copy3(fx, pos, orb(i));
    };
    let at_trail = |fx: &mut Fx, off: u32| {
        let j = fx.mem.i16(t + off);
        copy3(fx, pos, trail(j));
    };
    let sub = |fx: &mut Fx| fx.mem.i16(t + 0x34);
    match s {
        0x01E0_2370 | 0x01E0_34A0 => {
            at_orb(fx);
            quads4::band18_turned(fx, D, t, 1, pos);
        }
        0x01E0_2500 | 0x01E0_2650 => {
            let (tex, local, mode) = if s == 0x01E0_2500 { (0x8954, 0x60, 0x21) } else { (0x895C, 0x70, 0x11) };
            let w = fx.mem.read_u32(D + tex);
            fx.mem.write_u32(D + 0x88, w);
            let c = sub(fx);
            rect_d(fx, sp + local, 0x14, c);
            at_orb(fx);
            fx.mem.set16(D + 0xBE, 0);
            sprite(fx, t, mode, pos);
        }
        0x01E0_2A60 | 0x01E0_2C10 | 0x01E0_2D80 => {
            let (tex, local, id, n, ra) = match s {
                0x01E0_2A60 => (0x2EC, 0x90, 0x9B, 7, 0x01E0_48F8),
                0x01E0_2C10 => (0x2EC, 0xA0, 0x9B, 7, 0x01E0_49C8),
                _ => (0x8960, 0xB0, 0x14, 2, 0x01E0_4AA0),
            };
            let w = fx.mem.read_u32(D + tex);
            fx.mem.write_u32(D + 0x88, w);
            let c = sub(fx);
            rect_d(fx, sp + local, id, c);
            fx.mem.set16(D + 0xB8, n);
            let v = fx.rand_from(D, ra);
            fx.mem.set16(D + 0xBE, v % 4);
            if s == 0x01E0_2D80 {
                let j = fx.mem.i16(t + 0xA);
                copy3(fx, D + 0x20, trail(j));
            } else {
                for c in 0..3u32 {
                    let v = ee::from_i32(fx.mem.i16(t + 2 * c));
                    fx.mem.setf(D + 0x20 + 4 * c, v);
                }
            }
            quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        0x01E0_2F40 => {
            let w = fx.mem.read_u32(D + 0x8958);
            fx.mem.write_u32(D + 0x88, w);
            let c = sub(fx);
            rect_d(fx, sp + 0xC0, 0x14, c);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            for o in [0x20u32, 0x24, 0x28] {
                fx.mem.set32(D + o, 0);
            }
            fx.mem.set16(D + 0xBE, 1);
            let j = fx.mem.i16(t + 0x30);
            tube(fx, t, trail(j), 0, false);
        }
        0x01E0_32B0 | 0x01E0_33B0 => {
            at_trail(fx, 0x30);
            quads4::ring_band_t(fx, D, t, 0x21, pos);
        }
        _ => {}
    }
}

/// 0x1E009A0(D, task, mode, pos): one textured sprite (9 values, FST) of
/// half size i16 +0x38 at `pos` facing the camera (scale 0.1), turned about
/// z by the task's angles +0x10 (0x1099E0 axis 2), coloured +0x28, UVs D+0x94
/// in orientation D+0xBE (the record's 0x1E00100 = `uv::uv_rect`), texture
/// D+0x88; ALPHA by (mode >> 4) & 15 (0: 0x42, 1: 0x44, else 0x48), depth
/// mode & 15 (0 the mean w, 1 the far Z at key 0, else key -1) on D+0x80.
fn sprite(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1C0);
    copy4(fx, sp + 0x50, pos);
    let d = sp + 0x128;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, (4u64 << 32) | 0x2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let alpha = match (mode >> 4) & 0xF {
        0 => 0x80_0000_0042u64,
        1 => 0x80_0000_0044,
        _ => 0x80_0000_0048,
    };
    fx.desc_alpha(d, alpha);
    crate::c061::rgbaq_at(fx, sp + 0x1B8, t + 0x28, d + 0x10);
    let order = fx.mem.i16(D + 0xBE);
    uv::uv_rect(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x110);
    copy3(fx, sp + 0xA0, sp + 0x50);
    l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x110, sp + 0xA0);
    let s = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x60, s, s);
    l2::rotate4(fx, sp + 0x60, t + 0x10, 2, D + 0xD8);
    if l2::project(fx, sp + 0x60, SPR, sp + 0xD0, d, sp + 0xC0) {
        let ot = fx.mem.read_u32(D + 0x80);
        match mode & 0xF {
            0 => {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0xC0)) as i32;
                fx.submit(d, ot, key);
            }
            1 => {
                l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                fx.submit(d, ot, 0);
            }
            _ => fx.submit(d, ot, -1),
        }
    }
    fx.leave(0x1C0);
}

/// The Lagrange curve (44769f26f5eb's basis 0) through `p` at `t` into
/// `out` (xyz), summed as the original's accumulator chain.
fn curve(fx: &mut Fx, out: u32, p: [u32; 4], t: f32) {
    use ee::{add, mul};
    let w = spline::weights(t, 0).unwrap_or([0.0; 4]);
    for c in 0..3u32 {
        let v: [f32; 4] = std::array::from_fn(|i| fx.mem.f(p[i] + 4 * c));
        let acc = add(mul(v[2], w[2]), add(mul(v[0], w[0]), mul(v[1], w[1])));
        fx.mem.setf(out + 4 * c, add(acc, mul(v[3], w[3])));
    }
}

/// 0x1E04FE0(D, task, trail, mode): a textured tube along the trail
/// (points i16 +0xE .. i16 +6; the first segment's point before is the
/// first point): per segment the curve's points at t = 0 (the previous
/// end) and 1, two strip3 quads (vertical, then horizontal with half the z
/// offset) of half width size / 10 narrowing by size / 80 n a segment,
/// turned by the task's angles +0x10 at D+0x20; colours +0x28 / +0x2C,
/// the later pair's alpha down by alpha / 3n a segment; UVs D+0x94 in
/// orientation D+0xBE, texture D+0x88; depth `mode` (0 the mean w, 1 the
/// far Z at key 0, 2 key -1).
///
/// `halves`: b116's compile (card 274 0x1E05CE0,
/// b80f0c0dadf1; fdiff: five words): the segments from 0 (not +0xE), each
/// in two pieces (the step 0.5, clamped to 1 from 0.9; the start of each
/// piece the previous end), the width and alpha falling a piece [F].
pub(crate) fn tube(fx: &mut Fx, t: u32, a: u32, mode: i32, halves: bool) {
    use ee::{add, div, sub};
    let sp = fx.enter(0x340);
    let ot = OT;
    let mut f25 = ee::from_i32(fx.mem.i16(t + 0x38));
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x288;
    f25 = div(f25, 10.0);
    l2::desc_strip3(fx, d);
    fx.mem.setf(sp + 0xDC, f25);
    for (local, c, to) in [(0x318u32, 0x28u32, 0x10u32), (0x320, 0x2C, 0x28), (0x328, 0x28, 0x40), (0x330, 0x2C, 0x58)] {
        crate::c061::rgbaq_at(fx, sp + local, t + c, d + to);
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    let colour = sp + 0x33C;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(colour + k, v);
    }
    fx.mem.write_u32(sp + 0x110, a);
    let pa = sp + 0x140;
    copy4(fx, sp + 0x270, a);
    copy4(fx, pa, sp + 0x270);
    copy4(fx, pa + 0x10, sp + 0x270);
    copy4(fx, pa + 0x20, a + 0x10);
    copy4(fx, pa + 0x30, a + 0x20);
    copy3(fx, sp + 0x1D0, D + 0x20);
    l2::xform_vp(fx, sp + 0x1F0, 0, t + 0x10, sp + 0x1D0);
    for (to, from) in [(0xE0u32, 0x150u32), (0xE4, 0x160), (0xE8, 0x170), (0xEC, 0x154), (0xF0, 0x144), (0xF4, 0x164), (0xF8, 0x174)] {
        let w = fx.mem.read_u32(sp + from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.mem.set32(sp + 0x100, n - 1);
    let step = ee::mul(8.0, ee::from_i32(n));
    fx.mem.setf(sp + 0xFC, step);
    let fp = 3 * n;
    let (mut seg, pieces, dt) = if halves { (0, 2, 0.5f32) } else { (fx.mem.i16(t + 0xE), 1, 1.5f32) };
    while seg < n - 1 {
        let p = (a as i32 + 0x10 * seg) as u32;
        let pts = if seg == 0 { [pa, pa + 0x10, pa + 0x20, pa + 0x30] } else { [p - 0x10, p, p + 0x10, p + 0x20] };
        let mut f24 = 0.0f32;
        for piece in 0..pieces {
            if seg == 0 && piece == 0 {
                curve(fx, sp + 0x250, pts, f24);
                copy4(fx, sp + 0x120, sp + 0x250);
            } else {
                copy4(fx, sp + 0x120, sp + 0x180);
            }
            f24 = add(f24, dt);
            if !(f24 < f32::from_bits(0x3F66_6666)) {
                f24 = 1.0;
            }
            let l = if seg == 0 { 0x260 } else { 0x240 };
            curve(fx, sp + l, pts, f24);
            copy4(fx, sp + 0x130, sp + l);
            copy4(fx, sp + 0x180, sp + 0x130);
            l2::colour_strip3(fx, d, colour, 1);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let q = fx.div(al, fp).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(colour + 3)) - q;
            fx.mem.write_u8(colour + 3, if v > 0 { v as u8 } else { 0 });
            l2::colour_strip3(fx, d, colour, 2);
            let dw = div(fx.mem.f(sp + 0xDC), fx.mem.f(sp + 0xFC));
            let f26 = f25;
            f25 = sub(f25, dw);
            let pv: [u32; 3] = std::array::from_fn(|c| fx.mem.read_u32(sp + 0x120 + 4 * c as u32));
            let qv: [u32; 3] = std::array::from_fn(|c| fx.mem.read_u32(sp + 0x130 + 4 * c as u32));
            let v = sp + 0x190;
            for s1 in 0..2 {
                for (o, src) in [(0u32, pv), (0x10, pv), (0x20, qv), (0x30, qv)] {
                    for c in 0..3u32 {
                        fx.mem.write_u32(v + o + 4 * c, src[c as usize]);
                    }
                }
                let bump = |fx: &mut Fx, at: u32, by: f32, up: bool| {
                    let x = fx.mem.f(v + at);
                    fx.mem.setf(v + at, if up { add(x, by) } else { sub(x, by) });
                };
                if s1 == 0 {
                    bump(fx, 0x14, f26, true);
                    bump(fx, 0x34, f25, true);
                    bump(fx, 0x4, f26, false);
                    bump(fx, 0x24, f25, false);
                } else {
                    let h2 = div(f26, 2.0);
                    bump(fx, 0x10, f26, true);
                    let h1 = div(f25, 2.0);
                    bump(fx, 0x30, f25, true);
                    bump(fx, 0x18, h2, true);
                    bump(fx, 0x38, h1, true);
                    bump(fx, 0x0, f26, false);
                    bump(fx, 0x20, f25, false);
                    bump(fx, 0x8, h2, false);
                    bump(fx, 0x28, h1, false);
                }
                if mode <= 2 && l2::project(fx, v, SPR, sp + 0x1F0, d, sp + 0x1E0) {
                    match mode {
                        0 => {
                            let key = ee::fixunssfsi(fx.mem.f(sp + 0x1E0)) as i32;
                            fx.submit(d, ot, key);
                        }
                        1 => {
                            l2::far_z_desc(fx, d, 0);
                            fx.submit(d, ot, 0);
                        }
                        _ => fx.submit(d, ot, -1),
                    }
                }
            }
        }
        seg += 1;
    }
    fx.leave(0x340);
}
