//! BMODULE b004 (card 292), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E00C40) [F]. The entry
//! takes (D, param). b499's family on one pool: 200 tasks of 0x40 bytes
//! at D+0x100 (alive +0x36, step +0x24 as a record address, stepped inline
//! or by direct `jal`s), spawned by 0x1E012B0 (b499's task200 on this
//! record's offsets: colour D+0x3454, point D+0x80, words D+0xA0.. at
//! +0x10, D+0x34A0 at +0x3A, D+0x34A8 at +0x30), the sines D+0x34E0. The
//! attacker's nodes 0x26 / 0x25 keep 10-point histories (D+0x3300 /
//! D+0x33A0, a72bfb6bb70a). The emitter 0x1E00080: the facing D+0x349C =
//! the unit's turn / 180 - 180, s1 = 15 x -cos(facing); on 0x2D a violet
//! trail (0x1E004E0, drawn by 0x1E013F0: b159's gouraud trail along node
//! 0x26's history with 18 quads a segment, radius 2 + 0.3 a segment); on
//! 0x2E..0x53 a red cloud (b499's 0x1E01FA0 ended on 0x54); on 0x4D the
//! history head held at D+0x3440; on 0x4E forty pale sparks (b499's
//! 0x1E01C50 fading from 0x50 by 0x20) in a light column (0x1E01A00 =
//! b499's 0x1E03F70 at D+0x3440); on 0x4F a flash (b499's 0x1E01990) and
//! sixty ray motes (b499's 0x1E01D30); 0x4F..0x51 thirty violet sway motes
//! a update (b499's 0x1E01AF0) around node 0x26's newest point. Returns 1
//! on update 0x45, 4 on 0x59, 2 on 0x4F, 8 once the 130-update timer
//! D+0x3450 has run out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::exe::{camera_vp_into, mul_mem, rot_trans_into};
use crate::blib::spawn as lspawn;
use crate::bus::Bus;
use crate::c061::rgbaq_at;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const VP: u32 = D + 0x40;
const POS: u32 = D + 0x80;
const TEX: u32 = D + 0xFC;
const POOL: u32 = D + 0x100;
const HIST_A: u32 = D + 0x3300;
const HIST_B: u32 = D + 0x33A0;
const HOLD: u32 = D + 0x3440;
const TIMER: u32 = D + 0x3450;
const COLOUR: u32 = D + 0x3454;
const OTW: u32 = D + 0x345C;
const MEAN: u32 = D + 0x3484;
const NODES: u32 = D + 0x3494;
const FACE: u32 = D + 0x349C;
const SLOT3A: u32 = D + 0x34A0;
const SLOT30: u32 = D + 0x34A8;
const COUNTER: u32 = D + 0x34AC;
const TAB: u32 = D + 0x34E0;

