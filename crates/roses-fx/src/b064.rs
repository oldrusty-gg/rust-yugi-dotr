//! BMODULE b064 (card 232) and its twin b355 (card 248), hand-ported from
//! the records (linked at 0x1E00000; entries 0x1E030C0 / 0x1E030B0) [F]
//!. One program: b355's code from 0x1E017D0 on is
//! b064's 0x10 lower; b355's emitter does not spawn the 0x2020 step, and a
//! few colours, sizes and the frame tint differ ([`Variant`]). The entry
//! takes (D, param). A task pool of 300 tasks of 0x40 bytes at D+0x2A0
//! (alive +0x36, step +0x24 as a record address), spawned by 0x1E03A10,
//! drawn with the second library (58c265bfc632 `quads4::sprite_spec`,
//! 383a1b86ead2 `quads4::ring_at_m`, 5705ae2c0496 `quads4::band18_tex`)
//! and the records' own sprites, rings and bands, each set up into the
//! entry's descriptor sp + 0xC0 (mode 0) and drawn (mode 1) by two calls.
//! The attacker's node 0x21 throws smoke, sparks and a rising column of
//! puffs (D+0x4F10.. points, one per arm); from update 110 the target
//! burns (flames, glows, bands). Returns 1 on update 110, 3 on 245, 6 on
//! 121..239, 8 once the 266-update timer D+0x51C0 has run out.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0x12C;
const POOL: u32 = D + 0x2A0;
const POS: u32 = D + 0x250;
const PTS: u32 = D + 0x4F10;
const HOLD: u32 = D + 0x5190;
const TIMER: u32 = D + 0x51C0;
const COLOUR: u32 = D + 0x51C4;
const OTW: u32 = D + 0x51CC;
const MEAN: u32 = D + 0x51EC;
const NODE: u32 = D + 0x51FC;
const ANGLE: u32 = D + 0x5206;
const FLIP: u32 = D + 0x520A;
const INDEX: u32 = D + 0x520C;
const COUNTER: u32 = D + 0x5210;
const AGE: u32 = D + 0x5212;
const FACING: u32 = D + 0x5214;
const ARMS: u32 = D + 0x5218;
const BYTES: u32 = D + 0x5242;
const TAB: u32 = D + 0x5344;

/// The twin's differences.
#[derive(Clone, Copy)]
pub struct Variant {
    /// b355: its code from 0x1E017D0 on sits 0x10 lower.
    b355: bool,
}

impl Variant {
    /// A b064 record address (from 0x1E017E0) in this record.
    fn at(self, a: u32) -> u32 {
        if self.b355 {
            a - 0x10
        } else {
            a
        }
    }
    /// A b064 emitter address after its first block in this record.
    fn em(self, a: u32) -> u32 {
        if self.b355 {
            a - 0x18
        } else {
            a
        }
    }
}

const B064: Variant = Variant { b355: false };
const B355: Variant = Variant { b355: true };

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    entry(fx, param, B064)
}

pub fn program_b355(fx: &mut Fx, param: i32) -> i32 {
    entry(fx, param, B355)
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// rand % m (HI) from return address `ra` (rand's frame: `s0`).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// 0x1A2B80 on `v` from return address `ra` (its residue).
fn fix(fx: &mut Fx, v: f32, ra: u32) -> i32 {
    fx.fix_residue(v, ra);
    ee::fixsfsi(v)
}

/// The entry 0x1E030C0 (D, param).
fn entry(fx: &mut Fx, param: i32, v: Variant) -> i32 {
    let sp = fx.enter(0x150);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, sp, param, v) } else { step(fx, sp, param, v) };
    fx.leave(0x150);
    ret
}

/// The yaw of the attacker (0x1F7370's rotation y / 180 as an integer) at
/// `out`, through 0x1A2B80 from `ra`.
fn yaw(fx: &mut Fx, out: u32, ra: u32) -> i32 {
    let h = fx.self_handle();
    fx.unit_rotation(h, out);
    let y = ee::div(ee::from_i32(fx.mem.i32(out + 4)), 180.0);
    fix(fx, y, ra)
}

fn init(fx: &mut Fx, sp: u32, param: i32, v: Variant) -> i32 {
    fx.bzero(D, 0x54B0);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x520E, param);
    fx.mem.set32(TIMER, 0x10A);
    let y = yaw(fx, sp + 0xB0, v.at(0x01E0_3158));
    fx.mem.set16(FACING, y);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x21, 1);
    fx.mem.set16(NODE, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0xD, 0);
    fx.mem.set16(NODE + 2, r);
    for k in 0xDu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x294 + 4 * (k - 0xD), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x268, t);
    for k in 0..3u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x51A0 + 4 * k, t);
    }
    for k in 0..0x100u32 {
        let r = rmod(fx, D, v.at(0x01E0_31F8), 0x14);
        let a = BYTES + k;
        fx.mem.set8(a, r - 0xA);
        let b = fx.mem.read_u8(a) as i8;
        if (0..5).contains(&b) {
            fx.mem.write_u8(a, 5);
        }
        let b = fx.mem.read_u8(a) as i8;
        if (-4..0).contains(&b) {
            fx.mem.set8(a, -5);
        }
    }
    for k in 0x64..0x100u32 {
        fx.mem.write_u8(BYTES + k, 0);
    }
    0
}

/// The steps (b064 addresses, also the tasks' +0x24) and frame sizes; the
/// entry's return address after step i's `jal` is 0x1E032F8 + 0x20 i.
const STEPS: [(u32, u32); 13] = [
    (0x01E0_17E0, 0x30),
    (0x01E0_1A00, 0x30),
    (0x01E0_1C00, 0x30),
    (0x01E0_1D70, 0x30),
    (0x01E0_1E80, 0x20),
    (0x01E0_2020, 0x40),
    (0x01E0_2210, 0x50),
    (0x01E0_2450, 0x40),
    (0x01E0_2670, 0x30),
    (0x01E0_28F0, 0x30),
    (0x01E0_2A90, 0x30),
    (0x01E0_2C40, 0x30),
    (0x01E0_2F10, 0x90),
];

