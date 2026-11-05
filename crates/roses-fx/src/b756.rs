//! BMODULE b756 (card 152), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E043B0, data area a0 = 0x1ED0000, 0xB0D0 bytes) [F]
//!: b771's task family on a pool of 150 tasks of
//! 0x70 at D+0x2A0 (alive +0x58, step +0x44 through `jalr`), spawned by
//! 0x1E05060 (colour D+0xA904, +0x50 from D+0xAD56). The emitter 0x1E01920
//! counts D+0xAD5E: on 1 the stage light 0x4260, on 5 the object op 0x4330;
//! on 0x5A a petal crown 0x3670 and a quad cross 0x2CE0 at node 1; on 0x90
//! / 0xA3 band rings 0x3F20 / 0x40D0, on 0x9E six rings 0x21C0; odd 0xB0..
//! 0x17C spiral ribbons 0x2360 (two 20-point histories D+0x4470 / D+0x45B0
//! + 0x280 k); odd 0xBE..0x1B7 beam pairs 0x34F0 between nodes 0x2E / 0x2F
//! (D+0x20 / D+0x30 closing by a twelfth / thirtieth of their gap); from
//! 0x10E a band 0x2900 and quads 0x2AD0 at the point D+0xA870 (node 0x2F
//! until 0x18A, then sliding by 5.5 until 0x1A8); 0xFA..0x186 rings 0x3170
//! / 0x2E30 from node 0x2E towards 0x2F; 0x12D..0x1A7 rings 0x3BA0; on
//! 0x1A8 / 0x1B2 / 0x1BC a burst (0x3880, 12 petals 0x39B0, 8 rings
//! 0x3BA0). Step 0x1E03D40 is dispatched but never spawned [F: no spawn
//! names it]; not ported. Draws: `quads4::ring_facing_flat_z` (0x13D0),
//! `billboard::ring_quads` (0xE20), a private ring band 0x9B0 (b219's with
//! the outer points at height +0x3C), petals 0x51F0 (b721's limited by
//! D+0xAD50), b771's ribbon 0x5530, beams 0x5DF0, the cross 0x61D0. Returns
//! 1 / 3 / 6 / 8 on 0x19E / 0x1DB / 0x1A9..0x1D5 / timer out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::c061::blend_alpha;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 150;
const POS: u32 = D + 0x250;
const TEX_A: u32 = D + 0x260;
const TEX_B: u32 = D + 0x29C;
const POINT: u32 = D + 0xA870;
const TEX5: u32 = D + 0xA8E0;
const TIMER: u32 = D + 0xA900;
const COLOUR: u32 = D + 0xA904;
const OT_AT: u32 = D + 0xA90C;
const N1: u32 = D + 0xACEC;
const N2E: u32 = D + 0xACEE;
const N2F: u32 = D + 0xACF0;
const CELLS: u32 = D + 0xAD0C;
const LIMIT: u32 = D + 0xAD50;
const INDEX: u32 = D + 0xAD56;
const NEXT: u32 = D + 0xAD58;
const FACE: u32 = D + 0xAD5A;
const COUNTER: u32 = D + 0xAD5E;
const BYTES: u32 = D + 0xAD60;
const BYTES2: u32 = D + 0xAE60;
const SINES: u32 = D + 0xAF60;

const RING: u32 = 0x01E0_21C0;
const SPIRAL: u32 = 0x01E0_2360;
const BAND: u32 = 0x01E0_2900;
const QUADS: u32 = 0x01E0_2AD0;
const CROSS: u32 = 0x01E0_2CE0;
const RING_G: u32 = 0x01E0_2E30;
const QUADS_G: u32 = 0x01E0_3170;
const BEAM: u32 = 0x01E0_34F0;
const CROWN: u32 = 0x01E0_3670;
const BURST: u32 = 0x01E0_3880;
const PETAL: u32 = 0x01E0_39B0;
const RING_S: u32 = 0x01E0_3BA0;
const BAND_A: u32 = 0x01E0_3F20;
const BAND_B: u32 = 0x01E0_40D0;
const LIGHT: u32 = 0x01E0_4260;
const OBJECT: u32 = 0x01E0_4330;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// rand % m (HI) from return address `ra` (rand's frame: `s0`).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn odd(v: i32) -> bool {
    v % 2 == 1
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The entry 0x1E043B0 (a0 = D, a1 = param; frame 0x130).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x130);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x130);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB0D0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xAD5C, param);
    fx.mem.set32(TIMER, 0x1DE);
    let h = fx.self_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(N1, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x2E, 0);
    fx.mem.set16(N2E, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x2F, 0);
    fx.mem.set16(N2F, r);
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xACFA, r);
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x288 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(TEX_A, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x120);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x124)), 180.0);
    fx.fix_residue(y, 0x01E0_4554);
    fx.mem.set16(FACE, ee::fixsfsi(y));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_4584, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(k, 0x01E0_45DC);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_4634, 0x14);
        fx.mem.set8(BYTES2 + k, v - 0xA);
    }
    for k in 0..0x1Eu32 {
        let v = rmod(fx, k, 0x01E0_4684, 0xF);
        fx.mem.set16(CELLS + 2 * k, v);
    }
    0
}

