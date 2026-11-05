//! BMODULE b411 (card 517), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02790, data area 0x1ED0000) [F]: a pool of 300 tasks (0x70 bytes at D+0x2A0, alive +0x58, step
//! +0x44 through `jalr`, 0x1E02780 only returns; spawned by 0x1E02F40 =
//! b252's spawn with the colour D+0x9C84, the slot D+0x9D20 into +0x50 /
//! +0x52 and the angle D+0x9D12 into +0x5A), a 280-update timer D+0x9C80,
//! the attacker's nodes 0x28 / 0x38 / 0x48 / 0x30 / 0x2F / 0x2C, the
//! target's node 1, textures (4, 0xE..0xF), (5, 0..1) and the second
//! library (`ring_quads`, `quad`, `arc_strips`, `tube`, `history`). The
//! emitter 0x1E00FD0 counts D+0x9D28: on update 1 the stage light
//! (0x1E02470, down to 0x30 for 0x97 updates, back near the end); on 0x3B
//! three arcs (0x1E02170) riding the nodes 0x28 / 0x38 / 0x48; on 0x6C /
//! 0x82 / 0x8C at node 0x38 / 0x48 / 0x28 (slot 0 / 1 / 2: the point
//! D+0x9BF0 + 0x10 slot) an orb (0x1E01D30: a 70-point history D+0x8610 +
//! 0x460 slot drawn by `tube`), its arc (0x1E022F0) and twenty motes
//! (0x1E01960) thrown out at 18-degree steps; from those updates to 0xC6 /
//! 0xDC / 0xE6 a blue mote ring (0x1E015C0) a update at the slot's point.
//! The orb drifts 3.5 along z (by the attacker's facing) and sinks 0.3 for
//! 50 updates, then (slot 1) sheds an arc burst (0x1E01B30) and a white
//! mote ring (0x1E01790) on its odd ages, with a frame flash
//! (0x1E02580 = b590's 0x1E02450 in by 0xC under 0x30 to update 0xFA) at
//! age 50. Returns 1 on update 0x9A, 4 on 0x10E, 2 on 0xFF, 6 on
//! 0xA5..0xF9, 8 once the timer has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, spawn, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 300;
/// The step functions a task holds (+0x44), as the record's addresses.
const MOTES_B: u32 = 0x01E0_15C0;
const MOTES_W: u32 = 0x01E0_1790;
const MOTE: u32 = 0x01E0_1960;
const BURST: u32 = 0x01E0_1B30;
const ORB: u32 = 0x01E0_1D30;
const ARC_N: u32 = 0x01E0_2170;
const ARC_O: u32 = 0x01E0_22F0;
const LIGHT: u32 = 0x01E0_2470;
const FLASH: u32 = 0x01E0_2580;
const IDLE: u32 = 0x01E0_2780;
/// The return address of the entry's `jalr` into a step (0x1E02A98 + 8).
const TASK_RA: u64 = 0x01E0_2AA0;
const POS: u32 = D + 0x250;
/// The three slots' points (16 bytes each).
const POINTS: u32 = D + 0x9BF0;
const TEX: u32 = D + 0x9C60;
const TIMER: u32 = D + 0x9C80;
const COLOUR: u32 = D + 0x9C84;
const NODES: u32 = D + 0x9CB4;
const TNODE: u32 = D + 0x9CC4;
const ANGLE: u32 = D + 0x9D12;
const SLOT: u32 = D + 0x9D20;
const FACING: u32 = D + 0x9D24;
const COUNTER: u32 = D + 0x9D28;
const BYTES: u32 = D + 0x9D2A;
const SINES: u32 = D + 0x9F2C;
/// The orbs' histories (70 points of 16 bytes a slot).
const HISTORY: u32 = D + 0x8610;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn history(slot: i32) -> u32 {
    HISTORY.wrapping_add((0x460 * slot) as u32)
}

fn point(slot: i32) -> u32 {
    POINTS.wrapping_add((0x10 * slot) as u32)
}