fn step(fx: &mut Fx, sp: u32, param: i32, v: Variant) -> i32 {
    emit(fx, param, v);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        for (i, &(a, size)) in STEPS.iter().enumerate() {
            if fx.mem.read_u32(t + 0x24) == v.at(a) {
                let ra = v.at(0x01E0_32F8 + 0x20 * i as u32);
                let s = fx.enter(size);
                if size == 0x20 {
                    fx.saved(s, &[(0x10, u64::from(ra)), (0, u64::from(D))]);
                } else {
                    fx.saved(s, &[(0x20, u64::from(ra)), (0x10, 0), (0, u64::from(D))]);
                }
                run_step(fx, a, t, s, v);
                fx.leave(size);
            }
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    fx.saved(sp - 0x20, &[(0x10, u64::from(v.at(0x01E0_349C))), (0, u64::from(D))]);
    let z = ee::sub(fx.mem.f(sp + 0x58), 25.0);
    fx.mem.setf(sp + 0x58, z);
    let mut count = 0u32;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        count += 1;
        draw(fx, sp, t, k, count, v);
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0x6E {
        1
    } else if c == 0xF5 {
        3
    } else if (0x79..0xF0).contains(&c) {
        6
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

fn run_step(fx: &mut Fx, a: u32, t: u32, sp: u32, v: Variant) {
    match a {
        0x01E0_17E0 => drift(fx, t, v),
        0x01E0_1A00 => puff(fx, t, v),
        0x01E0_1C00 => smoke(fx, t, v),
        0x01E0_1D70 => arm(fx, t, v),
        0x01E0_1E80 => wisp(fx, t, v),
        0x01E0_2020 => wisp_b(fx, t, sp, v),
        0x01E0_2210 => spark(fx, t, sp, v),
        0x01E0_2450 => ember(fx, t, sp, v),
        0x01E0_2670 => flame(fx, t, v, false),
        0x01E0_28F0 => band(fx, t, v, false),
        0x01E0_2A90 => band(fx, t, v, true),
        0x01E0_2C40 => flame(fx, t, v, true),
        _ => tint(fx, t, sp, v),
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
}

fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
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

fn odd(n: i32) -> bool {
    n % 2 == 1
}

/// i16 `at` += `k` x (sine or cosine of i16 `deg`) through 0x1A2B80.
fn wobble(fx: &mut Fx, t: u32, at: u32, deg: u32, cos: i32, k: f32, ra: u32) {
    let a = fx.mem.i16(t + deg);
    let s = l2::sin_deg(fx, TAB, a, cos);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + at)), ee::mul(k, s));
    let r = fix(fx, f, ra);
    fx.mem.set16(t + at, r);
}

/// The common start of the drift / puff / smoke steps: +0x3A, then the
/// cell / size / +0x30 per step.
fn turn(fx: &mut Fx, t: u32, ra: u32) {
    let r = rmod(fx, t, ra, 0x168);
    fx.mem.set16(t + 0x3A, r);
}

/// 0x1E017E0 (D, task): a violet mote drifting about its point.
fn drift(fx: &mut Fx, t: u32, v: Variant) {
    l2::rgb(fx, t + 0x28, 0x3F, 0x30, 0x3F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        turn(fx, t, v.at(0x01E0_1824));
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, v.at(0x01E0_1844), 0x1E);
        fx.mem.set16(t + 0x30, r - 0xF);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 0xA {
        fade_in(fx, t, 0x2B, 0x60, 9);
    } else {
        fade_out(fx, t, 0x11, 0xA);
    }
    age(fx, t);
    if odd(fx.mem.i16(t + 0x32)) {
        let s = fx.mem.i16(t + 0x38);
        if s < 0x12 {
            fx.mem.set16(t + 0x38, s + 1);
        }
    }
    let n = fx.mem.i16(t + 0x32);
    if fx.div(n, 3).1 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    wobble(fx, t, 8, 0x3A, 0, 10.0, v.at(0x01E0_196C));
    wobble(fx, t, 0xA, 0x30, 1, 4.0, v.at(0x01E0_19A4));
    wobble(fx, t, 0xC, 0x3A, 1, 10.0, v.at(0x01E0_19DC));
}

/// 0x1E01A00 (D, task): a grey puff (cells 0..7 every fifth update).
fn puff(fx: &mut Fx, t: u32, v: Variant) {
    l2::rgb(fx, t + 0x28, 0x3F, 0x3F, 0x3F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0);
        turn(fx, t, v.at(0x01E0_1A44));
        fx.mem.set16(t + 0x34, 0);
        let r = rmod(fx, t, v.at(0x01E0_1A64), 0xB4);
        fx.mem.set16(t + 0x30, r - 0x5A);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 8 {
        fade_in(fx, t, 0x2B, 0xA0, 8);
    } else {
        fade_out(fx, t, 7, 5);
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38);
    if s < 0xD {
        fx.mem.set16(t + 0x38, s + 1);
    }
    let n = fx.mem.i16(t + 0x32);
    if fx.div(n, 5).1 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 7 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    wobble(fx, t, 8, 0x3A, 0, 13.0, v.at(0x01E0_1B68));
    wobble(fx, t, 0xA, 0x30, 1, 13.0, v.at(0x01E0_1BA0));
    wobble(fx, t, 0xC, 0x3A, 1, 13.0, v.at(0x01E0_1BD8));
}

/// 0x1E01C00 (D, task): the column's smoke sprites.
fn smoke(fx: &mut Fx, t: u32, v: Variant) {
    l2::rgb(fx, t + 0x28, 0x2F, 0x2F, 0x2F);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xE);
        turn(fx, t, v.at(0x01E0_1C48));
        let r = rmod(fx, t, v.at(0x01E0_1C64), 7);
        fx.mem.set16(t + 0x34, r);
        let r = rmod(fx, t, v.at(0x01E0_1C80), 0x1E);
        fx.mem.set16(t + 0x30, r - 0xF);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 4 {
        fade_in(fx, t, 0x2B, 0x60, 0x18);
    } else {
        fade_out(fx, t, 0x19, 0x17);
    }
    age(fx, t);
    let s = fx.mem.i16(t + 0x38);
    if s < 0x14 {
        fx.mem.set16(t + 0x38, s + 1);
    }
    wobble(fx, t, 8, 0x3A, 0, 20.0, v.at(0x01E0_1D48));
}

/// 0x1E01D70 (D, task): an arm puff: size 5, the cell 300 sin(+0x3A), x
/// 5 z sin(+0x32) of its arm point's z; +0x3A on by 10 to 180.
fn arm(fx: &mut Fx, t: u32, v: Variant) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 5);
        fx.mem.set16(t + 0x34, 0);
        fx.rand_from(t, v.at(0x01E0_1DA4));
        l2::rgb(fx, t + 0x28, 0x18, 0x18, 0x18);
        fx.mem.set16(t + 0x3A, 0);
        fx.mem.set16(t + 0x36, 2);
        let a = if fx.mem.i32(TIMER) >= 0x1A { 0x40 } else { 0 };
        fx.mem.write_u8(t + 0x2B, a);
    }
    let a = fx.mem.i16(t + 0x3A);
    let s = l2::sin_deg(fx, TAB, a, 0);
    let r = fix(fx, ee::mul(300.0, s), v.at(0x01E0_1E04));
    fx.mem.set16(t + 0x34, r);
    let a = fx.mem.i16(t + 0x32);
    let s = l2::sin_deg(fx, TAB, a, 0);
    let i = fx.mem.i16(t + 0x30);
    let z = fx.mem.f((PTS as i32 + 8 + 0x10 * i) as u32);
    let r = fix(fx, ee::mul(ee::mul(5.0, z), s), v.at(0x01E0_1E3C));
    fx.mem.set16(t + 8, r);
    let a = fx.mem.i16(t + 0x3A) + 0xA;
    fx.mem.set16(t + 0x3A, a);
    if fx.mem.i16(t + 0x3A) >= 0xB4 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// The wisps' fade: while the timer is at least 18, in by 0x13 (under
