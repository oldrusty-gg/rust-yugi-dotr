//! BMODULE b183 (card 41), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E021D0, data area 0x1ED0000) [F] (helper A10, 31
//! Oct 2026): a pool of 150 tasks of 0x70 bytes at D+0x2A0 (alive +0x58,
//! step +0x44 called through `jalr`; spawned by 0x1E02880, 8b19c0d03b97,
//! b491's too) drawn as flat rings facing the camera (9c13e08cfd2f at
//! 0x1E00920, `quads4::ring_facing_flat`) and one quad (40d081dab923 at
//! 0x1E00E70, `billboard::quad_b`), UVs by uv_rect 0x1E00080, plus a unit
//! flash (0x1E029C0, never reached). Updates 10..20: two white rings a
//! update at the attacker's node 0xF; update 15 a glowing quad there (5 up,
//! 3 toward the facing) drifting to the target from its update 61 with
//! grey rings in its wake, bursting on its update 140 into 30 grey and on
//! 140 / 141 four white rings; the stage dimmed (0x1E01E50) until the
//! timer is under 21. Returns 1 on update 145, 4 on 180, 2 on 155, 8 once
//! the 195-update timer +0x44A0 has run out.
//!
//! The flash step 0x1E01F30 is run only by the unit flash 0x1E029C0, and
//! nothing spawns a flash task [F: the spawn `jal`s name 0x1E012F0 /
//! 0x1E01480 / 0x1E01E50 / 0x1E01990 / 0x1E01B50 / 0x1E01CE0]: neither is
//! ported.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
const RING: u32 = 0x01E0_12F0;
const ORB: u32 = 0x01E0_1480;
const BURST: u32 = 0x01E0_1990;
const BURST_W: u32 = 0x01E0_1B50;
const WAKE: u32 = 0x01E0_1CE0;
const LIGHT: u32 = 0x01E0_1E50;
const FLASH: u32 = 0x01E0_1F30;
/// The return address of the entry's `jalr` into a step (0x1E02470 + 8).
const TASK_RA: u64 = 0x01E0_2478;
const POS: u32 = D + 0x250;
const TEX: u32 = D + 0x4480;
const TIMER: u32 = D + 0x44A0;
const COLOUR: u32 = D + 0x44A4;
const NODE: u32 = D + 0x44D4;
const KIND: u32 = D + 0x4502;
const FACING: u32 = D + 0x4506;
const COUNTER: u32 = D + 0x450A;
const BYTES: u32 = D + 0x450C;
const BYTES_B: u32 = D + 0x460C;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E021D0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    fx.mem.write_u32(D + 0x44AC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4880);
    l2::sine_table(fx, D + 0x470C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4508, param);
    fx.mem.set32(TIMER, 0xC3);
    for (k, (target, node)) in [(false, 0xF), (false, 0x1C), (false, 0xD)].into_iter().enumerate() {
        let h = if target { fx.target_handle() } else { fx.self_handle() };
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x44E4, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..4 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_2324);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_234C, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2394, 0x14);
        fx.mem.set8(BYTES_B + k, v - 0xA);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_23E4);
        fx.mem.set8(BYTES_B + k, v % 4);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s1: the flash tasks counted here.
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => {}
            FLASH => flashes += 1,
            // The steps' saves: the entry's s0 = D, s1 = the flashes, s2 =
            // 0, s3 = the slot, s4 = its index.
            RING => small(fx, t, 0),
            BURST => small(fx, t, 1),
            BURST_W => small(fx, t, 2),
            WAKE => small(fx, t, 3),
            ORB => orb(fx, t, flashes, slot, k),
            LIGHT => light(fx, t),
            _ => panic!("b183: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let ring = |fx: &mut Fx, local: u32, tex: u32, rows: i32, table: u32| {
            cell(fx, t, sp + local);
            let w = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, rows);
            fx.mem.set16(D + 0xC0, 0);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, table);
            fx.mem.set16(D + 0xB8, 0);
        };
        if fx.mem.read_u32(t + 0x44) == RING {
            ring(fx, 0x70, TEX, 8, BYTES_B);
        }
        if fx.mem.read_u32(t + 0x44) == ORB {
            cell(fx, t, sp + 0x80);
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xBE, 0);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x60 + 4 * k, v);
            }
            billboard::quad_b(fx, uv::uv_rect, D, t, 0x21, sp + 0x60);
        }
        if fx.mem.read_u32(t + 0x44) == BURST {
            let tex = if fx.mem.i16(t + 0xC) == 0 { TEX + 8 } else { TEX + 0xC };
            ring(fx, 0x90, tex, 5, BYTES);
        }
        if fx.mem.read_u32(t + 0x44) == BURST_W {
            ring(fx, 0xA0, TEX, 8, BYTES_B);
        }
        if fx.mem.read_u32(t + 0x44) == WAKE {
            ring(fx, 0xB0, TEX + 8, 0xA, BYTES);
        }
    }
    // The unit flash 0x1E029C0 (its flash tasks 0x1E01F30) runs only when
    // a flash task is alive: never [F].
    debug_assert_eq!(flashes, 0);
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x91 => 1,
        0xB4 => 4,
        0x9B => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell 0x14 frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E02880(D, step, 0): the first free task cleared and set off at
