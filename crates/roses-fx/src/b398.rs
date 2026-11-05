//! BMODULE b398 (card 191), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05EB0, data area a0 = 0x1ED0000) [F] (helper B10,
//! 31 Oct 2026): s0 = D, s1 = the OT (0x206E40), s2 the parameter then the
//! spawn loops' counters. Update 2 a long shake (+0x40) and the frame fade
//! (+0x5EC, `window::frame_fade`, updates 2..63) with node 0x59 read
//! (+0x51C4, 5 lower); 64 two flashes (+0x3CA8, 0x4C apart) on node 0x6E
//! and the shake again; on even updates 68..148 a ribbon (+0x3D8C, 0x68
//! apart) from node 0x6E; 124 an arch (+0xAC, 0x54 apart, copied on every
//! update while it counts) from the attacker's point to the target; 150 at
//! the target the fade (+0xA4), three sheets (+0x4DCC), two flashes, 200
//! bouncing sprites (+0x648), four bars (+0x3B68), a ribbon and a shake.
//! The stage dimmed (0x1F7F10(0x30)) on updates 2..190, lit by 0x1031C0
//! (+0xA0). The streaks (+0x518C), the split frame quad (+0x61C, own
//! 0x1E05670) and the window sprite (+0x638, own 0x1E05D20) are stepped
//! but never set (no store to them [F]). Returns 6 on updates 150..186, 2
//! on 147, 1 on 140, 8 on 206 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `arches`
//! 0x1E00410, `fade` 0x1E00D00, `ribbon` 0x1E01240, `bars` 0x1E01DD0,
//! `flash` 0x1E027E0, `sheets` 0x1E03390, `bounce` 0x1E03900, `streaks`
//! 0x1E04370, `frame_fade` 0x1E05190 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{alpha_mode, first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x51D4;

/// The entry 0x1E05EB0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0x52E0).
    fx.bzero(D, 0x52E0);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x59, 0), (0x14, 0x6E, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xC8);
    // The entry's s0 = D, s1 = the OT, s2 = 3 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5FE4, [Some(D), Some(OT), Some(3)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// ((rand >> 4) % m - m / 2) / 100.
fn jitter(fx: &mut Fx, ra: u32, m: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - m / 2), 100.0)
}

/// The camera shake (+0x40), long (`hold`: delay 1 on node 0x59) or at
/// the target.
fn shake_long(fx: &mut Fx) {
    set_counter4(fx, D + 0x40, 0x84, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0x46, b);
    fx.mem.write_u8(D + 0x44, 2);
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
}