/// 0x60) until update `until`, then out by 3; after it out by 9.
fn wisp_fade(fx: &mut Fx, t: u32, until: i32) {
    if fx.mem.i32(TIMER) >= 0x12 {
        if fx.mem.i16(t + 0x32) >= until {
            fade_out(fx, t, 5, 3);
        } else {
            fade_in(fx, t, 0x2B, 0x60, 0x13);
        }
    } else {
        fade_out(fx, t, 0xB, 9);
    }
}

/// +0x3A turned by 12 (+0x30 = 0) or -12.
fn spin(fx: &mut Fx, t: u32) {
    let d = if fx.mem.i16(t + 0x30) == 0 { 0xC } else { -0xC };
    let a = fx.mem.i16(t + 0x3A) + d;
    fx.mem.set16(t + 0x3A, a);
}

/// 0x1E01E80 (D, task): a rising blue wisp (alternate ones turning the
/// other way, D+0x520A flipping), swaying by 3 sin(6 age).
fn wisp(fx: &mut Fx, t: u32, v: Variant) {
    if fx.mem.i16(t + 0x36) == 1 {
        for o in [0x10u32, 0x14, 0x18] {
            fx.mem.set32(t + o, 0);
        }
        fx.mem.set16(t + 0x34, 1);
        fx.mem.set16(t + 0x38, 1);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0x80);
        let f = fx.mem.i16(FLIP);
        fx.mem.set16(t + 0x30, f);
        let f = if fx.mem.i16(FLIP) == 0 { 1 } else { 0 };
        fx.mem.set16(FLIP, f);
    }
    age(fx, t);
    wisp_fade(fx, t, 0x33);
    let s = fx.mem.i16(t + 0x38) + 5;
    fx.mem.set16(t + 0x38, s);
    let a = fx.mem.i16(t + 0x32) * 6;
    let s = l2::sin_deg(fx, TAB, a as i16 as i32, 0);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + 2)), ee::mul(3.0, s));
    let r = fix(fx, f, v.at(0x01E0_1FC0));
    fx.mem.set16(t + 2, r);
    spin(fx, t);
    l2::rgb(fx, t + 0x28, 0x20, 0x20, 0x68);
}

/// 0x1E02020 (D, task): a wisp pushed back along the attacker's facing
/// (9 a update), its cell random -10..9.
fn wisp_b(fx: &mut Fx, t: u32, sp: u32, v: Variant) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set32(t + 0x10, 0x4000);
        fx.mem.set32(t + 0x14, 0);
        fx.mem.set32(t + 0x18, 0);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x38, 1);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2B, 0x80);
        let f = fx.mem.i16(FLIP);
        fx.mem.set16(t + 0x30, f);
    }
    let y = yaw(fx, sp + 0x30, v.at(0x01E0_20B0));
    let c = l2::sin_deg(fx, TAB, (y as i16 as i32) - 0xB4, 1);
    let f = ee::add(ee::from_i32(fx.mem.i16(t + 4)), ee::mul(9.0, ee::neg(c)));
    let r = fix(fx, f, v.at(0x01E0_20F0));
    fx.mem.set16(t + 4, r);
    age(fx, t);
    wisp_fade(fx, t, 0x10);
    let r = rmod(fx, t, v.at(0x01E0_2188), 0x14);
    fx.mem.set16(t + 8, r - 0xA);
    let s = fx.mem.i16(t + 0x38) + 2;
    fx.mem.set16(t + 0x38, s);
    spin(fx, t);
    l2::rgb(fx, t + 0x28, 0x18, 0x18, 0x30);
}

/// The task's i16 position = `p` + rand % m - m / 2 per axis (0x1A2B80).
fn scatter(fx: &mut Fx, t: u32, p: u32, ms: [(i32, f32, u32, u32); 3]) {
    for (k, (m, half, rra, fra)) in ms.into_iter().enumerate() {
        let r = rmod(fx, t, rra, m);
        let k = k as u32;
        let f = ee::sub(ee::add(fx.mem.f(p + 4 * k), ee::from_i32(r)), half);
        let r = fix(fx, f, fra);
        fx.mem.set16(t + 2 * k, r);
    }
}

/// Alpha out by 0xC (floor 0xE) once the timer is under 17.
fn late_fade(fx: &mut Fx, t: u32, below: i32) {
    if fx.mem.i32(TIMER) < below {
        fade_out(fx, t, 0xE, 0xC);
    }
}

/// 0x1E02210 (D, task): a white spark about node 0x21 drifting back along
/// the attacker's facing (7 a update).
fn spark(fx: &mut Fx, t: u32, sp: u32, v: Variant) {
    l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    if fx.mem.i16(t + 0x36) == 1 {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE);
        fx.node_position(h, n, sp + 0x30);
        let r = |a: u32| v.at(a);
        scatter(fx, t, sp + 0x30, [(0x3C, 30.0, r(0x01E0_226C), r(0x01E0_229C)), (0x3C, 30.0, r(0x01E0_22A4), r(0x01E0_22D4)), (0x3C, 30.0, r(0x01E0_22DC), r(0x01E0_230C))]);
        fx.mem.set16(t + 0x38, 1);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 3 {
        fade_in(fx, t, 0x2B, 0x70, 0x25);
    }
    if fx.mem.i16(t + 0x32) >= 0x11 {
        fade_out(fx, t, 0xE, 0xC);
    }
    late_fade(fx, t, 0x11);
    let y = yaw(fx, sp + 0x40, v.at(0x01E0_23E4));
    let c = l2::sin_deg(fx, TAB, (y as i16 as i32) - 0xB4, 1);
    let f = ee::sub(ee::from_i32(fx.mem.i16(t + 4)), ee::mul(7.0, c));
    let r = fix(fx, f, v.at(0x01E0_2420));
    fx.mem.set16(t + 4, r);
    age(fx, t);
}

