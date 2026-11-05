//! BMODULE b443 (card 580), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04160, data area 0x1ED0000) [F]: b207's program shape on 150 tasks of 0x40 bytes at D+0x2F0
//! (alive +0x36, step +0x24), spawned by the record's 0x1E04CD0 (b233's
//! spawn without the angle: colour D+0x2DF4, point D+0x250, parameter
//! D+0x2E3E). Nodes 0x16 / 0x18 / 0x1A / 0x6A / 0x40 at D+0x2E24... The
//! emitter 0x1E02330 counts D+0x2E42 with b207's tail (unit points D+0x290
//! / 0x2A0 counted down by 2.4 / 3.8, node points on 0x30..0x36; none of
//! them feeds a spawn): on 1 the object op 0x1F8380(3, ..) (task
//! 0x1E040B0, inline in the entry) and a task with the bare step 0x1E04150
//! (never stepped); 0x4C six orbiting glows 0x1E03C80 (`band18` at the
//! point parameter / 11 of the way from node 0x16 to node 0x18);
//! 0x4C..0x4F twelve rising flames 0x1E02C40 a update at node 0x16
//! (`band9_tex`, following the node); 0x7E eight swirling tubes 0x1E037A0
//! (`spline::tube` mode 2 over eight node histories D+0x28E0 + 0xA0 k);
//! 0x89 a bright disc 0x1E03DA0 (`fan18_far0`), twelve growing flames
//! 0x1E02EA0 (`band9_tex`), twelve smoke bands 0x1E030C0 (`band18_tex`) and
//! ten spinning funnels 0x1E033B0 (record draw 0x1E01B00: rings of nine
//! wedges). The steps 0x1E02940 / 0x2B60 / 0x32E0 / 0x3660 / 0x3B20 /
//! 0x3BD0 / 0x3E80 / 0x3FF0 are tested but never spawned (not ported, nor
//! the draws 0x1E01EA0 behind them). Returns 1 on update 0x7F, 4 on 0x93, 2
//! on 0x89, 8 once the 190-update timer D+0x2DF0 has run out. No 0x1F8100.
use crate::blib::{quads4, spawn as bspawn, spline};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;

const RISE: u32 = 0x01E0_2C40;
const GROW: u32 = 0x01E0_2EA0;
const SMOKE: u32 = 0x01E0_30C0;
const FUNNEL: u32 = 0x01E0_33B0;
const TUBE: u32 = 0x01E0_37A0;
const ORBIT: u32 = 0x01E0_3C80;
const DISC: u32 = 0x01E0_3DA0;
const OBJECT: u32 = 0x01E0_40B0;
const HOLD: u32 = 0x01E0_4150;

