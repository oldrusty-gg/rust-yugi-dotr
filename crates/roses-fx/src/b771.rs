//! BMODULE b771 (card 668), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03520, data area 0x1ED0000, 0xACA0 bytes) [F]
//!: b465 / b482's task family on a pool of 150
//! tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step +0x44 through `jalr`;
//! the flash 0x1E03230 counted, not called), spawned by 0x1E03DE0 (b193's
//! spawn at this layout: colour D+0xA8C4, +0x50 / +0x52 from D+0xA926).
//!
//! The emitter 0x1E01630 counts D+0xA92E: the point D+0xA870 follows the
//! attacker's node 9 until 0x9A, then slides a 32nd of the way to the
//! target's node 1 a update until 0xB9; on 1 the flash 0x1E03230 (the
//! target drawn into the render target, green), a green arc 0x1E01FB0 and
//! a green rim 0x1E02FD0 at the target (y 37), from 0x190 green mote rings
//! 0x1E01D90 there; on 0x2A the object op 0x1E031B0; 0x26..0x46 and
//! 0x76..0x1DB smoke rings 0x1E02460 at node 9; 0x76..0x118 two bubble
//! rings 0x1E02C10 and a green arc 0x1E022D0; on 0x76 a shrinking arc
//! 0x1E02130; even updates 0x76..0x1DB an arc 0x1E01C40 at the target; on
//! 0xBA ten bubble rings at the point; odd updates 0xC8..0x1DB two spiral
//! ribbons 0x1E026F0 (two 20-point histories D+0x4470 / D+0x45B0 + 0x280 k,
//! drawn by 0x1E03F80, b191's ribbon keyed by the mean w). Arcs draw by
//! 0x1E00F60 (`arc_strips_flat` with the start angle +0x52 + +0x5A). Returns
//! 1 / 4 / 2 / 6 on 0x12C / 0x1F9 / 0x1EF / 0x137..0x1E9, 8 once the
//! 0x1FD-update timer D+0xA8C0 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::blib::{billboard, spawn, uv};
use crate::bus::Bus;
use crate::c061::blend_alpha;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
const POS: u32 = D + 0x250;
const POINT: u32 = D + 0xA870;
const TEX5: u32 = D + 0xA8A0;
const TIMER: u32 = D + 0xA8C0;
const COLOUR: u32 = D + 0xA8C4;
const LAYER: u32 = D + 0xA8D4;
const FLASH_DESC: u32 = D + 0xA8D8;
const SCALE: u32 = D + 0xA8E4;
const NODE9: u32 = D + 0xA8F8;
const TNODE: u32 = D + 0xA908;
const INDEX: u32 = D + 0xA926;
const NEXT: u32 = D + 0xA928;
const COUNTER: u32 = D + 0xA92E;
const BYTES: u32 = D + 0xA930;
const BYTES_B: u32 = D + 0xAA30;
const SINES: u32 = D + 0xAB30;

