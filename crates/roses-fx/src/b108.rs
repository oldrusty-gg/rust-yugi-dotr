//! BMODULE b108 (card 369), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E028F0, data area 0x1ED0000, 0x6E60 bytes) [F]
//!: b770's layout family (150 tasks of 0x70
//! bytes at D+0x2A0, alive +0x58, step +0x44 through `jalr`, spawned by
//! 0x1E02FF0 = b770's spawn on the colour D+0x6584 and the kind D+0x6ADC).
//! The emitter 0x1E016D0 counts D+0x6AE4: on 0xA a rising ring at the
//! attacker's node 0x67; on 0x7D a flat petal band at node 1; on odd
//! updates 0x7D..0xD2 a spoke fan at node 0x67; on 0xD2..0x11D every
//! fourth (at most nine) a point within 75 of the target's node 1 that
//! drifts back to it by 1/23 for 22 updates, with a fire trail (a 50-point
//! history drawn as b116's tube 0x1E03430) and a band at its ends; on
//! 0xD3..0x11D motes at the target; on 0x104 eight spoke fans; on 0x10D a
//! wide ring. Library: `quads4::ring_facing_flat_z` (0x1E01180, uv_rect
//! 0x1E00100), `quads4::ring_band_f` (0x1E009A0), `spawn::history`
//! (0x1E00080); copied: b770's spokes (0x1E00DF0 = its 0x1E00D70), b094's
//! band (0x1E03130 without the angle limit, angles negated) and b116's
//! tube (b258's `halves` compile at the 0x70-task offsets, size the float
//! +0x48). The eight steps (0x1E01B60..0x1E027D0) sit inside the emitter's
//! scan range (no `jal` names them). Returns 1 on update 0xCD, 4 on 0x15E,
//! 2 on 0x136, 6 on 0xD8..0x130, 8 once the 365-update timer D+0x6580 has
//! run out.
use crate::b076::node_position_saving;
use crate::blib::{quads4, spawn as bspawn, spline, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0x96;
const POOL: u32 = D + 0x2A0;

const MOTE: u32 = 0x01E0_1B60;
const SPOKE: u32 = 0x01E0_1CF0;
const BAND2: u32 = 0x01E0_1EB0;
const TRAIL: u32 = 0x01E0_2010;
const FLAT: u32 = 0x01E0_21B0;
const RISE: u32 = 0x01E0_2380;
const FAN: u32 = 0x01E0_25B0;
const WIDE: u32 = 0x01E0_27D0;
/// The return address of the entry's `jalr` into a step (0x1E02B30 + 8).
const TASK_RA: u64 = 0x01E0_2B38;

const POS: u32 = D + 0x250;
const TEX5: u32 = D + 0x6560;
/// Trail k's 50 points at TRAILS + 0x320 k.
const TRAILS: u32 = D + 0x4470;
/// Point k's current place (+0x10 k) and its start (HOME).
const POINTS: u32 = D + 0x63B0;
const HOME: u32 = D + 0x6450;
const TIMER: u32 = D + 0x6580;
const COLOUR: u32 = D + 0x6584;
const OTW: u32 = D + 0x658C;
const NODES: u32 = D + 0x696C;
const TARGET_NODE: u32 = D + 0x697A;
const CELLS: u32 = D + 0x698C;
const NPOINTS: u32 = D + 0x6AC0;
const AGES: u32 = D + 0x6AC2;
const KIND: u32 = D + 0x6ADC;
const FACING: u32 = D + 0x6AE0;
const COUNTER: u32 = D + 0x6AE4;
const BYTES: u32 = D + 0x6AE6;
const SINES: u32 = D + 0x6CE8;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The entry 0x1E028F0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(OTW, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x6E60);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x6AE2, param);
    fx.mem.set32(TIMER, 0x16D);
    for (k, (n, clear)) in [(0x67, 1), (1, 0), (0xD, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0xA..0x10u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x288 + 4 * (k - 0xA), t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX5 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.fix_residue(y, 0x01E0_2A4C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = the loop count.
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_2A74, 0x3C);
        fx.mem.set8(BYTES + k, v - 0x1E);
    }
    for k in 0..0x1Eu32 {
        let v = rmod(fx, k, 0x01E0_2ABC, 0xF);
        fx.mem.set16(CELLS + 2 * k, v);
    }
    0
}

fn kind_of(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x50)
}

fn trail_of(fx: &mut Fx, t: u32) -> u32 {
    let k = kind_of(fx, t);
    (TRAILS as i32 + 0x320 * k) as u32
}