/// 0x1E02450 (D, task): a dark ember about the target (size 14 growing to
/// 20).
fn ember(fx: &mut Fx, t: u32, sp: u32, v: Variant) {
    l2::rgb(fx, t + 0x28, 0x10, 0x10, 0x10);
    if fx.mem.i16(t + 0x36) == 1 {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x30);
        fx.saved(sp - 0x20, &[(0x10, u64::from(v.at(0x01E0_24A0))), (0, u64::from(t))]);
        let r = |a: u32| v.at(a);
        scatter(fx, t, sp + 0x30, [(0x1E, 15.0, r(0x01E0_24A8), r(0x01E0_24D8)), (0x14, 10.0, r(0x01E0_24E0), r(0x01E0_2510)), (0x1E, 15.0, r(0x01E0_2518), r(0x01E0_2548))]);
        fx.mem.set16(t + 0x38, 0xE);
        turn(fx, t, v.at(0x01E0_2558));
        let r = rmod(fx, t, v.at(0x01E0_2574), 7);
        fx.mem.set16(t + 0x34, r);
        let r = rmod(fx, t, v.at(0x01E0_2590), 0x1E);
        fx.mem.set16(t + 0x30, r - 0xF);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < 3 {
        fade_in(fx, t, 0x2B, 0x60, 0x20);
    } else {
        fade_out(fx, t, 0x19, 0x17);
    }
    late_fade(fx, t, 0x11);
    age(fx, t);
    let s = fx.mem.i16(t + 0x38);
    if s < 0x14 {
        fx.mem.set16(t + 0x38, s + 1);
    }
}

/// 0x1E02670 / 0x1E02C40 (`b`) (D, task): a grey flame ring about the
/// target: size 12..33 (+0x30 set) or the target's radius + 2 (to 26),
/// the radius +0x30 kept in +0xA, sliding along x by the facing.
fn flame(fx: &mut Fx, t: u32, v: Variant, b: bool) {
    let r = |a: u32| v.at(a);
    if fx.mem.i16(t + 0x36) == 1 {
        if fx.mem.i16(t + 0x30) != 0 {
            let x = rmod(fx, t, r(if b { 0x01E0_2C78 } else { 0x01E0_26A8 }), 0x16);
            fx.mem.set16(t + 0x38, x + 0xC);
        } else {
            let h = fx.target_handle();
            let rad = crate::blib::afterimage::unit_radius(fx, h);
            let x = fix(fx, ee::add(2.0, rad), r(if b { 0x01E0_2CB8 } else { 0x01E0_26E8 }));
            fx.mem.set16(t + 0x38, x);
            if fx.mem.i16(t + 0x38) >= 0x1B {
                fx.mem.set16(t + 0x38, 0x1A);
            }
        }
        l2::rgb(fx, t + 0x28, 0x40, 0x40, 0x40);
        fx.mem.set16(t + 0x34, 0);
        let a = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, a);
        if b {
            let x = rmod(fx, t, r(0x01E0_2CF8), 0x3C);
            fx.mem.set16(t + 0x30, x + 0xA);
            let x = rmod(fx, t, r(0x01E0_2D18), 5);
            fx.mem.set16(t + 0xE, x + 0xA);
            let x = rmod(fx, t, r(0x01E0_2D38), 0x23);
            fx.mem.set16(t + 8, x);
            let x = fx.rand_from(t, r(0x01E0_2D54));
            fx.mem.set16(t + 6, x % 2 + 1);
        } else {
            let x = rmod(fx, t, r(0x01E0_2728), 0x1E);
            fx.mem.set16(t + 0x30, x + 0x3C);
            let x = rmod(fx, t, r(0x01E0_2748), 5);
            fx.mem.set16(t + 0xE, x + 0xA);
            let x = rmod(fx, t, r(0x01E0_2768), 0x23);
            fx.mem.set16(t + 8, x);
            let x = rmod(fx, t, r(0x01E0_2784), 5);
            fx.mem.set16(t + 6, x + 2);
        }
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x2B, 0x81, 0x40);
    } else {
        fade_out(fx, t, 0xD, 0xB);
    }
    let slide = |fx: &mut Fx| {
        let x = fx.mem.i16(t + 4) + if fx.mem.i16(FACING) != 0 { -1 } else { 1 };
        fx.mem.set16(t + 4, x);
    };
    if b {
        if fx.mem.i16(COUNTER) >= 0xDD {
            fade_out(fx, t, 0xE, 0xC);
        }
        if fx.mem.i16(t + 0x32) != 0 {
            let s = fx.mem.i16(t + 0x38);
            if s < 0x1E {
                fx.mem.set16(t + 0x38, s + 1);
            }
        }
        if odd(fx.mem.i16(t + 0x32)) {
            let x = fx.mem.i16(t + 0x30) + 2;
            fx.mem.set16(t + 0x30, x);
        }
        if odd(fx.mem.i16(t + 0x32)) {
            let c = fx.mem.i16(t + 0x34);
            fx.mem.set16(t + 0x34, if c < 5 { c + 1 } else { 0 });
        }
        let n = fx.mem.i16(t + 0x32);
        if n != 0 && odd(n) {
            slide(fx);
        }
    } else {
        late_fade(fx, t, 0x23);
        if fx.mem.i16(t + 0x32) != 0 {
            let s = fx.mem.i16(t + 0x38) + 1;
            fx.mem.set16(t + 0x38, s);
        }
        if odd(fx.mem.i16(t + 0x32)) {
            let c = fx.mem.i16(t + 0x34);
            if c < 5 {
                fx.mem.set16(t + 0x34, c + 1);
            }
        }
        if fx.mem.i16(t + 0x32) != 0 {
            let x = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
            fx.mem.set16(t + 0x30, x);
        }
        if fx.mem.i16(t + 0x32) != 0 {
            slide(fx);
        }
    }
    age(fx, t);
}

/// 0x1E028F0 / 0x1E02A90 (`b`) (D, task): the burning bands (flat, turned
/// x by 0x4000; 0x2A90's y turn random), flickering between two colours,
/// growing by +8; out by 0x13 once grown, by 0xF when the timer is under
/// 10.
fn band(fx: &mut Fx, t: u32, v: Variant, b: bool) {
    let r = |a: u32| v.at(a);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, if b && v.b355 { 5 } else { 0 });
        fx.mem.set32(t + 0x10, 0x4000);
        if b {
            let x = rmod(fx, t, r(0x01E0_2AC4), 0xFFFF);
            fx.mem.set32(t + 0x14, x);
            let x = rmod(fx, t, r(0x01E0_2AE0), 7);
            fx.mem.set16(t + 0x34, x);
            fx.mem.set16(t + 0x36, 2);
            fx.mem.set16(t + 8, 1);
            fx.mem.set16(t + 6, -3);
            l2::rgb(fx, t + 0x2C, 8, 8, 0x80);
        } else {
            fx.mem.set32(t + 0x14, 0);
            let x = rmod(fx, t, r(0x01E0_2928), 7);
            fx.mem.set16(t + 0x34, x);
            fx.mem.set16(t + 0x36, 2);
            let x = rmod(fx, t, r(0x01E0_294C), 3);
            fx.mem.set16(t + 8, x + 4);
            fx.mem.set16(t + 6, 6);
            l2::rgb(fx, t + 0x2C, 0x80, 0x80, 0x80);
        }
    }
    if fx.mem.i16(t + 0x32) < 2 {
        if b && v.b355 {
            fade_in(fx, t, 0x2B, 0x50, 0x28);
        } else {
            fade_in(fx, t, 0x2B, 0x40, 0x20);
        }
    } else {
        let grown = if !b { 0x29 } else if v.b355 { 0x1F } else { 0x15 };
        if fx.mem.i16(t + 0x38) >= grown {
            fade_out(fx, t, 0x15, 0x13);
        }
        if fx.mem.i32(TIMER) < 0xA {
            fade_out(fx, t, 0x11, 0xF);
        }
    }
    // (odd, even) colours.
    let (o, e) = match (b, v.b355) {
        (false, false) => ((8, 0xF0), (0x10, 0x80)),
        (false, true) => ((0x30, 0xF0), (0x20, 0xC0)),
        (true, false) => ((8, 0x50), (0x10, 0x30)),
        (true, true) => ((0x10, 0x80), (0x20, 0x50)),
    };
    let (g, bl) = if odd(fx.mem.i16(t + 0x32)) { o } else { e };
    l2::rgb(fx, t + 0x28, g, g, bl);
    if b {
        if fx.mem.i16(t + 0x32) != 0 {
            let s = fx.mem.i16(t + 0x38) + fx.mem.i16(t + 8);
            fx.mem.set16(t + 0x38, s);
        }
        age(fx, t);
    } else {
        age(fx, t);
        let s = fx.mem.i16(t + 0x38) + fx.mem.i16(t + 8);
        fx.mem.set16(t + 0x38, s);
    }
}

