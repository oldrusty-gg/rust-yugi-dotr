//! BMODULE b479 (card 414), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02980) [F]. b499's
//! two-pool program on the same layout (200 tasks of 0x40 bytes at D+0x3F0
//! spawned by e8e1debea8f3 [`spawn::task200`], 300 of 0x70 bytes at
//! D+0x35F0 by 1f31920d0d56 [`spawn::task300`]) without the node
//! histories, four textures (5, k) and its own emitter 0x1E01490: update 1
//! a grey / red arc ring (0x1E02850, radius 60) following the attacker's
//! node 0x26; to update 0x33 a white flake (0x1E01CE0), a pink puff
//! (0x1E024C0) and a ring of quads (0x1E01EB0) a update at node 0x26; on
//! 0x33 a red flash, twenty b701 red fliers (0x1E020E0) and b701's arc ring
//! (0x1E02380, radius 160) there, and forty white sparks (0x1E01A70) at
//! node 0x45 drawn as light columns (0x1E03630, b544's compile); 0x55..0x5F
//! a red cloud (0x1E01B70) at the never-written history D+0xB930 (0, 0, 0).
//! The steps 0x1E01910 (b499's sway) and 0x1E01B60 are compared but never
//! spawned (not ported). Returns 1 on update 0x29, 4 on 0x7D, 2 on 0x50, 6
//! on 0x34..0x4A, 8 once the 0x80-update timer D+0xBAB0 has run out.
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
const FLASH: u32 = 0x01E0_1890;
const SPARK: u32 = 0x01E0_1A70;
const CLOUD: u32 = 0x01E0_1B70;
/// Pool B's steps.
const FLAKE: u32 = 0x01E0_1CE0;
const RING: u32 = 0x01E0_1EB0;
const FLY: u32 = 0x01E0_20E0;
const ARC: u32 = 0x01E0_2380;
const PUFF: u32 = 0x01E0_24C0;
const GLOW: u32 = 0x01E0_2850;
const JALR_RA: u64 = 0x01E0_2F0C;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x140);
    fx.mem.write_u32(OTW, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x140);
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
    fx.mem.set32(TIMER, 0x80);
    for (k, node) in [0x26, 0x2D, 0x45, 0x4C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(TEX, t);
    for k in 0..4u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX5 + 4 * k, t);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, 0x01E0_2AC8, 0x1E);
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
        if st(fx) == SPARK {
            let s = fx.enter(0x20);
            fx.saved(s, &[(0x10, 0x01E0_2D38), (0, u64::from(D))]);
            spark(fx, t);
            fx.leave(0x20);
        }
        if st(fx) == CLOUD {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, 0x01E0_2D58), (0x10, u64::from(s1)), (0, u64::from(D))]);
            cloud(fx, t);
            fx.leave(0x30);
        }
    }
    let d = sp + 0xB0;
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
        if st(fx) == SPARK {
            pair(fx, column, 0x100, 0x30, 0, t, d, 0x01E0_2E38, s);
        }
        if st(fx) == CLOUD {
            pair(fx, sprite_99, 0x100, 0x40, 0, t, d, 0x01E0_2EB8, s);
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
            RING | FLY => 0x20,
            _ => 0x30,
        };
        let s = fx.enter(size);
        if size == 0x20 {
            fx.saved(s, &[(0x10, JALR_RA), (0, u64::from(D))]);
        } else {
            fx.saved(s, &[(0x20, JALR_RA), (0x10, u64::from(s1)), (0, u64::from(D))]);
        }
        match f {
            FLAKE => flake(fx, t),
            RING => ring(fx, t),
            FLY => fly(fx, t, s1),
            ARC => arc(fx, t),
            PUFF => puff(fx, t),
            GLOW => glow(fx, t),
            _ => panic!("b479: unknown task step {f:#x}"),
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
        if f(fx) == FLAKE {
            let w = fx.mem.read_u32(TEX5);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x14, c);
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
            cell(fx, sp + 0x80, 0x14, c);
            let w = fx.mem.read_u32(TEX5 + 4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 6);
            let v = fx.mem.i16(ta + 0xA);
            fx.mem.set16(D + 0xC0, v);
            copy3(fx, D + 0x20, t + 0x10);
            quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == FLY {
            let w = fx.mem.read_u32(TEX5 + 0xC);
            fx.mem.write_u32(D + 0x88, w);
            let c = fx.mem.i16(ta + 0x34);
            cell(fx, sp + 0x90, 0x9B, c);
            fx.mem.set16(D + 0x94, 0);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x40);
            fx.mem.set16(D + 0x9A, 0x40);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if f(fx) == ARC {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if f(fx) == PUFF {
            let c = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xA0, 0x14, c);
            let w = fx.mem.read_u32(TEX5 + 8);
            fx.mem.write_u32(D + 0x88, w);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
        }
        if f(fx) == GLOW {
            fx.mem.set16(D + 0xB8, 0x24);
            billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x29 => 1,
        0x7D => 4,
        0x50 => 2,
        0x34..=0x4A => 6,
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

/// 0x1E01490(D, 0): the spawn colour, the update counter, the facing and
/// the spawns (frame 0x40: ra, the entry's s1 = 0, s0 = D; its own s1 = D,
/// s0 the loop counters, the entry's s0 = D before the first loop).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, 0x01E0_2B44), (0x10, 0), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x30);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x34)), 180.0);
    let r = fix(fx, y, 0x01E0_152C);
    fx.mem.set16(FACE, r);
    let f = fx.mem.i16(FACE) - 0xB4;
    fx.mem.set16(FACE, f);
    let f = fx.mem.i16(FACE);
    // The cosine is taken and left unused.
    let _ = l2::sin_deg(fx, TAB, f, 1);
    fx.mem.set16(SLOT30, 0);
    fx.mem.set32(POS + 0xC, 0);
    // The spawns' frames: ra and the emitter's s0..s2 (s2 the entry's =
    // param).
    let saves = |s0: u32, ra: u32| [(0x50, u64::from(ra)), (0x20, param as i64 as u64), (0x10, u64::from(D)), (0, u64::from(s0))];
    let task200 = |fx: &mut Fx, step: u32, s0: u32, ra: u32| {
        lspawn::task200(fx, D, step, 0, &saves(s0, ra));
    };
    let task300 = |fx: &mut Fx, step: u32, s0: u32, ra: u32| {
        lspawn::task300(fx, D, step, 0, &saves(s0, ra));
    };
    let mut s0 = D;
    if fx.mem.i16(COUNTER) == 0x33 {
        task200(fx, FLASH, s0, 0x01E0_15AC);
    }
    if fx.mem.i16(COUNTER) == 1 {
        fx.mem.write_u8(COLOUR + 3, 0);
    }
    node(fx, 0, POS);
    if fx.mem.i16(COUNTER) == 1 {
        task300(fx, GLOW, s0, 0x01E0_161C);
    }
    for (f, ra) in [(FLAKE, 0x01E0_1648u32), (PUFF, 0x01E0_1674), (RING, 0x01E0_16A0)] {
        if fx.mem.i16(COUNTER) < 0x34 {
            task300(fx, f, s0, ra);
        }
    }
    if fx.mem.i16(COUNTER) == 0x33 {
        s0 = 0;
        while s0 < 0x14 {
            task300(fx, FLY, s0, 0x01E0_16D0);
            s0 += 1;
        }
        task300(fx, ARC, s0, 0x01E0_16F4);
    }
    if fx.mem.i16(COUNTER) == 0x33 {
        copy3(fx, HOLD, POS);
    }
    if fx.mem.i16(COUNTER) == 0x33 {
        node(fx, 3, POS);
        s0 = 0;
        while s0 < 0x28 {
            for (o, ra) in [(0x20u32, 0x01E0_1780u32), (0x24, 0x01E0_17A0), (0x28, 0x01E0_17C0)] {
                let v = fx.rand_from(s0, ra) >> 4;
                let m = fx.div(v, 0xFFFF).1;
                fx.mem.set32(POS + o, m);
            }
            task200(fx, SPARK, s0, 0x01E0_17E8);
            s0 += 1;
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x55..0x60).contains(&c) {
        copy3(fx, POS, HIST_A);
        fx.mem.set16(SLOT30, 0);
        fx.mem.set16(SLOT3A, 0);
        task200(fx, CLOUD, s0, 0x01E0_1878);
    }
    fx.leave(0x40);
}

fn age(fx: &mut Fx, t: u32, at: u32) {
    let n = fx.mem.i16(t + at) + 1;
    fx.mem.set16(t + at, n);
}

/// The entry's case for step 0x1E01890: a flash's alpha to 0xF0 at once,
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

/// 0x1E01A70 (D, task): a white spark column climbing 2 a update (height
/// +6 up by 4 to 70) from 5, alpha 0xC0, fading by 12 from 60 (both
/// alphas); ends at 181.
fn spark(fx: &mut Fx, t: u32) {
    let h = fx.mem.i16(t + 6);
    if h < 0x46 {
        fx.mem.set16(t + 6, h + 4);
    }
    let y = fx.mem.i16(t + 2) + 2;
    fx.mem.set16(t + 2, y);
    l2::rgb(fx, t + 0x28, 0xFF, 0xFF, 0xFF);
    if fx.mem.i16(t + 2) >= 0xB5 {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 2, 5);
        fx.mem.set16(t, 0);
        fx.mem.set16(t + 4, 0);
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

/// 0x1E01B70 (D, task): b499's red cloud: size 6 (22 from update 94)
/// falling by 1 every third update, cells to 7, alpha up by 0x50 for 2
/// updates then down by 10; 0x20 on update 96, the end after it.
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

/// Alternate updates' colour +0x60: `odd` on odd ages, else `even`.
fn blink(fx: &mut Fx, t: u32, odd: i32, even: i32) {
    let c = if fx.mem.i16(t + 0x54) % 2 == 1 { odd } else { even };
    l2::rgb(fx, t + 0x60, c, c, c);
}

/// 0x1E01CE0 (D, task): b499's flake at node 0x26 in grey (cell 5), size
/// 35 shrinking by 2, both alphas up by 0x40 for 3 updates, then the first
/// down by 9 twice an update; +0x56 flips; blinking 0x20 / 0x40.
fn flake(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x23);
        fx.mem.set16(t + 0xA, 0x23);
        fx.mem.set16(t + 0x56, 5);
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
    blink(fx, t, 0x20, 0x40);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1E84);
    age(fx, t, 0x54);
}

/// 0x1E01EB0 (D, task): b499's ring of quads with its count +0x50 growing
/// by +0xE an update after the first; alpha up by 0x20 for 5 updates,
/// then down by 8; blinking.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let r = rmod(fx, t, 0x01E0_1ED8, 7);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x56, r);
        let r = rmod(fx, t, 0x01E0_1F04, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rmod(fx, t, 0x01E0_1F20, 0x168);
        fx.mem.set16(t + 0x5C, r);
        let v = fx.rand_from(t, 0x01E0_1F3C).wrapping_rem(8);
        fx.mem.set16(t + 0xA, v);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = fx.rand_from(t, 0x01E0_1F60) >> 4;
        let m = fx.div(v, 0x19).1;
        fx.mem.set16(t + 0x50, m + 5);
        let r = rmod(fx, t, 0x01E0_1F84, 0x1C2);
        fx.mem.set16(t + 0x52, r + 0x12C);
        let r = rmod(fx, t, 0x01E0_1FA4, 0x14);
        fx.mem.set16(t + 0xE, r + 0xA);
        let r = rmod(fx, t, 0x01E0_1FC4, 0x23);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, 0x01E0_1FE0, 5);
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
    blink(fx, t, 0x80, 0xC0);
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50).wrapping_add(fx.mem.i16(t + 0xE));
        fx.mem.set16(t + 0x50, v);
    }
    age(fx, t, 0x54);
}

