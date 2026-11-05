//! BMODULE b852 (card 681), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E037A0, data area 0x1ED0000, 0xCF30 bytes) [F]
//!: b703 / b293's program shape (400 tasks of
//! 0x70 at D+0x2A0, alive +0x58, step +0x44 through `jalr`, 0x1E03790 the
//! idle step; spawned by 0x1E041A0, b522's spawn: kind D+0xCBB0 into +0x50
//! / +0x52, angle D+0xCBA2) drawn with the second library and b703's
//! ribbon (0x1E04350 word for word, [`crate::b703::ribbon_saving`]).
//! Nodes: the attacker's 0x5A / 0x60 / 0x62 / 0x30 / 0x2F / 0x2C
//! (D+0xCB44..), the target's 1 (D+0xCB54); timer 0x136; two byte tables
//! D+0xCBBA / D+0xCCBA.
//!
//! Every update node 0x62's point is pushed into the history D+0xB580. On
//! update 1 the stage light (0x1E033D0, dimmed to 0x60) and a grey frame
//! tint (0x1E034E0, lilac from 0xFA, growing in from 0xFB); below 0xFF two
//! dark red rings a update (0x1E02150 / 0x1E02420) pulled from node 0x5A
//! toward node 0x60 / from 0x60 toward 0x62; 0xFB..0x100 twenty white rings (0x1E028B0)
//! along node 0x62 -> the history; on 0x50 a violet ribbon task
//! (0x1E03070, the histories of nodes 0x60 / 0x62, drawn three times); on
//! 0xFD at the target (45 up) ten white rings (0x1E02A60), twenty motes
//! (0x1E02E20), four columns (0x1E02C20, `sprite_quads`) and an arc
//! (0x1E032B0). Rings by `billboard::ring_quads`. The ring 0x1E026F0 is
//! drawn but never spawned (not ported). Returns 1 on 0xF5, 4 on 0x136, 2
//! on 0x113, 6 on 0x100..0x10D, 8 once the timer D+0xCB10 has run out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, spawn as hist, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const RING_A: u32 = 0x01E0_2150;
const RING_B: u32 = 0x01E0_2420;
const RING_C: u32 = 0x01E0_26F0;
const RING_D: u32 = 0x01E0_28B0;
const RING_E: u32 = 0x01E0_2A60;
const COLUMN: u32 = 0x01E0_2C20;
const MOTE: u32 = 0x01E0_2E20;
const RIBBON: u32 = 0x01E0_3070;
const ARC: u32 = 0x01E0_32B0;
const LIGHT: u32 = 0x01E0_33D0;
const TINT: u32 = 0x01E0_34E0;
const IDLE: u32 = 0x01E0_3790;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3B58;
const HIST: [u32; 2] = [D + 0xB1D0, D + 0xB310];
const AIM: u32 = D + 0xCAD0;
const TEX: u32 = D + 0xCAF0;
const TIMER: u32 = D + 0xCB10;
const COLOUR: u32 = D + 0xCB14;
const NODES: u32 = D + 0xCB44;
const ANGLE: u32 = D + 0xCBA2;
const KIND: u32 = D + 0xCBB0;
const FACING: u32 = D + 0xCBB4;
const COUNTER: u32 = D + 0xCBB8;
const BYTES: u32 = D + 0xCBBA;
const BYTES_B: u32 = D + 0xCCBA;
const TAB: u32 = D + 0xCDBC;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E037A0 (s0 = 0 / the live count, s1 = D, s2 = param /
/// the task pointer, s3 = the index).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    fx.mem.write_u32(D + 0xCB1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xCF30);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xCBB6, param);
    fx.mem.set32(TIMER, 0x136);
    for (k, n) in [0x5A, 0x60, 0x62, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xCB54, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_3990);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_39C0, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3A14, 0x1E);
        fx.mem.set8(BYTES_B + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, k, 0x01E0_3A6C, 0x14);
        fx.mem.set8(BYTES_B + k, v - 0xA);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_3ABC);
        fx.mem.set16(D + 0xCB64 + 2 * k, v % 8);
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
            RING_A | RING_B | RIBBON => 0x40,
            MOTE => 0x30,
            TINT => 0x90,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        }
        match f {
            RING_A => pulled(fx, t, s, RING_A, false),
            RING_B => pulled(fx, t, s, RING_B, true),
            RING_D => ring(fx, t, RING_D, false),
            RING_E => ring(fx, t, RING_E, true),
            COLUMN => column(fx, t),
            MOTE => mote(fx, t),
            RIBBON => ribbon_task(fx, t, s),
            ARC => arc(fx, t),
            LIGHT => light(fx, t),
            TINT => tint(fx, t, s),
            _ => panic!("b852: unknown task step {f:#x}"),
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
        0xF5 => 1,
        0x136 => 4,
        0x113 => 2,
        0x100..=0x10D => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

fn draw(fx: &mut Fx, sp: u32, t: u32, s0: u32, k: u32) {
    let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    for (step, local, b8, site) in [(RING_A, 0x50u32, 2, 0x01E0_3C24u32), (RING_B, 0x60, 2, 0x01E0_3CE8), (RING_C, 0x70, 4, 0x01E0_3DA8)] {
        if f(fx) != step {
            continue;
        }
        let sub = fx.mem.i16(t + 0x56);
        cell(fx, sp + local, 0x14, sub);
        tex(fx, TEX);
        let sub = fx.mem.i16(t + 0x56);
        fx.mem.set16(D + 0x94, sub << 5);
        fx.mem.set16(D + 0x96, 0);
        fx.mem.set16(D + 0x98, 0x20);
        fx.mem.set16(D + 0x9A, 0x40);
        fx.mem.set16(D + 0xB8, b8);
        let v = fx.rand_from(s0, site);
        fx.mem.set16(D + 0xBE, v % 4);
        fx.mem.set16(D + 0xB0, 0);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    for (step, local, site, b0, table) in [(RING_D, 0x80u32, 0x01E0_3E48u32, 0, BYTES_B), (RING_E, 0x90, 0x01E0_3EE8, 1, BYTES)] {
        if f(fx) != step {
            continue;
        }
        let sub = fx.mem.i16(t + 0x56);
        cell(fx, sp + local, 0x14, sub);
        tex(fx, TEX + 4);
        fx.mem.set16(D + 0xB8, 5);
        let v = fx.rand_from(s0, site);
        fx.mem.set16(D + 0xBE, v % 4);
        fx.mem.set16(D + 0xB0, b0);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, table);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f(fx) == COLUMN {
        tex(fx, D + 0x29C);
        cell(fx, sp + 0xA0, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        quads4::sprite_quads(fx, D, t, 0, t + 0x10);
    }
    if f(fx) == MOTE {
        tex(fx, TEX + 8);
        let sub = fx.mem.i16(t + 0x56);
        cell(fx, sp + 0xB0, 0x14, sub);
        fx.mem.set16(D + 0xBE, 0);
        billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
    }
    if f(fx) == RIBBON {
        for ra in [0x01E0_4040u64, 0x01E0_405C, 0x01E0_4078] {
            let saves = [u64::from(s0), u64::from(D), u64::from(t - 0x2A0), u64::from(k), 0, 0, 0, 0, 0, ra];
            crate::b703::ribbon_saving(fx, t, HIST[0], HIST[1], saves);
        }
    }
    if f(fx) == ARC {
        fx.mem.set16(D + 0xB8, 0x24);
        billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
}

/// 0x1E041A0(D, step, 0): b522's spawn on this record's globals (colour
/// D+0xCB14, kind D+0xCBB0 into +0x50 / +0x52, angle D+0xCBA2 into
/// +0x5A); its frame's saves the emitter's s0 = D / s1, the entry's s2 =
/// the parameter, s3 / s4 the runner's (0 [H]) and `ra`.
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
            let s = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x52, s);
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

/// 0x1E01D30(D, 0): the update counter, node 0x62's history and the
/// spawns (frame 0x50: ra, the entry's s1 = D, s0 = 0; then s0 = D, s1 a
/// loop count).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, 0x01E0_3B18), (0x10, u64::from(D)), (0, 0)]);
    let mut s1 = u64::from(D);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx, k: u32, out: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, out, u64::from(D), Some(s1), ra);
    };
    node(fx, 2, sp + 0x30, s1, 0x01E0_1D7C);
    hist::history(fx, D + 0xB580, 0x14, sp + 0x30);
    if c(fx) == 1 {
        spawn(fx, LIGHT, s1, param, 0x01E0_1DC4);
    }
    if c(fx) == 1 {
        spawn(fx, TINT, s1, param, 0x01E0_1DF0);
    }
    if c(fx) < 0xFF {
        for k in 0..4 {
            let w = fx.mem.read_u32(AIM + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        node(fx, 0, POS, s1, 0x01E0_1E48);
        spawn(fx, RING_A, s1, param, 0x01E0_1E5C);
        spawn(fx, RING_B, s1, param, 0x01E0_1E70);
    }
    if (0xFB..0x101).contains(&c(fx)) {
        s1 = 0;
        while s1 < 0xA {
            node(fx, 2, POS, s1, 0x01E0_1EBC);
            let k = ee::from_i32(s1 as i32);
            for o in 0..3u32 {
                let p = fx.mem.f(POS + 4 * o);
                let h = fx.mem.f(D + 0xB460 + 4 * o);
                let v = ee::sub(p, ee::mul(k, ee::div(ee::sub(p, h), 10.0)));
                fx.mem.setf(POS + 4 * o, v);
            }
            spawn(fx, RING_D, s1, param, 0x01E0_1F4C);
            spawn(fx, RING_D, s1, param, 0x01E0_1F60);
            s1 += 1;
        }
    }
    if c(fx) == 0x50 {
        spawn(fx, RIBBON, s1, param, 0x01E0_1FB8);
    }
    if c(fx) == 0xFD {
        let h = fx.target_handle();
        fx.unit_position(h, POS);
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_1FE0), (0, u64::from(D))]);
        fx.mem.write_u32(POS + 4, 0x4234_0000);
        s1 = 0;
        while s1 < 0xA {
            spawn(fx, RING_E, s1, param, 0x01E0_2004);
            s1 += 1;
        }
        s1 = 0;
        while s1 < 0x14 {
            spawn(fx, MOTE, s1, param, 0x01E0_2040);
            s1 += 1;
        }
        for (a, kind, ra) in [(0, 0, 0x01E0_208Cu32), (0x5A, 0, 0x01E0_20BC), (0x2D, 1, 0x01E0_20F0), (0x87, 1, 0x01E0_2124)] {
            fx.mem.set16(ANGLE, a);
            fx.mem.set16(KIND, kind);
            spawn(fx, COLUMN, s1, param, ra);
        }
        spawn(fx, ARC, s1, param, 0x01E0_2138);
    }
    fx.leave(0x50);
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

