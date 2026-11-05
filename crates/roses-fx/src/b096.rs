//! BMODULE b096 (card 106), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E00AF0, data area a0 = 0x1ED0000) [F] (helper B3, 30
//! Oct 2026): b076's family, one pool of 500 bouncing sprites
//! (`sprite::bounce` at 0x1E00080, 0x44-byte records at +0x3C) thrown in
//! bursts: on updates 2..27 six a update from the attacker's node 0x10,
//! falling sideways (mirrored by the attacker's facing +0x30); on 40..71
//! three a update from node 0xC, thrown out with random speed; on 88..264
//! three a update between nodes 0xD / 0x10, drifting toward node 0xC; on
//! updates 180, 268 and 320 twenty-eight round the target (+-12.5). Returns
//! 6 on updates 155..326, 2 on 327, 8 on 420 (and at once for a parameter
//! of -2 / -3).
use crate::b076::{node_position_saving, rand_bytes, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const POOL: u32 = D + 0x3C;

/// The entry 0x1E00AF0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x100);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x100);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x860C);
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
    for (at, node, clear) in [(0xCu32, 0xD, 1), (0x10, 0x10, 0), (0x14, 0xC, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The entry's s0 = 2 (the texture loop's); s1 / s2: the caller's.
    rand_bytes(fx, D + 0x850C, 0x100, 0x01E0_0C28, [Some(2), None, None]);
    0
}

/// The entry's s0 / s1 (64 bits) as callee prologues save them; the
/// caller's s1 is 0 [run].
struct Regs {
    s0: u64,
    s1: u64,
}

fn sx(v: i32) -> u64 {
    i64::from(v) as u64
}

fn node(fx: &mut Fx, slot_at: u32, out: u32, r: &Regs, ra: u32) {
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(slot_at);
    node_position_saving(fx, h, s, out, r.s0, Some(r.s1), ra);
}

/// (rand >> 4) % m - off, over 100 (rand from `ra`, the entry's s0 = `k`).
fn offset(fx: &mut Fx, k: u32, ra: u32, m: i32, off: i32) -> f32 {
    let x = fx.rand_from(k, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h - off), 100.0)
}

/// (rand >> 4) % m x 200 - off, over 100.
fn steps(fx: &mut Fx, k: u32, ra: u32, m: i32, off: i32) -> f32 {
    let x = fx.rand_from(k, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h * 200 - off), 100.0)
}

/// The fields every throw sets (`ras`: the rands' returns for +4, +9,
/// +0xE and +0xC; the colour masks `m`).
fn sprite(fx: &mut Fx, b: u32, k: u32, ras: [u32; 4], m: i32) {
    fx.mem.write_u8(b + 5, 0);
    let x = fx.rand_from(k, ras[0]);
    fx.mem.set8(b + 4, ((x >> 4) & 3) << 4);
    fx.mem.write_u8(b + 8, 0);
    fx.mem.write_u8(b + 0xB, 1);
    fx.mem.write_u8(b + 0xA, 5);
    let x = fx.rand_from(k, ras[1]);
    fx.mem.set8(b + 9, (x >> 4) & 3);
    let x = fx.rand_from(k, ras[2]);
    fx.mem.set8(b + 0xE, ((x >> 4) & m) + 0x30);
    fx.mem.write_u8(b + 0xD, 0x10);
    let x = fx.rand_from(k, ras[3]);
    fx.mem.set8(b + 0xC, ((x >> 4) & m) + 0x30);
}

/// Updates 180 / 268 / 320 [F 0x1E00C8C]: 28 round the target (sp + 0xF0).
fn burst(fx: &mut Fx, sp: u32, r: &mut Regs) {
    for k in 0..0x1Cu32 {
        let i = first_free(fx, POOL, 0x44, 0x1F4);
        r.s1 = sx(i);
        if i < 0 {
            continue;
        }
        r.s1 = sx(0x44 * i);
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x30, ((k << 1) & 0xFF) as i32, 0xFF);
        sprite(fx, b, k, [0x01E0_0CF4, 0x01E0_0D24, 0x01E0_0D3C, 0x01E0_0D60], 0x3F);
        let x = fx.rand_from(k, 0x01E0_0D7C);
        fx.mem.set8(b + 0xF, ((x >> 4) & 0x3F) + 0x50);
        fx.mem.write_u32(b + 0x10, 0x437A_0000);
        let f = steps(fx, k, 0x01E0_0DA0, 6, 0x258);
        fx.mem.setf(b + 0x14, ee::add(18.0, f));
        fx.mem.write_u32(b + 0x1C, 0);
        fx.mem.write_u32(b + 0x18, 0);
        fx.mem.write_u32(b + 0x30, 0x3F80_0000);
        for o in [0x40u32, 0x3C, 0x38, 0x34, 0x2C] {
            fx.mem.write_u32(b + o, 0);
        }
        let f = offset(fx, k, 0x01E0_0E30, 0x9C4, 0x4E2);
        let v = ee::add(fx.mem.f(sp + 0xF0), f);
        fx.mem.setf(b + 0x20, v);
        let f = offset(fx, k, 0x01E0_0E80, 0x9C4, 0x4E2);
        let v = ee::add(ee::sub(fx.mem.f(sp + 0xF4), 8.0), f);
        fx.mem.setf(b + 0x24, v);
        let f = offset(fx, k, 0x01E0_0EE0, 0x9C4, 0x4E2);
        let v = ee::add(fx.mem.f(sp + 0xF8), f);
        fx.mem.setf(b + 0x28, v);
    }
    r.s0 = 0x1C;
}

