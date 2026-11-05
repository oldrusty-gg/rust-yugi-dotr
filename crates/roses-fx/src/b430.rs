//! BMODULE b430 (card 577), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02FB0, data area 0x1ED0000, 0x37D0 bytes) [F]
//!: b505's older pool, 40 tasks of 0x40 bytes at
//! D+0x410 (alive +0x36, age +0x32, step +0x24 as a record address), on a
//! 400-update timer D+0xE80.
//!
//! The init lays out a lumpy shell (0x1E04A50): 20 rows of 18 points in the
//! tables X D+0x1CD4 / Y D+0x2274 / Z D+0x2814 (rows 0x48). The emitter
//! 0x1E01820: on 0x7F the shell task (0x1E02AE0: every cell of the shell
//! drawn as a textured strip about the attacker, cells appearing 8 a
//! update, flying apart from 0xEC, 0x1E03DB0), on 0xED the inner shell
//! (0x1E02970 / 0x1E04470); 0x7E..0xA2 two puffs an update rising from
//! D+0xE20 (0x1E01F20), on 0x55 a square at node 0x21 (0x1E02610 /
//! 0x1E01510), on 0xED sixteen puffs at nodes 0x54 / 0x55 (0x1E020B0);
//! 0xFA..0x159 smoke between the units (0x1E02310); 0x103..0x159 two kinds
//! at the target (0x1E02490, 0x1E027E0). Returns 1 on update 0xF0, 4 on
//! 0x168, 2 on 0x15E, 6 on 0xFB..0x158, 8 once the timer has run out.
//!
//! The steps 0x1E01E10, 0x1E02230, 0x1E02D20, 0x1E02EA0, 0x1E02F60 with the
//! draws 0x1E00920 / 0x1E00BB0 are tested by the entry but never spawned
//! [F: the emitter's spawns name 0x1E02AE0, 0x1E02970, 0x1E01F20, 0x1E02610,
//! 0x1E020B0, 0x1E02310, 0x1E02490, 0x1E027E0 only]; not ported.
use crate::blib::exe::libm_cosf;
use crate::blib::quads4;
use crate::blib::uv;
use crate::bus::Bus;
use crate::c061::{blend_alpha, desc_uv4, rgbaq_at, scale_tenth, to_degrees_neg};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0x28;
const POOL: u32 = D + 0x410;
const POS: u32 = D + 0x250;
const TIMER: u32 = D + 0xE80;
const COLOUR: u32 = D + 0xE84;
const OTW: u32 = D + 0xE8C;
const MEAN_W: u32 = D + 0xE98;
const NODES: u32 = D + 0xEA8;
const COUNTER: u32 = D + 0x3358;
const SIDE: u32 = D + 0x335A;
const BYTES_A: u32 = D + 0x335C;
const BYTES_B: u32 = D + 0x345C;
const BYTES_C: u32 = D + 0x355C;
const TAB: u32 = D + 0x365C;
const X: u32 = D + 0x1CD4;
const Y: u32 = D + 0x2274;
const Z: u32 = D + 0x2814;