const ARC_T: u32 = 0x01E0_1C40;
const MOTES: u32 = 0x01E0_1D90;
const ARC_G: u32 = 0x01E0_1FB0;
const ARC_S: u32 = 0x01E0_2130;
const ARC_N: u32 = 0x01E0_22D0;
const SMOKE: u32 = 0x01E0_2460;
const SPIRAL: u32 = 0x01E0_26F0;
const BUBBLE: u32 = 0x01E0_2C10;
const RIM: u32 = 0x01E0_2FD0;
const OBJECT: u32 = 0x01E0_31B0;
const FLASH: u32 = 0x01E0_3230;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3890;
const SPR: u32 = 0x7000_0000;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E03520 (frame 0xC0; s0 = D, s3 = the parameter).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0xA8CC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xACA0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xA92C, param);
    fx.mem.set32(TIMER, 0x1FD);
    for (k, node) in [9, 0x34, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE9 + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TNODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_3718);
    fx.mem.set16(D + 0xA92A, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3748, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 100..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_37A4);
        fx.mem.set8(BYTES + k, v & 3);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_37FC, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s1: the flash tasks counted (saved by the steps).
    let mut flashes = 0u32;
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
            flashes = (flashes as i32 + 1) as i16 as u32;
            continue;
        }
        run_task(fx, f, slot + 0x2A0, [flashes, slot, k]);
    }
    let mut live = 0u32;
    let mut a3 = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        live = (live as i32 + 1) as i16 as u32;
        let f = fx.mem.read_u32(t + 0x44);
        if f == ARC_T {
            fx.mem.set16(D + 0xB8, 0x18);
            target_point(fx, sp + 0x60, 0x01E0_3918, 37.0);
            a3 = arcs_spun(fx, t, 0x20, sp + 0x60, a3);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == MOTES {
            tex(fx, TEX5 + 4);
            cell(fx, t, sp + 0x70, 0x14);
            fx.mem.set16(D + 0xB8, 5);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == ARC_G {
            fx.mem.set16(D + 0xB8, 0x24);
            target_point(fx, sp + 0x60, 0x01E0_39EC, 39.0);
            a3 = arcs_spun(fx, t, 0x20, sp + 0x60, a3);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (f, ra) in [(ARC_S, 0x01E0_3A48u32), (ARC_N, 0x01E0_3A98)] {
            if fx.mem.read_u32(t + 0x44) != f {
                continue;
            }
            fx.mem.set16(D + 0xB8, 0x18);
            let h = fx.self_handle();
            let s = fx.mem.i16(NODE9);
            node_position_saving(fx, h, s, sp + 0x60, u64::from(D), Some(u64::from(flashes)), ra);
            a3 = arcs_spun(fx, t, 0x21, sp + 0x60, a3);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == SMOKE {
            tex(fx, TEX5);
            cell(fx, t, sp + 0x80, 0x14);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == SPIRAL {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0x90, 0x97);
            let h = fx.mem.i16(D + 0x9A) >> 1;
            fx.mem.set16(D + 0x9A, h);
            let v = fx.mem.i16(D + 0x96) + fx.mem.i16(D + 0x9A);
            fx.mem.set16(D + 0x96, v);
            target_point(fx, D + 0x20, 0x01E0_3BB8, 37.0);
            fx.mem.set16(D + 0xBE, 1);
            let (ha, hb) = hists(fx, t);
            a3 = ribbon(fx, t, ha, hb, a3);
        }
        if fx.mem.read_u32(t + 0x44) == BUBBLE {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0xA0, 0x9B);
            let n = if fx.mem.i16(t + 0x52) != 0 { 7 } else { 8 };
            fx.mem.set16(D + 0xB8, n);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == RIM {
            fx.mem.set16(D + 0xB8, 0x24);
            target_point(fx, sp + 0x60, 0x01E0_3CBC, 39.0);
            billboard::arc_strips(fx, D, t, 0x20, sp + 0x60);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    if flashes != 0 {
        unit_flash(fx, [u64::from(D), u64::from(flashes), u64::from(live)], a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x12C => 1,
        0x1F9 => 4,
        0x1EF => 2,
        0x137..=0x1E9 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The target's position (0x1F71A0, from the entry with s0 = D) at `out`
/// with y = `y`.
fn target_point(fx: &mut Fx, out: u32, ra: u32, y: f32) {
    let h = fx.target_handle();
    unit_position_saving(fx, h, out, u64::from(D), ra);
    fx.mem.setf(out + 4, y);
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// The atlas cell `id` frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// The task's history pair: D+0x4470 / D+0x45B0 + 0x280 x i16 +0x50.
fn hists(fx: &mut Fx, t: u32) -> (u32, u32) {
    let i = fx.mem.i16(t + 0x50);
    let base = (D as i32 + 0x280 * i) as u32;
    (base + 0x4470, base + 0x45B0)
}

/// A step's frame with its saves: `slots` (offset, register) and ra at
/// `ra_at` (the entry's `jalr`).
fn enter_step(fx: &mut Fx, frame: u32, ra_at: u32, slots: &[(u32, u64)]) -> u32 {
    let sp = fx.enter(frame);
    fx.saved(sp, slots);
    fx.saved(sp, &[(ra_at, TASK_RA)]);
    sp
}

/// `regs`: the entry's s1 (the flash tasks so far), s3 (the slot), s4
/// (the index) at the `jalr`.
fn run_task(fx: &mut Fx, f: u32, t: u32, regs: [u32; 3]) {
    let d = u64::from(D);
    let s1 = u64::from(regs[0]);
    match f {
        ARC_T => {
            enter_step(fx, 0x20, 0x10, &[(0, d)]);
            arc_t(fx, t);
            fx.leave(0x20);
        }
        MOTES => {
            enter_step(fx, 0x40, 0x30, &[(0x20, 0), (0x10, s1), (0, d)]);
            motes(fx, t);
            fx.leave(0x40);
        }
        ARC_G => {
            enter_step(fx, 0x30, 0x20, &[(0x10, s1), (0, d)]);
            arc_g(fx, t);
            fx.leave(0x30);
        }
        ARC_S => {
            enter_step(fx, 0x30, 0x20, &[(0x10, s1), (0, d)]);
            arc_s(fx, t);
            fx.leave(0x30);
        }
        ARC_N => {
            enter_step(fx, 0x20, 0x10, &[(0, d)]);
            arc_n(fx, t, regs[0]);
            fx.leave(0x20);
        }
        SMOKE => {
            enter_step(fx, 0x20, 0x10, &[(0, d)]);
            smoke(fx, t);
            fx.leave(0x20);
        }
        SPIRAL => {
            let sp = enter_step(fx, 0x70, 0x50, &[(0x40, u64::from(regs[1])), (0x30, 0), (0x20, s1), (0x10, d)]);
            // swc1 f21 / f20: the runner's (0) [H].
            fx.mem.write_u32(sp + 4, 0);
            fx.mem.write_u32(sp, 0);
            spiral(fx, t, sp);
            fx.leave(0x70);
        }
        BUBBLE => {
            enter_step(fx, 0x30, 0x20, &[(0x10, s1), (0, d)]);
            bubble(fx, t);
            fx.leave(0x30);
        }
        RIM => {
            enter_step(fx, 0x30, 0x20, &[(0x10, s1), (0, d)]);
            rim(fx, t);
            fx.leave(0x30);
        }
        OBJECT => {
            let sp = enter_step(fx, 0x30, 0x10, &[(0, d)]);
            object(fx, t, sp);
            fx.leave(0x30);
        }
        _ => panic!("b771: unknown task step {f:#x}"),
    }
}

/// 0x1E03DE0(D, step, 0): the first free task cleared and set off at
/// D+0x250 with the colour D+0xA8C4 twice and the index D+0xA926 (+0x50,
/// +0x52). Frame 0x60 (ra, s4 .. s0: the emitter's s0 = D / s1, the
/// entry's s2 = 0 / s3 = the parameter, the runner's s4 (0 [H])).
fn spawn(fx: &mut Fx, f: u32, s1: u64, p: u64, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, p), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
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
            l2::to_i16x4(fx, t, POS, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(POS + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x50, a);
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x52, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E01630(D, 0) (frame 0x50: ra +0x20, the entry's s1 = 0 +0x10, s0 =
/// D): the update counter, the point and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    let d = u64::from(D);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x20, 0x01E0_3850), (0x10, 0), (0, d)]);
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    let c = counter(fx);
    if c < 0x9B {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE9);
        node_position_saving(fx, h, s, POINT, d, Some(0), 0x01E0_169C);
    } else {
        if c == 0x9B {
            let h = fx.target_handle();
            let s = fx.mem.i16(TNODE);
            node_position_saving(fx, h, s, sp + 0x30, d, Some(0), 0x01E0_16D0);
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(POINT + 8)), 32.0);
            fx.mem.setf(D + 0xA898, v);
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x34), fx.mem.f(POINT + 4)), 32.0);
            fx.mem.setf(D + 0xA894, v);
        }
        if counter(fx) < 0xBA {
            let v = ee::add(fx.mem.f(POINT + 8), fx.mem.f(D + 0xA898));
            fx.mem.setf(POINT + 8, v);
            let v = ee::add(fx.mem.f(POINT + 4), fx.mem.f(D + 0xA894));
            fx.mem.setf(POINT + 4, v);
        }
    }
    let go = |fx: &mut Fx, f: u32, s1: u64, ra: u32| {
        spawn(fx, f, s1, p, ra);
    };
    if counter(fx) == 0x76 {
        go(fx, ARC_S, 0, 0x01E0_17B4);
    }
    let c = counter(fx);
    if (0x26..0x47).contains(&c) {
        node9(fx, 0x01E0_17F8);
        fx.mem.set16(INDEX, 1);
        go(fx, SMOKE, 0, 0x01E0_181C);
    }
    let c = counter(fx);
    if (0x76..0x1DC).contains(&c) {
        if c < 0x119 {
            fx.mem.set16(INDEX, 0);
            go(fx, BUBBLE, 0, 0x01E0_186C);
            fx.mem.set16(INDEX, 1);
            go(fx, BUBBLE, 0, 0x01E0_1890);
            go(fx, ARC_N, 0, 0x01E0_18A4);
        }
        node9(fx, 0x01E0_18C4);
        fx.mem.set16(INDEX, 0);
        go(fx, SMOKE, 0, 0x01E0_18E4);
        if counter(fx) >= 0x118 {
            go(fx, SMOKE, 0, 0x01E0_1910);
        }
        if counter(fx).wrapping_rem(2) == 0 {
            go(fx, ARC_T, 0, 0x01E0_194C);
        }
    }
    let mut s1 = 0u64;
    if counter(fx) == 0xBA {
        for k in 0..4 {
            let w = fx.mem.read_u32(POINT + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        fx.mem.set16(INDEX, 0);
        for k in 0..10u64 {
            go(fx, BUBBLE, k, 0x01E0_19B4);
            s1 = k + 1;
        }
    }
    let c = counter(fx);
    if (0xC8..0x1DC).contains(&c) && c.wrapping_rem(2) == 1 {
        for ra in [0x01E0_1A48u32, 0x01E0_1AB4] {
            let i = fx.mem.i16(NEXT);
            fx.mem.set16(INDEX, i);
            go(fx, SPIRAL, s1, ra);
            let i = fx.mem.i16(NEXT) + 1;
            fx.mem.set16(NEXT, i);
            if fx.mem.i16(NEXT) >= 0x28 {
                fx.mem.set16(NEXT, 0);
            }
        }
    }
    if counter(fx) < 0x190 {
        fx.mem.write_u32(SCALE, 0x40A0_0000);
    } else {
        let v = ee::add(fx.mem.f(SCALE), f32::from_bits(0x3E85_1EB8));
        fx.mem.setf(SCALE, v);
        let h = fx.target_handle();
        unit_position_saving(fx, h, POS, d, 0x01E0_1B5C);
        fx.mem.setf(POS + 4, 37.0);
        if counter(fx) < 0x1DC {
            go(fx, MOTES, s1, 0x01E0_1B90);
        }
    }
    if counter(fx) == 1 {
        let h = fx.target_handle();
        unit_position_saving(fx, h, POS, d, 0x01E0_1BBC);
        fx.mem.setf(POS + 4, 37.0);
        go(fx, ARC_G, s1, 0x01E0_1BD8);
        go(fx, RIM, s1, 0x01E0_1BEC);
        go(fx, FLASH, s1, 0x01E0_1C00);
    }
    if counter(fx) == 0x2A {
        go(fx, OBJECT, s1, 0x01E0_1C2C);
    }
    fx.leave(0x50);
}

/// The attacker's node 9 into D+0x250 (from the emitter, s0 = D, s1 = 0).
fn node9(fx: &mut Fx, ra: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i16(NODE9);
    node_position_saving(fx, h, s, POS, u64::from(D), Some(0), ra);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn odd(v: i32) -> bool {
    v.wrapping_rem(2) == 1
}

/// Alpha byte `a`: + `up` below `cap` while `rising`, else - `down` from
/// `floor` up, else the task ends.
fn fade(fx: &mut Fx, t: u32, a: u32, rising: bool, cap: u8, up: u8, floor: u8, down: u8) {
    let v = fx.mem.read_u8(t + a);
    if rising {
        if v < cap {
            fx.mem.write_u8(t + a, v.wrapping_add(up));
        }
    } else if v >= floor {
        fx.mem.write_u8(t + a, v.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// Only the fall: - `down` from `floor` up, else the task ends.
fn fall(fx: &mut Fx, t: u32, a: u32, floor: u8, down: u8) {
    let v = fx.mem.read_u8(t + a);
    if v >= floor {
        fx.mem.write_u8(t + a, v.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// Alpha +0x63 = 0x60 for the life +6, then out by 11.
fn hold(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        fall(fx, t, 0x63, 0xD, 0xB);
    }
}

/// The colour at `c` by the age's parity: `odd_c` on odd updates, else
/// `even_c`.
fn flicker(fx: &mut Fx, t: u32, c: u32, odd_c: (i32, i32, i32), even_c: (i32, i32, i32)) {
    let v = if odd(fx.mem.i16(t + 0x54)) { odd_c } else { even_c };
    l2::rgb(fx, t + c, v.0, v.1, v.2);
}

/// The first update's arc setup: colours, span / step 0xF, alive 2, +0x67
/// cleared.
fn arc_first(fx: &mut Fx, t: u32, c0: (i32, i32, i32), c1: (i32, i32, i32), span: i32, clear52: bool) {
    l2::rgb(fx, t + 0x60, c0.0, c0.1, c0.2);
    l2::rgb(fx, t + 0x64, c1.0, c1.1, c1.2);
    if clear52 {
        fx.mem.set16(t + 0x52, 0);
    }
    fx.mem.set16(t + 0x5A, span);
    fx.mem.set16(t + 0x5C, span);
}

/// 0x1E01C40 (D, t; frame 0x20): an arc ring growing 25 a update from 10
/// (outer = inner - 15), grey over flickering green; in by 0x40 below
/// 0xC0 for 4 updates, then out by 11.
fn arc_t(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        arc_first(fx, t, (0x40, 0x40, 0x40), (0x80, 0xFF, 0x80), 0xF, false);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 4;
    fade(fx, t, 0x63, rising, 0xC0, 0x40, 0xD, 0xB);
    flicker(fx, t, 0x64, (0x80, 0xFF, 0x80), (0x80, 0xC0, 0x80));
    let r = ee::add(fx.mem.f(t + 0x48), 25.0);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, 15.0));
    age(fx, t);
}

/// 0x1E01D90 (D, t; frame 0x40): a grey mote ring (size 10, cell 5,
/// random angles, radius 6 s + rand % (5 s) by the scale s = D+0xA8E4,
/// growing rand % 3 + 1; row rand & 7 + 1, life rand % 3 + 1), rising 2.5
/// a update; in by 0x40 below 0xC0, out by 13.
fn motes(fx: &mut Fx, t: u32) {
    let at = MOTES;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.set16(t + 0x56, 5);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        for (o, ra) in [(0x20u32, at + 0x54), (0x24, at + 0x70), (0x28, at + 0x8C)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let x = ee::mul(5.0, fx.mem.f(SCALE));
        fx.fix_residue(x, at + 0xB8);
        let m = ee::fixsfsi(x) as i16 as i32;
        let v = rmod(fx, t, at + 0xC4, m);
        let x = ee::add(ee::mul(6.0, fx.mem.f(SCALE)), ee::from_i32(v));
        fx.fix_residue(x, at + 0x104);
        fx.mem.set16(t + 0x50, ee::fixsfsi(x));
        let v = rmod(fx, t, at + 0x10C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, at + 0x128, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, at + 0x148);
        fx.mem.set16(t + 8, (v & 7) + 1);
        let v = rmod(fx, t, at + 0x168, 3);
        fx.mem.set16(t + 6, v + 1);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, 0x63, rising, 0xC0, 0x40, 0xF, 0xD);
    let y = ee::add(fx.mem.f(t + 0x14), 2.5);
    fx.mem.setf(t + 0x14, y);
    age(fx, t);
}

/// 0x1E01FB0 (D, t; frame 0x30): a pale arc ring of radius 10 s (s =
/// D+0xA8E4, read again every update), over flickering green; in by 0x40
/// below 0xC0 for 4 updates, out by 11 from update 0x1E6.
fn arc_g(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let r = ee::mul(10.0, fx.mem.f(SCALE));
        fx.mem.setf(t + 0x48, r);
        fx.mem.write_u32(t + 0x4C, 0);
        arc_first(fx, t, (0xCF, 0xCF, 0xCF), (0x80, 0xFF, 0x80), 0xA, true);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade(fx, t, 0x63, true, 0xC0, 0x40, 0, 0);
    } else if counter(fx) >= 0x1E6 {
        fall(fx, t, 0x63, 0xD, 0xB);
    }
    flicker(fx, t, 0x64, (0x80, 0xFF, 0x80), (0x80, 0xC0, 0x80));
    let r = ee::mul(10.0, fx.mem.f(SCALE));
    fx.mem.setf(t + 0x48, r);
    age(fx, t);
}

/// 0x1E02130 (D, t; frame 0x30): an arc ring of radius 110 facing the
/// camera (0x205580, turned 0x4000), in by 12 below 0x80 until update
/// 0x1E0, then out by 11; from 0x119 shrinking by 5 down to 60.
fn arc_s(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42DC_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        arc_first(fx, t, (0x40, 0x40, 0x40), (0x80, 0xFF, 0x80), 0xF, false);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = counter(fx) < 0x1E0;
    fade(fx, t, 0x63, rising, 0x80, 0xC, 0xD, 0xB);
    flicker(fx, t, 0x64, (0x80, 0xFF, 0x80), (0x80, 0xC0, 0x80));
    if counter(fx) >= 0x119 {
        let r = fx.mem.f(t + 0x48);
        if !(r <= 60.0) {
            fx.mem.setf(t + 0x48, ee::sub(r, 5.0));
        }
    }
    face(fx, t, u64::from(D), 0x01E0_22A0);
    age(fx, t);
}

/// 0x205580 into the angles +0x20 (its frames below the step's: s0 = the
/// task, s1 `s1`), then +0x20 up by 0x4000.
fn face(fx: &mut Fx, t: u32, s1: u64, ra: u32) {
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), s1, ra);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
}

/// 0x1E022D0 (D, t; frame 0x20): a dim arc ring of radius 20 + rand % 80
/// growing 1 a update, facing the camera, over flickering dark green; in
/// by 0x30 below 0x60 for 2 updates, then out by 11. `s1` the entry's s1.
fn arc_n(fx: &mut Fx, t: u32, s1: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_22F4, 0x50);
        fx.mem.setf(t + 0x48, ee::add(20.0, ee::from_i32(v)));
        fx.mem.write_u32(t + 0x4C, 0);
        arc_first(fx, t, (0x48, 0x48, 0x48), (0x40, 0x80, 0x40), 0xF, false);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 2;
    fade(fx, t, 0x63, rising, 0x60, 0x30, 0xD, 0xB);
    flicker(fx, t, 0x64, (0x20, 0x80, 0x20), (0x20, 0x40, 0x20));
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, r);
    }
    face(fx, t, u64::from(s1), 0x01E0_2430);
    age(fx, t);
}

/// 0x1E02460 (D, t; frame 0x20): a smoke ring (size rand % 6 + 6, cell
/// rand & 3, grey 0x80 for +0x52 = 0 / 0xC0 for 1, random angles, radius
/// rand % 30 + 10 / rand % 20 + 10 growing rand % 3 + 10, row rand & 7 + 1,
/// life rand % 3 + 2 at 0x60, then out by 11); kind 0 turns 3 and sinks
/// 1.5 a update.
fn smoke(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2484, 6);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 6));
        let v = fx.rand_from(t, 0x01E0_24B0);
        fx.mem.set16(t + 0x56, v & 3);
        if fx.mem.i16(t + 0x52) == 0 {
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        }
        if fx.mem.i16(t + 0x52) == 1 {
            l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        }
        for (o, ra) in [(0x20u32, 0x01E0_2514u32), (0x24, 0x01E0_2530), (0x28, 0x01E0_254C)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        if fx.mem.i16(t + 0x52) == 0 {
            let v = rmod(fx, t, 0x01E0_2578, 0x1E);
            fx.mem.set16(t + 0x50, v + 0xA);
        }
        if fx.mem.i16(t + 0x52) == 1 {
            let v = rmod(fx, t, 0x01E0_25AC, 0x14);
            fx.mem.set16(t + 0x50, v + 0xA);
        }
        let v = rmod(fx, t, 0x01E0_25D0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_25EC, 3);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, 0x01E0_260C);
        fx.mem.set16(t + 8, (v & 7) + 1);
        let v = rmod(fx, t, 0x01E0_262C, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t);
    if fx.mem.i16(t + 0x52) == 0 {
        let a = fx.mem.i16(t + 0x5A) + 3;
        fx.mem.set16(t + 0x5A, a);
        let y = ee::sub(fx.mem.f(t + 0x14), 1.5);
        fx.mem.setf(t + 0x14, y);
    }
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
}

