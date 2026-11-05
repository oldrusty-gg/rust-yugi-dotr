//! BMODULE b256 (card 630), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04050, data area 0x1ED0000, 0xCF30 bytes) [F]
//!: b287's program (card 335: b703's 400-task pool
//! of 0x70 bytes at D+0x2A0, b522's spawn 0x1E048C0) with b550's unit flash
//! (0x1E04FB0: the flash tasks 0x1E03DA0 counted by the entry, stepped in
//! a packet layer; descriptor D+0xCB28, layer D+0xCB24) and twenty 20-point
//! node histories D+0xB1D0 + 0x140 k. Nodes: the attacker's 0x3F / 0x49 /
//! 0x17 / 0x18 / 0x33 / 0x34 (D+0xCB44..), the target's 1; timer 0xD2.
//!
//! Update 0x17 the stage light (0x1E03A60) and a lilac flash of the target
//! (0x1E03DA0, in by 9 until update 0xB7); 0x61 / 0x76 / 0x80 thirteen
//! white motes at node 0x3F, 0x6B / 0x81 at node 0x49 (1.5 lower); 0x7B /
//! 0x8F a violet ribbon task (0x1E02FC0: kind 0 four histories each of
//! nodes 0x17 / 0x18 stacked 1.4 apart, kind 1 of nodes 0x33 / 0x34, drawn
//! as three ribbons by b640's ribbon 0x1E04A70); 0x7C / 0x92 a lilac frame
//! tint (0x1E03B50); 0x7B..0x86 at node 0x18 and 0x91..0x9B at node 0x34 a
//! white ring (0x1E022E0, `ring_facing`) and four violet sparkles a update
//! (0x1E02A40, `billboard::quad`); on 0x80 / 0x95 there an arc fan
//! (0x1E02D00, `arc_strips`) and two columns (0x1E028B0, `sprite_quads`
//! x2). The quads 0x1E02540 are drawn but never spawned (not ported).
//! Returns 1 on 0x78, 4 on 0xBE, 2 on 0xA5, 6 on 0x83..0x9F, 8 once the
//! timer D+0xCB10 has run out.
use crate::b076::node_position_saving;
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
const RING: u32 = 0x01E0_22E0;
const COLUMN: u32 = 0x01E0_28B0;
const SPARKLE: u32 = 0x01E0_2A40;
const FAN: u32 = 0x01E0_2D00;
const MOTE: u32 = 0x01E0_2DF0;
const RIBBONS: u32 = 0x01E0_2FC0;
const LIGHT: u32 = 0x01E0_3A60;
const TINT: u32 = 0x01E0_3B50;
/// The flash tasks, run by the unit flash 0x1E04FB0.
const FLASH: u32 = 0x01E0_3DA0;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_4370;
const HIST: u32 = D + 0xB1D0;
const AIM: u32 = D + 0xCAD0;
const TEX: u32 = D + 0xCAF0;
const TIMER: u32 = D + 0xCB10;
const COLOUR: u32 = D + 0xCB14;
const LAYER: u32 = D + 0xCB24;
const FLASH_DESC: u32 = D + 0xCB28;
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

/// The last XYZ2 of the last primitive drawn since `before` (0 when none).
fn last_xyz2(fx: &mut Fx, before: usize) -> u32 {
    match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => 0,
    }
}