const POOL: u32 = D + 0x2F0;
const POS: u32 = D + 0x250;
const HISTS: u32 = D + 0x28E0;
const KEEP: u32 = D + 0x28D0;
const TIMER: u32 = D + 0x2DF0;
const COLOUR: u32 = D + 0x2DF4;
const OT2: u32 = D + 0x2DFC;
const NODES: u32 = D + 0x2E24;
const PARAM: u32 = D + 0x2E3E;
const PARAM_COPY: u32 = D + 0x2E40;
const COUNTER: u32 = D + 0x2E42;
const BYTES: u32 = D + 0x2E44;
const SINES: u32 = D + 0x2F44;
const TEX_F: u32 = D + 0x2EC;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E04160 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1B0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OT2, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x1B0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x30B0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(PARAM_COPY, param);
    fx.mem.set32(TIMER, 0xBE);
    for (k, (n, clear)) in [(0x16, 1), (0x18, 0), (0x1A, 0), (0x6A, 0), (0x40, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2D8 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x2B8, t);
    let t = fx.texture(5, 0);
    fx.mem.write_u32(D + 0x2870, t);
    // rand's frame: s0 = D.
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_42AC);
        let r = fx.div(v, 0x1E).1;
        fx.mem.write_u8(BYTES + k, (r - 0xF) as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    for k in 0..150 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            RISE => run(fx, t, 0x40, 0x01E0_4360, rise),
            GROW => run(fx, t, 0x30, 0x01E0_4380, grow_flame),
            SMOKE => run(fx, t, 0x30, 0x01E0_43A0, smoke),
            FUNNEL => run(fx, t, 0x30, 0x01E0_43E0, funnel),
            TUBE => run(fx, t, 0xC0, 0x01E0_4420, tube),
            ORBIT => run(fx, t, 0x30, 0x01E0_4590, orbit),
            DISC => run(fx, t, 0x20, 0x01E0_45B0, disc),
            OBJECT => {
                for (to, from) in [(0x1ACu32, 0x01E0_4E00u32), (0x1A8, 0x01E0_4E08)] {
                    let w = fx.mem.read_u32(from);
                    fx.mem.write_u32(sp + to, w);
                }
                if first(fx, t) {
                    fx.mem.set16(t + 0x36, 2);
                }
                fx.boundary(0x001F_8380, "object op 0x1F8380", [3, 0, sp + 0x1AC, sp + 0x1A8]);
                if age(fx, t) >= 0x23 {
                    fx.mem.set16(t + 0x36, 0);
                }
                grow(fx, t);
            }
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x80);
    let d = sp + 0x118;
    for k in 0..150 {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            RISE | GROW => {
                let r = if fx.mem.read_u32(t + 0x24) == RISE { sp + 0xA0 } else { sp + 0xB0 };
                cell(fx, t, r, 0x9B);
                fx.mem.set16(D + 0xB8, 1);
                quads4::band9_tex(fx, D, t, d);
                fx.mem.set16(D + 0xB8, 0);
            }
            SMOKE => {
                cell(fx, t, sp + 0xC0, 0x97);
                fx.mem.set16(D + 0xB8, 1);
                fx.mem.set16(D + 0xBE, 0);
                quads4::band18_tex(fx, D, t, d);
                fx.mem.set16(D + 0xB8, 0);
            }
            FUNNEL => {
                cell(fx, t, sp + 0xE0, 0x9B);
                fx.mem.set16(D + 0xB8, 1);
                for k in 0..3 {
                    let v = ee::from_i32(fx.mem.i16(t + 2 * k));
                    fx.mem.setf(sp + 0x80 + 4 * k, v);
                }
                fx.mem.set16(D + 0xBE, 0);
                funnel_draw(fx, t, sp + 0x80);
            }
            TUBE => {
                let v = fx.mem.read_u32(TEX_F);
                fx.mem.write_u32(D + 0x88, v);
                for (o, v) in [(0x94u32, 1), (0x96, 0x10), (0x98, 0x1F), (0x9A, 1)] {
                    fx.mem.set16(D + o, v);
                }
                let f = fx.mem.i16(t + 0x34);
                l2::rect(fx, sp + 0x100, 0x97, f);
                for k in 0..4 {
                    let w = fx.mem.read_u32(sp + 0x100 + 4 * k);
                    fx.mem.write_u32(D + 0x94 + 4 * k, w);
                }
                fx.mem.set16(D + 0x98, 1);
                for o in [0x20u32, 0x24, 0x28] {
                    fx.mem.write_u32(D + o, 0);
                }
                fx.mem.set16(D + 0xBE, 1);
                for k in 0..8 {
                    spline::tube(fx, 0x01E0_12E0, D, t, HISTS + 0xA0 * k, 2);
                }
            }
            ORBIT => {
                node_at(fx, 0, sp + 0x80);
                node_at(fx, 1, sp + 0x90);
                let p = ee::from_i32(fx.mem.i16(t + 0x30));
                for k in 0..3 {
                    let a = fx.mem.f(sp + 0x80 + 4 * k);
                    let b = fx.mem.f(sp + 0x90 + 4 * k);
                    let step = ee::div(ee::sub(b, a), 11.0);
                    fx.mem.setf(sp + 0x90 + 4 * k, step);
                    fx.mem.setf(sp + 0x80 + 4 * k, ee::add(a, ee::mul(step, p)));
                }
                quads4::band18(fx, D, t, 1, sp + 0x80);
            }
            DISC => quads4::fan18_far0(fx, D, t, d),
            _ => {}
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x7F => 1,
        0x93 => 4,
        0x89 => 2,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The texture (4, 0xF) and the atlas cell `id` frame +0x34 (0x106960
/// into `r`) copied to D+0x88 / D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let v = fx.mem.read_u32(TEX_F);
    fx.mem.write_u32(D + 0x88, v);
    let f = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, f);
    for k in 0..4 {
        let w = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// A step's frame with its saves of the entry's s1 = 0 / s0 = D (ra at
/// the frame's top less 0x20, or 0x10 in the 0x20 frame).
fn run(fx: &mut Fx, t: u32, size: u32, ra: u32, f: impl FnOnce(&mut Fx, u32, u32)) {
    let sp = fx.enter(size);
    let (ra, d) = (u64::from(ra), u64::from(D));
    match size {
        0x20 => fx.saved(sp, &[(0x10, ra), (0, d)]),
        0xC0 => fx.saved(sp, &[(0xA0, ra), (0x20, 0), (0x10, d)]),
        _ => fx.saved(sp, &[(0x20, ra), (0x10, 0), (0, d)]),
    }
    f(fx, t, sp);
    fx.leave(size);
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    fx.mem.i16(t + 0x32)
}

fn grow(fx: &mut Fx, t: u32) {
    let n = age(fx, t) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x36) == 1
}

fn odd(fx: &mut Fx, t: u32) -> bool {
    age(fx, t).wrapping_rem(2) == 1
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

/// Alpha +0x2B up by `up` while below `below`.
fn rise_alpha(fx: &mut Fx, t: u32, below: u8, up: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a < below {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
    }
}

fn rand_rem(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// Orange (0x80, 0x40, 0) on parameter 0, else cyan (0, 0x80, 0x80).
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x30) == 0 {
        l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
    } else {
        l2::rgb(fx, t + 0x28, 0, 0x80, 0x80);
    }
}

/// Out by 0xF (ends below 0x11) once the timer is under 0xA.
fn timer_out(fx: &mut Fx, t: u32) {
    if fx.mem.i32(TIMER) < 0xA {
        fall(fx, t, 0x11, 0xF);
    }
}

/// 0x1E02C40 (frame `sp`): a rising flame (size 0x46 shrinking by +0xE =
/// rand % 3 + 6, turn +0x10 rand % 0x4000, +0x14 rand % 0xFFFF, frame
/// rand & 3, +6 = 1): alpha up by 0x50 (below 0xA0) on ages 0 / 1, then
/// out by 0x13 below size 0xC, ending below size -0xC, out by 0xF once the
/// timer is under 0xA; (0x40, 0x40, 0x80) on odd ages, else (0x80, 0x80,
/// 0xB0); its point node 0x16's every update.
fn rise(fx: &mut Fx, t: u32, sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x46);
        let v = fx.rand_from(t, 0x01E0_2C70);
        fx.mem.set32(t + 0x10, v.wrapping_rem(0x4000));
        let r = rand_rem(fx, t, 0x01E0_2C90, 0xFFFF);
        fx.mem.set32(t + 0x14, r);
        let v = fx.rand_from(t, 0x01E0_2CAC);
        fx.mem.set16(t + 0x34, v.wrapping_rem(4));
        fx.mem.set16(t + 0x36, 2);
        let r = rand_rem(fx, t, 0x01E0_2CD0, 3);
        fx.mem.set16(t + 0xE, r + 6);
        fx.mem.set16(t + 6, 1);
        tint(fx, t);
    }
    if age(fx, t) < 2 {
        rise_alpha(fx, t, 0xA0, 0x50);
    } else {
        if fx.mem.i16(t + 0x38) < 0xC {
            fall(fx, t, 0x15, 0x13);
        }
        if fx.mem.i16(t + 0x38) < -0xC {
            fx.mem.set16(t + 0x36, 0);
        }
        timer_out(fx, t);
    }
    if odd(fx, t) {
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x80);
    } else {
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0xB0);
    }
    node_at(fx, 0, sp + 0x30);
    for (k, ra) in [(0u32, 0x01E0_2E44u32), (1, 0x01E0_2E50), (2, 0x01E0_2E5C)] {
        let v = fx.mem.f(sp + 0x30 + 4 * k);
        fx.fix_residue(v, ra);
        fx.mem.set16(t + 2 * k, ee::fixsfsi(v));
    }
    grow(fx, t);
    let s = fx.mem.i16(t + 0x38).wrapping_sub(fx.mem.i16(t + 0xE));
    fx.mem.set16(t + 0x38, s);
}

