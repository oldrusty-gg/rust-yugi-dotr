//! BMODULE b774 (card 87), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E025F0, data area 0x3890 bytes) [F] (helper E16,
//! 4 Nov 2026) from E15's map: a pool of 170 tasks of 0x40 bytes at D+0x660
//! (alive +0x36, age +0x32, step +0x24 compared by address, b267's shape)
//! spawned by 0x1E02D30 (the first free task cleared, colours D+0x36D4 at
//! +0x28 / +0x2C, the point D+0x40 as i16, D+0x3716 at +0x30), and 30
//! records of 0x30 at D+0x30E0 (alive +0x26) spawned by 0x1E02E50. The
//! emitter 0x1E00080 moves the point D+0x610 out along the attacker's
//! nodes 0x49 -> 0x4A, then (0xB5..0xFE) toward the target's node 1 and
//! (from 0xFF) circles D+0x620 by 2; its steps: the stage light 0x1E02080
//! (update 1), on 0x27 the object op 0x2160 (0x1F8380(3, 0, ..), inline in
//! the entry, ended on 0x3A) and the window tint 0x1E021E0; on 0x50 a
//! sphere 0x1E015D0, a blue sprite 0x1E00E50, the camera light 0x1E01E30
//! and ten trails 0x1E009D0 (+0x30 = 0..9, eight jittered points each at
//! D+0x110 + 0x80 k); on odd updates from 0x51 domes 0x1E01760, from 0x51
//! glows 0x1E011C0 and sparks 0x1E018C0, from 0xFB rings 0x1E014A0, from
//! 0x100 green bursts 0x1E01B70 (the last four while the timer is >= 0x1F);
//! records 0x1E01CD0 on 0xBA..0xF6. Draws: 0x1E03350 (trails), 0x1E036A0
//! (sprite), 0x1E02F40 (a 12-piece ring for 0x0F90 / 0x11C0 tasks),
//! 0x1E03B90 (an 18-piece ring), 0x1E03840 (the sphere, 18 x 12),
//! 0x1E03E70 / 0x1E041B0 (b148's dome band, y 90 sin), 0x1E02410 (a
//! billboard), 0x1E04510 (a gouraud quad drawn twice). Returns 1 / 4 / 2 /
//! 6 on 0xF5 / 0x163 / 0x159 / 0x100..0x153, 8 once the timer D+0x36D0
//! (0x176) is out. The step 0x1E00F90 is tested but never spawned.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0xAA;
const POOL: u32 = D + 0x660;
const RECS: u32 = 0x1E;
const REC_POOL: u32 = D + 0x30E0;
const SPAWN_AT: u32 = D + 0x40;
const TEX: u32 = D + 0x10C;
const POINT: u32 = D + 0x610;
const SAVED: u32 = D + 0x620;
const START: u32 = D + 0x630;
const LAMP: u32 = D + 0x3680;
const NODES: u32 = D + 0x36B0;
const TNODE: u32 = D + 0x36C0;
const TIMER: u32 = D + 0x36D0;
const COLOUR: u32 = D + 0x36D4;
const OT_AT: u32 = D + 0x36DC;
const GROW: u32 = D + 0x36F4;
const SIZE: u32 = D + 0x36F8;
const MEAN: u32 = D + 0x36FC;
const ANGLE: u32 = D + 0x370E;
const YSTEP: u32 = D + 0x3710;
const KIND: u32 = D + 0x3716;
const ORBIT: u32 = D + 0x3718;
const COUNTER: u32 = D + 0x371E;
const SINES: u32 = D + 0x3724;
/// 0.5 as a double.
const D05: u64 = 0x3FE0_0000_0000_0000;

