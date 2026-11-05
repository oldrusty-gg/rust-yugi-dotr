//! BMODULE b144 (card 66), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03720, data area 0x1ED0000) [F]: a task pool of 200 (0x70 bytes at +0x2A0, alive +0x58, step
//! +0x44 as a record address) drawn with the second library: rings
//! (`billboard::ring_quads`, 0x1E00920), arc strips
//! (`billboard::arc_strips`, 0x1E00ED0), spark fans (`quads4::sparks`,
//! 0x1E01230), quads (`billboard::quad`, 0x1E01630) and a four-quad
//! sprite (`quads4::sprite_quads`, 0x1E018F0), UVs by uv_rect 0x1E00080.
//! Up to update 90 rings and an arc ring ride the attacker's node 0x14
//! (two light planes from update 19); on update 43 a wide arc and six
//! rings start there; from 110 to 133 (odd updates) rings and arcs at the
//! target (y 33), from 118 to 143 spark fans, eight rings, forty flying
//! quads in a circle and more every update, a wide white arc and a
//! whole-window frame flash (0x1E034D0 = `b627::flash_from`). Returns 1
//! on update 110, 4 on 175, 2 on 155, 6 on 121..149, 8 once the
//! 190-update timer +0x5A70 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The step functions a task holds (+0x44), as the record's addresses.
const RING_A: u32 = 0x01E0_2030;
const ARC_A: u32 = 0x01E0_22A0;
const RING_W: u32 = 0x01E0_2430;
const FAN: u32 = 0x01E0_2630;
const FLY_A: u32 = 0x01E0_2750;
const FLY_B: u32 = 0x01E0_2930;
const ARC_W: u32 = 0x01E0_2B50;
const RING_N: u32 = 0x01E0_2C40;
const PLANE: u32 = 0x01E0_2E30;
const ARC_N: u32 = 0x01E0_2F30;
const RING_C: u32 = 0x01E0_3040;
const ARC_S: u32 = 0x01E0_3210;
const RING_S: u32 = 0x01E0_3300;
const FLASH: u32 = 0x01E0_34D0;
/// The return address of the entry's `jalr` into a step (0x1E03A50 + 8).
const TASK_RA: u64 = 0x01E0_3A58;
const TIMER: u32 = D + 0x5A70;
const COLOUR: u32 = D + 0x5A74;
const COUNTER: u32 = D + 0x5AD2;
const BYTES: u32 = D + 0x5AD4;
const BYTES2: u32 = D + 0x5BD4;
const BYTES3: u32 = D + 0x5CD4;
const SINES: u32 = D + 0x5DD4;