/// The atlas rect `id` frame +0x56 through the entry's local `r` into
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let s = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, s);
    copy4(fx, D + 0x94, r);
}

fn self_node(fx: &mut Fx, node: u32, out: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(node);
    fx.node_position(h, n, out);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = POOL + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        let size = if matches!(f, TRAIL | FLAT | RISE) { 0x30 } else { 0x20 };
        let s = fx.enter(size);
        if size == 0x30 {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(D)), (0, 0)]);
        } else {
            fx.saved(s, &[(0x10, TASK_RA), (0, 0)]);
        }
        match f {
            MOTE => mote(fx, t),
            SPOKE => spoke(fx, t),
            BAND2 => band2(fx, t),
            TRAIL => trail(fx, t),
            FLAT => flat(fx, t),
            RISE => rise(fx, t),
            FAN => fan(fx, t),
            WIDE => wide(fx, t),
            _ => panic!("b108: unknown task step {f:#x}"),
        }
        fx.leave(size);
    }
    // The entry's s0: the live tasks counted (saved by the draws' node
    // lookups; the frame's ra is a missing texture's TEX1 later).
    let mut alive = 0u64;
    for k in 0..TASKS {
        let t = POOL + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        alive += 1;
        let node_at = |fx: &mut Fx, node: u32, ra: u32| {
            let h = fx.self_handle();
            let n = fx.mem.i16(node);
            node_position_saving(fx, h, n, sp + 0x50, alive, Some(u64::from(D)), ra);
        };
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == MOTE {
            cell(fx, t, sp + 0x60, 0x14);
            let w = fx.mem.read_u32(TEX5 + 4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 6);
            let r = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, r);
            copy3(fx, D + 0x20, t + 0x10);
            quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == SPOKE {
            spokes(fx, D, t, 0x21, t + 0x10);
        }
        if f(fx) == BAND2 {
            let a = trail_of(fx, t);
            quads4::ring_band_f(fx, D, t, 0x21, a);
            let a = trail_of(fx, t) + 0x310;
            quads4::ring_band_f(fx, D, t, 0x21, a);
        }
        if f(fx) == TRAIL {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            for (o, v) in [(0x94u32, 1), (0x96, 0x10), (0x98, 0x1F), (0x9A, 1)] {
                fx.mem.set16(D + o, v);
            }
            cell(fx, t, sp + 0x70, 0x97);
            fx.mem.set16(D + 0x98, 1);
            for o in [0x20u32, 0x24, 0x28] {
                fx.mem.write_u32(D + o, 0);
            }
            fx.mem.set16(D + 0xBE, 1);
            let a = trail_of(fx, t);
            tube(fx, t, a, 0);
        }
        if f(fx) == FLAT {
            let w = fx.mem.read_u32(TEX5);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x80, 0x14);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            node_at(fx, NODES + 2, 0x01E0_2DDC);
            band(fx, t, 0x20, t + 0x10);
        }
        if f(fx) == RISE {
            match kind_of(fx, t) {
                0 => node_at(fx, NODES, 0x01E0_2E28),
                1 => node_at(fx, NODES + 2, 0x01E0_2E50),
                _ => {}
            }
            quads4::ring_band_f(fx, D, t, 0x21, sp + 0x50);
        }
        if f(fx) == FAN {
            if kind_of(fx, t) == 0 {
                node_at(fx, NODES, 0x01E0_2EA0);
            }
            if kind_of(fx, t) == 1 {
                node_at(fx, NODES + 2, 0x01E0_2EC8);
                let y = ee::sub(fx.mem.f(sp + 0x54), 1.5);
                fx.mem.setf(sp + 0x54, y);
            }
            spokes(fx, D, t, 0x21, sp + 0x50);
        }
        if f(fx) == WIDE {
            quads4::ring_band_f(fx, D, t, 0x21, t + 0x10);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0xCD => 1,
        0x15E => 4,
        0x136 => 2,
        0xD8..=0x130 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E02FF0(D, step, 0) = b770's spawn: the first free task cleared and
/// set off at D+0x250 (i16 and xyz) with the colour D+0x6584 (twice) and
/// the kind D+0x6ADC.
fn spawn(fx: &mut Fx, step: u32) -> i32 {
    let f = fx.enter(0x60);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x58 + 0x70 * k) == 0) {
        Some(k) => {
            let t = POOL + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for o in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            copy3(fx, t + 0x10, POS);
            let s = fx.mem.i16(KIND);
            fx.mem.set16(t + 0x50, s);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    let _ = f;
    fx.leave(0x60);
    ret
}

fn target_node(fx: &mut Fx, out: u32) {
    let h = fx.target_handle();
    let n = fx.mem.i16(TARGET_NODE);
    fx.node_position(h, n, out);
}

/// 0x1E016D0(D, 0): the update counter and the spawns.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x40, 0x01E0_2B08), (0x30, 0), (0x20, 0), (0x10, u64::from(D)), (0, 0)]);
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    if counter(fx) == 0xA {
        fx.mem.set16(KIND, 0);
        self_node(fx, NODES, POS);
        spawn(fx, RISE);
    }
    let c = counter(fx);
    if (0x7D..0xD3).contains(&c) && c % 2 == 1 {
        fx.mem.set16(KIND, 0);
        self_node(fx, NODES, POS);
        spawn(fx, FAN);
    }
    if counter(fx) == 0x7D {
        fx.mem.set16(KIND, 0);
        self_node(fx, NODES + 2, POS);
        spawn(fx, FLAT);
    }
    if counter(fx) == 0x10D {
        target_node(fx, POS);
        fx.mem.set16(KIND, 0);
        spawn(fx, WIDE);
    }
    let c = counter(fx);
    if (0xD2..0x11E).contains(&c) && c % 4 == 1 {
        let np = fx.mem.i16(NPOINTS);
        if np < 9 {
            fx.mem.set16(KIND, np);
            target_node(fx, POS);
            for (axis, ra) in [(4u32, 0x01E0_1884u32), (0, 0x01E0_18D8), (8, 0x01E0_192C)] {
                let v = rmod(fx, 0, ra, 0x96) - 0x4B;
                let p = ee::add(fx.mem.f(POS + axis), ee::from_i32(v));
                fx.mem.setf(POS + axis, p);
                let k = fx.mem.i16(KIND) as u32;
                fx.mem.setf(HOME + 0x10 * k + axis, p);
                let k = fx.mem.i16(KIND) as u32;
                fx.mem.setf(POINTS + 0x10 * k + axis, p);
            }
            spawn(fx, TRAIL);
            spawn(fx, BAND2);
            let n = fx.mem.i16(NPOINTS) + 1;
            fx.mem.set16(NPOINTS, n);
        }
    }
    if counter(fx) >= 0xD3 {
        let mut k = 0u32;
        while (k as i32) < fx.mem.i16(NPOINTS) {
            target_node(fx, POS);
            if fx.mem.i16(AGES + 2 * k) < 0x16 {
                for axis in [0u32, 4, 8] {
                    let d = ee::div(ee::sub(fx.mem.f(POS + axis), fx.mem.f(HOME + 0x10 * k + axis)), 23.0);
                    let v = ee::add(fx.mem.f(POINTS + 0x10 * k + axis), d);
                    fx.mem.setf(POINTS + 0x10 * k + axis, v);
                }
            }
            let a = fx.mem.i16(AGES + 2 * k) + 1;
            fx.mem.set16(AGES + 2 * k, a);
            k += 1;
        }
    }
    if counter(fx) == 0x104 {
        target_node(fx, POS);
        for _ in 0..8 {
            spawn(fx, SPOKE);
        }
    }
    let c = counter(fx);
    if (0xD3..0x11E).contains(&c) {
        target_node(fx, POS);
        spawn(fx, MOTE);
    }
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x54)
}