/// D+0x250 (i16 +0, float +0x10) with the colour D+0x44A4 (twice), kind
/// D+0x4502 (+0x50); `saves` the caller's s0..s4, `ra` the return address.
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        let t = D + 0x2A0 + 0x70 * k;
        fx.bzero(t, 0x70);
        fx.mem.set16(t + 0x58, 1);
        for o in [0x60u32, 0x64] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        for k in 0..3 {
            let w = fx.mem.read_u32(POS + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
        let s = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x50, s);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

fn node_pos(fx: &mut Fx) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE);
    fx.node_position(h, slot, POS);
}

/// 0x1E01150(D, 0): the update counter and the emitters (frame 0x20: ra,
/// the entry's s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_2438), (0, u64::from(D))]);
    // The spawn's saves: s0 = D, the entry's s1 = s2 = 0, s3 = the
    // parameter, s4 the runner's (0 [H]).
    let saves = [u64::from(D), 0, 0, param as i64 as u64, 0];
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if (0xA..0x15).contains(&c) {
        for (rra, sra) in [(0x01E0_11A8u32, 0x01E0_11E0u32), (0x01E0_1200, 0x01E0_1238)] {
            node_pos(fx);
            let v = rmod(fx, D, rra, 7);
            let y = ee::add(fx.mem.f(POS + 4), ee::from_i32(v));
            fx.mem.setf(POS + 4, y);
            spawn(fx, RING, saves, sra);
        }
    }
    if fx.mem.i16(COUNTER) == 0xF {
        node_pos(fx);
        let y = ee::add(fx.mem.f(POS + 4), 5.0);
        fx.mem.setf(POS + 4, y);
        let z = fx.mem.f(POS + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 3.0) } else { ee::add(z, 3.0) };
        fx.mem.setf(POS + 8, z);
        spawn(fx, ORB, saves, 0x01E0_12CC);
        spawn(fx, LIGHT, saves, 0x01E0_12E0);
    }
    fx.leave(0x20);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The rings' alpha: 0x80 during the life, then down by 8 (the task ends
/// below 0xA).
fn fade(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x63, a - 8);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
}

