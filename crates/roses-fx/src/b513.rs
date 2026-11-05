//! BMODULE b513 (card 499), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02690, data area 0x1ED0000, 0x3960 bytes) [F]
//!: b393's (card 513, b393.rs) program on its
//! own globals (200 tasks of 0x40 bytes at D+0x2E0, alive +0x36, step
//! +0x24 by direct `jal`; spawn 0x1E02E60 = b018's on colour D+0x36A4,
//! point D+0x250, parameter D+0x36E2). Attacker nodes 0x1D / 0x62 / 0x20 /
//! 0x68 / 0xA5 / 0xB0 (D+0x36C8..), the target's 1 (D+0x36D6); timer
//! 0x10C; no histories.
//!
//! The emitter 0x1E01450 counts D+0x36E6: below 0xB3 two red streak rings
//! a update at node 0x1D (0x1E01880), below 0xCC two at node 0x62
//! (0x1E021D0), both drawn by b052's 0x1E03730 copy (0x1E02F80); on the
//! updates 0x12 / 0x2C / 0x4B / 0x65 / 0x83 / 0x98 sixteen grey smoke
//! sprites (0x1E01A10, b225's, `billboard::sprite_square` pushed by +8 /
//! +0xC over 10) at node 0xB0, on 0x1F / 0x3A / 0x57 / 0x72 / 0x8F sixteen
//! at node 0xA5; on 0xB2 (node 0x20) and 0xCB (node 0x68) eight ember
//! rings 0x1E02050 (`ring_t20`), eight red rings 0x1E01EB0 (`ring_t20` on
//! cell 0xA), thirty dark red rings 0x1E01D00 (`ring_t`), a glow disc
//! 0x1E01B70 (`fan18`) and b018's camera light 0x1E02360 (lamp D+0x34E0).
//! Returns 1 on update 0xA8, 4 on 0xFA, 2 on 0xDC, 6 on 0xB3..0xD6, 8 once
//! the timer D+0x36A0 has run out. The steps 0x1E01C50 (inline) /
//! 0x1E024E0 / 0x1E025A0 (inline) are tested but never spawned (not
//! ported). No 0x1F8100.
use crate::b076::node_position_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::c061::{desc_uv4, rgbaq_at};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 200;

const STREAK_A: u32 = 0x01E0_1880;
const SMOKE: u32 = 0x01E0_1A10;
const GLOW: u32 = 0x01E0_1B70;
const DARK: u32 = 0x01E0_1D00;
const RING: u32 = 0x01E0_1EB0;
const EMBER: u32 = 0x01E0_2050;
const STREAK_B: u32 = 0x01E0_21D0;
const LIGHT: u32 = 0x01E0_2360;

const POOL: u32 = D + 0x2E0;
const POS: u32 = D + 0x250;
const TEX: u32 = D + 0x2A0;
const TEX_D: u32 = D + 0x2D4;
const LAMP: u32 = D + 0x34E0;
const KEEP: u32 = D + 0x3520;
const TEX5: u32 = D + 0x3680;
const TIMER: u32 = D + 0x36A0;
const COLOUR: u32 = D + 0x36A4;
const OT2: u32 = D + 0x36AC;
const NODES: u32 = D + 0x36C8;
const PARAM: u32 = D + 0x36E2;
const COUNTER: u32 = D + 0x36E6;
const BYTES: u32 = D + 0x36E8;
const SINES: u32 = D + 0x37E8;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02690 (D, param) (s0 = D, s1 = 0 / the live count, s2 =
/// param / the task pointer, s3 = the index).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x170);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x170);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3960);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x36E4, param);
    fx.mem.set32(TIMER, 0x10C);
    for (k, n) in [0x1D, 0x62, 0x20, 0x68, 0xA5, 0xB0].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x36D6, r);
    // 0x1E025F0(0, D): the textures.
    for k in 0..3 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    for k in 0xD..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(TEX_D + 4 * (k as u32 - 0xD), t);
    }
    let t = fx.texture(5, 0);
    fx.mem.write_u32(TEX5, t);
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_27AC, 0x1E);
        fx.mem.write_u8(BYTES + k, (r - 0xF) as u8);
    }
    0
}

/// The texture at `tex` and the atlas cell `id` frame +0x34 (0x106960
/// into `r`) copied to D+0x88 / D+0x94..+0xA3 (b018's `cell`).
fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32, id: i32) {
    let v = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, v);
    let f = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, f);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// A step's frame (`size`) and its saves of the entry's s0 = D (and s1 =
