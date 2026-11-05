//! BMODULE b005 (card 293), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03BC0, data area a0 = 0x1ED0000, 0x21D0 bytes)
//! [F]: s0 = D, the OT (0x206E40) in s1, s2 the
//! parameter. Init: the handles (+4 / +8), the attacker's rotation
//! (+0x2C), textures (4, 0xF) and (5, 0..1), nodes 1 (cleared, +0xC) /
//! 0x28 (+0x10), the stage light ramp (+0xD0: 2 / 0x72) and the random
//! bytes (+0x20C8). Every update the target's position (sp + 0x80).
//! Update 0x1A a flash (+0xD4) and a shake on node +0x10; odd 0x1D..0x3D a
//! spike (+0x120) there; 0x3C two domes (+0x1988, `dome::domes`) aimed
//! (0x101940) from node +0x10 to node +0xC while they run, brightening 8
//! an update to 0x80; 0x46 the fade (+0x1980), four rings (+0x1A78,
//! `band::bands_b`) and eight bead fans (+0x1D58, `beam::beads_at`) at the
//! target and a shake. Returns 6 on 0x46..0x62, 2 on 0x63, 1 on 0x3C, 8
//! on 0x78 (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `fade` (a
//! register twin) 0x1E003A0, `flash_b` 0x1E00870, `domes` 0x1E012E0,
//! `spikes` (twin) 0x1E01D20, `bands_b` (twin) 0x1E02800, `beads_at`
//! (twin) 0x1E02F40 [`crate::blib`]: the twins differ from the ported
//! compilations only in temporary registers.
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{aim_angles, first_free, set_counter4}};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x70;
const FLASH: u32 = D + 0xD4;
const SPIKES: u32 = D + 0x120;
const FADE: u32 = D + 0x1980;
const DOMES: u32 = D + 0x1988;
const RINGS: u32 = D + 0x1A78;
const BEADS: u32 = D + 0x1D58;
const BYTES: u32 = D + 0x20C8;

/// The entry 0x1E03BC0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x21D0).
    fx.bzero(D, 0x21D0);
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
    for (k, (node, clear)) in [(1, 1), (0x28, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0xC + 4 * k as u32, r);
    }
    fx.mem.set16(D + 0xD0, 2);
    fx.mem.set16(D + 0xD2, 0x72);
    // The entry's s0 = D, s1 = 2 (the texture loop's), s2 the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_3CAC, [Some(D), Some(2), Some(param as u32)]);
    0
}

/// rand() >> 4 with the entry's s0 (D) saved in rand's frame.
fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, u64::from(D));
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand() >> 4
}

fn set(fx: &mut Fx, r: u32, bytes: &[(u32, u8)], words: &[(u32, u32)]) {
    for &(o, v) in bytes {
        fx.mem.write_u8(r + o, v);
    }
    for &(o, v) in words {
        fx.mem.write_u32(r + o, v);
    }
}

/// Update 0x46 [F 0x1E03EB0]: the fade, four rings, eight bead fans and a
/// shake at the target.
fn burst(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 0xC, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    for k in 0..4u32 {
        let r = RINGS + 0x5C * k;
        set_counter4(fx, r, 0x20, ((2 * k) & 0xFF) as i32, 0xFF);
        set(fx, r, &[(4, 0), (7, 0x24), (0xC, 1), (8, 1), (9, 2), (0xA, 6), (0xB, 0)], &[(0x10, 0x3F80_0000), (0x14, 0), (0x18, 0x4000_0000), (0x1C, 0x3F80_0000), (0x20, 0x3F00_0000)]);
        set(fx, r, &[(0x28, 0x80), (0x29, 0x80), (0x2A, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0), (0x2B, 0x40)], &[]);
        copy_v4(fx, sp + 0x80, r + 0x2C);
        fx.mem.write_u32(r + 0x3C, 0x4000);
    }
    for k in 0..8i32 {
        let r = BEADS + 0x58 * k as u32;
        set_counter4(fx, r, 0x20, k & 0xFF, 0xFF);
        let c = fx.mem.u8(r) - 4;
        fx.mem.set8(r, c);
        set(fx, r, &[(4, 0xD), (5, 0), (8, 0x32), (0xA, 0), (0xC, 2), (0xB, 2)], &[(0x1C, 0x4270_0000), (0x20, 0xBF19_999A), (0x10, BYTES), (0x14, 0x4000_0000)]);
        fx.mem.set8(r + 9, k);
        set(fx, r, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0x60)], &[]);
        let (_, m) = fx.div(k, 3);
        let o = match m {
            2 => Some(0x24),
            1 => Some(0x25),
            0 => Some(0x26),
            _ => None,
        };
        if let Some(o) = o {
            let v = fx.mem.u8(r + o) + 0x30;
            fx.mem.set8(r + o, v);
        }
        copy_v4(fx, sp + 0x80, r + 0x28);
    }
    set_counter4(fx, SHAKE, 0x20, 0, 0xFF);
    fx.mem.write_u8(SHAKE + 5, 0);
    fx.mem.write_u8(SHAKE + 4, 1);
    set(fx, SHAKE, &[], &[(0x14, 0x3FE6_6666), (0x18, 0x3FE6_6666), (0x1C, 0x3F80_0000), (0x30, 0), (0x34, 0x4220_0000), (0x50, 0x40C0_0000)]);
    copy_v4(fx, sp + 0x80, SHAKE + 0x20);
}