/// 0x106960(r, id, cell +0x56) and its 16 bytes copied to D+0x94.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let c = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, c);
    copy4(fx, D + 0x94, r);
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// D+0x20.. = the task's float position.
fn centre(fx: &mut Fx, t: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(D + 0x20 + 4 * k, w);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let p = param as i64 as u64;
    emit(fx, p);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f != 0 {
            run_task(fx, f, t, k);
        }
    }
    let mut s0 = 0u64;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        s0 += 1;
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            RING | RING_G | RING_S => {
                let (r, ra) = match f {
                    RING => (0x90, 0),
                    RING_G => (0xB0, 1),
                    _ => (0x110, 2),
                };
                cell(fx, t, sp + r, 0x14);
                let c = counter(fx);
                let (w, n, table) = match ra {
                    0 => (TEX5 + 4, 4, BYTES),
                    1 => (TEX_A, 1, BYTES2),
                    _ => (TEX_A, if c < 0x154 { 1 } else { 3 }, BYTES2),
                };
                tex(fx, w);
                fx.mem.set16(D + 0xB8, n);
                let v = fx.mem.i16(t + 0xA);
                fx.mem.set16(D + 0xC0, v);
                centre(fx, t);
                quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, table);
                fx.mem.set16(D + 0xB8, 0);
            }
            SPIRAL => {
                tex(fx, TEX_B);
                let c = fx.mem.i16(t + 0x56);
                l2::rect(fx, sp + 0xA0, 0x97, c);
                copy4(fx, D + 0x94, sp + 0xA0);
                let v = fx.mem.i16(D + 0x9A) >> 1;
                fx.mem.set16(D + 0x9A, v);
                let v = fx.mem.i16(D + 0x96) + fx.mem.i16(D + 0x9A);
                fx.mem.set16(D + 0x96, v);
                let h = fx.self_handle();
                unit_position_saving(fx, h, sp + 0x60, s0, 0x01E0_4870);
                centre(fx, t);
                fx.mem.set16(D + 0xBE, 1);
                let i = fx.mem.i16(t + 0x50) as u32;
                ribbon(fx, t, D + 0x4470 + 0x280 * i, D + 0x45B0 + 0x280 * i);
            }
            BAND => band(fx, t, 0x20, t + 0x10),
            QUADS | QUADS_G => {
                tex(fx, TEX_B);
                cell(fx, t, sp + if f == QUADS { 0xC0 } else { 0xE0 }, 0x9B);
                fx.mem.set16(D + 0xB8, if f == QUADS { 0xA } else { 0xB });
                fx.mem.set16(D + 0xB0, 1);
                billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES2);
                fx.mem.set16(D + 0xB8, 0);
                fx.mem.set16(D + 0xB0, 0);
            }
            CROSS => {
                tex(fx, TEX5 + 8);
                cell(fx, t, sp + 0xD0, 0x14);
                fx.mem.set16(D + 0xB8, 1);
                fx.mem.set16(D + 0xBE, 0);
                cross(fx, t, 0x20, t + 0x10);
            }
            BEAM => beam_draw(fx, t, sp, s0),
            CROWN | PETAL => {
                tex(fx, TEX5);
                cell(fx, t, sp + if f == CROWN { 0xF0 } else { 0x100 }, 0x14);
                fx.mem.set16(D + 0xB8, 1);
                fx.mem.set16(D + 0xBE, 0);
                petals(fx, t, 0x20, t + 0x10);
            }
            BURST => band(fx, t, 0x21, t + 0x10),
            BAND_A => band(fx, t, 0x10, t + 0x10),
            BAND_B => band(fx, t, 0x20, t + 0x10),
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    let c = counter(fx);
    match c {
        0x19E => 1,
        0x1DB => 3,
        0x1A9..=0x1D5 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's beam branch 0x1E04B1C: D+0x20 / D+0x30 at nodes 0x2E / 0x2F
/// and closing by a twelfth / a thirtieth of their gap, then the beams.
fn beam_draw(fx: &mut Fx, t: u32, sp: u32, s0: u64) {
    let d = u64::from(D);
    for (n, out, ra) in [(N2E, D + 0x20, 0x01E0_4B3Cu32), (N2F, D + 0x30, 0x01E0_4B5C), (N2E, sp + 0x70, 0x01E0_4B7C), (N2F, sp + 0x80, 0x01E0_4B9C)] {
        let h = fx.self_handle();
        let slot = fx.mem.i16(n);
        node_position_saving(fx, h, slot, out, s0, Some(d), ra);
    }
    for c in 0..3u32 {
        let g = ee::div(ee::sub(fx.mem.f(sp + 0x80 + 4 * c), fx.mem.f(sp + 0x70 + 4 * c)), 12.0);
        let v = ee::add(fx.mem.f(D + 0x20 + 4 * c), g);
        fx.mem.setf(D + 0x20 + 4 * c, v);
    }
    for c in 0..3u32 {
        let g = ee::div(ee::sub(fx.mem.f(sp + 0x70 + 4 * c), fx.mem.f(sp + 0x80 + 4 * c)), 30.0);
        let v = ee::add(fx.mem.f(D + 0x30 + 4 * c), g);
        fx.mem.setf(D + 0x30 + 4 * c, v);
    }
    for o in [0x60u32, 0x64, 0x68] {
        fx.mem.write_u32(sp + o, 0);
    }
    let mode = if counter(fx) < 0x12C { 0x21 } else { 0x20 };
    beams(fx, t, mode, sp + 0x60);
}

/// A step through the entry's `jalr` (frame `size`: the entry's s0 = 0, s1
/// = D, s2 = D + 0x70 k, s3 = k as the step saves them; its ra 0x1E04708).
fn enter_step(fx: &mut Fx, size: u32, n: u32, base: u32, k: u32) -> u32 {
    let sp = fx.enter(size);
    let regs = [0u64, u64::from(D), u64::from(D) + 0x70 * u64::from(k), u64::from(k)];
    for i in 0..n {
        fx.saved(sp + base + 0x10 * i, &[(0, regs[i as usize])]);
    }
    fx.saved(sp + base + 0x10 * n, &[(0, 0x01E0_4708)]);
    sp
}

fn run_task(fx: &mut Fx, f: u32, t: u32, k: u32) {
    match f {
        RING => {
            enter_step(fx, 0x20, 1, 0, k);
            ring(fx, t);
            fx.leave(0x20);
        }
        SPIRAL => {
            let sp = enter_step(fx, 0x80, 4, 0x10, k);
            spiral(fx, t, sp);
            fx.leave(0x80);
        }
        BAND => {
            enter_step(fx, 0x30, 2, 0, k);
            band_step(fx, t);
            fx.leave(0x30);
        }
        QUADS => {
            enter_step(fx, 0x30, 2, 0, k);
            quads(fx, t);
            fx.leave(0x30);
        }
        CROSS => {
            enter_step(fx, 0x30, 2, 0, k);
            cross_step(fx, t);
            fx.leave(0x30);
        }
        RING_G => {
            let sp = enter_step(fx, 0x60, 2, 0x10, k);
            ring_g(fx, t, sp);
            fx.leave(0x60);
        }
        QUADS_G => {
            let sp = enter_step(fx, 0x60, 2, 0x10, k);
            quads_g(fx, t, sp);
            fx.leave(0x60);
        }
        BEAM => {
            enter_step(fx, 0x20, 1, 0, k);
            beam(fx, t);
            fx.leave(0x20);
        }
        CROWN => {
            enter_step(fx, 0x30, 2, 0, k);
            crown(fx, t);
            fx.leave(0x30);
        }
        BURST => {
            enter_step(fx, 0x20, 1, 0, k);
            burst(fx, t);
            fx.leave(0x20);
        }
        PETAL => {
            enter_step(fx, 0x20, 1, 0, k);
            petal(fx, t);
            fx.leave(0x20);
        }
        RING_S => {
            enter_step(fx, 0x20, 1, 0, k);
            ring_s(fx, t);
            fx.leave(0x20);
        }
        BAND_A => {
            enter_step(fx, 0x30, 2, 0, k);
            band_a(fx, t);
            fx.leave(0x30);
        }
        BAND_B => {
            enter_step(fx, 0x20, 1, 0, k);
            band_b(fx, t);
            fx.leave(0x20);
        }
        LIGHT => {
            let sp = fx.enter(0x10);
            fx.saved(sp, &[(0, 0x01E0_4708)]);
            light(fx, t);
            fx.leave(0x10);
        }
        OBJECT => {
            let sp = enter_step(fx, 0x30, 1, 0, k);
            object(fx, t, sp);
            fx.leave(0x30);
        }
        f => debug_assert!(false, "b756: unknown task step {f:#x}"),
    }
}

/// 0x1E05060(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16 and floats +0x10) with the colour D+0xA904 (twice),
/// +0x50 = D+0xAD56; 1, or 0 (frame 0x60: the caller's s0 / s1, the
/// entry's s2 = the parameter, the runner's s3 / s4; `ra`).
fn spawn(fx: &mut Fx, f: u32, s1: u64, p: u64, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, s1), (0, u64::from(D))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(POS + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let v = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x50, v);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E01920(D, 0): the update counter and the emitter (frame 0x30: ra,
/// the entry's s1 = D, s0 = 0).
fn emit(fx: &mut Fx, p: u64) {
    let sp = fx.enter(0x30);
    let d = u64::from(D);
    fx.saved(sp, &[(0x20, 0x01E0_46D8), (0x10, d), (0, 0)]);
    let mut s1 = d;
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    if counter(fx) == 5 {
        spawn(fx, OBJECT, s1, p, 0x01E0_1974);
    }
    if counter(fx) == 1 {
        spawn(fx, LIGHT, s1, p, 0x01E0_19A0);
    }
    if counter(fx) == 0x5A {
        fx.mem.set16(INDEX, 0);
        at_node(fx, N1, POS, s1, 0x01E0_19E0);
        fx.mem.write_u32(POS + 4, 0x4204_0000);
        spawn(fx, CROWN, s1, p, 0x01E0_19FC);
        spawn(fx, CROSS, s1, p, 0x01E0_1A10);
    }
    if counter(fx) == 0x90 {
        at_node(fx, N1, POS, s1, 0x01E0_1A48);
        fx.mem.write_u32(POS + 4, 0x41FC_0000);
        fx.mem.set16(INDEX, 0);
        fx.mem.set16(INDEX, 1);
        spawn(fx, BAND_A, s1, p, 0x01E0_1A80);
    }
    if counter(fx) == 0xA3 {
        at_node(fx, N1, POS, s1, 0x01E0_1AB8);
        fx.mem.write_u32(POS + 4, 0x41FC_0000);
        fx.mem.set16(INDEX, 0);
        spawn(fx, BAND_B, s1, p, 0x01E0_1AE0);
        fx.mem.set16(INDEX, 1);
        spawn(fx, BAND_B, s1, p, 0x01E0_1B04);
    }
    if counter(fx) == 0x9E {
        fx.mem.set16(INDEX, 0);
        at_node(fx, N1, POS, s1, 0x01E0_1B44);
        fx.mem.write_u32(POS + 4, 0x420C_0000);
        for k in 0..6u64 {
            spawn(fx, RING, k, p, 0x01E0_1B68);
        }
        s1 = 6;
    }
    if counter(fx) >= 0x5F {
        let v = fx.mem.i16(LIMIT);
        if v < 0x168 {
            fx.mem.set16(LIMIT, v + 7);
        }
    }
    let c = counter(fx);
    if (0xB0..0x17D).contains(&c) && odd(c) {
        at_node(fx, N1, POS, s1, 0x01E0_1C34);
        fx.mem.write_u32(POS + 4, 0x4204_0000);
        for ra in [0x01E0_1C68u32, 0x01E0_1CD4] {
            let v = fx.mem.i16(NEXT);
            fx.mem.set16(INDEX, v);
            spawn(fx, SPIRAL, s1, p, ra);
            let v = fx.mem.i16(NEXT) + 1;
            fx.mem.set16(NEXT, v);
            if fx.mem.i16(NEXT) >= 0x28 {
                fx.mem.set16(NEXT, 0);
            }
        }
    }
    let c = counter(fx);
    if (0xBE..0x1B8).contains(&c) && odd(c) {
        spawn(fx, BEAM, s1, p, 0x01E0_1D6C);
    }
    let c = counter(fx);
    if (0x12D..0x1A8).contains(&c) {
        for k in 0..3 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        let c = counter(fx);
        if c < 0x154 {
            if fx.div(c, 3).1 == 1 {
                spawn(fx, RING_S, s1, p, 0x01E0_1E0C);
            }
        } else if odd(c) {
            spawn(fx, RING_S, s1, p, 0x01E0_1E4C);
        }
    }
    let c = counter(fx);
    if c < 0x18A {
        at_node(fx, N2F, POINT, s1, 0x01E0_1E88);
    } else {
        if c < 0x1A8 {
            let z = fx.mem.f(POINT + 8);
            let z = if fx.mem.i16(FACE) != 0 { ee::sub(z, 5.5) } else { ee::add(z, 5.5) };
            fx.mem.setf(POINT + 8, z);
        }
        if matches!(counter(fx), 0x1A8 | 0x1B2 | 0x1BC) {
            for k in 0..3 {
                let w = fx.mem.read_u32(POINT + 4 * k);
                fx.mem.write_u32(POS + 4 * k, w);
            }
            spawn(fx, BURST, s1, p, 0x01E0_1F78);
            for k in 0..0xCu64 {
                spawn(fx, PETAL, k, p, 0x01E0_1F94);
            }
            for k in 0..8u64 {
                spawn(fx, RING_S, k, p, 0x01E0_1FD0);
            }
            s1 = 8;
        }
    }
    if counter(fx) == 0x10E {
        spawn(fx, BAND, s1, p, 0x01E0_2024);
    }
    let c = counter(fx);
    if (0x10E..0x1A9).contains(&c) {
        if c < 0x154 {
            if odd(c) {
                spawn(fx, QUADS, s1, p, 0x01E0_2088);
            }
        } else {
            spawn(fx, QUADS, s1, p, 0x01E0_20A4);
        }
    }
    at_node(fx, N2E, POS, s1, 0x01E0_20C4);
    let c = counter(fx);
    if (0xFA..0x187).contains(&c) {
        if odd(c) {
            spawn(fx, QUADS_G, s1, p, 0x01E0_211C);
        }
        let c = counter(fx);
        if c < 0x12D {
            if odd(c) {
                spawn(fx, RING_G, s1, p, 0x01E0_2180);
            }
        } else {
            spawn(fx, RING_G, s1, p, 0x01E0_21A8);
        }
    }
    fx.leave(0x30);
}

fn at_node(fx: &mut Fx, node: u32, out: u32, s1: u64, ra: u32) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(node);
    node_position_saving(fx, h, slot, out, u64::from(D), Some(s1), ra);
}

/// a >= floor: a - down, else the task ends.
fn fall(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if a >= floor {
        fx.mem.write_u8(t + 0x63, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The second alpha +0x67 down by `down` from `floor`.
fn fall2(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x67);
    if a >= floor {
        fx.mem.write_u8(t + 0x67, a - down);
    }
}

fn add48(fx: &mut Fx, t: u32, v: f32) {
    let r = ee::add(fx.mem.f(t + 0x48), v);
    fx.mem.setf(t + 0x48, r);
}

fn grey2(fx: &mut Fx, t: u32, g: i32) {
    l2::rgb(fx, t + 0x60, g, g, g);
    l2::rgb(fx, t + 0x64, g, g, g);
}

/// +0x64 flickers (0xF0, 8, 0x80) / (0x80, 8, 0x40) by the age.
fn flicker(fx: &mut Fx, t: u32, r: i32) {
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x64, r, 8, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0x80, 8, 0x40);
    }
}

/// The camera's angles (0x205580) at +0x20, x turned by 0x4000.
fn face(fx: &mut Fx, t: u32, ra: u32) {
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), ra);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
}

