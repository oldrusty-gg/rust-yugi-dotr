//! BMODULE b423 (card 31), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02BA0, data area 0x1ED0000) [F]: the target spun round (its object +0x20 turned 0x1000 a update
//! from its own, 0x1F7410 / 0x1F72D0) while 256 animated strips
//! (`blib::spin::strips`, +0xE0 / particles +0x160) rise round the
//! attacker's position (+0x2160, following it to the target's point from
//! update 0x6F over +0x3920 updates), with 60 spinning motes (+0x21D0,
//! `blib::spin::motes`) and from 0x6E 256 sprites (+0x2890) rising round
//! the moving point; the target's object op 0x1F83B0(2) and a frame quad
//! from 0x5C. Returns 1 on update 0xAC, 6 on 0xB6..0x15D, 2 on 0x15E, 8
//! when the 0x17B updates are over.
//!
//! Library (record addresses): `colours4` 0x1E00080, the frame quad
//! 0x1E00120, `motes` 0x1E00240, `sprites_spawn` 0x1E009C0, `sprites`
//! 0x1E00B50, `strips` 0x1E02440.
use crate::blib::spin;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const TURN: u32 = D + 0x3900;
const STRIPS: u32 = D + 0xE0;
const PARTS: u32 = D + 0x160;
const MOTES: u32 = D + 0x21D0;
const SPRITES: u32 = D + 0x2890;
const X: u32 = D + 0x2160;

/// 0x1F7410(out): `out` cleared, then the target unit's object +0x20..
/// (0x1F35C0) copied into it; 1 when the object exists [F].
pub(crate) fn target_turn(fx: &mut Fx, out: u32) -> i32 {
    for k in [0xCu32, 8, 4, 0] {
        fx.mem.write_u32(out + k, 0);
    }
    match target_object(fx) {
        Some(o) => {
            for k in 0..4 {
                let w = fx.mem.read_u32(o + 0x20 + 4 * k);
                fx.mem.write_u32(out + 4 * k, w);
            }
            1
        }
        None => 0,
    }
}

/// 0x1F72D0(v): the target unit's object +0x20.. = `v` (0x1F36D0) [F].
pub(crate) fn set_target_turn(fx: &mut Fx, v: u32) -> i32 {
    match target_object(fx) {
        Some(o) => {
            for k in 0..4 {
                let w = fx.mem.read_u32(v + 4 * k);
                fx.mem.write_u32(o + 0x20 + 4 * k, w);
            }
            1
        }
        None => 0,
    }
}

fn target_object(fx: &mut Fx) -> Option<u32> {
    let h = fx.target_handle();
    let (side, slot) = (h >> 8, h & 0xFF);
    if !(0..3).contains(&side) || !(0..0xB).contains(&slot) {
        return None;
    }
    let i = fx.object_index(side, slot);
    if i < 0 {
        return None;
    }
    let o = fx.object_at(i);
    (o != 0).then_some(o)
}

/// What differs between the records of this program.
struct V {
    /// bzero size.
    size: u32,
    /// The two node requests' words, the counters (updates left, update,
    /// the parameter, the follow countdown).
    nodes: (u32, u32),
    left: u32,
    n: u32,
    param: u32,
    follow: u32,
    /// The target spun round (b423), the turn's cap word.
    turn: bool,
    cap: u32,
    /// The frame quad on these updates; or (b551) a tint (b262's, colour
    /// +0x3910, alpha up 4 to 0x40 before 0x15E, then down).
    quad: (i32, i32),
    tint: bool,
    /// The strips' address and the update their alpha starts rising.
    strips: u32,
    rise: i32,
    /// The motes' cell size (w, h) and the strips' / sprites' green.
    mote_cell: (i32, i32),
    green: u8,
    /// Return addresses: rand x 11 (init), the three transforms, rand x 5
    /// (update).
    ra: [u32; 19],
}

const B423: V = V {
    size: 0x3930,
    nodes: (0x3924, 0x3928),
    left: 0x3914,
    n: 0x3918,
    param: 0x391C,
    follow: 0x3920,
    turn: true,
    cap: 0x3910,
    quad: (0x5C, i32::MAX),
    tint: false,
    strips: 0x01E0_2440,
    rise: 0x5A,
    mote_cell: (0x80, 0x40),
    green: 0x40,
    ra: [
        0x01E0_2CDC, 0x01E0_2CF8, 0x01E0_2D5C, 0x01E0_2D78, 0x01E0_2D94, 0x01E0_2EC4, 0x01E0_2EE0, 0x01E0_2F10, 0x01E0_2F2C, 0x01E0_3038, 0x01E0_3060, 0x01E0_311C, 0x01E0_3124, 0x01E0_312C, 0x01E0_32C8,
        0x01E0_32F8, 0x01E0_3410, 0x01E0_34F8, 0x01E0_3600,
    ],
};

