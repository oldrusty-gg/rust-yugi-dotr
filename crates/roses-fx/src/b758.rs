//! BMODULE b758 (card 86), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04D60, data area a0 = 0x1ED0000, 0x5D60 bytes)
//! [F]: s0 = D, s1 the parameter then pointers
//! / counters, s2 = the OT (0x206E40). Every call copies the 0x44-byte
//! rodata table 0x1E05FE0 to sp + 0xD0 (the domes' alpha wave, a byte per
//! word). Init: the handles (+4 / +8), the attacker's rotation (+0x2C),
//! textures (4, 0xF) and (5, 0..3), nodes 1 (cleared, +0xC) / 0x35 / 0x3E
//! / 0x55 (+0x10..+0x18), the light ramp (+0xA0: 2 / 0x1EE) and the random
//! bytes (+0x5C5C). Every update the target (sp + 0xA0), node +0x10 (+0x5C28)
//! and node +0x14 (sp + 0xB0), the point 1.2 of the way from the first to
//! the second (+0x5C38). Update 100: eight tiles (+0x5898, `sheet::tiles`,
//! delays 0..7, 1.0 + 0.2 k growing by 1.8) at node +0x10, the centre
//! +0x5C48 that point; updates 109..116 stop one tile growing each, 117..128
//! their alpha + 3. Update 160: a long flash (+0x1DCC, 400 wide), four domes
//! (+0x5AB8, `dome::domes`, delays 8..11) and a shake at node +0x18; the
//! flag +0x5C58 (to 397) for eight sprites (+0x2098, `drop::drops`) a
//! update thrown about the centre, a spike there, the flash / domes
//! following it (radii + 0.3 to 10, turned +-0x200 a update from 325 to
//! +-0x4000) and the centre rising 0.3 to 70 from 200. 360 the domes' spin
//! +-0x600, 392 the fade (+0xA4), 396 the centre to the target (40 nearer
//! by D+0x30) and the domes 40 wide, alpha 0x30, 397 the flash again (900
//! wide) at the target and the window (+0x5BF8, `window::frame_fade_b`,
//! 397..440). The domes' alpha follows the wave until 396. Updates
//! 140..389 eight sprites a update from node +0x18, 90..99 / 435..479 from
//! node +0x14. Returns 6 on 400..420, 2 on 421, 1 on 390, 8 on 500 (no
//! drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `fade`
//! 0x1E003A0, `flash_b` 0x1E00870 (twice; +0x190C never armed),
//! `spikes` 0x1E012E0, `bars_b` 0x1E01DC0 (+0x1E18 never armed), `drops`
//! 0x1E026C0, `bands_b` 0x1E03020 (+0x55B8 never armed), `tiles`
//! 0x1E03760, `domes` 0x1E03EA0, `frame_fade_b` 0x1E048E0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const FADE: u32 = D + 0xA4;
const SPIKES: u32 = D + 0xAC;
const FLASH_A: u32 = D + 0x190C;
const FLASH: u32 = D + 0x1DCC;
const BARS: u32 = D + 0x1E18;
const DROPS: u32 = D + 0x2098;
const BANDS: u32 = D + 0x55B8;
const TILES: u32 = D + 0x5898;
const DOMES: u32 = D + 0x5AB8;
const WINDOW: u32 = D + 0x5BF8;
const NODE: u32 = D + 0x5C28;
const AHEAD: u32 = D + 0x5C38;
const CENTRE: u32 = D + 0x5C48;
const FLAG: u32 = D + 0x5C58;
const BYTES: u32 = D + 0x5C5C;
/// The domes' alpha wave (rodata, 0x44 bytes).
const WAVE: u32 = 0x01E0_5FE0;

