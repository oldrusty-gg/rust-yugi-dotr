//! BMODULE b749 (card 646), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E031C0, data area 0x1ED0000) [F]: b020's 250-task pool and layout (D+0x70xx; spawned by 0x1E038D0,
//! b020's spawn without the angle) on the second library (`uv_rect`,
//! `ring_quads`, `quads4::sparks`, `billboard::quad`). The emitter
//! 0x1E02000 follows the attacker's node 1 (jittered +-20, y 30, 10
//! against the facing); on 0x62..0x85 when the update % 3 is 1 a white
//! spark fan (0x1E028A0, radius 200 growing 95 a update) and a white mote
//! ring (0x1E02BF0, b747's mote, `ring_quads` of 10 from the bytes
//! D+0x70B4); on updates 0x62 and 0x76 72 grey quads (0x1E029D0) flying
//! out along angles 0, 5, .. 355 at 5 a update, growing by 15 to 400; on
//! 0x60 a pale green window tint (0x1E02DC0) whose inset swings by 0.3.
//! The steps 0x1E02310 (an STQ ring), 0x1E024F0 (b082's `ring_v`) and
//! 0x1E026A0 (b615's quads) are never spawned: their draw cases are not
//! ported. Returns 1 on update 100, 4 on 195, 2 on 175, 6 on 111..169, 8
//! once the 200-update timer D+0x7050 has run out.
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
const FAN: u32 = 0x01E0_28A0;
const FLY: u32 = 0x01E0_29D0;
const MOTE: u32 = 0x01E0_2BF0;
const WINDOW: u32 = 0x01E0_2DC0;
/// The return address of the entry's `jalr` into a step (0x1E03438 + 8).
const TASK_RA: u64 = 0x01E0_3440;
/// The return address of the entry's call into the emitter 0x1E02000.
const EMIT_RA: u64 = 0x01E0_3410;
const SPAWN_AT: u32 = D + 0x250;
const TEX: u32 = D + 0x7030;
const TIMER: u32 = D + 0x7050;
const COLOUR: u32 = D + 0x7054;
const NODE_A: u32 = D + 0x707C;
const NODE_B: u32 = D + 0x707E;
const NODE_C: u32 = D + 0x7080;
const TNODE: u32 = D + 0x708A;
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

/// The entry 0x1E031C0 (a0 = D, a1 = param; s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(D + 0x705C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x7420);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x70B0, param);
    fx.mem.set32(TIMER, 0xC8);
    for (at, node, clear) in [(NODE_A, 1, 1), (NODE_B, 6, 0), (NODE_C, 0xD, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TNODE, r);
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x288 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_331C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3344, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(BYTES + k, 0);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_33C4, 0x28);
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
            FAN => fan(fx, t),
            FLY => fly(fx, t),
            MOTE => mote(fx, t),
            WINDOW => window(fx, t),
            _ => panic!("b749: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == MOTE {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x90, 0x14, sub);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == FAN {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0x94, 0);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x1F);
            fx.mem.set16(D + 0x9A, 0x7F);
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            fx.mem.set16(D + 0xB8, 0x18);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == FLY {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xA0, 0x14, sub);
            fx.mem.set16(D + 0xBE, 5);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, v);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x64 => 1,
        0xC3 => 4,
        0xAF => 2,
        0x6F..=0xA9 => 6,
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

/// 0x1E038D0(D, step, 0): the first free of the 250 tasks cleared and set
/// off at D+0x250 with the colour D+0x7054 twice and the index D+0x70AA
/// (+0x50), +0x40 0. `saves`: its prologue's (0x60: s0 +0 .. s4 +0x40, ra
/// +0x50).
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

