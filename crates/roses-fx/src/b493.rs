//! BMODULE b493 (card 467), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E038B0, data area 0x1ED0000) [F]: b228's program (card 377) on a pool of 150 tasks (0x70 bytes at
//! +0x2A0, alive +0x58, step +0x44 through `jalr`; spawned by 0x1E04150 =
//! b193's spawn with +0x52 from +0xBCEC (colour +0xBC94, +0x50 from
//! +0xBCEA)), drawn with the second library (`uv_rect`,
//! `ring_facing_flat_z`, `ring_quads`, `quad`, `ring_band_f` and a record
//! copy of `quad_b_twice` sized by the float +0x48). The emitter 0x1E01F90
//! counts +0xBCF4 and keeps the attacker's node 0x10 (+-2 at random) at
//! D+0x250: on update 100 two bands (0x1E03460: `ring_band_f` riding the
//! attacker's nodes 0x10 / 0x39); on the odd updates 63..99 an orb
//! (0x1E02200: a coloured glow at its slot's centre +0xB4F0 + 0x10 k, one
//! of five colours by +0x52 = rand % 5, riding the attacker's node 0x10 /
//! 0x39 to update 100, then drifting by +0x30 / +0x34; it grows from age
//! 41 and flies 3.6 a update along z by the facing; at age 88 five mote
//! rings (0x1E02B40), two spark rings (0x1E02DF0) and a flash quad
//! (0x1E03310) at the centre). The tube step 0x1E03450 (`jr ra`) and the
//! band steps 0x1E03100 / 0x1E035F0 are drawn but never spawned (not
//! ported; the TRAIL3 tube 0x1E042F0 is only their draw). Returns 1 on
//! update 150, 4 on 220, 2 on 205, 6 on 161..199, 8 once the 230-update
//! timer +0xBC90 has run out.
use crate::b076::node_position_saving;
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::c061::{desc_uv4, rgbaq_at};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const ORB: u32 = 0x01E0_2200;
const MOTE: u32 = 0x01E0_2B40;
const SPARKS: u32 = 0x01E0_2DF0;
const FLASH: u32 = 0x01E0_3310;
const BAND: u32 = 0x01E0_3460;
/// The return address of the entry's `jalr` into a step (0x1E03BA8 + 8).
const TASK_RA: u64 = 0x01E0_3BB0;
const CENTRES: u32 = D + 0xB4F0;
const TEX: u32 = D + 0xBC70;
const TIMER: u32 = D + 0xBC90;
const COLOUR: u32 = D + 0xBC94;
const NODE1: u32 = D + 0xBCBC;
const SLOT: u32 = D + 0xBCEA;
const SLOT2: u32 = D + 0xBCEC;
const NEXT: u32 = D + 0xBCEE;
const YAW: u32 = D + 0xBCF0;
const COUNTER: u32 = D + 0xBCF4;
const BYTES: u32 = D + 0xBCF6;
const BYTES_B: u32 = D + 0xBDF6;

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// rand (the caller's s0 = `s0`) from `ra`, mod `m` (truncated, `div`).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// rand from `ra` masked (`andi`, rand being non-negative).
fn rand_and(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    fx.rand_from(s0, ra) & m
}

fn odd(n: i32) -> bool {
    n % 2 == 1
}

/// The five spawn colours by +0x52 (red, green, blue, yellow, violet).
fn palette(n: i32) -> Option<(i32, i32, i32)> {
    match n {
        0 => Some((0x80, 8, 8)),
        1 => Some((8, 0x80, 8)),
        2 => Some((8, 8, 0x80)),
        3 => Some((0x80, 0x80, 8)),
        4 => Some((0x40, 8, 0x80)),
        _ => None,
    }
}

/// +0x60 / +0x64 in the colour of +0x52.
fn colour_both(fx: &mut Fx, t: u32) {
    if let Some((r, g, b)) = palette(fx.mem.i16(t + 0x52)) {
        l2::rgb(fx, t + 0x60, r, g, b);
        l2::rgb(fx, t + 0x64, r, g, b);
    }
}

