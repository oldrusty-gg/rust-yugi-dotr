//! BMODULE b194 (card 176), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01B30) [F]. The entry
//! takes (D, param). b130 / b117's task-pool family: 220 tasks of 0x40 at
//! D+0x524 (alive +0x36, step +0x24 as a record address), spawned by
//! 0x1E021A0 (b538's spawn: the first free task from 0, set off at D+0x80
//! as i16 with the colour D+0x3DD4 twice, +0x30 = the kind D+0x3E2A), the
//! sines D+0x3E34, timer D+0x3DD0 = 0xA0. Nodes (all the attacker's):
//! 0x18 / 0xB / 1 / 0x33 / 0x2F / 0x21 at D+0x3E18.., textures (4, 2)
//! D+0xF8 and (4, 0xE / 0xF) D+0x128 / D+0x12C.
//!
//! The emitter 0x1E00780 spawns only five steps: 0x1E01760 (six motes, one
//! per kind / node, on every update 0x34..0x84 and ten sets on 0x16),
//! 0x1E01010 (two flames a update at nodes 0x18 / 0xB jittered, 0x32..0x56),
//! 0x1E00E80 (twenty fans on 0x5B, kinds 0 / 1), 0x1E015C0 (fifty drifts
//! on 0x5B) and 0x1E01180 (a ray from node 0x18 toward node 1, 0x55..0x5F).
//! The record's other steps (0x1E00C60, 0x1E014B0, 0x1E01A20, 0x1E01AE0)
//! and their draws (0x1E00080 sprite_spec, 0x1E02AB0) are never spawned
//! [F: no `jal 0x1E021A0` with those addresses]. Draws (each twice, mode 0
//! then 1, into the entry's descriptor sp + 0x88): fans 0x1E02550 (nine
//! two-degree slivers), flames 0x1E022C0 (a quad turned by the attacker's
//! yaw, sheared by +6 / +0xE), rays 0x1E027D0 (nine one-degree slivers of a
//! 9-value descriptor from uv_rect4), motes / drifts 0x1E02CD0 (b403's
//! drift_draw; motes with ALPHA 0x42). Returns 1 on 0x51, 4 on 0x7D, 2 on
//! 0x69, 6 on 0x5C..0x63, 8 once the timer has run out.
use crate::blib::spawn::node_history;
use crate::blib::uv::uv_rect4;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0xDC;
const POOL: u32 = D + 0x524;
const POS: u32 = D + 0x80;
const TEX_A: u32 = D + 0xF8;
const TEX_B: u32 = D + 0x12C;
const HIST: u32 = D + 0x3D20;
const HIST_TOP: u32 = D + 0x3DB0;
const TIMER: u32 = D + 0x3DD0;
const COLOUR: u32 = D + 0x3DD4;
const OTW: u32 = D + 0x3DDC;
const MEAN_W: u32 = D + 0x3E04;
const NODE: u32 = D + 0x3E18;
const KIND: u32 = D + 0x3E2A;
const PARAM: u32 = D + 0x3E2C;
const COUNTER: u32 = D + 0x3E2E;
const TAB: u32 = D + 0x3E34;

const FAN: u32 = 0x01E0_0E80;
const FLAME: u32 = 0x01E0_1010;
const RAY: u32 = 0x01E0_1180;
const DRIFT: u32 = 0x01E0_15C0;
const MOTE: u32 = 0x01E0_1760;

const D05: f64 = 0.5;
const ALPHA_42: u64 = (0x80 << 32) | 0x42;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x120);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x120);
    ret
}

fn rmod(fx: &mut Fx, m: i32) -> i32 {
    let v = fx.rand();
    fx.div(v, m).1
}

fn fix(fx: &mut Fx, v: f32) -> i32 {
    fx.fix_residue(v, 0);
    ee::fixsfsi(v)
}

fn fixu(fx: &mut Fx, at: u32) -> i32 {
    let v = fx.mem.f(at);
    fx.fix_residue(v, 0);
    ee::fixunssfsi(v) as i32
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3FA0);
    l2::sine_table(fx, TAB);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0xA0);
    for o in [0xC0u32, 0xC4, 0xC8] {
        fx.mem.set32(D + o, 0);
    }
    for (k, (n, clear)) in [(0x18, 1), (0xB, 0), (1, 0), (0x33, 0), (0x2F, 0), (0x21, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(TEX_A, t);
    for k in 0xEu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x128 + 4 * (k - 0xE), t);
    }
    0
}