/// 0x1E02150 / 0x1E02420 (`back`) (D, task; frame 0x40 at `sp`, s1 = D):
/// a dark red (0x48, 8, 0x18) ring: size rand % 20 + 30, cell row 0..2,
/// radius rand % 10 + 5, angle rand % 360, growth rand % 6 + 5, row 0..34,
/// life rand % 4 (8) + 7; in by 0x40 under 0xC0, out by 0x13; its point
/// node 0x5A's (0x60's) pulled by +0x52 fifteenths toward node 0x60
/// (0x62), +0x52 counting up; the size up by 1 from age 1, +0x5A turning
/// 6; the age held at 20 from update 0x100.
fn pulled(fx: &mut Fx, t: u32, sp: u32, f: u32, back: bool) {
    use ee::{div, from_i32, mul, sub};
    let r = |o: u32| f + o;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, r(0x2C), 0x14);
        fx.mem.setf(t + 0x48, from_i32(v + 0x1E));
        let v = rmod(fx, t, r(0x58), 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x48, 8, 0x18);
        let v = rmod(fx, t, r(0x84), 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, r(0xA4), 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, r(0xC0), 6);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rmod(fx, t, r(0xE0), 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, r(0xFC));
        fx.mem.set16(t + 6, if back { v % 8 } else { v % 4 } + 7);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13);
    }
    let (from, to) = if back { (1u32, 2u32) } else { (0, 1) };
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + 2 * from);
    node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), r(0x194));
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES + 2 * to);
    node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), r(0x1B4));
    for k in 0..3u32 {
        let v = div(sub(fx.mem.f(sp + 0x30 + 4 * k), fx.mem.f(t + 0x10 + 4 * k)), 15.0);
        fx.mem.setf(sp + 0x30 + 4 * k, v);
    }
    let by = from_i32(fx.mem.i16(t + 0x52));
    for k in 0..3u32 {
        let v = mul(fx.mem.f(sp + 0x30 + 4 * k), by);
        add(fx, t + 0x10 + 4 * k, v);
    }
    let n = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, n);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x48, 1.0);
    }
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
    age(fx, t);
    if fx.mem.i16(COUNTER) >= 0x100 {
        fx.mem.set16(t + 0x54, 0x14);
    }
}