/// Updates 88..264 [F 0x1E00F50]: three between nodes +0xC / +0x10,
/// drifting toward node +0x14.
fn stream(fx: &mut Fx, sp: u32, r: &Regs) {
    node(fx, D + 0x14, sp + 0xA0, r, 0x01E0_0F80);
    node(fx, D + 0xC, sp + 0xB0, r, 0x01E0_0F98);
    node(fx, D + 0x10, sp + 0xC0, r, 0x01E0_0FB0);
    for k in 0..3u32 {
        let i = first_free(fx, POOL, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x80, 0, 0xFF);
        sprite(fx, b, k, [0x01E0_1014, 0x01E0_1044, 0x01E0_105C, 0x01E0_1080], 0x1F);
        let x = fx.rand_from(k, 0x01E0_109C);
        fx.mem.set8(b + 0xF, ((x >> 4) & 0x3F) + 0x50);
        fx.mem.write_u32(b + 0x10, 0x42A0_0000);
        let x = fx.rand_from(k, 0x01E0_10C0) >> 4;
        let f = ee::div(ee::from_i32((x % 4) * 200 - 0x190), 100.0);
        fx.mem.setf(b + 0x14, ee::add(10.0, f));
        fx.mem.write_u32(b + 0x1C, 0);
        fx.mem.write_u32(b + 0x18, 0);
        for c in [0u32, 4, 8] {
            let a = fx.mem.f(sp + 0xB0 + c);
            let d = ee::sub(fx.mem.f(sp + 0xC0 + c), a);
            fx.mem.setf(sp + 0xD0 + c, ee::add(a, ee::mul(0.5, d)));
        }
        let third = f32::from_bits(0x3E99_999A);
        let f = offset(fx, k, 0x01E0_1194, 0x28, 0x14);
        let d = ee::sub(fx.mem.f(sp + 0xD0), fx.mem.f(sp + 0xA0));
        fx.mem.setf(b + 0x2C, ee::add(ee::mul(third, d), f));
        let f = offset(fx, k, 0x01E0_1200, 0x28, 0x14);
        fx.mem.setf(b + 0x30, f);
        let f = offset(fx, k, 0x01E0_124C, 0x28, 0x14);
        let d = ee::sub(fx.mem.f(sp + 0xD8), fx.mem.f(sp + 0xA8));
        fx.mem.setf(b + 0x34, ee::add(ee::mul(third, d), f));
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
            fx.mem.write_u32(b + 0x20 + 4 * c, w);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(b + o, 0);
        }
    }
}

/// Updates 2..27 [F 0x1E012F0]: six from node +0x10, falling sideways.
fn fall(fx: &mut Fx, sp: u32, r: &Regs) {
    node(fx, D + 0x10, sp + 0xA0, r, 0x01E0_1320);
    for k in 0..6u32 {
        let i = first_free(fx, POOL, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, 0x14, 0, 0xFF);
        sprite(fx, b, k, [0x01E0_1384, 0x01E0_13B4, 0x01E0_13CC, 0x01E0_13F0], 0x3F);
        fx.mem.write_u8(b + 0xF, 0x80);
        fx.mem.write_u32(b + 0x10, 0x4120_0000);
        fx.mem.write_u32(b + 0x14, 0x4080_0000);
        fx.mem.write_u32(b + 0x1C, 0);
        fx.mem.write_u32(b + 0x18, 0);
        let v = if k & 1 != 0 {
            let f = offset(fx, k, 0x01E0_1438, 0xA, 5);
            ee::add(f32::from_bits(0xBE4C_CCCD), f)
        } else {
            let f = offset(fx, k, 0x01E0_149C, 0xA, 5);
            ee::add(f32::from_bits(0x3E4C_CCCD), f)
        };
        fx.mem.setf(b + 0x2C, v);
        let f = offset(fx, k, 0x01E0_14F8, 0xA, 5);
        fx.mem.setf(b + 0x30, f);
        fx.mem.write_u32(b + 0x3C, 0x3C23_D70A);
        fx.mem.write_u32(b + 0x34, 0xBF4C_CCCD);
        fx.mem.write_u32(b + 0x40, 0x3D4C_CCCD);
        let w = fx.mem.read_u32(sp + 0xA0);
        fx.mem.write_u32(b + 0x20, w);
        let w = fx.mem.read_u32(sp + 0xA4);
        fx.mem.write_u32(b + 0x24, w);
        let v = ee::add(1.5, fx.mem.f(sp + 0xA8));
        fx.mem.setf(b + 0x28, v);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            for o in [0x34u32, 0x40] {
                let v = ee::mul(fx.mem.f(b + o), -1.0);
                fx.mem.setf(b + o, v);
            }
            let v = ee::sub(fx.mem.f(sp + 0xA8), 1.5);
            fx.mem.setf(b + 0x28, v);
        }
    }
}

