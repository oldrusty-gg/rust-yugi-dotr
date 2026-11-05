//! BMODULE b062 (card 273), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05C40, data area a0 = 0x1ED0000, 0x9A80 bytes)
//! [F]: s6 = D, the OT at sp + 0xA8, s0 the
//! parameter then counters / pointers. Updates 0x3A..0x3D / 0x4B..0x4E a
//! window tint (0x2079D0); 0xA0 the centre of nodes 0x13 / 0x1B and nodes
//! 0x61 / 0x42 (+0x3C / +0x48 / +0x54), their centroid (+0x60) and a long
//! shake (+0x70) there; 0xA2 three pairs of flashes and three halos on
//! the three points; 0xCD three ribbons (+0x938C, `dash::ribbons`)
//! between them, stretching 6 an update to 0xF4; 0xE0 twenty comets
//! (+0x6064, `comet::comets_b`) circling the centroid (the record's
//! 0x1E05AA0 steers them every update); 0x104 the fade (+0x605C) and two
//! flashes; 0x105 two tubes (+0x7B7C, `segtube::tubes`) from the centroid
//! to the target, two flashes, two domes (+0x7BD4, `dome::domes`) and
//! three helix ribbons (+0x9914, their paths redrawn by 0x102CB0 while
//! alive); 0x15C the black fade. Drops (+0x594, `drop::drops`) on
//! 0x5F..0x91 from the node pairs and 0xA0..0xC7 from the three points,
//! spikes (+0x47FC, `spike::spikes`) on odd 0xA3..0x111 at the points and
//! odd 0xFD..0x167 at the target / centroid, sprays (+0x7CC4,
//! `spray::sprays_b`) 0x113..0x167 at the target. Returns 6 on updates
//! 0x113..0x164, 2 on 0x165, 1 on 0x109, 8 on 0x1C2 (no drawing then) and
//! at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash_b`
//! 0x1E003A0, `ribbons` 0x1E00E10, `domes` 0x1E01680, `tubes` 0x1E020C0,
//! `fade` 0x1E02A30, `spikes` 0x1E02F00, `drops` 0x1E039E0, `comets_b`
//! 0x1E04340, `sprays_b` 0x1E05290 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, libm_cosf, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PI: u32 = 0x4049_0FDB;
const SHAKE: u32 = D + 0x70;
const LIGHT: u32 = D + 0xD0;
const FLASH: u32 = D + 0xD4;
const DROPS: u32 = D + 0x594;
const SPIKES: u32 = D + 0x47FC;
const FADE: u32 = D + 0x605C;
const COMETS: u32 = D + 0x6064;
const AIMS: u32 = D + 0x775C;
const RADII: u32 = D + 0x78C4;
const PATH: u32 = D + 0x793C;
const TUBES: u32 = D + 0x7B7C;
const DOMES: u32 = D + 0x7BD4;
const SPRAYS: u32 = D + 0x7CC4;
const RIBBONS: u32 = D + 0x938C;
const HELIX: u32 = D + 0x9914;
const BYTES: u32 = D + 0x9974;

/// The entry 0x1E05C40 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x110);
    fx.mem.write_u32(sp + 0xA8, OT);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x110);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    // 0x199208 memset(D, 0, 0x9A80).
    fx.bzero(D, 0x9A80);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..7 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x13, 1), (0x10, 0x1B, 0), (0x14, 0x61, 0), (0x18, 0x69, 0), (0x1C, 0x42, 0), (0x20, 0x3A, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(LIGHT, 2);
    fx.mem.set16(LIGHT + 2, 0x1BC);
    // The entry's s0 = 7 (the texture loop's), s1 / s2 the runner's 0.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5D90, [Some(7), Some(0), Some(0)]);
    0
}

/// The s registers the calls save.
struct Regs {
    s0: u64,
    s1: u64,
    s2: u64,
    s3: u64,
    s4: u64,
    s5: u64,
}

fn sx(v: i32) -> u64 {
    i64::from(v) as u64
}

fn rand4(fx: &mut Fx, r: &Regs, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, r.s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand() >> 4
}

/// node_position(self, D[at], out) with the entry's s0 / s1.
fn node_at(fx: &mut Fx, at: u32, out: u32, r: &Regs, ra: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(at));
    node_position_saving(fx, h, s, out, r.s0, Some(r.s1), ra);
}

