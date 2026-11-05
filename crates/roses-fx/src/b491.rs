//! BMODULE b491 (card 383), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E035C0, data area 0x1ED0000) [F] (helper A11, 31
//! Oct 2026): b183's pool and data layout (150 tasks of 0x70 bytes at
//! D+0x2A0, alive +0x58, step +0x44 called through `jalr`; spawned by
//! 0x1E03E50, 8b19c0d03b97, b183's spawn) drawn as flat rings facing the
//! camera (9c13e08cfd2f at 0x1E00D70, `quads4::ring_facing_flat`), a quad
//! (40d081dab923 at 0x1E01870, `billboard::quad_b`), arcs of quads
//! (6e60344f604f at 0x1E01B50, `billboard::arc_quads`), a ring band
//! (2f3eecc0b281 at 0x1E00920, `quads4::ring_band_f`) and rings of quads
//! (7369422df7f8 at 0x1E012C0, `billboard::ring_quads`), UVs by uv_rect
//! 0x1E00080, plus a unit flash (0x1E03F90: the attacker drawn into the
//! target in a packet layer, then the flash task 0x1E03360 stepped).
//! Update 1: the stage light, the flash (grey 0xC0, in by 0x18 for 200
//! updates) and an object op (0x1F8380); to update 209 a white ring a
//! update at one of the attacker's nodes 0x01 / 0x22 / 0x1C / 0x19 / 0x2C
//! / 0x31 / 0x38 (update % 7); on 86..199 every fifth update a glowing
//! quad from node 1 drifting to the target for 45 updates, then bursting
//! into seven rings, three arcs and a red band. Returns 1 on update 115,
//! 4 on 275, 2 on 250, 6 on 126..244, 8 once the 290-update timer +0x44A0
//! has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const RING: u32 = 0x01E0_21E0;
const ORB: u32 = 0x01E0_2370;
const ARCS: u32 = 0x01E0_2810;
const RING_W: u32 = 0x01E0_29B0;
const RING_S: u32 = 0x01E0_2B40;
const RING_B: u32 = 0x01E0_2D00;
const BAND: u32 = 0x01E0_2EC0;
const RINGQ: u32 = 0x01E0_3010;
const LIGHT: u32 = 0x01E0_3210;
const OBJECT: u32 = 0x01E0_32E0;
const FLASH: u32 = 0x01E0_3360;
/// The return address of the entry's `jalr` into a step (0x1E038D8 + 8).
const TASK_RA: u64 = 0x01E0_38E0;
const POS: u32 = D + 0x250;
const TEX: u32 = D + 0x4480;
const TIMER: u32 = D + 0x44A0;
const COLOUR: u32 = D + 0x44A4;
const LAYER: u32 = D + 0x44B4;
const FLASH_DESC: u32 = D + 0x44B8;
const NODE: u32 = D + 0x44D4;
const KIND: u32 = D + 0x4502;
const FACING: u32 = D + 0x4506;
const COUNTER: u32 = D + 0x450A;
const BYTES: u32 = D + 0x450C;
const BYTES_B: u32 = D + 0x460C;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E035C0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xF0);
    fx.mem.write_u32(D + 0x44AC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xF0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4880);
    l2::sine_table(fx, D + 0x470C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4508, param);
    fx.mem.set32(TIMER, 0x122);
    for (k, node) in [1, 0x22, 0x1C, 0x19, 0x2C, 0x31, 0x38, 0x19].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x44E4, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xE0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xE4)), 180.0);
    fx.fix_residue(y, 0x01E0_378C);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = D.
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_37B4, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_37FC, 0x14);
        fx.mem.set8(BYTES_B + k, v - 0xA);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_384C);
        fx.mem.set8(BYTES_B + k, v % 4);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s1: the flash tasks counted here.
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => {}
            FLASH => flashes += 1,
            // The small steps' saves: the entry's s0 = D (and ra).
            RING | RING_W | RING_S | RING_B => small(fx, t, f),
            ARCS => arcs(fx, t),
            BAND => band(fx, t),
            RINGQ => ringq(fx, t),
            LIGHT => light(fx, t),
            OBJECT => object(fx, t),
            ORB => orb(fx, t, flashes, slot, k),
            _ => panic!("b491: unknown task step {f:#x}"),
        }
    }
    // a3 as the draws' last library call left it, for the unit flash's
    // first TEXFLUSH (b180's lesson): 0 after the rings / quad / ring of
    // quads, the last XYZ2 drawn after the arcs / band [run].
    let mut a3 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let ring = |fx: &mut Fx, local: u32, tex: u32, rows: i32, table: u32| {
            cell(fx, t, sp + local, 0x14);
            let w = fx.mem.read_u32(tex);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, rows);
            fx.mem.set16(D + 0xC0, 0);
            for k in 0..3 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, v);
            }
            quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, table);
            fx.mem.set16(D + 0xB8, 0);
            0
        };
        let at_task = |fx: &mut Fx, n: u32| {
            for k in 0..n {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x60 + 4 * k, v);
            }
        };
        if fx.mem.read_u32(t + 0x44) == RING {
            a3 = ring(fx, 0x70, TEX, 8, BYTES_B);
        }
        if fx.mem.read_u32(t + 0x44) == ORB {
            cell(fx, t, sp + 0x80, 0x14);
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xBE, 0);
            at_task(fx, 3);
            billboard::quad_b(fx, uv::uv_rect, D, t, 0x21, sp + 0x60);
            a3 = 0;
        }
        if fx.mem.read_u32(t + 0x44) == ARCS {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x90, 0x9B);
            at_task(fx, 4);
            fx.mem.set16(D + 0xB8, 9);
            billboard::arc_quads(fx, uv::uv_rect, D, t, 0x20, sp + 0x60);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == RING_W {
            a3 = ring(fx, 0xA0, TEX, 8, BYTES_B);
        }
        if fx.mem.read_u32(t + 0x44) == RING_S {
            a3 = ring(fx, 0xB0, TEX, 0xA, BYTES);
        }
        if fx.mem.read_u32(t + 0x44) == RING_B {
            a3 = ring(fx, 0xC0, TEX + 8, 0xA, BYTES);
        }
        if fx.mem.read_u32(t + 0x44) == BAND {
            at_task(fx, 4);
            let before = fx.out.prims.len();
            quads4::ring_band_f(fx, D, t, 0x21, sp + 0x60);
            a3 = last_xyz2(fx, before);
        }
        if fx.mem.read_u32(t + 0x44) == RINGQ {
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0xD0, 0x97);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            a3 = 0;
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
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
        0x73 => 1,
        0x113 => 4,
        0xFA => 2,
        0x7E..=0xF4 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
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

