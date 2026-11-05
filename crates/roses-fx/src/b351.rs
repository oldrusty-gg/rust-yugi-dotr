//! BMODULE b351 (card 187), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06CE0, data area a0 = 0x1ED0000, 0x2760 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then a record pointer, s3 / s4 counters, f20..f22 the half
//! node-to-node offset. Init: the handles (+4 / +8), the attacker's
//! rotation (+0x2C), textures (4, 0xF) and (5, 0), nodes 0x1B (cleared,
//! +0xC) / 0x1A (+0x10) / 0x43 (+0x14), half the target's radius, at least
//! 24 (+0x275C, 0x1F6A70), the light ramp (+0xA0: 0x74 / 0xA9), the random
//! byte table (+0x265C). Every update the target's animated point
//! (0x1F65D0 kind 1) into sp + 0x70; on updates 2..0x28 the local spawner
//! 0x1E069F0 ([`drops`]: five pairs of scatter drops along the node 0x1B
//! -> 0x1A line, placed by the direction table at 0x1E07BB0 turned by
//! (0x28 - n) x 8 degrees). Update 0x28: the rod (+0x25B8, `rod::rods_b`,
//! 0x96 counts, node 0x1B -> 0x1A, 45 / 0.15 / 0.1), the ring record
//! (+0x25F0, `iris::rings`, 0x14 counts, eight rings of kinds 0 / 4 / 2 /
//! 2 / 4 / 0 / 5 / 0 at radii 30..120, alphas 0x80 / 0x40, turned
//! +-0x5800 by D+0x30) at the midpoint of the two nodes, two flashes
//! there (+0x840, `star::flash_b`, 0x14 counts, 80 / 240 wide), four bars
//! (+0xD08, `cross::bars_b`, 0x14 counts, 700 x 190 then 300 x 190 + 45 k)
//! and a shake (+0x40, 0x14). Updates 0x46: the arc (+0xA4, `arc::arcs_b`,
//! pool +0x104, 0x78 counts) node 0x1B -> 0x1A. Update 0x74: one arch
//! (+0x744, `arch::arches_b`, 0x18 counts) from the attacker's point
//! towards the target's over 18 steps, turned -0x4000 / 0x1800 (negated
//! through 0x1A2B80 unless D+0x30 == 0x8000). Update 0x80 at the target's
//! point: four fans (+0x8D8, `wedge::fans`, 0x1A counts, radius +0x275C
//! growing by (k + 1) r / 54, 300 -> 480 / 480 -> 300 / 0 -> 180 / 180 ->
//! 0 degrees, the arch's turn), a flash (700 wide), eight bead beams
//! (+0xA48, `beam::beads_at 34 3c`, 0x18 + rand & 3 counts, delays rand &
//! 7, 15 + 2 k long, 1.6 + (rand % 100 - 50) / 100 wide) and a shake
//! (0x18) at the target (0x1F71A0). Returns 6 on 0x80..0x94, 2 on 0x95, 1
//! on 0x76, 8 on 0xAF (no drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080 (0538257fe1fa),
//! `arcs_b` 0x1E003A0, `arches_b` 0x1E014C0, `beads_at` 0x1E01D20,
//! `fans` 0x1E029A0, `flash_b` 0x1E02E20, `bars_b` 0x1E03890,
//! `scatter_drops_b` 0x1E04190, `rods_b` 0x1E04B30, `rings` 0x1E05B10
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const LIGHT: u32 = D + 0xA0;
const ARC: u32 = D + 0xA4;
const ARCH: u32 = D + 0x744;
const FLASH: u32 = D + 0x840;
const FANS: u32 = D + 0x8D8;
const BEADS: u32 = D + 0xA48;
const BARS: u32 = D + 0xD08;
const DROPS: u32 = D + 0xE48;
const ROD: u32 = D + 0x25B8;
const RINGS: u32 = D + 0x25F0;
const BYTES: u32 = D + 0x265C;
const RADIUS: u32 = D + 0x275C;
/// The spawner's five direction rows (rodata after the code).
const TABLE: u32 = 0x01E0_7BB0;
const PI: u32 = 0x4049_0FDB;

