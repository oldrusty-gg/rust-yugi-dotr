//! BMODULE b396 (card 410), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01F00, data area 0x1ED0000) [F]: b631's pool of 400 tasks (0x70 bytes at +0x2A0, alive +0x58,
//! step +0x44 as a record address; spawn a6cf8ad55488 at 0x1E025D0 =
//! `blib::spawn::task400`) drawn with the second library: quads
//! (`billboard::quad`, 0x1E00ED0), rings (`billboard::ring_quads`,
//! 0x1E00920, a random orientation every draw) and spark fans
//! (`quads4::sparks`, 0x1E01190), UVs by uv_rect 0x1E00080. From update
//! 34 to 78 two white quads a update leave the point halfway between the
//! attacker's nodes 7 and 0xF (5 lower), drifting up and toward the
//! facing; from 73 to 78 ten rings and two fans (red / pink-white) start
//! at the target. Returns 1 on update 63, 4 on 115, 2 on 93, 6 on 74..87,
//! 8 once the 120-update timer +0xB210 has run out.
use crate::blib::{billboard, quads4, spawn::task400, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
/// The step functions a task holds (+0x44), as the record's addresses.
const QUAD: u32 = 0x01E0_1790;
const RING: u32 = 0x01E0_1990;
const FAN_A: u32 = 0x01E0_1B40;
const FAN_B: u32 = 0x01E0_1CF0;
const IDLE: u32 = 0x01E0_1EF0;
/// The return address of the entry's `jalr` into a step (0x1E02208 + 8).
const TASK_RA: u64 = 0x01E0_2210;
const TIMER: u32 = D + 0xB210;
const COUNTER: u32 = D + 0xB2B8;
const BYTES: u32 = D + 0xB2BA;
const SINES: u32 = D + 0xB4BC;

/// The entry 0x1E01F00.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(D + 0xB21C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB630);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB2B6, param);
    fx.mem.set32(TIMER, 0x78);
    for (at, node, clear) in [(0xB244u32, 7, 1), (0xB246, 0xF, 0), (0xB248, 0x35, 0), (0xB24A, 0x30, 0), (0xB24C, 0x2F, 0), (0xB24E, 0x2C, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB254, r);
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k, t);
    }
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xB1F0 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.mem.set16(D + 0xB2B4, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand();
        fx.mem.set16(D + 0xB264 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, sp, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 || f == IDLE {
            continue;
        }
        let t = slot + 0x2A0;
        match f {
            QUAD => quad(fx, t),
            RING => ring(fx, t),
            FAN_A => fan(fx, t, false),
            FAN_B => fan(fx, t, true),
            _ => panic!("b396: unknown task step {f:#x}"),
        }
    }
    let mut alive = 0u32;
    for k in 0..TASKS {
        let s = D + 0x70 * k;
        if fx.mem.i16(s + 0x2F8) == 0 {
            continue;
        }
        alive += 1;
        let t = s + 0x2A0;
        if fx.mem.read_u32(t + 0x44) == QUAD {
            let w = fx.mem.read_u32(D + 0xB1F0);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x50, 0x14);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            let w = fx.mem.read_u32(D + 0xB1F4);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x60, 0x14);
            fx.mem.set16(D + 0xB8, 4);
            // rand from the entry (its s0: the live tasks so far).
            let v = fx.rand_from(alive, 0x01E0_2330);
            fx.mem.set16(D + 0xBE, v % 4);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (f, local) in [(FAN_A, 0x70u32), (FAN_B, 0x80)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            cell(fx, t, sp + local, 0x97);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            let w = fx.mem.read_u32(D + 0xB1F8);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0x94, 0);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x20);
            fx.mem.set16(D + 0x9A, 0x40);
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 0xC);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x3F => 1,
        0x73 => 4,
        0x5D => 2,
        0x4A..=0x57 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / +0x56 (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// 0x1E01590(D, 0): the update counter and the spawns (frame 0x50: the
/// entry's s1 = D, s0 = 0); the spawns' saves: the entry's s4 / s3 = 0,
/// s2 = the parameter, the emitter's s1, s0 = D.
fn emit(fx: &mut Fx, _entry_sp: u32, param: i32) {
    let sp = fx.enter(0x50);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x20, 0x01E0_21D0), (0x10, u64::from(D)), (0, 0)]);
    let go = |fx: &mut Fx, f: u32, ra: u32, s1: u64| {
        task400(fx, D, f, 0, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, s1), (0, u64::from(D))]);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = fx.mem.i16(COUNTER);
    if (0x22..0x4F).contains(&c) {
        let h = fx.self_handle();
        let s = fx.mem.i16(D + 0xB244);
        fx.node_position(h, s, D + 0x250);
        let h = fx.self_handle();
        let s = fx.mem.i16(D + 0xB246);
        fx.node_position(h, s, sp + 0x30);
        for k in 0..3 {
            let half = ee::div(ee::sub(fx.mem.f(sp + 0x30 + 4 * k), fx.mem.f(D + 0x250 + 4 * k)), 2.0);
            fx.mem.setf(sp + 0x30 + 4 * k, half);
        }
        for k in 0..3 {
            let v = ee::add(fx.mem.f(D + 0x250 + 4 * k), fx.mem.f(sp + 0x30 + 4 * k));
            fx.mem.setf(D + 0x250 + 4 * k, v);
        }
        let y = ee::sub(fx.mem.f(D + 0x254), 5.0);
        fx.mem.setf(D + 0x254, y);
        go(fx, QUAD, 0x01E0_16CC, u64::from(D));
        go(fx, QUAD, 0x01E0_16E0, u64::from(D));
    }
    let c = fx.mem.i16(COUNTER);
    if (0x49..0x4F).contains(&c) {
        let h = fx.target_handle();
        fx.unit_position(h, D + 0x250);
        for k in 0..0xAu64 {
            go(fx, RING, 0x01E0_1734, k);
        }
        go(fx, FAN_A, 0x01E0_1768, 0xA);
        go(fx, FAN_B, 0x01E0_177C, 0xA);
    }
    fx.leave(0x50);
}

