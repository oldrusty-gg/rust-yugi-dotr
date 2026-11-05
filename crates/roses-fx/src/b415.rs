//! BMODULE b415 (card 493), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02F70, data area 0x1ED0000, 0xCF30 bytes) [F]
//!: b292's program shape (card 185: 400 tasks of
//! 0x70 at D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E02F60 the
//! idle step; spawned by 0x1E03770, b292's spawn with the kind D+0xCBB4 /
//! angle D+0xCBA6) with the steps of b482 / b641 / b247b / b646 / b292 at
//! other constants. Nodes: the attacker's 0x11 / 0x12 / 0xD / 0x30 / 0x2F /
//! 0x2C (D+0xCB48..), the target's 1 (D+0xCB58); timer 0xBE.
//!
//! On 0x70 the attacker's node 0xD into D+0xCAD0; on 0x70..0x78 two sparks
//! a update there (0x1E020D0, b482's with the 0x80 alpha for 15 updates;
//! kind 0 pink / 1 violet, `quads4::sparks`); on 0x70 a red arc fan
//! (0x1E022B0, b641's with (0xFF, 0x40, 0x40) inside, `arc_strips`), a red
//! / orange tint (0x1E02900, b646's in by 0x19, `quads4::tint4` keyed by a
//! point 50 in front of the target), a dark red frame tint (0x1E02CD0,
//! b292's with ALPHA 0x48, (0x80, 0, 0) / (0, 0, 0x80) by kind, in by 0x20
//! under 0x60 for 6 updates) and fifty sparkles (0x1E023A0: b631's spark
//! without its child, flying by rand / 80, turned by the camera, drawn by
//! `quads4::quad_turned` on cell 0x9B with the row +0x56 << 5); on 0x87 the
//! same fan / tint (blue) / frame tint (blue) at node 0x11 and four columns
//! (0x1E026F0, b247b's word for word, `quads4::sprite_quads` at the task's
//! point) at angles 0 / 90 / 45 / 135; on 0x7C a blue ribbon task
//! (0x1E02A90, b292's with the nodes 0x12 / 0x11 and the out from 0x8F)
//! drawn by 0x1E03910 (b703's ribbon in a frame 0x10 higher, the `b` side's
//! alphas written as bytes). Returns 1 on 0x66, 4 on 0xB9, 2 on 0xA0, 6 on
//! 0x71..0x9A, 8 once the timer D+0xCB10 has run out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::b590::project_residue;
use crate::blib::{billboard, quads4, spawn as hist, spline, uv};
use crate::bus::Bus;
use crate::c061::rgbaq_at;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const SPARK: u32 = 0x01E0_20D0;
const FAN: u32 = 0x01E0_22B0;
const SPARKLE: u32 = 0x01E0_23A0;
const COLUMN: u32 = 0x01E0_26F0;
const TINT4: u32 = 0x01E0_2900;
const TRAILS: u32 = 0x01E0_2A90;
const FRAME_TINT: u32 = 0x01E0_2CD0;
const IDLE: u32 = 0x01E0_2F60;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3280;
/// The two node histories (20 points of 16 bytes).
const HIST: [u32; 2] = [D + 0xB1D0, D + 0xB310];
const AIM: u32 = D + 0xCAD0;
const TEX: u32 = D + 0xCAF0;
const TIMER: u32 = D + 0xCB10;
const COLOUR: u32 = D + 0xCB14;
const TINT_W: u32 = D + 0xCB34;
const NODES: u32 = D + 0xCB48;
const ANGLE: u32 = D + 0xCBA6;
const KIND: u32 = D + 0xCBB4;
const FACING: u32 = D + 0xCBB8;
const COUNTER: u32 = D + 0xCBBC;
const BYTES: u32 = D + 0xCBBE;
const TAB: u32 = D + 0xCDC0;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The entry 0x1E02F70 (s0 = 0 / the live count, s1 = D, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let _ = TAB;
    let sp = fx.enter(0x150);
    fx.mem.write_u32(D + 0xCB1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x150);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xCF30);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xCBBA, param);
    fx.mem.set32(TIMER, 0xBE);
    for (k, n) in [0x11, 0x12, 0xD, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xCB58, r);
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k, t);
    }
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_3164);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3194, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_31E4);
        fx.mem.set16(D + 0xCB68 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == IDLE {
            continue;
        }
        // The step's prologue: ra, the entry's s1 = D, s0 = 0.
        let frame = match f {
            SPARKLE => 0x30,
            TRAILS => 0x40,
            FRAME_TINT => 0x80,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        match frame {
            0x30 | 0x40 => fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]),
            _ => fx.saved(s, &[(0x10, TASK_RA), (0, 0)]),
        }
        match f {
            SPARK => spark(fx, t),
            FAN => fan(fx, t),
            SPARKLE => sparkle(fx, t),
            COLUMN => column(fx, t),
            TINT4 => tint(fx, t),
            TRAILS => trails(fx, t, s),
            FRAME_TINT => frame_tint(fx, t, s),
            _ => panic!("b415: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    // The entry's s0: the live tasks counted in the draw loop.
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        s0 += 1;
        draw(fx, sp, t, s0, k);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x66 => 1,
        0xB9 => 4,
        0xA0 => 2,
        0x71..=0x9A => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    copy4(fx, D + 0x94, r);
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32, k: u32) {
    let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    if f(fx) == SPARK {
        let kind = fx.mem.i16(t + 0x50);
        let sub = fx.mem.i16(t + 0x56);
        if kind == 0 {
            cell(fx, sp + 0x70, 0x97, sub);
            tex(fx, D + 0x29C);
        } else if kind == 1 {
            cell(fx, sp + 0x80, 0x9D, sub);
            tex(fx, D + 0x29C);
        }
        fx.mem.set16(D + 0xB8, 0);
        for c in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * c);
            fx.mem.write_u32(D + 0x20 + 4 * c, w);
        }
        fx.mem.set16(D + 0xB8, 0xC);
        fx.mem.set16(D + 0xBE, 0);
        quads4::sparks(fx, D, t, 0x20, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f(fx) == FAN {
        fx.mem.set16(D + 0xB8, 0x24);
        billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f(fx) == SPARKLE {
        tex(fx, TEX);
        let sub = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0x90, 0x9B, sub);
        let o = fx.mem.i16(t + 0x52);
        fx.mem.set16(D + 0xBE, o);
        let r = fx.mem.i16(t + 0x56) << 5;
        fx.mem.set16(D + 0x94, r);
        fx.mem.set16(D + 0x96, 0);
        fx.mem.set16(D + 0x98, 0x20);
        fx.mem.set16(D + 0x9A, 0x20);
        quads4::quad_turned(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
    }
    if f(fx) == COLUMN {
        tex(fx, D + 0x29C);
        cell(fx, sp + 0xA0, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        let kind = fx.mem.i16(t + 0x50);
        let n = if kind < 2 {
            2
        } else if kind == 2 {
            1
        } else {
            0
        };
        for _ in 0..n {
            quads4::sprite_quads(fx, D, t, 0, t + 0x10);
        }
    }
    if f(fx) == TINT4 {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x60);
        // 0x1F71A0's frame below the entry's (its saved ra, s0).
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_3568), (0, u64::from(s0))]);
        let z = fx.mem.f(sp + 0x68);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 50.0) } else { ee::add(z, 50.0) };
        fx.mem.setf(sp + 0x68, z);
        let p = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
        let mut x = fx.mem.read_u64(sp + 0xD8);
        if let Some(w) = fx.project(p, None, &mut x) {
            fx.mem.write_u64(sp + 0xD8, x);
            fx.mem.setf(TINT_W, w);
        }
        project_residue(fx, sp, sp + 0x60);
        for kind in 0..3 {
            if fx.mem.i16(t + 0x50) == kind {
                let w = fx.mem.f(TINT_W);
                quads4::tint4(fx, t, 0x21, w);
            }
        }
    }
    if f(fx) == TRAILS {
        let saves = [u64::from(s0), u64::from(D), u64::from(t - 0x2A0), u64::from(sp + 0xD8), u64::from(k), 0, 0, 0, 0, 0x01E0_3680];
        ribbon(fx, t, HIST[0], HIST[1], saves);
    }
}