/// Update 0x3C [F 0x1E03DE0]: two domes.
fn domes(fx: &mut Fx) {
    for k in 0..2u32 {
        let r = DOMES + 0x50 * k;
        set_counter4(fx, r, 0xE, 0, 0xFF);
        set(fx, r, &[(0xA, 4), (7, 9), (8, 1), (9, 2)], &[(0xC, 0x4040_0000), (0x14, 0x4150_0000)]);
        if k == 1 {
            set(fx, r, &[], &[(0x10, 0xBD23_D70A), (0xC, 0x4040_0000), (0x38, 0x8000), (0x48, 0x80)]);
        } else {
            set(fx, r, &[], &[(0x10, 0xBDA3_D70A), (0xC, 0x4000_0000), (0x48, 0x80)]);
        }
        set(fx, r, &[(0x1C, 0x80), (0x1D, 0x80), (0x1E, 0x80), (0x1F, 0)], &[(0x18, 0)]);
    }
}

/// Update 0x1A [F 0x1E03D08]: a flash and a shake on node +0x10.
fn flash(fx: &mut Fx) {
    set_counter4(fx, FLASH, 0x2C, 0, 0xFF);
    fx.mem.write_u8(FLASH + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(FLASH + 6, b);
    set(fx, FLASH, &[(4, 0x81), (7, 9), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x20), (9, 0x20), (8, 0x80), (0xF, 0xC0)], &[(0x10, 0x4334_0000), (0x14, 0), (0x44, 0), (0x40, 0), (0x3C, 0)]);
    set_counter4(fx, SHAKE, 0x2C, 0, 0xFF);
    fx.mem.write_u8(SHAKE + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(SHAKE + 6, b);
    fx.mem.write_u8(SHAKE + 4, 4);
    set(fx, SHAKE, &[], &[(0x14, 0x3F80_0000), (0x18, 0x3F99_999A), (0x1C, 0x3FB3_3333), (0x30, 0), (0x34, 0x4220_0000), (0x50, 0x4080_0000)]);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x80, u64::from(D), 0x01E0_3CCC);
    match fx.mem.u16(D) {
        0x78 => return 8,
        0x46 => burst(fx, sp),
        0x3C => domes(fx),
        0x1A => flash(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x1C..0x3E).contains(&n) && n & 1 != 0 {
        spike(fx);
    }
    if fx.mem.u8(DOMES) > 0 {
        for o in [0x1Fu32, 0x6F] {
            let a = fx.mem.u8(DOMES + o);
            if a < 0x80 {
                fx.mem.set8(DOMES + o, a + 8);
            }
        }
        let h = fx.mem.i32(D + 4);
        let s = fx.mem.i32(D + 0xC);
        node_position_saving(fx, h, s, sp + 0x60, u64::from(D), Some(u64::from(OT)), 0x01E0_42C8);
        let s = fx.mem.i32(D + 0x10);
        node_position_saving(fx, h, s, sp + 0x70, u64::from(D), Some(u64::from(OT)), 0x01E0_42DC);
        for c in 0..3u32 {
            let v = crate::ee::sub(fx.mem.f(sp + 0x60 + 4 * c), fx.mem.f(sp + 0x70 + 4 * c));
            fx.mem.setf(sp + 0x90 + 4 * c, v);
        }
        aim_angles(fx, sp + 0x90, DOMES + 0x30);
        copy_v4(fx, DOMES + 0x30, DOMES + 0x80);
        copy_v4(fx, sp + 0x60, DOMES + 0x70);
        copy_v4(fx, DOMES + 0x70, DOMES + 0x20);
    }
    shake_saving(fx, SHAKE, u64::from(D), 0x01E0_4388, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xD0, n, 0x01E0_4394, [Some(u64::from(D)), Some(u64::from(OT))]);
    let ot = OT;
    blib::fade::fade(fx, FADE, ot);
    blib::star::flash_b(fx, 0x01E0_0870, FLASH, ot, 1);
    blib::spike::spikes(fx, 0x01E0_1D20, SPIKES, ot, 0x3C);
    blib::dome::domes(fx, 0x01E0_12E0, DOMES, ot, 3);
    blib::band::bands_b(fx, 0x01E0_2800, RINGS, ot, 8);
    blib::beam::beads_at(fx, 0x01E0_2F40, 0x34, 0x3C, BEADS, ot, 0xA);
    let n = fx.mem.u16(D);
    if (0x46..0x63).contains(&n) {
        6
    } else if n == 0x63 {
        2
    } else if n == 0x3C {
        1
    } else {
        0
    }
}

/// Odd updates 0x1D..0x3D [F 0x1E0416C]: a spike on node +0x10.
fn spike(fx: &mut Fx) {
    let j = first_free(fx, SPIKES, 0x68, 0x3C);
    if j < 0 {
        return;
    }
    let r = SPIKES + 0x68 * j as u32;
    set_counter4(fx, r, 0x12, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0x85);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(r + 6, b);
    set(fx, r, &[(0xA, 0), (0xB, 2), (0xC, 3)], &[]);
    let v = rand4(fx, 0x01E0_41E4);
    fx.mem.set8(r + 9, v);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    set(fx, r, &[(0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0), (0x27, 0x30), (8, 0x12)], &[(0x14, 0x4020_0000), (0x1C, 0x420C_0000)]);
    for (o, ra) in [(0x58u32, 0x01E0_4244), (0x5C, 0x01E0_4258), (0x60, 0x01E0_426C)] {
        let v = rand4(fx, ra) & 0x1FF;
        fx.mem.set32(r + o, v);
    }
}