/// 0 in a 0x30 / 0x50 frame).
fn run_step(fx: &mut Fx, t: u32, size: u32, ra: u32, f: impl FnOnce(&mut Fx, u32, u32)) {
    let sp = fx.enter(size);
    let (ra, d) = (u64::from(ra), u64::from(D));
    match size {
        0x20 => fx.saved(sp, &[(0x10, ra), (0, d)]),
        _ => fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, d)]),
    }
    f(fx, t, sp);
    fx.leave(size);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            STREAK_A => run_step(fx, t, 0x50, 0x01E0_281C, |fx, t, sp| streak(fx, t, sp, 0, [0x01E0_18B0, 0x01E0_18EC, 0x01E0_1920, 0x01E0_193C, 0x01E0_19D0, 0x01E0_19D8, 0x01E0_19E4, 0x01E0_19F0])),
            SMOKE => run_step(fx, t, 0x30, 0x01E0_2840, |fx, t, _| smoke(fx, t)),
            GLOW => run_step(fx, t, 0x20, 0x01E0_2860, |fx, t, _| glow(fx, t)),
            DARK => run_step(fx, t, 0x20, 0x01E0_2920, |fx, t, _| dark(fx, t)),
            RING => run_step(fx, t, 0x20, 0x01E0_2940, |fx, t, _| ring(fx, t)),
            EMBER => run_step(fx, t, 0x20, 0x01E0_2960, |fx, t, _| ember(fx, t)),
            STREAK_B => run_step(fx, t, 0x50, 0x01E0_2980, |fx, t, sp| streak(fx, t, sp, 1, [0x01E0_2200, 0x01E0_223C, 0x01E0_2270, 0x01E0_228C, 0x01E0_2320, 0x01E0_2328, 0x01E0_2334, 0x01E0_2340])),
            LIGHT => run_step(fx, t, 0x30, 0x01E0_29A0, |fx, t, _| light(fx, t)),
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_2A34), (0, u64::from(D))]);
    let d = sp + 0xD8;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            STREAK_A => {
                cell(fx, t, TEX_D + 8, sp + 0x60, 0x9B);
                streaks(fx, t, d, BYTES);
            }
            SMOKE => {
                cell(fx, t, TEX5, sp + 0x70, 0x14);
                for c in 0..3u32 {
                    let v = ee::from_i32(fx.mem.i16(t + 2 * c));
                    fx.mem.setf(sp + 0x50 + 4 * c, v);
                }
                for (c, o) in [(0u32, 8u32), (2, 0xC)] {
                    let v = ee::div(ee::from_i32(fx.mem.i16(t + o)), 10.0);
                    let p = ee::add(fx.mem.f(sp + 0x50 + 4 * c), v);
                    fx.mem.setf(sp + 0x50 + 4 * c, p);
                }
                billboard::sprite_square(fx, 0x01E0_1260, uv::uv_rect4, D, t, 0, sp + 0x50);
            }
            GLOW => {
                cell(fx, t, TEX_D + 8, sp + 0x80, 0x9B);
                quads4::fan18(fx, D, t, d);
            }
            DARK => {
                cell(fx, t, TEX_D + 8, sp + 0x90, 0x9B);
                billboard::ring_t(fx, uv::uv_rect4, D, t, d, BYTES);
            }
            RING => {
                cell(fx, t, TEX + 4, sp + 0xA0, 0xA);
                billboard::ring_t20(fx, uv::uv_rect4, D, t, d, BYTES);
            }
            EMBER => {
                cell(fx, t, TEX_D + 8, sp + 0xB0, 0x9B);
                billboard::ring_t20(fx, uv::uv_rect4, D, t, d, BYTES);
            }
            STREAK_B => {
                cell(fx, t, TEX_D + 8, sp + 0xC0, 0x9B);
                streaks(fx, t, d, BYTES);
            }
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0xA8 => 1,
        0xFA => 4,
        0xDC => 2,
        0xB3..=0xD6 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// Alpha +0x2B down by `down` while at least `floor`, else the task ends.
fn fall(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a - down);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