/// Update 150 [F 0x1E06400]: the strike at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0xA4, 0xC, 0, 0xFF);
    fx.mem.write_u8(D + 0xA8, 1);
    // Three sheets (+0x4DCC, 0x3C apart).
    for s2 in 0..3i32 {
        let r = D + 0x4DCC + 0x3C * s2 as u32;
        set_counter4(fx, r, 0x16, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 1), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x50, r + 0x2C);
        let dy = ee::add(-1.5, ee::mul(1.5, ee::from_i32(s2)));
        let y = ee::add(fx.mem.f(r + 0x30), dy);
        fx.mem.setf(r + 0x30, y);
        fx.mem.write_u32(r + 0x10, 0x4040_0000);
        fx.mem.write_u32(r + 0x14, 0x4000_0000);
        fx.mem.write_u32(r + 0x18, if s2 == 1 { 0x4080_0000 } else { 0x4040_0000 });
        fx.mem.set32(r + 0x24, 0x4000);
    }
    // Two flashes (+0x3CA8).
    for s3 in 0..2i32 {
        let r = D + 0x3CA8 + 0x4C * s3 as u32;
        set_counter4(fx, r, 0x16, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (7, 9)] {
            fx.mem.write_u8(r + o, v);
        }
        if s3 != 0 {
            fx.mem.write_u32(r + 0x10, 0x42C8_0000);
            fx.mem.write_u32(r + 0x14, 0x4270_0000);
            fx.mem.write_u32(r + 0x18, 0x41A0_0000);
        } else {
            let b = fx.mem.read_u8(r + 4);
            fx.mem.write_u8(r + 4, b | 1);
            fx.mem.write_u32(r + 0x10, 0x43FA_0000);
            fx.mem.write_u32(r + 0x18, 0);
            fx.mem.write_u32(r + 0x14, 0);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xA, 0x80), (9, 0x80), (8, 0x10), (0xF, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        for o in [0x44u32, 0x40, 0x3C] {
            fx.mem.write_u32(r + o, 0);
        }
        copy_v4(fx, sp + 0x50, r + 0x1C);
    }
    // 200 bouncing sprites (+0x648, 0x44 apart), delayed s2 % 15.
    for s2 in 0..0xC8i32 {
        let i = first_free(fx, D + 0x648, 0x44, 0xC8);
        if i < 0 {
            continue;
        }
        let count = ((rand4(fx, 0x01E0_6670) & 0xF) + 0x20) & 0xFF;
        let r = D + 0x648 + 0x44 * i as u32;
        let (_, m) = fx.div(s2, 0xF);
        set_counter4(fx, r, count, m & 0xFF, 0xFF);
        for (o, v) in [(4u32, 0x42u8), (8, 2), (0xB, 2), (0xA, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = (rand4(fx, 0x01E0_66DC) & 3) + 2;
        fx.mem.set8(r + 9, v);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u32(r + 0x10, 0x42A0_0000);
        fx.mem.write_u32(r + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(r + 0x18, 0xBD75_C28F);
        let w = fx.mem.read_u32(D + 0x51C8);
        fx.mem.write_u32(r + 0x1C, w);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x50 + 4 * c);
            fx.mem.write_u32(r + 0x20 + 4 * c, w);
        }
        let f = jitter(fx, 0x01E0_674C, 0xB4);
        fx.mem.setf(r + 0x2C, f);
        let f = jitter(fx, 0x01E0_6798, 0x64);
        fx.mem.setf(r + 0x30, ee::add(f32::from_bits(0x3F4C_CCCD), f));
        let f = jitter(fx, 0x01E0_67F4, 0xB4);
        fx.mem.setf(r + 0x34, f);
    }
    // Four bars (+0x3B68, 0x50 apart).
    for s2 in 0..4i32 {
        let r = D + 0x3B68 + 0x50 * s2 as u32;
        set_counter4(fx, r, 0x16, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u32(r + 0x10, if s2 == 2 { 0x44C8_0000 } else { 0x447A_0000 });
        fx.mem.write_u32(r + 0x14, 0x4248_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x40), (0xA, 0x80), (9, 0x40), (8, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.setf(r + 0x18, ee::mul(45.0, ee::from_i32(s2)));
        fx.mem.write_u32(r + 0x1C, 0x4080_0000);
        copy_v4(fx, sp + 0x50, r + 0x20);
    }
    // A ribbon (+0x3D8C; the slot not checked).
    let i = first_free(fx, D + 0x3D8C, 0x68, 0x28);
    let r = D.wrapping_add(0x3D8C).wrapping_add((0x68 * i) as u32);
    fx.mem.write_u8(r + 4, 5);
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    let b = fx.mem.read_u8(r + 4);
    fx.mem.write_u8(r + 4, b | 2);
    fx.mem.write_u32(r + 0x14, 0x3FCC_CCCD);
    fx.mem.write_u32(r + 0x1C, 0x42A0_0000);
    for (o, v) in [(0x23u32, 0x80u8), (0x27, 0x80), (5, 0), (0xA, 2), (0xB, 2), (0xC, 3), (8, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = rand4(fx, 0x01E0_69E0);
    fx.mem.set8(r + 9, x);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    copy_v4(fx, sp + 0x50, r + 0x28);
    // The camera shake (+0x40) at the target.
    set_counter4(fx, D + 0x40, 0x20, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x3FCC_CCCD);
    fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, D + 0x60, u64::from(D), 0x01E0_6AA0);
}

/// Update 124 [F 0x1E062CC]: the arch template (+0xAC) from the
/// attacker's point toward the target over 32 counts.
fn arch(fx: &mut Fx, sp: u32, s2: u64) {
    let a = D + 0xAC;
    set_counter4(fx, a, 0x20, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0xC0), (0xA, 0x80), (9, 0x80), (8, 0x10), (0xB, 0x10)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x10, 0x4110_0000);
    fx.mem.write_u32(a + 0x18, 0x3F00_0000);
    fx.mem.write_u32(a + 0x14, 0x4000_0000);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, a + 0x24);
    // 0x1F65D0's 0x50 frame (s0 = D, s1 = OT, s2, s3 (the runner's 0),
    // ra) [F].
    fx.saved(sp - 0x50, &[(0, u64::from(D)), (0x10, u64::from(OT)), (0x20, s2), (0x30, 0), (0x40, 0x01E0_634C)]);
    for c in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x50 + 4 * c), fx.mem.f(a + 0x24 + 4 * c)), 32.0);
        fx.mem.setf(a + 0x34 + 4 * c, v);
    }
    fx.mem.set32(a + 0x44, -0x4000);
    let z = ee::add(fx.mem.f(a + 0x2C), 0.0);
    fx.mem.setf(a + 0x2C, z);
    if fx.mem.read_u32(D + 0x30) == 0x8000 {
        return;
    }
    let f = ee::mul(ee::from_i32(fx.mem.i32(a + 0x44)), -1.0);
    fx.fix_residue(f, 0x01E0_63E0);
    fx.mem.set32(a + 0x44, ee::fixsfsi(f));
    let z = ee::sub(fx.mem.f(a + 0x2C), 0.0);
    fx.mem.setf(a + 0x2C, z);
}

