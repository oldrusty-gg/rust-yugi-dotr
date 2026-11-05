//! BMODULE b056 (card 402), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02A70, data area a0 = 0x1ED0000, 0x3C10 bytes) [F]
//!: c061's rising-trail program (25 records of
//! 0x98 at D+0x2D0 from update 0xCD, b521.rs's step / draw, spawn 0x1E038A0,
//! grey 0x28, one texture a slice) with b505's older pool of 150 tasks of
//! 0x40 at D+0x11A8 (alive +0x36, step +0x24, spawn 0x1E03A10). Steps: a
//! full-window tint 0x1660 (b505's, on 0xC8); facing wisps 0x1700 / 0x1BD0
//! at the attacker on odd 0xC8..0x168 drifting off the facing; sparks
//! 0x1D80 at nodes 0x37 / 0x3D on 0xD0..0x130; rays 0x18A0 at node 1 on
//! 0x82..0x84 and 0x169..0x176; a fan 0x1AF0 of 18 slivers at node 1 on
//! 0x83..0x117; chips 0x2600 at the nodes on 0x82..0x8F; a centre drift
//! 0x1E30 (the points' radius grows, D+0x38E0 slides over the unit gap);
//! sparks 0x1FC0 and rings 0x2890 at the target on 0x131..0x180; the
//! camera pull 0x21A0 (slots 4 / 5 from D+0x3730) on 0xD0..0x130. Steps
//! 0x1E023E0 / 0x1E02780 are dispatched and drawn but never spawned [F:
//! no spawn names them]; not ported. Returns 1 / 4 / 2 / 6 / 8 on 0xFA /
//! 0x186 / 0x17C / 0x105..0x176 / timer out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b089::dcmp;
use crate::b494::camera_angles_saving;
use crate::blib::{exe::camera_slots45, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TRAILS: u32 = 25;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x24), as the record's addresses.
const TINT: u32 = 0x01E0_1660;
const WISP_A: u32 = 0x01E0_1700;
const RAY: u32 = 0x01E0_18A0;
const FAN: u32 = 0x01E0_1AF0;
const WISP_B: u32 = 0x01E0_1BD0;
const SPARK_A: u32 = 0x01E0_1D80;
const CENTRE_STEP: u32 = 0x01E0_1E30;
const SPARK_B: u32 = 0x01E0_1FC0;
const CAMERA: u32 = 0x01E0_21A0;
const CHIP: u32 = 0x01E0_2600;
const RING: u32 = 0x01E0_2890;
const TEX_CHIP: u32 = D + 0x298;
const TEX_B: u32 = D + 0x2CC;
const CAM: u32 = D + 0x3730;
const SELF_NODES: u32 = D + 0x3760;
const POINTS: u32 = D + 0x3840;
const CENTRE: u32 = D + 0x38E0;
const TEX5: u32 = D + 0x38F0;
const TIMER: u32 = D + 0x3910;
const COLOUR: u32 = D + 0x3914;
const OTW: u32 = D + 0x391C;
const LAST: u32 = D + 0x3920;
const RADII: u32 = D + 0x3924;
const HEIGHT: u32 = D + 0x3950;
const MEAN_W: u32 = D + 0x3954;
const ANGLES: u32 = D + 0x3964;
const SNODE: u32 = D + 0x3978;
const TNODE: u32 = D + 0x397A;
const NODE_A: u32 = D + 0x397C;
const NODE_B: u32 = D + 0x397E;
const FACING: u32 = D + 0x3988;
const REC_ANGLE: u32 = D + 0x398C;
const INDEX: u32 = D + 0x3992;
const COUNTER: u32 = D + 0x3996;
const BYTES: u32 = D + 0x3998;
const SINES: u32 = D + 0x3A98;
/// 1.0 / 0.1 as doubles.
const D1: u64 = 0x3FF0_0000_0000_0000;
const D01: u64 = 0x3FB9_9999_9999_999A;

/// The entry 0x1E02A70 (a0 = D, a1 = param; frame 0x110).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x110);
    fx.mem.write_u32(OTW, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x110);
    ret
}

/// rand % m (HI) from return address `ra` (rand's frame: `s0`).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// 0x1A2B80 on `v` from return address `ra` (its residue).
fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3C10);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    let h = fx.self_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(SNODE, r);
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2B8 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(TEX_CHIP, t);
    let t = fx.texture(5, 0);
    fx.mem.write_u32(TEX5, t);
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TNODE, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x37, 0);
    fx.mem.set16(NODE_A, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x3D, 0);
    fx.mem.set16(NODE_B, r);
    fx.mem.set16(D + 0x3994, param);
    fx.mem.set32(TIMER, 0x19A);
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2BA0, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_2BD4, 0xA);
        fx.mem.set8(BYTES + k, v - 5);
    }
    0
}