/// The entry 0x1E04D60 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x240);
    for k in 0..0x11u32 {
        let w = fx.mem.read_u32(WAVE + 4 * k);
        fx.mem.write_u32(sp + 0xD0 + 4 * k, w);
    }
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x240);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0x5D60).
    fx.bzero(D, 0x5D60);
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
    for (k, (node, clear)) in [(1, 1), (0x35, 0), (0x3E, 0), (0x55, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x1EE);
    fx.mem.write_u32(FLAG, 0);
    // The entry's s0 = D, s1 = 4 (the texture loop's), s2 = the OT.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4EB8, [Some(D), Some(4), Some(OT)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// 0x20C0B0(out, 0, rot, 0): the rotation alone, its 0x70 frame (the
/// caller's s0..s3 +0x20.., ra +0x60) left below the caller's sp [F; run:
/// s0's upper half is a later frame fade's stale vertex-3 z].
fn rot_into(fx: &mut Fx, out: u32, rot: u32, s: [u64; 4], ra: u32) {
    let r = [fx.mem.i32(rot), fx.mem.i32(rot + 4), fx.mem.i32(rot + 8)];
    let m = fx.trs(None, Some(r), None);
    fx.write_m4(out, &m);
    let f = fx.sp - 0x70;
    fx.saved(f, &[(0x20, s[0]), (0x30, s[1]), (0x40, s[2]), (0x50, s[3]), (0x60, u64::from(ra))]);
}

/// 0x20C540(out, m, v).
fn turn(fx: &mut Fx, out: u32, m: u32, v: u32) {
    let m = fx.read_m4(m);
    let x = fx.read_v4(v);
    let r = fx.transform(Some(&m), x);
    fx.write_v4(out, r);
}

fn copy4(fx: &mut Fx, from: u32, to: u32) {
    for k in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The long flash's fields (+0x1DCC): kind `k`, `size` wide.
fn flash(fx: &mut Fx, count: i32, k: u8, size: u32) {
    set_counter4(fx, FLASH, count, 0, 0xFF);
    fx.mem.write_u8(FLASH + 4, k);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x60), (9, 0x60), (8, 0), (7, 9)] {
        fx.mem.write_u8(FLASH + o, v);
    }
    fx.mem.write_u32(FLASH + 0x10, size);
    for o in [0x14u32, 0x44, 0x40, 0x3C] {
        fx.mem.write_u32(FLASH + o, 0);
    }
    fx.mem.write_u8(FLASH + 5, 0);
}

/// Update 100 [F 0x1E04FB4]: eight tiles at node +0x10.
fn tiles(fx: &mut Fx) {
    copy4(fx, AHEAD, CENTRE);
    for k in 0..8u32 {
        let t = TILES + 0x44 * k;
        set_counter4(fx, t, 0xFF, k as i32, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 3), (9, 5), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(t + o, v);
        }
        let s = ee::add(1.0, ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(k as i32)));
        fx.mem.setf(t + 0xC, s);
        fx.mem.write_u32(t + 0x10, 0x3FE6_6666);
        for o in [0x14u32, 0x15, 0x16] {
            fx.mem.write_u8(t + o, 0x80);
        }
        fx.mem.write_u8(t + 0x17, 0x10);
        fx.mem.write_u32(t + 0x1C, 0);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(NODE + 4 * c);
            fx.mem.write_u32(t + 0x38 + 4 * c, w);
        }
    }
}