const TRAIL: u32 = 0x01E0_09D0;
const SPRITE: u32 = 0x01E0_0E50;
const RING_A: u32 = 0x01E0_0F90;
const GLOW: u32 = 0x01E0_11C0;
const RING: u32 = 0x01E0_14A0;
const SPHERE: u32 = 0x01E0_15D0;
const DOME: u32 = 0x01E0_1760;
const SPARK: u32 = 0x01E0_18C0;
const BURST: u32 = 0x01E0_1B70;
const LIGHT: u32 = 0x01E0_1E30;
const STAGE: u32 = 0x01E0_2080;
const OBJECT: u32 = 0x01E0_2160;
const WINDOW: u32 = 0x01E0_21E0;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn timer(fx: &mut Fx) -> i32 {
    fx.mem.i32(TIMER)
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// 0x1A2B80 on `v` from `ra`: its frame and the integer.
fn fixi(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

fn enter_saved(fx: &mut Fx, size: u32, slots: &[(u32, u64)]) -> u32 {
    let sp = fx.enter(size);
    fx.saved(sp, slots);
    sp
}

fn copy_words(fx: &mut Fx, to: u32, from: u32, n: u32) {
    for k in 0..n {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn add16(fx: &mut Fx, a: u32, d: i32) {
    let v = fx.mem.i16(a).wrapping_add(d);
    fx.mem.set16(a, v);
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x36) == 1
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x32)
}

fn grow_age(fx: &mut Fx, t: u32) {
    add16(fx, t + 0x32, 1);
}

/// The alpha +0x2B down by `down` while at least `floor`, else the task
/// ends.
fn fade_out(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let v = fx.mem.read_u8(t + 0x2B);
    if v >= floor {
        fx.mem.write_u8(t + 0x2B, v.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

fn add_alpha(fx: &mut Fx, t: u32, up: u8) {
    let v = fx.mem.read_u8(t + 0x2B);
    fx.mem.write_u8(t + 0x2B, v.wrapping_add(up));
}

/// Three rand % 0xFFFF angles into +0x10 / +0x14 / +0x18 (rand from the
/// return addresses `ras`, the step's s0 = `s0`).
fn spin3(fx: &mut Fx, t: u32, s0: u32, ras: [u32; 3]) {
    for (o, ra) in [0x10u32, 0x14, 0x18].into_iter().zip(ras) {
        let r = rmod(fx, s0, ra, 0xFFFF);
        fx.mem.set32(t + o, r);
    }
}

/// The task's point = the emitter's D+0x610 through 0x1A2B80 (from `ras`).
fn at_point(fx: &mut Fx, t: u32, ras: [u32; 3]) {
    for (k, ra) in ras.into_iter().enumerate() {
        let v = fx.mem.f(POINT + 4 * k as u32);
        let i = fixi(fx, v, ra);
        fx.mem.set16(t + 2 * k as u32, i);
    }
}

/// The entry 0x1E025F0(D, param) (frame 0x120: s0 = the param / the step
/// loop's index, s1 = 0 / the drawn count, s2 = D, s3 the slot / index).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x120);
    fx.mem.write_u32(OT_AT, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(0x120);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3890);
    let p = param as i64 as u64;
    l2::sine_table_saving(fx, SINES, p, 0, 0x01E0_2634);
    fx.mem.set16(D + 0x371C, param);
    fx.mem.set32(TIMER, 0x176);
    for o in [0xC0u32, 0xC4, 0xC8] {
        fx.mem.write_u32(D + o, 0);
    }
    let s = [Some(p), Some(0), Some(u64::from(D))];
    for (k, (n, clear, ra)) in [(0x49, 1, 0x01E0_2660u32), (0x4A, 0, 0x01E0_2678), (1, 0, 0x01E0_2690)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = crate::b076::request_node_saving(fx, h, n, clear, s, ra);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = crate::b076::request_node_saving(fx, h, 1, 1, s, 0x01E0_26A8);
    fx.mem.set16(TNODE, r);
    for k in 0..0x10u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0xD0 + 4 * k, t);
    }
    0
}

/// (double) `v` < `b` through 0x1A2C30 (from `ra1`) and 0x17BCA0 (from
/// `ra2`) with the frames they leave (b267's).
fn dlt(fx: &mut Fx, v: f32, b: u64, s0: u32, ra1: u32, ra2: u32) -> bool {
    let sp = fx.sp;
    fx.fix_residue(v, ra1);
    let f = sp - 0x30;
    let (class, sign, exp, mant) = (fx.mem.read_u32(f), fx.mem.read_u32(f + 4), fx.mem.read_u32(f + 8), fx.mem.read_u32(f + 0xC));
    let g = sp - 0x60;
    fx.mem.write_u32(g, class);
    fx.mem.write_u32(g + 4, sign);
    fx.mem.write_u32(g + 8, exp);
    fx.mem.write_u64(g + 0x10, u64::from(mant) << 30);
    fx.mem.write_u64(g + 0x20, 0x001A_2C60);
    let bits = v.to_bits();
    let a = if (bits >> 23) & 0xFF == 0 { u64::from(bits >> 31) << 63 } else { f64::from(v).to_bits() };
    fx.saved(sp - 0x10, &[(0, u64::from(ra2))]);
    let h = sp - 0x70;
    fx.mem.write_u64(h + 0x40, a);
    fx.mem.write_u64(h + 0x48, b);
    fx.mem.write_u64(h + 0x50, u64::from(s0));
    fx.mem.write_u64(h + 0x58, 0x0017_BCB0);
    unpack_d(fx, a, h);
    unpack_d(fx, b, h + 0x20);
    f64::from_bits(a) < f64::from_bits(b)
}

/// 0x1A2E08's unpack of the double `x` at `p` (b117's).
fn unpack_d(fx: &mut Fx, x: u64, p: u32) {
    fx.mem.write_u32(p + 4, (x >> 63) as u32);
    let e = ((x >> 52) & 0x7FF) as u32;
    let m = x & 0xF_FFFF_FFFF_FFFF;
    if e == 0 {
        fx.mem.write_u32(p, 2);
    } else if e == 0x7FF {
        if m == 0 {
            fx.mem.write_u32(p, 4);
        } else {
            fx.mem.write_u32(p, u32::from(m & (1 << 51) != 0));
            fx.mem.write_u64(p + 0x10, m);
        }
    } else {
        fx.mem.write_u64(p + 0x10, (m << 8) | (1 << 60));
        fx.mem.write_u32(p + 8, e.wrapping_sub(0x3FF));
        fx.mem.write_u32(p, 3);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x24);
        match f {
            TRAIL => trail(fx, t, k),
            SPRITE => sprite(fx, t, k),
            RING_A => panic!("b774: step {f:#x} is never spawned"),
            GLOW => glow(fx, t, k),
            RING => ring(fx, t, k),
            SPHERE => sphere(fx, t, k),
            DOME => dome(fx, t, k),
            SPARK => spark(fx, t, k),
            BURST => burst(fx, t, k),
            LIGHT => light(fx, t, k),
            STAGE => stage(fx, t, k),
            OBJECT => {
                for (to, from) in [(0x11Cu32, 0x01E0_4800u32), (0x118, 0x01E0_4808)] {
                    let w = fx.mem.read_u32(from);
                    fx.mem.write_u32(sp + to, w);
                }
                fx.boundary(0x001F_8380, "object op 0x1F8380", [3, 0, sp + 0x11C, sp + 0x118]);
                if counter(fx) == 0x3A {
                    fx.mem.set16(t + 0x36, 0);
                }
            }
            WINDOW => window(fx, t, k),
            _ => {}
        }
    }
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x50, u64::from(TASKS), 0x01E0_28FC);
    let z = ee::sub(fx.mem.f(sp + 0x58), 25.0);
    fx.mem.setf(sp + 0x58, z);
    let d = sp + 0x88;
    let mut count = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        let r = Regs { s: [D + 0x40 * k, count, D, k] };
        let f = fx.mem.read_u32(t + 0x24);
        match f {
            TRAIL => {
                trail_draw(fx, t, d, 0, &r, 0x01E0_294C);
                trail_draw(fx, t, d, 1, &r, 0x01E0_2960);
            }
            SPRITE => {
                sprite_draw(fx, t, d, 0, &r, 0x01E0_2988);
                sprite_draw(fx, t, d, 1, &r, 0x01E0_299C);
            }
            RING_A | GLOW => {
                let ra = if f == GLOW { 0x01E0_2A08 } else { 0x01E0_29C8 };
                ring12(fx, t, d, 0, &r, ra);
                ring12(fx, t, d, 1, &r, ra + 0x14);
            }
            RING => {
                ring18(fx, t, d, 0, &r, 0x01E0_2A48);
                ring18(fx, t, d, 1, &r, 0x01E0_2A5C);
            }
            SPHERE => sphere_draw(fx, t, d, &r, 0x01E0_2A88),
            DOME => {
                dome_draw(fx, t, d, 0, &r, 0x01E0_2AB0, &DOME_TASK);
                dome_draw(fx, t, d, 1, &r, 0x01E0_2AC4, &DOME_TASK);
            }
            SPARK => {
                let tex = fx.mem.read_u32(TEX);
                fx.mem.write_u32(sp + 0x60, tex);
                fx.mem.write_u32(sp + 0x68, 0);
                fx.mem.write_u32(sp + 0x64, 0x9B);
                fx.mem.write_u32(sp + 0x6C, 0);
                let ot = fx.mem.read_u32(OT_AT);
                board(fx, ot, t, sp + 0x60, &r, 0x01E0_2B04);
            }
            BURST => {
                quad_draw(fx, t, d, 0, &r, 0x01E0_2B30);
                quad_draw(fx, t, d, 1, &r, 0x01E0_2B44);
            }
            _ => {}
        }
    }
    for j in 0..RECS {
        let p = REC_POOL + 0x30 * j;
        if fx.mem.i16(p + 0x26) != 0 {
            record(fx, p, j, D + 0x30 * j);
        }
    }
    for j in 0..RECS {
        let p = REC_POOL + 0x30 * j;
        if fx.mem.i16(p + 0x26) != 0 {
            let r = Regs { s: [D + 0x30 * j, j, D, TASKS] };
            dome_draw(fx, p, d, 0, &r, 0x01E0_2BB0, &DOME_REC);
            dome_draw(fx, p, d, 1, &r, 0x01E0_2BC4, &DOME_REC);
        }
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x70);
    if counter(fx) >= 0x15 {
        let s0 = D + 0x30 * RECS;
        let v = fx.mem.f(D + 0xC0);
        if dlt(fx, v, D05, s0, 0x01E0_2C04, 0x01E0_2C14) {
            let v = ee::add(fx.mem.f(D + 0xC0), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC0, v);
        }
        fx.mem.write_u32(D + 0xC4, 0x3DCC_CCCD);
        let v = fx.mem.f(D + 0xC8);
        if dlt(fx, v, D05, s0, 0x01E0_2C4C, 0x01E0_2C5C) {
            let v = ee::add(fx.mem.f(D + 0xC8), f32::from_bits(0x3C23_D70A));
            fx.mem.setf(D + 0xC8, v);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0xF5 => 1,
        0x163 => 4,
        0x159 => 2,
        0x100..=0x153 => 6,
        _ if timer(fx) > 0 => 0,
        _ => 8,
    }
}

// ------------------------------------------------------------ emitter

/// 0x1E02D30(D, step, 0) (frame 0x60: ra +0x50, the runner's s4 / s3 (0
/// [H]), the entry's s2 = D, the emitter's s1 / s0 = D).
fn spawn(fx: &mut Fx, f: u32, s1: u64, ra: u32) {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, s1), (0, u64::from(D))]);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x40 * k + 0x36) == 0) {
        let t = POOL + 0x40 * k;
        fx.bzero(t, 0x40);
        fx.mem.set16(t + 0x36, 1);
        for o in [0x28u32, 0x2C] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + o + 3, a);
        }
        l2::to_i16x4(fx, t, SPAWN_AT, 0);
        let v = fx.mem.i16(KIND);
        fx.mem.set16(t + 0x30, v);
        fx.mem.write_u32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, f);
    }
    fx.leave(0x60);
}

/// 0x1E02E50(D) (frame 0x40: ra +0x30, the entry's s2 = D, the emitter's
/// s1 / s0 = D): a record of 0x30 (colour +0x2C, the point D+0x40 as i16,
/// D+0x3716 at +0x20).
fn spawn_rec(fx: &mut Fx, s1: u64, ra: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, u64::from(ra)), (0x20, u64::from(D)), (0x10, s1), (0, u64::from(D))]);
    if let Some(j) = (0..RECS).find(|&j| fx.mem.i16(REC_POOL + 0x30 * j + 0x26) == 0) {
        let p = REC_POOL + 0x30 * j;
        fx.bzero(p, 0x30);
        fx.mem.set16(p + 0x26, 1);
        let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
        l2::rgb(fx, p + 0x2C, i32::from(r), i32::from(g), i32::from(b));
        let a = fx.mem.read_u8(COLOUR + 3);
        fx.mem.write_u8(p + 0x2F, a);
        l2::to_i16x4(fx, p, SPAWN_AT, 0);
        let v = fx.mem.i16(KIND);
        fx.mem.set16(p + 0x20, v);
    }
    fx.leave(0x40);
}

