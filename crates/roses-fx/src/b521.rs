//! BMODULE b521 (card 285), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01BB0, data area a0 = 0x1ED0000, 0x57B0 bytes)
//! [F]: c061's rising-trail program (the Exodia
//! finale's 0x98 trail records, here 25 at D+0x2D0) with a 150-task pool
//! of 0x70 at D+0x11B0 (`jalr` steps). Update 1 a window tint (0x1E01970,
//! (0x80, 0x80, 0xB0), ST inset +0x48 + 20) at the attacker; 0x40 ten
//! points (D+0x53C0, y -30 + 17 k, radius 3 but the first 0) spinning by 4
//! degrees a update round the attacker's position 25 down (D+0x5460, z
//! sliding by a fiftieth of the units' z gap over 0xAA..0xDC) and the
//! first trail record (30 up, colour 0x20 grey): each record rises round
//! the points (c061's step with grey 0x48 colours, the alpha in under 50),
//! spawning the next while the timer is 31 or more, drawn as 12 slices
//! through four nested scales (textures 5 / 0 and 4 / 15 rect 0x96).
//! 0x40..0x140 a grey textured quad (0x1E01440, size 8..30 growing,
//! drifting, turned by its random spins) and a white facing ring
//! (0x1E01790, radius 100..149 widening) a update at the centre. Returns 1
//! / 4 / 2 / 6 / 8 on 0xCD / 0x163 / 0x154 / 0xD8..0x14E / timer out.
use crate::b076::{node_position_saving, request_node_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TRAILS: u32 = 25;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const QUAD: u32 = 0x01E0_1440;
const RING: u32 = 0x01E0_1790;
const TINT: u32 = 0x01E0_1970;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_1DDC;
const POINTS: u32 = D + 0x53C0;
const CENTRE: u32 = D + 0x5460;
const SELF_POS: u32 = D + 0x5470;
const SLIDE: u32 = D + 0x5488;
const TEX5: u32 = D + 0x5490;
const TIMER: u32 = D + 0x54B0;
const COLOUR: u32 = D + 0x54B4;
const OTW: u32 = D + 0x54BC;
const LAST: u32 = D + 0x54C0;
const RADII: u32 = D + 0x54C4;
const HEIGHT: u32 = D + 0x54F0;
const MEAN_W: u32 = D + 0x54F4;
const ANGLES: u32 = D + 0x5504;
const TNODE: u32 = D + 0x551A;
const FACING: u32 = D + 0x5528;
const REC_ANGLE: u32 = D + 0x552C;
const INDEX: u32 = D + 0x5532;
const COUNTER: u32 = D + 0x5536;
const BYTES: u32 = D + 0x5538;
const SINES: u32 = D + 0x5638;

/// The entry 0x1E01BB0 (a0 = D, a1 = param; frame 0x110).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x110);
    fx.mem.write_u32(OTW, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x110);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    let p = param as i64 as u64;
    let d = u64::from(D);
    fx.bzero(D, 0x57B0);
    l2::sine_table_saving(fx, SINES, d, 0, 0x01E0_1BFC);
    l2::sine_table_saving(fx, D + 0xD8, d, 0, 0x01E0_1C04);
    let h = fx.self_handle();
    let r = request_node_saving(fx, h, 1, 1, [Some(d), Some(0), Some(p)], 0x01E0_1C1C);
    fx.mem.set16(D + 0x5518, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2C8 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    let s = [Some(d), Some(d + 0xC), Some(p)];
    let h = fx.target_handle();
    let r = request_node_saving(fx, h, 1, 1, s, 0x01E0_1C88);
    fx.mem.set16(TNODE, r);
    let h = fx.self_handle();
    let r = request_node_saving(fx, h, 0x37, 0, s, 0x01E0_1CA0);
    fx.mem.set16(D + 0x551C, r);
    let h = fx.self_handle();
    let r = request_node_saving(fx, h, 0x3D, 0, s, 0x01E0_1CB8);
    fx.mem.set16(D + 0x551E, r);
    fx.mem.set16(D + 0x5534, param);
    fx.mem.set32(TIMER, 0x168);
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_1CD4);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.set8(BYTES + k, 0);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TRAILS {
        let r = D + 0x2D0 + 0x98 * k;
        if fx.mem.i16(r + 0x88) != 0 {
            trail_step(fx, r, k);
        }
    }
    for k in 0..TRAILS {
        let r = D + 0x2D0 + 0x98 * k;
        if fx.mem.i16(r + 0x88) != 0 {
            trail_draw(fx, r, sp + 0x80, 0, k);
            trail_draw(fx, r, sp + 0x80, 1, k);
        }
    }
    for k in 0..TASKS {
        let t = D + 0x11B0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x44) {
            0 => {}
            QUAD => quad_step(fx, t),
            RING => ring_step(fx, t),
            TINT => tint(fx, t),
            f => panic!("b521: unknown task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x11B0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == QUAD {
            let w = fx.mem.read_u32(TEX5 + 4);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x60, sub);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x3F)] {
                fx.mem.set16(D + o, v);
            }
            fx.mem.set16(D + 0xBE, 0);
            let a = fx.mem.i16(t + 0x5A);
            let c = l2::sin_deg(fx, SINES, a, 1);
            let x = ee::add(fx.mem.f(t + 0x10), ee::mul(fx.mem.f(t + 0x48), c));
            fx.mem.setf(sp + 0x50, x);
            let y = fx.mem.read_u32(t + 0x14);
            fx.mem.write_u32(sp + 0x54, y);
            let a = fx.mem.i16(t + 0x5A);
            let s = l2::sin_deg(fx, SINES, a, 0);
            let z = ee::add(fx.mem.f(t + 0x18), ee::mul(fx.mem.f(t + 0x48), s));
            fx.mem.setf(sp + 0x58, z);
            billboard::quad(fx, billboard::uv_rect_q, D, t, 0x10, sp + 0x50);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, sub);
            let w = fx.mem.read_u32(TEX5 + 8);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 5);
            let o = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, o);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            quads4::ring_facing_flat(fx, billboard::uv_rect_q, D, t, 0x10, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xCD => 1,
        0x163 => 4,
        0x154 => 2,
        0xD8..=0x14E => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x106960(r, 0x14, sub) and its 16 bytes copied to D+0x94.
fn cell(fx: &mut Fx, r: u32, sub: i32) {
    l2::rect(fx, r, 0x14, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E00CE0(D, 0): the update counter and the emitter (frame 0xA0: ra,
/// the runner's s4 / s3 (0), the entry's s2 = the parameter, s1 = 0, s0
/// = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0xA0);
    let p = param as i64 as u64;
    let d = u64::from(D);
    fx.saved(sp, &[(0x50, 0x01E0_1D30), (0x40, 0), (0x30, 0), (0x20, p), (0x10, 0), (0, d)]);
    // The emitter's s0 / s1 at its calls.
    let (mut s0, mut s1) = (d, 0u64);
    for (k, v) in [0x80u8, 0, 0, 0].into_iter().enumerate() {
        fx.mem.write_u8(COLOUR + k as u32, v);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 1 {
        spawn_task(fx, TINT, 0, [d, 0, p, 0, d], 0x01E0_0D3C);
    }
    if fx.mem.i16(COUNTER) == 0x40 {
        for k in 0..10u32 {
            fx.mem.setf(POINTS + 4 + 0x10 * k, ee::from_i32(-0x1E + 0x11 * k as i32));
        }
        fx.mem.set32(RADII, 0);
        for k in 1..10u32 {
            fx.mem.write_u32(RADII + 4 * k, 0x4040_0000);
        }
        for k in 0..10u32 {
            let v = fx.rand_from(D + 2 * k, 0x01E0_0DB4);
            let (_, h) = fx.div(v, 0x168);
            fx.mem.set16(ANGLES + 2 * k, h);
        }
        for k in 0..3 {
            fx.mem.write_u8(COLOUR + k, 0x20);
        }
        fx.mem.set32(D + 0x250, 0);
        fx.mem.write_u32(D + 0x254, 0x41F0_0000);
        fx.mem.set32(D + 0x258, 0);
        fx.mem.set16(D + 0x552E, 1);
        fx.mem.set32(HEIGHT, 0);
        fx.mem.set16(INDEX, 1);
        fx.mem.set16(REC_ANGLE, 0);
        spawn_trail(fx, [0, 10, p], 0x01E0_0E24);
        s0 = 1;
        s1 = 10;
    }
    if fx.mem.i16(COUNTER) == 1 {
        let h = fx.self_handle();
        unit_position_saving(fx, h, SELF_POS, s0, 0x01E0_0E54);
        let x = fx.mem.read_u32(SELF_POS);
        fx.mem.write_u32(CENTRE, x);
        let y = ee::sub(fx.mem.f(SELF_POS + 4), 25.0);
        fx.mem.setf(CENTRE + 4, y);
        let z = fx.mem.read_u32(SELF_POS + 8);
        fx.mem.write_u32(CENTRE + 8, z);
        let h = fx.self_handle();
        unit_position_saving(fx, h, sp + 0x70, s0, 0x01E0_0E8C);
        let h = fx.target_handle();
        unit_position_saving(fx, h, sp + 0x80, s0, 0x01E0_0EA0);
        let dz = ee::sub(fx.mem.f(sp + 0x88), fx.mem.f(sp + 0x78));
        fx.mem.setf(SLIDE, ee::div(dz, 50.0));
    }
    let c = fx.mem.i16(COUNTER);
    if (0xAA..0xDD).contains(&c) {
        let z = ee::add(fx.mem.f(CENTRE + 8), fx.mem.f(SLIDE));
        fx.mem.setf(CENTRE + 8, z);
    }
    let h = fx.target_handle();
    let slot = fx.mem.i16(TNODE);
    node_position_saving(fx, h, slot, sp + 0x60, s0, Some(s1), 0x01E0_0F04);
    for k in 0..10u32 {
        let a = fx.mem.i16(ANGLES + 2 * k);
        let c = l2::sin_deg(fx, SINES, a, 1);
        let r = fx.mem.f(RADII + 4 * k);
        fx.mem.setf(POINTS + 0x10 * k, ee::mul(r, c));
        let a = fx.mem.i16(ANGLES + 2 * k);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let r = fx.mem.f(RADII + 4 * k);
        fx.mem.setf(POINTS + 8 + 0x10 * k, ee::mul(r, s));
        let a = fx.mem.i16(ANGLES + 2 * k) + 4;
        fx.mem.set16(ANGLES + 2 * k, a);
        let a = fx.mem.i16(ANGLES + 2 * k);
        if a >= 0x168 {
            let (_, h) = fx.div(a, 0x168);
            fx.mem.set16(ANGLES + 2 * k, h);
        }
    }
    let (s0, s1, s2, s3) = (d + 0x14, d + 0x28, d + 0xA0, 10u64);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.fix_residue(y, 0x01E0_101C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    let f = fx.mem.i16(FACING) - 0xB4;
    fx.mem.set16(FACING, f);
    let c = fx.mem.i16(COUNTER);
    if (0x40..0x141).contains(&c) {
        for (f, ra) in [(QUAD, 0x01E0_1074u32), (RING, 0x01E0_10A0)] {
            for k in 0..3 {
                let w = fx.mem.read_u32(CENTRE + 4 * k);
                fx.mem.write_u32(D + 0x250 + 4 * k, w);
            }
            spawn_task(fx, f, 0, [s0, s1, s2, s3, d], ra);
        }
    }
    fx.leave(0xA0);
}

/// 0x1E021A0(D, step, arg): a new task (the first free of 150) with the
/// colour D+0x54B4 (both +0x60 / +0x64), at D+0x250, index D+0x5532 in
/// +0x50 / +0x52; 1, or 0 (frame 0x60: the caller's s0..s4 and `ra`).
fn spawn_task(fx: &mut Fx, step: u32, arg: u32, s: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, s[4]), (0x30, s[3]), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x1208 + 0x70 * k) == 0) else {
        fx.leave(0x60);
        return 0;
    };
    let t = D + 0x11B0 + 0x70 * k;
    fx.bzero(t, 0x70);
    fx.mem.set16(t + 0x58, 1);
    for o in [0x60u32, 0x64] {
        let (r, g, b) = (fx.mem.u8(COLOUR), fx.mem.u8(COLOUR + 1), fx.mem.u8(COLOUR + 2));
        l2::rgb(fx, t + o, r, g, b);
        let a = fx.mem.read_u8(COLOUR + 3);
        fx.mem.write_u8(t + o + 3, a);
    }
    l2::to_i16x4(fx, t, D + 0x250, 0);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x250 + 4 * k);
        fx.mem.write_u32(t + 0x10 + 4 * k, w);
    }
    let v = fx.mem.i16(INDEX);
    fx.mem.set16(t + 0x50, v);
    let v = fx.mem.i16(INDEX);
    fx.mem.set16(t + 0x52, v);
    fx.mem.write_u32(t + 0x40, arg);
    fx.mem.write_u32(t + 0x44, step);
    fx.leave(0x60);
    1
}

