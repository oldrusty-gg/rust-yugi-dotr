//! BMODULE b500 (card 417), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03A40, data area 0x1ED0000, 0x4270 bytes) [F]
//!: the disc family's pool (200 tasks of 0x40 at
//! D+0x2F0, alive +0x36, age +0x32, step +0x24, spawned by 0x1E046A0 =
//! b148's spawn on its own globals: colour D+0x3BB4, kind D+0x3BF6) with
//! every other global 0x5A0 higher (timer D+0x3BB0 = 400, nodes
//! D+0x3BDC.., counter D+0x3BFC, tables D+0x3BFE.., sines D+0x4100,
//! group-5 textures D+0x3B90..). On update 0x1F a ribbon task (0x1E025C0:
//! four 10-point node histories D+0x35D0 / 0x3670 / 0x3710 / 0x37B0 of
//! nodes 0xE / 0xF / 0x1C / 0x1D, drawn as two gouraud ribbons by
//! 0x1E047C0, alpha 0x60 for 190 updates) and an object op task
//! (0x1E03910, stepped inline: 0x1F8380(2, 0, ..) for 10 updates). On
//! updates 0xDD / 0x10C / 0x112 / 0x118 at node 0xF and 0xF3 / 0x10F /
//! 0x115 at node 0x1D: a disc (`spawn::disc_step` ede11d4434f2,
//! `quads4::fan18`), ten strip rings (`task40::ring_strip`,
//! `billboard::ring_strip3`), seven grey quad rings (0x1E02D10, b046's
//! ring growing by 20..59, `quads4::ring_spun`), five pushed sprites
//! (0x1E03260, `quads4::sprite_turned`) and a six-sprite ring (0x1E033E0,
//! drawn by 0x1E01D60). On 0x5A / 0x79 / 0x96 / 0xB4 twelve pushed sprites
//! at node 0x21, on 0x4D / 0x69 / 0x87 / 0xA5 / 0xC1 twelve at node 0x28;
//! 0x50..0x64 the spawn point at node 0x21. Returns 1 on update 0xC8, 3
//! on 0x122, 6 on 0xDC..0x117, 8 once the timer has run out.
//!
//! The steps 0x1E029E0 (b148's sprite), 0x1E02BD0 (`task40::band_grow`),
//! 0x1E02E90, 0x1E03020, 0x1E03500, 0x1E036D0 and 0x1E03850 are tested by
//! the entry but never spawned [F: the emitter's spawns name 0x1E025C0 /
//! 0x1E03910 / 0x1E02AF0 / 0x1E02830 / 0x1E02D10 / 0x1E03260 / 0x1E033E0
//! only]; not ported. No 0x1F8100.
use crate::b046::ring_by;
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b148::{age, facing_of, rmod, spawn_task_on};
use crate::blib::{billboard, quads4, spawn, spline, task40, uv};
use crate::bus::Bus;
use crate::c061::{desc_uv4, rgbaq_at, to_degrees_neg};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
const POOL: u32 = D + 0x2F0;
const POS: u32 = D + 0x250;
const TIMER: u32 = D + 0x3BB0;
const COLOUR: u32 = D + 0x3BB4;
const NODES: u32 = D + 0x3BDC;
const KIND: u32 = D + 0x3BF6;
const COUNTER: u32 = D + 0x3BFC;
const TABLE: u32 = D + 0x3BFE;
const SINES_B: u32 = D + 0x4100;
const SPR: u32 = 0x7000_0000;

const HIST: u32 = 0x01E0_25C0;
const STRIP: u32 = 0x01E0_2830;
const DISC: u32 = 0x01E0_2AF0;
const RING: u32 = 0x01E0_2D10;
const PUSH: u32 = 0x01E0_3260;
const FAN: u32 = 0x01E0_33E0;
const OBJECT: u32 = 0x01E0_3910;