fn grow(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn radius(fx: &mut Fx, t: u32) {
    let v = fx.mem.i16(t + 0x30).wrapping_add(fx.mem.i16(t + 0xE));
    fx.mem.set16(t + 0x30, v);
}

/// 0x1E01880 / 0x1E021D0 (D, task) (frame 0x50 at `sp`): a red streak ring
/// (0x80, 0x20, 0x20) with k = rand % 4 (+0x3A): size 10, frame 2, +0x18
/// rand % 0xFFFF, growth 0x11 - k / 2, row rand % 10, life rand % 3 + 1;
/// 0x80 for its life, then out by 0xA; the radius +0x30 up by the growth;
/// the point node `node` (0x1D / 0x62). `ra`: the rand sites, the node's
/// and the three conversions'.
fn streak(fx: &mut Fx, t: u32, sp: u32, node: u32, ra: [u32; 8]) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xA);
        let v = fx.rand_from(t, ra[0]);
        fx.mem.set16(t + 0x3A, v % 4);
        fx.mem.set16(t + 0x34, 2);
        l2::rgb(fx, t + 0x28, 0x80, 0x20, 0x20);
        let r = rmod(fx, t, ra[1], 0xFFFF);
        fx.mem.set32(t + 0x18, r);
        fx.mem.set16(t + 0x30, 0);
        let k = fx.mem.i16(t + 0x3A) >> 1;
        fx.mem.set16(t + 0xE, 0x11 - k);
        let r = rmod(fx, t, ra[2], 0xA);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, ra[3], 3);
        fx.mem.set16(t + 6, r + 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    radius(fx, t);
    grow(fx, t);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 2 * node);
    node_position_saving(fx, h, s, sp + 0x30, u64::from(t), Some(u64::from(D)), ra[4]);
    for i in 0..3u32 {
        let v = fx.mem.f(sp + 0x30 + 4 * i);
        fx.fix_residue(v, ra[5 + i as usize]);
        fx.mem.set16(t + 2 * i, ee::fixsfsi(v));
    }
}

/// 0x1E01A10 (D, task): b225's grey smoke sprite (size 60, cell 5, (0x80,
/// 0x80, 0x80), +0x30 / +0x3A rand % 360): 0x80 for 7 updates, then out by
/// 15; +8 / +0xC drift 16 sin / cos of +0x3A (the sines D+0x37E8).
fn smoke(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        fx.mem.set16(t + 0x34, 5);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let r = rmod(fx, t, 0x01E0_1A60, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, t, 0x01E0_1A7C, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xF);
    }
    for (at, cos, ra) in [(8u32, 0, 0x01E0_1B0Cu32), (0xC, 1, 0x01E0_1B44)] {
        let a = fx.mem.i16(t + 0x3A);
        let s = l2::sin_deg(fx, SINES, a, cos);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(16.0, s));
        fx.fix_residue(v, ra);
        fx.mem.set16(t + at, ee::fixsfsi(v));
    }
    grow(fx, t);
}

/// 0x1E01B70 (D, task): a glow disc, centre (0xC0, 0xC0, 0xC0) rim (0x20,
/// 0x10, 0, 0x10), radius 0x14A, +0x3A rand % 360: 0xC0 for 5 updates,
/// then out by 0xA.
fn glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x14A);
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x2C, 0x20, 0x10, 0);
        let r = rmod(fx, t, 0x01E0_1BC4, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0x10);
    }
    if fx.mem.i16(t + 0x32) < 5 {
        fx.mem.write_u8(t + 0x2B, 0xC0);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    grow(fx, t);
}

/// 0x1E01D00 (D, task): b205's dark ring in dark red (0x80, 0x10, 0x10):
/// size 0x14, frame rand % 4, angles +0x10..+0x18 rand % 0xFFFF, +0x30 =
/// 0, +0x3A rand % 360, growth rand % 26 + 13, row rand % 10, life rand %
/// 7 + 5; in by 0x38 under 0x71 for its life, then out by 0xA; the radius
/// up by the growth from age 1.
fn dark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x14);
        let v = fx.rand_from(t, 0x01E0_1D28);
        fx.mem.set16(t + 0x34, v % 4);
        for (o, ra) in [(0x10u32, 0x01E0_1D44u32), (0x14, 0x01E0_1D60), (0x18, 0x01E0_1D7C)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        l2::rgb(fx, t + 0x28, 0x80, 0x10, 0x10);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, t, 0x01E0_1DA8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, 0x01E0_1DC4, 0x1A);
        fx.mem.set16(t + 0xE, r + 0xD);
        let r = rmod(fx, t, 0x01E0_1DE4, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_1E00, 7);
        fx.mem.set16(t + 6, r + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x71 {
            fx.mem.write_u8(t + 0x2B, a + 0x38);
        }
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        radius(fx, t);
    }
    grow(fx, t);
}

