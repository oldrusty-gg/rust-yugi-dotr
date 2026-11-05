//! BMODULE b577 (card 93), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01ED0, data area 0x1ED0000) [F]: b232's program (150 tasks of 0x70 at +0x2A0, alive +0x58, step
//! +0x44 through `jalr`) with its globals from +0x44B0 up 0x5A0 higher
//! (timer +0x4A50 = 420, colour +0x4A54, OT +0x4A5C, the camera's +0x20 /
//! +0x30 (0x1FACE0 / 0x1FAF40) at +0x4A6C / +0x4A70, nodes +0x4A90..,
//! cells +0x4AB0, angle flag +0x4AFC, counter +0x4B06, sines +0x4D08) and
//! its own emitter 0x1E00A40: every update the node point +0x4470 at the
//! attacker's node 0x1C, pulled halfway (x) to node 0x1F and 5 along z by
//! the facing; update 1 the stage light (b232's); 0x2D the frame quad
//! (b232's, out once the timer is under 0x15); 0x6E ten rings of 18 strips
//! (0x1E00E40, b232's ring drawn at a scale of 0.3, the first three with
//! the angle flag 0 / 1 / 2, from 2 a random turn) round the halfway node
//! point; 0xC8 a violet tint (0x1E01490, `quads4::tint4` at 5 x the camera
//! offset) and four arcs (0x1E01590 / 0x1E01100, `arc_strips_flat`); 0xCE
//! a violet trail (0x1E01680: a 70-point history D+0x4480 of the head
//! D+0x48E0 drawn as the untextured gouraud tube
//! `tubetrail::tube_gouraud(GOURAUD2)`, 8 points growing to 0x44) and a
//! widening arc (0x1E018C0, drawn at the head and the node point); odd
//! 0xFC..0x17C arcs (0x1E01250); 0xCA..0x17C at node 1 (y 42) and odd
//! updates a spark ring (0x1E01360, `quads4::sparks`), 0xE2..0x12C the
//! point at node 1 rising by 10. The flash task 0x1E01C90 is never spawned
//! (only counted, as b232 counts it; no unit flash is drawn). Returns 1 /
//! 4 / 2 / 6 / 8 on 0xE2 / 0x195 / 0x18B / 0xED..0x185 / timer out.
use crate::blib::{billboard, quads4, spawn as bspawn, tubetrail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0x96;
const RINGS: u32 = 0x01E0_0E40;
const ARC_C: u32 = 0x01E0_1100;
const ARC_A: u32 = 0x01E0_1250;
const SPARKS: u32 = 0x01E0_1360;
const TINT: u32 = 0x01E0_1490;
const ARC_B: u32 = 0x01E0_1590;
const TRAIL: u32 = 0x01E0_1680;
const ARC_D: u32 = 0x01E0_18C0;
const LIGHT: u32 = 0x01E0_1A00;
const FRAME: u32 = 0x01E0_1AE0;
const FLASH: u32 = 0x01E0_1C90;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_218C;
const TIMER: u32 = D + 0x4A50;
const COLOUR: u32 = D + 0x4A54;
const OT_AT: u32 = D + 0x4A5C;
const EYE: u32 = D + 0x4A6C;
const AT: u32 = D + 0x4A70;
const TINT_W: u32 = D + 0x4A7C;
const NODE: u32 = D + 0x4A90;
const RECTS: u32 = D + 0x4AB0;
const ANGLE_FLAG: u32 = D + 0x4AFC;
const FACING: u32 = D + 0x4B00;
const FLASHES: u32 = D + 0x4B04;
const COUNTER: u32 = D + 0x4B06;
const SINES: u32 = D + 0x4D08;
const POINT: u32 = D + 0x4470;
const HEAD: u32 = D + 0x48E0;

/// The entry 0x1E01ED0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let e = fx.camera_eye(0);
    fx.mem.write_u32(EYE, e);
    let a = fx.camera_at30(0);
    fx.mem.write_u32(AT, a);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4E80);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4B02, param);
    fx.mem.set32(TIMER, 0x1A4);
    for (k, n) in [1, 0x1C, 0x1F, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0x4AA0, r);
    for (k, id) in [0xEu32, 0xF].into_iter().enumerate() {
        let tex = fx.texture(4, id as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k as u32, tex);
    }
    for k in 0..3u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x4A30 + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(v, 0x01E0_2084);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(k, 0x01E0_20AC);
        let (_, h) = fx.div(r, 0x1E);
        fx.mem.write_u8(D + 0x4B08 + k, (h - 0xF) as u8);
    }
    for k in 0..0x1Eu32 {
        let r = fx.rand_from(k, 0x01E0_20F4);
        fx.mem.set16(RECTS + 2 * k, r.wrapping_rem(8));
    }
    0
}