const PUFF: u32 = 0x01E0_1F20;
const BURST: u32 = 0x01E0_20B0;
const SMOKE: u32 = 0x01E0_2310;
const SPARK: u32 = 0x01E0_2490;
const SQUARE: u32 = 0x01E0_2610;
const CHIP: u32 = 0x01E0_27E0;
const INNER: u32 = 0x01E0_2970;
const SHELL: u32 = 0x01E0_2AE0;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn words(fx: &mut Fx, to: u32, from: u32, n: u32) {
    for c in 0..n {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

/// 0x1A2C70 with its residue frame.
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// 0x1A2B80 with its residue frame.
fn fixs(fx: &mut Fx, f: f32, ra: u32) -> i32 {
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}

/// The entry 0x1E02FB0 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1C0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x1C0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x37D0);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3356, param);
    fx.mem.set32(TIMER, 0x190);
    for (k, node) in [0x54, 0x55, 0x21, 0x37, 0x1F, 0x34, 0x36, 0x37].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x408 + 4 * k, t);
    }
    for k in 0..4u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xE60 + 4 * k, t);
    }
    shell_init(fx);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x110);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x114)), 180.0);
    let y = fixs(fx, y, 0x01E0_3170);
    fx.mem.set16(SIDE, y);
    for o in [0x3A0u32, 0x3B0, 0x3A4, 0x3B4, 0x3A8, 0x3B8] {
        fx.mem.write_u32(D + o, 0x3DA3_D70A);
    }
    fx.mem.set16(D + 0xEC0, 0);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x70);
    let x = fx.mem.f(sp + 0x70);
    fx.mem.setf(D + 0xE10, x);
    let y = ee::add(5.0, fx.mem.f(sp + 0x74));
    fx.mem.setf(D + 0xE14, y);
    let z = fx.mem.f(sp + 0x78);
    fx.mem.setf(D + 0xE18, z);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x80);
    let f = ee::div(ee::sub(fx.mem.f(sp + 0x78), fx.mem.f(sp + 0x88)), 50.0);
    fx.mem.setf(sp + 0x78, f);
    let z = ee::sub(fx.mem.f(D + 0xE18), f);
    fx.mem.setf(D + 0xE18, z);
    words(fx, D + 0xE20, D + 0xE10, 3);
    let y = ee::sub(fx.mem.f(D + 0xE24), 14.0);
    fx.mem.setf(D + 0xE24, y);
    for k in 0..0x168u32 {
        let v = rmod(fx, D, 0x01E0_3250, 0x64);
        fx.mem.setf(D + 0x2DB4 + 4 * k, ee::from_i32(v + 3));
        let v = rmod(fx, D, 0x01E0_327C, 0x64);
        fx.mem.set16(D + 0x1194 + 2 * k, v - 0x32);
        let v = rmod(fx, D, 0x01E0_329C, 0x64);
        fx.mem.set16(D + 0x1464 + 2 * k, v - 0x32);
        let h = fx.self_handle();
        fx.unit_position(h, sp + 0x70);
        let t = fx.target_handle();
        fx.unit_position(t, sp + 0x80);
        let v = if fx.mem.i16(SIDE) != 0 {
            let r = rmod(fx, D, 0x01E0_32F0, 0x190);
            -(r + 0xAA)
        } else {
            rmod(fx, D, 0x01E0_3328, 0xC8) + 0x10E
        };
        fx.mem.setf(D + 0x2DB4 + 4 * k, ee::from_i32(v));
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3370, 0xC);
        fx.mem.set8(BYTES_A + k, v - 6);
    }
    for k in 0x64..0xB4u32 {
        fx.mem.write_u8(BYTES_A + k, 0);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_33C4, 0xC);
        fx.mem.set8(BYTES_B + k, v - 6);
    }
    for k in 0xB4..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_33F8);
        fx.mem.set8(BYTES_B + k, v % 4 - 2);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3434, 0x1E);
        fx.mem.set8(BYTES_C + k, v - 0xF);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let (f, ra, size): (fn(&mut Fx, u32), u32, u32) = match fx.mem.read_u32(t + 0x24) {
            PUFF => (puff, 0x01E0_34C8, 0x20),
            BURST => (burst, 0x01E0_34E8, 0x20),
            SMOKE => (smoke, 0x01E0_3528, 0x20),
            SPARK => (spark, 0x01E0_3548, 0x20),
            SQUARE => (square, 0x01E0_3568, 0x30),
            CHIP => (chip, 0x01E0_3588, 0x20),
            INNER => (inner, 0x01E0_35A8, 0x30),
            SHELL => (shell, 0x01E0_35C8, 0x30),
            _ => continue,
        };
        // The step's frame: the saves a stale TEX0 / TEX1 of a missing
        // texture reads back [run].
        let q = fx.enter(size);
        fx.saved(q, &[(0, u64::from(D)), (size - 0x10, u64::from(ra))]);
        if size == 0x30 {
            fx.saved(q + 0x10, &[(0, 0)]);
        }
        f(fx, t);
        fx.leave(size);
    }
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            PUFF => {
                cell(fx, sp + 0x90 + 0x10, t, 0xE60, 0x14);
                words(fx, sp + 0x70, D + 0xE20, 3);
                fx.mem.set16(D + 0xB8, 4);
                fx.mem.set16(D + 0xBE, 0);
                ring_plain(fx, t, BYTES_A, sp + 0x70);
                fx.mem.set16(D + 0xB8, 0);
            }
            BURST => {
                cell(fx, sp + 0xB0, t, 0xE60, 0x14);
                at_task(fx, sp, t);
                fx.mem.set16(D + 0xB8, 6);
                fx.mem.set16(D + 0xBE, 0);
                ring_plain(fx, t, BYTES_B, sp + 0x70);
                fx.mem.set16(D + 0xB8, 0);
            }
            SMOKE | SPARK => {
                let (r, n) = if fx.mem.read_u32(t + 0x24) == SMOKE { (0xD0, 7) } else { (0xE0, 3) };
                cell(fx, sp + r, t, 0xE68, 0x14);
                at_task(fx, sp, t);
                fx.mem.set16(D + 0xB8, n);
                fx.mem.set16(D + 0xBE, 0);
                quads4::ring_at(fx, uv::uv_rect, D, t, BYTES_C, sp + 0x70);
                fx.mem.set16(D + 0xB8, 0);
            }
            SQUARE => {
                cell(fx, sp + 0xF0, t, 0xE6C, 0x14);
                words(fx, sp + 0x70, D + 0x3C0, 3);
                fx.mem.set16(D + 0xBE, 0);
                square_draw(fx, t, sp + 0x70);
            }
            CHIP => {
                cell(fx, sp + 0x100, t, 0xE64, 0x14);
                at_task(fx, sp, t);
                fx.mem.set16(D + 0xB8, 3);
                let o = fx.mem.i16(t + 0xC);
                fx.mem.set16(D + 0xBE, o);
                quads4::ring_at(fx, uv::uv_rect, D, t, BYTES_C, sp + 0x70);
                fx.mem.set16(D + 0xB8, 0);
            }
            INNER => mesh(fx, t, sp + 0x128, true),
            SHELL => mesh(fx, t, sp + 0x128, false),
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xF0 => 1,
        0x168 => 4,
        0x15E => 2,
        0xFB..=0x158 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// Texture D+`tex` into D+0x88, the atlas cell `id` frame +0x34 (0x106960