/// 0x1E02EA0: a growing flame (size 0 growing by +0xE = rand % 2 + 2, turn
/// +0x10 rand % 0x4000, +0x14 rand % 0xFFFF, frame rand % 7, +6 = 1,
/// (0x20, 0x20, 0x80) then (0x20, 0x20, 0x60)): alpha up by 0x50 (below
/// 0xA0) on ages 0 / 1, then out by 0x13 from size 0x51, ending below size
/// -0xC, out by 0xF once the timer is under 0xA.
fn grow_flame(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        let v = fx.rand_from(t, 0x01E0_2ECC);
        fx.mem.set32(t + 0x10, v.wrapping_rem(0x4000));
        let r = rand_rem(fx, t, 0x01E0_2EEC, 0xFFFF);
        fx.mem.set32(t + 0x14, r);
        let r = rand_rem(fx, t, 0x01E0_2F08, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
        let v = fx.rand_from(t, 0x01E0_2F2C);
        fx.mem.set16(t + 0xE, v.wrapping_rem(2) + 2);
        fx.mem.set16(t + 6, 1);
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80);
    }
    if age(fx, t) < 2 {
        rise_alpha(fx, t, 0xA0, 0x50);
    } else {
        if fx.mem.i16(t + 0x38) >= 0x51 {
            fall(fx, t, 0x15, 0x13);
        }
        if fx.mem.i16(t + 0x38) < -0xC {
            fx.mem.set16(t + 0x36, 0);
        }
        timer_out(fx, t);
    }
    l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x60);
    grow(fx, t);
    let s = fx.mem.i16(t + 0x38).wrapping_add(fx.mem.i16(t + 0xE));
    fx.mem.set16(t + 0x38, s);
}

