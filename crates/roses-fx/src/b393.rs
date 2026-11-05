//! BMODULE b393 (card 513), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03190, data area 0x1ED0000, 0x3B10 bytes) [F]
//!: b018 / b052's program (200 tasks of 0x40
//! bytes at D+0x478, alive +0x36, step +0x24 by direct `jal`; spawn
//! 0x1E03A40 = b018's on colour D+0x3844, point D+0x2D0, parameter
//! D+0x389A) with its globals 0x20 higher. Attacker nodes 0x55 / 0x54 /
//! 0x2F / 0x33 / 0x3F / 0x48 (D+0x3888..), the target's 1 (D+0x3896);
//! timer 0xC8; the histories D+0x3770 (node 0x55) / D+0x36D0 (node 0x54)
//! filled on update 0x18 and pushed by `spawn::node_history` (drawn by
//! nothing: the ribbon / cross steps are never spawned).
//!
//! The emitter 0x1E01DA0 counts D+0x389E: below 0x89 two red streak rings
//! a update (0x1E023B0, b052's streak with k = rand % 4, drawn by b052's
//! 0x1E03730 copy); on 0x34..0x88 a grey smoke sprite (0x1E02610, b225's
//! with the sines D+0x39A4, `billboard::sprite_square`) at each of the
//! nodes 0x2F / 0x33 / 0x3F / 0x48; on 0x88 at node 0x55 eight ember
//! rings 0x1E02C40 (`ring_t20`), eight red rings 0x1E02AB0 (b148's
//! ring_t step, `ring_t20` on cell 0xA), thirty dark red rings 0x1E02900
//! (b205's, `ring_t`), a burning disc 0x1E02770 (`spawn::disc_step`
//! 5225257f1409, `fan18`) and b018's camera light 0x1E02E60 (lamp
//! D+0x3680). Returns 1 on update 0x7E, 4 on 0x92, 2 on 0x88, 8 once the
//! timer D+0x3840 has run out. The steps 0x1E02190 / 0x1E02850 /
//! 0x1E02DB0 / 0x1E02FE0 / 0x1E030A0 are tested but never spawned (not
//! ported). No 0x1F8100.
use crate::b076::node_position_saving;
use crate::blib::{billboard, quads4, spawn as bspawn, uv};
use crate::bus::Bus;
use crate::c061::{desc_uv4, rgbaq_at};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 200;

const STREAK: u32 = 0x01E0_23B0;
const SMOKE: u32 = 0x01E0_2610;
const DISC: u32 = 0x01E0_2770;
const DARK: u32 = 0x01E0_2900;
const RING: u32 = 0x01E0_2AB0;
const EMBER: u32 = 0x01E0_2C40;
const LIGHT: u32 = 0x01E0_2E60;

const POOL: u32 = D + 0x478;
const POS: u32 = D + 0x2D0;
const TEX: u32 = D + 0x340;
const TEX_D: u32 = D + 0x374;
const LAMP: u32 = D + 0x3680;
const HIST_A: u32 = D + 0x36D0;
const HIST_B: u32 = D + 0x3770;
const KEEP: u32 = D + 0x36C0;
const TEX5: u32 = D + 0x3820;
const TIMER: u32 = D + 0x3840;
const COLOUR: u32 = D + 0x3844;
const OT2: u32 = D + 0x384C;
const NODES: u32 = D + 0x3888;
const PARAM: u32 = D + 0x389A;
const COUNTER: u32 = D + 0x389E;
const BYTES: u32 = D + 0x38A2;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E03190 (D, param) (s0 = D, s1 = 0 / the live count, s2 =
/// param / the task pointer, s3 = the index).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x180);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x180);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3B10);
    l2::sine_table(fx, D + 0x39A4);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x389C, param);
    fx.mem.set32(TIMER, 0xC8);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.write_u32(D + o, 0);
    }
    for (k, n) in [0x55, 0x54, 0x2F, 0x33, 0x3F, 0x48].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x3896, r);
    // 0x1E030F0(0, D): the textures.
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
        let r = rmod(fx, D, 0x01E0_32B8, 0x1E);
        fx.mem.write_u8(BYTES + k, (r - 0xF) as u8);
    }
    0
}