/// The three words at `p` + rand % m - `half` (`dy` first added to y) to
/// D+0x250 (`ras`: rand's return addresses).
fn scatter(fx: &mut Fx, p: u32, m: i32, half: f32, dy: Option<f32>, ras: [u32; 3]) {
    for c in 0..3u32 {
        let r = rmod(fx, D, ras[c as usize], m);
        let mut v = fx.mem.f(p + 4 * c);
        if c == 1 {
            if let Some(dy) = dy {
                v = ee::add(dy, v);
            }
        }
        let v = ee::sub(ee::add(v, ee::from_i32(r)), half);
        fx.mem.setf(D + 0x250 + 4 * c, v);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let p = param as i64 as u64;
    let d = u64::from(D);
    emit(fx, p);
    for k in 0..TRAILS {
        let r = D + 0x2D0 + 0x98 * k;
        if fx.mem.i16(r + 0x88) != 0 {
            trail_step(fx, r, k, p);
        }
    }
    for k in 0..TRAILS {
        let r = D + 0x2D0 + 0x98 * k;
        if fx.mem.i16(r + 0x88) != 0 {
            trail_draw(fx, r, sp + 0x80, 0, k, p);
            trail_draw(fx, r, sp + 0x80, 1, k, p);
        }
    }
    let s1 = d + 0x98 * 25;
    if fx.mem.i16(COUNTER) == 0xC8 {
        spawn_task(fx, TINT, [d, s1, 25, p, 0], 0x01E0_2CB4);
    }
    let h = fx.self_handle();
    unit_position_saving(fx, h, sp + 0x60, d, 0x01E0_2CC8);
    let c = fx.mem.i16(COUNTER);
    let (s1, s2) = (u64::from(sp + 0x64), u64::from(sp + 0x68));
    if (0xC8..0x169).contains(&c) && c % 2 == 1 {
        for (f, ras, ra4, ra) in [(WISP_A, [0x01E0_2D10u32, 0x01E0_2D44, 0x01E0_2D8C], 0x01E0_2DC4u32, 0x01E0_2DE8u32), (WISP_B, [0x01E0_2E00, 0x01E0_2E34, 0x01E0_2E78], 0x01E0_2EAC, 0x01E0_2ED0)] {
            scatter(fx, sp + 0x60, 0x3C, 30.0, Some(40.0), ras);
            let a = rmod(fx, D, ra4, 0x168);
            fx.mem.set16(REC_ANGLE, a);
            spawn_task(fx, f, [d, s1, s2, 0, 0], ra);
        }
        let c = fx.mem.i16(COUNTER);
        if (0xD0..0x131).contains(&c) {
            for (n, ras, ra, nra) in [(NODE_A, [0x01E0_2F1Cu32, 0x01E0_2F50, 0x01E0_2F84], 0x01E0_2FC8u32, 0x01E0_2F10u32), (NODE_B, [0x01E0_2FF8, 0x01E0_302C, 0x01E0_3060], 0x01E0_30A4, 0x01E0_2FEC)] {
                let h = fx.self_handle();
                let slot = fx.mem.i16(n);
                node_position_saving(fx, h, slot, sp + 0x60, d, Some(s1), nra);
                scatter(fx, sp + 0x60, 0x28, 20.0, None, ras);
                spawn_task(fx, SPARK_A, [d, s1, s2, 0, 0], ra);
            }
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x131..0x181).contains(&c) {
        for k in 0..2u64 {
            spawn_task(fx, SPARK_B, [d, k, s2, 0, 0], 0x01E0_30E4);
        }
    }
    if fx.mem.i16(COUNTER) == 0xD0 {
        spawn_task(fx, CAMERA, [d, s1, s2, 0, 0], 0x01E0_311C);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x82..=0x84).contains(&c) {
        for k in 0..5 {
            ray_spawn(fx, 0x01E0_3164, 0x01E0_3188, [d, k, s2, 0, 0]);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x169..0x177).contains(&c) && fx.div(c, 3).1 == 1 {
        for k in 0..2 {
            ray_spawn(fx, 0x01E0_31F4, 0x01E0_3218, [d, k, 0, 0, 0]);
        }
    }
    for k in 0..TASKS {
        let t = D + 0x11A8 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let s = [d, d + 0x40 * u64::from(k), u64::from(k)];
        match fx.mem.read_u32(t + 0x24) {
            TINT => tint(fx, t),
            WISP_A => wisp_a(fx, t, s),
            WISP_B => wisp_b(fx, t, s),
            SPARK_A => spark_a(fx, t),
            CAMERA => camera(fx, t, s),
            CENTRE_STEP => centre(fx, t, s),
            SPARK_B => spark_b(fx, t, s),
            RAY => ray(fx, t, s),
            FAN => fan(fx, t, s),
            CHIP => chip(fx, t, s),
            RING => ring(fx, t),
            f => debug_assert!(false, "b056: unknown task step {f:#x}"),
        }
    }
    let dd = sp + 0x80;
    let mut s1 = 0u64;
    for k in 0..TASKS {
        let t = D + 0x11A8 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        if f == TINT {
            s1 += 1;
            let ot = fx.mem.read_u32(OTW);
            l2::colour_strip4(fx, dd, t + 0x28, 0);
            l2::desc_gouraud4(fx, dd, 0);
            l2::fullscreen4(fx, dd);
            fx.submit(dd, ot, 0);
        }
        let s = [d, s1, d + 0x40 * u64::from(k), u64::from(k), 0];
        match f {
            WISP_A => sprite_pair(fx, t, dd, s, [0x01E0_3580, 0x01E0_3594]),
            WISP_B => sprite_pair(fx, t, dd, s, [0x01E0_35C0, 0x01E0_35D4]),
            RAY => sprite_pair(fx, t, dd, s, [0x01E0_3680, 0x01E0_3694]),
            SPARK_A => tenth_pair(fx, t, dd, s, [0x01E0_3600, 0x01E0_3614]),
            SPARK_B => tenth_pair(fx, t, dd, s, [0x01E0_3640, 0x01E0_3654]),
            FAN => {
                fan_draw(fx, t, dd, 0, s, 0x01E0_36C0);
                fan_draw(fx, t, dd, 1, s, 0x01E0_36D4);
            }
            CHIP => chip_draw(fx, t, dd, s),
            RING => {
                let w = fx.mem.read_u32(TEX5);
                fx.mem.write_u32(D + 0x88, w);
                let sub = fx.mem.i16(t + 0x34);
                l2::rect(fx, sp + 0x70, 0x14, sub);
                for k in 0..4 {
                    let v = fx.mem.read_u32(sp + 0x70 + 4 * k);
                    fx.mem.write_u32(D + 0x94 + 4 * k, v);
                }
                for k in 0..3 {
                    let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                    fx.mem.setf(sp + 0x60 + 4 * k, v);
                }
                fx.mem.set16(D + 0xB8, 3);
                fx.mem.set16(D + 0xBE, 0);
                quads4::ring_at(fx, uv::uv_rect, D, t, BYTES, sp + 0x60);
                fx.mem.set16(D + 0xB8, 0);
            }
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xFA => 1,
        0x186 => 4,
        0x17C => 2,
        0x105..=0x176 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// A ray at the node copy D+0x3760 (y 30): the entry's inline spawn.
fn ray_spawn(fx: &mut Fx, ra1: u32, ra2: u32, s: [u64; 5]) {
    let x = fx.mem.read_u32(SELF_NODES);
    fx.mem.write_u32(D + 0x250, x);
    fx.mem.write_u32(D + 0x254, 0x41F0_0000);
    let z = fx.mem.read_u32(SELF_NODES + 8);
    fx.mem.write_u32(D + 0x258, z);
    let a = rmod(fx, D, ra1, 0x168);
    fx.mem.set16(REC_ANGLE, a);
    spawn_task(fx, RAY, s, ra2);
}

/// 0x1E00CE0(D, 0): the update counter and the emitter (frame 0x90: ra,
/// the runner's s4 / the entry's s3 = the parameter, s2 / s1 (0), s0 = D).
fn emit(fx: &mut Fx, p: u64) {
    let sp = fx.enter(0x90);
    let d = u64::from(D);
    fx.saved(sp, &[(0x50, 0x01E0_2C10), (0x40, 0), (0x30, p), (0x20, 0), (0x10, 0), (0, d)]);
    let mut s1 = 0u64;
    for (k, v) in [0x80u8, 0, 0, 0].into_iter().enumerate() {
        fx.mem.write_u8(COLOUR + k as u32, v);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 0xCD {
        for k in 0..10u32 {
            fx.mem.setf(POINTS + 4 + 0x10 * k, ee::from_i32(-0x1E + 0x11 * k as i32));
        }
        fx.mem.set32(RADII, 0);
        for k in 1..10u32 {
            fx.mem.write_u32(RADII + 4 * k, 0x4040_0000);
        }
        for k in 0..10u32 {
            let h = rmod(fx, D, 0x01E0_0D94, 0x168);
            fx.mem.set16(ANGLES + 2 * k, h);
        }
        for k in 0..3 {
            fx.mem.write_u8(COLOUR + k, 0x20);
        }
        fx.mem.set32(D + 0x250, 0);
        fx.mem.write_u32(D + 0x254, 0x41F0_0000);
        fx.mem.set32(D + 0x258, 0);
        fx.mem.set16(D + 0x398E, 1);
        fx.mem.set32(HEIGHT, 0);
        fx.mem.set16(INDEX, 1);
        fx.mem.set16(REC_ANGLE, 0);
        spawn_trail(fx, [d, 0, 10], 0x01E0_0E04);
        spawn_task(fx, CENTRE_STEP, [d, 1, 10, p, 0], 0x01E0_0E24);
        s1 = 1;
    }
    let h = fx.target_handle();
    let slot = fx.mem.i16(TNODE);
    node_position_saving(fx, h, slot, sp + 0x60, d, Some(s1), 0x01E0_0E3C);
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
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x70);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x74)), 180.0);
    let f = fix(fx, y, 0x01E0_0F54);
    fx.mem.set16(FACING, f);
    let f = fx.mem.i16(FACING) - 0xB4;
    fx.mem.set16(FACING, f);
    let h = fx.self_handle();
    let slot = fx.mem.i16(SNODE);
    node_position_saving(fx, h, slot, sp + 0x80, d, Some(d + 0x3760), 0x01E0_0F7C);
    for k in 0..10u32 {
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * c);
            fx.mem.write_u32(SELF_NODES + 0x10 * k + 4 * c, w);
        }
    }
    s1 = d + 0x3800;
    let (s2, s3) = (d + 0x28, d + 0xA0);
    let c = fx.mem.i16(COUNTER);
    if c >= 0x83 && fx.div(c, 5).1 == 1 && c < 0x118 {
        spawn_task(fx, FAN, [d, s1, s2, s3, 10], 0x01E0_0FF8);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x82..0x90).contains(&c) {
        let (a1, a2) = (u64::from(sp + 0x64), u64::from(sp + 0x68));
        for k in 0..5u64 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODE_A);
            node_position_saving(fx, h, slot, sp + 0x60, d, Some(if k == 0 { s1 } else { a1 }), 0x01E0_1030);
            scatter(fx, sp + 0x60, 0x14, 10.0, None, [0x01E0_1038, 0x01E0_106C, 0x01E0_10A4]);
            spawn_task(fx, CHIP, [d, a1, a2, k, 10], 0x01E0_10EC);
        }
        for k in 0..5u64 {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODE_B);
            node_position_saving(fx, h, slot, sp + 0x60, d, Some(a1), 0x01E0_1118);
            let r = rmod(fx, D, 0x01E0_1120, 0x14);
            let v = ee::sub(ee::add(fx.mem.f(sp + 0x60), ee::from_i32(r)), 10.0);
            fx.mem.setf(D + 0x250, v);
            let r = rmod(fx, D, 0x01E0_1154, 0xA);
            let v = ee::sub(fx.mem.f(sp + 0x64), ee::from_i32(r));
            fx.mem.setf(D + 0x254, v);
            let r = rmod(fx, D, 0x01E0_1184, 0x14);
            let v = ee::sub(ee::add(fx.mem.f(sp + 0x68), ee::from_i32(r)), 10.0);
            fx.mem.setf(D + 0x258, v);
            spawn_task(fx, CHIP, [d, a1, a2, k, 10], 0x01E0_11C8);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x131..0x181).contains(&c) {
        let h = fx.target_handle();
        unit_position_saving(fx, h, sp + 0x60, d, 0x01E0_1208);
        scatter(fx, sp + 0x60, 0x14, 10.0, None, [0x01E0_1210, 0x01E0_1244, 0x01E0_1278]);
        spawn_task(fx, RING, [d, s1, s2, s3, 10], 0x01E0_12BC);
    }
    fx.leave(0x90);
}