/// D+0x40 = the emitter's point (xyz).
fn spawn_at_point(fx: &mut Fx) {
    copy_words(fx, SPAWN_AT, POINT, 3);
}

/// One spawn (the original's single-pass loop: s1 0 at the call, 1 after)
/// with ANGLE = rand % 360 at the point.
fn spawn_turned(fx: &mut Fx, f: u32, s1: &mut u64, ras: [u32; 2]) {
    *s1 = 0;
    let r = rmod(fx, D, ras[0], 0x168);
    fx.mem.set16(ANGLE, r);
    spawn_at_point(fx);
    spawn(fx, f, *s1, ras[1]);
    *s1 = 1;
}

/// p += q when `lt`: `a` < `cap`, by `by`.
fn grow_to(fx: &mut Fx, a: u32, cap: f32, by: f32) {
    let v = fx.mem.f(a);
    if v < cap {
        fx.mem.setf(a, ee::add(v, by));
    }
}

/// 0x1E00080(D, 0) (frame 0x50: ra +0x20, the entry's s1 = 0 +0x10, s0 =
/// the param +0).
fn emit(fx: &mut Fx, param: i32) {
    let sp = enter_saved(fx, 0x50, &[(0x20, 0x01E0_26E4), (0x10, 0), (0, param as i64 as u64)]);
    let d = u64::from(D);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    let mut s1 = 0u64;
    if counter(fx) == 1 {
        spawn(fx, STAGE, s1, 0x01E0_00D0);
    }
    if counter(fx) == 0x27 {
        spawn(fx, OBJECT, s1, 0x01E0_00F4);
    }
    if counter(fx) == 0x27 {
        spawn(fx, WINDOW, s1, 0x01E0_0118);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES);
    crate::b076::node_position_saving(fx, h, n, sp + 0x30, d, Some(s1), 0x01E0_0130);
    s1 = u64::from(sp + 0x40);
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES + 2);
    crate::b076::node_position_saving(fx, h, n, sp + 0x40, d, Some(s1), 0x01E0_014C);
    let c = counter(fx);
    if c < 0xC0 && (c == 0x50 || c >= 0xAA) {
        out_along(fx, sp, POINT);
    }
    if counter(fx) == 0xB4 {
        out_along(fx, sp, START);
    }
    let c = counter(fx);
    if c.wrapping_rem(2) == 1 && timer(fx) >= 0x24 && c >= 0x51 {
        spawn_turned(fx, DOME, &mut s1, [0x01E0_02B0, 0x01E0_02EC]);
    }
    if counter(fx) == 0x50 {
        spawn_turned(fx, SPHERE, &mut s1, [0x01E0_0314, 0x01E0_0350]);
    }
    if counter(fx) >= 0x51 && timer(fx) >= 0x1F {
        spawn_turned(fx, GLOW, &mut s1, [0x01E0_038C, 0x01E0_03C8]);
    }
    if counter(fx) == 0x50 {
        spawn(fx, SPRITE, s1, 0x01E0_03FC);
    }
    if counter(fx) == 0x50 {
        spawn(fx, LIGHT, s1, 0x01E0_0420);
    }
    if counter(fx) == 0x50 {
        for j in 0..0xAu64 {
            s1 = j;
            fx.mem.set16(KIND, j as i32);
            spawn(fx, TRAIL, s1, 0x01E0_044C);
        }
        s1 = 0xA;
    }
    let c = counter(fx);
    if (0xBA..0xF7).contains(&c) {
        spawn_at_point(fx);
        spawn_rec(fx, s1, 0x01E0_04A8);
    }
    if counter(fx) >= 0xFB && timer(fx) >= 0x1F {
        spawn_at_point(fx);
        spawn(fx, RING, s1, 0x01E0_04F4);
    }
    if counter(fx) >= 0x51 && timer(fx) >= 0x1F {
        spawn_at_point(fx);
        spawn(fx, SPARK, s1, 0x01E0_0540);
    }
    if counter(fx) >= 0x100 && timer(fx) >= 0x1F {
        spawn_at_point(fx);
        spawn(fx, BURST, s1, 0x01E0_058C);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES);
    crate::b076::node_position_saving(fx, h, n, sp + 0x30, d, Some(s1), 0x01E0_05A4);
    let h = fx.target_handle();
    let n = fx.mem.i16(TNODE);
    crate::b076::node_position_saving(fx, h, n, sp + 0x30, d, Some(s1), 0x01E0_05BC);
    let c = counter(fx);
    if (0xB5..0xFF).contains(&c) {
        for (o, by) in [(4u32, 200.0f32), (8, 100.0), (0, 100.0)] {
            let step = ee::div(ee::sub(fx.mem.f(sp + 0x30 + o), fx.mem.f(START + o)), by);
            let v = ee::add(fx.mem.f(POINT + o), step);
            fx.mem.setf(POINT + o, v);
        }
    }
    let c = counter(fx);
    if (0x50..0x8C).contains(&c) {
        for (at, v) in [(0x50, 0x3F80_0000u32), (0x55, 0x4000_0000), (0x5A, 0x4040_0000), (0x5F, 0x4080_0000), (0x82, 0x4120_0000)] {
            if counter(fx) == at {
                fx.mem.write_u32(GROW, v);
            }
        }
        grow_to(fx, SIZE, 90.0, 2.0);
    }
    for (lo, hi, gcap, scap, sby) in [(0x8C, 0xAB, 30.0f32, 180.0f32, 3.0f32), (0xB6, 0xBA, 32.0, 210.0, 5.0), (0xBE, 0xE7, 50.0, 300.0, 4.0)] {
        let c = counter(fx);
        if (lo..hi).contains(&c) {
            grow_to(fx, GROW, gcap, 1.0);
            grow_to(fx, SIZE, scap, sby);
        }
    }
    fx.mem.set16(YSTEP, 8);
    if counter(fx) == 0xFE {
        copy_words(fx, SAVED, POINT, 4);
        let r = rmod(fx, D, 0x01E0_08CC, 0x168);
        fx.mem.set16(ORBIT, r);
        let r = rmod(fx, D, 0x01E0_08E8, 0x168);
        fx.mem.set16(ORBIT + 2, r);
    }
    if counter(fx) >= 0xFF {
        copy_words(fx, POINT, SAVED, 4);
        for (o, a) in [(0u32, ORBIT), (4, ORBIT + 2)] {
            let deg = fx.mem.i16(a);
            let c = l2::sin_deg(fx, SINES, deg, 1);
            let v = ee::add(fx.mem.f(POINT + o), ee::mul(2.0, c));
            fx.mem.setf(POINT + o, v);
        }
        let r = rmod(fx, D, 0x01E0_0984, 0x168);
        fx.mem.set16(ORBIT, r);
        let r = rmod(fx, D, 0x01E0_09A0, 0x168);
        fx.mem.set16(ORBIT + 2, r);
    }
    fx.leave(0x50);
}

/// sp + 0x30 = B - A (B = sp + 0x40), then `out` = B + (sp + 0x30) (xyz).
fn out_along(fx: &mut Fx, sp: u32, out: u32) {
    for o in [0u32, 4, 8] {
        let v = ee::sub(fx.mem.f(sp + 0x40 + o), fx.mem.f(sp + 0x30 + o));
        fx.mem.setf(sp + 0x30 + o, v);
    }
    for o in [0u32, 4, 8] {
        let v = ee::add(fx.mem.f(sp + 0x40 + o), fx.mem.f(sp + 0x30 + o));
        fx.mem.setf(out + o, v);
    }
}

// ------------------------------------------------------------ steps

/// The step loop's registers: s0 = the index `k`, s1 = 0, s2 = D, s3 =
/// D + 0x40 k.
fn s3_of(k: u32) -> u64 {
    u64::from(D + 0x40 * k)
}