/// Update 160 [F 0x1E05098]: the long flash, four domes and a shake.
fn domes(fx: &mut Fx) {
    fx.mem.set32(FLAG, 1);
    flash(fx, 0xFF, 0x83, 0x43C8_0000);
    for k in 0..4u32 {
        let d = DOMES + 0x50 * k;
        set_counter4(fx, d, 0xFF, (k + 8) as i32 & 0xFF, 0xFF);
        for (o, v) in [(0xAu32, 1u8), (7, 9), (8, 2), (9, 2)] {
            fx.mem.write_u8(d + o, v);
        }
        fx.mem.write_u32(d + 0xC, 0x40A0_0000);
        fx.mem.write_u32(d + 0x10, 0x3C23_D70A);
        fx.mem.write_u32(d + 0x14, 0);
        fx.mem.write_u32(d + 0x18, 0);
        for o in [0x1Cu32, 0x1D, 0x1E] {
            fx.mem.write_u8(d + o, 0x80);
        }
        fx.mem.write_u8(d + 0x1F, 0);
    }
    set_counter4(fx, SHAKE, 0xDE, 0, 0xFF);
    fx.mem.write_u8(SHAKE + 5, 1);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(SHAKE + 6, b);
    fx.mem.write_u8(SHAKE + 4, 4);
    fx.mem.write_u32(SHAKE + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(SHAKE + 0x18, 0x3F99_999A);
    fx.mem.write_u32(SHAKE + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(SHAKE + 0x30, 0);
    fx.mem.write_u32(SHAKE + 0x34, 0x4220_0000);
    fx.mem.write_u32(SHAKE + 0x50, 0x40C0_0000);
}

/// Update 396 [F 0x1E0524C]: the centre to the target, the domes 40 wide.
fn gather(fx: &mut Fx, sp: u32) {
    copy4(fx, sp + 0xA0, CENTRE);
    for k in 0..4u32 {
        let d = DOMES + 0x50 * k;
        fx.mem.write_u32(d + 0x14, 0x4220_0000);
        fx.mem.write_u8(d + 0x1F, 0x30);
    }
    let z = fx.mem.f(CENTRE + 8);
    let z = if fx.mem.read_u32(D + 0x30) == 0x8000 { ee::sub(z, 40.0) } else { ee::add(z, 40.0) };
    fx.mem.setf(CENTRE + 8, z);
}

/// Update 397 [F 0x1E052DC]: the flash at the target and the window.
fn window(fx: &mut Fx, sp: u32) {
    flash(fx, 0x10, 0x81, 0x4461_0000);
    fx.mem.write_u32(FLAG, 0);
    copy4(fx, sp + 0xA0, FLASH + 0x1C);
    let n = fx.mem.u16(D);
    fx.mem.set16(WINDOW, n);
    fx.mem.set16(WINDOW + 2, 0x1B8);
    fx.mem.write_u8(WINDOW + 5, 7);
    fx.mem.write_u8(WINDOW + 6, 2);
    fx.mem.write_u32(WINDOW + 8, 0);
    for (o, v) in [(0x14u32, -8), (0xC, -8), (0x18, 8), (0x10, 8), (0x20, -8), (0x1C, -8), (0x28, 8), (0x24, 8)] {
        fx.mem.set32(WINDOW + o, v);
    }
    for o in [0x2Eu32, 0x2D, 0x2C] {
        fx.mem.write_u8(WINDOW + o, 0x80);
    }
}

/// A sprite's common fields (+0x2098 + 0x44 i) after its count.
fn sprite_fields(fx: &mut Fx, p: u32, k4: u8, len: u32, bend: u32, ra: u32) {
    fx.mem.write_u8(p + 4, k4);
    fx.mem.write_u8(p + 8, 0);
    fx.mem.write_u8(p + 0xB, 2);
    let v = (rand4(fx, ra) + 2) & 3;
    fx.mem.set8(p + 9, v);
    fx.mem.write_u8(p + 0xA, 0);
    for o in [0xFu32, 0xC, 0xD, 0xE] {
        fx.mem.write_u8(p + o, 0x80);
    }
    fx.mem.write_u32(p + 0x10, len);
    fx.mem.write_u32(p + 0x14, bend);
    fx.mem.write_u32(p + 0x1C, 0);
    fx.mem.write_u32(p + 0x18, 0);
}

/// Three random u16 angles at `rot` and their rotation into `m` (the
/// entry's s1 / s3 `s13`, 0x20C0B0's return `ra`).
fn rand_rotation(fx: &mut Fx, m: u32, rot: u32, ras: [u32; 3], s13: [u64; 2], ra: u32) {
    for (k, ra) in ras.into_iter().enumerate() {
        let v = rand4(fx, ra) & 0xFFFF;
        fx.mem.set32(rot + 4 * k as u32, v);
    }
    rot_into(fx, m, rot, [u64::from(D), s13[0], u64::from(OT), s13[1]], ra);
}

/// While the flag holds [F 0x1E054AC]: eight sprites thrown about the
/// centre, a spike there, the flash and the domes following it. Returns
/// the entry's s1.
fn follow(fx: &mut Fx, sp: u32) -> u64 {
    for s3 in 0..8u64 {
        let i = first_free(fx, DROPS, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let v = rand4(fx, 0x01E0_54D8);
        let (_, r) = fx.div(v, 3);
        let f20 = ee::add(12.0, ee::div(ee::from_i32(200 * r - 0x12C), 100.0));
        fx.fix_residue(f20, 0x01E0_5530);
        let count = ee::fixsfsi(f20) & 0xFF;
        let p = DROPS + 0x44 * i as u32;
        set_counter4(fx, p, count, 0, 0xFF);
        sprite_fields(fx, p, 0, 0x41A0_0000, 0, 0x01E0_5568);
        rand_rotation(fx, sp + 0x120, sp + 0x170, [0x01E0_55A8, 0x01E0_55B8, 0x01E0_55C8], [u64::from(sp + 0xC0), s3], 0x01E0_55E8);
        fx.mem.write_u32(sp + 0x168, 0);
        fx.mem.write_u32(sp + 0x160, 0);
        let v = rand4(fx, 0x01E0_55F4);
        let (_, r) = fx.div(v, 7);
        let y = ee::add(8.0, ee::div(ee::from_i32(200 * r - 0x2BC), 100.0));
        fx.mem.setf(sp + 0x164, y);
        turn(fx, sp + 0xC0, sp + 0x120, sp + 0x160);
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(CENTRE + 4 * c), fx.mem.f(sp + 0xC0 + 4 * c));
            fx.mem.setf(p + 0x20 + 4 * c, v);
        }
        for c in 0..3u32 {
            let v = ee::neg(ee::div(fx.mem.f(sp + 0xC0 + 4 * c), f20));
            fx.mem.setf(p + 0x2C + 4 * c, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(p + o, 0);
        }
    }
    let mut s1 = u64::from(sp + 0xC0);
    let i = first_free(fx, SPIKES, 0x68, 0x3C);
    if i >= 0 {
        s1 = u64::from(0x68 * i as u32);
        let q = SPIKES + 0x68 * i as u32;
        set_counter4(fx, q, 0xC, 0, 0xFF);
        fx.mem.write_u8(q + 4, 5);
        fx.mem.write_u32(q + 0x14, 0x4020_0000);
        fx.mem.write_u32(q + 0x1C, 0x41A0_0000);
        fx.mem.write_u8(q + 8, 8);
        fx.mem.write_u8(q + 5, 0);
        copy4(fx, CENTRE, q + 0x28);
        for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 3)] {
            fx.mem.write_u8(q + o, v);
        }
        let v = rand4(fx, 0x01E0_5784);
        fx.mem.set8(q + 9, v);
        fx.mem.write_u32(q + 0x10, BYTES);
        fx.mem.write_u32(q + 0x18, 0x3F80_0000);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x30)] {
            fx.mem.write_u8(q + o, v);
        }
        for (o, ra) in [(0x58u32, 0x01E0_57CC), (0x5C, 0x01E0_57E0), (0x60, 0x01E0_57F4)] {
            let v = rand4(fx, ra) & 0x7F;
            fx.mem.set32(q + o, v);
        }
    }
    copy4(fx, CENTRE, FLASH + 0x1C);
    for k in 0..4u32 {
        let d = DOMES + 0x50 * k;
        let r = fx.mem.f(d + 0x14);
        if r <= 10.0 {
            fx.mem.setf(d + 0x14, ee::add(r, f32::from_bits(0x3E99_999A)));
        }
        if fx.mem.u16(D) >= 0x145 {
            let a = fx.mem.i32(d + 0x30);
            if fx.mem.read_u32(D + 0x30) == 0x8000 {
                if a < 0x4000 {
                    fx.mem.set32(d + 0x30, a + 0x200);
                }
            } else if a >= -0x3FFF {
                fx.mem.set32(d + 0x30, a - 0x200);
            }
        }
        copy4(fx, CENTRE, d + 0x20);
    }
    if fx.mem.u16(D) >= 0xC8 {
        let y = fx.mem.f(CENTRE + 4);
        if y < 70.0 {
            fx.mem.setf(CENTRE + 4, ee::add(y, f32::from_bits(0x3E99_999A)));
        }
    }
    s1
}

