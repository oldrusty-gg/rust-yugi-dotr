//! BMODULE b106 (card 137), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05620, data area a0 = 0x1ED0000) [F] (helper B9, 31
//! Oct 2026): s1 = D, s2 = the OT, s0 the parameter then the spawn loops'
//! offsets. Node bursts (+0xA4, motes +0xE8) every update; the attacker's
//! echoes (+0xFB8, `radiate::echoes`) set on updates 55 / 73 / 88 (six
//! steps), 105 (four) and 124 / 140 / 154 / 171 (twelve, with the object
//! op 0x1F83B0 and node 0x19 read into +0x1E74); on 120 / 132 / 150 / 166
//! / 180 a strike from node 0x19 (sp + 0x90) and from +0x1E74 to the
//! target (sp + 0x80): flashes (+0xFDC, 0x4C apart), rays (+0x123C, 0x74),
//! cones (+0x1734, 0x48), ribbons (+0x1324, 0x68), two more flashes and a
//! camera shake (+0x40). The stage light pulsed by 0x1031C0 (+0xA0).
//! Returns 6 on updates 120..176, 1 on 110, 2 on 183, 4 on 222, 8 on 250
//! (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `beads` 0x1E015F0, `rays`
//! 0x1E023C0, `cones` 0x1E02F30, `flash` 0x1E037A0, `echoes` 0x1E04350,
//! `ribbon` 0x1E04A90 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{aim_angles, first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry 0x1E05620 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x1E90).
    fx.bzero(D, 0x1E90);
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
    for (at, node, clear) in [(0xCu32, 0x19, 1), (0x10, 0x20, 0), (0x14, 0x2B, 0), (0x18, 0x32, 0), (0x1C, 1, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): specs 0x1E06628.. (four), motes +0xE8 /
    // +0x7E8.
    for (dst, src) in [(0xCCu32, 0x18u32), (0xCD, 0x14), (0xCE, 0x10), (0xCF, 0xC)] {
        let b = fx.mem.read_u8(D + src);
        fx.mem.write_u8(D + dst, b);
    }
    for (k, spec) in [0x01E0_6628u32, 0x01E0_6638, 0x01E0_6648, 0x01E0_6650].into_iter().enumerate() {
        fx.mem.write_u32(D + 0xAC + 4 * k as u32, spec);
    }
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    fx.mem.write_u16(D + 0xD8, 0);
    fx.mem.write_u32(D + 0xDC, 0x42C8_0000);
    fx.mem.write_u32(D + 0xE4, 0x40A0_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xF4);
    // The entry's s0 = the parameter, s1 = D, s2 = the OT.
    rand_bytes(fx, D + 0x1D74, 0x100, 0x01E0_5814, [Some(param as u32), Some(D), Some(OT)]);
    0
}

fn copy_v4(fx: &mut Fx, from: u32, to: u32) {
    for c in 0..4u32 {
        let v = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, v);
    }
}

/// sp + 0xA0 = (sp + 0x80 - `from`) (xyz), divided by 3 and negated
/// when `third` (the second form: -(sp + 0x80 - +0x1E74) / 3).
fn delta(fx: &mut Fx, sp: u32, from: u32, third: bool, neg: bool) {
    for c in 0..3u32 {
        let mut v = ee::sub(fx.mem.f(sp + 0x80 + 4 * c), fx.mem.f(from + 4 * c));
        if neg {
            v = -v;
        }
        if third {
            v = ee::div(v, 3.0);
        }
        fx.mem.setf(sp + 0xA0 + 4 * c, v);
    }
}

/// sp + 0x80 - sp + 0xA0 into `out` (xyz).
fn back(fx: &mut Fx, sp: u32, out: u32) {
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(sp + 0x80 + 4 * c), fx.mem.f(sp + 0xA0 + 4 * c));
        fx.mem.setf(out + 4 * c, v);
    }
}