/// b273 (card 656) [F, entry 0x1E02BA0]: b423 without the target's turn
/// or object op, the counters at +0x3900.., the frame quad on 0x28..0x15D,
/// tall mote cells and greener strips / sprites (0x60).
const B273: V = V {
    size: 0x3920,
    nodes: (0x3910, 0x3914),
    left: 0x3900,
    n: 0x3904,
    param: 0x3908,
    follow: 0x390C,
    turn: false,
    cap: 0x3910,
    quad: (0x28, 0x15E),
    tint: false,
    strips: 0x01E0_2440,
    rise: 0x5A,
    mote_cell: (0x40, 0x80),
    green: 0x60,
    ra: [
        0x01E0_2CB8, 0x01E0_2CD4, 0x01E0_2D38, 0x01E0_2D54, 0x01E0_2D70, 0x01E0_2EA0, 0x01E0_2EBC, 0x01E0_2EEC, 0x01E0_2F08, 0x01E0_301C, 0x01E0_3044, 0x01E0_3104, 0x01E0_310C, 0x01E0_3114, 0x01E0_3218,
        0x01E0_3248, 0x01E0_3360, 0x01E0_3448, 0x01E0_3550,
    ],
};

/// b551 (card 286) [F, entry 0x1E02BE0]: b423 with the counters one word
/// up (+0x3918..), a reddish tint (+0x3910) in place of the frame quad,
/// tall mote cells, the strips (at 0x1E02480) brightening from 0x46.
const B551: V = V {
    size: 0x3930,
    nodes: (0x3928, 0x392C),
    left: 0x3918,
    n: 0x391C,
    param: 0x3920,
    follow: 0x3924,
    turn: true,
    cap: 0x3914,
    quad: (0, 0),
    tint: true,
    strips: 0x01E0_2480,
    rise: 0x46,
    mote_cell: (0x40, 0x80),
    green: 0x40,
    ra: [
        0x01E0_2D38, 0x01E0_2D54, 0x01E0_2DB8, 0x01E0_2DD4, 0x01E0_2DF0, 0x01E0_2F20, 0x01E0_2F3C, 0x01E0_2F6C, 0x01E0_2F88, 0x01E0_3098, 0x01E0_30C0, 0x01E0_317C, 0x01E0_3184, 0x01E0_318C, 0x01E0_3358,
        0x01E0_3388, 0x01E0_34A0, 0x01E0_3588, 0x01E0_3690,
    ],
};

/// rand & 0xFFFF (an angle) with the entry's s0 (D) from `ra`.
fn angle(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(D, ra) & 0xFFFF
}

/// 0.1 x (rand % m) with the entry's s0 (D) from `ra`.
fn tenth(fx: &mut Fx, ra: u32, m: i32) -> f32 {
    let r = fx.rand_from(D, ra);
    let (_, h) = fx.div(r, m);
    ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h))
}

/// The entry 0x1E02BA0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B423)
}

/// b273's entry 0x1E02BA0.
pub fn program_b273(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B273)
}

/// b551's entry 0x1E02BE0.
pub fn program_b551(fx: &mut Fx, param: i32) -> i32 {
    run(fx, param, &B551)
}

