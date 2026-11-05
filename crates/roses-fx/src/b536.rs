//! BMODULE b536 (card 545), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E039B0, data area a0 = 0x1ED0000) [F] (helper B3, 30
//! Oct 2026): b076's family, a lightning storm. On update 60 a fade
//! (+0x3BE4), a flash at the attacker's node, eight bead beams (+0x3924 +
//! 0x58 k) from the attacker's node 1, a long camera shake (+0x40) and four
//! jagged bolts of six segments (0x102A50; `bolt::bolts`, pool of 100 at
//! +0x2024) from the node to the target; on updates 10..139 three short
//! bolts and a flash a update round a random point near the node; on even
//! updates 36..199 a ribbon (`streak::ribbon`, pool of 60 at +0xA4) from
//! the node; on 60..196 four flashes and ribbons a update round the target
//! (+-14); the stage light pulsed by 0x1031C0 (+0xA0: down on update 2, up
//! on 218). Returns 1 on update 50, 6 on 60..196, 2 on 197, 8 on 224 (and
//! at once for a parameter of -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `ribbon` 0x1E00FC0, `bolts` 0x1E01B50, `fade` 0x1E026A0,
//! `beads` 0x1E02BE0 [`crate::blib`]; 0x102A50 `bolt::jag`.
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;

/// The entry's s0 / s1 as the callees' prologues save them (64 bits; the
/// caller's s1 is 0 [run: 0x1031C0's save of it in the interpreter]).
struct Regs {
    s0: u64,
    s1: Option<u64>,
}

fn sx(v: i32) -> u64 {
    i64::from(v) as u64
}

/// rand() from return address `ra` with the entry's s0 [F 0x19AE88: `sd
/// s0` / `sd ra` 0x10 below the caller].
fn rand_at(fx: &mut Fx, r: &Regs, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, r.s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

/// (rand >> 4) % m - off, over 100 [F].
fn offset(fx: &mut Fx, r: &Regs, ra: u32, m: i32, off: i32) -> f32 {
    let x = rand_at(fx, r, ra) >> 4;
    let (_, h) = fx.div(x, m);
    ee::div(ee::from_i32(h - off), 100.0)
}

/// The entry 0x1E039B0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1E0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x1E0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x3CF0);
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
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 1, 1);
    fx.mem.set32(D + 0xC, r);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xDA);
    // The entry's s0 = 2 (the texture loop's); s1 / s2: the caller's.
    rand_bytes(fx, D + 0x3BEC, 0x100, 0x01E0_3AC4, [Some(2), None, None]);
    0
}

fn node(fx: &mut Fx, out: u32) {
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0xC);
    fx.node_position(h, s, out);
}

/// A flash record (`star::flash`, pool of 24 at +0x1904): `count`, flags
/// +4, colour `c` (+0xC..+0xF), +8..+0xA, size +0x10, at `at` (vec4).
fn flash(fx: &mut Fx, r: &mut Regs, count: i32, flags: u8, c: [u8; 4], b8: [u8; 3], size: u32, at: u32) {
    let i = first_free(fx, D + 0x1904, 0x4C, 0x18);
    r.s1 = Some(sx(i));
    if i < 0 {
        return;
    }
    r.s0 = sx(0x4C * i);
    let f = D + 0x4C * i as u32 + 0x1904;
    set_counter4(fx, f, count, 0, 0xFF);
    fx.mem.write_u8(f + 5, 0);
    fx.mem.write_u8(f + 4, flags);
    fx.mem.write_u8(f + 7, 9);
    for (k, v) in c.iter().enumerate() {
        fx.mem.write_u8(f + 0xC + k as u32, *v);
    }
    for (k, v) in b8.iter().enumerate() {
        fx.mem.write_u8(f + 8 + k as u32, *v);
    }
    fx.mem.write_u32(f + 0x14, 0);
    fx.mem.write_u32(f + 0x10, size);
    fx.mem.write_u32(f + 0x14, 0);
    copy_v4(fx, at, f + 0x1C);
}