/// 0x1E03770(D, step, 0): b292's spawn on this record's globals (colour
/// D+0xCB14, kind D+0xCBB4 into +0x50, angle D+0xCBA6 into +0x5A); its
/// frame's saves the emitter's s0 = D / s1, the entry's s2 = the
/// parameter, s3 / s4 the runner's (0 [H]) and `ra`.
fn spawn(fx: &mut Fx, step: u32, s1: u64, param: i32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, s1), (0x20, param as i64 as u64), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for o in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(POS + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let s = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x50, s);
            let a = fx.mem.i16(ANGLE);
            fx.mem.set16(t + 0x5A, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E01D70(D, 0): the update counter and the spawns (frame 0x30: ra,
/// the entry's s1 = D, s0 = 0; then s0 = D, s1 a loop count).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3240), (0x10, u64::from(D)), (0, 0)]);
    let mut s1 = u64::from(D);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx, k: u32, out: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, out, u64::from(D), Some(s1), ra);
    };
    if c(fx) == 0x7C {
        spawn(fx, TRAILS, s1, param, 0x01E0_1DC4);
    }
    if c(fx) == 0x70 {
        node(fx, 2, AIM, s1, 0x01E0_1E00);
    }
    if (0x70..0x79).contains(&c(fx)) {
        copy4(fx, POS, AIM);
        fx.mem.set16(KIND, 0);
        spawn(fx, SPARK, s1, param, 0x01E0_1E6C);
        fx.mem.set16(KIND, 1);
        spawn(fx, SPARK, s1, param, 0x01E0_1E90);
    }
    if c(fx) == 0x70 {
        copy4(fx, POS, AIM);
        fx.mem.set16(KIND, 0);
        spawn(fx, FAN, s1, param, 0x01E0_1EF0);
        spawn(fx, TINT4, s1, param, 0x01E0_1F04);
        spawn(fx, FRAME_TINT, s1, param, 0x01E0_1F18);
        fx.mem.set16(KIND, 0);
        s1 = 0;
        while s1 < 0x32 {
            spawn(fx, SPARKLE, s1, param, 0x01E0_1F40);
            s1 += 1;
        }
    }
    if c(fx) == 0x87 {
        node(fx, 0, POS, s1, 0x01E0_1FA0);
        spawn(fx, FAN, s1, param, 0x01E0_1FB4);
        fx.mem.set16(KIND, 1);
        spawn(fx, TINT4, s1, param, 0x01E0_1FD8);
        spawn(fx, FRAME_TINT, s1, param, 0x01E0_1FEC);
        for (a, kind, ra) in [(0, 0, 0x01E0_2018u32), (0x5A, 0, 0x01E0_2048), (0x2D, 1, 0x01E0_207C), (0x87, 1, 0x01E0_20B0)] {
            fx.mem.set16(ANGLE, a);
            fx.mem.set16(KIND, kind);
            spawn(fx, COLUMN, s1, param, ra);
        }
    }
    fx.leave(0x30);
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// 0x1E020D0 (D, task): b482's sparks (0x1E02A40) with the alpha 0x80 for
/// 15 updates: radius 120 (inner 62) growing by (200..279) / 10 a update
/// with the inner at 0.2 of it; kind 0 pink on black, 1 violet on grey;
/// random angles +0x20..+0x28 (% 0xFFFF); then down by 5.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42F0_0000);
        fx.mem.setf(t + 0x4C, ee::sub(120.0, 58.0));
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xC0, 0x80, 0x80);
            l2::rgb(fx, t + 0x64, 0, 0, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xA0, 0x80, 0xFF);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        }
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        let v = rmod(fx, t, 0x01E0_2188, 0x50);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0xC8));
        let q = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, q);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, site) in [(0x20u32, 0x01E0_21D0u32), (0x24, 0x01E0_21EC), (0x28, 0x01E0_2208)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 0xF {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 7, 5);
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3E4C_CCCD), r));
    age(fx, t);
}

