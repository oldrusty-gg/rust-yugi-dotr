//! BMODULE b198 (card 560), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E060E0, data area a0 = 0x1ED0000) [F] (helper B10,
//! 31 Oct 2026): s3 = D, s0 = the OT (0x206E40), s1 the parameter then the
//! spawn loops' counters. Update 23 the attacker's echoes (+0x15E0,
//! `radiate::echoes`, 9 counts); 30 at node 0x3C four bars (+0x18F8, 0x50
//! apart), a flash (+0x1844) and a shake (+0x40); 46 an arc (+0x1604) from
//! node 0x3C to 0x3D and long echoes (180 counts, delayed 4); 137 at the
//! target a flash, bead trails (+0x1C54) from node 0x3C into the beads
//! (+0x1CD0), a ribbon (+0x1890) and a shake; 138 / 144 / 150 three sheets
//! (+0x1A38, 0x3C apart) each; on updates 55..230 two bouncing sprites
//! (+0xA0, 0x44 apart) a update from nodes 0x10 / 0x1D. Returns 6 on
//! updates 138..170, 2 on 135, 1 on 128, 8 on 200 (no drawing then) and
//! at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `bounce`
//! 0x1E00410, `arcs` 0x1E00E80, `flash` 0x1E02100, `ribbon` 0x1E02CB0,
//! `echoes` 0x1E03840, `bars` 0x1E03F80, `sheets` 0x1E04990, `beads`
//! 0x1E04F00, `bead_trails_b` 0x1E05CD0 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E060E0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0x3270).
    fx.bzero(D, 0x3270);
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
    for (at, node, clear) in [(0xCu32, 0x10, 1), (0x10, 0x1D, 0), (0x14, 0x3C, 0), (0x18, 0x3D, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The entry's s0 = the OT, s1 = 3 (the texture loop's), s2 the runner's.
    rand_bytes(fx, D + 0x3170, 0x100, 0x01E0_621C, [Some(OT), Some(3), None]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 4
}

/// The echoes record (+0x15E0): `count` counts after `delay`, `steps`.
fn echoes_set(fx: &mut Fx, count: i32, delay: i32, steps: u8) {
    let e = D + 0x15E0;
    set_counter4(fx, e, count, delay, 0xFF);
    for (o, v) in [(4u32, 0u8), (8, 6), (9, 1), (0xA, steps), (0xB, 0)] {
        fx.mem.write_u8(e + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(e + 0xC, h);
    for (o, v) in [(0x10u32, 0x80u8), (0x11, 0x80), (0x12, 0x80), (0x13, 0x20)] {
        fx.mem.write_u8(e + o, v);
    }
}

/// The flash (+0x1844) at `from`.
fn flash_set(fx: &mut Fx, count: i32, size: u32, glow: u32, rgb: [u8; 3], from: u32) {
    let a = D + 0x1844;
    set_counter4(fx, a, count, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x10, size);
    fx.mem.write_u32(a + 0x14, glow);
    for o in [0xCu32, 0xD, 0xE] {
        fx.mem.write_u8(a + o, 0xC0);
    }
    for (o, v) in [(0xAu32, rgb[0]), (9, rgb[1]), (8, rgb[2]), (0xF, 0xD0)] {
        fx.mem.write_u8(a + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(a + o, 0);
    }
    copy_v4(fx, from, a + 0x1C);
}

/// The camera shake (+0x40) at `from`.
fn shake_set(fx: &mut Fx, k: [u32; 3], from: u32) {
    set_counter4(fx, D + 0x40, 0x16, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, k[0]);
    fx.mem.write_u32(D + 0x58, k[1]);
    fx.mem.write_u32(D + 0x5C, k[2]);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x4220_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, from, D + 0x60);
}

/// Update 30 [F 0x1E0630C]: four bars at node 0x3C (sp + 0x60), the flash
/// and the shake there.
fn bars_start(fx: &mut Fx, sp: u32, s1: u64) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14));
    node_position_saving(fx, h, s, sp + 0x60, u64::from(OT), Some(s1), 0x01E0_6320);
    for k in 0..4i32 {
        let r = D + 0x18F8 + 0x50 * k as u32;
        set_counter4(fx, r, 0x16, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        if k == 0 {
            fx.mem.write_u32(r + 0x10, 0x447A_0000);
            fx.mem.write_u32(r + 0x18, 0);
            fx.mem.write_u32(r + 0x14, 0x41A0_0000);
        } else {
            fx.mem.write_u32(r + 0x10, 0x43C8_0000);
            fx.mem.setf(r + 0x18, ee::mul(45.0, ee::from_i32(k)));
            fx.mem.write_u32(r + 0x14, 0x4170_0000);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0xC0), (9, 0x20), (8, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x40C0_0000);
        copy_v4(fx, sp + 0x60, r + 0x20);
    }
    flash_set(fx, 0x16, 0x4334_0000, 0, [0x40, 0x10, 0x80], sp + 0x60);
    shake_set(fx, [0x3FE6_6666, 0x3F80_0000, 0x4000_0000], sp + 0x60);
}

/// Update 46 [F 0x1E0656C]: the arc from node 0x3C to 0x3D and the long
/// echoes.
fn arc_start(fx: &mut Fx) {
    let a = D + 0x1604;
    set_counter4(fx, a, 0xB4, 0, 0xFF);
    fx.mem.write_u8(a + 4, 1);
    fx.mem.write_u32(a + 8, D + 0x1664);
    fx.mem.write_u8(a + 0x30, 0x28);
    fx.mem.write_u8(a + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(a + 0x32, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(a + 0x33, b);
    fx.mem.write_u32(a + 0x38, 0x3F80_0000);
    fx.mem.write_u8(a + 0x34, 0);
    fx.mem.write_u16(a + 0x36, 0);
    for (o, v) in [(0x47u32, 0x80u8), (0x44, 0x80), (0x45, 0x80), (0x46, 0x80), (0x4A, 0xC0), (0x49, 0x10), (0x48, 0x40)] {
        fx.mem.write_u8(a + o, v);
    }
    let mut k = 0u32;
    while (k as i32) < i32::from(fx.mem.read_u8(a + 0x31)) + 1 {
        fx.mem.set16(D + 0x1622 + 2 * k, -1);
        fx.mem.set16(D + 0x1610 + 2 * k, -1);
        k += 1;
    }
    echoes_set(fx, 0xB4, 4, 4);
}

/// Update 137 [F 0x1E0668C]: the strike at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32, s1: u64) {
    flash_set(fx, 0x20, 0x4448_0000, 0x41C8_0000, [0x80, 0x20, 0xA0], sp + 0x50);
    // Bead trails (+0x1C54) from node 0x3C into the beads (+0x1CD0).
    let b = D + 0x1C54;
    set_counter4(fx, b, 0x18, 0, 0xFF);
    fx.mem.write_u8(b + 4, 0);
    fx.mem.write_u8(b + 5, 1);
    let v = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(b + 6, v);
    fx.mem.write_u32(b + 8, D + 0x1CD0);
    for (o, v) in [(0x10u32, 0x3Cu8), (0x12, 8), (0x11, 0xE), (0x13, 1), (0x16, 2), (0x14, 3)] {
        fx.mem.write_u8(b + o, v);
    }
    fx.mem.write_u32(b + 0x18, 0x3F80_0000);
    fx.mem.write_u32(b + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(b + 0x20, 0x42C8_0000);
    fx.mem.write_u32(b + 0x24, 0xBE4C_CCCD);
    fx.mem.write_u32(b + 0xC, D + 0x3170);
    for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
        fx.mem.write_u8(b + o, 0x80);
    }
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14));
    node_position_saving(fx, h, s, b + 0x3C, u64::from(OT), Some(s1), 0x01E0_67D8);
    copy_v4(fx, b + 0x3C, b + 0x4C);
    // The ribbon (+0x1890).
    let r = D + 0x1890;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4020_0000);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = rand4(fx, 0x01E0_6854);
    fx.mem.set8(r + 9, x);
    fx.mem.write_u32(r + 0x10, D + 0x3170);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, sp + 0x50, r + 0x28);
    fx.mem.write_u32(r + 0x58, 0x80);
    fx.mem.write_u32(r + 0x5C, 0x100);
    fx.mem.write_u32(r + 0x60, 0x180);
    shake_set(fx, [0x4000_0000, 0x3F80_0000, 0x3FE6_6666], sp + 0x50);
}