/// into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, t: u32, tex: u32, id: i32) {
    let w = fx.mem.read_u32(D + tex);
    fx.mem.write_u32(D + 0x88, w);
    let sub = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, sub);
    words(fx, D + 0x94, r, 4);
}

/// sp + 0x70 = the task's i16 position as floats.
fn at_task(fx: &mut Fx, sp: u32, t: u32) {
    for k in 0..3 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0x70 + 4 * k, v);
    }
}

/// 0x1E03C90(D, step, 0): the first free task cleared and set off at
/// D+0x250 (as i16) with the colour D+0xE84 (twice), +0x30 = D+0xEBE.
fn spawn(fx: &mut Fx, f: u32) {
    fx.enter(0x60);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
        let t = POOL + 0x40 * k;
        fx.bzero(t, 0x40);
        fx.mem.set16(t + 0x36, 1);
        for c in [0x28u32, 0x2C] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + c + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let a = fx.mem.i16(D + 0xEBE);
        fx.mem.set16(t + 0x30, a);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, f);
    }
    fx.leave(0x60);
}

/// The node slot `slot` of the attacker into `out`.
fn node(fx: &mut Fx, slot: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i16(D + slot);
    fx.node_position(h, s, out);
}

/// The attacker's and the target's positions into sp + 0x50 / 0x60.
fn units(fx: &mut Fx, sp: u32) {
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x50);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x60);
}

/// D+0x250.. = sp + 0x60.. each + rand % 30 - 15.
fn near_target(fx: &mut Fx, sp: u32, ras: [u32; 3]) {
    for (k, ra) in ras.into_iter().enumerate() {
        let k = k as u32;
        let r = rmod(fx, 0, ra, 0x1E);
        let f = ee::sub(ee::add(fx.mem.f(sp + 0x60 + 4 * k), ee::from_i32(r)), 15.0);
        fx.mem.setf(POS + 4 * k, f);
    }
}

/// 0x1E01820(D, 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0, u64::from(D)), (0x10, 0), (0x20, u64::MAX), (0x30, 0), (0x40, 0x01E0_3470)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    if c == 0x7F {
        spawn(fx, SHELL);
    }
    if c == 0xED {
        spawn(fx, INNER);
    }
    if (0x7E..0xA3).contains(&c) {
        spawn(fx, PUFF);
        spawn(fx, PUFF);
    }
    if c == 0x55 {
        node(fx, 0xEAC, sp + 0x50);
        node(fx, 0xEAC, D + 0x3C0);
        let y = ee::add(fx.mem.f(D + 0x3C4), f32::from_bits(0x3E19_999A));
        fx.mem.setf(D + 0x3C4, y);
        words(fx, POS, sp + 0x50, 3);
        spawn(fx, SQUARE);
    }
    if (0x7F..0xAB).contains(&c) {
        let y = ee::add(fx.mem.f(D + 0xE24), f32::from_bits(0x3F0C_CCCD));
        fx.mem.setf(D + 0xE24, y);
    }
    if c == 0xED {
        for slot in [0xEA8u32, 0xEAA] {
            node(fx, slot, sp + 0x50);
            words(fx, POS, sp + 0x50, 3);
            for _ in 0..8 {
                spawn(fx, BURST);
            }
        }
    }
    if c >= 0x7F {
        let v = fx.mem.i16(D + 0xEC0);
        fx.mem.set16(D + 0xEC0, if v < 0x157 { v + 8 } else { 0x156 });
    }
    if (0xFA..0x15A).contains(&c) {
        units(fx, sp);
        for k in 0..3u32 {
            let f = ee::div(ee::sub(fx.mem.f(sp + 0x60 + 4 * k), fx.mem.f(sp + 0x50 + 4 * k)), 65.0);
            fx.mem.setf(sp + 0x60 + 4 * k, f);
        }
        let r = rmod(fx, sp + 0x68, 0x01E0_1B40, 0x50);
        let f = ee::sub(ee::add(fx.mem.f(sp + 0x50), ee::from_i32(r)), 40.0);
        fx.mem.setf(POS, f);
        let r = rmod(fx, sp + 0x68, 0x01E0_1B74, 0x50);
        let f = ee::add(5.0, fx.mem.f(sp + 0x54));
        let f = ee::sub(ee::add(f, ee::from_i32(r)), 40.0);
        fx.mem.setf(POS + 4, f);
        let f2 = ee::from_i32(fx.mem.i16(D + 0x3354));
        let f = ee::add(fx.mem.f(sp + 0x58), ee::mul(fx.mem.f(sp + 0x68), f2));
        fx.mem.setf(POS + 8, f);
        spawn(fx, SMOKE);
    }
    if (0xEB..0x15D).contains(&c) {
        let v = fx.mem.i16(D + 0x3354) + 1;
        fx.mem.set16(D + 0x3354, v);
    }
    if (0x103..0x15A).contains(&c) {
        units(fx, sp);
        near_target(fx, sp, [0x01E0_1C58, 0x01E0_1C8C, 0x01E0_1CC4]);
        spawn(fx, SPARK);
        units(fx, sp);
        near_target(fx, sp, [0x01E0_1D3C, 0x01E0_1D70, 0x01E0_1DA4]);
        spawn(fx, CHIP);
    }
    fx.leave(0x70);
}