/// 0x1E03A10(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour D+0x3914 (twice), +0x3A = D+0x398C,
/// +0x30 = D+0x3992; 1, or 0 (frame 0x60: the caller's s0..s4 and `ra`).
fn spawn_task(fx: &mut Fx, f: u32, s: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, s[4]), (0x30, s[3]), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x11DE + 0x40 * k) == 0) {
        Some(k) => {
            let t = D + 0x11A8 + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for c in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            let a = fx.mem.i16(REC_ANGLE);
            fx.mem.set16(t + 0x3A, a);
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// A step's frame: the entry's s0.. `s` (`n` of them), then ra.
fn step_frame(fx: &mut Fx, size: u32, s: [u64; 3], n: u32, ra: u32) -> u32 {
    let sp = fx.enter(size);
    for k in 0..n {
        fx.saved(sp + 0x10 * k, &[(0, s[k as usize])]);
    }
    fx.saved(sp + 0x10 * n, &[(0, u64::from(ra))]);
    sp
}

/// The drift both wisps share: off the facing by 5 sin / cos, up by 2 cos
/// +0x3A, +0x3A by 10.
fn wisp_drift(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    let s2 = fx.mem.i16(FACING);
    let s = l2::sin_deg(fx, SINES, s2, 0);
    let v = ee::sub(ee::from_i32(fx.mem.i16(t)), ee::mul(5.0, s));
    let v = fix(fx, v, ras[0]);
    fx.mem.set16(t, v);
    let a = fx.mem.i16(t + 0x3A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let v = ee::add(ee::from_i32(fx.mem.i16(t + 2)), ee::mul(2.0, c));
    let v = fix(fx, v, ras[1]);
    fx.mem.set16(t + 2, v);
    let c = l2::sin_deg(fx, SINES, s2, 1);
    let v = ee::sub(ee::from_i32(fx.mem.i16(t + 4)), ee::mul(5.0, c));
    let v = fix(fx, v, ras[2]);
    fx.mem.set16(t + 4, v);
    let a = fx.mem.i16(t + 0x3A) + 0xA;
    fx.mem.set16(t + 0x3A, a);
}

/// 0x1E01700: size 0x96, cell +0x34 up to 7 every fourth update, grey
/// 0x3F; in by 2 under 0x30 for 20 updates, out by 4.
fn wisp_a(fx: &mut Fx, t: u32, s: [u64; 3]) {
    step_frame(fx, 0x40, s, 3, 0x01E0_32F8);
    fx.mem.set16(t + 0x38, 0x96);
    let c = fx.mem.i16(COUNTER);
    if c % 4 == 1 && fx.mem.i16(t + 0x34) < 7 {
        let v = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, v);
    }
    l2::rgb(fx, t + 0x28, 0x3F, 0x3F, 0x3F);
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 0x14 {
        if a < 0x30 {
            fx.mem.write_u8(t + 0x2B, a + 2);
        }
    } else if a >= 6 {
        fx.mem.write_u8(t + 0x2B, a - 4);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    wisp_drift(fx, t, [0x01E0_1804, 0x01E0_183C, 0x01E0_1874]);
    fx.leave(0x40);
}

/// 0x1E01BD0: yellow (0x40 / 0x20) by the update's parity, size 13 / 9; in
/// by 5 under 0x70 for 20 updates, out by 13.
fn wisp_b(fx: &mut Fx, t: u32, s: [u64; 3]) {
    step_frame(fx, 0x40, s, 3, 0x01E0_3318);
    if fx.mem.i16(COUNTER) % 2 == 1 {
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0);
        fx.mem.set16(t + 0x38, 0xD);
    } else {
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0);
        fx.mem.set16(t + 0x38, 9);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 0x14 {
        if a < 0x70 {
            fx.mem.write_u8(t + 0x2B, a + 5);
        }
    } else if a >= 0xF {
        fx.mem.write_u8(t + 0x2B, a - 0xD);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    wisp_drift(fx, t, [0x01E0_1CDC, 0x01E0_1D14, 0x01E0_1D4C]);
    fx.leave(0x40);
}

/// 0x1E018A0: a ray of grey 0x80 / 0xA0 / 0xC0 at random, cell rand % 7,
/// width +0x38 growing every fourth update to 20, speed +0xE 5..22 along
/// its angle +0x3A (offsets +8 / +0xC, sinking by +0xE / 2); out by 3
/// after 20 updates.
fn ray(fx: &mut Fx, t: u32, s: [u64; 3]) {
    step_frame(fx, 0x40, s, 3, 0x01E0_3460);
    fx.mem.set16(t + 0x38, 0x96);
    if fx.mem.i16(t + 0x36) == 1 {
        let r = rmod(fx, t, 0x01E0_18D8, 3);
        let g = [0x80, 0xA0, 0xC0][r as usize];
        l2::rgb(fx, t + 0x28, g, g, g);
        let v = rmod(fx, t, 0x01E0_1958, 7);
        fx.mem.set16(t + 0x34, v);
        fx.mem.set16(t + 0x38, 0);
        let v = rmod(fx, t, 0x01E0_1978, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.write_u8(t + 0x2B, 0x30);
        let v = rmod(fx, t, 0x01E0_199C, 0x12);
        fx.mem.set16(t + 0xE, v + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    let age = fx.mem.i16(t + 0x32);
    if age < 0x14 {
        let w = fx.mem.i16(t + 0x38);
        if w < 0x14 && age % 4 == 1 {
            fx.mem.set16(t + 0x38, w + 1);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 5 {
            fx.mem.write_u8(t + 0x2B, a - 3);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    let a = fx.mem.i16(t + 0x3A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let e = ee::from_i32(fx.mem.i16(t + 0xE));
    let v = ee::sub(ee::from_i32(fx.mem.i16(t + 8)), ee::mul(e, s));
    let v = fix(fx, v, 0x01E0_1A70);
    fx.mem.set16(t + 8, v);
    let a = fx.mem.i16(t + 0x3A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let e = ee::from_i32(fx.mem.i16(t + 0xE));
    let v = ee::sub(ee::from_i32(fx.mem.i16(t + 0xC)), ee::mul(e, c));
    let v = fix(fx, v, 0x01E0_1AA8);
    fx.mem.set16(t + 0xC, v);
    let v = fx.mem.i16(t + 0xC) - (fx.mem.i16(t + 0xE) >> 1);
    fx.mem.set16(t + 0xC, v);
    fx.leave(0x40);
}

/// 0x1E01AF0: a fan at the node copy (y 30), grey 0x40, opening by 5 to
/// 0x64 (+0x38), turning by 12; at 0x80, out by 16 from size 0x1F.
fn fan(fx: &mut Fx, t: u32, s: [u64; 3]) {
    step_frame(fx, 0x30, s, 2, 0x01E0_3480);
    l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x40);
    let v = fx.mem.f(SELF_NODES);
    let v = fix(fx, v, 0x01E0_1B24);
    fx.mem.set16(t, v);
    fx.mem.set16(t + 2, 0x1E);
    let v = fx.mem.f(SELF_NODES + 8);
    let v = fix(fx, v, 0x01E0_1B38);
    fx.mem.set16(t + 4, v);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0x80);
    }
    if fx.mem.i16(t + 0x38) >= 0x1F {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0x10);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let w = fx.mem.i16(t + 0x38);
    if w < 0x64 {
        fx.mem.set16(t + 0x38, w + 5);
    }
    let a = fx.mem.i16(t + 0x3A) + 0xC;
    fx.mem.set16(t + 0x3A, a);
    fx.leave(0x30);
}

/// 0x1E01D80 (inlined in the entry 0x1E03328): a yellow spark, size 15; in
/// by 0x16 under 0x70 for 5 updates, then out by 13 growing by 7.
fn spark_a(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x40, 0x40, 0);
    fx.mem.set16(t + 0x38, 0xF);
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 5 {
        if a < 0x70 {
            fx.mem.write_u8(t + 0x2B, a + 0x16);
        }
    } else {
        if a >= 0xF {
            fx.mem.write_u8(t + 0x2B, a - 0xD);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
        let v = fx.mem.i16(t + 0x38) + 7;
        fx.mem.set16(t + 0x38, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    let a = fx.mem.i16(t + 0x3A) + 0xA;
    fx.mem.set16(t + 0x3A, a);
}

/// 0x1E01E30: the centre D+0x38E0 from the attacker (20 down) slid a 110th
/// of the units' gap a update in x / z until within 3 of the target in z;
/// the first radius grows by 1 to 20 every fourth update meanwhile.
fn centre(fx: &mut Fx, t: u32, s: [u64; 3]) {
    let sp = step_frame(fx, 0x60, s, 3, 0x01E0_3400);
    let h = fx.self_handle();
    unit_position_saving(fx, h, sp + 0x40, s[0], 0x01E0_1E5C);
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, u64::from(sp + 0x50), 0x01E0_1E74);
    if fx.mem.i16(t + 0x36) == 1 {
        let x = fx.mem.read_u32(sp + 0x40);
        fx.mem.write_u32(CENTRE, x);
        let y = ee::sub(fx.mem.f(sp + 0x44), 20.0);
        fx.mem.setf(CENTRE + 4, y);
        let z = fx.mem.read_u32(sp + 0x48);
        fx.mem.write_u32(CENTRE + 8, z);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x36) == 2 {
        let dx = ee::div(ee::sub(fx.mem.f(sp + 0x50), fx.mem.f(sp + 0x40)), 110.0);
        let x = ee::add(fx.mem.f(CENTRE), dx);
        fx.mem.setf(CENTRE, x);
        let dz = ee::div(ee::sub(fx.mem.f(sp + 0x58), fx.mem.f(sp + 0x48)), 110.0);
        let z = ee::add(fx.mem.f(CENTRE + 8), dz);
        fx.mem.setf(CENTRE + 8, z);
        let g = ee::sub(fx.mem.f(sp + 0x58), fx.mem.f(CENTRE + 8));
        let r = fx.sqrtf(ee::mul(g, g));
        if ee_lt(r, 3.0) {
            fx.mem.set16(t + 0x36, 3);
        }
        if fx.mem.i16(t + 0x32) % 4 == 1 {
            let v = fx.mem.f(RADII);
            if ee_lt(v, 20.0) {
                fx.mem.setf(RADII, ee::add(v, 1.0));
            }
        }
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x60);
}

/// 0x1E01FC0: a yellow spark within 40 of the target, rising by 2, size
/// 15; in by 0x38 under 0x70 for 2 updates, then out by 13 growing by 7.
fn spark_b(fx: &mut Fx, t: u32, s: [u64; 3]) {
    let sp = step_frame(fx, 0x50, s, 2, 0x01E0_3420);
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x40, u64::from(t), 0x01E0_1FE8);
    if fx.mem.i16(t + 0x36) == 1 {
        for (c, ras) in [(0u32, [0x01E0_2000u32, 0x01E0_2030]), (1, [0x01E0_2038, 0x01E0_2068]), (2, [0x01E0_2070, 0x01E0_20A0])] {
            let r = rmod(fx, t, ras[0], 0x50);
            let v = ee::sub(ee::add(fx.mem.f(sp + 0x40 + 4 * c), ee::from_i32(r)), 40.0);
            let v = fix(fx, v, ras[1]);
            fx.mem.set16(t + 2 * c, v);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0);
    } else {
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0);
    }
    fx.mem.set16(t + 0x38, 0xF);
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 2 {
        if a < 0x70 {
            fx.mem.write_u8(t + 0x2B, a + 0x38);
        }
    } else {
        if a >= 0xF {
            fx.mem.write_u8(t + 0x2B, a - 0xD);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
        let v = fx.mem.i16(t + 0x38) + 7;
        fx.mem.set16(t + 0x38, v);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    let v = fx.mem.i16(t + 2) + 2;
    fx.mem.set16(t + 2, v);
    let a = fx.mem.i16(t + 0x3A) + 0xA;
    fx.mem.set16(t + 0x3A, a);
    fx.leave(0x50);
}

/// 0x1E021A0: camera slots 4 / 5 from D+0x3730 on the attacker: the pull
/// (+0x30 = 0) to 128 (+12 / +3, 110 out, D+0x3758 up by 0.1 to 1), then
/// (+0x30 = 1) back down (-12 / -3, by 0.1 to 0.1); off at update 0x131.
fn camera(fx: &mut Fx, t: u32, s: [u64; 3]) {
    let sp = step_frame(fx, 0x40, s, 2, 0x01E0_33E0);
    let h = fx.self_handle();
    unit_position_saving(fx, h, sp + 0x30, u64::from(t), 0x01E0_21C8);
    if fx.mem.i16(t + 0x36) == 1 {
        for k in 0..4 {
            fx.mem.set32(CAM + 4 * k, 0);
        }
        fx.mem.set32(CAM + 0x28, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let x = fx.mem.read_u32(sp + 0x30);
    fx.mem.write_u32(CAM + 0x10, x);
    fx.mem.set32(CAM + 0x14, 0);
    let z = fx.mem.read_u32(sp + 0x38);
    fx.mem.write_u32(CAM + 0x18, z);
    if fx.mem.i16(t + 0x30) == 0 {
        let v = fx.mem.f(CAM);
        if ee_lt(v, 128.0) {
            fx.mem.setf(CAM, ee::add(v, 12.0));
        }
        let v = fx.mem.f(CAM + 4);
        if ee_lt(v, 128.0) {
            fx.mem.setf(CAM + 4, ee::add(v, 3.0));
        }
        fx.mem.set32(CAM + 0x20, 0);
        fx.mem.write_u32(CAM + 0x24, 0x42DC_0000);
        let v = fx.mem.f(CAM + 0x28);
        if dcmp(fx, v, D1, t, 0x01E0_2290, 0x01E0_22A0, false) {
            let r = ee::add(fx.mem.f(CAM + 0x28), f32::from_bits(0x3DCC_CCCD));
            fx.mem.setf(CAM + 0x28, r);
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else {
        let v = fx.mem.f(CAM);
        if !ee_le(v, 20.0) {
            fx.mem.setf(CAM, ee::sub(v, 12.0));
        }
        let v = fx.mem.f(CAM + 4);
        if !ee_le(v, 9.0) {
            fx.mem.setf(CAM + 4, ee::sub(v, 3.0));
        }
        let v = fx.mem.f(CAM + 0x28);
        if dcmp(fx, v, D01, t, 0x01E0_2348, 0x01E0_2368, true) {
            let r = ee::sub(fx.mem.f(CAM + 0x28), f32::from_bits(0x3DCC_CCCD));
            fx.mem.setf(CAM + 0x28, r);
        } else {
            fx.mem.set16(t + 0x30, 0);
        }
    }
    if fx.mem.i16(COUNTER) == 0x131 {
        camera_slots45(fx, 0);
        fx.mem.set16(t + 0x36, 0);
    } else {
        camera_slots45(fx, CAM);
    }
    fx.leave(0x40);
}

/// 0x1E02600: a chip (size 8, cell rand % 7, grey 0x80), 11 sin / cos of
/// +0x3A into +8 / +0xC, +0xA falling by 2; at 0x80 for 7 updates, then
/// out by 20.
fn chip(fx: &mut Fx, t: u32, s: [u64; 3]) {
    step_frame(fx, 0x30, s, 2, 0x01E0_34A0);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 8);
        let v = rmod(fx, t, 0x01E0_2630, 7);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x34, v);
        let v = rmod(fx, t, 0x01E0_265C, 0x168);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, 0x01E0_2678, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x16 {
            fx.mem.write_u8(t + 0x2B, a - 0x14);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let a = fx.mem.i16(t + 0x3A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let v = ee::add(ee::from_i32(fx.mem.i16(t + 8)), ee::mul(11.0, s));
    let v = fix(fx, v, 0x01E0_2708);
    fx.mem.set16(t + 8, v);
    let v = fx.mem.i16(t + 0xA) - 2;
    fx.mem.set16(t + 0xA, v);
    let a = fx.mem.i16(t + 0x3A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let v = ee::add(ee::from_i32(fx.mem.i16(t + 0xC)), ee::mul(11.0, c));
    let v = fix(fx, v, 0x01E0_274C);
    fx.mem.set16(t + 0xC, v);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x30);
}

/// 0x1E02890: a ring (size 7..26, grey 0xF0, radius 150..799, angle +0x3A
/// turning by 9, +0xE 65..124, row +8 0..29, life +6 3..7, cell to 6); in
/// by 0x19 under 0x80 for +6 updates, then out by 20; up by 1..4 a update.
fn ring(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_34E0), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x36) == 1 {
        let v = rmod(fx, t, 0x01E0_28B4, 0x14);
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
        fx.mem.set16(t + 0x38, v + 7);
        let v = rmod(fx, t, 0x01E0_28E4, 0x28A);
        fx.mem.set16(t + 0x30, v + 0x96);
        let v = rmod(fx, t, 0x01E0_2904, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, 0x01E0_2920, 0x3C);
        fx.mem.set16(t + 0xE, v + 0x41);
        let v = rmod(fx, t, 0x01E0_2940, 0x1E);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_295C, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a + 0x19);
        }
    } else if a >= 0x16 {
        fx.mem.write_u8(t + 0x2B, a - 0x14);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 && fx.mem.i16(t + 0x34) < 6 {
        let c = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, c);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.rand_from(t, 0x01E0_2A0C);
        let y = fx.mem.i16(t + 2) + (v % 4 + 1);
        fx.mem.set16(t + 2, y);
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    let a = fx.mem.i16(t + 0x3A) + 9;
    fx.mem.set16(t + 0x3A, a);
    fx.leave(0x20);
}

/// 0x1E01660 (inlined in the entry 0x1E03250): b505's tint: alpha +0x2B up
/// by 10 to 0xA0 (+0x30 = 0), down by 10 (+0x30 = 1) until below 12, when
/// +0x30 is 0 again (and from update 0x15F the task ends); (0x5F, 0x3F,
/// 0x3F).
fn tint(fx: &mut Fx, t: u32) {
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x30) == 0 {
        if a < 0xA0 {
            fx.mem.write_u8(t + 0x2B, a + 0xA);
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else if a >= 0xC {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    } else {
        if fx.mem.i16(COUNTER) >= 0x15F {
            fx.mem.set16(t + 0x36, 0);
        }
        fx.mem.set16(t + 0x30, 0);
    }
    l2::rgb(fx, t + 0x28, 0x5F, 0x3F, 0x3F);
}

fn draw_frame(fx: &mut Fx, size: u32, s: [u64; 5], ra: u32) -> u32 {
    let sp = fx.enter(size);
    for k in 0..5 {
        fx.saved(sp + 0x10 * k, &[(0, s[k as usize])]);
    }
    fx.saved(sp + 0x50, &[(0, u64::from(ra))]);
    sp
}

fn sprite_pair(fx: &mut Fx, t: u32, d: u32, s: [u64; 5], ras: [u32; 2]) {
    sprite(fx, t, d, 0, s, ras[0]);
    sprite(fx, t, d, 1, s, ras[1]);
}

fn tenth_pair(fx: &mut Fx, t: u32, d: u32, s: [u64; 5], ras: [u32; 2]) {
    tenth(fx, t, d, 0, s, ras[0]);
    tenth(fx, t, d, 1, s, ras[1]);
}

/// 0x1E041D0(D, task, desc, mode): a camera-facing quad, scale 0.1, of
/// half size i16 +0x38 (a ray: 200 by +0x38 at the position + offsets /
/// 10), sprite 0x14 texture D+0x298 (wisp B: sprite 0x9B, texture
/// D+0x2CC).
fn sprite(fx: &mut Fx, t: u32, d: u32, mode: i32, s: [u64; 5], ra: u32) {
    let sp = draw_frame(fx, 0x120, s, ra);
    let f = fx.mem.read_u32(t + 0x24);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xC0, if f == WISP_B { 0x9B } else { 0x14 }, c);
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x28, 0);
        l2::uv_strip3(fx, d, sp + 0xC0, 0);
        let w = fx.mem.read_u32(if f == WISP_B { TEX_B } else { TEX_CHIP });
        fx.desc_texture(d, w, 1);
    } else {
        l2::camera_angles_to(fx, sp + 0x110);
        if f == RAY {
            for k in 0..3u32 {
                let off = fx.mem.i16(t + 8 + 2 * k);
                fx.mult(0x6666_6667, off);
                let hi = fx.hilo().1;
                let q = (hi >> 2).wrapping_add(((off as u32) >> 31) as i32);
                let v = fx.mem.i16(t + 2 * k).wrapping_add(q);
                fx.mem.setf(sp + 0xA0 + 4 * k, ee::from_i32(v));
            }
        } else {
            for k in 0..3u32 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                fx.mem.setf(sp + 0xA0 + 4 * k, v);
            }
        }
        for k in 0..3 {
            fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
        }
        l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x110, sp + 0xA0);
        let h = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, if f == RAY { 200.0 } else { h }, h);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xD0, OT);
    }
    fx.leave(0x120);
}

/// 0x1E047D0 (b089's sprite_p, offsets / 10): a camera-facing quad of
/// half size i16 +0x38 scaled 0.1, sprite 0x9B, texture D+0x2CC.
fn tenth(fx: &mut Fx, t: u32, d: u32, mode: i32, s: [u64; 5], ra: u32) {
    let sp = draw_frame(fx, 0x120, s, ra);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xC0, 0x9B, c);
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x28, 0);
        l2::uv_strip3(fx, d, sp + 0xC0, 0);
        let w = fx.mem.read_u32(TEX_B);
        fx.desc_texture(d, w, 1);
    } else {
        l2::camera_angles_to(fx, sp + 0x110);
        for c in 0..3u32 {
            let off = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * c)), 10.0);
            let v = ee::add(ee::from_i32(fx.mem.i16(t + 2 * c)), off);
            fx.mem.setf(sp + 0xA0 + 4 * c, v);
        }
        for c in 0..3u32 {
            fx.mem.write_u32(sp + 0xB0 + 4 * c, 0x3DCC_CCCD);
        }
        l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x110, sp + 0xA0);
        let v = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, v, v);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xD0, OT);
    }
    fx.leave(0x120);
}