/// The entry 0x1E04050 (s0 = param / the task pointer, s1 = the index, s2
/// = the flash tasks, s4 = D, s7 = the live count).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    fx.mem.write_u32(D + 0xCB1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xE0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xCF30);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xCBB6, param);
    fx.mem.set32(TIMER, 0xD2);
    for (k, n) in [0x3F, 0x49, 0x17, 0x18, 0x33, 0x34].into_iter().enumerate() {
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
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(y, 0x01E0_4254);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_4284, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_42D4);
        fx.mem.set16(D + 0xCB64 + 2 * k, v % 8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let mut a3 = emit(fx, param);
    let mut flashes = 0u32;
    // a3 as the steps' last calls leave it (when nothing is drawn after
    // them): the ribbon task's colour 0xFF (or its last history's point
    // pointer), a ring's grey; the stage light leaves it.
    for k in 0..TASKS {
        let p = D + 0x70 * k;
        let t = p + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => continue,
            FLASH => {
                flashes += 1;
                continue;
            }
            _ => {}
        }
        // The step's prologue: the entry's s0 = the pointer, s1 = the
        // index, s2 = the flashes so far.
        let frame = match f {
            SPARKLE | MOTE => 0x30,
            RIBBONS => 0x50,
            TINT => 0x80,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        match frame {
            0x20 | 0x80 => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(p))]),
            0x30 => fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(k)), (0, u64::from(p))]),
            _ => fx.saved(s, &[(0x30, TASK_RA), (0x20, u64::from(flashes)), (0x10, u64::from(k)), (0, u64::from(p))]),
        }
        match f {
            RING => a3 = ring(fx, t),
            COLUMN => column(fx, t),
            SPARKLE => sparkle(fx, t),
            FAN => fan(fx, t),
            MOTE => mote(fx, t),
            RIBBONS => a3 = ribbons(fx, t, s, p),
            LIGHT => light(fx, t),
            TINT => tint(fx, t, s),
            _ => panic!("b256: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    // a3 as the draws' last library call leave it, for the unit flash's
    // first 0x207BB0.
    let mut live = 0u32;
    for k in 0..TASKS {
        let p = D + 0x70 * k;
        let t = p + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live += 1;
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == RING {
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x90, 0x14, sub);
            fx.mem.set16(D + 0xC0, 0);
            tex(fx, TEX);
            fx.mem.set16(D + 0xB8, 0xA);
            for c in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * c);
                fx.mem.write_u32(D + 0x20 + 4 * c, w);
            }
            fx.mem.set16(D + 0xBE, 0);
            billboard::ring_facing(fx, uv::uv_rect, D, t, 0x20, BYTES);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == COLUMN {
            tex(fx, D + 0x29C);
            cell(fx, sp + 0xB0, 0x9A, 0);
            fx.mem.set16(D + 0xBE, 0);
            for _ in 0..2 {
                let before = fx.out.prims.len();
                quads4::sprite_quads(fx, D, t, 0, t + 0x10);
                a3 = last_xyz2(fx, before);
            }
        }
        if f(fx) == SPARKLE {
            tex(fx, TEX + 8);
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
            a3 = 0;
        }
        if f(fx) == FAN {
            fx.mem.set16(D + 0xB8, 0x24);
            let before = fx.out.prims.len();
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            a3 = last_xyz2(fx, before);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == MOTE {
            tex(fx, TEX + 4);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xC0, 0x14, sub);
            fx.mem.set16(D + 0xBE, 0);
            billboard::quad(fx, uv::uv_rect, D, t, 0x10, t + 0x10);
            a3 = 0;
        }
        if f(fx) == RIBBONS {
            let before = fx.out.prims.len();
            match fx.mem.i16(t + 0x50) {
                0 => {
                    for j in 0..3u32 {
                        let saves = [u64::from(p), u64::from(k), u64::from(flashes), u64::from(j), u64::from(D), u64::from(D + 0x140 * j), u64::from(t), u64::from(live), 0, 0x01E0_4700];
                        ribbon(fx, t, HIST + 0x140 * j, HIST + 0x140 * (j + 4), saves);
                    }
                }
                1 => {
                    for j in 10..13u32 {
                        let saves = [u64::from(p), u64::from(k), u64::from(flashes), u64::from(D + 0x140 * j), u64::from(D), u64::from(t), u64::from(j), u64::from(live), 0, 0x01E0_4780];
                        ribbon(fx, t, HIST + 0x140 * j, HIST + 0x140 * (j + 4), saves);
                    }
                }
                _ => {}
            }
            // The ribbon leaves a3 = 0 (its projections' loop) [run].
            let _ = before;
            a3 = 0;
        }
    }
    if flashes != 0 {
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x78 => 1,
        0xBE => 4,
        0xA5 => 2,
        0x83..=0x9F => 6,
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

/// 0x1E048C0(D, step, 0): b522's spawn (b287's); its frame's saves the
/// emitter's s0 = D / s1, the entry's s2 = 0 (no flashes yet), the
/// runner's s3 (0 [H]), the entry's s4 = D and `ra`.
fn spawn(fx: &mut Fx, step: u32, s1: u64, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, s1), (0x20, 0), (0x30, 0), (0x40, u64::from(D)), (0x50, u64::from(ra))]);
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

