//! BMODULE b023 (card 102), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E009E0, data area a0 = 0x1ED0000, 0x2914 bytes)
//! [F]: an 'a0 = D' record in a 0x70 frame (s3 =
//! D, s0 = the OT). The count is the u16 D+0; on 0xB6..0x133 each update a
//! sprite (one of 150 at +0x3C, 0x44; count 0x8C, size 100 + 4, colour
//! (0x60, rand, rand) alpha 0xF0, a random orientation) at the attacker's
//! node 0x1C drifting (rand % 20 - 10) / 100 and 1.4 sideways (by the
//! attacker's turn D+0x30); on odd updates 0x12D..0x1A3 one at a random
//! point +-13 round the target (count 0x30, size 240 + 25, rising 0.25);
//! all 160 drawn by the record-local `sprites` (an older compile of
//! sprite::bounce: 0x1E00080). Object ops 0x1F83B0 from the table
//! 0x1E01180; the attacker tinted back on 0x1D5. Returns 1 on 0x122, 6 on
//! 0x12C..0x1C8, 2 on 0x1C9, 8 on 0x1D6 (and at once for -2 / -3).
use crate::blib::exe::{atlas_cell, camera_vp_into, countdown, first_free, set_counter4, tex_rect};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const FRAME: u32 = 0x70;
const SPRITES: u32 = D + 0x3C;
/// The object-op table in the record.
const OPS: u32 = 0x01E0_1180;

/// The entry 0x1E009E0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    if (param.wrapping_add(3) as u32) < 2 {
        return 8;
    }
    let sp = fx.enter(FRAME);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(FRAME);
    ret
}

fn init(fx: &mut Fx, _sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x2914);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    // 0x1F7370's frame (0x20: s0 = the OT, ra).
    let f = fx.sp - 0x20;
    fx.saved(f, &[(0, u64::from(OT)), (0x10, 0x01E0_0A6C)]);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    let h = fx.mem.i32(D + 4);
    let s = [Some(1), Some(param as i64 as u64), None];
    let r = crate::b076::request_node_saving(fx, h, 0x1C, 1, s, 0x01E0_0ABC);
    fx.mem.set32(D + 0xC, r);
    let h = fx.mem.i32(D + 4);
    let r = crate::b076::request_node_saving(fx, h, 0x2E, 0, s, 0x01E0_0AD4);
    fx.mem.set32(D + 0x10, r);
    crate::b076::rand_bytes(fx, D + 0x2814, 0x100, 0x01E0_0AE8, [Some(1), Some(param as u32), None]);
    0
}

/// rand >> 4 (rand's saved s0 the OT).
fn r4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 4
}