/// 0x1E009D0 (frame 0x60): a trail (size rand % 10 + 3, turn rand % 30 -
/// 15), magenta (0x2F, 0x10, 0x2F; bright from 0x8D), in by 0xF0 / (size
/// - 1) for `size` updates, then out by 10 below timer 30, else restarting
/// below 0x11; its eight points D+0x110 + 0x80 (+0x30) jittered by rand % 8
/// - 4, 8 k apart in y (0x11 k from 0x8C), the first at x = z = 0.
fn trail(fx: &mut Fx, t: u32, k: u32) {
    let _ = enter_saved(fx, 0x60, &[(0x50, 0x01E0_2718), (0x40, 0), (0x30, s3_of(k)), (0x20, u64::from(D)), (0x10, 0), (0, u64::from(k))]);
    if counter(fx) >= 0x8D {
        l2::rgb(fx, t + 0x28, 0xFF, 0x10, 0xFF);
    } else {
        l2::rgb(fx, t + 0x28, 0x2F, 0x10, 0x2F);
    }
    if first(fx, t) {
        let r = rmod(fx, k, 0x01E0_0A48, 0xA);
        fx.mem.set16(t + 0x38, r + 3);
        let r = rmod(fx, k, 0x01E0_0A68, 0x1E);
        fx.mem.set16(t + 0x3A, r - 0xF);
        fx.mem.set16(t + 0x34, 0);
        spin3(fx, t, k, [0x01E0_0A8C, 0x01E0_0AA8, 0x01E0_0AC4]);
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.set16(t + 0x32, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let r = fx.mem.i16(t + 0x38);
    if age(fx, t) < r {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0xF0 {
            let q = fx.div(0xF0, r - 1).0;
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(q as u8));
        }
    } else if timer(fx) < 0x1E {
        fade_out(fx, t, 0x11, 0xA);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x11 {
            fx.mem.set16(t + 0x36, 1);
        } else {
            fx.mem.write_u8(t + 0x2B, a - 0xF);
        }
    }
    grow_age(fx, t);
    let base = trail_base(fx, t);
    for j in 0..8u32 {
        let p = base + 0x10 * j;
        let v = fx.rand_from(j, 0x01E0_0C64);
        fx.mem.setf(p, ee::from_i32(v.wrapping_rem(8) - 4));
        let v = fx.rand_from(j, 0x01E0_0C90);
        fx.mem.setf(p + 8, ee::from_i32(v.wrapping_rem(8) - 4));
        let y = if counter(fx) < 0x8C {
            let s = fx.mem.i16(YSTEP);
            fx.mult(j as i32, s)
        } else {
            0x11 * j as i32
        };
        fx.mem.setf(p + 4, ee::from_i32(y));
    }
    fx.mem.write_u32(base, 0);
    fx.mem.write_u32(base + 8, 0);
    let v = fx.mem.read_u32(t + 0x18).wrapping_add((fx.mem.i16(t + 0x3A) << 6) as u32);
    fx.mem.write_u32(t + 0x18, v);
    if timer(fx) < 0x1E {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x2B, a - 0xE);
        }
    }
    fx.leave(0x60);
}

/// The trail's points by +0x30: D+0x110 + 0x80 n for 0..=8, else D+0x590.
fn trail_base(fx: &mut Fx, t: u32) -> u32 {
    match fx.mem.i16(t + 0x30) {
        n @ 0..=8 => D + 0x110 + 0x80 * n as u32,
        _ => D + 0x590,
    }
}

/// 0x1E00E50 (frame 0x30): a blue sprite at the point (size 0x1E growing
/// by 1), in by 0xC for 10 updates, then out by 9.
fn sprite(fx: &mut Fx, t: u32, k: u32) {
    let _ = enter_saved(fx, 0x30, &[(0x20, 0x01E0_2738), (0x10, 0), (0, u64::from(k))]);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x1E);
        let _ = fx.rand_from(t, 0x01E0_0E80);
        l2::rgb(fx, t + 0x28, 0, 0, 0xFF);
        spin3(fx, t, t, [0x01E0_0E9C, 0x01E0_0EB8, 0x01E0_0ED4]);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0xA {
        add_alpha(fx, t, 0xC);
    } else {
        fade_out(fx, t, 0xB, 9);
    }
    add16(fx, t + 0x38, 1);
    grow_age(fx, t);
    at_point(fx, t, [0x01E0_0F54, 0x01E0_0F60, 0x01E0_0F6C]);
    fx.leave(0x30);
}

/// The fade-in steps of 0x1E00F90 / 0x1E011C0 by the update: 1 below 0x8C,
/// 2 / 3 / 4 below 0x91 / 0x96 / 0x9B, else 5.
fn steps_in(fx: &mut Fx) -> i32 {
    match counter(fx) {
        c if c < 0x8C => 1,
        c if c < 0x91 => 2,
        c if c < 0x96 => 3,
        c if c < 0x9B => 4,
        _ => 5,
    }
}

/// 0x1E011C0 (frame 0x40): a glow ring at the point (size 5 growing by 3
/// below D+0x36F4), violet by the update, in over `steps_in` updates by
/// 0x80 / n, then out by 10.
fn glow(fx: &mut Fx, t: u32, k: u32) {
    let _ = enter_saved(fx, 0x40, &[(0x30, 0x01E0_2778), (0x20, u64::from(D)), (0x10, 0), (0, u64::from(k))]);
    let n = steps_in(fx);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 5);
        let _ = fx.rand_from(n as u32, 0x01E0_1274);
        let c = counter(fx);
        let rgb = match c {
            _ if c < 0x8C => Some((0x2F, 0x1F, 0x40)),
            _ if c < 0x101 => Some((0x4F, 0x2F, 0x80)),
            0x102 => Some((0x40, 0x1F, 0x70)),
            0x103 => Some((0x30, 0x1F, 0x60)),
            0x104 => Some((0x20, 0x1F, 0x50)),
            _ if c >= 0x105 => Some((0x10, 0x1F, 0x40)),
            _ => None,
        };
        if let Some((r, g, b)) = rgb {
            l2::rgb(fx, t + 0x28, r, g, b);
        }
        spin3(fx, t, n as u32, [0x01E0_1368, 0x01E0_1384, 0x01E0_13A0]);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < n {
        let q = fx.div(0x80, n).0;
        add_alpha(fx, t, q as u8);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    let s = fx.mem.i16(t + 0x38);
    if ee::from_i32(s) < fx.mem.f(GROW) {
        fx.mem.set16(t + 0x38, s + 3);
    }
    grow_age(fx, t);
    at_point(fx, t, [0x01E0_1460, 0x01E0_146C, 0x01E0_1478]);
    fx.leave(0x40);
}

/// 0x1E014A0 (frame 0x20): an orange ring (0x80, 0x40, 0x20), in by 0x18
/// for 4 updates, out by 9 once its radius is 0x1F; radius +5 to 0x64,
/// turning by 12.
fn ring(fx: &mut Fx, t: u32, k: u32) {
    let _ = enter_saved(fx, 0x20, &[(0x10, 0x01E0_2798), (0, u64::from(k))]);
    l2::rgb(fx, t + 0x28, 0x80, 0x40, 0x20);
    if first(fx, t) {
        spin3(fx, t, t, [0x01E0_14DC, 0x01E0_14F8, 0x01E0_1514]);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x38, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 4 {
        add_alpha(fx, t, 0x18);
    }
    if fx.mem.i16(t + 0x38) >= 0x1F {
        fade_out(fx, t, 0xB, 9);
    }
    if fx.mem.i16(t + 0x38) < 0x64 {
        add16(fx, t + 0x38, 5);
    }
    grow_age(fx, t);
    add16(fx, t + 0x3A, 0xC);
    fx.leave(0x20);
}

/// 0x1E015D0 (frame 0x30): the sphere at the point (size D+0x36F8), red
/// flickering on odd ages, in by 6 below 0x40 for 10 updates, out by 9
/// below timer 30.
fn sphere(fx: &mut Fx, t: u32, k: u32) {
    let _ = enter_saved(fx, 0x30, &[(0x20, 0x01E0_27B8), (0x10, 0), (0, u64::from(k))]);
    if first(fx, t) {
        spin3(fx, t, t, [0x01E0_15FC, 0x01E0_1618, 0x01E0_1634]);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
    }
    if age(fx, t) < 0xA && fx.mem.read_u8(t + 0x2B) < 0x40 {
        add_alpha(fx, t, 6);
    }
    if age(fx, t).wrapping_rem(2) == 1 {
        l2::rgb(fx, t + 0x28, 0xFF, 0x10, 0x20);
    } else {
        l2::rgb(fx, t + 0x28, 0x5F, 0x10, 0x20);
    }
    let v = fx.mem.f(SIZE);
    let s = fixi(fx, v, 0x01E0_16E4);
    fx.mem.set16(t + 0x38, s);
    at_point(fx, t, [0x01E0_16F0, 0x01E0_16FC, 0x01E0_1708]);
    if timer(fx) < 0x1E {
        fade_out(fx, t, 0xB, 9);
    }
    fx.leave(0x30);
}

/// 0x1E01760 (frame 0x30): a dome band (0x3F, 0x10, 0x30) of size
/// D+0x36F8 from latitude 0x5A rising by 8 to 0x10E, alpha 0x50 at once;
/// at the point for its first 4 updates.
fn dome(fx: &mut Fx, t: u32, k: u32) {
    let _ = enter_saved(fx, 0x30, &[(0x20, 0x01E0_27D8), (0x10, 0), (0, u64::from(k))]);
    l2::rgb(fx, t + 0x28, 0x3F, 0x10, 0x30);
    if first(fx, t) {
        spin3(fx, t, t, [0x01E0_17A4, 0x01E0_17C0, 0x01E0_17DC]);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x30, 0x5A);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
    }
    if age(fx, t) <= 0 && fx.mem.read_u8(t + 0x2B) < 0x50 {
        add_alpha(fx, t, 0x50);
    }
    let v = fx.mem.f(SIZE);
    let s = fixi(fx, v, 0x01E0_1838);
    fx.mem.set16(t + 0x38, s);
    if age(fx, t) < 4 {
        at_point(fx, t, [0x01E0_1854, 0x01E0_1860, 0x01E0_186C]);
    }
    grow_age(fx, t);
    if fx.mem.i16(t + 0x30) < 0x10E {
        add16(fx, t + 0x30, 8);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x30);
}

