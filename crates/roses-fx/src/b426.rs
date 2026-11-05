//! BMODULE b426 (card 3), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03710, data area 0x1ED0000, 0x57E0 bytes) [F]
//!: a pool of 180 tasks of 0x70 bytes at D+0x2A0
//! (alive +0x58, step +0x44 called through `jalr`, the stub 0x1E03700
//! skipped; spawned by 0x1E04630, b770's 0x1E03800) drawn with quads
//! (`billboard::quad` 0x1E01320), bands (`quads4::ring_band_f` 0x1E00920)
//! and rings of quads (`quads4::ring_quads_z` 0x1E00D70), UVs by uv_rect
//! 0x1E00080.
//!
//! Six orbs (updates 83, 120, 154, 183, 216, 246) start at the attacker's
//! node 0x2D (D+0x5190 + 16 n) and drift a 50th of the way to a jittered
//! point at the target's node 1 a update (y / z only: the record computes
//! the z step twice and never the x one) for 25 updates and again from
//! their update 126 to 148; each is a growing grey quad (0x1E01E50) and a
//! pulsing red orb (0x1E02370: a quad and a band, sparks of grey quads
//! (0x1E02EB0) and rings (0x1E03050) while moving, a burst of 10 rings
//! (0x1E02BA0), 10 more (0x1E01FD0) and a wide band (0x1E02880) on its
//! update 149) with 18 yellow quads flying out (0x1E031D0). Updates 60..80
//! a yellow ring (0x1E03530) a update at the node. The ring table D+0x5570
//! [100..179] is rewritten every update from the attacker's nodes 0x2D /
//! 0x2E / 0x31. Returns 1 on update 263, 4 on 425, 2 on 415, 6 on
//! 274..409, 8 once the 420-update timer D+0x52F0 has run out.
//!
//! Steps 0x1E02170, 0x1E026D0, 0x1E029D0 and 0x1E03410 (with its draw, a
//! spark ring 0x1E015E0 aimed by a square root) are drawn by the entry but
//! never spawned [F: the spawn `jal`s name 0x1E02370 / 0x1E01E50 /
//! 0x1E031D0 / 0x1E03530 / 0x1E02BA0 / 0x1E01FD0 / 0x1E02880 / 0x1E02DA0 /
//! 0x1E02EB0 / 0x1E03050]: not ported.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xB4;
const GREY: u32 = 0x01E0_1E50;
const BURST_B: u32 = 0x01E0_1FD0;
const UNUSED_A: u32 = 0x01E0_2170;
const ORB: u32 = 0x01E0_2370;
const UNUSED_B: u32 = 0x01E0_26D0;
const WIDE: u32 = 0x01E0_2880;
const UNUSED_C: u32 = 0x01E0_29D0;
const BURST: u32 = 0x01E0_2BA0;
const BAND: u32 = 0x01E0_2DA0;
const SPARK: u32 = 0x01E0_2EB0;
const RING: u32 = 0x01E0_3050;
const FLY: u32 = 0x01E0_31D0;
const UNUSED_D: u32 = 0x01E0_3410;
const YELLOW: u32 = 0x01E0_3530;
const STUB: u32 = 0x01E0_3700;
/// The return address of the entry's `jalr` into a step (0x1E03B60 + 8).
const TASK_RA: u64 = 0x01E0_3B68;
const POS: u32 = D + 0x250;
/// The orbs' points (16 bytes each) and their steps (+0xA0).
const ORBS: u32 = D + 0x5190;
const TEX: u32 = D + 0x52D0;
const TIMER: u32 = D + 0x52F0;
const COLOUR: u32 = D + 0x52F4;
const NODES: u32 = D + 0x5324;
const TARGET_NODE: u32 = D + 0x5334;
/// The orbs' ages (i16 each).
const AGES: u32 = D + 0x5344;
const KIND: u32 = D + 0x5366;
const ORB_N: u32 = D + 0x5368;
const FACING: u32 = D + 0x536A;
const COUNTER: u32 = D + 0x536E;
const BYTES: u32 = D + 0x5370;
const BYTES_B: u32 = D + 0x5470;
const BYTES_C: u32 = D + 0x5570;
const SINES: u32 = D + 0x5670;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E03710.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x180);
    fx.mem.set16(sp + 0xA0, 0);
    fx.mem.write_u32(D + 0x52FC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x180);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x57E0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x536C, param);
    fx.mem.set32(TIMER, 0x1A4);
    for (k, node) in [0x2D, 0x2E, 0x31, 0x31, 0x30, 0x2D].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..4u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x170);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x174)), 180.0);
    fx.fix_residue(y, 0x01E0_38B8);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_38E0, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_3934);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3984, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(BYTES_B + k, 0);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3A04, 0x1E);
        fx.mem.set8(BYTES_C + k, v - 0xF);
    }
    for k in 0x64..0xB4u32 {
        let v = rmod(fx, D, 0x01E0_3A54, 0x14);
        fx.mem.set8(BYTES_C + k, v - 0xA);
    }
    for k in 0xB4..0x100u32 {
        if fx.mem.i16(FACING) != 0 {
            let v = rmod(fx, D, 0x01E0_3AB0, 0x14);
            fx.mem.set8(BYTES_C + k, -v);
        } else {
            let v = rmod(fx, D, 0x01E0_3AD8, 0x14);
            fx.mem.set8(BYTES_C + k, v);
        }
    }
    0
}