/// Update 60 [F 0x1E03B10].
fn storm(fx: &mut Fx, sp: u32, r: &mut Regs) {
    set_counter4(fx, D + 0x3BE4, 0x18, 0, 0xFF);
    fx.mem.write_u8(D + 0x3BE8, 1);
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0x14);
    fx.node_position(h, s, sp + 0xC0);
    let i = first_free(fx, D + 0x1904, 0x4C, 0x18);
    r.s1 = Some(sx(i));
    if i >= 0 {
        r.s0 = sx(0x4C * i);
        let f = D + 0x4C * i as u32 + 0x1904;
        set_counter4(fx, f, 0x14, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0x80), (7, 9), (0xA, 0x40), (9, 0x20), (8, 0x80), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0)] {
            fx.mem.write_u8(f + o, v);
        }
        fx.mem.write_u32(f + 0x10, 0x42C8_0000);
        fx.mem.write_u32(f + 0x14, 0x42C8_0000);
        fx.mem.write_u32(f + 0x18, 0x4270_0000);
        copy_v4(fx, sp + 0xC0, f + 0x1C);
    }
    for k in 0..8u32 {
        r.s1 = Some(u64::from(k));
        r.s0 = u64::from(0x58 * k);
        let b = D + 0x58 * k + 0x3924;
        set_counter4(fx, b, 0x20, k as i32, 0xFF);
        let c = fx.mem.read_u8(b).wrapping_sub(4);
        fx.mem.write_u8(b, c);
        fx.mem.write_u8(b + 4, 5);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 8, 0x1E);
        fx.mem.write_u8(b + 9, k as u8);
        fx.mem.write_u8(b + 0xA, 0);
        fx.mem.write_u8(b + 0xC, 2);
        let x = rand_at(fx, r, 0x01E0_3C84);
        fx.mem.set8(b + 0xB, ((x >> 4) & 3) + 4);
        fx.mem.write_u32(b + 0x1C, 0x4270_0000);
        fx.mem.write_u32(b + 0x20, 0x3F80_0000);
        fx.mem.write_u32(b + 0x10, D + 0x3BEC);
        fx.mem.write_u32(b + 0x14, 0x4000_0000);
        for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x27, 0x60)] {
            fx.mem.write_u8(b + o, v);
        }
        node(fx, b + 0x28);
    }
    r.s1 = Some(8);
    let s = D + 0x40;
    set_counter4(fx, s, 0x8C, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0xA0, D + 0x60);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
    fx.mem.write_u32(D + 0x68, 0);
    node(fx, sp + 0xC0);
    for _ in 0..4 {
        let j = sp + 0xE0;
        fx.mem.set32(j, 6);
        fx.mem.write_u32(j + 4, 0x3F00_0000);
        fx.mem.write_u32(j + 0x20, sp + 0x110);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0xC0 + 4 * c);
            fx.mem.write_u32(j + 8 + 4 * c, w);
            let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
            fx.mem.write_u32(j + 0x14 + 4 * c, w);
        }
        blib::bolt::jag(fx, j);
        for k in 0..6u32 {
            r.s0 = u64::from(k);
            let i = first_free(fx, D + 0x2024, 0x40, 0x64);
            r.s1 = Some(sx(i));
            if i < 0 {
                break;
            }
            r.s1 = Some(sx(i << 6));
            let b = D + 0x40 * i as u32 + 0x2024;
            set_counter4(fx, b, 0x8C, (k & 1) as i32, 0xFF);
            fx.mem.write_u8(b + 5, 0);
            fx.mem.write_u8(b + 4, 1);
            for c in 0..6 {
                let w = fx.mem.read_u32(sp + 0x110 + 12 * k + 4 * c);
                fx.mem.write_u32(b + 0x1C + 4 * c, w);
            }
            fx.mem.write_u8(b + 8, 9);
            fx.mem.write_u32(b + 0x10, 0x3F33_3333);
            fx.mem.write_u32(b + 0x14, 0x3E4C_CCCD);
            fx.mem.write_u8(b + 0xB, 1);
            fx.mem.write_u8(b + 0xD, 3);
            fx.mem.write_u8(b + 0xC, 0);
            for (o, v) in [(0x18u32, 0x80u8), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
                fx.mem.write_u8(b + o, v);
            }
            r.s0 = u64::from(k + 1);
        }
    }
}