fn sin(fx: &mut Fx, deg: i32, cos: i32) -> f32 {
    l2::sin_deg(fx, SINES, deg as i16 as i32, cos)
}

/// One history point at sp + 0x60 (x = r cos, z = r sin of `deg`, y
/// already at sp + 0x64).
fn hist_point(fx: &mut Fx, sp: u32, r: f32, deg: i32) {
    let s = sin(fx, deg, 0);
    fx.mem.setf(sp + 0x68, ee::mul(r, s));
    let c = sin(fx, deg, 1);
    fx.mem.setf(sp + 0x60, ee::mul(r, c));
}

/// 0x1E026F0 (D, t; frame 0x70, s3 = D, s2 = t): a spiral ribbon pair
/// about the target (D+0x20 at draw time): radius s = D+0xA8E4, the angle
/// +0x5A ((rand >> 4) % 360) turning 12 a update (the way (rand >> 4) & 1
/// picks), the second history 30 degrees behind and +0xA / 10 higher (+0xA
/// = rand % 6 + 2); both rising +0x34 (1.5 a update from age 5); 20-point
/// histories filled at the start, n = +6 points (2, up to 0x12); green
/// flickering, in by 0x15 below 0x80 for 10 updates, then out by 9.
fn spiral(fx: &mut Fx, t: u32, sp: u32) {
    let (ha, hb) = hists(fx, t);
    let mut f20;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, D, 0x01E0_272C, 6);
        let h = fx.mem.i16(D + 0xA922) + v + 2;
        fx.mem.set16(t + 0xA, h);
        f20 = ee::div(ee::from_i32(h as i16 as i32), 10.0);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        let s0 = sp + 0x64;
        fx.mem.write_u32(s0, 0);
        let v = fx.rand_from(s0, 0x01E0_279C);
        fx.mem.set16(t + 0x5A, (v >> 4) % 0x168);
        let v = fx.rand_from(s0, 0x01E0_27BC);
        fx.mem.set16(t + 0x52, (v >> 4) & 1);
        let r = fx.mem.f(SCALE);
        fx.mem.setf(t + 0x48, r);
        let a = fx.mem.i16(t + 0x5A);
        hist_point(fx, sp, r, a);
        for k in 0..0x13u32 {
            let e = ha + 0x10 * k;
            for (c, o) in [(0u32, 0x60u32), (4, 0x64), (8, 0x68)] {
                let w = fx.mem.read_u32(sp + o);
                fx.mem.write_u32(e + c, w);
            }
        }
        let a = fx.mem.i16(t + 0x5A) - 0x1E;
        hist_point(fx, sp, r, a);
        fx.mem.setf(sp + 0x64, f20);
        for k in 0..0x13u32 {
            let e = hb + 0x10 * k;
            for (c, o) in [(0u32, 0x60u32), (4, 0x64), (8, 0x68)] {
                let w = fx.mem.read_u32(sp + o);
                fx.mem.write_u32(e + c, w);
            }
        }
        fx.mem.set16(t + 6, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < 0xA;
    fade(fx, t, 0x63, rising, 0x80, 0x15, 0xB, 9);
    f20 = ee::div(ee::add(ee::from_i32(fx.mem.i16(t + 0xA)), fx.mem.f(t + 0x34)), 10.0);
    let a = fx.mem.i16(t + 0x5A);
    let a = if fx.mem.i16(t + 0x52) != 0 { a + 0xC } else { a - 0xC };
    fx.mem.set16(t + 0x5A, a);
    let a = fx.mem.i16(t + 0x5A);
    if a >= 0x168 {
        fx.mem.set16(t + 0x5A, a % 0x168);
    }
    let y = ee::add(ee::from_i32(fx.mem.i16(t + 8)), fx.mem.f(t + 0x34));
    fx.mem.setf(sp + 0x64, y);
    let r = fx.mem.f(t + 0x48);
    let a = fx.mem.i16(t + 0x5A);
    hist_point(fx, sp, r, a);
    spawn::history(fx, ha + 0x130, 0x14, sp + 0x60);
    let y = ee::add(fx.mem.f(t + 0x34), ee::add(f20, ee::from_i32(fx.mem.i16(t + 8))));
    fx.mem.setf(sp + 0x64, y);
    let a = fx.mem.i16(t + 0x5A) - 0x1E;
    hist_point(fx, sp, r, a);
    spawn::history(fx, hb + 0x130, 0x14, sp + 0x60);
    flicker(fx, t, 0x60, (0x20, 0x50, 0x20), (0x10, 0x40, 0x10));
    if fx.mem.i16(t + 0x54) != 0 {
        let n = fx.mem.i16(t + 6);
        if n < 0x12 {
            fx.mem.set16(t + 6, n + 1);
        }
    }
    if fx.mem.i16(t + 0x54) >= 5 {
        let y = ee::add(fx.mem.f(t + 0x34), 1.5);
        fx.mem.setf(t + 0x34, y);
    }
    age(fx, t);
}

