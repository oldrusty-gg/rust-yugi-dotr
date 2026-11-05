//! BMODULE b172 (card 317), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E064B0, data area a0 = 0x1ED0000, 0x8EF0 bytes)
//! [F]: a second-library monolith (s2 = D, s3
//! the OT, s0 the parameter). Init: handles D+4 / D+8, the self unit's
//! rotation D+0x2C, nodes 1 (cleared) / 0x4B / 0xC / 0x49 into D+0xC..
//! 0x18, light ramp D+0xA0 = (2, 0xFD), 256 random bytes D+0x8408. Each
//! update (the counter u16 D+0) reads the target's position into sp +
//! 0x80; 0x103 returns 8 at once; 0xE a streak (+0xFA4, `radiate::
//! streaks_b`) from node 1 (the centroid D+0x8EE0, y from node 0x4B - 2);
//! 0x22 thirty comets (+0x8508, `comet::comets_b`) with random aims
//! D+0x8D00 / radii D+0x8E68 steered by 0x1E062B0 about the centroid;
//! 0x32 eight tiles (+0x221C, `sheet::tiles`) there, released one a
//! update on 0x40..0x47; 0x48 fifty falling sparks (+0x172C, 0x1E02940);
//! 0x4E thirty-six frame-sampling wedges (+0xFDC, 0x1E020F0) round the
//! centroid; 0x54 two emitters (+0x3184, 0x1E04070) at nodes 0xC / 0x49
//! leaving sprite rings (+0x243C, 0x1E03680); 0x9C two flashes (+0x323C,
//! `star::flash_b`) there; 0xC3 thirty-two bursts and two hundred drops
//! (+0x3320, `drop::drops`), three spikes (+0x82D0, `spike::spikes`), a
//! wide flash and a shake (+0x40) at the target. Returns 6 on 0xC3..0xDF,
//! 2 on 0xE0, 1 on 0xB9, 8 on 0x103 and at once for -2 / -3.
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `comets_b`
//! 0x1E003A0, `streaks_b` 0x1E012F0, `tiles` 0x1E02F40, `flash_b`
//! 0x1E04400, `drops` 0x1E04E70, `spikes` 0x1E057D0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, unit_position_saving};
use crate::blib::{
    self,
    arc::resample,
    exe::{aim_angles, alpha_mode, atlas_cell, camera_vp_into, countdown, cvt_u32, first_free, libm_cosf, mul_mem, node_source, rot_trans_into, set_counter4, tex_rect, translation_into},
};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PI: u32 = 0x4049_0FDB;
const ASPECT: u32 = 0x3FAA_3D71;
const SHAKE: u32 = D + 0x40;
const LIGHT: u32 = D + 0xA0;
const STREAK: u32 = D + 0xFA4;
const WEDGES: u32 = D + 0xFDC;
const SPARKS: u32 = D + 0x172C;
const TILES: u32 = D + 0x221C;
const RINGS: u32 = D + 0x243C;
const EMITTERS: u32 = D + 0x3184;
const FLASHES: u32 = D + 0x323C;
const DROPS: u32 = D + 0x3320;
const SPIKES: u32 = D + 0x82D0;
const BYTES: u32 = D + 0x8408;
const COMETS: u32 = D + 0x8508;
const AIMS: u32 = D + 0x8D00;
const RADII: u32 = D + 0x8E68;
const CENTRE: u32 = D + 0x8EE0;

/// The entry 0x1E064B0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x8EF0);
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
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x4B, 0), (0x14, 0xC, 0), (0x18, 0x49, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(LIGHT, 2);
    fx.mem.set16(LIGHT + 2, 0xFD);
    // The entry's s0 = 3 (the texture loop's), s1 the runner's 0, s2 = D.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_65D0, [Some(3), Some(0), Some(D)]);
    0
}

fn sx(v: i32) -> u64 {
    i64::from(v) as u64
}