/// 0x1E030C0: a smoke band (size 0 growing by +8 = rand % 2 + 2, turn
/// +0x10 rand % 0x4000 + 0x2000, +0x14 rand % 0x4000 - 0x2000, frame rand
/// % 7, +6 = 0xA, rim (8, 8, 0x80)): alpha up by 0x20 (below 0x40) on
/// ages 0 / 1, then out by 0x13 from size 0x29 and by 0xF once the timer
/// is under 0xA; (0x20, 0x20, 0x80) on odd ages, else (0x40, 0x40, 0x60).
fn smoke(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        let v = fx.rand_from(t, 0x01E0_30EC);
        fx.mem.set32(t + 0x10, v.wrapping_rem(0x4000) + 0x2000);
        let v = fx.rand_from(t, 0x01E0_310C);
        fx.mem.set32(t + 0x14, v.wrapping_rem(0x4000) - 0x2000);
        let r = rand_rem(fx, t, 0x01E0_312C, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
        let v = fx.rand_from(t, 0x01E0_3150);
        fx.mem.set16(t + 8, v.wrapping_rem(2) + 2);
        fx.mem.set16(t + 6, 0xA);
        tint(fx, t);
        l2::rgb(fx, t + 0x2C, 8, 8, 0x80);
    }
    if age(fx, t) < 2 {
        rise_alpha(fx, t, 0x40, 0x20);
    } else {
        if fx.mem.i16(t + 0x38) >= 0x29 {
            fall(fx, t, 0x15, 0x13);
        }
        timer_out(fx, t);
    }
    if odd(fx, t) {
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80);
    } else {
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x60);
    }
    grow(fx, t);
    let s = fx.mem.i16(t + 0x38).wrapping_add(fx.mem.i16(t + 8));
    fx.mem.set16(t + 0x38, s);
}

