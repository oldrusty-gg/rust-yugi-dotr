//! BMODULE b481 (card 441), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03C90, data area 0x1ED0000, 0xCF30 bytes) [F]
//!: b256's layout (card 630, `b256.rs`): 400
//! tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step +0x44 through
//! `jalr`, the stub 0x1E03C80 skipped), b522's spawn 0x1E04590 (colour
//! D+0xCB14, kind D+0xCBB0 at +0x50 / +0x52, angle D+0xCBA2 at +0x5A),
//! twenty 20-point histories D+0xB1D0 + 0x140 k; no unit flash. Nodes: the
//! attacker's 0x14 / 0x1C / 0x26 / 0x2E / 0x39 / 0x37 / 0x35 / 0x34
//! (D+0xCB44..), the target's 1; timer 0x46.
//!
//! The emitter 0x1E020F0 counts D+0xCBB8: 0x1D at nodes 0x14 / 0x1C and
//! 0x20 at nodes 0x26 / 0x2E eighteen white motes each (0x1E02910, angles
//! 0..340 by 20, each 1.0 lower); 0x1E a violet ribbon task (0x1E03230:
//! histories 0..3 at nodes 0x39 / 0x37 / 0x35 / 0x34 pushed while held,
//! 4..7 at node 0x2E stacked 1.4 apart; drawn as three ribbons by b256's
//! ribbon 0x1E05050) and a tube task (0x1E02EB0, histories 0..2 as a
//! four-fin gouraud tube 0x1E04740, four pieces a segment, the old width
//! on every fin); 0x1E..0x26 at nodes 0x39 / 0x37 / 0x35 / 0x34 ten violet
//! sparkles a update (0x1E02F70, b256's); 0x24 at node 0x37 under the
//! target's x / z two lilac columns (0x1E02690), an arc fan (0x1E02820),
//! fifteen white rings (0x1E02AE0, b256's ring growing faster) and a grey
//! spark burst (0x1E02D40, `quads4::sparks`). Returns 1 on 0x1B, 4 on
//! 0x41, 2 on 0x25, 8 once the timer D+0xCB10 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
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
const COLUMN: u32 = 0x01E0_2690;
const FAN: u32 = 0x01E0_2820;
const MOTE: u32 = 0x01E0_2910;
const RING: u32 = 0x01E0_2AE0;
const BURST: u32 = 0x01E0_2D40;
const TUBE: u32 = 0x01E0_2EB0;
const SPARKLE: u32 = 0x01E0_2F70;
const RIBBONS: u32 = 0x01E0_3230;
/// The stub step (`jr ra`), skipped by the entry.
const STUB: u32 = 0x01E0_3C80;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3FE8;
const HIST: u32 = D + 0xB1D0;
const TEX: u32 = D + 0xCAF0;
const TIMER: u32 = D + 0xCB10;
const COLOUR: u32 = D + 0xCB14;
const NODES: u32 = D + 0xCB44;
const ANGLE: u32 = D + 0xCBA2;
const KIND: u32 = D + 0xCBB0;
const FACING: u32 = D + 0xCBB4;
const COUNTER: u32 = D + 0xCBB8;
const BYTES: u32 = D + 0xCBBA;
const TAB: u32 = D + 0xCDBC;

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

fn hist_at(k: u32) -> u32 {
    HIST + 0x140 * k
}