/// The strike [F 0x1E05A10]: `s4` (1 on update 120, else 2) sources.
/// Returns the entry's s0 after it.
fn strike(fx: &mut Fx, sp: u32, s4: i32) -> u64 {
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0xC);
    fx.node_position(h, s, sp + 0x90);
    let mut s0 = (2 * s4) as u64;
    // Flashes (+0xFDC, 0x4C apart).
    for s3 in 0..2 * s4 {
        let i = first_free(fx, D + 0xFDC, 0x4C, 8);
        if i < 0 {
            break;
        }
        let o = 0x4C * i as u32;
        let r = D + o;
        set_counter4(fx, r + 0xFDC, 8, 0, 0xFF);
        fx.mem.write_u8(r + 0xFE1, 0);
        fx.mem.write_u8(r + 0xFE0, 0x41);
        fx.mem.write_u8(r + 0xFE3, 9);
        for k in [0xFEBu32, 0xFE8, 0xFE9, 0xFEA] {
            fx.mem.write_u8(r + k, 0xC0);
        }
        fx.mem.write_u8(r + 0xFE6, 0x80);
        fx.mem.write_u8(r + 0xFE5, 0x60);
        fx.mem.write_u8(r + 0xFE4, 0x10);
        fx.mem.write_u32(r + 0xFF0, 0);
        let b = fx.mem.read_u8(r + 0xFE0);
        if s3 & 1 == 1 {
            fx.mem.write_u8(r + 0xFE0, b | 2);
            fx.mem.write_u32(r + 0xFEC, 0x437A_0000);
        } else {
            fx.mem.write_u8(r + 0xFE0, b | 0x84);
            fx.mem.write_u32(r + 0xFEC, 0x43FA_0000);
        }
        let src = if s3 < 2 { sp + 0x90 } else { D + 0x1E74 };
        copy_v4(fx, src, r + 0xFF8);
    }
    // Rays (+0x123C, 0x74 apart).
    for s3 in 0..s4 {
        let r = D + 0x74 * s3 as u32;
        set_counter4(fx, r + 0x123C, 0xC, 0, 0xFF);
        let c = fx.mem.read_u8(r + 0x123C).wrapping_sub(2);
        fx.mem.write_u8(r + 0x123C, c);
        fx.mem.write_u8(r + 0x1240, 0);
        fx.mem.write_u8(r + 0x1241, 0);
        fx.mem.write_u8(r + 0x1244, 0x1E);
        let x = fx.rand_from(s0 as u32, 0x01E0_5C14);
        fx.mem.set8(r + 0x1245, x >> 4);
        fx.mem.write_u32(r + 0x1254, 0x4270_0000);
        fx.mem.write_u32(r + 0x1248, D + 0x1D74);
        fx.mem.write_u32(r + 0x124C, 0x4080_0000);
        fx.mem.write_u32(r + 0x1250, 0x3F19_999A);
        fx.mem.write_u8(r + 0x125A, 0x80);
        for k in [0x1259u32, 0x1258, 0x125C, 0x125D, 0x125E, 0x125F] {
            fx.mem.write_u8(r + k, 0x60);
        }
        if s3 == 0 {
            copy_v4(fx, sp + 0x90, r + 0x1260);
            delta(fx, sp, sp + 0x90, false, false);
        } else {
            copy_v4(fx, D + 0x1E74, r + 0x1260);
            delta(fx, sp, D + 0x1E74, false, false);
        }
        aim_angles(fx, sp + 0xA0, r + 0x1290);
    }
    // Cones (+0x1734, 0x48 apart).
    for s3 in 0..s4 {
        let i = first_free(fx, D + 0x1734, 0x48, 0xA);
        if i < 0 {
            break;
        }
        let r = D + 0x48 * i as u32;
        set_counter4(fx, r + 0x1734, 0xE, 0, 0xFF);
        fx.mem.write_u8(r + 0x1738, 0);
        fx.mem.write_u8(r + 0x1739, 0);
        fx.mem.write_u8(r + 0x173B, 9);
        fx.mem.write_u8(r + 0x173C, 0);
        fx.mem.write_u32(r + 0x1740, 0x4040_0000);
        fx.mem.write_u32(r + 0x1744, 0x3ECC_CCCD);
        fx.mem.write_u32(r + 0x1748, 0x3F19_999A);
        fx.mem.write_u32(r + 0x174C, 0);
        fx.mem.write_u32(r + 0x1750, 0x3DCC_CCCD);
        for k in [0x1757u32, 0x1754, 0x1755, 0x1756] {
            fx.mem.write_u8(r + k, 0x80);
        }
        fx.mem.write_u8(r + 0x175A, 0xC0);
        fx.mem.write_u8(r + 0x1759, 0x60);
        fx.mem.write_u8(r + 0x1758, 0x60);
        fx.mem.write_u8(r + 0x175B, 0);
        if s3 == 0 {
            delta(fx, sp, sp + 0x90, false, false);
            copy_v4(fx, sp + 0x90, r + 0x175C);
        } else {
            delta(fx, sp, D + 0x1E74, false, false);
            copy_v4(fx, D + 0x1E74, r + 0x175C);
        }
        aim_angles(fx, sp + 0xA0, r + 0x176C);
    }
    // Ribbons (+0x1324, 0x68 apart).
    let mut s3 = 0;
    while (s3 as u64) < s0 {
        let i = first_free(fx, D + 0x1324, 0x68, 0xA);
        if i < 0 {
            break;
        }
        let r = D + 0x68 * i as u32;
        fx.mem.write_u8(r + 0x1328, 5);
        if s3 & 1 != 0 {
            set_counter4(fx, r + 0x1324, 0x14, 0, 0xFF);
            let b = fx.mem.read_u8(r + 0x1328);
            fx.mem.write_u8(r + 0x1328, b | 2);
            fx.mem.write_u32(r + 0x1338, 0x3F80_0000);
            fx.mem.write_u32(r + 0x1340, 0x42C8_0000);
            fx.mem.write_u8(r + 0x1347, 0x80);
            fx.mem.write_u8(r + 0x134B, 0x80);
        } else {
            set_counter4(fx, r + 0x1324, 0x12, 0, 0xFF);
            fx.mem.write_u32(r + 0x1338, 0x4000_0000);
            fx.mem.write_u32(r + 0x1340, 0x4220_0000);
            fx.mem.write_u8(r + 0x134B, 0xC0);
            fx.mem.write_u8(r + 0x1347, 0);
        }
        fx.mem.write_u8(r + 0x1329, 0);
        fx.mem.write_u8(r + 0x132E, 1);
        fx.mem.write_u8(r + 0x132F, 2);
        fx.mem.write_u8(r + 0x1330, 3);
        fx.mem.write_u8(r + 0x132C, 0x28);
        let x = fx.rand_from(s0 as u32, 0x01E0_5FF8);
        fx.mem.set8(r + 0x132D, x >> 4);
        fx.mem.write_u32(r + 0x1334, D + 0x1D74);
        fx.mem.write_u32(r + 0x133C, 0x3F80_0000);
        for k in [0x1348u32, 0x1349, 0x134A, 0x1344, 0x1345, 0x1346] {
            fx.mem.write_u8(r + k, 0x80);
        }
        if s3 < 2 {
            delta(fx, sp, sp + 0x90, true, false);
        } else {
            delta(fx, sp, D + 0x1E74, true, true);
        }
        aim_angles(fx, sp + 0xA0, r + 0x136C);
        back(fx, sp, r + 0x134C);
        s3 += 1;
    }
    // Two more flashes.
    for s3 in 0..s4 {
        let i = first_free(fx, D + 0xFDC, 0x4C, 8);
        if i < 0 {
            break;
        }
        let o = 0x4C * i as u32;
        s0 = u64::from(o);
        let r = D + o;
        set_counter4(fx, r + 0xFDC, 0x16, 0, 0xFF);
        fx.mem.write_u8(r + 0xFE1, 0);
        fx.mem.write_u8(r + 0xFE0, 1);
        fx.mem.write_u8(r + 0xFE3, 9);
        for k in [0xFEBu32, 0xFE8, 0xFE9, 0xFEA] {
            fx.mem.write_u8(r + k, 0xC0);
        }
        fx.mem.write_u8(r + 0xFE6, 0x60);
        fx.mem.write_u8(r + 0xFE5, 0x10);
        fx.mem.write_u8(r + 0xFE4, 0x40);
        fx.mem.write_u32(r + 0xFF0, 0);
        fx.mem.write_u32(r + 0xFEC, 0x43FA_0000);
        if s3 == 0 {
            delta(fx, sp, sp + 0x90, true, false);
        } else {
            delta(fx, sp, D + 0x1E74, true, true);
        }
        back(fx, sp, r + 0xFF8);
    }
    // The camera shake (+0x40) at the target.
    set_counter4(fx, D + 0x40, 0x14, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 1);
    fx.mem.write_u32(D + 0x54, 0x4000_0000);
    fx.mem.write_u32(D + 0x58, 0x3F80_0000);
    fx.mem.write_u32(D + 0x5C, 0x3FB3_3333);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0x80, D + 0x60);
    s0
}