fn run(fx: &mut Fx, param: i32, v: &V) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param < 0 { step(fx, sp, param, v) } else { init(fx, sp, param, v) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32, v: &V) -> i32 {
    fx.bzero(D, v.size);
    for x in [0u32, 0x70, 0x2160] {
        fx.xform_init(D + x);
    }
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x12, 1);
    fx.mem.set32(D + v.nodes.0, r);
    let h = fx.self_handle();
    let r = fx.request_node(h, 1, 0);
    fx.mem.set32(D + v.nodes.1, r);
    fx.mem.set32(D + v.left, 0x17B);
    fx.mem.set32(D + v.n, 1);
    fx.mem.set32(D + v.param, param);
    fx.mem.set32(D + v.follow, 0x48);
    if v.tint {
        for (o, b) in [(0x3912u32, 0x80u8), (0x3911, 0x40), (0x3910, 0x30)] {
            fx.mem.write_u8(D + o, b);
        }
    }
    if v.turn {
        if v.tint {
            fx.mem.write_u8(D + 0x3913, 0);
        }
        target_turn(fx, TURN);
        let a = fx.mem.read_u32(TURN + 4);
        fx.mem.write_u32(D + v.cap, a);
        if fx.mem.read_u32(D + v.cap) == 0 {
            fx.mem.write_u32(D + v.cap, 0x1_0000);
        }
    }
    fx.texture5(sp + 0x80, 0);
    // The motes: flags 0x40001001, half size 2.0, cell (0, 0, 0x80, 0x40),
    // white, all turned by (0, 0x400, 0).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(MOTES, tex);
    fx.mem.write_u32(MOTES + 4, 0x4000_1001);
    fx.mem.write_u32(MOTES + 0xC, 0x4000_0000);
    for (o, x) in [(0x20u32, 0i32), (0x22, 0), (0x24, v.mote_cell.0), (0x26, v.mote_cell.1)] {
        fx.mem.set16(MOTES + o, x);
    }
    for o in [0xAu32, 9, 8, 0xB] {
        fx.mem.write_u8(MOTES + o, 0x80);
    }
    for (o, x) in [(0x10u32, 0u32), (0x14, 0x400), (0x18, 0)] {
        fx.mem.write_u32(MOTES + o, x);
    }
    for k in 0..0x3Cu32 {
        let p = MOTES + 0x3F0 + 0xC * k;
        let a = angle(fx, v.ra[0]);
        let y = tenth(fx, v.ra[1], 1000);
        fx.mem.setf(p + 4, y);
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(4.0, c));
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(4.0, s));
        for (o, ra) in [(0u32, v.ra[2]), (4, v.ra[3]), (8, v.ra[4])] {
            let a = angle(fx, ra);
            fx.mem.set32(MOTES + 0x30 + 0x10 * k + o, a);
        }
    }
    fx.texture4(sp + 0x80, 0xF);
    // The strips (+0xE0): flags 0x9003, colour (0x80, 0x40, 0x30), alpha
    // 0, 32 x 32 cells at (0x80, 0xC0) of a 0x80 x 0x100 sheet, lifted 8,
    // growing x 1.08, 256 particles, 8 frames each update.
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(STRIPS, tex);
    for (o, x) in [(6u32, 0x80u8), (5, v.green), (4, 0x30), (7, 0)] {
        fx.mem.write_u8(STRIPS + o, x);
    }
    fx.mem.write_u32(STRIPS + 8, 0x9003);
    fx.mem.write_u32(STRIPS + 0xC, PARTS);
    fx.mem.write_u32(STRIPS + 0x60, 0x4100_0000);
    for (o, x) in [(0x10u32, 0x80i32), (0x12, 0xC0), (0x14, 0x20), (0x16, 0x20), (0x18, 0x80), (0x1A, 0xC0), (0x1C, 0x80), (0x1E, 0x100)] {
        fx.mem.set16(STRIPS + o, x);
    }
    fx.mem.write_u32(STRIPS + 0x64, 0x3F8A_3D71);
    for (o, x) in [(0x72u32, 0i32), (0x74, 0x18), (0x76, 0x100), (0x78, 0x100), (0x7C, 8), (0x7E, 1)] {
        fx.mem.set16(STRIPS + o, x);
    }
    // The four strip points: (sin, 0, cos) of 0, 4096, 8192, 12288 radians.
    for k in 0..4u32 {
        let t = ee::from_i32((0x1000 * k) as i32);
        let c = fx.cosf(t);
        fx.mem.setf(STRIPS + 0x28 + 0x10 * k, c);
        let s = fx.sinf(t);
        fx.mem.setf(STRIPS + 0x20 + 0x10 * k, s);
        fx.mem.write_u32(STRIPS + 0x24 + 0x10 * k, 0);
    }
    let mut k = 0;
    while k < fx.mem.i16(STRIPS + 0x76) {
        let p = PARTS + 0x20 * k as u32;
        fx.mem.set16(p + 0xE, 0);
        fx.mem.write_u8(p + 0xC, 0x10);
        let r = fx.rand_from(D, v.ra[5]);
        fx.mem.set8(p + 0xD, r % 8);
        let y = tenth(fx, v.ra[6], 1000);
        fx.mem.setf(p + 4, y);
        let a = angle(fx, v.ra[7]);
        fx.mem.set32(p + 0x14, a);
        let s = tenth(fx, v.ra[8], 0x3C);
        fx.mem.setf(p + 0x10, s);
        k += 1;
    }
    // The sprites (+0x2890): flags 0x58000003, colour (0x80, 0x40, 0x30)
    // alpha 0x10, size 30, cells as the strips', 256 particles at +0x28F0.
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(SPRITES, tex);
    fx.mem.write_u32(SPRITES + 4, 0x5800_0003);
    for (o, x) in [(0xAu32, 0x80u8), (9, v.green), (8, 0x30), (0xB, 0x10)] {
        fx.mem.write_u8(SPRITES + o, x);
    }
    fx.mem.write_u32(SPRITES + 0xC, 0x41F0_0000);
    for o in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(SPRITES + o, 0);
    }
    for (o, x) in [(0x30u32, 0x80i32), (0x32, 0xC0), (0x34, 0x20), (0x36, 0x20), (0x38, 0x80), (0x3A, 0xC0), (0x3C, 0x80), (0x3E, 0x40)] {
        fx.mem.set16(SPRITES + o, x);
    }
    fx.mem.write_u32(SPRITES + 0x40, D + 0x28F0);
    fx.mem.write_u32(SPRITES + 0x44, 0);
    fx.mem.write_u32(SPRITES + 0x48, 0);
    fx.mem.write_u32(SPRITES + 0x4C, 0x4120_0000);
    fx.mem.write_u32(SPRITES + 0x50, 0x40A0_0000);
    fx.mem.set16(SPRITES + 0x5A, 0x100);
    fx.mem.set16(SPRITES + 0x5C, 8);
    fx.mem.set16(SPRITES + 0x5E, 1);
    fx.sprites_spawn(SPRITES, 30.0);
    let mut k = 0;
    while k < fx.mem.i16(SPRITES + 0x5A) {
        let p = D + 0x28F0 + 0x10 * k as u32;
        let r = fx.rand_from(D, v.ra[9]);
        fx.mem.set16(p + 0xE, -(r % 16));
        let b = fx.mem.read_u8(SPRITES + 0xB);
        fx.mem.write_u8(p + 0xC, b);
        let r = fx.rand_from(D, v.ra[10]);
        fx.mem.set8(p + 0xD, r % 8);
        k += 1;
    }
    0
}