/// 0x1E021C0: a ring (size 50 growing by 2, grey 0xC0, row +8, life +6
/// 3..7, cell rand % 7, radius +0x50 0..49 widening by +0xE 8..27, turn
/// +0xA); at 0x80 for +6 updates, then out by 8.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4248_0000);
        let v = rmod(fx, t, 0x01E0_21E8, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        let v = fx.rand_from(t, 0x01E0_2214);
        fx.mem.set16(t + 0xA, v % 8);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = rmod(fx, t, 0x01E0_2238, 0x32);
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, t, 0x01E0_2254, 0x14);
        fx.mem.set16(t + 0xE, v + 8);
        let v = rmod(fx, t, 0x01E0_2274, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2290, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fall(fx, t, 0xA, 8);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        add48(fx, t, 2.0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    age(fx, t);
}

/// `n` history points at `at` = the vector at `sp` + 0x60.
fn fill(fx: &mut Fx, sp: u32, at: u32) {
    for k in 0..0x13u32 {
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(at + 0x10 * k + 4 * c, w);
        }
    }
}

/// sp + 0x68 / + 0x60 = r sin / cos of `a` degrees.
fn circle(fx: &mut Fx, sp: u32, r: f32, a: i32) {
    let s = l2::sin_deg(fx, SINES, a, 0);
    fx.mem.setf(sp + 0x68, ee::mul(r, s));
    let c = l2::sin_deg(fx, SINES, a, 1);
    fx.mem.setf(sp + 0x60, ee::mul(r, c));
}