/// 0x1E018C0 (frame 0x30): a spark (size 1, cell rand % 4, yellow / red /
/// orange by rand % 3, angles rand % 360, jittered by rand % 20 - 10),
/// alpha 0x2A for 3 updates then out by 10; its drift +8 / +0xA / +0xC by
/// 14 sin / cos of the angles.
fn spark(fx: &mut Fx, t: u32, k: u32) {
    let _ = enter_saved(fx, 0x30, &[(0x20, 0x01E0_27F8), (0x10, 0), (0, u64::from(k))]);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 1);
        let v = fx.rand_from(t, 0x01E0_18EC);
        let r = fx.rand_from(t, 0x01E0_1908);
        fx.mem.set16(t + 0x34, v.wrapping_rem(4));
        let c = fx.div(r, 3).1 as i16 as i32;
        match c {
            0 => l2::rgb(fx, t + 0x28, 0xFF, 0xF0, 0),
            1 => l2::rgb(fx, t + 0x28, 0xFF, 0, 0),
            2 => l2::rgb(fx, t + 0x28, 0xFF, 0x80, 0),
            _ => {}
        }
        let r = rmod(fx, t, 0x01E0_1998, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, t, 0x01E0_19B4, 0x168);
        fx.mem.set16(t + 0x3A, r);
        for (o, ra) in [(0u32, 0x01E0_19D0u32), (2, 0x01E0_1A00), (4, 0x01E0_1A30)] {
            let r = rmod(fx, t, ra, 0x14);
            let v = fx.mem.i16(t + o) + ((r - 0xA) as i16 as i32);
            fx.mem.set16(t + o, v);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 3 {
        fx.mem.write_u8(t + 0x2B, 0x2A);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    grow_age(fx, t);
    for (o, a, cos, ra) in [(8u32, 0x3Au32, 0, 0x01E0_1AE0u32), (0xA, 0x30, 1, 0x01E0_1B18), (0xC, 0x3A, 1, 0x01E0_1B50)] {
        let deg = fx.mem.i16(t + a);
        let s = l2::sin_deg(fx, SINES, deg, cos);
        let v = ee::add(ee::from_i32(fx.mem.i16(t + o)), ee::mul(14.0, s));
        let i = fixi(fx, v, ra);
        fx.mem.set16(t + o, i);
    }
    fx.leave(0x30);
}

/// 0x1E01B70 (frame 0x20): a green burst (size 1, cell rand % 4, angles
/// rand % 360), alpha 0x18 for 10 updates, then out by 9.
fn burst(fx: &mut Fx, t: u32, k: u32) {
    let _ = enter_saved(fx, 0x20, &[(0x10, 0x01E0_2818), (0, u64::from(k))]);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 1);
        let v = fx.rand_from(t, 0x01E0_1B94);
        let _ = fx.rand_from(t, 0x01E0_1BB0);
        fx.mem.set16(t + 0x34, v.wrapping_rem(4));
        l2::rgb(fx, t + 0x28, 0, 0xFF, 0);
        l2::rgb(fx, t + 0x2C, 0, 0xFF, 0);
        let r = rmod(fx, t, 0x01E0_1BE0, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, t, 0x01E0_1BFC, 0x168);
        fx.mem.set16(t + 0x3A, r);
        spin3(fx, t, t, [0x01E0_1C18, 0x01E0_1C34, 0x01E0_1C50]);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x18);
    } else {
        fade_out(fx, t, 0xB, 9);
    }
    grow_age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01E30 (frame 0x30): the camera light D+0x3680 (camera 0's slots 4
/// / 5, 0x1F7E20) at the point's y / z, its colour rising to (128, 255,
/// 255) (from 0xFA (.., 255, 128, 128)), level up by 0.01 to 1, down by
/// 0.03 to 0.3 below timer 40; the slots cleared below timer 30; ended on
/// timer 1.
fn light(fx: &mut Fx, t: u32, k: u32) {
    let _ = enter_saved(fx, 0x30, &[(0x20, 0x01E0_2838), (0x10, 0), (0, u64::from(k))]);
    if first(fx, t) {
        for (o, v) in [(0u32, 0u32), (4, 0), (8, 0), (0xC, 0), (0x20, 0), (0x24, 0x427C_0000), (0x28, 0)] {
            fx.mem.write_u32(LAMP + o, v);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    fx.mem.write_u32(LAMP + 0x10, 0);
    copy_words(fx, LAMP + 0x14, POINT + 4, 2);
    add16(fx, t + 0x3A, 6);
    if counter(fx) == 0xFA {
        fx.mem.write_u32(LAMP + 4, 0x437F_0000);
        fx.mem.write_u32(LAMP + 8, 0x4300_0000);
        fx.mem.write_u32(LAMP + 0xC, 0);
        fx.mem.write_u32(LAMP + 0xC, 0x4300_0000);
    }
    if fx.mem.i16(t + 0x30) == 0 {
        let lv = fx.mem.f(LAMP + 0x28);
        if lv < 1.0 {
            fx.mem.setf(LAMP + 0x28, ee::add(lv, f32::from_bits(0x3C23_D70A)));
        } else {
            fx.mem.set16(t + 0x30, 1);
        }
        for (o, cap, by, top) in [(0u32, 128.0f32, 3.0f32, 0x4300_0000u32), (4, 240.0, 6.0, 0x437F_0000), (0xC, 240.0, 6.0, 0x437F_0000)] {
            let v = fx.mem.f(LAMP + o);
            if v < cap {
                fx.mem.setf(LAMP + o, ee::add(v, by));
            } else {
                fx.mem.write_u32(LAMP + o, top);
            }
        }
    }
    if timer(fx) < 0x28 {
        let lv = fx.mem.f(LAMP + 0x28);
        if !(lv <= f32::from_bits(0x3E99_999A)) {
            fx.mem.setf(LAMP + 0x28, ee::sub(lv, f32::from_bits(0x3CF5_C28F)));
        }
    }
    let p = if timer(fx) < 0x1E { 0 } else { LAMP };
    crate::blib::exe::camera_slots45(fx, p);
    if timer(fx) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
    fx.leave(0x30);
}

/// 0x1E02080 (frame 0x20): the stage light (0x1F7F10) down by 10 to 0x30
/// while the timer is at 31 or more, then back up by 10 to 0x80; 0x80 and
/// the task's end on timer 2.
fn stage(fx: &mut Fx, t: u32, k: u32) {
    let _ = enter_saved(fx, 0x20, &[(0x10, 0x01E0_2858), (0, u64::from(k))]);
    if first(fx, t) {
        fx.mem.set16(t + 0x30, 0x80);
        fx.mem.set16(t + 0x36, 2);
    }
    let l = fx.mem.i16(t + 0x30);
    if timer(fx) >= 0x1F {
        fx.mem.set16(t + 0x30, if l < 0x31 { 0x30 } else { l - 0xA });
    } else {
        fx.mem.set16(t + 0x30, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x30) >= 0x81 {
            fx.mem.set16(t + 0x30, 0x80);
        }
    }
    if timer(fx) == 2 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x36, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x30);
        fx.set_stage_light(i32::from(l));
    }
    fx.leave(0x20);
}

/// 0x1E021E0 (frame 0x90): a window sprite (0x6FF8 .. 0x8FF8) sampling the
/// frame (0x207EF0, filter 1) at ST inset by +0x38 (7) / 512, colour (0x50,
/// 0x20, 0x50, alpha +0x2B), ALPHA 0x48, key 0; in by 0xE below 0x48 while
/// the timer is at least 26, then out by 4 (b432's tint).
fn window(fx: &mut Fx, t: u32, k: u32) {
    let sp = enter_saved(fx, 0x90, &[(0x20, 0x01E0_28D8), (0x10, 0), (0, u64::from(k))]);
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 7);
        l2::rgb(fx, t + 0x28, 0x50, 0x20, 0x50);
        fx.mem.set16(t + 0x36, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(sp + 0x48 + k, c);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for o in [0x50u32, 0x54] {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + o, ee::div(s, 512.0));
    }
    for o in [0x60u32, 0x64] {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + o, ee::div(ee::sub(512.0, s), 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if timer(fx) >= 0x1A {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x48 {
            fx.mem.write_u8(t + 0x2B, a + 0xE);
        }
    } else {
        fade_out(fx, t, 6, 4);
    }
    grow_age(fx, t);
    fx.leave(0x90);
}

/// 0x1E01CD0 (frame 0x40: the record loop's s1 = `j`, s0 = the record): a
/// record dome (0x20, 0x10, 0x10), turned -0x4000 / -0xC000 about x by
/// the attacker's facing, 0x3800 about z; alpha in by 0x40 below 0x80 for
/// 2 updates, then out by 5; size D+0x36F4 + 10; latitude 0x5A up by 8 to
/// 0xA0.
fn record(fx: &mut Fx, p: u32, j: u32, s0: u32) {
    let sp = enter_saved(fx, 0x40, &[(0x20, 0x01E0_2B78), (0x10, u64::from(j)), (0, u64::from(s0))]);
    l2::rgb(fx, p + 0x2C, 0x20, 0x10, 0x10);
    if fx.mem.i16(p + 0x26) == 1 {
        let h = fx.self_handle();
        fx.unit_rotation(h, sp + 0x30);
        let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0x34)), 180.0);
        let facing = fixi(fx, v, 0x01E0_1D48);
        fx.mem.set32(p + 0x10, if facing != 0 { -0x4000 } else { -0xC000 });
        fx.mem.write_u32(p + 0x14, 0);
        fx.mem.write_u32(p + 0x18, 0x3800);
        fx.mem.set16(p + 0x24, 0);
        fx.mem.set16(p + 0x20, 0x5A);
        fx.mem.set16(p + 0x26, 2);
        fx.mem.write_u8(p + 0x2F, 0);
    }
    if fx.mem.i16(p + 0x22) < 2 {
        let a = fx.mem.read_u8(p + 0x2F);
        if a < 0x80 {
            fx.mem.write_u8(p + 0x2F, a + 0x40);
        }
    } else {
        let a = fx.mem.read_u8(p + 0x2F);
        if a >= 0x11 {
            fx.mem.write_u8(p + 0x2F, a - 5);
        } else {
            fx.mem.set16(p + 0x26, 0);
        }
    }
    let v = ee::add(10.0, fx.mem.f(GROW));
    let s = fixi(fx, v, 0x01E0_1DEC);
    fx.mem.set16(p + 0x28, s);
    add16(fx, p + 0x22, 1);
    if fx.mem.i16(p + 0x20) < 0xA0 {
        add16(fx, p + 0x20, 8);
    }
    fx.leave(0x40);
}