fn grow(fx: &mut Fx, t: u32) {
    let n = age(fx, t) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x58) == 1
}

fn alpha(fx: &mut Fx, t: u32) -> u8 {
    fx.mem.read_u8(t + 0x63)
}

/// Alpha down by `by` while at least `floor`, else the task ends.
fn fade(fx: &mut Fx, t: u32, floor: u8, by: u8) {
    let a = alpha(fx, t);
    if a >= floor {
        fx.mem.write_u8(t + 0x63, a - by);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn addf(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// The outer colour +0x64 by the age's parity: (0xF0, 8, 0x80) odd, (0x80,
/// 8, 0x40) even.
fn flicker(fx: &mut Fx, t: u32) {
    if age(fx, t) % 2 == 1 {
        l2::rgb(fx, t + 0x64, 0xF0, 8, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0x80, 8, 0x40);
    }
}

/// 0x205580 into the angles +0x20 (its frames left below the step's: s0
/// the task, s1 = D, return `ra`; a later missing texture's TEX1 reads
/// that ra), then +0x20 up by 0x4000.
fn spin(fx: &mut Fx, t: u32, ra: u32) {
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), ra);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
}

/// 0x1E01B60: a grey mote (size 5.0 growing by 1, cell rand & 3, row rand
/// & 7, +0xC the kind, +0x50 rand % 150 + 320 falling by +0xE = rand % 30
/// + 15 (the task ends at 0), +8 rand % 35, life rand % 5 + 3 at 0x80).
fn mote(fx: &mut Fx, t: u32) {
    let at = MOTE;
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        let v = fx.rand_from(t, at + 0x28);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        let v = fx.rand_from(t, at + 0x5C);
        fx.mem.set16(t + 0xA, v % 8);
        let k = kind_of(fx, t);
        fx.mem.set16(t + 0xC, k);
        let v = rmod(fx, t, at + 0x80, 0x96);
        fx.mem.set16(t + 0x50, v + 0x140);
        let v = rmod(fx, t, at + 0xA0, 0x1E);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = rmod(fx, t, at + 0xC0, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, at + 0xDC, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if age(fx, t) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    }
    if age(fx, t) != 0 {
        addf(fx, t + 0x48, 1.0);
    }
    if age(fx, t) != 0 {
        let r = kind_of(fx, t);
        if r > 0 {
            let v = r - fx.mem.i16(t + 0xE);
            fx.mem.set16(t + 0x50, v);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    grow(fx, t);
}

/// 0x1E01CF0: a spoke fan (radii 30 / 2.5, angles +0x20.. rand % 0xFFFF,
/// (0x80, 0x80, 0x80) / (0xF0, 8, 8), +0x5A rand % 360): alpha in by 0x15
/// (below 0x80) for 0x23 updates, then out by 9; kind 0 grows by 12.3.
fn spoke(fx: &mut Fx, t: u32) {
    let at = SPOKE;
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x41F0_0000);
        fx.mem.write_u32(t + 0x4C, 0x4020_0000);
        for (o, ra) in [(0x20u32, 0x30u32), (0x24, 0x4C), (0x28, 0x68)] {
            let v = rmod(fx, t, at + ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xF0, 8, 8);
        let v = rmod(fx, t, at + 0xA8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if age(fx, t) < 0x23 {
        let a = alpha(fx, t);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x15));
        }
    } else {
        fade(fx, t, 0xB, 9);
    }
    if kind_of(fx, t) == 0 {
        addf(fx, t + 0x48, f32::from_bits(0x4144_CCCD));
    }
    flicker(fx, t);
    grow(fx, t);
}

/// 0x1E01EB0: the band at a trail's ends (radius 20 growing by 3.3 from
/// update 0x1A, white / (0x80, 0x40, 0x10)): alpha up by 0x10 for 0x32
/// updates, then out by 5; facing the camera, turning 0x4000 a update.
fn band2(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x40, 0x10);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if age(fx, t) < 0x32 {
        let a = alpha(fx, t);
        fx.mem.write_u8(t + 0x63, a.wrapping_add(0x10));
    } else {
        fade(fx, t, 7, 5);
    }
    if age(fx, t) >= 0x1A {
        addf(fx, t + 0x48, f32::from_bits(0x4053_3333));
    }
    if kind_of(fx, t) == 0 {
        flicker(fx, t);
    }
    spin(fx, t, 0x01E0_1FE0);
    grow(fx, t);
}