/// 0x1E00080(last, 0x14, 0, v): the 20-point history ending at `last`
/// moved one on, its first point = `v` (xyz).
fn shift(fx: &mut Fx, last: u32, v: u32) {
    for k in (0..0x14u32).rev() {
        let to = last - 0x10 * (0x13 - k);
        let from = if k != 0 { to - 0x10 } else { v };
        for c in 0..3 {
            let w = fx.mem.read_u32(from + 4 * c);
            fx.mem.write_u32(to + 4 * c, w);
        }
    }
}

/// 0x1E02360: a spiral ribbon: two 20-point histories of rings (radii
/// +0x48 / +0x4C, the second 30 degrees back and 50 up) moved on each
/// update, turning 12 degrees; in by 0x15 under 0x80 for 7 updates, then
/// out by 5.
fn spiral(fx: &mut Fx, t: u32, sp: u32) {
    let h = fx.self_handle();
    unit_position_saving(fx, h, sp + 0x70, 0, 0x01E0_2390);
    let mut s0 = 0u32;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x34, 0x4248_0000);
        let v = fx.rand_from(0, 0x01E0_23CC);
        fx.mem.set16(t + 8, v % 2);
        let v = rmod(fx, 0, 0x01E0_23E8, 0x64);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x12));
        l2::rgb(fx, t + 0x64, 0x58, 0x58, 0x58);
        fx.mem.write_u32(t + 0x4C, 0x4190_0000);
        s0 = sp + 0x64;
        fx.mem.write_u32(s0, 0);
        let v = rmod(fx, s0, 0x01E0_2430, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let (r, a) = (fx.mem.f(t + 0x48), fx.mem.i16(t + 0x5A));
        circle(fx, sp, r, a);
        let i = fx.mem.i16(t + 0x50) as u32;
        fill(fx, sp, D + 0x4470 + 0x280 * i);
        let (r, a) = (fx.mem.f(t + 0x4C), (fx.mem.i16(t + 0x5A) - 0x1E) as i16 as i32);
        circle(fx, sp, r, a);
        fx.mem.setf(sp + 0x64, 50.0);
        fill(fx, sp, D + 0x45B0 + 0x280 * i);
        fx.mem.set16(t + 6, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 7 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x15);
        }
    } else {
        fall(fx, t, 7, 5);
    }
    let n = fx.mem.i16(t + 0x54);
    let v = if fx.div(n, 0xA).1 == 1 {
        rmod(fx, s0, 0x01E0_2684, 0x2D)
    } else {
        rmod(fx, s0, 0x01E0_26B0, 5)
    };
    let f20 = ee::from_i32(v - 5);
    let a = fx.mem.i16(t + 0x5A) + 0xC;
    fx.mem.set16(t + 0x5A, a);
    let a = fx.mem.i16(t + 0x5A);
    if a >= 0x168 {
        let (_, h) = fx.div(a, 0x168);
        fx.mem.set16(t + 0x5A, h);
    }
    fx.mem.write_u32(sp + 0x64, 0);
    let (r, a) = (fx.mem.f(t + 0x48), fx.mem.i16(t + 0x5A));
    circle(fx, sp, r, a);
    let i = fx.mem.i16(t + 0x50) as u32;
    shift(fx, D + 0x45A0 + 0x280 * i, sp + 0x60);
    fx.mem.setf(sp + 0x64, ee::add(5.0, f20));
    let (r, a) = (fx.mem.f(t + 0x4C), (fx.mem.i16(t + 0x5A) - 0x1E) as i16 as i32);
    circle(fx, sp, r, a);
    let i = fx.mem.i16(t + 0x50) as u32;
    shift(fx, D + 0x46E0 + 0x280 * i, sp + 0x60);
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x60, 0x50, 0x10, 0x20);
    } else {
        l2::rgb(fx, t + 0x60, 0x40, 8, 0x10);
    }
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 6) < 0xE {
        let v = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, v);
    }
    age(fx, t);
}

/// 0x1E02900: a band at the point D+0xA870 facing the camera, radius 0..
/// 100 by 1, alpha +0x63 up by 10 for 12 updates; out by 9 from update
/// 0x1A9 growing by 2 more.
fn band_step(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x64, 0xF0, 0x80, 0x10);
        let v = rmod(fx, t, 0x01E0_2958, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xC {
        let a = fx.mem.read_u8(t + 0x63).wrapping_add(0xA);
        fx.mem.write_u8(t + 0x63, a);
    }
    if counter(fx) >= 0x1A9 {
        fall(fx, t, 0xB, 9);
        add48(fx, t, 2.0);
    }
    flicker(fx, t, 0xF0);
    let v = fx.mem.f(t + 0x48);
    if ee_lt(v, 100.0) {
        add48(fx, t, 1.0);
    }
    copy4(fx, t + 0x10, POINT);
    face(fx, t, 0x01E0_2AA0);
    age(fx, t);
}

fn spins(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    for (o, ra) in [(0x20u32, ras[0]), (0x24, ras[1]), (0x28, ras[2])] {
        let v = rmod(fx, t, ra, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

/// 0x1E02AD0: a ring of quads at the point, size 6..15, at 0x60 for +6
/// (15..17) updates, then out by 6.
fn quads(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2AFC, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 6));
        let v = fx.rand_from(t, 0x01E0_2B28);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0x40, 0x20, 0);
        spins(fx, t, [0x01E0_2B5C, 0x01E0_2B78, 0x01E0_2B94]);
        fx.mem.set16(t + 0x52, 1);
        fx.mem.set16(t + 0x50, 0);
        let v = rmod(fx, t, 0x01E0_2BBC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2BD8, 6);
        fx.mem.set16(t + 0xE, v + 7);
        let v = fx.rand_from(t, 0x01E0_2BF8);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_2C18, 3);
        fx.mem.set16(t + 6, v + 0xF);
        fx.mem.set16(t + 0x58, 2);
        copy4(fx, t + 0x10, POINT);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        fall(fx, t, 8, 6);
    }
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
}

