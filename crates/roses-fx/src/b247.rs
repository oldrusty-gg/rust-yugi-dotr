//! BMODULE b247 (card 247), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02420, data area 0x1ED0000) [F]: b494's task pool (150 x 0x70 at +0x2A0) on the second library
//! (`uv_rect`, `ring_quads`, `ring_band_fy`, `quad`) with five record-local
//! steps: on update 1 a full-window frame tint (0x1E02230, (0xA0, 0x78,
//! 0x78), ST inset by +0x48 / 512); on 41..310 a blue mote (0x1E01C90) a
//! update at the attacker's node 0x3F (ring_quads of 4, the bytes +0x451E);
//! on 212..330 every second update a widening red band (0x1E019D0,
//! `ring_band_fy`) from the attacker's position +-5, swaying by 10 x +0x38
//! x cos(+0x5C); on 230..350 every fourth update eight motes (0x1E01E40,
//! `quad`) at the target's eight random nodes (picked at launch: 0x1F66F0
//! x rand); the attacker's node 1 followed to +0x4470, slid to the target's
//! first node over updates 155..186 and copied to +0x250 on 186; the two
//! angle slots +0x450C / +0x450E turned 6 degrees a update. Returns 1 on
//! update 220, 4 on 360, 2 on 355, 6 on 231..349, 8 once the 374-update
//! timer +0x44B0 has run out.
use crate::b219::quads_with;
use crate::b076::request_node_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const BAND: u32 = 0x01E0_19D0;
const MOTE: u32 = 0x01E0_1C90;
const MOTE2: u32 = 0x01E0_1E40;
const QUADS: u32 = 0x01E0_2030;
const TINT: u32 = 0x01E0_2230;
const IDLE: u32 = 0x01E0_2410;
/// The return address of the entry's `jalr` into a step (0x1E02708 + 8).
const TASK_RA: u64 = 0x01E0_2710;
const TIMER: u32 = D + 0x44B0;
const COLOUR: u32 = D + 0x44B4;
const NODE1: u32 = D + 0x44E4;
const NODE2: u32 = D + 0x44E6;
const NODES: u32 = D + 0x44F4;
const ANGLE: u32 = D + 0x4506;
const ANGLES: u32 = D + 0x450C;
const INDEX: u32 = D + 0x4514;
const FACING: u32 = D + 0x4518;
const COUNTER: u32 = D + 0x451C;
const BYTES: u32 = D + 0x451E;
const BYTES2: u32 = D + 0x461E;
const FOLLOW: u32 = D + 0x4470;
const SPAWN_AT: u32 = D + 0x250;
const SINES: u32 = D + 0xD8;

/// The entry 0x1E02420 (a0 = D, a1 = param; s0 = D, s1 = param, s3 = 0).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(D + 0x44BC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4890);
    let p = param as i64 as u64;
    l2::sine_table_saving(fx, D + 0x4720, u64::from(D), p, 0x01E0_246C);
    l2::sine_table_saving(fx, SINES, u64::from(D), p, 0x01E0_2474);
    fx.mem.set16(D + 0x451A, param);
    fx.mem.set32(TIMER, 0x176);
    let h = fx.self_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(NODE1, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x3F, 0);
    fx.mem.set16(NODE2, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x4490 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.fix_residue(y, 0x01E0_253C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_2564);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_25AC);
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(BYTES2 + k, h - 0x14);
    }
    // Eight random nodes of the target (node count 0x1F66F0, rand % count,
    // 0 -> 1; the first request clears).
    for k in 0..8u32 {
        let h = fx.target_handle();
        let n = fx.unit_nodes(h);
        let v = fx.rand_from(D, 0x01E0_2608);
        let mut node = fx.div(v, n).1 as i16 as i32;
        if node == 0 {
            node = 1;
        }
        let h = fx.target_handle();
        // The entry's s0 = D, s1 = D + 2k, s2 = k; the last pick's frame
        // is read back by the frame tint (TEX1).
        let ra = if k == 0 { 0x01E0_265C } else { 0x01E0_2680 };
        let r = request_node_saving(fx, h, node, i32::from(k == 0), [Some(u64::from(D)), Some(u64::from(D + 2 * k)), Some(u64::from(k))], ra);
        fx.mem.set16(NODES + 2 * k, r);
    }
    fx.mem.set16(ANGLES, 0x10E);
    fx.mem.set16(ANGLES + 2, 0x5A);
    0
}