// ------------------------------------------------------------ draws

/// The entry's s0..s3 at a draw call.
struct Regs {
    s: [u32; 4],
}

impl Regs {
    fn r(&self, k: usize) -> u64 {
        u64::from(self.s[k])
    }
}

/// A draw's prologue: frame `size`, ra at `ra_at`, the entry's s0..s3 at
/// `s_at` + 0x10 k, the runner's s4.. (0 [H]) above them up to `top`, and
/// the first `fl` of f20 / f21 (0 [H]) at +0 / +4.
fn enter_draw(fx: &mut Fx, size: u32, ra_at: u32, s_at: u32, top: u32, fl: u32, r: &Regs, ra: u32) -> u32 {
    let sp = fx.enter(size);
    let mut at = s_at;
    let mut k = 0;
    while at < top {
        let v = if k < 4 { r.r(k) } else { 0 };
        fx.saved(sp + at, &[(0, v)]);
        at += 0x10;
        k += 1;
    }
    fx.saved(sp, &[(ra_at, u64::from(ra))]);
    for k in 0..fl {
        fx.mem.write_u32(sp + 4 * k, 0);
    }
    sp
}

/// The header half of the draws: a strip3 on the atlas cell (`sprite`,
/// `cell`), coloured `c`, texture D+0x10C.
fn header(fx: &mut Fx, d: u32, rect: u32, sprite: i32, cell: i32, c: u32) {
    l2::rect(fx, rect, sprite, cell);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, c, 0);
    l2::uv_strip3(fx, d, rect, 0);
    let tex = fx.mem.read_u32(TEX);
    fx.desc_texture(d, tex, 1);
}

/// The task's i16 point as floats at `out`.
fn point_i16(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(out + 4 * k, v);
    }
}

/// The task's point as the draws take it: i16 xyz + i16 +8 / +0xA / +0xC
/// over 10 (integer division by the magic multiply) into `out` (b267's).
fn point_int(fx: &mut Fx, t: u32, out: u32) {
    for (c, o) in [(0u32, 8u32), (2, 0xA), (4, 0xC)] {
        let v = fx.mem.i16(t + o);
        let hi = ((i64::from(v) * 0x6666_6667) >> 32) as i32;
        fx.mult(0x6666_6667, v);
        let q = (hi >> 2) + ((v as u32) >> 31) as i32;
        let p = fx.mem.i16(t + c) + q;
        fx.mem.setf(out + 2 * c, ee::from_i32(p));
    }
}

/// Project the quad at `input` with `m` into the scratchpad (mean w at
/// D+0x36FC) and submit it on D+0x36DC keyed by it (0x1A2C70 from `ra`).
fn project_mean(fx: &mut Fx, input: u32, m: u32, d: u32, ra: u32) {
    if l2::project(fx, input, SPR, m, d, MEAN) {
        let w = fx.mem.f(MEAN);
        fx.fix_residue(w, ra);
        let key = ee::fixunssfsi(w) as i32;
        let ot = fx.mem.read_u32(OT_AT);
        fx.submit(d, ot, key);
    }
}

/// 0x1E03350(D, t, d, mode) (frame 0x170): mode 0 heads `d` (sprite 0x96);
/// mode 1 draws the trail: five quads between its points (D+0x110 + 0x80
/// (+0x30)) at the emitter's point scaled (0.5, 0.5, 1), turned by +0x10,
/// half width +0x38 narrowing by a fifth a segment.
fn trail_draw(fx: &mut Fx, t: u32, d: u32, mode: i32, r: &Regs, ra: u32) {
    let sp = enter_draw(fx, 0x170, 0x80, 0x10, 0x80, 2, r, ra);
    let ot = OT;
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        header(fx, d, sp + 0xF0, 0x96, c, t + 0x28);
        fx.leave(0x170);
        return;
    }
    crate::b494::camera_angles_saving(fx, sp + 0x140, u64::from(ot), u64::from(t), 0x01E0_33FC);
    copy_words(fx, sp + 0xD0, POINT, 3);
    fx.mem.write_u32(sp + 0xE0, 0x3F00_0000);
    fx.mem.write_u32(sp + 0xE4, 0x3F00_0000);
    fx.mem.write_u32(sp + 0xE8, 0x3F80_0000);
    l2::xform_vp(fx, sp + 0x100, sp + 0xE0, t + 0x10, sp + 0xD0);
    let mut f20 = ee::from_i32(fx.mem.i16(t + 0x38));
    let f21 = f20;
    let mut a = trail_base(fx, t);
    for _ in 0..5 {
        copy_words(fx, sp + 0xD0, a, 3);
        let f0 = ee::div(f21, 5.0);
        copy_words(fx, sp + 0x150, sp + 0xD0, 4);
        copy_words(fx, sp + 0x90, sp + 0x150, 4);
        copy_words(fx, sp + 0xA0, sp + 0x150, 4);
        let v = ee::add(fx.mem.f(sp + 0x90), f20);
        fx.mem.setf(sp + 0x90, v);
        let v = ee::sub(fx.mem.f(sp + 0xA0), f20);
        f20 = ee::sub(f20, f0);
        fx.mem.setf(sp + 0xA0, v);
        copy_words(fx, sp + 0xD0, a + 0x10, 3);
        copy_words(fx, sp + 0x160, sp + 0xD0, 4);
        copy_words(fx, sp + 0xB0, sp + 0x160, 4);
        copy_words(fx, sp + 0xC0, sp + 0x160, 4);
        let v = ee::add(fx.mem.f(sp + 0xB0), f20);
        fx.mem.setf(sp + 0xB0, v);
        let v = ee::sub(fx.mem.f(sp + 0xC0), f20);
        fx.mem.setf(sp + 0xC0, v);
        l2::project_submit(fx, sp + 0x90, d, sp + 0x100, ot);
        a += 0x10;
    }
    fx.leave(0x170);
}