/// 0x1E01CF0(D, 0): the update counter and the spawns (frame 0x30: ra, the
/// entry's s1 (the runner's, 0 [H]) / s0 = the parameter; then s0 = D, s1
/// a loop count).
fn emit(fx: &mut Fx, param: i32) -> u32 {
    let sp = fx.enter(0x30);
    // a3 as the last call leaves it: 1 after 0x1F6830 [run], the colour's
    // blue (0) after a spawn.
    let a3 = std::cell::Cell::new(0u32);
    fx.saved(sp, &[(0x20, 0x01E0_4330), (0x10, 0), (0, param as i64 as u64)]);
    let mut s1 = 0u64;
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx, k: u32, out: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, out, u64::from(D), Some(s1), ra);
        a3.set(1);
    };
    let spawn = |fx: &mut Fx, step: u32, s1: u64, ra: u32| {
        spawn(fx, step, s1, ra);
        a3.set(u32::from(fx.mem.read_u8(COLOUR + 2)));
    };
    if (0x97..0x160).contains(&c(fx)) {
        node(fx, 0, AIM, s1, 0x01E0_1D64);
        let y = ee::add(fx.mem.f(AIM + 4), f32::from_bits(0x4053_3333));
        fx.mem.setf(AIM + 4, y);
        copy4(fx, POS, AIM);
    }
    for (lo, hi, k, ra_node, ras) in [
        (0x7B, 0x87, 3u32, 0x01E0_1DF8u32, [0x01E0_1E0Cu32, 0x01E0_1E20, 0x01E0_1E34, 0x01E0_1E48, 0x01E0_1E5C]),
        (0x91, 0x9C, 5, 0x01E0_1EA0, [0x01E0_1EB4, 0x01E0_1EC8, 0x01E0_1EDC, 0x01E0_1EF0, 0x01E0_1F04]),
    ] {
        if (lo..hi).contains(&c(fx)) {
            node(fx, k, POS, s1, ra_node);
            spawn(fx, RING, s1, ras[0]);
            for ra in &ras[1..] {
                spawn(fx, SPARKLE, s1, *ra);
            }
        }
    }
    if c(fx) == 0x7B {
        fx.mem.set16(KIND, 0);
        spawn(fx, RIBBONS, s1, 0x01E0_1F40);
    }
    if c(fx) == 0x8F {
        fx.mem.set16(KIND, 1);
        spawn(fx, RIBBONS, s1, 0x01E0_1F7C);
    }
    if c(fx) == 0x7C {
        spawn(fx, TINT, s1, 0x01E0_1FA8);
    }
    if c(fx) == 0x92 {
        spawn(fx, TINT, s1, 0x01E0_1FD4);
    }
    for (at, k, ra_node, ra_fan, cols) in [
        (0x80, 3u32, 0x01E0_2008u32, 0x01E0_201Cu32, [(0x2D, 0, 0x01E0_204Cu32), (0x87, 1, 0x01E0_2080)]),
        (0x95, 5, 0x01E0_20B8, 0x01E0_20CC, [(0x87, 0, 0x01E0_20FC), (0x2D, 1, 0x01E0_2130)]),
    ] {
        if c(fx) == at {
            node(fx, k, POS, s1, ra_node);
            spawn(fx, FAN, s1, ra_fan);
            for (a, kind, ra) in cols {
                fx.mem.set16(ANGLE, a);
                fx.mem.set16(KIND, kind);
                spawn(fx, COLUMN, s1, ra);
            }
        }
    }
    for (set, k, ra_node, ra) in [(&[0x61, 0x76, 0x80][..], 0u32, 0x01E0_2180u32, 0x01E0_21B0u32), (&[0x6B, 0x81][..], 1, 0x01E0_221C, 0x01E0_224C)] {
        if set.contains(&c(fx)) {
            node(fx, k, POS, s1, ra_node);
            let y = ee::sub(fx.mem.f(POS + 4), 1.5);
            fx.mem.setf(POS + 4, y);
            s1 = 0;
            while s1 < 0xD {
                spawn(fx, MOTE, s1, ra);
                s1 += 1;
            }
        }
    }
    if c(fx) == 0x17 {
        spawn(fx, LIGHT, s1, 0x01E0_22A0);
    }
    if c(fx) == 0x17 {
        spawn(fx, FLASH, s1, 0x01E0_22C8);
    }
    fx.leave(0x30);
    a3.get()
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