/// 0x1E028B0 / 0x1E02A60 (`late`) (D, task): a white ring: size rand % 15
/// + 15 (25), cell row 0..2, radius rand % 20 + 10 growing by rand % 20 +
/// 25 (65) from age 1, angle rand % 360, row 0..34, life rand % 8 + 7
/// (10); in by 0x40 under 0xC0, out by 0x13 (9); the late one turns 6.
fn ring(fx: &mut Fx, t: u32, f: u32, late: bool) {
    let r = |o: u32| f + o;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, r(0x24), 0xF);
        fx.mem.setf(t + 0x48, ee::from_i32(v + if late { 0x19 } else { 0xF }));
        let v = rmod(fx, t, r(0x50), 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = rmod(fx, t, r(0x7C), 0x14);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, r(0x9C), 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, r(0xB8), 0x14);
        fx.mem.set16(t + 0xE, v + if late { 0x41 } else { 0x19 });
        let v = rmod(fx, t, r(0xD8), 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, r(0xF4));
        fx.mem.set16(t + 6, v % 8 + if late { 0xA } else { 7 });
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else if late {
        fade_out(fx, t, 0x63, 0xB, 9);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    if late {
        let a = fx.mem.i16(t + 0x5A) + 6;
        fx.mem.set16(t + 0x5A, a);
    }
    age(fx, t);
}

