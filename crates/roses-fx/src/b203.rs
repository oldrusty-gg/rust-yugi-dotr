//! BMODULE b203 (card 321), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03600, data area 0x1ED0000, 0x49A0 bytes) [F]
//!: b615's family (150 tasks of 0x70 bytes at
//! D+0x2A0, alive +0x58, step +0x44 through `jalr`; spawned by the
//! record's own 0x1E03FB0 with the colour D+0x44C4, +0x50 = D+0x4522 and
//! +0x5A = D+0x4516) drawn with ring quads (`quads4::ring_quads_z`
//! 0x1E00D70), a ring band (`quads4::ring_band_f` 0x1E00920), quads
//! (`billboard::quad` 0x1E01320), flat arcs (`billboard::arc_strips_flat`
//! 0x1E015E0) and sparks (`quads4::sparks` 0x1E01940), UVs by uv_rect.
//! The emitter 0x1E01D40 counts D+0x452A: below 0x86 / 0x5E ring tasks
//! (0x1E027D0) on the attacker's nodes 0x40 / 0x4C, 0x4E..0x85 a smoke
//! ring (0x1E02950) on node 0x14 (10 to the facing); on 0xD3 / 0xEB the
//! point D+0x4470 / D+0x4480 at node 0x14 with ten radial quads
//! (0x1E02B50, +0x50 = 36 k) and a pulse quad (0x1E02D10), the point
//! then walking a fourteenth of the way to the target's node 1 (z / y)
//! for 13 / 12 updates leaving rings; on 0xE0 / 0xF7 at the point ten
//! radial quads, five white rings (0x1E02E90), five spark fans
//! (0x1E03140), four violet rings (0x1E032D0) and a flat arc (0x1E03030).
//! The ring column 0x1E023A0 (b615's) and the band 0x1E034E0 are
//! never spawned. Returns 1 on update 0xD6, 3 on 0x109, 6 on 0xE1..0x103,
//! 8 once the 0x10D-update timer D+0x44C0 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const COLUMN: u32 = 0x01E0_23A0;
const RING: u32 = 0x01E0_27D0;
const SMOKE: u32 = 0x01E0_2950;
const RADIAL: u32 = 0x01E0_2B50;
const PULSE: u32 = 0x01E0_2D10;
const WHITE: u32 = 0x01E0_2E90;
const ARC: u32 = 0x01E0_3030;
const FAN: u32 = 0x01E0_3140;
const VIOLET: u32 = 0x01E0_32D0;
const BAND: u32 = 0x01E0_34E0;
const IDLE: u32 = 0x01E0_35F0;
/// The return address of the entry's `jalr` into a step (0x1E039C8 + 8).
const TASK_RA: u64 = 0x01E0_39D0;
const POS: u32 = D + 0x250;
const POINT_A: u32 = D + 0x4470;
const POINT_B: u32 = D + 0x4480;
const DY: u32 = D + 0x4494;
const DZ: u32 = D + 0x4498;
const TEX: u32 = D + 0x44A0;
const TIMER: u32 = D + 0x44C0;
const COLOUR: u32 = D + 0x44C4;
const NODES: u32 = D + 0x44F4;
const TARGET_NODE: u32 = D + 0x4504;
const SLOT: u32 = D + 0x4522;
const FACING: u32 = D + 0x4526;
const COUNTER: u32 = D + 0x452A;
const BYTES: u32 = D + 0x452C;
const BYTES_B: u32 = D + 0x462C;
const BYTES_C: u32 = D + 0x472C;
const SINES: u32 = D + 0x482C;

/// rand from the `jal` at `site` with s0 = `s0`, % m.
fn rmod(fx: &mut Fx, s0: u32, site: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, site + 8);
    fx.div(v, m).1
}