/// The entry 0x1E06CE0 (a0 = D, a1 = param).
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
    // 0x199208 memset(D, 0, 0x2760).
    fx.bzero(D, 0x2760);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 0x1B, 1), (0x10, 0x1A, 0), (0x14, 0x43, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    let r = blib::afterimage::unit_radius(fx, t);
    let v = ee::mul(0.5, r);
    fx.mem.setf(RADIUS, v);
    if ee_lt(v, 24.0) {
        fx.mem.write_u32(RADIUS, 0x41C0_0000);
    }
    fx.mem.set16(LIGHT, 0x74);
    // The `sh` of +0xA2 sits in rand_bytes' delay slot: the constant.
    fx.mem.set16(LIGHT + 2, 0xA9);
    // The entry's s0 = D, s1 = 1 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6E24, [Some(D), Some(1), Some(param as u32)]);
    0
}

/// The camera shake (+0x40) [F]: `count` counts, kind 1 at `pos` (copied
/// after), amplitudes 1.8 / 1.8 / 1, far 40, rate 4.
fn shake(fx: &mut Fx, count: i32) {
    set_counter4(fx, SHAKE, count, 0, 0xFF);
    fx.mem.write_u8(SHAKE + 5, 0);
    fx.mem.write_u8(SHAKE + 4, 1);
    fx.mem.write_u32(SHAKE + 0x14, 0x3FE6_6666);
    fx.mem.write_u32(SHAKE + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(SHAKE + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(SHAKE + 0x30, 0);
    fx.mem.write_u32(SHAKE + 0x34, 0x4220_0000);
    fx.mem.write_u32(SHAKE + 0x50, 0x4080_0000);
}

/// 0x1E069F0 (311f84691901, this record's own; a1 unused) [F]: five pairs
/// of scatter drops (+0xE48, 100 slots) on the k / 4 points of the node
/// +0xC -> +0x10 line (k 0..4), each displaced by the table row k
/// (0x1E07BB0, copied to sp + 0x130) aimed (0x101A00) and applied
/// (0x20C540) to (r cos a, 0, r sin a), r = 0.4 (0x28 - n), a = 45 k + 8
/// (0x28 - n) degrees; 9 counts, mask 7, flags 0, cell 3 of layout 2,
/// alpha 0x80, 35 wide, spread 0x12C. Frame 0x180: s0..s7, ra, f20..f27;
/// the entry's s0 / s1 (D / the OT) saved by the node_position calls, the
/// local's s0 (D then 2) / s1 (k) by 0x101A00.
fn drops(fx: &mut Fx) {
    let sp = fx.enter(0x180);
    for k in 0..5u32 {
        for h in [0u32, 8] {
            let v = fx.mem.read_u64(TABLE + 0x10 * k + h);
            fx.mem.write_u64(sp + 0x130 + 0x10 * k + h, v);
        }
    }
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, sp + 0xB0, u64::from(D), Some(u64::from(OT)), 0x01E0_6A80);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, s, sp + 0xC0, u64::from(D), Some(u64::from(OT)), 0x01E0_6A94);
    let quarter: [f32; 3] = std::array::from_fn(|c| ee::div(ee::sub(fx.mem.f(sp + 0xB0 + 4 * c as u32), fx.mem.f(sp + 0xC0 + 4 * c as u32)), 4.0));
    let left = ee::from_i32(0x28 - i32::from(fx.mem.u16(D)));
    let f26 = ee::mul(f32::from_bits(0x3ECC_CCCD), left);
    let f27 = ee::mul(8.0, left);
    let mut s0 = u64::from(D);
    for k in 0..5i32 {
        blib::spray::aim_rotation(fx, sp + 0x130 + 0x10 * k as u32, sp + 0xF0, [s0, k as u64], 0x01E0_6B1C);
        let a = ee::add(ee::mul(45.0, ee::from_i32(k)), f27);
        let ang = ee::div(ee::mul(f32::from_bits(PI), a), 180.0);
        let c = libm_cosf(fx, ang);
        fx.mem.setf(sp + 0xD0, ee::mul(f26, c));
        fx.mem.set32(sp + 0xD4, 0);
        let s = fx.libm_sinf(ang);
        fx.mem.setf(sp + 0xD8, ee::mul(f26, s));
        let m = fx.read_m4(sp + 0xF0);
        let v = std::array::from_fn(|c| fx.mem.f(sp + 0xD0 + 4 * c as u32));
        let e = fx.transform(Some(&m), v);
        for (c, x) in e.into_iter().enumerate() {
            fx.mem.setf(sp + 0xE0 + 4 * c as u32, x);
        }
        let kf = ee::from_i32(k);
        let off: [f32; 3] = std::array::from_fn(|c| ee::mul(quarter[c], kf));
        s0 = 2;
        for _ in 0..2 {
            let i = first_free(fx, DROPS, 0x3C, 0x64);
            if i < 0 {
                continue;
            }
            let r = DROPS + 0x3C * i as u32;
            set_counter4(fx, r, 9, 0, 7);
            for (o, v) in [(5u32, 0u8), (4, 0), (8, 0), (9, 3), (0xA, 2), (0xB, 0), (0xF, 0x80), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.write_u32(r + 0x10, 0x420C_0000);
            fx.mem.set32(r + 0x14, 0x12C);
            for c in 0..3u32 {
                let n = ee::add(fx.mem.f(sp + 0xC0 + 4 * c), off[c as usize]);
                let v = ee::add(fx.mem.f(sp + 0xE0 + 4 * c), n);
                fx.mem.setf(r + 0x18 + 4 * c, v);
            }
        }
    }
    fx.leave(0x180);
}

/// Update 0x28 [F 0x1E06E94]: the rod, the rings at the midpoint of nodes
/// +0xC / +0x10, two flashes, four bars and a shake. Returns the entry's
/// (s2, s3, f22).
fn rings(fx: &mut Fx, sp: u32) -> (u64, u64, f32) {
    set_counter4(fx, ROD, 0x96, 0, 0xFF);
    fx.mem.write_u8(ROD + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(ROD + 8, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(ROD + 9, b);
    fx.mem.write_u8(ROD + 4, 2);
    fx.mem.write_u32(ROD + 0x10, 0x3E19_999A);
    fx.mem.write_u32(ROD + 0x14, 0x3DCC_CCCD);
    fx.mem.write_u32(ROD + 0xC, 0x4234_0000);
    for (o, v) in [(0x1Bu32, 0xC0u8), (0x18, 0xC0), (0x19, 0xC0), (0x1A, 0xC0), (0x1E, 0x80), (0x1D, 0x80), (0x1C, 0), (0x1F, 0)] {
        fx.mem.write_u8(ROD + o, v);
    }
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, sp + 0x80, u64::from(D), Some(u64::from(OT)), 0x01E0_6F1C);
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, s, sp + 0x90, u64::from(D), Some(u64::from(OT)), 0x01E0_6F30);
    let half: [f32; 3] = std::array::from_fn(|c| ee::mul(0.5, ee::sub(fx.mem.f(sp + 0x80 + 4 * c as u32), fx.mem.f(sp + 0x90 + 4 * c as u32))));
    set_counter4(fx, RINGS, 0x14, 0, 0xFF);
    fx.mem.write_u8(RINGS + 5, 0);
    fx.mem.write_u8(RINGS + 0x10, 8);
    fx.mem.write_u8(RINGS + 0x11, 0x80);
    fx.mem.write_u8(RINGS + 0x12, 0x40);
    fx.mem.write_u32(RINGS + 0x34, 0x4080_0000);
    fx.mem.write_u32(RINGS + 0x38, 0x3F99_999A);
    for c in 0..3u32 {
        let v = ee::add(fx.mem.f(sp + 0x90 + 4 * c), half[c as usize]);
        fx.mem.setf(RINGS + 0x3C + 4 * c, v);
    }
    if fx.mem.read_u32(D + 0x30) == 0x8000 {
        fx.mem.set32(RINGS + 0x4C, -0x5800);
    } else {
        fx.mem.set32(RINGS + 0x4C, 0x5800);
    }
    fx.mem.set32(RINGS + 0x60, 0x80);
    for (j, (kind, radius)) in [(0u8, 0x41F0_0000u32), (4, 0x4270_0000), (2, 0x42C8_0000), (2, 0x4220_0000), (4, 0x42F0_0000), (0, 0x4270_0000), (5, 0x42F0_0000), (0, 0x41F0_0000)].into_iter().enumerate() {
        fx.mem.write_u8(RINGS + 8 + j as u32, kind);
        fx.mem.write_u32(RINGS + 0x14 + 4 * j as u32, radius);
    }
    // Two flashes (+0x840, 0x4C apart), 80 + 160 k wide.
    for k in 0..2i32 {
        let f = FLASH + 0x4C * k as u32;
        set_counter4(fx, f, 0x14, 0, 0xFF);
        fx.mem.write_u8(f + 5, 0);
        fx.mem.write_u8(f + 4, 0x81);
        fx.mem.write_u8(f + 7, 9);
        let w = ee::add(80.0, ee::mul(160.0, ee::from_i32(k)));
        fx.mem.setf(f + 0x10, w);
        fx.mem.write_u32(f + 0x14, 0);
        for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x80), (8, 0), (0xF, 0xB0)] {
            fx.mem.write_u8(f + o, v);
        }
        copy_v4(fx, RINGS + 0x3C, f + 0x1C);
    }
    // Four bars (+0xD08, 0x50 apart).
    for k in 0..4i32 {
        let b = BARS + 0x50 * k as u32;
        set_counter4(fx, b, 0x14, 0, 0xFF);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 4, 0x80);
        if k == 0 {
            fx.mem.write_u32(b + 0x10, 0x442F_0000);
            fx.mem.write_u32(b + 0x18, 0x433E_0000);
        } else {
            fx.mem.write_u32(b + 0x10, 0x4396_0000);
            let h = ee::add(190.0, ee::mul(45.0, ee::from_i32(k)));
            fx.mem.setf(b + 0x18, h);
        }
        fx.mem.write_u32(b + 0x14, 0x41A0_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x20), (0xA, 0x80), (9, 0x80), (8, 0)] {
            fx.mem.write_u8(b + o, v);
        }
        fx.mem.write_u32(b + 0x1C, 0x3FC0_0000);
        copy_v4(fx, RINGS + 0x3C, b + 0x20);
    }
    shake(fx, 0x14);
    copy_v4(fx, RINGS + 0x3C, SHAKE + 0x20);
    (u64::from(D + 0x140), 4, half[2])
}