const FLASH: u32 = 0x01E0_0460;
const TRAIL: u32 = 0x01E0_04E0;
const SWAY: u32 = 0x01E0_0560;
const SPARK: u32 = 0x01E0_06B0;
const RAY: u32 = 0x01E0_0790;
const CLOUD: u32 = 0x01E0_09F0;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x100);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x100);
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
    fx.bzero(D, 0x3650);
    l2::sine_table(fx, TAB);
    fx.mem.set16(D + 0x34AA, param);
    fx.mem.set32(TIMER, 0x82);
    for (k, node) in [0x26, 0x25, 0x45, 0x4C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(TEX, t);
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, or the second history's end after
    // the fills.
    let mut s1 = param as i64 as u64;
    for (k, hist, at, ra) in [(0u32, HIST_A, 0x50u32, 0x01E0_0D28u32), (1, HIST_B, 0x60, 0x01E0_0D7C)] {
        if fx.mem.i16(COUNTER) == 0 {
            let h = fx.self_handle();
            let n = fx.mem.i16(NODES + 2 * k);
            node_position_saving(fx, h, n, sp + at, u64::from(D), Some(u64::from(hist)), ra);
            for j in 0..10u32 {
                copy3(fx, hist + 0x10 * j, sp + at);
            }
            s1 = u64::from(hist + 0xA0);
        }
    }
    let n = fx.mem.i16(NODES);
    lspawn::node_history(fx, HIST_A + 0x90, 0xA, n);
    let n = fx.mem.i16(NODES + 2);
    lspawn::node_history(fx, HIST_B + 0x90, 0xA, n);
    fx.saved(sp - 0x70, &[(0x50, 0x01E0_0DC8), (0x40, 0), (0x30, 0), (0x20, 0), (0x10, s1), (0, u64::from(D))]);
    emit(fx, s1);
    for k in 0..200u32 {
        let slot = D + 0x40 * k;
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let st = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        if st(fx) == FLASH {
            flash(fx, t);
        }
        if st(fx) == TRAIL {
            trail_step(fx, t);
        }
        if st(fx) == SWAY {
            sway(fx, t);
        }
        if st(fx) == SPARK {
            let s = fx.enter(0x20);
            fx.saved(s, &[(0x10, 0x01E0_1040), (0, u64::from(D))]);
            spark(fx, t);
            fx.leave(0x20);
        }
        if st(fx) == RAY {
            let s = fx.enter(0x40);
            fx.saved(s, &[(0x30, 0x01E0_1060), (0x20, u64::from(slot)), (0x10, u64::from(k)), (0, u64::from(D))]);
            ray(fx, t);
            fx.leave(0x40);
        }
        if st(fx) == CLOUD {
            let s = fx.enter(0x30);
            fx.saved(s, &[(0x20, 0x01E0_1080), (0x10, u64::from(k)), (0, u64::from(D))]);
            cloud(fx, t);
            fx.leave(0x30);
        }
    }
    // The entry's s3: the runner's (0) [H] until a flash is drawn, then
    // the OT.
    let mut s3 = 0u32;
    let d = sp + 0x70;
    for k in 0..200u32 {
        let slot = D + 0x40 * k;
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let st = |fx: &mut Fx| fx.mem.read_u32(t + 0x24);
        if st(fx) == FLASH {
            s3 = fx.mem.read_u32(OTW);
            l2::colour_strip4(fx, d, t + 0x28, 0);
            l2::desc_gouraud4(fx, d, 0);
            l2::fullscreen4(fx, d);
            fx.submit(d, s3, 0);
        }
        let regs = [D, slot, k, s3];
        if st(fx) == TRAIL {
            frame(fx, TRAIL_F, 0x01E0_1118, regs);
            trail(fx, t);
        }
        if st(fx) == SWAY {
            pair(fx, sprite_9b, SPRITE_F, t, d, 0x01E0_1140, regs);
        }
        if st(fx) == SPARK {
            pair(fx, column, COLUMN_F, t, d, 0x01E0_1180, regs);
        }
        if st(fx) == RAY {
            pair(fx, sprite_9b, SPRITE_F, t, d, 0x01E0_11C0, regs);
        }
        if st(fx) == CLOUD {
            pair(fx, sprite_99, SPRITE99_F, t, d, 0x01E0_1200, regs);
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x45 => 1,
        0x59 => 4,
        0x4F => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

type Draw = fn(&mut Fx, u32, u32, i32);

/// A draw's frame: (size, the s0 slot, the ra slot, the saved registers
/// past s3 (the runner's 0) [H]).
type Lay = (u32, u32, u32, u32);
const SPRITE_F: Lay = (0x110, 0, 0x50, 1);
const SPRITE99_F: Lay = (0x100, 0, 0x40, 0);
const COLUMN_F: Lay = (0x100, 0, 0x30, 0);
const TRAIL_F: Lay = (0x1F0, 0x10, 0xA0, 5);

/// A callee frame below the current sp: the entry's s0..s3 from the s0
/// slot up, then the ra.
fn frame(fx: &mut Fx, lay: Lay, ra: u32, regs: [u32; 4]) {
    let (size, base, r, more) = lay;
    let f = fx.sp - size;
    for (j, v) in regs.iter().enumerate() {
        fx.saved(f + base + 0x10 * j as u32, &[(0, u64::from(*v))]);
    }
    for j in 0..more {
        fx.saved(f + base + 0x40 + 0x10 * j, &[(0, 0)]);
    }
    fx.saved(f, &[(r, u64::from(ra))]);
}

/// A draw called with mode 0 then 1 (the entry's ra after each `jal`:
/// `ra`, `ra` + 0x14).
fn pair(fx: &mut Fx, g: Draw, lay: Lay, t: u32, d: u32, ra: u32, regs: [u32; 4]) {
    for (mode, r) in [(0, ra), (1, ra + 0x14)] {
        frame(fx, lay, r, regs);
        g(fx, t, d, mode);
    }
}

/// 0x1E00C10(h) (frame 0x20, ra +0): the unit's turn (0x1F7370 into its
/// sp + 0x10) +4 as a float.
fn turn_of(fx: &mut Fx, h: i32, ra: u32) -> f32 {
    let f = fx.enter(0x20);
    fx.saved(f, &[(0, u64::from(ra))]);
    fx.unit_rotation(h, f + 0x10);
    let v = ee::from_i32(fx.mem.i32(f + 0x14));
    fx.leave(0x20);
    v
}

/// 0x1E00080(D, 0) (frame 0x40: ra, the entry's s2 = 0, s1, s0 = D): the
/// spawn colour (0x80, 0, 0, 0), the update counter, the facing and the
/// spawns (its s0 the loop counters, s1 = 15 x -cos(facing), s2 = D).
fn emit(fx: &mut Fx, s1_in: u64) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_0DD4), (0x20, 0), (0x10, s1_in), (0, u64::from(D))]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    let r = turn_of(fx, h, 0x01E0_00C4);
    let v = fix(fx, ee::div(r, 180.0), 0x01E0_00E4);
    fx.mem.set16(FACE, v);
    let v = fx.mem.i16(FACE) - 0xB4;
    fx.mem.set16(FACE, v);
    let a = fx.mem.i16(FACE);
    let c = l2::sin_deg(fx, TAB, a, 1);
    let s1 = fix(fx, ee::mul(15.0, ee::neg(c)), 0x01E0_0118) as u32;
    fx.mem.set16(SLOT30, 0);
    fx.mem.write_u32(D + 0x8C, 0);
    let mut s0 = D;
    let go = |fx: &mut Fx, f: u32, s0: u32, ra: u32| {
        let saves = [(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, u64::from(s1)), (0, u64::from(s0))];
        spawn(fx, f, &saves);
    };
    if fx.mem.i16(COUNTER) == 0x4F {
        go(fx, FLASH, s0, 0x01E0_0144);
    }
    if fx.mem.i16(COUNTER) == 0x2D {
        go(fx, TRAIL, s0, 0x01E0_0168);
    }
    fx.mem.write_u8(COLOUR + 3, 0);
    if fx.mem.i16(COUNTER) == 0x4D {
        copy3(fx, HOLD, HIST_A);
    }
    if fx.mem.i16(COUNTER) == 0x4E {
        for k in 0..0x28u32 {
            s0 = k;
            let left = -fx.mem.i16(FACE) < 0x5A;
            let r = fx.rand_from(s0, if left { 0x01E0_01C4 } else { 0x01E0_01EC });
            let x = if left { r % 0x8000 - 0x8000 } else { r % 0x8000 };
            fx.mem.set32(D + 0xA0, x);
            let r = fx.rand_from(s0, 0x01E0_0210);
            fx.mem.set32(D + 0xA8, r % 0x8000 + 0x4000);
            for o in [0x80u32, 0x84, 0x88] {
                fx.mem.write_u32(D + o, 0);
            }
            go(fx, SPARK, s0, 0x01E0_024C);
        }
    }
    s0 = 0;
    let n = fx.mem.i16(COUNTER);
    if (0x4F..=0x51).contains(&n) {
        for k in 0..0x1Eu32 {
            s0 = k;
            for (o, h, ra) in [(0x80u32, 0x3300u32, 0x01E0_0294u32), (0x84, 0x3304, 0x01E0_02C8)] {
                let r = rmod(fx, s0, ra, 0x14);
                let v = ee::sub(ee::add(fx.mem.f(D + h), ee::from_i32(r)), 10.0);
                fx.mem.setf(D + o, v);
            }
            let r = rmod(fx, s0, 0x01E0_02FC, 0x14);
            let z = ee::sub(ee::add(fx.mem.f(D + 0x3308), ee::from_i32(r)), 10.0);
            fx.mem.setf(D + 0x88, ee::add(ee::from_i32(s1 as i32), z));
            let r = rmod(fx, s0, 0x01E0_033C, 0x50);
            fx.mem.set16(SLOT30, r);
            let r = rmod(fx, s0, 0x01E0_0358, 0x168);
            fx.mem.set16(SLOT3A, r);
            go(fx, SWAY, s0, 0x01E0_037C);
        }
        s0 = 0x1E;
    }
    if fx.mem.i16(COUNTER) == 0x4F {
        for k in 0..0x3Cu32 {
            s0 = k;
            copy3(fx, POS, HIST_A);
            let z = ee::add(fx.mem.f(HIST_A + 8), ee::from_i32(s1 as i32));
            fx.mem.setf(D + 0x88, z);
            go(fx, RAY, s0, 0x01E0_03DC);
        }
        s0 = 0x3C;
    }
    let n = fx.mem.i16(COUNTER);
    if (0x2E..0x54).contains(&n) {
        copy3(fx, POS, HIST_A);
        fx.mem.set16(SLOT30, 0);
        fx.mem.set16(SLOT3A, 0);
        go(fx, CLOUD, s0, 0x01E0_0440);
    }
    fx.leave(0x40);
}