/// 0x1E022B0 (D, task): b641's arc fan with white outer / red (0xFF, 0x40,
/// 0x40) inner colours: outer radius 220, 10-degree arcs every 10 degrees;
/// its outer alpha up by 0x60 below 0xC0 for 14 updates, then down by 9;
/// faces the camera turned by 0x4000 a update about x.
fn fan(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x435C_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xE {
        fade_in(fx, t, 0x67, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x67, 0xB, 9);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2370);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E023A0 (D, task; s1 = D): a grey sparkle of size 8..23 (cell row
/// (rand >> 4) % 3) flying by a random velocity (+0x34 0..99, +0x30 /
/// +0x38 -50..49, all / 80), turned by the camera, a random angle +0x5A
/// turning by (rand >> 4) % 20 - 10, spins +4 / +2 / +0, kind 0..3, life
/// 22..31; in by 0x40 under 0x80 before update 0xB4, out by 9.
fn sparkle(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_23CC);
        fx.mem.set16(t + 8, (v >> 4) % 16 + 8);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_23F8);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, 0x01E0_242C, 0x64);
        fx.mem.setf(t + 0x34, ee::from_i32(v));
        let v = rmod(fx, t, 0x01E0_2454, 0x64);
        fx.mem.setf(t + 0x30, ee::from_i32(v - 0x32));
        let v = rmod(fx, t, 0x01E0_2480, 0x64);
        fx.mem.setf(t + 0x38, ee::from_i32(v - 0x32));
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 80.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_24D8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_24F4);
        let r = fx.div(v >> 4, 0x14).1;
        fx.mem.set16(t + 0x5C, r - 0xA);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2518);
        for (o, ra) in [(4u32, 0x01E0_2520u32), (2, 0x01E0_2540), (0, 0x01E0_2560)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        let v = fx.rand_from(t, 0x01E0_2580);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
        let v = rmod(fx, t, 0x01E0_25A0, 0xA);
        fx.mem.set16(t + 6, v + 0x16);
    }
    if fx.mem.i16(COUNTER) < 0xB4 && fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for (p, v) in [(0x10u32, 0x30u32), (0x14, 0x34), (0x18, 0x38)] {
            let d = fx.mem.f(t + v);
            add(fx, t + p, d);
        }
    }
    for (a, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + a).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + a, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_26BC);
    age(fx, t);
    let a = fx.mem.i16(t + 0x5A) + fx.mem.i16(t + 0x5C);
    fx.mem.set16(t + 0x5A, a);
}

