//! BMODULE b469 (card 124), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E042D0, data area a0 = 0x1ED0000) [F] (helper B5, 31
//! Oct 2026): b076's "a0 = D" family (s0 = D, s1 = the OT). On update 40
//! two long flashes on the attacker's nodes 0x1A / 0x2B (+0x1904), on 57 a
//! third (+0x199C), on 60 a camera shake at the target (+0x40), eight bead
//! beams from node 0x1A (+0x3CB4, staggered) and four jagged bolts
//! (0x102A50, six segments each, `bolt::bolts` +0x1DC4) node -> target; on
//! 76 the flashes widen; on 118 a wide flash at the target and two segment
//! spawners (+0x2CC4, `emit::segment_spawns`, feeding the bolt pool); on
//! even updates 32..191 two ribbons a update from the nodes (+0xA4, pool
//! of 60); on updates 118..254 two flashes and two ribbons a update at
//! random points about the target; the stage light pulsed by 0x1031C0
//! (+0xA0: down on update 2, up on 264). The flicker pool (+0x2EA4,
//! `flicker::flickers`) is drawn but never filled. Returns 1 on update
//! 108, 6 on 118..254, 2 on 255, 8 on 270 (and at once for a parameter of
//! -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `ribbon` 0x1E00FC0, `bolts` 0x1E01B50, `segment_spawns`
//! 0x1E026A0, `flickers` 0x1E02A20, `beads` 0x1E03500 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b586::segment_spawns_saving;
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x4394;
const FLASHES: u32 = D + 0x1904;
const BOLTS: u32 = D + 0x1DC4;
const RIBBONS: u32 = D + 0xA4;

/// The entry 0x1E042D0 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x1A0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x44A0);
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
    for (at, node, clear) in [(0xCu32, 0x1A, 1), (0x10, 0x2B, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x108);
    // The entry's s0 = D, s1 = the OT, s2 = 2 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_43EC, [Some(D), Some(OT), Some(2)]);
    0
}

/// The flash bytes +4 / +7 / +0xA / +9 / +8 and the colour +0xF, +0xC..E.
pub(crate) fn flash_bytes(fx: &mut Fx, r: u32, b4: u8, a: u8, b9: u8, b8: u8, c: u8) {
    for (o, v) in [(4u32, b4), (7, 9), (0xA, a), (9, b9), (8, b8), (0xF, c), (0xC, c), (0xD, c), (0xE, c)] {
        fx.mem.write_u8(r + o, v);
    }
}

/// Update 40 [F 0x1E04468]: two long flashes on nodes +0xC / +0x10.
fn charge(fx: &mut Fx) {
    for k in 0..2u32 {
        let r = FLASHES + 0x4C * k;
        set_counter4(fx, r, 0xA0, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        flash_bytes(fx, r, 0x85, 0x40, 0x20, 0x80, 0xC0);
        fx.mem.write_u32(r + 0x10, 0x4316_0000);
        fx.mem.write_u32(r + 0x14, 0);
    }
}

/// Update 57 [F 0x1E04524]: a third flash on node +0xC.
pub(crate) fn charge2(fx: &mut Fx) {
    let r = D + 0x199C;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    flash_bytes(fx, r, 0x81, 0x40, 0x20, 0x80, 0xF0);
    fx.mem.write_u32(r + 0x10, 0x4348_0000);
    fx.mem.write_u32(r + 0x14, 0x4316_0000);
}

/// Update 60 [F 0x1E045A0]: the shake at the target (sp + 0x60), eight
/// bead beams on node +0xC and four jagged bolts node -> target.
fn bolts_start(fx: &mut Fx, sp: u32) {
    let s = D + 0x40;
    set_counter4(fx, s, 0xC8, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    beads8(fx, sp, [0x01E0_4694, 0x01E0_46F0, 0x01E0_4718]);
    for j in 0..4u32 {
        jag_from(fx, sp, 0x3F00_0000);
        for s in 0..6u32 {
            let i = first_free(fx, BOLTS, 0x40, 0x3C);
            if i < 0 {
                break;
            }
            let b = BOLTS + ((i as u32) << 6);
            set_counter4(fx, b, 0x8C, ((j + 9 * s) & 0xFF) as i32, 0xFF);
            bolt_bytes(fx, sp, b, s, 9, [0x3F33_3333, 0x3E4C_CCCD], 3);
        }
    }
}

/// The eight bead beams (+0x3CB4 + 0x58 k, delays k) on node +0xC and that
/// node's position into sp + 0x80 [F b469 0x1E0461C, b110 0x1E0477C]; `ras`
/// the rand / node-position return addresses.
pub(crate) fn beads8(fx: &mut Fx, sp: u32, ras: [u32; 3]) {
    for k in 0..8i32 {
        let r = D + 0x3CB4 + 0x58 * k as u32;
        set_counter4(fx, r, 0x1C, k & 0xFF, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(4);
        fx.mem.write_u8(r, c);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x19);
        fx.mem.set8(r + 9, k);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xC, 2);
        let x = fx.rand_from(D, ras[0]) >> 4;
        fx.mem.set8(r + 0xB, x & 3);
        fx.mem.write_u32(r + 0x1C, 0x420C_0000);
        fx.mem.write_u32(r + 0x20, 0xBF19_999A);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0xC);
        node_position_saving(fx, h, n, r + 0x28, u64::from(D), Some(u64::from(OT)), ras[1]);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(OT)), ras[2]);
}