/// (double) `v` < 0.5 (0x1A2C30 then 0x17BCA0; a denormal converts to 0).
fn below_half(v: f32) -> bool {
    let bits = v.to_bits();
    let a = if (bits >> 23) & 0xFF == 0 { 0.0 } else { f64::from(v) };
    a < D05
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    if fx.mem.i16(COUNTER) == 0x2D {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0x70);
        for k in 0..9u32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
                fx.mem.write_u32(HIST + 0x10 * k + 4 * c, w);
            }
        }
    }
    let n = fx.mem.i16(NODE + 4);
    node_history(fx, HIST_TOP, 0xA, n);
    emit(fx);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            FAN => fan(fx, t),
            FLAME => flame(fx, t),
            RAY => ray(fx, t),
            DRIFT => drift(fx, t),
            MOTE => mote(fx, t),
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    let z = ee::sub(fx.mem.f(sp + 0x58), 25.0);
    fx.mem.setf(sp + 0x58, z);
    let d = sp + 0x88;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        let g: fn(&mut Fx, u32, u32, i32) = match fx.mem.read_u32(t + 0x24) {
            FAN => fan_draw,
            FLAME => flame_draw,
            RAY => ray_draw,
            DRIFT | MOTE => sprite_draw,
            _ => continue,
        };
        g(fx, t, d, 0);
        g(fx, t, d, 1);
    }
    if fx.mem.i16(COUNTER) >= 0x15 {
        for o in [0xC0u32, 0xC8] {
            if o == 0xC8 {
                fx.mem.write_u32(D + 0xC4, 0x3DCC_CCCD);
            }
            let v = fx.mem.f(D + o);
            if below_half(v) {
                let v = ee::add(fx.mem.f(D + o), f32::from_bits(0x3C23_D70A));
                fx.mem.setf(D + o, v);
            }
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x51 => 1,
        0x7D => 4,
        0x69 => 2,
        0x5C..=0x63 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E021A0(D, step, 0): the first free task from 0 (b538's spawn).
fn spawn(fx: &mut Fx, f: u32) -> i32 {
    let sp = fx.enter(0x60);
    let _ = sp;
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(POOL + 0x40 * k + 0x36) == 0) {
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
            let a = fx.mem.i16(KIND);
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

fn kinds(fx: &mut Fx, ks: std::ops::Range<i32>, f: u32) {
    for k in ks {
        fx.mem.set16(KIND, k);
        spawn(fx, f);
    }
}

/// 0x1E00780(D, 0) (frame 0x50): the spawn colour (0x80, 0, 0, 0), the
/// update counter and the spawns.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x50);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    if c < 0x85 && c >= 0x34 {
        kinds(fx, 0..6, MOTE);
    }
    if fx.mem.i16(COUNTER) == 0x16 {
        for _ in 0..0xA {
            kinds(fx, 0..2, MOTE);
        }
    }
    if fx.mem.i16(COUNTER) == 0x16 {
        for _ in 0..0xA {
            kinds(fx, 2..6, MOTE);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x32..0x57).contains(&c) {
        for slot in [0u32, 2] {
            let h = fx.self_handle();
            let n = fx.mem.i16(NODE + slot);
            fx.node_position(h, n, sp + 0x40);
            let r = rmod(fx, 0xA) - 5;
            let x = ee::sub(fx.mem.f(sp + 0x40), ee::from_i32(r));
            fx.mem.setf(POS, x);
            if fx.mem.i16(COUNTER) >= 0x49 {
                let r = rmod(fx, 0xA) - 5;
                let x = ee::add(fx.mem.f(sp + 0x40), ee::from_i32(r));
                fx.mem.setf(POS, x);
            }
            let r = rmod(fx, 0xF);
            let y = ee::add(fx.mem.f(sp + 0x44), ee::from_i32(r));
            fx.mem.setf(POS + 4, y);
            let z = fx.mem.read_u32(sp + 0x48);
            fx.mem.write_u32(POS + 8, z);
            spawn(fx, FLAME);
        }
    }
    if fx.mem.i16(COUNTER) == 0x5B {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0x40);
        for c in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * c);
            fx.mem.write_u32(POS + 4 * c, w);
        }
        for k in 0..2 {
            fx.mem.set16(KIND, k);
            for _ in 0..0xA {
                spawn(fx, FAN);
            }
        }
        for _ in 0..0x32 {
            spawn(fx, DRIFT);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x55..0x60).contains(&c) {
        spawn(fx, RAY);
    }
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    n
}

fn fade_in(fx: &mut Fx, t: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a < cap {
        fx.mem.write_u8(t + 0x2B, a.wrapping_add(up));
    }
}

/// Alpha +0x2B down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x2B);
    if a >= floor {
        fx.mem.write_u8(t + 0x2B, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// i16 `at` += `k` x (sine or cosine of i16 `deg`) through 0x1A2B80.
fn wobble(fx: &mut Fx, t: u32, at: u32, deg: u32, cos: i32, k: f32) {
    let a = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, TAB, a, cos);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(k, s));
    let r = fix(fx, f);
    fx.mem.set16(t + at, r);
}

/// The task's i16 position = the floats at `p` through 0x1A2B80.
fn place(fx: &mut Fx, t: u32, p: u32) {
    for c in 0..3u32 {
        let v = fx.mem.f(p + 4 * c);
        let r = fix(fx, v);
        fx.mem.set16(t + 2 * c, r);
    }
}

/// 0x1E00E80 (D, task): a fan (size 8 up by 2, +0x10 rand & 0x3FFF, +0x14
/// rand % 0xFFFF, cell rand % 7; orange or teal by kind; +6 = 1).
fn fan(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 8);
        let r = fx.rand();
        fx.mem.set32(t + 0x10, r & 0x3FFF);
        let r = rmod(fx, 0xFFFF);
        fx.mem.set32(t + 0x14, r);
        let r = rmod(fx, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 6, 1);
        if fx.mem.i16(t + 0x30) == 0 {
            l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
        } else {
            l2::rgb(fx, t + 0x28, 0, 0x80, 0x80);
        }
    }
    if fx.mem.i16(t + 0x32) < 2 {
        fade_in(fx, t, 0xA0, 0x50);
    } else {
        if fx.mem.i16(t + 0x38) >= 0x10 {
            fade_out(fx, t, 0xF, 0xD);
        }
        if fx.mem.i32(TIMER) < 0xA {
            fade_out(fx, t, 0x11, 0xF);
        }
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38) + 2;
    fx.mem.set16(t + 0x38, s);
}