/// Updates 40..71 [F 0x1E015E8]: three from node +0x14, thrown out.
fn throw(fx: &mut Fx, sp: u32, r: &Regs) {
    node(fx, D + 0x14, sp + 0xA0, r, 0x01E0_1618);
    for k in 0..3u32 {
        let i = first_free(fx, POOL, 0x44, 0x1F4);
        if i < 0 {
            continue;
        }
        let f = steps(fx, k, 0x01E0_164C, 6, 0x258);
        let speed = ee::add(20.0, f);
        fx.fix_residue(speed, 0x01E0_16B4);
        let count = ee::fixsfsi(speed) & 0xFF;
        let b = POOL + 0x44 * i as u32;
        set_counter4(fx, b, count, 0, 0xFF);
        fx.mem.write_u8(b + 4, 0);
        fx.mem.write_u8(b + 8, 0);
        fx.mem.write_u8(b + 0xB, 1);
        let x = fx.rand_from(k, 0x01E0_16F8);
        fx.mem.set8(b + 9, (x >> 4) & 3);
        fx.mem.write_u8(b + 0xA, 5);
        let x = fx.rand_from(k, 0x01E0_1718);
        fx.mem.set8(b + 0xE, ((x >> 4) & 0x1F) + 0x30);
        fx.mem.write_u8(b + 0xD, 0x10);
        let x = fx.rand_from(k, 0x01E0_173C);
        fx.mem.set8(b + 0xC, ((x >> 4) & 0x1F) + 0x30);
        let x = fx.rand_from(k, 0x01E0_1758);
        fx.mem.set8(b + 0xF, ((x >> 4) & 0x3F) + 0x50);
        fx.mem.write_u32(b + 0x10, 0x42C8_0000);
        fx.mem.write_u32(b + 0x14, 0x4100_0000);
        fx.mem.write_u32(b + 0x1C, 0);
        fx.mem.write_u32(b + 0x18, 0);
        let f = steps(fx, k, 0x01E0_178C, 5, 0x1F4);
        fx.mem.setf(sp + 0xB0, f);
        let x = fx.rand_from(k, 0x01E0_17EC) >> 4;
        let f = ee::div(ee::from_i32((x % 2) * 200 - 0xC8), 100.0);
        fx.mem.setf(sp + 0xB4, ee::add(2.0, f));
        let f = steps(fx, k, 0x01E0_185C, 3, 0x12C);
        fx.mem.setf(sp + 0xB8, ee::add(3.0, f));
        if fx.mem.read_u32(D + 0x30) == 0x8000 {
            let v = ee::mul(fx.mem.f(sp + 0xB8), -1.0);
            fx.mem.setf(sp + 0xB8, v);
        }
        for c in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(sp + 0xA0 + c), fx.mem.f(sp + 0xB0 + c));
            fx.mem.setf(b + 0x20 + c, v);
        }
        for c in [0u32, 4, 8] {
            let v = ee::div(fx.mem.f(sp + 0xB0 + c), speed);
            fx.mem.setf(b + 0x2C + c, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(b + o, 0);
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let mut r = Regs { s0: sx(param), s1: 0 };
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xF0, r.s0, 0x01E0_0C50);
    match fx.mem.u16(D) {
        0x1A4 => return 8,
        0x140 | 0x10C | 0xB4 => burst(fx, sp, &mut r),
        _ => {}
    }
    if (0x58..0x109).contains(&fx.mem.u16(D)) {
        stream(fx, sp, &r);
        r.s0 = 3;
    }
    if (2..0x1C).contains(&fx.mem.u16(D)) {
        fall(fx, sp, &r);
        r.s0 = 6;
    }
    if (0x28..0x48).contains(&fx.mem.u16(D)) {
        throw(fx, sp, &r);
    }
    blib::sprite::bounce(fx, POOL, OT, 0x1F4);
    let n = fx.mem.u16(D);
    if (0x9B..0x147).contains(&n) {
        6
    } else if n == 0x147 {
        2
    } else {
        0
    }
}
