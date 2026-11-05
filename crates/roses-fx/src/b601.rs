//! BMODULE b601 (card 600), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06660, data area a0 = 0x1ED0000, 0x4BA0 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then counters / pointers; compiled without scheduling (nop
//! delay slots). Update 2 a rod (+0x4740, `rod::rods_b`) between the
//! attacker's nodes +0x14 / +0x18, an arc (+0x430C, `arc::arcs_b`, pool
//! +0x436C) along them and a long shake; 56 the attacker's streaks
//! (+0x4A60, `radiate::streaks_b`); 66..80 two sprites a update (+0xA4,
//! `drop::drops`) from each of nodes +0xC / +0x10, 80..106 four a update
//! from node +0x1C; every fourth update 68..104 the attacker's object op
//! 0x1F83B0 (mode 0x18, a stack colour pair); 106 at the target the fade
//! (+0x4778), a spike (+0x4598), 200 sprites, four bands (+0x4780,
//! `band::bands_b`, delayed 0 / 2 / 4 / 6), a flash (+0x454C) and a shake.
//! Returns 6 on updates 106..134, 2 on 103, 1 on 96, 8 on 160 (no drawing
//! then) and at once for -2 / -3. The crossbars (+0x4600, `cross::bars_b`)
//! are drawn, never started.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `drops`
//! 0x1E003A0, `arcs_b` 0x1E00D00, `flash_b` 0x1E01E20, `spikes`
//! 0x1E02890, `bars_b` 0x1E03370, `rods_b` 0x1E03C70, `fade` 0x1E04C50,
//! `bands_b` 0x1E05120, `streaks_b` 0x1E05860 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const DROPS: u32 = D + 0xA4;
const ARC: u32 = D + 0x430C;
const FLASH: u32 = D + 0x454C;
const SPIKE: u32 = D + 0x4598;
const BARS: u32 = D + 0x4600;
const ROD: u32 = D + 0x4740;
const FADE: u32 = D + 0x4778;
const BANDS: u32 = D + 0x4780;
const STREAKS: u32 = D + 0x4A60;
const BYTES: u32 = D + 0x4A98;

/// The entry 0x1E06660 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x4BA0).
    fx.bzero(D, 0x4BA0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..3 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x10, 1), (0x10, 0x1D, 0), (0x14, 0x3B, 0), (0x18, 0x3C, 0), (0x1C, 1, 0), (0x20, 0x2F, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x9A);
    // The entry's s0 = D, s1 = 3 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_679C, [Some(D), Some(3), Some(param as u32)]);
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

/// Update 2 [F 0x1E067FC]: the rod and the arc between nodes +0x14 /
/// +0x18, a long shake at node +0x14.
fn start(fx: &mut Fx) {
    let r = ROD;
    set_counter4(fx, r, 0x6A, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let v = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(r + 8, v);
    let v = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(r + 9, v);
    fx.mem.write_u8(r + 4, 2);
    fx.mem.write_u32(r + 0x10, 0x3DCC_CCCD);
    fx.mem.write_u32(r + 0x14, 0x3D4C_CCCD);
    fx.mem.write_u32(r + 0xC, 0x42B4_0000);
    for (o, v) in [(0x18u32, 0x80u8), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0xC0), (0x1E, 0x10), (0x1D, 0x20), (0x1C, 0xC0), (0x1F, 0x10)] {
        fx.mem.write_u8(r + o, v);
    }
    let a = ARC;
    set_counter4(fx, a, 0x6A, 0, 0xFF);
    fx.mem.write_u8(a + 4, 1);
    fx.mem.write_u32(a + 8, D + 0x436C);
    fx.mem.write_u8(a + 0x30, 0x28);
    fx.mem.write_u8(a + 0x31, 8);
    let v = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(a + 0x32, v);
    let v = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(a + 0x33, v);
    fx.mem.write_u32(a + 0x38, 0x3F8C_CCCD);
    fx.mem.write_u8(a + 0x34, 0);
    fx.mem.write_u16(a + 0x36, 0);
    for (o, v) in [(0x47u32, 0x80u8), (0x44, 0x80), (0x45, 0x80), (0x46, 0x80), (0x4A, 0x10), (0x49, 0x20), (0x48, 0xC0)] {
        fx.mem.write_u8(a + o, v);
    }
    let len = fx.mem.u8(a + 0x31) as u32;
    for k in 0..=len {
        fx.mem.set16(a + 0x1E + 2 * k, -1);
        fx.mem.set16(a + 0xC + 2 * k, -1);
    }
    set_counter4(fx, D + 0x40, 0x6A, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 1);
    let v = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0x46, v);
    fx.mem.write_u8(D + 0x44, 4);
    fx.mem.write_u32(D + 0x54, 0x3F80_0000);
    fx.mem.write_u32(D + 0x58, 0x3F80_0000);
    fx.mem.write_u32(D + 0x5C, 0x4000_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
}