/// The entry 0x1E03720.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xF0);
    fx.mem.write_u32(D + 0x5A7C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xF0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x5F40);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x5AD0, param);
    fx.mem.set32(TIMER, 0xBE);
    for (at, node, clear) in [(0x5A9Cu32, 0xB, 1), (0x5A9E, 0x14, 0), (0x5AA0, 0xD, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x5AAA, r);
    for k in 0xAu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x260 + 4 * k, t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x5A50 + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xE0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xE4)), 180.0);
    fx.mem.set16(D + 0x5ACE, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand();
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(BYTES2 + k, h - 0x14);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(BYTES2 + k, h);
    }
    // The third table's loop stores into +0x5BD4 again (+0x5CD4 stays
    // clear) [F 0x1E039FC].
    for k in 0..0x100u32 {
        let v = fx.rand();
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES2 + k, h - 0xF);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
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
        let t = slot + 0x2A0;
        match f {
            RING_A => ring_a(fx, t),
            ARC_A => arc_a(fx, t),
            RING_W => ring_w(fx, t),
            FAN => fan(fx, t),
            FLY_A => fly(fx, t, false),
            FLY_B => fly(fx, t, true),
            ARC_W => arc_const(fx, t, 0x43FA_0000, 0x11, 0x01E0_2C10),
            RING_N => ring_n(fx, t),
            PLANE => plane(fx, t),
            ARC_N => arc_n(fx, t),
            RING_C => ring_c(fx, t),
            ARC_S => arc_const(fx, t, 0x42A0_0000, 0xA, 0x01E0_32D0),
            RING_S => ring_s(fx, t),
            FLASH => crate::b627::flash_from(fx, t, TASK_RA),
            _ => panic!("b144: unknown task step {f:#x}"),
        }
    }
    let mut alive = 0u64;
    for k in 0..TASKS {
        let s = D + 0x70 * k;
        if fx.mem.i16(s + 0x2F8) == 0 {
            continue;
        }
        alive += 1;
        let t = s + 0x2A0;
        let is = |fx: &mut Fx, f: u32| fx.mem.read_u32(t + 0x44) == f;
        // 0x1F6830 from the draw loop leaves its frame (the entry's s0 =
        // D, s1 = the live tasks so far) below the entry's sp [run].
        let node = |fx: &mut Fx, slot: u32, out: u32, ra: u32| {
            let h = fx.self_handle();
            let n = fx.mem.i16(D + slot);
            crate::b076::node_position_saving(fx, h, n, out, u64::from(D), Some(alive), ra);
        };
        if is(fx, RING_A) {
            texture(fx, 0x29C);
            cell(fx, t, sp + 0x60, 0x9B);
            rings(fx, t, 7, 1, BYTES2);
        }
        if is(fx, ARC_A) {
            fx.mem.set16(D + 0xB8, 0x18);
            node(fx, 0x5A9C, sp + 0x50, 0x01E0_3B50);
            billboard::arc_strips(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if is(fx, RING_W) {
            texture(fx, 0x5A50);
            cell(fx, t, sp + 0x70, 0x14);
            rings(fx, t, 7, 0, BYTES);
        }
        if is(fx, FAN) {
            texture(fx, 0x5A54);
            fx.mem.set16(D + 0x94, 0);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x1F);
            fx.mem.set16(D + 0x9A, 0x7F);
            fx.mem.set16(D + 0xB8, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            fx.mem.set16(D + 0xB8, 0x18);
            fx.mem.set16(D + 0xBE, 0);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (f, local) in [(FLY_A, 0x80u32), (FLY_B, 0x90)] {
            if is(fx, f) {
                texture(fx, 0x5A58);
                cell(fx, t, sp + local, 0x14);
                fx.mem.set16(D + 0xBE, 5);
                for k in 0..3 {
                    let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                    fx.mem.write_u32(sp + 0x50 + 4 * k, w);
                }
                billboard::quad(fx, uv::uv_rect, D, t, 0x10, sp + 0x50);
            }
        }
        if is(fx, ARC_W) {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if is(fx, RING_N) {
            texture(fx, 0x29C);
            cell(fx, t, sp + 0xA0, 0x9B);
            fx.mem.set16(D + 0xB8, 7);
            fx.mem.set16(D + 0xB0, 1);
            node(fx, 0x5A9E, t + 0x10, 0x01E0_3E20);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if is(fx, PLANE) {
            node(fx, 0x5A9C, sp + 0x50, 0x01E0_3E6C);
            texture(fx, 0x29C);
            l2::rect(fx, sp + 0xB0, 0x9A, 0);
            for k in 0..4 {
                let w = fx.mem.read_u32(sp + 0xB0 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, w);
            }
            fx.mem.set16(D + 0xBE, 0);
            fx.mem.write_u32(sp + 0x5C, 0);
            quads4::sprite_quads(fx, D, t, 0, sp + 0x50);
        }
        if is(fx, ARC_N) {
            fx.mem.set16(D + 0xB8, 0x18);
            node(fx, 0x5A9E, t + 0x10, 0x01E0_3EF8);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if is(fx, RING_C) {
            texture(fx, 0x5A50);
            cell(fx, t, sp + 0xC0, 0x14);
            rings(fx, t, 4, 0, BYTES3);
        }
        if is(fx, ARC_S) {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if is(fx, RING_S) {
            texture(fx, 0x5A50);
            cell(fx, t, sp + 0xD0, 0x14);
            rings(fx, t, 8, 0, BYTES2);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x6E => 1,
        0xAF => 4,
        0x9B => 2,
        0x79..=0x95 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn texture(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(D + at);
    fx.mem.write_u32(D + 0x88, w);
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

/// `n` quads on a ring in layout `layout` pushed out by `table`.
fn rings(fx: &mut Fx, t: u32, n: i32, layout: i32, table: u32) {
    fx.mem.set16(D + 0xB8, n);
    fx.mem.set16(D + 0xB0, layout);
    billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, table);
    fx.mem.set16(D + 0xB8, 0);
    fx.mem.set16(D + 0xB0, 0);
}

/// The attacker's node in slot D + `slot` into `out`.
fn node(fx: &mut Fx, slot: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i16(D + slot);
    fx.node_position(h, s, out);
}

/// 0x1E01C60(D, 0): the update counter and the spawns (frame 0x40: the
/// entry's s2 = the parameter, s1 = 0, s0 = D). 0x1E04120's saves: the
/// entry's s4 / s3 = 0, the emitter's s2 / s1, s0 = D.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_3A28), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let (mut s2, mut s1) = (p, 0u64);
    let go = |fx: &mut Fx, f: u32, ra: u32, s2: u64, s1: u64| {
        spawn(fx, f, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, s2), (0x10, s1), (0, u64::from(D))]);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) < 0x5B {
        go(fx, RING_N, 0x01E0_1CA0, s2, s1);
        go(fx, ARC_N, 0x01E0_1CB4, s2, s1);
        node(fx, 0x5A9E, D + 0x250);
        go(fx, RING_C, 0x01E0_1CE0, s2, s1);
    }
    if fx.mem.i16(COUNTER) == 0x13 {
        fx.mem.set16(D + 0x5ABE, 0);
        go(fx, PLANE, 0x01E0_1D08, s2, s1);
        fx.mem.set16(D + 0x5ABE, 0x5A);
        go(fx, PLANE, 0x01E0_1D24, s2, s1);
    }
    let h = fx.target_handle();
    fx.unit_position(h, D + 0x250);
    fx.mem.write_u32(D + 0x254, 0x4204_0000);
    let c = fx.mem.i16(COUNTER);
    if (0x6E..0x86).contains(&c) && c % 2 == 1 {
        go(fx, RING_A, 0x01E0_1D90, s2, s1);
        go(fx, ARC_A, 0x01E0_1DA4, s2, s1);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x76..0x90).contains(&c) {
        if fx.div(c, 3).1 == 1 || c == 0x76 {
            go(fx, FAN, 0x01E0_1E00, s2, s1);
        }
        if fx.mem.i16(COUNTER) == 0x76 {
            for k in 0..8u64 {
                go(fx, RING_W, 0x01E0_1E2C, s2, k);
            }
            for k in 0..0x14u64 {
                let a = 0x12 * k;
                fx.mem.set16(D + 0x5ACA, a as i32);
                go(fx, FLY_A, 0x01E0_1E70, k, a);
                go(fx, FLY_B, 0x01E0_1E84, k, a);
            }
            s2 = 0x14;
            s1 = 0x168;
        }
        if fx.mem.i16(COUNTER) >= 0x76 {
            go(fx, RING_W, 0x01E0_1EDC, s2, s1);
            let v = fx.rand_from(D, 0x01E0_1EE4);
            let (_, h) = fx.div(v, 0x168);
            fx.mem.set16(D + 0x5ACA, h);
            go(fx, FLY_A, 0x01E0_1F08, s2, s1);
            go(fx, FLY_B, 0x01E0_1F1C, s2, s1);
        }
    }
    if fx.mem.i16(COUNTER) == 0x2B {
        node(fx, 0x5A9E, D + 0x250);
        go(fx, ARC_S, 0x01E0_1F58, s2, s1);
        for ra in [0x01E0_1F6C, 0x01E0_1F80, 0x01E0_1F94, 0x01E0_1FA8, 0x01E0_1FBC, 0x01E0_1FD0] {
            go(fx, RING_S, ra, s2, s1);
        }
    }
    if fx.mem.i16(COUNTER) == 0x76 {
        go(fx, ARC_W, 0x01E0_1FF4, s2, s1);
    }
    if fx.mem.i16(COUNTER) == 0x76 {
        go(fx, FLASH, 0x01E0_2018, s2, s1);
    }
    fx.leave(0x40);
}

/// 0x1E04120(D, step, 0): the first free task cleared and set off at
/// D+0x250 with the colour +0x5A74 twice, angle +0x5ABE (+0x5A), +0x5ACA
/// (+0x50). `saves`: its prologue's.
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
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
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(D + 0x5ABE);
            fx.mem.set16(t + 0x5A, a);
            let a = fx.mem.i16(D + 0x5ACA);
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

/// A step's prologue: frame 0x20 (ra, the entry's s0 = D) or 0x30 (ra,
/// the entry's s1 = 0, s0 = D).
fn prologue(fx: &mut Fx, bytes: u32) {
    let sp = fx.enter(bytes);
    if bytes == 0x20 {
        fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    } else {
        fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    }
}

/// rand from a step's `jal` at `site` (its s0 = the task).
fn r(fx: &mut Fx, t: u32, site: u32) -> i32 {
    fx.rand_from(t, site + 8)
}

fn rmod(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    let v = r(fx, t, site);
    fx.div(v, m).1
}

/// 0x205580(t + 0x20) from a step's `jal` at `site` (frame 0x30: the
/// step's s1, s0 = t).
fn angles(fx: &mut Fx, t: u32, s1: u32, site: u32) {
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, u64::from(site + 8)), (0x10, u64::from(s1)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
}

/// Alpha `a` (+0x63 / +0x67): + `up` below `cap` until age `until`, then
/// - `down` from `floor` up, else the task ends.
fn fade(fx: &mut Fx, t: u32, a: u32, until: i32, cap: u8, up: u8, floor: u8, down: u8) {
    let v = fx.mem.read_u8(t + a);
    if fx.mem.i16(t + 0x54) < until {
        if v < cap {
            fx.mem.write_u8(t + a, v + up);
        }
    } else if v >= floor {
        fx.mem.write_u8(t + a, v - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// Alpha +0x63 = `full` for the life +6, then out by 11.
fn hold(fx: &mut Fx, t: u32, full: u8) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, full);
    } else {
        let v = fx.mem.read_u8(t + 0x63);
        if v >= 0xD {
            fx.mem.write_u8(t + 0x63, v - 0xB);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
}

fn grow_radius(fx: &mut Fx, t: u32) {
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The random turn +0x20 / +0x24 / +0x28 (rand % 0xFFFF each).
fn turn3(fx: &mut Fx, t: u32, sites: [u32; 3]) {
    for (o, site) in [0x20u32, 0x24, 0x28].into_iter().zip(sites) {
        let v = rmod(fx, t, site, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

/// 0x1E02030: a pink ring (frame 0x20; +0x52 = 0 always [F]): quad size
/// 2..6, radius 50..149 growing by 5..24, a random turn about y.
fn ring_a(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x52) == 0 {
            let v = rmod(fx, t, 0x01E0_2058, 5);
            fx.mem.setf(t + 0x48, ee::from_i32(v + 2));
        }
        if fx.mem.i16(t + 0x52) == 1 {
            let v = rmod(fx, t, 0x01E0_2098, 5);
            fx.mem.setf(t + 0x48, ee::from_i32(v + 6));
        }
        let v = r(fx, t, 0x01E0_20C8);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xC0, 0x70, 0x70);
        let v = rmod(fx, t, 0x01E0_20FC, 0xFFFF);
        fx.mem.set32(t + 0x24, v);
        if fx.mem.i16(t + 0x52) == 0 {
            let v = rmod(fx, t, 0x01E0_2128, 0x64);
            fx.mem.set16(t + 0x50, v + 0x32);
            let v = rmod(fx, t, 0x01E0_2148, 0x14);
            fx.mem.set16(t + 0xE, v + 5);
        }
        if fx.mem.i16(t + 0x52) == 1 {
            let v = rmod(fx, t, 0x01E0_217C, 0x1E);
            fx.mem.set16(t + 0x50, v + 0xA);
            let v = rmod(fx, t, 0x01E0_219C, 3);
            fx.mem.set16(t + 0xE, v + 0xA);
        }
        let v = rmod(fx, t, 0x01E0_21C0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = r(fx, t, 0x01E0_21DC);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_21FC, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t, 0x60);
    grow_radius(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E022A0: a pink arc ring (frame 0x20): radii 10 (growing by 6 to
/// 150) / 60 (by 40), height 2 growing by +0xA (5..39), turning by 0x200.
fn arc_a(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.write_u32(t + 0x4C, 0x4270_0000);
        fx.mem.write_u32(t + 0x34, 0x4000_0000);
        let v = rmod(fx, t, 0x01E0_22D0, 0x23);
        fx.mem.set16(t + 0xA, v + 5);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 2);
        let v = rmod(fx, t, 0x01E0_2320, 0xFFFF);
        fx.mem.set32(t + 0x24, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x67, 7, 0x80, 0x2A, 0xD, 0xB);
    let a = fx.mem.read_u32(t + 0x24).wrapping_add(0x200);
    fx.mem.write_u32(t + 0x24, a);
    let r = fx.mem.f(t + 0x48);
    if r < 150.0 {
        fx.mem.setf(t + 0x48, ee::add(r, 6.0));
    }
    let r = ee::add(fx.mem.f(t + 0x4C), 40.0);
    fx.mem.setf(t + 0x4C, r);
    let h = ee::add(fx.mem.f(t + 0x34), ee::from_i32(fx.mem.i16(t + 0xA)));
    fx.mem.setf(t + 0x34, h);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02430: a white ring (frame 0x20): quad size 25..44 growing by 1,
/// radius 10..29 growing by 60..109, a random turn.
fn ring_w(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_244C, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x19));
        let v = r(fx, t, 0x01E0_2478);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        turn3(fx, t, [0x01E0_24AC, 0x01E0_24C8, 0x01E0_24E4]);
        let v = rmod(fx, t, 0x01E0_2500, 0x14);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_2520, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_253C, 0x32);
        fx.mem.set16(t + 0xE, v + 0x3C);
        let v = r(fx, t, 0x01E0_255C);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_257C, 5);
        fx.mem.set16(t + 6, v + 4);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t, 0x60);
    grow_radius(fx, t);
    let s = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, s);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02630: a white spark fan (frame 0x20): radii 200 / 142, the outer
/// growing by 60 a update, the inner following 250 behind; turning by
/// 0x100.
fn fan(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.setf(t + 0x4C, ee::sub(200.0, 58.0));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.write_u32(t + 0x34, 0x4416_0000);
        let s = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, s);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 8 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let v = fx.mem.read_u8(t + 0x63);
        if v >= 0x10 {
            fx.mem.write_u8(t + 0x63, v - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, 250.0));
    let a = fx.mem.read_u32(t + 0x24).wrapping_add(0x100);
    fx.mem.write_u32(t + 0x24, a);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02750 (`late` false: size 150, flying from its first update, in
/// for 10) / 0x1E02930 (`late`: size 100, rising by 4 for 6 updates, then
/// flying and rising by 1.5, in for 12), frame 0x30: a pink quad growing
/// by 15 a update, flying out along its angle +0x50 at 3.5, turning +0x5A
/// by 5 to 90; both alphas in by 0x40, then +0x63 out by 14.
fn fly(fx: &mut Fx, t: u32, late: bool) {
    prologue(fx, 0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        let size = if late { 0x64 } else { 0x96 };
        fx.mem.set16(t + 8, size);
        fx.mem.set16(t + 0xA, size);
        let v = r(fx, t, if late { 0x01E0_295C } else { 0x01E0_277C });
        fx.mem.set16(t + 0x56, v % 8);
        let a = fx.mem.i16(t + 0x50);
        let c = l2::sin_deg(fx, SINES, a, 1);
        fx.mem.setf(t + 0x30, ee::mul(3.5, c));
        let a = fx.mem.i16(t + 0x50);
        let s = l2::sin_deg(fx, SINES, a, 0);
        fx.mem.setf(t + 0x38, ee::mul(3.5, s));
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x5C, 0);
    }
    if fx.mem.i16(t + 0x54) < if late { 0xC } else { 0xA } {
        for a in [0x63u32, 0x67] {
            let v = fx.mem.read_u8(t + a);
            if v < 0xC0 {
                fx.mem.write_u8(t + a, v + 0x40);
            }
        }
    } else {
        for end in [true, false] {
            let v = fx.mem.read_u8(t + 0x63);
            if v >= 0x10 {
                fx.mem.write_u8(t + 0x63, v - 0xE);
            } else if end {
                fx.mem.set16(t + 0x58, 0);
            }
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let w = fx.mem.i16(t + o) + 0xF;
            fx.mem.set16(t + o, w);
        }
    }
    let age_now = fx.mem.i16(t + 0x54);
    let flying = if late { age_now >= 6 } else { age_now != 0 };
    if flying {
        let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
        fx.mem.setf(t + 0x10, x);
        let z = ee::add(fx.mem.f(t + 0x18), fx.mem.f(t + 0x38));
        fx.mem.setf(t + 0x18, z);
        if late {
            let y = ee::add(fx.mem.f(t + 0x14), 1.5);
            fx.mem.setf(t + 0x14, y);
        }
        let a = fx.mem.i16(t + 0x5A);
        if a < 0x5A {
            fx.mem.set16(t + 0x5A, a + 5);
        }
    }
    if late && fx.mem.i16(t + 0x54) < 6 {
        let y = ee::add(fx.mem.f(t + 0x14), 4.0);
        fx.mem.setf(t + 0x14, y);
    }
    angles(fx, t, D, if late { 0x01E0_2B20 } else { 0x01E0_28FC });
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02B50 (outer radius 500, in for 17) / 0x1E03210 (80, in for 10),
/// frame 0x20: a white-pink arc ring, its outer alpha in by 42 below 0x80,
/// then out by 7; the turn +0x20 held at 0x4000. `angles_at`: the
/// 0x205580 `jal`.
fn arc_const(fx: &mut Fx, t: u32, outer: u32, until: i32, angles_at: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.write_u32(t + 0x4C, outer);
        fx.mem.write_u32(t + 0x34, 0x4000_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0xA0, 0xA0);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x67, until, 0x80, 0x2A, 9, 7);
    angles(fx, t, 0, angles_at);
    fx.mem.write_u32(t + 0x20, 0x4000);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02C40: a pink ring at node 0x14 (frame 0x20): quad size 1..4,
/// radius 2..21 growing by 2..9, a random turn.
fn ring_n(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = r(fx, t, 0x01E0_2C5C);
        fx.mem.setf(t + 0x48, ee::from_i32(v % 4 + 1));
        let v = r(fx, t, 0x01E0_2C88);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xC0, 0x70, 0x70);
        turn3(fx, t, [0x01E0_2CBC, 0x01E0_2CD8, 0x01E0_2CF4]);
        let v = rmod(fx, t, 0x01E0_2D10, 0x14);
        fx.mem.set16(t + 0x50, v + 2);
        let v = r(fx, t, 0x01E0_2D30);
        fx.mem.set16(t + 0xE, v % 8 + 2);
        let v = rmod(fx, t, 0x01E0_2D50, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = r(fx, t, 0x01E0_2D6C);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_2D8C, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t, 0x80);
    grow_radius(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02E30: a light plane (frame 0x20) of 1.5 x 5 growing by 4 in
/// height, in by 0x40 for 4 updates, then out by 9.
fn plane(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0xC0);
        fx.mem.write_u32(t + 0x30, 0x3FC0_0000);
        fx.mem.write_u32(t + 0x34, 0x40A0_0000);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x63, 4, 0x80, 0x40, 0xB, 9);
    if fx.mem.i16(t + 0x54) != 0 {
        let h = ee::add(fx.mem.f(t + 0x34), 4.0);
        fx.mem.setf(t + 0x34, h);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02F30: a dark arc ring at node 0x14 (frame 0x20): outer radius
/// 10..59, in by 0x40 for 2 updates, then out by 9.
fn arc_n(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        let v = rmod(fx, t, 0x01E0_2F4C, 0x32);
        fx.mem.setf(t + 0x4C, ee::from_i32(v + 0xA));
        fx.mem.write_u32(t + 0x34, 0x4000_0000);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x40);
        l2::rgb(fx, t + 0x60, 0x40, 0x20, 0x20);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade(fx, t, 0x67, 2, 0x80, 0x40, 0xB, 9);
    angles(fx, t, 0, 0x01E0_300C);
    fx.mem.write_u32(t + 0x20, 0x4000);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03040: a white ring of four (frame 0x20): quad size 4..11, radius
/// 50..149 growing by 5..14.
fn ring_c(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = r(fx, t, 0x01E0_305C);
        fx.mem.setf(t + 0x48, ee::from_i32(v % 8 + 4));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        turn3(fx, t, [0x01E0_30A4, 0x01E0_30C0, 0x01E0_30DC]);
        let v = rmod(fx, t, 0x01E0_30F8, 0x64);
        fx.mem.set16(t + 0x50, v + 0x32);
        let v = rmod(fx, t, 0x01E0_3118, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_3134, 0xA);
        fx.mem.set16(t + 0xE, v + 5);
        let v = r(fx, t, 0x01E0_3154);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_3174, 5);
        fx.mem.set16(t + 6, v + 4);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t, 0x60);
    grow_radius(fx, t);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03300: a white ring of eight (frame 0x20): quad size 2..5, radius
/// 5..34 growing by 10..24.
fn ring_s(fx: &mut Fx, t: u32) {
    prologue(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = r(fx, t, 0x01E0_331C);
        fx.mem.setf(t + 0x48, ee::from_i32(v % 4 + 2));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        turn3(fx, t, [0x01E0_3364, 0x01E0_3380, 0x01E0_339C]);
        let v = rmod(fx, t, 0x01E0_33B8, 0x1E);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, 0x01E0_33D8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_33F4, 0xF);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = r(fx, t, 0x01E0_3414);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_3434, 5);
        fx.mem.set16(t + 6, v + 4);
        fx.mem.set16(t + 0x58, 2);
    }
    hold(fx, t, 0x60);
    grow_radius(fx, t);
    age(fx, t);
    fx.leave(0x20);
}