/// rand from return address `ra` with the entry's s0 (64 bits) saved in
/// its frame.
fn rand64(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

/// ((rand >> 4) % m - half) / 100.
fn jit(fx: &mut Fx, s0: u64, ra: u32, m: i32, half: i32) -> f32 {
    let v = rand64(fx, s0, ra) >> 4;
    let (_, r) = fx.div(v, m);
    ee::div(ee::from_i32(r - half), 100.0)
}

fn copy3(fx: &mut Fx, from: u32, to: u32) {
    for k in 0..3u32 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The entry's s0 / s1 after the dispatch (light_ramp saves them).
struct Regs {
    s0: u64,
    s1: u64,
    s4: u64,
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut r = Regs { s0: sx(param), s1: 0, s4: 0 };
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x80, r.s0, 0x01E0_65F0);
    let n = fx.mem.u16(D);
    if n == 0x103 {
        return 8;
    }
    match n {
        0xC3 => finale(fx, sp, &mut r),
        0x9C => {
            for k in 0..2u32 {
                let f = FLASHES + 0x4C * k;
                set_counter4(fx, f, 0x40, 0, 0xFF);
                fx.mem.write_u8(f + 5, 1);
                let b = fx.mem.read_u8(D + 0x14 + 4 * k);
                fx.mem.write_u8(f + 6, b);
                fx.mem.write_u8(f + 4, 0x81);
                fx.mem.write_u8(f + 7, 9);
                fx.mem.write_u32(f + 0x10, 0x4348_0000);
                fx.mem.write_u32(f + 0x14, 0);
                for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0x80), (8, 0x20)] {
                    fx.mem.write_u8(f + o, v);
                }
                fx.mem.write_u8(EMITTERS + 0x5C * k + 0x2B, 0x50);
            }
            r.s0 = u64::from(D + 0x4C * 2);
            r.s1 = u64::from(D + 8);
            r.s4 = u64::from(D + 0x5C * 2);
        }
        0x54 => {
            for k in 0..2u32 {
                let g = EMITTERS + 0x5C * k;
                set_counter4(fx, g, 0x86, 0, 0xFF);
                fx.mem.write_u8(g + 4, 0);
                fx.mem.write_u8(g + 5, 1);
                let b = fx.mem.read_u8(D + 0x14 + 4 * k);
                fx.mem.write_u8(g + 6, b);
                fx.mem.write_u32(g + 8, RINGS);
                fx.mem.write_u32(g + 0xC, BYTES);
                for (o, v) in [(0x10u32, 0x32u8), (0x11, 8), (0x12, 8), (0x13, 2), (0x14, 5), (0x15, 2), (0x16, 2), (0x17, 3)] {
                    fx.mem.write_u8(g + o, v);
                }
                fx.mem.write_u32(g + 0x18, 0x3F99_999A);
                fx.mem.write_u32(g + 0x1C, 0x3F80_0000);
                fx.mem.write_u32(g + 0x20, 0x42F0_0000);
                for (o, v) in [(0x2Au32, 0x60u8), (0x29, 0x80), (0x28, 0x30), (0x2B, 0x20)] {
                    fx.mem.write_u8(g + o, v);
                }
                let h = fx.mem.i32(D + 4);
                let s = fx.mem.i32(D + 0x14 + 4 * k);
                fx.node_position(h, s, g + 0x3C);
                copy_v4(fx, g + 0x3C, g + 0x4C);
            }
            r.s0 = u64::from(D + 0x5C * 2);
            r.s1 = u64::from(D + 8);
            r.s4 = 2;
        }
        0x4E => {
            for k in 0..0x24u32 {
                let g = WEDGES + 0x34 * k;
                set_counter4(fx, g, 0x5E, 0, 0xFF);
                fx.mem.write_u8(g + 4, 0);
                fx.mem.write_u8(g + 5, 0);
                fx.mem.write_u8(g + 8, 8);
                fx.mem.write_u8(g + 9, 2);
                fx.mem.write_u32(g + 0xC, 0x4170_0000);
                fx.mem.write_u32(g + 0x10, 0);
                fx.mem.setf(g + 0x14, ee::mul(10.0, ee::from_i32(k as i32)));
                fx.mem.write_u32(g + 0x18, 0);
                fx.mem.write_u32(g + 0x1C, 0x4120_0000);
                for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x27, 0), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x90)] {
                    fx.mem.write_u8(g + o, v);
                }
                copy3(fx, CENTRE, g + 0x28);
            }
            r.s0 = u64::from(D + 0x34 * 0x24);
            r.s1 = 0x24;
        }
        0x48 => {
            for k in 0..0x32 {
                let i = first_free(fx, SPARKS, 0x38, 0x32);
                r.s0 = sx(i);
                if i < 0 {
                    continue;
                }
                let v = rand64(fx, r.s0, 0x01E0_6AE0);
                let s0 = D + 0x38 * i as u32;
                r.s0 = u64::from(s0);
                let g = s0 + 0x172C;
                set_counter4(fx, g, 0x58, v & 0xF, 7);
                for (o, b) in [(4u32, 0u8), (5, 0), (8, 0), (9, 0), (0xA, 6), (0xB, 2)] {
                    fx.mem.write_u8(g + o, b);
                }
                fx.mem.write_u32(g + 0xC, 0x898);
                let f = jit(fx, r.s0, 0x01E0_6B30, 100, 50);
                fx.mem.setf(g + 0x18, ee::add(f32::from_bits(0x3FCC_CCCD), f));
                for o in [0x1Cu32, 0x1D, 0x1E, 0x1F] {
                    fx.mem.write_u8(g + o, 0x80);
                }
                fx.mem.write_u32(g + 0x14, 0x4248_0000);
                copy3(fx, CENTRE, g + 0x20);
                let _ = k;
            }
            r.s1 = 0x32;
        }
        0x40..=0x47 => {
            fx.mem.write_u8(D + 0x44 * n as u32 + 0x1120, 4);
        }
        0x32 => {
            for k in 0..8u32 {
                let t = TILES + 0x44 * k;
                set_counter4(fx, t, 0x78, k as i32, 0xFF);
                fx.mem.write_u8(t + 5, 0);
                fx.mem.write_u8(t + 4, 0);
                fx.mem.write_u8(t + 8, 1);
                fx.mem.write_u8(t + 9, 5);
                fx.mem.write_u8(t + 0xB, 2);
                let f = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(k as i32));
                fx.mem.write_u8(t + 0xA, 0);
                fx.mem.setf(t + 0xC, ee::add(1.0, f));
                fx.mem.write_u32(t + 0x10, 0x3F80_0000);
                for (o, v) in [(0x14u32, 0x80u8), (0x15, 0x80), (0x16, 0x80), (0x17, 0x10)] {
                    fx.mem.write_u8(t + o, v);
                }
                fx.mem.write_u32(t + 0x1C, (-0x3800i32) as u32);
                fx.mem.write_u32(t + 0x2C, 0x400);
                copy3(fx, CENTRE, t + 0x38);
            }
            r.s0 = u64::from(D + 0x44 * 8);
            r.s1 = 8;
        }
        0x22 => {
            for _ in 0..0x1E {
                let i = first_free(fx, COMETS, 0x44, 0x1E);
                r.s0 = sx(i);
                if i < 0 {
                    continue;
                }
                let iu = i as u32;
                let v = rand64(fx, r.s0, 0x01E0_67B0) >> 4;
                let (_, h) = fx.div(v, 5);
                fx.mem.setf(AIMS + 0xC * iu + 4, ee::div(ee::from_i32(200 * h - 500), 100.0));
                let f = jit(fx, r.s0, 0x01E0_6810, 0x96, 0x4B);
                fx.mem.setf(RADII + 4 * iu, ee::sub(3.0, f));
                let delay = (rand64(fx, r.s0, 0x01E0_6860) >> 4) & 7;
                let s0 = D + 0x44 * iu;
                r.s0 = u64::from(s0);
                let c = s0 + 0x8508;
                set_counter4(fx, c, 0x14, delay, 7);
                fx.mem.write_u8(c + 5, 0);
                fx.mem.write_u8(c + 4, 6);
                fx.mem.write_u32(c + 8, D + 0xA4);
                fx.mem.write_u32(c + 0x28, 0xF0);
                for (o, b) in [(0x1Eu32, 8u8), (0x1F, 0), (0x21, 2), (0x22, 3), (0x23, 6), (0x37, 0x80), (0x34, 0x80), (0x35, 0x80), (0x36, 0x80)] {
                    fx.mem.write_u8(c + o, b);
                }
                fx.mem.write_u32(c + 0x2C, 0x42C8_0000);
                fx.mem.write_u32(c + 0x30, 0x41F0_0000);
                let m = fx.mem.u8(c + 0x1E);
                for k in 0..(m + 1) as u32 {
                    fx.mem.write_u16(c + 0xC + 2 * k, 0xFFFF);
                }
                r.s0 = u64::from(s0 + 2 * (m + 1) as u32);
            }
            r.s1 = 0x1E;
        }
        0xE => {
            let h = fx.mem.i32(D + 4);
            let s = fx.mem.i32(D + 0xC);
            fx.node_position(h, s, CENTRE);
            let h = fx.mem.i32(D + 4);
            let s = fx.mem.i32(D + 0x10);
            fx.node_position(h, s, sp + 0x90);
            let y = ee::sub(fx.mem.f(sp + 0x94), 2.0);
            set_counter4(fx, STREAK, 0x18, 0, 0xFF);
            fx.mem.setf(CENTRE + 4, y);
            for (o, b) in [(4u32, 0x80u8), (9, 2), (0xA, 0xFF), (8, 8), (0xB, 8)] {
                fx.mem.write_u8(STREAK + o, b);
            }
            let h = fx.mem.read_u32(D + 4);
            fx.mem.write_u32(STREAK + 0xC, h);
            for (o, b) in [(0x16u32, 0x80u8), (0x15, 0x80), (0x14, 0x80), (0x17, 0x10), (0x12, 0x80), (0x11, 0x80), (0x10, 0x80), (0x13, 0x30), (5, 1)] {
                fx.mem.write_u8(STREAK + o, b);
            }
            let b = fx.mem.read_u8(D + 0xC);
            fx.mem.write_u8(STREAK + 6, b);
        }
        _ => {}
    }
    steer(fx, &r);
    blib::shake::camera_shake(fx, SHAKE);
    let n = fx.mem.u16(D);
    light_ramp(fx, LIGHT, n, 0x01E0_748C, [Some(r.s0), Some(r.s1)]);
    blib::comet::comets_b(fx, 0x01E0_03A0, COMETS, OT, 0x1E);
    blib::radiate::streaks_b(fx, 0x01E0_12F0, STREAK, OT, 1);
    sparks(fx, SPARKS, OT, 0x32);
    wedges(fx, WEDGES, OT, 0x24);
    blib::sheet::tiles(fx, 0x01E0_2F40, TILES, OT, 8);
    emitters(fx, EMITTERS, 2);
    rings(fx, RINGS, OT, 0x32);
    blib::star::flash_b(fx, 0x01E0_4400, FLASHES, OT, 3);
    blib::drop::drops(fx, DROPS, OT, 0x12C);
    blib::spike::spikes(fx, 0x01E0_57D0, SPIKES, OT, 3);
    let n = fx.mem.u16(D);
    if (0xC3..0xE0).contains(&n) {
        6
    } else if n == 0xE0 {
        2
    } else if n == 0xB9 {
        1
    } else {
        0
    }
}

