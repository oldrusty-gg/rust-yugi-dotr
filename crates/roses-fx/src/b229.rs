//! BMODULE b229 (card 460), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03C60, data area a0 = 0x1ED0000, 0x8A50 bytes)
//! [F]: s0 =
//! D, s1 = the OT (0x206E40), s2 the parameter then offsets. Update 36
//! 200 bouncing sprites (+0x470, 0x44 apart) thrown from node 0x2A; 55 the
//! attacker's streaks (+0x158, `radiate::streaks_b`, 48 counts, from node
//! 0x36; node 0x2A from update 86), their far alpha growing by 3 to 0x80;
//! on updates 33..95 ten more sprites a update from node 0x2A; 95 at the
//! target the fade (+0x8940), four bands (+0x190, 0x5C apart), spikes
//! (+0xA4), a flash (+0x10C), 120 sprites and a shake (+0x40). The stage
//! light pulsed by 0x1031C0 (+0xA0). Returns 6 on updates 95..123, 2 on
//! 92, 1 on 85, 8 on 150 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (0538257fe1fa),
//! `spikes` 0x1E003A0, `flash_b` 0x1E00E80, `streaks_b` 0x1E018F0,
//! `fade` 0x1E026F0 (58d5c5a9444d), `drops` 0x1E02BC0, `bands_b`
//! 0x1E03520 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SPRITES: u32 = D + 0x470;
const STREAKS: u32 = D + 0x158;
const BYTES: u32 = D + 0x8948;

/// The entry 0x1E03C60 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x80);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x80);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x8A50).
    fx.bzero(D, 0x8A50);
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
    for (at, node, clear) in [(0xCu32, 0x14, 1), (0x10, 0x12, 0), (0x14, 0x2B, 0), (0x18, 1, 0), (0x1C, 0x2A, 0), (0x20, 0x36, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x90);
    // The entry's s0 = D, s1 = 4 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_3DA0, [Some(D), Some(4), Some(param as u32)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// ((rand >> 4) % m - `half`) / 100.
fn jit(fx: &mut Fx, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// The sprites thrown from node 0x2A (sp + 0x70) [F 0x1E03E20 / 0x1E046FC]:
/// `n` tries, counts `count`, delays rand & 7 when `delayed`, the ras of
/// the variant's rands / spread `m` (sp + 0x70.. jitter) and the up-speed
/// base / jitter.
struct Throw {
    n: i32,
    count: i32,
    delayed: bool,
    ras: [u32; 11],
    spread: (i32, i32),
    rise: (u32, i32, i32),
    lift: f32,
}

fn throw(fx: &mut Fx, sp: u32, t: &Throw) {
    let ra = &t.ras;
    let mut s3 = 0i32;
    while s3 < t.n {
        let i = first_free(fx, SPRITES, 0x44, 0x1F4);
        if i < 0 {
            s3 += 1;
            continue;
        }
        let delay = if t.delayed { rand4(fx, ra[0]) & 7 } else { 0 };
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, t.count, delay, 0xFF);
        let v = rand4(fx, ra[1]);
        fx.mem.set8(r + 4, (v & 3) << 4);
        let k = s3 & 7;
        if k == 0 || k == 1 {
            fx.mem.write_u8(r + 8, 2);
            for o in [0xFu32, 0xC, 0xD, 0xE] {
                fx.mem.write_u8(r + o, 0x80);
            }
            let v = rand4(fx, ra[2]);
            let (_, m) = fx.div(v, 0x28);
            let f = ee::add(180.0, ee::div(ee::from_i32(200 * m - 4000), 100.0));
            fx.mem.setf(r + 0x10, f);
            fx.mem.write_u32(r + 0x14, 0);
        } else {
            fx.mem.write_u8(r + 8, 1);
            for (o, v) in [(0xEu32, 0x80u8), (0xD, 0xA0), (0xC, 0xC0), (0xF, 0x40)] {
                fx.mem.write_u8(r + o, v);
            }
            let v = rand4(fx, ra[3]);
            let (_, m) = fx.div(v, 0x28);
            let f = ee::add(40.0, ee::div(ee::from_i32(200 * m - 4000), 100.0));
            fx.mem.setf(r + 0x10, f);
            let v = rand4(fx, ra[4]);
            let mut b = v & 1;
            if v < 0 && b != 0 {
                b -= 2;
            }
            let f = ee::add(3.0, ee::div(ee::from_i32(200 * b - 200), 100.0));
            fx.mem.setf(r + 0x14, f);
        }
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xA, 0);
        let v = rand4(fx, ra[5]);
        fx.mem.set8(r + 9, v & 7);
        fx.mem.write_u32(r + 0x1C, 0);
        fx.mem.write_u32(r + 0x18, 0);
        let (m, half) = t.spread;
        let f = ee::add(fx.mem.f(sp + 0x70), jit(fx, ra[6], m, half));
        fx.mem.setf(r + 0x20, f);
        let base = ee::sub(fx.mem.f(sp + 0x74), t.lift);
        let (m2, half2) = if t.lift == 0.0 { (m, half) } else { (0x64, 0x32) };
        let f = ee::add(base, jit(fx, ra[7], m2, half2));
        fx.mem.setf(r + 0x24, f);
        let (m3, half3) = if t.lift == 0.0 { (m, half) } else { (0x12C, 0x96) };
        let f = ee::add(fx.mem.f(sp + 0x78), jit(fx, ra[8], m3, half3));
        fx.mem.setf(r + 0x28, f);
        let (base, m4, half4) = t.rise;
        let f = ee::add(f32::from_bits(base), jit(fx, ra[9], m4, half4));
        fx.mem.setf(r + 0x30, f);
        let f = jit(fx, ra[10], 0x3C, 0x1E);
        fx.mem.setf(r + 0x2C, f);
        s3 += 1;
    }
}