/// 0x1E022E0 (D, task): b418's white ring: size 30, cell row (rand >> 4)
/// % 8, angles +0x5A / +0x5C rand % 360, +0xA 2 on odd rands else 0, +0xC
/// the kind, radius rand % 10 + 5 growing by (rand >> 4) % 10 + 5 from age
/// 1, +0x52 rand % 350 + 100, row 0..34, life 3..7; in by 0xC0 under 0xC0,
/// out by 15; grey 0x80 on odd ages, 0x60 on even.
fn ring(fx: &mut Fx, t: u32) -> u32 {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41F0_0000);
        let v = fx.rand_from(t, 0x01E0_2308);
        fx.mem.set16(t + 0x56, (v >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = rmod(fx, t, 0x01E0_2340, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_235C, 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = fx.rand_from(t, 0x01E0_2378);
        fx.mem.set16(t + 0xA, if v % 2 != 0 { 2 } else { 0 });
        let kind = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, kind);
        let v = rmod(fx, t, 0x01E0_23B0, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, 0x01E0_23D0, 0x15E);
        fx.mem.set16(t + 0x52, v + 0x64);
        let v = fx.rand_from(t, 0x01E0_23F0);
        let r = fx.div(v >> 4, 0xA).1;
        fx.mem.set16(t + 0xE, r + 5);
        let v = rmod(fx, t, 0x01E0_2414, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2430, 5);
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
    g as u32
}

/// 0x1E028B0 (D, task): b589's column in lilac (0xF0, 0xE0, 0xFF) both
/// sides: size 6, by kind +0x50 (18, 1000) / (18, 250) / (18, 400) at
/// +0x30 / +0x34; in by 0x40 under 0x80 for 4 updates, out by 9; from age
/// 1 +0x34 grows by 40 (kind 0) / 20 (kind 1).
fn column(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xF0, 0xE0, 0xFF);
        l2::rgb(fx, t + 0x64, 0xF0, 0xE0, 0xFF);
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

/// 0x1E02A40 (D, task; s1 = D): b544's sparkle in violet (0x80, 0x40,
/// 0xFF): size 150 x 75, cell row (rand >> 4) % 3, a velocity (rand % 100
/// - 50) / 60 never applied, +0x5C rand % 360, spins +4 / +2 / +0 added to
/// the camera turn, its third word then 1.5 less (facing) / more and the
/// camera turn taken again; kind 0..3; in by 0x40 under 0x80 for 10
/// updates, out by 9.
fn sparkle(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x96);
        let w = fx.mem.i16(t + 8) >> 1;
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2A7C);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_2AB0u32), (0x30, 0x01E0_2ADC), (0x38, 0x01E0_2B08)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_2B60, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2B7C);
        for (o, ra) in [(4u32, 0x01E0_2B84u32), (2, 0x01E0_2BA4), (0, 0x01E0_2BC4)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
        let v = fx.rand_from(t, 0x01E0_2BE4);
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
    let (v, ra) = if fx.mem.i16(FACING) != 0 { (ee::sub(w, 1.5), 0x01E0_2CACu32) } else { (ee::add(w, 1.5), 0x01E0_2CD0) };
    fx.fix_residue(v, ra);
    fx.mem.set32(t + 0x28, ee::fixsfsi(v));
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2CDC);
    age(fx, t);
}