/// The entry 0x1E03C90 (s0 = param / the slot pointer, s1 = the index, s2
/// = the live count, s4 = D).
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
    fx.mem.set32(TIMER, 0x46);
    for (k, n) in [0x14, 0x1C, 0x26, 0x2E, 0x39, 0x37, 0x35, 0x34].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xCB54, r);
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k, t);
    }
    for k in 0..4u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(y, 0x01E0_3ECC);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3EFC, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_3F4C);
        fx.mem.set16(D + 0xCB64 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let p = D + 0x70 * k;
        let t = p + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == STUB {
            continue;
        }
        // The step's prologue: the entry's s0 = the pointer, s1 = the
        // index, s2 = 0.
        let frame = match f {
            MOTE | SPARKLE => 0x30,
            RIBBONS => 0x50,
            COLUMN | FAN | RING | BURST | TUBE => 0x20,
            _ => panic!("b481: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match frame {
            0x20 => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(p))]),
            0x30 => fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(k)), (0, u64::from(p))]),
            _ => fx.saved(s, &[(0x30, TASK_RA), (0x20, 0), (0x10, u64::from(k)), (0, u64::from(p))]),
        }
        match f {
            COLUMN => column(fx, t),
            FAN => fan(fx, t, k),
            MOTE => mote(fx, t),
            RING => ring(fx, t),
            BURST => burst(fx, t),
            TUBE => tube_task(fx, t),
            SPARKLE => sparkle(fx, t),
            _ => ribbons(fx, t, s, p),
        }
        fx.leave(frame);
    }
    let mut live = 0u32;
    for k in 0..TASKS {
        let p = D + 0x70 * k;
        let t = p + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == COLUMN {
            tex(fx, D + 0x29C);
            cell(fx, sp + 0x80, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            for _ in 0..2 {
                quads4::sprite_quads(fx, D, t, 0, t + 0x10);
            }
        }
        if f(fx) == FAN {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == MOTE {
            tex(fx, TEX);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x90, 0x14, sub);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
        }
        if f(fx) == RING {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xA0, 0x14, sub);
            fx.mem.set16(D + 0xC0, 0);
            tex(fx, TEX + 8);
            fx.mem.set16(D + 0xB8, 0xA);
            l2::copy3(fx, D + 0x20, t + 0x10);
            fx.mem.set16(D + 0xBE, 0);
            billboard::ring_facing(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == BURST {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xB0, 0x96, sub);
            tex(fx, TEX + 0xC);
            for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x20), (0x9A, 0x7F), (0xB8, 0)] {
                fx.mem.set16(D + o, v);
            }
            l2::copy3(fx, D + 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0x18);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == TUBE {
            for o in [0x20u32, 0x24, 0x28] {
                fx.mem.write_u32(D + o, 0);
            }
            for j in 0..3 {
                tube4(fx, D, t, hist_at(j));
            }
        }
        if f(fx) == SPARKLE {
            tex(fx, TEX + 4);
            let o = fx.mem.i16(t + 0x52);
            fx.mem.set16(D + 0xBE, o);
            let c = fx.mem.i16(t + 0x56);
            if c < 2 {
                fx.mem.set16(D + 0x94, c << 6);
                fx.mem.set16(D + 0x96, 0);
            } else {
                fx.mem.set16(D + 0x94, (c - 2) << 6);
                fx.mem.set16(D + 0x96, 0x40);
            }
            fx.mem.set16(D + 0x98, 0x40);
            fx.mem.set16(D + 0x9A, 0x40);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if f(fx) == RIBBONS {
            // The ribbon's saves: the draw loop's s0..s7 (s7 the runner's,
            // 0 [H]), fp (the runner's, 0 [H]) and the return address.
            match fx.mem.i16(t + 0x50) {
                0 => {
                    for j in 0..3u32 {
                        let saves = [u64::from(p), u64::from(k), u64::from(live), u64::from(D + 0x140 * j), u64::from(D), u64::from(t), u64::from(j), 0, 0, 0x01E0_4408];
                        ribbon(fx, t, hist_at(j), hist_at(j + 1), saves);
                    }
                }
                1 => {
                    for j in 10..13u32 {
                        let saves = [u64::from(p), u64::from(k), u64::from(live), u64::from(D + 0x140 * j), u64::from(D), u64::from(t), u64::from(j), 0, 0, 0x01E0_4488];
                        ribbon(fx, t, hist_at(j), hist_at(j + 4), saves);
                    }
                }
                _ => {}
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x1B => 1,
        0x41 => 4,
        0x25 => 2,
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

/// 0x1E04590(D, step, 0): b256's spawn (b522's); `saves` the caller's
/// s0..s4, `ra` the return address.
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, saves[0]), (0x10, saves[1]), (0x20, saves[2]), (0x30, saves[3]), (0x40, saves[4]), (0x50, u64::from(ra))]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
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
    }
    fx.leave(0x60);
}