/// 0x1E02C20 (D, task): a white / red (0xFF, 0x40, 0x40) column, size 6, by
/// kind +0x50 (20, 870) / (20, 630) / (8, 400) at +0x30 / +0x34; in by
/// 0x40 under 0x80 for 10 updates, out by 9; from age 1 kind 0 grows
/// (4, 200) / kind 1 (2, 100) a update, +0x52 counts to 25.
fn column(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x40, 0x40);
        for (kind, w, h) in [(0, 0x41A0_0000u32, 0x4459_8000u32), (1, 0x41A0_0000, 0x441D_8000), (2, 0x4100_0000, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x52, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    for (kind, dh, dw) in [(0, 200.0f32, 4.0f32), (1, 100.0, 2.0)] {
        if fx.mem.i16(t + 0x50) == kind {
            if fx.mem.i16(t + 0x54) != 0 {
                add(fx, t + 0x34, dh);
            }
            if fx.mem.i16(t + 0x54) != 0 {
                add(fx, t + 0x30, dw);
            }
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let n = fx.mem.i16(t + 0x52);
        if n < 0x19 {
            fx.mem.set16(t + 0x52, n + 1);
        }
    }
    age(fx, t);
}

/// 0x1E02E20 (D, task; s1 = D): a lilac (0xEF, 0xDF, 0xFF) mote of size
/// 150, cell 2..4, random angles +0x5A / +0x5C ((rand >> 4) % 360), thrown
/// 8.6 out (y by sin +0x5A, x by cos +0x5A, z by cos +0x5C), then drifting
/// 0.6 a update; in by 0x60 under 0xC0 for 26 updates, out by 0x13; turned
/// by the camera.
fn mote(fx: &mut Fx, t: u32) {
    let throw = |fx: &mut Fx, k: f32| {
        for (at, a, cos) in [(0x14u32, 0x5Au32, 0), (0x10, 0x5A, 1), (0x18, 0x5C, 1)] {
            let deg = fx.mem.i16(t + a);
            let s = l2::sin_deg(fx, TAB, deg, cos);
            add(fx, t + at, ee::mul(k, s));
        }
    };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x96);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2E58);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xEF, 0xDF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2E90);
        let r = fx.div(v >> 4, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        let v = fx.rand_from(t, 0x01E0_2EB0);
        let r = fx.div(v >> 4, 0x168).1;
        fx.mem.set16(t + 0x5C, r);
        throw(fx, f32::from_bits(0x4109_999A));
    }
    if fx.mem.i16(t + 0x54) < 0x1A {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        throw(fx, f32::from_bits(0x3F19_999A));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_3048);
    age(fx, t);
}