/// Updates 10..139 [F 0x1E03F24]: three short bolts and a flash round a
/// random point near the node.
fn sparks(fx: &mut Fx, sp: u32, r: &mut Regs) {
    node(fx, sp + 0xC0);
    for (c, ra) in [(0u32, 0x01E0_3F40u32), (4, 0x01E0_3F90), (8, 0x01E0_3FE0)] {
        let f = offset(fx, r, ra, 0x7D0, 0x3E8);
        let v = ee::add(fx.mem.f(sp + 0xC0 + c), f);
        fx.mem.setf(sp + 0xD0 + c, v);
    }
    for k in 0..3u32 {
        r.s0 = u64::from(k);
        let i = first_free(fx, D + 0x2024, 0x40, 0x64);
        r.s1 = Some(sx(i));
        if i < 0 {
            break;
        }
        r.s1 = Some(sx(i << 6));
        let b = D + 0x40 * i as u32 + 0x2024;
        set_counter4(fx, b, 0xC, k as i32, 0xFF);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 4, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0xD0 + 4 * c);
            fx.mem.write_u32(b + 0x28 + 4 * c, w);
            fx.mem.write_u32(b + 0x1C + 4 * c, w);
        }
        for (c, ra) in [(0u32, 0x01E0_40BCu32), (4, 0x01E0_411C), (8, 0x01E0_417C)] {
            let x = rand_at(fx, r, ra) >> 4;
            let (_, h) = fx.div(x, 0xB);
            let f = ee::div(ee::from_i32(h * 200 - 0x44C), 100.0);
            let v = ee::add(fx.mem.f(b + 0x28 + c), f);
            fx.mem.setf(b + 0x28 + c, v);
        }
        fx.mem.write_u8(b + 8, 6);
        fx.mem.write_u32(b + 0x10, 0x3F80_0000);
        fx.mem.write_u32(b + 0x14, 0x3E99_999A);
        fx.mem.write_u8(b + 0xB, 1);
        fx.mem.write_u8(b + 0xD, 4);
        fx.mem.write_u8(b + 0xC, 0);
        for (o, v) in [(0x18u32, 0x80u8), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x20)] {
            fx.mem.write_u8(b + o, v);
        }
        r.s0 = u64::from(k + 1);
    }
    flash(fx, r, 0xC, 5, [0xD0; 4], [0x80, 0, 0], 0x43AF_0000, sp + 0xD0);
}

/// A ribbon record (`streak::ribbon`, pool of 60 at +0xA4).
fn ribbon_at(fx: &mut Fx, r: &mut Regs) -> Option<u32> {
    let i = first_free(fx, D + 0xA4, 0x68, 0x3C);
    r.s1 = Some(sx(i));
    (i >= 0).then(|| D + 0x68 * i as u32 + 0xA4)
}

/// Even updates 36..199 [F 0x1E042F4]: a ribbon from the node.
fn node_ribbon(fx: &mut Fx, r: &mut Regs) {
    let Some(b) = ribbon_at(fx, r) else { return };
    r.s0 = u64::from(b - D - 0xA4);
    set_counter4(fx, b, 0x10, 0, 0xFF);
    fx.mem.write_u8(b + 4, 5);
    fx.mem.write_u32(b + 0x14, 0x4080_0000);
    fx.mem.write_u32(b + 0x1C, 0x42A0_0000);
    fx.mem.write_u8(b + 8, 0x18);
    fx.mem.write_u8(b + 5, 1);
    let n = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(b + 6, n);
    fx.mem.write_u8(b + 0xA, 0);
    fx.mem.write_u8(b + 0xB, 2);
    fx.mem.write_u8(b + 0xC, 3);
    let x = rand_at(fx, r, 0x01E0_43C8);
    fx.mem.set8(b + 9, x >> 4);
    fx.mem.write_u32(b + 0x10, D + 0x3BEC);
    fx.mem.write_u32(b + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(b + o, 0x80);
    }
    fx.mem.write_u8(b + 0x23, 0x10);
    fx.mem.write_u8(b + 0x27, 0x40);
    for (o, ra) in [(0x58u32, 0x01E0_4418u32), (0x5C, 0x01E0_4430), (0x60, 0x01E0_4448)] {
        let x = rand_at(fx, r, ra);
        fx.mem.set32(b + o, (x >> 4) & 0x7F);
    }
}