/// 0x1E020F0(D, 0): the update counter and the spawns (frame 0x60: ra, the
/// entry's s2 (0) / s1 (the runner's, 0 [H]) / s0 = the parameter; then
/// s0 = D, s1 / s2 loop counts).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x30, 0x01E0_3FA8), (0x20, 0), (0x10, 0), (0, param as i64 as u64)]);
    let d = u64::from(D);
    let (mut s1, mut s2) = (0u64, 0u64);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx, k: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, POS, d, Some(s1), ra);
    };
    // The spawns' saves: s0 = D, s1, s2, the entry's s3 (the runner's, 0
    // [H]) and s4 = D.
    let sv = |s1: u64, s2: u64| [d, s1, s2, 0, d];
    if c(fx) == 0x24 {
        node(fx, 5, s1, 0x01E0_215C);
        let h = fx.target_handle();
        unit_position_saving(fx, h, sp + 0x40, d, 0x01E0_2170);
        let x = fx.mem.read_u32(sp + 0x40);
        fx.mem.write_u32(POS, x);
        let z = fx.mem.read_u32(sp + 0x48);
        fx.mem.write_u32(POS + 8, z);
        fx.mem.set16(ANGLE, 0x2D);
        fx.mem.set16(KIND, 0);
        spawn(fx, COLUMN, sv(s1, s2), 0x01E0_21B0);
        fx.mem.set16(ANGLE, 0x87);
        fx.mem.set16(KIND, 1);
        spawn(fx, COLUMN, sv(s1, s2), 0x01E0_21E4);
        spawn(fx, FAN, sv(s1, s2), 0x01E0_21F8);
        s1 = 0;
        while s1 < 0xF {
            spawn(fx, RING, sv(s1, s2), 0x01E0_2214);
            s1 += 1;
        }
        spawn(fx, BURST, sv(s1, s2), 0x01E0_2248);
    }
    if c(fx) == 0x1E {
        fx.mem.set16(KIND, 0);
        spawn(fx, RIBBONS, sv(s1, s2), 0x01E0_2280);
        spawn(fx, TUBE, sv(s1, s2), 0x01E0_2294);
    }
    if (0x1E..0x27).contains(&c(fx)) {
        for (k, node_ra, ras) in [
            (4u32, 0x01E0_22D8u32, &[0x01E0_22ECu32, 0x01E0_2300, 0x01E0_2314][..]),
            (5, 0x01E0_2334, &[0x01E0_2348, 0x01E0_235C, 0x01E0_2370][..]),
            (6, 0x01E0_2390, &[0x01E0_23A4, 0x01E0_23B8][..]),
            (7, 0x01E0_23D8, &[0x01E0_23EC, 0x01E0_2400][..]),
        ] {
            node(fx, k, s1, node_ra);
            for &ra in ras {
                spawn(fx, SPARKLE, sv(s1, s2), ra);
            }
        }
    }
    for (at, sets) in [(0x1D, [(0u32, 0x01E0_2438u32, 0x01E0_2478u32), (1, 0x01E0_24C4, 0x01E0_2504)]), (0x20, [(2, 0x01E0_2570, 0x01E0_25B0), (3, 0x01E0_25FC, 0x01E0_263C)])] {
        if c(fx) != at {
            continue;
        }
        for (k, node_ra, ra) in sets {
            node(fx, k, s1, node_ra);
            s2 = 0;
            s1 = 0;
            while s2 < 0x12 {
                fx.mem.set16(ANGLE, s1 as i32);
                let y = ee::sub(fx.mem.f(POS + 4), 1.0);
                fx.mem.setf(POS + 4, y);
                spawn(fx, MOTE, sv(s1, s2), ra);
                s1 += 0x14;
                s2 += 1;
            }
        }
    }
    fx.leave(0x60);
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