/// 0x1E02F10 (D, task): the frame tint (the library frame quad, its
/// descriptor at sp + 0x38: (0x80, 0x80, 0x80; b355 0xA0) and alpha +0x2B,
/// ST 0..1 (b355 1/256 .. 0x3F7F0000)); in by 0xC (under 0x60) for 8
/// updates, out by 0xC once the timer is under 17; ends on timer 9.
fn tint(fx: &mut Fx, t: u32, sp: u32, v: Variant) {
    let d = sp + 0x38;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let blue = if v.b355 { 0xA0 } else { 0x80 };
    for (k, c) in [0x80u8, 0x80, blue].into_iter().enumerate() {
        fx.mem.write_u8(d + 0x10 + k as u32, c);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    fx.mem.write_u8(d + 0x13, a);
    fx.mem.write_f32(d + 0x14, 1.0);
    let (s0, s1) = if v.b355 { (0x3B80_0000u32, 0x3F7F_0000u32) } else { (0, 0x3F80_0000) };
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let w = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (w & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    fx.mem.write_u32(d + 0x18, s0);
    fx.mem.write_u32(d + 0x1C, s0);
    fx.mem.write_u32(d + 0x28, s1);
    fx.mem.write_u32(d + 0x2C, s1);
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [0x226, 0x26]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(t + 0x32) < 8 {
        fade_in(fx, t, 0x2B, 0x60, 0xC);
    }
    late_fade(fx, t, 0x11);
    age(fx, t);
    if fx.mem.i32(TIMER) == 9 {
        fx.mem.set16(t + 0x36, 0);
    }
}

/// 0x1E03A10(D, step, a2): a new task (first free of 300) with the spawn
/// colour D+0x51C4 (both colours), at D+0x250, index D+0x520C, age
/// D+0x5212.
fn spawn(fx: &mut Fx, step: u32) {
    fx.enter(0x60);
    let mut task = None;
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            fx.bzero(t, 0x40);
            fx.mem.set16(t + 0x36, 1);
            task = Some(t);
            break;
        }
    }
    if let Some(t) = task {
        for at in [0x28u32, 0x2C] {
            let c: [i32; 3] = std::array::from_fn(|k| i32::from(fx.mem.read_u8(COLOUR + k as u32)));
            l2::rgb(fx, t + at, c[0], c[1], c[2]);
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + at + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        let i = fx.mem.i16(INDEX);
        fx.mem.set16(t + 0x30, i);
        let a = fx.mem.i16(AGE);
        fx.mem.set16(t + 0x32, a);
        fx.mem.set32(t + 0x20, 0);
        fx.mem.write_u32(t + 0x24, step);
    }
    fx.leave(0x60);
}

/// D+0x250 = the column's first point (+ `dx` in x) about sp + 0x50,
/// 10 up.
fn column_at(fx: &mut Fx, sp: u32, dx: Option<f32>) {
    let mut x = ee::add(fx.mem.f(sp + 0x50), fx.mem.f(PTS));
    if let Some(dx) = dx {
        x = ee::add(x, dx);
    }
    fx.mem.setf(POS, x);
    let y = ee::add(10.0, fx.mem.f(PTS + 4));
    fx.mem.setf(POS + 4, y);
    let z = ee::add(fx.mem.f(sp + 0x58), fx.mem.f(PTS + 8));
    fx.mem.setf(POS + 8, z);
}

/// D+0x258 moved 5 towards the attacker's facing.
fn facing_z(fx: &mut Fx) {
    let z = fx.mem.f(POS + 8);
    let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 5.0) } else { ee::sub(z, 5.0) };
    fx.mem.setf(POS + 8, z);
}