/// 0x1E02010: trail k's fire (+6 points from 8 up by 2 to 0x30, the 49
/// points from point k's start, size 10; alpha 0xC0 for 0x50 updates, then
/// out by 5; (0xF0, 0x40, 0x40) / (0xF0, 0xF0, 0x40) by the age's parity);
/// the history pushed with point k.
fn trail(fx: &mut Fx, t: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x63, 0);
        let k = kind_of(fx, t);
        let from = (HOME as i32 + 0x10 * k) as u32;
        let to = trail_of(fx, t);
        for i in 0..0x31u32 {
            copy3(fx, to + 0x10 * i, from);
        }
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 6) < 0x30 {
        let v = fx.mem.i16(t + 6) + 2;
        fx.mem.set16(t + 6, v);
    }
    if age(fx, t) < 0x50 {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade(fx, t, 7, 5);
    }
    if age(fx, t) % 2 == 0 {
        l2::rgb(fx, t + 0x60, 0xF0, 0x40, 0x40);
    } else {
        l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0x40);
    }
    let k = kind_of(fx, t);
    let p = (POINTS as i32 + 0x10 * k) as u32;
    let a = trail_of(fx, t) + 0x310;
    bspawn::history(fx, a, 0x32, p);
    grow(fx, t);
}

/// 0x1E021B0: the flat petal band at node 1 (radii 9 / 6, cell rand % 7,
/// +8 rand % 3 + 4): alpha and +0x67 in by 0xC (below 0x80) for 0x14
/// updates, then from update 0xD3 out by 9; grey 0x80 / 0xC0 by the age's
/// parity; +0x20 0x4000, +0x28 up by 0x100.
fn flat(fx: &mut Fx, t: u32) {
    let at = FLAT;
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x4110_0000);
        fx.mem.setf(t + 0x4C, ee::sub(9.0, 3.0));
        let v = rmod(fx, t, at + 0x48, 7);
        fx.mem.set16(t + 0x56, v);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, at + 0x6C, 3);
        fx.mem.set16(t + 8, v + 4);
    }
    if age(fx, t) < 0x14 {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a.wrapping_add(0xC));
            }
        }
    } else if counter(fx) >= 0xD3 {
        fade(fx, t, 0xB, 9);
    }
    let g = if age(fx, t) % 2 == 1 { 0x80 } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    l2::rgb(fx, t + 0x64, g, g, g);
    fx.mem.write_u32(t + 0x20, 0x4000);
    self_node(fx, NODES + 2, t + 0x10);
    let v = fx.mem.read_u32(t + 0x28).wrapping_add(0x100);
    fx.mem.write_u32(t + 0x28, v);
    grow(fx, t);
}