/// Update 0xA0 [F 0x1E05EBC]: the three points, their centroid and a long
/// shake there.
fn points(fx: &mut Fx, sp: u32, r: &mut Regs) {
    node_at(fx, D + 0xC, sp + 0xB0, r, 0x01E0_5ECC);
    r.s0 = u64::from(sp + 0xC0);
    node_at(fx, D + 0x10, sp + 0xC0, r, 0x01E0_5EE0);
    (r.s0, r.s1) = (u64::from(sp + 0xB4), u64::from(sp + 0xB8));
    for c in 0..3u32 {
        let a = fx.mem.f(sp + 0xB0 + 4 * c);
        let v = ee::add(a, ee::div(ee::sub(fx.mem.f(sp + 0xC0 + 4 * c), a), 2.0));
        fx.mem.setf(D + 0x3C + 4 * c, v);
    }
    for (slot, to, ra) in [(0x14u32, 0x48u32, 0x01E0_5F48u32), (0x1C, 0x54, 0x01E0_5F70)] {
        node_at(fx, D + slot, sp + 0xB0, r, ra);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0xB0 + 4 * c);
            fx.mem.write_u32(D + to + 4 * c, w);
        }
    }
    for k in 0..3u32 {
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(D + 0x60 + 4 * c), fx.mem.f(D + 0x3C + 0xC * k + 4 * c));
            fx.mem.setf(D + 0x60 + 4 * c, v);
        }
    }
    for c in 0..3u32 {
        let v = ee::div(fx.mem.f(D + 0x60 + 4 * c), 3.0);
        fx.mem.setf(D + 0x60 + 4 * c, v);
    }
    let s = SHAKE;
    set_counter4(fx, s, 0x88, 4, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    for o in [0x14u32, 0x18, 0x1C] {
        fx.mem.write_u32(s + o, 0x3FC0_0000);
    }
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4270_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    for c in 0..3u32 {
        let w = fx.mem.read_u32(D + 0x60 + 4 * c);
        fx.mem.write_u32(s + 0x20 + 4 * c, w);
    }
    let z = fx.mem.f(s + 0x28);
    let z = if fx.mem.read_u32(D + 0x30) == 0x8000 { ee::sub(z, 3.0) } else { ee::add(z, 3.0) };
    fx.mem.setf(s + 0x28, z);
}

/// The colour bytes (+0xA, +9, +8) of a halo / flash on point `k`.
fn tone(fx: &mut Fx, q: u32, k: u32) {
    let v: [u8; 3] = match k {
        0 => [0xC0, 0x20, 0x40],
        1 => [0, 0x80, 0],
        _ => [0x60, 0, 0x60],
    };
    for (o, b) in [(0xAu32, v[0]), (9, v[1]), (8, v[2])] {
        fx.mem.write_u8(q + o, b);
    }
}

/// Update 0xA2 [F 0x1E060AC]: on each point a pair of flashes (150 / 300
/// wide, delayed 8 k) and a halo.
fn halos(fx: &mut Fx, r: &mut Regs) {
    for k in 0..3u32 {
        let s2 = 8 * k;
        let s7 = 0x82 - s2;
        for s5 in 0..2u32 {
            let q = FLASH + 0x4C * (2 * k + s5);
            set_counter4(fx, q, (s7 & 0xFF) as i32, (s2 & 0xFF) as i32, 0xFF);
            fx.mem.write_u8(q + 5, 0);
            fx.mem.write_u8(q + 4, 0x81);
            fx.mem.write_u8(q + 7, 9);
            let (bit, w) = if s5 != 0 { (4, 0x4396_0000) } else { (2, 0x4316_0000) };
            let b = fx.mem.read_u8(q + 4) | bit;
            fx.mem.write_u8(q + 4, b);
            fx.mem.write_u32(q + 0x10, w);
            for o in [0xFu32, 0xC, 0xD, 0xE] {
                fx.mem.write_u8(q + o, 0xC0);
            }
            tone(fx, q, k);
            fx.mem.write_u32(q + 0x14, 0);
            for c in 0..3u32 {
                let w = fx.mem.read_u32(D + 0x3C + 0xC * k + 4 * c);
                fx.mem.write_u32(q + 0x1C + 4 * c, w);
            }
        }
        let q = FLASH + 0x4C * (k + 6);
        set_counter4(fx, q, 0x18, (s2 & 0xFF) as i32, 0xFF);
        fx.mem.write_u8(q + 5, 0);
        fx.mem.write_u8(q + 4, 0x80);
        fx.mem.write_u8(q + 7, 9);
        fx.mem.write_u32(q + 0x10, 0x4348_0000);
        fx.mem.write_u32(q + 0x14, 0x4248_0000);
        fx.mem.write_u32(q + 0x18, 0x41F0_0000);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(q + o, 0x80);
        }
        tone(fx, q, k);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(D + 0x3C + 0xC * k + 4 * c);
            fx.mem.write_u32(q + 0x1C + 4 * c, w);
        }
    }
    (r.s0, r.s1, r.s2, r.s3, r.s4, r.s5) = (3, u64::from(D + 0x4C * 8), 24, u64::from(D + 0x24), u64::from(D + 0x4C * 5), 2);
}