fn enter_step(fx: &mut Fx, t: u32, size: i32, grey: i32) -> bool {
    if fx.mem.i16(t + 0x36) != 1 {
        return false;
    }
    fx.mem.set16(t + 0x38, size);
    l2::rgb(fx, t + 0x28, grey, grey, grey);
    true
}

/// [+0x2B] up by `up` while below `cap`.
fn fade_in(fx: &mut Fx, t: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a < cap {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
    }
}

/// [+0x2B] down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a - down);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

/// The steps' common randoms: +0x30 = rand % 0x28A + 0x96, +0x3A = rand %
/// 0x168, +0xE = rand % 0x3C + 0x41, +8 = rand % 0x1E (at `ra` + 0x20 / ...).
fn randoms(fx: &mut Fx, t: u32, ras: [u32; 4]) {
    let v = rmod(fx, t, ras[0], 0x28A);
    fx.mem.set16(t + 0x30, v + 0x96);
    let v = rmod(fx, t, ras[1], 0x168);
    fx.mem.set16(t + 0x3A, v);
    let v = rmod(fx, t, ras[2], 0x3C);
    fx.mem.set16(t + 0xE, v + 0x41);
    let v = rmod(fx, t, ras[3], 0x1E);
    fx.mem.set16(t + 8, v);
}

/// +0x30 += +0xE once the task has aged.
fn grow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
        fx.mem.set16(t + 0x30, v);
    }
}

/// +0x34 up to 6 once the task has aged.
fn frames(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x32) != 0 {
        let v = fx.mem.i16(t + 0x34);
        if v < 6 {
            fx.mem.set16(t + 0x34, v + 1);
        }
    }
}

/// 0x1E01F20.
fn puff(fx: &mut Fx, t: u32) {
    if enter_step(fx, t, 0x41, 0xF0) {
        let v = rmod(fx, t, 0x01E0_1F60, 0xFA);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, 0x01E0_1F7C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, 0x01E0_1F98, 5);
        fx.mem.set16(t + 0xE, v + 0xF);
        let v = rmod(fx, t, 0x01E0_1FB8, 0x1E);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1FD4, 7);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x81, 0x40);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    grow(fx, t);
    let v = fx.rand_from(t, 0x01E0_206C);
    fx.mem.set16(t + 0x34, v % 8);
    age(fx, t);
}

/// 0x1E020B0.
fn burst(fx: &mut Fx, t: u32) {
    if enter_step(fx, t, 0x5F, 0xFF) {
        let v = rmod(fx, t, 0x01E0_20F0, 0xFA);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, 0x01E0_210C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, 0x01E0_2128, 0x3C);
        fx.mem.set16(t + 0xE, v + 0x41);
        let v = rmod(fx, t, 0x01E0_2148, 0x1E);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2164, 7);
        fx.mem.set16(t + 6, v + 6);
        let v = fx.rand_from(t, 0x01E0_2184);
        fx.mem.set16(t + 0x34, v % 8);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x80, 0x80);
    } else {
        fade_out(fx, t, 0xC, 0xA);
    }
    grow(fx, t);
    age(fx, t);
}