/// 0x1E02A90(D, step, 0): the first free task cleared and set off at
/// +0x250 with the colour +0x44B4 twice, angle +0x4506 (+0x5A), node index
/// +0x4514 (+0x50). `saves`: its prologue's (0x60: ra, s4 .. s0).
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, SPAWN_AT, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(SPAWN_AT + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(ANGLE);
            fx.mem.set16(t + 0x5A, a);
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E015F0(D, 0): the update counter, the followed node, the spawns and
/// the angles' turn. Frame 0x60 (ra, the entry's s2 = 0, s1 = the
/// parameter, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x30, 0x01E0_26D0), (0x20, 0), (0x10, param as i64 as u64), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let saves = |ra: u32, s2: u64, s0: u64| [(0x50u32, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, s2), (0x10, u64::from(D)), (0, s0)];
    let c = fx.mem.i16(COUNTER);
    if c < 0x9B {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE1);
        fx.node_position(h, slot, FOLLOW);
    } else {
        if c == 0x9B {
            let h = fx.target_handle();
            let slot = fx.mem.i16(NODES);
            fx.node_position(h, slot, sp + 0x40);
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(FOLLOW + 8)), 32.0);
            fx.mem.setf(D + 0x4488, v);
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x44), fx.mem.f(FOLLOW + 4)), 32.0);
            fx.mem.setf(D + 0x4484, v);
        }
        if fx.mem.i16(COUNTER) < 0xBA {
            let v = ee::add(fx.mem.f(FOLLOW + 8), fx.mem.f(D + 0x4488));
            fx.mem.setf(FOLLOW + 8, v);
            let v = ee::add(fx.mem.f(FOLLOW + 4), fx.mem.f(D + 0x4484));
            fx.mem.setf(FOLLOW + 4, v);
        }
    }
    if fx.mem.i16(COUNTER) == 0xBA {
        for k in 0..4 {
            let w = fx.mem.read_u32(FOLLOW + 4 * k);
            fx.mem.write_u32(SPAWN_AT + 4 * k, w);
        }
        fx.mem.set16(INDEX, 0);
    }
    // The delay slot clears the index on every update.
    fx.mem.set16(INDEX, 0);
    if fx.mem.i16(COUNTER) == 0x28 {
        fx.mem.set16(ANGLE, 0);
        fx.mem.set16(INDEX, 1);
        fx.mem.set16(ANGLE, 0x5A);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xD4..0x14B).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE2);
        fx.node_position(h, slot, sp + 0x40);
        let c = fx.mem.i16(COUNTER);
        let (parity, ra, sign) = if c % 2 == 0 { (0, 0x01E0_17C0, -1.0) } else { (1, 0x01E0_181C, 1.0) };
        fx.mem.set16(INDEX, parity);
        let h = fx.self_handle();
        fx.unit_position(h, SPAWN_AT);
        let y = ee::add(fx.mem.f(SPAWN_AT + 4), 5.0);
        fx.mem.setf(SPAWN_AT + 4, y);
        let x = ee::add(fx.mem.f(SPAWN_AT), ee::mul(sign, 5.0));
        fx.mem.setf(SPAWN_AT, x);
        spawn(fx, BAND, &saves(ra, 0, u64::from(D)));
    }
    let c = fx.mem.i16(COUNTER);
    if (0x29..0x137).contains(&c) {
        fx.mem.set16(INDEX, 0);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE2);
        fx.node_position(h, slot, SPAWN_AT);
        spawn(fx, MOTE, &saves(0x01E0_1864, 0, u64::from(D)));
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, TINT, &saves(0x01E0_1888, 0, u64::from(D)));
    }
    let c = fx.mem.i16(COUNTER);
    if (0xE6..0x15F).contains(&c) && c % 4 == 0 {
        for k in 0..8u32 {
            let h = fx.target_handle();
            let slot = fx.mem.i16(NODES + 2 * k);
            fx.node_position(h, slot, sp + 0x40);
            for o in 0..4 {
                let w = fx.mem.read_u32(sp + 0x40 + 4 * o);
                fx.mem.write_u32(SPAWN_AT + 4 * o, w);
            }
            spawn(fx, MOTE2, &saves(0x01E0_191C, u64::from(k), u64::from(D + 2 * k)));
        }
    }
    for o in [0u32, 2] {
        let a = fx.mem.i16(ANGLES + o) + 6;
        fx.mem.set16(ANGLES + o, a);
        let a = fx.mem.i16(ANGLES + o);
        if a >= 0x168 {
            let (_, h) = fx.div(a, 0x168);
            fx.mem.set16(ANGLES + o, h);
        }
    }
    fx.leave(0x60);
}