/// 0x1E03E50(D, step, 0) (8b19c0d03b97, b183's 0x1E02880): the first free
/// task cleared and set off at D+0x250 (i16 +0, float +0x10) with the
/// colour D+0x44A4 (twice), kind D+0x4502 (+0x50); `saves` the caller's
/// s0..s4, `ra` the return address.
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

/// x / y (/ z) of D+0x250 += rand % 10 - 5 (rand's frames s0 = D).
fn jitter(fx: &mut Fx, sites: &[u32]) {
    for (k, &ra) in sites.iter().enumerate() {
        let v = rmod(fx, D, ra, 0xA);
        let o = POS + 4 * k as u32;
        let p = ee::add(fx.mem.f(o), ee::from_i32(v - 5));
        fx.mem.setf(o, p);
    }
}

/// 0x1E01FA0(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = 0, s0 = D; then s0 = D, s1 = the count).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_38A0), (0x10, 0), (0, u64::from(D))]);
    // The spawns' saves: s0 = D, s1 (the entry's 0, then the count), the
    // entry's s2 = 0, s3 = the parameter, s4 the runner's (0 [H]).
    let p = param as i64 as u64;
    let saves = |s1: u64| [u64::from(D), s1, 0, p, 0];
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, LIGHT, saves(0), 0x01E0_1FDC);
        spawn(fx, FLASH, saves(0), 0x01E0_1FF0);
        spawn(fx, OBJECT, saves(0), 0x01E0_2004);
    }
    let c = fx.mem.i16(COUNTER);
    let s1 = c as i64 as u64;
    if c < 0xD2 {
        let h = fx.self_handle();
        let r = fx.div(c, 7).1;
        let slot = fx.mem.i16(NODE + 2 * r as u32);
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(s1), 0x01E0_2048);
        jitter(fx, &[0x01E0_2050, 0x01E0_2084, 0x01E0_20B8]);
        spawn(fx, RING_S, saves(s1), 0x01E0_20F4);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x56..0xC8).contains(&c) && fx.div(c, 5).1 == 1 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE);
        node_position_saving(fx, h, slot, POS, u64::from(D), Some(s1), 0x01E0_2148);
        jitter(fx, &[0x01E0_2150, 0x01E0_2184]);
        spawn(fx, ORB, saves(s1), 0x01E0_21C0);
    }
    fx.leave(0x30);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// A step's prologue (frame 0x20: ra, the entry's s0 = D).