/// 0x1E01010 (D, task) (frame 0x40): an orange flame (size 0x32), its
/// shear +6 = +-0x14 after update 0x4E / 0x55, +0xE up by 5.
fn flame(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x40);
    l2::rgb(fx, t + 0x28, 0x80, 0x40, 0);
    if fx.mem.i16(t + 0x36) == 1 {
        let h = fx.self_handle();
        fx.unit_rotation(h, sp + 0x30);
        let c = fx.mem.i16(COUNTER);
        if c >= 0x55 {
            fx.mem.set16(t + 6, 0x14);
        } else if c >= 0x4E {
            fx.mem.set16(t + 6, -0x14);
        }
        fx.mem.set32(t + 0x14, 0);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        fade_in(fx, t, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0xB, 9);
    }
    if fx.mem.i16(COUNTER) >= 0x55 {
        let v = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, v);
    }
    fx.mem.set16(t + 0x38, 0x32);
    age(fx, t);
    if fx.mem.i32(TIMER) < 0xF {
        fade_out(fx, t, 0xF, 0xD);
    }
    let v = fx.mem.i16(t + 0xE) + 5;
    fx.mem.set16(t + 0xE, v);
    fx.leave(0x40);
}

/// 0x108B90(x, y): the angle of (x, y) in degrees by octant (45 y / x or
/// 45 x / y, integer division).
fn angle(x: i32, y: i32) -> i32 {
    if x == 0 && y == 0 {
        return 0;
    }
    let (ax, ay) = (i64::from(x.wrapping_abs()), i64::from(y.wrapping_abs()));
    if ax + ay == 0 {
        return 0;
    }
    let lo = |a: i64, b: i64| (45 * a / b) as i32;
    let v = if y >= 0 {
        if x > 0 {
            if ax == ay {
                0x2D
            } else if ay < ax {
                lo(ay, ax)
            } else {
                90 - lo(ax, ay)
            }
        } else if ax == ay {
            0x87
        } else if ay < ax {
            180 - lo(ay, ax)
        } else {
            lo(ax, ay) + 90
        }
    } else if x > 0 {
        if ax == ay {
            0x13B
        } else if ay < ax {
            360 - lo(ay, ax)
        } else {
            lo(ax, ay) + 270
        }
    } else if ax == ay {
        0xE1
    } else if ay < ax {
        lo(ay, ax) + 180
    } else {
        270 - lo(ax, ay)
    };
    v
}

