//! BMODULE b386 (card 73), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02750, data area 0x1ED0000) [F]: b038's family: a pool of 400 tasks (0x70 bytes at D+0x2A0, alive
//! +0x58, step +0x44 called through `jalr`) drawn as rings of one
//! camera-facing quad each (98b02c86a827 at 0x1E00920,
//! `billboard::ring_facing`; 9c13e08cfd2f at 0x1E00E90,
//! `quads4::ring_facing_flat`), UVs by uv_rect 0x1E00080. Every update
//! first stores the camera's +0x20 / +0x30 (0x1FACE0 / 0x1FAF40) at
//! D+0xB204 / D+0xB208. The emitter 0x1E013E0: to update 81 one rising
//! yellow mote (0x1E01AC0) and one yellow puff (0x1E02480) a update at
//! the attacker's node 0xA; updates 81..225 three yellow puffs
//! (0x1E016B0) and three white flashes (0x1E01D10) a update there,
//! drifting toward the target and bobbing 40 x (sin +0x5A - sin 70) in y;
//! updates 150..250 one white flash a update at the target (0x1E02110).
//! Returns 1 on update 100, 3 on 265, 6 on 111..259, 8 once the 275-update
//! timer D+0xB1F0 has run out.
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const PUFF: u32 = 0x01E0_16B0;
const MOTE: u32 = 0x01E0_1AC0;
const FLASH: u32 = 0x01E0_1D10;
const FLASH_T: u32 = 0x01E0_2110;
const PUFF_B: u32 = 0x01E0_2480;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_2A20;
const TIMER: u32 = D + 0xB1F0;
const COLOUR: u32 = D + 0xB1F4;
const NODE: u32 = D + 0xB228;
const INDEX: u32 = D + 0xB256;
const COUNTER: u32 = D + 0xB25E;
const BYTES: u32 = D + 0xB260;
const SINES: u32 = D + 0xB460;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02750 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xB1FC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let eye = fx.camera_eye(0);
    fx.mem.write_u32(D + 0xB204, eye);
    let at = fx.camera_at30(0);
    fx.mem.write_u32(D + 0xB208, at);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xB5D0);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xB25C, param);
    fx.mem.set32(TIMER, 0x113);
    for (at, node, clear) in [(0xB228u32, 0xA, 1), (0xB22A, 0x1C, 0), (0xB22C, 0xD, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(D + at, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xB238, r);
    for k in 0xE..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x298 + 4 * (k as u32 - 0xE), t);
    }
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0xB1D0 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(y, 0x01E0_2908);
    fx.mem.set16(D + 0xB25A, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2938, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 100..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_2994);
        fx.mem.set8(BYTES + k, v % 4);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        // The step's prologue: ra, the entry's s1 = 0, s0 = D.
        let frame = match f {
            PUFF => 0x50,
            FLASH => 0x60,
            FLASH_T => 0x30,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
        }
        match f {
            PUFF => puff(fx, t, s),
            MOTE => mote(fx, t),
            FLASH => flash(fx, t, s),
            FLASH_T => flash_t(fx, t),
            PUFF_B => puff_b(fx, t),
            _ => panic!("b386: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        // (cell local, texture, D+0xB8, bob, flat)
        let spec = match f {
            PUFF => Some((0x50u32, 0xB1D0u32, 1, true, false)),
            MOTE => Some((0x60, 0xB1D4, 6, false, true)),
            FLASH => Some((0x70, 0xB1D4, 2, true, false)),
            FLASH_T => Some((0x80, 0xB1D4, 2, false, false)),
            PUFF_B => Some((0x90, 0xB1D0, 1, true, false)),
            _ => None,
        };
        let Some((local, tex, n, bob, flat)) = spec else { continue };
        let sub = fx.mem.i16(t + 0x56);
        l2::rect(fx, sp + local, 0x14, sub);
        for k in 0..4 {
            let v = fx.mem.read_u32(sp + local + 4 * k);
            fx.mem.write_u32(D + 0x94 + 4 * k, v);
        }
        let o = if flat { fx.mem.i16(t + 0xA) } else { 5 };
        fx.mem.set16(D + 0xC0, o);
        let w = fx.mem.read_u32(D + tex);
        fx.mem.write_u32(D + 0x88, w);
        fx.mem.set16(D + 0xB8, n);
        for k in 0..3 {
            let v = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(D + 0x20 + 4 * k, v);
        }
        if bob {
            let s = l2::sin_deg(fx, SINES, 0x46, 0);
            let y = ee::sub(fx.mem.f(D + 0x24), ee::mul(40.0, s));
            fx.mem.setf(D + 0x24, y);
            let a = fx.mem.i16(t + 0x5A);
            let s = l2::sin_deg(fx, SINES, a, 0);
            let y = ee::add(fx.mem.f(D + 0x24), ee::mul(40.0, s));
            fx.mem.setf(D + 0x24, y);
        }
        if flat {
            quads4::ring_facing_flat(fx, uv::uv_rect, D, t, 0x20, BYTES);
        } else {
            fx.mem.set16(D + 0xBE, 0);
            billboard::ring_facing(fx, uv::uv_rect, D, t, 0x20, BYTES);
        }
        fx.mem.set16(D + 0xB8, 0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x64 => 1,
        0x109 => 3,
        0x6F..=0x103 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// D+0x250 = the attacker's node 0xA.
fn place(fx: &mut Fx) {
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE);
    fx.node_position(h, slot, POS);
}

/// 0x1E013E0(D, 0): the update counter and the emitters (frame 0x40: ra,
/// the entry's s2 = param, s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_29F0), (0x20, param as i64 as u64), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if (0x51..0xE2).contains(&c(fx)) {
        for _ in 0..3 {
            place(fx);
            spawn(fx, PUFF);
        }
    }
    if c(fx) < 0x52 {
        place(fx);
        spawn(fx, MOTE);
    }
    if (0x51..0xE2).contains(&c(fx)) {
        for _ in 0..3 {
            place(fx);
            spawn(fx, FLASH);
        }
    }
    if (0x96..0xFB).contains(&c(fx)) {
        let h = fx.target_handle();
        fx.unit_position(h, POS);
        spawn(fx, FLASH_T);
    }
    if c(fx) < 0x52 {
        place(fx);
        spawn(fx, PUFF_B);
    }
    fx.leave(0x40);
}

