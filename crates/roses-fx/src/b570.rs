//! BMODULE b570 (card 20), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03020, data area 0x1ED0000) [F]: two task pools, 200 older tasks of 0x40 bytes at D+0x2A0 (alive
//! +0x36, step +0x24, run by address) and 200 tasks of 0x70 bytes at
//! D+0x4120 (alive +0x58, step +0x44, called through `jalr`), drawn with
//! the second library: rings at D+0x20 with a mode (383a1b86ead2 at
//! 0x1E00EA0, `quads4::ring_at_m`) or at the task (737e393f7a6a at
//! 0x1E00920, `quads4::ring_at_mt`) and single quads (add5d81dd3fe at
//! 0x1E01400, `billboard::quad`), UVs by uv_rect 0x1E00080. From update
//! 121 to 284 purple smoke columns rise from the attacker's node 0x1D
//! (heights / depths in two tables of 200 floats, D+0x9914 / D+0x9C34,
//! drifting to the target) with purple sparks from node 0x1B; until 249
//! grey rings between nodes 0x1B and 0x1C; from 191 white flashes at a
//! random one of eight target nodes; from 319 to 349 sparks at node 0x1D
//! and its positions kept in a table at D+0x34A0. Returns 1 on update 160,
//! 4 on 365, 2 on 295, 6 on 181..289, 8 once the 365-update timer +0x98F0
//! has run out.
//!
//! The older pool's steps 0x1E02250 (a printf), 0x1E02430, 0x1E02B80
//! (inlined in the entry), 0x1E02C30, 0x1E02D10 (a frame tint through
//! 0x1F7E20), 0x1E02F00 (the stage light) and 0x1E02FD0 (an object op
//! through 0x1F8380) are never spawned [F: the emitter's spawn `jal`s name
//! 0x1E01C30 / 0x1E020A0 / 0x1E02280 and 0x1E02680 / 0x1E028B0 only]; not
//! ported.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The older pool's steps (+0x24).
const SMOKE: u32 = 0x01E0_1C30;
const RING: u32 = 0x01E0_20A0;
const FLASH: u32 = 0x01E0_2280;
/// The second pool's steps (+0x44).
const SPARK: u32 = 0x01E0_2680;
const SPARK_T: u32 = 0x01E0_28B0;
const POOL2: u32 = D + 0x4120;
const TEX: u32 = D + 0x98A0;
const TIMER: u32 = D + 0x98F0;
const COLOUR: u32 = D + 0x98F4;
/// The smoke columns' heights / depths, one per slot +0xA.
const HEIGHTS: u32 = D + 0x9914;
const DEPTHS: u32 = D + 0x9C34;
const NODES: u32 = D + 0x9F64;
const TNODES: u32 = D + 0x9F74;
const NEXT: u32 = D + 0x9F88;
const NEXT2: u32 = D + 0x9F8A;
const SLOT: u32 = D + 0x9F90;
const FACING: u32 = D + 0x9F94;
const COUNTER: u32 = D + 0x9F96;
const BYTES: u32 = D + 0x9F98;
const BYTES2: u32 = D + 0xA098;
const BYTES3: u32 = D + 0xA198;