/// Update 56 [F 0x1E06990]: the attacker's streaks.
fn streaks(fx: &mut Fx) {
    let s = STREAKS;
    set_counter4(fx, s, 0x12, 0, 0xFF);
    for (o, v) in [(4u32, 0x80u8), (9, 2), (0xA, 0xFF), (8, 0xA), (0xB, 6)] {
        fx.mem.write_u8(s + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(s + 0xC, h);
    for (o, v) in [(0x16u32, 0x80u8), (0x15, 0), (0x14, 0), (0x17, 0x10), (0x12, 0x80), (0x11, 0), (0x10, 0), (0x13, 0x80), (5, 1)] {
        fx.mem.write_u8(s + o, v);
    }
    let v = fx.mem.read_u8(D + 0x20);
    fx.mem.write_u8(s + 6, v);
}

/// Update 106 [F 0x1E06A0C]: at the target (sp + 0x60) the fade, a
/// spike, 200 sprites, four bands, a flash and a shake.
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0xC, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    let q = SPIKE;
    set_counter4(fx, q, 0x1E, 0, 0xFF);
    fx.mem.write_u8(q + 4, 5);
    fx.mem.write_u32(q + 0x14, 0x4020_0000);
    fx.mem.write_u32(q + 0x1C, 0x4220_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(q + o, v);
    }
    let x = rand4(fx, 0x01E0_6A78);
    fx.mem.set8(q + 9, x);
    fx.mem.write_u32(q + 0x10, BYTES);
    fx.mem.write_u32(q + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(q + o, v);
    }
    copy_v4(fx, sp + 0x60, q + 0x28);
    fx.mem.write_u32(q + 0x58, 0x40);
    fx.mem.write_u32(q + 0x5C, 0x80);
    fx.mem.write_u32(q + 0x60, 0x100);
    for _ in 0..0xC8 {
        let i = first_free(fx, DROPS, 0x44, 0xFA);
        if i < 0 {
            continue;
        }
        let delay = rand4(fx, 0x01E0_6B18) & 0xF;
        let q = DROPS + 0x44 * i as u32;
        set_counter4(fx, q, 0x20, delay, 0xFF);
        for (o, v) in [(4u32, 0u8), (8, 1), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(q + o, v);
        }
        let v = rand4(fx, 0x01E0_6B5C) & 7;
        fx.mem.set8(q + 9, v);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(q + o, 0x80);
        }
        let v = rand4(fx, 0x01E0_6B80);
        let (_, r) = fx.div(v, 0x14);
        let f = ee::add(120.0, ee::div(ee::from_i32(200 * r - 0x7D0), 100.0));
        fx.mem.setf(q + 0x10, f);
        fx.mem.write_u32(q + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(q + 0x18, 0xBDB8_51EC);
        fx.mem.write_u32(q + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(q + 0x20 + 4 * c, w);
        }
        let f = jit(fx, 0x01E0_6C10, 0xE6, 0x73);
        fx.mem.setf(q + 0x2C, f);
        let f = ee::add(f32::from_bits(0x3F4C_CCCD), jit(fx, 0x01E0_6C4C, 0xE6, 0x73));
        fx.mem.setf(q + 0x30, f);
        let f = jit(fx, 0x01E0_6C94, 0xE6, 0x73);
        fx.mem.setf(q + 0x34, f);
    }
    for s4 in 0..4u32 {
        let b = BANDS + 0x5C * s4;
        set_counter4(fx, b, 0x20, (2 * s4) as i32, 0xFF);
        for (o, v) in [(4u32, 0u8), (7, 0x24), (0xC, 2), (8, 1), (9, 2), (0xA, 0), (0xB, 4)] {
            fx.mem.write_u8(b + o, v);
        }
        fx.mem.write_u32(b + 0x10, 0x4000_0000);
        fx.mem.write_u32(b + 0x14, 0);
        fx.mem.write_u32(b + 0x18, 0x3F80_0000);
        fx.mem.write_u32(b + 0x1C, 0x3F66_6666);
        for o in [0x2Bu32, 0x27, 0x28, 0x29, 0x2A, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x80);
        }
        copy_v4(fx, sp + 0x60, b + 0x2C);
        fx.mem.set32(b + 0x3C, 0x4000);
    }
    let f = FLASH;
    set_counter4(fx, f, 0x20, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x40), (9, 0x10), (8, 0xC0)] {
        fx.mem.write_u8(f + o, v);
    }
    fx.mem.write_u32(f + 0x14, 0);
    fx.mem.write_u32(f + 0x10, 0x4448_0000);
    copy_v4(fx, sp + 0x60, f + 0x1C);
    set_counter4(fx, D + 0x40, 0x20, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
}

