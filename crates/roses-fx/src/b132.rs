//! BMODULE b132 (card 615), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06B80, data area a0 = 0x1ED0000, 0x2630 bytes)
//! [F]: s0 = D, s1 = the OT (0x206E40), s2 the
//! parameter then counters, s3..s6 pointers / counters. Init sets the
//! spark-emitter group (+0xA0, `spawn::emitters` / `sprite::sparks`: its
//! two update lists the record's 0x1E07860 / 0x1E07880 at nodes 0xD /
//! 0x73). Update 0x86 a long shake at node 0x68, then on 0x86 / 0x107 /
//! 0x10F / 0x116 five drops (+0x1D18, `drop::drops_d`) falling from node
//! 0x68; 0x88 eight rays (+0x165C, `spike::spikes_c`) and a flash at node
//! 0x68; 0x94 the comet (+0x2268, `comet::comets_b`) there; 0xF9 at the
//! target a spike (+0x19BC), eight bead fans (+0x1A24, `beam::beads_at`),
//! a flash, the frame rays (+0x1CE8, `window::frame_rays_b`) and a
//! shake; on 0x9E..0xFF one or two short flashes at node 0x68 an update.
//! Returns 6 on updates 0xF9..0x114, 2 on 0xF6, 1 on 0xEF, 8 on 0x154 (no
//! drawing then) and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `sparks`
//! 0x1E003A0, `fan_ring` 0x1E00C70, `emitters` 0x1E00F40, `flash_b`
//! 0x1E01390, `bars_b` 0x1E01E00, `spikes_c` 0x1E02700, `spikes`
//! 0x1E03430, `beads_at` 0x1E03F10, `frame_rays_b` 0x1E04B90, `drops_d`
//! 0x1E05400, `comets_b` 0x1E05C30 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const GROUP: u32 = D + 0xA0;
const SPARKS: u32 = D + 0xE4;
const FLASH: u32 = D + 0xC34;
const BARS: u32 = D + 0x151C;
const RAYS: u32 = D + 0x165C;
const SPIKE: u32 = D + 0x19BC;
const BEADS: u32 = D + 0x1A24;
const FRAME: u32 = D + 0x1CE8;
const DROPS: u32 = D + 0x1D18;
const COMET: u32 = D + 0x2268;
const BYTES: u32 = D + 0x252C;