/// 0x1E02690 (D, task): b256's column in lilac (0xF0, 0xE0, 0xFF) over
/// violet (0x80, 0x40, 0xFF): size 6, by kind +0x50 (18, 1000) / (18,
/// 250) / (18, 400) at +0x30 / +0x34; in by 0x40 under 0x80 for 4 updates,
/// out by 9; from age 1 +0x34 grows by 40 (kind 0) / 20 (kind 1).
fn column(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xF0, 0xE0, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0xFF);
        for (kind, h) in [(0, 0x447A_0000u32), (1, 0x437A_0000), (2, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, 0x4190_0000);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    for (kind, by) in [(0, 40.0f32), (1, 20.0)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            add(fx, t + 0x34, by);
        }
    }
    age(fx, t);
}

/// 0x1E02820 (D, task): b256's arc fan, (0x80, 0x40, 0xFF) inside, (0xE0,
/// 0xD0, 0xFF) outside: outer radius 140, 10-degree arcs every 10 degrees;
/// its outer alpha up by 0x60 below 0xC0 for 10 updates, then down by 9;
/// faces the camera turned by 0x4000. `k` the entry's s1.
fn fan(fx: &mut Fx, t: u32, k: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x430C_0000);
        l2::rgb(fx, t + 0x64, 0xE0, 0xD0, 0xFF);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x67, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x67, 0xB, 9);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(k), 0x01E0_28E0);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02910 (D, task; s1 = D): a white mote: size 100, cell 2..4, thrown
/// 8.6 out along its spawn angle +0x5A, then drifting 1.5 a update, +0x52
/// (rand >> 4) % 4; in by 0x60 under 0xC0 for 6 updates, out by 19;
/// turned by the camera.
fn mote(fx: &mut Fx, t: u32) {
    let drift = |fx: &mut Fx, k: f32| {
        for (at, cos) in [(0x18u32, 0), (0x10, 1)] {
            let a = fx.mem.i16(t + 0x5A);
            let s = l2::sin_deg(fx, TAB, a, cos);
            add(fx, t + at, ee::mul(k, s));
        }
    };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2948);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        drift(fx, f32::from_bits(0x4109_999A));
        let v = fx.rand_from(t, 0x01E0_29E0);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
    }
    if fx.mem.i16(t + 0x54) < 6 {
        fade_in(fx, t, 0x63, 0xC0, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        drift(fx, 1.5);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2AB8);
    age(fx, t);
}

/// 0x1E02AE0 (D, task): b256's white ring growing by (rand >> 4) % 70 +
/// 45 from radius rand % 40 + 5: size 30, cell row (rand >> 4) % 8, angles
/// +0x5A / +0x5C rand % 360, +0xA 2 on odd rands else 0, +0xC the kind,
/// +0x52 rand % 350 + 100, row 0..34, life 3..7; in by 0xC0 under 0xC0, out
/// by 15; grey 0x80 on odd ages, 0x60 on even.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41F0_0000);
        let v = fx.rand_from(t, 0x01E0_2B08);
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = rmod(fx, t, 0x01E0_2B40, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2B5C, 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = fx.rand_from(t, 0x01E0_2B78);
        fx.mem.set16(t + 0xA, if v % 2 != 0 { 2 } else { 0 });
        let kind = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, kind);
        let v = rmod(fx, t, 0x01E0_2BB0, 0x28);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, 0x01E0_2BD0, 0x15E);
        fx.mem.set16(t + 0x52, v + 0x64);
        let v = fx.rand_from(t, 0x01E0_2BF0);
        let r = fx.div(v >> 4, 0x46).1;
        fx.mem.set16(t + 0xE, r + 0x2D);
        let v = rmod(fx, t, 0x01E0_2C14, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2C30, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0xC0, 0xC0);
    } else {
        fade_out(fx, t, 0x63, 0x11, 0xF);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let g = if fx.mem.i16(t + 0x54) % 2 == 1 { 0x80 } else { 0x60 };
    l2::rgb(fx, t + 0x60, g, g, g);
    age(fx, t);
}