/// Update 0xCD [F 0x1E062EC]: three ribbons between the points.
fn ribbons(fx: &mut Fx, r: &mut Regs) {
    for k in 0..3u32 {
        let a = RIBBONS + 0x20 * k;
        set_counter4(fx, a, 0x5F, 0, 0xFF);
        fx.mem.write_u8(a + 5, 0);
        fx.mem.write_u8(a + 4, 0);
        fx.mem.write_u8(a + 8, 1);
        let (p, q) = [(0x3Cu32, 0x48u32), (0x48, 0x54), (0x54, 0x3C)][k as usize];
        let path = D + 0x9344 + 0x18 * k;
        for c in 0..3u32 {
            let w = fx.mem.read_u32(D + p + 4 * c);
            fx.mem.write_u32(path + 4 * c, w);
        }
        for c in 0..3u32 {
            let w = fx.mem.read_u32(D + q + 4 * c);
            fx.mem.write_u32(path + 0xC + 4 * c, w);
        }
        fx.mem.write_u8(a + 9, k as u8 + 1);
        fx.mem.write_u8(a + 0xA, 2);
        fx.mem.write_u8(a + 0xB, 0);
        fx.mem.write_u8(a + 0xC, 3);
        fx.mem.write_u32(a + 0x10, path);
        fx.mem.write_u32(a + 0x14, 0);
        fx.mem.write_u32(a + 0x18, 0x3E99_999A);
        for o in [0x1Fu32, 0x1C, 0x1D, 0x1E] {
            fx.mem.write_u8(a + o, 0x80);
        }
    }
    (r.s2, r.s0, r.s1) = (3, u64::from(D + 0x60), u64::from(D + 0x48));
}

/// Update 0xE0 [F 0x1E06508]: twenty comets.
fn comets(fx: &mut Fx, r: &mut Regs) {
    for s1 in 0..0x14u64 {
        r.s1 = s1;
        let i = first_free(fx, COMETS, 0x44, 0x1E);
        r.s0 = sx(i);
        if i < 0 {
            continue;
        }
        let i = i as u32;
        let v = rand4(fx, r, 0x01E0_652C);
        let (_, m) = fx.div(v, 3);
        fx.mem.setf(AIMS + 0xC * i + 4, ee::div(ee::from_i32(200 * m - 0x12C), 100.0));
        let v = rand4(fx, r, 0x01E0_6590);
        let (_, m) = fx.div(v, 0x96);
        fx.mem.setf(RADII + 4 * i, ee::sub(2.0, ee::div(ee::from_i32(m - 0x4B), 100.0)));
        let delay = rand4(fx, r, 0x01E0_65E0) & 7;
        let c = COMETS + 0x44 * i;
        r.s0 = u64::from(D + 0x44 * i);
        set_counter4(fx, c, 0x24, delay, 7);
        fx.mem.write_u8(c + 5, 0);
        fx.mem.write_u8(c + 4, 6);
        fx.mem.write_u32(c + 8, D + 0x685C);
        fx.mem.write_u32(c + 0x28, 0xF0);
        for (o, v) in [(0x1Eu32, 8u8), (0x1F, 0), (0x21, 2), (0x22, 3), (0x23, 6), (0x37, 0x80), (0x34, 0x80), (0x35, 0x80), (0x36, 0x80)] {
            fx.mem.write_u8(c + o, v);
        }
        fx.mem.write_u32(c + 0x2C, 0x437A_0000);
        fx.mem.write_u32(c + 0x30, 0x42C8_0000);
        let n = u32::from(fx.mem.read_u8(c + 0x1E)) + 1;
        for k in 0..n {
            fx.mem.write_u16(c + 0xC + 2 * k, 0xFFFF);
        }
        r.s0 = u64::from(D + 0x44 * i + 2 * n);
    }
    r.s1 = 0x14;
}

/// A flash record's fields from update 0x104 / 0x105.
fn burst_fields(fx: &mut Fx, q: u32, wide: u32, high: u32) {
    fx.mem.write_u8(q + 5, 0);
    fx.mem.write_u8(q + 4, 0x80);
    fx.mem.write_u8(q + 7, 9);
    fx.mem.write_u8(q + 0xA, 0x40);
    fx.mem.write_u8(q + 9, 0x20);
    fx.mem.write_u8(q + 8, 0x80);
    for o in [0xFu32, 0xC, 0xD, 0xE] {
        fx.mem.write_u8(q + o, 0xC0);
    }
    fx.mem.write_u32(q + 0x10, wide);
    fx.mem.write_u32(q + 0x14, high);
}