/// 0x1E00F30(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x70).
fn emit(fx: &mut Fx, param: i32, v: Variant) {
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x40, u64::from(v.at(0x01E0_32C4))), (0x30, 0), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    let mut s0 = D;
    l2::rgb(fx, COLOUR, 0x80, 0, 0);
    fx.mem.write_u8(COLOUR + 3, 0);
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE);
    fx.node_position(h, n, sp + 0x50);
    let c = fx.mem.i16(COUNTER);
    if (0x35..0xE6).contains(&c) && fx.div(c, 6).1 == 1 {
        s0 = 0;
        let r = rmod(fx, s0, 0x01E0_0FCC, 0x168);
        fx.mem.set16(ANGLE, r);
        copy3_pos(fx, sp + 0x50);
        if !v.b355 {
            spawn(fx, 0x01E0_2020);
        }
        let y = ee::sub(fx.mem.f(sp + 0x54), 30.0);
        fx.mem.setf(POS + 4, y);
        if fx.mem.i16(COUNTER) >= 0x83 {
            spawn(fx, v.at(0x01E0_1E80));
        }
        s0 = 1;
    }
    let c = fx.mem.i16(COUNTER);
    if (0x2B..0xEB).contains(&c) && fx.div(c, 3).1 == 1 && c >= 0x83 {
        spawn(fx, v.at(0x01E0_2210));
        spawn(fx, v.at(0x01E0_2450));
    }
    if fx.mem.i16(COUNTER) == 1 {
        fx.mem.set16(ARMS, 0);
        for k in 0..20u32 {
            fx.mem.set16(ARMS + 2 + 2 * k, [0, 0x14, -0x14][k as usize % 3]);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if c >= 0x21 && fx.div(c, 0xE).1 == 1 {
        let n = fx.mem.i16(ARMS);
        if n < 0xF {
            fx.mem.set16(ARMS, n + 3);
        }
    }
    if odd(fx.mem.i16(COUNTER)) {
        let mut k = 0i32;
        s0 = D;
        while k < fx.mem.i16(ARMS) {
            let a = fx.mem.i16(ARMS + 2 + 2 * k as u32);
            fx.mem.set16(AGE, a);
            let p = PTS + 0x10 * k as u32;
            let x = ee::add(fx.mem.f(sp + 0x50), fx.mem.f(p));
            fx.mem.setf(POS, x);
            let y = ee::add(10.0, fx.mem.f(PTS + 4));
            fx.mem.setf(POS + 4, y);
            let z = ee::add(fx.mem.f(sp + 0x58), fx.mem.f(p + 8));
            fx.mem.setf(POS + 8, z);
            fx.mem.set16(INDEX, 0);
            if !crate::exe::ee_le(fx.mem.f(p + 8), -300.0) {
                spawn(fx, v.at(0x01E0_1D70));
            }
            let y = yaw(fx, sp + 0x60, v.em(0x01E0_126C));
            let c = l2::sin_deg(fx, TAB, (y as i16 as i32) - 0xB4, 1);
            let d = (f64::from(c) * f64::from_bits(0x4005_9999_9999_999A)) as f32;
            let z = ee::sub(fx.mem.f(p + 8), d);
            fx.mem.setf(p + 8, z);
            k = (k + 1) as i16 as i32;
            s0 = s0.wrapping_add(2);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if c >= 0x21 && c % 4 == 1 && fx.mem.i32(TIMER) >= 0x1F {
        column_at(fx, sp, None);
        spawn(fx, v.at(0x01E0_1C00));
        for deg in [0x14, -0x14] {
            let s = l2::sin_deg(fx, TAB, deg, 0);
            let off = ee::div(ee::mul(ee::mul(5.0, fx.mem.f(PTS + 8)), s), 10.0);
            column_at(fx, sp, Some(off));
            spawn(fx, v.at(0x01E0_1C00));
        }
        s0 = 1;
    }
    if fx.mem.i16(COUNTER) == 0x32 {
        spawn(fx, v.at(0x01E0_2F10));
    }
    let target = |fx: &mut Fx, ra: u32, s0: u32| {
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x50);
        fx.saved(sp - 0x20, &[(0x10, u64::from(ra)), (0, u64::from(s0))]);
    };
    let c = fx.mem.i16(COUNTER);
    if (0x6E..0xE2).contains(&c) {
        target(fx, v.em(0x01E0_1524), s0);
        let x = fx.mem.read_u32(sp + 0x50);
        fx.mem.write_u32(POS, x);
        fx.mem.write_u32(POS + 4, 0x4220_0000);
        let z = fx.mem.read_u32(sp + 0x58);
        fx.mem.write_u32(POS + 8, z);
        facing_z(fx);
        fx.mem.set16(INDEX, 0);
        spawn(fx, v.at(0x01E0_2670));
    }
    let c = fx.mem.i16(COUNTER);
    if (0x6E..0xDD).contains(&c) {
        target(fx, v.em(0x01E0_15C4), s0);
        let r = fx.rand_from(s0, v.em(0x01E0_15CC)) % 4;
        let x = ee::sub(ee::add(fx.mem.f(sp + 0x50), ee::from_i32(r)), 2.0);
        fx.mem.setf(POS, x);
        fx.mem.write_u32(POS + 4, 0x4228_0000);
        let z = fx.mem.read_u32(sp + 0x58);
        fx.mem.write_u32(POS + 8, z);
        facing_z(fx);
        fx.mem.set16(INDEX, 0);
        spawn(fx, v.at(0x01E0_2C40));
    }
    let c = fx.mem.i16(COUNTER);
    if (0x78..0xE7).contains(&c) {
        target(fx, v.em(0x01E0_169C), s0);
        copy3_pos(fx, sp + 0x50);
        fx.mem.set16(INDEX, 0);
        spawn(fx, v.at(0x01E0_28F0));
    }
    let c = fx.mem.i16(COUNTER);
    if (0x37..0x7E).contains(&c) {
        if c == 0x37 {
            let h = fx.self_handle();
            let n = fx.mem.i16(NODE);
            fx.node_position(h, n, HOLD);
        }
        let r = rmod(fx, s0, v.em(0x01E0_1714), 6);
        fx.mem.setf(sp + 0x50, ee::from_i32(r - 3));
        let r = rmod(fx, s0, v.em(0x01E0_1740), 6);
        fx.mem.setf(sp + 0x54, ee::from_i32(r - 3));
        for o in [0x50u32, 0x54] {
            let x = ee::div(fx.mem.f(sp + o), 10.0);
            fx.mem.setf(sp + o, x);
        }
        let x = ee::add(fx.mem.f(HOLD), fx.mem.f(sp + 0x50));
        fx.mem.setf(POS, x);
        let y = ee::add(fx.mem.f(HOLD + 4), fx.mem.f(sp + 0x54));
        fx.mem.setf(POS + 4, y);
        let z = fx.mem.read_u32(HOLD + 8);
        fx.mem.write_u32(POS + 8, z);
        fx.mem.set16(INDEX, 0);
        spawn(fx, v.at(0x01E0_2A90));
    }
    fx.leave(0x70);
}

fn copy3_pos(fx: &mut Fx, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(POS + 4 * k, w);
    }
}

/// The atlas rectangle of sprite `id` frame `sub` (into the entry's local
/// `local`) as D+0x94.
fn rect_d(fx: &mut Fx, local: u32, id: i32, sub: i32) {
    l2::rect(fx, local, id, sub);
    copy4(fx, D + 0x94, local);
}

fn centre_task(fx: &mut Fx, t: u32) {
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(D + 0x20 + 4 * k, v);
    }
}