/// Update 0x46 [F 0x1E07268]: the arc (+0xA4, pool +0x104 of 8 + 1) from
/// node +0xC to node +0x10.
fn arc(fx: &mut Fx) {
    set_counter4(fx, ARC, 0x78, 0, 0xFF);
    fx.mem.write_u8(ARC + 4, 1);
    fx.mem.write_u32(ARC + 8, D + 0x104);
    fx.mem.write_u8(ARC + 0x30, 0x64);
    fx.mem.write_u8(ARC + 0x31, 8);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(ARC + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(ARC + 0x33, b);
    fx.mem.write_u32(ARC + 0x38, 0x3F93_3333);
    fx.mem.write_u8(ARC + 0x34, 0);
    fx.mem.write_u16(ARC + 0x36, 0);
    fx.mem.write_u32(ARC + 0x40, 0x4120_0000);
    for (o, v) in [(0x47u32, 0xC0u8), (0x44, 0xC0), (0x45, 0xC0), (0x46, 0xC0), (0x4A, 0x80), (0x49, 0x80), (0x48, 0x10)] {
        fx.mem.write_u8(ARC + o, v);
    }
    let mut t0 = 0;
    while t0 < fx.mem.u8(ARC + 0x31) + 1 {
        fx.mem.set16(ARC + 0x1E + 2 * t0 as u32, -1);
        fx.mem.set16(ARC + 0xC + 2 * t0 as u32, -1);
        t0 += 1;
    }
}

/// Update 0x74 [F 0x1E07330]: the arch (+0x744) from the attacker's point
/// (0x1F65D0 kind 1) towards the target's (sp + 0x70) in 18 steps; the
/// loop runs once (s3 = 0).
fn arch(fx: &mut Fx, sp: u32) {
    set_counter4(fx, ARCH, 0x18, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xF, 0xC0), (0xA, 0xC0), (9, 0xC0), (8, 0), (0xB, 0x10)] {
        fx.mem.write_u8(ARCH + o, v);
    }
    fx.mem.write_u32(ARCH + 0x10, 0x4120_0000);
    fx.mem.write_u32(ARCH + 0x14, 0x4000_0000);
    fx.mem.write_u32(ARCH + 0x18, 0x3FC0_0000);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, ARCH + 0x24);
    // 0x1F65D0's 0x50 frame (s0 = D, s1 = OT, s2 = D, s3 = 0, ra) [F].
    fx.saved(fx.sp - 0x50, &[(0, u64::from(D)), (0x10, u64::from(OT)), (0x20, u64::from(D)), (0x30, 0), (0x40, 0x01E0_73A8)]);
    for c in 0..3u32 {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x70 + 4 * c), fx.mem.f(ARCH + 0x24 + 4 * c)), 18.0);
        fx.mem.setf(ARCH + 0x34 + 4 * c, v);
    }
    fx.mem.set32(ARCH + 0x44, -0x4000);
    fx.mem.set32(ARCH + 0x4C, 0x1800);
    let z = ee::add(fx.mem.f(ARCH + 0x2C), 0.0);
    fx.mem.setf(ARCH + 0x2C, z);
    if fx.mem.read_u32(D + 0x30) == 0x8000 {
        return;
    }
    for (o, ra) in [(0x44u32, 0x01E0_743Cu32), (0x4C, 0x01E0_745C)] {
        let f = ee::mul(ee::from_i32(fx.mem.i32(ARCH + o)), -1.0);
        fx.fix_residue(f, ra);
        fx.mem.set32(ARCH + o, ee::fixsfsi(f));
    }
    let z = ee::sub(fx.mem.f(ARCH + 0x2C), ee::mul(2.0, 0.0));
    fx.mem.setf(ARCH + 0x2C, z);
}

