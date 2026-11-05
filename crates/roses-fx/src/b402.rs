//! BMODULE b402 (card 249), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02CE0, data area 0x1ED0000) [F]: b491's task pool (150 x 0x70 at D+0x2A0, alive +0x58, step +0x44
//! through `jalr`, 0x1E02CD0 only returns and is skipped; spawned by
//! 0x1E034C0 = `spawn::task70` with the colour D+0x44B4 and the slot
//! D+0x4514), no unit flash, drawn with the second library (`ring_quads`,
//! `arc_strips_flat`) and the record's wave strips 0x1E03600 (b161's
//! 0x1E03500 with the task's scale +0x38 and waves of 12 / 20). The
//! emitter 0x1E01BD0 counts D+0x451C: to 0x9A the attacker's node 0x12
//! followed to D+0x4470, slid over 0x9B..0xB9 a 32nd of the way to the
//! target's node 1 a update (y / z), copied to the spawn point on 0xBA;
//! update 1 the object op 0x1F8380(3, 0, ..) (0x1E02C50, 0x91 updates);
//! 0x48 two white rings (0x1E01F20) at nodes 0x29 / 0x34; 0x4A..0xA1 an
//! opening wave ring (0x1E02860) at node 0x12; 0x8D..0xFA on the odd
//! updates a closing wave ring (0x1E025B0) at the target's node 1, the
//! rings' start angle D+0x450E up by 3 a update; 0xA5..0x106 every fourth
//! update at the target's node (x / y +-10) a white arc (0x1E02B70) and a
//! white ring of quads (0x1E029C0). The steps 0x1E020D0 / 0x1E02260 /
//! 0x1E02460 / 0x1E02710 are drawn but never spawned (not ported).
//! Returns 1 on update 0x83, 4 on 0x118, 2 on 0x109, 6 on 0x8E..0x103, 8
//! once the 290-update timer D+0x44B0 has run out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::spawn::{self, Pool70};
use crate::blib::{billboard, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const RING: u32 = 0x01E0_1F20;
const WAVE_C: u32 = 0x01E0_25B0;
const WAVE_O: u32 = 0x01E0_2860;
const RINGQ: u32 = 0x01E0_29C0;
const ARC: u32 = 0x01E0_2B70;
const OBJECT: u32 = 0x01E0_2C50;
const IDLE: u32 = 0x01E0_2CD0;
const UNSPAWNED: [u32; 4] = [0x01E0_20D0, 0x01E0_2260, 0x01E0_2460, 0x01E0_2710];
/// The return address of the entry's `jalr` into a step (0x1E02FA0 + 8).
const TASK_RA: u64 = 0x01E0_2FA8;
/// The scratchpad, where the library projects.
const SPR: u32 = 0x7000_0000;
const POS: u32 = D + 0x250;
const POINT: u32 = D + 0x4470;
const TEX: u32 = D + 0x4490;
const TIMER: u32 = D + 0x44B0;
const NODES: u32 = D + 0x44E4;
const TNODE: u32 = D + 0x44F4;
const WAVE: u32 = D + 0x450C;
const SPIN: u32 = D + 0x450E;
const SLOT: u32 = D + 0x4514;
const FACING: u32 = D + 0x4518;
const COUNTER: u32 = D + 0x451C;
const BYTES: u32 = D + 0x451E;
const BYTES_B: u32 = D + 0x461E;
const SINES: u32 = D + 0x4720;
const POOL: Pool70 = Pool70 { base: 0x2A0, n: 150, colour: 0x44B4, pos: 0x250, slot: 0x4514 };

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02CE0 (s0 = D, s1 = 0 then the live count, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    fx.mem.write_u32(D + 0x44BC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4890);
    l2::sine_table(fx, D + 0x4720);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x451A, param);
    fx.mem.set32(TIMER, 0x122);
    for (k, node) in [0x12, 0x29, 0x34, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TNODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_2E74);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_2E9C, 0x1E);
        fx.mem.set8(BYTES + k, r - 0xF);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_2EE4, 0x1E);
        fx.mem.set8(BYTES_B + k, r - 0xF);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(BYTES_B + k, 0);
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
        let frame = match f {
            0 | IDLE => continue,
            OBJECT => 0x30,
            RING | WAVE_C | WAVE_O | RINGQ | ARC => 0x20,
            _ => panic!("b402: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        // The steps' saves: ra, the entry's s0 = D.
        fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        match f {
            RING => ring(fx, t),
            WAVE_C => wave_c(fx, t),
            WAVE_O => wave_o(fx, t),
            RINGQ => ringq(fx, t),
            ARC => arc(fx, t),
            _ => object(fx, t, s),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let cell = |fx: &mut Fx, r: u32, id: i32| {
            let sub = fx.mem.i16(t + 0x56);
            l2::rect(fx, r, id, sub);
            for k in 0..4 {
                let x = fx.mem.read_u32(r + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, x);
            }
        };
        let tex = |fx: &mut Fx, at: u32| {
            let w = fx.mem.read_u32(at);
            fx.mem.write_u32(D + 0x88, w);
        };
        let f = fx.mem.read_u32(t + 0x44);
        if UNSPAWNED.contains(&f) {
            panic!("b402: unported draw case {f:#x}");
        }
        if f == RING {
            tex(fx, TEX + 8);
            cell(fx, sp + 0x60, 0x14);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x10, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        for (g, r) in [(WAVE_C, 0x90u32), (WAVE_O, 0xA0)] {
            if fx.mem.read_u32(t + 0x44) == g {
                cell(fx, sp + r, 0x97);
                tex(fx, TEX);
                fx.mem.set16(D + 0x94, 0);
                fx.mem.set16(D + 0x96, 0);
                fx.mem.set16(D + 0x98, 0x1F);
                fx.mem.set16(D + 0x9A, 0x7F);
                fx.mem.set16(D + 0xB8, 0x48);
                wave_strips(fx, D, t, 0x21, t + 0x10);
                fx.mem.set16(D + 0xB8, 0);
            }
        }
        if fx.mem.read_u32(t + 0x44) == RINGQ {
            tex(fx, TEX + 4);
            cell(fx, sp + 0xB0, 0x14);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x83 => 1,
        0x118 => 4,
        0x109 => 2,
        0x8E..=0x103 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E01BD0(D, 0) (frame 0x40: ra, the entry's s0 = D): the update
/// counter, the followed point and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x10, 0x01E0_2F68), (0, u64::from(D))]);
    // The spawns' saves: s0 = D, the entry's s1 = 0, s2 = the parameter,
    // s3 / s4 the runner's (0).
    let p = param as i64 as u64;
    let go = |fx: &mut Fx, f: u32, jal: u32| {
        let saves = [(0x50, u64::from(jal + 8)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, 0), (0, u64::from(D))];
        spawn::task70(fx, D, POOL, f, 0, &saves);
    };
    let at = |fx: &mut Fx, target: bool, slot: u32, out: u32, jal: u32| {
        let h = if target { fx.target_handle() } else { fx.self_handle() };
        let s = fx.mem.i16(slot);
        node_position_saving(fx, h, s, out, u64::from(D), Some(0), jal + 8);
    };
    let spin = |fx: &mut Fx| {
        let a = fx.mem.i16(SPIN) + 3;
        fx.mem.set16(SPIN, a);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if c < 0x9B {
        at(fx, false, NODES, POINT, 0x01E0_1C0C);
    } else {
        if c == 0x9B {
            at(fx, true, TNODE, sp + 0x20, 0x01E0_1C38);
            let dz = ee::div(ee::sub(fx.mem.f(sp + 0x28), fx.mem.f(POINT + 8)), 32.0);
            fx.mem.setf(D + 0x4488, dz);
            let dy = ee::div(ee::sub(fx.mem.f(sp + 0x24), fx.mem.f(POINT + 4)), 32.0);
            fx.mem.setf(D + 0x4484, dy);
        }
        if fx.mem.i16(COUNTER) < 0xBA {
            let z = ee::add(fx.mem.f(POINT + 8), fx.mem.f(D + 0x4488));
            fx.mem.setf(POINT + 8, z);
            let y = ee::add(fx.mem.f(POINT + 4), fx.mem.f(D + 0x4484));
            fx.mem.setf(POINT + 4, y);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if c == 1 {
        go(fx, OBJECT, 0x01E0_1CBC);
    }
    if fx.mem.i16(COUNTER) == 0xBA {
        for k in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        fx.mem.set16(SLOT, 0);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x4A..0xA2).contains(&c) {
        at(fx, false, NODES, POS, 0x01E0_1D24);
        go(fx, WAVE_O, 0x01E0_1D38);
        spin(fx);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x8D..0xFB).contains(&c) {
        at(fx, true, TNODE, POS, 0x01E0_1D78);
        if fx.mem.i16(COUNTER) % 2 == 1 {
            go(fx, WAVE_C, 0x01E0_1DB0);
        }
        spin(fx);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xA5..0x107).contains(&c) && c % 4 == 1 {
        at(fx, true, TNODE, POS, 0x01E0_1E10);
        for (o, ra) in [(0u32, 0x01E0_1E20u32), (4, 0x01E0_1E54)] {
            let r = rmod(fx, D, ra, 0x14);
            let v = ee::add(fx.mem.f(POS + o), ee::from_i32(r - 0xA));
            fx.mem.setf(POS + o, v);
        }
        go(fx, ARC, 0x01E0_1E88);
        go(fx, RINGQ, 0x01E0_1E9C);
    }
    if fx.mem.i16(COUNTER) == 0x48 {
        at(fx, false, NODES + 2, POS, 0x01E0_1EC4);
        go(fx, RING, 0x01E0_1ED8);
        at(fx, false, NODES + 4, POS, 0x01E0_1EF0);
        go(fx, RING, 0x01E0_1F04);
    }
    fx.leave(0x40);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// Byte +0x63 down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if a >= floor {
        fx.mem.write_u8(t + 0x63, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01F20 (D, task) (b517's 0x1E01960): a white ring of quads (size
/// 100, cell frame 3, radius 10..24 growing by 10 from age 1, a random
/// angle, row 1 + rand % 8, life 30..44 at alpha 0xC0, then out by 11);
/// white / grey by age when +0x52 is 1.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        fx.mem.set16(t + 0x56, 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let r = rmod(fx, t, 0x01E0_1F68, 0xF);
        fx.mem.set16(t + 0x50, r + 0xA);
        let r = rmod(fx, t, 0x01E0_1F88, 0x168);
        fx.mem.set16(t + 0x5A, r);
        fx.mem.set16(t + 0xE, 0xA);
        let r = fx.rand_from(t, 0x01E0_1FAC);
        fx.mem.set16(t + 8, r % 8 + 1);
        let r = rmod(fx, t, 0x01E0_1FCC, 0xF);
        fx.mem.set16(t + 6, r + 0x1E);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade_out(fx, t, 0xD, 0xB);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
    if fx.mem.i16(t + 0x52) == 1 {
        if fx.mem.i16(t + 0x54) % 2 == 1 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        } else {
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        }
    }
}

/// 0x1E025B0 (D, task): a grey closing wave ring (wave index 1, steps 5)
/// from radius 1300 by 65..94.9 a update (the outer 150 outside), scale
/// 0.05, turned by the camera's angles (the first + 0x4000); alpha 0x80,
/// ended below radius 170.
fn wave_c(fx: &mut Fx, t: u32) {
    use ee::{add, div, from_i32, sub};
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x44A2_8000);
        fx.mem.setf(t + 0x4C, sub(1300.0, 58.0));
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x40);
        fx.mem.set16(t + 0x50, 1);
        fx.mem.set16(t + 0x5A, 5);
        fx.mem.set16(t + 0x5C, 5);
        let v = rmod(fx, t, 0x01E0_262C, 0x12C);
        fx.mem.setf(t + 0x34, from_i32(v + 0x28A));
        let v = div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.write_u32(t + 0x38, 0x3D4C_CCCD);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 2 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else if fx.mem.f(t + 0x48) < 170.0 {
        fx.mem.set16(t + 0x58, 0);
    }
    let r = sub(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, add(150.0, r));
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_26E4);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E02860 (D, task): a grey opening wave ring (wave index 0, steps 5)
/// from radius 60 by 10..34.9 a update (the outer 30 inside), scale 0.05,
/// turned by the camera's angles (the first + 0x4000); alpha 0x80 for 2
/// updates, then out by 14.
fn wave_o(fx: &mut Fx, t: u32) {
    use ee::{add, div, from_i32, sub};
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4270_0000);
        fx.mem.setf(t + 0x4C, sub(60.0, 58.0));
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x40);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 5);
        fx.mem.set16(t + 0x5C, 5);
        let v = rmod(fx, t, 0x01E0_28D4, 0xFA);
        fx.mem.setf(t + 0x34, from_i32(v + 0x64));
        let v = div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.write_u32(t + 0x38, 0x3D4C_CCCD);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 2 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x10, 0xE);
    }
    let r = add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, sub(r, 30.0));
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2988);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E029C0 (D, task) (b279's 0x1E02160 in white): a ring of quads (size
/// 8..17, cell 0..3, radius 5..34 growing by 15..39, a random angle, row
/// 0..34, life 4..7); in by 0x40 under 0xC0, out by 16.
fn ringq(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_29E4, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 8));
        let v = fx.rand_from(t, 0x01E0_2A10);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = rmod(fx, t, 0x01E0_2A44, 0x1E);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, 0x01E0_2A64, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2A80, 0x19);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = rmod(fx, t, 0x01E0_2AA0, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_2ABC);
        fx.mem.set16(t + 6, v % 4 + 4);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else {
        fade_out(fx, t, 0x12, 0x10);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02B70 (D, task) (b747's 0x1E02420): an arc (`arc_strips_flat`;
/// white / pale green, radius 110, the outer +0x4C 0, angle 15 spanning
/// 15) turned by the camera's angles (the first + 0x4000); alpha 0x80 for
/// 3 updates, then out by 11.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42DC_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0xFF, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0xD, 0xB);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2C1C);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E02C50 (D, task) (frame 0x30 at `sp`): the object op 0x1F8380(3,
/// 0, the record's words 0x1E03B00 / 0x1E03B08 at sp + 0x28 / + 0x2C, 0)
/// each update; the task ends from age 0x91.
fn object(fx: &mut Fx, t: u32, sp: u32) {
    for (to, from) in [(0x28u32, 0x01E0_3B00u32), (0x2C, 0x01E0_3B08)] {
        let w = fx.mem.read_u32(from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.boundary(0x001F_8380, "object op 0x1F8380", [3, 0, sp + 0x28, sp + 0x2C]);
    age(fx, t);
    if fx.mem.i16(t + 0x54) >= 0x91 {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E03600(D, task, mode, pos): b161's wave strips (0x1E03500) with the
/// task's scale +0x38 on all three axes, the start angle D+0x450E alone
/// and waves of 12 (index 0) / 20 (index 1) times sin(D+0x450C) on the
/// sines D+0x4720 [F fdiff: the only differences].
fn wave_strips(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    use ee::{add, mul};
    let sp = fx.enter(0x220);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    let span = fx.mem.i16(t + 0x5C);
    let d = sp + 0x170;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 0);
    l2::uv_strip3(fx, d, d0 + 0x94, 0);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for (local, c, to) in [(0x200u32, 0x60u32, 0x10u32), (0x208, 0x64, 0x40), (0x210, 0x60, 0x28), (0x218, 0x64, 0x58)] {
        crate::c061::rgbaq_at(fx, sp + local, t + c, d + to);
    }
    for k in 0..3 {
        let s = fx.mem.read_u32(t + 0x38);
        fx.mem.write_u32(sp + 0x110 + 4 * k, s);
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * k);
        fx.mem.write_u32(sp + 0x100 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x20, sp + 0x100);
    for o in [0xC4u32, 0xD4, 0xE4, 0xF4] {
        fx.mem.set32(sp + o, 0);
    }
    let (inner, outer) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    fx.mem.set16(WAVE, 0);
    let depth = mode & 0xF;
    let mut a = fx.mem.i16(SPIN);
    // f20: the wave, kept across strips (set on every edge for the
    // indices 0 / 1 the record spawns).
    let mut f20 = 0.0f32;
    let wave = |fx: &mut Fx, f20: &mut f32| {
        let by = match fx.mem.i16(t + 0x50) {
            0 => Some(12.0),
            1 => Some(20.0),
            _ => None,
        };
        if let Some(by) = by {
            let w = fx.mem.i16(WAVE);
            let s = l2::sin_deg(fx, SINES, w, 0);
            *f20 = mul(by, s);
        }
    };
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let s4 = a;
        let s = (fx.mem.i16(t + 0x5A) + s4) as i16 as i32;
        wave(fx, &mut f20);
        match fx.mem.i16(t + 0x50) {
            0 => {
                let w = fx.mem.i16(WAVE) + 0x14;
                fx.mem.set16(WAVE, w);
            }
            1 => {
                let w = fx.mem.i16(WAVE) + 0x28;
                fx.mem.set16(WAVE, w);
            }
            _ => {}
        }
        for (r, x, z) in [(inner, 0xC0u32, 0xC8u32), (outer, 0xE0, 0xE8)] {
            let c = l2::sin_deg(fx, SINES, s, 1);
            fx.mem.setf(sp + x, add(f20, mul(r, c)));
            let sn = l2::sin_deg(fx, SINES, s, 0);
            fx.mem.setf(sp + z, add(f20, mul(r, sn)));
        }
        wave(fx, &mut f20);
        let e = (s4 + (fx.mem.i16(t + 0x5A) + span)) as i16 as i32;
        for (r, x, z) in [(inner, 0xD0u32, 0xD8u32), (outer, 0xF0, 0xF8)] {
            let c = l2::sin_deg(fx, SINES, e, 1);
            fx.mem.setf(sp + x, add(f20, mul(r, c)));
            let sn = l2::sin_deg(fx, SINES, e, 0);
            fx.mem.setf(sp + z, add(f20, mul(r, sn)));
        }
        if depth <= 1 && l2::project(fx, sp + 0xC0, SPR, sp + 0x120, d, sp + 0x160) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            if depth == 0 {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                fx.submit(d, ot, key);
            } else {
                l2::far_z_desc(fx, d, 1);
                fx.submit(d, ot, 0);
            }
        }
        k += 1;
        a = (a + fx.mem.i16(t + 0x5A)) as i16 as i32;
    }
    fx.leave(0x220);
}