/// The alpha byte `o`'s fade: in by `up.1` below `up.0` while `rising`,
/// else out by `down.1` while at least `down.0`, else the task ends.
fn fade(fx: &mut Fx, t: u32, o: u32, rising: bool, up: (u8, u8), down: (u8, u8)) {
    let a = fx.mem.read_u8(t + o);
    if rising {
        if a < up.0 {
            fx.mem.write_u8(t + o, a + up.1);
        }
    } else if a >= down.0 {
        fx.mem.write_u8(t + o, a - down.1);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E019D0 (D, task; frame 0x30: ra, the entry's s1 = the slot, s0 = D):
/// a red band (outer (0xFF, 0x50, 0x50) / inner flickering (0xFF, 0x50,
/// 0x50) and (0xC0, 0x30, 0x30)) at its index's angle slot, widening by
/// 6.5 a update from 3 (inner 3 less), turned 0x4000, swaying to +-50 in
/// +0x34 by 1.6 and up to 10 in +0x38 by 0.2 (the sway's direction by the
/// facing), its angle +9 a update, moving 5 along z (by the facing); in by
/// 0x40 for 35 updates, then out by 9; the age held at 35 from update 351.
fn band(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(t - 0x2A0)), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4040_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0x50, 0x50);
        fx.mem.set16(t + 0x58, 2);
        let n = fx.mem.i16(t + 0x50) as u32;
        let a = fx.mem.i16(ANGLES.wrapping_add(n.wrapping_mul(2)));
        fx.mem.set16(t + 0x5C, a);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x23;
    fade(fx, t, 0x67, rising, (0x80, 0x40), (0xB, 9));
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x60, 0xFF, 0x50, 0x50);
    } else {
        l2::rgb(fx, t + 0x60, 0xC0, 0x30, 0x30);
    }
    fx.mem.write_u32(t + 0x20, 0x4000);
    let facing = fx.mem.i16(FACING) != 0;
    let sway = fx.mem.f(t + 0x34);
    if facing {
        if sway < 50.0 {
            fx.mem.setf(t + 0x34, ee::add(sway, f32::from_bits(0x3FCC_CCCD)));
        }
    } else if !(sway <= -50.0) {
        fx.mem.setf(t + 0x34, ee::sub(sway, f32::from_bits(0x3FCC_CCCD)));
    }
    let lift = fx.mem.f(t + 0x38);
    if lift < 10.0 {
        fx.mem.setf(t + 0x38, ee::add(lift, f32::from_bits(0x3E4C_CCCD)));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x48), 6.5);
        fx.mem.setf(t + 0x48, r);
        // Both facings: the inner radius 3 less.
        let r = fx.mem.f(t + 0x48);
        fx.mem.setf(t + 0x4C, ee::sub(r, 3.0));
    }
    let a = fx.mem.i16(t + 0x5C) + 9;
    fx.mem.set16(t + 0x5C, a);
    age(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        let z = fx.mem.f(t + 0x18);
        let z = if facing { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(t + 0x18, z);
    }
    if fx.mem.i16(COUNTER) >= 0x15F {
        fx.mem.set16(t + 0x54, 0x23);
    }
    fx.leave(0x30);
}