/// 0x1E03070 (D, task; frame 0x40 at `sp`): b292's ribbon task in violet
/// (0x80, 0x40, 0xC0) every update; on its first update size 5, +0xE 30
/// and the histories filled (19 points) with nodes 0x60 / 0x62; in by 0x3C
/// under 0xF0 to update 0x58, then out by 0x1D (ending under 0x1F); to
/// update 0x59 each history takes its node's point; +6 = 6.
fn ribbon_task(fx: &mut Fx, t: u32, sp: u32) {
    l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xC0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        fx.mem.set16(t + 0xE, 0x1E);
        for (k, ra) in [(0usize, 0x01E0_30D8u32), (1, 0x01E0_3144)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 + 2 * k as u32);
            node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ra);
            for e in 0..0x13u32 {
                l2::copy3(fx, HIST[k] + 0x10 * e, sp + 0x30);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0x59 {
        fade_in(fx, t, 0x63, 0xF0, 0x3C);
    } else {
        fade_out(fx, t, 0x63, 0x1F, 0x1D);
    }
    if fx.mem.i16(COUNTER) < 0x5A {
        for (k, ra) in [(0usize, 0x01E0_3228u32), (1, 0x01E0_3260)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 + 2 * k as u32);
            node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ra);
            hist::history(fx, HIST[k] + 0x130, 0x14, sp + 0x30);
        }
    }
    fx.mem.set16(t + 6, 6);
    age(fx, t);
}

/// 0x1E032B0 (D, task): an arc (inner (0x80, 0x40, 0xFF), outer grey 0xC0)
/// of radius 350 growing 0.4 a update, 10 x 10, turned by the camera +
/// 0x4000; its outer alpha up by 0x60 under 0xC0 for 14 updates, then
/// down by 9.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x43AF_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_3370);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x4C, f32::from_bits(0x3ECC_CCCD));
    }
    age(fx, t);
}

/// 0x1E033D0 (D, task): b465's stage light down by 10 to 0x60 for 0x97
/// updates, back up by 10 to 0x80 once the timer is under 30; 0x80 and
/// the task's end when the timer is at 3.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x97 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x61 { 0x60 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
}

/// 0x1E034E0 (D, task; frame 0x90 at `sp`): b292's frame tint in grey 0x60
/// (lilac (0x84, 0x74, 0x94) from update 0xFA), the inset from 2.5 up by
/// 0.3 below 5 from 0xFB; in by 0xE below 0x48 and from 0xFB by 0x16 below
/// 0x58 to update 0x117, then out by 4 (ending under 6).
fn tint(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x60);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) == 0xFA {
        l2::rgb(fx, t + 0x60, 0x84, 0x74, 0x94);
    }
    let d = sp + 0x38;
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
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(COUNTER) >= 0xFB {
        let w = fx.mem.f(t + 0x48);
        if w < 5.0 {
            fx.mem.setf(t + 0x48, ee::add(w, f32::from_bits(0x3E99_999A)));
        }
    }
    if fx.mem.i16(COUNTER) < 0x118 {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x48 {
            fx.mem.write_u8(t + 0x63, a + 0xE);
        }
        if fx.mem.i16(COUNTER) >= 0xFB {
            let a = fx.mem.read_u8(t + 0x63);
            if a < 0x58 {
                fx.mem.write_u8(t + 0x63, a + 0x16);
            }
        }
    } else {
        fade_out(fx, t, 0x63, 6, 4);
    }
    age(fx, t);
}
