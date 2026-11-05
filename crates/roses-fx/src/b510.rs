//! BMODULE b510 (card 660), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04D40, data area a0 = 0x1ED0000, 0x56C0 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then offsets. Init sets the spark-emitter group (+0xA4,
//! `spawn::emitters` / `sprite::sparks`: update lists 0x1E05758 /
//! 0x1E05748 at nodes 0x2A / 0x24, late window 0x6B..0x70). Update 2 node
//! 0x24 read (+0x55A4, 5 lower); 116 an arc (+0xFB8, 16 counts) between
//! nodes 0x1F / 0x1E; 121 at the target the fade (+0x47B8), two spikes
//! (+0x480C, the b381 pair), 180 bouncing sprites (+0x1298, delayed s /
//! 18), a flash (+0x47C0), three panels (+0x543C, `panel::panels`, turned
//! +-0x2000 about z) and a shake (+0x40). Returns 6 on updates 121..151, 2
//! on 152, 1 on 111, 8 on 214 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (0538257fe1fa),
//! `sparks` 0x1E003A0, `fan_ring` 0x1E00C70, `emitters` 0x1E00F40,
//! `flash_b` 0x1E01390, `drops` 0x1E01E00, `fade` 0x1E02760, `arcs_b`
//! 0x1E02C30, `spikes` 0x1E03D50, `panels` 0x1E04830 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const ARCS: u32 = D + 0xFB8;
const SPRITES: u32 = D + 0x1298;
const FADE: u32 = D + 0x47B8;
const FLASH: u32 = D + 0x47C0;
const SPIKES: u32 = D + 0x480C;
const PANELS: u32 = D + 0x543C;
const BYTES: u32 = D + 0x55B4;

/// The entry 0x1E04D40 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x56C0).
    fx.bzero(D, 0x56C0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..4 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x24, 1), (0x10, 0x2A, 0), (0x14, 0x1F, 0), (0x18, 0x1E, 0), (0x1C, 1, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The emitter group (+0xA4).
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xCC, b);
    fx.mem.write_u8(D + 0xD7, b);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_5758);
    fx.mem.write_u32(D + 0xB0, 0x01E0_5748);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.write_u16(D + 0xD8, 0x6B);
    fx.mem.write_u16(D + 0xDA, 0x70);
    fx.mem.write_u32(D + 0xDC, 0x4334_0000);
    fx.mem.write_u32(D + 0xE0, 0x42F0_0000);
    fx.mem.write_u32(D + 0xE4, 0x40E0_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xD0);
    // The entry's s0 = D, s1 = 4 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4EDC, [Some(D), Some(4), Some(param as u32)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// Update 116 [F 0x1E04F68]: the arc between the nodes D[0x14] / D[0x18].