/// 0x1E02310.
fn smoke(fx: &mut Fx, t: u32) {
    if enter_step(fx, t, 0x14, 0x60) {
        randoms(fx, t, [0x01E0_2350, 0x01E0_2370, 0x01E0_238C, 0x01E0_23AC]);
        let v = rmod(fx, t, 0x01E0_23C8, 7);
        fx.mem.set16(t + 6, v + 6);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x80, 0x80);
    } else {
        fade_out(fx, t, 0x16, 0x14);
    }
    frames(fx, t);
    age(fx, t);
}

/// 0x1E02490.
fn spark(fx: &mut Fx, t: u32) {
    if enter_step(fx, t, 0x28, 0x40) {
        randoms(fx, t, [0x01E0_24D0, 0x01E0_24F0, 0x01E0_250C, 0x01E0_252C]);
        let v = rmod(fx, t, 0x01E0_2548, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x80, 0x19);
    } else {
        fade_out(fx, t, 0x16, 0x14);
    }
    frames(fx, t);
    age(fx, t);
}

/// 0x1E02610.
fn square(fx: &mut Fx, t: u32) {
    if enter_step(fx, t, 8, 0x80) {
        randoms(fx, t, [0x01E0_2658, 0x01E0_2678, 0x01E0_2694, 0x01E0_26B4]);
        fx.mem.set16(t + 6, 8);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x80, 0x20);
    } else {
        fade_out(fx, t, 0xC, 0xA);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let z = fx.mem.f(D + 0x3C8);
        let z = if fx.mem.i16(SIDE) != 0 { ee::sub(z, 0.5) } else { ee::add(z, 0.5) };
        fx.mem.setf(D + 0x3C8, z);
        let y = ee::add(fx.mem.f(D + 0x3C4), f32::from_bits(0x3DCC_CCCD));
        fx.mem.setf(D + 0x3C4, y);
        let s = fx.mem.i16(t + 0x38) + 1;
        fx.mem.set16(t + 0x38, s);
    }
    age(fx, t);
}

/// 0x1E027E0.
fn chip(fx: &mut Fx, t: u32) {
    if enter_step(fx, t, 0x14, 0x40) {
        randoms(fx, t, [0x01E0_2820, 0x01E0_2840, 0x01E0_285C, 0x01E0_287C]);
        let v = rmod(fx, t, 0x01E0_2898, 5);
        fx.mem.set16(t + 6, v + 3);
        let v = fx.rand_from(t, 0x01E0_28B8);
        fx.mem.set16(t + 0x34, v % 8);
        let v = fx.rand_from(t, 0x01E0_28D4);
        fx.mem.set16(t + 0xC, v % 8);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x80, 0x19);
    } else {
        fade_out(fx, t, 0xC, 0xA);
    }
    age(fx, t);
}

/// 0x1E02970.
fn inner(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        l2::rgb(fx, t + 0x28, 0x50, 0x50, 0x78);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, 0x01E0_29B8, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(COUNTER) >= 0xF3 {
        fade_out(fx, t, 8, 6);
    }
    if fx.mem.i16(COUNTER) >= 0xED {
        if fx.mem.i16(t + 0x32) % 2 == 1 {
            l2::rgb(fx, t + 0x28, 0x50, 0x50, 0x78);
        } else {
            l2::rgb(fx, t + 0x28, 0xA0, 0xA0, 0xA0);
        }
        let v = fx.mem.i16(t + 0x30) + 1;
        fx.mem.set16(t + 0x30, v);
        for o in [0x3B0u32, 0x3B4, 0x3B8] {
            let f = ee::add(fx.mem.f(D + o), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + o, f);
        }
    }
    age(fx, t);
}