fn copy3(fx: &mut Fx, from: u32, to: u32) {
    for c in 0..3u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

/// Update 0x104 [F 0x1E066BC]: the fade and two flashes (at the target,
/// then at the centroid).
fn flare(fx: &mut Fx, sp: u32, r: &mut Regs) {
    set_counter4(fx, FADE, 0x10, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 1);
    for s1 in 0..2u32 {
        r.s1 = u64::from(s1);
        let i = first_free(fx, FLASH, 0x4C, 0x10);
        if i < 0 {
            continue;
        }
        let s0 = D + 0x4C * i as u32;
        r.s0 = u64::from(s0);
        let q = s0 + 0xD4;
        set_counter4(fx, q, 0x18, 0, 0xFF);
        burst_fields(fx, q, 0x43FA_0000, 0x4316_0000);
        fx.mem.write_u32(q + 0x18, 0x42A0_0000);
        if s1 != 0 {
            copy3(fx, D + 0x60, q + 0x1C);
        } else {
            copy_v4(fx, sp + 0xD0, q + 0x1C);
        }
    }
    r.s1 = 2;
}

/// Update 0x105 [F 0x1E067D0]: the tubes' path from the centroid to the
/// target, two tubes, two flashes, two domes and three helix ribbons.
fn finale(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let d: [f32; 3] = std::array::from_fn(|c| ee::div(ee::sub(fx.mem.f(sp + 0xD0 + 4 * c as u32), fx.mem.f(D + 0x60 + 4 * c as u32)), 4.0));
    for k in 0..5u32 {
        let f = ee::from_i32(k as i32);
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(D + 0x60 + 4 * c), ee::mul(d[c as usize], f));
            fx.mem.setf(PATH + 0xC * k + 4 * c, v);
        }
    }
    for s1 in 0..2u32 {
        let t = TUBES + 0x2C * s1;
        set_counter4(fx, t, 0x2A, 0, 0xFF);
        for (o, v) in [(7u32, 0xCu8), (8, 4), (0xB, 8), (9, 5), (0xA, 2)] {
            fx.mem.write_u8(t + o, v);
        }
        fx.mem.write_u32(t + 0xC, PATH);
        fx.mem.write_u32(t + 0x14, 0x3F80_0000);
        fx.mem.write_u32(t + 0x18, 0x3CA3_D70A);
        fx.mem.write_u32(t + 0x1C, 0x3E99_999A);
        let (w, len, c) = if s1 != 0 { (0x4000_0000, 0x4100_0000, 0xC0u8) } else { (0x3F80_0000, 0x4180_0000, 0x80) };
        fx.mem.write_u32(t + 0x10, w);
        fx.mem.write_u32(t + 0x20, len);
        for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
            fx.mem.write_u8(t + o, c);
        }
        fx.mem.write_u32(t + 0x24, 0);
    }
    for s1 in 0..2u32 {
        let i = first_free(fx, FLASH, 0x4C, 0x10);
        if i < 0 {
            continue;
        }
        let q = FLASH + 0x4C * i as u32;
        fx.mem.write_u8(q + 5, 0);
        let b = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(q + 6, b);
        burst_fields(fx, q, 0x4496_0000, 0);
        fx.mem.write_u8(q + 4, 0x83);
        if s1 & 1 != 0 {
            set_counter4(fx, q, 0x2A, 0, 0xFF);
            copy3(fx, D + 0x60, q + 0x1C);
        } else {
            set_counter4(fx, q, 0x7C, 0, 0xFF);
            copy_v4(fx, sp + 0xD0, q + 0x1C);
        }
    }
    for s1 in 0..2u32 {
        let d = DOMES + 0x50 * s1;
        set_counter4(fx, d, 0x7C, (s1 & 0xFF) as i32, 0xFF);
        for (o, v) in [(0xAu32, 4u8), (7, 9), (8, 5), (9, 2)] {
            fx.mem.write_u8(d + o, v);
        }
        fx.mem.write_u32(d + 0xC, 0x4040_0000);
        fx.mem.write_u32(d + 0x14, 0x4234_0000);
        fx.mem.write_u32(d + 0x10, if s1 == 1 { 0xBDA3_D70A } else { 0xBCF5_C28F });
        fx.mem.write_u32(d + 0xC, 0x4000_0000);
        if s1 == 1 {
            fx.mem.write_u32(d + 0x38, 0x8000);
        }
        fx.mem.write_u32(d + 0x48, 0x80);
        copy_v4(fx, sp + 0xD0, d + 0x20);
        let (turn, z) = if fx.mem.read_u32(D + 0x30) == 0x8000 {
            (-0x4000, ee::sub(fx.mem.f(d + 0x28), fx.mem.f(d + 0x14)))
        } else {
            (0x4000, ee::add(fx.mem.f(d + 0x28), fx.mem.f(d + 0x14)))
        };
        fx.mem.set32(d + 0x30, turn);
        fx.mem.setf(d + 0x28, z);
        fx.mem.write_u32(d + 0x18, 0);
        for (o, v) in [(0x1Cu32, 0x80u8), (0x1D, 0x80), (0x1E, 0x80), (0x1F, 0x60)] {
            fx.mem.write_u8(d + o, v);
        }
    }
    for s2 in 0..3u32 {
        let b = HELIX + 0x20 * s2;
        set_counter4(fx, b, 0x2A, 0, 0xFF);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 4, 1);
        fx.mem.write_u8(b + 8, 0x23);
        fx.mem.write_u8(b + 9, s2 as u8 + 1);
        fx.mem.write_u8(b + 0xA, 2);
        fx.mem.write_u8(b + 0xB, 0);
        fx.mem.write_u8(b + 0xC, 3);
        fx.mem.write_u32(b + 0x10, D + 0x93EC + 0x1B0 * s2);
        fx.mem.write_u32(b + 0x14, 0x4348_0000);
        fx.mem.write_u32(b + 0x18, 0x3F19_999A);
        for o in [0x1Fu32, 0x1C, 0x1D, 0x1E] {
            fx.mem.write_u8(b + o, 0x80);
        }
    }
    (r.s2, r.s0, r.s1) = (3, u64::from(D + 0x60), u64::from(D + 0x1B0 * 3));
}