/// 0x1E02F80(D, step, arg): the first free task cleared and set off at
/// D+0x250 with the colour D+0xB1F4 (twice), index D+0xB256.
fn spawn(fx: &mut Fx, f: u32) {
    fx.enter(0x60);
    if let Some(k) = (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        let t = D + 0x2A0 + 0x70 * k;
        fx.bzero(t, 0x70);
        fx.mem.set16(t + 0x58, 1);
        for c in [0x60u32, 0x64] {
            let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
            l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
            let a = fx.mem.read_u8(COLOUR + 3);
            fx.mem.write_u8(t + c + 3, a);
        }
        l2::to_i16x4(fx, t, POS, 0);
        for k in 0..3 {
            let w = fx.mem.read_u32(POS + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
        let a = fx.mem.i16(INDEX);
        fx.mem.set16(t + 0x50, a);
        fx.mem.write_u32(t + 0x40, 0);
        fx.mem.write_u32(t + 0x44, f);
    }
    fx.leave(0x60);
}

/// Byte +0x63 up by 0xC0 while under 0xC0 before age +6, then down by
/// `down` (dead under `floor`).
fn fade(fx: &mut Fx, t: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0xC0));
        }
    } else if a >= floor {
        fx.mem.write_u8(t + 0x63, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The first steps' common part (the rand return addresses in `ra`, in
/// order): size, cell (rand >> 4) % 8, colour, angle 70, +0x5C, flag +0xA,
/// +0xC = +0x50, +0x50 (`radius`: 0, rand % 50 + 10 or (rand >> 4) % 450 +
/// 30), +0x52 / +0xE / +8 / +6 random, a velocity (-20..19) / 60 per
/// axis.
fn setup(fx: &mut Fx, t: u32, size: u32, rgb: (i32, i32, i32), radius: u8, ra: &[u32]) {
    fx.mem.write_u32(t + 0x48, size);
    let mut r = ra.iter().copied();
    let mut next = || r.next().expect("rand site");
    let v = fx.rand_from(t, next());
    fx.mem.set16(t + 0x56, (v >> 4) % 8);
    l2::rgb(fx, t + 0x60, rgb.0, rgb.1, rgb.2);
    fx.mem.set16(t + 0x5A, 0x46);
    let v = rmod(fx, t, next(), 0x168);
    fx.mem.set16(t + 0x5C, v);
    let v = fx.rand_from(t, next());
    fx.mem.set16(t + 0xA, if v % 2 != 0 { 2 } else { 0 });
    let a = fx.mem.i16(t + 0x50);
    fx.mem.set16(t + 0xC, a);
    match radius {
        0 => fx.mem.set16(t + 0x50, 0),
        1 => {
            let v = rmod(fx, t, next(), 0x32);
            fx.mem.set16(t + 0x50, v + 0xA);
        }
        _ => {
            let v = fx.rand_from(t, next());
            let h = fx.div(v >> 4, 0x1C2).1;
            fx.mem.set16(t + 0x50, h + 0x1E);
        }
    }
    let v = rmod(fx, t, next(), 0x15E);
    fx.mem.set16(t + 0x52, v + 0x64);
    let v = rmod(fx, t, next(), 0x14);
    fx.mem.set16(t + 0xE, v + 0xA);
    let v = rmod(fx, t, next(), 0x23);
    fx.mem.set16(t + 8, v);
    let v = rmod(fx, t, next(), 5);
    fx.mem.set16(t + 6, v + 3);
    for o in [0x30u32, 0x34, 0x38] {
        let v = rmod(fx, t, next(), 0x28);
        fx.mem.setf(t + o, ee::from_i32(v - 0x14));
    }
    for o in [0x30u32, 0x34, 0x38] {
        let v = ee::div(fx.mem.f(t + o), 60.0);
        fx.mem.setf(t + o, v);
    }
}

/// The z speed (target z - node 0xA z) / 80 into +0x3C (the step's locals
/// sp + 0x30 / + 0x40), the frames 0x1F71A0 / 0x1F6830 leave.
fn z_speed(fx: &mut Fx, t: u32, sp: u32, ra: [u32; 2]) {
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x30, u64::from(t), ra[0]);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE);
    crate::b076::node_position_saving(fx, h, slot, sp + 0x40, u64::from(t), Some(u64::from(D)), ra[1]);
    let dz = ee::div(ee::sub(fx.mem.f(sp + 0x38), fx.mem.f(sp + 0x48)), 80.0);
    fx.mem.setf(t + 0x3C, dz);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The age held at 100 from update 251 of the effect.
fn hold(fx: &mut Fx, t: u32) {
    if fx.mem.i16(COUNTER) >= 0xFB {
        fx.mem.set16(t + 0x54, 0x64);
    }
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

fn young(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6)
}

/// 0x1E016B0 (D, task): a yellow puff (size 30, + 0.8 while young) at
/// node 0xA moving for its updates 11..69, toward the target in z while
/// young (life 80..99), then rising by 1.5; angle +0x5A up by 3 to 160.
fn puff(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let ra = [0x01E0_16E0, 0x01E0_171C, 0x01E0_1738, 0x01E0_1774, 0x01E0_1794, 0x01E0_17B4, 0x01E0_17D0, 0x01E0_17F0, 0x01E0_181C, 0x01E0_1848];
        setup(fx, t, 0x41F0_0000, (0xCF, 0xCF, 0x40), 0, &ra);
        z_speed(fx, t, sp, [0x01E0_18AC, 0x01E0_18CC]);
        let v = fx.rand_from(t, 0x01E0_18F8);
        let h = fx.div(v >> 4, 0x14).1;
        fx.mem.set16(t + 6, h + 0x50);
        fx.mem.set16(t + 0x58, 2);
    }
    fade(fx, t, 0xE, 0xC);
    if (0xB..0x46).contains(&fx.mem.i16(t + 0x54)) {
        for o in [0u32, 4, 8] {
            let v = fx.mem.f(t + 0x30 + o);
            add(fx, t + 0x10 + o, v);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        if young(fx, t) {
            let v = fx.mem.f(t + 0x3C);
            add(fx, t + 0x18, v);
        }
        if !young(fx, t) {
            add(fx, t + 0x14, 1.5);
        }
        if young(fx, t) {
            add(fx, t + 0x48, f32::from_bits(0x3F4C_CCCD));
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let a = fx.mem.i16(t + 0x5A);
        if a < 0xA0 {
            fx.mem.set16(t + 0x5A, a + 3);
        }
    }
    hold(fx, t);
    age(fx, t);
}

/// 0x1E01AC0 (D, task): a rising mote (size 6, cell 0..3, flashing
/// yellow / white on odd / even updates), radius 2..6 growing by 1..4,
/// rising by (rand % 10) / 15 a update, spinning by 10; 0x80, out by 8.
fn mote(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        let v = fx.rand_from(t, 0x01E0_1AE8);
        fx.mem.set16(t + 0x56, v % 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x40);
        let v = rmod(fx, t, 0x01E0_1B1C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_1B38);
        fx.mem.set16(t + 0xA, v % 8);
        let a = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, a);
        let v = rmod(fx, t, 0x01E0_1B5C, 5);
        fx.mem.set16(t + 0x50, v + 2);
        let v = fx.rand_from(t, 0x01E0_1B7C);
        fx.mem.set16(t + 0xE, v % 4 + 1);
        let v = rmod(fx, t, 0x01E0_1B9C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_1BB8, 5);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    if young(fx, t) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x63, a - 8);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x40);
    } else {
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0x20);
    }
    let v = rmod(fx, t, 0x01E0_1C78, 0xA);
    fx.mem.setf(t + 0x34, ee::from_i32(v));
    let v = ee::div(fx.mem.f(t + 0x34), 15.0);
    fx.mem.setf(t + 0x34, v);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.f(t + 0x34);
        add(fx, t + 0x14, v);
    }
    let a = fx.mem.i16(t + 0x5A) + 0xA;
    fx.mem.set16(t + 0x5A, a);
    age(fx, t);
}