/// The entry 0x1E06B80 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    // 0x199208 memset(D, 0, 0x2630).
    fx.bzero(D, 0x2630);
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
    for (at, node, clear) in [(0xCu32, 0x68, 1), (0x10, 0xD, 0), (0x14, 0x73, 0), (0x18, 1, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The emitter group (+0xA0).
    let g = GROUP;
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(g + 0x28, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(g + 0x29, b);
    fx.mem.write_u32(g + 8, 0x01E0_7860);
    fx.mem.write_u32(g + 0xC, 0x01E0_7880);
    fx.mem.write_u32(g, SPARKS);
    fx.mem.write_u8(g + 0x30, 0x10);
    fx.mem.write_u32(g + 4, D + 0x464);
    fx.mem.write_u8(g + 0x31, 0x32);
    fx.mem.write_u8(g + 0x32, 1);
    fx.mem.set16(g + 0x34, 0);
    fx.mem.write_u32(g + 0x38, 0x430C_0000);
    fx.mem.write_u32(g + 0x3C, 0);
    fx.mem.write_u32(g + 0x40, 0x40C0_0000);
    // The entry's s0 = D, s1 = 2 (the texture loop's), s2 = the parameter.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6CF4, [Some(D), Some(2), Some(param as u32)]);
    0
}

fn rand4(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) >> 4
}

/// node_position(self, node 0x68, sp + 0x90) with the entry's s0 / s1.
fn node_c(fx: &mut Fx, sp: u32, ra: u32) {
    let (h, s) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, s, sp + 0x90, u64::from(D), Some(u64::from(OT)), ra);
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand4(fx, ra);
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

/// The s registers the draws' frames save.
struct Regs {
    s2: u64,
    s3: u64,
    s4: u64,
    s5: u64,
    s6: u64,
}

/// Updates 0x86 / 0x107 / 0x10F / 0x116 [F 0x1E06DDC]: five drops falling
/// from node 0x68.
fn drops(fx: &mut Fx, sp: u32, r: &mut Regs) {
    for s2 in 0..5u32 {
        r.s2 = u64::from(s2);
        let i = first_free(fx, DROPS, 0x44, 0x14);
        if i < 0 {
            continue;
        }
        let s3 = D + 0x44 * i as u32;
        r.s3 = u64::from(s3);
        let q = s3 + 0x1D18;
        set_counter4(fx, q, 0x20, 0, 0xFF);
        let v = rand4(fx, 0x01E0_6E20);
        fx.mem.set8(q + 4, ((v & 3) << 4) | 4);
        fx.mem.write_u8(q + 9, 1);
        fx.mem.write_u8(q + 8, 1);
        let v = rand4(fx, 0x01E0_6E44);
        fx.mem.set8(q + 0xA, (v + 2) & 3);
        fx.mem.write_u8(q + 0xB, 0);
        for o in [0xCu32, 0xD, 0xE] {
            fx.mem.write_u8(q + o, 0x80);
        }
        fx.mem.write_u8(q + 0xF, 0x50);
        fx.mem.write_u32(q + 0x10, 0x4316_0000);
        fx.mem.write_u32(q + 0x14, 0x4100_0000);
        fx.mem.write_u32(q + 0x18, 0xBCA3_D70A);
        fx.mem.write_u32(q + 0x1C, 0);
        node_c(fx, sp, 0x01E0_6EA0);
        let f = ee::add(fx.mem.f(sp + 0x90), jit(fx, 0x01E0_6EA8, 0x46, 0x23));
        fx.mem.setf(q + 0x20, f);
        let f = ee::sub(fx.mem.f(sp + 0x94), 3.0);
        fx.mem.setf(q + 0x24, f);
        let f = ee::add(fx.mem.f(sp + 0x98), jit(fx, 0x01E0_6EF8, 0x46, 0x23));
        fx.mem.setf(q + 0x28, f);
        let f = jit(fx, 0x01E0_6F38, 0x3C, 0x1E);
        fx.mem.setf(q + 0x2C, f);
        fx.mem.write_u32(q + 0x30, 0x3ECC_CCCD);
        let f = jit(fx, 0x01E0_6F80, 0x3C, 0x1E);
        fx.mem.setf(q + 0x34, f);
    }
    r.s2 = 5;
}

/// Update 0x88 [F 0x1E06FCC]: eight rays and a flash at node 0x68.
fn rays(fx: &mut Fx, r: &mut Regs) {
    for s3 in 0..8u32 {
        let q = RAYS + 0x6C * s3;
        set_counter4(fx, q, 0xE, (s3 & 0xFF) as i32, 0xFF);
        fx.mem.write_u8(q + 4, 3);
        fx.mem.write_u32(q + 0x14, 0x40E0_0000);
        fx.mem.write_u32(q + 0x18, 0x4040_0000);
        fx.mem.write_u32(q + 0x20, 0x41F0_0000);
        fx.mem.write_u8(q + 5, 1);
        let b = fx.mem.read_u8(D + 0xC);
        fx.mem.write_u8(q + 6, b);
        for (o, v) in [(0xAu32, 0u8), (0xB, 2), (0xC, 7), (8, 0x19)] {
            fx.mem.write_u8(q + o, v);
        }
        let v = rand4(fx, 0x01E0_7034);
        fx.mem.set8(q + 9, v);
        fx.mem.write_u32(q + 0x10, BYTES);
        for o in [0x28u32, 0x29, 0x2A, 0x24, 0x25, 0x26, 0x2B] {
            fx.mem.write_u8(q + o, 0x80);
        }
        fx.mem.write_u8(q + 0x27, 0x40);
    }
    r.s3 = 8;
    r.s2 = u64::from(D + 0x6C * 8);
    let a = FLASH;
    set_counter4(fx, a, 0x10, 0xC, 0xFF);
    fx.mem.write_u8(a + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(a + 6, b);
    for (o, v) in [(4u32, 0x81u8), (7, 9), (0xA, 0xC0), (9, 0x40), (8, 0x10), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x10, 0x4548_0000);
    fx.mem.write_u32(a + 0x14, 0xC348_0000);
}

/// Update 0x94 [F 0x1E070F4]: the comet at node 0x68.
fn comet(fx: &mut Fx) {
    let c = COMET;
    set_counter4(fx, c, 0x6C, 0, 0xFF);
    fx.mem.write_u8(c + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(c + 6, b);
    fx.mem.write_u8(c + 4, 5);
    fx.mem.write_u32(c + 8, D + 0x22AC);
    fx.mem.write_u32(c + 0x28, 0x28);
    for (o, v) in [(0x1Eu32, 8u8), (0x1F, 0), (0x21, 2), (0x22, 3), (0x23, 3), (0x34, 0x80), (0x35, 0x80), (0x36, 0x80), (0x37, 0x80)] {
        fx.mem.write_u8(c + o, v);
    }
    fx.mem.write_u32(c + 0x2C, 0x4348_0000);
    fx.mem.write_u32(c + 0x30, 0x4348_0000);
    let n = u32::from(fx.mem.read_u8(c + 0x1E)) + 1;
    for k in 0..n {
        fx.mem.write_u16(c + 0xC + 2 * k, 0xFFFF);
    }
}

/// Update 0xF9 [F 0x1E071A8]: at the target (sp + 0x80) a spike, eight
/// bead fans, a flash, the frame rays and a shake.
fn strike(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let s = SPIKE;
    set_counter4(fx, s, 0x1E, 0, 0xFF);
    fx.mem.write_u8(s + 4, 5);
    fx.mem.write_u32(s + 0x14, 0x4033_3333);
    fx.mem.write_u32(s + 0x1C, 0x4220_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 7), (8, 0xA0)] {
        fx.mem.write_u8(s + o, v);
    }
    let v = rand4(fx, 0x01E0_71F8);
    fx.mem.set8(s + 9, v);
    fx.mem.write_u32(s + 0x10, BYTES);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    for (o, v) in [(0x20u32, 0x80u8), (0x21, 0x80), (0x22, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(s + o, v);
    }
    copy_v4(fx, sp + 0x80, s + 0x28);
    fx.mem.write_u32(s + 0x58, 0x40);
    fx.mem.write_u32(s + 0x5C, 0x80);
    fx.mem.write_u32(s + 0x60, 0x100);
    for s2 in 0..8u32 {
        let q = BEADS + 0x58 * s2;
        let v = rand4(fx, 0x01E0_7280);
        set_counter4(fx, q, 0x20, v & 7, 0xFF);
        fx.mem.write_u8(q + 4, 5);
        fx.mem.write_u8(q + 5, 0);
        fx.mem.write_u8(q + 8, 0x50);
        let v = rand4(fx, 0x01E0_72B0);
        fx.mem.set8(q + 9, v);
        fx.mem.write_u8(q + 0xA, 0);
        fx.mem.write_u8(q + 0xC, 2);
        let v = fx.rand_from(D, 0x01E0_72C8);
        fx.mem.set8(q + 0xB, (v & 3) + 4);
        fx.mem.write_u32(q + 0x1C, 0x41C8_0000);
        fx.mem.write_u32(q + 0x20, 0);
        fx.mem.write_u32(q + 0x10, BYTES);
        fx.mem.write_u32(q + 0x14, 0x3FB3_3333);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(q + o, 0x80);
        }
        copy_v4(fx, sp + 0x80, q + 0x28);
    }
    r.s2 = 8;
    r.s3 = u64::from(D + 0x58 * 8);
    let a = FLASH;
    set_counter4(fx, a, 0x20, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 1), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x60), (9, 0x10), (8, 0xC0)] {
        fx.mem.write_u8(a + o, v);
    }
    fx.mem.write_u32(a + 0x14, 0);
    fx.mem.write_u32(a + 0x10, 0x4448_0000);
    copy_v4(fx, sp + 0x80, a + 0x1C);
    let f = FRAME;
    set_counter4(fx, f, 0x18, 0, 0xFF);
    copy_v4(fx, sp + 0x80, f + 0xC);
    for (o, v) in [(0x1Cu32, 0xC0u8), (0x1D, 0xC0), (0x1E, 0xC0), (0x1F, 0x70), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x10), (8, 0xC), (9, 2), (0xA, 0xFF)] {
        fx.mem.write_u8(f + o, v);
    }
    set_counter4(fx, D + 0x40, 0x20, 0, 0xFF);
    fx.mem.write_u8(D + 0x45, 0);
    fx.mem.write_u8(D + 0x44, 0);
    fx.mem.write_u32(D + 0x54, 0x3FC0_0000);
    fx.mem.write_u32(D + 0x58, 0x3F80_0000);
    fx.mem.write_u32(D + 0x5C, 0x4000_0000);
    fx.mem.write_u32(D + 0x70, 0);
    fx.mem.write_u32(D + 0x74, 0x42A0_0000);
    fx.mem.write_u32(D + 0x90, 0x40C0_0000);
    copy_v4(fx, sp + 0x80, D + 0x60);
}