/// 0x1E02380: the rising ring (radius 20, kind 1 300; kind 0 grey /
/// (0xF0, 0x80, 0x10), else white / (0x80, 0x40, 0x10); +0x5A rand %
/// 360): kind 0 alpha up by 0x10 for 12 updates, then from update 0xD3
/// out by 9; kind 1 in by 0x1B (below 0xC0) for 12, then out by 9; facing
/// the camera, turning 0x4000 a update.
fn rise(fx: &mut Fx, t: u32) {
    let at = RISE;
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        if kind_of(fx, t) == 1 {
            fx.mem.write_u32(t + 0x48, 0x4396_0000);
        }
        if kind_of(fx, t) == 0 {
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
            l2::rgb(fx, t + 0x64, 0xF0, 0x80, 0x10);
        } else {
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
            l2::rgb(fx, t + 0x64, 0x80, 0x40, 0x10);
        }
        let v = rmod(fx, t, at + 0xAC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if kind_of(fx, t) == 0 {
        if age(fx, t) < 0xC {
            let a = alpha(fx, t);
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x10));
        } else if counter(fx) >= 0xD3 {
            fade(fx, t, 0xB, 9);
        }
    }
    if kind_of(fx, t) == 1 {
        if age(fx, t) < 0xC {
            let a = alpha(fx, t);
            if a < 0xC0 {
                fx.mem.write_u8(t + 0x63, a.wrapping_add(0x1B));
            }
        } else {
            fade(fx, t, 0xB, 9);
        }
    }
    if kind_of(fx, t) == 0 {
        flicker(fx, t);
    }
    spin(fx, t, 0x01E0_2574);
    grow(fx, t);
}

