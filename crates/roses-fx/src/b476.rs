//! BMODULE b476 (card 412), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03810, data area 0x1ED0000, 0x33B0 bytes) [F]
//!: the disc family's pool cut to 150 tasks of
//! 0x40 at D+0x2F0 (spawned by 0x1E041C0 = b148's spawn on 150 tasks with
//! the colour D+0x2DF4 / kind D+0x2E36), timer D+0x2DF0 = 0x149, nodes
//! D+0x2E1C.. (0x1F / 0x25 / 0x19 / 0x1A / 0x2B / 0x2C / 0x35), counter
//! D+0x2E3A, tables D+0x2E3C (rand % 30 - 15) / D+0x2F3C (rand % 10 - 5),
//! group-5 textures D+0x2DD0... On updates 0xBE / 0xD2 the histories of
//! nodes 0x19 / 0x1A (D+0x28C0 / 0x2960) and 0x2B / 0x2C (D+0x2A00 /
//! 0x2AA0) filled, then kept (10 points) every update; on 0xBE / 0xD2 a
//! red ribbon task (0x1E02FD0, stepped inline: alpha 0x80 to update 0xE6,
//! then out by 9) drawn by b501's ribbon between one pair. To 0x40 the
//! spawn point at node 0x25 (kept from update 0x33); 0x39..0xA5 a grey
//! sprite (0x1E02DF0, `billboard::sprite_square`) at each of nodes 0x1F /
//! 0x25; on 0xC6 / 0xDA at node 0x19 / 0x2B a disc (`spawn::disc_step`),
//! five grey camera-facing quad rings (0x1E02A80, drawn by 0x1E012B0)
//! and five quad rings (0x1E03370, `quads4::ring_spun`) and a camera task
//! (`task40::camera_at`); 0xC1..0xC7 / 0xD5..0xDB at node 0x1A / 0x2C five
//! tick rings (0x1E03090, `quads4::ring_ticks` on the side of the kind:
//! the facing) and five half rings (0x1E03220, drawn by 0x1E01BB0 on the
//! side of +0x3A). Returns 1 on update 0xB4, 4 on 0xE6, 2 on 0xDC, 6 on
//! 0xBF..0xD6, 8 once the timer has run out.
//!
//! The steps 0x1E02BE0 (b417's spark), 0x1E03650 and the object op
//! 0x1E03710 (inline 0x1F8380) are tested by the entry but never spawned
//! [F: the emitter's spawns name 0x1E02FD0 / 0x1E02DF0 / 0x1E02EF0 /
//! 0x1E02A80 / 0x1E03370 / 0x1E034D0 / 0x1E03090 / 0x1E03220 only]; not
//! ported. No 0x1F8100.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b148::{age, facing_of, fade, rmod, spawn_task_on};
use crate::blib::{billboard, quads4, spawn, task40, uv};
use crate::bus::Bus;
use crate::c061::{blend_alpha, desc_uv4, rgbaq_at, scale_tenth, to_degrees_neg};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
const POOL: u32 = D + 0x2F0;
const POS: u32 = D + 0x250;
const TIMER: u32 = D + 0x2DF0;
const COLOUR: u32 = D + 0x2DF4;
const NODES: u32 = D + 0x2E1C;
const KIND: u32 = D + 0x2E36;
const COUNTER: u32 = D + 0x2E3A;
const TABLE: u32 = D + 0x2E3C;
const TABLE_B: u32 = D + 0x2F3C;
const SPR: u32 = 0x7000_0000;

const RING_A: u32 = 0x01E0_2A80;
const SPRITE: u32 = 0x01E0_2DF0;
const DISC: u32 = 0x01E0_2EF0;
const RIBBON: u32 = 0x01E0_2FD0;
const TICK: u32 = 0x01E0_3090;
const HALF: u32 = 0x01E0_3220;
const RING_B: u32 = 0x01E0_3370;
const CAMERA: u32 = 0x01E0_34D0;