fn orb_point(n: i32) -> u32 {
    (ORBS as i32 + 16 * n) as u32
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == STUB {
            continue;
        }
        // The steps' prologues: the entry's s0 = D, s1 = the slot, s2 =
        // its index.
        let frame = match f {
            ORB => 0x40,
            FLY | YELLOW => 0x30,
            GREY | BURST_B | WIDE | BURST | BAND | SPARK | RING => 0x20,
            _ => panic!("b426: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match frame {
            0x20 => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
            0x30 => fx.saved(s, &[(0x20, TASK_RA), (0x10, u64::from(slot)), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0x30, TASK_RA), (0x20, u64::from(k)), (0x10, u64::from(slot)), (0, u64::from(D))]),
        }
        match f {
            GREY => grey(fx, t),
            BURST_B => ring_b(fx, t, true),
            ORB => orb(fx, t),
            WIDE => wide(fx, t),
            BURST => burst(fx, t),
            BAND => band(fx, t),
            SPARK => spark(fx, t),
            RING => ring_b(fx, t, false),
            FLY => fly(fx, t),
            _ => yellow(fx, t),
        }
        fx.leave(frame);
    }
    // The attacker's nodes 0x2D / 0x2E / 0x31 (the entry's s0 = D, s1 =
    // past the last slot).
    let s1 = Some(u64::from(D + 0x70 * TASKS));
    for (i, (local, ra)) in [(0xC0u32, 0x01E0_3BA4u32), (0xD0, 0x01E0_3BC0), (0xE0, 0x01E0_3BDC)].into_iter().enumerate() {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * i as u32);
        node_position_saving(fx, h, slot, sp + local, u64::from(D), s1, ra);
    }
    let f = |fx: &mut Fx, o: u32| fx.mem.f(sp + o);
    for o in [0xE0u32, 0xE4, 0xE8] {
        let v = ee::div(ee::sub(f(fx, o), f(fx, o - 0x10)), 2.0);
        fx.mem.setf(sp + o, v);
    }
    let v = ee::sub(ee::add(f(fx, 0xD0), f(fx, 0xE0)), f(fx, 0xC0));
    fx.mem.setf(sp + 0xC0, v);
    let v = ee::div(ee::sub(ee::add(f(fx, 0xD4), f(fx, 0xE4)), f(fx, 0xC4)), 2.0);
    fx.mem.setf(sp + 0xC4, v);
    for k in 0x64..0xB4u32 {
        let v = ee::mul(10.0, f(fx, 0xC4));
        fx.fix_residue(v, 0x01E0_3C98);
        fx.mem.set8(BYTES_C + k, ee::fixsfsi(v));
    }
    let v = ee::sub(ee::add(f(fx, 0xD8), f(fx, 0xE8)), f(fx, 0xC8));
    fx.mem.setf(sp + 0xC8, v);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let n = fx.mem.i16(sp + 0xA0) + 1;
        fx.mem.set16(sp + 0xA0, n);
        let g = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        let shown = |fx: &mut Fx| {
            let a = fx.mem.i16(t + 0x54);
            a < 0x19 || a >= 0x7E
        };
        if g(fx) == GREY {
            tex(fx, TEX);
            cell(fx, t, sp + 0xF0, 0x14);
            let r = fx.mem.i16(t + 0x56);
            for (o, v) in [(0x94u32, r << 6), (0x96, 0), (0x98, 0x3F), (0x9A, 0x3F), (0xBE, 5)] {
                fx.mem.set16(D + o, v);
            }
            let kind = fx.mem.i16(t + 0x50);
            copy4(fx, sp + 0xB0, orb_point(kind));
            if shown(fx) {
                billboard::quad(fx, uv::uv_rect, D, t, 0x21, sp + 0xB0);
            }
        }
        if g(fx) == BURST_B {
            cell(fx, t, sp + 0x100, 0x14);
            tex(fx, TEX + 4);
            rings(fx, t, 0, BYTES_B);
        }
        debug_assert_ne!(g(fx), UNUSED_A);
        if g(fx) == ORB {
            let kind = fx.mem.i16(t + 0x50);
            copy4(fx, sp + 0xB0, orb_point(kind));
            if shown(fx) {
                billboard::quad(fx, uv::uv_rect, D, t, 0x21, sp + 0xB0);
            }
            if shown(fx) {
                quads4::ring_band_f(fx, D, t, 0x21, sp + 0xB0);
            }
        }
        debug_assert_ne!(g(fx), UNUSED_B);
        if g(fx) == WIDE {
            let i = fx.mem.i16(t + 0x52);
            copy4(fx, sp + 0xB0, orb_point(i));
            quads4::ring_band_f(fx, D, t, 0x20, sp + 0xB0);
        }
        debug_assert_ne!(g(fx), UNUSED_C);
        if g(fx) == BURST {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0x120, 0x97);
            rings(fx, t, 1, BYTES_B);
        }
        if g(fx) == BAND {
            copy4(fx, sp + 0xB0, t + 0x10);
            quads4::ring_band_f(fx, D, t, 0x20, sp + 0xB0);
        }
        for (step, local) in [(SPARK, 0x130u32), (FLY, 0x150)] {
            if g(fx) == step {
                tex(fx, TEX + 8);
                cell(fx, t, sp + local, 0x14);
                fx.mem.set16(D + 0xBE, 5);
                for k in 0..3 {
                    let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                    fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
                }
                billboard::quad(fx, uv::uv_rect, D, t, 0x11, sp + 0xB0);
            }
            if step == SPARK && g(fx) == RING {
                cell(fx, t, sp + 0x140, 0x14);
                tex(fx, TEX + 4);
                fx.mem.set16(D + 0xB8, 0xA);
                rings(fx, t, 0, BYTES_B);
            }
        }
        debug_assert_ne!(g(fx), UNUSED_D);
        if g(fx) == YELLOW {
            tex(fx, D + 0x29C);
            cell(fx, t, sp + 0x160, 0x9B);
            rings(fx, t, 1, BYTES_C);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x107 => 1,
        0x1A9 => 4,
        0x19F => 2,
        0x112..=0x199 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
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
    copy4(fx, D + 0x94, r);
}