/// 0x1E012B0(D, step, 0): b499's task200 on this record's layout: the
/// first free of 200 tasks (0x40 bytes at D+0x100, alive +0x36) cleared
/// and set off.
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
    let ret = match (0..200u32).find(|&k| fx.mem.i16(POOL + 0x40 * k + 0x36) == 0) {
        Some(k) => {
            let t = POOL + 0x40 * k;
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            for o in [0x28u32, 0x2C] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            copy3(fx, t + 0x10, D + 0xA0);
            let a = fx.mem.i16(SLOT3A);
            fx.mem.set16(t + 0x3A, a);
            let a = fx.mem.i16(SLOT30);
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

fn age(fx: &mut Fx, t: u32, at: u32) {
    let n = fx.mem.i16(t + at) + 1;
    fx.mem.set16(t + at, n);
}

/// The entry's case for step 0x1E00460: a flash's alpha to 0xF0 at once,
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

/// The entry's case for step 0x1E004E0: the trail's colour from the record
/// (0x1E01F80: 0x5F, 0, 0, 0x60), ended on update 0x50; its segment count
/// +0x32 8 until update 0x46, then down by 1 to 0.
fn trail_step(fx: &mut Fx, t: u32) {
    for k in 0..4 {
        let b = fx.mem.read_u8(0x01E0_1F80 + k);
        fx.mem.write_u8(t + 0x28 + k, b);
    }
    if fx.mem.i16(COUNTER) == 0x50 {
        fx.mem.set16(t + 0x36, 0);
    }
    if fx.mem.i16(COUNTER) < 0x46 {
        fx.mem.set16(t + 0x32, 8);
    } else {
        let v = fx.mem.i16(t + 0x32);
        if v != 0 {
            fx.mem.set16(t + 0x32, v - 1);
        }
    }
}

/// i16 `at` += 20 x (sine or cosine of `deg`) through 0x1A2B80 from `ra`.
fn wobble(fx: &mut Fx, t: u32, at: u32, deg: i32, cos: i32, ra: u32) {
    let s = l2::sin_deg(fx, TAB, deg, cos);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(20.0, s));
    let r = fix(fx, f, ra);
    fx.mem.set16(t + at, r);
}

/// The entry's case for step 0x1E00560: size 9, violet, alpha up by 9
/// (under 0x60) for 10 updates then down by 10; swaying by 20.
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
    wobble(fx, t, 8, g, 0, 0x01E0_0FA8);
    let g = fx.mem.i16(t + 0x30);
    wobble(fx, t, 0xA, g, 1, 0x01E0_0FE0);
    let g = fx.mem.i16(t + 0x3A);
    wobble(fx, t, 0xC, g, 1, 0x01E0_1018);
}

