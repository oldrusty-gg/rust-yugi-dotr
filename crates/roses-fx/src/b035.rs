//! BMODULE b035 (card 104), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02090, data area a0 = 0x1ED0000) [F] (helper B3, 30
//! Oct 2026): b076's family. On update 92 two flashes (`star::flash`, pool
//! of 40 at +0xA4, 0x4C each) at the attacker's nodes 0x29 / 0x2A and a
//! fade (+0xC84); on updates 95..179 (not multiples of 8) two ribbons a
//! update (`streak::ribbon`, pool of 40 at +0xC8C, 0x68 each) from those
//! nodes; on 132 a camera shake (+0x40); on 132..176 two small flashes and
//! two ribbons a update at random points round the target (+-15); the stage
//! light pulsed by 0x1031C0 (+0xA0: down on update 2, up on 220); object
//! ops 0x1F83B0 from the record's table 0x1E02A40. Returns 1 on update
//! 122, 6 on 132..176, 5 on 177, 8 on 226 (and at once for a parameter of
//! -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `flash`
//! 0x1E00410, `ribbon` 0x1E00FC0, `fade` 0x1E01B50 [`crate::blib`].
use crate::b076::{copy_v4, rand_bytes, shake_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The object-op table (rodata, 0x14-byte entries up to a 0 start).
const OPS: u32 = 0x01E0_2A40;

/// 0x1031C0(p, n) [F]: the stage light 0x80 - 16 k on the k-th update
/// (1..5) from u16 p[0], 0x30 + 16 k from u16 p[1]; leaves its frame (0x30:
/// the caller's s0 +0 / s1 +0x10 (`None`: not known, left), `ra` +0x20).
pub(crate) fn light_ramp(fx: &mut Fx, p: u32, n: i32, ra: u32, caller_s: [Option<u64>; 2]) {
    let f = fx.sp - 0x30;
    for (k, v) in caller_s.iter().enumerate() {
        if let Some(v) = v {
            fx.saved(f + 0x10 * k as u32, &[(0, *v)]);
        }
    }
    fx.saved(f, &[(0x20, u64::from(ra))]);
    let k = (n - fx.mem.u16(p) + 1) & 0xFFFF;
    if k != 0 && k < 6 {
        fx.set_stage_light((0x80 - (k << 4)) & 0xFF);
    }
    let k = (n - fx.mem.u16(p + 2) + 1) & 0xFFFF;
    if k != 0 && k < 6 {
        fx.set_stage_light(((k << 4) + 0x30) & 0xFF);
    }
}

/// The entry's object-op walk [F 0x1E02824..0x1E028E8]: the first entry
/// of the table at `ops` (0x14 bytes: u16 start, u16 end, mode +4, flags
/// +5 (0x80 the target, 0x40 fade: bytes +0xF / +0x13 = (n - start) x
/// rate +6, 3: a2), arguments +8 / +0xC / +0x10) whose [start, end] holds
/// `n` reports 0x1F83B0(handle, mode, flags & 3, entry + 0xC).
pub(crate) fn object_ops(fx: &mut Fx, ops: u32, n: i32) {
    let mut e = ops;
    while fx.mem.u16(e) != 0 {
        if n >= fx.mem.u16(e) && fx.mem.u16(e + 2) >= n {
            let flags = fx.mem.u8(e + 5);
            let h = if flags & 0x80 != 0 { fx.target_handle() } else { fx.self_handle() };
            if flags & 0x40 != 0 {
                let rate = fx.mem.u8(e + 6);
                let d = n - fx.mem.u8(e);
                let v = fx.mult(rate, d);
                fx.mem.set8(e + 0x13, v);
                fx.mem.set8(e + 0xF, v);
            }
            let mode = fx.mem.u8(e + 4);
            fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h as u32, mode as u32, (flags & 3) as u32, e + 0xC]);
            return;
        }
        e += 0x14;
    }
}

/// The entry 0x1E02090 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x1DD0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 0x29, 1), (0x10, 0x2A, 0), (0x14, 0x4F, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xDC);
    // The entry's s0 = D, s1 = the OT, s2 = 1 (the texture loop's).
    rand_bytes(fx, D + 0x1CCC, 0x100, 0x01E0_21C0, [Some(D), Some(OT), Some(1)]);
    0
}