/// Ten rows (D+0xB8) of `ring_quads_z` in layout `layout` (D+0xB0) on
/// `table`, both cleared after.
fn rings(fx: &mut Fx, t: u32, layout: i32, table: u32) {
    fx.mem.set16(D + 0xB8, 0xA);
    fx.mem.set16(D + 0xB0, layout);
    quads4::ring_quads_z(fx, uv::uv_rect, D, t, 0x20, table);
    fx.mem.set16(D + 0xB8, 0);
    fx.mem.set16(D + 0xB0, 0);
}

/// 0x1E04630(D, step, 0): the first free task cleared and set off at
/// D+0x250 (i16 +0, floats +0x10..+0x18) with the colour D+0x52F4
/// (twice), kind D+0x5366 (+0x50); `saves` the caller's s0..s4, `ra` the
/// return address.
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 5], ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, saves[4]), (0x30, saves[3]), (0x20, saves[2]), (0x10, saves[1]), (0, saves[0])]);
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
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, step);
    }
    fx.leave(0x60);
}

/// 0x1E01AD0(D, 0): the update counter, the orbs and the spawns (frame
/// 0x70: ra, the entry's s3 / s2 (the runner's, 0 [H]), s1 = the
/// parameter, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x40, 0x01E0_3B28), (0x30, 0), (0x20, 0), (0x10, param as i64 as u64), (0, u64::from(D))]);
    let d = u64::from(D);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The emitter's s0 / s1 / s2 after its first loop: the orb count N,
    // D + 2 N, D + 16 N.
    let mut i = 0i32;
    while i < fx.mem.i16(ORB_N) {
        let at = (AGES as i32 + 2 * i) as u32;
        let p = orb_point(i);
        if fx.mem.i16(at) == 0 {
            let h = fx.target_handle();
            let slot = fx.mem.i16(TARGET_NODE);
            node_position_saving(fx, h, slot, sp + 0x50, i as u32 as u64, Some(u64::from(at - 0x5344)), 0x01E0_1B2C);
            let v = rmod(fx, i as u32, 0x01E0_1B34, 0x14);
            let x = ee::add(fx.mem.f(sp + 0x50), ee::from_i32(v - 0xA));
            fx.mem.setf(sp + 0x50, x);
            let v = rmod(fx, i as u32, 0x01E0_1B68, 0x14);
            let x = ee::add(fx.mem.f(sp + 0x54), ee::from_i32(v - 0xA));
            fx.mem.setf(sp + 0x54, x);
            for o in [8u32, 4, 8] {
                let v = ee::div(ee::sub(fx.mem.f(sp + 0x50 + o), fx.mem.f(p + o)), 50.0);
                fx.mem.setf(p + 0xA0 + o, v);
            }
            let a = fx.mem.i16(at) + 1;
            fx.mem.set16(at, a);
        }
        i = (i + 1) as i16 as i32;
    }
    let n_orbs = fx.mem.i16(ORB_N);
    let mut i = 0i32;
    while i < fx.mem.i16(ORB_N) {
        let at = (AGES as i32 + 2 * i) as u32;
        let p = orb_point(i);
        let a = fx.mem.i16(at);
        if a != 0 {
            if a < 0x19 || (0x7E..0x95).contains(&a) {
                for o in [8u32, 4, 0] {
                    let v = ee::add(fx.mem.f(p + o), fx.mem.f(p + 0xA0 + o));
                    fx.mem.setf(p + o, v);
                }
            }
            let a = fx.mem.i16(at) + 1;
            fx.mem.set16(at, a);
        }
        i = (i + 1) as i16 as i32;
    }
    let big = i64::from(n_orbs.max(0));
    let (mut s0, mut s1, s2) = (big as u64, d + 2 * big as u64, d + 16 * big as u64);
    let c = fx.mem.i16(COUNTER);
    if [0x53, 0x78, 0x9A, 0xB7, 0xD8, 0xF6].contains(&c) {
        let m = fx.mem.i16(ORB_N);
        fx.mem.set16(KIND, m);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        let k = fx.mem.i16(KIND);
        node_position_saving(fx, h, slot, orb_point(k), s0, Some(s1), 0x01E0_1D48);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, POS, s0, Some(s1), 0x01E0_1D60);
        spawn(fx, ORB, [s0, s1, s2, d, 0], 0x01E0_1D74);
        spawn(fx, GREY, [s0, s1, s2, d, 0], 0x01E0_1D88);
        for j in 0..0x12u64 {
            s1 = j;
            s0 = 0x14 * j;
            fx.mem.set16(KIND, s0 as i32);
            spawn(fx, FLY, [s0, s1, s2, d, 0], 0x01E0_1DAC);
        }
        s0 = 0x14 * 0x12;
        s1 = 0x12;
        let m = fx.mem.i16(ORB_N) + 1;
        fx.mem.set16(ORB_N, m);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x3C..0x51).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, POS, s0, Some(s1), 0x01E0_1E1C);
        spawn(fx, YELLOW, [s0, s1, s2, d, 0], 0x01E0_1E30);
    }
    fx.leave(0x70);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn odd_age(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54).wrapping_rem(2) == 1
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends
/// (`end`; otherwise the byte is left).
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8, end: bool) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else if end {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// Both alphas in by 0x40 under 0xC0 for `n` updates, then +0x63 out by
/// 14 twice (the task ending on the first).
fn pulse(fx: &mut Fx, t: u32, n: i32) {
    if fx.mem.i16(t + 0x54) < n {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
        fade_in(fx, t, 0x67, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE, true);
        // The record's second fade reads and writes +0x63 again (+0x67
        // stays) [F: 0x1E01F2C / 0x1E02FCC / 0x1E032FC].
        fade_out(fx, t, 0x63, 0x10, 0xE, false);
    }
}

/// +0x20 / +0x24 / +0x28 = rand % 0xFFFF each.
fn turn(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    for (o, ra) in [0x20u32, 0x24, 0x28].into_iter().zip(ras) {
        let v = rmod(fx, t, ra, 0xFFFF);
        fx.mem.set32(t + o, v);
    }
}

/// 0x1E01E50 (D, task; frame 0x20): a grey quad 25 x 25 growing by 10 to
/// 84, its cell row toggling 0 / 1; in for 149 updates; turned by the
/// camera.
fn grey(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x19);
        fx.mem.set16(t + 0xA, 0x19);
        let v = fx.rand_from(t, 0x01E0_1E7C);
        fx.mem.set16(t + 0x56, v % 2);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    pulse(fx, t, 0x95);
    let r = fx.mem.i16(t + 0x56);
    fx.mem.set16(t + 0x56, i32::from(r == 0));
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x4B {
                fx.mem.set16(t + o, s + 0xA);
            }
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(t - 0x2A0), 0x01E0_1FA8);
    age(fx, t);
}