/// Update 0xC3 [F 0x1E06E54..0x1E0745C]: thirty-two bursts and two
/// hundred drops from the target, three spikes, a wide flash, a shake.
fn finale(fx: &mut Fx, sp: u32, r: &mut Regs) {
    for k in 0..0x20u32 {
        let d = DROPS + 0x44 * k;
        let delay = (rand64(fx, r.s0, 0x01E0_6E60) >> 4) & 0xF;
        set_counter4(fx, d, 0x1C, delay, 0xFF);
        let v = rand64(fx, r.s0, 0x01E0_6E80) >> 4;
        fx.mem.set8(d + 4, (v & 3) << 4);
        fx.mem.write_u8(d + 8, 2);
        fx.mem.write_u8(d + 0xB, 1);
        fx.mem.write_u8(d + 9, 2);
        fx.mem.write_u8(d + 0xA, 5);
        let v = ((rand64(fx, r.s0, 0x01E0_6EB0) >> 4) & 0x7F) + 0x30;
        for o in [0xCu32, 0xD, 0xE] {
            fx.mem.set8(d + o, v);
        }
        let v = ((rand64(fx, r.s0, 0x01E0_6ECC) >> 4) & 0x7F) + 0x30;
        fx.mem.set8(d + 0xF, v);
        fx.mem.write_u32(d + 0x10, 0x4270_0000);
        fx.mem.write_u32(d + 0x14, 0x41F0_0000);
        fx.mem.write_u32(d + 0x1C, 0);
        fx.mem.write_u32(d + 0x18, 0);
        copy3(fx, sp + 0x80, d + 0x20);
        r.s0 = u64::from(sp + 0x84);
        r.s1 = u64::from(sp + 0x88);
        for (o, ra) in [(0x2Cu32, 0x01E0_6F18u32), (0x30, 0x01E0_6F54), (0x34, 0x01E0_6F90)] {
            let f = jit(fx, r.s0, ra, 0x12C, 0x96);
            fx.mem.setf(d + o, f);
        }
        for (o, s) in [(0x38u32, 0x2Cu32), (0x3C, 0x30), (0x40, 0x34)] {
            let v = ee::neg(ee::div(fx.mem.f(d + s), 18.0));
            fx.mem.setf(d + o, v);
        }
    }
    for k in 0..3u32 {
        let s = SPIKES + 0x68 * k;
        set_counter4(fx, s, 0x10, 4 * k as i32, 0xFF);
        fx.mem.write_u8(s + 4, 5);
        fx.mem.write_u32(s + 0x14, 0x4040_0000);
        fx.mem.write_u32(s + 0x1C, 0x420C_0000);
        fx.mem.write_u8(s + 5, 0);
        fx.mem.write_u8(s + 0xA, 0);
        fx.mem.write_u8(s + 0xB, 2);
        fx.mem.write_u8(s + 0xC, 6);
        fx.mem.write_u8(s + 8, 0x5A);
        let v = rand64(fx, r.s0, 0x01E0_70A8);
        fx.mem.set8(s + 9, v >> 4);
        fx.mem.write_u32(s + 0x10, BYTES);
        fx.mem.write_u32(s + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(s + o, 0x80);
        }
        fx.mem.write_u8(s + 0x23, 0x10);
        fx.mem.write_u8(s + 0x27, 0x80);
        copy_v4(fx, sp + 0x80, s + 0x28);
        fx.mem.write_u32(s + 0x58, 0x40);
        fx.mem.write_u32(s + 0x5C, 0x80);
        fx.mem.write_u32(s + 0x60, 0x100);
    }
    for k in 0..0xC8 {
        let i = first_free(fx, DROPS, 0x44, 0x12C);
        r.s4 = sx(i);
        if i < 0 {
            continue;
        }
        r.s4 = u64::from(D + 0x44 * i as u32);
        let count = (((rand64(fx, r.s0, 0x01E0_71D8) >> 4) & 0xF) + 0x20) & 0xFF;
        let (_, h) = fx.div(k, 15);
        let d = DROPS + 0x44 * i as u32;
        set_counter4(fx, d, count, h & 0xFF, 0xFF);
        fx.mem.write_u8(d + 4, 0x42);
        fx.mem.write_u8(d + 8, 0);
        fx.mem.write_u8(d + 0xB, 2);
        fx.mem.write_u8(d + 0xA, 0);
        let v = ((rand64(fx, r.s0, 0x01E0_7230) >> 4) & 3) + 2;
        fx.mem.set8(d + 9, v);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(d + o, 0x80);
        }
        fx.mem.write_u32(d + 0x10, 0x428C_0000);
        fx.mem.write_u32(d + 0x14, 0xBF4C_CCCD);
        fx.mem.write_u32(d + 0x18, 0xBD4C_CCCD);
        let y = fx.mem.read_u32(CENTRE + 4);
        fx.mem.write_u32(d + 0x1C, y);
        copy3(fx, sp + 0x80, d + 0x20);
        let f = jit(fx, r.s0, 0x01E0_72A0, 0x96, 0x4B);
        fx.mem.setf(d + 0x2C, f);
        let f = jit(fx, r.s0, 0x01E0_72DC, 100, 50);
        fx.mem.setf(d + 0x30, ee::add(f32::from_bits(0x3F66_6666), f));
        let f = jit(fx, r.s0, 0x01E0_7324, 0x96, 0x4B);
        fx.mem.setf(d + 0x34, f);
    }
    let f = FLASHES + 0x4C * 2;
    set_counter4(fx, f, 0x20, 0, 0xFF);
    fx.mem.write_u8(f + 5, 0);
    fx.mem.write_u8(f + 4, 0x81);
    fx.mem.write_u8(f + 7, 9);
    for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x40), (9, 0x80), (8, 0x80)] {
        fx.mem.write_u8(f + o, v);
    }
    fx.mem.write_u32(f + 0x14, 0);
    fx.mem.write_u32(f + 0x10, 0x4461_0000);
    copy_v4(fx, sp + 0x80, f + 0x1C);
    set_counter4(fx, SHAKE, 0x20, 0, 0xFF);
    fx.mem.write_u8(SHAKE + 5, 0);
    fx.mem.write_u8(SHAKE + 4, 2);
    fx.mem.write_u32(SHAKE + 0x14, 0x3F99_999A);
    fx.mem.write_u32(SHAKE + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(SHAKE + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(SHAKE + 0x30, 0);
    fx.mem.write_u32(SHAKE + 0x34, 0x42A0_0000);
    fx.mem.write_u32(SHAKE + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x80, SHAKE + 0x20);
}

/// 0x1E062B0(D) [F] (05a45204e54e, b062's 0x1E05AA0 at other offsets with
/// y / z swapped): each counting comet turned about its aim vector (D +
/// 0x8D00 + 12 k, 0x101A00) at radius (count - 4) x D+0x8E68[k] (not below
/// 0), angle 45 k + 16 elapsed degrees (0x20C540: (r cos, 0, r sin)), from
/// the centroid D+0x8EE0; flag 8 on its last 3 counts. Frame 0xD0.
fn steer(fx: &mut Fx, r: &Regs) {
    let sp = fx.enter(0xD0);
    // The entry's s0..s4 (s2 = D, s3 the OT) and ra.
    fx.saved(sp, &[(0x10, r.s0), (0x20, r.s1), (0x30, u64::from(D)), (0x40, u64::from(OT)), (0x50, r.s4), (0x60, 0x01E0_7478)]);
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
        blib::spray::aim_rotation(fx, AIMS + 0xC * k, sp + 0x90, [u64::from(k), u64::from(D + 0x44 * k)], 0x01E0_6338);
        let (total, n) = (fx.mem.u8(c + 2), fx.mem.u8(c));
        let mut f = ee::mul(ee::from_i32(n - 4), fx.mem.f(RADII + 4 * k));
        if ee_lt(f, 0.0) {
            f = 0.0;
        }
        let a = ee::add(ee::mul(45.0, ee::from_i32(k as i32)), ee::mul(16.0, ee::from_i32(total - n)));
        let ang = ee::div(ee::mul(f32::from_bits(PI), a), 180.0);
        let cs = libm_cosf(fx, ang);
        fx.mem.setf(sp + 0x70, ee::mul(f, cs));
        fx.mem.write_u32(sp + 0x74, 0);
        let sn = fx.libm_sinf(ang);
        fx.mem.setf(sp + 0x78, ee::mul(f, sn));
        let m = fx.read_m4(sp + 0x90);
        let v = fx.read_v4(sp + 0x70);
        let t = fx.transform(Some(&m), v);
        fx.write_v4(sp + 0x80, t);
        for e in 0..3u32 {
            let v = ee::add(fx.mem.f(CENTRE + 4 * e), fx.mem.f(sp + 0x80 + 4 * e));
            fx.mem.setf(c + 0x38 + 4 * e, v);
        }
    }
    fx.leave(0xD0);
}

