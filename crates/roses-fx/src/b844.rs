//! BMODULE b844 (card 673), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02B50, data area a0 = 0x1ED0000) [F] (helper F10,
//! 3 Nov 2026): b387's library (card 487, `crate::b387`; s0 = D, s1 = the
//! OT). Seven nodes (0x76 / 0x7B / 0x85 / 0x8C / 0x7F / 0x89 / 0x91 ->
//! D+0xC..0x24). On update 40 a flash on node +0xC (+0xA4, 300 wide),
//! relit 380 wide whenever it runs out; on 81 / 89 / 110 one of three
//! groups of four flashes (+0x1A80 + 0x130 g, 160 - 15 k wide, lasting
//! until update 240) laid each update from node +0x10 + 4 g to node +0x1C
//! + 4 g in quarters; on 198 a 1500-wide flash at the target (+0xF0); on
//! even updates 198..225 a ribbon (+0x220, random bytes +0x4ED0); on
//! 198..231 two comets a update into each of two pools (+0x1E10 / +0x2608,
//! `comet::comets_c`, velocities +0x2E00 / +0x2F68, falling 0.08); the
//! self unit's flag (0x1F69C0) on 193..245; the stage light (+0xA0).
//! Returns 1 on update 188, 6 on 198..226, 2 on 227, 8 on 430 (and at once
//! for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (+0x40, never
//! started), `comets_c` 0x1E00410, `flash` 0x1E01410, `ribbon` 0x1E01FC0
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const GROUPS: u32 = D + 0x1A80;
const RIBBON: u32 = D + 0x220;
const BYTES: u32 = D + 0x4ED0;
/// The two comet pools: records, point pool, velocities, node-position
/// return address, the three rands' return addresses.
const POOLS: [(u32, u32, u32, u32, [u32; 3]); 2] = [
    (D + 0x1E10, D + 0x30D0, D + 0x2E00, 0x01E0_33D0, [0x01E0_33F4, 0x01E0_3448, 0x01E0_34A4]),
    (D + 0x2608, D + 0x3FD0, D + 0x2F68, 0x01E0_35E8, [0x01E0_360C, 0x01E0_3660, 0x01E0_36BC]),
];

/// The entry 0x1E02B50 (a0 = D, a1 = param; s0 = D, s1 = the OT, s2 =
/// param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4FD0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..2 {
        fx.texture(5, k);
    }
    for (k, (node, clear)) in [(0x76, 1), (0x7B, 0), (0x85, 0), (0x8C, 0), (0x7F, 0), (0x89, 0), (0x91, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x1A8);
    rand_bytes(fx, BYTES, 0x100, 0x01E0_2CEC, [Some(D), Some(OT), Some(2)]);
    0
}