fn frame20(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
}

/// 0x1E021E0 (white, size 50, cell 3), 0x1E029B0 (white, 100, cell 3),
/// 0x1E02B40 / 0x1E02D00 (white, 3 / 23, cell 0..7 stepping) (D, task): a
/// flat ring: angle +0x5A random, +0xA rand % 8, +0xC = the spawn kind
/// +0x50, radius +0x50 10..59 / 20..99 / 4..103 / 4..103 growing by +0xE
/// 6..15 / 14..23 / 2..9 / 10..19, row 0..34, life 3..7 (1..5 stepping);
/// 0x80 during the life, then down by 8 (b183's 0x1E012F0 family).
fn small(fx: &mut Fx, t: u32, f: u32) {
    frame20(fx);
    // (size, cell rand, radius m / add, growth m / add, life add)
    let (size, stepping, rm, ra, em, ea, la) = match f {
        RING => (0x4248_0000u32, false, 0x32, 0xA, 0xA, 6, 3),
        RING_W => (0x42C8_0000, false, 0x50, 0x14, 0xA, 0xE, 3),
        RING_S => (0x4040_0000, true, 0x64, 4, 8, 2, 1),
        _ => (0x41B8_0000, true, 0x64, 4, 0xA, 0xA, 1),
    };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, size);
        // The stepping steps' sites are 0x14 further on (their cell rand
        // first).
        let o = if stepping { 0x14 } else { 0 };
        if stepping {
            let v = fx.rand_from(t, f + 0x28);
            fx.mem.set16(t + 0x56, v % 8);
        } else {
            fx.mem.set16(t + 0x56, 3);
        }
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let v = rmod(fx, t, f + 0x48 + o, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, f + 0x64 + o);
        fx.mem.set16(t + 0xA, v % 8);
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, k);
        let v = rmod(fx, t, f + 0x88 + o, rm);
        fx.mem.set16(t + 0x50, v + ra);
        let v = if f == RING_S { fx.rand_from(t, f + 0xA8 + o) % 8 } else { rmod(fx, t, f + 0xA8 + o, em) };
        fx.mem.set16(t + 0xE, v + ea);
        let v = rmod(fx, t, f + 0xC8 + o, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xE4 + o, 5);
        fx.mem.set16(t + 6, v + la);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x63, a - 8);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    if stepping {
        let c = fx.mem.i16(t + 0x56) + 1;
        fx.mem.set16(t + 0x56, if c >= 8 { 0 } else { c });
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02810 (D, task): an arc of quads (pale yellow, inner radius 12..21
/// growing by 2, the outer +0x4C left 0; orientation +0x52 rand % 5,
/// random angles +0x20..+0x28, from 45 degrees spanning 6); in by 0x40
/// under 0xC0 for 3 updates, out by 9.
fn arcs(fx: &mut Fx, t: u32) {
    frame20(fx);
    let f = ARCS;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xC));
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0x80);
        let v = rmod(fx, t, f + 0x74, 5);
        fx.mem.set16(t + 0x52, v);
        let v = rmod(fx, t, f + 0x90, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, site) in [(0x20u32, f + 0xB8), (0x24, f + 0xD4), (0x28, f + 0xF0)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x5A, 0x2D);
        fx.mem.set16(t + 0x5C, 6);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 3 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let r = ee::add(fx.mem.f(t + 0x48), 2.0);
    fx.mem.setf(t + 0x48, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02EC0 (D, task): the band (inner radius 100, the outer +0x4C left
/// 0; inner grey 0xCF, outer red 0x80 / 0xC0 by its age's parity, alpha
/// 8): in by 0x40 under 0xC0 for 4 updates, out by 14; turned by the
/// camera's angles (the first + 0x4000).
fn band(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 8);
        let v = rmod(fx, t, BAND + 0x54, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 4 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let r = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0x80 } else { 0xC0 };
    l2::rgb(fx, t + 0x64, r, 8, 8);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, BAND + 0x124);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03010 (D, task): a ring of quads in the task's random angles
/// (blue-grey (0x50, 0x50, 0xC0), size 4..6, cell 0..3, radius 20..59
/// growing by 1..3, row 1..8, life 2..4, +0x52 counting); 0x60 during the
/// life, then down by 11.
fn ringq(fx: &mut Fx, t: u32) {
    frame20(fx);
    let f = RINGQ;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 3);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 4));
        let v = fx.rand_from(t, f + 0x50);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0x50, 0x50, 0xC0);
        for (o, site) in [(0x20u32, f + 0x84), (0x24, f + 0xA0), (0x28, f + 0xBC)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, f + 0xE0, 0x28);
        fx.mem.set16(t + 0x50, v + 0x14);
        let v = rmod(fx, t, f + 0x100, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0x11C, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, f + 0x13C);
        fx.mem.set16(t + 8, v % 8 + 1);
        let v = rmod(fx, t, f + 0x15C, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xD {
            fx.mem.write_u8(t + 0x63, a - 0xB);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let n = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, n);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03210 (D, task): the stage light (0x1F7F10) down by 10 to 0x30
/// while the timer is at 31 or more, then back up by 10 to 0x80; 0x80 and
/// the task's end when the timer is at 6. No age.
fn light(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let l = fx.mem.i16(t + 0x50);
    if fx.mem.i32(TIMER) >= 0x1F {
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
    }
    if fx.mem.i32(TIMER) == 6 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x20);
}