/// The sparks' draw head: the whole 32 x 64 cell, the task's position in
/// D+0x20..+0x28.
fn sparks_head(fx: &mut Fx, t: u32) {
    for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x1F), (0x9A, 0x3F), (0xB8, 0)] {
        fx.mem.set16(D + o, v);
    }
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(D + 0x20 + 4 * k, w);
    }
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn arc(fx: &mut Fx, t: u32, b8: i32, pos: u32) {
    fx.mem.set16(D + 0xB8, b8);
    billboard::arc_strips_flat(fx, D, t, 0x21, pos);
    fx.mem.set16(D + 0xB8, 0);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.mem.set16(FLASHES, 0);
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 {
            continue;
        }
        if f == FLASH {
            let n = fx.mem.i16(FLASHES) + 1;
            fx.mem.set16(FLASHES, n);
            continue;
        }
        run_task(fx, f, slot + 0x2A0);
    }
    let mut live = 0u64;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        live = u64::from((live as i32 + 1) as i16 as u16);
        let f = fx.mem.read_u32(t + 0x44);
        if f == RINGS {
            let s1 = Some(u64::from(D));
            for (n, out, ra) in [(0x4A92u32, sp + 0x70, 0x01E0_2218), (0x4A92, sp + 0x80, 0x01E0_2230), (0x4A94, sp + 0x90, 0x01E0_2248)] {
                let h = fx.self_handle();
                let s = fx.mem.i16(D + n);
                crate::b076::node_position_saving(fx, h, s, out, live, s1, ra);
            }
            let x = ee::div(ee::sub(fx.mem.f(sp + 0x94), fx.mem.f(sp + 0x84)), 2.0);
            fx.mem.setf(sp + 0x84, x);
            let x = ee::add(fx.mem.f(sp + 0x74), x);
            fx.mem.setf(sp + 0x74, x);
            along(fx, sp + 0x78);
            let w = fx.mem.read_u32(D + 0x4A34);
            fx.mem.write_u32(D + 0x88, w);
            let frame = fx.mem.i16(t + 0x56);
            l2::rect(fx, sp + 0xA0, 0x14, frame);
            for c in 0..4 {
                let v = fx.mem.read_u32(sp + 0xA0 + 4 * c);
                fx.mem.write_u32(D + 0x94 + 4 * c, v);
            }
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            ring(fx, t, 0x20, sp + 0x70);
        }
        if f == ARC_A {
            copy3(fx, sp + 0x70, D + 0x4480);
            arc(fx, t, 0x24, sp + 0x70);
        }
        if f == SPARKS {
            let w = fx.mem.read_u32(D + 0x4A38);
            fx.mem.write_u32(D + 0x88, w);
            sparks_head(fx, t);
            fx.mem.set16(D + 0xB8, 0x18);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == TINT {
            let (e, a) = (fx.mem.read_u32(EYE), fx.mem.read_u32(AT));
            for c in 0..3 {
                let p = fx.mem.f(e + 4 * c);
                let q = fx.mem.f(a + 4 * c);
                fx.mem.setf(sp + 0x70 + 4 * c, ee::add(p, ee::mul(5.0, ee::sub(q, p))));
            }
            let v = [fx.mem.f(sp + 0x70), fx.mem.f(sp + 0x74), fx.mem.f(sp + 0x78)];
            let mut xy = fx.mem.read_u64(sp + 0xD8);
            if let Some(w) = fx.project(v, None, &mut xy) {
                fx.mem.write_u64(sp + 0xD8, xy);
                fx.mem.setf(TINT_W, w);
            }
            crate::b590::project_residue(fx, sp, sp + 0x70);
            let w = fx.mem.f(TINT_W);
            quads4::tint4(fx, t, 0x11, w);
        }
        if f == ARC_B {
            arc(fx, t, 0x24, POINT);
        }
        if f == ARC_C {
            arc(fx, t, 0x12, POINT);
        }
        if f == TRAIL {
            fx.mem.set16(D + 0xB8, 0x12);
            for o in [0x20u32, 0x24, 0x28] {
                fx.mem.write_u32(D + o, 0);
            }
            tubetrail::tube_gouraud(fx, &tubetrail::GOURAUD2, D, t, D + 0x4480, 0);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f == ARC_D {
            copy3(fx, sp + 0x70, D + 0x4480);
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips_flat(fx, D, t, 0x21, sp + 0x70);
            billboard::arc_strips_flat(fx, D, t, 0x21, POINT);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xE2 => 1,
        0x195 => 4,
        0x18B => 2,
        0xED..=0x185 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn run_task(fx: &mut Fx, f: u32, t: u32) {
    match f {
        RINGS => rings(fx, t),
        ARC_C => arc_c(fx, t),
        ARC_A => arc_a(fx, t),
        SPARKS => sparks(fx, t),
        TINT => tint(fx, t),
        ARC_B => arc_b(fx, t),
        TRAIL => trail(fx, t),
        ARC_D => arc_d(fx, t),
        LIGHT => light(fx, t),
        FRAME => frame(fx, t),
        _ => panic!("b577: unknown task step {f:#x}"),
    }
}

/// 0x1E00A40(D, 0): the update counter +0x4B06, the node point and the
/// spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x10, 0x01E0_214C), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let dd = Some(u64::from(D));
    for (node, out, ra) in [(0x4A92u32, POINT, 0x01E0_0A70), (0x4A92, sp + 0x20, 0x01E0_0A88), (0x4A94, sp + 0x30, 0x01E0_0AA0)] {
        let h = fx.self_handle();
        let s = fx.mem.i16(D + node);
        crate::b076::node_position_saving(fx, h, s, out, u64::from(D), dd, ra);
    }
    let x = ee::div(ee::sub(fx.mem.f(sp + 0x34), fx.mem.f(sp + 0x24)), 2.0);
    fx.mem.setf(sp + 0x24, x);
    let x = ee::add(fx.mem.f(D + 0x4474), x);
    fx.mem.setf(D + 0x4474, x);
    along(fx, D + 0x4478);
    let n = fx.mem.i16(COUNTER);
    if n == 1 {
        spawn(fx, LIGHT, param, 0x01E0_0B38);
    }
    if fx.mem.i16(COUNTER) == 0x6E {
        fx.mem.set16(ANGLE_FLAG, 0);
        spawn(fx, RINGS, param, 0x01E0_0B60);
        fx.mem.set16(ANGLE_FLAG, 1);
        spawn(fx, RINGS, param, 0x01E0_0B7C);
        fx.mem.set16(ANGLE_FLAG, 2);
        for ra in [0x01E0_0B98u32, 0x0BAC, 0x0BC0, 0x0BD4, 0x0BE8, 0x0BFC, 0x0C10, 0x0C24] {
            spawn(fx, RINGS, param, 0x01E0_0000 | (ra & 0xFFFF));
        }
    }
    if fx.mem.i16(COUNTER) == 0x2D {
        spawn(fx, FRAME, param, 0x01E0_0C48);
    }
    if fx.mem.i16(COUNTER) == 0xC8 {
        spawn(fx, TINT, param, 0x01E0_0C6C);
        spawn(fx, ARC_B, param, 0x01E0_0C80);
        spawn(fx, ARC_B, param, 0x01E0_0C94);
        spawn(fx, ARC_C, param, 0x01E0_0CA8);
        spawn(fx, ARC_C, param, 0x01E0_0CBC);
    }
    if fx.mem.i16(COUNTER) == 0xCE {
        spawn(fx, TRAIL, param, 0x01E0_0CE0);
        spawn(fx, ARC_D, param, 0x01E0_0CF4);
    }
    let n = fx.mem.i16(COUNTER);
    if (0xFC..0x17D).contains(&n) && odd(n) {
        spawn(fx, ARC_A, param, 0x01E0_0D44);
    }
    let n = fx.mem.i16(COUNTER);
    if (0xCA..0x17D).contains(&n) {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE);
        crate::b076::node_position_saving(fx, h, s, D + 0x250, u64::from(D), dd, 0x01E0_0D78);
        let h = fx.self_handle();
        fx.unit_position(h, D + 0x250);
        // 0x1F71A0's frame (its saved ra, s0 = D).
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_0D8C), (0, u64::from(D))]);
        fx.mem.write_u32(D + 0x254, 0x4228_0000);
        if odd(fx.mem.i16(COUNTER)) {
            spawn(fx, SPARKS, param, 0x01E0_0DCC);
        }
        let v = fx.mem.i16(D + 0x4AF6) + 3;
        fx.mem.set16(D + 0x4AF6, v);
    }
    let n = fx.mem.i16(COUNTER);
    if (0xE2..0x12D).contains(&n) {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE);
        crate::b076::node_position_saving(fx, h, s, D + 0x250, u64::from(D), dd, 0x01E0_0E0C);
        fx.mem.write_u32(D + 0x258, 0);
        let y = ee::add(fx.mem.f(D + 0x254), 10.0);
        fx.mem.setf(D + 0x254, y);
    }
    fx.leave(0x40);
}