/// 0x1E02D00 (D, task): b641's arc fan in lilac (0xF0, 0xE0, 0xFF) both
/// sides: outer radius 140, 10-degree arcs every 10 degrees; its outer
/// alpha up by 0x60 below 0xC0 for 10 updates, then down by 9; faces the
/// camera turned by 0x4000.
fn fan(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x430C_0000);
        l2::rgb(fx, t + 0x64, 0xF0, 0xE0, 0xFF);
        l2::rgb(fx, t + 0x60, 0xF0, 0xE0, 0xFF);
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2DC0);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02DF0 (D, task; s1 = D): b646's white mote: size 130, cell 2..4, a
/// random angle +0x5A ((rand >> 4) % 360), thrown 8.6 out along it, then
/// drifting 0.8 a update; in by 0x40 under 0x80 for 10 updates, out by 9;
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
        fx.mem.set16(t + 8, 0x82);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2E28);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        let v = fx.rand_from(t, 0x01E0_2E60);
        let r = fx.div(v >> 4, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        drift(fx, f32::from_bits(0x4109_999A));
    }
    if fx.mem.i16(t + 0x54) < 0xA {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        drift(fx, f32::from_bits(0x3F4C_CCCD));
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2F98);
    age(fx, t);
}

/// 0x1E02FC0 (D, task; frame 0x50 at `sp`, the entry's pointer `p`): a
/// violet (0xC0, 0x40, 0xFF) ribbon task, size 5, +0xE 30; kind 0 fills
/// the histories 0..3 with node 0x17 stacked 1.4 down a history and 4..7
/// with node 0x18 (+6 = 14, held 11 updates), kind 1 the histories 10..13 /
/// 14..17 with nodes 0x33 / 0x34 (+6 = 19, held 20); alpha 0xFF while
/// held, then down by 9; while held the points are pushed
/// (`spawn::history`).
fn ribbons(fx: &mut Fx, t: u32, sp: u32, p: u32) -> u32 {
    l2::rgb(fx, t + 0x60, 0xC0, 0x40, 0xFF);
    let mut a3 = 0xFFu32;
    let mut s0 = u64::from(p);
    let down = f32::from_bits(0x3FB3_3333);
    // (kind, (node, first history, fill ra, push ra) x 2, +6, +0x52).
    let sets: [(i32, [(u32, u32, u32, u32); 2], i32, i32); 2] = [
        (0, [(2, 0, 0x01E0_3038, 0x01E0_376C), (3, 4, 0x01E0_31DC, 0x01E0_3820)], 0xE, 0xB),
        (1, [(4, 10, 0x01E0_33A8, 0x01E0_38F4), (5, 14, 0x01E0_3548, 0x01E0_39A8)], 0x13, 0x14),
    ];
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        for (kind, nodes, six, hold) in sets {
            if fx.mem.i16(t + 0x50) != kind {
                continue;
            }
            fx.mem.set16(t + 0xE, 0x1E);
            for (n, first, ra, _) in nodes {
                let h = fx.self_handle();
                let slot = fx.mem.i16(NODES + 2 * n);
                node_position_saving(fx, h, slot, sp + 0x40, s0, Some(u64::from(t)), ra);
                for j in 0..4u32 {
                    if j != 0 {
                        let y = ee::sub(fx.mem.f(sp + 0x44), down);
                        fx.mem.setf(sp + 0x44, y);
                    }
                    s0 = u64::from(sp + 0x44);
                    let h0 = HIST + 0x140 * (first + j);
                    for e in 0..0x13u32 {
                        l2::copy3(fx, h0 + 0x10 * e, sp + 0x40);
                    }
                }
            }
            fx.mem.set16(t + 6, six);
            fx.mem.set16(t + 0x52, hold);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 0x52) {
        fx.mem.write_u8(t + 0x63, 0xFF);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    for (kind, nodes, _, _) in sets {
        if fx.mem.i16(t + 0x50) != kind || fx.mem.i16(t + 0x52) < fx.mem.i16(t + 0x54) {
            continue;
        }
        for (n, first, _, ra) in nodes {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 * n);
            node_position_saving(fx, h, slot, sp + 0x40, s0, Some(u64::from(t)), ra);
            for j in 0..4u32 {
                if j != 0 {
                    let y = ee::sub(fx.mem.f(sp + 0x44), down);
                    fx.mem.setf(sp + 0x44, y);
                }
                s0 = u64::from(sp + 0x44);
                hist::history(fx, HIST + 0x140 * (first + j) + 0x130, 0x14, sp + 0x40);
                a3 = sp + 0x40;
            }
        }
    }
    age(fx, t);
    a3
}