/// 0x1E01EB0 (D, task): b148's ring_t step in red (0x80, 0x20, 0x20):
/// size 0xC, frame rand % 4, +0x18 / +0x10 rand % 0xFFFF, +0x30 rand % 16,
/// +0x3A rand % 360, growth rand % 16 + 8, row rand % 10, life rand % 10 +
/// 5; 0x80 for its life, then out by 15; the radius up by the growth.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xC);
        let v = fx.rand_from(t, 0x01E0_1ED8);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0x80, 0x20, 0x20);
        for (o, ra) in [(0x18u32, 0x01E0_1F0Cu32), (0x10, 0x01E0_1F28)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        let v = fx.rand_from(t, 0x01E0_1F44);
        fx.mem.set16(t + 0x30, v % 16);
        let r = rmod(fx, t, 0x01E0_1F60, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, 0x01E0_1F7C);
        fx.mem.set16(t + 0xE, v % 16 + 8);
        let r = rmod(fx, t, 0x01E0_1F9C, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_1FB8, 0xA);
        fx.mem.set16(t + 6, r + 5);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xF);
    }
    radius(fx, t);
    grow(fx, t);
}

/// 0x1E02050 (D, task): an ember ring (0x80, 0x40, 0): size 0xC, frame
/// rand % 4, +0x18 rand % 0xFFFF, +0x30 rand % 16, +0x3A rand % 360,
/// growth rand % 16 + 8, row rand % 10, life rand % 3 + 1; 0x80 for its
/// life, then out by 0xA; the radius up by the growth.
fn ember(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xC);
        let v = fx.rand_from(t, 0x01E0_2078);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        let r = rmod(fx, t, 0x01E0_20AC, 0xFFFF);
        fx.mem.set32(t + 0x18, r);
        let v = fx.rand_from(t, 0x01E0_20C8);
        fx.mem.set16(t + 0x30, v % 16);
        let r = rmod(fx, t, 0x01E0_20E4, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, 0x01E0_2100);
        fx.mem.set16(t + 0xE, v % 16 + 8);
        let r = rmod(fx, t, 0x01E0_2120, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_213C, 3);
        fx.mem.set16(t + 6, r + 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    radius(fx, t);
    grow(fx, t);
}