/// A history's nine points from `node`'s position (through `local`).
fn fill(fx: &mut Fx, node: u32, local: u32, hist: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(node);
    fx.node_position(h, n, local);
    for i in 0..9 {
        for k in 0..3 {
            let w = fx.mem.read_u32(local + 4 * k);
            fx.mem.write_u32(hist + 0x10 * i + 4 * k, w);
        }
    }
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

/// A step's frame (`size`) and its saves of the entry's s0 = D / s1 = 0
/// (and s2 = the task pointer in a 0x70 frame).
fn run_step(fx: &mut Fx, t: u32, size: u32, ra: u32, f: impl FnOnce(&mut Fx, u32, u32)) {
    let sp = fx.enter(size);
    let (ra, d) = (u64::from(ra), u64::from(D));
    match size {
        0x20 => fx.saved(sp, &[(0x10, ra), (0, d)]),
        0x30 => fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, d)]),
        _ => {
            fx.saved(sp, &[(0x40, ra), (0x30, u64::from(t - 0x478)), (0x20, 0), (0x10, d)]);
            // swc1 f20 / f21: the runner's (0) [H].
            fx.mem.write_u32(sp, 0);
            fx.mem.write_u32(sp + 4, 0);
        }
    }
    f(fx, t, sp);
    fx.leave(size);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    if counter(fx) == 0x18 {
        fill(fx, NODES, sp + 0xC0, HIST_B);
    }
    if counter(fx) == 0x18 {
        fill(fx, NODES + 2, sp + 0xD0, HIST_A);
    }
    let n = fx.mem.i16(NODES);
    bspawn::node_history(fx, HIST_B + 0x90, 0xA, n);
    let n = fx.mem.i16(NODES + 2);
    bspawn::node_history(fx, HIST_A + 0x90, 0xA, n);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            STREAK => run_step(fx, t, 0x70, 0x01E0_3418, streak),
            SMOKE => run_step(fx, t, 0x30, 0x01E0_3438, |fx, t, _| smoke(fx, t)),
            DISC => {
                // The library step enters its own 0x20 frame.
                let s = fx.sp - 0x20;
                fx.saved(s, &[(0x10, 0x01E0_3458), (0, u64::from(D))]);
                let c = bspawn::disc("5225257f1409").expect("constants");
                bspawn::disc_step_with(fx, c, DISC, t);
            }
            DARK => run_step(fx, t, 0x20, 0x01E0_3518, |fx, t, _| dark(fx, t)),
            RING => run_step(fx, t, 0x20, 0x01E0_3538, |fx, t, _| ring(fx, t)),
            EMBER => run_step(fx, t, 0x20, 0x01E0_3558, |fx, t, _| ember(fx, t)),
            LIGHT => run_step(fx, t, 0x30, 0x01E0_3620, |fx, t, _| light(fx, t)),
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_36B4), (0, u64::from(D))]);
    let d = sp + 0xE8;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            STREAK => {
                cell(fx, t, TEX_D + 8, sp + 0x60, 0x9B);
                streaks(fx, t, d, BYTES);
            }
            SMOKE => {
                cell(fx, t, TEX5, sp + 0x70, 0x14);
                for c in 0..3u32 {
                    let v = ee::from_i32(fx.mem.i16(t + 2 * c));
                    fx.mem.setf(sp + 0x50 + 4 * c, v);
                }
                billboard::sprite_square(fx, 0x01E0_1BB0, uv::uv_rect4, D, t, 0, sp + 0x50);
            }
            DISC => {
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
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x7E => 1,
        0x92 => 4,
        0x88 => 2,
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

/// 0x1E023B0 (D, task) (frame 0x70 at `sp`): b052's streak ring in red
/// (0x80, 0x20, 0x20) with k = rand % 4 (+0x3A): size 6, frame 2, +0x18
/// rand % 0xFFFF, growth 7 - k / 2, row rand % 10, life rand % 3 + 1; 0x80
/// for its life, then out by 0xA; the radius +0x30 up by the growth; the
/// point k tenths of the way from node 0x55 to node 0x54.
fn streak(fx: &mut Fx, t: u32, sp: u32) {
    use ee::{add, div, from_i32, mul, sub};
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 6);
        let v = fx.rand_from(t, 0x01E0_23EC);
        fx.mem.set16(t + 0x3A, v % 4);
        fx.mem.set16(t + 0x34, 2);
        l2::rgb(fx, t + 0x28, 0x80, 0x20, 0x20);
        let r = rmod(fx, t, 0x01E0_2428, 0xFFFF);
        fx.mem.set32(t + 0x18, r);
        fx.mem.set16(t + 0x30, 0);
        let k = fx.mem.i16(t + 0x3A) >> 1;
        fx.mem.set16(t + 0xE, 7 - k);
        let r = rmod(fx, t, 0x01E0_245C, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_2478, 3);
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
    let s = fx.mem.i16(NODES);
    node_position_saving(fx, h, s, sp + 0x50, u64::from(t), Some(u64::from(D)), 0x01E0_250C);
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES + 2);
    node_position_saving(fx, h, s, sp + 0x60, u64::from(t), Some(u64::from(sp + 0x60)), 0x01E0_2528);
    let n1 = [fx.mem.f(sp + 0x50), fx.mem.f(sp + 0x54), fx.mem.f(sp + 0x58)];
    let n2 = [fx.mem.f(sp + 0x60), fx.mem.f(sp + 0x64), fx.mem.f(sp + 0x68)];
    let step: [f32; 3] = std::array::from_fn(|i| div(sub(n2[i], n1[i]), 10.0));
    for (i, ra) in [0x01E0_2598u32, 0x01E0_25BC, 0x01E0_25E0].into_iter().enumerate() {
        let k = from_i32(fx.mem.i16(t + 0x3A));
        let v = add(n1[i], mul(step[i], k));
        fx.fix_residue(v, ra);
        fx.mem.set16(t + 2 * i as u32, ee::fixsfsi(v));
    }
}