/// 0x1E02000(D, 0): the update counter, the followed node and the spawns
/// (frame 0x50: ra, the entry's s2 = the parameter, s1 = 0, s0 = D; the
/// attacker's node 6 into sp + 0x40, unused).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, EMIT_RA), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let dd = u64::from(D);
    // The spawn's saves: s0 = D, s1 / s2 the emitter's, s3 / s4 the
    // runner's (0) [H].
    let saves = |s1: u64, s2: u64, ra: u32| [(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, s2), (0x10, s1), (0, dd)];
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_A);
    node_position_saving(fx, h, slot, SPAWN_AT, dd, Some(0), 0x01E0_2038);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_B);
    node_position_saving(fx, h, slot, sp + 0x40, dd, Some(0), 0x01E0_2050);
    let r = rmod(fx, D, 0x01E0_2058, 0x28);
    let x = ee::add(fx.mem.f(D + 0x250), ee::from_i32(r - 0x14));
    fx.mem.setf(D + 0x250, x);
    fx.mem.write_u32(D + 0x254, 0x41F0_0000);
    let r = rmod(fx, D, 0x01E0_2094, 0x28);
    let z = ee::add(fx.mem.f(D + 0x258), ee::from_i32(r - 0x14));
    fx.mem.setf(D + 0x258, z);
    let z = fx.mem.f(D + 0x258);
    let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 10.0) } else { ee::add(z, 10.0) };
    fx.mem.setf(D + 0x258, z);
    let c = fx.mem.i16(COUNTER);
    if (0x64..0x79).contains(&c) {
        let i = fx.mem.i16(NEXT);
        fx.mem.set16(INDEX, i);
        let i = fx.mem.i16(NEXT) + 1;
        fx.mem.set16(NEXT, i);
        if fx.mem.i16(NEXT) >= 0x78 {
            fx.mem.set16(NEXT, 0);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x62..0x86).contains(&c) && fx.div(c, 3).1 == 1 {
        spawn(fx, FAN, &saves(0, p, 0x01E0_21F0));
        spawn(fx, MOTE, &saves(0, p, 0x01E0_2204));
    }
    for (when, ra) in [(0x62, 0x01E0_2238u32), (0x76, 0x01E0_229C)] {
        if fx.mem.i16(COUNTER) == when {
            for k in 0..0x48u64 {
                fx.mem.set16(INDEX, 5 * k as i32);
                spawn(fx, FLY, &saves(5 * k, k, ra));
            }
        }
    }
    if fx.mem.i16(COUNTER) == 0x60 {
        spawn(fx, WINDOW, &saves(0, p, 0x01E0_22F4));
    }
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// A 0x20-frame step's prologue (ra, the entry's s0 = D).
fn frame20(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
}

/// 0x1E028A0 (D, task): b144's spark fan with its own speed: white /
/// grey 0x30, radii 200 / 142, the outer growing by 95 a update, the inner
/// following 400 behind; turning by 0x100; alpha 0x80 for 13 updates,
/// then out by 14.
fn fan(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.write_u32(t + 0x34, 0x446D_8000);
        let s = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, s);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xD {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let v = fx.mem.read_u8(t + 0x63);
        if v >= 0x10 {
            fx.mem.write_u8(t + 0x63, v - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, 400.0));
    let a = fx.mem.read_u32(t + 0x24).wrapping_add(0x100);
    fx.mem.write_u32(t + 0x24, a);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E029D0 (D, task; frame 0x30: ra, the entry's s1 = 0, s0 = D): a grey
/// quad (size 150 growing by 15 to 400, cell rand % 8 stepping on odd ages
/// and wrapping 8 -> 3) flying out along its angle +0x50 at 5 a update
/// (x by the cosine, z by the sine); both alphas in by 0x40 under 0xC0 for
/// 40 updates, then +0x63 out by 4 (twice); faces the camera.
fn fly(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x96);
        fx.mem.set16(t + 0xA, 0x96);
        let v = fx.rand_from(t, 0x01E0_2A04);
        fx.mem.set16(t + 0x56, v % 8);
        let a = fx.mem.i16(t + 0x50);
        let c = l2::sin_deg(fx, SINES, a, 1);
        fx.mem.setf(t + 0x30, ee::mul(5.0, c));
        let a = fx.mem.i16(t + 0x50);
        let s = l2::sin_deg(fx, SINES, a, 0);
        fx.mem.setf(t + 0x38, ee::mul(5.0, s));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x5C, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x28 {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0xC0 {
                fx.mem.write_u8(t + o, a + 0x40);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 6 {
            fx.mem.write_u8(t + 0x63, a - 4);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 6 {
            fx.mem.write_u8(t + 0x63, a - 4);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 8) < 0x190 {
        let s = fx.mem.i16(t + 8) + 0xF;
        fx.mem.set16(t + 8, s);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
        fx.mem.setf(t + 0x10, x);
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x38));
        fx.mem.setf(t + 0x18, z);
    }
    let age_now = fx.mem.i16(t + 0x54);
    if age_now < 0x28 {
        if age_now % 2 == 1 {
            let c = fx.mem.i16(t + 0x56) + 1;
            fx.mem.set16(t + 0x56, c);
        }
        if fx.mem.i16(t + 0x56) >= 8 {
            fx.mem.set16(t + 0x56, 3);
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2BC0);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02BF0 (D, task): b747's mote ring (`ring_quads` of 10 from the
/// bytes D+0x70B4) with its own numbers: white, size 30..59, cell 4,
/// random angles, radius 5..34 growing by 100..199, row 1..8, life 7..16;
/// alpha 0xC0 during the life, then out by 11.
fn mote(fx: &mut Fx, t: u32) {
    frame20(fx);
    let f = MOTE;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 0x1E);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x1E));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        for (o, site) in [(0x20u32, f + 0x68), (0x24, f + 0x84), (0x28, f + 0xA0)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = rmod(fx, t, f + 0xBC, 0x1E);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, f + 0xDC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xF8, 0x64);
        fx.mem.set16(t + 0xE, v + 0x64);
        let v = rmod(fx, t, f + 0x118, 8);
        fx.mem.set16(t + 8, v + 1);
        let v = rmod(fx, t, f + 0x138, 0xA);
        fx.mem.set16(t + 6, v + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xD {
            fx.mem.write_u8(t + 0x63, a - 0xB);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02DC0 (D, task; frame 0x80: ra, the entry's s0 = D): a window
/// sprite (0x6FF8 .. 0x8FF8) sampling the frame (0x207EF0, filter 1) at ST
/// inset by +0x48 / 512, colour (0x5F, 0x80, 0x5F, alpha +0x63), ALPHA
/// 0x44, key 0; the inset swings by 0.3 a update (up while below 100 on
/// ages 0..9, 20..29, 40..49, 60..69, down while above 0 on 10..19,
/// 30..39, 50..59); alpha in by 10 below 0x68 for 70 updates, then out by
/// 4 with the inset sinking to 0.
fn window(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x80);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in [0x5Fu8, 0x80, 0x5F].into_iter().enumerate() {
        fx.mem.write_u8(sp + 0x38 + k as u32, c);
    }
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(sp + 0x3B, a);
    fx.mem.write_u32(sp + 0x3C, 0x3F80_0000);
    for (at, v) in [(sp + 0x48, 0x6FF8u16), (sp + 0x58, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for o in [0x40u32, 0x44] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(s, 512.0));
    }
    for o in [0x50u32, 0x54] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(ee::sub(512.0, s), 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let step = f32::from_bits(0x3E99_999A);
    let age_now = fx.mem.i16(t + 0x54);
    if age_now < 0x46 {
        let s = fx.mem.f(t + 0x48);
        if (age_now / 10) % 2 == 0 {
            if s < 100.0 {
                fx.mem.setf(t + 0x48, ee::add(s, step));
            }
        } else if !(s <= 0.0) {
            fx.mem.setf(t + 0x48, ee::sub(s, step));
        }
    }
    if fx.mem.i16(t + 0x54) < 0x46 {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x68 {
            fx.mem.write_u8(t + 0x63, a + 0xA);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 6 {
            fx.mem.write_u8(t + 0x63, a - 4);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let s = fx.mem.f(t + 0x48);
        if !(s <= 0.0) {
            fx.mem.setf(t + 0x48, ee::sub(s, step));
        } else {
            fx.mem.write_u32(t + 0x48, 0);
        }
    }
    age(fx, t);
    fx.leave(0x80);
}