/// 0x1E026F0 (D, task): b247b's column (0x1E02320) word for word: (0x60,
/// 0x60, 0xFF) / (0x40, 0x40, 0xFF), size 6, by kind +0x50 (28, 1600) /
/// (28, 500) / (6, 100; colours (0x80, 0x80, 0xC0) / (0x40, 0x40, 0xFF))
/// at +0x30 / +0x34; in by 0x60 under 0xC0 for 6 updates, out by 9; from
/// age 1 +0x34 grows by 60 / 20 / 5; +0x5A turns 6 a update.
fn column(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0xFF);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x30, 0x41E0_0000);
            fx.mem.write_u32(t + 0x34, 0x44C8_0000);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x30, 0x41E0_0000);
            fx.mem.write_u32(t + 0x34, 0x43FA_0000);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            fx.mem.write_u32(t + 0x30, 0x40C0_0000);
            fx.mem.write_u32(t + 0x34, 0x42C8_0000);
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xC0);
            l2::rgb(fx, t + 0x64, 0x40, 0x40, 0xFF);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 6 {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    for (kind, by) in [(0, 60.0f32), (1, 20.0), (2, 5.0)] {
        if fx.mem.i16(t + 0x50) == kind {
            if fx.mem.i16(t + 0x54) != 0 {
                add(fx, t + 0x34, by);
            }
            let a = fx.mem.i16(t + 0x5A) + 6;
            fx.mem.set16(t + 0x5A, a);
        }
    }
    age(fx, t);
}