/// 0x1E05AA0(D) [F] (a28e05118a9d, this record's own): each counting
/// comet turned about its aim vector (+0x775C + 12 k, 0x101A00) at radius
/// (count - 4) x +0x78C4[k] (not below 0), angle 45 k + 16 elapsed
/// degrees (0x20C540), from the centroid; flag 8 on its last 3 counts.
/// Frame 0xD0: f20 / f21 +0 / +4, s0..s4 +0x10.., ra +0x60.
fn steer(fx: &mut Fx) {
    let sp = fx.enter(0xD0);
    for k in 0..0x1Eu32 {
        let c = COMETS + 0x44 * k;
        let n = fx.mem.u8(c);
        if n == 0 {
            continue;
        }
        if n < 4 {
            let b = fx.mem.read_u8(c + 4) | 8;
            fx.mem.write_u8(c + 4, b);
        }
        blib::spray::aim_rotation(fx, AIMS + 0xC * k, sp + 0x90, [u64::from(k), u64::from(D + 0x44 * k)], 0x01E0_5B0C);
        let (total, n) = (fx.mem.u8(c + 2), fx.mem.u8(c));
        let mut f = ee::mul(ee::from_i32(n - 4), fx.mem.f(RADII + 4 * k));
        if crate::exe::ee_lt(f, 0.0) {
            f = 0.0;
        }
        let a = ee::add(ee::mul(45.0, ee::from_i32(k as i32)), ee::mul(16.0, ee::from_i32(total - n)));
        let ang = ee::div(ee::mul(f32::from_bits(PI), a), 180.0);
        let cs = libm_cosf(fx, ang);
        fx.mem.setf(sp + 0x70, ee::mul(f, cs));
        fx.mem.write_u32(sp + 0x78, 0);
        let sn = fx.libm_sinf(ang);
        fx.mem.setf(sp + 0x74, ee::mul(f, sn));
        let m = fx.read_m4(sp + 0x90);
        let v = fx.read_v4(sp + 0x70);
        let t = fx.transform(Some(&m), v);
        fx.write_v4(sp + 0x80, t);
        for e in 0..3u32 {
            let v = ee::add(fx.mem.f(D + 0x60 + 4 * e), fx.mem.f(sp + 0x80 + 4 * e));
            fx.mem.setf(c + 0x38 + 4 * e, v);
        }
    }
    fx.leave(0xD0);
}

/// A signed remainder by 4 without `div` (LO / HI kept): -((200 r - 400)
/// / 100).
fn quarter(fx: &mut Fx, r: &Regs, ra: u32) -> f32 {
    let v = rand4(fx, r, ra);
    let m = v.wrapping_rem(4);
    ee::neg(ee::div(ee::from_i32(200 * m - 400), 100.0))
}

fn drop_fields(fx: &mut Fx, q: u32) {
    fx.mem.write_u8(q + 4, 0x41);
    fx.mem.write_u8(q + 0xB, 2);
    fx.mem.write_u8(q + 9, 3);
    fx.mem.write_u8(q + 0xA, 0);
    for o in [0xCu32, 0xD, 0xE, 0xF] {
        fx.mem.write_u8(q + o, 0x80);
    }
}

/// Updates 0x5F..0x91 [F 0x1E06EC4]: two drops a update from node pair
/// (0x13 / 0x1B, 0x61 / 0x69, 0x42 / 0x3A by the update).
fn trickle(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let n = fx.mem.u16(D);
    let idx = u32::from(n >= 0x77) + u32::from(n >= 0x85);
    r.s0 = u64::from(D + 8 * idx);
    for s4 in 0..2u32 {
        r.s4 = u64::from(s4);
        let i = first_free(fx, DROPS, 0x44, 0xFA);
        r.s1 = sx(i);
        if i < 0 {
            continue;
        }
        fx.fix_residue(24.0, 0x01E0_6EF4);
        let s1 = D + 0x44 * i as u32;
        r.s1 = u64::from(s1);
        let q = s1 + 0x594;
        set_counter4(fx, q, 24, 0, 0xFF);
        drop_fields(fx, q);
        fx.mem.write_u8(q + 8, 6);
        fx.mem.write_u32(q + 0x10, 0x430C_0000);
        for o in [0x14u32, 0x1C, 0x18] {
            fx.mem.write_u32(q + o, 0);
        }
        let s0 = D + 8 * idx;
        if s4 & 1 != 0 {
            node_at(fx, s0 + 0xC, sp + 0xB0, r, 0x01E0_6F7C);
        } else {
            node_at(fx, s0 + 0x10, sp + 0xB0, r, 0x01E0_6F94);
        }
        copy3(fx, sp + 0xB0, q + 0x20);
        r.s2 = u64::from(sp + 0xC0);
        let f = quarter(fx, r, 0x01E0_6FB4);
        fx.mem.setf(sp + 0xC0, f);
        r.s3 = u64::from(sp + 0xC4);
        let f = quarter(fx, r, 0x01E0_7010);
        fx.mem.setf(sp + 0xC4, f);
        let f = quarter(fx, r, 0x01E0_706C);
        fx.mem.setf(sp + 0xC8, f);
        for c in 0..3u32 {
            let v = ee::div(fx.mem.f(sp + 0xC0 + 4 * c), 24.0);
            fx.mem.setf(q + 0x2C + 4 * c, v);
        }
        for o in [0x40u32, 0x3C, 0x38] {
            fx.mem.write_u32(q + o, 0);
        }
    }
    r.s4 = 2;
}