/// 0x1E04470(D, task, desc, mode): 18 flat slivers (8 values, prim 0x14,
/// ALPHA 0x48) round the task's point, inner radius i16 +0x38 - 2, outer
/// +0x38, angle +0x3A + 20 k, keyed by their mean w.
fn fan_draw(fx: &mut Fx, t: u32, d: u32, mode: i32, s: [u64; 5], ra: u32) {
    let sp = fx.enter(0x120);
    for k in 0..5 {
        fx.saved(sp + 0x10 * k, &[(0, s[k as usize])]);
    }
    fx.saved(sp + 0x50, &[(0, 0), (0x10, u64::from(ra))]);
    if mode == 0 {
        fx.mem.write_u16(d + 2, 0x14);
        fx.mem.write_u16(d, 8);
        fx.mem.write_u64(d + 8, 0x4141_4141);
        fx.mem.write_u32(d + 4, 0);
        fx.desc_alpha(d, 0x80_0000_0048);
        for o in [0x44u32, 0x34, 0x24, 0x14] {
            fx.mem.write_u32(d + o, 0x3F80_0000);
        }
        for o in [0x1Fu32, 0x2F, 0x3F, 0x4F] {
            fx.mem.write_u8(d + o, 0);
        }
        for (o, l) in [(0x10u32, 0x100u32), (0x20, 0x108), (0x30, 0x110), (0x40, 0x118)] {
            l2::rgbaq(fx, sp + l, t + 0x28);
            for k in 0..2 {
                let w = fx.mem.read_u32(sp + l + 4 * k);
                fx.mem.write_u32(d + o + 4 * k, w);
            }
        }
    } else {
        for k in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0xB0 + 4 * k, v);
        }
        l2::xform_vp(fx, sp + 0xC0, 0, 0, sp + 0xB0);
        for o in [0x74u32, 0x84, 0x94, 0xA4] {
            fx.mem.write_u32(sp + o, 0);
        }
        let mut s1 = 0i32;
        for _ in 0..0x12 {
            for (add, outer, inner) in [(0, 0x90u32, 0x70u32), (0x14, 0xA0, 0x80)] {
                let a = (fx.mem.i16(t + 0x3A) + add + s1) as i16 as i32;
                let c = l2::sin_deg(fx, SINES, a, 1);
                let r = ee::from_i32(fx.mem.i16(t + 0x38));
                fx.mem.setf(sp + outer, ee::mul(r, c));
                let sn = l2::sin_deg(fx, SINES, a, 0);
                let r = ee::from_i32(fx.mem.i16(t + 0x38));
                fx.mem.setf(sp + outer + 8, ee::mul(r, sn));
                let c = l2::sin_deg(fx, SINES, a, 1);
                let r = ee::from_i32(fx.mem.i16(t + 0x38) - 2);
                fx.mem.setf(sp + inner, ee::mul(r, c));
                let sn = l2::sin_deg(fx, SINES, a, 0);
                let r = ee::from_i32(fx.mem.i16(t + 0x38) - 2);
                fx.mem.setf(sp + inner + 8, ee::mul(r, sn));
            }
            if l2::project(fx, sp + 0x70, SPR, sp + 0xC0, d, MEAN_W) {
                let w = fx.mem.f(MEAN_W);
                fx.fix_residue(w, 0x01E0_477C);
                let key = ee::fixunssfsi(w) as i32;
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
            s1 = (s1 + 0x14) as i16 as i32;
        }
    }
    fx.leave(0x120);
}