/// Updates 138 / 144 / 150 [F 0x1E0694C]: three sheets at the target.
fn sheets_start(fx: &mut Fx, sp: u32) {
    for s1 in 0..3i32 {
        let i = first_free(fx, D + 0x1A38, 0x3C, 9);
        if i < 0 {
            continue;
        }
        let r = D + 0x1A38 + 0x3C * i as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0), (8, 2), (0xA, 0), (9, 6), (0xB, 2), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x50, r + 0x2C);
        let k = f32::from_bits(0x3F4C_CCCD);
        let dy = ee::add(f32::from_bits(0xBF4C_CCCD), ee::mul(k, ee::from_i32(s1)));
        let y = ee::add(fx.mem.f(r + 0x30), dy);
        fx.mem.setf(r + 0x30, y);
        fx.mem.write_u32(r + 0x10, 0x4000_0000);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        fx.mem.write_u32(r + 0x18, if s1 == 1 { 0x40C0_0000 } else { 0x40A0_0000 });
        let turn = match fx.mem.u16(D) {
            0x96 => 0x3600,
            0x90 => 0x4000,
            _ => 0x4A00,
        };
        fx.mem.set32(r + 0x24, turn);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            let v = fx.mem.i32(r + 0x24).wrapping_neg();
            fx.mem.set32(r + 0x24, v);
        }
    }
}