/// The entry 0x1E02790 (s0 = 0 then the live count, s1 = D, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(D + 0x9C8C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xA0A0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x9D26, param);
    fx.mem.set32(TIMER, 0x118);
    for (k, node) in [0x28, 0x38, 0x48, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TNODE, r);
    for k in 0xEu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x260 + 4 * k, t);
    }
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.fix_residue(y, 0x01E0_2980);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // The loops' rand frames save the counter s0.
    for k in 0..0x100u32 {
        let r = rmod(fx, k, 0x01E0_29B0, 0x14);
        fx.mem.set8(BYTES + k, r - 0xA);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_2A04);
        fx.mem.set16(D + 0x9CD4 + 2 * k, v.wrapping_rem(8));
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    let dd = u64::from(D);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        let frame = match f {
            0 | IDLE => continue,
            MOTES_B | MOTES_W | BURST | LIGHT => 0x20,
            MOTE | ARC_N | ARC_O => 0x30,
            ORB => 0x60,
            FLASH => 0x90,
            _ => panic!("b411: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        // The steps' saves: the entry's s0 = 0, s1 = D, s2 = the slot, s3
        // = its index.
        match frame {
            0x20 => fx.saved(s, &[(0x10, TASK_RA), (0, 0)]),
            0x30 | 0x90 => fx.saved(s, &[(0x20, TASK_RA), (0x10, dd), (0, 0)]),
            _ => fx.saved(s, &[(0x40, TASK_RA), (0x30, u64::from(k)), (0x20, u64::from(t - 0x2A0)), (0x10, dd), (0, 0)]),
        }
        match f {
            MOTES_B => motes(fx, t, MOTES_B),
            MOTES_W => motes(fx, t, MOTES_W),
            MOTE => mote(fx, t),
            BURST => burst(fx, t),
            ORB => orb(fx, t, s),
            ARC_N => arc_n(fx, t),
            ARC_O => arc_o(fx, t),
            LIGHT => light(fx, t),
            _ => flash(fx, t, s),
        }
        fx.leave(frame);
    }
    let mut count = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        count += 1;
        draw(fx, sp, t, count);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x9A => 1,
        0x10E => 4,
        0xFF => 2,
        0xA5..=0xF9 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw of one live task by its step (s0 = `count`, the live
/// tasks so far).
fn draw(fx: &mut Fx, sp: u32, t: u32, count: u32) {
    let cell = |fx: &mut Fx, r: u32| {
        let sub = fx.mem.i16(t + 0x56);
        l2::rect(fx, sp + r, 0x14, sub);
        for k in 0..4 {
            let x = fx.mem.read_u32(sp + r + 4 * k);
            fx.mem.write_u32(D + 0x94 + 4 * k, x);
        }
    };
    let tex = |fx: &mut Fx, k: u32| {
        let w = fx.mem.read_u32(TEX + 4 * k);
        fx.mem.write_u32(D + 0x88, w);
    };
    let strip = |fx: &mut Fx| {
        let v = fx.mem.i16(t + 0x56) << 5;
        fx.mem.set16(D + 0x94, v);
        fx.mem.set16(D + 0x96, 0);
        fx.mem.set16(D + 0x98, 0x20);
        fx.mem.set16(D + 0x9A, 0x40);
    };
    let g = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
    if g(fx) == MOTES_B {
        cell(fx, 0x60);
        tex(fx, 1);
        strip(fx);
        fx.mem.set16(D + 0xB8, 4);
        let v = fx.rand_from(count, 0x01E0_2B6C);
        fx.mem.set16(D + 0xBE, v.wrapping_rem(4));
        fx.mem.set16(D + 0xB0, 0);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if g(fx) == MOTES_W {
        cell(fx, 0x70);
        tex(fx, 0);
        fx.mem.set16(D + 0xB8, 4);
        let v = fx.rand_from(count, 0x01E0_2C10);
        fx.mem.set16(D + 0xBE, v.wrapping_rem(4));
        fx.mem.set16(D + 0xB0, 1);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if g(fx) == MOTE {
        tex(fx, 1);
        strip(fx);
        let v = fx.mem.i16(t + 0x52);
        fx.mem.set16(D + 0xBE, v);
        billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
    }
    if g(fx) == BURST {
        fx.mem.set16(D + 0xB8, 0x24);
        billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if g(fx) == ORB {
        tex(fx, 0);
        cell(fx, 0x80);
        let v = fx.mem.i16(D + 0x96) + 0xF;
        fx.mem.set16(D + 0x96, v);
        fx.mem.set16(D + 0x98, 0x1F);
        fx.mem.set16(D + 0x9A, 1);
        let h = fx.self_handle();
        unit_position_saving(fx, h, sp + 0x50, u64::from(count), 0x01E0_2D70);
        for o in [0x20u32, 0x24, 0x28] {
            fx.mem.write_u32(D + o, 0);
        }
        fx.mem.set16(D + 0xBE, 2);
        let s = fx.mem.i16(t + 0x50);
        tubetrail::tube(fx, D, t, history(s), 0);
    }
    if g(fx) == ARC_N {
        fx.mem.set16(D + 0xB8, 0x24);
        billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if g(fx) == ARC_O {
        fx.mem.set16(D + 0xB8, 0x24);
        let s = fx.mem.i16(t + 0x50);
        billboard::arc_strips(fx, D, t, 0x21, history(s));
        fx.mem.set16(D + 0xB8, 0);
    }
}

/// 0x1E02F40(D, step, 0): b252's spawn on this record's globals (300
/// tasks, colour D+0x9C84, +0x50 / +0x52 from D+0x9D20, +0x5A from
/// D+0x9D12); `regs` the caller's s0..s4, `ra` the return address.
fn spawn(fx: &mut Fx, f: u32, regs: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, regs[4]), (0x30, regs[3]), (0x20, regs[2]), (0x10, regs[1]), (0, regs[0])]);
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
            // 0x107390's `sq ra` upper half (0) (b232's).
            fx.mem.write_u64(sp - 8, 0);
            for c in 0..3 {
                let w = fx.mem.read_u32(POS + 4 * c);
                fx.mem.write_u32(t + 0x10 + 4 * c, w);
            }
            let i = fx.mem.i16(SLOT);
            fx.mem.set16(t + 0x50, i);
            let i = fx.mem.i16(SLOT);
            fx.mem.set16(t + 0x52, i);
            let a = fx.mem.i16(ANGLE);
            fx.mem.set16(t + 0x5A, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E00FD0(D, 0) (frame 0x40: ra, the entry's s2 = param, s1 = D, s0 =
/// 0; s2 = D, s0 / s1 the mote loops' counters): the update counter
/// D+0x9D28 and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let dd = u64::from(D);
    fx.saved(sp, &[(0x30, 0x01E0_2A60), (0x20, param as i64 as u64), (0x10, dd), (0, 0)]);
    let (mut s0, mut s1) = (0u64, dd);
    // The spawns' saves: s0 / s1, s2 = D, s3 / s4 the runner's (0).
    let go = |fx: &mut Fx, f: u32, s0: u64, s1: u64, jal: u32| {
        spawn(fx, f, [s0, s1, dd, 0, 0], jal + 8);
    };
    let node = |fx: &mut Fx, k: u32, out: u32, s0: u64, s1: u64, jal: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, s, out, s0, Some(s1), jal + 8);
    };
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 1 {
        go(fx, LIGHT, s0, s1, 0x01E0_1020);
    }
    if fx.mem.i16(COUNTER) == 0x3B {
        for (slot, jal) in [(0, 0x01E0_1058u32), (1, 0x01E0_107C), (2, 0x01E0_10A0)] {
            fx.mem.set16(SLOT, slot);
            go(fx, ARC_N, s0, s1, jal);
        }
    }
    // The three orbs: (update, last stream update, node, slot, the call
    // sites, whether the loop counts in s0 (else s1)).
    let orbs = [
        (0x6C, 0xC7, 1u32, 0, [0x01E0_10D8u32, 0x01E0_1118, 0x01E0_112C, 0x01E0_1140, 0x01E0_11A8], 0x01E0_121C, 0x01E0_1258, true),
        (0x82, 0xDD, 2, 1, [0x01E0_1290, 0x01E0_12D4, 0x01E0_12E8, 0x01E0_12FC, 0x01E0_1364], 0, 0x01E0_13F8, false),
        (0x8C, 0xE7, 0, 2, [0x01E0_1430, 0x01E0_1474, 0x01E0_1488, 0x01E0_149C, 0x01E0_1504], 0, 0x01E0_1598, false),
    ];
    for (at, until, k, slot, jals, stream_node, stream, counts_s0) in orbs {
        if fx.mem.i16(COUNTER) == at {
            node(fx, k, POS, s0, s1, jals[0]);
            fx.mem.set16(SLOT, slot);
            let s = fx.mem.i16(SLOT);
            node(fx, k, point(s), s0, s1, jals[1]);
            go(fx, ORB, s0, s1, jals[2]);
            go(fx, ARC_O, s0, s1, jals[3]);
            let s = fx.mem.i16(SLOT);
            copy4(fx, POS, point(s));
            for i in 0..0x14u64 {
                let a = 0x12 * i;
                fx.mem.set16(ANGLE, a as i32);
                if counts_s0 {
                    (s0, s1) = (i, a);
                } else {
                    (s0, s1) = (a, i);
                }
                go(fx, MOTE, s0, s1, jals[4]);
            }
            if counts_s0 {
                (s0, s1) = (0x14, 0x168);
            } else {
                (s0, s1) = (0x168, 0x14);
            }
        }
        let c = fx.mem.i16(COUNTER);
        if (at..until).contains(&c) {
            if stream_node != 0 {
                node(fx, k, POS, s0, s1, stream_node);
            }
            copy4(fx, POS, point(slot));
            go(fx, MOTES_B, s0, s1, stream);
        }
    }
    fx.leave(0x40);
}

/// +0x63 (`alpha`) in by `up` under `top` while `rising`, else out by
/// `down` (the task ends below `floor`).
fn fade(fx: &mut Fx, at: u32, t: u32, rising: bool, up: u8, top: u8, floor: u8, down: u8) {
    let a = fx.mem.read_u8(at);
    if rising {
        if a < top {
            fx.mem.write_u8(at, a.wrapping_add(up));
        }
    } else if a >= floor {
        fx.mem.write_u8(at, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E015C0 / 0x1E01790 (D, task) (b757's 0x1E01E00 on other values): a
/// ring of motes (size 30..36 growing by 1, cell 0..2, blue / white,
/// radius 5..14 growing by 2..6 / 35..84, a random angle, row 0..34, life
/// 9..16); in by 0x40 under 0xC0 during the life, out by 0x13.
fn motes(fx: &mut Fx, t: u32, at: u32) {
    let k = at - MOTES_B;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_15E4 + k, 7);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0x1E));
        let v = rmod(fx, t, 0x01E0_1610 + k, 3);
        fx.mem.set16(t + 0x56, v);
        if at == MOTES_B {
            l2::rgb(fx, t + 0x60, 0x20, 0x20, 0xFF);
        } else {
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        }
        let v = rmod(fx, t, 0x01E0_163C + k, 0xA);
        fx.mem.set16(t + 0x50, v + 5);
        let v = rmod(fx, t, 0x01E0_165C + k, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = if at == MOTES_B { rmod(fx, t, 0x01E0_1678, 5) + 2 } else { rmod(fx, t, 0x01E0_1848, 0x32) + 0x23 };
        fx.mem.set16(t + 0xE, v);
        let v = rmod(fx, t, 0x01E0_1698 + k, 0x23);
        fx.mem.set16(t + 8, v);
        let v = fx.rand_from(t, 0x01E0_16B4 + k);
        fx.mem.set16(t + 6, v.wrapping_rem(8) + 9);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t + 0x63, t, rising, 0x40, 0xC0, 0x15, 0x13);
    if fx.mem.i16(t + 0x54) != 0 {
        let v = ee::add(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, v);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// y += `by` sin +0x5A, x += `by` cos +0x5A.
fn drift(fx: &mut Fx, t: u32, by: f32) {
    use ee::{add, mul};
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let y = add(fx.mem.f(t + 0x14), mul(by, s));
    fx.mem.setf(t + 0x14, y);
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let x = add(fx.mem.f(t + 0x10), mul(by, c));
    fx.mem.setf(t + 0x10, x);
}

/// 0x1E01960 (D, task) (b377's 0x1E02150 in blue, rising): a mote (size 40,
/// cell 2..4, +0x52 a random 0..3 orientation) thrown 2 out at its spawn
/// angle, then drifting 0.5 a update; in by 0x60 under 0xC0 for 5
/// updates, out by 0x13; turned by the camera's angles.
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x28);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_1998);
        let r = fx.div(v >> 4, 3).1;
        fx.mem.set16(t + 0x56, r + 2);
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        drift(fx, t, 2.0);
        let v = fx.rand_from(t, 0x01E0_1A28);
        fx.mem.set16(t + 0x52, (v >> 4).wrapping_rem(4));
    }
    let rising = fx.mem.i16(t + 0x54) < 5;
    fade(fx, t + 0x63, t, rising, 0x60, 0xC0, 0x15, 0x13);
    if fx.mem.i16(t + 0x54) != 0 {
        drift(fx, t, 0.5);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1B00);
    age(fx, t);
}

/// 0x1E01B30 (D, task) (b432's 0x1E024D0 in violet): an arc burst; slot
/// 0: white over red, outer radius 220, life 14, out by 9; slot 1: violet
/// over grey, random angles, life 8, widening by 25 (the outer 0.6 of it),
/// out by 15; +0x67 in by 0x60 under 0xC0 during the life; spans 10 / 10.
fn burst(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x4C, 0x435C_0000);
            fx.mem.set16(t + 6, 0xE);
            l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
            l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.set16(t + 6, 8);
            l2::rgb(fx, t + 0x64, 0x40, 0x20, 0x80);
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
            for (o, ra) in [(0x20u32, 0x01E0_1BD8u32), (0x24, 0x01E0_1BF4), (0x28, 0x01E0_1C10)] {
                let v = rmod(fx, t, ra, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
        }
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x67);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a.wrapping_add(0x60));
        }
    } else {
        if fx.mem.i16(t + 0x50) == 0 {
            fade(fx, t + 0x67, t, false, 0, 0, 0xB, 9);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fade(fx, t + 0x67, t, false, 0, 0, 0x11, 0xF);
        }
    }
    if fx.mem.i16(t + 0x50) == 1 {
        let r = ee::add(fx.mem.f(t + 0x48), 25.0);
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3F19_999A), r));
    }
    age(fx, t);
}