/// The entry 0x1E03A40 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1B0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x3BBC, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x1B0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4270);
    l2::sine_table(fx, SINES_B);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3BFA, param);
    fx.mem.set32(TIMER, 0x190);
    for (k, n) in [0xE, 0xF, 0x1C, 0x1D, 0x21, 0x28, 0x14, 0x37].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // 0x1E03990(0, D): the textures (frame 0x40: ra, the entry's s2 =
    // param, s1 = 0, s0 = D).
    let f = fx.enter(0x40);
    fx.saved(f, &[(0x30, 0x01E0_3B6C), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
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
        fx.mem.write_u32(D + 0x3B90 + 4 * k as u32, t);
    }
    fx.leave(0x40);
    let f = facing_of(fx, sp + 0xD0, 0x01E0_3BA8);
    fx.mem.set16(D + 0x3BF8, f);
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3BB4, 0x1E);
        fx.mem.set8(TABLE + k, v - 0xF);
    }
    let b = TABLE + 0x100;
    for k in 0..0x64u32 {
        fx.mem.write_u8(b + k, 0);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_3C08, 5);
        fx.mem.set8(b + k, v);
    }
    let f = facing_of(fx, sp + 0xE0, 0x01E0_3C64);
    for k in 0xB4..0x100u32 {
        let (ra, sign) = if f != 0 { (0x01E0_3C7C, 1) } else { (0x01E0_3CA4, -1) };
        let v = fx.rand_from(D, ra);
        fx.mem.set8(b + k, sign * (v % 4));
    }
    let b = TABLE + 0x200;
    for k in 0..0x64u32 {
        fx.mem.write_u8(b + k, 0);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_3D04, 6);
        fx.mem.set8(b + k, v);
    }
    let f = facing_of(fx, sp + 0xF0, 0x01E0_3D60);
    for k in 0xB4..0x100u32 {
        let (ra, sign) = if f != 0 { (0x01E0_3D78, 1) } else { (0x01E0_3DA4, -1) };
        let v = rmod(fx, D, ra, 0x14);
        fx.mem.set8(b + k, sign * (v + 5));
    }
    let b = TABLE + 0x300;
    for k in 0..0x64u32 {
        fx.mem.write_u8(b + k, 0);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_3E04, 3);
        fx.mem.set8(b + k, v);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_3E30);
        fx.mem.set8(b + k, v % 2 - 1);
        let f = facing_of(fx, sp + 0x100, 0x01E0_3E8C);
        let c = fx.mem.i8(b + k);
        fx.mem.set8(b + k, if f != 0 { c + 2 } else { c - 2 });
    }
    // The third table refilled: rand % 30 - 15, 0..4 pushed to 7..11 and
    // -4..-1 to -11..-8.
    let b = TABLE + 0x200;
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3ECC, 0x1E) - 0xF;
        let v = match v {
            0..=4 => v + 7,
            -4..=-1 => v - 7,
            _ => v,
        };
        fx.mem.set8(b + k, v);
    }
    let b = TABLE + 0x400;
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3F54, 0x1E);
        fx.mem.set8(b + k, v - 0xF);
    }
    for k in 0x64..0x100u32 {
        fx.mem.write_u8(b + k, 0);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    let low = fx.sp;
    let dd = u64::from(D);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let s2 = u64::from(D + 0x40 * k);
        match fx.mem.read_u32(t + 0x24) {
            HIST => {
                hist(fx, t);
                fx.saved(low - 0x80, &[(0x30, 0x01E0_3FE4), (0x20, s2), (0x10, 0), (0, dd)]);
            }
            STRIP => {
                task40::ring_strip(fx, STRIP, D, t);
                fx.saved(low - 0x20, &[(0x10, 0x01E0_4008), (0, dd)]);
            }
            DISC => {
                let c = spawn::disc("ede11d4434f2").expect("constants");
                spawn::disc_step_with(fx, c, DISC, t);
                fx.saved(low - 0x20, &[(0x10, 0x01E0_4048), (0, dd)]);
            }
            RING => {
                ring_by(fx, RING, (0x14, 2, 0x14), t);
                fx.saved(low - 0x20, &[(0x10, 0x01E0_4088), (0, dd)]);
            }
            PUSH => {
                push(fx, t);
                fx.saved(low - 0x30, &[(0x20, 0x01E0_40E8), (0x10, 0), (0, dd)]);
            }
            FAN => {
                fan(fx, t);
                fx.saved(low - 0x30, &[(0x10, 0x01E0_4108), (0, dd)]);
            }
            OBJECT => {
                for (to, from) in [(0x1ACu32, 0x5100u32), (0x1A8, 0x5108)] {
                    let w = fx.mem.read_u32(0x01E0_0000 + from);
                    fx.mem.write_u32(sp + to, w);
                }
                fx.boundary(0x001F_8380, "object op 0x1F8380", [2, 0, sp + 0x1AC, sp + 0x1A8]);
                if fx.mem.i16(t + 0x32) >= 0xA {
                    fx.mem.set16(t + 0x36, 0);
                }
                age(fx, t);
            }
            f => {
                debug_assert!(false, "b500: unported task step {f:#x}");
            }
        }
    }
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x50, dd, 0x01E0_41FC);
    let d = sp + 0x118;
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
        if f(fx) == HIST {
            let w = fx.mem.read_u32(D + 0x2EC);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 1), (0x96, 0x10), (0x98, 0x1F), (0x9A, 1)] {
                fx.mem.set16(D + o, v);
            }
            let sub = fx.mem.i16(t + 0x34);
            l2::rect(fx, sp + 0x60, 0x97, sub);
            for k in 0..4 {
                let v = fx.mem.read_u32(sp + 0x60 + 4 * k);
                fx.mem.write_u32(D + 0x94 + 4 * k, v);
            }
            fx.mem.set16(D + 0x98, 1);
            for k in 0..3 {
                fx.mem.write_u32(D + 0x20 + 4 * k, 0);
            }
            fx.mem.set16(D + 0xBE, 1);
            let saves = |ra: u32| [dd, s1, u64::from(D + 0x40 * k), u64::from(k), u64::from(ra)];
            ribbon(fx, t, D + 0x3540, D + 0x35E0, saves(0x01E0_42AC));
            ribbon(fx, t, D + 0x3680, D + 0x3720, saves(0x01E0_42C0));
        }
        if f(fx) == STRIP {
            set_rect(fx, D + 0x2EC, sp + 0x70, 0x9A);
            fx.mem.set16(D + 0xB8, 5);
            billboard::ring_strip3(fx, D, t, d, TABLE + 0x200);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == DISC {
            set_rect(fx, D + 0x2EC, sp + 0x80, 0x9B);
            quads4::fan18(fx, D, t, d);
        }
        if f(fx) == RING {
            set_rect(fx, D + 0x3B98, sp + 0x90, 0x14);
            fx.mem.set16(D + 0xB8, 0xA);
            point_at(fx, D + 0x20);
            quads4::ring_spun(fx, uv::uv_rect, D, t, 0, TABLE);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == PUSH {
            set_rect(fx, D + 0x3B9C, sp + 0xB0, 0x14);
            fx.mem.set16(D + 0xBE, 0);
            point_at(fx, sp + 0x50);
            quads4::sprite_turned(fx, uv::uv_rect, D, t, 0, sp + 0x50);
        }
        if f(fx) == FAN {
            set_rect(fx, D + 0x2B0, sp + 0xC0, 0x14);
            ring6(fx, t, 0);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xC8 => 1,
        0x122 => 3,
        0xDC..=0x117 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E02180(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the spawns (frame 0x40: ra, the entry's s1 = 0, s0 = D; its s0 is
/// D, s1 each loop's count, both saved by the spawns with the entry's s2 =
/// param and s3 / s4 the runner's (0 [H])).
fn emit(fx: &mut Fx, param: i32) {
    let p = param as i64 as u64;
    let dd = u64::from(D);
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, 0x01E0_3FB0), (0x10, 0), (0, dd)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let spawn = |fx: &mut Fx, step: u32, s1: u64, ra: u32| spawn_task_on(fx, TASKS, COLOUR, KIND, step, [dd, s1, p, 0, 0], ra);
    let node = |fx: &mut Fx, k: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, slot, sp + 0x30, dd, Some(s1), ra);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    };
    let mut s1 = 0u64;
    if c(fx) == 0x1F {
        spawn(fx, HIST, s1, 0x01E0_21D0);
    }
    if c(fx) == 0x1F {
        spawn(fx, OBJECT, s1, 0x01E0_21F4);
    }
    // (updates, node, return addresses: node, disc, strips, rings, pushed
    // sprites, the six-sprite ring).
    let sets: [(&[i32], u32, [u32; 6]); 2] = [
        (&[0xDD, 0x10C, 0x112, 0x118], 1, [0x01E0_2240, 0x01E0_226C, 0x01E0_2284, 0x01E0_22AC, 0x01E0_22D8, 0x01E0_22FC]),
        (&[0xF3, 0x10F, 0x115], 3, [0x01E0_233C, 0x01E0_2368, 0x01E0_2380, 0x01E0_23A8, 0x01E0_23D4, 0x01E0_23F8]),
    ];
    for (when, k, ra) in sets {
        if when.contains(&c(fx)) {
            node(fx, k, s1, ra[0]);
            spawn(fx, DISC, s1, ra[1]);
            for i in 0..0xAu64 {
                spawn(fx, STRIP, i, ra[2]);
            }
            for i in 0..7u64 {
                spawn(fx, RING, i, ra[3]);
            }
            fx.mem.set16(KIND, 0);
            for i in 0..5u64 {
                spawn(fx, PUSH, i, ra[4]);
            }
            s1 = 5;
            spawn(fx, FAN, s1, ra[5]);
        }
    }
    let sets: [(&[i32], u32, u32, u32); 2] = [(&[0x5A, 0x79, 0x96, 0xB4], 4, 0x01E0_244C, 0x01E0_2478), (&[0x4D, 0x69, 0x87, 0xA5, 0xC1], 5, 0x01E0_24E8, 0x01E0_2514)];
    for (when, k, ra_node, ra_spawn) in sets {
        if when.contains(&c(fx)) {
            fx.mem.set16(KIND, 0);
            for i in 0..0xCu64 {
                node(fx, k, i, ra_node);
                spawn(fx, PUSH, i, ra_spawn);
            }
            s1 = 0xC;
        }
    }
    if (0x50..0x65).contains(&c(fx)) {
        node(fx, 4, s1, 0x01E0_255C);
        fx.mem.set16(KIND, 0);
        fx.mem.set16(KIND, 1);
    }
    fx.leave(0x40);
}

/// 0x1E025C0 (D, task): the ribbons' task: on the first update colour
/// (0x40, 0x10, 0x10), the histories D+0x3540 / 0x35E0 / 0x3680 / 0x3720
/// (9 points each) filled with nodes 0xE / 0xF / 0x1C / 0x1D, 5 points
/// drawn (+6), alpha 0, +0xA rand % 17 + 44, +8 rand % 8 + 5, +0x3A rand %
/// 360, size 5; alpha 0x60 for 190 updates, then out by 0x13 (over below
/// 0x15); each update the four 10-point histories take their nodes. Frame
/// 0x80 (s0 = t, s1 = D, s2 the history).
fn hist(fx: &mut Fx, t: u32) {
    let at = HIST;
    let sp = fx.enter(0x80);
    let dd = u64::from(D);
    if fx.mem.i16(t + 0x36) == 1 {
        l2::rgb(fx, t + 0x28, 0x40, 0x10, 0x10);
        for (k, (first, local, ra)) in [(0x3540u32, 0x40u32, 0x54u32), (0x35E0, 0x50, 0x98), (0x3680, 0x60, 0xDC), (0x3720, 0x70, 0x120)].into_iter().enumerate() {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 * k as u32);
            node_position_saving(fx, h, slot, sp + local, u64::from(t), Some(dd), at + ra);
            for i in 0..9u32 {
                for c in 0..3 {
                    let w = fx.mem.read_u32(sp + local + 4 * c);
                    fx.mem.write_u32(D + first + 0x10 * i + 4 * c, w);
                }
            }
        }
        fx.mem.set16(t + 6, 5);
        fx.mem.write_u8(t + 0x2B, 0);
        let v = rmod(fx, t, at + 0x15C, 0x11);
        fx.mem.set16(t + 0xA, v + 0x2C);
        let v = fx.rand_from(t, at + 0x17C);
        fx.mem.set16(t + 8, v % 8 + 5);
        fx.mem.set16(t + 0x34, 0);
        let v = rmod(fx, t, at + 0x1A0, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x38, 5);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xBE {
        fx.mem.write_u8(t + 0x2B, 0x60);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x15 {
            fx.mem.write_u8(t + 0x2B, a - 0x13);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    for (k, p) in [0x35D0u32, 0x3670, 0x3710, 0x37B0].into_iter().enumerate() {
        let n = fx.mem.i16(NODES + 2 * k as u32);
        spawn::node_history(fx, D + p, 0xA, n);
    }
    age(fx, t);
    fx.leave(0x80);
}

/// 0x1E03260 (D, task): a pushed sprite (size 60, cell 5, (0x80, 0x60,
/// 0x60); +0x30 rand % 360 for kind 0, else 0; +0x3A rand % 360); 0x80 for
/// 7 updates, out by 0xF; while +0x30 is set pushed each update by 10 sin
/// / 10 cos of +0x3A (the second sine table) into +8 / +0xC. Frame 0x30.
fn push(fx: &mut Fx, t: u32) {
    let at = PUSH;
    fx.enter(0x30);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        fx.mem.set16(t + 0x34, 5);
        l2::rgb(fx, t + 0x28, 0x80, 0x60, 0x60);
        if fx.mem.i16(t + 0x30) == 0 {
            let v = rmod(fx, t, at + 0x5C, 0x168);
            fx.mem.set16(t + 0x30, v);
        } else {
            fx.mem.set16(t + 0x30, 0);
        }
        let v = rmod(fx, t, at + 0x84, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0xF);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x30) != 0 {
        for (o, cos, ra) in [(8u32, 0, 0x120u32), (0xC, 1, 0x158)] {
            let deg = fx.mem.i16(t + 0x3A);
            let s = l2::sin_deg(fx, SINES_B, deg, cos);
            let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(10.0, s));
            fx.fix_residue(v, at + ra);
            fx.mem.set16(t + o, ee::fixsfsi(v));
        }
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E033E0 (D, task): the six-sprite ring (size 10, cell 0 up to 7 from
/// the second update, +0x3A rand % 360, radius +6 from 0 growing by +8 = 2
/// a update, grey); alpha 0x80 on the first update, then out by 0x12 (over
/// below 0x14). The attacker's rotation read first (unused). Frame 0x30.
fn fan(fx: &mut Fx, t: u32) {
    let at = FAN;
    let sp = fx.enter(0x30);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xA);
        fx.mem.set16(t + 0x34, 0);
        let v = rmod(fx, t, at + 0x40, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 8, 2);
        fx.mem.set16(t + 6, 0);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) <= 0 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, 0x80);
        }
    } else if a >= 0x14 {
        fx.mem.write_u8(t + 0x2B, a - 0x12);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x32) != 0 && fx.mem.i16(t + 0x34) < 7 {
        let v = fx.mem.i16(t + 0x34) + 1;
        fx.mem.set16(t + 0x34, v);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 6) + fx.mem.i16(t + 8);
        fx.mem.set16(t + 6, v);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E01D60 (D, task, mode): six textured sprites (9 values, FST) of half
/// size i16 +0x38 at 60-degree steps from i16 +0x3A on a circle of radius
/// i16 +6 (y sin, z cos; the second sine table D+0xD8) about the task's
/// i16 point, the ring turned by the task's angles +0x10 (no scale), each
/// sprite turned about y by the camera's negated y angle / 182; coloured
/// +0x28, UVs D+0x94 in orientation D+0xBE, texture D+0x88; ALPHA by mode
/// >> 4 (0: 0x48, 1: 0x44, else 0x42); keyed by the mean w into D+0x80
/// (mode & 15 = 1 at the far Z, else not drawn for 2+).
fn ring6(fx: &mut Fx, t: u32, mode: i32) {
    let sp = fx.enter(0x1F0);
    let d = sp + 0x158;
    desc_uv4(fx, d);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    let alpha = match mode >> 4 {
        0 => 0x80_0000_0048u64,
        1 => 0x80_0000_0044,
        _ => 0x80_0000_0042,
    };
    fx.desc_alpha(d, alpha);
    rgbaq_at(fx, sp + 0x1E8, t + 0x28, d + 0x10);
    let order = fx.mem.i16(D + 0xBE);
    uv::uv_rect(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xE0 + 4 * k, v);
    }
    l2::camera_angles_to(fx, sp + 0x140);
    to_degrees_neg(fx, sp + 0x144);
    l2::xform_vp(fx, sp + 0xF0, 0, t + 0x10, sp + 0xE0);
    fx.mem.write_u32(sp + 0xE0, 0);
    let q = sp + 0xA0;
    let mut a = 0i32;
    for _ in 0..6 {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, q, s, s);
        l2::rotate4(fx, q, sp + 0x140, 0, D + 0xD8);
        let deg = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let c = l2::sin_deg(fx, D + 0xD8, deg, 1);
        let r = ee::from_i32(fx.mem.i16(t + 6));
        fx.mem.setf(sp + 0xE8, ee::mul(r, c));
        let s = l2::sin_deg(fx, D + 0xD8, deg, 0);
        let r = ee::from_i32(fx.mem.i16(t + 6));
        fx.mem.setf(sp + 0xE4, ee::mul(r, s));
        for axis in 0..3 {
            let v = fx.mem.f(sp + 0xE0 + 4 * axis as u32);
            l2::add_column(fx, q, axis, v);
        }
        if l2::project(fx, q, SPR, sp + 0xF0, d, sp + 0x130) {
            let ot = fx.mem.read_u32(D + 0x80);
            match mode & 0xF {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x130)) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x130)) as i32;
                    fx.submit(d, ot, key);
                }
                _ => {}
            }
        }
        a = (a + 0x3C) as i16 as i32;
    }
    fx.leave(0x1F0);
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 0x1E047C0 (D, task, a, b): b501's gouraud ribbon (0x1E047C0 there)
/// between the point histories `a` and `b` (i16 +6 points; four strips a
/// segment by `spline::spline` basis 0, key 0 through the camera), its
/// alpha from 0: the later pair's up by alpha / (2 n) a strip (to 0xFF)
/// over the first 2 n strips, then down by it (to 0) [F]. `saves`: the
/// draw loop's s0 / s1 / s2 / s3 and the return address (s4..fp the
/// runner's, 0 [H]).
fn ribbon(fx: &mut Fx, t: u32, a: u32, b: u32, saves: [u64; 5]) {
    use ee::add;
    let sp = fx.enter(0x320);
    fx.saved(sp, &[(0x10, saves[0]), (0x20, saves[1]), (0x30, saves[2]), (0x40, saves[3]), (0x50, 0), (0x60, 0), (0x70, 0), (0x80, 0), (0x90, 0), (0xA0, saves[4])]);
    fx.mem.write_u32(sp + 0x100, a);
    fx.mem.write_u32(sp + 0xF0, b);
    fx.mem.write_u32(sp + 0xB8, OT);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x268;
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    let colour = sp + 0x31C;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(colour + k, v);
    }
    fx.mem.write_u8(colour + 3, 0);
    let (pa, pb) = (sp + 0x1A0, sp + 0x1E0);
    for (p, src) in [(pa, a), (pb, b)] {
        copy4(fx, p, src);
        copy4(fx, p + 0x10, src);
        copy4(fx, p + 0x20, src + 0x10);
        copy4(fx, p + 0x30, src + 0x20);
    }
    fx.mem.write_u32(sp + 0xC0, 0);
    fx.mem.set32(sp + 0xD0, 2 * n);
    fx.mem.set32(sp + 0xE0, n - 1);
    let half = (4 * n) >> 1;
    let mut strip = 0i32;
    let mut s0 = 0;
    while s0 < n - 1 {
        let off = (0x10 * (s0 - 1)) as u32;
        let (s4, s5) = (a.wrapping_add(off), b.wrapping_add(off));
        let mut f20 = 0.0f32;
        for _ in 0..4 {
            rgbaq_at(fx, sp + 0x2F8, colour, d + 0x10);
            rgbaq_at(fx, sp + 0x300, colour, d + 0x30);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let q = fx.div(al, 2 * n).0 as i16 as i32;
            let c = i32::from(fx.mem.read_u8(colour + 3));
            let v = if strip < half {
                if c + q < 0xFF {
                    c + q
                } else {
                    0xFF
                }
            } else if c - q > 0 {
                c - q
            } else {
                0
            };
            fx.mem.write_u8(colour + 3, v as u8);
            rgbaq_at(fx, sp + 0x308, colour, d + 0x20);
            rgbaq_at(fx, sp + 0x310, colour, d + 0x40);
            let (qa, qb, oa, ob) = if s0 != 0 { (s4, s5, 0x220u32, 0x230u32) } else { (pa, pb, 0x240, 0x250) };
            spline::spline(fx, sp + 0x160, qa, f20, 0, [0.0; 4]);
            spline::spline(fx, sp + 0x180, qb, f20, 0, [0.0; 4]);
            f20 = add(f20, 0.25);
            spline::spline(fx, sp + oa, qa, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x170, sp + oa);
            spline::spline(fx, sp + ob, qb, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x190, sp + ob);
            for (to, from) in [(0x120u32, 0x160u32), (0x130, 0x170), (0x140, 0x180), (0x150, 0x190)] {
                copy3(fx, sp + to, sp + from);
            }
            if l2::project(fx, sp + 0x120, SPR, 0, d, sp + 0x110) {
                fx.submit(d, OT, 0);
            }
            strip += 1;
        }
        let v = fx.mem.i32(sp + 0xC0) + 4;
        fx.mem.set32(sp + 0xC0, v);
        s0 += 1;
    }
    fx.leave(0x320);
}