/// The is-zero test of `c.eq.s` with 0 (denormals are 0).
fn is_zero(v: f32) -> bool {
    v.to_bits() & 0x7F80_0000 == 0
}

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v);
    fx.mem.write_u16(a, x as u16);
}

/// 0x1E02940(recs, ot, n) -> 1 [F] (1f569aef40cf, this record's own; a
/// 0x1C0 frame laid out as `drop::drops`'): 0x38-byte falling sparks.
/// Per record: countdown; texture (5, +8); i32 +0xC += +0x10; the length
/// gone masked by +3 (g): when 0, or on the first count, a unit +5 puts
/// +0x20.. on its node (0x101BA0) and two rands scatter x / z (+0x2C /
/// +0x34 = ((rand >> 4) % +0xC - +0xC / 2) / 100, +0x30 = 0); drawn at
/// +0x20 + the scatter, y + g x +0x18; colour +0x1C.. (BGR), alpha +0x1F
/// (unless flag 0x40: x count / 8 over the last 8, else x (+3 - g) / 8
/// below 8); atlas cell +0xA (flag 1: g & 7, 7 - count below 7) of layout
/// +9, blend +0xB; billboard half size 0x1018E0(+0x14, w), y x 1.33, key
/// w (flag 0x80: depth 0xFFFFFE, key 0).
fn sparks(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1C0);
    fx.mem.set32(sp + 0xB4, n);
    fx.mem.write_u32(sp + 0xB8, ot);
    let mut p = recs;
    for i in 0..n {
        if i != 0 {
            p += 0x38;
        }
        spark(fx, sp, p, ot);
    }
    fx.leave(0x1C0);
    1
}