/// The flash fields the record's flashes share: 9 / 0x83-style kind, size
/// `w`, no turn.
fn flash_fields(fx: &mut Fx, r: u32, kind: u8, a: u8, b9: u8, b8: u8, c: [u8; 4], w: f32) {
    fx.mem.write_u8(r + 4, kind);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u8(r + 0xA, a);
    fx.mem.write_u8(r + 9, b9);
    fx.mem.write_u8(r + 8, b8);
    for (o, v) in [0xCu32, 0xD, 0xE, 0xF].into_iter().zip(c) {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.setf(r + 0x10, w);
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u32(r + 0x18, 0);
}

/// ((rand >> 4) % 600 - 300) / 100 (rand's s0 = D).
fn spread(fx: &mut Fx, ra: u32) -> f32 {
    let x = fx.rand_from(D, ra) >> 4;
    let (_, h) = fx.div(x, 600);
    ee::div(ee::from_i32(h - 300), 100.0)
}

fn comet(fx: &mut Fx, sp: u32, (recs, pool, vel, ra_node, ra): (u32, u32, u32, u32, [u32; 3])) {
    let i = first_free(fx, recs, 0x44, 0x1E);
    if i < 0 {
        return;
    }
    let r = recs + 0x44 * i as u32;
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 4);
    fx.mem.write_u32(r + 8, pool);
    fx.mem.write_u32(r + 0x28, 0xF0);
    for (o, v) in [(0x1Eu32, 7u8), (0x1F, 1), (0x21, 2), (0x22, 4), (0x23, 6), (0x37, 0x80), (0x34, 0x80), (0x35, 0x80), (0x36, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x2C, 0x4316_0000);
    fx.mem.write_u32(r + 0x30, 0x4348_0000);
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(r + 0x1E)) + 1 {
        fx.mem.set16(r + 0xC + 2 * a, -1);
        a += 1;
    }
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x18));
    node_position_saving(fx, h, slot, sp + 0x80, u64::from(D), Some(u64::from(OT)), ra_node);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
        fx.mem.write_u32(r + 0x38 + 4 * c, w);
    }
    let v = vel + 12 * i as u32;
    let x = spread(fx, ra[0]);
    fx.mem.setf(v, x);
    let y = spread(fx, ra[1]);
    fx.mem.setf(v + 4, ee::add(f32::from_bits(0x3F4C_CCCD), y));
    let z = spread(fx, ra[2]);
    fx.mem.setf(v + 8, z);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(D), 0x01E0_2D14);
    let n = fx.mem.u16(D);
    match n {
        0x1AE => return 8,
        0xC6 => {
            let r = D + 0xF0;
            set_counter4(fx, r, 0x34, 0, 0xFF);
            fx.mem.write_u8(r + 5, 0);
            flash_fields(fx, r, 0x83, 0x80, 0, 0x60, [0xF0; 4], 1500.0);
            copy_v4(fx, sp + 0x70, r + 0x1C);
        }
        0x51 | 0x59 | 0x6E => {
            let g = match n {
                0x6E => 2,
                0x59 => 1,
                _ => 0,
            };
            for k in 0..4i32 {
                let r = GROUPS + 0x130 * g + 0x4C * k as u32;
                set_counter4(fx, r, (0xF0 - i32::from(n)) & 0xFF, 0, 0xFF);
                fx.mem.write_u8(r + 5, 0);
                let w = ee::sub(160.0, ee::mul(15.0, ee::from_i32(k)));
                flash_fields(fx, r, 0x83, 0xA0, 0x10, 0x80, [0xC0, 0xC0, 0xC0, 0x40], w);
            }
        }
        0x28 => {
            let r = D + 0xA4;
            set_counter4(fx, r, 0x10, 0, 0xFF);
            fx.mem.write_u8(r + 5, 1);
            let b = fx.mem.read_u8(D + 0xC);
            fx.mem.write_u8(r + 6, b);
            flash_fields(fx, r, 0x81, 0x80, 0, 0x60, [0xC0; 4], 300.0);
        }
        _ => {}
    }
    if fx.mem.u16(D) >= 0x29 && fx.mem.read_u8(D + 0xA4) == 0 {
        set_counter4(fx, D + 0xA4, 0x10, 0, 0xFF);
        fx.mem.write_u32(D + 0xB4, 0x43BE_0000);
        fx.mem.write_u32(D + 0xB8, 0);
    }
    for g in 0..3u32 {
        let r = GROUPS + 0x130 * g;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x10 + 4 * g);
        node_position_saving(fx, h, s, r + 0x1C, u64::from(D), Some(u64::from(OT)), 0x01E0_3050);
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0x1C + 4 * g);
        node_position_saving(fx, h, s, r + 0x100, u64::from(D), Some(u64::from(OT)), 0x01E0_3068);
        let q: [f32; 3] = core::array::from_fn(|c| {
            let c = 4 * c as u32;
            ee::div(ee::sub(fx.mem.f(r + 0x100 + c), fx.mem.f(r + 0x1C + c)), 4.0)
        });
        for c in 0..3u32 {
            let p = ee::add(fx.mem.f(r + 0x1C + 4 * c), q[c as usize]);
            fx.mem.setf(r + 0x68 + 4 * c, p);
        }
        for c in 0..3u32 {
            let p = ee::add(fx.mem.f(r + 0x1C + 4 * c), ee::mul(2.0, q[c as usize]));
            fx.mem.setf(r + 0xB4 + 4 * c, p);
        }
    }
    let n = fx.mem.u16(D);
    if (0xC6..0xE2).contains(&n) && n & 1 == 0 {
        ribbon(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0xC6..0xE8).contains(&n) {
        for _ in 0..2 {
            comet(fx, sp, POOLS[0]);
            comet(fx, sp, POOLS[1]);
        }
    }
    for i in 0..0x1Eu32 {
        for &(recs, _, vel, _, _) in &POOLS {
            let r = recs + 0x44 * i;
            if fx.mem.read_u8(r) == 0 {
                continue;
            }
            let v = vel + 12 * i;
            let y = ee::sub(fx.mem.f(v + 4), f32::from_bits(0x3DA3_D70A));
            fx.mem.setf(v + 4, y);
            for c in 0..3u32 {
                let p = ee::add(fx.mem.f(r + 0x38 + 4 * c), fx.mem.f(v + 4 * c));
                fx.mem.setf(r + 0x38 + 4 * c, p);
            }
        }
    }
    let n = fx.mem.u16(D);
    if (0xC1..0xF6).contains(&n) {
        crate::b369::unit_flag(fx, 1, u64::from(D), 0x01E0_384C);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_385C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_386C, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::comet::comets_c(fx, 0x01E0_0410, POOLS[0].0, OT, 0x1E);
    blib::comet::comets_c(fx, 0x01E0_0410, POOLS[1].0, OT, 0x1E);
    blib::star::flash(fx, 0x01E0_1410, D + 0xA4, OT, 5);
    blib::star::flash(fx, 0x01E0_1410, GROUPS, OT, 0xC);
    blib::streak::ribbon(fx, 0x01E0_1FC0, RIBBON, OT, 0x3C);
    let n = fx.mem.u16(D);
    if (0xC6..0xE3).contains(&n) {
        6
    } else if n == 0xE3 {
        2
    } else if n == 0xBC {
        1
    } else {
        0
    }
}

/// Even updates of 198..225 [F 0x1E03150]: one ribbon at the target (sp +
/// 0x70) with three random turns +0x58.. (& 0x7F).
fn ribbon(fx: &mut Fx, sp: u32) {
    let i = first_free(fx, RIBBON, 0x68, 0x3C);
    if i < 0 {
        return;
    }
    let r = RIBBON + 0x68 * i as u32;
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0x85);
    fx.mem.write_u32(r + 0x14, 0x4080_0000);
    fx.mem.write_u32(r + 0x1C, 0x42A0_0000);
    for (o, v) in [(8u32, 0x20u8), (5, 0), (0xA, 0), (0xB, 2), (0xC, 3)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(D, 0x01E0_31E4);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x10), (0x27, 0x40)] {
        fx.mem.write_u8(r + o, v);
    }
    for (k, ra) in [0x01E0_3234u32, 0x01E0_324C, 0x01E0_3264].into_iter().enumerate() {
        let x = (fx.rand_from(D, ra) >> 4) & 0x7F;
        fx.mem.set32(r + 0x58 + 4 * k as u32, x);
    }
    copy_v4(fx, sp + 0x70, r + 0x28);
}