/// 0x1E02C10 (D, t; frame 0x30, s1 = D): a green bubble ring at the
/// attacker's node 9 (size rand % 5 + 2 / + 6 by +0x52, cell rand & 3,
/// random angles; radius / growth by the update and the kind; row rand &
/// 7 + 1, life rand % 3 + 2 at 0x60, then out by 11); the radius grows by
/// +0xE a update below update 0xDC, by twice that to 0x117.
fn bubble(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x52) == 0 {
            let v = rmod(fx, t, 0x01E0_2C48, 5);
            fx.mem.setf(t + 0x48, ee::from_i32(v + 2));
        }
        if fx.mem.i16(t + 0x52) == 1 {
            let v = rmod(fx, t, 0x01E0_2C88, 5);
            fx.mem.setf(t + 0x48, ee::from_i32(v + 6));
        }
        let v = fx.rand_from(t, 0x01E0_2CB8);
        fx.mem.set16(t + 0x56, v & 3);
        l2::rgb(fx, t + 0x60, 0x50, 0xC0, 0x50);
        for (o, ra) in [(0x20u32, 0x01E0_2CECu32), (0x24, 0x01E0_2D08), (0x28, 0x01E0_2D24)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let early = counter(fx) < 0x118;
        let rows: [(i32, i32, u32, i32, i32, u32); 2] = if early {
            [(0x64, 0x32, 0x01E0_2D60, 0x14, 5, 0x01E0_2D80), (0x1E, 0xA, 0x01E0_2DB4, 3, 0xA, 0x01E0_2DD4)]
        } else {
            [(0x32, 0x1E, 0x01E0_2E08, 0xA, 5, 0x01E0_2E28), (0xF, 0xA, 0x01E0_2E5C, 3, 4, 0x01E0_2E7C)]
        };
        for (kind, (m, add, ra, gm, gadd, gra)) in rows.into_iter().enumerate() {
            if fx.mem.i16(t + 0x52) != kind as i32 {
                continue;
            }
            let v = rmod(fx, t, ra, m);
            fx.mem.set16(t + 0x50, v + add);
            let v = rmod(fx, t, gra, gm);
            fx.mem.set16(t + 0xE, v + gadd);
        }
        let v = rmod(fx, t, 0x01E0_2EA0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_2EBC);
        fx.mem.set16(t + 8, (v & 7) + 1);
        let v = rmod(fx, t, 0x01E0_2EDC, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODE9);
    node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_2F60);
    let c = counter(fx);
    if c < 0xDC {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    } else if c < 0x118 {
        let r = fx.mem.i16(t + 0x50) + 2 * fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02FD0 (D, t; frame 0x30): the green rim (arc strips, inner / outer
/// 8 s, the outer 30 up rising 3 a update from update 0x191), grey 0x80
/// inside over flickering green; +0x67 in by 0x40 below 0x80 for 4
/// updates, out by 11 from update 0x1E6.
fn rim(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let r = ee::mul(10.0, fx.mem.f(SCALE));
        fx.mem.setf(t + 0x4C, r);
        let r = ee::mul(10.0, fx.mem.f(SCALE));
        fx.mem.setf(t + 0x4C, r);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x60, 0x80, 0xFF, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.write_u32(t + 0x34, 0x41F0_0000);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade(fx, t, 0x67, true, 0x80, 0x40, 0, 0);
    } else if counter(fx) >= 0x1E6 {
        fall(fx, t, 0x67, 0xD, 0xB);
    }
    flicker(fx, t, 0x60, (0x80, 0xFF, 0x80), (0x80, 0xC0, 0x80));
    let r = ee::mul(8.0, fx.mem.f(SCALE));
    fx.mem.setf(t + 0x4C, r);
    let r = ee::mul(8.0, fx.mem.f(SCALE));
    fx.mem.setf(t + 0x48, r);
    if counter(fx) >= 0x191 {
        let y = ee::add(fx.mem.f(t + 0x34), 3.0);
        fx.mem.setf(t + 0x34, y);
    }
    age(fx, t);
}

/// 0x1E031B0 (D, t; frame 0x30): the object op 0x1F8380(3, 0, the
/// record's words 0x1E04A00 / 0x1E04A08 at sp + 0x28 / + 0x2C, 0) each
/// update; the task ends from age 0x23.
fn object(fx: &mut Fx, t: u32, sp: u32) {
    for (to, from) in [(0x28u32, 0x01E0_4A00u32), (0x2C, 0x01E0_4A08)] {
        let w = fx.mem.read_u32(from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.boundary(0x001F_8380, "object op 0x1F8380", [3, 0, sp + 0x28, sp + 0x2C]);
    age(fx, t);
    if fx.mem.i16(t + 0x54) >= 0x23 {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E03230(D, t) (frame 0x50: ra, s1 = the unit flash's index, s0 = D):
/// b252's flash sprite (the target over the window, green (0x10, 0xC0,
/// 0x10), UVs inset by +8 / +0xA tenths over 512) into the descriptor at
/// D+0xA8D8 once; in by 0xA below 0x80 until update 0x1E5, then out by 9.
fn flash(fx: &mut Fx, t: u32, k: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_4944), (0x20, u64::from(k)), (0x10, u64::from(D))]);
    // swc1 f21 / f20: the runner's (0) [H].
    fx.mem.write_u32(sp + 4, 0);
    fx.mem.write_u32(sp, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x10, 0xC0, 0x10);
        fx.mem.set16(t + 0x58, 2);
    }
    let hs = fx.self_handle();
    let s = fx.mem.i16(NODE9);
    node_position_saving(fx, hs, s, sp + 0x40, u64::from(t), Some(u64::from(D)), 0x01E0_32D8);
    let hs = fx.self_handle();
    unit_position_saving(fx, hs, sp + 0x40, u64::from(t), 0x01E0_32EC);
    let d = fx.mem.read_u32(FLASH_DESC);
    for c in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + c);
        fx.mem.write_u8(d + 0x10 + c, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    fx.mem.setf(d + 0x18, ee::div(w, 512.0));
    fx.mem.setf(d + 0x1C, ee::div(h, 512.0));
    fx.mem.setf(d + 0x28, ee::div(ee::sub(512.0, w), 512.0));
    fx.mem.setf(d + 0x2C, ee::div(ee::sub(512.0, ee::mul(3.0, h)), 512.0));
    fx.submit(d, 0, 0);
    let rising = counter(fx) < 0x1E5;
    fade(fx, t, 0x63, rising, 0x80, 0xA, 0xB, 9);
    age(fx, t);
    fx.leave(0x50);
}

/// 0x1E04840(D) (b264's 0x1E04800 on this record's globals, the target
/// drawn): in a packet layer (0x2078A0) the target drawn into the render
/// target (0x1F8270 filter 0, ALPHA 0x48), TEXFLUSH before and after;
/// every flash task stepped; the layer linked into the ordering table at
/// key 0. `regs` the entry's s0..s2.
fn unit_flash(fx: &mut Fx, regs: [u64; 3], a3: u32) {
    let sp = fx.enter(0xA0);
    fx.saved(sp, &[(0x30, 0x01E0_3D14), (0x20, regs[2]), (0x10, regs[1]), (0, regs[0])]);
    let d = sp + 0x48;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.target_handle();
    let (_, r) = fx.unit_draw(h, 0, sp + 0x88, sp + 0x90);
    fx.texflush(r);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, k);
        }
    }
    let layer = fx.mem.read_u32(LAYER);
    fx.layer_end(layer, OT, 0);
    fx.leave(0xA0);
}