fn spark(fx: &mut Fx, sp: u32, p: u32, ot: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    let id = fx.mem.u8(p + 8);
    let tex = fx.texture(5, id);
    let v = fx.mem.i32(p + 0xC).wrapping_add(fx.mem.i32(p + 0x10));
    fx.mem.set32(p + 0xC, v);
    let g = (fx.mem.u8(p + 2) - fx.mem.u8(p)) & fx.mem.u8(p + 3);
    if g == 0 || fx.mem.u8(p + 2) - 1 == fx.mem.u8(p) {
        if fx.mem.read_u8(p + 5) != 0 {
            node_source(fx, p, sp + 0x110);
            copy3(fx, sp + 0x110, p + 0x20);
        }
        for (o, ra) in [(0x2Cu32, 0x01E0_2A48u32), (0x34, 0x01E0_2A84)] {
            let r = fx.rand_from(sp + 0x158, ra) >> 4;
            let m = fx.mem.i32(p + 0xC);
            let (_, h) = fx.div(r, m);
            let v = ee::div(ee::from_i32(h - (m >> 1)), 100.0);
            fx.mem.setf(p + o, v);
            if o == 0x2C {
                fx.mem.write_u32(p + 0x30, 0);
            }
        }
    }
    let x = ee::add(fx.mem.f(p + 0x20), fx.mem.f(p + 0x2C));
    fx.mem.setf(sp + 0x100, x);
    let z = ee::add(fx.mem.f(p + 0x28), fx.mem.f(p + 0x34));
    fx.mem.setf(sp + 0x108, z);
    let y = ee::add(fx.mem.f(p + 0x24), ee::mul(ee::from_i32(g), fx.mem.f(p + 0x18)));
    fx.mem.setf(sp + 0x104, y);
    camera_vp_into(fx, sp + 0xC0);
    let d = sp + 0x138;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (s, o) in [(0x1Eu32, 0x148u32), (0x1D, 0x149), (0x1C, 0x14A)] {
        let v = fx.mem.read_u8(p + s);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x14C, 0x3F80_0000);
    let cell = if fx.mem.u8(p + 4) & 1 != 0 {
        let c = fx.mem.u8(p);
        if c < 7 {
            7 - c
        } else {
            g & 7
        }
    } else {
        fx.mem.u8(p + 0xA)
    };
    let b = fx.mem.read_u8(p + 0xB);
    fx.mem.write_u8(sp + 0x125, b);
    let layout = fx.mem.u8(p + 9);
    atlas_cell(fx, sp + 0x120, cell, layout);
    tex_rect(fx, d, sp + 0x120, tex);
    let a = fx.mem.read_u8(p + 0x1F);
    fx.mem.write_u8(sp + 0x14B, a);
    if fx.mem.u8(p + 4) & 0x40 == 0 {
        let c = fx.mem.u8(p);
        if c < 8 {
            let a = fx.mem.u8(p + 0x1F) >> 3;
            let v = fx.mult(a, c);
            fx.mem.set8(sp + 0x14B, v);
        } else {
            let left = fx.mem.u8(p + 3) - g;
            if left < 8 {
                let a = fx.mem.u8(p + 0x1F) >> 3;
            let v = fx.mult(a, left);
                fx.mem.set8(sp + 0x14B, v);
            }
        }
    }
    let m = fx.read_m4(sp + 0xC0);
    if !fx.project_at(sp + 0x100, &m, sp + 0x1B0, sp + 0x1BC) || !Fx::on_screen(fx.mem.read_u64(sp + 0x1B0)) {
        return;
    }
    let size = fx.mem.f(p + 0x14);
    let w = fx.mem.f(sp + 0x1BC);
    let half = fx.view_scale(size, w);
    let depth = if fx.mem.u8(p + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x188, 0xFF_FFFE);
        0.0
    } else {
        let z = fx.xyz_z(sp + 0x1B0);
        fx.set_xyz_z(sp + 0x188, z);
        fx.mem.f(sp + 0x1BC)
    };
    for (from, to) in [(0x188u32, 0x178u32), (0x178, 0x168), (0x168, 0x158)] {
        let z = fx.xyz_z(sp + from);
        fx.set_xyz_z(sp + to, z);
    }
    let x = ee::from_i32(fx.mem.u16(sp + 0x1B0));
    let v = ee::sub(x, half);
    for a in [0x178u32, 0x158] {
        fix16(fx, sp + a, v);
    }
    let v = ee::add(x, half);
    for a in [0x188u32, 0x168] {
        fix16(fx, sp + a, v);
    }
    let y = ee::from_i32(fx.mem.u16(sp + 0x1B2));
    let hy = ee::mul(f32::from_bits(ASPECT), half);
    let v = ee::sub(y, hy);
    for a in [0x16Au32, 0x15A] {
        fix16(fx, sp + a, v);
    }
    let v = ee::add(y, hy);
    for a in [0x18Au32, 0x17A] {
        fix16(fx, sp + a, v);
    }
    let key = ee::fixunssfsi(depth) as i32;
    fx.submit(sp + 0x138, ot, key);
}

/// 0x1E03680(recs, ot, n) -> 1 [F] (b6cc0afca09c, this record's own; a
/// 0x210 frame): 0x44-byte sprite rings. Per record: countdown; texture
/// (5, +0xA); size +0x1C += +0x20; at +0x38 (a unit +5: its node,
/// 0x101BA0) turned by +0x28 (0x20C0B0 / 0x20B370 into the camera's);
/// colour +0x24.. (BGR), alpha +0x27 (x count / 8 over the last 8 unless
/// flag 0x40); atlas cell +0xC of layout +0xB, blend +0xD; +8 sprites:
/// the first at the centre, the others at angle 360 b / 255 (b the next
/// byte of the table *(+0x10) from +9), radius +0x18, height +0x14 (gone
/// + (b' & 3)) b' / 255; each a billboard of half size 0x1018E0(+0x1C, w)
/// (y x 1.33) with the corner order of the next byte's bits 0x30, key w
/// (flag 0x80: depth 0xFFFFFE, key 0).
fn rings(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x210);
    fx.mem.set32(sp + 0xC4, n);
    fx.mem.write_u32(sp + 0xC8, ot);
    fx.mem.write_u32(sp + 0xB0, 0);
    let mut p = recs;
    for i in 0..n {
        if i != 0 {
            p += 0x44;
        }
        ring(fx, sp, p, ot);
        fx.mem.set32(sp + 0xB0, i + 1);
    }
    fx.leave(0x210);
    1
}