/// 0x1E02CE0: the quad cross (size 12, grey 0x80), in by 9 under 0x80 for
/// 13 updates; out by 9 from update 0x19B; from 0x96 turning about y by
/// 0x100.
fn cross_step(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4140_0000);
        grey2(fx, t, 0x80);
        let v = rmod(fx, t, 0x01E0_2D3C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.write_u32(t + 0x20, 0x4000);
    }
    if fx.mem.i16(t + 0x54) < 0xD {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 9);
        }
    } else if counter(fx) >= 0x19B {
        fall(fx, t, 0xB, 9);
        fall2(fx, t, 0xB, 9);
    }
    if counter(fx) >= 0x96 {
        let v = fx.mem.read_u32(t + 0x24).wrapping_sub(0x100);
        fx.mem.write_u32(t + 0x24, v);
    }
    age(fx, t);
}

/// The position of the node rings: node 0x2E minus `g`-th parts of the
/// gap to node 0x2F, the part count +0x52 from age 1.
fn on_gap(fx: &mut Fx, t: u32, sp: u32, f20: f32, g: f32) {
    copy3(fx, t + 0x10, sp + 0x40);
    if fx.mem.i16(t + 0x54) == 0 {
        for c in 0..3u32 {
            let q = ee::div(ee::sub(fx.mem.f(sp + 0x40 + 4 * c), fx.mem.f(sp + 0x50 + 4 * c)), g);
            let v = ee::sub(fx.mem.f(t + 0x10 + 4 * c), q);
            fx.mem.setf(t + 0x10 + 4 * c, v);
        }
    } else {
        for c in 0..3u32 {
            let q = ee::div(ee::sub(fx.mem.f(sp + 0x40 + 4 * c), fx.mem.f(sp + 0x50 + 4 * c)), g);
            fx.mem.setf(sp + 0x40 + 4 * c, q);
        }
        for c in 0..3u32 {
            let v = ee::sub(fx.mem.f(t + 0x10 + 4 * c), ee::mul(fx.mem.f(sp + 0x40 + 4 * c), f20));
            fx.mem.setf(t + 0x10 + 4 * c, v);
        }
    }
}

fn gap_nodes(fx: &mut Fx, sp: u32, ras: [u32; 2]) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(N2E);
    node_position_saving(fx, h, slot, sp + 0x40, u64::from(D), Some(0), ras[0]);
    let h = fx.self_handle();
    let slot = fx.mem.i16(N2F);
    node_position_saving(fx, h, slot, sp + 0x50, u64::from(sp + 0x50), Some(0), ras[1]);
}

/// 0x1E02E30: a ring (size 10 growing by 3.5, grey 0x20) leaving node
/// 0x2E along the gap to node 0x2F; at 0x80 for +6 updates, out by 9.
fn ring_g(fx: &mut Fx, t: u32, sp: u32) {
    let f20 = ee::from_i32(fx.mem.i16(t + 0x52));
    gap_nodes(fx, sp, [0x01E0_2E74, 0x01E0_2E98]);
    let s0 = sp + 0x50;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let v = rmod(fx, s0, 0x01E0_2EB4, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x20);
        let v = fx.rand_from(s0, 0x01E0_2EE0);
        fx.mem.set16(t + 0xA, v % 8);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = rmod(fx, s0, 0x01E0_2F04, 0x14);
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, s0, 0x01E0_2F20, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = rmod(fx, s0, 0x01E0_2F40, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, s0, 0x01E0_2F5C, 5);
        fx.mem.set16(t + 6, v + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fall(fx, t, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        add48(fx, t, 3.5);
    }
    on_gap(fx, t, sp, f20, 20.0);
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    age(fx, t);
}

/// 0x1E03170: a ring of quads leaving node 0x2E along the gap (thirtieths),
/// at 0x60 for +6 updates, then out by 6.
fn quads_g(fx: &mut Fx, t: u32, sp: u32) {
    let f20 = ee::from_i32(fx.mem.i16(t + 0x52));
    gap_nodes(fx, sp, [0x01E0_31B4, 0x01E0_31D8]);
    let s0 = sp + 0x50;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(s0, 0x01E0_31F0);
        fx.mem.setf(t + 0x48, ee::from_i32(v % 8 + 2));
        let v = fx.rand_from(s0, 0x01E0_321C);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0x20, 0x10, 0);
        for (o, ra) in [(0x20u32, 0x01E0_3250u32), (0x24, 0x01E0_326C), (0x28, 0x01E0_3288)] {
            let v = rmod(fx, s0, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        fx.mem.set16(t + 0x50, 0);
        let v = rmod(fx, s0, 0x01E0_32B0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, s0, 0x01E0_32CC, 6);
        fx.mem.set16(t + 0xE, v + 6);
        let v = fx.rand_from(s0, 0x01E0_32EC);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, s0, 0x01E0_330C, 3);
        fx.mem.set16(t + 6, v + 0xF);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        fall(fx, t, 8, 6);
    }
    on_gap(fx, t, sp, f20, 30.0);
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
}

/// 0x1E034F0: a beam pair, width +0x48 up by 0.3 to 1.2; in by 0x2A under
/// 0x80 for 3 updates, out by 12.
fn beam(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x28, 0x28, 0x28);
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x64, 0x80, 8, 8);
        let v = rmod(fx, t, 0x01E0_3540, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x2A);
        }
    } else {
        fall(fx, t, 0xE, 0xC);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.f(t + 0x48);
        if ee_lt(v, f32::from_bits(0x3F99_999A)) {
            add48(fx, t, f32::from_bits(0x3E99_999A));
        }
    }
    flicker(fx, t, 0xC0);
    age(fx, t);
}

/// Both alphas in by `up` under 0x80.
fn rise2(fx: &mut Fx, t: u32, up: u8) {
    for o in [0x63u32, 0x67] {
        let a = fx.mem.read_u8(t + o);
        if a < 0x80 {
            fx.mem.write_u8(t + o, a + up);
        }
    }
}

/// 0x1E03670: the crown of petals, radii 17 / 11, in by 12 for 20 updates,
/// out by 9 from update 0x19B; turned half round by the side until 0x96,
/// then by 0x100 a update.
fn crown(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4188_0000);
        fx.mem.setf(t + 0x4C, ee::sub(17.0, 6.0));
        let v = rmod(fx, t, 0x01E0_36B8, 7);
        fx.mem.set16(t + 0x56, v);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, 0x01E0_36DC, 3);
        fx.mem.set16(t + 8, v + 4);
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        rise2(fx, t, 0xC);
    } else if counter(fx) >= 0x19B {
        fall(fx, t, 0xB, 9);
        fall2(fx, t, 0xB, 9);
    }
    let g = if odd(fx.mem.i16(t + 0x54)) { 0x80 } else { 0xC0 };
    grey2(fx, t, g);
    if counter(fx) < 0x96 {
        let v = if fx.mem.i16(FACE) != 0 { 0 } else { 0x8000 };
        fx.mem.write_u32(t + 0x24, v);
    } else {
        let v = fx.mem.read_u32(t + 0x24).wrapping_add(0x100);
        fx.mem.write_u32(t + 0x24, v);
    }
    age(fx, t);
}

/// 0x1E03880: the burst ring, white to (0xFF, 0x80, 0x10), radius 550
/// (+10 from age 13), in by 0x20 under 0x80 for 15 updates, out by 6.
fn burst(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4409_8000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x10);
        let v = rmod(fx, t, 0x01E0_38D8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xF {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x20);
        }
    } else {
        fall(fx, t, 8, 6);
    }
    if fx.mem.i16(t + 0x54) >= 0xD {
        add48(fx, t, 10.0);
    }
    face(fx, t, 0x01E0_397C);
    age(fx, t);
}