/// 0x1E033B0: a spinning funnel (size 0 growing by +8 = rand % 2 + 2, turn
/// +0x10 rand % 0x4000 + 0x2000, +0x18 / +0x14 rand % 0x4000 - 0x2000,
/// ring twist +0x1C = -4, frame 2, 4 rings, angle rand % 360 falling by 8
/// an update; colour by the parameter ((0x80, 0x40, 0) / (0x80, 0x80, 8)),
/// rim (0x80, 8, 8), then both (0x20, 0x20, 0x80) after the first update):
/// alphas +0x2B / +0x2F up by 0x20 (below 0x40) on ages 0 / 1, then from
/// size 0x1F down by 7 (+0x2B ending the task below 9), and +0x2B out by
/// 0xF once the timer is under 0xA.
fn funnel(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0);
        let v = fx.rand_from(t, 0x01E0_33DC);
        fx.mem.set32(t + 0x10, v.wrapping_rem(0x4000) + 0x2000);
        let v = fx.rand_from(t, 0x01E0_33FC);
        fx.mem.set32(t + 0x18, v.wrapping_rem(0x4000) - 0x2000);
        let v = fx.rand_from(t, 0x01E0_341C);
        fx.mem.set32(t + 0x14, v.wrapping_rem(0x4000) - 0x2000);
        fx.mem.set32(t + 0x1C, -4);
        fx.mem.set16(t + 0x34, 2);
        fx.mem.set16(t + 0x36, 2);
        let v = fx.rand_from(t, 0x01E0_3450);
        fx.mem.set16(t + 8, v.wrapping_rem(2) + 2);
        fx.mem.set16(t + 6, 4);
        let r = rand_rem(fx, t, 0x01E0_3478, 0x168);
        fx.mem.set16(t + 0x3A, r);
        if fx.mem.i16(t + 0x30) == 0 {
            l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        } else {
            l2::rgb(fx, t + 0x28, 0x80, 0x80, 8);
        }
        l2::rgb(fx, t + 0x2C, 0x80, 8, 8);
    }
    if age(fx, t) < 2 {
        rise_alpha(fx, t, 0x40, 0x20);
        let a = fx.mem.read_u8(t + 0x2F);
        if a < 0x40 {
            fx.mem.write_u8(t + 0x2F, a.wrapping_add(0x20));
        }
    } else {
        if fx.mem.i16(t + 0x38) >= 0x1F {
            fall(fx, t, 9, 7);
            let a = fx.mem.read_u8(t + 0x2F);
            if a >= 9 {
                fx.mem.write_u8(t + 0x2F, a - 7);
            }
        }
        timer_out(fx, t);
    }
    if odd(fx, t) {
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80);
    } else {
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x60);
    }
    l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x80);
    l2::rgb(fx, t + 0x2C, 0x20, 0x20, 0x80);
    grow(fx, t);
    let s = fx.mem.i16(t + 0x38).wrapping_add(fx.mem.i16(t + 8));
    fx.mem.set16(t + 0x38, s);
    let a = fx.mem.i16(t + 0x3A) - 8;
    fx.mem.set16(t + 0x3A, a);
}

/// Node slot `k`'s position into `out`.
fn node_at(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(NODES + 2 * k);
    fx.node_position(h, n, out);
}