fn ring(fx: &mut Fx, sp: u32, p: u32, ot: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    let id = fx.mem.u8(p + 0xA);
    let tex = fx.texture(5, id);
    let v = ee::add(fx.mem.f(p + 0x1C), fx.mem.f(p + 0x20));
    fx.mem.setf(p + 0x1C, v);
    copy3(fx, p + 0x38, sp + 0x160);
    node_source(fx, p, sp + 0x160);
    camera_vp_into(fx, sp + 0x110);
    rot_trans_into(fx, sp + 0xD0, p + 0x28, sp + 0x160);
    mul_mem(fx, sp + 0x110, sp + 0xD0);
    let mut s1 = fx.mem.u8(p + 9);
    let gone = fx.mem.u8(p + 2) - fx.mem.u8(p);
    let d = sp + 0x188;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (s, o) in [(0x26u32, 0x198u32), (0x25, 0x199), (0x24, 0x19A)] {
        let v = fx.mem.read_u8(p + s);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x19C, 0x3F80_0000);
    let b = fx.mem.read_u8(p + 0xD);
    fx.mem.write_u8(sp + 0x175, b);
    let (cell, layout) = (fx.mem.u8(p + 0xC), fx.mem.u8(p + 0xB));
    atlas_cell(fx, sp + 0x170, cell, layout);
    tex_rect(fx, d, sp + 0x170, tex);
    let c = fx.mem.u8(p);
    if c < 8 && fx.mem.u8(p + 4) & 0x40 == 0 {
        let a = fx.mem.u8(p + 0x27) >> 3;
            let v = fx.mult(a, c);
        fx.mem.set8(sp + 0x19B, v);
    } else {
        let a = fx.mem.read_u8(p + 0x27);
        fx.mem.write_u8(sp + 0x19B, a);
    }
    let mut s7 = 0;
    while s7 < fx.mem.u8(p + 8) {
        if s7 == 0 {
            fx.mem.write_u32(sp + 0x158, 0);
            fx.mem.write_u32(sp + 0x154, 0);
            fx.mem.write_u32(sp + 0x150, 0);
        } else {
            let t = fx.mem.read_u32(p + 0x10);
            let b = fx.mem.u8(t.wrapping_add((s1 & 0xFF) as u32));
            let f = ee::div(ee::mul(360.0, ee::from_i32(b)), 255.0);
            let ang = ee::div(ee::mul(f32::from_bits(PI), f), 180.0);
            let cs = libm_cosf(fx, ang);
            let v = ee::mul(fx.mem.f(p + 0x18), cs);
    fx.mem.setf(sp + 0x150, v);
            let sn = fx.libm_sinf(ang);
            let v = ee::mul(fx.mem.f(p + 0x18), sn);
    fx.mem.setf(sp + 0x158, v);
            let t = fx.mem.read_u32(p + 0x10);
            let b = fx.mem.u8(t.wrapping_add(((s1 + 1) & 0xFF) as u32));
            let h = ee::mul(fx.mem.f(p + 0x14), ee::from_i32(gone + (b & 3)));
            let h = ee::mul(h, ee::from_i32(b));
            fx.mem.setf(sp + 0x154, ee::div(h, 255.0));
            s1 = (s1 + 3) & 0xFF;
        }
        sprite(fx, sp, p, ot, &mut s1);
        s7 += 1;
    }
}

fn sprite(fx: &mut Fx, sp: u32, p: u32, ot: u32, s1: &mut i32) {
    let m = fx.read_m4(sp + 0x110);
    if !fx.project_at(sp + 0x150, &m, sp + 0x200, sp + 0x20C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x200)) {
        return;
    }
    let size = fx.mem.f(p + 0x1C);
    let w = fx.mem.f(sp + 0x20C);
    let half = fx.view_scale(size, w);
    let t = fx.mem.read_u32(p + 0x10);
    let o = fx.mem.u8(t.wrapping_add((*s1 & 0xFF) as u32)) & 0x30;
    *s1 = (*s1 + 1) & 0xFF;
    // (x - h, x + h, y - hy, y + hy) targets by the corner order.
    let (xl, xr, yl, yh): ([u32; 2], [u32; 2], [u32; 2], [u32; 2]) = match o {
        0x10 => ([0x1B8, 0x1A8], [0x1D8, 0x1C8], [0x1DA, 0x1BA], [0x1CA, 0x1AA]),
        0x20 => ([0x1D8, 0x1B8], [0x1C8, 0x1A8], [0x1DA, 0x1CA], [0x1BA, 0x1AA]),
        0x30 => ([0x1D8, 0x1C8], [0x1B8, 0x1A8], [0x1CA, 0x1AA], [0x1DA, 0x1BA]),
        _ => ([0x1C8, 0x1A8], [0x1D8, 0x1B8], [0x1BA, 0x1AA], [0x1DA, 0x1CA]),
    };
    let x = ee::from_i32(fx.mem.u16(sp + 0x200));
    for (o, v) in [(xl, ee::sub(x, half)), (xr, ee::add(x, half))] {
        for a in o {
            fix16(fx, sp + a, v);
        }
    }
    let y = ee::from_i32(fx.mem.u16(sp + 0x202));
    let hy = ee::mul(f32::from_bits(ASPECT), half);
    for (o, v) in [(yl, ee::sub(y, hy)), (yh, ee::add(y, hy))] {
        for a in o {
            fix16(fx, sp + a, v);
        }
    }
    if fx.mem.u8(p + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x1D8, 0xFF_FFFE);
        fx.mem.write_u32(sp + 0x20C, 0);
    } else {
        let z = fx.xyz_z(sp + 0x200);
        fx.set_xyz_z(sp + 0x1D8, z);
    }
    for (from, to) in [(0x1D8u32, 0x1C8u32), (0x1C8, 0x1B8), (0x1B8, 0x1A8)] {
        let z = fx.xyz_z(sp + from);
        fx.set_xyz_z(sp + to, z);
    }
    for o in [0x1DFu32, 0x1CF, 0x1BF, 0x1AF] {
        fx.mem.write_u8(sp + o, 0);
    }
    let key = ee::fixunssfsi(fx.mem.f(sp + 0x20C)) as i32;
    fx.submit(sp + 0x188, ot, key);
}

/// 0x1E04070(g, _, n) -> 1 [F] (e52533f9ed98, this record's own; a 0x2C0
/// frame): `n` 0x5C-byte emitters of sprite-ring records. Per emitter:
/// countdown, nothing on its first count; the position +0x2C from a node
/// with +5 (0x101BA0); +0x17 below 2: one ring at it, else +0x17 rings
/// along the curve through +0x2C, +0x3C, +0x4C (0x102550 on the path at
/// sp + 0x80, samples from sp + 0xE4). Each ring a free record of the pool
/// *(+8) (+0x10 of them, 0x101C30) counted +0x11 (0x101AD0): +0x14 = +0x18
/// unless 0, +0x18 = +0x1C, flags +4, +5 = 0, sprites +8 = +0x12, size /
/// growth +0x1C / +0x20 = +0x20 / +0x24, table start +9 = rand >> 4, +0xA..
/// +0xD = +0x13..+0x16, table +0x10 = +0xC, colour +0x24 = +0x28, position
/// +0x38 = the sample, angles +0x28 of (0, 1, 0) (0x101940). Then the
/// history +0x4C = +0x3C, +0x3C = +0x2C.
fn emitters(fx: &mut Fx, recs: u32, n: i32) -> i32 {
    let sp = fx.enter(0x2C0);
    let mut g = recs;
    for i in 0..n {
        if i != 0 {
            g += 0x5C;
        }
        emitter(fx, sp, g);
    }
    fx.leave(0x2C0);
    1
}