/// The entry's draw of one live task (index `k`, `count` the live tasks
/// so far: the entry's s3 / s1).
fn draw(fx: &mut Fx, sp: u32, t: u32, k: u32, count: u32, v: Variant) {
    let s = fx.mem.read_u32(t + 0x24);
    let d = sp + 0xC0;
    let saves = |fx: &mut Fx, at: u32, ra: u32, size: u32| {
        let f = fx.sp - size;
        let (r, s3) = match size {
            0x110 => (0x50, 0x30),
            _ if at == 0x3F10 => (0x70, 0x40),
            _ => (0x60, 0x30),
        };
        fx.saved(f, &[(r, u64::from(ra)), (s3, u64::from(k)), (s3 - 0x10, u64::from(D + 0x40 * k)), (s3 - 0x20, u64::from(count)), (s3 - 0x30, u64::from(D))]);
    };
    // (step, draw, the entry's ra after the first `jal`).
    let pair = |fx: &mut Fx, f: fn(&mut Fx, u32, u32, i32, Variant), at: u32, ra: u32| {
        let size = if at == 0x3F10 || at == 0x4290 { 0x120 } else { 0x110 };
        saves(fx, at, v.at(ra), size);
        f(fx, t, d, 0, v);
        saves(fx, at, v.at(ra + 0x14), size);
        f(fx, t, d, 1, v);
    };
    let spec = |fx: &mut Fx| {
        let w = fx.mem.read_u32(D + 0x268);
        fx.mem.write_u32(sp + 0x60, w);
        fx.mem.set32(sp + 0x68, 0);
        fx.mem.set32(sp + 0x64, 0x14);
        fx.mem.set32(sp + 0x6C, 0);
        let ot = fx.mem.read_u32(OTW);
        quads4::sprite_spec(fx, ot, t, sp + 0x60);
    };
    let ring = |fx: &mut Fx, local: u32, n: i32, ra: u32| {
        let w = fx.mem.read_u32(D + 0x268);
        fx.mem.write_u32(D + 0x88, w);
        let c = fx.mem.i16(t + 0x34);
        rect_d(fx, sp + local, 0x14, c);
        fx.mem.set16(D + 0xB8, n);
        let r = fx.rand_from(D, v.at(ra));
        fx.mem.set16(D + 0xBE, r % 4);
        centre_task(fx, t);
        quads4::ring_at_m(fx, uv::uv_rect4, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    };
    let band = |fx: &mut Fx, local: u32| {
        let w = fx.mem.read_u32(D + 0x29C);
        fx.mem.write_u32(D + 0x88, w);
        let c = fx.mem.i16(t + 0x34);
        rect_d(fx, sp + local, 0x97, c);
        fx.mem.set16(D + 0xB8, 1);
        quads4::band18_tex(fx, D, t, d);
        fx.mem.set16(D + 0xB8, 0);
    };
    let is = |a: u32| s == v.at(a);
    if is(0x01E0_17E0) {
        pair(fx, sprite_a, 0x3D70, 0x01E0_34EC);
    }
    if is(0x01E0_1A00) {
        pair(fx, sprite_b, 0x3B40, 0x01E0_3528);
    }
    if is(0x01E0_1C00) {
        spec(fx);
    }
    if is(0x01E0_1D70) {
        pair(fx, sprite_b, 0x3B40, 0x01E0_35A8);
    }
    if is(0x01E0_1E80) {
        pair(fx, ring_wisp, 0x3F10, 0x01E0_35E8);
    }
    if is(0x01E0_2020) {
        pair(fx, cone, 0x4290, 0x01E0_3628);
        pair(fx, ring_wisp, 0x3F10, 0x01E0_3650);
    }
    if is(0x01E0_2210) {
        pair(fx, sprite_w, 0x4640, 0x01E0_3690);
    }
    if is(0x01E0_2450) {
        spec(fx);
    }
    if is(0x01E0_2670) {
        ring(fx, 0x70, 3, 0x01E0_3748);
    }
    if is(0x01E0_28F0) {
        band(fx, 0x80);
    }
    if is(0x01E0_2A90) {
        band(fx, 0x90);
    }
    if is(0x01E0_2C40) {
        ring(fx, 0xA0, 2, 0x01E0_3900);
    }
}

/// The trans vector at `out`: the task's i16 position + i16 +8 / +0xA /
/// +0xC over 10 (integer division by the magic multiply).
fn trans_tenth(fx: &mut Fx, t: u32, out: u32) {
    for k in 0..3u32 {
        let off = fx.mem.i16(t + 8 + 2 * k);
        fx.mult(0x6666_6667, off);
        let hi = fx.hilo().1;
        let q = (hi >> 2).wrapping_add(((off as u32) >> 31) as i32);
        let v = fx.mem.i16(t + 2 * k).wrapping_add(q);
        fx.mem.setf(out + 4 * k, ee::from_i32(v));
    }
}

/// i16 `v` / 10 by the magic multiply.
fn tenth(fx: &mut Fx, v: i32) -> i32 {
    fx.mult(0x6666_6667, v);
    let hi = fx.hilo().1;
    (hi >> 2).wrapping_add(((v as u32) >> 31) as i32)
}

/// Mode 0 of the sprites: rect `id` (cell `sub`) into the local `r`, the
/// strip3 header, colour `c`, UVs, texture word `tex`.
fn setup(fx: &mut Fx, d: u32, r: u32, c: u32, tex: u32) {
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, c, 0);
    l2::uv_strip3(fx, d, r, 0);
    let w = fx.mem.read_u32(tex);
    fx.desc_texture(d, w, 1);
}

/// 0x1E03D70(D, task, desc, mode): a camera-facing quad of half size i16
/// +0x38 at the task's position (+ offsets / 10), UVs of sprite 0x14 cell
/// +0x34, texture D+0x268, keyed by the mean w (0x109870).
fn sprite_a(fx: &mut Fx, t: u32, d: u32, mode: i32, _v: Variant) {
    let sp = fx.enter(0x110);
    let ot = OT;
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xB0, 0x14, c);
        setup(fx, d, sp + 0xB0, t + 0x28, D + 0x268);
    } else {
        l2::camera_angles_to(fx, sp + 0x100);
        trans_tenth(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, sp + 0x60, s, s);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, ot);
    }
    fx.leave(0x110);
}

/// 0x1065E0(out, 2, 0): the second atlas's rectangle 2 frame 0 (0x8F, 0,
/// 0x1F, 0x40) through its 0x10 local (the only case these records use)
/// [F].
fn rect2(fx: &mut Fx, out: u32) {
    let sp = fx.sp - 0x10;
    for (k, x) in [0x8Fi32, 0, 0x1F, 0x40].into_iter().enumerate() {
        fx.mem.set16(sp + 2 * k as u32, x);
    }
    copy4(fx, out, sp);
}

/// 0x1E03B40(D, task, desc, mode): a quad of width i16 +0x38 and height
/// +0x34 / 10 (its lower vertices at y 0), turned x by 0x3A00 (+0x30 = 0)
/// or 0x4000, UVs of the second atlas's rectangle 2, texture D+0x29C.
fn sprite_b(fx: &mut Fx, t: u32, d: u32, mode: i32, _v: Variant) {
    let sp = fx.enter(0x110);
    let ot = OT;
    if mode == 0 {
        l2::rect(fx, sp + 0xB0, 0x99, 0);
        rect2(fx, sp + 0xB0);
        setup(fx, d, sp + 0xB0, t + 0x28, D + 0x29C);
    } else {
        l2::camera_angles_to(fx, sp + 0x100);
        trans_tenth(fx, t, sp + 0xA0);
        let z = if fx.mem.i16(t + 0x30) == 0 { 1 } else { 0 };
        let x = if fx.div(z, 3).1 != 0 { 0x3A00 } else { 0x4000 };
        fx.mem.set32(sp + 0x104, x);
        fx.mem.set32(sp + 0x100, 0);
        fx.mem.set32(sp + 0x108, 0);
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let w = ee::from_i32(fx.mem.i16(t + 0x38));
        let x = fx.mem.i16(t + 0x34);
        let h = tenth(fx, x);
        l2::quad_xy(fx, sp + 0x60, w, ee::from_i32(h));
        fx.mem.set32(sp + 0x84, 0);
        fx.mem.set32(sp + 0x94, 0);
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, ot);
    }
    fx.leave(0x110);
}