fn centre(fx: &mut Fx, t: u32) -> u32 {
    CENTRES + 0x10 * (fx.mem.i16(t + 0x50) as u32)
}

/// The entry 0x1E038B0 (a0 = D, a1 = param; s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    fx.mem.write_u32(D + 0xBC9C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xC070);
    l2::sine_table(fx, D + 0xBEF8);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xBCF2, param);
    fx.mem.set32(TIMER, 0xE6);
    for (at, n, clear) in [(NODE1, 0x10, 1), (D + 0xBCBE, 0x39, 0), (D + 0xBCC0, 0xD, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(at, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(D + 0xBCCA, r);
    for k in 0xEu32..0x10 {
        let tex = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x260 + 4 * k, tex);
    }
    let tex = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, tex);
    let tex = fx.texture(5, 0);
    fx.mem.write_u32(TEX, tex);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xA0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xA4)), 180.0);
    fx.fix_residue(v, 0x01E0_3A4C);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3A7C, 0x1E);
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_3AD4);
        fx.mem.write_u8(BYTES + k, v.wrapping_rem(4) as u8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3B2C, 0x28);
        fx.mem.write_u8(BYTES_B + k, (v - 0x14) as u8);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        // The steps' prologues: the entry's s0 = D, s1 = 0.
        let frame = match f {
            ORB => 0x40,
            BAND => 0x30,
            MOTE | SPARKS | FLASH => 0x20,
            _ => panic!("b493: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match f {
            ORB => {
                fx.saved(s, &[(0x30, TASK_RA), (0x20, 0), (0x10, u64::from(D))]);
                // swc1 f20: the runner's (0) [H].
                fx.mem.write_u32(s, 0);
            }
            BAND => fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
        }
        match f {
            ORB => orb(fx, t, slot, k),
            MOTE => mote(fx, t),
            SPARKS => sparks(fx, t),
            FLASH => flash(fx, t),
            _ => band(fx, t),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        draw(fx, sp, t);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x96 => 1,
        0xDC => 4,
        0xCD => 2,
        0xA1..=0xC7 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` frame +0x56 (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, id: i32, r: u32) {
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, frame);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

fn texture(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// The entry's draw of one live task by its step (s0 = D).
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    if f == ORB {
        cell(fx, t, 0x9B, sp + 0x60);
        texture(fx, D + 0x29C);
        fx.mem.set16(D + 0xBE, 0);
        for o in 0..3 {
            let c = centre(fx, t);
            let v = fx.mem.f(c + 4 * o);
            fx.mem.setf(sp + 0x50 + 4 * o, v);
        }
        quad_twice48(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
    }
    if f == MOTE {
        cell(fx, t, 0x9B, sp + 0x70);
        texture(fx, D + 0x29C);
        fx.mem.set16(D + 0xB8, 6);
        let v = fx.mem.i16(t + 0xA);
        fx.mem.set16(D + 0xC0, v);
        for o in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * o);
            fx.mem.write_u32(D + 0x20 + 4 * o, w);
        }
        quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == SPARKS {
        texture(fx, D + 0x29C);
        cell(fx, t, 0x9B, sp + 0x80);
        fx.mem.set16(D + 0xB8, 0xA);
        fx.mem.set16(D + 0xB0, 1);
        billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
        fx.mem.set16(D + 0xB8, 0);
        fx.mem.set16(D + 0xB0, 0);
    }
    if f == FLASH {
        texture(fx, TEX);
        let c = fx.mem.i16(t + 0x56) << 6;
        fx.mem.set16(D + 0x94, c);
        fx.mem.set16(D + 0x96, 0);
        fx.mem.set16(D + 0x98, 0x3F);
        fx.mem.set16(D + 0x9A, 0x3F);
        fx.mem.set16(D + 0xBE, 0);
        billboard::quad(fx, uv::uv_rect, D, t, 0x21, t + 0x10);
    }
    if f == BAND {
        for o in 0..4 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * o);
            fx.mem.write_u32(sp + 0x50 + 4 * o, w);
        }
        quads4::ring_band_f(fx, D, t, 0x21, sp + 0x50);
    }
    if f != ORB && f != MOTE && f != SPARKS && f != FLASH && f != BAND && f != 0 {
        panic!("b493: unported draw case {f:#x}");
    }
}

/// 0x1E04E30(D, task, mode, pos): `billboard::quad_b_twice` (c004d75a62da)
/// with the quad's size the float +0x48 both ways (in place of i16 +8 /
/// +0xA) [F fdiff vs card 84's 0x1E050A0: only those loads differ].
fn quad_twice48(fx: &mut Fx, uv: billboard::Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    const SPR: u32 = 0x7000_0000;
    let frame = 0x1B0;
    let sp = fx.enter(frame);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x50 + 4 * k, w);
    }
    let alpha = match (mode >> 4) & 0xF {
        0 => crate::fxlib::ALPHA_42,
        1 => crate::fxlib::ALPHA_44,
        _ => 0x80_0000_0048,
    };
    let d = sp + frame - 0x98;
    desc_uv4(fx, d);
    fx.desc_alpha(d, alpha);
    rgbaq_at(fx, sp + frame - 8, t + 0x60, d + 0x10);
    let order = fx.mem.i16(d0 + 0xBE);
    uv(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x50 + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
        fx.mem.write_u32(sp + 0xA0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, t + 0x20, sp + 0xA0);
    let s = fx.mem.f(t + 0x48);
    l2::quad_xy(fx, sp + 0x60, s, s);
    if l2::project(fx, sp + 0x60, SPR, sp + 0xD0, d, sp + 0xC0) {
        let far = [d + 0x50, d + 0x40, d + 0x30, d + 0x20];
        let ot = fx.mem.read_u32(d0 + 0x80);
        match mode & 0xF {
            0 => {
                for _ in 0..2 {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0xC0)) as i32;
                    fx.submit(d, ot, key);
                }
            }
            1 => {
                l2::far_z(fx, &far);
                fx.submit(d, ot, 0);
                fx.submit(d, ot, 0);
            }
            _ => {
                fx.submit(d, ot, -1);
                fx.submit(d, ot, -1);
            }
        }
    }
    fx.leave(frame);
}

/// The spawn's registers: s0..s4 at the call.
type Regs = [u64; 5];

/// 0x1E04150(D, step, 0) (b193's 0x1E03A90 with +0x52 from +0xBCEC): the
/// first free task cleared and set off at +0x250 with the colour +0xBC94
/// twice, +0x50 / +0x52 from +0xBCEA / +0xBCEC. Frame 0x60 (ra, s4 .. s0).
fn spawn(fx: &mut Fx, f: u32, r: Regs, ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, r[4]), (0x30, r[3]), (0x20, r[2]), (0x10, r[1]), (0, r[0])]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x70 * k + 0x2F8) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for c in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + c, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + c + 3, a);
            }
            l2::to_i16x4(fx, t, D + 0x250, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(D + 0x250 + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(SLOT);
            fx.mem.set16(t + 0x50, a);
            let a = fx.mem.i16(SLOT2);
            fx.mem.set16(t + 0x52, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E01F90(D, 0) (frame 0x40: ra, the entry's s2 = param, s1 = 0, s0 =
/// D): the update counter +0xBCF4, the bands, the spawn point and the orbs.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_3B80), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 0x64 {
        let r: Regs = [u64::from(D), 0, u64::from(D), 0, 0];
        fx.mem.set16(SLOT, 0);
        spawn(fx, BAND, r, 0x01E0_1FF4);
        fx.mem.set16(SLOT, 1);
        spawn(fx, BAND, r, 0x01E0_2018);
    }
    let h = fx.self_handle();
    let s = fx.mem.i16(NODE1);
    node_position_saving(fx, h, s, D + 0x250, u64::from(D), Some(0), 0x01E0_2038);
    for (o, ra) in [(0u32, 0x01E0_2040u32), (4, 0x01E0_2074), (8, 0x01E0_20A8)] {
        let v = rand_and(fx, D, ra, 3) - 2;
        let x = ee::add(fx.mem.f(D + 0x250 + o), ee::from_i32(v));
        fx.mem.setf(D + 0x250 + o, x);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x3F..=0x64).contains(&c) && odd(c) {
        let r: Regs = [0, 1, u64::from(D), 0, 0];
        let v = fx.mem.i16(NEXT);
        fx.mem.set16(SLOT, v);
        let v = rmod(fx, 0, 0x01E0_2150, 5);
        fx.mem.set16(SLOT2, v);
        spawn(fx, ORB, r, 0x01E0_217C);
        let n = fx.mem.i16(NEXT) + 1;
        fx.mem.set16(NEXT, n);
        if fx.mem.i16(NEXT) >= 0x78 {
            fx.mem.set16(NEXT, 0);
        }
    }
    fx.leave(0x40);
}

/// +0x63 down by `by` twice when at least `min` (the task ends when the
/// first finds it below).
fn fade_out2(fx: &mut Fx, t: u32, min: u8, by: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if a >= min {
        fx.mem.write_u8(t + 0x63, a - by);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if a >= min {
        fx.mem.write_u8(t + 0x63, a - by);
    }
}

/// +0x63 to `top` before age +6, then out by `by` (ends below `min`).
fn fade_by_life(fx: &mut Fx, t: u32, top: u8, min: u8, by: u8) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, top);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= min {
            fx.mem.write_u8(t + 0x63, a - by);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
}

/// +0x63 in by `inc` below `cap` before age `until`, then out by 9 (ends
/// below 11).
fn fade_in_out(fx: &mut Fx, t: u32, until: i32, cap: u8, inc: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < until {
        if a < cap {
            fx.mem.write_u8(t + 0x63, a + inc);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// z by `by` along the facing (- when the attacker's yaw is non-zero).
fn along_z(fx: &mut Fx, at: u32, by: f32) {
    let z = fx.mem.f(at);
    let z = if fx.mem.i16(YAW) != 0 { ee::sub(z, by) } else { ee::add(z, by) };
    fx.mem.setf(at, z);
}

/// 0x1E02200 (frame 0x40: s0 = the task, s1 = D, f20): an orb at its
/// slot's centre, coloured by +0x52; see the module notes.
fn orb(fx: &mut Fx, t: u32, slot: u32, k: u32) {
    let s0 = t;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4000_0000);
        colour_both(fx, t);
        let a = rand_and(fx, s0, 0x01E0_2358, 0xF) - 8;
        fx.mem.setf(t + 0x30, ee::from_i32(a));
        let b = rand_and(fx, s0, 0x01E0_2384, 0xF) - 8;
        fx.mem.setf(t + 0x34, ee::from_i32(b));
        for o in [0x30u32, 0x34] {
            let v = ee::div(fx.mem.f(t + o), 35.0);
            fx.mem.setf(t + o, v);
        }
        for o in 0..3 {
            let c = centre(fx, t);
            let v = fx.mem.read_u32(t + 0x10 + 4 * o);
            fx.mem.write_u32(c + 4 * o, v);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(COUNTER) < 0x64 {
        let h = fx.self_handle();
        let (node, ra) = if odd(fx.mem.i16(t + 0x50)) { (NODE1, 0x01E0_2488) } else { (D + 0xBCBE, 0x01E0_24B0) };
        let s = fx.mem.i16(node);
        node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(u64::from(D)), ra);
        let a = rmod(fx, s0, 0x01E0_24B8, 0x14) - 0xA;
        let f20 = ee::from_i32(a);
        let b = rmod(fx, s0, 0x01E0_24DC, 0x14);
        let f20 = ee::div(f20, 20.0);
        let c = centre(fx, t);
        let x = ee::add(fx.mem.f(t + 0x10), f20);
        fx.mem.setf(c, x);
        let dy = ee::div(ee::from_i32(b), 20.0);
        let c = centre(fx, t);
        let y = ee::add(fx.mem.f(t + 0x14), dy);
        fx.mem.setf(c + 4, y);
        let c = centre(fx, t);
        let z = fx.mem.read_u32(t + 0x18);
        fx.mem.write_u32(c + 8, z);
    } else {
        let dy = ee::div(fx.mem.f(t + 0x34), 2.0);
        let c = centre(fx, t);
        let y = ee::add(fx.mem.f(c + 4), dy);
        fx.mem.setf(c + 4, y);
        let dx = ee::div(fx.mem.f(t + 0x30), 2.0);
        let c = centre(fx, t);
        let x = ee::add(fx.mem.f(c), dx);
        fx.mem.setf(c, x);
    }
    if fx.mem.i16(t + 0x54) < 0x5F {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else {
        fade_out2(fx, t, 0x1A, 0x18);
    }
    let lit = odd(fx.mem.i16(t + 0x54));
    let c = match fx.mem.i16(t + 0x52) {
        0 => Some(if lit { (0xFF, 0x40, 0x40) } else { (0x80, 8, 8) }),
        1 => Some(if lit { (8, 0xFF, 8) } else { (0x40, 0x80, 0x40) }),
        2 => Some(if lit { (0x40, 0x40, 0xFF) } else { (8, 8, 0x80) }),
        3 => Some(if lit { (0xFF, 0xFF, 0x40) } else { (0x80, 0x80, 8) }),
        4 => Some(if lit { (0x80, 0x10, 0xFF) } else { (0x40, 8, 0x80) }),
        _ => None,
    };
    if let Some((r, g, b)) = c {
        l2::rgb(fx, t + 0x60, r, g, b);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2850);
    if fx.mem.i16(t + 0x54) == 0x58 {
        for o in 0..3 {
            let c = centre(fx, t);
            let w = fx.mem.read_u32(c + 4 * o);
            fx.mem.write_u32(D + 0x250 + 4 * o, w);
        }
        let v = fx.mem.i16(t + 0x52);
        fx.mem.set16(SLOT2, v);
        let r: Regs = [u64::from(t), u64::from(D), u64::from(slot), u64::from(k), 0];
        let mut ra = 0x01E0_28D8u32;
        for f in [MOTE, MOTE, MOTE, MOTE, MOTE, SPARKS, SPARKS, FLASH] {
            spawn(fx, f, r, ra);
            ra += 0x14;
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x63..0x83).contains(&c) {
        let at = centre(fx, t) + 8;
        along_z(fx, at, f32::from_bits(0x3DCC_CCCD));
    }
    age(fx, t);
    if fx.mem.i16(t + 0x54) >= 0x29 {
        let at = centre(fx, t) + 8;
        along_z(fx, at, f32::from_bits(0x4066_6666));
        let s = ee::add(fx.mem.f(t + 0x48), 0.5);
        fx.mem.setf(t + 0x48, s);
    }
    if fx.mem.i16(COUNTER) >= 0x65 {
        let s = fx.mem.f(t + 0x48);
        if s < 20.0 && fx.mem.i16(t + 0x54) < 0x28 {
            fx.mem.setf(t + 0x48, ee::add(s, f32::from_bits(0x3ECC_CCCD)));
        }
    }
}

/// 0x1E02B40 (frame 0x20: s0 = the task): a mote ring in the orb's
/// colour (size 10, 6 quads by `ring_facing_flat_z`), random turn, radius
/// rand % 50 + 10 growing by rand % 40 + 20, life rand % 5 + 3.
fn mote(fx: &mut Fx, t: u32) {
    let s0 = t;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4120_0000);
        let v = rand_and(fx, s0, 0x01E0_2B68, 3);
        fx.mem.set16(t + 0x56, v);
        colour_both(fx, t);
        let v = rmod(fx, s0, 0x01E0_2CAC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_and(fx, s0, 0x01E0_2CC8, 7);
        fx.mem.set16(t + 0xA, v);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = rmod(fx, s0, 0x01E0_2CEC, 0x32) + 0xA;
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, s0, 0x01E0_2D0C, 0x28) + 0x14;
        fx.mem.set16(t + 0xE, v);
        let v = rmod(fx, s0, 0x01E0_2D2C, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, s0, 0x01E0_2D48, 5) + 3;
        fx.mem.set16(t + 6, v);
        fx.mem.set16(t + 0x58, 2);
    }
    fade_by_life(fx, t, 0x80, 0xA, 8);
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    age(fx, t);
}

/// 0x1E02DF0 (frame 0x20: s0 = the task): a spark ring in the orb's
/// colour (size rand % 10 + 13, `ring_quads` layout 1 at random angles),
/// radius rand % 150 + 170 growing by rand % 3 + 1, life rand % 3 + 2.
fn sparks(fx: &mut Fx, t: u32) {
    let s0 = t;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, s0, 0x01E0_2E14, 0xA) + 0xD;
        fx.mem.setf(t + 0x48, ee::from_i32(v));
        let v = rand_and(fx, s0, 0x01E0_2E40, 3);
        fx.mem.set16(t + 0x56, v);
        colour_both(fx, t);
        for (o, ra) in [(0x20u32, 0x01E0_2F84u32), (0x24, 0x01E0_2FA0), (0x28, 0x01E0_2FBC)] {
            let v = rmod(fx, s0, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, s0, 0x01E0_2FE0, 0x96) + 0xAA;
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, s0, 0x01E0_3000, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, s0, 0x01E0_301C, 3) + 1;
        fx.mem.set16(t + 0xE, v);
        let v = rand_and(fx, s0, 0x01E0_303C, 7) + 1;
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, s0, 0x01E0_305C, 3) + 2;
        fx.mem.set16(t + 6, v);
        fx.mem.set16(t + 0x58, 2);
    }
    fade_by_life(fx, t, 0x60, 0xD, 0xB);
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
}

/// 0x1E03310 (frame 0x20: s0 = the task): a grey flash quad (size 80
/// growing 1 a update, cells 0..5 on the odd ages from 3), in by 0x60 for
/// 5 updates, facing the camera.
fn flash(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x50);
        fx.mem.set16(t + 0xA, 0x50);
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    fade_in_out(fx, t, 5, 0xC0, 0x60);
    let a = fx.mem.i16(t + 0x54);
    if a >= 2 && odd(a) {
        let v = fx.mem.i16(t + 0x56) + 1;
        fx.mem.set16(t + 0x56, v);
    }
    if fx.mem.i16(t + 0x56) >= 6 {
        fx.mem.set16(t + 0x56, 5);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 8) + 1;
        fx.mem.set16(t + 8, v);
        let v = fx.mem.i16(t + 0xA) + 1;
        fx.mem.set16(t + 0xA, v);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_3430);
    age(fx, t);
}

/// 0x1E03460 (frame 0x30: s0 = the task, s1 = D): a band (inner radius
/// 35, white inside, grey flickering outside) riding the attacker's node
/// +0xBCBC / +0xBCBE by +0x50, in by 0x10 below 0x80 for 63 updates,
/// facing the camera a quarter turn over.
fn band(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x420C_0000);
        l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
        l2::rgb(fx, t + 0x64, 0x60, 0x60, 0x60);
        let v = rmod(fx, t, 0x01E0_34BC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade_in_out(fx, t, 0x3F, 0x80, 0x10);
    let h = fx.self_handle();
    let i = fx.mem.i16(t + 0x50) as u32;
    let s = fx.mem.i16(NODE1 + 2 * i);
    node_position_saving(fx, h, s, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_3558);
    let g = if odd(fx.mem.i16(t + 0x54)) { 0x80 } else { 0x70 };
    l2::rgb(fx, t + 0x64, g, g, g);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_35B8);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}