/// 0x1E02030(D, ..): a new trail record (the first free of 25) at D+0x250
/// following D+0x54C0 (angle D+0x552C, index D+0x5532, height D+0x54F0);
/// 1, or 0 when all are in use (frame 0x40: the caller's s0..s2, `ra`).
fn spawn_trail(fx: &mut Fx, s: [u64; 3], ra: u32) -> i32 {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, u64::from(ra)), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    let Some(k) = (0..TRAILS).find(|&k| fx.mem.i16(D + 0x358 + 0x98 * k) == 0) else {
        fx.leave(0x40);
        return 0;
    };
    let rec = D + 0x2D0 + 0x98 * k;
    fx.bzero(rec, 0x98);
    fx.mem.set16(rec + 0x88, 1);
    for k in 0..4 {
        let w = fx.mem.read_u32(D + 0x250 + 4 * k);
        fx.mem.write_u32(rec + 0x30 + 4 * k, w);
    }
    let v = fx.mem.i16(REC_ANGLE);
    fx.mem.set16(rec + 0x92, v);
    let prev = fx.mem.read_u32(LAST);
    fx.mem.write_u32(rec + 0x70, prev);
    let v = fx.mem.i16(INDEX);
    fx.mem.set16(rec + 0x8E, v);
    let h = fx.mem.f(HEIGHT);
    fx.mem.setf(rec + 4, h);
    fx.mem.setf(rec + 0x14, ee::sub(h, 18.0));
    let prev = fx.mem.read_u32(LAST);
    fx.mem.write_u32(rec + 0x70, prev);
    if prev != 0 {
        fx.mem.write_u32(prev + 0x74, rec);
    }
    fx.mem.set16(rec + 0x94, 1);
    fx.mem.set16(rec + 0x96, 1);
    fx.mem.write_u32(LAST, rec);
    fx.leave(0x40);
    1
}

