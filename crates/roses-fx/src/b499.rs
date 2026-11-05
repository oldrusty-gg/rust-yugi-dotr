//! BMODULE b499 (card 348), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02B90) [F]. The entry
//! takes (D, param). b701's family: 200 tasks of 0x40 bytes at D+0x3F0
//! (alive +0x36, step +0x24 called by direct `jal`s or inline; spawned by
//! e8e1debea8f3 [`spawn::task200`]) and 300 of 0x70 bytes at D+0x35F0
//! (alive +0x58, step +0x44 through `jalr`; 1f31920d0d56
//! [`spawn::task300`]). The attacker's nodes 0x2C / 0x2D keep 10-point
//! histories (D+0xB930 / D+0xB9D0, a72bfb6bb70a); the emitter 0x1E01200
//! throws blue sparks from nodes 0x45 / 0x4C (39..65), white flakes at node
//! 0x2D (16..94), grey puffs (to 94), a flare on 39 / 84, on 93 a white
//! flash, 40 sparks, 60 rays and a 30-mote violet burst (93..95) at node
//! 0x2C's newest point, 85..95 a red cloud; an object op task (0x1F8380)
//! to update 115. Drawn by billboard quads, a flat ring
//! (`quads4::ring_facing_flat`), a gouraud tube along node 0x2C's history
//! (0x1E038F0), a light column (0x1E03F70) and two sprites. Returns 1 on
//! update 0x54, 4 on 0x7D, 2 on 0x5E, 8 once the 135-update timer
//! D+0xBAB0 has run out.
use crate::blib::{billboard, exe::{camera_vp_into, mul_mem, rot_trans_into}, quads4, spawn as lspawn, uv};
use crate::b494::camera_angles_saving;
use crate::bus::Bus;
use crate::c061::rgbaq_at;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const VP: u32 = D + 0x290;
const POS: u32 = D + 0x2D0;
const TEX: u32 = D + 0x34C;
const POOL_A: u32 = D + 0x3F0;
const POOL_B: u32 = D + 0x35F0;
const HIST_A: u32 = D + 0xB930;
const HIST_B: u32 = D + 0xB9D0;
const KEEP: u32 = D + 0xBA70;
const HOLD: u32 = D + 0xBA80;
const TEX5: u32 = D + 0xBA90;
const TIMER: u32 = D + 0xBAB0;
const COLOUR: u32 = D + 0xBAB4;
const OTW: u32 = D + 0xBABC;
const MEAN: u32 = D + 0xBAE4;
const NODES: u32 = D + 0xBAF4;
const FACE: u32 = D + 0xBB04;
const SLOT3A: u32 = D + 0xBB08;
const SLOT30: u32 = D + 0xBB10;
const PARAM: u32 = D + 0xBB12;
const COUNTER: u32 = D + 0xBB14;
const BYTES: u32 = D + 0xBB18;
const TAB: u32 = D + 0xBC18;

/// Pool A's steps.
const FLASH: u32 = 0x01E0_1990;
const FLARE: u32 = 0x01E0_1A10;
const SWAY: u32 = 0x01E0_1AF0;
const SPARK: u32 = 0x01E0_1C50;
const RAY: u32 = 0x01E0_1D30;
const CLOUD: u32 = 0x01E0_1FA0;
/// Pool B's steps.
const BLUE: u32 = 0x01E0_2110;
const FLAKE: u32 = 0x01E0_2370;
const RING: u32 = 0x01E0_2550;
const PUFF: u32 = 0x01E0_2770;
const OBJECT: u32 = 0x01E0_2B00;
const JALR_RA: u64 = 0x01E0_328C;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x160);
    fx.mem.write_u32(OTW, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x160);
    ret
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

fn fixu(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixunssfsi(v) as i32
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

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0xBD90);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0x87);
    for (k, node) in [0x2C, 0x2D, 0x45, 0x4C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(TEX, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX5 + 4 * k, t);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_2CD8, 0x1E);
        fx.mem.set8(BYTES + k, r - 0xF);
    }
    0
}