/// The entry 0x1E03020.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x98FC, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xA410);
    l2::sine_table(fx, D + 0xA298);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x9F92, param);
    fx.mem.set32(TIMER, 0x16D);
    for (k, node) in [0x1F, 0x20, 0x1B, 0x1C, 0x1D, 0x1D].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // Eight random nodes of the target (rand's frame: s0 = the count, as
    // the 64-bit register).
    for k in 0..8u32 {
        let h = fx.target_handle();
        let n = fx.unit_nodes(h);
        let f = fx.sp.wrapping_sub(0x10);
        fx.mem.write_u64(f, n as i64 as u64);
        fx.mem.write_u64(f + 8, 0x01E0_3180);
        let v = fx.rand();
        let node = match fx.div(v >> 4, n).1 {
            0 => 1,
            h => h,
        };
        let h = fx.target_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(TNODES + 2 * k, r);
    }
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x288 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x268, t);
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for (table, site) in [(BYTES, 0x01E0_32D4), (BYTES2, 0x01E0_3314), (BYTES3, 0x01E0_3354)] {
        for k in 0..0x100u32 {
            let v = fx.rand_from(k, site);
            fx.mem.set8(table + k, v % 16 - 8);
        }
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(BYTES3 + k, 0);
    }
    for k in 0..8u32 {
        let v = fx.rand_from(k, 0x01E0_33B4);
        fx.mem.set16(D + 0xC0 + 2 * k, v % 4);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let s = D + 0x40 * k;
        let t = s + 0x2A0;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == SMOKE {
            smoke(fx, t, s);
        }
        if fx.mem.read_u32(t + 0x24) == RING {
            ring(fx, t);
        }
        if fx.mem.read_u32(t + 0x24) == FLASH {
            flash(fx, t);
        }
        debug_assert!(!matches!(fx.mem.read_u32(t + 0x24), 0x01E0_2250 | 0x01E0_2430 | 0x01E0_2B80 | 0x01E0_2C30 | 0x01E0_2D10 | 0x01E0_2F00 | 0x01E0_2FD0));
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x60);
    // 0x1F71A0's frame below the entry's (its saved ra, the entry's s0).
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_361C), (0, 0)]);
    // The entry's s0: the live tasks counted in the draw loops.
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        s0 += 1;
        if fx.mem.read_u32(t + 0x24) == SMOKE {
            whole_uv(fx);
            let n = fx.mem.i16(t + 0xC);
            fx.mem.set16(D + 0xB8, n);
            if fx.mem.i16(t + 0x32) >= 4 {
                fx.mem.set16(D + 0xB8, 3);
                rand_orient(fx, s0, 0x01E0_3690);
                let x = ee::from_i32(fx.mem.i16(t));
                fx.mem.setf(D + 0x20, x);
                let j = fx.mem.i16(t + 0xA) as u32;
                let w = fx.mem.read_u32(HEIGHTS.wrapping_add(4 * j));
                fx.mem.write_u32(D + 0x24, w);
                let w = fx.mem.read_u32(DEPTHS.wrapping_add(4 * j));
                fx.mem.write_u32(D + 0x28, w);
                quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
        }
        if fx.mem.read_u32(t + 0x24) == RING {
            cell(fx, t, sp + 0x70);
            fx.mem.set16(D + 0xB8, 4);
            rand_orient(fx, s0, 0x01E0_3778);
            let mode = fx.mem.i16(t + 0xA) << 4;
            quads4::ring_at_mt(fx, uv::uv_rect, D, t, mode, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x24) == FLASH {
            cell(fx, t, sp + 0x80);
            fx.mem.set16(D + 0xB8, 2);
            rand_orient(fx, s0, 0x01E0_3818);
            for k in 0..3 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                fx.mem.setf(D + 0x20 + 4 * k, v);
            }
            quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x21, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    for k in 0..TASKS {
        let t = POOL2 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x44) {
            0 => {}
            SPARK => spark(fx, t, s0, false),
            SPARK_T => spark(fx, t, s0, true),
            f => panic!("b570: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = POOL2 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        // (The entry's s0 counts these too; nothing reads it after.)
        if fx.mem.read_u32(t + 0x44) == SPARK {
            // The cell row is the older pool's task k's +0x34 (as the
            // original indexes).
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(D + 0x2A0 + 0x40 * k + 0x34);
            l2::rect(fx, sp + 0x90, 0x9B, sub);
            for c in 0..4 {
                let v = fx.mem.read_u32(sp + 0x90 + 4 * c);
                fx.mem.write_u32(D + 0x94 + 4 * c, v);
            }
            whole_uv(fx);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == SPARK_T {
            whole_uv(fx);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xA0 => 1,
        0x16D => 4,
        0x127 => 2,
        0xB5..=0x121 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// Texture (5, 0) with the whole cell: UV (1, 1) size (63, 63).
fn whole_uv(fx: &mut Fx) {
    let w = fx.mem.read_u32(TEX);
    fx.mem.write_u32(D + 0x88, w);
    for (o, v) in [(0x94u32, 1), (0x96, 1), (0x98, 0x3F), (0x9A, 0x3F)] {
        fx.mem.set16(D + o, v);
    }
}

/// Texture (5, 1) and the atlas cell 0x14 / +0x34 (0x106960 into `r`)
/// copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let w = fx.mem.read_u32(TEX + 4);
    fx.mem.write_u32(D + 0x88, w);
    let sub = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// D+0xBE = rand % 4 from the entry (rand's return address `ra`).
fn rand_orient(fx: &mut Fx, s0: u32, ra: u32) {
    let v = fx.rand_from(s0, ra);
    fx.mem.set16(D + 0xBE, v % 4);
}

/// rand % m (HI) from a step's `jal` at `site` (rand's frame: s0 = `t`).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// 0x1A2B80 (float to integer) from `site` with its frame's residue.
fn fix(fx: &mut Fx, v: f32, site: u32) -> i32 {
    fx.fix_residue(v, site + 8);
    ee::fixsfsi(v)
}

/// A node of the attacker into D+0x250.. (through the frame's sp + 0x30).
fn at_node(fx: &mut Fx, sp: u32, at: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(at);
    fx.node_position(h, slot, sp + 0x30);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
        fx.mem.write_u32(D + 0x250 + 4 * k, w);
    }
}

/// 0x1E016C0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x50: ra, the entry's s1 = D / s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_33F0), (0x10, u64::from(D)), (0, 0)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    // Sparks at node 0x1D (the second one).
    if (0x13F..0x15E).contains(&c) {
        at_node(fx, sp, NODES + 0xA);
        spawn2(fx, SPARK);
    }
    let c = fx.mem.i16(COUNTER);
    // A smoke column at node 0x1D, 4.5 towards the facing.
    if (0x79..0x11D).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 8);
        fx.node_position(h, slot, sp + 0x30);
        for k in 0..2 {
            let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        let z0 = fx.mem.f(sp + 0x38);
        let z = if fx.mem.i16(FACING) != 0 { ee::add(4.5, z0) } else { ee::sub(z0, 4.5) };
        fx.mem.setf(D + 0x258, z);
        let s = fx.mem.i16(NEXT);
        fx.mem.set16(SLOT, s);
        spawn(fx, SMOKE);
        let s = fx.mem.i16(NEXT) + 1;
        fx.mem.set16(NEXT, s);
        if fx.mem.i16(NEXT) >= 0xC8 {
            fx.mem.set16(NEXT, 0);
        }
    }
    let c = fx.mem.i16(COUNTER);
    // A purple spark at node 0x1B, 2 up and 1 against the facing.
    if (0x79..0x12C).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 4);
        fx.node_position(h, slot, sp + 0x30);
        let x = fx.mem.read_u32(sp + 0x30);
        fx.mem.write_u32(D + 0x250, x);
        let y = ee::add(2.0, fx.mem.f(sp + 0x34));
        fx.mem.setf(D + 0x254, y);
        let z0 = fx.mem.f(sp + 0x38);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z0, 1.0) } else { ee::add(1.0, z0) };
        fx.mem.setf(D + 0x258, z);
        spawn2(fx, SPARK_T);
    }
    let c = fx.mem.i16(COUNTER);
    // A ring half way between nodes 0x1B and 0x1C.
    if c < 0xFA {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 4);
        fx.node_position(h, slot, sp + 0x30);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 6);
        fx.node_position(h, slot, sp + 0x40);
        for k in 0..3 {
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x40 + 4 * k), fx.mem.f(sp + 0x30 + 4 * k)), 2.0);
            fx.mem.setf(sp + 0x40 + 4 * k, v);
        }
        for k in 0..3 {
            let v = ee::add(fx.mem.f(sp + 0x30 + 4 * k), fx.mem.f(sp + 0x40 + 4 * k));
            fx.mem.setf(D + 0x250 + 4 * k, v);
        }
        let c = fx.mem.i16(COUNTER);
        let s = if fx.div(c, 5).1 == 1 { 1 } else { 2 };
        fx.mem.set16(SLOT, s);
        spawn(fx, RING);
    }
    let c = fx.mem.i16(COUNTER);
    // Node 0x1D's positions kept at D+0x34A0 (200 entries).
    if (0x13F..0x15E).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 0xA);
        let j = fx.mem.i16(NEXT2);
        fx.node_position(h, slot, (D + 0x34A0).wrapping_add((j << 4) as u32));
        let j = fx.mem.i16(NEXT2);
        fx.mem.set16(SLOT, j);
        fx.mem.set16(NEXT2, j + 1);
        if fx.mem.i16(NEXT2) >= 0xC8 {
            fx.mem.set16(NEXT2, 0);
        }
    }
    let c = fx.mem.i16(COUNTER);
    // A white flash at one of the target's eight nodes.
    if (0xBF..0x15E).contains(&c) && fx.div(c, 7).1 != 1 {
        let v = fx.rand_from(D, 0x01E0_1BB4);
        let j = (v % 8) as u32;
        let h = fx.target_handle();
        let slot = fx.mem.i16(TNODES + 2 * j);
        fx.node_position(h, slot, sp + 0x30);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        spawn(fx, FLASH);
    }
    fx.leave(0x50);
}