/// 0x1E02680(D, step, 0): the first free task cleared and set off at
/// D+0x250 with the colour +0x4A54 (twice), angle flag +0x4AFC; the
/// caller's s0..s4 (D, D, param, 0, 0) and `ra` in its frame.
fn spawn(fx: &mut Fx, f: u32, param: i32, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    let d = u64::from(D);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, param as i64 as u64), (0x10, d), (0, d)]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
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
            l2::to_i16x4(fx, t, D + 0x250, 0);
            // 0x107390's `sq ra` upper half (0) (b232's).
            fx.mem.write_u64(sp - 8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(ANGLE_FLAG);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// rand % m (HI) from a step's rand returning to `ra` (s0 = the task).
fn rand_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// The alpha byte `a` up by `up.1` while below `up.0` when `rising`, else
/// down by `down.1` while at least `down.0` (the task ending below when
/// `ends`).
fn fade(fx: &mut Fx, t: u32, a: u32, rising: bool, up: (u8, u8), down: (u8, u8), ends: bool) {
    let v = fx.mem.read_u8(t + a);
    if rising {
        if v < up.0 {
            fx.mem.write_u8(t + a, v.wrapping_add(up.1));
        }
    } else if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else if ends {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The z at `a` 5 along the attacker's facing (down when +0x4B00).
fn along(fx: &mut Fx, a: u32) {
    let z = fx.mem.f(a);
    let back = fx.mem.i16(FACING) != 0;
    fx.mem.setf(a, if back { ee::sub(z, 5.0) } else { ee::add(z, 5.0) });
}

fn odd(v: i32) -> bool {
    v.wrapping_rem(2) == 1
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The steps' frames: 0x20 (ra +0x10, s0 = 0) or 0x30 (ra +0x20, s1 = D
/// +0x10, s0 = 0).
fn enter_step(fx: &mut Fx, frame: u32) -> u32 {
    let sp = fx.enter(frame);
    if frame == 0x20 {
        fx.saved(sp, &[(0x10, TASK_RA), (0, 0)]);
    } else {
        fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
    }
    sp
}

/// 0x1E00E40: b232's ring (cell frame 0..6, row +8 4..6, phase +0xE; with
/// the angle flag 2 a random turn +0x20..+0x28) fading in by 0xC until
/// update 0x14F then out by 9, flickering grey; radius +0x48 up to 20 by
/// 0.5 (inner +0x4C a fifth of it), its turn +0x5C 20 sin(+0xE), +0xE + 5
/// and +0x5A + 10 a update.
fn rings(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_mod(fx, t, 0x01E0_0E6C, 7);
        fx.mem.set16(t + 0x56, v);
        fx.mem.set16(t + 0x58, 2);
        let v = rand_mod(fx, t, 0x01E0_0E90, 3);
        fx.mem.set16(t + 8, v + 4);
        let v = rand_mod(fx, t, 0x01E0_0EB0, 0x168);
        fx.mem.set16(t + 0xE, v);
        if fx.mem.i16(t + 0x50) >= 2 {
            for (o, ra) in [(0x20u32, 0x01E0_0EE0), (0x24, 0x01E0_0EFC), (0x28, 0x01E0_0F18)] {
                let v = rand_mod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
        }
        fx.mem.set16(t + 0x5C, 0);
        fx.mem.set16(t + 0x5A, 0);
    }
    let rising = fx.mem.i16(COUNTER) < 0x14F;
    fade(fx, t, 0x63, rising, (0x80, 0xC), (0xB, 9), true);
    fade(fx, t, 0x67, rising, (0x80, 0xC), (0xB, 9), false);
    let g = if odd(fx.mem.i16(t + 0x54)) { 0x80 } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    l2::rgb(fx, t + 0x64, g, g, g);
    if fx.mem.i16(COUNTER) != 0 {
        let r = fx.mem.f(t + 0x48);
        if ee_lt(r, 20.0) {
            fx.mem.setf(t + 0x48, ee::add(r, 0.5));
        }
        let inner = ee::mul(f32::from_bits(0x3E4C_CCCD), fx.mem.f(t + 0x48));
        fx.mem.setf(t + 0x4C, inner);
        if ee_lt(inner, 0.0) {
            fx.mem.write_u32(t + 0x4C, 0);
        }
    }
    let a = fx.mem.i16(t + 0xE);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let v = ee::mul(20.0, s);
    fx.fix_residue(v, 0x01E0_10C0);
    fx.mem.set16(t + 0x5C, ee::fixsfsi(v));
    let e = fx.mem.i16(t + 0xE) + 5;
    fx.mem.set16(t + 0xE, e);
    let y = fx.mem.i16(t + 0x5A) + 0xA;
    fx.mem.set16(t + 0x5A, y);
    age(fx, t);
    fx.leave(0x30);
}

/// The arcs' first update: radius `r` (inner 0), white / violet
/// (0x80, 0x20, 0xFF), +0x5A / +0x5C 10, the outer alpha 0.
fn arc_start(fx: &mut Fx, t: u32, r: u32) {
    fx.mem.write_u32(t + 0x48, r);
    fx.mem.write_u32(t + 0x4C, 0);
    l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    l2::rgb(fx, t + 0x64, 0x80, 0x20, 0xFF);
}

/// In by 0x40 below 0xC0 while `rising`, else out by 0xB (ending under 0xD).
fn arc_fade(fx: &mut Fx, t: u32, rising: bool) {
    fade(fx, t, 0x63, rising, (0xC0, 0x40), (0xD, 0xB), true);
}

/// 0x1E01100: an arc of radius 120 turned at random (+0x20..+0x28 rand %
/// 0xFFFF), +0x5A 20 / +0x5C 3, in for 0x16 updates, growing by 3.
fn arc_c(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        arc_start(fx, t, 0x42F0_0000);
        for (o, ra) in [(0x20u32, 0x01E0_1158), (0x24, 0x01E0_1174), (0x28, 0x01E0_1190)] {
            let v = rand_mod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x5A, 0x14);
        fx.mem.set16(t + 0x5C, 3);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x16;
    arc_fade(fx, t, rising);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x48), 3.0);
        fx.mem.setf(t + 0x48, r);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01250: an arc of radius 40 (+0x20 0x4000) at 0xC0 for 5 updates,
/// then out by 0xB; radius + 30 a update, inner 40 less.
fn arc_a(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        arc_start(fx, t, 0x4220_0000);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 5 {
        if fx.mem.read_u8(t + 0x63) < 0xC0 {
            fx.mem.write_u8(t + 0x63, 0xC0);
        }
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0xD, 0xB), true);
    }
    let r = ee::add(fx.mem.f(t + 0x48), 30.0);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, 40.0));
    fx.mem.set32(t + 0x20, 0x4000);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01360: b232's spark ring (radius 200, rising by 95 a update, white,