/// 0x1E02D40 (D, task): a grey (0x50) spark burst over black: radius 200
/// (the inner 142) widening by (rand % 150 + 400) / 10 a update (the inner
/// a tenth of it), +0x50 (rand >> 4) % 2, +0x5A / +0x5C 30, turned 0x2000
/// about z; at 0x80 for 20 updates, then out by 14.
fn burst(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        let v = fx.rand_from(t, 0x01E0_2D80);
        fx.mem.set16(t + 0x50, (v >> 4) % 2);
        l2::rgb(fx, t + 0x60, 0x50, 0x50, 0x50);
        l2::rgb(fx, t + 0x64, 0, 0, 0);
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        let v = rmod(fx, t, 0x01E0_2DD4, 0x96);
        fx.mem.setf(t + 0x34, ee::from_i32(v + 0x190));
        let q = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, q);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.write_u32(t + 0x28, 0x2000);
    }
    if fx.mem.i16(t + 0x54) < 0x14 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE);
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3DCC_CCCD), r));
    age(fx, t);
}

/// 0x1E02EB0 (D, task): the tube task: violet (0x80, 0x40, 0xFF) both
/// sides every update, size 100, ten points (+6); at 0x80 for 16 updates,
/// then out by 19.
fn tube_task(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
    l2::rgb(fx, t + 0x64, 0x80, 0x40, 0xFF);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x10 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 0x15, 0x13);
    }
    fx.mem.set16(t + 6, 0xA);
    age(fx, t);
}

/// 0x1E02F70 (D, task; s1 = D): b256's sparkle (0x1E02A40) in violet
/// (0x80, 0x40, 0xFF), its rand / camera return addresses 0x530 on.
fn sparkle(fx: &mut Fx, t: u32) {
    const O: u32 = 0x530;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x96);
        let w = fx.mem.i16(t + 8) >> 1;
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2A7C + O);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_2AB0u32), (0x30, 0x01E0_2ADC), (0x38, 0x01E0_2B08)] {
            let v = rmod(fx, t, ra + O, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_2B60 + O, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2B7C + O);
        for (o, ra) in [(4u32, 0x01E0_2B84u32), (2, 0x01E0_2BA4), (0, 0x01E0_2BC4)] {
            let v = fx.rand_from(t, ra + O);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        let v = fx.rand_from(t, 0x01E0_2BE4 + O);
        fx.mem.set16(t + 0x52, (v >> 4) % 4);
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    for (a, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + a).wrapping_add(fx.mem.i16(t + s) as u32);
        fx.mem.write_u32(t + a, v);
    }
    let w = ee::from_i32(fx.mem.i32(t + 0x28));
    let (v, ra) = if fx.mem.i16(FACING) != 0 { (ee::sub(w, 1.5), 0x01E0_2CAC + O) } else { (ee::add(w, 1.5), 0x01E0_2CD0 + O) };
    fx.fix_residue(v, ra);
    fx.mem.set32(t + 0x28, ee::fixsfsi(v));
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2CDC + O);
    age(fx, t);
}