/// 0x1E01FD0 (`grow`, D, task) / 0x1E03050 (D, task) (frame 0x20): a ring
/// of quads (size 30 / 10, white / grey, cell row 0..6, angle 0..359 up 1
/// a update, +0xA 0..7, +0xC the kind, radius 10..29 / 10..89 (0x1E01FD0
/// growing by +0xE 16..45), row 0..34, at 0x80 for 2..4 updates, then out
/// by 8).
fn ring_b(fx: &mut Fx, t: u32, grow: bool) {
    let o = if grow { 0 } else { 0x1080 };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, if grow { 0x41F0_0000 } else { 0x4120_0000 });
        let v = rmod(fx, t, 0x01E0_1FF8 + o, 7);
        fx.mem.set16(t + 0x56, v);
        let g = if grow { 0xFF } else { 0x80 };
        l2::rgb(fx, t + 0x60, g, g, g);
        let v = rmod(fx, t, 0x01E0_2024 + o, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_2040 + o);
        fx.mem.set16(t + 0xA, v % 8);
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, k);
        let v = rmod(fx, t, 0x01E0_2064 + o, if grow { 0x14 } else { 0x50 });
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_2084 + o, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, 0x01E0_20A4 + o, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_20C0 + o, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade_out(fx, t, 0x63, 0xA, 8, true);
    }
    let a = fx.mem.i16(t + 0x5A) + 1;
    fx.mem.set16(t + 0x5A, a);
    if grow && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02370 (D, task; frame 0x40): the orb (radius 100, grey over red
/// (0x80 / 0x60, 0x80 / 0x60, 8) by its age's parity), in for 149 updates;
/// on its update 149 at its point ten rings 0x1E02BA0, ten 0x1E01FD0 and a
/// wide band 0x1E02880; on 7..12 every +0xE-th update a band 0x1E02DA0;
/// while shown (before 25 or from 126, under 149) a spark 0x1E02EB0 and a
/// ring 0x1E03050 a update; turned by the camera and 0x4000 more.
fn orb(fx: &mut Fx, t: u32) {
    let d = u64::from(D);
    // The spawns' saves: s0 (the entry's D, or the loop counter), s1 = the
    // task, s2 = D, the entry's s3 / s4 (the runner's, 0 [H]).
    let mut s0 = d;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 8);
        let v = rmod(fx, D, 0x01E0_23D0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, D, 0x01E0_23EC, 0xA);
        fx.mem.set16(t + 0xE, v + 1);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let g = if odd_age(fx, t) { 0x80 } else { 0x60 };
    l2::rgb(fx, t + 0x64, g, g, 8);
    if fx.mem.i16(t + 0x54) < 0x95 {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0x10, 0xE, true);
    }
    let at_point = |fx: &mut Fx| {
        let k = fx.mem.i16(t + 0x50);
        copy4(fx, POS, orb_point(k));
    };
    if fx.mem.i16(t + 0x54) == 0x95 {
        at_point(fx);
        for i in 0..0xAu64 {
            spawn(fx, BURST, [i, u64::from(t), d, 0, 0], 0x01E0_250C);
        }
        for i in 0..0xAu64 {
            spawn(fx, BURST_B, [i, u64::from(t), d, 0, 0], 0x01E0_2548);
        }
        s0 = 0xA;
        spawn(fx, WIDE, [s0, u64::from(t), d, 0, 0], 0x01E0_2580);
    }
    let a = fx.mem.i16(t + 0x54);
    if (7..0xD).contains(&a) && (a < 0x19 || a >= 0x7E) {
        let e = fx.mem.i16(t + 0xE);
        if fx.div(a, e).1 == 1 {
            at_point(fx);
            spawn(fx, BAND, [s0, u64::from(t), d, 0, 0], 0x01E0_2614);
        }
    }
    let a = fx.mem.i16(t + 0x54);
    if a < 0x95 && (a < 0x19 || a >= 0x7E) {
        at_point(fx);
        spawn(fx, SPARK, [s0, u64::from(t), d, 0, 0], 0x01E0_267C);
        spawn(fx, RING, [s0, u64::from(t), d, 0, 0], 0x01E0_2690);
    }
    camera_angles_saving(fx, t + 0x20, s0, u64::from(t), 0x01E0_269C);
    let r = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, r);
    age(fx, t);
}