/// 0x1E04640(D, task, desc, mode): a camera-facing quad of half size (10
/// x, x = i16 +0x38) turned y by 0x4000, UVs of sprite 0x14 cell +0x34,
/// texture D+0x268.
fn sprite_w(fx: &mut Fx, t: u32, d: u32, mode: i32, _v: Variant) {
    let sp = fx.enter(0x110);
    let ot = OT;
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0xB0, 0x14, c);
        setup(fx, d, sp + 0xB0, t + 0x28, D + 0x268);
    } else {
        l2::camera_angles_to(fx, sp + 0x100);
        fx.mem.set32(sp + 0x104, 0x4000);
        trans_tenth(fx, t, sp + 0xA0);
        l2::xform_vp(fx, sp + 0xC0, 0, sp + 0x100, sp + 0xA0);
        let r = fx.mem.i16(t + 0x38);
        l2::quad_xy(fx, sp + 0x60, ee::from_i32(10 * r), ee::from_i32(r));
        l2::project_submit(fx, sp + 0x60, d, sp + 0xC0, ot);
    }
    fx.leave(0x110);
}

/// 0x1E03F10(D, task, desc, mode): a ring of 18 quads between radius i16
/// +0x38 and that minus 50 (step 0x1E80) / 10 (0x2020) / 0, turned by the
/// task's angles +0x10, UVs of sprite 0x96 cell +0x34, texture D+0x29C;
/// each keyed by its mean w (D+0x51EC) on D+0x51CC.
fn ring_wisp(fx: &mut Fx, t: u32, d: u32, mode: i32, v: Variant) {
    let sp = fx.enter(0x120);
    if mode == 0 {
        let c = fx.mem.i16(t + 0x34);
        l2::rect(fx, sp + 0x110, 0x96, c);
        setup(fx, d, sp + 0x110, t + 0x28, D + 0x29C);
    } else {
        trans_tenth(fx, t, sp + 0xC0);
        l2::xform_vp(fx, sp + 0xD0, 0, t + 0x10, sp + 0xC0);
        for o in [0x84u32, 0x94, 0xA4, 0xB4] {
            fx.mem.set32(sp + o, 0);
        }
        let s = fx.mem.read_u32(t + 0x24);
        let mut w = 0.0f32;
        if s == v.at(0x01E0_1E80) {
            w = 50.0;
        }
        if s == v.at(0x01E0_2020) {
            w = 10.0;
        }
        let mut a = 0i32;
        for _ in 0..0x12 {
            for (deg, inner, outer) in [((fx.mem.i16(t + 0x3A) + a) as i16 as i32, 0xA0u32, 0x80u32), ((fx.mem.i16(t + 0x3A) + 0x14 + a) as i16 as i32, 0xB0, 0x90)] {
                for (o, narrow) in [(inner, false), (outer, true)] {
                    for (cos, off) in [(1, 0u32), (0, 8)] {
                        let c = l2::sin_deg(fx, TAB, deg, cos);
                        let mut r = ee::from_i32(fx.mem.i16(t + 0x38));
                        if narrow {
                            r = ee::sub(r, w);
                        }
                        fx.mem.setf(sp + o + off, ee::mul(r, c));
                    }
                }
            }
            if l2::project(fx, sp + 0x80, SPR, sp + 0xD0, d, MEAN) {
                let key = ee::fixunssfsi(fx.mem.f(MEAN)) as i32;
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
            a = (a + 0x14) as i16 as i32;
        }
    }
    fx.leave(0x120);
}

/// 0x1E04290(D, task, desc, mode): a cone of 9 quads (40 degrees each)
/// between radius i16 +0x38 - 5 (y 0) and +0x38 - 12 - 5 (y 40), turned by
/// the task's angles +0x10, coloured the record's constant 0x1E04800 with
/// alpha +0x2B, UVs of sprite 0x9B cell 3, texture D+0x29C; keyed by the
/// mean w (D+0x51EC) on D+0x51CC.
fn cone(fx: &mut Fx, t: u32, d: u32, mode: i32, _v: Variant) {
    let sp = fx.enter(0x120);
    let k = fx.mem.read_u32(0x01E0_4800);
    fx.mem.write_u32(sp + 0x11C, k);
    if mode == 0 {
        l2::rect(fx, sp + 0x100, 0x9B, 3);
        l2::desc_strip3(fx, d);
        let a = fx.mem.read_u8(t + 0x2B);
        fx.mem.write_u8(sp + 0x11F, a);
        l2::colour_strip3(fx, d, sp + 0x11C, 0);
        l2::uv_strip3(fx, d, sp + 0x100, 0);
        let w = fx.mem.read_u32(D + 0x29C);
        fx.desc_texture(d, w, 1);
    } else {
        trans_tenth(fx, t, sp + 0xB0);
        l2::xform_vp(fx, sp + 0xC0, 0, t + 0x10, sp + 0xB0);
        fx.mem.write_u32(sp + 0x74, 0x4220_0000);
        fx.mem.write_u32(sp + 0x84, 0x4220_0000);
        fx.mem.set32(sp + 0x94, 0);
        fx.mem.set32(sp + 0xA4, 0);
        let mut a = 0i32;
        for _ in 0..9 {
            for (deg, inner, outer) in [((fx.mem.i16(t + 0x3A) + a) as i16 as i32, 0x90u32, 0x70u32), ((fx.mem.i16(t + 0x3A) + 0x28 + a) as i16 as i32, 0xA0, 0x80)] {
                for (o, top) in [(inner, false), (outer, true)] {
                    for (cos, off) in [(1, 0u32), (0, 8)] {
                        let c = l2::sin_deg(fx, TAB, deg, cos);
                        let r = fx.mem.i16(t + 0x38);
                        let r = if top { ee::sub(ee::sub(ee::from_i32(r), 12.0), 5.0) } else { ee::from_i32(r - 5) };
                        fx.mem.setf(sp + o + off, ee::mul(r, c));
                    }
                }
            }
            if l2::project(fx, sp + 0x70, SPR, sp + 0xC0, d, MEAN) {
                let key = ee::fixunssfsi(fx.mem.f(MEAN)) as i32;
                let ot = fx.mem.read_u32(OTW);
                fx.submit(d, ot, key);
            }
            a = (a + 0x28) as i16 as i32;
        }
    }
    fx.leave(0x120);
}