/// 0x1E02AE0.
fn shell(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        l2::rgb(fx, t + 0x28, 0x50, 0x50, 0x78);
        let v = rmod(fx, t, 0x01E0_2B28, 0xB4);
        fx.mem.set16(t + 0x30, v);
        let v = rmod(fx, t, 0x01E0_2B44, 0x168);
        fx.mem.set16(t + 0x3A, v);
        fx.mem.write_u8(t + 0x2B, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(COUNTER) >= 0x15A {
        fade_out(fx, t, 8, 6);
    }
    if fx.mem.i16(COUNTER) == 0xED {
        l2::rgb(fx, t + 0x28, 0xA0, 0xA0, 0xF0);
    }
    age(fx, t);
    if fx.mem.i16(COUNTER) < 0xEC {
        return;
    }
    let v = fx.mem.i16(D + 0xEC2);
    if v < 0x156 {
        fx.mem.set16(D + 0xEC2, v + 0x156);
    }
    if fx.mem.i16(D + 0xEC2) >= 0x157 {
        fx.mem.set16(D + 0xEC2, 0x156);
    }
    if fx.mem.i16(COUNTER) >= 0x100 {
        for o in [0x3A0u32, 0x3A4, 0x3A8] {
            let f = fx.mem.f(D + o);
            if f64::from(f) < f64::from_bits(0x3FC3_3333_3333_3333) {
                fx.mem.setf(D + o, ee::add(f, f32::from_bits(0x3C23_D70A)));
            }
        }
    }
}

/// 0x1E00DA0 (D, task, table, pos): [`quads4::ring_at`] without the offset's
/// turn about z (frame 0x200): the offsets go straight into the columns.
fn ring_plain(fx: &mut Fx, t: u32, table: u32, pos: u32) {
    let sp = fx.enter(0x200);
    words(fx, sp + 0x90, pos, 4);
    let d = sp + 0x168;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha(2));
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, sp + 0x1F8, t + 0x28, d + 0x10);
    uv::uv_rect(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    let count = match fx.mem.i16(D + 0xB8) {
        0 => 0x14,
        n => n,
    };
    scale_tenth(fx, sp + 0xF0);
    l2::camera_angles_to(fx, sp + 0x150);
    words(fx, sp + 0xE0, sp + 0x90, 3);
    l2::xform_vp(fx, sp + 0x110, sp + 0xF0, 0, sp + 0xE0);
    to_degrees_neg(fx, sp + 0x154);
    to_degrees_neg(fx, sp + 0x150);
    let tab = D + 0xD8;
    for k in 0..count {
        let size = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0xA0, size, size);
        l2::rotate4(fx, sp + 0xA0, sp + 0x150, 0, tab);
        l2::rotate4(fx, sp + 0xA0, sp + 0x150, 1, tab);
        let row = table.wrapping_add(k as u32);
        let half = k >> 1;
        for (axis, off) in [(0, 0u32), (1, 0x64), (2, 0xB4)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::div(ee::from_i32(fx.mem.i8(row.wrapping_add(i as u32).wrapping_add(off))), 20.0);
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + half);
            l2::add_column(fx, sp + 0xA0, axis, ee::mul(b, r));
        }
        if l2::project(fx, sp + 0xA0, SPR, sp + 0x110, d, sp + 0x100) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x100)) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x200);
}

/// 0x1E01510 (D, task, 0, pos) (b257's 0x1E01880 `square`): four strip3
/// quads of size +0x38 at `pos` turned by the camera's angles (scale
/// 0.05), colours +0x28, keyed by mean w.
fn square_draw(fx: &mut Fx, t: u32, pos: u32) {
    let sp = fx.enter(0x1E0);
    words(fx, sp + 0x60, pos, 4);
    let d = sp + 0x130;
    l2::desc_strip3(fx, d);
    for (local, to) in [(0x1C0u32, 0x10u32), (0x1C8, 0x28), (0x1D0, 0x40), (0x1D8, 0x58)] {
        rgbaq_at(fx, sp + local, t + 0x28, d + to);
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xC0 + 4 * k, 0x3D4C_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x120);
    words(fx, sp + 0xB0, sp + 0x60, 3);
    l2::xform_vp(fx, sp + 0xE0, sp + 0xC0, sp + 0x120, sp + 0xB0);
    let quad = sp + 0x70;
    for _ in 0..4 {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, quad, s, s);
        l2::rotate4(fx, quad, sp + 0x120, 2, D + 0xD8);
        if l2::project(fx, quad, SPR, sp + 0xE0, d, sp + 0xD0) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0xD0)) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x1E0);
}

/// v += c (0x107430).
fn add3(fx: &mut Fx, v: u32, c: u32) {
    for k in 0..3u32 {
        let f = ee::add(fx.mem.f(v + 4 * k), fx.mem.f(c + 4 * k));
        fx.mem.setf(v + 4 * k, f);
    }
}

/// v *= s (0x107770).
fn mul3(fx: &mut Fx, v: u32, s: u32) {
    for k in 0..3u32 {
        let f = ee::mul(fx.mem.f(v + 4 * k), fx.mem.f(s + 4 * k));
        fx.mem.setf(v + 4 * k, f);
    }
}