/// 0x1E02880 (D, task; frame 0x20): a wide band (radius 500, (0xFF, 0xCF,
/// 0xCF) over red (0x80 / 0xC0, same, 8)), in by 0x40 for 4 updates, out
/// by 9; turned by the camera and 0x4000 more.
fn wide(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x43FA_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x64, 0x80, 8, 8);
        let v = rmod(fx, t, 0x01E0_28D4, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    let g = if odd_age(fx, t) { 0x80 } else { 0xC0 };
    l2::rgb(fx, t + 0x64, g, g, 8);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(t - 0x2A0), 0x01E0_29A4);
    let r = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, r);
    age(fx, t);
}

/// 0x1E02BA0 (D, task; frame 0x20): a ring of quads (size 10..19,
/// (0xC0, 0xC0, 0x50), cell row 0..3, turned at random, radius 200..399
/// growing by 1..3, row +0x52 from 1 stepping, +8 1..8, at 0x60 for 2..4
/// updates then out by 11).
fn burst(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2BC4, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = fx.rand_from(t, 0x01E0_2BF0);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0x50);
        turn(fx, t, [0x01E0_2C24, 0x01E0_2C40, 0x01E0_2C5C]);
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_2C80, 0xC8);
        fx.mem.set16(t + 0x50, v + 0xC8);
        let v = rmod(fx, t, 0x01E0_2CA0, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2CBC, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, 0x01E0_2CDC);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_2CFC, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        fade_out(fx, t, 0x63, 0xD, 0xB, true);
    }
    let r = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, r);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
}

