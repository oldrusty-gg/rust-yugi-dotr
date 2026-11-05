//! BMODULE b708 (card 352), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01DC0, data area 0x1ED0000) [F]: b280's program (its spawn be48a2c22bcb, `b280::spawn52`, and its
//! wide band step) on the same layout, drawn with 20-degree bands
//! (d058fcddf114 at 0x1E02420, `quads4::ring_band_fy20`), bands
//! (853ed233d9de at 0x1E00920, `quads4::ring_band_fy`) and flat facing
//! rings turned about z (2f5a28a746a2 at 0x1E00D80,
//! `quads4::ring_facing_flat_z`). On updates 30..170 three sinking wide
//! bands a update from the attacker's nodes 0x25 / 0x29 / 0x2D (odd
//! updates) or 0x53 / 0x57 / 0x5B (even); below 195 on even updates a
//! white disc band and a ring at a point within 15 of the target. Returns
//! 1 on update 120, 3 on 205, 6 on 131..199, 8 once the 210-update timer
//! +0x4C10 has run out.
use crate::b280::{band, spawn52, Band};
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const WIDE: u32 = 0x01E0_1610;
const DISC: u32 = 0x01E0_1B10;
const RING: u32 = 0x01E0_1C00;
/// The return address of the entry's `jalr` into a step (0x1E02058 + 8).
const TASK_RA: u64 = 0x01E0_2060;
const TIMER: u32 = D + 0x4C10;
const NODES: u32 = D + 0x4C48;
const SWAY: u32 = D + 0x4C70;
const SLOT: u32 = D + 0x4C76;
const FACING: u32 = D + 0x4C7A;
const COUNTER: u32 = D + 0x4C7E;
const BYTES: u32 = D + 0x4C80;
const BYTES2: u32 = D + 0x4D80;

const WIDE_BAND: Band = Band { wide: true, ra: TASK_RA, colour: (0x80, 0x10, 0xFF), ages: 0x19, alpha_in: 0x80, sweep_ages: 8, amplitude: 26.5, sink: true };

/// The entry 0x1E01DC0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    fx.mem.write_u32(D + 0x4C1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4FF0);
    l2::sine_table(fx, D + 0x4E80);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4C7C, param);
    fx.mem.set32(TIMER, 0xD2);
    for (k, node) in [0x25, 0x29, 0x2D, 0x53, 0x57, 0x5B].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.texture(5, 0);
    fx.mem.write_u32(D + 0x4BF0, t);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x70);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x74)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // The entry's s0 is D here (rand's frame).
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_1F34);
        let (_, h) = fx.div(v, 0x18);
        fx.mem.set8(BYTES + k, h - 0xC);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_1F84);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_1FD4);
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
            WIDE => band(fx, t, &WIDE_BAND),
            DISC => disc(fx, t),
            RING => crate::b082::ring_v(fx, t, RING, TASK_RA, 0x41A0_0000, 0xC0, (0xA, 5), (0xF, 9), (0x11, 0xF)),
            _ => panic!("b708: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == WIDE || f == DISC {
            for k in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, w);
            }
        }
        if f == WIDE {
            let a = (fx.mem.i16(t + 0x5C) + fx.mem.i16(SWAY)) as i16 as i32;
            let s = l2::sin_deg(fx, D + 0xD8, a, 0);
            let x = ee::add(fx.mem.f(sp + 0x50), ee::mul(fx.mem.f(t + 0x30), s));
            fx.mem.setf(sp + 0x50, x);
            quads4::ring_band_fy20(fx, D, t, 0x20, sp + 0x50);
        }
        if f == DISC {
            quads4::ring_band_fy(fx, D, t, 0x20, sp + 0x50);
        }
        if f == RING {
            let sub = fx.mem.i16(t + 0x56);
            crate::b615::cell(fx, sp + 0x60, 0x14, sub);
            let w = fx.mem.read_u32(D + 0x4BF0);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 6);
            let o = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, o);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x78 => 1,
        0xCD => 3,
        0x83..=0xC7 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E012D0(D, 0): the update counter and the emitter (frame 0x50: ra,
/// the entry's s3 (taken as 0), s2 = the parameter, s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x40, 0x01E0_2020), (0x30, 0), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    fx.node_position(h, slot, D + 0x250);
    let z = fx.mem.f(D + 0x258);
    let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 2.0) } else { ee::add(z, 2.0) };
    fx.mem.setf(D + 0x258, z);
    let mut s2 = p;
    let c = fx.mem.i16(COUNTER);
    if (0x1E..0xAB).contains(&c) {
        s2 = 3;
        for k in 0..3u32 {
            let c = fx.mem.i16(COUNTER);
            let (_, m) = fx.div(c, 6);
            fx.mem.set16(SLOT, m);
            let node = if fx.mem.i16(COUNTER) % 2 == 1 { NODES + 2 * k } else { NODES + 6 + 2 * k };
            let h = fx.self_handle();
            let slot = fx.mem.i16(node);
            fx.node_position(h, slot, D + 0x250);
            spawn52(fx, WIDE, &[(0x50, 0x01E0_142C), (0x40, 0), (0x30, u64::from(D)), (0x20, 3), (0x10, u64::from(D + 2 * k)), (0, u64::from(k))]);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if c < 0xC3 && c % 2 == 0 {
        fx.mem.set16(SLOT, c % 2);
        let h = fx.target_handle();
        crate::b076::unit_position_saving(fx, h, D + 0x250, 1, 0x01E0_14D4);
        for (o, ra) in [(0x250u32, 0x01E0_14DCu32), (0x254, 0x01E0_1510), (0x258, 0x01E0_1544)] {
            let v = fx.rand_from(1, ra);
            let (_, m) = fx.div(v, 0x1E);
            let x = ee::add(fx.mem.f(D + o), ee::from_i32(m - 0xF));
            fx.mem.setf(D + o, x);
        }
        let saves = |ra: u32| [(0x50, u64::from(ra)), (0x40, 0), (0x30, u64::from(D)), (0x20, s2), (0x10, 0), (0, 1)];
        spawn52(fx, DISC, &saves(0x01E0_1580));
        spawn52(fx, RING, &saves(0x01E0_1594));
    }
    let a = fx.mem.i16(SWAY) + 6;
    fx.mem.set16(SWAY, a);
    let a = fx.mem.i16(SWAY);
    if a >= 0x168 {
        let (_, m) = fx.div(a, 0x168);
        fx.mem.set16(SWAY, m);
    }
    fx.leave(0x50);
}

/// 0x1E01B10 (D, task): a disc band (frame 0x20: ra, the entry's s0 = D):
/// white (0xC0) inside, (0x80, 0x10, 0xFF) outside, radius 5 growing by
/// 3.5; in by 0x40 under 0x80 for 6 ages, then out by 14; turned by the
/// camera's angles (the first + 0x4000).
fn disc(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 6 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a < 0x10 {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), 0, DISC + 0xB0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    let r = ee::add(fx.mem.f(t + 0x48), 3.5);
    fx.mem.setf(t + 0x48, r);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}