/// 0x1E006B0 (D, task): a pale spark climbing 2 a update (height +6 up by
/// 4 to 10), alpha 0xC0, fading by 0x20 from 0x50 (both alphas); ends at
/// 181.
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
    if fx.mem.i16(t + 2) >= 0x50 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x21 {
            fx.mem.set16(t + 0x36, 0);
        } else {
            fx.mem.write_u8(t + 0x2B, a - 0x20);
        }
        let a = fx.mem.read_u8(t + 0x2F);
        if a >= 0x21 {
            fx.mem.write_u8(t + 0x2F, a - 0x20);
        }
    }
}

/// 0x1E00790 (D, task): a ray mote swaying by 20 about the facing (the
/// angle -D+0x349C + +0x3A): size 1, a random cell, yellow / red / orange.
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
        let v = fx.rand_from(s0 as u32, 0x01E0_07D0);
        let f = fx.sp - 0x10;
        fx.mem.write_u64(f, s0 as i64 as u64);
        fx.mem.set16(t + 0x34, v.wrapping_rem(4));
        let k = rmod(fx, 0x01E0_07EC, 3) as i16 as i32;
        match k {
            0 => l2::rgb(fx, t + 0x28, 0xFF, 0xF0, 0),
            1 => l2::rgb(fx, t + 0x28, 0xFF, 0, 0),
            2 => l2::rgb(fx, t + 0x28, 0xFF, 0x80, 0),
            _ => {}
        }
        let r = rmod(fx, 0x01E0_087C, 0xB4);
        fx.mem.set16(t + 0x3A, r + 0x5A);
        let r = rmod(fx, 0x01E0_089C, 0x168);
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
    wobble(fx, t, 8, g, 0, 0x01E0_094C);
    let g = fx.mem.i16(t + 0x30);
    wobble(fx, t, 0xA, g, 0, 0x01E0_0984);
    let g = (s0 + fx.mem.i16(t + 0x3A)) as i16 as i32;
    wobble(fx, t, 0xC, g, 1, 0x01E0_09C8);
}