/// 0x1E01D30 (D, task) (frame 0x60 at `sp`: s0 = sp + 0x54, s1 = sp +
/// 0x58, s2 = the task, s3 = D): the orb at its slot's point P (copied to
/// sp + 0x50): on its first update a white rim, the 70-point history
/// filled with P, +6 = 20 (growing by 1 to 50 from age 1), size 17; both
/// alphas in by 0x2A under 0x80 for 0x5A updates, then the size down by
/// 1.5 (the task ends below 0); P pushed onto the history on ages < 50 and
/// > 0x50; white / pale grey on odd / even ages below 0x50; for 50
/// updates P drifts 3.5 along z (- when the attacker's yaw is non-zero)
/// and sinks 0.3, after that it is the spawn point (slot 1) of an arc
/// burst and a white mote ring on its odd ages, on age 50 of the frame
/// flash.
fn orb(fx: &mut Fx, t: u32, sp: u32) {
    use ee::{add, sub};
    let slot = fx.mem.i16(t + 0x50);
    let p = point(slot);
    for o in [0u32, 4, 8] {
        let w = fx.mem.read_u32(p + o);
        fx.mem.write_u32(sp + 0x50 + o, w);
    }
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        let s = fx.mem.i16(t + 0x50);
        let h = history(s);
        for k in 0..0x46u32 {
            for o in [0u32, 4, 8] {
                let w = fx.mem.read_u32(sp + 0x50 + o);
                fx.mem.write_u32(h + 0x10 * k + o, w);
            }
        }
        fx.mem.set16(t + 6, 0x14);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.write_u32(t + 0x48, 0x4188_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x5A {
        for at in [t + 0x63, t + 0x67] {
            let a = fx.mem.read_u8(at);
            if a < 0x80 {
                fx.mem.write_u8(at, a + 0x2A);
            }
        }
    } else {
        let s = fx.mem.f(t + 0x48);
        if s < 0.0 {
            fx.mem.set16(t + 0x58, 0);
        } else {
            fx.mem.setf(t + 0x48, sub(s, 1.5));
        }
    }
    let a = fx.mem.i16(t + 0x54);
    if a < 0x32 || a >= 0x51 {
        let s = fx.mem.i16(t + 0x50);
        spawn::history(fx, history(s) + 0x450, 0x46, sp + 0x50);
    }
    let a = fx.mem.i16(t + 0x54);
    if a < 0x50 {
        if a % 2 == 1 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        } else {
            l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let n = fx.mem.i16(t + 6);
        if n < 0x32 {
            fx.mem.set16(t + 6, n + 1);
        }
    }
    if fx.mem.i16(t + 0x54) < 0x32 {
        let s = fx.mem.i16(t + 0x50);
        let z = fx.mem.f(point(s) + 8);
        let z = if fx.mem.i16(FACING) != 0 { sub(z, 3.5) } else { add(z, 3.5) };
        fx.mem.setf(point(s) + 8, z);
        let s = fx.mem.i16(t + 0x50);
        let y = sub(fx.mem.f(point(s) + 4), f32::from_bits(0x3E99_999A));
        fx.mem.setf(point(s) + 4, y);
    } else {
        let s = fx.mem.i16(t + 0x50);
        copy4(fx, POS, point(s));
        fx.mem.set16(SLOT, 1);
        // The spawns' saves: s0 = sp + 0x54, s1 = sp + 0x58, s2 = the
        // task, s3 = D, s4 the runner's (0).
        let regs = [u64::from(sp + 0x54), u64::from(sp + 0x58), u64::from(t), u64::from(D), 0];
        if fx.mem.i16(t + 0x54) % 2 != 0 {
            spawn(fx, BURST, regs, 0x01E0_20E4);
        }
        if fx.mem.i16(t + 0x54) % 2 == 1 {
            spawn(fx, MOTES_W, regs, 0x01E0_211C);
        }
        if fx.mem.i16(t + 0x54) == 0x32 {
            spawn(fx, FLASH, regs, 0x01E0_2140);
        }
    }
    age(fx, t);
}

/// +0x64 white / pale grey on odd / even ages, then the camera's angles
/// (the first + 0x4000) from `ra`.
fn flicker_face(fx: &mut Fx, t: u32, ra: u32) {
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
    } else {
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), ra);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
}