/// bright for 0xD updates, then fading by 0xE), the inner radius 400 less
/// held at 0.
fn sparks(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.write_u32(t + 0x34, 0x446D_8000);
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xD {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0x10, 0xE), true);
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    let inner = ee::sub(r, 400.0);
    fx.mem.setf(t + 0x4C, inner);
    if ee_lt(inner, 0.0) {
        fx.mem.write_u32(t + 0x4C, 0);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01490: the violet tint (0xF0 grey over (0x80, 0x10, 0xFF)) in by
/// 0x19 to 0x80 for 0x16 updates (+6), then out by 4.
fn tint(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0xFF);
        fx.mem.set16(t + 6, 0x16);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade(fx, t, 0x63, true, (0x80, 0x19), (0, 0), false);
        fade(fx, t, 0x67, true, (0x80, 0x19), (0, 0), false);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (6, 4), true);
        let v = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if v >= 6 { v - 4 } else { 0 });
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01590: an arc of radius 75 facing the camera (+0x20 the camera's
/// angle + 0x4000), in for 0x16 updates.
fn arc_b(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        arc_start(fx, t, 0x4296_0000);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x16;
    arc_fade(fx, t, rising);
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1650);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01680: the trail: 8 points (to 0x44) of the 70-point history
/// D+0x4480 of the head D+0x48E0 (pushed for 0x32 updates; its z 5 along
/// the facing for updates 1..0x21), half width 30; 0xC0 until update 0x181,
/// then out by 9 and the width down by 1; white / 0xC0 grey by age over (0x40, 0x10,
/// 0x80).
fn trail(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x63, 0);
        for k in 0..0x45u32 {
            copy3(fx, D + 0x4480 + 0x10 * k, POINT);
        }
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
        for k in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(HEAD + 4 * k, w);
        }
        fx.mem.write_u32(t + 0x48, 0x41F0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let n = fx.mem.i16(t + 6);
    if n < 0x44 {
        fx.mem.set16(t + 6, n + 1);
    }
    if fx.mem.i16(COUNTER) < 0x181 {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0xB, 9), true);
        let w = fx.mem.f(t + 0x48);
        if !(w <= 0.0) {
            fx.mem.setf(t + 0x48, ee::sub(w, 1.0));
        }
    }
    let g = if odd(fx.mem.i16(t + 0x54)) { 0xC0 } else { 0xFF };
    l2::rgb(fx, t + 0x60, g, g, g);
    if fx.mem.i16(t + 0x54) < 0x32 {
        bspawn::history(fx, D + 0x48D0, 0x46, HEAD);
    }
    let a = fx.mem.i16(t + 0x54);
    if a != 0 && a < 0x22 {
        along(fx, HEAD + 8);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E018C0: an arc of radius 250 facing the camera, in by 0x40 until
/// update 0x188, from 0x143 growing by 20 below 250.
fn arc_d(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        arc_start(fx, t, 0x437A_0000);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(COUNTER) < 0x188;
    arc_fade(fx, t, rising);
    if fx.mem.i16(COUNTER) >= 0x143 {
        let r = fx.mem.f(t + 0x48);
        if ee_lt(r, 250.0) {
            fx.mem.setf(t + 0x48, ee::add(r, 20.0));
        }
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_19CC);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E01A00: b232's stage light on the timer +0x4A50.
fn light(fx: &mut Fx, t: u32) {
    enter_step(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x54) < 0x97 {
        fx.mem.set16(t + 0x50, if v < 0x31 { 0x30 } else { v - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
    }
    age(fx, t);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x20);
}

/// 0x1E01AE0: b232's frame quad in a 0x90 frame (locals 0x10 higher),
/// fading out once the timer is under 0x15.
fn frame(fx: &mut Fx, t: u32) {
    let sp = enter_step(fx, 0x90);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.mem.read_u8(t + 0x63);
    for (k, v) in [0x90u8, 0x90, 0x78, a].into_iter().enumerate() {
        fx.mem.write_u8(sp + 0x48 + k as u32, v);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, xy) in [(0x58u32, 0x6FF8u16), (0x68, 0x8FF8)] {
        fx.mem.write_u16(sp + at, xy);
        fx.mem.write_u16(sp + at + 2, xy);
        let v = fx.mem.read_u64(sp + at);
        fx.mem.write_u64(sp + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(sp + at + 7, 0);
    }
    for (o, v) in [(0x50u32, 0u32), (0x54, 0), (0x60, 0x3F80_0000), (0x64, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, v);
    }
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [0x226, 0x26]);
    fx.mem.write_u64(sp - 0x28, 0);
    fx.submit(d, OT, 0);
    let rising = fx.mem.i32(TIMER) >= 0x15;
    fade(fx, t, 0x63, rising, (0x50, 0xA), (8, 6), true);
    age(fx, t);
    fx.leave(0x90);
}

/// 0x1E027C0(D, t, mode, pos): b232's ring of 18 strips in a 0x220 frame
/// (the descriptor at sp + 0x190, the angles at sp + 0x170, the cell at sp
/// + 0x180, w at sp + 0x160) at a scale of 0.3 (0x1097D0's scale sp +
/// 0x110, the matrix at sp + 0x120), cells +0x4AB0, sines +0x4D08.
fn ring(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x220);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    let d = sp + 0x190;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 1);
    l2::colour_strip3(fx, d, t + 0x64, 2);
    let o = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, o);
    let rh2 = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, rh2, 1);
    let b = mode >> 4;
    let alpha = if b == 0 { 0x80_0000_0042 } else { 0x80_0000_0040 | ((b as i64 as u64) << 2) };
    fx.desc_alpha(d, alpha);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * k);
        fx.mem.write_u32(sp + 0x110 + 4 * k, 0x3E99_999A);
        fx.mem.write_u32(sp + 0x100 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x20, sp + 0x100);
    for o in [0xC4u32, 0xD4, 0xE4, 0xF4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let (r_in, r_out) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let a = fx.mem.i16(t + 0x5A);
    fx.mem.set32(sp + 0x174, a);
    let a = fx.mem.i16(t + 0x5C);
    fx.mem.set32(sp + 0x178, a);
    let mut s3 = 0i32;
    for k in 0..0x12u32 {
        if k != 0 {
            for o in [0xC4u32, 0xD4, 0xE4, 0xF4] {
                fx.mem.write_u32(sp + o, 0);
            }
        }
        let frame = fx.mem.i16(RECTS + 2 * k);
        l2::rect(fx, sp + 0x180, 0x14, frame);
        for c in 0..4 {
            let w = fx.mem.read_u32(sp + 0x180 + 4 * c);
            fx.mem.write_u32(D + 0x94 + 4 * c, w);
        }
        let o = fx.mem.i16(D + 0xBE);
        l2::uv_strip3(fx, d, D + 0x94, o);
        for (deg, inner, outer) in [(s3 - 0xC8, 0xE0u32, 0xC0u32), (s3 - 0xB4, 0xF0, 0xD0)] {
            let deg = deg as i16 as i32;
            let c = l2::sin_deg(fx, SINES, deg, 1);
            let s = l2::sin_deg(fx, SINES, deg, 0);
            fx.mem.setf(sp + inner, ee::mul(c, r_in));
            fx.mem.setf(sp + inner + 8, ee::mul(s, r_in));
            fx.mem.setf(sp + outer, ee::mul(c, r_out));
            fx.mem.setf(sp + outer + 8, ee::mul(s, r_out));
        }
        l2::rotate4(fx, sp + 0xC0, sp + 0x170, 2, SINES);
        l2::rotate4(fx, sp + 0xC0, sp + 0x170, 0, SINES);
        if l2::project(fx, sp + 0xC0, 0x7000_0000, sp + 0x120, d, sp + 0x160) {
            let w = fx.mem.f(sp + 0x160);
            fx.fix_residue(w, 0x01E0_2AA4);
            let key = ee::fixunssfsi(w) as i32;
            let ot = fx.mem.read_u32(OT_AT);
            fx.submit(d, ot, key);
        }
        s3 = (s3 + 0x14) as i16 as i32;
    }
    fx.leave(0x220);
}