fn emitter(fx: &mut Fx, sp: u32, g: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    if fx.mem.u8(g) == fx.mem.u8(g + 2) - 1 {
        return;
    }
    node_source(fx, g, g + 0x2C);
    let segs = if fx.mem.u8(g + 0x17) < 2 {
        copy_v4(fx, g + 0x2C, sp + 0xE4);
        copy_v4(fx, g + 0x3C, sp + 0xF4);
        1
    } else {
        let p = sp + 0x80;
        fx.mem.set32(p, 3);
        for (k, src) in [0x2Cu32, 0x3C, 0x4C].into_iter().enumerate() {
            for c in 0..3u32 {
                let w = fx.mem.read_u32(g + src + 4 * c);
                fx.mem.write_u32(p + 4 + 4 * k as u32 + 0x20 * c, w);
            }
        }
        resample(fx, p);
        fx.mem.u8(g + 0x17)
    };
    for k in 0..segs.max(0) as u32 {
        let pool = fx.mem.read_u32(g + 8);
        let count = fx.mem.u8(g + 0x10);
        let i = first_free(fx, pool, 0x44, count);
        if i < 0 {
            continue;
        }
        let off = 0x44 * i as u32;
        let rec = |fx: &mut Fx| fx.mem.read_u32(g + 8).wrapping_add(off);
        let b = rec(fx);
        let c = fx.mem.u8(g + 0x11);
        set_counter4(fx, b, c, 0, 0xFF);
        fx.mem.write_u32(sp + 0x2B8, 0);
        fx.mem.write_u32(sp + 0x2B0, 0);
        fx.mem.write_u32(sp + 0x2B4, 0x3F80_0000);
        let s = fx.mem.f(g + 0x18);
        if !is_zero(s) {
            let b = rec(fx);
            fx.mem.setf(b + 0x14, s);
        }
        let w = fx.mem.read_u32(g + 0x1C);
        let b = rec(fx);
        fx.mem.write_u32(b + 0x18, w);
        let v = fx.mem.read_u8(g + 4);
        fx.mem.write_u8(b + 4, v);
        fx.mem.write_u8(b + 5, 0);
        let v = fx.mem.read_u8(g + 0x12);
        fx.mem.write_u8(b + 8, v);
        for (dst, src) in [(0x1Cu32, 0x20u32), (0x20, 0x24)] {
            let w = fx.mem.read_u32(g + src);
            fx.mem.write_u32(b + dst, w);
        }
        let r = fx.rand_from(g, 0x01E0_4298);
        let b = rec(fx);
        fx.mem.set8(b + 9, r >> 4);
        for (dst, src) in [(0xAu32, 0x13u32), (0xB, 0x14), (0xC, 0x15), (0xD, 0x16)] {
            let v = fx.mem.read_u8(g + src);
            fx.mem.write_u8(b + dst, v);
        }
        let w = fx.mem.read_u32(g + 0xC);
        fx.mem.write_u32(b + 0x10, w);
        for k in 0..4u32 {
            let v = fx.mem.read_u8(g + 0x28 + k);
            fx.mem.write_u8(b + 0x24 + k, v);
        }
        copy3(fx, sp + 0xE4 + 0x10 * k, b + 0x38);
        aim_angles(fx, sp + 0x2B0, b + 0x28);
    }
    copy_v4(fx, g + 0x3C, g + 0x4C);
    copy_v4(fx, g + 0x2C, g + 0x3C);
}

/// 0x1E020F0(recs, ot, n) -> 1 [F] (97ab8d570f29, this record's own; a
/// 0x240 frame): 0x34-byte wedges sampling the frame. Per record:
/// countdown; radius +0xC += +0x10, angle +0x14 += +0x18; about +0x28 (a
/// unit +5: its node, 0x101BA0; 0x20C0B0 / 0x20B370 into the camera's)
/// the points at the radius and angles +0x14 / +0x14 + +0x1C degrees
/// (libm cosf / sinf, y 0) projected (0x204D80, w at sp + 0x230 / 0x234);
/// drawn when the first's x is not left of the second's: a four-vertex
/// strip (0x207EF0, filter 1) from the two points (y capped below 36864
/// to 0x9000) down to y 28672 - 16 (+8 + (count & 3)), ST the screen
/// position less 28672 (T 0 on the top edge), colours +0x24.. (bottom,
/// alpha +0x27) / +0x20.. (top, alpha +0x23; both x count / 8 over the
/// last 8), blend +9 (0x101C80 / 0x207E20), key the mean w.
fn wedges(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x240);
    fx.mem.set32(sp + 0xB4, n);
    fx.mem.write_u32(sp + 0xB8, ot);
    let mut p = recs;
    for i in 0..n {
        if i != 0 {
            p += 0x34;
        }
        wedge(fx, sp, p, ot);
    }
    fx.leave(0x240);
    1
}