/// Updates 55..230 [F 0x1E06B24]: two bouncing sprites from nodes 0x10 /
/// 0x1D (sp + 0x60), jittered.
fn sprites(fx: &mut Fx, sp: u32) {
    for s1 in 0..2i32 {
        let i = first_free(fx, D + 0xA0, 0x44, 0x50);
        if i < 0 {
            continue;
        }
        let r = D + 0xA0 + 0x44 * i as u32;
        set_counter4(fx, r, 0x24, 0, 0xFF);
        let v = rand4(fx, 0x01E0_6B7C);
        fx.mem.set8(r + 4, (v & 3) << 4);
        fx.mem.write_u8(r + 8, 0);
        fx.mem.write_u8(r + 0xB, 1);
        fx.mem.write_u8(r + 0xA, 5);
        let v = rand4(fx, 0x01E0_6BAC);
        fx.mem.set8(r + 9, v & 3);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x60)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x4316_0000);
        fx.mem.write_u32(r + 0x14, 0x4100_0000);
        fx.mem.write_u32(r + 0x18, 0xBCA3_D70A);
        fx.mem.write_u32(r + 0x1C, 0);
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0xC + 4 * s1 as u32);
        node_position_saving(fx, h, s, sp + 0x60, u64::from(OT), Some(s1 as u64), 0x01E0_6C10);
        let v = rand4(fx, 0x01E0_6C18);
        let (_, m) = fx.div(v, 0x78);
        let x = ee::add(fx.mem.f(sp + 0x60), ee::div(ee::from_i32(m - 0x3C), 100.0));
        fx.mem.setf(r + 0x20, x);
        let y = fx.mem.read_u32(sp + 0x64);
        fx.mem.write_u32(r + 0x24, y);
        let v = rand4(fx, 0x01E0_6C70);
        let (_, m) = fx.div(v, 0x78);
        let z = ee::add(fx.mem.f(sp + 0x68), ee::div(ee::from_i32(m - 0x3C), 100.0));
        fx.mem.setf(r + 0x28, z);
        let v = rand4(fx, 0x01E0_6CC0);
        let (_, m) = fx.div(v, 0x64);
        fx.mem.setf(r + 0x2C, ee::div(ee::from_i32(m - 0x32), 100.0));
        fx.mem.write_u32(r + 0x30, 0x3ECC_CCCD);
        let v = rand4(fx, 0x01E0_6D14);
        let (_, m) = fx.div(v, 0x64);
        fx.mem.setf(r + 0x34, ee::div(ee::from_i32(m - 0x32), 100.0));
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_6244);
    let s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0xC8 => return 8,
        0x96 | 0x90 | 0x8A => sheets_start(fx, sp),
        0x89 => strike(fx, sp, s1),
        0x2E => arc_start(fx),
        0x1E => bars_start(fx, sp, s1),
        0x17 => echoes_set(fx, 9, 0, 6),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x37..0xE7).contains(&n) {
        sprites(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_6D7C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::sprite::bounce(fx, D + 0xA0, OT, 0x50);
    // a3 = what arcs left (the echoes' first TEXFLUSH reports it) [run].
    let (_, a3) = blib::arc::arcs_a3(fx, 0x01E0_0E80, D + 0x1604, OT, 1);
    blib::radiate::echoes(fx, 0x01E0_3840, D + 0x15E0, OT, 1, a3);
    blib::streak::ribbon(fx, 0x01E0_2CB0, D + 0x1890, OT, 1);
    blib::cross::bars(fx, 0x01E0_3F80, D + 0x18F8, OT, 4);
    blib::sheet::sheets(fx, 0x01E0_4990, D + 0x1A38, OT, 9);
    // sheets' 0x220 frame saves the runner's s5..s7 / fp (0) and ra at
    // +0x50..+0x90: the bead trails' samples take their w from there
    // (sp - 0x1D0 + 0x10 k) [run].
    fx.saved(sp - 0x220 + 0x50, &[(0, 0), (0x10, 0), (0x20, 0), (0x30, 0), (0x40, 0x01E0_6DF4)]);
    blib::emit::bead_trails_b(fx, 0x01E0_5CD0, D + 0x1C54, 1);
    blib::beam::beads(fx, 0x01E0_4F00, D + 0x1CD0, OT, 0x3C);
    blib::star::flash(fx, 0x01E0_2100, D + 0x1844, OT, 1);
    let n = fx.mem.u16(D);
    if (0x8A..0xAB).contains(&n) {
        6
    } else if n == 0x87 {
        2
    } else if n == 0x80 {
        1
    } else {
        0
    }
}