/// 0x1E037A0 (frame `sp`): eight swirling tubes ((0x20, 0x20, 0xF0), 9
/// points, size 1, +0xA rand % 0x11 + 0x2C, +8 (rand & 7) + 5, angle rand
/// % 360): on its first update the eight 10-point histories D+0x28E0 +
/// 0xA0 k filled with node 0x16's position; alpha 0xC0 for 0xB updates,
/// then down by 0x23 (ends below 0x25); every update the histories pushed
/// (`spawn::node_history`) and each point j of history k swirled: x += 3
/// sin(8 j) - 3 cos(0x5A + 8 j) + 2 cos(0x28 k), y += 3 cos(8 j) - 3
/// sin(0x5A + 8 j) + 2 sin(0x28 k).
fn tube(fx: &mut Fx, t: u32, sp: u32) {
    if first(fx, t) {
        l2::rgb(fx, t + 0x28, 0x20, 0x20, 0xF0);
        for k in 0..8u32 {
            node_at(fx, 0, sp + 0xB0);
            let hist = HISTS + 0xA0 * k;
            for i in 0..9 {
                for j in 0..3 {
                    let w = fx.mem.read_u32(sp + 0xB0 + 4 * j);
                    fx.mem.write_u32(hist + 0x10 * i + 4 * j, w);
                }
            }
        }
        fx.mem.set16(t + 6, 9);
        fx.mem.write_u8(t + 0x2B, 0);
        let r = rand_rem(fx, t, 0x01E0_3884, 0x11);
        fx.mem.set16(t + 0xA, r + 0x2C);
        let v = fx.rand_from(t, 0x01E0_38A4);
        fx.mem.set16(t + 8, v.wrapping_rem(8) + 5);
        fx.mem.set16(t + 0x34, 0);
        let r = rand_rem(fx, t, 0x01E0_38C8, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x38, 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if age(fx, t) < 0xB {
        fx.mem.write_u8(t + 0x2B, 0xC0);
    } else {
        fall(fx, t, 0x25, 0x23);
    }
    for k in 0..8u32 {
        let n = fx.mem.i16(NODES);
        bspawn::node_history(fx, HISTS + 0xA0 * k + 0x90, 0xA, n);
    }
    let sin = |fx: &mut Fx, deg: i32, cos: i32| l2::sin_deg(fx, SINES, deg as i16 as i32, cos);
    let mut s2 = 0i32;
    for k in 0..8u32 {
        let (mut s3, mut s4) = (0i32, 0x5Ai32);
        for j in 0..10u32 {
            let p = HISTS + 0xA0 * k + 0x10 * j;
            let a = sin(fx, s3, 0);
            let f = ee::mul(3.0, a);
            let b = sin(fx, s4, 1);
            let f = ee::sub(f, ee::mul(3.0, b));
            let x = fx.mem.f(p);
            fx.mem.setf(p, ee::add(x, f));
            let c = sin(fx, s3, 1);
            let f = ee::mul(3.0, c);
            let e = sin(fx, s4, 0);
            let f = ee::sub(f, ee::mul(3.0, e));
            let y = fx.mem.f(p + 4);
            fx.mem.setf(p + 4, ee::add(y, f));
            let g = sin(fx, s2, 1);
            let x = fx.mem.f(p);
            fx.mem.setf(p, ee::add(x, ee::mul(2.0, g)));
            let h = sin(fx, s2, 0);
            let y = fx.mem.f(p + 4);
            fx.mem.setf(p + 4, ee::add(y, ee::mul(2.0, h)));
            s3 = s3.wrapping_add(8);
            s4 = s4.wrapping_add(8);
        }
        s2 = (s2 + 0x28) as i16 as i32;
    }
    grow(fx, t);
}

/// 0x1E03C80: an orbiting glow (size 0x3C, (0x70, 0x70, 0x70) / (0x10,
/// 0x10, 0xC0), +0x18 = 0x4000, angle = the parameter): alpha up by 4 for
/// 0x12 updates, then 0x40 / 0x50 on odd / even ages; ends at age 0xD3 or
/// on update 0x8A.
fn orbit(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x3C);
        l2::rgb(fx, t + 0x28, 0x70, 0x70, 0x70);
        l2::rgb(fx, t + 0x2C, 0x10, 0x10, 0xC0);
        fx.mem.set32(t + 0x18, 0x4000);
        let p = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0x3A, p);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if age(fx, t) < 0x12 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(4));
    } else {
        let v = if odd(fx, t) { 0x40 } else { 0x50 };
        fx.mem.write_u8(t + 0x2B, v);
    }
    grow(fx, t);
    if age(fx, t) >= 0xD3 {
        fx.mem.set16(t + 0x36, 0);
    }
    if counter(fx) == 0x8A {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E03DA0: the bright disc (size 0x12C, (0xF0, 0xF0, 0xF0) / (0x30,
/// 0x30, 0xF0), +0x2F = 0x10, angle rand % 360): alpha up by 0x60 on ages
/// 0 / 1, then down by 0xA (ends below 0x11).
fn disc(fx: &mut Fx, t: u32, _sp: u32) {
    if first(fx, t) {
        fx.mem.set16(t + 0x38, 0x12C);
        l2::rgb(fx, t + 0x28, 0xF0, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x2C, 0x30, 0x30, 0xF0);
        let r = rand_rem(fx, t, 0x01E0_3DF4, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0x10);
    }
    if age(fx, t) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x60));
    } else {
        fall(fx, t, 0x11, 0xA);
    }
    grow(fx, t);
}