/// 0x1E03DB0 (`inner` false, frame 0x1A0) / 0x1E04470 (true, frame 0x190)
/// (D, task, desc, 0): every cell i < D+0xEC0 of the shell as a strip3 quad
/// at the descriptor `d` (atlas cell 0x9C shrunk by 2, colours +0x28,
/// texture D+0x40C): its corners relative to the first (scaled by D+0x3A0
/// / D+0x3B0), from 0xEC turned by the i16 tables D+0x1734 / D+0x1A04, at
/// D+0xE10 + the first corner x D+0x3A0; the shell's cells then fly along
/// their random vectors by +D+0xEC4, the inner shell's spread by +0x30.
fn mesh(fx: &mut Fx, t: u32, d: u32, inner: bool) {
    let frame = if inner { 0x190 } else { 0x1A0 };
    let sp = fx.enter(frame);
    // The locals of the two functions are 0x10 apart.
    let o = if inner { sp - 0x10 } else { sp };
    let r = o + 0x130;
    l2::rect(fx, r, 0x9C, 0);
    for (a, v) in [(0u32, 2), (2, 2), (4, -2), (6, -2)] {
        let x = fx.mem.i16(r + a) + v;
        fx.mem.set16(r + a, x);
    }
    l2::desc_strip3(fx, d);
    for (k, to) in [0x10u32, 0x28, 0x40, 0x58].into_iter().enumerate() {
        rgbaq_at(fx, o + 0x180 + 8 * k as u32, t + 0x28, d + to);
    }
    l2::uv_strip3(fx, d, r, 0);
    let tex = fx.mem.read_u32(D + 0x40C);
    fx.desc_texture(d, tex, 1);
    let h = fx.self_handle();
    fx.unit_position(h, o + 0x150);
    let h = fx.target_handle();
    fx.unit_position(h, o + 0x160);
    let f = ee::div(ee::neg(ee::sub(fx.mem.f(o + 0x158), fx.mem.f(o + 0x168))), 60.0);
    fx.mem.setf(o + 0x158, f);
    let q = [o + 0xF0, o + 0x100, o + 0x110, o + 0x120];
    let c = o + 0x170;
    let rot = o + 0x140;
    let ra = if inner { [0x01E0_47FC, 0x01E0_4860, 0x01E0_49C4] } else { [0x01E0_4140, 0x01E0_41A4, 0x01E0_4340] };
    for row in 1..0x13u32 {
        for col in 0..0x12u32 {
            let i = (row - 1) * 0x12 + col;
            let s1 = 0x48 * row + 4 * col;
            let w = if col == 0x11 { 0x48 * row } else { s1 + 4 };
            for (k, tab) in [X, Y, Z].into_iter().enumerate() {
                let v = fx.mem.read_u32(tab + s1);
                fx.mem.write_u32(c + 4 * k as u32, v);
            }
            if (i as i32) < fx.mem.i16(D + 0xEC0) {
                for (k, tab) in [X, Y, Z].into_iter().enumerate() {
                    let k = k as u32;
                    let cc = fx.mem.f(c + 4 * k);
                    for (v, at) in q.into_iter().zip([s1, w, s1 + 0x48, w + 0x48]) {
                        let f = ee::sub(fx.mem.f(tab + at), cc);
                        fx.mem.setf(v + 4 * k, f);
                    }
                }
                mul3(fx, c, D + 0x3A0);
                for v in q {
                    mul3(fx, v, D + if inner { 0x3B0 } else { 0x3A0 });
                }
                if fx.mem.i16(COUNTER) >= 0xEC {
                    let a = ee::from_i32(fx.mem.i16(D + 0x1734 + 2 * i));
                    let a = fixs(fx, a, ra[0]);
                    fx.mem.set32(rot + 4, a);
                    for v in q {
                        l2::rotate_deg(fx, v, rot, 0, TAB);
                    }
                    let a = ee::from_i32(fx.mem.i16(D + 0x1A04 + 2 * i));
                    let a = fixs(fx, a, ra[1]);
                    fx.mem.set32(rot, a);
                    for v in q {
                        l2::rotate_deg(fx, v, rot, 1, TAB);
                    }
                }
                for k in 0..3u32 {
                    let f = fx.mem.f(D + 0xE10 + 4 * k);
                    l2::add_column(fx, q[0], k as i32, f);
                }
                for v in q {
                    add3(fx, v, c);
                }
                if inner {
                    let y = ee::div(fx.mem.f(c + 4), 5.0);
                    fx.mem.setf(c + 4, y);
                    for k in 0..3u32 {
                        let r = ee::from_i32(fx.mem.i16(t + 0x30));
                        let f = ee::mul(fx.mem.f(c + 4 * k), r);
                        l2::add_column(fx, q[0], k as i32, f);
                    }
                } else {
                    let n = fx.mem.i16(D + 0xEC4 + 2 * i);
                    let f = ee::div(ee::mul(fx.mem.f(D + 0x2DB4 + 4 * i), ee::from_i32(n)), 100.0);
                    l2::add_column(fx, q[0], 2, f);
                    let a = fx.mem.i16(D + 0x1194 + 2 * i);
                    let n = fx.mem.i16(D + 0xEC4 + 2 * i);
                    l2::add_column(fx, q[0], 0, ee::from_i32(a.wrapping_mul(n) / 100));
                    let a = fx.mem.i16(D + 0x1464 + 2 * i);
                    let n = fx.mem.i16(D + 0xEC4 + 2 * i);
                    l2::add_column(fx, q[0], 1, ee::from_i32(a.wrapping_mul(n) / 100));
                }
                if l2::project(fx, q[0], SPR, 0, d, MEAN_W) {
                    let w = fx.mem.f(MEAN_W);
                    let key = fixu(fx, w, ra[2]);
                    let ot = fx.mem.read_u32(OTW);
                    fx.submit(d, ot, key as i32);
                }
            }
            if !inner && fx.mem.i16(COUNTER) >= 0xEC && (i as i32) < fx.mem.i16(D + 0xEC2) {
                let n = fx.mem.i16(D + 0xEC4 + 2 * i) + 1;
                fx.mem.set16(D + 0xEC4 + 2 * i, n);
                let (a, b) = if i < 0x64 {
                    (0xE, 8)
                } else if i < 0xC8 {
                    (-0xE, 8)
                } else {
                    (0xE, -8)
                };
                let v = fx.mem.i16(D + 0x1734 + 2 * i) + a;
                fx.mem.set16(D + 0x1734 + 2 * i, v);
                let v = fx.mem.i16(D + 0x1A04 + 2 * i) + b;
                fx.mem.set16(D + 0x1A04 + 2 * i, v);
            }
        }
    }
    fx.leave(frame);
}