/// 0x1E012F0 (`kind` 0), 0x1E01990 (1), 0x1E01B50 (2), 0x1E01CE0 (3) (D,
/// task): a flat ring (frame 0x20: the entry's s0 = D): size 50 / 10 /
/// 100 / 5, cell 3 (0x1E01990: 0..7, stepping), white / grey, angle +0x5A
/// random, +0xA rand % 8, +0xC = the spawn kind +0x50, radius +0x50 10..59
/// / 10..59 / 20..99 / 4..53 growing by +0xE 6..15 / 26..35 / 14..23 / (not
/// growing), row 0..34, life 3..7 (0x1E01CE0: 1..5).
fn small(fx: &mut Fx, t: u32, kind: usize) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    let f = [RING, BURST, BURST_W, WAKE][kind];
    // (size, grey, radius m / add, growth m / add, life add); the rand
    // sites from the step's address.
    let (size, grey, rm, ra, em, ea, la) = [(0x4248_0000u32, 0xFF, 0x32, 0xA, 0xA, 6, 3), (0x4120_0000, 0x80, 0x32, 0xA, 0xA, 0x1A, 3), (0x42C8_0000, 0xFF, 0x50, 0x14, 0xA, 0xE, 3), (0x40A0_0000, 0x80, 0x32, 4, 0xA, 6, 1)][kind];
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, size);
        // 0x1E01990's sites are 0x14 further on (its cell rand first).
        let o = if kind == 1 { 0x14 } else { 0 };
        if kind == 1 {
            let v = fx.rand_from(t, f + 0x28);
            fx.mem.set16(t + 0x56, v % 8);
        } else {
            fx.mem.set16(t + 0x56, 3);
        }
        l2::rgb(fx, t + 0x60, grey, grey, grey);
        let v = rmod(fx, t, f + 0x48 + o, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, f + 0x64 + o);
        fx.mem.set16(t + 0xA, v % 8);
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, k);
        let v = rmod(fx, t, f + 0x88 + o, rm);
        fx.mem.set16(t + 0x50, v + ra);
        let v = rmod(fx, t, f + 0xA8 + o, em);
        fx.mem.set16(t + 0xE, v + ea);
        let v = rmod(fx, t, f + 0xC8 + o, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xE4 + o, 5);
        fx.mem.set16(t + 6, v + la);
        fx.mem.set16(t + 0x58, 2);
    }
    fade(fx, t);
    if kind != 3 && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    if kind == 1 {
        let c = fx.mem.i16(t + 0x56) + 1;
        fx.mem.set16(t + 0x56, if c >= 8 { 0 } else { c });
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01480 (D, task): the glowing quad (frame 0x70: the entry's s0 = D,
/// s1 = the flashes, s2 = 0, s3 = the slot): size 20 growing by 2 to 50,
/// white / pale grey by its age's parity, both alphas in by 0x2A under
/// 0x80 for 140 updates, then out by 0xE twice; from its update 61 to 139
/// it moves a 75th of the way from the attacker to the target a update,
/// leaving a grey ring; on update 140 thirty grey rings burst out (within
/// 5), on 140 / 141 two white ones (within 10); its cell steps 0..7 from
/// update 61; turned by the camera's angles.
fn orb(fx: &mut Fx, t: u32, flashes: u32, slot: u32, k: u32) {
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x40, TASK_RA), (0x30, u64::from(slot)), (0x20, 0), (0x10, u64::from(flashes)), (0, u64::from(D))]);
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_14B0);
    let h = fx.self_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x60, u64::from(sp + 0x60), 0x01E0_14C8);
    for o in [0u32, 4, 8] {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x50 + o), fx.mem.f(sp + 0x60 + o)), 75.0);
        fx.mem.setf(sp + 0x50 + o, v);
    }
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x14);
        fx.mem.set16(t + 0xA, 0x14);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x8C {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
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
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x32 {
                fx.mem.set16(t + o, s + 2);
            }
        }
    }
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    let a = fx.mem.i16(t + 0x54);
    if (0x3D..0x8C).contains(&a) {
        for o in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(sp + 0x50 + o));
            fx.mem.setf(t + 0x10 + o, v);
        }
    }
    // The spawns' saves: the step's s0 = the loop counter, s1 = sp + 0x58,
    // s2 = the task, s3 = D, s4 = the entry's (the slot's index).
    let burst = |fx: &mut Fx, n: u32, m: i32, ras: [u32; 3], step: u32, sra: u32| {
        for i in 0..n {
            for o in 0..4u32 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * o);
                fx.mem.write_u32(POS + 4 * o, w);
            }
            for (o, ra) in [0u32, 4, 8].into_iter().zip(ras) {
                let v = rmod(fx, i, ra, m);
                let p = ee::add(fx.mem.f(POS + o), ee::from_i32(v - m / 2));
                fx.mem.setf(POS + o, p);
            }
            spawn(fx, step, [u64::from(i), u64::from(sp + 0x58), u64::from(t), u64::from(D), u64::from(k)], sra);
        }
    };
    if fx.mem.i16(t + 0x54) == 0x8C {
        burst(fx, 0x1E, 0xA, [0x01E0_16FC, 0x01E0_1730, 0x01E0_1764], BURST, 0x01E0_17A0);
    }
    let a = fx.mem.i16(t + 0x54);
    if (0x8C..0x8E).contains(&a) {
        burst(fx, 2, 0x14, [0x01E0_1810, 0x01E0_1844, 0x01E0_1878], BURST_W, 0x01E0_18B4);
    }
    let a = fx.mem.i16(t + 0x54);
    if (0x3D..0x8C).contains(&a) {
        for o in 0..4u32 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * o);
            fx.mem.write_u32(POS + 4 * o, w);
        }
        // s0 here: sp + 0x54 (no burst loop runs on these updates).
        spawn(fx, WAKE, [u64::from(sp + 0x54), u64::from(sp + 0x58), u64::from(t), u64::from(D), u64::from(k)], 0x01E0_1928);
    }
    if fx.mem.i16(t + 0x54) >= 0x3D {
        let c = fx.mem.i16(t + 0x56) + 1;
        fx.mem.set16(t + 0x56, if c >= 8 { 0 } else { c });
    }
    // s0: the second burst loop's counter (2) after it ran, else sp + 0x54.
    let s0 = if (0x8C..0x8E).contains(&a) { 2 } else { u64::from(sp + 0x54) };
    camera_angles_saving(fx, t + 0x20, s0, u64::from(sp + 0x58), 0x01E0_1964);
    age(fx, t);
    fx.leave(0x70);
}

/// 0x1E01E50 (D, task): the stage light (0x1F7F10) down by 10 to 0x30
/// while the timer is at 21 or more, then back up by 10 to 0x80 (the task
/// ending there); 0x80 when the timer is at 2. A 0x10 frame (ra).
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x10);
    fx.saved(sp, &[(0, TASK_RA)]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i32(TIMER) >= 0x15 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else {
        let l = fx.mem.i16(t + 0x50);
        if l < 0x80 {
            fx.mem.set16(t + 0x50, l + 0xA);
        } else {
            fx.mem.set16(t + 0x50, 0x80);
            fx.mem.set16(t + 0x58, 0);
        }
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    if fx.mem.i32(TIMER) == 2 {
        fx.set_stage_light(0x80);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x10);
}