/// Updates 0xA0..0xC7 [F 0x1E07124]: four drops a update at each point.
fn showers(fx: &mut Fx, sp: u32, r: &mut Regs) {
    for s4 in 0..3u32 {
        r.s4 = u64::from(s4);
        let s0 = D + 0xC * s4;
        r.s0 = u64::from(s0);
        for s3 in 0..4u32 {
            r.s3 = u64::from(s3);
            let i = first_free(fx, DROPS, 0x44, 0xFA);
            r.s1 = sx(i);
            if i < 0 {
                continue;
            }
            fx.fix_residue(12.0, 0x01E0_715C);
            let s1 = D + 0x44 * i as u32;
            r.s1 = u64::from(s1);
            let q = s1 + 0x594;
            set_counter4(fx, q, 12, 0, 0xFF);
            drop_fields(fx, q);
            fx.mem.write_u8(q + 8, s4 as u8 + 1);
            fx.mem.write_u32(q + 0x10, 0x42A0_0000);
            fx.mem.write_u32(q + 0x14, 0xC080_0000);
            fx.mem.write_u32(q + 0x1C, 0);
            fx.mem.write_u32(q + 0x18, 0);
            for (c, ra) in [(0u32, 0x01E0_720Cu32), (1, 0x01E0_7260), (2, 0x01E0_72B4)] {
                let v = rand4(fx, r, ra);
                let (_, m) = fx.div(v, 0x12);
                let f = ee::neg(ee::div(ee::from_i32(200 * m - 0x708), 100.0));
                fx.mem.setf(sp + 0xB0 + 4 * c, f);
                if c == 0 {
                    r.s2 = u64::from(sp + 0xB4);
                }
            }
            for c in 0..3u32 {
                let v = ee::add(fx.mem.f(s0 + 0x3C + 4 * c), fx.mem.f(sp + 0xB0 + 4 * c));
                fx.mem.setf(q + 0x20 + 4 * c, v);
            }
            for c in 0..3u32 {
                let v = ee::neg(ee::div(fx.mem.f(sp + 0xB0 + 4 * c), 12.0));
                fx.mem.setf(q + 0x2C + 4 * c, v);
            }
            for o in [0x40u32, 0x3C, 0x38] {
                fx.mem.write_u32(q + o, 0);
            }
        }
        r.s3 = 4;
    }
    (r.s4, r.s0) = (3, u64::from(D + 0x24));
}

/// A spike's common fields (+0x47FC + 0x68 i).
fn spike_fields(fx: &mut Fx, q: u32, r: &Regs, ra: u32) {
    let v = rand4(fx, r, ra);
    fx.mem.set8(q + 9, v);
    fx.mem.write_u32(q + 0x10, BYTES);
    fx.mem.write_u32(q + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(q + o, 0x80);
    }
    fx.mem.write_u8(q + 0x23, 0x10);
    fx.mem.write_u8(q + 0x27, 0x40);
}

fn spike_spins(fx: &mut Fx, q: u32, r: &Regs, ra: [u32; 3]) {
    for (o, ra) in [(0x58u32, ra[0]), (0x5C, ra[1]), (0x60, ra[2])] {
        let v = rand4(fx, r, ra);
        fx.mem.set32(q + o, v & 0x7F);
    }
}