/// Updates 0x9E..0xFF [F 0x1E074F4]: two (from 0xF5 one) short flashes at
/// node 0x68, the first made a 1-count bright one.
fn sparkle(fx: &mut Fx, sp: u32, r: &mut Regs) {
    let n = if fx.mem.u16(D) < 0xF5 { 2 } else { 1 };
    r.s2 = n as u64;
    for s6 in 0..n {
        r.s6 = s6 as u64;
        let i = first_free(fx, FLASH, 0x4C, 0x1E);
        r.s3 = i64::from(i) as u64;
        if i < 0 {
            continue;
        }
        node_c(fx, sp, 0x01E0_7520);
        let s3 = D + 0x4C * i as u32;
        let a = s3 + 0xC34;
        (r.s3, r.s4, r.s5) = (u64::from(s3), u64::from(a), u64::from(a + 4));
        set_counter4(fx, a, 6, 0, 0xFF);
        for (o, v) in [(5u32, 0u8), (4, 0x41), (7, 9), (0xF, 0xA0), (0xC, 0xA0), (0xD, 0xA0), (0xE, 0xA0), (0xA, 0xC0), (9, 0x40), (8, 0x10)] {
            fx.mem.write_u8(a + o, v);
        }
        fx.mem.write_u32(a + 0x14, 0xC170_0000);
        fx.mem.write_u32(a + 0x10, 0x43A0_0000);
        copy_v4(fx, sp + 0x90, a + 0x1C);
        if s6 == 0 {
            set_counter4(fx, a, 1, 0, 0xFF);
            let b = fx.mem.read_u8(a + 4) | 0x80;
            fx.mem.write_u8(a + 4, b);
        }
    }
    r.s6 = n as u64;
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut r = Regs { s2: i64::from(param) as u64, s3: 0, s4: 0, s5: 0, s6: 0 };
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x80, u64::from(D), 0x01E0_6D14);
    match fx.mem.u16(D) {
        0x154 => return 8,
        0xF9 => strike(fx, sp, &mut r),
        0x94 => comet(fx),
        0x88 => {
            r.s3 = 0;
            rays(fx, &mut r);
        }
        v => {
            // The delay slots of the 0x88 / 0x116 tests clear s3 / s2.
            (r.s3, r.s2) = (0, 0);
            if v == 0x86 {
                set_counter4(fx, D + 0x40, 0x73, 0, 0xFF);
                fx.mem.write_u8(D + 0x45, 1);
                let b = fx.mem.read_u8(D + 0xC);
                fx.mem.write_u8(D + 0x46, b);
                fx.mem.write_u8(D + 0x44, 0);
                fx.mem.write_u32(D + 0x54, 0x4000_0000);
                fx.mem.write_u32(D + 0x58, 0x3FE6_6666);
                fx.mem.write_u32(D + 0x5C, 0x3F80_0000);
                fx.mem.write_u32(D + 0x70, 0);
                fx.mem.write_u32(D + 0x74, 0x42A0_0000);
                fx.mem.write_u32(D + 0x90, 0x4080_0000);
            }
            if matches!(v, 0x86 | 0x107 | 0x10F | 0x116) {
                drops(fx, sp, &mut r);
            }
        }
    }
    let n = fx.mem.u16(D);
    if (0x9E..0x100).contains(&n) {
        sparkle(fx, sp, &mut r);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_760C, 0x01E0_00D8, [0x01E0_0374, 0x01E0_0388]);
    let n = fx.mem.u16(D);
    blib::spawn::emitters(fx, 0x01E0_0F40, GROUP, n);
    // emitters' 0xC0 frame saves the entry's s0..s6 and ra.
    fx.saved(sp - 0xC0, &[(0, u64::from(D)), (0x10, u64::from(OT)), (0x20, r.s2), (0x30, r.s3), (0x40, r.s4), (0x50, r.s5), (0x60, r.s6), (0x70, 0x01E0_7618)]);
    let a3 = blib::sprite::sparks_a3(fx, SPARKS, OT, 0x10).1;
    blib::window::frame_rays_b(fx, 0x01E0_4B90, FRAME, OT, 1, a3.unwrap_or(0), [0, 0]);
    blib::star::flash_b(fx, 0x01E0_1390, FLASH, OT, 0x1E);
    blib::cross::bars_b(fx, 0x01E0_1E00, BARS, OT, 4);
    blib::spike::spikes_c(fx, 0x01E0_2700, RAYS, OT, 8);
    blib::spike::spikes(fx, 0x01E0_3430, SPIKE, OT, 1);
    blib::beam::beads_at(fx, 0x01E0_3F10, 0x34, 0x3C, BEADS, OT, 8);
    blib::drop::drops_d(fx, DROPS, OT, 0x14);
    blib::comet::comets_b(fx, 0x01E0_5C30, COMET, OT, 1);
    let n = fx.mem.u16(D);
    if (0xF9..0x115).contains(&n) {
        6
    } else if n == 0xF6 {
        2
    } else if n == 0xEF {
        1
    } else {
        0
    }
}