/// 0x1E02900 (D, task): b646's tint (0x1E024D0) with both alphas in by
/// 0x19: kind 0 red / orange, 1 blue / violet, life 8; 2 grey, 22; then
/// out by 4 (the task ends when the first is below 6, the second is
/// cleared).
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let kinds: [([i32; 3], [i32; 3], i32); 3] = [([0xF0, 0, 0x30], [0xF0, 0x30, 0], 8), ([0, 0x30, 0xF0], [0x30, 0, 0xF0], 8), ([0x58, 0x58, 0x78], [0x58, 0x58, 0x78], 0x16)];
        for (kind, (a, b, life)) in kinds.into_iter().enumerate() {
            if fx.mem.i16(t + 0x50) == kind as i32 {
                l2::rgb(fx, t + 0x60, a[0], a[1], a[2]);
                l2::rgb(fx, t + 0x64, b[0], b[1], b[2]);
                fx.mem.set16(t + 6, life);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a.wrapping_add(0x19));
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 6 {
            fx.mem.write_u8(t + 0x63, a - 4);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        fx.mem.write_u8(t + 0x67, if a >= 6 { a - 4 } else { 0 });
    }
    age(fx, t);
}

/// 0x1E02A90 (D, task): b292's ribbon task in blue (0x20, 0x20, 0xC0)
/// every update; on its first update size 5, +0xE 30 and the histories
/// filled (19 points) with nodes 0x12 / 0x11; in by 0x3C under 0xF0 to
/// update 0x8E, then out by 13 (ending under 15); to update 0x8E each
/// history takes its node's point (`spawn::history`, 20 points); +6 = 15.
fn trails(fx: &mut Fx, t: u32, sp: u32) {
    l2::rgb(fx, t + 0x60, 0x20, 0x20, 0xC0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        fx.mem.set16(t + 0xE, 0x1E);
        for (k, n, ra) in [(0usize, 1u32, 0x01E0_2AF8u32), (1, 0, 0x01E0_2B64)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 * n);
            node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ra);
            for e in 0..0x13u32 {
                l2::copy3(fx, HIST[k] + 0x10 * e, sp + 0x30);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0x8F {
        fade_in(fx, t, 0x63, 0xF0, 0x3C);
    } else {
        fade_out(fx, t, 0x63, 0xF, 0xD);
    }
    if fx.mem.i16(COUNTER) < 0x8F {
        for (k, n, ra) in [(0usize, 1u32, 0x01E0_2C48u32), (1, 0, 0x01E0_2C80)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 * n);
            node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ra);
            hist::history(fx, HIST[k] + 0x130, 0x14, sp + 0x30);
        }
    }
    fx.mem.set16(t + 6, 0xF);
    age(fx, t);
}