/// 0x1E036A0(D, t, d, mode) (frame 0x110): mode 0 heads `d` (sprite 0x9C);
/// mode 1 a camera-facing quad of half size +0x38 at the task's point.
fn sprite_draw(fx: &mut Fx, t: u32, d: u32, mode: i32, r: &Regs, ra: u32) {
    let sp = enter_draw(fx, 0x110, 0x50, 0, 0x50, 0, r, ra);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        header(fx, d, sp + 0xB0, 0x9C, c, t + 0x28);
        fx.leave(0x110);
        return;
    }
    crate::b494::camera_angles_saving(fx, sp + 0x100, u64::from(OT), u64::from(d), 0x01E0_373C);
    point_int(fx, t, sp + 0xA0);
    l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
    let s = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x60, s, s);
    l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, OT);
    fx.leave(0x110);
}

/// 0x1E02410(ot, t, params) (frame 0x1A0): a camera-facing quad of half
/// size +0x38 at the task's point on its own descriptor sp + 0x110 (atlas
/// params +4 cell +0x34, texture params +0), submitted at its mean w.
fn board(fx: &mut Fx, ot: u32, t: u32, params: u32, r: &Regs, ra: u32) {
    let sp = enter_draw(fx, 0x1A0, 0x40, 0, 0x40, 0, r, ra);
    let d = sp + 0x110;
    let id = fx.mem.i16(params + 4);
    let c = fx.mem.i16(t + 0x34);
    l2::rect(fx, sp + 0xA0, id, c);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    l2::uv_strip3(fx, d, sp + 0xA0, 0);
    let tex = fx.mem.read_u32(params);
    fx.desc_texture(d, tex, 1);
    crate::b494::camera_angles_saving(fx, sp + 0x100, u64::from(d), u64::from(params), 0x01E0_2494);
    point_int(fx, t, sp + 0x90);
    l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0x90);
    let s = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x50, s, s);
    if fx.mem.read_u32(params + 0xC) == 0 {
        l2::project_submit(fx, sp + 0x50, d, sp + 0xC0, ot);
    } else if l2::project(fx, sp + 0x50, SPR, sp + 0xC0, d, sp + 0xB0) {
        l2::far_z_desc(fx, d, 0);
        let w = fx.mem.f(sp + 0xB0);
        fx.fix_residue(w, 0x01E0_25BC);
        let key = ee::fixunssfsi(w) as i32;
        fx.submit(d, ot, key);
    }
    fx.leave(0x1A0);
}

/// 0x1E02F40(D, t, d, mode) (frame 0x160): mode 0 heads `d` (sprite 0x97);
/// mode 1 a ring of 12 quads of 30 degrees from +0x3A between radius
/// +0x38 and +0x38 - w (w 10, the radius from 0x8C, 3 under 10) at the
/// task's point scaled (0.8, 1, 0.8), turned by +0x10; while task j's
/// step (the original reads the pool's j-th task, not this one) is
/// 0x11C0 the inner corners' y jitter by rand % 20 - 10 (kept).
fn ring12(fx: &mut Fx, t: u32, d: u32, mode: i32, r: &Regs, ra: u32) {
    let sp = enter_draw(fx, 0x160, 0xA0, 0x10, 0xA0, 1, r, ra);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        header(fx, d, sp + 0x150, 0x97, c, t + 0x28);
        fx.leave(0x160);
        return;
    }
    point_i16(fx, t, sp + 0xF0);
    l2::set3(fx, sp + 0x100, f32::from_bits(0x3F4C_CCCD), 1.0, f32::from_bits(0x3F4C_CCCD));
    l2::xform_vp(fx, sp + 0x110, sp + 0x100, t + 0x10, sp + 0xF0);
    for o in [0xB4u32, 0xC4, 0xD4, 0xE4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let rad = fx.mem.i16(t + 0x38);
    let w = if rad - 0xA < 0 {
        3.0
    } else if counter(fx) < 0x8C {
        10.0
    } else {
        ee::from_i32(rad)
    };
    let mut s4 = 0i32;
    for j in 0..0xCu32 {
        for (e, outer, inner) in [(0i32, 0xD0u32, 0xB0u32), (0x1E, 0xE0, 0xC0)] {
            let deg = (fx.mem.i16(t + 0x3A) + e + s4) as i16 as i32;
            for (o, cos, less) in [(outer, 1, false), (outer + 8, 0, false), (inner, 1, true), (inner + 8, 0, true)] {
                let s = l2::sin_deg(fx, SINES, deg, cos);
                let mut rr = ee::from_i32(fx.mem.i16(t + 0x38));
                if less {
                    rr = ee::sub(rr, w);
                }
                fx.mem.setf(sp + o, ee::mul(rr, s));
            }
        }
        if fx.mem.read_u32(POOL + 0x40 * j + 0x24) == GLOW {
            for (o, ra) in [(0xD4u32, 0x01E0_3268u32), (0xE4, 0x01E0_3294)] {
                let v = rmod(fx, d, ra, 0x14);
                fx.mem.setf(sp + o, ee::from_i32(v - 0xA));
            }
        }
        project_mean(fx, sp + 0xB0, sp + 0x110, d, 0x01E0_32E8);
        s4 = (s4 + 0x1E) as i16 as i32;
    }
    fx.leave(0x160);
}

/// 0x1E03B90(D, t, d, mode) (frame 0x120): mode 0 heads `d` (sprite 0x96);
/// mode 1 a flat ring of 18 quads of 20 degrees from +0x3A between radius
/// +0x38 and max(+0x38 - 10, -10) at the task's point, turned by +0x10.
fn ring18(fx: &mut Fx, t: u32, d: u32, mode: i32, r: &Regs, ra: u32) {
    let sp = enter_draw(fx, 0x120, 0x70, 0, 0x70, 0, r, ra);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        header(fx, d, sp + 0x110, 0x96, c, t + 0x28);
        fx.leave(0x120);
        return;
    }
    point_i16(fx, t, sp + 0xC0);
    l2::xform_vp(fx, sp + 0xD0, 0, t + 0x10, sp + 0xC0);
    for o in [0x84u32, 0x94, 0xA4, 0xB4] {
        fx.mem.write_u32(sp + o, 0);
    }
    let mut inner = (fx.mem.i16(t + 0x38) - 0xA) as i16 as i32;
    if inner < -0xA {
        inner = -0xA;
    }
    let fi = ee::from_i32(inner);
    let mut s4 = 0i32;
    for _ in 0..0x12 {
        for (e, outer, inn) in [(0i32, 0xA0u32, 0x80u32), (0x14, 0xB0, 0x90)] {
            let deg = (fx.mem.i16(t + 0x3A) + e + s4) as i16 as i32;
            for (o, cos, rr) in [(outer, 1, None), (outer + 8, 0, None), (inn, 1, Some(fi)), (inn + 8, 0, Some(fi))] {
                let s = l2::sin_deg(fx, SINES, deg, cos);
                let rr = rr.unwrap_or_else(|| ee::from_i32(fx.mem.i16(t + 0x38)));
                fx.mem.setf(sp + o, ee::mul(rr, s));
            }
        }
        project_mean(fx, sp + 0x80, sp + 0xD0, d, 0x01E0_3E18);
        s4 = (s4 + 0x14) as i16 as i32;
    }
    fx.leave(0x120);
}