/// 0x1E03230 (D, task; frame 0x50 at `sp`, the entry's pointer `p`): a
/// violet (0xC0, 0x40, 0xFF) ribbon task, size 5, +0xE 30. Kind 0 fills
/// the histories 0..3 with nodes 0x39 / 0x37 / 0x35 / 0x34 and 4..7 with
/// node 0x2E stacked 1.4 down a history (+6 = 10, held 11 updates); kind 1
/// the histories 10..13 with the same nodes and 14..17 with node 0x37
/// stacked (+6 = 19, held 20). Alpha 0xFF for 6 updates, then down by 9;
/// while held (age <= +0x52) kind 0 pushes 0..3 at their nodes, kind 1
/// pushes 10..13 at node 0x39 stacked and 14..17 at node 0x37 stacked
/// (`spawn::history`).
fn ribbons(fx: &mut Fx, t: u32, sp: u32, p: u32) {
    l2::rgb(fx, t + 0x60, 0xC0, 0x40, 0xFF);
    let down = f32::from_bits(0x3FB3_3333);
    let s1 = Some(u64::from(t));
    let at_node = |fx: &mut Fx, n: u32, s0: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * n);
        node_position_saving(fx, h, slot, sp + 0x40, s0, s1, ra);
    };
    let fill = |fx: &mut Fx, h: u32| {
        for e in 0..0x13u32 {
            l2::copy3(fx, h + 0x10 * e, sp + 0x40);
        }
    };
    let lower = |fx: &mut Fx| {
        let y = ee::sub(fx.mem.f(sp + 0x44), down);
        fx.mem.setf(sp + 0x44, y);
    };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        // (kind, the nodes of the first four, their fill return addresses,
        // the stacked node and its return address, first history, +6,
        // +0x52).
        let sets: [(i32, [u32; 4], [u32; 4], u32, u32, u32, i32, i32); 2] = [
            (0, [4, 5, 6, 7], [0x01E0_32A8, 0x01E0_3314, 0x01E0_3384, 0x01E0_33F4], 3, 0x01E0_3464, 0, 0xA, 0xB),
            (1, [4, 5, 6, 7], [0x01E0_3630, 0x01E0_369C, 0x01E0_370C, 0x01E0_377C], 5, 0x01E0_37EC, 10, 0x13, 0x14),
        ];
        for (kind, nodes, ras, stacked, sra, first, six, hold) in sets {
            if fx.mem.i16(t + 0x50) != kind {
                continue;
            }
            fx.mem.set16(t + 0xE, 0x1E);
            for j in 0..4u32 {
                at_node(fx, nodes[j as usize], u64::from(p), ras[j as usize]);
                fill(fx, hist_at(first + j));
            }
            at_node(fx, stacked, u64::from(p), sra);
            for j in 0..4u32 {
                if j != 0 {
                    lower(fx);
                }
                fill(fx, hist_at(first + 4 + j));
            }
            fx.mem.set16(t + 6, six);
            fx.mem.set16(t + 0x52, hold);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 6 {
        fx.mem.write_u8(t + 0x63, 0xFF);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    let held = |fx: &mut Fx| fx.mem.i16(t + 0x54) <= fx.mem.i16(t + 0x52);
    let push = |fx: &mut Fx, h: u32| {
        hist::history(fx, h + 0x130, 0x14, sp + 0x40);
    };
    if fx.mem.i16(t + 0x50) == 0 && held(fx) {
        for (j, ra) in [(0u32, 0x01E0_3A08u32), (1, 0x01E0_3A40), (2, 0x01E0_3A78), (3, 0x01E0_3AB0)] {
            at_node(fx, 4 + j, u64::from(p), ra);
            push(fx, hist_at(j));
        }
    }
    if fx.mem.i16(t + 0x50) == 1 && held(fx) {
        let s0 = u64::from(sp + 0x44);
        for (n, first, s0, ra) in [(4u32, 10u32, u64::from(p), 0x01E0_3B0Cu32), (5, 14, s0, 0x01E0_3BC0)] {
            at_node(fx, n, s0, ra);
            for j in 0..4u32 {
                if j != 0 {
                    lower(fx);
                }
                push(fx, hist_at(first + j));
            }
        }
    }
    age(fx, t);
}

/// 0x1E05050 (D, task, a, b): b256's ribbon (b640's: b703's with the `b`
/// side's alpha down by 3 a strip while above 4, else 0); `saves` the draw
/// loop's s0..s7, fp and the return address.
fn ribbon(fx: &mut Fx, t: u32, a: u32, b: u32, saves: [u64; 10]) {
    use ee::add;
    let sp = fx.enter(0x300);
    let offs = [0u32, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90];
    let list: Vec<(u32, u64)> = offs.iter().zip(saves).map(|(&o, v)| (o, v)).collect();
    fx.saved(sp + 0x10, &list);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 0xD0, a);
    fx.mem.write_u32(sp + 0xC0, b);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x238;
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
    let (pa, pb) = (sp + 0x170, sp + 0x1B0);
    for (p, src) in [(pa, a), (pb, b)] {
        copy4(fx, p, src);
        copy4(fx, p + 0x10, src);
        copy4(fx, p + 0x20, src + 0x10);
        copy4(fx, p + 0x30, src + 0x20);
    }
    rgbaq_at(fx, sp + 0x2C8, ca, d + 0x10);
    rgbaq_at(fx, sp + 0x2D0, ca, d + 0x20);
    rgbaq_at(fx, sp + 0x2D8, cb, d + 0x30);
    rgbaq_at(fx, sp + 0x2E0, cb, d + 0x40);
    fx.mem.set32(sp + 0xB0, n - 1);
    let mut s7 = 0;
    while s7 < n - 1 {
        let off = (0x10 * (s7 - 1)) as u32;
        let (s2, s3) = (a.wrapping_add(off), b.wrapping_add(off));
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            rgbaq_at(fx, sp + 0x2E8, cb, d + 0x30);
            let v = i32::from(fx.mem.read_u8(cb + 3));
            fx.mem.write_u8(cb + 3, if v - 4 > 0 { (v - 3) as u8 } else { 0 });
            rgbaq_at(fx, sp + 0x2F0, cb, d + 0x40);
            let (qa, qb, oa, ob) = if s7 != 0 { (s2, s3, 0x1F0u32, 0x200u32) } else { (pa, pb, 0x210, 0x220) };
            spline::spline(fx, sp + 0x130, qa, f20, 0, [0.0; 4]);
            spline::spline(fx, sp + 0x150, qb, f20, 0, [0.0; 4]);
            f20 = add(f20, 0.25);
            spline::spline(fx, sp + oa, qa, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x140, sp + oa);
            spline::spline(fx, sp + ob, qb, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x160, sp + ob);
            for (to, from) in [(0xF0u32, 0x130u32), (0x100, 0x140), (0x110, 0x150), (0x120, 0x160)] {
                l2::copy3(fx, sp + to, sp + from);
            }
            if l2::project(fx, sp + 0xF0, SPR, 0, d, sp + 0xE0) {
                fx.submit(d, OT, 0);
            }
        }
        s7 += 1;
    }
    fx.leave(0x300);
}

