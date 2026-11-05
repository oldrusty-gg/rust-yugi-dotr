//! BMODULE b547 (card 127), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02510, data area 0x1ED0000) [F]: b615's program at a lower data layout (D+0x44xx) with its own
//! columns: a pool of 150 tasks of 0x70 bytes at D+0x2A0 (`jalr` steps)
//! drawn with flat facing rings (9c13e08cfd2f at 0x1E00D80,
//! `quads4::ring_facing_flat`), rings of quads (7369422df7f8 at 0x1E012D0)
//! and band rings (3fb2be4df9d4 at 0x1E00920, `quads4::ring_band`); the
//! ring / quad / band steps are b615's (a75a0f66bc56 the same code). On
//! updates 3 k + 1 below 156 two grey columns rise 8 units off the
//! attacker's node 0x13 (they drift for ages 21..29, their cell row
//! flickers, from age 146 they lean to the facing and from 183 they rise);
//! at age 215 each throws two rings. Returns 1 on update 160, 3 on 320, 6
//! on 171..314, 8 once the 382-update timer +0x4490 has run out.
use crate::b615::{band, cell, quads, ring, rmod, spawn_at};
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const COLUMN: u32 = 0x01E0_19E0;
const RING: u32 = 0x01E0_2070;
const QUADS: u32 = 0x01E0_2200;
const BAND: u32 = 0x01E0_2400;
/// The return address of the entry's `jalr` into a step (0x1E02800 + 8).
const TASK_RA: u64 = 0x01E0_2808;
const TEX: u32 = D + 0x4470;
const TIMER: u32 = D + 0x4490;
const COLOUR: u32 = D + 0x4494;
const NODES: u32 = D + 0x44BC;
const SLOT: u32 = D + 0x44EA;
const NEXT: u32 = D + 0x44EC;
const FACING: u32 = D + 0x44EE;
const COUNTER: u32 = D + 0x44F2;
const BYTES: u32 = D + 0x44F4;
const BYTES2: u32 = D + 0x45F4;
const SINES: u32 = D + 0x46F4;

/// The entry 0x1E02510.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(D + 0x449C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4860);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x44F0, param);
    fx.mem.set32(TIMER, 0x17E);
    for (k, node) in [0x13, 0x1C, 0xD].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x44CC, r);
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x288 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // The entry's s0 is D here (rand's frame).
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_2694);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_26E4);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_273C);
        fx.mem.set8(BYTES + k, v % 16 - 8);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_278C);
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(BYTES2 + k, h - 0x14);
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
            COLUMN => column(fx, t),
            RING => ring(fx, t, RING, TASK_RA, 0x41A0_0000),
            QUADS => quads(fx, t, QUADS, TASK_RA),
            BAND => band(fx, t, BAND, TASK_RA),
            _ => panic!("b547: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == COLUMN {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x60, 0x14, sub);
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 3);
            let o = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, o);
            let x = fx.mem.read_u32(t + 0x10);
            fx.mem.write_u32(D + 0x20, x);
            let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
            fx.mem.setf(D + 0x24, y);
            let z = fx.mem.read_u32(t + 0x18);
            fx.mem.write_u32(D + 0x28, z);
            fx.mem.set16(D + 0xBE, 0);
            quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x10, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x14, sub);
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 6);
            fx.mem.set16(D + 0xC0, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == QUADS {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x9B, sub);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == BAND {
            for k in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, w);
            }
            quads4::ring_band(fx, D, t, 0x21, sp + 0x50);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xA0 => 1,
        0x140 => 3,
        0xAB..=0x13A => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E01880(D, 0): the update counter and the emitter (frame 0x40: ra,
/// the entry's s2 = the parameter, s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_27D8), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if c > 0 && c < 0x9C && fx.div(c, 3).1 == 1 {
        for k in 0..2u64 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES);
            fx.node_position(h, slot, D + 0x250);
            let z = fx.mem.f(D + 0x258);
            let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 8.0) } else { ee::sub(z, 8.0) };
            fx.mem.setf(D + 0x258, z);
            let s = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, s);
            spawn_at(fx, COLUMN, COLOUR, SLOT, &[(0x50, 0x01E0_1974), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, 2), (0, k)]);
            let s = fx.mem.i16(NEXT) + 1;
            fx.mem.set16(NEXT, s);
            if fx.mem.i16(NEXT) >= 0x78 {
                fx.mem.set16(NEXT, 0);
            }
        }
    }
    fx.leave(0x40);
}