/// Odd updates 0xA3..0x111 [F 0x1E073C0]: a spike at each point.
fn needles(fx: &mut Fx, r: &mut Regs) {
    for s2 in 0..3u32 {
        r.s2 = u64::from(s2);
        let s0 = D + 0xC * s2;
        r.s0 = u64::from(s0);
        let i = first_free(fx, SPIKES, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let s1 = D + 0x68 * i as u32;
        r.s1 = u64::from(s1);
        let q = s1 + 0x47FC;
        set_counter4(fx, q, 0x1C, 0, 0xFF);
        fx.mem.write_u8(q + 4, 5);
        fx.mem.write_u32(q + 0x14, 0x3F99_999A);
        fx.mem.write_u32(q + 0x1C, 0x41F0_0000);
        fx.mem.write_u8(q + 8, 0x14);
        fx.mem.write_u8(q + 5, 0);
        fx.mem.write_u8(q + 0xA, s2 as u8 + 1);
        fx.mem.write_u8(q + 0xB, 2);
        fx.mem.write_u8(q + 0xC, 3);
        spike_fields(fx, q, r, 0x01E0_7484);
        copy3(fx, s0 + 0x3C, q + 0x28);
        spike_spins(fx, q, r, [0x01E0_74E8, 0x01E0_74F8, 0x01E0_7508]);
    }
    (r.s2, r.s0) = (3, u64::from(D + 0x24));
}

/// Odd updates 0xFD..0x167 [F 0x1E075B4]: a spike at the target (and
/// before 0x11F one at the centroid).
fn stabs(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let n = if fx.mem.u16(D) < 0x11F { 2u32 } else { 1 };
    r.s0 = u64::from(n);
    for s2 in 0..n {
        r.s2 = u64::from(s2);
        let i = first_free(fx, SPIKES, 0x68, 0x3C);
        if i < 0 {
            continue;
        }
        let s1 = D + 0x68 * i as u32;
        r.s1 = u64::from(s1);
        let q = s1 + 0x47FC;
        set_counter4(fx, q, 0x10, 0, 0xFF);
        fx.mem.write_u8(q + 4, 0x85);
        fx.mem.write_u8(q + 8, 0x28);
        fx.mem.write_u8(q + 5, 0);
        fx.mem.write_u8(q + 0xA, 0);
        fx.mem.write_u8(q + 0xB, 2);
        fx.mem.write_u8(q + 0xC, 3);
        spike_fields(fx, q, r, 0x01E0_7640);
        if s2 != 0 {
            fx.mem.write_u32(q + 0x14, 0x4120_0000);
            fx.mem.write_u32(q + 0x1C, 0x42A0_0000);
            copy3(fx, D + 0x60, q + 0x28);
        } else {
            fx.mem.write_u32(q + 0x14, 0x4160_0000);
            fx.mem.write_u32(q + 0x1C, 0x42C8_0000);
            copy_v4(fx, sp + 0xD0, q + 0x28);
        }
        spike_spins(fx, q, r, [0x01E0_76F8, 0x01E0_7708, 0x01E0_7718]);
    }
    r.s2 = u64::from(n);
}

/// Updates 0x113..0x167 [F 0x1E07754]: a spray at the target.
fn spray(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let i = first_free(fx, SPRAYS, 0x60, 0x3C);
    if i < 0 {
        return;
    }
    let s0 = 0x60 * i as u32;
    r.s0 = u64::from(s0);
    let p = SPRAYS + s0;
    set_counter4(fx, p, 0x20, 0, 0xFF);
    fx.mem.write_u8(p + 5, 0);
    fx.mem.write_u8(p + 4, 6);
    let v = rand4(fx, r, 0x01E0_77A4);
    fx.mem.set8(p + 9, v);
    fx.mem.write_u8(p + 0xA, 0);
    let v = rand4(fx, r, 0x01E0_77B8);
    fx.mem.set8(p + 0xB, v & 7);
    fx.mem.write_u8(p + 0xC, 0);
    fx.mem.write_u8(p + 0xD, 2);
    fx.mem.write_u32(p + 0x10, BYTES);
    fx.mem.write_u32(p + 0x24, 0);
    for o in [0x28u32, 0x29, 0x2A] {
        fx.mem.write_u8(p + o, 0x80);
    }
    fx.mem.write_u8(p + 0x2B, 0x60);
    copy3(fx, sp + 0xD0, p + 0x3C);
    fx.mem.write_u8(p + 8, 0x1E);
    fx.mem.write_u32(p + 0x1C, 0x41A0_0000);
    let v = rand4(fx, r, 0x01E0_7828);
    let (_, m) = fx.div(v, 0xA);
    let f = ee::add(40.0, ee::div(ee::from_i32(200 * m - 0x3E8), 100.0));
    fx.mem.setf(p + 0x14, f);
    fx.mem.write_u32(p + 0x20, 0x42F0_0000);
    let left = fx.mem.read_u32(D + 0x30) == 0x8000;
    fx.mem.set32(p + 0x2C, if left { -0x4000 } else { 0x4000 });
    let v = rand4(fx, r, if left { 0x01E0_78A4 } else { 0x01E0_7914 });
    let m = v.wrapping_rem(2);
    let x = ee::div(ee::from_i32(200 * m - 0xC8), 100.0);
    let f = if left { ee::add(-3.0, x) } else { ee::sub(3.0, x) };
    fx.mem.setf(p + 0x50, f);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut r = Regs { s0: sx(param), s1: 0, s2: 0, s3: 0, s4: 0, s5: 0 };
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0xD0, r.s0, 0x01E0_5DB0);
    let n = fx.mem.u16(D);
    match n {
        0x1C2 => return 8,
        0x15C => {
            set_counter4(fx, FADE, 0x20, 0, 0xFF);
            fx.mem.write_u8(FADE + 4, 0x82);
        }
        0x105 => finale(fx, sp, &mut r),
        0x104 => flare(fx, sp, &mut r),
        _ => {
            // The delay slots of the 0xE0 / 0xCD / 0xA2 tests clear s1 /
            // s2 / s0.
            r.s1 = 0;
            match n {
                0xE0 => comets(fx, &mut r),
                0xCD => {
                    r.s2 = 0;
                    ribbons(fx, &mut r);
                }
                _ => {
                    (r.s2, r.s0) = (0, 0);
                    match n {
                        0xA2 => halos(fx, &mut r),
                        0xA0 => points(fx, sp, &mut r),
                        0x3A..=0x3D | 0x4B..=0x4E => {
                            for o in [0x10Cu32, 0x10D, 0x10E] {
                                fx.mem.write_u8(sp + o, 0x80);
                            }
                            fx.mem.write_u8(sp + 0x10F, 0x50);
                            let a = fx.alpha_preset(0);
                            fx.frame_pass(OT, 0, 1, a, sp + 0x10C, 0x01E0_5EB4);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    if fx.mem.read_u8(TUBES) == 0xF {
        for k in 0..2u32 {
            let t = TUBES + 0x2C * k;
            let v = ee::neg(ee::div(fx.mem.f(t + 0x20), 16.0));
            fx.mem.setf(t + 0x24, v);
        }
    }
    if fx.mem.read_u8(HELIX) != 0 {
        let p = sp + 0xE0;
        for s2 in 0..3u32 {
            let b = HELIX + 0x20 * s2;
            fx.mem.set32(p, 0x24);
            let radius = if fx.mem.u8(HELIX) < 0x10 { ee::from_i32(fx.mem.u8(b)) } else { 16.0 };
            fx.mem.setf(p + 4, radius);
            let e = fx.mem.u8(b + 2) - fx.mem.u8(b);
            let phase = ee::sub(ee::add(90.0, ee::mul(120.0, ee::from_i32(s2 as i32))), ee::mul(20.0, ee::from_i32(e)));
            fx.mem.setf(p + 8, phase);
            fx.mem.write_u32(p + 0xC, 0x41A0_0000);
            fx.mem.write_u32(p + 0x28, D + 0x93EC + 0x1B0 * s2);
            copy3(fx, D + 0x60, p + 0x10);
            copy3(fx, sp + 0xD0, p + 0x1C);
            blib::bolt::helix(fx, p);
        }
        (r.s2, r.s0, r.s1) = (3, u64::from(D + 0x60), u64::from(D + 0x1B0 * 3));
    }
    let n = fx.mem.u16(D);
    if (0x5F..0x92).contains(&n) {
        trickle(fx, sp, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0xA0..0xC8).contains(&n) {
        showers(fx, sp, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0xA2..0x113).contains(&n) && n & 1 != 0 {
        needles(fx, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0xCD..0xF5).contains(&n) {
        for k in 0..3u32 {
            let a = RIBBONS + 0x20 * k + 0x14;
            let v = ee::add(fx.mem.f(a), 6.0);
            fx.mem.setf(a, v);
        }
    }
    steer(fx);
    let n = fx.mem.u16(D);
    if (0xFC..0x168).contains(&n) && n & 1 != 0 {
        stabs(fx, sp, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0x113..0x168).contains(&n) {
        spray(fx, sp, &mut r);
    }
    shake_saving(fx, SHAKE, r.s0, 0x01E0_797C, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    light_ramp(fx, LIGHT, n, 0x01E0_7988, [Some(r.s0), Some(r.s1)]);
    blib::fade::fade(fx, FADE, OT);
    // fade's 0xF0 frame saves the entry's s0..s6 (+0x10..) and ra (+0x80).
    fx.saved(sp - 0xF0, &[(0x10, r.s0), (0x20, r.s1), (0x30, r.s2), (0x40, r.s3), (0x50, r.s4), (0x60, r.s5), (0x70, u64::from(D)), (0x80, 0x01E0_7994)]);
    blib::star::flash_b(fx, 0x01E0_03A0, FLASH, OT, 0x10);
    blib::drop::drops(fx, DROPS, OT, 0xFA);
    blib::spike::spikes(fx, 0x01E0_2F00, SPIKES, OT, 0x3C);
    blib::comet::comets_b(fx, 0x01E0_4340, COMETS, OT, 0x1E);
    blib::segtube::tubes(fx, 0x01E0_20C0, TUBES, OT, 2);
    blib::dome::domes(fx, 0x01E0_1680, DOMES, OT, 3);
    blib::spray::sprays_b(fx, 0x01E0_5290, SPRAYS, OT, 0x3C);
    blib::dash::ribbons(fx, 0x01E0_0E10, RIBBONS, OT, 3);
    blib::dash::ribbons(fx, 0x01E0_0E10, HELIX, OT, 3);
    let n = fx.mem.u16(D);
    if (0x113..0x165).contains(&n) {
        6
    } else if n == 0x165 {
        2
    } else if n == 0x109 {
        1
    } else {
        0
    }
}