/// The entry 0x1E03810 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x190);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x2DFC, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x190);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x33B0);
    l2::sine_table(fx, D + 0x323C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x2E38, param);
    fx.mem.set32(TIMER, 0x149);
    for (k, n) in [0x1F, 0x25, 0x19, 0x1A, 0x2B, 0x2C, 0x35].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E03760(0, D): the textures (frame 0x40: ra, the entry's s2 =
    // param, s1 = 0, s0 = D).
    let f = fx.enter(0x40);
    fx.saved(f, &[(0x30, 0x01E0_3924), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2B0 + 4 * k as u32, t);
    }
    for k in 0xD..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2E4 + 4 * (k as u32 - 0xD), t);
    }
    for k in 0..4 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x2DD0 + 4 * k as u32, t);
    }
    fx.leave(0x40);
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_392C, 0x1E);
        fx.mem.set8(TABLE + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3960, 0xA);
        fx.mem.set8(TABLE_B + k, v - 5);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let dd = u64::from(D);
    emit(fx, param);
    // The histories' first points (the entry's s0 = D, s1 = 0).
    let fill = [(0xBE, 2u32, 0xB0u32, 0x01E0_39C4u32, 0x28C0u32), (0xBE, 3, 0xC0, 0x01E0_3A18, 0x2960), (0xD2, 4, 0xD0, 0x01E0_3A70, 0x2A00), (0xD2, 5, 0xE0, 0x01E0_3AC8, 0x2AA0)];
    for (when, k, local, ra, first) in fill {
        if fx.mem.i16(COUNTER) == when {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 * k);
            node_position_saving(fx, h, slot, sp + local, dd, Some(0), ra);
            for i in 0..9u32 {
                for c in 0..3 {
                    let w = fx.mem.read_u32(sp + local + 4 * c);
                    fx.mem.write_u32(D + first + 0x10 * i + 4 * c, w);
                }
            }
        }
    }
    // The entry's s2: param, or past the last history filled.
    let s2 = match fx.mem.i16(COUNTER) {
        0xBE => u64::from(D + 0x29F0),
        0xD2 => u64::from(D + 0x2B30),
        _ => param as i64 as u64,
    };
    for (k, p, ra) in [(2u32, 0x2950u32, 0x01E0_3B08u32), (3, 0x29F0, 0x01E0_3B18), (4, 0x2A90, 0x01E0_3B28), (5, 0x2B30, 0x01E0_3B38)] {
        let n = fx.mem.i16(NODES + 2 * k);
        spawn::node_history(fx, D + p, 0xA, n);
        // Its frame: the entry's s0..s4 (s3 / s4 the runner's, 0 [H]) and
        // ra [run: s4 at +0x40 is a later missing texture's TEX1].
        fx.saved(fx.sp - 0x70, &[(0, dd), (0x10, 0), (0x20, s2), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
    }
    let low = fx.sp;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            RING_A => {
                ring_by(fx, RING_A, (8, 40, 10, 40, 9, 2), t);
                fx.saved(low - 0x20, &[(0x10, 0x01E0_3B6C), (0, dd)]);
            }
            SPRITE => {
                sprite(fx, t);
                fx.saved(low - 0x20, &[(0x10, 0x01E0_3BB0), (0, dd)]);
            }
            DISC => {
                spawn::disc_step(fx, DISC, t);
                fx.saved(low - 0x20, &[(0x10, 0x01E0_3BD0), (0, dd)]);
            }
            RIBBON => {
                if fx.mem.i16(t + 0x36) == 1 {
                    l2::rgb(fx, t + 0x28, 0xFF, 0x20, 0x20);
                    fx.mem.set16(t + 6, 3);
                    fx.mem.write_u8(t + 0x2B, 0x80);
                    fx.mem.set16(t + 0x38, 6);
                    fx.mem.set16(t + 0x36, 2);
                }
                if fx.mem.i16(COUNTER) < 0xE6 {
                    fx.mem.write_u8(t + 0x2B, 0x80);
                } else {
                    let a = fx.mem.read_u8(t + 0x2B);
                    if a >= 0xB {
                        fx.mem.write_u8(t + 0x2B, a - 9);
                    } else {
                        fx.mem.set16(t + 0x36, 0);
                    }
                }
                age(fx, t);
            }
            TICK => {
                tick(fx, t);
                fx.saved(low - 0x30, &[(0x20, 0x01E0_3C98), (0x10, 0), (0, dd)]);
            }
            HALF => {
                half(fx, t);
                fx.saved(low - 0x20, &[(0x10, 0x01E0_3CB8), (0, dd)]);
            }
            RING_B => {
                ring_by(fx, RING_B, (4, 36, 0x28, 40, 10, 5), t);
                fx.saved(low - 0x20, &[(0x10, 0x01E0_3CD8), (0, dd)]);
            }
            CAMERA => {
                task40::camera_at(fx, CAMERA, D, t, 0x2870, 0x2DF0);
                fx.saved(low - 0x30, &[(0x20, 0x01E0_3CF8), (0x10, 0), (0, dd)]);
            }
            f => {
                debug_assert!(false, "b476: unported task step {f:#x}");
            }
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, dd, 0x01E0_3D8C);
    // The entry's s1: the live tasks seen so far (counted in a delay slot).
    let mut s1 = 0u64;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        s1 += 1;
        let set_rect = |fx: &mut Fx, tex: u32, local: u32, id: i32| {
            let w = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x34);
            l2::rect(fx, local, id, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(local + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
        };
        let point_at = |fx: &mut Fx, to: u32| {
            for k in 0..3 {
                let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                fx.mem.setf(to + 4 * k, v);
            }
        };
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        if f(fx) == RING_A {
            set_rect(fx, D + 0x2DD8, sp + 0x60, 0x14);
            fx.mem.set16(D + 0xB8, 0xA);
            point_at(fx, D + 0x20);
            ring_faced(fx, t, 0, TABLE, true);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == SPRITE {
            set_rect(fx, D + 0x2DD0, sp + 0x70, 0x14);
            point_at(fx, sp + 0x50);
            billboard::sprite_square(fx, 0x01E0_0C80, uv::uv_rect, D, t, 0, sp + 0x50);
        }
        if f(fx) == DISC {
            quads4::fan18(fx, D, t, sp + 0xF8);
        }
        if f(fx) == RIBBON {
            let (a, b, ra) = if fx.mem.i16(t + 0x30) == 0 { (0x28C0, 0x2960, 0x01E0_3F54) } else { (0x2A00, 0x2AA0, 0x01E0_3F70) };
            crate::b501::ribbon(fx, t, D + a, D + b, [dd, s1, u64::from(D + 0x40 * k), u64::from(k), 0, ra]);
        }
        if f(fx) == TICK {
            set_rect(fx, D + 0x2DDC, sp + 0x80, 0x14);
            fx.mem.set16(D + 0xB8, 1);
            let side = fx.mem.i16(t + 0x30);
            quads4::ring_ticks(fx, D, t, side);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == HALF {
            set_rect(fx, D + 0x2DDC, sp + 0x90, 0x14);
            fx.mem.set16(D + 0xB8, 3);
            let side = fx.mem.i16(t + 0x3A);
            ring_faced(fx, t, side, TABLE, false);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == RING_B {
            set_rect(fx, D + 0x2EC, sp + 0xA0, 0x9B);
            fx.mem.set16(D + 0xB8, 0xA);
            point_at(fx, D + 0x20);
            quads4::ring_spun(fx, uv::uv_rect, D, t, 0, TABLE_B);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xB4 => 1,
        0xE6 => 4,
        0xDC => 2,
        0xBF..=0xD6 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E02490(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0x70: ra, the entry's s2 = param, s1 = 0, s0 =
/// D). Its s1 is D, s0 / s2 sp + 0x44 / + 0x48 after the sprites' or the
/// points' first node, s0 the loops' counts; the spawns save them with the
/// entry's s3 / s4 (the runner's, 0 [H]).
fn emit(fx: &mut Fx, param: i32) {
    let p = param as i64 as u64;
    let dd = u64::from(D);
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x30, 0x01E0_399C), (0x20, p), (0x10, 0), (0, dd)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx, k: u32, out: u32, s0: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, out, s0, Some(dd), ra);
    };
    let pos = |fx: &mut Fx, from: u32| {
        for k in 0..3 {
            let w = fx.mem.read_u32(from + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    let spawn = |fx: &mut Fx, step: u32, s0: u64, s2: u64, ra: u32| spawn_task_on(fx, TASKS, COLOUR, KIND, step, [s0, dd, s2, 0, 0], ra);
    let (q0, q2) = (u64::from(sp + 0x44), u64::from(sp + 0x48));
    let (mut s0, mut s2) = (dd, p);
    if c(fx) == 0x33 {
        node(fx, 1, D + 0x28B0, s0, 0x01E0_24F0);
    }
    if (0x34..0x41).contains(&c(fx)) {
        node(fx, 1, sp + 0x40, s0, 0x01E0_2524);
        pos(fx, D + 0x28B0);
    }
    if c(fx) == 0xBE {
        fx.mem.set16(KIND, 0);
        spawn(fx, RIBBON, s0, s2, 0x01E0_2578);
    }
    if c(fx) == 0xD2 {
        fx.mem.set16(KIND, 1);
        spawn(fx, RIBBON, s0, s2, 0x01E0_25A4);
    }
    if (0x39..0xA6).contains(&c(fx)) {
        node(fx, 0, sp + 0x40, s0, 0x01E0_25D8);
        (s0, s2) = (q0, q2);
        pos(fx, sp + 0x40);
        spawn(fx, SPRITE, s0, s2, 0x01E0_260C);
        node(fx, 1, sp + 0x40, s0, 0x01E0_2624);
        pos(fx, sp + 0x40);
        spawn(fx, SPRITE, s0, s2, 0x01E0_2650);
    }
    if (0x17..0x6B).contains(&c(fx)) {
        node(fx, 2, sp + 0x40, s0, 0x01E0_2684);
        (s0, s2) = (q0, q2);
        pos(fx, sp + 0x40);
        fx.mem.set16(KIND, 0);
        node(fx, 3, sp + 0x40, s0, 0x01E0_26BC);
        pos(fx, sp + 0x40);
        fx.mem.set16(KIND, 1);
    }
    // (update, node, return addresses: node, disc, quad rings, spun rings,
    // camera).
    let sets: [(i32, u32, [u32; 5]); 2] = [(0xC6, 2, [0x01E0_2704, 0x01E0_2730, 0x01E0_2748, 0x01E0_275C, 0x01E0_2780]), (0xDA, 4, [0x01E0_28A0, 0x01E0_28CC, 0x01E0_28E4, 0x01E0_28F8, 0x01E0_291C])];
    for (when, k, ra) in sets {
        if c(fx) == when {
            node(fx, k, sp + 0x40, s0, ra[0]);
            pos(fx, sp + 0x40);
            spawn(fx, DISC, s0, s2, ra[1]);
            for i in 0..5u64 {
                spawn(fx, RING_A, i, s2, ra[2]);
                spawn(fx, RING_B, i, s2, ra[3]);
            }
            s0 = 5;
            spawn(fx, CAMERA, s0, s2, ra[4]);
        }
    }
    // (updates, node, the facing's local, the kind for a facing of 0,
    // return addresses: node, rand, 0x1A2B80, tick ring, half ring).
    let sets: [(std::ops::Range<i32>, u32, u32, i32, [u32; 5]); 2] = [
        (0xC1..0xC8, 3, 0x50, 1, [0x01E0_27B4, 0x01E0_27C0, 0x01E0_2810, 0x01E0_2854, 0x01E0_2868]),
        (0xD5..0xDC, 5, 0x60, 0, [0x01E0_2950, 0x01E0_295C, 0x01E0_29AC, 0x01E0_29F4, 0x01E0_2A08]),
    ];
    for (when, k, local, kind0, ra) in sets {
        if when.contains(&c(fx)) {
            node(fx, k, sp + 0x40, s0, ra[0]);
            for i in 0..5u64 {
                s0 = i;
                let v = rmod(fx, i as u32, ra[1], 0x168);
                fx.mem.set16(D + 0x2E2E, v);
                let f = facing_of(fx, sp + local, ra[2]);
                fx.mem.set16(KIND, if f == 0 { kind0 } else { 1 - kind0 });
                pos(fx, sp + 0x40);
                spawn(fx, TICK, s0, s2, ra[3]);
                spawn(fx, HALF, s0, s2, ra[4]);
            }
            s0 = 5;
        }
    }
    if c(fx) == 0x44 {
        node(fx, 4, sp + 0x40, s0, 0x01E0_2A40);
        pos(fx, sp + 0x40);
        fx.mem.set16(KIND, 1);
    }
    fx.leave(0x70);
}

/// The two quad rings' step (0x1E02A80 / 0x1E03370): size 15, cell rand %
/// `k.0`, grey, radius +0x30 from 0 growing by +0xE = rand % `k.1` + `k.2`
/// from the second update, +0x3A rand % 360, row +8 rand % `k.3`, life +6
/// rand % `k.4` + `k.5`; 0x80 during the life, out by 0xF.
fn ring_by(fx: &mut Fx, at: u32, k: (i32, i32, i32, i32, i32, i32), t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xF);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x34, v % k.0);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, at + 0x5C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, at + 0x78, k.1);
        fx.mem.set16(t + 0xE, v + k.2);
        let v = rmod(fx, t, at + 0x98, k.3);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, at + 0xB4, k.4);
        fx.mem.set16(t + 6, v + k.5);
        fx.mem.set16(t + 0x36, 2);
    }
    fade(fx, t, Some(0x80));
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, v);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02DF0 (D, task): a grey sprite (size 60, +0x30 rand % 180, +0x3A
/// rand % 360), its cell rand % 8 each update; 0x80 for 12 updates, out
/// by 0xF.
fn sprite(fx: &mut Fx, t: u32) {
    let at = SPRITE;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, at + 0x40, 0xB4);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, at + 0x5C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xC {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0xF);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let v = fx.rand_from(t, at + 0xC0);
    fx.mem.set16(t + 0x34, v % 8);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03090 (D, task): a grey tick ring (size -10 growing by +8 = rand %
/// 3 + 4 from the second update, angles x rand % 0x4000 - 0x2000 / y rand
/// % 0xFFFF, cell rand % 7, +6 = 20); alpha in by 0x40 (below 0x80) for 2
/// updates, then out by 0x13 once the size is 0x15+ (over below 0x15), and
/// out by 0xF (over below 0x11) once the timer is under 10. Frame 0x30.
fn tick(fx: &mut Fx, t: u32) {
    let at = TICK;
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, -0xA);
        let v = fx.rand_from(t, at + 0x30);
        fx.mem.set32(t + 0x10, v % 0x4000 - 0x2000);
        let v = rmod(fx, t, at + 0x50, 0xFFFF);
        fx.mem.set32(t + 0x14, v);
        let v = rmod(fx, t, at + 0x6C, 7);
        fx.mem.set16(t + 0x34, v);
        fx.mem.set16(t + 0x36, 2);
        let v = rmod(fx, t, at + 0x90, 3);
        fx.mem.set16(t + 8, v + 4);
        fx.mem.set16(t + 6, 0x14);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else {
        if fx.mem.i16(t + 0x38) >= 0x15 {
            let a = fx.mem.read_u8(t + 0x2B);
            if a >= 0x15 {
                fx.mem.write_u8(t + 0x2B, a - 0x13);
            } else {
                fx.mem.set16(t + 0x36, 0);
            }
        }
        if fx.mem.i32(TIMER) < 0xA {
            let a = fx.mem.read_u8(t + 0x2B);
            if a >= 0x11 {
                fx.mem.write_u8(t + 0x2B, a - 0xF);
            } else {
                fx.mem.set16(t + 0x36, 0);
            }
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x38) + fx.mem.i16(t + 8);
        fx.mem.set16(t + 0x38, v);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E03220 (D, task): a grey half ring (size 20, cell 1, angle z rand %
/// 0xFFFF, +0x3A = the kind (the side drawn), radius +0x30 from 0 growing
/// by rand % 18 + 130 from the second update, row rand % 10, life rand %
/// 7 + 3); in by 0x40 (to 0x80) during the life, then out by 0xA.
fn half(fx: &mut Fx, t: u32) {
    let at = HALF;
    fx.enter(0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x14);
        fx.mem.set16(t + 0x34, 1);
        let v = rmod(fx, t, at + 0x2C, 0xFFFF);
        fx.mem.set32(t + 0x18, v);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let s = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, s);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, at + 0x60, 0x12);
        fx.mem.set16(t + 0xE, v + 0x82);
        let v = rmod(fx, t, at + 0x80, 0xA);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, at + 0x9C, 7);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x81 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, v);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E012B0 (`spun`: D, task, mode, table) / 0x1E01BB0 (D, task, table,
/// side): `quads4::ring_spun`'s ring of D+0xB8 (0: 20) textured quads
/// (blend 2) whose quads face the camera first (turned about x / y by its
/// negated angles / 182), then are pushed out by the bytes `table` / 20
/// times +0x30 + k / 2 and turned by the task's angles +0x10 (axes 0, 1,
/// 2). 0x1E012B0: UVs in D+0xBE's orientation, through D+0x20 turned by
/// the task's angles too (scale 0.1), depth `mode` (0 the mean w, 1 the
/// far Z at key 0, 2 key -1). 0x1E01BB0: UVs in orientation 0, at the
/// task's i16 point (not turned), each quad drawn (mean w) only when its
/// first vertex's x is below 0 (side 0) or above 0 (else).
fn ring_faced(fx: &mut Fx, t: u32, mode: i32, table: u32, spun: bool) {
    let d0 = D;
    let sp = fx.enter(0x200);
    let d = sp + 0x168;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha(2));
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, sp + 0x1F8, t + 0x28, d + 0x10);
    let order = if spun { fx.mem.i16(d0 + 0xBE) } else { 0 };
    uv::uv_rect(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    let count = match fx.mem.i16(d0 + 0xB8) {
        0 => 0x14,
        n => n,
    };
    scale_tenth(fx, sp + 0xF0);
    l2::camera_angles_to(fx, sp + 0x150);
    if spun {
        for k in 0..3 {
            let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
            fx.mem.write_u32(sp + 0xE0 + 4 * k, w);
        }
        to_degrees_neg(fx, sp + 0x154);
        to_degrees_neg(fx, sp + 0x150);
        l2::xform_vp(fx, sp + 0x110, sp + 0xF0, t + 0x10, sp + 0xE0);
    } else {
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0xE0 + 4 * k, v);
        }
        l2::xform_vp(fx, sp + 0x110, sp + 0xF0, 0, sp + 0xE0);
        to_degrees_neg(fx, sp + 0x154);
        to_degrees_neg(fx, sp + 0x150);
    }
    let tab = d0 + 0xD8;
    let q = sp + 0xA0;
    let mut k = 0i32;
    while k < count {
        let size = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, q, size, size);
        l2::rotate4(fx, q, sp + 0x150, 0, tab);
        l2::rotate4(fx, q, sp + 0x150, 1, tab);
        let row = table.wrapping_add(k as u32);
        let half = k >> 1;
        for (axis, off) in [(0, 0u32), (1, 0x64), (2, 0xB4)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::div(ee::from_i32(fx.mem.i8(row.wrapping_add(i as u32).wrapping_add(off))), 20.0);
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + half);
            l2::add_column(fx, q, axis, ee::mul(b, r));
        }
        for axis in 0..3 {
            l2::rotate4(fx, q, t + 0x10, axis, tab);
        }
        let ot = fx.mem.read_u32(d0 + 0x80);
        if spun {
            if (0..=2).contains(&mode) && l2::project(fx, q, SPR, sp + 0x110, d, sp + 0x100) {
                match mode {
                    0 => {
                        let key = ee::fixunssfsi(fx.mem.f(sp + 0x100)) as i32;
                        fx.submit(d, ot, key);
                    }
                    1 => {
                        l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                        fx.submit(d, ot, 0);
                    }
                    _ => fx.submit(d, ot, -1),
                }
            }
        } else {
            let x = fx.mem.f(q);
            let shown = if mode == 0 { x < 0.0 } else { !(x <= 0.0) };
            if shown && l2::project(fx, q, SPR, sp + 0x110, d, sp + 0x100) {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x100)) as i32;
                fx.submit(d, ot, key);
            }
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x200);
}