fn wedge(fx: &mut Fx, sp: u32, p: u32, ot: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    let v = ee::add(fx.mem.f(p + 0xC), fx.mem.f(p + 0x10));
    fx.mem.setf(p + 0xC, v);
    let v = ee::add(fx.mem.f(p + 0x14), fx.mem.f(p + 0x18));
    fx.mem.setf(p + 0x14, v);
    copy3(fx, p + 0x28, sp + 0x190);
    if fx.mem.read_u8(p + 5) != 0 {
        node_source(fx, p, sp + 0x190);
    }
    camera_vp_into(fx, sp + 0xC0);
    translation_into(fx, sp + 0x100, sp + 0x190);
    mul_mem(fx, sp + 0xC0, sp + 0x100);
    let pi = f32::from_bits(PI);
    let a = ee::div(ee::mul(pi, fx.mem.f(p + 0x14)), 180.0);
    let cs = libm_cosf(fx, a);
    let v = ee::mul(fx.mem.f(p + 0xC), cs);
    fx.mem.setf(sp + 0x170, v);
    fx.mem.write_u32(sp + 0x174, 0);
    let sn = fx.libm_sinf(a);
    let v = ee::mul(fx.mem.f(p + 0xC), sn);
    fx.mem.setf(sp + 0x178, v);
    let a = ee::div(ee::mul(pi, ee::add(fx.mem.f(p + 0x14), fx.mem.f(p + 0x1C))), 180.0);
    let cs = libm_cosf(fx, a);
    let v = ee::mul(fx.mem.f(p + 0xC), cs);
    fx.mem.setf(sp + 0x180, v);
    fx.mem.write_u32(sp + 0x184, 0);
    let sn = fx.libm_sinf(a);
    let v = ee::mul(fx.mem.f(p + 0xC), sn);
    fx.mem.setf(sp + 0x188, v);
    let m = fx.read_m4(sp + 0xC0);
    if !fx.project_at(sp + 0x170, &m, sp + 0x160, sp + 0x230) {
        return;
    }
    if !fx.project_at(sp + 0x180, &m, sp + 0x168, sp + 0x234) {
        return;
    }
    if fx.mem.u16(sp + 0x160) < fx.mem.u16(sp + 0x168) {
        return;
    }
    for a in [0x162u32, 0x16A] {
        if !ee_lt(ee::from_i32(fx.mem.u16(sp + a)), 36864.0) {
            fx.mem.write_u16(sp + a, 0x9000);
        }
    }
    let d = sp + 0x1A0;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    // a2 = sp - 0x10 as 0x204D80 leaves it, a3 its w pointer [run].
    fx.frame_desc(d, 1, [sp - 0x10, sp + 0x234]);
    for (s, o) in [(0x26u32, 0u32), (0x25, 1), (0x24, 2)] {
        let v = fx.mem.read_u8(p + s);
        fx.mem.write_u8(sp + 0x1C8 + o, v);
        fx.mem.write_u8(sp + 0x1B0 + o, v);
        let v = fx.mem.read_u8(p + s - 4);
        fx.mem.write_u8(sp + 0x1F8 + o, v);
        fx.mem.write_u8(sp + 0x1E0 + o, v);
    }
    let c = fx.mem.u8(p);
    if c < 8 {
        let a = fx.mem.u8(p + 0x27) >> 3;
            let v = fx.mult(a, c);
        fx.mem.set8(sp + 0x1CB, v);
        fx.mem.set8(sp + 0x1B3, v);
        // mult1: LO1, not LO.
        let v = (fx.mem.u8(p + 0x23) >> 3).wrapping_mul(fx.mem.u8(p));
        fx.mem.set8(sp + 0x1FB, v);
        fx.mem.set8(sp + 0x1E3, v);
    } else {
        let v = fx.mem.read_u8(p + 0x27);
        fx.mem.write_u8(sp + 0x1CB, v);
        fx.mem.write_u8(sp + 0x1B3, v);
        let v = fx.mem.read_u8(p + 0x23);
        fx.mem.write_u8(sp + 0x1FB, v);
        fx.mem.write_u8(sp + 0x1E3, v);
    }
    for o in [0x1FCu32, 0x1E4, 0x1CC, 0x1B4] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    for o in [0x20Fu32, 0x1F7, 0x1DF, 0x1C7] {
        fx.mem.write_u8(sp + o, 0);
    }
    let b = fx.mem.read_u8(p + 9);
    fx.mem.write_u8(sp + 0x145, b);
    alpha_mode(fx, sp + 0x140, sp + 0x23C);
    let s = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x23C + k));
    let alpha = s(fx, 0) | (s(fx, 1) << 2) | (s(fx, 2) << 4) | (s(fx, 3) << 6);
    fx.desc_alpha(d, alpha);
    let f = fx.mem.read_u16(d + 2) | 0x800;
    fx.mem.write_u16(d + 2, f);
    let st = |fx: &mut Fx, a: u32, v: u32| {
        let h = fx.mem.read_u16(sp + a);
        fx.mem.write_u16(sp + a, (h & 0xC000) | (v & 0x3FFF) as u16);
    };
    let org = 28672.0;
    let x0 = ee::sub(ee::from_i32(fx.mem.u16(sp + 0x160)), org);
    let x1 = ee::sub(ee::from_i32(fx.mem.u16(sp + 0x168)), org);
    st(fx, 0x1B8, ee::fixunssfsi(x0));
    st(fx, 0x1BA, 0);
    st(fx, 0x1D0, ee::fixunssfsi(x1));
    st(fx, 0x1D2, 0);
    let y0 = ee::sub(ee::from_i32(fx.mem.u16(sp + 0x162)), org);
    st(fx, 0x1E8, ee::fixunssfsi(x0));
    st(fx, 0x1EA, ee::fixunssfsi(y0));
    let y1 = ee::sub(ee::from_i32(fx.mem.u16(sp + 0x16A)), org);
    st(fx, 0x200, ee::fixunssfsi(x1));
    st(fx, 0x202, ee::fixunssfsi(y1));
    let z = fx.xyz_z(sp + 0x160);
    fx.set_xyz_z(sp + 0x1F0, z);
    let z = fx.xyz_z(sp + 0x1F0);
    fx.set_xyz_z(sp + 0x1C0, z);
    let h = (fx.mem.u8(p + 8) + (fx.mem.u8(p) & 3)) << 4;
    let top = ee::sub(org, ee::from_i32(h));
    let z = fx.xyz_z(sp + 0x168);
    fx.set_xyz_z(sp + 0x208, z);
    let z = fx.xyz_z(sp + 0x208);
    fx.set_xyz_z(sp + 0x1D8, z);
    let x = fx.mem.read_u16(sp + 0x160);
    fx.mem.write_u16(sp + 0x1F0, x);
    fx.mem.write_u16(sp + 0x1C0, x);
    let x = fx.mem.read_u16(sp + 0x168);
    fx.mem.write_u16(sp + 0x208, x);
    fx.mem.write_u16(sp + 0x1D8, x);
    let y = ee::fixunssfsi(top) as u16;
    fx.mem.write_u16(sp + 0x1DA, y);
    fx.mem.write_u16(sp + 0x1C2, y);
    let y = fx.mem.read_u16(sp + 0x162);
    fx.mem.write_u16(sp + 0x1F2, y);
    let y = fx.mem.read_u16(sp + 0x16A);
    fx.mem.write_u16(sp + 0x20A, y);
    let k = ee::fixunssfsi(ee::add(fx.mem.f(sp + 0x230), fx.mem.f(sp + 0x234)));
    let key = ee::fixunssfsi(ee::div(cvt_u32(k), 2.0)) as i32;
    fx.submit(d, ot, key);
}