/// 0x1E03840(D, t, d, 0) (frame 0x160, one call): the header (sprite 0x9C)
/// then the sphere: 18 bands of 20 degrees x 12 pieces of 30 from +0x3A,
/// radius 45, at the task's point scaled +0x38 / 2000.
fn sphere_draw(fx: &mut Fx, t: u32, d: u32, r: &Regs, ra: u32) {
    let sp = enter_draw(fx, 0x160, 0xA0, 0x10, 0xA0, 2, r, ra);
    let c = fx.mem.i16(t + 0x34);
    header(fx, d, sp + 0x150, 0x9C, c, t + 0x28);
    point_i16(fx, t, sp + 0xF0);
    let s = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 2000.0);
    l2::set3(fx, sp + 0x100, s, s, s);
    l2::xform_vp(fx, sp + 0x110, sp + 0x100, 0, sp + 0xF0);
    let (mut s7, mut s6) = (0i32, 0x14i32);
    for _ in 0..0x12 {
        let a = s7 as i16 as i32;
        let b = s6 as i16 as i32;
        let y = ee::mul(45.0, l2::sin_deg(fx, SINES, a, 0));
        fx.mem.setf(sp + 0xB4, y);
        fx.mem.setf(sp + 0xC4, y);
        let y = ee::mul(45.0, l2::sin_deg(fx, SINES, b, 0));
        fx.mem.setf(sp + 0xD4, y);
        fx.mem.setf(sp + 0xE4, y);
        let f20 = ee::mul(45.0, l2::sin_deg(fx, SINES, a, 1));
        let f21 = ee::mul(45.0, l2::sin_deg(fx, SINES, b, 1));
        let mut s4 = 0i32;
        for _ in 0..0xC {
            for (e, o21, o20) in [(0i32, 0xD0u32, 0xB0u32), (0x1E, 0xE0, 0xC0)] {
                let deg = (fx.mem.i16(t + 0x3A) + e + s4) as i16 as i32;
                for (o, cos, f) in [(o21, 1, f21), (o21 + 8, 0, f21), (o20, 1, f20), (o20 + 8, 0, f20)] {
                    let v = l2::sin_deg(fx, SINES, deg, cos);
                    fx.mem.setf(sp + o, ee::mul(f, v));
                }
            }
            project_mean(fx, sp + 0xB0, sp + 0x110, d, 0x01E0_3B0C);
            s4 = (s4 + 0x1E) as i16 as i32;
        }
        s7 += 0x14;
        s6 += 0x14;
    }
    fx.leave(0x160);
}

/// A dome kind (0x1E03E70 for tasks, 0x1E041B0 for the records): the
/// offsets of cell, colour, size, latitude and longitude, the latitude
/// cap, the band's height in degrees and the scale divisor.
struct DomeKind {
    cell: u32,
    colour: u32,
    size: u32,
    lat: u32,
    lon: u32,
    cap: Option<i32>,
    band: i32,
    div: f32,
    ras: [u32; 1],
}

const DOME_TASK: DomeKind = DomeKind { cell: 0x34, colour: 0x28, size: 0x38, lat: 0x30, lon: 0x3A, cap: None, band: 0x14, div: 1500.0, ras: [0x01E0_4144] };
const DOME_REC: DomeKind = DomeKind { cell: 0x24, colour: 0x2C, size: 0x28, lat: 0x20, lon: 0x2A, cap: Some(0xA0), band: 0x28, div: 250.0, ras: [0x01E0_449C] };

/// 0x1E03E70 / 0x1E041B0(D, t, d, mode) (frame 0x140): mode 0 heads `d`
/// (sprite 0x97); mode 1 b148's dome band (12 pieces of 30 degrees from
/// the longitude between the latitude and latitude + band (radius 90, y 90
/// sin; the records' cosines from the latitude capped at 0xA0 and + 30)) at
/// the point scaled size / div, turned by +0x10.
fn dome_draw(fx: &mut Fx, t: u32, d: u32, mode: i32, r: &Regs, ra: u32, k: &DomeKind) {
    let sp = enter_draw(fx, 0x140, 0x80, 0x10, 0x80, 2, r, ra);
    if mode == 0 {
        let c = fx.mem.i16(t + k.cell);
        header(fx, d, sp + 0x130, 0x97, c, t + k.colour);
        fx.leave(0x140);
        return;
    }
    point_i16(fx, t, sp + 0xD0);
    let s = ee::div(ee::from_i32(fx.mem.i16(t + k.size)), k.div);
    l2::set3(fx, sp + 0xE0, s, s, s);
    l2::xform_vp(fx, sp + 0xF0, sp + 0xE0, t + 0x10, sp + 0xD0);
    let lat = fx.mem.i16(t + k.lat);
    let lat_c = match k.cap {
        Some(cap) if lat >= cap + 1 => cap,
        _ => lat,
    };
    let y = ee::mul(90.0, l2::sin_deg(fx, SINES, lat, 0));
    fx.mem.setf(sp + 0x94, y);
    fx.mem.setf(sp + 0xA4, y);
    let lat2 = (fx.mem.i16(t + k.lat) + k.band) as i16 as i32;
    let y = ee::mul(90.0, l2::sin_deg(fx, SINES, lat2, 0));
    fx.mem.setf(sp + 0xB4, y);
    fx.mem.setf(sp + 0xC4, y);
    let (c1, c2) = match k.cap {
        None => (lat, lat2),
        Some(_) => (lat_c, (lat_c + 0x1E) as i16 as i32),
    };
    let f20 = ee::mul(90.0, l2::sin_deg(fx, SINES, c1, 1));
    let f21 = ee::mul(90.0, l2::sin_deg(fx, SINES, c2, 1));
    let mut a = 0i32;
    for _ in 0..0xC {
        for (off, outer, inner) in [(0i32, 0xB0u32, 0x90u32), (0x1E, 0xC0, 0xA0)] {
            let deg = (fx.mem.i16(t + k.lon) + off + a) as i16 as i32;
            let c = l2::sin_deg(fx, SINES, deg, 1);
            fx.mem.setf(sp + outer, ee::mul(f21, c));
            let s = l2::sin_deg(fx, SINES, deg, 0);
            fx.mem.setf(sp + outer + 8, ee::mul(f21, s));
            let c = l2::sin_deg(fx, SINES, deg, 1);
            fx.mem.setf(sp + inner, ee::mul(f20, c));
            let s = l2::sin_deg(fx, SINES, deg, 0);
            fx.mem.setf(sp + inner + 8, ee::mul(f20, s));
        }
        project_mean(fx, sp + 0x90, sp + 0xF0, d, k.ras[0]);
        a = (a + 0x1E) as i16 as i32;
    }
    fx.leave(0x140);
}

/// 0x1E04510(D, t, d, mode) (frame 0x120): mode 0 heads `d` as an
/// untextured gouraud strip4 (additive) coloured +0x28 (first two) / +0x2C
/// (last two); mode 1 draws twice a quad between radius 3 and 25 over 10
/// degrees from 0 in the plane y = 0 at the task's point, turned by +0x10.
fn quad_draw(fx: &mut Fx, t: u32, d: u32, mode: i32, r: &Regs, ra: u32) {
    let sp = enter_draw(fx, 0x120, 0x60, 0, 0x60, 0, r, ra);
    if mode == 0 {
        l2::desc_gouraud4(fx, d, 0);
        l2::fullscreen4(fx, d);
        for (k, (o, c)) in [(0x100u32, 0x28u32), (0x108, 0x28), (0x110, 0x2C), (0x118, 0x2C)].into_iter().enumerate() {
            l2::rgbaq(fx, sp + o, t + c);
            copy_words(fx, d + 0x10 + 0x10 * k as u32, sp + o, 2);
        }
        fx.leave(0x120);
        return;
    }
    point_i16(fx, t, sp + 0xB0);
    l2::xform_vp(fx, sp + 0xC0, 0, t + 0x10, sp + 0xB0);
    for _ in 0..2 {
        for o in [0x74u32, 0x84, 0x94, 0xA4] {
            fx.mem.write_u32(sp + o, 0);
        }
        let (a, b) = (0i32, 0xAi32);
        for (deg, rows) in [(a, [(0x90u32, 25.0f32), (0x70, 3.0)]), (b, [(0xA0, 25.0), (0x80, 3.0)])] {
            for (o, rad) in rows {
                let c = l2::sin_deg(fx, SINES, deg, 1);
                fx.mem.setf(sp + o, ee::mul(rad, c));
                let s = l2::sin_deg(fx, SINES, deg, 0);
                fx.mem.setf(sp + o + 8, ee::mul(rad, s));
            }
        }
        project_mean(fx, sp + 0x70, sp + 0xC0, d, 0x01E0_477C);
    }
    fx.leave(0x120);
}