/// The arcs' first update: outer radius 40, `rim` over blue, spans 10 / 10.
fn arc_first(fx: &mut Fx, t: u32, rim: (i32, i32, i32)) {
    fx.mem.write_u32(t + 0x4C, 0x4220_0000);
    l2::rgb(fx, t + 0x64, rim.0, rim.1, rim.2);
    l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
    fx.mem.set16(t + 0x52, 0);
    fx.mem.set16(t + 0x5A, 0xA);
    fx.mem.set16(t + 0x5C, 0xA);
    fx.mem.set16(t + 0x58, 2);
    fx.mem.write_u8(t + 0x67, 0);
}

/// 0x1E02170 (D, task) (frame 0x30: s0 = the task, s1 = D): an arc riding
/// the attacker's node of its slot (0x28 / 0x38 / 0x48); +0x67 in by 0x30
/// under 0xC0 for 100 updates, then out by 15; flickering, facing the
/// camera.
fn arc_n(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        arc_first(fx, t, (0xCF, 0xCF, 0xFF));
    }
    let rising = fx.mem.i16(t + 0x54) < 0x64;
    fade(fx, t + 0x67, t, rising, 0x30, 0xC0, 0x11, 0xF);
    let h = fx.self_handle();
    let k = fx.mem.i16(t + 0x50);
    let s = fx.mem.i16(NODES.wrapping_add((2 * k) as u32));
    node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_2258);
    flicker_face(fx, t, 0x01E0_22B8);
    age(fx, t);
}