/// 0x1E019E0 (D, task): a ring column (frame 0x40): size 10, grey, cell
/// row 0 / 1 (flickering; 4 / 5 for ages 146..156, 2 / 3 for 157..180
/// once the update is past 150), angles random, orientation +0xA 0 / 2,
/// the spawn's slot in +0xC, radius growing by 15 to 100..449, row 0..34,
/// a drift +0x30..+0x38 (-20..19) / 18 for ages 21..29 (z 1 to the facing
/// / 1.3 away); in by 0x20 under 0x80 until update 320, then out by 8. At
/// age 215 it throws two rings at points +-15 off. +0x34 = 3 sin(+0x5C)
/// (+0x5C stepping by 6); for ages 146..180 it moves 5 a update to the
/// facing and sinks by 1 while above the target's height - 10; from age
/// 183 it rises by 0.4.
fn column(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    let f = COLUMN;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let v = fx.rand_from(t, f + 0x30);
        fx.mem.set16(t + 0x56, v % 2);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x5C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0x78, 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = fx.rand_from(t, f + 0x9C);
        fx.mem.set16(t + 0xA, if v % 2 != 0 { 2 } else { 0 });
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        fx.mem.set16(t + 0x50, 0);
        let v = rmod(fx, t, f + 0xD0, 0x15E);
        fx.mem.set16(t + 0x52, v + 0x64);
        let v = rmod(fx, t, f + 0xF0, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rmod(fx, t, f + 0x110, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0x12C, 5);
        fx.mem.set16(t + 6, v + 3);
        for (o, site) in [(0x30u32, 0x14Cu32), (0x34, 0x178), (0x38, 0x1A4)] {
            let v = rmod(fx, t, f + site, 0x28);
            fx.mem.setf(t + o, ee::from_i32(v - 0x14));
        }
        for o in [0x30u32, 0x34, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 18.0);
            fx.mem.setf(t + o, q);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0x140 {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x20);
        }
        let r = fx.mem.i16(t + 0x50);
        if r < fx.mem.i16(t + 0x52) {
            fx.mem.set16(t + 0x50, r + 0xF);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x63, a - 8);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let age = fx.mem.i16(t + 0x54);
    if (0x15..0x1E).contains(&age) {
        let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
        fx.mem.setf(t + 0x10, x);
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x38));
        fx.mem.setf(t + 0x18, z);
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 1.0) } else { ee::add(z, f32::from_bits(0x3FA6_6666)) };
        fx.mem.setf(t + 0x18, z);
    }
    if fx.mem.i16(t + 0x54) == 0xD7 {
        for k in 0..4 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        let k = (t - D - 0x2A0) / 0x70;
        let saves = |ra: u32| [(0x50, u64::from(ra)), (0x40, 0), (0x30, u64::from(k)), (0x20, u64::from(t - 0x2A0)), (0x10, u64::from(D)), (0, u64::from(t))];
        for (sites, ra) in [([0x01E0_1CF8u32, 0x01E0_1D2C, 0x01E0_1D60], 0x01E0_1DA4u32), ([0x01E0_1DA4, 0x01E0_1DD8, 0x01E0_1E0C], 0x01E0_1E50)] {
            for (o, site) in [0x250u32, 0x254, 0x258].into_iter().zip(sites) {
                let r = rmod(fx, t, site, 0x1E);
                let v = ee::add(fx.mem.f(D + o), ee::from_i32(r - 0xF));
                fx.mem.setf(D + o, v);
            }
            spawn_at(fx, RING, COLOUR, SLOT, &saves(ra));
        }
    }
    let age = fx.mem.i16(t + 0x54);
    let cellv = fx.mem.i16(t + 0x56);
    let toggle = if cellv != 0 { 0 } else { 1 };
    let next = if fx.mem.i16(COUNTER) < 0x97 {
        toggle
    } else if (0x92..0x9D).contains(&age) {
        if cellv == 4 { 5 } else { 4 }
    } else if (0x9D..0xB5).contains(&age) {
        if cellv == 2 { 3 } else { 2 }
    } else {
        toggle
    };
    fx.mem.set16(t + 0x56, next);
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    let a = fx.mem.i16(t + 0x5C);
    let s = l2::sin_deg(fx, SINES, a, 1);
    fx.mem.setf(t + 0x34, ee::mul(3.0, s));
    let age = fx.mem.i16(t + 0x54);
    if (0x92..0xB5).contains(&age) {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(t + 0x18, z);
        let lim = ee::sub(fx.mem.f(sp + 0x34), 10.0);
        let y = fx.mem.f(t + 0x14);
        if lim < y {
            fx.mem.setf(t + 0x14, ee::sub(y, 1.0));
        }
    }
    if fx.mem.i16(t + 0x54) >= 0xB7 {
        let y = ee::add(fx.mem.f(t + 0x14), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(t + 0x14, y);
    }
    let a = fx.mem.i16(t + 0x5C) + 6;
    fx.mem.set16(t + 0x5C, a);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x40);
}