/// 0x1E04980(D, t, d, 0) (b294's chip_draw): one textured square (9
/// values, FST, ALPHA 0x48, flags 0x824) of the i16 size +0x38: cell 0x14
/// frame +0x34, colour +0x28, texture D+0x38F0, at the task's i16 point
/// plus its drift over 10, scale 0.1, turned by the camera; keyed by its
/// mean w.
fn chip_draw(fx: &mut Fx, t: u32, d: u32, s: [u64; 5]) {
    let sp = fx.enter(0x110);
    fx.saved(sp, &[(0x30, 0x01E0_3700), (0x20, s[2]), (0x10, s[1]), (0, s[0])]);
    let sub = fx.mem.i16(t + 0x34);
    l2::rect(fx, sp + 0xA0, 0x14, sub);
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u16(d + 2, 0x824);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    l2::rgbaq(fx, sp + 0x108, t + 0x28);
    for k in 0..2 {
        let w = fx.mem.read_u32(sp + 0x108 + 4 * k);
        fx.mem.write_u32(d + 0x10 + 4 * k, w);
    }
    uv::uv_rect4(fx, d, sp + 0xA0, 0);
    let tex = fx.mem.read_u32(TEX5);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x90 + 4 * k, 0x3DCC_CCCD);
    }
    camera_angles_saving(fx, sp + 0xF0, u64::from(d), u64::from(t), 0x01E0_4A74);
    for k in 0..3 {
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        fx.mem.setf(sp + 0x80 + 4 * k, ee::add(p, o));
    }
    l2::xform_vp(fx, sp + 0xB0, sp + 0x90, sp + 0xF0, sp + 0x80);
    let v = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x40, v, v);
    if l2::project(fx, sp + 0x40, SPR, sp + 0xB0, d, MEAN_W) {
        let w = fx.mem.f(MEAN_W);
        fx.fix_residue(w, 0x01E0_4B48);
        let key = ee::fixunssfsi(w) as i32;
        let ot = fx.mem.read_u32(OTW);
        fx.submit(d, ot, key);
    }
    fx.leave(0x110);
}