/// 0x1E01B00 (D, task, 0, pos) (frame 0x340): +6 rings of nine textured
/// strip3 wedges (10 degrees every 40 from +0x3A plus the ring's twist,
/// which grows by i16 +0x1C a ring from 2) in the task's TRS (unit scale,
/// turn +0x10, at `pos`): ring r spans radii size / n x r .. size / n x (r
/// + 1) (each wedge's inner edge the previous ring's outer one, 0 for the
/// first); UVs D+0x94 in D+0xBE's order, the cell's height D+0x9A divided
/// by n and its y stepped by it each ring; colours +0x28 / +0x2C,
/// texture D+0x88; each wedge submitted D+0xB8 + 1 times at its mean w.
fn funnel_draw(fx: &mut Fx, t: u32, pos: u32) {
    let sp = fx.enter(0x340);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xD0 + 4 * k, w);
    }
    fx.mem.set16(sp + 0xC0, 2);
    let d = sp + 0x2B0;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 1);
    l2::colour_strip3(fx, d, t + 0x2C, 2);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x130 + 4 * k, 0x3F80_0000);
        let w = fx.mem.read_u32(sp + 0xD0 + 4 * k);
        fx.mem.write_u32(sp + 0x120 + 4 * k, w);
    }
    let n = fx.mem.i16(t + 6);
    let h = fx.mem.i16(D + 0x9A);
    let q = fx.div(h, n).0;
    fx.mem.set16(D + 0x9A, q);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    l2::xform_vp(fx, sp + 0x260, sp + 0x130, t + 0x10, sp + 0x120);
    for o in [0xE4u32, 0xF4, 0x104, 0x114, 0xE0, 0xE8, 0xF0, 0xF8] {
        fx.mem.write_u32(sp + o, 0);
    }
    let mut f20 = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), ee::from_i32(fx.mem.i16(t + 6)));
    let f21 = f20;
    fx.bzero(sp + 0x140, 0x120);
    fx.mem.set32(sp + 0xB0, 0);
    let tab = D + 0xD8;
    while fx.mem.i32(sp + 0xB0) < fx.mem.i16(t + 6) {
        let fp = fx.mem.i16(sp + 0xC0);
        let mut s3 = 0i32;
        for k in 0..9u32 {
            let s4 = sp + 0x140 + 0x10 * k;
            let s5 = s3 as i16 as i32;
            let a = fp.wrapping_add(fx.mem.i16(t + 0x3A).wrapping_add(s5)) as i16 as i32;
            let c = l2::sin_deg(fx, tab, a, 1);
            fx.mem.setf(sp + 0x100, ee::mul(f20, c));
            let s = l2::sin_deg(fx, tab, a, 0);
            fx.mem.setf(sp + 0x108, ee::mul(f20, s));
            let b = fp.wrapping_add(s5.wrapping_add(fx.mem.i16(t + 0x3A) + 0xA)) as i16 as i32;
            let c = l2::sin_deg(fx, tab, b, 1);
            fx.mem.setf(sp + 0x110, ee::mul(f20, c));
            let s = l2::sin_deg(fx, tab, b, 0);
            fx.mem.setf(sp + 0x118, ee::mul(f20, s));
            for (to, from) in [(0xE0u32, 0u32), (0xE8, 8), (0xF0, 0x90), (0xF8, 0x98)] {
                let w = fx.mem.read_u32(s4 + from);
                fx.mem.write_u32(sp + to, w);
            }
            for (from, to) in [(0x100u32, 0u32), (0x108, 8), (0x110, 0x90), (0x118, 0x98)] {
                let w = fx.mem.read_u32(sp + from);
                fx.mem.write_u32(s4 + to, w);
            }
            if l2::project(fx, sp + 0xE0, SPR, sp + 0x260, d, sp + 0x2A0) {
                let mut m = 0;
                while m < fx.mem.i16(D + 0xB8) + 1 {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x2A0)) as i32;
                    let ot = fx.mem.read_u32(D + 0x80);
                    fx.submit(d, ot, key);
                    m += 1;
                }
            }
            s3 = (s3 + 0x28) as i16 as i32;
        }
        let y = fx.mem.i16(D + 0x96).wrapping_add(fx.mem.i16(D + 0x9A));
        fx.mem.set16(D + 0x96, y);
        let order = fx.mem.i16(D + 0xBE);
        l2::uv_strip3(fx, d, D + 0x94, order);
        let r = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, r);
        f20 = ee::add(f20, f21);
        let tw = fx.mem.i16(sp + 0xC0).wrapping_add(fx.mem.i16(t + 0x1C));
        fx.mem.set16(sp + 0xC0, tw);
    }
    fx.leave(0x340);
}