/// 0x1E03A60 (D, task): b287's stage light down by 10 to 0x30 for 30
/// updates, back up by 10 to 0x80 once the timer is under 20; 0x80 and the
/// task's end when the timer is at 3.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x1E {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x14 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
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

/// 0x1E03B50 (D, task; frame 0x80 at `sp`): b144's frame tint in lilac
/// (0x80, 0x70, 0x90): b020's window sprite sampling the frame (0x207EF0,
/// filter 1) at ST inset by +0x48 / 512, ALPHA 0x44, key 0; before age 10
/// in by 0x13 below 0x60 and the inset up by 0.3 below 100, then out by 6
/// (ending under 8) and the inset down by 0.3 to 0.
fn tint(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x28;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u8(d + 0x10, 0x80);
    fx.mem.write_u8(d + 0x11, 0x70);
    fx.mem.write_u8(d + 0x12, 0x90);
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(d + 0x13, a);
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
    if fx.mem.i16(t + 0x54) < 0xA {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x60 {
            fx.mem.write_u8(t + 0x63, a + 0x13);
        }
        let w = fx.mem.f(t + 0x48);
        if w < 100.0 {
            fx.mem.setf(t + 0x48, ee::add(w, f32::from_bits(0x3E99_999A)));
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 8 {
            fx.mem.write_u8(t + 0x63, a - 6);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let w = fx.mem.f(t + 0x48);
        if w <= 0.0 {
            fx.mem.write_u32(t + 0x48, 0);
        } else {
            fx.mem.setf(t + 0x48, ee::sub(w, f32::from_bits(0x3E99_999A)));
        }
    }
    age(fx, t);
}

/// 0x1E03DA0(D, t): b287's flash step in lilac (0xF0, 0xE0, 0xFF): alpha
/// up by 9 below 0x80 until update 0xB7, then down by 7 (the task ends
/// under 9). `regs` the unit flash's s0 / s1 (the slot index, the live
/// count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_50BC), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f21 / f20: the runner's (0) [H].
    fx.mem.write_u32(sp + 4, 0);
    fx.mem.write_u32(sp, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0xF0, 0xE0, 0xFF);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
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
    fx.mem.setf(d + 0x2C, ee::div(ee::sub(512.0, h), 512.0));
    fx.submit(d, 0, 0);
    if fx.mem.i16(COUNTER) < 0xB7 {
        fade_in(fx, t, 0x63, 0x80, 9);
    } else {
        fade_out(fx, t, 0x63, 9, 7);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E04FB0(D): b550's unit flash (b287's 0x1E03F90 but the step): in a
/// packet layer the attacker drawn into the target (0x1F8270 filter 0;
/// ALPHA 0x48), TEXFLUSH before and after; every flash task stepped; the
/// layer linked into the ordering table at key 0 when any task was alive.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xB0);
    let d = sp + 0x58;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    let mut alive = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        alive += 1;
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, [k, alive]);
        }
    }
    if alive != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}

/// 0x1E04A70 (D, task, a, b): b640's ribbon (b703's with the `b` side's
/// alpha down by 3 a strip while above 4, else 0); `saves` the draw loop's
/// s0..s7, fp and the return address.
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