/// Eight sprites a update from node D[`slot`] (sp + 0xB0): `base`
/// counts +- 1; `thrown` outwards from the node (else inwards, placed at
/// it). Returns the entry's s1.
#[allow(clippy::too_many_arguments)]
fn node_sprites(fx: &mut Fx, sp: u32, slot: u32, base: f32, div: f32, thrown: bool, m: u32, ras: &[u32; 11]) -> Option<u64> {
    let mut s1 = None;
    for _ in 0..8 {
        let i = first_free(fx, DROPS, 0x44, 0x96);
        s1 = Some(i as i64 as u64);
        if i < 0 {
            continue;
        }
        let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + slot));
        node_position_saving(fx, h, s, sp + 0xB0, u64::from(D), Some(i as i64 as u64), ras[0]);
        let v = rand4(fx, ras[1]);
        let (_, r) = fx.div(v, 2);
        let f20 = ee::add(base, ee::div(ee::from_i32(200 * r - 0xC8), 100.0));
        fx.fix_residue(f20, ras[2]);
        let count = ee::fixsfsi(f20) & 0xFF;
        let p = DROPS + 0x44 * i as u32;
        set_counter4(fx, p, count, 0, 0xFF);
        sprite_fields(fx, p, 0x41, 0x4170_0000, 0xBF00_0000, ras[3]);
        rand_rotation(fx, m, m + 0x50, [ras[4], ras[5], ras[6]], [i as i64 as u64, u64::from(p - 0x2098)], ras[9]);
        fx.mem.write_u32(m + 0x48, 0);
        fx.mem.write_u32(m + 0x40, 0);
        let r1 = rand4(fx, ras[7]) & 0xFF;
        s1 = Some(r1 as u64);
        let r2 = rand4(fx, ras[8]) & 0xFF;
        fx.mem.setf(m + 0x44, ee::div(ee::from_i32(r1 - r2), div));
        turn(fx, sp + 0xC0, m, m + 0x40);
        for c in 0..3u32 {
            let v = if thrown { ee::add(fx.mem.f(sp + 0xB0 + 4 * c), fx.mem.f(sp + 0xC0 + 4 * c)) } else { fx.mem.f(sp + 0xB0 + 4 * c) };
            fx.mem.setf(p + 0x20 + 4 * c, v);
        }
        for c in 0..3u32 {
            let v = ee::div(fx.mem.f(sp + 0xC0 + 4 * c), f20);
            fx.mem.setf(p + 0x2C + 4 * c, if thrown { ee::neg(v) } else { v });
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(p + o, 0);
        }
    }
    s1
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let s1p = param as i64 as u64;
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xA0, u64::from(D), 0x01E0_4ED8);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, s, NODE, u64::from(D), Some(s1p), 0x01E0_4EE8);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14));
    node_position_saving(fx, h, s, sp + 0xB0, u64::from(D), Some(s1p), 0x01E0_4EF8);
    for c in 0..3u32 {
        let a = fx.mem.f(NODE + 4 * c);
        let v = ee::add(a, ee::mul(f32::from_bits(0x3F99_999A), ee::sub(fx.mem.f(sp + 0xB0 + 4 * c), a)));
        fx.mem.setf(AHEAD + 4 * c, v);
    }
    let mut s1 = s1p;
    match fx.mem.u16(D) {
        0x1F4 => return 8,
        0x18D => window(fx, sp),
        0x18C => gather(fx, sp),
        0x188 => {
            set_counter4(fx, FADE, 0x10, 0, 0xFF);
            fx.mem.write_u8(FADE + 4, 1);
        }
        0x168 => {
            let v = if fx.mem.read_u32(D + 0x30) == 0x8000 { 0x600 } else { -0x600 };
            for k in 0..4u32 {
                fx.mem.set32(DOMES + 0x50 * k + 0x48, v);
            }
        }
        0xA0 => {
            domes(fx);
            s1 = u64::from(D + 0x140);
        }
        0x64 => {
            tiles(fx);
            s1 = u64::from(D + 0x220);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x6D..0x75).contains(&n) {
        fx.mem.write_u8(TILES + 0x44 * (n - 0x6D) as u32 + 4, 4);
    }
    let n = fx.mem.u16(D);
    if (0x75..0x81).contains(&n) {
        for k in 0..8u32 {
            let a = TILES + 0x44 * k + 0x17;
            let v = fx.mem.read_u8(a).wrapping_add(3);
            fx.mem.write_u8(a, v);
        }
    }
    if fx.mem.read_u8(DOMES) != 0 && fx.mem.u16(D) < 0x18C {
        for k in 0..4u32 {
            let n = fx.mem.u16(D);
            let w = fx.mem.read_u8(sp + 0xD0 + 4 * ((n - 0xAA) & 0xF) as u32);
            fx.mem.write_u8(DOMES + 0x50 * k + 0x1F, (w << 1).wrapping_add(0x10));
        }
    }
    if fx.mem.i32(FLAG) != 0 {
        s1 = follow(fx, sp);
    }
    let n = fx.mem.u16(D);
    if (0x8C..0x186).contains(&n) {
        if let Some(v) = node_sprites(fx, sp, 0x18, 12.0, 50.0, true, sp + 0x180, &[0x01E0_5978, 0x01E0_5980, 0x01E0_59E0, 0x01E0_5A1C, 0x01E0_5A60, 0x01E0_5A70, 0x01E0_5A80, 0x01E0_5AAC, 0x01E0_5AB8, 0x01E0_5AA0, 0]) {
            s1 = v;
        }
    }
    let n = fx.mem.u16(D);
    if (0x5A..0x64).contains(&n) || (0x1B3..0x1E0).contains(&n) {
        if let Some(v) = node_sprites(fx, sp, 0x14, 14.0, 40.0, false, sp + 0x1E0, &[0x01E0_5BE8, 0x01E0_5BF0, 0x01E0_5C50, 0x01E0_5C8C, 0x01E0_5CD0, 0x01E0_5CE0, 0x01E0_5CF0, 0x01E0_5D1C, 0x01E0_5D28, 0x01E0_5D10, 0]) {
            s1 = v;
        }
    }
    shake_saving(fx, SHAKE, u64::from(D), 0x01E0_5DCC, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5DD8, [Some(u64::from(D)), Some(s1)]);
    blib::fade::fade(fx, FADE, OT);
    let (_, flash_a_a3) = blib::star::flash_b_a3(fx, 0x01E0_0870, FLASH_A, OT, 0x10);
    let (_, flash_a3) = blib::star::flash_b_a3(fx, 0x01E0_0870, FLASH, OT, 1);
    let (_, spike_a3) = blib::spike::spikes_a3(fx, 0x01E0_12E0, SPIKES, OT, 0x3C);
    let (_, drop_a3) = blib::drop::drops_a3(fx, DROPS, OT, 0xC8);
    blib::cross::bars_b(fx, 0x01E0_1DC0, BARS, OT, 8);
    blib::band::bands_b(fx, 0x01E0_3020, BANDS, OT, 8);
    blib::sheet::tiles(fx, 0x01E0_3760, TILES, OT, 8);
    let (_, dome_a3) = blib::dome::domes_a3(fx, 0x01E0_3EA0, DOMES, OT, 4);
    let n = fx.mem.u16(D);
    // The a3 the last drawing call left [run]: the domes', the drops',
    // the spikes', the flashes' (bars / bands / tiles never count in the
    // window, [H] not chained), else 0 from the dispatch's delay slot
    // (0x1E04F90; [H] the fade's not chained).
    let a3 = dome_a3.or(drop_a3).or(spike_a3).or(flash_a3).or(flash_a_a3).unwrap_or(0);
    blib::window::frame_fade_b(fx, 0x01E0_48E0, WINDOW, OT, n, a3);
    let n = fx.mem.u16(D);
    if (0x190..0x1A5).contains(&n) {
        6
    } else if n == 0x1A5 {
        2
    } else if n == 0x186 {
        1
    } else {
        0
    }
}