/// A new sprite's common fields [F 0x1E00BB4 / 0x1E00DCC].
fn sprite_fields(fx: &mut Fx, s: u32, count: i32, size: u32, grow: u32, ras: [u32; 4]) {
    set_counter4(fx, s, count, 0, 0xFF);
    let r = r4(fx, ras[0]);
    fx.mem.set8(s + 4, ((r & 3) << 4) | 4);
    fx.mem.write_u8(s + 9, 0);
    fx.mem.write_u8(s + 8, 1);
    let r = r4(fx, ras[1]);
    fx.mem.set8(s + 0xA, (r + 2) & 3);
    fx.mem.write_u8(s + 0xB, 0);
    let r = r4(fx, ras[2]);
    fx.mem.set8(s + 0xE, (r & 0x3F) + 0x18);
    let r = r4(fx, ras[3]);
    fx.mem.set8(s + 0xD, (r & 0x3F) + 0x18);
    fx.mem.write_u8(s + 0xC, 0x60);
    fx.mem.write_u8(s + 0xF, 0xF0);
    fx.mem.write_u32(s + 0x10, size);
    fx.mem.write_u32(s + 0x14, grow);
    fx.mem.write_u32(s + 0x1C, 0);
    fx.mem.write_u32(s + 0x18, 0);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let n = fx.mem.u16(D);
    if n == 0x1D6 {
        return 8;
    }
    if n == 0x1D5 {
        for o in [0x6F, 0x6C, 0x6D, 0x6E] {
            fx.mem.write_u8(sp + o, 0x80);
        }
        let h = fx.mem.i32(D + 4);
        fx.unit_tint_mode(h, -1, sp + 0x6C, [u64::from(OT), param as i64 as u64], 0x01E0_0B48);
    }
    let n = fx.mem.u16(D);
    if (0xB6..0x134).contains(&n) {
        let i = first_free(fx, SPRITES, 0x44, 0x96);
        if i >= 0 {
            let s = SPRITES + 0x44 * i as u32;
            sprite_fields(fx, s, 0x8C, 0x42C8_0000, 0x4080_0000, [0x01E0_0BC4, 0x01E0_0BF0, 0x01E0_0C10, 0x01E0_0C2C]);
            let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
            crate::b076::node_position_saving(fx, h, slot, sp + 0x50, u64::from(OT), Some(u64::from(0x44 * i as u32)), 0x01E0_0C7C);
            for k in 0..3u32 {
                let v = fx.mem.read_u32(sp + 0x50 + 4 * k);
                fx.mem.write_u32(s + 0x20 + 4 * k, v);
            }
            for (o, ra) in [(0x2Cu32, 0x01E0_0CA0), (0x30, 0x01E0_0CEC)] {
                let r = r4(fx, ra);
                let (_, m) = fx.div(r, 0x14);
                fx.mem.setf(s + o, ee::div(ee::from_i32(m - 10), 100.0));
            }
            let v = if fx.mem.read_u32(D + 0x30) == 0x8000 { 0xBFB3_3333 } else { 0x3FB3_3333 };
            fx.mem.write_u32(s + 0x34, v);
        }
    }
    let n = fx.mem.u16(D);
    if (0x12D..0x1A4).contains(&n) && n & 1 != 0 {
        let t = fx.mem.i32(D + 8);
        crate::b076::unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_0D90);
        let i = first_free(fx, SPRITES, 0x44, 0x96);
        if i >= 0 {
            let s = SPRITES + 0x44 * i as u32;
            sprite_fields(fx, s, 0x30, 0x4370_0000, 0x41C8_0000, [0x01E0_0DDC, 0x01E0_0E08, 0x01E0_0E28, 0x01E0_0E44]);
            let t = fx.mem.i32(D + 8);
            crate::b076::unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_0E90);
            for (k, ra) in [0x01E0_0E98u32, 0x01E0_0EE8, 0x01E0_0F38].into_iter().enumerate() {
                let r = r4(fx, ra);
                let (_, m) = fx.div(r, 0xA28);
                let d = ee::div(ee::from_i32(m - 0x514), 100.0);
                let p = fx.mem.f(sp + 0x50 + 4 * k as u32);
                fx.mem.setf(s + 0x20 + 4 * k as u32, ee::add(p, d));
            }
            fx.mem.write_u32(s + 0x34, 0);
            fx.mem.write_u32(s + 0x2C, 0);
            fx.mem.write_u32(s + 0x30, 0x3E80_0000);
        }
    }
    let n = fx.mem.u16(D);
    crate::b035::object_ops(fx, OPS, n);
    sprites(fx, SPRITES, OT, 0xA0);
    let n = fx.mem.u16(D);
    if (0x12C..0x1C9).contains(&n) {
        6
    } else if n == 0x1C9 {
        2
    } else if n == 0x122 {
        1
    } else {
        0
    }
}

/// 0x1A2C70 with its residue frame.
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// The local 0x1E00080 (71014611a1ab, 600 words) [F]: `sprites(g, ot, n)
/// -> 1` steps and draws `n` sprite records (0x44 bytes; a 0x1A0 frame,
/// the descriptor at sp + 0x118) as sprite::bounce compiled earlier: the
/// texture (5, +9) with flag 4, else (4, 2); velocity / position / size
/// as bounce, the y held at the floor (no rebound, no node source); the
/// cell +0xA, or with flag 8 the animation (((elapsed >> +0xB) & 3) + 2,
/// elapsed under 3 as it is, the last 7 counts 7 - (count >> +0xB)) of
/// layout 0, blend +8; alpha +0xF faded over the first 16 counts; the
/// projection's z on all four corners, corners through 0x1A2C70, key the
/// depth.
fn sprites(fx: &mut Fx, g: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    let mut g = g;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x44;
        }
        one(fx, sp, g, ot);
        i += 1;
    }
    fx.leave(0x1A0);
    1
}