/// 0x1E010C0(D, rec): c061's trail step on the ten points D+0x53C0 with
/// grey 0x48 colours, the alpha in by 10 under 0x50 below 50, else out by
/// 14; the next record spawned while the timer is 31 or more (frame 0x90:
/// ra, the runner's s4, the entry's s3 = k, s2 = D + 0x98 k, s1 = 0, s0 =
/// D).
fn trail_step(fx: &mut Fx, rec: u32, k: u32) {
    let sp = fx.enter(0x90);
    let d = u64::from(D);
    fx.saved(sp, &[(0x50, 0x01E0_1D50), (0x40, 0), (0x30, u64::from(k)), (0x20, d + 0x98 * u64::from(k)), (0x10, 0), (0, d)]);
    fx.mem.set32(sp + 0x64, 0);
    l2::rgb(fx, rec + 0x80, 0x48, 0x48, 0x48);
    l2::rgb(fx, rec + 0x84, 0x48, 0x48, 0x48);
    for k in 0..3 {
        fx.mem.set32(rec + 0x30 + 4 * k, 0);
    }
    if crate::exe::ee_lt(fx.mem.f(rec + 4), 50.0) {
        for a in [rec + 0x83, rec + 0x87] {
            let v = fx.mem.u8(a);
            if v < 0x50 {
                fx.mem.set8(a, v + 0xA);
            }
        }
    } else {
        for a in [rec + 0x83, rec + 0x87] {
            let v = fx.mem.u8(a);
            if v >= 0x11 {
                fx.mem.set8(a, v - 0xE);
            }
        }
    }
    let mut s0 = 1i32;
    while s0 < 8 {
        let p = D + 0x10 * s0 as u32;
        if crate::exe::ee_le(fx.mem.f(p + 0x53C4), fx.mem.f(rec + 4)) {
            s0 += 1;
            continue;
        }
        let x = fx.mem.read_u32(POINTS);
        fx.mem.write_u32(sp + 0x60, x);
        let z = fx.mem.read_u32(POINTS + 8);
        fx.mem.write_u32(sp + 0x68, z);
        let b = p + 0x53C0;
        let a = D + 0x10 * (s0 - 1) as u32 + 0x53C0;
        l2::segment_point(fx, sp + 0x70, sp + 0x60, rec, a, b);
        for k in 0..4 {
            let w = fx.mem.read_u32(sp + 0x70 + 4 * k);
            fx.mem.write_u32(rec + 0x40 + 4 * k, w);
        }
        l2::segment_point(fx, sp + 0x80, sp + 0x60, rec, a, b);
        for k in 0..4 {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * k);
            fx.mem.write_u32(rec + 0x50 + 4 * k, w);
        }
        let w = fx.mem.i16(rec + 0x8A);
        if 20 * s0 >= w {
            fx.mem.set16(rec + 0x8A, w + 7);
        }
        let w = fx.mem.i16(rec + 0x8C);
        if 19 * s0 >= w {
            fx.mem.set16(rec + 0x8C, w + 6);
        }
        s0 = 9;
    }
    let head = fx.mem.f(rec + 4);
    if crate::exe::ee_le(head, 80.0) {
        let head = ee::add(head, 4.0);
        fx.mem.setf(rec + 4, head);
        let tail = fx.mem.f(rec + 0x14);
        if !crate::exe::ee_le(ee::sub(head, tail), 18.0) {
            fx.mem.setf(rec + 0x14, ee::add(tail, 4.0));
        }
    } else {
        let tail = ee::add(fx.mem.f(rec + 0x14), 4.0);
        fx.mem.setf(rec + 0x14, tail);
        if !crate::exe::ee_le(tail, fx.mem.f(rec + 4)) {
            let next = fx.mem.read_u32(rec + 0x74);
            if next != 0 {
                fx.mem.set32(next + 0x70, 0);
            }
            fx.mem.set16(rec + 0x88, 0);
        }
    }
    if fx.mem.i16(rec + 0x88) == 1 && !crate::exe::ee_le(fx.mem.f(rec + 0x14), 0.0) {
        fx.mem.write_u32(D + 0x54EC, 0x4160_0000);
        fx.mem.write_u32(LAST, rec);
        fx.mem.set16(D + 0x552E, 1);
        let tail = fx.mem.read_u32(rec + 0x14);
        fx.mem.write_u32(HEIGHT, tail);
        fx.mem.set16(INDEX, 1);
        let a = fx.mem.i16(rec + 0x92) - 0xF;
        fx.mem.set16(REC_ANGLE, a);
        if fx.mem.i32(TIMER) >= 0x1F && spawn_trail(fx, [s0 as u64, u64::from(rec), d], 0x01E0_1408) != 0 {
            fx.mem.set16(rec + 0x88, 2);
        }
    }
    fx.leave(0x90);
}