/// 0x1E03B40(D, step, 0): the first free older task cleared and set off
/// at D+0x250 (as i16) with the colour D+0x98F4 (twice), +0x30 = D+0x9F90;
/// 1 when one was free.
fn spawn(fx: &mut Fx, step: u32) -> i32 {
    fx.enter(0x60);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2D6 + 0x40 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            colours(fx, t + 0x28);
            l2::to_i16x4(fx, t, D + 0x250, 0);
            let a = fx.mem.i16(SLOT);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// The spawn colour (0x107FD0 + alpha) at `c` and `c` + 4.
fn colours(fx: &mut Fx, c: u32) {
    for o in [0u32, 4] {
        let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
        l2::rgb(fx, c + o, i32::from(r), i32::from(g), i32::from(b));
        let a = fx.mem.read_u8(COLOUR + 3);
        fx.mem.write_u8(c + o + 3, a);
    }
}

/// 0x1E03CB0(D, step, 0): the first free second-pool task cleared and set
/// off at D+0x250 (as i16 and as floats +0x10) with the colour (+0x60,
/// +0x64), +0x50 = D+0x9F90.
fn spawn2(fx: &mut Fx, step: u32) -> i32 {
    fx.enter(0x60);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL2 + 0x58 + 0x70 * k) == 0) {
        Some(k) => {
            let t = POOL2 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            colours(fx, t + 0x60);
            l2::to_i16x4(fx, t, D + 0x250, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(SLOT);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E01C30 (D, task): a purple smoke column (frame 0x70: the entry's s2
/// = the slot `s`, s1 = D, s0 = 0): size 12..23 growing by 2 (under 60)
/// and 3 on odd ages, its slot +0xA (= +0x30) in the height / depth tables
/// (the task's y - 0.7, z), angle +0x3A random turning by 2, row 0..34,
/// life 37..61, +0xC the x distance of nodes 0x20 - 0x1F (also the ring's
/// quad count while young); fades in by 0x40 (under age 314), out by 7
/// (under age 314) or 9 (from 315), as the original compares the age; the
/// height sinks by 0.02 and the depth drifts by the target's distance /
/// 70.
fn smoke(fx: &mut Fx, t: u32, s: u32) {
    let f = SMOKE;
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x30, 0x01E0_3424), (0x20, u64::from(s)), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rmod(fx, t, f + 0x28, 0xC);
        fx.mem.set16(t + 0x38, v + 0xC);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x80, 0x10, 0xFF);
        let j = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, j);
        let j = j as u32;
        let y = ee::from_i32(fx.mem.i16(t + 2));
        fx.mem.setf(HEIGHTS.wrapping_add(4 * j), y);
        let z = ee::from_i32(fx.mem.i16(t + 4));
        fx.mem.setf(DEPTHS.wrapping_add(4 * j), z);
        let p = HEIGHTS.wrapping_add(4 * j);
        let y = ee::sub(fx.mem.f(p), f32::from_bits(0x3F33_3333));
        fx.mem.setf(p, y);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, f + 0xE0, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, f + 0xFC, 5);
        fx.mem.set16(t + 0xE, v + 2);
        let v = rmod(fx, t, f + 0x11C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0x138, 0x19);
        fx.mem.set16(t + 6, v + 0x25);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        fx.node_position(h, slot, sp + 0x40);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2);
        fx.node_position(h, slot, sp + 0x50);
        let dx = ee::sub(fx.mem.f(sp + 0x50), fx.mem.f(sp + 0x40));
        let dx = fix(fx, dx, f + 0x1A4);
        fx.mem.set16(t + 0xC, dx);
        fx.mem.set16(t + 0x36, 2);
    }
    let age = fx.mem.i16(t + 0x32);
    if age < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x2B);
        if age < 0x13A && a < 0x81 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else if age < 0x13A {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 9 {
            fx.mem.write_u8(t + 0x2B, a - 7);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x32) >= 0x13B {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x2B, a - 9);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let g = fx.mem.i16(t + 0x38);
        if g < 0x3C {
            fx.mem.set16(t + 0x38, g + 2);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x40);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x50);
    if fx.mem.i16(t + 0x32) != 0 {
        let dz = ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(sp + 0x58));
        fx.mem.setf(sp + 0x48, dz);
        let dz = ee::div(dz, 70.0);
        fx.mem.setf(sp + 0x48, dz);
        let j = fx.mem.i16(t + 0xA) as u32;
        let p = DEPTHS.wrapping_add(4 * j);
        let z = ee::add(fx.mem.f(p), dz);
        fx.mem.setf(p, z);
    }
    let j = fx.mem.i16(t + 0xA) as u32;
    let p = HEIGHTS.wrapping_add(4 * j);
    let y = ee::sub(fx.mem.f(p), f32::from_bits(0x3CA3_D70A));
    fx.mem.setf(p, y);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 5 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let g = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, g + 3);
    }
    for (o, site) in [(0x10u32, f + 0x3AC), (0x14, f + 0x3C8)] {
        let v = rmod(fx, t, site, 0x168);
        fx.mem.set32(t + o, v);
    }
    let a = fx.mem.i16(t + 0x3A);
    let turn = if fx.mem.i16(t + 0xE) % 2 == 1 { 2 } else { -2 };
    fx.mem.set16(t + 0x3A, a + turn);
    let v = rmod(fx, t, f + 0x428, 0x168);
    fx.mem.set32(t + 0x18, v);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x70);
}