/// 0x1E020E0 (D, task): b701's 0x1E02030 word for word: a red quad of
/// size 100 growing by 10 to 200 (a random cell row), flying (-50..49) /
/// 60 a update, turned by the camera's angles; in by 0x40 under 0x80 for
/// 20 updates, out by 9. `s1` the entry's.
fn fly(fx: &mut Fx, t: u32, s1: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
        let v = fx.rand_from(t, 0x01E0_2110);
        fx.mem.set16(t + 0x56, (v >> 4) % 3);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_2144u32), (0x30, 0x01E0_2170), (0x38, 0x01E0_219C)] {
            let v = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(v - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let q = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, q);
        }
        let v = rmod(fx, t, 0x01E0_21F4, 0x168);
        fx.mem.set16(t + 0x5C, v);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(s1), 0x01E0_2210);
        for (o, ra) in [(4u32, 0x01E0_2218u32), (2, 0x01E0_2238), (0, 0x01E0_2258)] {
            let v = fx.rand_from(t, ra);
            fx.mem.set16(t + o, v % 0x800 - 0x400);
        }
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x14 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let s = fx.mem.i16(t + 8);
        if s < 0xC8 {
            fx.mem.set16(t + 8, s + 0xA);
        }
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
        for o in [0u32, 4, 8] {
            let v = ee::add(fx.mem.f(t + 0x10 + o), fx.mem.f(t + 0x30 + o));
            fx.mem.setf(t + 0x10 + o, v);
        }
    }
    for (o, by) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.read_u32(t + o).wrapping_add(fx.mem.i16(t + by) as u32);
        fx.mem.write_u32(t + o, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(s1), 0x01E0_2360);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E02380 (D, task): b701's arc ring (0x1E022D0) with radius 160: white