/// White on odd effect updates, grey on even.
fn flicker(fx: &mut Fx, t: u32) {
    let g = if fx.mem.i16(COUNTER) % 2 == 1 { 0xFF } else { 0x80 };
    l2::rgb(fx, t + 0x60, g, g, g);
}

/// 0x1E01D10 (D, task): a white flash (size 10) at node 0xA, radius 10..59
/// growing by 10..29, toward the target in z while young (life 10..19);
/// out by 0xE.
fn flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let ra = [0x01E0_1D40, 0x01E0_1D7C, 0x01E0_1D98, 0x01E0_1DD0, 0x01E0_1DF0, 0x01E0_1E10, 0x01E0_1E30, 0x01E0_1E4C, 0x01E0_1E6C, 0x01E0_1E98, 0x01E0_1EC4];
        setup(fx, t, 0x4120_0000, (0xFF, 0xFF, 0xFF), 1, &ra);
        z_speed(fx, t, sp, [0x01E0_1F28, 0x01E0_1F48]);
        let v = fx.rand_from(t, 0x01E0_1F74);
        let h = fx.div(v >> 4, 0xA).1;
        fx.mem.set16(t + 6, h + 0xA);
        fx.mem.set16(t + 0x58, 2);
    }
    fade(fx, t, 0x10, 0xE);
    flicker(fx, t);
    let h = fx.target_handle();
    crate::b076::unit_position_saving(fx, h, sp + 0x50, u64::from(t), 0x01E0_205C);
    if fx.mem.i16(t + 0x54) != 0 && young(fx, t) {
        let v = fx.mem.f(t + 0x3C);
        add(fx, t + 0x18, v);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let a = fx.mem.i16(t + 0x5A);
        if a < 0xA0 {
            fx.mem.set16(t + 0x5A, a + 3);
        }
    }
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    hold(fx, t);
    age(fx, t);
}