/// 0x1E009F0 (D, task): the red cloud: size 6 (22 from update 0x4F)
/// falling by 1 every third update, cells to 7, alpha up by 0x50 for 2
/// updates then down by 10; 0x20 on update 0x53, ended on 0x54.
fn cloud(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x9F, 0, 0x3F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.set16(t + 0x34, 0);
        let s = if fx.mem.i16(COUNTER) < 0x4F { 6 } else { 0x16 };
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
    if fx.mem.i16(COUNTER) == 0x53 {
        fx.mem.write_u8(t + 0x2B, 0x20);
    }
    if fx.mem.i16(COUNTER) == 0x54 {
        fx.mem.set16(t + 0x36, 0);
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

/// 0x1E01860(D, task, desc, mode): a camera-facing quad of half size i16
/// +0x38 at the task's position (+ offsets / 10), sprite 0x9B, texture
/// D+0xFC (b499's 0x1E03DD0).
fn sprite_9b(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x110);
    if mode == 0 {
        setup(fx, t, d, sp + 0xB0, 0x9B);
    } else {
        camera_angles_saving(fx, sp + 0x100, u64::from(OT), u64::from(d), 0x01E0_18FC);
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

/// 0x1E01DA0(D, task, desc, mode): a camera-facing quad of half size i16
/// +0x38 at the task's position, sprite 0x99, texture D+0xFC, at the far
/// Z, keyed by its mean w (D+0x3484) on D+0x345C (b499's 0x1E04350).
fn sprite_99(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x100);
    if mode == 0 {
        setup(fx, t, d, sp + 0xA0, 0x99);
    } else {
        camera_angles_saving(fx, sp + 0xF0, u64::from(D), 1, 0x01E0_1E3C);
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
            let key = fixu(fx, m, 0x01E0_1ED8);
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x100);
}

/// 0x1E01A00(D, task, desc, mode): the light column (b499's 0x1E03F70:
/// sprite 0x9C cell +0x34, colours +0x28 / +0x2C, texture D+0xFC, width 1,
/// at the held point D+0x3440, angles +0x10).
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
        rot_trans_into(fx, sp + 0x80, t + 0x10, HOLD);
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
            let key = fixu(fx, m, 0x01E0_1D6C);
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x100);
}