/// 0x1E01180 (D, task) (frame 0x90): a dark ray at node 0x18 (until update
/// 0x60) pitched toward node 1 by the attacker's facing.
fn ray(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    let yaw = fix(fx, y) as i16;
    if fx.mem.i16(t + 0x36) == 1 {
        let r = rmod(fx, 0xA);
        fx.mem.set16(t + 0x38, r + 0xA);
        for o in [0x18u32, 0x10, 0x14] {
            fx.mem.set32(t + o, 0);
        }
        let r = rmod(fx, 7);
        fx.mem.set16(t + 0x34, r);
        fx.mem.set16(t + 0x36, 2);
        let r = rmod(fx, 0xF);
        fx.mem.set16(t + 6, r + 5);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x3A, r);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE);
    fx.node_position(h, n, sp + 0x60);
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE + 4);
    fx.node_position(h, n, sp + 0x70);
    if fx.mem.i16(COUNTER) < 0x60 {
        place(fx, t, sp + 0x60);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        fade_in(fx, t, 0x80, 0x40);
    } else {
        if fx.mem.i16(t + 0x38) >= 0x1A {
            fade_out(fx, t, 0xF, 0xD);
        }
        if fx.mem.i32(TIMER) < 0xA {
            fade_out(fx, t, 0x11, 0xF);
        }
    }
    l2::rgb(fx, t + 0x28, 0x20, 0x20, 0);
    let f = |fx: &mut Fx, o: u32| fx.mem.f(sp + o);
    let dx = ee::sub(f(fx, 0x60), f(fx, 0x70));
    let dz = ee::sub(f(fx, 0x68), f(fx, 0x78));
    let xx = ee::mul(dx, dx);
    let zz = ee::mul(dz, dz);
    let dy = ee::sub(f(fx, 0x64), f(fx, 0x74));
    let s = fx.sqrtf(ee::add(xx, zz));
    let a = fix(fx, s) as i16 as i32;
    let s = fx.sqrtf(ee::add(xx, ee::add(ee::mul(dy, dy), zz)));
    let b = fix(fx, s) as i16 as i32;
    let v = angle(a, b) as i16 as i32;
    if fx.mem.i16(COUNTER) < 0x60 {
        let v = if yaw != 0 { v } else { v.wrapping_neg() };
        fx.mem.set32(t + 0x10, v.wrapping_mul(180));
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38) + 1;
    fx.mem.set16(t + 0x38, s);
    fx.leave(0x90);
}

/// 0x1E015C0 (D, task): b112's drift (size 13, cell 2, (0x60, 0x40, 0),
/// alpha 0x80 for 10 updates then out by 10, wobbling by 9).
fn drift(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xD);
        fx.mem.set16(t + 0x34, 2);
        l2::rgb(fx, t + 0x28, 0x60, 0x40, 0);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x30, r);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
    wobble(fx, t, 8, 0x3A, 0, 9.0);
    wobble(fx, t, 0xA, 0x30, 1, 9.0);
    wobble(fx, t, 0xC, 0x3A, 1, 9.0);
}

/// 0x1E01760 (D, task) (frame 0x40): a mote at the attacker's node of its
/// kind (violet for kinds 0 / 1, else grey), held there for its first 6
/// (kinds >= 2: 4) updates, growing by 5 to 0x28.
fn mote(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x40);
    if fx.mem.i16(t + 0x36) == 1 {
        if fx.mem.i16(t + 0x30) < 2 {
            l2::rgb(fx, t + 0x28, 0x3F, 0x30, 0x3F);
        } else {
            l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x10);
        }
        fx.mem.set16(t + 0x38, 0xA);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, 0x5A);
        fx.mem.set16(t + 6, r);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 3 {
        fade_in(fx, t, 0x60, 0x20);
    } else {
        fade_out(fx, t, 0xB, 9);
    }
    let a = age(fx, t);
    if a < 6 {
        let h = fx.self_handle();
        let kind = fx.mem.i16(t + 0x30);
        let n = fx.mem.i16(NODE.wrapping_add((2 * kind) as u32));
        fx.node_position(h, n, sp + 0x30);
        if fx.mem.i16(t + 0x30) < 2 || fx.mem.i16(t + 0x32) < 4 {
            place(fx, t, sp + 0x30);
        }
    }
    let s = fx.mem.i16(t + 0x38);
    if s < 0x28 {
        fx.mem.set16(t + 0x38, s + 5);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    wobble(fx, t, 8, 0x3A, 0, 8.0);
    wobble(fx, t, 0xA, 6, 1, 2.0);
    wobble(fx, t, 0xC, 0x3A, 1, 8.0);
    fx.leave(0x40);
}