/// 0x1E020A0 (D, task): a grey ring (frame 0x20): size 20 growing by 1,
/// the radius +0x30 kept in +0xA (its blend in the draw) and growing from
/// 0 by 5..9, row 0..24, life 30..39, a random angle; fades in by 0x20
/// under 0x40, out by 9; the cell frame counting to 5 on odd ages.
fn ring(fx: &mut Fx, t: u32) {
    let f = RING;
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_3448), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x14);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x70, 0x70, 0x70);
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, f + 0x44, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rmod(fx, t, f + 0x64, 0x19);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0x80, 0xA);
        fx.mem.set16(t + 6, v + 0x1E);
        let v = rmod(fx, t, f + 0xA0, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x40 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x20));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    step_tail(fx, t, 5, 1);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
}

/// The cell frame counting to `cells` on odd ages; with age the radius by
/// +0xE and the size by `grow`.
fn step_tail(fx: &mut Fx, t: u32, cells: i32, grow: i32) {
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < cells {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if grow != 0 && fx.mem.i16(t + 0x32) != 0 {
        let g = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, g + grow);
    }
}

/// 0x1E02280 (D, task): a white flash (frame 0x20): size 50 growing by 2,
/// the radius kept in +0xA and growing from 0 by 4, row 0..24, life 7..14,
/// a random angle; fades in by 0x40 under 0x80, out by 13; the cell frame
/// counting to 3 on odd ages; rises by 1 a update.
fn flash(fx: &mut Fx, t: u32) {
    let f = FLASH;
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_3488), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x32);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0xFF, 0xFF, 0xFF);
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        fx.mem.set16(t + 0x30, 0);
        fx.mem.set16(t + 0xE, 4);
        let v = rmod(fx, t, f + 0x4C, 0x19);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, f + 0x70);
        fx.mem.set16(t + 6, v % 8 + 7);
        let v = rmod(fx, t, f + 0x88, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else if a >= 0xD {
        fx.mem.write_u8(t + 0x2B, a - 0xD);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let g = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, g + 2);
    }
    step_tail(fx, t, 3, 0);
    if fx.mem.i16(t + 0x32) != 0 {
        let y = fx.mem.i16(t + 2) + 1;
        fx.mem.set16(t + 2, y);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x20);
}