/// 0x1E013F0(D, task, desc, 0) (frame 0x1F0): b159's gouraud trail along
/// node 0x26's history D+0x3300..: for each of the +0x32 - 1 segments a
/// ring of eighteen quads (20 degrees each) between the segment's point at
/// radius r (2 for the first, + 0.3 a segment) and the next point at r +
/// 0.3, the task's colour with alpha falling by alpha / +0x32 a segment, at
/// the far Z (key 0).
fn trail(fx: &mut Fx, t: u32) {
    use ee::{add, mul};
    let sp = fx.enter(0x1F0);
    fx.mem.write_u32(sp + 0xFC, t);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 0xC8, OT);
    let s1 = sp + 0x158;
    let n = fx.mem.i16(t + 0x32);
    l2::desc_gouraud4(fx, s1, 0);
    l2::fullscreen4(fx, s1);
    fx.mem.set32(sp + 0xE0, n);
    fx.mem.set32(sp + 0xB0, 0);
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(sp + 0x1EC + k, b);
    }
    fx.mem.set32(sp + 0xD0, n - 1);
    let step = f32::from_bits(0x3E99_999A);
    let mut f20 = 2.0f32;
    let mut a = 0i32;
    let bump = |fx: &mut Fx, at: u32, r: f32, deg: i32, cos: i32| {
        let c = l2::sin_deg(fx, TAB, deg, cos);
        let v = add(fx.mem.f(at), mul(r, c));
        fx.mem.setf(at, v);
    };
    while a < n - 1 {
        let s3 = HIST_A + 0x10 * a as u32;
        l2::colour_strip4(fx, s1, sp + 0x1EC, 1);
        let al = i32::from(fx.mem.read_u8(t + 0x2B));
        let q = fx.div(al, n).0;
        let b = fx.mem.read_u8(sp + 0x1EF).wrapping_sub(q as u8);
        fx.mem.write_u8(sp + 0x1EF, b);
        l2::colour_strip4(fx, s1, sp + 0x1EC, 2);
        let mut s2 = 0i32;
        for _ in 0..0x12 {
            for o in [0x110u32, 0x120] {
                copy3(fx, sp + o, s3);
            }
            let s4 = (s2 + 0x14) as i16 as i32;
            let r = f20;
            bump(fx, sp + 0x114, r, s2, 1);
            bump(fx, sp + 0x110, r, s2, 0);
            bump(fx, sp + 0x118, r, s2, 1);
            bump(fx, sp + 0x124, r, s4, 1);
            bump(fx, sp + 0x120, r, s4, 0);
            bump(fx, sp + 0x128, r, s4, 1);
            for o in [0x130u32, 0x140] {
                copy3(fx, sp + o, s3 + 0x10);
            }
            let r2 = || add(f20, step);
            bump(fx, sp + 0x134, r2(), s2, 1);
            bump(fx, sp + 0x130, r2(), s2, 0);
            bump(fx, sp + 0x138, r2(), s2, 1);
            bump(fx, sp + 0x144, r2(), s4, 1);
            bump(fx, sp + 0x140, r2(), s4, 0);
            bump(fx, sp + 0x148, r2(), s4, 1);
            if l2::project(fx, sp + 0x110, SPR, 0, s1, sp + 0x100) {
                l2::far_z_desc(fx, s1, 1);
                let ot = fx.mem.read_u32(sp + 0xC8);
                fx.submit(s1, ot, 0);
            }
            s2 = s4;
        }
        f20 = add(f20, step);
        a += 1;
        fx.mem.set32(sp + 0xB0, a);
    }
    fx.leave(0x1F0);
}