/// The Lagrange curve (44769f26f5eb's basis 0) through `p` at `t` into
/// `out` (xyz), as `tubetrail`'s.
fn curve(fx: &mut Fx, out: u32, p: [u32; 4], t: f32) {
    use ee::{add, mul};
    let w = spline::weights(t, 0).unwrap_or([0.0; 4]);
    for c in 0..3u32 {
        let v: [f32; 4] = std::array::from_fn(|i| fx.mem.f(p[i] + 4 * c));
        let acc = add(mul(v[2], w[2]), add(mul(v[0], w[0]), mul(v[1], w[1])));
        fx.mem.setf(out + 4 * c, add(acc, mul(v[3], w[3])));
    }
}

/// 0x1E04740 (D, task, trail, _) (e97ec7e99868, 580 words): the gouraud
/// tube (`tubetrail::tube_gouraud`, 9769823e2004 at 0.92) cut into four
/// pieces a segment (t += 0.25, clamped to 1 from 0.9), four fins a piece
/// all pushed out by the piece's starting half width (the width down by
/// size / 20n a piece: +0xDC = 2n), always keyed by the mean w (no depth
/// argument: a3 is unread).
fn tube4(fx: &mut Fx, d0: u32, t: u32, a: u32) {
    use ee::{add, div, sub};
    let sp = fx.enter(0x320);
    let mut f30 = div(fx.mem.f(t + 0x48), 10.0);
    fx.mem.write_u32(sp + 0x100, a);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x270;
    fx.mem.setf(sp + 0xD0, f30);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for (local, c, to) in [(0x300u32, 0x60u32, 0x10u32), (0x308, 0x64, 0x20), (0x310, 0x60, 0x30), (0x318, 0x64, 0x40)] {
        rgbaq_at(fx, sp + local, t + c, d + to);
    }
    fx.desc_alpha(d, 0x80_0000_0048);
    copy4(fx, sp + 0x260, a);
    copy4(fx, sp + 0x130, sp + 0x260);
    copy4(fx, sp + 0x140, sp + 0x260);
    copy4(fx, sp + 0x150, a + 0x10);
    copy4(fx, sp + 0x160, a + 0x20);
    for k in 0..3 {
        let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0x1C0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x1E0, 0, t + 0x20, sp + 0x1C0);
    let w = fx.mem.read_u32(sp + 0x140);
    fx.mem.write_u32(sp + 0xD4, w);
    let w = fx.mem.read_u32(sp + 0x150);
    fx.mem.write_u32(sp + 0xD8, w);
    fx.mem.set32(sp + 0xE0, 0);
    fx.mem.set32(sp + 0xF0, n - 1);
    fx.mem.setf(sp + 0xDC, ee::mul(2.0, ee::from_i32(n)));
    let first = [sp + 0x130, sp + 0x140, sp + 0x150, sp + 0x160];
    let v = sp + 0x180;
    let mut seg = 0i32;
    while seg < n - 1 {
        let mut f29 = 0.0f32;
        let p = (a as i32 + 0x10 * (seg as i16 as i32)) as u32;
        let pts = if seg == 0 { first } else { [p - 0x10, p, p + 0x10, p + 0x20] };
        for piece in 0..4 {
            if seg == 0 && piece == 0 {
                curve(fx, sp + 0x240, pts, f29);
                copy4(fx, sp + 0x110, sp + 0x240);
            } else {
                copy4(fx, sp + 0x110, sp + 0x170);
            }
            f29 = add(f29, 0.25);
            if !(f29 < f32::from_bits(0x3F66_6666)) {
                f29 = 1.0;
            }
            let e = if seg == 0 { 0x250 } else { 0x230 };
            curve(fx, sp + e, pts, f29);
            copy4(fx, sp + 0x120, sp + e);
            copy4(fx, sp + 0x170, sp + 0x120);
            let f31 = f30;
            f30 = sub(f30, div(fx.mem.f(sp + 0xD0), fx.mem.f(sp + 0xDC)));
            for fin in 0..4 {
                l2::copy3(fx, v, sp + 0x110);
                l2::copy3(fx, v + 0x10, sp + 0x110);
                l2::copy3(fx, v + 0x20, sp + 0x120);
                l2::copy3(fx, v + 0x30, sp + 0x120);
                let bump = |fx: &mut Fx, at: u32, up: bool| {
                    let x = fx.mem.f(v + at);
                    fx.mem.setf(v + at, if up { add(x, f31) } else { sub(x, f31) });
                };
                match fin {
                    0 => {
                        bump(fx, 0x14, true);
                        bump(fx, 0x34, true);
                    }
                    1 => {
                        bump(fx, 0x14, false);
                        bump(fx, 0x34, false);
                    }
                    2 => {
                        bump(fx, 0x10, false);
                        bump(fx, 0x30, false);
                    }
                    _ => {
                        bump(fx, 0x10, true);
                        bump(fx, 0x30, true);
                    }
                }
                if l2::project(fx, v, SPR, sp + 0x1E0, d, sp + 0x1D0) {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x1D0)) as i32;
                    fx.submit(d, OT, key);
                }
            }
        }
        seg += 1;
        fx.mem.set32(sp + 0xE0, seg);
    }
    fx.leave(0x320);
}