/// 0x1E032E0 (D, task): the object op 0x1F8380(3, 1, the record's floats
/// 0x1E04100 / 0x1E04108 at sp + 0x28 / + 0x2C, 0) each update, for 201
/// updates (frame 0x30: ra, the entry's s0 = D).
fn object(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    for (to, from) in [(0x28u32, 0x01E0_4100u32), (0x2C, 0x01E0_4108)] {
        let w = fx.mem.read_u32(from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.boundary(0x001F_8380, "object op 0x1F8380", [3, 1, sp + 0x28, sp + 0x2C]);
    if fx.mem.i16(t + 0x54) >= 0xC9 {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02370 (D, task): the glowing quad (frame 0x90: the entry's s0 = D,
/// s1 = the flashes, s2 = 0, s3 = the slot, s4 = its index): size 15
/// growing by 1 to 50, white / pale grey by its age's parity, both alphas
/// in by 0x2A under 0x80 for 45 updates, then out by 0xE twice; for 45
/// updates it moves a 45th of the way from the attacker to the target a
/// update (and from its update 6, while above 40, by (40 - node 1's y) /
/// 65 more in y); on update 45 seven white rings burst out (within 10),
/// three arcs and the band; its cell steps 0..7; turned by the camera's
/// angles.
fn orb(fx: &mut Fx, t: u32, flashes: u32, slot: u32, k: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x50, TASK_RA), (0x40, u64::from(k)), (0x30, u64::from(slot)), (0x20, 0), (0x10, u64::from(flashes)), (0, u64::from(D))]);
    let h = fx.target_handle();
    unit_position_saving(fx, h, sp + 0x60, u64::from(D), 0x01E0_23A4);
    let h = fx.self_handle();
    unit_position_saving(fx, h, sp + 0x70, u64::from(sp + 0x70), 0x01E0_23BC);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODE);
    node_position_saving(fx, h, s, sp + 0x80, u64::from(sp + 0x70), Some(u64::from(flashes)), 0x01E0_23D4);
    for o in [0u32, 4, 8] {
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x60 + o), fx.mem.f(sp + 0x70 + o)), 45.0);
        fx.mem.setf(sp + 0x60 + o, v);
    }
    let v = ee::div(ee::sub(40.0, fx.mem.f(sp + 0x84)), 65.0);
    fx.mem.setf(sp + 0x84, v);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xF);
        fx.mem.set16(t + 0xA, 0xF);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x2D {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let s = fx.mem.i16(t + o);
            if s < 0x32 {
                fx.mem.set16(t + o, s + 1);
            }
        }
    }
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, g);
    if fx.mem.i16(t + 0x54) < 0x2D {
        for o in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(sp + 0x60 + o));
            fx.mem.setf(t + 0x10 + o, v);
        }
        if fx.mem.i16(t + 0x54) >= 6 {
            let y = fx.mem.f(t + 0x14);
            if !(y <= 40.0) {
                let dy = fx.mem.f(sp + 0x84);
                fx.mem.setf(t + 0x14, ee::add(y, dy));
            }
        }
    }
    // The spawns' saves: the step's s0 = the loop counter, s1 = sp + 0x68,
    // s2 = sp + 0x84, s3 = the task, s4 = D.
    let saves = |i: u32| [u64::from(i), u64::from(sp + 0x68), u64::from(sp + 0x84), u64::from(t), u64::from(D)];
    let burst = fx.mem.i16(t + 0x54) == 0x2D;
    if burst {
        for i in 0..7u32 {
            for o in 0..4u32 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * o);
                fx.mem.write_u32(POS + 4 * o, w);
            }
            for (o, ra) in [(0u32, 0x01E0_2660u32), (4, 0x01E0_2694), (8, 0x01E0_26C8)] {
                let v = rmod(fx, i, ra, 0x14);
                let p = ee::add(fx.mem.f(POS + o), ee::from_i32(v - 0xA));
                fx.mem.setf(POS + o, p);
            }
            spawn(fx, RING_B, saves(i), 0x01E0_2704);
        }
        for i in 0..3u32 {
            for o in 0..4u32 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * o);
                fx.mem.write_u32(POS + 4 * o, w);
            }
            spawn(fx, ARCS, saves(i), 0x01E0_2774);
        }
        spawn(fx, BAND, saves(3), 0x01E0_27A8);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let c = fx.mem.i16(t + 0x56) + 1;
        fx.mem.set16(t + 0x56, if c >= 8 { 0 } else { c });
    }
    // s0: the arcs loop's count (3) after the burst, else sp + 0x64.
    let s0 = if burst { 3 } else { u64::from(sp + 0x64) };
    camera_angles_saving(fx, t + 0x20, s0, u64::from(sp + 0x68), 0x01E0_27E0);
    age(fx, t);
    fx.leave(0x90);
}