/// 0x1E02DA0 (D, task; frame 0x20): a band (grey 0xCF over (0x80, 0x80,
/// 8)) from radius 20 widening by 20 (the outer 20 beyond), in by 0x40
/// for 13 updates, out by 9.
fn band(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 8);
        let v = rmod(fx, t, 0x01E0_2DF4, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.write_u32(t + 0x20, 0x4000);
    }
    if fx.mem.i16(t + 0x54) < 0xD {
        fade_in(fx, t, 0x63, 0xC0, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9, true);
    }
    let r = ee::add(fx.mem.f(t + 0x48), 20.0);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::add(20.0, r));
    age(fx, t);
}

/// 0x1E02EB0 (D, task; frame 0x20): a yellow (0xFF, 0xFF, 0x80) quad 150
/// x 150 shrinking by 15 to 20, cell row 0..7, in by 0x40 for 3 updates,
/// out by 14; turned by the camera.
fn spark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x96);
        fx.mem.set16(t + 0xA, 0x96);
        let v = fx.rand_from(t, 0x01E0_2EDC);
        fx.mem.set16(t + 0x56, v % 8);
        fx.mem.write_u32(t + 0x34, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_2F30, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2F4C, 0xB4);
        fx.mem.set16(t + 0x5C, v - 0x5A);
    }
    pulse(fx, t, 3);
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s >= 0x15 {
                fx.mem.set16(t + o, s - 0xF);
            }
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(t - 0x2A0), 0x01E0_302C);
    age(fx, t);
}