/// The record's spawn 0x1E04CD0(D, step, 0) (frame 0x60): the first free
/// task cleared and set off at D+0x250 (as i16) with the colour D+0x2DF4
/// (twice) and +0x30 = D+0x2E3E.
fn spawn(fx: &mut Fx, f: u32) -> i32 {
    fx.enter(0x60);
    let ret = match (0..150u32).find(|&k| fx.mem.i16(POOL + 0x36 + 0x40 * k) == 0) {
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
            let p = fx.mem.i16(PARAM);
            fx.mem.set16(t + 0x30, p);
            fx.mem.write_u32(t + 0x20, 0);
            fx.mem.write_u32(t + 0x24, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// Node slot `k`'s position (into the emitter's sp + 0x40) copied to the
/// spawn point.
fn at_node(fx: &mut Fx, sp: u32, k: u32) {
    node_at(fx, k, sp + 0x40);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
        fx.mem.write_u32(POS + 4 * k, w);
    }
}

/// The spawn point (x, y - 10, 40 + z) of the unit point at `p`.
fn at_unit(fx: &mut Fx, p: u32) {
    let x = fx.mem.f(p);
    fx.mem.setf(POS, x);
    let y = fx.mem.f(p + 4);
    fx.mem.setf(POS + 4, ee::sub(y, 10.0));
    let z = fx.mem.f(p + 8);
    fx.mem.setf(POS + 8, ee::add(40.0, z));
}

/// 0x1E02330(D, 0) (frame 0x50): the spawn colour (0x80, 0, 0, 0), the
/// update counter, the spawn points and the spawns (b207's emitter on
/// other spawns, the same tail).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x50);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    if counter(fx) == 0x7E {
        spawn(fx, TUBE);
    }
    if counter(fx) == 1 {
        spawn(fx, OBJECT);
    }
    if counter(fx) == 0x4C {
        for k in 0..6 {
            fx.mem.set16(PARAM, k);
            spawn(fx, ORBIT);
        }
    }
    let c = counter(fx);
    if (0x4C..0x50).contains(&c) {
        at_node(fx, sp, 0);
        for _ in 0..12 {
            spawn(fx, RISE);
        }
    }
    if counter(fx) == 0x89 {
        at_node(fx, sp, 0);
        spawn(fx, DISC);
        for _ in 0..12 {
            spawn(fx, GROW);
        }
        for _ in 0..12 {
            spawn(fx, SMOKE);
        }
        for _ in 0..10 {
            spawn(fx, FUNNEL);
        }
    }
    let c = counter(fx);
    if (0x5E..0x82).contains(&c) {
        at_node(fx, sp, 0);
    }
    if counter(fx) == 0x33 {
        node_at(fx, 1, KEEP);
    }
    let c = counter(fx);
    if (0x34..0x41).contains(&c) {
        node_at(fx, 1, sp + 0x40);
        for k in 0..3 {
            let w = fx.mem.read_u32(KEEP + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
    }
    if counter(fx) == 1 {
        let h = fx.self_handle();
        fx.unit_position(h, D + 0x290);
        let h = fx.self_handle();
        fx.unit_position(h, D + 0x2A0);
    }
    let c = counter(fx);
    if (0x70..0xC3).contains(&c) {
        let z = fx.mem.f(D + 0x298);
        fx.mem.setf(D + 0x298, ee::sub(z, f32::from_bits(0x4019_999A)));
        at_unit(fx, D + 0x290);
        fx.mem.set16(PARAM, 0);
    }
    if counter(fx) == 0x50 {
        at_node(fx, sp, 2);
        fx.mem.set16(PARAM, 0);
        at_node(fx, sp, 3);
        fx.mem.set16(PARAM, 0);
    }
    let c = counter(fx);
    if (0xAB..0xE1).contains(&c) {
        let z = fx.mem.f(D + 0x2A8);
        fx.mem.setf(D + 0x2A8, ee::sub(z, f32::from_bits(0x4073_3333)));
        at_unit(fx, D + 0x2A0);
        fx.mem.set16(PARAM, 1);
    }
    let c = counter(fx);
    if (0x70..0xBF).contains(&c) {
        at_unit(fx, D + 0x290);
        fx.mem.set16(PARAM, 0);
    }
    let c = counter(fx);
    if (0xA9..0xDD).contains(&c) {
        at_unit(fx, D + 0x2A0);
        fx.mem.set16(PARAM, 1);
    }
    if counter(fx) == 1 {
        spawn(fx, HOLD);
    }
    let c = counter(fx);
    if (0x30..=0x33).contains(&c) {
        at_node(fx, sp, 0);
        fx.mem.set16(PARAM, 0);
    }
    let c = counter(fx);
    if (0x33..=0x36).contains(&c) {
        at_node(fx, sp, 1);
        fx.mem.set16(PARAM, 1);
    }
    fx.leave(0x50);
}