/// 0x1E03360(D, t): the flash's frame sprite (the target over the window,
/// grey 0xC0, UVs from +8 / 10 and +0xA / 10 over 512, the bottom edge at
/// (512 - 3 (+0xA / 10)) / 512) into the descriptor at +0x44B8, submitted
/// twice with no ordering table; alpha up by 0x18 below 0xC0 for 200
/// updates, then down by 9. `k` the unit flash's s1 (the slot index).
fn flash(fx: &mut Fx, t: u32, k: u32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_405C), (0x20, u64::from(k)), (0x10, u64::from(D))]);
    // swc1 f20 / f21: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        fx.mem.set16(t + 0x58, 2);
    }
    let hd = fx.self_handle();
    let s = fx.mem.i16(NODE);
    node_position_saving(fx, hd, s, sp + 0x40, u64::from(t), Some(u64::from(D)), 0x01E0_33FC);
    let hd = fx.self_handle();
    unit_position_saving(fx, hd, sp + 0x40, u64::from(t), 0x01E0_3410);
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
    let bottom = ee::div(ee::sub(512.0, ee::mul(3.0, h)), 512.0);
    for (o, v) in [(0x18u32, ee::div(w, 512.0)), (0x1C, ee::div(h, 512.0)), (0x28, ee::div(ee::sub(512.0, w), 512.0)), (0x2C, bottom)] {
        fx.mem.setf(d + o, v);
    }
    fx.submit(d, 0, 0);
    fx.submit(d, 0, 0);
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xC8 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x18);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
    fx.leave(0x50);
}

/// 0x1E03F90(D): in a packet layer (0x2078A0) the attacker drawn into the
/// target (0x1F8270 filter 0; ALPHA 0x48), TEXFLUSH before and after;
/// every flash task stepped (0x1E03360); the layer linked into the
/// ordering table at key 0.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xA0);
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
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x88, sp + 0x90);
    fx.texflush(regs);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) != 0 && fx.mem.read_u32(t + 0x44) == FLASH {
            flash(fx, t, k);
        }
    }
    let layer = fx.mem.read_u32(LAYER);
    fx.layer_end(layer, OT, 0);
    fx.leave(0xA0);
}