/// 0x1E025B0: a spoke fan at node 0x67 (radii 10 / 2.5, kind 1 90 / 90;
/// +0x28 rand % 0xFFFF up by 0x200, grey / (0xF0, 8, 8), +0x5A rand %
/// 360): kind 0 in by 0x15 (below 0x80) for 12 updates then out by 9,
/// growing by 4; kind 1 up by 0x10 for 12, growing by 10.
fn fan(fx: &mut Fx, t: u32) {
    let at = FAN;
    if first(fx, t) {
        if kind_of(fx, t) == 0 {
            fx.mem.write_u32(t + 0x48, 0x4120_0000);
            fx.mem.write_u32(t + 0x4C, 0x4020_0000);
        }
        if kind_of(fx, t) == 1 {
            fx.mem.write_u32(t + 0x48, 0x42B4_0000);
            fx.mem.write_u32(t + 0x4C, 0x42B4_0000);
        }
        let v = rmod(fx, t, at + 0x5C, 0xFFFF);
        fx.mem.set32(t + 0x28, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xF0, 8, 8);
        let v = rmod(fx, t, at + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if kind_of(fx, t) == 0 {
        if age(fx, t) < 0xC {
            let a = alpha(fx, t);
            if a < 0x80 {
                fx.mem.write_u8(t + 0x63, a.wrapping_add(0x15));
            }
        } else {
            fade(fx, t, 0xB, 9);
        }
    }
    if kind_of(fx, t) == 1 && age(fx, t) < 0xC {
        let a = alpha(fx, t);
        fx.mem.write_u8(t + 0x63, a.wrapping_add(0x10));
    }
    if kind_of(fx, t) == 0 {
        addf(fx, t + 0x48, 4.0);
    }
    if kind_of(fx, t) == 1 {
        addf(fx, t + 0x48, 10.0);
    }
    flicker(fx, t);
    fx.mem.write_u32(t + 0x20, 0x4000);
    let v = fx.mem.read_u32(t + 0x28).wrapping_add(0x200);
    fx.mem.write_u32(t + 0x28, v);
    grow(fx, t);
}

/// 0x1E027D0: the wide ring at the target (radius 200 growing by 12,
/// white / (0xF0, 0x40, 0x40), +0x5A rand % 360): kind 0 in by 0x20
/// (below 0xC0) for 0x25 updates, then out by 9; facing the camera.
fn wide(fx: &mut Fx, t: u32) {
    let at = WIDE;
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0x4348_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x64, 0xF0, 0x40, 0x40);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, at + 0x58, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
    }
    if age(fx, t) < 0x25 {
        if kind_of(fx, t) == 0 {
            let a = alpha(fx, t);
            if a < 0xC0 {
                fx.mem.write_u8(t + 0x63, a.wrapping_add(0x20));
            }
        }
    } else {
        fade(fx, t, 0xB, 9);
    }
    addf(fx, t + 0x48, 12.0);
    spin(fx, t, 0x01E0_28BC);
    grow(fx, t);
}

/// 0x1E00DF0 = b770's 0x1E00D70 (51069f8b19a6) `spokes(D, task, mode,
/// pos)`: six gouraud strips 60 degrees apart from +0x5A, each 4 degrees
/// wide between the radii +0x48 (at height -+0x34) and +0x4C (at -+0x3C)
/// in the x / z plane, coloured +0x60 (inner) / +0x64 (outer); through
/// the task's angles +0x20 at `pos` (scale 0.1); blend (mode >> 4), depth
/// mode & 15 (0 the mean w, 1 the far Z at key 0, else key -1).
fn spokes(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1F0);
    copy4(fx, sp + 0xA0, pos);
    let d = sp + 0x160;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    let b = (mode >> 4) as i16 as i64;
    let alpha = if b == 0 { 0x80_0000_0042u64 } else { ((b << 2) as u64) | 0x80_0000_0040 };
    fx.desc_alpha(d, alpha);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x100 + 4 * k, 0x3DCC_CCCD);
        let w = fx.mem.read_u32(sp + 0xA0 + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x110, sp + 0x100, t + 0x20, sp + 0xF0);
    let depth = mode & 0xF;
    for (o, h) in [(0xB4u32, 0x34u32), (0xC4, 0x34), (0xD4, 0x3C), (0xE4, 0x3C)] {
        let y = ee::neg(fx.mem.f(t + h));
        fx.mem.setf(sp + o, y);
    }
    let (r0, r1) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let tab = d0 + 0xD8;
    let mut s2 = 0i32;
    for _ in 0..6 {
        let s4 = s2 as i16 as i32;
        let a = (fx.mem.i16(t + 0x5A) + s4) as i16 as i32;
        let e = (s4 + (fx.mem.i16(t + 0x5A) + 4)) as i16 as i32;
        for (deg, r, x, z) in [(a, r0, 0xB0u32, 0xB8u32), (a, r1, 0xD0, 0xD8), (e, r0, 0xC0, 0xC8), (e, r1, 0xE0, 0xE8)] {
            let c = l2::sin_deg(fx, tab, deg, 1);
            fx.mem.setf(sp + x, ee::mul(r, c));
            let sn = l2::sin_deg(fx, tab, deg, 0);
            fx.mem.setf(sp + z, ee::mul(r, sn));
        }
        if l2::project(fx, sp + 0xB0, SPR, sp + 0x110, d, sp + 0x150) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            match depth {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x150)) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z_desc(fx, d, 1);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        s2 = (s2 + 0x3C) as i16 as i32;
    }
    fx.leave(0x1F0);
}