/// 0x1E04A50(D, 0): the shell's tables.
fn shell_init(fx: &mut Fx) {
    let sp = fx.enter(0xD0);
    let i16w = |v: i32| v as i16 as i32;
    let mut radius = 0i32;
    for row in 0..0x14i32 {
        let mut deg = 0i32;
        for col in 0..0x12u32 {
            let at = 0x48 * row as u32 + 4 * col;
            let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), ee::from_i32(deg)), 180.0);
            let c = libm_cosf(fx, a);
            fx.mem.setf(X + at, ee::mul(ee::from_i32(radius), c));
            let s = fx.libm_sinf(a);
            fx.mem.setf(Z + at, ee::mul(ee::from_i32(radius), s));
            deg = i16w(deg + 0x14);
        }
        radius = i16w(if row < 9 { radius + 0xF } else { radius - 5 });
        if row > 0 && row < 0xF {
            radius = rmod(fx, row as u32, 0x01E0_4B64, 0xF) + 0x19;
        }
        if radius >= 0x64 {
            radius = 0x64;
        }
        if radius <= 0 {
            radius = 0;
        }
        if row == 0 || row >= 0x12 {
            radius = 0;
        }
    }
    let mut s7 = 1i32;
    for row in 0..0x14i32 {
        let a = i16w(rmod(fx, 0, 0x01E0_4C10, 0xA) - 5);
        let b = i16w(rmod(fx, a as u32, 0x01E0_4C34, 0xA) - 5);
        let s5 = s7;
        let s2 = s5 >> 1;
        for col in 0..0x12u32 {
            let at = 0x48 * row as u32 + 4 * col;
            let r0 = fx.rand_from(0, 0x01E0_4C70);
            let r0 = i16w(fx.div(r0, s5).1);
            let r1 = fx.rand_from(r0 as u32, 0x01E0_4C8C);
            let r1 = i16w(fx.div(r1, s5).1);
            let f = ee::add(fx.mem.f(X + at), ee::from_i32(a + (r0 - s2)));
            fx.mem.setf(X + at, f);
            let f = ee::add(fx.mem.f(Z + at), ee::from_i32(b + (r1 - s2)));
            fx.mem.setf(Z + at, f);
        }
        if row < 0xA {
            if s5 < 0x1E {
                s7 = i16w(s7 + 3);
            }
            if row >= 2 {
                s7 = rmod(fx, 0, 0x01E0_4D2C, 0xA) + 2;
            }
        } else {
            if s5 >= 9 {
                s7 = i16w(s7 - 7);
            }
            if row < 0x12 {
                s7 = rmod(fx, 0, 0x01E0_4D7C, 0xA) + 2;
            }
        }
        if row == 0x12 {
            s7 = 1;
        }
        if s7 <= 0 {
            s7 = 1;
        }
    }
    let mut base = -0xA0i32;
    for row in 0..0x14i32 {
        for col in 0..0x12u32 {
            let at = 0x48 * row as u32 + 4 * col;
            let v = rmod(fx, base as u32, 0x01E0_4E20, 0x14);
            fx.mem.setf(Y + at, ee::from_i32(base + v - 0xA));
        }
        if row < 0x13 {
            base = i16w(base + 0x10);
        }
    }
    fx.mem.write_u32(sp, 0);
    fx.leave(0xD0);
}