/// 0x1E02CD0 (D, task; frame 0x80 at `sp`: ra, the entry's s0 = 0): b292's
/// frame tint with ALPHA 0x48 and its colour by kind (0 (0x80, 0, 0), 1
/// (0, 0, 0x80)): b020's window sprite (0x6FF8 .. 0x8FF8) sampling the
/// frame (0x207EF0, filter 1) at ST inset by +0x48 / 512, key 0; before
/// age 6 in by 0x20 below 0x60 and the inset up by 0.3 below 100, then
/// out by 4 (ending under 6) and the inset down by 0.2 to 0.
fn frame_tint(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x58, 2);
        match fx.mem.i16(t + 0x50) {
            0 => l2::rgb(fx, t + 0x60, 0x80, 0, 0),
            1 => l2::rgb(fx, t + 0x60, 0, 0, 0x80),
            _ => {}
        }
    }
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, c);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, v) in [(d + 0x20, 0x6FF8u16), (d + 0x30, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for o in [0x18u32, 0x1C] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(d + o, ee::div(s, 512.0));
    }
    for o in [0x28u32, 0x2C] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(d + o, ee::div(ee::sub(512.0, s), 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(t + 0x54) < 6 {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x60 {
            fx.mem.write_u8(t + 0x63, a + 0x20);
        }
        let w = fx.mem.f(t + 0x48);
        if w < 100.0 {
            fx.mem.setf(t + 0x48, ee::add(w, f32::from_bits(0x3E99_999A)));
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 6 {
            fx.mem.write_u8(t + 0x63, a - 4);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let w = fx.mem.f(t + 0x48);
        if w <= 0.0 {
            fx.mem.write_u32(t + 0x48, 0);
        } else {
            fx.mem.setf(t + 0x48, ee::sub(w, f32::from_bits(0x3E4C_CCCD)));
        }
    }
    age(fx, t);
}

/// 0x1E03910 (D, task, a, b): b703's ribbon (0x1E03E20) with its locals
/// 0x10 higher and the `b` side's alphas written as bytes (desc +0x33 /
/// +0x43) instead of RGBAQs: i16 +6 points; per segment four strips
/// (a(t), a(t + 1/4), b(t), b(t + 1/4) by `spline::spline` basis 0) at
/// key 0 through the camera; `saves` the draw loop's s0..s7, fp and the
/// return address.
fn ribbon(fx: &mut Fx, t: u32, a: u32, b: u32, saves: [u64; 10]) {
    use ee::add;
    let sp = fx.enter(0x300);
    let offs = [0u32, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90];
    let list: Vec<(u32, u64)> = offs.iter().zip(saves).map(|(&o, v)| (o, v)).collect();
    fx.saved(sp + 0x10, &list);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 0xE0, a);
    fx.mem.write_u32(sp + 0xD0, b);
    fx.mem.write_u32(sp + 0xB8, OT);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x248;
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    let (ca, cb) = (sp + 0x2F8, sp + 0x2FC);
    for c in [ca, cb] {
        for k in 0..4 {
            let v = fx.mem.read_u8(t + 0x60 + k);
            fx.mem.write_u8(c + k, v);
        }
    }
    fx.mem.write_u8(ca + 3, 0);
    let (pa, pb) = (sp + 0x180, sp + 0x1C0);
    for (p, src) in [(pa, a), (pb, b)] {
        copy4(fx, p, src);
        copy4(fx, p + 0x10, src);
        copy4(fx, p + 0x20, src + 0x10);
        copy4(fx, p + 0x30, src + 0x20);
    }
    rgbaq_at(fx, sp + 0x2D8, ca, d + 0x10);
    rgbaq_at(fx, sp + 0x2E0, ca, d + 0x20);
    rgbaq_at(fx, sp + 0x2E8, cb, d + 0x30);
    rgbaq_at(fx, sp + 0x2F0, cb, d + 0x40);
    fx.mem.set32(sp + 0xC0, n - 1);
    let mut s4 = i32::from(fx.mem.read_u8(t + 0x63));
    let fp = fx.div(s4, (3 * n) as i16 as i32).0 as i16 as i32;
    let mut s7 = 0;
    while s7 < n - 1 {
        let off = (0x10 * (s7 - 1)) as u32;
        let (s2, s3) = (a.wrapping_add(off), b.wrapping_add(off));
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            fx.mem.write_u8(d + 0x33, s4 as u8);
            s4 = (s4 - fp) as i16 as i32;
            if s4 < 0 {
                s4 = 0;
            }
            fx.mem.write_u8(d + 0x43, s4 as u8);
            let (qa, qb, oa, ob) = if s7 != 0 { (s2, s3, 0x200u32, 0x210u32) } else { (pa, pb, 0x220, 0x230) };
            spline::spline(fx, sp + 0x140, qa, f20, 0, [0.0; 4]);
            spline::spline(fx, sp + 0x160, qb, f20, 0, [0.0; 4]);
            f20 = add(f20, 0.25);
            spline::spline(fx, sp + oa, qa, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x150, sp + oa);
            spline::spline(fx, sp + ob, qb, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x170, sp + ob);
            for (to, from) in [(0x100u32, 0x140u32), (0x110, 0x150), (0x120, 0x160), (0x130, 0x170)] {
                l2::copy3(fx, sp + to, sp + from);
            }
            if l2::project(fx, sp + 0x100, SPR, 0, d, sp + 0xF0) {
                fx.submit(d, OT, 0);
            }
        }
        s7 += 1;
    }
    fx.leave(0x300);
}