/// 0x1E022F0(D, rec, desc, pass): c061's trail draw (pass 0 the strip3
/// header and colours, pass 1 the 12 slices) with no turn, scales (2, 0,
/// 2) / (1.4, 0.5) / (1.2, 1) / (0.8, 1.5) at D+0x5460, each slice drawn
/// with texture 5 / 0 over (0, 0, 31, 127) and texture 4 / 15 rect 0x96
/// (frame 0x360: ra, the runner's fp / s7..s4 (0), the entry's s3 = k, s2
/// = D + 0x98 k, s1 = 0, s0 = D).
fn trail_draw(fx: &mut Fx, rec: u32, d: u32, pass: i32, k: u32) {
    let sp = fx.enter(0x360);
    let dd = u64::from(D);
    let ra = if pass == 0 { 0x01E0_1D88 } else { 0x01E0_1D9C };
    fx.saved(sp, &[(0x90, ra), (0x80, 0), (0x70, 0), (0x60, 0), (0x50, 0), (0x40, 0), (0x30, u64::from(k)), (0x20, dd + 0x98 * u64::from(k)), (0x10, 0), (0, dd)]);
    let prev = fx.mem.read_u32(rec + 0x70);
    if pass == 0 {
        l2::desc_strip3(fx, d);
        rgbaq_at(fx, sp + 0x340, rec + 0x80, d + 0x10);
        rgbaq_at(fx, sp + 0x348, rec + 0x80, d + 0x28);
        rgbaq_at(fx, sp + 0x350, rec + 0x84, d + 0x40);
        rgbaq_at(fx, sp + 0x358, rec + 0x84, d + 0x58);
        fx.leave(0x360);
        return;
    }
    let scale = |fx: &mut Fx, xz: u32, y: u32| {
        fx.mem.write_u32(sp + 0xE0, xz);
        fx.mem.write_u32(sp + 0xE4, y);
        fx.mem.write_u32(sp + 0xE8, xz);
    };
    scale(fx, 0x4000_0000, 0);
    let tex = fx.mem.read_u32(D + 0x2C8);
    fx.desc_texture(d, tex, 1);
    l2::rect(fx, sp + 0xF0, 0x8C, 0);
    l2::uv_strip3(fx, d, sp + 0xF0, 0);
    l2::xform_vp(fx, sp + 0x100, sp + 0xE0, 0, CENTRE);
    for (k, (xz, y)) in [(0x3FB3_3333u32, 0x3F00_0000u32), (0x3F99_999A, 0x3F80_0000), (0x3F4C_CCCD, 0x3FC0_0000), (0x3F00_0000, 0x4000_0000)].into_iter().enumerate() {
        scale(fx, xz, y);
        let tmp = sp + 0x240 + 0x40 * k as u32;
        l2::xform_vp(fx, tmp, sp + 0xE0, 0, CENTRE);
        for i in 0..16 {
            let w = fx.mem.read_u32(tmp + 4 * i);
            fx.mem.write_u32(sp + 0x140 + 0x40 * k as u32 + 4 * i, w);
        }
    }
    let mut s1 = 0i32;
    for _ in 0..12 {
        let ring = |fx: &mut Fx, src: u32, width: u32, base: i32, at: [u32; 4]| {
            let a = (base + fx.mem.i16(src + 0x92)) as i16 as i32;
            let c = l2::sin_deg(fx, SINES, a, 1);
            let w = ee::from_i32(fx.mem.i16(src + width));
            fx.mem.setf(sp + at[0], ee::mul(w, c));
            let s = l2::sin_deg(fx, SINES, a, 0);
            let w = ee::from_i32(fx.mem.i16(src + width));
            fx.mem.setf(sp + at[1], ee::mul(w, s));
            let a = (base + fx.mem.i16(src + 0x92) - 0x1E) as i16 as i32;
            let c = l2::sin_deg(fx, SINES, a, 1);
            let w = ee::from_i32(fx.mem.i16(src + width));
            fx.mem.setf(sp + at[2], ee::mul(w, c));
            let s = l2::sin_deg(fx, SINES, a, 0);
            let w = ee::from_i32(fx.mem.i16(src + width));
            fx.mem.setf(sp + at[3], ee::mul(w, s));
        };
        if prev != 0 {
            ring(fx, prev, 0x8C, s1, [0xA0, 0xA8, 0xB0, 0xB8]);
            let tail = fx.mem.read_u32(prev + 0x14);
            fx.mem.write_u32(rec + 4, tail);
            l2::drift(fx, sp + 0xA0, prev + 0x40, rec + 0x40);
        } else {
            ring(fx, rec, 0x8A, s1, [0xA0, 0xA8, 0xB0, 0xB8]);
        }
        ring(fx, rec, 0x8C, s1, [0xC0, 0xC8, 0xD0, 0xD8]);
        let (h, t) = (fx.mem.read_u32(rec + 4), fx.mem.read_u32(rec + 0x14));
        fx.mem.write_u32(sp + 0xA4, h);
        fx.mem.write_u32(sp + 0xB4, h);
        fx.mem.write_u32(sp + 0xC4, t);
        fx.mem.write_u32(sp + 0xD4, t);
        for k in 0..4u32 {
            let a = sp + 0xA4 + 0x10 * k;
            if crate::exe::ee_lt(fx.mem.f(a), 0.0) {
                fx.mem.set32(a, 0);
            }
        }
        l2::add_rows(fx, sp + 0xA0, rec + 0x40);
        l2::drift(fx, sp + 0xC0, rec + 0x40, rec + 0x50);
        for s3 in 0..4u32 {
            let m = sp + 0x100 + 0x40 * s3;
            for pass in 0..2 {
                if pass == 0 {
                    let tex = fx.mem.read_u32(TEX5);
                    fx.desc_texture(d, tex, 1);
                    for (o, v) in [(0xF0u32, 0), (0xF2, 0), (0xF4, 0x1F), (0xF6, 0x7F)] {
                        fx.mem.set16(sp + o, v);
                    }
                } else {
                    let tex = fx.mem.read_u32(D + 0x2CC);
                    fx.desc_texture(d, tex, 1);
                    l2::rect(fx, sp + 0xF0, 0x96, 0);
                }
                l2::uv_strip3(fx, d, sp + 0xF0, 0);
                if l2::project(fx, sp + 0xA0, SPR, m, d, MEAN_W) {
                    let v = fx.mem.f(MEAN_W);
                    fx.fix_residue(v, if pass == 0 { 0x01E0_2910 } else { 0x01E0_297C });
                    let key = ee::fixunssfsi(v) as i32;
                    let ot = fx.mem.read_u32(OTW);
                    fx.submit(d, ot, key);
                }
            }
        }
        s1 = (s1 + 0x1E) as i16 as i32;
    }
    fx.leave(0x360);
}