/// 0x1E022F0 (D, task) (frame 0x30): [`arc_n`] in white at its slot's
/// point (drawn at the orb's history head), in for 0x55 updates.
fn arc_o(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        arc_first(fx, t, (0xFF, 0xFF, 0xFF));
    }
    let rising = fx.mem.i16(t + 0x54) < 0x55;
    fade(fx, t + 0x67, t, rising, 0x30, 0xC0, 0x11, 0xF);
    let s = fx.mem.i16(t + 0x50);
    copy4(fx, t + 0x10, point(s));
    flicker_face(fx, t, 0x01E0_2440);
    age(fx, t);
}

/// 0x1E02470 (D, task) (b065's 0x1E029C0 on 0x97 updates): the stage
/// light (0x1F7F10) down by 10 to 0x30 for 0x97 updates, back up by 10 to
/// 0x80 once the timer is under 30; 0x80 and the task's end when the
/// timer is at 3.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x97 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
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

/// 0x1E02580 (D, task) (frame 0x90 at `sp`; b590's 0x1E02450 with other
/// fade values): the frame flash: a window sprite (0x6FF8 .. 0x8FF8)
/// sampling the frame (0x207EF0) at ST +0x48 / 512 .. (512 - +0x48) / 512
/// (+0x48 = 2.5), colour (0x80, 0x80, 0x90, alpha +0x63), ALPHA 0x44, key
/// 0; in by 0xC under 0x30 until update 0xFA, then out by 4 (the task ends
/// below 6).
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x90);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, c);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(512.0, s), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(512.0, s), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let rising = fx.mem.i16(COUNTER) < 0xFA;
    fade(fx, t + 0x63, t, rising, 0xC, 0x30, 6, 4);
    age(fx, t);
}