fn node(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES + 2 * k);
    fx.node_position(h, n, out);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    if fx.mem.i16(COUNTER) == 0 {
        node(fx, 0, sp + 0xB0);
        for k in 0..10 {
            copy3(fx, HIST_A + 0x10 * k, sp + 0xB0);
        }
    }
    if fx.mem.i16(COUNTER) == 0 {
        node(fx, 1, sp + 0xC0);
        for k in 0..10 {
            copy3(fx, HIST_B + 0x10 * k, sp + 0xC0);
        }
    }
    let n = fx.mem.i16(NODES);
    lspawn::node_history(fx, HIST_A + 0x90, 0xA, n);
    let n = fx.mem.i16(NODES + 2);
    lspawn::node_history(fx, HIST_B + 0x90, 0xA, n);
    if fx.mem.i16(COUNTER) == 0x5E {
        node(fx, 1, HOLD);
    }
    emit(fx, param);
    // The entry's s1: the live tasks counted so far (both step loops).
    let mut s1 = 0u32;
    for k in 0..200u32 {
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        s1 += 1;
        let st = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        if st(fx) == FLASH {
            flash(fx, t);
        }
        if st(fx) == FLARE {
            // flare_step enters its own 0x20 frame; the caller's saves.
            let f = fx.sp - 0x20;
            fx.saved(f, &[(0x10, 0x01E0_2F10), (0, u64::from(D))]);
            crate::blib::spawn::flare_step(fx, FLARE, t);
        }
        if st(fx) == SWAY {
            sway(fx, t);
        }
        if st(fx) == SPARK {
            let s = fx.enter(0x20);
            fx.saved(s, &[(0x10, 0x01E0_3070), (0, u64::from(D))]);
            spark(fx, t);
            fx.leave(0x20);
        }
        if st(fx) == RAY {
            let s = fx.enter(0x40);
            fx.saved(s, &[(0x30, 0x01E0_3090), (0x20, u64::from(D + 0x40 * k)), (0x10, u64::from(s1)), (0, u64::from(D))]);
            ray(fx, t);
            fx.leave(0x40);
        }
        if st(fx) == CLOUD {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, 0x01E0_30B0), (0x10, u64::from(s1)), (0, u64::from(D))]);
            cloud(fx, t);
            fx.leave(0x30);
        }
    }
    let d = sp + 0xD0;
    for k in 0..200u32 {
        let t = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let s = [D, s1, D + 0x40 * k, k];
        let st = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        if st(fx) == FLASH {
            l2::colour_strip4(fx, d, t + 0x28, 0);
            l2::desc_gouraud4(fx, d, 0);
            l2::fullscreen4(fx, d);
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, 0);
        }
        if st(fx) == FLARE {
            frame(fx, 0x1C0, 0xA0, 0x10, 0x01E0_3150, s);
            tube(fx, t);
        }
        if st(fx) == SWAY {
            pair(fx, sprite_9b, 0x110, 0x50, 0, t, d, 0x01E0_3178, s);
        }
        if st(fx) == SPARK {
            pair(fx, column, 0x100, 0x30, 0, t, d, 0x01E0_31B8, s);
        }
        if st(fx) == RAY {
            pair(fx, sprite_9b, 0x110, 0x50, 0, t, d, 0x01E0_31F8, s);
        }
        if st(fx) == CLOUD {
            pair(fx, sprite_99, 0x100, 0x40, 0, t, d, 0x01E0_3238, s);
        }
    }
    for k in 0..300u32 {
        let t = POOL_B + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        s1 += 1;
        if f == 0 {
            continue;
        }
        let size = match f {
            BLUE | RING => 0x20,
            OBJECT => 0x40,
            _ => 0x30,
        };
        let s = fx.enter(size);
        if size == 0x20 {
            fx.saved(s, &[(0x10, JALR_RA), (0, u64::from(D))]);
        } else {
            fx.saved(s, &[(0x20, JALR_RA), (0x10, u64::from(s1)), (0, u64::from(D))]);
        }
        match f {
            BLUE => blue(fx, t, s1),
            FLAKE => flake(fx, t),
            RING => ring(fx, t),
            PUFF => puff(fx, t),
            OBJECT => object(fx, t),
            _ => panic!("b499: unknown task step {f:#x}"),
        }
        fx.leave(size);
    }
    for k in 0..300u32 {
        let t = POOL_B + 0x70 * k;
        // The original reads pool A's task k (+0x34 / +0xA) for some.
        let ta = POOL_A + 0x40 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == BLUE {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(ta + 0x34);
            cell(fx, sp + 0x70, 0x9B, c);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if f(fx) == FLAKE {
            let w = fx.mem.read_u32(TEX5);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x80, 0x14, c);
            let h = fx.mem.i16(ta + 0x34) << 6;
            fx.mem.set16(D + 0x94, h);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x3F);
            fx.mem.set16(D + 0x9A, 0x3F);
            fx.mem.set16(D + 0xBE, 5);
            copy4(fx, sp + 0x60, t + 0x10);
            let mode = if fx.mem.i16(t + 0x54) < 3 { 0x21 } else { 0x20 };
            billboard::quad(fx, uv::uv_rect, D, t, mode, sp + 0x60);
        }
        if f(fx) == RING {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x90, 0x14, c);
            let w = fx.mem.read_u32(TEX5 + 4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 6);
            let v = fx.mem.i16(ta + 0xA);
            fx.mem.set16(D + 0xC0, v);
            copy3(fx, D + 0x20, t + 0x10);
            quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == PUFF {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xA0, 0x14, c);
            let w = fx.mem.read_u32(TEX5 + 8);
            fx.mem.write_u32(D + 0x88, w);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x54 => 1,
        0x7D => 4,
        0x5E => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` frame `sub` (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    copy4(fx, D + 0x94, r);
}

type Draw = fn(&mut Fx, u32, u32, i32);

/// A callee frame below the current sp: ra at `ro`, the entry's s0..s3
/// from `base` up.
fn frame(fx: &mut Fx, size: u32, ro: u32, base: u32, ra: u32, s: [u32; 4]) {
    let f = fx.sp - size;
    fx.saved(f + base, &[(0x30, u64::from(s[3])), (0x20, u64::from(s[2])), (0x10, u64::from(s[1])), (0, u64::from(s[0]))]);
    fx.saved(f, &[(ro, u64::from(ra))]);
}

/// A draw called with mode 0 then 1 (the entry's ra after each `jal`:
/// `ra`, `ra` + 0x14).
#[allow(clippy::too_many_arguments)]
fn pair(fx: &mut Fx, f: Draw, size: u32, ro: u32, base: u32, t: u32, d: u32, ra: u32, s: [u32; 4]) {
    for (mode, r) in [(0, ra), (1, ra + 0x14)] {
        frame(fx, size, ro, base, r, s);
        f(fx, t, d, mode);
    }
}

/// rand % 0x8000 (C remainder) as the emitter takes it.
fn rem8000(fx: &mut Fx, s0: u32, ra: u32) -> i32 {
    fx.rand_from(s0, ra).wrapping_rem(0x8000)
}

/// 0x1E01200(D, 0): the spawn colour, the update counter, the facing and
/// the spawns (frame 0x50: ra, the entry's s2 = param, s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x30, 0x01E0_2E58), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x40);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x44)), 180.0);
    let r = fix(fx, y, 0x01E0_12A0);
    fx.mem.set16(FACE, r);
    let f = fx.mem.i16(FACE) - 0xB4;
    fx.mem.set16(FACE, f);
    let f = fx.mem.i16(FACE);
    let c = l2::sin_deg(fx, TAB, f, 1);
    let s1 = fix(fx, ee::mul(15.0, ee::neg(c)), 0x01E0_12F8);
    fx.mem.set16(SLOT30, 0);
    fx.mem.set32(POS + 0xC, 0);
    let mut s0 = D;
    // The spawns' frames: ra and the emitter's s0..s2.
    let task200 = |fx: &mut Fx, step: u32, s0: u32, ra: u32| {
        let saves = [(0x50, u64::from(ra)), (0x20, u64::from(D)), (0x10, s1 as i64 as u64), (0, u64::from(s0))];
        lspawn::task200(fx, D, step, 0, &saves);
    };
    let task300 = |fx: &mut Fx, step: u32, s0: u32, ra: u32| {
        let saves = [(0x50, u64::from(ra)), (0x20, u64::from(D)), (0x10, s1 as i64 as u64), (0, u64::from(s0))];
        lspawn::task300(fx, D, step, 0, &saves);
    };
    if fx.mem.i16(COUNTER) == 0x5D {
        task200(fx, FLASH, s0, 0x01E0_1338);
    }
    if fx.mem.i16(COUNTER) == 0x54 {
        task200(fx, FLARE, s0, 0x01E0_1364);
    }
    if fx.mem.i16(COUNTER) == 1 {
        task300(fx, OBJECT, s0, 0x01E0_1390);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x27..0x42).contains(&c) {
        s0 = 0;
        while s0 < 2 {
            for (k, ra_r, ra_s) in [(2u32, 0x01E0_13E0u32, 0x01E0_141Cu32), (3, 0x01E0_1444, 0x01E0_1480)] {
                node(fx, k, POS);
                let r = rmod(fx, s0, ra_r, 0x1E);
                let x = ee::add(fx.mem.f(POS), ee::from_i32(r - 0xF));
                fx.mem.setf(POS, x);
                task300(fx, BLUE, s0, ra_s);
            }
            s0 += 1;
        }
    }
    fx.mem.write_u8(COLOUR + 3, 0);
    node(fx, 1, POS);
    let c = fx.mem.i16(COUNTER);
    if (0x27..0x5F).contains(&c) {
        task300(fx, FLAKE, s0, 0x01E0_14F0);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x10..0x5F).contains(&c) {
        task300(fx, RING, s0, 0x01E0_1528);
    }
    if fx.mem.i16(COUNTER) == 0x27 {
        task200(fx, FLARE, s0, 0x01E0_1554);
    }
    if fx.mem.i16(COUNTER) < 0x5F {
        task300(fx, PUFF, s0, 0x01E0_1580);
    }
    if fx.mem.i16(COUNTER) < 0x5F {
        task300(fx, PUFF, s0, 0x01E0_15AC);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x58..0x5F).contains(&c) {
        task300(fx, PUFF, s0, 0x01E0_15E4);
    }
    if fx.mem.i16(COUNTER) == 0x4D {
        copy3(fx, KEEP, HIST_A);
    }
    if fx.mem.i16(COUNTER) == 0x5D {
        s0 = 0;
        while s0 < 0x28 {
            let f = -fx.mem.i16(FACE);
            let x = if f < 0x5A { rem8000(fx, s0, 0x01E0_1684) - 0x8000 } else { rem8000(fx, s0, 0x01E0_16AC) };
            fx.mem.set32(POS + 0x20, x);
            let z = rem8000(fx, s0, 0x01E0_16D0) + 0x4000;
            fx.mem.set32(POS + 0x28, z);
            for o in [0u32, 4, 8] {
                fx.mem.set32(POS + o, 0);
            }
            task200(fx, SPARK, s0, 0x01E0_170C);
            s0 += 1;
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x5D..=0x5F).contains(&c) {
        s0 = 0;
        while s0 < 0x1E {
            let r = rmod(fx, s0, 0x01E0_175C, 0x14);
            let x = ee::sub(ee::add(fx.mem.f(HIST_A), ee::from_i32(r)), 10.0);
            fx.mem.setf(POS, x);
            let r = rmod(fx, s0, 0x01E0_1798, 0x14);
            let y = ee::sub(ee::add(fx.mem.f(HIST_A + 4), ee::from_i32(r)), 10.0);
            fx.mem.setf(POS + 4, y);
            let r = rmod(fx, s0, 0x01E0_17D4, 0x14);
            let z = ee::add(ee::from_i32(s1), ee::sub(ee::add(fx.mem.f(HIST_A + 8), ee::from_i32(r)), 10.0));
            fx.mem.setf(POS + 8, z);
            let r = rmod(fx, s0, 0x01E0_181C, 0x50);
            fx.mem.set16(SLOT30, r);
            let r = rmod(fx, s0, 0x01E0_1838, 0x168);
            fx.mem.set16(SLOT3A, r);
            task200(fx, SWAY, s0, 0x01E0_1864);
            s0 += 1;
        }
    }
    if fx.mem.i16(COUNTER) == 0x5D {
        s0 = 0;
        while s0 < 0x3C {
            copy3(fx, POS, HIST_A);
            let z = ee::add(fx.mem.f(HIST_A + 8), ee::from_i32(s1));
            fx.mem.setf(POS + 8, z);
            task200(fx, RAY, s0, 0x01E0_18E4);
            s0 += 1;
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x55..0x60).contains(&c) {
        copy3(fx, POS, HIST_A);
        fx.mem.set16(SLOT30, 0);
        fx.mem.set16(SLOT3A, 0);
        task200(fx, CLOUD, s0, 0x01E0_1978);
    }
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32, at: u32) {
    let n = fx.mem.i16(t + at) + 1;
    fx.mem.set16(t + at, n);
}

/// The entry's case for step 0x1E01990: a flash's alpha to 0xF0 at once,
/// then (+0x30 = 1) down by 14 (the task ends under 0x10); colour (0x9F,
/// 0x5F, 0x5F).
fn flash(fx: &mut Fx, t: u32) {
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x30) == 0 {
        if a < 0xF0 {
            fx.mem.write_u8(t + 0x2B, 0xF0);
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
    } else if a < 0x10 {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.write_u8(t + 0x2B, a - 0xE);
    }
    l2::rgb(fx, t + 0x28, 0x9F, 0x5F, 0x5F);
}

/// i16 `at` += 20 x (sine or cosine of `deg`) through 0x1A2B80 from `ra`.
fn wobble(fx: &mut Fx, t: u32, at: u32, deg: i32, cos: i32, ra: u32) {
    let s = l2::sin_deg(fx, TAB, deg, cos);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(20.0, s));
    let r = fix(fx, f, ra);
    fx.mem.set16(t + at, r);
}

/// The entry's case for step 0x1E01AF0 (396290ea7919 inline): size 9,
/// violet, alpha up by 9 (under 0x60) for 10 updates then down by 10;
/// swaying by 20.
fn sway(fx: &mut Fx, t: u32) {
    fx.mem.set16(t + 0x38, 9);
    l2::rgb(fx, t + 0x28, 0x3F, 0, 0x6F);
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 0xA {
        if a < 0x60 {
            fx.mem.write_u8(t + 0x2B, a + 9);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    age(fx, t, 0x32);
    let g = fx.mem.i16(t + 0x3A);
    wobble(fx, t, 8, g, 0, 0x01E0_2FD4);
    let g = fx.mem.i16(t + 0x30);
    wobble(fx, t, 0xA, g, 1, 0x01E0_3010);
    let g = fx.mem.i16(t + 0x3A);
    wobble(fx, t, 0xC, g, 1, 0x01E0_304C);
}

/// 0x1E01C50 (D, task): a pale spark climbing 2 a update (height +6 up by
/// 4 to 10), alpha 0xC0, fading by 12 from 60 (both alphas); ends at 181.
fn spark(fx: &mut Fx, t: u32) {
    let h = fx.mem.i16(t + 6);
    if h < 0xA {
        fx.mem.set16(t + 6, h + 4);
    }
    let y = fx.mem.i16(t + 2) + 2;
    fx.mem.set16(t + 2, y);
    l2::rgb(fx, t + 0x28, 0xFF, 0xF0, 0xFF);
    if fx.mem.i16(t + 2) >= 0xB5 {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 2, 0xF);
        fx.mem.set16(t + 6, 0);
        fx.mem.write_u8(t + 0x2B, 0xC0);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 2) >= 0x3C {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0xE {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.write_u8(t + 0x2B, a - 0xC);
        }
        let a = fx.mem.read_u8(t + 0x2F);
        if a >= 0xE {
            fx.mem.write_u8(t + 0x2F, a - 0xC);
        }
    }
}

/// 0x1E01D30 (D, task): a ray mote swaying by 20 about the facing (the
/// angle -D+0xBB04 + +0x3A): size 1, a random cell, yellow / red / orange.
fn ray(fx: &mut Fx, t: u32) {
    let s0 = (-fx.mem.i16(FACE)) as i16 as i32;
    // rand's frame saves the sign-extended s0.
    let rmod = |fx: &mut Fx, ra: u32, m: i32| {
        let v = fx.rand_from(s0 as u32, ra);
        let f = fx.sp - 0x10;
        fx.mem.write_u64(f, s0 as i64 as u64);
        fx.div(v, m).1
    };
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 1);
        let v = fx.rand_from(s0 as u32, 0x01E0_1D78);
        let f = fx.sp - 0x10;
        fx.mem.write_u64(f, s0 as i64 as u64);
        fx.mem.set16(t + 0x34, v.wrapping_rem(4));
        let k = rmod(fx, 0x01E0_1D94, 3) as i16 as i32;
        match k {
            0 => l2::rgb(fx, t + 0x28, 0xFF, 0xF0, 0),
            1 => l2::rgb(fx, t + 0x28, 0xFF, 0, 0),
            2 => l2::rgb(fx, t + 0x28, 0xFF, 0x80, 0),
            _ => {}
        }
        let r = rmod(fx, 0x01E0_1E24, 0xB4);
        fx.mem.set16(t + 0x3A, r + 0x5A);
        let r = rmod(fx, 0x01E0_1E44, 0x168);
        fx.mem.set16(t + 0x30, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x11 {
            fx.mem.write_u8(t + 0x2B, a - 0xA);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    age(fx, t, 0x32);
    let g = (s0 + fx.mem.i16(t + 0x3A)) as i16 as i32;
    wobble(fx, t, 8, g, 0, 0x01E0_1EF8);
    let g = fx.mem.i16(t + 0x30);
    wobble(fx, t, 0xA, g, 0, 0x01E0_1F34);
    let g = (s0 + fx.mem.i16(t + 0x3A)) as i16 as i32;
    wobble(fx, t, 0xC, g, 1, 0x01E0_1F7C);
}

/// 0x1E01FA0 (D, task): the red cloud: size 6 (22 from update 94) falling
/// by 1 every third update, cells to 7, alpha up by 0x50 for 2 updates
/// then down by 10; 0x20 on update 96, the end after it.
fn cloud(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x9F, 0, 0x3F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.set16(t + 0x34, 0);
        let s = if fx.mem.i16(COUNTER) < 0x5E { 6 } else { 0x16 };
        fx.mem.set16(t + 0x38, s);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < 2 {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x50));
    } else if a < 0xC {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.write_u8(t + 0x2B, a - 0xA);
    }
    let c = fx.mem.i16(t + 0x34);
    if c < 7 {
        fx.mem.set16(t + 0x34, c + 1);
    }
    let n = fx.mem.i16(t + 0x32);
    if fx.div(n, 3).1 == 1 {
        let s = fx.mem.i16(t + 0x38);
        if s != 0 {
            fx.mem.set16(t + 0x38, s - 1);
        }
    }
    age(fx, t, 0x32);
    if fx.mem.i16(COUNTER) == 0x60 {
        fx.mem.write_u8(t + 0x2B, 0x20);
    }
    if fx.mem.i16(COUNTER) >= 0x61 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// (rand % 0x800) - 0x400 (C remainder).
fn turn(fx: &mut Fx, t: u32, ra: u32) -> i32 {
    fx.rand_from(t, ra).wrapping_rem(0x800) - 0x400
}

/// The three random turns +4 / +2 / +0 a first update takes.
fn turns(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    for (o, ra) in [(4u32, ras[0]), (2, ras[1]), (0, ras[2])] {
        let v = turn(fx, t, ra);
        fx.mem.set16(t + o, v);
    }
}

/// Alternate updates' colour +0x60: (0x80, 0x80, 0x80) on odd ages, else
/// (0xC0, 0xC0, 0xC0).
fn blink(fx: &mut Fx, t: u32) {
    let c = if fx.mem.i16(t + 0x54) % 2 == 1 { 0x80 } else { 0xC0 };
    l2::rgb(fx, t + 0x60, c, c, c);
}

/// The i32 angles +0x20.. += the i16 turns +4 / +2 / +0, then the camera's
/// angles over them.
fn spin(fx: &mut Fx, t: u32, s1: u64) {
    for (w, h) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.i32(t + w).wrapping_add(fx.mem.i16(t + h));
        fx.mem.set32(t + w, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), s1, 0x01E0_234C);
}

/// 0x1E02110 (D, task): a blue spark drifting by +0x30 / +0x34 (y falling
/// by 0.09 a update), alpha up by 0x40 (under 0x80) for 20 updates.
fn blue(fx: &mut Fx, t: u32, s1: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let r = rmod(fx, t, 0x01E0_2134, 3);
        fx.mem.set16(t + 8, r + 3);
        let v = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, v);
        let v = fx.rand_from(t, 0x01E0_215C) >> 4;
        let m = fx.div(v, 3).1;
        l2::rgb(fx, t + 0x60, 0x40, 0x10, 0x80);
        fx.mem.set16(t + 0x56, m);
        fx.mem.set16(t + 0x58, 2);
        let r = rmod(fx, t, 0x01E0_2190, 0x64);
        fx.mem.setf(t + 0x34, ee::from_i32(r - 0x32));
        let r = rmod(fx, t, 0x01E0_21BC, 0x64);
        fx.mem.setf(t + 0x30, ee::from_i32(r - 0x32));
        for o in [0x34u32, 0x30] {
            let v = ee::div(fx.mem.f(t + o), 400.0);
            fx.mem.setf(t + o, v);
        }
        let r = rmod(fx, t, 0x01E0_2208, 0x168);
        fx.mem.set16(t + 0x5C, r);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(s1), 0x01E0_2224);
        turns(fx, t, [0x01E0_222C, 0x01E0_224C, 0x01E0_226C]);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x14 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a < 0xB {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.write_u8(t + 0x63, a - 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let x = ee::add(fx.mem.f(t + 0x10), fx.mem.f(t + 0x30));
        fx.mem.setf(t + 0x10, x);
        let dy = ee::sub(fx.mem.f(t + 0x34), f32::from_bits(0x3DB8_51EC));
        let y = ee::add(fx.mem.f(t + 0x14), dy);
        fx.mem.setf(t + 0x14, y);
    }
    spin(fx, t, u64::from(s1));
    age(fx, t, 0x54);
}

/// 0x1E02370 (D, task): a white flake at node 0x2C, size 35 shrinking by
/// 2, both alphas up by 0x40 for 3 updates, then the first down by 9 twice
/// an update; +0x56 flips; blinking.
fn flake(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x23);
        fx.mem.set16(t + 0xA, 0x23);
        let v = fx.rand_from(t, 0x01E0_23A4);
        fx.mem.set16(t + 0x56, v.wrapping_rem(2));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        node(fx, 0, t + 0x10);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0xC0 {
                fx.mem.write_u8(t + o, a + 0x40);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0xB {
            fx.mem.set16(t + 0x58, 0);
        } else {
            fx.mem.write_u8(t + 0x63, a - 9);
        }
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x63, a - 9);
        }
    }
    let f = if fx.mem.i16(t + 0x56) != 0 { 0 } else { 1 };
    fx.mem.set16(t + 0x56, f);
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let v = fx.mem.i16(t + o) - 2;
            fx.mem.set16(t + o, v);
        }
    }
    blink(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2528);
    age(fx, t, 0x54);
}

/// 0x1E02550 (D, task): a ring of quads (ring_facing_flat's record): radii,
/// heights, counts and spans at random; alpha up by 0x20 for 5 updates,
/// then down by 8; blinking.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let r = rmod(fx, t, 0x01E0_2578, 7);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x56, r);
        let r = rmod(fx, t, 0x01E0_25A4, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rmod(fx, t, 0x01E0_25C0, 0x168);
        fx.mem.set16(t + 0x5C, r);
        let v = fx.rand_from(t, 0x01E0_25DC).wrapping_rem(8);
        fx.mem.set16(t + 0xA, v);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = fx.rand_from(t, 0x01E0_2600) >> 4;
        let m = fx.div(v, 0x19).1;
        fx.mem.set16(t + 0x50, m + 5);
        let r = rmod(fx, t, 0x01E0_2624, 0x1C2);
        fx.mem.set16(t + 0x52, r + 0x12C);
        let r = rmod(fx, t, 0x01E0_2644, 0x14);
        fx.mem.set16(t + 0xE, r + 0xA);
        let r = rmod(fx, t, 0x01E0_2664, 0x23);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_2680, 5);
        fx.mem.set16(t + 6, r + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 5 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x20);
        }
    } else if a < 0xA {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.write_u8(t + 0x63, a - 8);
    }
    blink(fx, t);
    age(fx, t, 0x54);
}

/// 0x1E02770 (D, task): a grey puff at node 0x2D (size 20..39, from update
/// 88 50..69 with a jittered y / z) drifting by +0x30.. / 200, growing by 2
/// (every third update before 88); alpha up by 0x40 for 8 updates.
fn puff(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(COUNTER) < 0x58 {
            let r = rmod(fx, t, 0x01E0_27B4, 0x14);
            fx.mem.set16(t + 8, r + 0x14);
            let v = fx.mem.i16(t + 8);
            fx.mem.set16(t + 0xA, v);
        } else {
            let r = rmod(fx, t, 0x01E0_27E4, 0x14);
            fx.mem.set16(t + 8, r + 0x32);
            let v = fx.mem.i16(t + 8);
            fx.mem.set16(t + 0xA, v);
            for (o, ra) in [(0x14u32, 0x01E0_280Cu32), (0x18, 0x01E0_2840)] {
                let r = rmod(fx, t, ra, 0xA);
                let v = ee::add(fx.mem.f(t + o), ee::from_i32(r - 5));
                fx.mem.setf(t + o, v);
            }
        }
        let v = fx.rand_from(t, 0x01E0_2878) >> 4;
        let m = fx.div(v, 3).1;
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x60);
        fx.mem.set16(t + 0x56, m);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_28ACu32), (0x30, 0x01E0_28D8), (0x38, 0x01E0_2904)] {
            let r = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(r - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let v = ee::div(fx.mem.f(t + o), 200.0);
            fx.mem.setf(t + o, v);
        }
        let r = rmod(fx, t, 0x01E0_295C, 0x168);
        fx.mem.set16(t + 0x5C, r);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2978);
        turns(fx, t, [0x01E0_2980, 0x01E0_29A0, 0x01E0_29C0]);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 8 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a < 0xB {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.write_u8(t + 0x63, a - 9);
    }
    let grow = |fx: &mut Fx| {
        let v = fx.mem.i16(t + 8) + 2;
        fx.mem.set16(t + 8, v);
        fx.mem.set16(t + 0xA, v);
    };
    if fx.mem.i16(COUNTER) < 0x58 {
        let n = fx.mem.i16(t + 0x54);
        if fx.div(n, 3).1 == 1 {
            grow(fx);
        }
    } else {
        grow(fx);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for (p, v) in [(0x10u32, 0x30u32), (0x14, 0x34), (0x18, 0x38)] {
            let x = ee::add(fx.mem.f(t + p), fx.mem.f(t + v));
            fx.mem.setf(t + p, x);
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2AD8);
    age(fx, t, 0x54);
}

/// 0x1E02B00 (D, task): the object op 0x1F8380(1, 0, the record's floats
/// 0x1E04500 / 0x1E04508 at sp + 0x38 / + 0x3C, 0) each update, to 115.
fn object(fx: &mut Fx, t: u32) {
    let sp = fx.sp;
    for (to, from) in [(0x38u32, 0x01E0_4500u32), (0x3C, 0x01E0_4508)] {
        let w = fx.mem.read_u32(from);
        fx.mem.write_u32(sp + to, w);
    }
    fx.boundary(0x001F_8380, "object op 0x1F8380", [1, 0, sp + 0x38, sp + 0x3C]);
    if fx.mem.i16(COUNTER) == 0x73 {
        fx.mem.set16(t + 0x58, 0);
    }
}

// ---------------------------------------------------------------- draws

fn setup(fx: &mut Fx, t: u32, d: u32, r: u32, id: i32) {
    let c = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, c);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    l2::uv_strip3(fx, d, r, 0);
    let w = fx.mem.read_u32(TEX);
    fx.desc_texture(d, w, 1);
}

/// 0x1E03DD0(D, task, desc, mode): a camera-facing quad of half size i16
/// +0x38 at the task's position (+ offsets / 10), sprite 0x9B, texture
/// D+0x34C.
fn sprite_9b(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        setup(fx, t, d, sp + 0xB0, 0x9B);
    } else {
        camera_angles_saving(fx, sp + 0x100, u64::from(OT), u64::from(d), 0x01E0_3E6C);
        for k in 0..3u32 {
            let off = fx.mem.i16(t + 8 + 2 * k);
            fx.mult(0x6666_6667, off);
            let hi = fx.hilo().1;
            let q = (hi >> 2).wrapping_add(((off as u32) >> 31) as i32);
            let v = fx.mem.i16(t + 2 * k).wrapping_add(q);
            fx.mem.setf(sp + 0xA0 + 4 * k, ee::from_i32(v));
        }
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
    }
    fx.leave(0x110);
}

/// 0x1E04350(D, task, desc, mode): a camera-facing quad of half size i16
/// +0x38 at the task's position, sprite 0x99, texture D+0x34C, at the far
/// Z, keyed by its mean w (D+0xBAE4) on D+0xBABC.
fn sprite_99(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x100);
    if mode == 0 {
        setup(fx, t, d, sp + 0xA0, 0x99);
    } else {
        camera_angles_saving(fx, sp + 0xF0, u64::from(D), 1, 0x01E0_43EC);
        for k in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0x90 + 4 * k, v);
        }
        l2::xform_vp(fx, sp + 0xB0, 0, sp + 0xF0, sp + 0x90);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x50, s, s);
        if l2::project(fx, sp + 0x50, SPR, sp + 0xB0, d, MEAN) {
            l2::far_z_desc(fx, d, 0);
            let m = fx.mem.f(MEAN);
            let key = fixu(fx, m, 0x01E0_4494);
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x100);
}

/// 0x1E03F70(D, task, desc, mode): the light column (b544's 0x1E038B0 with
/// sprite 0x9C cell +0x34, colours +0x28 / +0x2C, texture D+0x34C, width
/// 1, the point D+0xBA80, angles +0x10).
fn column(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x100);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xD0, 0x9C, c);
        l2::desc_strip3(fx, d);
        rgbaq_at(fx, sp + 0xE0, t + 0x28, d + 0x10);
        rgbaq_at(fx, sp + 0xE8, t + 0x28, d + 0x28);
        rgbaq_at(fx, sp + 0xF0, t + 0x2C, d + 0x40);
        rgbaq_at(fx, sp + 0xF8, t + 0x2C, d + 0x58);
        l2::uv_strip3(fx, d, sp + 0xD0, 0);
        let w = fx.mem.read_u32(TEX);
        fx.desc_texture(d, w, 1);
    } else {
        camera_vp_into(fx, VP);
        l2::copy3_dir(fx, HIST_A, sp + 0xC0, 1);
        copy3(fx, sp + 0xC0, HOLD);
        rot_trans_into(fx, sp + 0x80, t + 0x10, sp + 0xC0);
        mul_mem(fx, VP, sp + 0x80);
        for k in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0x40 + 4 * k, v);
        }
        copy4(fx, sp + 0x50, sp + 0x40);
        let x = ee::add(fx.mem.f(sp + 0x50), 1.0);
        fx.mem.setf(sp + 0x50, x);
        for k in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0x60 + 4 * k, v);
        }
        let y = ee::sub(fx.mem.f(sp + 0x64), ee::from_i32(fx.mem.i16(t + 6)));
        fx.mem.setf(sp + 0x64, y);
        copy4(fx, sp + 0x70, sp + 0x60);
        let x = ee::add(fx.mem.f(sp + 0x70), 1.0);
        fx.mem.setf(sp + 0x70, x);
        for o in [0x44u32, 0x54, 0x64, 0x74] {
            if !ee_le(fx.mem.f(sp + o), 200.0) {
                fx.mem.write_u32(sp + o, 0x4348_0000);
            }
        }
        for o in [0x44u32, 0x54, 0x64, 0x74] {
            if ee_lt(fx.mem.f(sp + o), 0.0) {
                fx.mem.set32(sp + o, 0);
            }
        }
        if l2::project(fx, sp + 0x40, SPR, VP, d, MEAN) {
            let m = fx.mem.f(MEAN);
            let key = fixu(fx, m, 0x01E0_431C);
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x100);
}

/// 0x1E038F0(D, task, desc, 0): a gouraud tube along node 0x2C's history
/// (9 segments of 18 quads, radius 2 + 0.3 a segment, the outer ring 0.3
/// wider), the task's colour with alpha falling by alpha / 10 a segment;
/// at the far Z, key 0.
fn tube(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x1C0);
    fx.mem.write_u32(sp + 0xCC, t);
    fx.mem.write_u32(sp + 0xC8, OT);
    let s1 = sp + 0x128;
    let mut f20 = 2.0f32;
    let wide = f32::from_bits(0x3E99_999A);
    l2::desc_gouraud4(fx, s1, 0);
    l2::fullscreen4(fx, s1);
    fx.mem.set32(sp + 0xB0, 0);
    copy_bytes4(fx, sp + 0x1BC, t + 0x28);
    for seg in 0..9u32 {
        l2::colour_strip4(fx, s1, sp + 0x1BC, 1);
        let al = i32::from(fx.mem.read_u8(t + 0x2B));
        fx.mult(0x6666_6667, al);
        let hi = fx.hilo().1;
        let q = (hi >> 2).wrapping_add(((al as u32) >> 31) as i32);
        let b = fx.mem.read_u8(sp + 0x1BF).wrapping_sub(q as u8);
        fx.mem.write_u8(sp + 0x1BF, b);
        l2::colour_strip4(fx, s1, sp + 0x1BC, 2);
        let p = HIST_A + 0x10 * seg;
        let mut s2 = 0i32;
        for _ in 0..0x12 {
            let s4 = (s2 + 0x14) as i16 as i32;
            for (src, o, r) in [(p, 0xE0u32, f20), (p + 0x10, 0x100, ee::add(f20, wide))] {
                copy3(fx, sp + o, src);
                copy3(fx, sp + o + 0x10, src);
                for (ang, at) in [(s2, o), (s4, o + 0x10)] {
                    let c = l2::sin_deg(fx, TAB, ang, 1);
                    let y = ee::add(fx.mem.f(sp + at + 4), ee::mul(r, c));
                    fx.mem.setf(sp + at + 4, y);
                    let s = l2::sin_deg(fx, TAB, ang, 0);
                    let x = ee::add(fx.mem.f(sp + at), ee::mul(r, s));
                    fx.mem.setf(sp + at, x);
                    let c = l2::sin_deg(fx, TAB, ang, 1);
                    let z = ee::add(fx.mem.f(sp + at + 8), ee::mul(r, c));
                    fx.mem.setf(sp + at + 8, z);
                }
            }
            if l2::project(fx, sp + 0xE0, SPR, 0, s1, sp + 0xD0) {
                l2::far_z_desc(fx, s1, 1);
                fx.submit(s1, OT, 0);
            }
            s2 = s4;
        }
        f20 = ee::add(f20, wide);
        fx.mem.set32(sp + 0xB0, seg as i32 + 1);
    }
    fx.leave(0x1C0);
}

fn copy_bytes4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let b = fx.mem.read_u8(from + k);
        fx.mem.write_u8(to + k, b);
    }
}