/// 0x1E02610 (D, task): b225's grey smoke sprite (size 60, cell 5, (0x80,
/// 0x80, 0x80), +0x30 / +0x3A rand % 360): 0x80 for 7 updates, then out by
/// 15; +8 / +0xC drift 16 sin / cos of +0x3A (the sines D+0x39A4).
fn smoke(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x3C);
        fx.mem.set16(t + 0x34, 5);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        let r = rmod(fx, t, 0x01E0_2660, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, t, 0x01E0_267C, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 7 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fall(fx, t, 0x11, 0xF);
    }
    for (at, cos, ra) in [(8u32, 0, 0x01E0_270Cu32), (0xC, 1, 0x01E0_2744)] {
        let a = fx.mem.i16(t + 0x3A);
        let s = l2::sin_deg(fx, D + 0x39A4, a, cos);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(16.0, s));
        fx.fix_residue(v, ra);
        fx.mem.set16(t + at, ee::fixsfsi(v));
    }
    grow(fx, t);
}

/// 0x1E02900 (D, task): b205's dark ring in dark red (0x80, 0x10, 0x10):
/// size 10, frame rand % 4, angles +0x10..+0x18 rand % 0xFFFF, +0x30 = 0,
/// +0x3A rand % 360, growth rand % 16 + 3, row rand % 10, life rand % 7 +
/// 5; in by 0x38 under 0x71 for its life, then out by 0xA; the radius up
/// by the growth from age 1.
fn dark(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xA);
        let v = fx.rand_from(t, 0x01E0_2928);
        fx.mem.set16(t + 0x34, v % 4);
        for (o, ra) in [(0x10u32, 0x01E0_2944u32), (0x14, 0x01E0_2960), (0x18, 0x01E0_297C)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        l2::rgb(fx, t + 0x28, 0x80, 0x10, 0x10);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, t, 0x01E0_29A8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, 0x01E0_29C4);
        fx.mem.set16(t + 0xE, v % 16 + 3);
        let r = rmod(fx, t, 0x01E0_29E4, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_2A00, 7);
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

/// 0x1E02AB0 (D, task): b148's ring_t step in red (0x80, 0x20, 0x20):
/// size 2, frame rand % 4, +0x18 / +0x10 rand % 0xFFFF, +0x30 = 0, +0x3A
/// rand % 360, growth rand % 16 + 3, row rand % 10, life rand % 10 + 5;
/// 0x80 for its life, then out by 15; the radius up by the growth.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        let v = fx.rand_from(t, 0x01E0_2AD8);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0x80, 0x20, 0x20);
        for (o, ra) in [(0x18u32, 0x01E0_2B0Cu32), (0x10, 0x01E0_2B28)] {
            let r = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, r);
        }
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, t, 0x01E0_2B48, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, 0x01E0_2B64);
        fx.mem.set16(t + 0xE, v % 16 + 3);
        let r = rmod(fx, t, 0x01E0_2B84, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_2BA0, 0xA);
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

/// 0x1E02C40 (D, task): an ember ring (0x80, 0x40, 0): size 2, frame rand
/// % 4, +0x18 rand % 0xFFFF, +0x30 = 0, +0x3A rand % 360, growth rand % 16
/// + 3, row rand % 10, life rand % 3 + 1; 0x80 for its life, then out by
/// 0xA; the radius up by the growth.
fn ember(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 2);
        let v = fx.rand_from(t, 0x01E0_2C68);
        fx.mem.set16(t + 0x34, v % 4);
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        let r = rmod(fx, t, 0x01E0_2C9C, 0xFFFF);
        fx.mem.set32(t + 0x18, r);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, t, 0x01E0_2CBC, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let v = fx.rand_from(t, 0x01E0_2CD8);
        fx.mem.set16(t + 0xE, v % 16 + 3);
        let r = rmod(fx, t, 0x01E0_2CF8, 0xA);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_2D14, 3);
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