/// The entry 0x1E03600.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xF0);
    fx.mem.write_u32(D + 0x44CC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xF0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x49A0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4528, param);
    fx.mem.set32(TIMER, 0x10D);
    for (k, node) in [0x40, 0x4C, 0x14, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xE0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xE4)), 180.0);
    fx.fix_residue(y, 0x01E0_3794);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // rand's frames: the entry's s0 = D.
    for (table, from, site, m, sub) in [
        (BYTES, 0u32, 0x01E0_37B4u32, 0x1E, 0xF),
        (BYTES, 0x64, 0x01E0_3804, 0, 0),
        (BYTES_B, 0, 0x01E0_3854, 0x28, 0x14),
        (BYTES_B, 0xB4, 0x01E0_38A4, 0x14, 0xA),
        (BYTES_C, 0, 0x01E0_38EC, 0x28, 0x14),
        (BYTES_C, 0xB4, 0x01E0_393C, 0xA, 5),
    ] {
        let to = if table == BYTES && from == 0x64 { 0xB4 } else { 0x100 };
        for k in from..to {
            let v = if m == 0 {
                fx.rand_from(D, site + 8) & 3
            } else {
                rmod(fx, D, site, m) - sub
            };
            fx.mem.set8(table + k, v);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 || f == IDLE {
            continue;
        }
        match f {
            COLUMN => column(fx, t, k),
            RING => ring(fx, t, 0x4120_0000, (0x28, 5, 0xA, 6), (0x80, 0x40, 0x20), [0x01E0_27F0, 0x01E0_2824, 0x01E0_2840, 0x01E0_2864, 0x01E0_2884, 0x01E0_28A4, 0x01E0_28C0], false),
            SMOKE => smoke(fx, t),
            RADIAL => radial(fx, t),
            PULSE => pulse(fx, t),
            WHITE => ring(fx, t, 0x420C_0000, (0x28, 5, 0x14, 0xC), (0x80, 0x80, 0x80), [0x01E0_2EB0, 0x01E0_2EE4, 0x01E0_2F00, 0x01E0_2F24, 0x01E0_2F44, 0x01E0_2F64, 0x01E0_2F80], true),
            ARC => arc(fx, t),
            FAN => fan(fx, t),
            VIOLET => violet(fx, t),
            BAND => band(fx, t),
            _ => panic!("b203: unported task step {f:#x}"),
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == COLUMN {
            cell(fx, t, sp + 0x60, 0x14);
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 6);
            let v = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, v);
            let w = fx.mem.read_u32(t + 0x10);
            fx.mem.write_u32(D + 0x20, w);
            let y = ee::add(fx.mem.f(t + 0x14), fx.mem.f(t + 0x34));
            fx.mem.setf(D + 0x24, y);
            let w = fx.mem.read_u32(t + 0x18);
            fx.mem.write_u32(D + 0x28, w);
            fx.mem.set16(D + 0xB8, 0);
        }
        for (step, r, table) in [(RING, sp + 0x70, BYTES_B), (SMOKE, sp + 0x80, BYTES_C), (WHITE, sp + 0xB0, BYTES_B)] {
            if f(fx) != step {
                continue;
            }
            cell(fx, t, r, 0x14);
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 4);
            fx.mem.set16(D + 0xB0, 0);
            if step == SMOKE {
                let h = fx.self_handle();
                let s = fx.mem.i16(NODES + 4);
                fx.node_position(h, s, sp + 0x50);
                let w = fx.mem.read_u32(sp + 0x50);
                fx.mem.write_u32(t + 0x10, w);
                let w = fx.mem.read_u32(sp + 0x54);
                fx.mem.write_u32(t + 0x14, w);
            }
            quads4::ring_quads_z(fx, uv::uv_rect, D, t, 0x20, table);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if f(fx) == RADIAL {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0x90, 0x14);
            fx.mem.set16(D + 0xBE, 5);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, w);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x11, sp + 0x50);
        }
        if f(fx) == PULSE {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, t, sp + 0xA0, 0x14);
            let v = rmod(fx, D, 0x01E0_3C98, 6);
            fx.mem.set16(D + 0xBE, v);
            let p = D.wrapping_add((fx.mem.i16(t + 0x50) << 4) as u32) + 0x4470;
            for k in 0..3 {
                let w = fx.mem.read_u32(p + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, w);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x21, sp + 0x50);
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == FAN {
            cell(fx, t, sp + 0xC0, 0x14);
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0x18);
            quads4::sparks(fx, D, t, 0x20, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == VIOLET {
            cell(fx, t, sp + 0xD0, 0x9B);
            let w = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 9);
            fx.mem.set16(D + 0xB0, 1);
            quads4::ring_quads_z(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if f(fx) == BAND {
            for k in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x50 + 4 * k, w);
            }
            quads4::ring_band_f(fx, D, t, 0x20, sp + 0x50);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xD6 => 1,
        0x109 => 3,
        0xE1..=0x103 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
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

/// 0x1E03FB0(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16 and its xyz at +0x10) with the colour D+0x44C4 (+0x60,
/// +0x64), +0x50 = D+0x4522, +0x5A = D+0x4516. `saves`: its prologue's
/// (ra, s4, s3, s2, s1, s0 of the caller).
fn spawn(fx: &mut Fx, step: u32, saves: [u64; 6]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, saves[0]), (0x40, saves[1]), (0x30, saves[2]), (0x20, saves[3]), (0x10, saves[4]), (0, saves[5])]);
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
            let s = fx.mem.i16(SLOT);
            fx.mem.set16(t + 0x50, s);
            let s = fx.mem.i16(D + 0x4516);
            fx.mem.set16(t + 0x5A, s);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