/// A sprite falling from node `slot` (sp + 0x70) jittered (rand returns
/// `ra`: the corner, the frame, x, z, vx, vz; x / z by `m` - `half`, vx /
/// vz by `mv` - `hv`; y lowered by `dy`) [F 0x1E06ED4 / 0x1E070C8].
#[allow(clippy::too_many_arguments)]
fn sprite(fx: &mut Fx, sp: u32, slot: u32, ra: [u32; 7], (m, half): (i32, i32), (mv, hv): (i32, i32), dy: f32) {
    let i = first_free(fx, DROPS, 0x44, 0xFA);
    if i < 0 {
        return;
    }
    let q = DROPS + 0x44 * i as u32;
    set_counter4(fx, q, 0x24, 0, 0xFF);
    let v = (rand4(fx, ra[0]) & 3) << 4;
    fx.mem.set8(q + 4, v);
    fx.mem.write_u8(q + 8, 0);
    fx.mem.write_u8(q + 0xB, 1);
    fx.mem.write_u8(q + 0xA, 5);
    let v = rand4(fx, ra[1]) & 3;
    fx.mem.set8(q + 9, v);
    for o in [0xCu32, 0xD, 0xE] {
        fx.mem.write_u8(q + o, 0x80);
    }
    fx.mem.write_u8(q + 0xF, 0x60);
    fx.mem.write_u32(q + 0x10, 0x4316_0000);
    fx.mem.write_u32(q + 0x14, 0x4100_0000);
    fx.mem.write_u32(q + 0x18, 0xBCA3_D70A);
    fx.mem.write_u32(q + 0x1C, 0);
    let (h, n) = (fx.mem.i32(D + 4), fx.mem.i32(D + slot));
    node_position_saving(fx, h, n, sp + 0x70, u64::from(D), Some(u64::from(OT)), ra[2]);
    let x = ee::add(fx.mem.f(sp + 0x70), jit(fx, ra[3], m, half));
    fx.mem.setf(q + 0x20, x);
    let y = if dy == 0.0 { fx.mem.f(sp + 0x74) } else { ee::sub(fx.mem.f(sp + 0x74), dy) };
    fx.mem.setf(q + 0x24, y);
    let z = ee::add(fx.mem.f(sp + 0x78), jit(fx, ra[4], m, half));
    fx.mem.setf(q + 0x28, z);
    let f = jit(fx, ra[5], mv, hv);
    fx.mem.setf(q + 0x2C, f);
    fx.mem.write_u32(q + 0x30, 0x3ECC_CCCD);
    let f = jit(fx, ra[6], mv, hv);
    fx.mem.setf(q + 0x34, f);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_67BC);
    match fx.mem.u16(D) {
        0xA0 => return 8,
        0x6A => strike(fx, sp),
        0x38 => streaks(fx),
        2 => start(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x42..0x51).contains(&n) {
        for s4 in 0..2u32 {
            let ra = [0x01E0_6F18, 0x01E0_6F40, 0x01E0_6F94, 0x01E0_6F9C, 0x01E0_6FE0, 0x01E0_7020, 0x01E0_7068];
            sprite(fx, sp, 0xC + 4 * s4, ra, (0x78, 0x3C), (0x64, 0x32), 0.0);
        }
    }
    let n = fx.mem.u16(D);
    if (0x50..0x6B).contains(&n) {
        for _ in 0..4 {
            let ra = [0x01E0_710C, 0x01E0_7134, 0x01E0_7188, 0x01E0_7190, 0x01E0_71E0, 0x01E0_7220, 0x01E0_7268];
            sprite(fx, sp, 0x1C, ra, (0xDC, 0x6E), (0x96, 0x4B), 4.0);
        }
    }
    let n = fx.mem.u16(D);
    if (0x44..0x6B).contains(&n) && n & 3 == 0 {
        for (o, v) in [(0x8Au32, 0xF0u8), (0x8E, 0xF0), (0x88, 0x40), (0x8C, 0x40), (0x8D, 0x40), (0x89, 0x40), (0x8F, 0), (0x8B, 0x60)] {
            fx.mem.write_u8(sp + o, v);
        }
        let h = fx.mem.read_u32(D + 4);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h, 0x18, 0, sp + 0x88]);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_7324, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_7330, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::drop::drops(fx, DROPS, OT, 0xFA);
    blib::arc::arcs_b(fx, 0x01E0_0D00, ARC, OT, 1);
    blib::spike::spikes(fx, 0x01E0_2890, SPIKE, OT, 1);
    blib::cross::bars_b(fx, 0x01E0_3370, BARS, OT, 4);
    blib::star::flash_b(fx, 0x01E0_1E20, FLASH, OT, 1);
    // [H] f22 = the runner's (0): the entry never sets it.
    blib::rod::rods_b(fx, 0x01E0_3C70, ROD, OT, 1, 0.0);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the entry's s0 / s1 (D / the OT), the
    // runner's s5 / s6 (0) and ra: s6's slot is the low word of the
    // streaks' vertex 1 XYZ, their first TEXFLUSH's a0 [run]; the entry's
    // s2..s4 [H] left.
    fx.saved(sp - 0xF0, &[(0x10, u64::from(D)), (0x20, u64::from(OT)), (0x60, 0), (0x70, 0), (0x80, 0x01E0_739C)]);
    blib::band::bands_b(fx, 0x01E0_5120, BANDS, OT, 8);
    blib::radiate::streaks_b(fx, 0x01E0_5860, STREAKS, OT, 1);
    let n = fx.mem.u16(D);
    if (0x6A..0x87).contains(&n) {
        6
    } else if n == 0x67 {
        2
    } else if n == 0x60 {
        1
    } else {
        0
    }
}