/// 0x1E02E60 (D, task): b018's camera light (camera 0's slots 4 / 5,
/// 0x1F7E20) on the lamp D+0x3680 at the task's point: its level D+0x36A8
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

/// 0x1E03B60(D, task, desc, table) (frame 0x150): b052's 0x1E03730 word
/// for word: `billboard::ring_t20` with the bytes `table` read at +0 /
/// +0x32 / +0x64 and each quad's Z set far before it is keyed by its mean
/// w into D+0x80.
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

/// 0x1E03A40(D, step, 0) (frame 0x60: ra +0x50, s4 / s3 the runner's (0)
/// [H], s2 / s1 the emitter's, s0 = D): b018's spawn on this record's
/// globals: the first free task cleared and set off at D+0x2D0 (as i16)
/// with the colour D+0x3844 (twice) and +0x30 = D+0x389A.
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

/// 0x1E01DA0(D, 0) (frame 0x60: ra +0x30, s2 +0x20 = the parameter, s1
/// +0x10 = 0, s0 = D): the spawn colour (0x80, 0, 0, 0), the update
/// counter and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x60);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_32F4), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    // (s1, s2) the emitter's at each call.
    let (mut s1, mut s2) = (0u64, p);
    let node = |fx: &mut Fx, k: u32, out: u32, s1: u64, ra: u32| {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODES + 2 * k);
        node_position_saving(fx, h, n, out, u64::from(D), Some(s1), ra);
    };
    if counter(fx) == 0x33 {
        node(fx, 1, KEEP, s1, 0x01E0_1E00);
    }
    let c = counter(fx);
    if (0x34..0x41).contains(&c) {
        node(fx, 1, sp + 0x40, s1, 0x01E0_1E34);
        for k in 0..3 {
            let w = fx.mem.read_u32(KEEP + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    }
    if counter(fx) < 0x89 {
        spawn(fx, STREAK, 0x01E0_1E84, s1, s2);
        spawn(fx, STREAK, 0x01E0_1E98, s1, s2);
    }
    let c = counter(fx);
    if (0x34..0x89).contains(&c) {
        for (k, ra_node, ra) in [(2u32, 0x01E0_1ECCu32, 0x01E0_1F00u32), (3, 0x01E0_1F18, 0x01E0_1F44), (4, 0x01E0_1F5C, 0x01E0_1F88), (5, 0x01E0_1FA0, 0x01E0_1FCC)] {
            node(fx, k, sp + 0x40, s1, ra_node);
            (s1, s2) = (u64::from(sp + 0x44), u64::from(sp + 0x48));
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0x40 + 4 * c);
                fx.mem.write_u32(POS + 4 * c, w);
            }
            spawn(fx, SMOKE, ra, s1, s2);
        }
    }
    if counter(fx) == 0x1A {
        node(fx, 2, sp + 0x40, s1, 0x01E0_1FF4);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * c);
            fx.mem.write_u32(POS + 4 * c, w);
        }
        fx.mem.set16(PARAM, 1);
        let y = ee::add(1.0, fx.mem.f(sp + 0x44));
        fx.mem.setf(POS + 4, y);
    }
    let at = |fx: &mut Fx, s1: u64, ra: u32| {
        node(fx, 0, sp + 0x40, s1, ra);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * c);
            fx.mem.write_u32(POS + 4 * c, w);
        }
        fx.mem.set16(PARAM, 1);
    };
    if counter(fx) == 0x88 {
        node(fx, 0, sp + 0x40, s1, 0x01E0_2054);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x50);
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_2068), (0, u64::from(D))]);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * c);
            fx.mem.write_u32(POS + 4 * c, w);
        }
        fx.mem.set16(PARAM, 1);
        s1 = 0;
        while s1 < 8 {
            spawn(fx, EMBER, 0x01E0_20A0, s1, s2);
            s1 += 1;
        }
    }
    if counter(fx) == 0x88 {
        at(fx, s1, 0x01E0_20D8);
        s1 = 0;
        while s1 < 8 {
            spawn(fx, RING, 0x01E0_2110, s1, s2);
            s1 += 1;
        }
        s1 = 0;
        while s1 < 0x1E {
            spawn(fx, DARK, 0x01E0_2138, s1, s2);
            s1 += 1;
        }
        spawn(fx, DISC, 0x01E0_215C, s1, s2);
        spawn(fx, LIGHT, 0x01E0_2170, s1, s2);
    }
    fx.leave(0x60);
}