fn copy4(fx: &mut Fx, from: u32, to: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 0x1E01D40(D, 0): the update counter, the points and the spawns (frame
/// 0x60: ra, the entry's s2 = the parameter, s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_3990), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    // The spawns' saves: ra, s4 / s3 the runner's (0 [H]), s2, s1, s0 = D.
    let sv = |ra: u32, s2: u64, s1: u64| [u64::from(ra), 0, 0, s2, s1, u64::from(D)];
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    let node = |fx: &mut Fx, slot: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i16(slot);
        fx.node_position(h, s, out);
    };
    if c(fx) == 0xD3 {
        node(fx, NODES + 4, POINT_A);
    }
    if c(fx) == 0xEB {
        node(fx, NODES + 4, POINT_B);
    }
    if c(fx) == 0xD3 {
        let h = fx.target_handle();
        let s = fx.mem.i16(TARGET_NODE);
        fx.node_position(h, s, sp + 0x40);
        node(fx, NODES, sp + 0x50);
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(sp + 0x58)), 14.0);
        fx.mem.setf(DZ, v);
        let v = ee::div(ee::sub(fx.mem.f(sp + 0x44), fx.mem.f(sp + 0x54)), 14.0);
        fx.mem.setf(DY, v);
    }
    for (lo, hi, point, ra) in [(0xD4, 0xE1, POINT_A, 0x01E0_1E84u32), (0xEC, 0xF8, POINT_B, 0x01E0_1EE4)] {
        if (lo..hi).contains(&c(fx)) {
            let z = ee::add(fx.mem.f(point + 8), fx.mem.f(DZ));
            fx.mem.setf(point + 8, z);
            copy4(fx, point, POS);
            spawn(fx, RING, sv(ra, p, 0));
        }
    }
    if c(fx) == 0xBA {
        copy4(fx, POINT_A, POS);
        fx.mem.set16(SLOT, 0);
    }
    if c(fx) < 0x86 {
        node(fx, NODES, POS);
        spawn(fx, RING, sv(0x01E0_1F54, p, 0));
    }
    if c(fx) < 0x5E {
        node(fx, NODES + 2, POS);
        spawn(fx, RING, sv(0x01E0_1F90, p, 0));
    }
    if (0x4E..0x86).contains(&c(fx)) {
        node(fx, NODES + 4, POS);
        let z = fx.mem.f(POS + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 10.0) } else { ee::add(z, 10.0) };
        fx.mem.setf(POS + 8, z);
        spawn(fx, SMOKE, sv(0x01E0_2018, p, 0));
    }
    if c(fx) == 0xD3 || c(fx) == 0xEB {
        node(fx, NODES + 4, POS);
        let (mut s1, mut s2) = (0u64, 0u64);
        for _ in 0..10 {
            fx.mem.set16(SLOT, s2 as i32);
            spawn(fx, RADIAL, sv(0x01E0_2070, s2, s1));
            s2 += 0x24;
            s1 += 1;
        }
        if c(fx) == 0xD3 {
            fx.mem.set16(SLOT, 0);
        }
        if c(fx) == 0xEB {
            fx.mem.set16(SLOT, 1);
        }
        spawn(fx, PULSE, sv(0x01E0_20DC, s2, s1));
    }
    for (when, point, ras) in [
        (0xE0, POINT_A, [0x01E0_2130u32, 0x01E0_2178, 0x01E0_21B8, 0x01E0_21F8, 0x01E0_2230]),
        (0xF7, POINT_B, [0x01E0_2284, 0x01E0_22D0, 0x01E0_2310, 0x01E0_2350, 0x01E0_2388]),
    ] {
        if c(fx) != when {
            continue;
        }
        copy4(fx, point, POS);
        let (mut s1, mut s2) = (0u64, 0u64);
        for _ in 0..10 {
            fx.mem.set16(SLOT, s1 as i32);
            spawn(fx, RADIAL, sv(ras[0], s2, s1));
            s1 += 0x24;
            s2 += 1;
        }
        for (step, n, ra) in [(WHITE, 5u64, ras[1]), (FAN, 5, ras[2]), (VIOLET, 4, ras[3])] {
            s1 = 0;
            for _ in 0..n {
                spawn(fx, step, sv(ra, s2, s1));
                s1 += 1;
            }
        }
        spawn(fx, ARC, sv(ras[4], s2, s1));
    }
    fx.leave(0x60);
}