/// (red inner) growing by 0.4, angles 10 / 10, facing the camera turned a
/// quarter; its outer alpha in by 0x60 under 0xC0 for 8 updates, out by
/// 15; it ends on update 195.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.write_u32(t + 0x4C, 0x4320_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 8 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a + 0x60);
        }
    } else if a >= 0x11 {
        fx.mem.write_u8(t + 0x67, a - 0xF);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2448);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x4C), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(t + 0x4C, r);
    }
    if fx.mem.i16(COUNTER) == 0xC3 {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// 0x1E024C0 (D, task): b499's puff in pink (0xFF, 0xC0, 0xC0) at node
/// 0x26 (size 50..79, from update 88 50..69 with a jittered y / z) drifting
/// by +0x30.. / 200, growing by 2 (every third update before 88); alpha up
/// by 0x40 for 8 updates.
fn puff(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(COUNTER) < 0x58 {
            let r = rmod(fx, t, 0x01E0_2504, 0x1E);
            fx.mem.set16(t + 8, r + 0x32);
            let v = fx.mem.i16(t + 8);
            fx.mem.set16(t + 0xA, v);
        } else {
            let r = rmod(fx, t, 0x01E0_2534, 0x14);
            fx.mem.set16(t + 8, r + 0x32);
            let v = fx.mem.i16(t + 8);
            fx.mem.set16(t + 0xA, v);
            for (o, ra) in [(0x14u32, 0x01E0_255Cu32), (0x18, 0x01E0_2590)] {
                let r = rmod(fx, t, ra, 0xA);
                let v = ee::add(fx.mem.f(t + o), ee::from_i32(r - 5));
                fx.mem.setf(t + o, v);
            }
        }
        let v = fx.rand_from(t, 0x01E0_25C8) >> 4;
        let m = fx.div(v, 3).1;
        l2::rgb(fx, t + 0x60, 0xFF, 0xC0, 0xC0);
        fx.mem.set16(t + 0x56, m);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x34u32, 0x01E0_25FCu32), (0x30, 0x01E0_2628), (0x38, 0x01E0_2654)] {
            let r = rmod(fx, t, ra, 0x64);
            fx.mem.setf(t + o, ee::from_i32(r - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let v = ee::div(fx.mem.f(t + o), 200.0);
            fx.mem.setf(t + o, v);
        }
        let r = rmod(fx, t, 0x01E0_26AC, 0x168);
        fx.mem.set16(t + 0x5C, r);
        camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_26C8);
        turns(fx, t, [0x01E0_26D0, 0x01E0_26F0, 0x01E0_2710]);
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
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2828);
    age(fx, t, 0x54);
}