/// Update 95 [F 0x1E04208]: the strike at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let f = D + 0x8940;
    set_counter4(fx, f, 0xC, 0, 0xFF);
    fx.mem.write_u8(f + 4, 1);
    // Four bands (+0x190, 0x5C apart), delayed 2 apart.
    for s4 in 0..4u32 {
        let b = D + 0x190 + 0x5C * s4;
        set_counter4(fx, b, 0x20, (2 * s4) as i32 & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 1), (8, 3), (9, 2), (0xA, 6), (0xB, 0)] {
            fx.mem.write_u8(b + o, v);
        }
        fx.mem.write_u32(b + 0x10, 0x3F80_0000);
        fx.mem.write_u32(b + 0x14, 0);
        fx.mem.write_u32(b + 0x18, 0x4000_0000);
        fx.mem.write_u32(b + 0x1C, 0x3F80_0000);
        fx.mem.write_u32(b + 0x20, 0x3F00_0000);
        for (o, v) in [(0x28u32, 0x80u8), (0x29, 0x80), (0x2A, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0), (0x2B, 0x40)] {
            fx.mem.write_u8(b + o, v);
        }
        copy_v4(fx, sp + 0x60, b + 0x2C);
        fx.mem.set32(b + 0x3C, 0x4000);
    }
    // The spikes (+0xA4).
    let r = D + 0xA4;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4000_0000);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 4), (8, 0x8C)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = rand4(fx, 0x01E0_4358);
    fx.mem.set8(r + 9, x);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x23, 0x10), (0x27, 0x60)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, sp + 0x60, r + 0x28);
    fx.mem.write_u32(r + 0x58, 0x40);
    fx.mem.write_u32(r + 0x5C, 0x80);
    fx.mem.write_u32(r + 0x60, 0x100);
    // The flash (+0x10C).
    let a = D + 0x10C;
    set_counter4(fx, a, 0x20, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0xC0), (9, 0x40), (8, 0)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x14, 0);
    fx.mem.write_u32(a + 0x10, 0x4448_0000);
    copy_v4(fx, sp + 0x60, a + 0x1C);
    // 120 sprites at the target.
    let mut s3 = 0;
    while s3 < 0x78 {
        let i = first_free(fx, SPRITES, 0x44, 0x1F4);
        if i < 0 {
            s3 += 1;
            continue;
        }
        let delay = rand4(fx, 0x01E0_447C) & 7;
        let r = SPRITES + 0x44 * i as u32;
        set_counter4(fx, r, 0x24, delay, 0xFF);
        let v = rand4(fx, 0x01E0_44AC);
        fx.mem.set8(r + 4, (v & 3) << 4);
        fx.mem.write_u8(r + 8, 2);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xA, 0);
        let v = rand4(fx, 0x01E0_44D0);
        fx.mem.set8(r + 9, (v + 4) & 7);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let v = rand4(fx, 0x01E0_44F8);
        let (_, m) = fx.div(v, 0x1E);
        let f = ee::add(150.0, ee::div(ee::from_i32(200 * m - 3000), 100.0));
        fx.mem.setf(r + 0x10, f);
        fx.mem.write_u32(r + 0x14, 0x40A0_0000);
        fx.mem.write_u32(r + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(r + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let f = jit(fx, 0x01E0_4584, 0x96, 0x4B);
        fx.mem.setf(r + 0x2C, f);
        let f = ee::add(1.0, jit(fx, 0x01E0_45C0, 0x96, 0x4B));
        fx.mem.setf(r + 0x30, f);
        let f = jit(fx, 0x01E0_4604, 0x96, 0x4B);
        fx.mem.setf(r + 0x34, f);
        s3 += 1;
    }
    // The camera shake (+0x40) at the target.
    set_counter4(fx, D + 0x40, 0x20, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x3F80_0000);
    fx.mem.write_u32(D + 0x58, 0x3FC0_0000);
    fx.mem.write_u32(D + 0x5C, 0x4000_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_3DC0);
    match fx.mem.u16(D) {
        0x96 => return 8,
        0x5F => strike(fx, sp),
        0x56 => {
            let b = fx.mem.read_u8(D + 0x1C);
            fx.mem.write_u8(STREAKS + 6, b);
        }
        0x37 => {
            let s = STREAKS;
            set_counter4(fx, s, 0x30, 0, 0xFF);
            for (o, v) in [(4u32, 0x80u8), (9, 2), (0xA, 0xFF), (8, 0x10), (0xB, 8)] {
                fx.mem.write_u8(s + o, v);
            }
            let h = fx.mem.read_u32(D + 4);
            fx.mem.write_u32(s + 0xC, h);
            for (o, v) in [(0x16u32, 0x20u8), (0x15, 0x20), (0x14, 0x80), (0x17, 0x10), (0x12, 0x40), (0x11, 0x40), (0x10, 0xF0), (0x13, 0), (5, 1)] {
                fx.mem.write_u8(s + o, v);
            }
            let b = fx.mem.read_u8(D + 0x20);
            fx.mem.write_u8(s + 6, b);
        }
        0x24 => {
            let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C));
            node_position_saving(fx, h, s, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_3E18);
            let t = Throw {
                n: 0xC8,
                count: 0x18,
                delayed: true,
                ras: [0x01E0_3E40, 0x01E0_3E70, 0x01E0_3EC0, 0x01E0_3F40, 0x01E0_3F98, 0x01E0_4000, 0x01E0_4018, 0x01E0_4058, 0x01E0_40A4, 0x01E0_40E4, 0x01E0_412C],
                spread: (0x9C4, 0x4E2),
                rise: (0x3F66_6666, 0x64, 0x32),
                lift: 4.0,
            };
            throw(fx, sp, &t);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x21..0x60).contains(&n) {
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C));
        node_position_saving(fx, h, s, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_46F4);
        let t = Throw {
            n: 0xA,
            count: 0x1C,
            delayed: false,
            ras: [0, 0x01E0_473C, 0x01E0_478C, 0x01E0_480C, 0x01E0_4864, 0x01E0_48D0, 0x01E0_48E8, 0x01E0_4928, 0x01E0_4968, 0x01E0_49A8, 0x01E0_49F0],
            spread: (0x258, 0x12C),
            rise: (0x3F19_999A, 0x32, 0x19),
            lift: 0.0,
        };
        throw(fx, sp, &t);
    }
    if fx.mem.u8(STREAKS) > 0 {
        let v = fx.mem.u8(STREAKS + 0x13);
        if v < 0x80 {
            fx.mem.set8(STREAKS + 0x13, v + 3);
        }
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_4A60, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4A6C, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0x8940, OT);
    // fade's 0xF0 frame saves the runner's s6 (0) at +0x70 and ra at
    // +0x80: the streaks' first TEXFLUSH reads them as vertex 1's stale
    // XYZ [run].
    fx.saved(sp - 0xF0, &[(0x70, 0), (0x80, 0x01E0_4A7C)]);
    blib::spike::spikes(fx, 0x01E0_03A0, D + 0xA4, OT, 1);
    blib::star::flash_b(fx, 0x01E0_0E80, D + 0x10C, OT, 1);
    blib::radiate::streaks_b(fx, 0x01E0_18F0, STREAKS, OT, 1);
    blib::band::bands_b(fx, 0x01E0_3520, D + 0x190, OT, 8);
    blib::drop::drops(fx, SPRITES, OT, 0x1F4);
    let n = fx.mem.u16(D);
    if (0x5F..0x7C).contains(&n) {
        6
    } else if n == 0x5C {
        2
    } else if n == 0x55 {
        1
    } else {
        0
    }
}