/// A step's prologue (frame `size`: ra, the entry's s1 = 0 when 0x30 /
/// 0x40, s0 = D).
fn enter_step(fx: &mut Fx, size: u32) -> u32 {
    let sp = fx.enter(size);
    if size == 0x20 {
        fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
    } else {
        fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    }
    sp
}

/// rand % 8 / % 4 as the record's signed remainder (rand >= 0).
fn rand_mask(fx: &mut Fx, t: u32, site: u32, m: i32) -> i32 {
    fx.rand_from(t, site + 8) % m
}

/// Alpha +0x63: `full` for the life +6, then out by `down` from `floor`.
fn hold_then_out(fx: &mut Fx, t: u32, full: u8, floor: u8, down: u8) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, full);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= floor {
            fx.mem.write_u8(t + 0x63, a - down);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
}

/// Both alphas in by 0x40 under 0xC0 until age `until`, then +0x63 out by
/// 14 twice (the second only from 0x10).
fn in_then_out_twice(fx: &mut Fx, t: u32, until: i32) {
    if fx.mem.i16(t + 0x54) < until {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0xC0 {
                fx.mem.write_u8(t + o, a + 0x40);
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
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x205580(t + 0x20) from a step's `jal` at `site` (frame 0x30: the
/// step's s1, s0 = t).
fn angles(fx: &mut Fx, t: u32, s1: u32, site: u32) {
    let base = fx.sp.wrapping_sub(0x30);
    fx.saved(base, &[(0x20, u64::from(site + 8)), (0x10, u64::from(s1)), (0, u64::from(t))]);
    l2::camera_angles_to(fx, t + 0x20);
}

/// 0x1E023A0 (D, task) (b615's ring column; never spawned here).
fn column(fx: &mut Fx, t: u32, k: u32) {
    let sp = enter_step(fx, 0x40);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let v = rmod(fx, t, 0x01E0_23C8, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, 0x01E0_23F4, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2410, 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = rand_mask(fx, t, 0x01E0_242C, 8);
        fx.mem.set16(t + 0xA, v);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        fx.mem.set16(t + 0x50, 0);
        let v = rmod(fx, t, 0x01E0_2454, 0x15E);
        fx.mem.set16(t + 0x52, v + 0x64);
        let v = rmod(fx, t, 0x01E0_2474, 0x14);
        fx.mem.set16(t + 0xE, v + 0xA);
        let v = rmod(fx, t, 0x01E0_2494, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_24B0, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x93 {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x20);
        }
        let r = fx.mem.i16(t + 0x50);
        if r < fx.mem.i16(t + 0x52) {
            fx.mem.set16(t + 0x50, r + 0xF);
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x63, a - 8);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(t + 0x54) == 0x87 {
        copy4(fx, t + 0x10, POS);
        let sv = |ra: u32| [u64::from(ra), 0, u64::from(k), u64::from(t - 0x2A0), u64::from(D), u64::from(t)];
        for ra in [0x01E0_2588u32, 0x01E0_259C, 0x01E0_25B0] {
            spawn(fx, RING, sv(ra));
        }
        for (o, site) in [(0x250u32, 0x01E0_25B0u32), (0x254, 0x01E0_25E4), (0x258, 0x01E0_2618)] {
            let r = rmod(fx, t, site, 0x1E);
            let v = ee::add(fx.mem.f(D + o), ee::from_i32(r - 0xF));
            fx.mem.setf(D + o, v);
        }
        if rmod(fx, t, 0x01E0_264C, 0xA) < 6 {
            spawn(fx, RADIAL, sv(0x01E0_2688));
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x30);
    let a = fx.mem.i16(t + 0x5C);
    let s = l2::sin_deg(fx, SINES, a, 1);
    fx.mem.setf(t + 0x34, ee::mul(3.0, s));
    let age_now = fx.mem.i16(t + 0x54);
    if (0x60..0x83).contains(&age_now) {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(t + 0x18, z);
        let lim = ee::sub(fx.mem.f(sp + 0x34), 10.0);
        let y = fx.mem.f(t + 0x14);
        if lim < y {
            fx.mem.setf(t + 0x14, ee::sub(y, 1.0));
        }
    }
    if fx.mem.i16(t + 0x54) >= 0x8D {
        let a = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, a);
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
        let y = ee::add(fx.mem.f(t + 0x14), 1.0);
        fx.mem.setf(t + 0x14, y);
        let g = ee::add(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, g);
    }
    let a = fx.mem.i16(t + 0x5C) + 6;
    fx.mem.set16(t + 0x5C, a);
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E027D0 / 0x1E02E90 (D, task): a ring (size `size`, cell 0..3,
/// colour `c`) of radius r0 + rand % m0 (+ rand % m1 + g0 a update when
/// `grow`), row 0..34, life 3..7; alpha 0x80, then out by 8. `k` = (m0,
/// r0, m1, g0); `sites` the seven rand `jal`s.
fn ring(fx: &mut Fx, t: u32, size: u32, k: (i32, i32, i32, i32), c: (i32, i32, i32), sites: [u32; 7], grow: bool) {
    let _sp = enter_step(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, size);
        let v = rand_mask(fx, t, sites[0], 4);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, c.0, c.1, c.2);
        let v = rmod(fx, t, sites[1], 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mask(fx, t, sites[2], 8);
        fx.mem.set16(t + 0xA, v);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        let v = rmod(fx, t, sites[3], k.0);
        fx.mem.set16(t + 0x50, v + k.1);
        let v = rmod(fx, t, sites[4], k.2);
        fx.mem.set16(t + 0xE, v + k.3);
        let v = rmod(fx, t, sites[5], 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, sites[6], 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_then_out(fx, t, 0x80, 0xA, 8);
    if grow && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02950 (D, task): a smoke ring on node 0x14 (size 10, cell 0..3,
/// orange) of radius 50..149 shrinking by 6..15, sinking 1.5 / rising 1.2
/// with the facing, turning by 5; alpha 0x80 for 3..7, then out by 8.
fn smoke(fx: &mut Fx, t: u32) {
    let _sp = enter_step(fx, 0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let v = rand_mask(fx, t, 0x01E0_2978, 4);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x40, 0x20);
        let v = rmod(fx, t, 0x01E0_29AC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_mask(fx, t, 0x01E0_29C8, 8);
        fx.mem.set16(t + 0xA, v);
        let w = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, w);
        let v = rmod(fx, t, 0x01E0_29EC, 0x64);
        fx.mem.set16(t + 0x50, v + 0x32);
        let v = rmod(fx, t, 0x01E0_2A0C, 0xA);
        fx.mem.set16(t + 0xE, v + 6);
        let v = rmod(fx, t, 0x01E0_2A2C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2A48, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_then_out(fx, t, 0x80, 0xA, 8);
    if fx.mem.i16(t + 0x54) != 0 {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 1.5) } else { ee::sub(z, f32::from_bits(0x3F99_999A)) };
        fx.mem.setf(t + 0x18, z);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) - fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
    let a = fx.mem.i16(t + 0x5A) + 5;
    fx.mem.set16(t + 0x5A, a);
    fx.leave(0x30);
}

/// 0x1E02B50 (D, task) (b144's flying quad): size 0x50, cell 0..7, flying
/// out along the angle +0x50 at 1.5 a update (sin / cos, x / y); both
/// alphas in by 0x40 for 4 updates, then out by 14 twice; growing by 15.
fn radial(fx: &mut Fx, t: u32) {
    let _sp = enter_step(fx, 0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x50);
        fx.mem.set16(t + 0xA, 0x50);
        let v = rand_mask(fx, t, 0x01E0_2B7C, 8);
        fx.mem.set16(t + 0x56, v);
        let a = fx.mem.i16(t + 0x50);
        let s = l2::sin_deg(fx, SINES, a, 1);
        fx.mem.setf(t + 0x30, ee::mul(1.5, s));
        let a = fx.mem.i16(t + 0x50);
        let c = l2::sin_deg(fx, SINES, a, 0);
        fx.mem.setf(t + 0x34, ee::mul(1.5, c));
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    in_then_out_twice(fx, t, 4);
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let v = fx.mem.i16(t + o) + 0xF;
            fx.mem.set16(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for (o, d) in [(0x10u32, 0x30u32), (0x14, 0x34)] {
            let v = ee::add(fx.mem.f(t + o), fx.mem.f(t + d));
            fx.mem.setf(t + o, v);
        }
    }
    angles(fx, t, D, 0x01E0_2CE0);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02D10 (D, task): a pulse quad (size 0x28, cell 3 / 5 alternating)
/// white / grey flickering on odd ages; both alphas in by 0x40 for 14
/// updates, then out by 14 twice.
fn pulse(fx: &mut Fx, t: u32) {
    let _sp = enter_step(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x28);
        fx.mem.set16(t + 0xA, 0x28);
        fx.mem.set16(t + 0x56, 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    in_then_out_twice(fx, t, 0xE);
    let c = if fx.mem.i16(t + 0x56) == 3 { 5 } else { 3 };
    fx.mem.set16(t + 0x56, c);
    let v = if fx.mem.i16(t + 0x54) % 2 == 1 { 0x80 } else { 0x70 };
    l2::rgb(fx, t + 0x60, v, v, v);
    angles(fx, t, 0, 0x01E0_2E68);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03030 (D, task) (b189's white arc with a violet (0xFF, 0xFF,
/// 0x40) outer): radius 220 growing by 1, spans 15 / 15, facing the
/// camera turned a quarter; in by 0x60 under 0xC0 for 7 updates, out by
/// 11.
fn arc(fx: &mut Fx, t: u32) {
    let _sp = enter_step(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x435C_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 7 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x60);
        }
    } else if a >= 0xD {
        fx.mem.write_u8(t + 0x63, a - 0xB);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = ee::add(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, v);
    }
    angles(fx, t, 0, 0x01E0_310C);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E03140 (D, task): a spark fan (radii 100 / 40, cell 0..7, spans
/// 15 / 15, random angles) widening by 35 a update; alpha 0x80 for 6
/// updates, then out by 9.
fn fan(fx: &mut Fx, t: u32) {
    let _sp = enter_step(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        fx.mem.setf(t + 0x4C, ee::sub(100.0, 58.0));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x50, 0);
        let v = rand_mask(fx, t, 0x01E0_31A0, 8);
        fx.mem.set16(t + 0x56, v);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.write_u32(t + 0x34, 0x43AF_0000);
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for (o, site) in [(0x20u32, 0x01E0_31F0u32), (0x24, 0x01E0_320C), (0x28, 0x01E0_3228)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 6 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, f32::from_bits(0x440E_8000)));
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E032D0 (D, task) (b611's flare): a violet ring of quads (size 10..29,
/// cell 0..3, (0xCF, 0x80, 0x80), random angles, +0x52 1 rising) of radius
/// 500..899 growing by 1..3, row 1..8, life 2..4; alpha 0x70, then out by
/// 9; turning by 2.
fn violet(fx: &mut Fx, t: u32) {
    let _sp = enter_step(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_32EC, 0x14);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = rand_mask(fx, t, 0x01E0_3318, 4);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0xCF, 0x80, 0x80);
        for (o, site) in [(0x20u32, 0x01E0_334Cu32), (0x24, 0x01E0_3368), (0x28, 0x01E0_3384)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_33A8, 0x190);
        fx.mem.set16(t + 0x50, v + 0x1F4);
        let v = rmod(fx, t, 0x01E0_33C8, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_33E4, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = rand_mask(fx, t, 0x01E0_3404, 8);
        fx.mem.set16(t + 8, v + 1);
        let v = rmod(fx, t, 0x01E0_3424, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_then_out(fx, t, 0x70, 0xB, 9);
    let a = fx.mem.i16(t + 0x5A) + 2;
    fx.mem.set16(t + 0x5A, a);
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E034E0 (D, task) (never spawned here): a widening ring band.
fn band(fx: &mut Fx, t: u32) {
    let _sp = enter_step(fx, 0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0xCF, 0xCF, 0xCF);
        l2::rgb(fx, t + 0x64, 0x80, 8, 8);
        let v = rmod(fx, t, 0x01E0_3528, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.write_u32(t + 0x20, 0x4000);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xD {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let r = ee::add(fx.mem.f(t + 0x48), 3.0);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::add(5.0, r));
    age(fx, t);
    fx.leave(0x20);
}