/// Mode 0 of the strip3 sprites: rect `id` (cell +0x34) into `r`, the
/// strip3 header, colour +0x28, UVs, texture `tex`.
fn setup(fx: &mut Fx, t: u32, d: u32, r: u32, id: i32, tex: u32) {
    let c = fx.mem.i16(t + 0x34);
    l2::rect(fx, r, id, c);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    l2::uv_strip3(fx, d, r, 0);
    let w = fx.mem.read_u32(tex);
    fx.desc_texture(d, w, 1);
}

/// The motes' (0x1E01760) rect 0x14 / texture D+0xF8, else 0x9B / D+0x12C.
fn setup_by_step(fx: &mut Fx, t: u32, d: u32, r: u32) {
    if fx.mem.read_u32(t + 0x24) == MOTE {
        setup(fx, t, d, r, 0x14, TEX_A);
    } else {
        setup(fx, t, d, r, 0x9B, TEX_B);
    }
}

/// The task's i16 position + its i16 +8 / +0xA / +0xC / 10.0 at `out`.
fn trans_tenth_f(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3u32 {
        let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(out + 4 * k, ee::add(p, o));
    }
}

/// The task's i16 position as floats at `out`.
fn trans(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(out + 4 * k, v);
    }
}

/// 0x1E02CD0 (D, task, desc, mode): b403's drift_draw (the motes with
/// ALPHA 0x42): a camera-facing square of half size +0x38 scaled 0.1 at the
/// task's position + offsets / 10.
fn sprite_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        setup_by_step(fx, t, d, sp + 0xC0);
    } else {
        if fx.mem.read_u32(t + 0x24) == MOTE {
            fx.desc_alpha(d, ALPHA_42);
        }
        for k in 0..3 {
            fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
        }
        l2::camera_angles_to(fx, sp + 0x110);
        trans_tenth_f(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x110, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xD0, OT);
    }
    fx.leave(0x120);
}

/// 0x1E022C0 (D, task, desc, mode): the flames: a quad of +0x38 x 1 scaled
/// 0.1, turned about y by -(yaw - 0x4000), its left corners at x 0, then x
/// += +0xE, its right corners' y += +6 and every y += +6.
fn flame_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x130);
    if mode == 0 {
        setup_by_step(fx, t, d, sp + 0xC0);
    } else {
        for k in 0..3 {
            fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
        }
        let h = fx.self_handle();
        fx.unit_rotation(h, sp + 0x120);
        let y = ee::sub(ee::from_i32(fx.mem.i32(sp + 0x124)), 16384.0);
        let r = fix(fx, ee::neg(y));
        fx.mem.set32(sp + 0x114, r);
        fx.mem.set32(sp + 0x110, 0);
        fx.mem.set32(sp + 0x118, 0);
        trans_tenth_f(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, sp + 0x110, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, 1.0);
        fx.mem.set32(sp + 0x60, 0);
        fx.mem.set32(sp + 0x80, 0);
        let e = ee::from_i32(fx.mem.i16(t + 0xE));
        l2::add_column(fx, sp + 0x60, 0, e);
        for o in [0x74u32, 0x94] {
            let v = ee::add(fx.mem.f(sp + o), ee::from_i32(fx.mem.i16(t + 6)));
            fx.mem.setf(sp + o, v);
        }
        let e = ee::from_i32(fx.mem.i16(t + 6));
        l2::add_column(fx, sp + 0x60, 1, e);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xD0, OT);
    }
    fx.leave(0x130);
}