/// Update 0x80 [F 0x1E074A0]: four fans, the first flash, eight bead beams
/// at the target's point (sp + 0x70) and a shake at the target. Returns the
/// entry's (s2, s3, s4).
fn strike(fx: &mut Fx, sp: u32) -> (u64, u64, u64) {
    for k in 0..4i32 {
        let f = FANS + 0x5C * k as u32;
        set_counter4(fx, f, 0x1A, 0, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (8, 3), (9, 8)] {
            fx.mem.write_u8(f + o, v);
        }
        let grow = ee::mul(f32::from_bits(0x3ECC_CCCD), ee::from_i32(k));
        fx.mem.setf(f + 0x14, ee::add(3.0, grow));
        let r = fx.mem.f(RADIUS);
        fx.mem.setf(f + 0x1C, r);
        let r = fx.mem.f(RADIUS);
        fx.mem.setf(f + 0x20, ee::div(ee::mul(ee::from_i32(k + 1), r), 54.0));
        for (o, v) in [(0x2Au32, 0xC0u8), (0x29, 0xC0), (0x28, 0), (0x2B, 0), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0xC0)] {
            fx.mem.write_u8(f + o, v);
        }
        copy_v4(fx, sp + 0x70, f + 0x2C);
        let (from, to, speed, step) = match k {
            0 => (0x4396_0000u32, 0x43F0_0000u32, 0x3FB3_3333u32, 0x4040_0000u32),
            1 => (0x43F0_0000, 0x4396_0000, 0x3FB3_3333, 0x4040_0000),
            2 => (0, 0x4334_0000, 0x400C_CCCD, 0x4080_0000),
            _ => (0x4334_0000, 0, 0x400C_CCCD, 0x4080_0000),
        };
        fx.mem.write_u32(f + 0xC, from);
        fx.mem.write_u32(f + 0x10, to);
        fx.mem.write_u32(f + 0x18, speed);
        fx.mem.write_u32(f + 0x14, step);
        let t = fx.mem.read_u32(ARCH + 0x4C);
        fx.mem.write_u32(f + 0x54, t);
        if fx.mem.read_u32(D + 0x30) != 0x8000 {
            fx.mem.write_u32(f + 0x4C, 0x8000);
        }
    }
    set_counter4(fx, FLASH, 0x1A, 0, 0xFF);
    fx.mem.write_u8(FLASH + 5, 0);
    fx.mem.write_u8(FLASH + 4, 0x81);
    fx.mem.write_u8(FLASH + 7, 9);
    fx.mem.write_u32(FLASH + 0x10, 0x442F_0000);
    fx.mem.write_u32(FLASH + 0x14, 0);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x80), (8, 0)] {
        fx.mem.write_u8(FLASH + o, v);
    }
    copy_v4(fx, sp + 0x70, FLASH + 0x1C);
    // Eight bead beams (+0xA48, 0x58 apart, the first free slot of 8).
    let (mut s2, mut s4) = (0u64, 0u64);
    for s3 in 0..8i32 {
        let i = first_free(fx, BEADS, 0x58, 8);
        s2 = i as i64 as u64;
        if i < 0 {
            continue;
        }
        let v = fx.rand_from(D, 0x01E0_7704);
        let count = (((v >> 4) & 3) + 0x18) & 0xFF;
        s4 = count as u64;
        let v = fx.rand_from(D, 0x01E0_7718);
        let b = BEADS + 0x58 * i as u32;
        s2 = u64::from(D + 0x58 * i as u32);
        set_counter4(fx, b, count, (v >> 4) & 7, 0xFF);
        fx.mem.write_u8(b + 4, 5);
        fx.mem.write_u8(b + 5, 0);
        // The `sb` of +8 sits in rand's delay slot: the constant 0x28.
        fx.mem.write_u8(b + 8, 0x28);
        let v = fx.rand_from(D, 0x01E0_7760);
        fx.mem.set8(b + 9, v >> 4);
        fx.mem.write_u8(b + 0xA, 0);
        fx.mem.write_u8(b + 0xB, 3);
        fx.mem.write_u8(b + 0xC, 2);
        let w = ee::add(15.0, ee::mul(2.0, ee::from_i32(s3)));
        fx.mem.setf(b + 0x1C, w);
        fx.mem.write_u32(b + 0x20, 0);
        fx.mem.write_u32(b + 0x10, BYTES);
        let v = fx.rand_from(D, 0x01E0_77B4);
        let (_, rem) = fx.div(v >> 4, 100);
        let spread = ee::div(ee::from_i32(rem - 50), 100.0);
        fx.mem.setf(b + 0x14, ee::add(f32::from_bits(0x3FCC_CCCD), spread));
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x90);
        }
        copy_v4(fx, sp + 0x70, b + 0x28);
    }
    shake(fx, 0x18);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, SHAKE + 0x20, u64::from(D), 0x01E0_78A0);
    (s2, 8, s4)
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, sp + 0x70);
    // 0x1F65D0's 0x50 frame (s0 = D, s1 = OT, s2 = the parameter, s3 (the
    // runner's 0), ra) [F].
    fx.saved(sp - 0x50, &[(0, u64::from(D)), (0x10, u64::from(OT)), (0x20, u64::MAX), (0x30, 0), (0x40, 0x01E0_6E48)]);
    // The entry's s2 / s3 / s4 and f22 after the block (the dispatch's
    // delay slots zero s2 / s3) [F]: the library prologues save them.
    let mut f22 = 0.0f32;
    let (s2, s3, s4): (u64, u64, u64) = match fx.mem.u16(D) {
        0xAF => return 8,
        0x80 => strike(fx, sp),
        0x74 => {
            arch(fx, sp);
            (u64::from(D + 0x54), 1, 0)
        }
        0x46 => {
            arc(fx);
            (0, 0, 0)
        }
        0x28 => {
            let (s2, s3, z) = rings(fx, sp);
            f22 = z;
            (s2, s3, u64::from(sp + 0x98))
        }
        _ => (0, 0, 0),
    };
    let _ = (s2, s3, s4);
    let n = fx.mem.u16(D);
    if (2..0x29).contains(&n) {
        drops(fx);
    }
    shake_saving(fx, SHAKE, u64::from(D), 0x01E0_7908, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, LIGHT, n, 0x01E0_7914, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::wedge::fans(fx, 0x01E0_29A0, FANS, OT, 4);
    blib::arc::arcs_b(fx, 0x01E0_03A0, ARC, OT, 1);
    blib::arch::arches_b(fx, 0x01E0_14C0, ARCH, OT, 3);
    blib::beam::beads_at(fx, 0x01E0_1D20, 0x34, 0x3C, BEADS, OT, 8);
    blib::drop::scatter_drops_b(fx, 0x01E0_4190, DROPS, OT, 0x64);
    blib::star::flash_b(fx, 0x01E0_2E20, FLASH, OT, 2);
    blib::cross::bars_b(fx, 0x01E0_3890, BARS, OT, 4);
    // [H] f22 = the runner's (0) but on update 0x28 (the half z offset).
    blib::rod::rods_b(fx, 0x01E0_4B30, ROD, OT, 1, f22);
    blib::iris::rings(fx, 0x01E0_5B10, RINGS, OT, 1);
    let n = fx.mem.u16(D);
    if (0x80..0x95).contains(&n) {
        6
    } else if n == 0x95 {
        2
    } else if n == 0x76 {
        1
    } else {
        0
    }
}