/// The echoes record (+0xFB8) set for `count` updates.
fn echoes_set(fx: &mut Fx, count: i32) {
    set_counter4(fx, D + 0xFB8, count, 0, 0xFF);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    let mut s0 = i64::from(param) as u64;
    unit_position_saving(fx, t, sp + 0x80, s0, 0x01E0_583C);
    match fx.mem.u16(D) {
        0xFA => return 8,
        0xB4 | 0xA6 | 0x96 | 0x84 | 0x78 => {
            let s4 = if fx.mem.u16(D) == 0x78 { 1 } else { 2 };
            s0 = strike(fx, sp, s4);
        }
        0xAB | 0x9A | 0x8C | 0x7C => {
            echoes_set(fx, 9);
            fx.mem.write_u8(D + 0xFCB, 0x30);
            fx.mem.write_u8(D + 0xFC2, 0xC);
            fx.mem.write_u8(D + 0xFC3, 3);
            for o in [0xBCu32, 0xBD, 0xBE, 0xB8, 0xB9, 0xBA] {
                fx.mem.write_u8(sp + o, 0x80);
            }
            fx.mem.write_u8(sp + 0xBB, 0xD0);
            fx.mem.write_u8(sp + 0xBF, 0);
            let h = fx.mem.i32(D + 4);
            fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h as u32, 0x14, 0, sp + 0xB8]);
            let h = fx.mem.i32(D + 4);
            let s = fx.mem.i32(D + 0xC);
            fx.node_position(h, s, D + 0x1E74);
        }
        0x69 => {
            echoes_set(fx, 0xC);
            fx.mem.write_u8(D + 0xFC2, 4);
            fx.mem.write_u8(D + 0xFC3, 3);
        }
        0x58 | 0x49 | 0x37 => {
            echoes_set(fx, 0xA);
            fx.mem.write_u8(D + 0xFBC, 0);
            for (o, v) in [(0xFC0u32, 6u8), (0xFC1, 1), (0xFC2, 6), (0xFC3, 1)] {
                fx.mem.write_u8(D + o, v);
            }
            let h = fx.mem.read_u32(D + 4);
            fx.mem.write_u32(D + 0xFC4, h);
            for o in [0xFC8u32, 0xFC9, 0xFCA] {
                fx.mem.write_u8(D + o, 0x80);
            }
            fx.mem.write_u8(D + 0xFCB, 0x20);
        }
        _ => {}
    }
    shake_saving(fx, D + 0x40, s0, 0x01E0_6388, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6398, [Some(s0), Some(u64::from(D))]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x20);
    blib::star::flash(fx, 0x01E0_37A0, D + 0xFDC, OT, 8);
    blib::ray::rays(fx, 0x01E0_23C0, D + 0x123C, OT, 2);
    blib::streak::ribbon(fx, 0x01E0_4A90, D + 0x1324, OT, 0xA);
    blib::beam::beads(fx, 0x01E0_15F0, D + 0x1A04, OT, 0xA);
    // a3 = what cones left (its table's address, the flags of its last
    // quad, ...) [run].
    let (_, a3) = blib::cone::cones_a3(fx, 0x01E0_2F30, D + 0x1734, OT, 0xA);
    blib::radiate::echoes(fx, 0x01E0_4350, D + 0xFB8, OT, 1, a3);
    let n = fx.mem.u16(D);
    if (0x78..0xB1).contains(&n) {
        6
    } else if n == 0xDE {
        4
    } else if n == 0xB7 {
        2
    } else if n == 0x6E {
        1
    } else {
        0
    }
}