/// The nine slivers of a fan (0x1E02550 / 0x1E027D0): `width` degrees
/// every 40 from +0x3A between the centre and radius +0x38 (the centre's y
/// `centre_y`), turned by +0x10, scaled (1, 1.5, 1) at the task's point,
/// each keyed by its mean w (D+0x3E04) into D+0x3DDC.
fn slivers(fx: &mut Fx, sp: u32, t: u32, d: u32, width: i32, centre_y: Option<f32>) {
    use ee::mul;
    trans(fx, t, sp + 0xC0);
    for (o, v) in [(0xD0u32, 0x3F80_0000u32), (0xD4, 0x3FC0_0000), (0xD8, 0x3F80_0000)] {
        fx.mem.write_u32(sp + o, v);
    }
    l2::xform_vp(fx, sp + 0xE0, sp + 0xD0, t + 0x10, sp + 0xC0);
    match centre_y {
        None => {
            for o in [0x84u32, 0x94, 0xA4, 0xB4] {
                fx.mem.write_u32(sp + o, 0);
            }
        }
        Some(y) => {
            fx.mem.write_u32(sp + 0x84, 0);
            fx.mem.write_u32(sp + 0x94, 0);
            fx.mem.setf(sp + 0xA4, y);
            fx.mem.setf(sp + 0xB4, y);
        }
    }
    let mut s1 = 0i32;
    for _ in 0..9 {
        let a = (fx.mem.i16(t + 0x3A) + s1) as i16 as i32;
        let c = l2::sin_deg(fx, TAB, a, 1);
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + 0xA0, mul(r, c));
        let s = l2::sin_deg(fx, TAB, a, 0);
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + 0xA8, mul(r, s));
        fx.mem.write_u32(sp + 0x80, 0);
        fx.mem.write_u32(sp + 0x88, 0);
        let b = (s1 + (fx.mem.i16(t + 0x3A) + width)) as i16 as i32;
        let c = l2::sin_deg(fx, TAB, b, 1);
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + 0xB0, mul(r, c));
        let s = l2::sin_deg(fx, TAB, b, 0);
        let r = ee::from_i32(fx.mem.i16(t + 0x38));
        fx.mem.setf(sp + 0xB8, mul(r, s));
        fx.mem.write_u32(sp + 0x90, 0);
        fx.mem.write_u32(sp + 0x98, 0);
        if l2::project(fx, sp + 0x80, SPR, sp + 0xE0, d, MEAN_W) {
            let key = fixu(fx, MEAN_W);
            let ot = fx.mem.read_u32(OTW);
            fx.submit(d, ot, key);
        }
        s1 = (s1 + 0x28) as i16 as i32;
    }
}

/// 0x1E02550 (D, task, desc, mode): mode 0 a strip3 (rect 0x97 cell +0x34,
/// texture D+0x12C); mode 1 (ALPHA 0x42 while +6) nine 2-degree slivers.
fn fan_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x130);
    if mode == 0 {
        setup(fx, t, d, sp + 0x120, 0x97, TEX_B);
    } else {
        if fx.mem.i16(t + 6) != 0 {
            fx.desc_alpha(d, ALPHA_42);
        }
        slivers(fx, sp, t, d, 2, None);
    }
    fx.leave(0x130);
}

/// 0x1E027D0 (D, task, desc, mode): mode 0 a 9-value descriptor (RGBAQ +
/// four UV / XYZ2 pairs, flags 0xA24, ALPHA 0x48), colour +0x28, UVs by
/// uv_rect4 of rect 0x97 cell +0x34, texture D+0x12C; mode 1 nine 1-degree
/// slivers whose centre is lifted by +6.
fn ray_draw(fx: &mut Fx, t: u32, d: u32, mode: i32) {
    let sp = fx.enter(0x140);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x120, 0x97, c);
        fx.mem.set16(d + 2, 0x24);
        fx.mem.set16(d, 9);
        fx.mem.write_u64(d + 8, (4 << 32) | 0x2424_2421);
        fx.mem.set32(d + 4, 0);
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        fx.desc_alpha(d, (0x80 << 32) | 0x48);
        fx.mem.set32(d + 0x14, 0);
        for o in [0x27u32, 0x37, 0x47, 0x57] {
            fx.mem.write_u8(d + o, 0);
        }
        l2::rgbaq(fx, sp + 0x138, t + 0x28);
        for k in 0..2 {
            let w = fx.mem.read_u32(sp + 0x138 + 4 * k);
            fx.mem.write_u32(d + 0x10 + 4 * k, w);
        }
        uv_rect4(fx, d, sp + 0x120, 0);
        let w = fx.mem.read_u32(TEX_B);
        fx.desc_texture(d, w, 1);
    } else {
        let y = ee::from_i32(fx.mem.i16(t + 6));
        slivers(fx, sp, t, d, 1, Some(y));
    }
    fx.leave(0x140);
}