/// 0x1E03130(D, t, mode, pos) (frame 0x1E0) = b094's band 0x1E03260
/// without its angle limit: 18 textured strip3 quads of a flat band
/// (outer +0x48, inner +0x4C) 20 degrees each at -(+0x5A + 90 + 20 k);
/// cell k from D+0x698C + 2k; turned +0x20 at `pos`, keyed by mean w into
/// D+0x658C.
fn band(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1E0);
    copy4(fx, sp + 0x90, pos);
    let d = sp + 0x150;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 1);
    l2::colour_strip3(fx, d, t + 0x64, 2);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    fx.desc_alpha(d, crate::c061::blend_alpha(mode >> 4));
    copy3(fx, sp + 0xE0, sp + 0x90);
    l2::xform_vp(fx, sp + 0xF0, 0, t + 0x20, sp + 0xE0);
    for o in [0xA4u32, 0xB4, 0xC4, 0xD4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let (f20, f21) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let mut s1 = 0i32;
    for k in 0..0x12u32 {
        let fr = fx.mem.i16(CELLS + 2 * k);
        l2::rect(fx, sp + 0x140, 0x14, fr);
        copy4(fx, D + 0x94, sp + 0x140);
        let order = fx.mem.i16(D + 0xBE);
        l2::uv_strip3(fx, d, D + 0x94, order);
        for (a0, outer, inner) in [(0i32, 0xC0u32, 0xA0u32), (0x14, 0xD0, 0xB0)] {
            let a = (fx.mem.i16(t + 0x5A) + a0 + s1 + 0x5A).wrapping_neg() as i16 as i32;
            let c = l2::sin_deg(fx, SINES, a, 1);
            let s = l2::sin_deg(fx, SINES, a, 0);
            fx.mem.setf(sp + outer, ee::mul(c, f20));
            fx.mem.setf(sp + outer + 8, ee::mul(s, f20));
            fx.mem.setf(sp + inner, ee::mul(c, f21));
            fx.mem.setf(sp + inner + 8, ee::mul(s, f21));
        }
        if l2::project(fx, sp + 0xA0, SPR, sp + 0xF0, d, sp + 0x130) {
            let w = fx.mem.f(sp + 0x130);
            fx.fix_residue(w, 0x01E0_33B8);
            let key = ee::fixunssfsi(w) as i32;
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
        s1 = (s1 + 0x14) as i16 as i32;
    }
    fx.leave(0x1E0);
}

/// The Lagrange curve (44769f26f5eb's basis 0) through `p` at `t` into
/// `out` (xyz), summed as the original's accumulator chain (b258's).
fn curve(fx: &mut Fx, out: u32, p: [u32; 4], t: f32) {
    use ee::{add, mul};
    let w = spline::weights(t, 0).unwrap_or([0.0; 4]);
    for c in 0..3u32 {
        let v: [f32; 4] = std::array::from_fn(|i| fx.mem.f(p[i] + 4 * c));
        let acc = add(mul(v[2], w[2]), add(mul(v[0], w[0]), mul(v[1], w[1])));
        fx.mem.setf(out + 4 * c, add(acc, mul(v[3], w[3])));
    }
}

/// 0x1E03430(D, task, trail, mode) = b116's 0x1E05CE0 (b258's tube in its
/// `halves` compile) at this layout's offsets: colours +0x60 / +0x64, the
/// angles +0x20, the half width the float +0x48 / 10 (not i16 +0x38).
fn tube(fx: &mut Fx, t: u32, a: u32, mode: i32) {
    use ee::{add, div, sub};
    let sp = fx.enter(0x340);
    let ot = OT;
    let mut f25 = fx.mem.f(t + 0x48);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x288;
    f25 = div(f25, 10.0);
    l2::desc_strip3(fx, d);
    fx.mem.setf(sp + 0xDC, f25);
    for (local, c, to) in [(0x318u32, 0x60u32, 0x10u32), (0x320, 0x64, 0x28), (0x328, 0x60, 0x40), (0x330, 0x64, 0x58)] {
        crate::c061::rgbaq_at(fx, sp + local, t + c, d + to);
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    let colour = sp + 0x33C;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(colour + k, v);
    }
    fx.mem.write_u32(sp + 0x110, a);
    let pa = sp + 0x140;
    copy4(fx, sp + 0x270, a);
    copy4(fx, pa, sp + 0x270);
    copy4(fx, pa + 0x10, sp + 0x270);
    copy4(fx, pa + 0x20, a + 0x10);
    copy4(fx, pa + 0x30, a + 0x20);
    copy3(fx, sp + 0x1D0, D + 0x20);
    l2::xform_vp(fx, sp + 0x1F0, 0, t + 0x20, sp + 0x1D0);
    for (to, from) in [(0xE0u32, 0x150u32), (0xE4, 0x160), (0xE8, 0x170), (0xEC, 0x154), (0xF0, 0x144), (0xF4, 0x164), (0xF8, 0x174)] {
        let w = fx.mem.read_u32(sp + from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.mem.set32(sp + 0x100, n - 1);
    let step = ee::mul(8.0, ee::from_i32(n));
    fx.mem.setf(sp + 0xFC, step);
    let fp = 3 * n;
    let (mut seg, pieces, dt) = (0, 2, 0.5f32);
    while seg < n - 1 {
        let p = (a as i32 + 0x10 * seg) as u32;
        let pts = if seg == 0 { [pa, pa + 0x10, pa + 0x20, pa + 0x30] } else { [p - 0x10, p, p + 0x10, p + 0x20] };
        let mut f24 = 0.0f32;
        for piece in 0..pieces {
            if seg == 0 && piece == 0 {
                curve(fx, sp + 0x250, pts, f24);
                copy4(fx, sp + 0x120, sp + 0x250);
            } else {
                copy4(fx, sp + 0x120, sp + 0x180);
            }
            f24 = add(f24, dt);
            if !(f24 < f32::from_bits(0x3F66_6666)) {
                f24 = 1.0;
            }
            let l = if seg == 0 { 0x260 } else { 0x240 };
            curve(fx, sp + l, pts, f24);
            copy4(fx, sp + 0x130, sp + l);
            copy4(fx, sp + 0x180, sp + 0x130);
            l2::colour_strip3(fx, d, colour, 1);
            let al = i32::from(fx.mem.read_u8(t + 0x63));
            let q = fx.div(al, fp).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(colour + 3)) - q;
            fx.mem.write_u8(colour + 3, if v > 0 { v as u8 } else { 0 });
            l2::colour_strip3(fx, d, colour, 2);
            let dw = div(fx.mem.f(sp + 0xDC), fx.mem.f(sp + 0xFC));
            let f26 = f25;
            f25 = sub(f25, dw);
            let pv: [u32; 3] = std::array::from_fn(|c| fx.mem.read_u32(sp + 0x120 + 4 * c as u32));
            let qv: [u32; 3] = std::array::from_fn(|c| fx.mem.read_u32(sp + 0x130 + 4 * c as u32));
            let v = sp + 0x190;
            for s1 in 0..2 {
                for (o, src) in [(0u32, pv), (0x10, pv), (0x20, qv), (0x30, qv)] {
                    for c in 0..3u32 {
                        fx.mem.write_u32(v + o + 4 * c, src[c as usize]);
                    }
                }
                let bump = |fx: &mut Fx, at: u32, by: f32, up: bool| {
                    let x = fx.mem.f(v + at);
                    fx.mem.setf(v + at, if up { add(x, by) } else { sub(x, by) });
                };
                if s1 == 0 {
                    bump(fx, 0x14, f26, true);
                    bump(fx, 0x34, f25, true);
                    bump(fx, 0x4, f26, false);
                    bump(fx, 0x24, f25, false);
                } else {
                    let h2 = div(f26, 2.0);
                    bump(fx, 0x10, f26, true);
                    let h1 = div(f25, 2.0);
                    bump(fx, 0x30, f25, true);
                    bump(fx, 0x18, h2, true);
                    bump(fx, 0x38, h1, true);
                    bump(fx, 0x0, f26, false);
                    bump(fx, 0x20, f25, false);
                    bump(fx, 0x8, h2, false);
                    bump(fx, 0x28, h1, false);
                }
                if mode <= 2 && l2::project(fx, v, SPR, sp + 0x1F0, d, sp + 0x1E0) {
                    match mode {
                        0 => {
                            let key = ee::fixunssfsi(fx.mem.f(sp + 0x1E0)) as i32;
                            fx.submit(d, ot, key);
                        }
                        1 => {
                            l2::far_z_desc(fx, d, 0);
                            fx.submit(d, ot, 0);
                        }
                        _ => fx.submit(d, ot, -1),
                    }
                }
            }
        }
        seg += 1;
    }
    fx.leave(0x340);
}