/// The low word of the last XYZ2 of the last primitive submitted since
/// `before`, else `old`.
fn last_xyz2(fx: &mut Fx, before: usize, old: u32) -> u32 {
    match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => old,
    }
}

/// 7b136f4ab143 (0x1E00F60, this record's only user): `arc_strips_flat`
/// with both angles moved by +0x52 too (the k-th arc from +0x52 + +0x5A +
/// a to that + +0x5C) [F word-compare against b279's 0x1E00D70]. Returns
/// a3 as the last submit leaves it (as b264's `last_xyz2`) [H].
fn arcs_spun(fx: &mut Fx, t: u32, mode: i32, pos: u32, a3: u32) -> u32 {
    let before = fx.out.prims.len();
    let sp = fx.enter(0x200);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    let span = fx.mem.i16(t + 0x5C);
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
    fx.mem.set32(sp + 0xC4, 0);
    fx.mem.set32(sp + 0xD4, 0);
    fx.mem.set32(sp + 0xE4, 0);
    fx.mem.set32(sp + 0xF4, 0);
    let (inner, outer) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let depth = mode & 0xF;
    let tab = D + 0xD8;
    let mut a = 0i32;
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xB8) {
        let s4 = a as i16 as i32;
        let s = (fx.mem.i16(t + 0x52) + (fx.mem.i16(t + 0x5A) + s4)) as i16 as i32;
        let e = (fx.mem.i16(t + 0x52) + (s4 + (fx.mem.i16(t + 0x5A) + span))) as i16 as i32;
        for (deg, r, x, z) in [(s, inner, 0xC0u32, 0xC8u32), (s, outer, 0xE0, 0xE8), (e, inner, 0xD0, 0xD8), (e, outer, 0xF0, 0xF8)] {
            let c = l2::sin_deg(fx, tab, deg, 1);
            fx.mem.setf(sp + x, ee::mul(r, c));
            let sn = l2::sin_deg(fx, tab, deg, 0);
            fx.mem.setf(sp + z, ee::mul(r, sn));
        }
        if depth <= 1 && l2::project(fx, sp + 0xC0, SPR, sp + 0x120, d, sp + 0x160) {
            let ot = fx.mem.read_u32(D + 0x80);
            if depth == 0 {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                fx.submit(d, ot, key);
            } else {
                l2::far_z_desc(fx, d, 1);
                fx.submit(d, ot, 0);
            }
        }
        k += 1;
        a = (a + fx.mem.i16(t + 0x5A)) as i16 as i32;
    }
    fx.leave(0x200);
    if depth == 1 {
        last_xyz2(fx, before, a3)
    } else {
        a3
    }
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

/// a907e1567858 (0x1E03F80; 0.96 b191's 0x1E03C40, ported there): b191's
/// textured strip3 ribbon between the histories `a` / `b`, but keyed by
/// the mean w (0x1A2C70 of sp + 0x110) instead of at the far Z with key 0
/// [F word-compare: the only difference].
fn ribbon(fx: &mut Fx, t: u32, a: u32, b: u32, a3: u32) -> u32 {
    let mut a3 = a3;
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
                fx.fix_residue(w, 0x01E0_47A0);
                let key = ee::fixunssfsi(w) as i32;
                let ot = fx.mem.read_u32(D + 0x80);
                fx.submit(d, ot, key);
                a3 = 0;
            }
        }
        s0 += 1;
    }
    fx.leave(0x390);
    a3
}