fn arc(fx: &mut Fx) {
    set_counter4(fx, ARCS, 0x10, 0, 0xFF);
    fx.mem.write_u8(ARCS + 4, 1);
    fx.mem.write_u32(ARCS + 8, D + 0x1018);
    fx.mem.write_u8(ARCS + 0x30, 0x32);
    fx.mem.write_u8(ARCS + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(ARCS + 0x32, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(ARCS + 0x33, b);
    fx.mem.write_u32(ARCS + 0x38, 0x3F80_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xC0), (0x4A, 0), (0x49, 0x80), (0x48, 0)] {
        fx.mem.write_u8(ARCS + o, v);
    }
    let mut k = 0u32;
    while (k as i32) < fx.mem.u8(ARCS + 0x31) + 1 {
        fx.mem.write_u16(ARCS + 0xC + 2 * k, 0xFFFF);
        fx.mem.write_u16(ARCS + 0x1E + 2 * k, 0xFFFF);
        k += 1;
    }
}

/// Update 121 [F 0x1E05018]: the strike at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 8, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    for s3 in 0..2u32 {
        let r = SPIKES + 0x68 * s3;
        if s3 == 0 {
            set_counter4(fx, r, 0x20, 0, 0xFF);
            for (o, v) in [(4u32, 7u8), (0xA, 3), (0xC, 3)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
            fx.mem.write_u32(r + 0x1C, 0x42B4_0000);
            for (o, v) in [(0x23u32, 0x80u8), (0x27, 0x20), (8, 0x78)] {
                fx.mem.write_u8(r + o, v);
            }
        } else {
            set_counter4(fx, r, 0x1C, 0, 0xFF);
            for (o, v) in [(4u32, 5u8), (0xA, 1), (0xC, 5)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.write_u32(r + 0x14, 0x4000_0000);
            fx.mem.write_u32(r + 0x1C, 0x4220_0000);
            for (o, v) in [(0x23u32, 0u8), (0x27, 0x60), (8, 0xA0)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.write_u32(r + 0x58, 0x40);
            fx.mem.write_u32(r + 0x5C, 0x80);
            fx.mem.write_u32(r + 0x60, 0x100);
        }
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 0xB, 2);
        let x = rand4(fx, 0x01E0_5108);
        fx.mem.set8(r + 9, x);
        fx.mem.write_u32(r + 0x10, BYTES);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x50, r + 0x28);
    }
    // 180 sprites, delayed s / 18.
    for s3 in 0..0xB4i32 {
        let i = first_free(fx, SPRITES, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let count = ((rand4(fx, 0x01E0_5190) & 0xF) + 0x20) & 0xFF;
        let r = SPRITES + 0x44 * i as u32;
        let p = i64::from(0x38E3_8E39i32) * i64::from(s3);
        let hi = (p >> 32) as i32;
        fx.set_hilo(p as i32, hi);
        let delay = (hi >> 2) + ((s3 as u32) >> 31) as i32;
        set_counter4(fx, r, count, delay & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0x42u8), (8, 1), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = (rand4(fx, 0x01E0_51FC) & 3) + 2;
        fx.mem.set8(r + 9, v);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u32(r + 0x10, 0x42A0_0000);
        fx.mem.write_u32(r + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBD75_C28F);
        let y = fx.mem.read_u32(D + 0x55A8);
        fx.mem.write_u32(r + 0x1C, y);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let f = jit(fx, 0x01E0_5264, 0x96, 0x4B);
        fx.mem.setf(r + 0x2C, f);
        let f = ee::add(f32::from_bits(0x3F33_3333), jit(fx, 0x01E0_52A0, 0x64, 0x32));
        fx.mem.setf(r + 0x30, f);
        let f = jit(fx, 0x01E0_52E8, 0x96, 0x4B);
        fx.mem.setf(r + 0x34, f);
    }
    // The flash (+0x47C0).
    let a = FLASH;
    set_counter4(fx, a, 0x20, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0x10), (8, 0x80)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x14, 0);
    fx.mem.write_u32(a + 0x10, 0x43FA_0000);
    copy_v4(fx, sp + 0x50, a + 0x1C);
    // Three panels (+0x543C, 0x3C apart).
    for s3 in 0..3i32 {
        let i = first_free(fx, PANELS, 0x3C, 6);
        if i < 0 {
            continue;
        }
        let r = PANELS + 0x3C * i as u32;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 2), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x50, r + 0x2C);
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x18, if s3 == 1 { 0x4040_0000 } else { 0x400C_CCCD });
        let x = ee::add(fx.mem.f(r + 0x2C), ee::add(-1.0, ee::from_i32(s3)));
        fx.mem.setf(r + 0x2C, x);
        let turn = if fx.mem.read_u32(D + 0x30) != 0x8000 { 0x2000 } else { -0x2000 };
        fx.mem.set32(r + 0x24, turn);
    }
    // The camera shake (+0x40) at the target.
    set_counter4(fx, D + 0x40, 0x20, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x3F80_0000);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3FB3_3333);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0x50, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(D), 0x01E0_4EFC);
    match fx.mem.u16(D) {
        0xD6 => return 8,
        0x79 => strike(fx, sp),
        0x74 => arc(fx),
        2 => {
            let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
            node_position_saving(fx, h, s, D + 0x55A4, u64::from(D), Some(u64::from(OT)), 0x01E0_4F48);
            let y = ee::sub(fx.mem.f(D + 0x55A8), 5.0);
            fx.mem.setf(D + 0x55A8, y);
        }
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5564, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5570, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    blib::spawn::emitters(fx, 0x01E0_0F40, D + 0xA4, n);
    blib::sprite::sparks(fx, D + 0xE8, OT, 0x20);
    blib::arc::arcs_b(fx, 0x01E0_2C30, ARCS, OT, 1);
    blib::star::flash_b(fx, 0x01E0_1390, FLASH, OT, 1);
    blib::spike::spikes(fx, 0x01E0_3D50, SPIKES, OT, 0x1E);
    blib::drop::drops(fx, SPRITES, OT, 0xC8);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the runner's s6 (0) at +0x70 and ra at +0x80.
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_55D8)]);
    blib::panel::panels(fx, 0x01E0_4830, PANELS, OT, 6);
    let n = fx.mem.u16(D);
    if n < 0x79 {
        if n == 0x6F {
            1
        } else {
            0
        }
    } else if n < 0x98 {
        6
    } else if n == 0x98 {
        2
    } else {
        0
    }
}