/// 0x1E039B0: a burst petal ring, radii 40 (+2) / 0, spun at random, in
/// by 0x15 under 0x80 for 9 updates, out by 8.
fn petal(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4220_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        let v = rmod(fx, t, 0x01E0_39DC, 7);
        fx.mem.set16(t + 0x56, v);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, 0x01E0_3A00, 3);
        fx.mem.set16(t + 8, v + 4);
        spins(fx, t, [0x01E0_3A20, 0x01E0_3A3C, 0x01E0_3A58]);
    }
    if fx.mem.i16(t + 0x54) < 9 {
        rise2(fx, t, 0x15);
    } else {
        fall(fx, t, 0xA, 8);
        fall2(fx, t, 0xA, 8);
    }
    let g = if odd(fx.mem.i16(t + 0x54)) { 0x80 } else { 0xC0 };
    grey2(fx, t, g);
    add48(fx, t, 2.0);
    age(fx, t);
}

/// 0x1E03BA0: a ring (size 50 growing by 1.5, grey 0x18, radius +0x50
/// 0..19 widening by +0xE 6..13), at 0x80 for +6 updates, out by 12.
fn ring_s(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4248_0000);
        let v = rmod(fx, t, 0x01E0_3BC8, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x18, 0x18, 0x18);
        let v = fx.rand_from(t, 0x01E0_3BF4);
        fx.mem.set16(t + 0xA, v % 8);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = rmod(fx, t, 0x01E0_3C18, 0x14);
        fx.mem.set16(t + 0x50, v);
        let v = fx.rand_from(t, 0x01E0_3C34);
        fx.mem.set16(t + 0xE, v % 8 + 6);
        let v = rmod(fx, t, 0x01E0_3C54, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_3C70, 5);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fall(fx, t, 0xE, 0xC);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        add48(fx, t, 1.5);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    age(fx, t);
}

/// 0x1E03F20: a band (index 0: radii 0 / 110, 1: 150 / 250), up by 0x10 for
/// 12 updates, out by 9 from 0x19B.
fn band_a(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x48, 0);
            fx.mem.write_u32(t + 0x4C, 0x42DC_0000);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x48, 0x4316_0000);
            fx.mem.write_u32(t + 0x4C, 0x437A_0000);
        }
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        l2::rgb(fx, t + 0x64, 0xF0, 8, 8);
        let v = rmod(fx, t, 0x01E0_3FAC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x50) < 2 {
        if fx.mem.i16(t + 0x54) < 0xC {
            let a = fx.mem.read_u8(t + 0x63).wrapping_add(0x10);
            fx.mem.write_u8(t + 0x63, a);
        } else if counter(fx) >= 0x19B {
            fall(fx, t, 0xB, 9);
            fall2(fx, t, 0xB, 9);
        }
    }
    flicker(fx, t, 0xF0);
    age(fx, t);
}

/// 0x1E040D0: a band (radii 110 / 250, both), its height +0x34 rising by
/// 10; in by 0x15 under 0xC0 for 12 updates, out by 3.
fn band_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x48, 0x42DC_0000);
            fx.mem.write_u32(t + 0x4C, 0x42DC_0000);
            fx.mem.write_u32(t + 0x34, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x48, 0x437A_0000);
            fx.mem.write_u32(t + 0x4C, 0x437A_0000);
            fx.mem.write_u32(t + 0x34, 0);
        }
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xF0, 8, 8);
        let v = rmod(fx, t, 0x01E0_4158, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xC {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x15);
        }
    } else {
        fall(fx, t, 5, 3);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = ee::add(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
    }
    flicker(fx, t, 0xF0);
    age(fx, t);
}

/// 0x1E04260: the stage light +0x50 (from 0x80): down by 10 to 0x30 while
/// the timer is 31 or more, then back up to 0x80 (0x1F7F10).
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let l = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x1F {
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
    }
    let l = fx.mem.read_u8(t + 0x50);
    fx.set_stage_light(i32::from(l));
}