/// A ribbon's common fields (`r`: a pool record).
fn ribbon_tail(fx: &mut Fx, r: u32) {
    fx.mem.write_u8(r + 0xA, 0);
    fx.mem.write_u8(r + 0xB, 2);
    fx.mem.write_u8(r + 0xC, 3);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, sp + 0x50);
    match fx.mem.u16(D) {
        0xE2 => return 8,
        0x84 => {
            let s = D + 0x40;
            set_counter4(fx, s, 0x30, 0, 0xFF);
            fx.mem.write_u8(s + 5, 0);
            fx.mem.write_u8(s + 4, 8);
            fx.mem.write_u32(s + 0x14, 0x3FCC_CCCD);
            fx.mem.write_u32(s + 0x18, 0x3F80_0000);
            fx.mem.write_u32(s + 0x1C, 0x4000_0000);
            fx.mem.write_u32(s + 0x30, 0);
            fx.mem.write_u32(s + 0x34, 0x4220_0000);
            copy_v4(fx, sp + 0x50, D + 0x60);
            fx.mem.write_u32(s + 0x50, 0x4080_0000);
        }
        0x5C => {
            for k in 0..2i32 {
                let i = first_free(fx, D + 0xA4, 0x4C, 0x28);
                if i < 0 {
                    continue;
                }
                let f = D + 0x4C * i as u32 + 0xA4;
                set_counter4(fx, f, 0xFF, k & 0xFF, 0xFF);
                fx.mem.write_u8(f + 5, 1);
                let b = fx.mem.read_u8(D + 4 * k as u32 + 0xC);
                fx.mem.write_u8(f + 6, b);
                for (o, v) in [(4u32, 0x85u8), (7, 9), (0xA, 0x40), (9, 0x10), (8, 0x80), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0)] {
                    fx.mem.write_u8(f + o, v);
                }
                fx.mem.write_u32(f + 0x10, 0x420C_0000);
                fx.mem.write_u32(f + 0x14, 0);
            }
            set_counter4(fx, D + 0xC84, 0x14, 0x14, 0xFF);
            fx.mem.write_u8(D + 0xC88, 0x82);
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x5F..0xB4).contains(&n) && n & 7 != 0 {
        for k in 0..2u32 {
            let i = first_free(fx, D + 0xC8C, 0x68, 0x28);
            if i < 0 {
                continue;
            }
            let r = D + 0x68 * i as u32 + 0xC8C;
            set_counter4(fx, r, 5, 1, 0xFF);
            fx.mem.write_u8(r + 4, 0x85);
            fx.mem.write_u32(r + 0x14, 0x4080_0000);
            fx.mem.write_u32(r + 0x1C, 0x4170_0000);
            fx.mem.write_u8(r + 8, 0x10);
            let b = fx.mem.read_u8(D + 4 * k + 0xC);
            fx.mem.write_u8(r + 6, b);
            fx.mem.write_u8(r + 5, 1);
            ribbon_tail(fx, r);
            let x = fx.rand_from(D, 0x01E0_2478);
            fx.mem.set8(r + 9, x >> 4);
            fx.mem.write_u32(r + 0x10, D + 0x1CCC);
            fx.mem.write_u32(r + 0x18, 0x3F80_0000);
            for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
                fx.mem.write_u8(r + o, 0x80);
            }
            fx.mem.write_u8(r + 0x23, 0x10);
            fx.mem.write_u8(r + 0x27, 0x40);
            for (o, ra) in [(0x58u32, 0x01E0_24C8u32), (0x5C, 0x01E0_24E0), (0x60, 0x01E0_24F8)] {
                let x = fx.rand_from(D, ra);
                fx.mem.set32(r + o, (x >> 4) & 0x7F);
            }
        }
    }
    let n = fx.mem.u16(D);
    if (0x84..0xB1).contains(&n) {
        for _ in 0..2 {
            for (c, ra) in [(0u32, 0x01E0_2550u32), (4, 0x01E0_259C), (8, 0x01E0_25E8)] {
                let x = fx.rand_from(D, ra) >> 4;
                let (_, h) = fx.div(x, 0xBB8);
                let f = ee::div(ee::from_i32(h - 0x5DC), 100.0);
                let v = ee::add(fx.mem.f(sp + 0x50 + c), f);
                fx.mem.setf(sp + 0x60 + c, v);
            }
            let i = first_free(fx, D + 0xA4, 0x4C, 0x28);
            if i >= 0 {
                let f = D + 0x4C * i as u32 + 0xA4;
                set_counter4(fx, f, 0xC, 0, 0xFF);
                for (o, v) in [(5u32, 0u8), (4, 0), (7, 9), (0xF, 0xB0), (0xC, 0xB0), (0xD, 0xB0), (0xE, 0xB0), (0xA, 0x40), (9, 0x10), (8, 0x80)] {
                    fx.mem.write_u8(f + o, v);
                }
                fx.mem.write_u32(f + 0x14, 0);
                fx.mem.write_u32(f + 0x10, 0x4396_0000);
                fx.mem.write_u32(f + 0x14, 0x4170_0000);
                fx.mem.write_u32(f + 0x18, 0x4120_0000);
                copy_v4(fx, sp + 0x60, f + 0x1C);
            }
            let i = first_free(fx, D + 0xC8C, 0x68, 0x28);
            if i >= 0 {
                let r = D + 0x68 * i as u32 + 0xC8C;
                fx.mem.write_u8(r + 4, 5);
                set_counter4(fx, r, 0xA, 0, 0xFF);
                fx.mem.write_u32(r + 0x14, 0x40A0_0000);
                fx.mem.write_u32(r + 0x1C, 0x4220_0000);
                fx.mem.write_u8(r + 0x27, 0xA0);
                fx.mem.write_u8(r + 0x23, 0);
                fx.mem.write_u8(r + 5, 0);
                ribbon_tail(fx, r);
                fx.mem.write_u8(r + 8, 0x28);
                let x = fx.rand_from(D, 0x01E0_2794);
                fx.mem.set8(r + 9, x >> 4);
                fx.mem.write_u32(r + 0x10, D + 0x1CCC);
                fx.mem.write_u32(r + 0x18, 0x3F80_0000);
                for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
                    fx.mem.write_u8(r + o, 0x80);
                }
                copy_v4(fx, sp + 0x60, r + 0x28);
            }
        }
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_2814, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_2824, [Some(u64::from(D)), Some(u64::from(OT))]);
    let n = fx.mem.u16(D);
    object_ops(fx, OPS, n);
    blib::star::flash(fx, 0x01E0_0410, D + 0xA4, OT, 0x28);
    blib::fade::fade(fx, D + 0xC84, OT);
    blib::streak::ribbon(fx, 0x01E0_0FC0, D + 0xC8C, OT, 0x28);
    let n = fx.mem.u16(D);
    if (0x84..0xB1).contains(&n) {
        6
    } else if n == 0xB1 {
        5
    } else if n == 0x7A {
        1
    } else {
        0
    }
}