/// rand (the caller's s0 = the task) from `ra`, mod `m` (truncated).
fn rmod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// 0x1E01C90 (D, task; frame 0x20: ra, the entry's s0 = D): a grey mote
/// ring (ring_quads of 4 from the bytes +0x451E) of radius rand % 3 + 1,
/// 2..16 quads from angle rand % 360 stepping 16..35, the byte index rand
/// % 35, its cell frame rand % 8 (cleared while below 7, else counting up);
/// in by 0x40 below 0xC0 for
/// rand % 3 + 3 updates, then out by 0xE.
fn mote(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_1CB4, 3);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 1));
        let v = rmod(fx, t, 0x01E0_1CE0, 8);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_1D14, 0xF);
        fx.mem.set16(t + 0x50, v + 2);
        let v = rmod(fx, t, 0x01E0_1D34, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_1D50, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, 0x01E0_1D70, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1D8C, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, 0x63, rising, (0xC0, 0x40), (0x10, 0xE));
    // The compiled test is inverted: the frame is cleared while below 7
    // and kept from 7 on [F 0x1E01E14].
    let c = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, if c < 7 { 0 } else { c });
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01E40 (D, task; frame 0x20): b494's mote on the target's nodes: a
/// quad of 30..59 (growing by 1 to 35 every third update) at angle rand %
/// 360, blue (0x30, 0x30, 0xFF / 0xC0) flickering, rising by 0.7, its cell
/// frame (rand >> 4) % 8 cycling; in by 0x40 for 7 updates, then out by 9;
/// the camera's angles into +0x20.
fn mote2(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_1E64, 0x1E);
        fx.mem.set16(t + 8, v + 0x1E);
        let v = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, v);
        let v = fx.rand_from(t, 0x01E0_1E8C);
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, 0x01E0_1EC8, 0x168);
        fx.mem.set16(t + 0x5C, v);
    }
    let rising = fx.mem.i16(t + 0x54) < 7;
    fade(fx, t, 0x63, rising, (0x80, 0x40), (0xB, 9));
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x60, 0x30, 0x30, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0x30, 0x30, 0xC0);
    }
    let c = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, if c >= 8 { 0 } else { c });
    if fx.mem.i16(t + 0x54) % 3 == 1 && fx.mem.i16(t + 8) < 0x23 {
        let v = fx.mem.i16(t + 8) + 1;
        fx.mem.set16(t + 8, v);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let y = ee::add(fx.mem.f(t + 0x14), f32::from_bits(0x3F33_3333));
        fx.mem.setf(t + 0x14, y);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(t - 0x2A0), 0x01E0_2010);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02230 (D, task; frame 0x90: ra, the entry's s1 = the slot, s0 = D):
/// a window sprite (0x6FF8 .. 0x8FF8) sampling the frame (0x207EF0, filter
/// 1) at ST inset by +0x48 (1) / 512, colour (0xA0, 0x78, 0x78, alpha
/// +0x63), ALPHA 0x44, key 0; in by 8 below 0x50 until update 355, then
/// out by 7.
fn tint(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(t - 0x2A0)), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in [0xA0u8, 0x78, 0x78].into_iter().enumerate() {
        fx.mem.write_u8(sp + 0x48 + k as u32, c);
    }
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(sp + 0x4B, a);
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(512.0, s), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(512.0, s), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let rising = fx.mem.i16(COUNTER) < 0x163;
    fade(fx, t, 0x63, rising, (0x50, 8), (9, 7));
    age(fx, t);
    fx.leave(0x90);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 || f == IDLE {
            continue;
        }
        let t = slot + 0x2A0;
        match f {
            BAND => band(fx, t),
            MOTE => mote(fx, t),
            MOTE2 => mote2(fx, t),
            QUADS => quads_with(fx, t, QUADS, [0xC0, 0x50, 0x50], 0xC8, TASK_RA),
            TINT => tint(fx, t),
            _ => panic!("b247: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let s = D + 0x70 * k;
        if fx.mem.i16(s + 0x2F8) == 0 {
            continue;
        }
        let t = s + 0x2A0;
        let f = fx.mem.read_u32(t + 0x44);
        if f == BAND {
            for o in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * o);
                fx.mem.write_u32(sp + 0x50 + 4 * o, w);
            }
            let idx = fx.mem.i16(t + 0x50);
            if idx == 0 || idx == 1 {
                let a = fx.mem.i16(t + 0x5C);
                let c = l2::sin_deg(fx, SINES, a, 1);
                let d = ee::mul(10.0, ee::mul(fx.mem.f(t + 0x38), c));
                let x = fx.mem.f(sp + 0x50);
                let x = if idx == 0 { ee::add(x, d) } else { ee::sub(x, d) };
                fx.mem.setf(sp + 0x50, x);
            }
            quads4::ring_band_fy(fx, D, t, 0x20, sp + 0x50);
        }
        if f == MOTE {
            let w = fx.mem.read_u32(D + 0x4490);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            l2::rect(fx, sp + 0x60, 0x14, sub);
            for k in 0..4 {
                let w = fx.mem.read_u32(sp + 0x60 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 4);
            let v = fx.rand_from(D, 0x01E0_2880);
            let (_, h) = fx.div(v, 4);
            fx.mem.set16(D + 0xBE, h);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == MOTE2 {
            let w = fx.mem.read_u32(D + 0x4494);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            l2::rect(fx, sp + 0x70, 0x14, sub);
            for k in 0..4 {
                let w = fx.mem.read_u32(sp + 0x70 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, w);
            }
            fx.mem.set16(D + 0xBE, 0);
            for o in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * o);
                fx.mem.write_u32(sp + 0x50 + 4 * o, w);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        }
        if f == QUADS {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            l2::rect(fx, sp + 0x80, 0x97, sub);
            for k in 0..4 {
                let w = fx.mem.read_u32(sp + 0x80 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xDC => 1,
        0x168 => 4,
        0x163 => 2,
        0xE7..=0x15D => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}