/// 0x1E04330 (frame 0x30): the object op 0x1F8380(3, 0, the record's words
/// 0x1E06480 / 0x1E06488 at sp + 0x28 / + 0x2C, 0) each update; the task
/// ends from age 0xA5.
fn object(fx: &mut Fx, t: u32, sp: u32) {
    for (to, from) in [(0x28u32, 0x01E0_6480u32), (0x2C, 0x01E0_6488)] {
        let w = fx.mem.read_u32(from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.boundary(0x001F_8380, "object op 0x1F8380", [3, 0, sp + 0x28, sp + 0x2C]);
    age(fx, t);
    if fx.mem.i16(t + 0x54) >= 0xA5 {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 0x1E05DF0(D, t, mode, pos) (frame 0x210): two gouraud strips from
/// D+0x20 / D+0x30 out to them + (+-(+0x48), +-(+0x4C)), coloured +0x64 /
/// +0x60; through TRS(angles +0x20, pos); blend (mode >> 4), depth mode &
/// 15 (0 the mean w, 1 the far Z at key 0, else key -1).
fn beams(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x210);
    copy4(fx, sp + 0xB0, pos);
    let d = sp + 0x160;
    for (l, c, to) in [(0x1F0u32, 0x64u32, 0x170u32), (0x1F8, 0x60, 0x180), (0x200, 0x64, 0x190), (0x208, 0x60, 0x1A0)] {
        l2::rgbaq(fx, sp + l, t + c);
        for k in 0..2 {
            let w = fx.mem.read_u32(sp + l + 4 * k);
            fx.mem.write_u32(sp + to + 4 * k, w);
        }
    }
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    copy3(fx, sp + 0x100, sp + 0xB0);
    l2::xform_vp(fx, sp + 0x110, 0, t + 0x20, sp + 0x100);
    let depth = mode & 0xF;
    let (f20, f21) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let (f22, f23) = (ee::neg(f20), ee::neg(f21));
    for s0 in 0..2 {
        for (o, y) in [(0xC4u32, 0x24u32), (0xD4, 0x24), (0xE4, 0x34), (0xF4, 0x34)] {
            let w = fx.mem.read_u32(D + y);
            fx.mem.write_u32(sp + o, w);
        }
        let (dx, dz) = if s0 == 0 { (f20, f21) } else { (f22, f23) };
        let (a, b, c, e) = (fx.mem.f(D + 0x20), fx.mem.f(D + 0x28), fx.mem.f(D + 0x30), fx.mem.f(D + 0x38));
        for (o, v) in [(0xC0u32, ee::add(a, dx)), (0xC8, ee::add(b, dz)), (0xD0, a), (0xD8, b), (0xE0, ee::add(c, dx)), (0xE8, ee::add(e, dz)), (0xF0, c), (0xF8, e)] {
            fx.mem.setf(sp + o, v);
        }
        if l2::project(fx, sp + 0xC0, SPR, sp + 0x110, d, sp + 0x150) {
            let ot = fx.mem.read_u32(D + 0x80);
            match depth {
                0 => {
                    let w = fx.mem.f(sp + 0x150);
                    fx.fix_residue(w, 0x01E0_60E0);
                    let key = ee::fixunssfsi(w) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z_desc(fx, d, 1);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
    }
    fx.leave(0x210);
}

/// 0x1E061D0(D, t, mode, pos) (frame 0x1E0): a strip3 cross: four quads
/// 0x109640 of half size +0x48 (modes 0..3) coloured +0x60, cell 0 of
/// rect 0x14, through TRS(angles +0x20, pos), keyed by their mean w.
fn cross(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1E0);
    copy4(fx, sp + 0x70, pos);
    let d = sp + 0x130;
    l2::desc_strip3(fx, d);
    for (l, to) in [(0x1C0u32, 0x140u32), (0x1C8, 0x158), (0x1D0, 0x170), (0x1D8, 0x188)] {
        l2::rgbaq(fx, sp + l, t + 0x60);
        for k in 0..2 {
            let w = fx.mem.read_u32(sp + l + 4 * k);
            fx.mem.write_u32(sp + to + 4 * k, w);
        }
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    fx.desc_alpha(d, blend_alpha(mode >> 4));
    copy3(fx, sp + 0xC0, sp + 0x70);
    l2::xform_vp(fx, sp + 0xD0, 0, t + 0x20, sp + 0xC0);
    for o in [0x84u32, 0x94, 0xA4, 0xB4] {
        fx.mem.write_u32(sp + o, 0);
    }
    l2::rect(fx, sp + 0x120, 0x14, 0);
    copy4(fx, D + 0x94, sp + 0x120);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    for s4 in 0..4 {
        let f = fx.mem.f(t + 0x48);
        fx.fix_residue(f, 0x01E0_63D0);
        let a = ee::fixsfsi(f);
        fx.fix_residue(f, 0x01E0_63DC);
        let b = ee::fixsfsi(f);
        l2::quad_ints(fx, sp + 0x80, a, b, s4);
        if l2::project(fx, sp + 0x80, SPR, sp + 0xD0, d, sp + 0x110) {
            let w = fx.mem.f(sp + 0x110);
            fx.fix_residue(w, 0x01E0_6418);
            let key = ee::fixunssfsi(w) as i32;
            let ot = fx.mem.read_u32(OT_AT);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x1E0);
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 0x108020(local, c) and the 8 bytes it builds copied to `to`.
fn rgbaq_to(fx: &mut Fx, local: u32, c: u32, to: u32) {
    l2::rgbaq(fx, local, c);
    for k in 0..2 {
        let w = fx.mem.read_u32(local + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// b191's Lagrange cubic through `p` at `t`.
fn lagrange(p: [[f32; 3]; 4], t: f32) -> [f32; 3] {
    use ee::{add, div, mul, neg, sub};
    let (tm1, tm2, tp1) = (sub(t, 1.0), sub(t, 2.0), add(1.0, t));
    let w0 = div(mul(neg(t), mul(tm1, tm2)), 6.0);
    let w3 = div(mul(tp1, mul(t, tm1)), 6.0);
    let w1 = div(mul(tm2, mul(tp1, tm1)), 2.0);
    let w2 = div(mul(neg(tp1), mul(t, tm2)), 2.0);
    std::array::from_fn(|c| {
        let f6 = add(mul(p[0][c], w0), mul(p[1][c], w1));
        let acc = add(mul(p[2][c], w2), f6);
        add(acc, mul(p[3][c], w3))
    })
}

fn points(fx: &mut Fx, at: [u32; 4]) -> [[f32; 3]; 4] {
    std::array::from_fn(|i| std::array::from_fn(|c| fx.mem.f(at[i] + 4 * c as u32)))
}

/// a907e1567858 (0x1E05530; b771.rs's copy, 560 words at 1.00): b191's
/// textured strip3 ribbon between the histories `a` / `b`, but keyed by
/// the mean w (0x1A2C70 of sp + 0x110) instead of at the far Z with key 0
/// [F word-compare: the only difference].
fn ribbon(fx: &mut Fx, t: u32, a: u32, b: u32) {
    let sp = fx.enter(0x390);
    let n = fx.mem.i16(t + 6);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0x160 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x230, 0, 0, sp + 0x160);
    let d = sp + 0x2D8;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 0);
    l2::uv_strip3(fx, d, D + 0x94, 0);
    let rh2 = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, rh2, 1);
    let (ca, cb) = (sp + 0x388, sp + 0x38C);
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(ca + k, v);
        let v = fx.mem.read_u8(t + 0x64 + k);
        fx.mem.write_u8(cb + k, v);
    }
    let fp = 2 * n;
    copy4(fx, sp + 0x2B0, a);
    for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
        let from = if k < 2 { sp + 0x2B0 } else { a + o };
        copy4(fx, sp + 0x1B0 + 0x10 * k as u32, from);
    }
    copy4(fx, sp + 0x2C0, b);
    for (k, o) in [0u32, 0, 0x10, 0x20].into_iter().enumerate() {
        let from = if k < 2 { sp + 0x2C0 } else { b + o };
        copy4(fx, sp + 0x1F0 + 0x10 * k as u32, from);
    }
    for (to, from) in [(0xD4u32, 0x1C0u32), (0xD8, 0x1D0), (0xDC, 0x1E0), (0xE0, 0x1C4), (0xE4, 0x1B4), (0xE8, 0x1D4), (0xEC, 0x1E4), (0xF0, 0x1C8), (0xF4, 0x1B8), (0xF8, 0x1D8), (0xFC, 0x1E8)] {
        let w = fx.mem.read_u32(sp + from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.mem.set32(sp + 0x100, n - 1);
    let mut s0 = 0i32;
    while s0 < fx.mem.i32(sp + 0x100) {
        let (pa, pb) = if s0 != 0 {
            let qa = a.wrapping_add((0x10 * s0) as u32);
            let qb = b.wrapping_add((0x10 * s0) as u32);
            ([qa.wrapping_sub(0x10), qa, qa + 0x10, qa + 0x20], [qb.wrapping_sub(0x10), qb, qb + 0x10, qb + 0x20])
        } else {
            let pa = sp + 0x1B0;
            let pb = sp + 0x1F0;
            ([pa, pa + 0x10, pa + 0x20, pa + 0x30], [pb, pb + 0x10, pb + 0x20, pb + 0x30])
        };
        let mut f20 = 0.0f32;
        for _ in 0..2 {
            let v = fx.mem.read_u8(ca + 3);
            fx.mem.write_u8(cb + 3, v);
            rgbaq_to(fx, sp + 0x368, ca, d + 0x10);
            rgbaq_to(fx, sp + 0x370, cb, d + 0x40);
            let al = i32::from(fx.mem.read_u8(t + 0x63));
            let q = fx.div(al, fp).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(ca + 3)) - q;
            fx.mem.write_u8(ca + 3, if v > 0 { v as u8 } else { 0 });
            let v = fx.mem.read_u8(ca + 3);
            fx.mem.write_u8(cb + 3, v);
            rgbaq_to(fx, sp + 0x378, ca, d + 0x28);
            rgbaq_to(fx, sp + 0x380, cb, d + 0x58);
            for i in 0..2u32 {
                let pts = points(fx, pa);
                let v = lagrange(pts, f20);
                let (out, keep) = if s0 != 0 { (sp + 0x270, sp + 0x170) } else { (sp + 0x290, sp + 0x170) };
                for (c, x) in v.into_iter().enumerate() {
                    fx.mem.setf(out + 4 * c as u32, x);
                }
                copy4(fx, keep + 0x10 * i, out);
                let pts = points(fx, pb);
                let v = lagrange(pts, f20);
                let out = if s0 != 0 { sp + 0x280 } else { sp + 0x2A0 };
                for (c, x) in v.into_iter().enumerate() {
                    fx.mem.setf(out + 4 * c as u32, x);
                }
                copy4(fx, sp + 0x190 + 0x10 * i, out);
                if i == 0 {
                    f20 = ee::add(f20, 0.5);
                }
                if f20 > 9.0 {
                    f20 = 1.0;
                }
            }
            for (to, from) in [(0x120u32, 0x170u32), (0x130, 0x180), (0x140, 0x190), (0x150, 0x1A0)] {
                for k in 0..3 {
                    let w = fx.mem.read_u32(sp + from + 4 * k);
                    fx.mem.write_u32(sp + to + 4 * k, w);
                }
            }
            if l2::project(fx, sp + 0x120, SPR, sp + 0x230, d, sp + 0x110) {
                let w = fx.mem.f(sp + 0x110);
                fx.fix_residue(w, 0x01E0_5D50);
                let key = ee::fixunssfsi(w) as i32;
                let ot = fx.mem.read_u32(D + 0x80);
                fx.submit(d, ot, key);
            }
        }
        s0 += 1;
    }
    fx.leave(0x390);
}

/// 0x1E051F0(D, t, mode, pos) (frame 0x1E0; b721's 0x1E055B0 with the
/// petals at or past the limit D+0xAD50 degrees skipped, their angle not
/// advancing): eighteen petals 20 degrees
/// apart from the angle +0x5A - 200, each a strip3 piece of 20 degrees
/// between the radii +0x48 / +0x4C in the x / z plane on its own atlas cell
/// (0x14, D+0xAD0C [k]), coloured +0x60 / +0x64, UVs in D+0xBE's
/// orientation; through TRS(angles +0x20, pos) x the view-projection; keyed
/// by its mean w on D+0xA90C; blend (mode >> 4).
fn petals(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1E0);
    copy4(fx, sp + 0x90, pos);
    let d = sp + 0x150;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 1);
    l2::colour_strip3(fx, d, t + 0x64, 2);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    fx.desc_alpha(d, crate::c061::blend_alpha(mode >> 4));
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x90 + 4 * k);
        fx.mem.write_u32(sp + 0xE0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xF0, 0, t + 0x20, sp + 0xE0);
    for o in [0xA4u32, 0xB4, 0xC4, 0xD4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let r0 = fx.mem.f(t + 0x48);
    let r1 = fx.mem.f(t + 0x4C);
    let mut s1 = 0i32;
    for k in 0..0x12u32 {
        if s1 >= fx.mem.i16(LIMIT) {
            continue;
        }
        let c = fx.mem.i16(CELLS + 2 * k);
        l2::rect(fx, sp + 0x140, 0x14, c);
        copy4(fx, D + 0x94, sp + 0x140);
        let order = fx.mem.i16(D + 0xBE);
        l2::uv_strip3(fx, d, D + 0x94, order);
        // (extra degrees, the r0 / r1 vertices)
        for (e, inner, outer) in [(0, 0xC0u32, 0xA0u32), (0x14, 0xD0, 0xB0)] {
            let a = (fx.mem.i16(t + 0x5A) + e + s1 - 200) as i16 as i32;
            let cs = l2::sin_deg(fx, SINES, a, 1);
            let sn = l2::sin_deg(fx, SINES, a, 0);
            fx.mem.setf(sp + inner, ee::mul(cs, r0));
            fx.mem.setf(sp + inner + 8, ee::mul(sn, r0));
            fx.mem.setf(sp + outer, ee::mul(cs, r1));
            fx.mem.setf(sp + outer + 8, ee::mul(sn, r1));
        }
        if l2::project(fx, sp + 0xA0, SPR, sp + 0xF0, d, sp + 0x130) {
            let w = fx.mem.f(sp + 0x130);
            fx.fix_residue(w, 0x01E0_54A8);
            let key = ee::fixunssfsi(w) as i32;
            let ot = fx.mem.read_u32(OT_AT);
            fx.submit(d, ot, key);
        }
        s1 = (s1 + 0x14) as i16 as i32;
    }
    fx.leave(0x1E0);
}

/// 0x1E009B0(D, t, mode, pos): quads4's ring_band_fy (853ed233d9de; 36
/// strips of 10 degrees between the radii +0x48 / +0x4C) with the outer
/// points at the height +0x3C too [F fdiff: the only difference].
fn band(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x200);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    let d = sp + 0x170;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * k);
        fx.mem.write_u32(sp + 0x110 + 4 * k, 0x3DCC_CCCD);
        fx.mem.write_u32(sp + 0x100 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x20, sp + 0x100);
    let y = fx.mem.read_u32(t + 0x34);
    fx.mem.write_u32(sp + 0xC4, y);
    fx.mem.write_u32(sp + 0xD4, y);
    let y = fx.mem.read_u32(t + 0x3C);
    fx.mem.write_u32(sp + 0xE4, y);
    fx.mem.write_u32(sp + 0xF4, y);
    let inner = fx.mem.f(t + 0x48);
    let outer = fx.mem.f(t + 0x4C);
    let depth = mode & 0xF;
    let tab = D + 0xD8;
    let mut a = 0i32;
    for k in 0..36 {
        if k == 0 {
            let s = (fx.mem.i16(t + 0x5A) + a) as i16 as i32;
            for (r, x, z) in [(inner, 0xC0u32, 0xC8u32), (outer, 0xE0, 0xE8)] {
                let c = l2::sin_deg(fx, tab, s, 1);
                fx.mem.setf(sp + x, ee::mul(r, c));
                let sn = l2::sin_deg(fx, tab, s, 0);
                fx.mem.setf(sp + z, ee::mul(r, sn));
            }
        } else {
            // The previous strip's end points (vec4s) become the start.
            for (to, from) in [(0xC0u32, 0xD0u32), (0xE0, 0xF0)] {
                for c in 0..4 {
                    let w = fx.mem.read_u32(sp + from + 4 * c);
                    fx.mem.write_u32(sp + to + 4 * c, w);
                }
            }
        }
        let e = (a + (fx.mem.i16(t + 0x5A) + 0xA)) as i16 as i32;
        for (r, x, z) in [(inner, 0xD0u32, 0xD8u32), (outer, 0xF0, 0xF8)] {
            let c = l2::sin_deg(fx, tab, e, 1);
            fx.mem.setf(sp + x, ee::mul(r, c));
            let sn = l2::sin_deg(fx, tab, e, 0);
            fx.mem.setf(sp + z, ee::mul(r, sn));
        }
        if l2::project(fx, sp + 0xC0, SPR, sp + 0x120, d, sp + 0x160) {
            let ot = fx.mem.read_u32(D + 0x80);
            match depth {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z_desc(fx, d, 1);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        a = (a + 0xA) as i16 as i32;
    }
    fx.leave(0x200);
}