/// 0x1E02360 (D, task): b018's camera light (camera 0's slots 4 / 5,
/// 0x1F7E20) on the lamp D+0x34E0 at the task's point: its level D+0x3508
/// up by 0.2 to 1, then down by 0.1 to 0.1, when the task ends (the slots
/// cleared); it ends with the timer at 1 too.
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x4080_0000), (8, 0x4000_0000), (0xC, 0), (0x20, 0), (0x24, 0x42F0_0000), (0x28, 0)] {
            fx.mem.write_u32(LAMP + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(LAMP + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let lv = fx.mem.f(LAMP + 0x28);
    if fx.mem.i16(t + 0x30) == 0 {
        if lv < 1.0 {
            fx.mem.setf(LAMP + 0x28, ee::add(lv, f32::from_bits(0x3E4C_CCCD)));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else {
        let tenth = f32::from_bits(0x3DCC_CCCD);
        if lv <= tenth {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.setf(LAMP + 0x28, ee::sub(lv, tenth));
        }
    }
    let p = if fx.mem.i16(t + 0x36) == 0 { 0 } else { LAMP };
    crate::blib::exe::camera_slots45(fx, p);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E02F80(D, task, desc, table) (frame 0x150): b052's 0x1E03730 word
/// for word (b393.rs's `streaks`): `billboard::ring_t20` with the bytes
/// `table` read at +0 / +0x32 / +0x64 and each quad's Z set far before it
/// is keyed by its mean w into D+0x80.
fn streaks(fx: &mut Fx, t: u32, d: u32, table: u32) {
    let sp = fx.enter(0x150);
    let l = |o: u32| sp + o - 0x10;
    desc_uv4(fx, d);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, l(0x158), t + 0x28, d + 0x10);
    uv::uv_rect4(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(l(0xE0) + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, l(0x140));
    for k in 0..3 {
        let v = fx.mem.read_u32(l(0x140) + 4 * k).wrapping_add(fx.mem.read_u32(t + 0x10 + 4 * k));
        fx.mem.write_u32(l(0x140) + 4 * k, v);
    }
    for k in 0..3 {
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(l(0xD0) + 4 * k, p);
    }
    l2::xform_vp(fx, l(0x100), l(0xE0), l(0x140), l(0xD0));
    let q = l(0x90);
    let mut k = 0i32;
    while k < 0x14 {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, q, s, s);
        for (axis, off) in [(0, 0u32), (1, 0x32), (2, 0x64)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::from_i32(fx.mem.i8(table.wrapping_add(k as u32).wrapping_add(i as u32).wrapping_add(off)));
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + (k >> 1));
            l2::add_column(fx, q, axis, ee::mul(ee::div(b, 20.0), r));
        }
        for (o, v) in [(4u32, 0u32), (0x14, 0), (0, 0x3F80_0000), (0x10, 0xBF80_0000), (8, 0), (0x18, 0)] {
            fx.mem.write_u32(q + o, v);
        }
        if l2::project(fx, q, SPR, l(0x100), d, l(0xF0)) {
            l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
            let key = ee::fixunssfsi(fx.mem.f(l(0xF0))) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x150);
}

/// 0x1E02E60(D, step, 0) (frame 0x60: ra +0x50, s4 / s3 the runner's (0)
/// [H], s2 = the entry's parameter / s1 the emitter's, s0 = D): b018's
/// spawn on this record's globals: the first free task cleared and set off
/// at D+0x250 (as i16) with the colour D+0x36A4 (twice) and +0x30 =
/// D+0x36E2.
fn spawn(fx: &mut Fx, f: u32, ra: u32, s1: u64, s2: u64) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, s2), (0x10, s1), (0, u64::from(D))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
        Some(k) => {
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
            let a = fx.mem.i16(PARAM);
            fx.mem.set16(t + 0x30, a);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E01450(D, 0) (frame 0x40: ra +0x20, s1 +0x10 = 0, s0 = D): the
/// spawn colour (0x80, 0, 0, 0), the update counter and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x20, 0x01E0_27E8), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    // s1 the emitter's at each call (s2 the entry's parameter).
    let mut s1 = 0u64;
    let node = |fx: &mut Fx, k: u32, out: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, n, out, u64::from(D), Some(s1), ra);
    };
    let to_pos = |fx: &mut Fx, from: u32| {
        for c in 0..3 {
            let w = fx.mem.read_u32(from + 4 * c);
            fx.mem.write_u32(POS + 4 * c, w);
        }
    };
    if counter(fx) == 0x33 {
        node(fx, 1, KEEP, s1, 0x01E0_14AC);
    }
    let c = counter(fx);
    if (0x34..0x41).contains(&c) {
        node(fx, 1, sp + 0x30, s1, 0x01E0_14E0);
        to_pos(fx, KEEP);
    }
    if counter(fx) < 0xB3 {
        spawn(fx, STREAK_A, 0x01E0_152C, s1, p);
        spawn(fx, STREAK_A, 0x01E0_1540, s1, p);
    }
    if counter(fx) < 0xCC {
        spawn(fx, STREAK_B, 0x01E0_1564, s1, p);
        spawn(fx, STREAK_B, 0x01E0_1578, s1, p);
    }
    for (list, k, ra_node, ra) in [(&[0x12, 0x2C, 0x4B, 0x65, 0x83, 0x98][..], 5u32, 0x01E0_15DCu32, 0x01E0_160Cu32), (&[0x1F, 0x3A, 0x57, 0x72, 0x8F][..], 4, 0x01E0_1678, 0x01E0_16A8)] {
        if list.contains(&counter(fx)) {
            node(fx, k, sp + 0x30, s1, ra_node);
            to_pos(fx, sp + 0x30);
            s1 = 0;
            while s1 < 0x10 {
                spawn(fx, SMOKE, ra, s1, p);
                s1 += 1;
            }
        }
    }
    for (pass, ras) in [(0, [0x01E0_16F8u32, 0x01E0_1720]), (1, [0x01E0_17A8, 0x01E0_17D0])] {
        let c = counter(fx);
        if c != 0xB2 && c != 0xCB {
            continue;
        }
        if counter(fx) == 0xB2 {
            node(fx, 2, sp + 0x30, s1, ras[0]);
        }
        if counter(fx) == 0xCB {
            node(fx, 3, sp + 0x30, s1, ras[1]);
        }
        to_pos(fx, sp + 0x30);
        fx.mem.set16(PARAM, 1);
        if pass == 0 {
            s1 = 0;
            while s1 < 8 {
                spawn(fx, EMBER, 0x01E0_1758, s1, p);
                s1 += 1;
            }
        } else {
            s1 = 0;
            while s1 < 8 {
                spawn(fx, RING, 0x01E0_1808, s1, p);
                s1 += 1;
            }
            s1 = 0;
            while s1 < 0x1E {
                spawn(fx, DARK, 0x01E0_1830, s1, p);
                s1 += 1;
            }
            spawn(fx, GLOW, 0x01E0_1854, s1, p);
            spawn(fx, LIGHT, 0x01E0_1868, s1, p);
        }
    }
    fx.leave(0x40);
}