/// 0x1E02850 (D, task): an arc ring (radius 60, white outer / red inner,
/// angles 10 / 10) at node 0x26 to update 0x33, facing the camera turned a
/// quarter; its outer alpha in by 0x60 under 0xC0 for 51 updates, out by
/// 15.
fn glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4270_0000);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < 0x33 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x67, a + 0x60);
        }
    } else if a < 0x11 {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.write_u8(t + 0x67, a - 0xF);
    }
    if fx.mem.i16(t + 0x54) < 0x34 {
        node(fx, 0, t + 0x10);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2948);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t, 0x54);
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

/// 0x1E03A40(D, task, desc, mode): b499's 0x1E04350 word for word: a
/// camera-facing quad of half size i16 +0x38 at the task's position, sprite
/// 0x99, texture D+0x34C, at the far Z, keyed by its mean w (D+0xBAE4) on
/// D+0xBABC.
fn sprite_99(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x100);
    if mode == 0 {
        setup(fx, t, d, sp + 0xA0, 0x99);
    } else {
        camera_angles_saving(fx, sp + 0xF0, u64::from(D), 1, 0x01E0_3ADC);
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
            let key = fixu(fx, m, 0x01E0_3B84);
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x100);
}

/// 0x1E03630(D, task, desc, mode): the light column (b544's 0x1E038B0 on
/// pool A's layout): mode 0 a strip3 of sprite 0x14 cell +0x34 (texture
/// D+0xBA98), bottom +0x28 / top +0x2C colours; mode 1 its four corners
/// (the task's i16 point, x + 4.7; the top y - height +6, all y clamped to
/// 0..200) through the camera x the task's angles +0x10 at the point
/// D+0xBA80, keyed by the mean w (D+0xBAE4) on D+0xBABC.
fn column(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x100);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xD0, 0x14, c);
        l2::desc_strip3(fx, d);
        rgbaq_at(fx, sp + 0xE0, t + 0x28, d + 0x10);
        rgbaq_at(fx, sp + 0xE8, t + 0x28, d + 0x28);
        rgbaq_at(fx, sp + 0xF0, t + 0x2C, d + 0x40);
        rgbaq_at(fx, sp + 0xF8, t + 0x2C, d + 0x58);
        l2::uv_strip3(fx, d, sp + 0xD0, 0);
        let w = fx.mem.read_u32(TEX5 + 8);
        fx.desc_texture(d, w, 1);
    } else {
        camera_vp_into(fx, VP);
        l2::copy3_dir(fx, HIST_A, sp + 0xC0, 1);
        copy3(fx, sp + 0xC0, HOLD);
        rot_trans_into(fx, sp + 0x80, t + 0x10, sp + 0xC0);
        mul_mem(fx, VP, sp + 0x80);
        let wide = f32::from_bits(0x4096_6666);
        for k in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0x40 + 4 * k, v);
        }
        copy4(fx, sp + 0x50, sp + 0x40);
        let x = ee::add(fx.mem.f(sp + 0x50), wide);
        fx.mem.setf(sp + 0x50, x);
        for k in 0..3u32 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(sp + 0x60 + 4 * k, v);
        }
        let y = ee::sub(fx.mem.f(sp + 0x64), ee::from_i32(fx.mem.i16(t + 6)));
        fx.mem.setf(sp + 0x64, y);
        if ee_lt(y, 0.0) {
            fx.mem.set32(sp + 0x64, 0);
        }
        copy4(fx, sp + 0x70, sp + 0x60);
        let x = ee::add(fx.mem.f(sp + 0x70), wide);
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
            let key = fixu(fx, m, 0x01E0_3A0C);
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x100);
}