/// 0x1E02110 (D, task): a white flash (size 10) at the target, radius
/// 30..479; rising by 1.5 once old.
fn flash_t(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let ra = [0x01E0_2140, 0x01E0_217C, 0x01E0_2198, 0x01E0_21D0, 0x01E0_21F4, 0x01E0_2214, 0x01E0_2234, 0x01E0_2250, 0x01E0_2270, 0x01E0_229C, 0x01E0_22C8];
        setup(fx, t, 0x4120_0000, (0xFF, 0xFF, 0xFF), 2, &ra);
        fx.mem.set16(t + 0x58, 2);
    }
    fade(fx, t, 0xE, 0xC);
    flicker(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        if young(fx, t) {
            let v = fx.mem.f(t + 0x3C);
            add(fx, t + 0x18, v);
        }
        if !young(fx, t) {
            add(fx, t + 0x14, 1.5);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let a = fx.mem.i16(t + 0x5A);
        if a < 0xA0 {
            fx.mem.set16(t + 0x5A, a + 3);
        }
    }
    age(fx, t);
}

/// 0x1E02480 (D, task): a yellow puff (size 20, + 0.8 while young) at
/// node 0xA, rising by 0.5 once old.
fn puff_b(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let ra = [0x01E0_24A8, 0x01E0_24E4, 0x01E0_2500, 0x01E0_253C, 0x01E0_255C, 0x01E0_257C, 0x01E0_2598, 0x01E0_25B8, 0x01E0_25E4, 0x01E0_2610];
        setup(fx, t, 0x41A0_0000, (0xCF, 0xCF, 0x40), 0, &ra);
        fx.mem.set16(t + 0x58, 2);
    }
    fade(fx, t, 0xE, 0xC);
    if fx.mem.i16(t + 0x54) != 0 {
        if !young(fx, t) {
            add(fx, t + 0x14, 0.5);
        }
        if young(fx, t) {
            add(fx, t + 0x48, f32::from_bits(0x3F4C_CCCD));
        }
    }
    age(fx, t);
}