/// 0x1E02680 / 0x1E028B0 (`target`) (D, task): a purple spark quad of the
/// second pool (frame 0x20 / 0x60: the entry's s0 = `s0`), its size 2..4
/// (3..5), cell row (rand >> 4) % 3, drift +0x30 / +0x34 in (-50..49) /
/// 400, angle +0x5C, turned by the camera's angles each update (0x205580
/// into +0x20) at a random offset +0..+4 (+-1024); 0x1E028B0 also drifts
/// +0x38 = the target's z distance / (50..149) / 10 a update. Fades in
/// by 0x40 under 0x80 for 90 (10) updates, out by 9; falls 0.09 (0.29) a
/// update.
fn spark(fx: &mut Fx, t: u32, s0: u32, target: bool) {
    let frame = if target { 0x60 } else { 0x20 };
    let sp = fx.enter(frame);
    if target {
        fx.saved(sp, &[(0x20, 0x01E0_3954), (0x10, u64::from(D)), (0, u64::from(s0))]);
    } else {
        fx.saved(sp, &[(0x10, 0x01E0_3954), (0, u64::from(s0))]);
    }
    // The rand / 0x205580 `jal`s (0x1E02680's; 0x1E028B0's 0x238 on).
    let b = if target { 0x238 } else { 0 };
    let at = |a: u32| a + b;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, at(0x01E0_269C), 3);
        fx.mem.set16(t + 8, v + if target { 3 } else { 2 });
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, at(0x01E0_26CC));
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h);
        l2::rgb(fx, t + 0x60, 0x80, 0x20, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, at(0x01E0_26F8), 0x64);
        fx.mem.setf(t + 0x34, ee::from_i32(v - 0x32));
        let v = rmod(fx, t, at(0x01E0_2724), 0x64);
        fx.mem.setf(t + 0x30, ee::from_i32(v - 0x32));
        for k in [0x34u32, 0x30] {
            let q = ee::div(fx.mem.f(t + k), 400.0);
            fx.mem.setf(t + k, q);
        }
        let v = rmod(fx, t, at(0x01E0_2770), 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), at(0x01E0_2794));
        for (k, site) in [(4u32, 0x01E0_2794u32), (2, 0x01E0_27B4), (0, 0x01E0_27D4)] {
            let v = fx.rand_from(t, at(site) + 8);
            fx.mem.set16(t + k, (v & 0x7FF) - 0x400);
        }
        if target {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 8);
            fx.node_position(h, slot, sp + 0x30);
            let h = fx.target_handle();
            fx.unit_position(h, sp + 0x40);
            let v = rmod(fx, t, 0x01E0_2A60, 0x64);
            let dz = ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(sp + 0x38));
            let q = ee::div(dz, ee::from_i32(v + 0x32));
            fx.mem.setf(t + 0x38, q);
            let q = ee::div(q, 10.0);
            fx.mem.setf(t + 0x38, q);
        }
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < if target { 0xA } else { 0x5A } {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        if target {
            let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x38));
            fx.mem.setf(t + 0x18, z);
        }
        let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
        fx.mem.setf(t + 0x10, x);
        let fall = f32::from_bits(if target { 0x3E94_7AE1 } else { 0x3DB8_51EC });
        let y = ee::add(fx.mem.f(t + 0x14), ee::sub(fx.mem.f(t + 0x34), fall));
        fx.mem.setf(t + 0x14, y);
    }
    let ra = if target { 0x01E0_2B54 } else { 0x01E0_2890 };
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), ra);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(frame);
}