fn one(fx: &mut Fx, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let tex = if fx.mem.u8(g + 4) & 4 != 0 {
        let id = fx.mem.u8(g + 9);
        fx.texture(5, id)
    } else {
        fx.texture(4, 2)
    };
    let add = |fx: &mut Fx, a: u32, b: f32| {
        let v = ee::add(fx.mem.f(a), b);
        fx.mem.setf(a, v);
    };
    let v = fx.mem.f(g + 0x38);
    add(fx, g + 0x2C, v);
    let v = ee::add(fx.mem.f(g + 0x3C), fx.mem.f(g + 0x18));
    add(fx, g + 0x30, v);
    let v = fx.mem.f(g + 0x40);
    add(fx, g + 0x34, v);
    for k in 0..3u32 {
        let v = fx.mem.f(g + 0x2C + 4 * k);
        add(fx, g + 0x20 + 4 * k, v);
    }
    let v = fx.mem.f(g + 0x14);
    add(fx, g + 0x10, v);
    let (y, floor) = (fx.mem.f(g + 0x24), fx.mem.f(g + 0x1C));
    if ee_lt(y, floor) {
        fx.mem.setf(g + 0x24, floor);
    }
    for k in 0..3u32 {
        let v = fx.mem.read_u32(g + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, v);
    }
    camera_vp_into(fx, sp + 0xB0);
    let d = sp + 0x118;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (src, dst) in [(0xEu32, 0x128u32), (0xD, 0x129), (0xC, 0x12A)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + dst, v);
    }
    fx.mem.write_u32(sp + 0x12C, 0x3F80_0000);
    let count = fx.mem.u8(g);
    let elapsed = fx.mem.u8(g + 2) - count;
    let mut cell = fx.mem.u8(g + 0xA);
    if fx.mem.u8(g + 4) & 8 != 0 {
        let sh = fx.mem.u8(g + 0xB) as u32;
        cell = ((elapsed >> (sh & 31)) & 3) + 2;
        if elapsed < 3 {
            cell = elapsed;
        }
        if count < 7 {
            cell = 7 - (count >> (sh & 31));
        }
    }
    let b = fx.mem.read_u8(g + 8);
    fx.mem.write_u8(sp + 0x105, b);
    atlas_cell(fx, sp + 0x100, cell, 0);
    tex_rect(fx, d, sp + 0x100, tex);
    let count = fx.mem.u8(g);
    if count < 0x10 {
        let k = fx.mem.u8(g + 0xF) >> 4;
        let v = fx.mult(k, count);
        fx.mem.set8(sp + 0x12B, v);
    } else {
        let v = fx.mem.read_u8(g + 0xF);
        fx.mem.write_u8(sp + 0x12B, v);
    }
    let m = fx.read_m4(sp + 0xB0);
    if !fx.project_at(sp + 0xF0, &m, sp + 0x190, sp + 0x19C) {
        return;
    }
    if !Fx::on_screen(fx.mem.read_u64(sp + 0x190)) {
        return;
    }
    let (size, w) = (fx.mem.f(g + 0x10), fx.mem.f(sp + 0x19C));
    let half = fx.view_scale(size, w);
    let mut z = fx.xyz_z(sp + 0x190);
    for o in [0x168u32, 0x158, 0x148, 0x138] {
        fx.set_xyz_z(sp + o, z);
        z = fx.xyz_z(sp + o);
    }
    // (x minus, x plus, y minus, y plus) corner pairs by orientation and
    // the conversions' return addresses.
    let (xm, xp, ym, yp, ras): ([u32; 2], [u32; 2], [u32; 2], [u32; 2], [u32; 4]) = match fx.mem.u8(g + 4) & 0x30 {
        0x10 => ([0x148, 0x138], [0x168, 0x158], [0x16A, 0x14A], [0x15A, 0x13A], [0x01E0_0470, 0x01E0_04BC, 0x01E0_0520, 0x01E0_056C]),
        0x20 => ([0x168, 0x148], [0x158, 0x138], [0x16A, 0x15A], [0x14A, 0x13A], [0x01E0_05C4, 0x01E0_0610, 0x01E0_0674, 0x01E0_06C0]),
        0x30 => ([0x168, 0x158], [0x148, 0x138], [0x15A, 0x13A], [0x16A, 0x14A], [0x01E0_0718, 0x01E0_0764, 0x01E0_07C8, 0x01E0_0814]),
        _ => ([0x158, 0x138], [0x168, 0x148], [0x14A, 0x13A], [0x16A, 0x15A], [0x01E0_086C, 0x01E0_08B8, 0x01E0_091C, 0x01E0_0968]),
    };
    let x = ee::from_i32(fx.mem.u16(sp + 0x190));
    let v = fix(fx, ee::sub(x, half), ras[0]);
    for o in xm {
        fx.mem.write_u16(sp + o, v as u16);
    }
    let v = fix(fx, ee::add(x, half), ras[1]);
    for o in xp {
        fx.mem.write_u16(sp + o, v as u16);
    }
    let y = ee::from_i32(fx.mem.u16(sp + 0x192));
    let hy = ee::mul(f32::from_bits(0x3FAA_3D71), half);
    let v = fix(fx, ee::sub(y, hy), ras[2]);
    for o in ym {
        fx.mem.write_u16(sp + o, v as u16);
    }
    let v = fix(fx, ee::add(y, hy), ras[3]);
    for o in yp {
        fx.mem.write_u16(sp + o, v as u16);
    }
    let w = fx.mem.f(sp + 0x19C);
    let key = fix(fx, w, 0x01E0_097C);
    fx.submit(d, ot, key as i32);
}