/// 0x1E031D0 (D, task; frame 0x30): a yellow quad 150 x 150 growing by 15
/// flying out at 3.5 along its kind's angle (x / y) and, after a while,
/// toward / away from the camera by 2 sin(+0x5A) (up 5 to 90), in by 0x40
/// for 10 updates, out by 14; turned by the camera.
fn fly(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x96);
        fx.mem.set16(t + 0xA, 0x96);
        let v = fx.rand_from(t, 0x01E0_3204);
        fx.mem.set16(t + 0x56, v % 8);
        let a = fx.mem.i16(t + 0x50);
        let c = l2::sin_deg(fx, SINES, a, 1);
        fx.mem.setf(t + 0x30, ee::mul(3.5, c));
        let a = fx.mem.i16(t + 0x50);
        let s = l2::sin_deg(fx, SINES, a, 0);
        fx.mem.setf(t + 0x34, ee::mul(3.5, s));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x5A, 0);
        fx.mem.set16(t + 0x5C, 0);
    }
    pulse(fx, t, 0xA);
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o) + 0xF;
            fx.mem.set16(t + o, s);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [0u32, 4] {
            let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
            fx.mem.setf(t + 0x10 + o, v);
        }
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, SINES, a, 0);
        let z = fx.mem.f(t + 0x18);
        let by = ee::mul(2.0, s);
        let z = if fx.mem.i16(FACING) != 0 { ee::add(z, by) } else { ee::sub(z, by) };
        fx.mem.setf(t + 0x18, z);
        let a = fx.mem.i16(t + 0x5A);
        if a < 0x5A {
            fx.mem.set16(t + 0x5A, a + 5);
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_33E8);
    age(fx, t);
}

/// 0x1E03530 (D, task; frame 0x30): a yellow-green (0xC0, 0xC0, 0x50) ring
/// of quads at the attacker's node 0x2D (size 4..11, cell row 0..3, radius
/// 200..399 closing by 10..39 to 30, row +0x52 from 1 stepping, +8 1..8),
/// in by 0x18 under 0x60 for 4..6 updates, out by 11.
fn yellow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = fx.rand_from(t, 0x01E0_355C);
        fx.mem.setf(t + 0x48, ee::from_i32(v % 8 + 4));
        let v = fx.rand_from(t, 0x01E0_3588);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0x50);
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_35C0, 0xC8);
        fx.mem.set16(t + 0x50, v + 0xC8);
        let v = rmod(fx, t, 0x01E0_35E0, 0x1E);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = fx.rand_from(t, 0x01E0_3600);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, 0x01E0_3620, 3);
        fx.mem.set16(t + 6, v + 4);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x63, 0x60, 0x18);
    } else {
        fade_out(fx, t, 0x63, 0xD, 0xB, true);
    }
    let r = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, r);
    let r = fx.mem.i16(t + 0x50);
    if r >= 0x1F {
        let e = fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r - e);
    } else {
        fx.mem.set16(t + 0x50, 0x1E);
    }
    age(fx, t);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_36E8);
}