/// A sprite at a random angle on the circle of radius +0x4C round the
/// moving point (+0x2180), at its height.
fn sprite_place(fx: &mut Fx, p: u32, ra: u32) {
    let a = angle(fx, ra);
    let c = fx.tcos(a);
    let z = ee::add(fx.mem.f(D + 0x2188), ee::mul(fx.mem.f(SPRITES + 0x4C), c));
    fx.mem.setf(p + 8, z);
    let s = fx.tsin(a);
    let x = ee::add(fx.mem.f(D + 0x2180), ee::mul(fx.mem.f(SPRITES + 0x4C), s));
    fx.mem.setf(p, x);
    let y = fx.mem.read_u32(D + 0x2184);
    fx.mem.write_u32(p + 4, y);
}

fn step(fx: &mut Fx, sp: u32, param: i32, v: &V) -> i32 {
    let (nn, follow) = (D + v.n, D + v.follow);
    let n = |fx: &mut Fx| fx.mem.i32(nn);
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x38F0);
    fx.mem.write_u32(D + 0x38F4, 0);
    if n(fx) < 0x6F {
        let h = fx.self_handle();
        fx.unit_position(h, D + 0x2180);
        fx.mem.write_u32(D + 0x2184, 0);
    } else {
        let f = fx.mem.i32(follow);
        fx.approach_by(D + 0x2180, D + 0x38F0, f);
        if f != 0 {
            fx.mem.set32(follow, f - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 = the parameter, s3 the runner's (0).
    let regs = [Some(D), Some(OT), Some(param as u32), Some(0)];
    fx.xform_world_saving(D, v.ra[11], regs);
    fx.xform_billboard_saving(D + 0x70, v.ra[12], regs);
    fx.xform_world_saving(X, v.ra[13], regs);
    if v.tint {
        if n(fx) < 0x15E {
            fx.step_up(D + 0x3913, 0x40, 4);
        } else {
            fx.step_down(D + 0x3913, 0, 4);
        }
        crate::b262::tint(fx, D + 0x3910);
    } else if (v.quad.0..v.quad.1).contains(&n(fx)) {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if v.turn && fx.mem.i32(follow) == 0 {
        let h = fx.target_handle();
        for o in [0x8Cu32, 0x8D, 0x8E, 0x8B, 0x88, 0x89, 0x8A] {
            fx.mem.write_u8(sp + o, 0x80);
        }
        fx.mem.write_u8(sp + 0x8F, 0);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h as u32, 2, 0, sp + 0x88]);
        set_target_turn(fx, TURN);
        if n(fx) < 0x15E {
            let a = fx.mem.read_u32(TURN + 4).wrapping_add(0x1000) & 0xFFFF;
            fx.mem.write_u32(TURN + 4, a);
        } else {
            // 0x103350(p, cap, step): up by `step` to `cap`.
            let cap = fx.mem.i32(D + v.cap);
            let a = fx.mem.i32(TURN + 4);
            fx.mem.set32(TURN + 4, if cap - 0x1000 < a { cap } else { a + 0x1000 });
        }
    }
    spin::strips(fx, v.strips, OT, STRIPS, PARTS, X);
    let m = n(fx);
    if (v.rise..0x15E).contains(&m) {
        fx.step_up(STRIPS + 7, 0x10, 1);
    } else {
        fx.step_down(STRIPS + 7, 0, 1);
    }
    let mut k = 0;
    while k < fx.mem.i16(STRIPS + 0x76) {
        let p = PARTS + 0x20 * k as u32;
        let y = ee::add(fx.mem.f(p + 4), if k % 2 == 0 { 2.0 } else { 3.0 });
        fx.mem.setf(p + 4, y);
        if !ee_lt(fx.mem.f(p + 4), 100.0) {
            let y = tenth(fx, v.ra[14], 0x1E);
            fx.mem.setf(p + 4, y);
            let s = tenth(fx, v.ra[15], 0x1E);
            fx.mem.setf(p + 0x10, s);
        }
        k += 1;
    }
    fx.mem.write_u32(sp + 0x74, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x78, 0x3F8A_3D71);
    fx.mem.write_u32(sp + 0x70, 0x3F8A_3D71);
    spin::motes(fx, OT, MOTES, X, sp + 0x70);
    if n(fx) >= 0x15E {
        fx.step_down(MOTES + 0xB, 0, 8);
    }
    for k in 0..0x3Cu32 {
        let p = MOTES + 0x3F0 + 0xC * k;
        let y = ee::add(fx.mem.f(p + 4), if k % 2 == 0 { 2.0 } else { 3.0 });
        fx.mem.setf(p + 4, y);
        if !ee_lt(fx.mem.f(p + 4), 100.0) {
            let a = angle(fx, v.ra[16]);
            fx.mem.write_u32(p + 4, 0);
            let c = fx.tcos(a);
            fx.mem.setf(p + 8, ee::mul(4.0, c));
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(4.0, s));
        }
        for o in [0u32, 4, 8] {
            let a = MOTES + 0x30 + 0x10 * k + o;
            let v = fx.mem.read_u32(a).wrapping_add(0x1000);
            fx.mem.write_u32(a, v);
        }
    }
    if n(fx) >= 0x6E {
        fx.sprites(SPRITES, D);
        if n(fx) >= 0x15E {
            fx.step_down(SPRITES + 0xB, 0, 1);
        }
        let mut k = 0;
        while k < fx.mem.i16(SPRITES + 0x5A) {
            let p = D + 0x28F0 + 0x10 * k as u32;
            if fx.mem.i16(p + 0xE) == 0 {
                sprite_place(fx, p, v.ra[17]);
            }
            if fx.mem.i16(p + 0xE) >= 0 {
                let (_, r) = fx.div(k, 3);
                let dy = match r {
                    0 => 1.0,
                    1 => 2.0,
                    _ => 3.0,
                };
                let y = ee::add(fx.mem.f(p + 4), dy);
                fx.mem.setf(p + 4, y);
                if fx.step_down(p + 0xC, 0, 1) == 0 {
                    fx.mem.set16(p + 0xE, 0);
                    fx.mem.write_u8(p + 0xC, 0x10);
                    sprite_place(fx, p, v.ra[18]);
                }
            }
            k += 1;
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(nn, m);
    let left = fx.mem.i32(D + v.left) - 1;
    fx.mem.set32(D + v.left, left);
    if m == 0x15E {
        2
    } else if m == 0xAC {
        1
    } else if (0xB6..0x15E).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