/// 0x1E05670 (91db711f13fb, this record's own) [F]: `split(p, ot, n) ->
/// 1`: while the update `n` lies in [u16 +0, u16 +2], two quads of the
/// frame drawn (0x207EF0, filter 1; a 0x160 frame, the descriptor at sp +
/// 0xC8): the window's top (y 28672 .. 28672 + +8 x 16, V 0 .. +8 x 16)
/// moved in x by +0xC x 16, then the rest (to y 0x8C00, V .. 0x1C00) by
/// +0x10 x 16; U 0 .. 0x2800; colour +0x1A / +0x19 / +0x18, alpha 16 per
/// update for +5 updates in, 8 x (u8 +2 - n) in the last 2 x +5, else 16
/// x +5; blend +6 (0x101C80 / 0x207E20), flags | 0x800; z 0xFFFFFF on
/// vertex 4 and 2 (and 1); keyed by +0x14 (0x1A2C70). `a3`: the caller's
/// a3, reported with 0x207EF0's call. Never set in this record.
pub fn split(fx: &mut Fx, p: u32, ot: u32, n: i32, a3: u32) -> i32 {
    let sp = fx.enter(0x160);
    let s0 = n & 0xFFFF;
    if s0 < fx.mem.u16(p) || fx.mem.u16(p + 2) < s0 {
        fx.leave(0x160);
        return 1;
    }
    let d = sp + 0xC8;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    fx.frame_desc(d, 1, [n as u32, a3]);
    for k in 0..3u32 {
        let v = fx.mem.read_u8(p + 0x1A - k);
        for o in [0x120u32, 0x108, 0xF0, 0xD8] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    for o in [0x124u32, 0x10C, 0xF4, 0xDC] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    fx.set_xyz_z(sp + 0x130, 0xFF_FFFF);
    let z = fx.xyz_z(sp + 0x130);
    fx.set_xyz_z(sp + 0x100, z);
    let z = fx.xyz_z(sp + 0x100);
    fx.set_xyz_z(sp + 0x100, z);
    let z = fx.xyz_z(sp + 0x100);
    fx.set_xyz_z(sp + 0xE8, z);
    for o in [0x137u32, 0x11F, 0x107, 0xEF] {
        fx.mem.write_u8(sp + o, 0);
    }
    let fade = fx.mem.u8(p + 5);
    let mut a = (fade << 4) & 0xFF;
    let into = s0 - fx.mem.u16(p);
    if into < fade {
        a = (into << 4) & 0xFF;
    }
    if fx.mem.u16(p + 2) - s0 < 2 * fade {
        a = ((fx.mem.u8(p + 2) - n) << 3) & 0xFF;
    }
    for o in [0x123u32, 0x10B, 0xF3, 0xDB] {
        fx.mem.set8(sp + o, a);
    }
    let b = fx.mem.read_u8(p + 6);
    fx.mem.write_u8(sp + 0xB5, b);
    alpha_mode(fx, sp + 0xB0, sp + 0x15C);
    let sel = |v: u8| u64::from(v);
    let alpha = sel(fx.mem.read_u8(sp + 0x15C)) | (sel(fx.mem.read_u8(sp + 0x15D)) << 2) | (sel(fx.mem.read_u8(sp + 0x15E)) << 4) | (sel(fx.mem.read_u8(sp + 0x15F)) << 6);
    fx.desc_alpha(d, alpha);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let st = |fx: &mut Fx, o: u32, v: i32| {
        let h = fx.mem.read_u16(sp + o);
        fx.mem.write_u16(sp + o, (h & 0xC000) | (v & 0x3FFF) as u16);
    };
    let v8 = fx.mem.i32(p + 8) << 4;
    for (o, v) in [(0xE0u32, 0), (0xE2, 0), (0xF8, 0x2800), (0xFA, 0), (0x110, 0), (0x112, v8), (0x128, 0x2800), (0x12A, v8)] {
        st(fx, o, v);
    }
    let fix = |fx: &mut Fx, base: f32, k: i32, ra: u32| {
        let v = ee::add(base, ee::from_i32(k << 4));
        fx.fix_residue(v, ra);
        ee::fixunssfsi(v) as u16
    };
    let xc = fx.mem.i32(p + 0xC);
    for (o, base, ra) in [(0xE8u32, 28672.0f32, 0x01E0_59E0u32), (0x100, 36864.0, 0x01E0_5A14), (0x118, 28672.0, 0x01E0_5A48), (0x130, 36864.0, 0x01E0_5A7C)] {
        let v = fix(fx, base, xc, ra);
        fx.mem.write_u16(sp + o, v);
    }
    fx.mem.write_u16(sp + 0x102, 0x7000);
    fx.mem.write_u16(sp + 0xEA, 0x7000);
    let y8 = fx.mem.i32(p + 8);
    let y = fix(fx, 28672.0, y8, 0x01E0_5AC4);
    fx.mem.write_u16(sp + 0x132, y);
    fx.mem.write_u16(sp + 0x11A, y);
    let w = fx.mem.f(p + 0x14);
    fx.fix_residue(w, 0x01E0_5AE0);
    fx.submit(d, ot, ee::fixunssfsi(w) as i32);
    let v8 = fx.mem.i32(p + 8) << 4;
    for (o, v) in [(0xE0u32, 0), (0xE2, v8), (0xF8, 0x2800), (0xFA, v8), (0x110, 0), (0x112, 0x1C00), (0x128, 0x2800), (0x12A, 0x1C00)] {
        st(fx, o, v);
    }
    let xc = fx.mem.i32(p + 0x10);
    for (o, base, ra) in [(0xE8u32, 28672.0f32, 0x01E0_5BD0u32), (0x100, 36864.0, 0x01E0_5C04), (0x118, 28672.0, 0x01E0_5C38), (0x130, 36864.0, 0x01E0_5C6C)] {
        let v = fix(fx, base, xc, ra);
        fx.mem.write_u16(sp + o, v);
    }
    let y8 = fx.mem.i32(p + 8);
    let y = fix(fx, 28672.0, y8, 0x01E0_5CA0);
    fx.mem.write_u16(sp + 0x102, y);
    fx.mem.write_u16(sp + 0xEA, y);
    fx.mem.write_u16(sp + 0x132, 0x8C00);
    fx.mem.write_u16(sp + 0x11A, 0x8C00);
    let w = fx.mem.f(p + 0x14);
    fx.fix_residue(w, 0x01E0_5CD0);
    fx.submit(d, ot, ee::fixunssfsi(w) as i32);
    fx.leave(0x160);
    1
}

/// 0x1E05D20 (b186b3c26dc3, this record's own) [F]: `window(p, ot, n) ->
/// 1`: a packet layer (0x2078A0, opened on every call) holding, while the
/// update `n` lies in [u16 +0, u16 +2], a window sprite (0x6FF8 ..
/// 0x8FF8, far Z) of colour +0xE / +0xD / +0xC, alpha +0xF, ALPHA preset 3
/// (0x208450), keyed by +8 (0x1A2C70), no ordering table; the layer closed
/// into `ot` at key 0 (0x207910) only then. Never set in this record.
pub fn window(fx: &mut Fx, p: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x80);
    let layer = fx.layer_begin();
    let s = n & 0xFFFF;
    if s < fx.mem.u16(p) || fx.mem.u16(p + 2) < s {
        fx.leave(0x80);
        return 1;
    }
    let d = sp + 0x50;
    fx.mem.write_u16(d + 2, 6);
    fx.mem.write_u16(d, 3);
    fx.mem.write_u64(d + 8, 0x441);
    fx.mem.write_u32(d + 4, 0);
    for (o, src) in [(0x60u32, 0xEu32), (0x61, 0xD), (0x62, 0xC), (0x63, 0xF)] {
        let v = fx.mem.read_u8(p + src);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x64, 0x3F80_0000);
    for (at, xy, z) in [(0x68u32, 0x6FF8u16, 0xFF_FFFFu64), (0x70, 0x8FF8, 0xFF_FFFE)] {
        fx.mem.write_u16(sp + at, xy);
        fx.mem.write_u16(sp + at + 2, xy);
        fx.set_xyz_z(sp + at, z);
        fx.mem.write_u8(sp + at + 7, 0);
    }
    let alpha = fx.alpha_preset(3);
    fx.desc_alpha(d, alpha);
    let w = fx.mem.f(p + 8);
    fx.fix_residue(w, 0x01E0_5E5C);
    fx.submit(d, 0, ee::fixunssfsi(w) as i32);
    fx.layer_end(layer, ot, 0);
    fx.leave(0x80);
    1
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(D), 0x01E0_600C);
    let s2 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xCE => return 8,
        0x96 => strike(fx, sp),
        0x7C => arch(fx, sp, s2),
        0x40 => {
            for k in 0..2i32 {
                let r = D + 0x3CA8 + 0x4C * k as u32;
                if k != 0 {
                    set_counter4(fx, r, 0x80, 0, 0xFF);
                    fx.mem.write_u8(r + 4, 0x83);
                    fx.mem.write_u32(r + 0x14, 0);
                    fx.mem.write_u32(r + 0x10, 0x42A0_0000);
                    fx.mem.write_u32(r + 0x18, 0);
                } else {
                    fx.mem.write_u8(r + 4, 0x82);
                    set_counter4(fx, r, 0x10, 0, 0xFF);
                    fx.mem.write_u32(r + 0x14, 0x40A0_0000);
                    fx.mem.write_u32(r + 0x10, 0x40A0_0000);
                    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
                }
                fx.mem.write_u8(r + 5, 1);
                let b = fx.mem.read_u8(D + 0x14);
                fx.mem.write_u8(r + 6, b);
                for (o, v) in [(7u32, 9u8), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x60), (9, 0x10), (8, 0x40)] {
                    fx.mem.write_u8(r + o, v);
                }
            }
            shake_long(fx);
        }
        2 => {
            let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
            node_position_saving(fx, h, s, D + 0x51C4, u64::from(D), Some(u64::from(OT)), 0x01E0_6068);
            let y = ee::sub(fx.mem.f(D + 0x51C8), 5.0);
            fx.mem.setf(D + 0x51C8, y);
            shake_long(fx);
            let f = D + 0x5EC;
            fx.mem.set16(f, 2);
            fx.mem.set16(f + 2, 0x3F);
            fx.mem.write_u8(f + 5, 6);
            fx.mem.write_u8(f + 6, 1);
            fx.mem.write_u32(f + 8, 0x4180_0000);
            for o in [0x2Eu32, 0x2D, 0x2C] {
                fx.mem.write_u8(f + o, 0x80);
            }
            for (o, v) in [(0x14u32, -1i32), (0xC, -1), (0x18, 1), (0x10, 1), (0x20, -1), (0x1C, -1), (0x28, 1), (0x24, 1)] {
                fx.mem.set32(f + o, v);
            }
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0xBF).contains(&n) {
        fx.set_stage_light(0x30);
    }
    let n = fx.mem.u16(D);
    if (0x44..0x96).contains(&n) && n & 1 == 0 {
        let i = first_free(fx, D + 0x3D8C, 0x68, 0x28);
        if i >= 0 {
            let r = D + 0x3D8C + 0x68 * i as u32;
            set_counter4(fx, r, 0x18, 0, 0xFF);
            fx.mem.write_u8(r + 4, 5);
            if fx.mem.u16(D) < 0x61 {
                fx.mem.write_u32(r + 0x14, 0x3ECC_CCCD);
                fx.mem.write_u32(r + 0x1C, 0x40A0_0000);
                fx.mem.write_u8(r + 8, 8);
            } else {
                fx.mem.write_u8(r + 4, 0x85);
                fx.mem.write_u32(r + 0x14, 0x3FA6_6666);
                fx.mem.write_u32(r + 0x1C, 0x4170_0000);
                fx.mem.write_u8(r + 8, 0xC);
            }
            fx.mem.write_u8(r + 5, 1);
            let b = fx.mem.read_u8(D + 0x14);
            fx.mem.write_u8(r + 6, b);
            for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 4)] {
                fx.mem.write_u8(r + o, v);
            }
            let x = rand4(fx, 0x01E0_6BF0);
            fx.mem.set8(r + 9, x);
            fx.mem.write_u32(r + 0x10, BYTES);
            fx.mem.write_u32(r + 0x18, 0x3F80_0000);
            for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x23, 0), (0x27, 0x30)] {
                fx.mem.write_u8(r + o, v);
            }
            for (o, ra) in [(0x58u32, 0x01E0_6C3Cu32), (0x5C, 0x01E0_6C54), (0x60, 0x01E0_6C6C)] {
                let v = rand4(fx, ra);
                fx.mem.set32(r + o, v & 0x7F);
            }
        }
    }
    if fx.mem.u8(D + 0xAC) > 0 {
        let a = D + 0xAC;
        let i = first_free(fx, a, 0x54, 0x10);
        if i >= 0 {
            let r = a + 0x54 * i as u32;
            for k in 0..0x15u32 {
                let w = fx.mem.read_u32(a + 4 * k);
                fx.mem.write_u32(r + 4 * k, w);
            }
            fx.mem.write_u8(r, 6);
            for o in [0x3Cu32, 0x38, 0x34] {
                fx.mem.write_u32(r + o, 0);
            }
            fx.mem.write_u32(r + 0x18, 0xBFC0_0000);
        }
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_6D18, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6D28, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::arch::arches(fx, 0x01E0_0410, D + 0xAC, OT, 0x10);
    blib::fade::fade(fx, D + 0xA4, OT);
    blib::star::flash(fx, 0x01E0_27E0, D + 0x3CA8, OT, 3);
    blib::cross::bars(fx, 0x01E0_1DD0, D + 0x3B68, OT, 4);
    blib::streak::ribbon(fx, 0x01E0_1240, D + 0x3D8C, OT, 0x28);
    blib::sheet::sheets(fx, 0x01E0_3390, D + 0x4DCC, OT, 0x10);
    let (_, bounce_a3) = blib::sprite::bounce_a3(fx, D + 0x648, OT, 0xC8);
    let (_, streaks_a3) = blib::radiate::streaks_a3(fx, 0x01E0_4370, D + 0x518C, OT, 1);
    // [H] a3 = what streaks / bounce left, else not modelled (0).
    let a3 = streaks_a3.or(bounce_a3).unwrap_or(0);
    let n = fx.mem.u16(D);
    blib::window::frame_fade(fx, 0x01E0_5190, D + 0x5EC, OT, n, a3);
    let n = fx.mem.u16(D);
    split(fx, D + 0x61C, OT, n, a3);
    let n = fx.mem.u16(D);
    window(fx, D + 0x638, OT, n);
    let n = fx.mem.u16(D);
    if (0x96..0xBB).contains(&n) {
        6
    } else if n == 0x93 {
        2
    } else if n == 0x8C {
        1
    } else {
        0
    }
}