/// Updates 60..196 [F 0x1E04484]: four flashes and ribbons round the
/// target.
fn hits(fx: &mut Fx, sp: u32, r: &mut Regs) {
    for k in 0..4u32 {
        r.s0 = u64::from(k);
        for (c, ra) in [(0u32, 0x01E0_4498u32), (4, 0x01E0_44E4), (8, 0x01E0_4530)] {
            let f = offset(fx, r, ra, 0xAF0, 0x578);
            let v = ee::add(fx.mem.f(sp + 0xA0 + c), f);
            fx.mem.setf(sp + 0xB0 + c, v);
        }
        let i = first_free(fx, D + 0x1904, 0x4C, 0x18);
        r.s1 = Some(sx(i));
        if i >= 0 {
            r.s1 = Some(sx(0x4C * i));
            let f = D + 0x4C * i as u32 + 0x1904;
            set_counter4(fx, f, 0xC, 0, 0xFF);
            for (o, v) in [(5u32, 0u8), (4, 1), (7, 9), (0xF, 0xD0), (0xC, 0xD0), (0xD, 0xD0), (0xE, 0xD0), (0xA, 0), (9, 0), (8, 0x80)] {
                fx.mem.write_u8(f + o, v);
            }
            fx.mem.write_u32(f + 0x14, 0);
            fx.mem.write_u32(f + 0x10, 0x4396_0000);
            fx.mem.write_u32(f + 0x14, 0);
            copy_v4(fx, sp + 0xB0, f + 0x1C);
        }
        let Some(b) = ribbon_at(fx, r) else { continue };
        r.s1 = Some(u64::from(b - D - 0xA4));
        fx.mem.write_u8(b + 4, 5);
        set_counter4(fx, b, 8, 0, 0xFF);
        fx.mem.write_u32(b + 0x14, 0x40A0_0000);
        fx.mem.write_u32(b + 0x1C, 0x4220_0000);
        fx.mem.write_u8(b + 0x27, 0x80);
        fx.mem.write_u8(b + 0x23, 0);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 0xA, 0);
        fx.mem.write_u8(b + 0xB, 2);
        fx.mem.write_u8(b + 0xC, 3);
        fx.mem.write_u8(b + 8, 0x28);
        let x = rand_at(fx, r, 0x01E0_46D8);
        fx.mem.set8(b + 9, x >> 4);
        fx.mem.write_u32(b + 0x10, D + 0x3BEC);
        fx.mem.write_u32(b + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(b + o, 0x80);
        }
        copy_v4(fx, sp + 0xB0, b + 0x28);
    }
    r.s0 = 4;
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let mut r = Regs { s0: sx(param), s1: Some(0) };
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0xA0);
    match fx.mem.u16(D) {
        0xE0 => return 8,
        0x3C => storm(fx, sp, &mut r),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0xA..0x8C).contains(&n) {
        sparks(fx, sp, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0x24..0xC8).contains(&n) && n & 1 == 0 {
        node_ribbon(fx, &mut r);
    }
    let n = fx.mem.u16(D);
    if (0x3C..0xC5).contains(&n) {
        hits(fx, sp, &mut r);
    }
    shake_saving(fx, D + 0x40, r.s0, 0x01E0_475C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_476C, [Some(r.s0), r.s1]);
    blib::fade::fade(fx, D + 0x3BE4, OT);
    blib::star::flash(fx, 0x01E0_0410, D + 0x1904, OT, 0x18);
    blib::streak::ribbon(fx, 0x01E0_0FC0, D + 0xA4, OT, 0x3C);
    blib::bolt::bolts(fx, 0x01E0_1B50, D + 0x2024, OT, 0x64);
    blib::beam::beads(fx, 0x01E0_2BE0, D + 0x3924, OT, 8);
    let n = fx.mem.u16(D);
    if (0x3C..0xC5).contains(&n) {
        6
    } else if n == 0xC5 {
        2
    } else if n == 0x32 {
        1
    } else {
        0
    }
}