/// A jagged path of six segments (0x102A50 on the spec at sp + 0xA0,
/// jitter `j4`) from sp + 0x80 to the target sp + 0x60 into sp + 0xD0.
pub(crate) fn jag_from(fx: &mut Fx, sp: u32, j4: u32) {
    jag_at(fx, sp, sp + 0x80, j4);
}

/// [`jag_from`] from the point at `from`.
pub(crate) fn jag_at(fx: &mut Fx, sp: u32, from: u32, j4: u32) {
    let p = sp + 0xA0;
    fx.mem.set32(p, 6);
    fx.mem.write_u32(p + 4, j4);
    fx.mem.write_u32(p + 0x20, sp + 0xD0);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(p + 8 + 4 * c, w);
    }
    for c in 0..3u32 {
        let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
        fx.mem.write_u32(p + 0x14 + 4 * c, w);
    }
    blib::bolt::jag(fx, p);
}

/// Segment `s` of the jagged path (sp + 0xD0 + 12 s, two points) into the
/// bolt `b` (width +8, sizes +0x10 / +0x14, +0xD), white, alpha 0x20.
pub(crate) fn bolt_bytes(fx: &mut Fx, sp: u32, b: u32, s: u32, w: u8, sizes: [u32; 2], bd: u8) {
    fx.mem.write_u8(b + 5, 0);
    fx.mem.write_u8(b + 4, 1);
    for c in 0..6u32 {
        let v = fx.mem.read_u32(sp + 0xD0 + 12 * s + 4 * c);
        fx.mem.write_u32(b + 0x1C + 4 * c, v);
    }
    fx.mem.write_u8(b + 8, w);
    fx.mem.write_u32(b + 0x10, sizes[0]);
    fx.mem.write_u32(b + 0x14, sizes[1]);
    for (o, v) in [(0xBu32, 1u8), (0xD, bd), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
        fx.mem.write_u8(b + o, v);
    }
}

/// Update 118 [F 0x1E048B8]: a wide flash `r` at the target (sp + 0x60)
/// and two segment spawners (+0x2CC4 + 0x3C k), with counts `counts`.
pub(crate) fn strike(fx: &mut Fx, sp: u32, r: u32, counts: [i32; 2]) {
    set_counter4(fx, r, counts[0], 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    flash_bytes(fx, r, 0x83, 0x40, 0x20, 0x80, 0x80);
    fx.mem.write_u32(r + 0x10, 0x447A_0000);
    fx.mem.write_u32(r + 0x14, 0);
    copy_v4(fx, sp + 0x60, r + 0x1C);
    for k in 0..2u32 {
        let r = D + 0x2CC4 + 0x3C * k;
        set_counter4(fx, r, counts[1], 0, 3);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u32(r + 8, BOLTS);
        for (o, v) in [(0xCu32, 0x28u8), (0xD, 0xC), (0xE, 1), (0xF, 0), (0x10, 5), (0x11, 0xC)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.set32(r + 0x14, 0x1770);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        fx.mem.write_u32(r + 0x1C, 0x3ECC_CCCD);
        for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(r + 0x24 + 4 * c, w);
        }
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(r + o, 0);
        }
    }
}

/// Even updates 32..191 [F 0x1E04A50]: a ribbon from each of nodes +0xC /
/// +0x10 (thin before update 70).
fn ribbons(fx: &mut Fx) {
    let look = [(0x3F4C_CCCD, 0x4170_0000, 0xC), (0x3FE6_6666, 0x41C8_0000, 0x12)];
    for k in 0..2u32 {
        ribbon_from(fx, D + 0xC + 4 * k, 0x46, look, [0x01E0_4B40, 0x01E0_4B90, 0x01E0_4BA8, 0x01E0_4BC0], BYTES);
    }
}

/// A ribbon's look before / from the threshold update: (+0x14, +0x1C, +8).
pub(crate) type Look = [(u32, u32, u8); 2];