/// The 8-byte RGBAQ of `c` (0x108020 through `local`) at `to`.
fn rgbaq_at(fx: &mut Fx, local: u32, c: u32, to: u32) {
    l2::rgbaq(fx, local, c);
    let (a, b) = (fx.mem.read_u32(local), fx.mem.read_u32(local + 4));
    fx.mem.write_u32(to, a);
    fx.mem.write_u32(to + 4, b);
}

/// rand % m (HI) from a step's `jal` at `site` (rand's frame: s0 = `t`).
fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    fx.div(v, m).1
}

/// rand % 0x1000 - 0x800 (the record's signed `andi` remainder).
fn spin(fx: &mut Fx, t: u32, site: u32) -> i32 {
    let v = fx.rand_from(t, site + 8);
    v % 0x1000 - 0x800
}

/// 0x1E01440 (D, task; frame 0x20: ra, the entry's s0 = D): a grey
/// textured quad (cell row (rand >> 4) % 3) of size 2 +0xA (4..9, by 2 to
/// 16 every third update, +8 = 2.5 +0xA), +0x48 5..104 off the centre at
/// angle +0x5A (turning by -10..9), turned by random spins (+0..+4 added to
/// +0x20..+0x28 each update), rising by 2; in by 0x40 under 0x80 for 40
/// updates, then out by 9.
fn quad_step(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    let f = QUAD;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x1C, 6);
        fx.mem.set16(t + 0xA, v + 4);
        let w = fx.mem.i16(t + 0xA);
        fx.mem.set16(t + 8, w << 1);
        let v = fx.rand_from(t, f + 0x50);
        let (_, h) = fx.div(v >> 4, 100);
        fx.mem.setf(t + 0x48, ee::add(5.0, ee::from_i32(h)));
        let v = fx.rand_from(t, f + 0x80);
        let (_, h) = fx.div(v >> 4, 3);
        fx.mem.set16(t + 0x56, h);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, f + 0xAC, 100);
        fx.mem.setf(t + 0x34, ee::from_i32(v - 0x32));
        let v = rmod(fx, t, f + 0xD8, 100);
        fx.mem.setf(t + 0x30, ee::from_i32(v - 0x32));
        let y = ee::div(fx.mem.f(t + 0x34), 400.0);
        fx.mem.setf(t + 0x34, y);
        let x = ee::div(fx.mem.f(t + 0x30), 400.0);
        fx.mem.setf(t + 0x30, x);
        let v = rmod(fx, t, f + 0x124, 0x14);
        fx.mem.set16(t + 0x5C, v - 0xA);
        let v = rmod(fx, t, f + 0x144, 0x168);
        fx.mem.set16(t + 0x5A, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), 0, f + 0x168);
        for (o, site) in [(4u32, 0x168u32), (2, 0x188), (0, 0x1A8)] {
            let v = spin(fx, t, f + site);
            fx.mem.set16(t + o, v);
        }
        for (o, site) in [(0x20u32, 0x1C8u32), (0x24, 0x1E4), (0x28, 0x200)] {
            let v = rmod(fx, t, f + site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x28 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let age = fx.mem.i16(t + 0x54);
    if fx.div(age, 3).1 == 1 {
        let w = fx.mem.i16(t + 0xA);
        if w < 0x10 {
            fx.mem.set16(t + 0xA, w + 2);
        }
        let v = ee::mul(2.5, ee::from_i32(fx.mem.i16(t + 0xA)));
        fx.fix_residue(v, f + 0x2C8);
        fx.mem.set16(t + 8, ee::fixsfsi(v));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let y = ee::add(fx.mem.f(t + 0x14), 2.0);
        fx.mem.setf(t + 0x14, y);
    }
    for (o, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + o, v);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    let a = fx.mem.i16(t + 0x5A) + fx.mem.i16(t + 0x5C);
    fx.mem.set16(t + 0x5A, a);
    fx.leave(0x20);
}

/// 0x1E01790 (D, task; frame 0x20: ra, the entry's s0 = D): a white facing
/// ring (size +0x48 100..129 growing by 6 for 140 updates, cell row rand %
/// 8, radius +0x50 100..149 widening by +0xE 36..55 for ages 1..89, angle
/// +0x5A, row +8 0..34); in by 0x2A under 0x80 for +6 (7..16) updates, then
/// out by 9.
fn ring_step(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    let f = RING;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x1C, 0x1E);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x64));
        let v = fx.rand_from(t, f + 0x50);
        fx.mem.set16(t + 0x56, v % 8);
        l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
        let v = rmod(fx, t, f + 0x7C, 0x32);
        fx.mem.set16(t + 0x50, v + 0x64);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, 0x14);
        fx.mem.set16(t + 0xE, v + 0x24);
        let v = rmod(fx, t, f + 0xD8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF4, 0xA);
        fx.mem.set16(t + 6, v + 7);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x2A));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let age = fx.mem.i16(t + 0x54);
    if age < 0x5A && age != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    if fx.mem.i16(t + 0x54) < 0x8C {
        let s = ee::add(fx.mem.f(t + 0x48), 6.0);
        fx.mem.setf(t + 0x48, s);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E01970 (D, task; frame 0x90: ra, the entry's s1 = 0, s0 = D): a
/// window sprite (0x6FF8 .. 0x8FF8) sampling the frame (0x207EF0, filter
/// 1) at ST inset by +0x48 (0) + 20 / 512, colour (0x80, 0x80, 0xB0, alpha
/// +0x63), ALPHA 0x44, key 0; while the timer is 21 or more in by 10 below
/// 0x50 before update 216 and out by 6 above 0x40 after 216, then out by 9.
fn tint(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xB0);
        fx.mem.set32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, c);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for o in [0x50u32, 0x54] {
        let s = ee::add(fx.mem.f(t + 0x48), 20.0);
        fx.mem.setf(sp + o, ee::div(s, 512.0));
    }
    for o in [0x60u32, 0x64] {
        let s = ee::sub(ee::add(512.0, fx.mem.f(t + 0x48)), 20.0);
        fx.mem.setf(sp + o, ee::div(s, 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let c = fx.mem.i16(COUNTER);
    if fx.mem.i32(TIMER) >= 0x15 {
        if c < 0xD8 {
            let a = fx.mem.read_u8(t + 0x63);
            if a < 0x50 {
                fx.mem.write_u8(t + 0x63, a + 0xA);
            }
        }
        if fx.mem.i16(COUNTER) >= 0xD9 {
            let a = fx.mem.read_u8(t + 0x63);
            if a >= 0x41 {
                fx.mem.write_u8(t + 0x63, a - 6);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x90);
}
