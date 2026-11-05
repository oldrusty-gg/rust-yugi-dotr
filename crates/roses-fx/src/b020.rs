//! BMODULE b020 (card 58), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E026C0, data area 0x1ED0000) [F]: b547 / b747's task pool, here 250 tasks of 0x70 bytes at D+0x2A0
//! (alive +0x58, step +0x44 through `jalr`; spawned by 0x1E02D70, b747's
//! spawn on this layout), drawn with the second library (`uv_rect`,
//! `ring_facing_flat_z`, `sprite_quads`, `billboard::quad`,
//! `quad_b_turned` twice: 0x1E011E0 and its copy 0x1E02EC0). On update 1
//! two sprite columns (0x1E022D0, b539's column: grey / dark red, at the
//! attacker's nodes 0x15 / 0x16) and a quad pair (0x1E023E0); on
//! 0x86..0xB7 three white sparks a update (0x1E01E70) from node 0x17
//! jittered +-10 and 50 to the facing; on 0xA0..0x136 a falling ring
//! (0x1E01FC0, `ring_facing_flat_z` of 32 from the bytes D+0x70B4) and four
//! turned quads (0x1E01B50) a update; on 0x80 a window tint (0x1E024C0,
//! (0x90, 0x78, 0x78)). Returns 1 on update 130, 4 on 335, 2 on 325, 6 on
//! 141..319, 8 once the 350-update timer D+0x7050 has run out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 250;
/// The step functions a task holds (+0x44), as the record's addresses.
const QUADS: u32 = 0x01E0_1B50;
const SPARK: u32 = 0x01E0_1E70;
const RING: u32 = 0x01E0_1FC0;
const COLUMN: u32 = 0x01E0_22D0;
const PAIR: u32 = 0x01E0_23E0;
const TINT: u32 = 0x01E0_24C0;
/// The return address of the entry's `jalr` into a step (0x1E02958 + 8).
const TASK_RA: u64 = 0x01E0_2960;
/// The return address of the entry's call into the emitter 0x1E017A0.
const EMIT_RA: u64 = 0x01E0_2930;
const SPAWN_AT: u32 = D + 0x250;
const TEX_Q: u32 = D + 0x27C;
const TEX_S: u32 = D + 0x29C;
const TEX: u32 = D + 0x7030;
const TIMER: u32 = D + 0x7050;
const COLOUR: u32 = D + 0x7054;
const NODE_A: u32 = D + 0x707C;
const NODE_B: u32 = D + 0x707E;
const NODE_C: u32 = D + 0x7080;
const TNODE: u32 = D + 0x708A;
const SPIN: u32 = D + 0x709E;
const INDEX: u32 = D + 0x70AA;
const NEXT: u32 = D + 0x70AC;
const FACING: u32 = D + 0x70AE;
const COUNTER: u32 = D + 0x70B2;
const BYTES: u32 = D + 0x70B4;
const BYTES2: u32 = D + 0x71B4;
const SINES: u32 = D + 0x72B4;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E026C0 (a0 = D, a1 = param; s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0x705C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x7420);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x70B0, param);
    fx.mem.set32(TIMER, 0x15E);
    for (at, node, clear) in [(NODE_A, 0x15, 1), (NODE_B, 0x16, 0), (NODE_C, 0x17, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TNODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(TEX_S, t);
    let t = fx.texture(4, 7);
    fx.mem.write_u32(TEX_Q, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_281C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // The entry's s0 is D here (rand's frame).
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2844, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_2894);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_28E4, 0x28);
        fx.mem.set8(BYTES2 + k, v - 0x14);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => {}
            QUADS => quads(fx, t),
            SPARK => spark(fx, t),
            RING => ring(fx, t),
            COLUMN => column(fx, t),
            PAIR => pair(fx, t),
            TINT => tint(fx, t),
            _ => panic!("b020: unknown task step {f:#x}"),
        }
    }
    // The entry's s1 counts the live tasks drawn so far; s2 = D + 0x70 k,
    // s3 = k (the node positions' frames).
    let mut s1 = 0u64;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        s1 += 1;
        let at_task = |fx: &mut Fx| {
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, v);
            }
        };
        if fx.mem.read_u32(t + 0x44) == QUADS {
            let w = fx.mem.read_u32(TEX_Q);
            fx.mem.write_u32(D + 0x88, w);
            let v = fx.mem.f(t + 0x3C);
            fx.fix_residue(v, 0x01E0_29C8);
            fx.mem.set16(D + 0xBE, ee::fixsfsi(v));
            fx.mem.set16(D + 0x94, 1);
            fx.mem.set16(D + 0x96, 1);
            fx.mem.set16(D + 0x98, 0x40);
            fx.mem.set16(D + 0x9A, 0x40);
            at_task(fx);
            billboard::quad_b_turned(fx, uv::uv_rect, D, t, 0x10, sp + 0x50);
        }
        if fx.mem.read_u32(t + 0x44) == SPARK {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x60, 0x14, sub);
            fx.mem.set16(D + 0xBE, 5);
            at_task(fx);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x14, sub);
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 5);
            let o = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, o);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        let regs = |fx: &mut Fx, node: u32, ra: u32| {
            let h = fx.self_handle();
            let slot = fx.mem.i16(node);
            node_position_saving(fx, h, slot, sp + 0x50, u64::from(D), Some(s1), ra);
        };
        if fx.mem.read_u32(t + 0x44) == COLUMN {
            regs(fx, NODE_A, 0x01E0_2B5C);
            let w = fx.mem.read_u32(TEX_S);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, sp + 0x80, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.write_u32(sp + 0x5C, 0);
            quads4::sprite_quads(fx, D, t, 0, sp + 0x50);
            regs(fx, NODE_B, 0x01E0_2BD0);
            quads4::sprite_quads(fx, D, t, 0, sp + 0x50);
        }
        if fx.mem.read_u32(t + 0x44) == PAIR {
            regs(fx, NODE_A, 0x01E0_2C14);
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, sp + 0x90, 0x14, 0);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.write_u32(sp + 0x5C, 0);
            billboard::quad_b_turned(fx, uv::uv_rect, D, t, 0x21, sp + 0x50);
            regs(fx, NODE_B, 0x01E0_2C88);
            billboard::quad_b_turned(fx, uv::uv_rect, D, t, 0x21, sp + 0x50);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x82 => 1,
        0x14F => 4,
        0x145 => 2,
        0x8D..=0x13F => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` frame `sub` (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E02D70(D, step, 0): the first free of the 250 tasks cleared and set
/// off at D+0x250 with the colour D+0x7054 twice, angle D+0x709E (+0x5A),
/// index D+0x70AA (+0x50), +0x40 0. `saves`: its prologue's (0x60: s0 +0
/// .. s4 +0x40, ra +0x50).
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
            let a = fx.mem.i16(SPIN);
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

/// D+0x250..+0x258 each moved by rand % 20 - 10 (rand's s0 `s0`).
fn jitter(fx: &mut Fx, s0: u32, ras: [u32; 3]) {
    for (o, ra) in [0x250u32, 0x254, 0x258].into_iter().zip(ras) {
        let r = rmod(fx, s0, ra, 0x14);
        let v = ee::add(fx.mem.f(D + o), ee::from_i32(r - 0xA));
        fx.mem.setf(D + o, v);
    }
}

/// 0x1E017A0(D, 0): the update counter and the spawns (frame 0x40: ra, the
/// entry's s2 = the parameter, s1 = 0, s0 = D; its s2 = D, s0 / s1 the
/// loops' index / count).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, EMIT_RA), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let dd = u64::from(D);
    // The spawn's saves: s0, s1 as the emitter's, s2 = D, s3 / s4 the
    // runner's (0) [H].
    let saves = |s0: u64, s1: u64, ra: u32| [(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, dd), (0x10, s1), (0, s0)];
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 1 {
        fx.mem.set16(SPIN, 0);
        spawn(fx, COLUMN, &saves(dd, 0, 0x01E0_17E4));
        fx.mem.set16(SPIN, 0x5A);
        spawn(fx, COLUMN, &saves(dd, 0, 0x01E0_1800));
        spawn(fx, PAIR, &saves(dd, 0, 0x01E0_1814));
    }
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_A);
    fx.node_position(h, slot, SPAWN_AT);
    jitter(fx, D, [0x01E0_1834, 0x01E0_1868, 0x01E0_189C]);
    let c = fx.mem.i16(COUNTER);
    if (0xA0..0x137).contains(&c) {
        spawn(fx, RING, &saves(dd, 0, 0x01E0_18FC));
    }
    let mut s0 = dd;
    let mut s1 = 0u64;
    let c = fx.mem.i16(COUNTER);
    if (0xA0..0x137).contains(&c) && c != 0 {
        let n = if c < 0x64 { 2 } else { 4 };
        s1 = n as u64;
        for k in 0..n {
            s0 = k as u64;
            let i = fx.mem.i16(NEXT);
            fx.mem.set16(INDEX, i);
            spawn(fx, QUADS, &saves(s0, s1, 0x01E0_1964));
            let i = fx.mem.i16(NEXT) + 1;
            fx.mem.set16(NEXT, i);
            if fx.mem.i16(NEXT) >= 0x78 {
                fx.mem.set16(NEXT, 0);
            }
        }
        s0 = n as u64;
    }
    let c = fx.mem.i16(COUNTER);
    if (0x86..0xB8).contains(&c) {
        for k in 0..3u32 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODE_C);
            fx.node_position(h, slot, SPAWN_AT);
            jitter(fx, k, [0x01E0_19F4, 0x01E0_1A28, 0x01E0_1A5C]);
            let z = fx.mem.f(D + 0x258);
            let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 50.0) } else { ee::sub(z, 50.0) };
            fx.mem.setf(D + 0x258, z);
            spawn(fx, SPARK, &saves(u64::from(k), s1, 0x01E0_1AE4));
        }
        s0 = 3;
    }
    if fx.mem.i16(COUNTER) == 0x80 {
        spawn(fx, TINT, &saves(s0, s1, 0x01E0_1B2C));
    }
    fx.leave(0x40);
}

/// A step's frame (0x30: s0 = the task, s1 = D, ra) as its callees see it.
fn frame30(fx: &mut Fx) -> u32 {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    sp
}

/// The task's drift: when past its first update z moves by +0x38 against
/// the facing (with the facing: down), then z += +0x34, x += +0x30.
fn drift(fx: &mut Fx, t: u32) {
    let z = fx.mem.f(t + 0x18);
    let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, fx.mem.f(t + 0x38)) } else { ee::add(z, fx.mem.f(t + 0x38)) };
    fx.mem.setf(t + 0x18, z);
    let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x18, z);
    let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
    fx.mem.setf(t + 0x10, x);
}

/// Random start offsets: x = rand % 40 - 20, y = 30, z = rand % 40 - 20,
/// a drift (rand % 40 - 20, rand % 30, rand % 100 + 30) / 10 at +0x30.
fn scatter(fx: &mut Fx, t: u32, ras: [u32; 5]) {
    let v = rmod(fx, t, ras[0], 0x28);
    fx.mem.setf(t + 0x10, ee::from_i32(v - 0x14));
    fx.mem.write_u32(t + 0x14, 0x41F0_0000);
    let v = rmod(fx, t, ras[1], 0x28);
    fx.mem.setf(t + 0x18, ee::from_i32(v - 0x14));
    let v = rmod(fx, t, ras[2], 0x28);
    fx.mem.setf(t + 0x30, ee::from_i32(v - 0x14));
    let v = rmod(fx, t, ras[3], 0x1E);
    fx.mem.setf(t + 0x34, ee::from_i32(v));
    let v = rmod(fx, t, ras[4], 0x64);
    fx.mem.setf(t + 0x38, ee::from_i32(v + 0x1E));
    for o in [0x30u32, 0x34, 0x38] {
        let q = ee::div(fx.mem.f(t + o), 10.0);
        fx.mem.setf(t + o, q);
    }
}

/// 0x1E01B50 (D, task): a turned quad (size rand % 30 + 5 square, grey
/// 0x80, cell rand % 3) scattered about the origin and drifting; both
/// alphas in by 0x2A under 0x80 for 16 updates, then out by 14 (twice);
/// faces the camera, its z-turn +0x5A up by 4.
fn quads(fx: &mut Fx, t: u32) {
    let sp = frame30(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_1B7C, 0x1E);
        fx.mem.set16(t + 8, v + 5);
        let v = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, v);
        let v = fx.rand_from(t, 0x01E0_1BA4);
        fx.mem.set16(t + 0xE, v % 8);
        let v = rmod(fx, t, 0x01E0_1BC0, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        scatter(fx, t, [0x01E0_1C00, 0x01E0_1C34, 0x01E0_1C60, 0x01E0_1C8C, 0x01E0_1CB4]);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, 0x01E0_1D14, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x10 {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x2A));
        }
        let a = fx.mem.read_u8(t + 0x67);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x67, a.wrapping_add(0x2A));
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        }
    }
    let _ = sp;
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1DB8);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i16(t + 0x54) != 0 {
        let a = fx.mem.i16(t + 0x5A) + 4;
        fx.mem.set16(t + 0x5A, a);
        let a = fx.mem.i16(t + 0x5A);
        if a >= 0x168 {
            let (_, m) = fx.div(a, 0x168);
            fx.mem.set16(t + 0x5A, m);
        }
        drift(fx, t);
    }
    fx.leave(0x30);
}

/// 0x1E01E70 (D, task): a white spark (size 10 x 1, cell rand % 6 + 2,
/// +0x34 0.5) moving 5 a update against the facing, turned 0x4000 in y;
/// in by 0x60 under 0xC0 for 15 updates, then out by 9; ends at effect
/// update 184.
fn spark(fx: &mut Fx, t: u32) {
    frame30(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 1);
        let v = rmod(fx, t, 0x01E0_1EA4, 6);
        fx.mem.set16(t + 0x56, v + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.write_u32(t + 0x34, 0x3F00_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xF {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x60));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(t + 0x18, z);
    }
    fx.mem.write_u32(t + 0x24, 0x4000);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i16(COUNTER) >= 0xB8 {
        fx.mem.set16(t + 0x58, 0);
    }
    fx.leave(0x30);
}

/// 0x1E01FC0 (D, task): a ring (radius 130 growing by 2, white 0xC0, cell
/// rand % 7, angle rand % 360, orientation rand % 8, the spawn's index in
/// +0xC, +0x50 rand % 10 + 5 stepping by rand % 15 + 9) scattered about
/// the origin and drifting; alpha 0x80 for rand % 5 + 7 updates, then out
/// by 7.
fn ring(fx: &mut Fx, t: u32) {
    frame30(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4302_0000);
        let v = rmod(fx, t, 0x01E0_1FF0, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        let v = rmod(fx, t, 0x01E0_201C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_2038);
        fx.mem.set16(t + 0xA, v % 8);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = rmod(fx, t, 0x01E0_205C, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        scatter(fx, t, [0x01E0_207C, 0x01E0_20B0, 0x01E0_20DC, 0x01E0_2108, 0x01E0_2130]);
        let v = rmod(fx, t, 0x01E0_2188, 0xF);
        fx.mem.set16(t + 0xE, v + 9);
        let v = rmod(fx, t, 0x01E0_21A8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_21C4, 5);
        fx.mem.set16(t + 6, v + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 9 {
            fx.mem.write_u8(t + 0x63, a - 7);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = ee::add(fx.mem.f(t + 0x48), 2.0);
    fx.mem.setf(t + 0x48, r);
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
        drift(fx, t);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// The 0x20 frame of the leaf-like steps (s0 = D, ra).
fn frame20(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
}

/// 0x1E022D0 (D, task): b539's light column (size 6, width 1.5, height 5
/// growing by 4, grey / dark red) turning by 6 a update; in by 0x40 under
/// 0x80 for 4 updates, then out by 9.
fn column(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0x10);
        fx.mem.write_u32(t + 0x30, 0x3FC0_0000);
        fx.mem.write_u32(t + 0x34, 0x40A0_0000);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 4 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let h = ee::add(fx.mem.f(t + 0x34), 4.0);
        fx.mem.setf(t + 0x34, h);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
    fx.leave(0x20);
}

/// 0x1E023E0 (D, task): b747's quad pair step (white, size 2, +0x48 100);
/// in by 0x60 under 0xC0 for 4 updates, then out by 9.
fn pair(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 8, 2);
        fx.mem.set16(t + 0xA, 2);
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 4 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x60));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E024C0 (D, task; frame 0x90: ra, the entry's s1 = 0, s0 = D): b747's
/// window sprite (0x6FF8 .. 0x8FF8) sampling the frame (0x207EF0, filter
/// 1) at ST inset by +0x48 (1) / 512, colour (0x90, 0x78, 0x78, alpha
/// +0x63), ALPHA 0x44, key 0; in by 5 below 0x50 until update 325, then
/// out by 7.
fn tint(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x58, 2);
        l2::rgb(fx, t + 0x60, 0x90, 0x78, 0x78);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, c);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for o in [0x50u32, 0x54] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(s, 512.0));
    }
    for o in [0x60u32, 0x64] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(ee::sub(512.0, s), 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(COUNTER) < 0x145 {
        if a < 0x50 {
            fx.mem.write_u8(t + 0x63, a + 5);
        }
    } else if a >= 9 {
        fx.mem.write_u8(t + 0x63, a - 7);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x90);
}