/// 0x1E038A0(D, ..): a new trail record (the first free of 25) at D+0x250
/// following D+0x3920 (angle D+0x398C, index D+0x3992, height D+0x3950);
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

/// 0x1E012E0(D, rec): c061's trail step on the ten points D+0x3840 with
/// grey 0x28 colours, the alpha in by 10 under 0x50 below 50, else out by
/// 14; the next record spawned while the timer is 31 or more (frame 0x90:
/// ra, the runner's s4, the entry's s3 = the parameter, s2 = k, s1 = D +
/// 0x98 k, s0 = D).
fn trail_step(fx: &mut Fx, rec: u32, k: u32, p: u64) {
    let sp = fx.enter(0x90);
    let d = u64::from(D);
    fx.saved(sp, &[(0x50, 0x01E0_2C30), (0x40, 0), (0x30, p), (0x20, u64::from(k)), (0x10, d + 0x98 * u64::from(k)), (0, d)]);
    fx.mem.set32(sp + 0x64, 0);
    l2::rgb(fx, rec + 0x80, 0x28, 0x28, 0x28);
    l2::rgb(fx, rec + 0x84, 0x28, 0x28, 0x28);
    for k in 0..3 {
        fx.mem.set32(rec + 0x30 + 4 * k, 0);
    }
    if ee_lt(fx.mem.f(rec + 4), 50.0) {
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
        if ee_le(fx.mem.f(p + 0x3844), fx.mem.f(rec + 4)) {
            s0 += 1;
            continue;
        }
        let x = fx.mem.read_u32(POINTS);
        fx.mem.write_u32(sp + 0x60, x);
        let z = fx.mem.read_u32(POINTS + 8);
        fx.mem.write_u32(sp + 0x68, z);
        let b = p + 0x3840;
        let a = D + 0x10 * (s0 - 1) as u32 + 0x3840;
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
    if ee_le(head, 80.0) {
        let head = ee::add(head, 4.0);
        fx.mem.setf(rec + 4, head);
        let tail = fx.mem.f(rec + 0x14);
        if !ee_le(ee::sub(head, tail), 18.0) {
            fx.mem.setf(rec + 0x14, ee::add(tail, 4.0));
        }
    } else {
        let tail = ee::add(fx.mem.f(rec + 0x14), 4.0);
        fx.mem.setf(rec + 0x14, tail);
        if !ee_le(tail, fx.mem.f(rec + 4)) {
            let next = fx.mem.read_u32(rec + 0x74);
            if next != 0 {
                fx.mem.set32(next + 0x70, 0);
            }
            fx.mem.set16(rec + 0x88, 0);
        }
    }
    if fx.mem.i16(rec + 0x88) == 1 && !ee_le(fx.mem.f(rec + 0x14), 0.0) {
        fx.mem.write_u32(D + 0x394C, 0x4160_0000);
        fx.mem.write_u32(LAST, rec);
        fx.mem.set16(D + 0x398E, 1);
        let tail = fx.mem.read_u32(rec + 0x14);
        fx.mem.write_u32(HEIGHT, tail);
        fx.mem.set16(INDEX, 1);
        let a = fx.mem.i16(rec + 0x92) - 0xF;
        fx.mem.set16(REC_ANGLE, a);
        if fx.mem.i32(TIMER) >= 0x1F && spawn_trail(fx, [s0 as u64, u64::from(rec), d], 0x01E0_1628) != 0 {
            fx.mem.set16(rec + 0x88, 2);
        }
    }
    fx.leave(0x90);
}

/// 0x1E03B40(D, rec, desc, pass): c061's trail draw (pass 0 the strip3
/// header and colours, pass 1 the 12 slices) with no turn, scales (2, 0,
/// 2) / (1.4, 0.5) / (1.2, 1) / (0.8, 1.5) at D+0x38E0, each slice drawn
/// once with texture 4 / 14 rect 0x8C (frame 0x360: ra, the runner's fp /
/// s7..s5, s4 (0), the entry's s3 = the parameter, s2 = k, s1 = D + 0x98
/// k, s0 = D).
fn trail_draw(fx: &mut Fx, rec: u32, d: u32, pass: i32, k: u32, p: u64) {
    let sp = fx.enter(0x360);
    let dd = u64::from(D);
    let ra = if pass == 0 { 0x01E0_2C68 } else { 0x01E0_2C7C };
    fx.saved(sp, &[(0x90, ra), (0x80, 0), (0x70, 0), (0x60, 0), (0x50, 0), (0x40, 0), (0x30, p), (0x20, u64::from(k)), (0x10, dd + 0x98 * u64::from(k)), (0, dd)]);
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
            if ee_lt(fx.mem.f(a), 0.0) {
                fx.mem.set32(a, 0);
            }
        }
        l2::add_rows(fx, sp + 0xA0, rec + 0x40);
        l2::drift(fx, sp + 0xC0, rec + 0x40, rec + 0x50);
        for s3 in 0..4u32 {
            let m = sp + 0x100 + 0x40 * s3;
            let tex = fx.mem.read_u32(D + 0x2C8);
            fx.desc_texture(d, tex, 1);
            l2::rect(fx, sp + 0xF0, 0x8C, 0);
            l2::uv_strip3(fx, d, sp + 0xF0, 0);
            if l2::project(fx, sp + 0xA0, SPR, m, d, MEAN_W) {
                let v = fx.mem.f(MEAN_W);
                fx.fix_residue(v, 0x01E0_415C);
                let key = ee::fixunssfsi(v) as i32;
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
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