/// A ribbon (pool +0xA4) from the node named by the byte at `node`, looking
/// `look[0]` before update `until`, `look[1]` from it, random bytes
/// `bytes`; `ras` its rands' return addresses [F b469 0x1E04A5C, b110 0x1E04BB0].
pub(crate) fn ribbon_from(fx: &mut Fx, node: u32, until: i32, look: Look, ras: [u32; 4], bytes: u32) {
    {
        let i = first_free(fx, RIBBONS, 0x68, 0x3C);
        if i < 0 {
            return;
        }
        let r = RIBBONS + 0x68 * i as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        fx.mem.write_u8(r + 4, 5);
        let (a, b, c) = look[usize::from(fx.mem.u16(D) >= until)];
        fx.mem.write_u32(r + 0x14, a);
        fx.mem.write_u32(r + 0x1C, b);
        fx.mem.write_u8(r + 8, c);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(node);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xB, 2);
        fx.mem.write_u8(r + 0xC, 3);
        let x = fx.rand_from(D, ras[0]);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, bytes);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        fx.mem.write_u8(r + 0x23, 0x10);
        fx.mem.write_u8(r + 0x27, 0x40);
        for (o, ra) in [(0x58u32, ras[1]), (0x5C, ras[2]), (0x60, ras[3])] {
            let x = fx.rand_from(D, ra) >> 4;
            fx.mem.set32(r + o, x & 0x7F);
        }
    }
}

/// Updates 118..254 [F 0x1E04C10]: twice a update, a point about the
/// target (x, y +- 14 at random, sp + 0x70; its w the stale sp + 0x7C)
/// gets a flash and a ribbon.
pub(crate) fn sparkle(fx: &mut Fx, sp: u32, ras: [u32; 3]) {
    for _ in 0..2 {
        for (c, ra) in [(0u32, ras[0]), (4, ras[1])] {
            let x = fx.rand_from(D, ra) >> 4;
            let (_, m) = fx.div(x, 0xAF0);
            let j = ee::div(ee::from_i32(m - 0x578), 100.0);
            let v = ee::add(fx.mem.f(sp + 0x60 + c), j);
            fx.mem.setf(sp + 0x70 + c, v);
        }
        let w = fx.mem.read_u32(sp + 0x68);
        fx.mem.write_u32(sp + 0x78, w);
        let i = first_free(fx, FLASHES, 0x4C, 0x10);
        if i >= 0 {
            let r = FLASHES + 0x4C * i as u32;
            set_counter4(fx, r, 0xC, 0, 0xFF);
            fx.mem.write_u8(r + 5, 0);
            flash_bytes(fx, r, 1, 0x60, 0x10, 0x40, 0xC0);
            fx.mem.write_u32(r + 0x14, 0);
            fx.mem.write_u32(r + 0x10, 0x4396_0000);
            copy_v4(fx, sp + 0x70, r + 0x1C);
        }
        spark_ribbon(fx, sp, ras[2], BYTES);
    }
}

/// A short wide ribbon (pool +0xA4) at the point sp + 0x70, random bytes
/// `bytes`, its rand returning to `ra` [F b469 0x1E04D78].
pub(crate) fn spark_ribbon(fx: &mut Fx, sp: u32, ra: u32, bytes: u32) {
    {
        let i = first_free(fx, RIBBONS, 0x68, 0x3C);
        if i < 0 {
            return;
        }
        let r = RIBBONS + 0x68 * i as u32;
        fx.mem.write_u8(r + 4, 5);
        set_counter4(fx, r, 8, 0, 0xFF);
        fx.mem.write_u32(r + 0x14, 0x40A0_0000);
        fx.mem.write_u32(r + 0x1C, 0x4220_0000);
        for (o, v) in [(0x27u32, 0xC0u8), (0x23, 0), (5, 0), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0x28)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(D, ra);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, bytes);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x70, r + 0x28);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_4414);
    match fx.mem.u16(D) {
        0x10E => return 8,
        0x76 => strike(fx, sp, D + 0x199C, [0x84, 0x80]),
        0x4C => {
            fx.mem.write_u32(D + 0x1960, 0x4348_0000);
            fx.mem.write_u32(D + 0x1914, 0x4348_0000);
        }
        0x3C => bolts_start(fx, sp),
        0x39 => charge2(fx),
        0x28 => charge(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x20..0xC0).contains(&n) && n & 1 == 0 {
        ribbons(fx);
    }
    let n = fx.mem.u16(D);
    if (0x76..0xFF).contains(&n) {
        sparkle(fx, sp, [0x01E0_4C18, 0x01E0_4C64, 0x01E0_4E18]);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_4E9C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_4EAC, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::star::flash(fx, 0x01E0_0410, FLASHES, OT, 0x10);
    blib::streak::ribbon(fx, 0x01E0_0FC0, RIBBONS, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_26A0, D + 0x2CC4, 8, 0x01E0_4EE8);
    blib::bolt::bolts(fx, 0x01E0_1B50, BOLTS, OT, 0x28);
    blib::flicker::flickers(fx, 0x01E0_2A20, D + 0x2EA4, OT, 0x3C);
    blib::beam::beads(fx, 0x01E0_3500, D + 0x3CB4, OT, 0x14);
    let n = fx.mem.u16(D);
    if (0x76..0xFF).contains(&n) {
        6
    } else if n == 0xFF {
        2
    } else if n == 0x6C {
        1
    } else {
        0
    }
}