/// A step's prologue: frame 0x20 (ra, the entry's s0 = 0) or 0x30 (ra,
/// the entry's s1 = D, s0 = 0).
fn prologue(fx: &mut Fx, bytes: u32) {
    let sp = fx.enter(bytes);
    if bytes == 0x20 {
        fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    } else {
        fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    }
}

/// rand % m (HI) from a step's `jal` at `site` (its s0 = the task).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// 0x1E01790: a white quad (frame 0x30) of size 100 drifting 0.6 a
/// update along a random angle, up by +0x34 (0..1.27) and 2 toward the
/// facing; in by 0x30 for 6 updates, then out by 19.
fn quad(fx: &mut Fx, t: u32) {
    prologue(fx, 0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        fx.mem.set16(t + 0xA, 0x64);
        let v = fx.rand_from(t, 0x01E0_17C8);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_1800);
        let (_, h) = fx.div(v >> 4, 0x168);
        fx.mem.set16(t + 0x5A, h);
        let v = rmod(fx, t, 0x01E0_1818, 0x14);
        fx.mem.setf(t + 0x34, ee::from_i32(v));
        let s = ee::div(fx.mem.f(t + 0x34), 15.0);
        fx.mem.setf(t + 0x34, s);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 6 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x30);
        }
    } else if a >= 0x15 {
        fx.mem.write_u8(t + 0x63, a - 0x13);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let by = f32::from_bits(0x3F19_999A);
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let z = ee::add(fx.mem.f(t + 0x18), ee::mul(by, s));
        fx.mem.setf(t + 0x18, z);
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, SINES, a, 1);
        let x = ee::add(fx.mem.f(t + 0x10), ee::mul(by, c));
        fx.mem.setf(t + 0x10, x);
        let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
        fx.mem.setf(t + 0x14, y);
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(D + 0xB2B4) != 0 { ee::add(z, 2.0) } else { ee::sub(z, 2.0) };
        fx.mem.setf(t + 0x18, z);
    }
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, 0x01E0_196C), (0x10, u64::from(D)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E01990: a grey ring (frame 0x20): quad size 25..44, radius 10..49
/// growing by 26..55, life +6 3..5; in by 0x40 for its life, then out by
/// 14.
fn ring(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_19AC, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x19));
        let v = rmod(fx, t, 0x01E0_19D8, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_1A04, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_1A24, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_1A40, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x1A);
        let v = rmod(fx, t, 0x01E0_1A60, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1A7C, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01B40 (turned 0x4000 about x, spark span 60) / 0x1E01CF0 (`b`: a
/// random turn, span 2), frame 0x20: a spark fan of radii 200 / 142
/// growing by +0x34 (15..22.9), the inner 0.1 of the outer; kind +0x50
/// ((rand >> 4) % 2): red or pink-white; full alpha for 8 updates, then
/// out by 5.
fn fan(fx: &mut Fx, t: u32, b: bool) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        let v = fx.rand_from(t, if b { 0x01E0_1D30 } else { 0x01E0_1B80 });
        fx.mem.set16(t + 0x50, (v >> 4) % 2);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xC0, 0x20, 0x20);
            l2::rgb(fx, t + 0x64, 0, 0, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xA0, 0xA0);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        }
        fx.mem.set16(t + 0x5A, 0x3C);
        fx.mem.set16(t + 0x5C, if b { 2 } else { 0x3C });
        let v = rmod(fx, t, if b { 0x01E0_1DC4 } else { 0x01E0_1C10 }, 0x50);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0x96));
        let s = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, s);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        if b {
            for (o, site) in [(0x20u32, 0x01E0_1E0Cu32), (0x24, 0x01E0_1E28), (0x20, 0x01E0_1E44)] {
                let v = rmod(fx, t, site, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
        } else {
            fx.mem.write_u32(t + 0x20, 0x4000);
        }
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 7 {
            fx.mem.write_u8(t + 0x63, a - 5);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3DCC_CCCD), r));
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}
