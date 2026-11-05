//! BMODULE b640 (card 202), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E037A0, data area 0x1ED0000) [F]: b731's task program (card 51) on a pool of 130 tasks (0x70
//! bytes at +0x2A0, alive +0x58, step +0x44 through `jalr`; spawned by
//! 0x1E042C0 = `spawn::task70` on POOL70_B4 f55f8bcd1468 (colour +0x9AD4,
//! +0x50 from +0x9DFA)), drawn with the second library (`uv_rect`,
//! `quad_b_twice`, `ring_facing_flat_z`, `ring_quads_z`, `arc_strips_flat`,
//! the tube `tubetrail::tube_const`) and a record fan (0x1E04EE0). The
//! emitter 0x1E01C70 counts +0x9E02: on update 1 a stage light 0x1E036C0
//! (down to 0x30 for 320 updates, back to 0x80); on 59 two white arcs
//! 0x1E030E0 (radius 110); 59..259 a spark ring 0x1E02EE0 a update at the
//! attacker's node 3 (its bytes +0xA004 + 0xB4.. drifting from 191 to 210);
//! on 213 an orb 0x1E01F10 at the attacker's node 3 and its tube 0x1E02C50
//! (a 70-point history +0x3BB0 + 0x460 k of the orb's centre +0x9330 +
//! 0x10 k): the orb rises and flies 6.5 a update along z by the facing for
//! 25 updates trailing fans 0x1E02770, then from age 27 on its odd ages
//! sheds two grey motes 0x1E02400 (b731's), two white spark rings
//! 0x1E02590, a violet arc 0x1E02AD0 and a violet mote 0x1E02920. The
//! steps 0x1E03220 / 0x1E03400 are drawn but never spawned (not ported).
//! Returns 1 on update 231, 3 on 323, 6 on 242..317, 8 once the 350-update
//! timer +0x9AD0 has run out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::b731::{self, MOTE_A};
use crate::blib::spawn::{self, history};
use crate::blib::{billboard, quads4, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 130;
const SPR: u32 = 0x7000_0000;
/// The step functions a task holds (+0x44), as the record's addresses.
const ORB: u32 = 0x01E0_1F10;
const MOTE: u32 = 0x01E0_2400;
const SPARKS: u32 = 0x01E0_2590;
const FAN: u32 = 0x01E0_2770;
const MOTE_V: u32 = 0x01E0_2920;
const ARC: u32 = 0x01E0_2AD0;
const TUBE: u32 = 0x01E0_2C50;
const SPARKS_Z: u32 = 0x01E0_2EE0;
const ARC_BIG: u32 = 0x01E0_30E0;
const LIGHT: u32 = 0x01E0_36C0;
/// The return address of the entry's `jalr` into a step (0x1E03B30 + 8).
const TASK_RA: u64 = 0x01E0_3B38;
const HISTORY: u32 = D + 0x3BB0;
const CENTRES: u32 = D + 0x9330;
const TEX_A: u32 = D + 0x9AB0;
const TEX_B: u32 = D + 0x9AB4;
const TIMER: u32 = D + 0x9AD0;
const OT_AT: u32 = D + 0x9ADC;
const RADII: u32 = D + 0x9AEC;
const NODE_A: u32 = D + 0x9CDC;
const TNODE: u32 = D + 0x9CEA;
const ANGLES: u32 = D + 0x9CFC;
const SLOT: u32 = D + 0x9DFA;
const NEXT: u32 = D + 0x9DFC;
const YAW: u32 = D + 0x9DFE;
const COUNTER: u32 = D + 0x9E02;
const BYTES: u32 = D + 0x9E04;
const BYTES_B: u32 = D + 0x9F04;
const BYTES_C: u32 = D + 0xA004;
const SINES: u32 = D + 0xA104;

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

fn slot(fx: &mut Fx, t: u32) -> u32 {
    fx.mem.i16(t + 0x50) as u32
}

fn grey(fx: &mut Fx, at: u32, g: i32) {
    l2::rgb(fx, at, g, g, g);
}

/// The entry 0x1E037A0 (a0 = D, a1 = param; s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xF0);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xF0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xA270);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x9E00, param);
    fx.mem.set32(TIMER, 0x15E);
    for (at, n, clear) in [(NODE_A, 3, 1), (D + 0x9CDE, 0x1D, 0), (D + 0x9CE0, 0x34, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(at, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(TNODE, r);
    for k in 0xAu32..0x10 {
        let tex = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x260 + 4 * k, tex);
    }
    let tex = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, tex);
    for k in 0..2u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX_A + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xE0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xE4)), 180.0);
    fx.fix_residue(v, 0x01E0_3940);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3970, 0x1E);
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_39CC);
        fx.mem.write_u8(BYTES + k, v.wrapping_rem(4) as u8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3A24, 0x28);
        fx.mem.write_u8(BYTES_B + k, (v - 0x14) as u8);
    }
    for k in 0xB4..0x100u32 {
        fx.mem.write_u8(BYTES_B + k, 0);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3AB4, 0x1E);
        fx.mem.write_u8(BYTES_C + k, (v - 0xF) as u8);
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
        // The steps' prologues: the entry's s0 = D, s1 = 0, s2 = the
        // slot, s3 = k.
        let frame = match f {
            TUBE => 0x60,
            ORB | SPARKS_Z => 0x30,
            MOTE | SPARKS | FAN | MOTE_V | ARC | ARC_BIG | LIGHT => 0x20,
            _ => panic!("b640: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match frame {
            0x60 => fx.saved(s, &[(0x40, TASK_RA), (0x30, u64::from(k)), (0x20, u64::from(slot)), (0x10, 0), (0, u64::from(D))]),
            0x30 => fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
        }
        match f {
            ORB => orb(fx, t, slot, k),
            MOTE => b731::mote(fx, t, MOTE, &MOTE_A),
            SPARKS => sparks(fx, t),
            FAN => fan_step(fx, t),
            MOTE_V => mote_v(fx, t),
            ARC => arc(fx, t),
            TUBE => tube_step(fx, s, t),
            SPARKS_Z => sparks_z(fx, t),
            ARC_BIG => arc_big(fx, t),
            _ => light(fx, t),
        }
        fx.leave(frame);
    }
    let mut drawn = 0u64;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        drawn += 1;
        draw(fx, sp, t, drawn);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xE7 => 1,
        0x143 => 3,
        0xF2..=0x13D => 6,
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

fn copy3(fx: &mut Fx, from: u32, to: u32) {
    for o in 0..3 {
        let w = fx.mem.read_u32(from + 4 * o);
        fx.mem.write_u32(to + 4 * o, w);
    }
}

/// The entry's draw of one live task by its step (s0 = D, s1 = the live
/// tasks counted so far, this one included).
fn draw(fx: &mut Fx, sp: u32, t: u32, drawn: u64) {
    let f = fx.mem.read_u32(t + 0x44);
    if f == ORB {
        cell(fx, t, 0x14, sp + 0x60);
        texture(fx, TEX_A);
        fx.mem.set16(D + 0xBE, 0);
        let c = CENTRES + 0x10 * slot(fx, t);
        copy3(fx, c, sp + 0x50);
        billboard::quad_b_twice(fx, uv::uv_rect, D, t, 0x21, sp + 0x50);
    }
    if f == MOTE {
        cell(fx, t, 0x14, sp + 0x70);
        texture(fx, TEX_A);
        fx.mem.set16(D + 0xB8, 6);
        let v = fx.mem.i16(t + 0xA);
        fx.mem.set16(D + 0xC0, v);
        copy3(fx, t + 0x10, D + 0x20);
        fx.mem.set16(D + 0xBE, 0);
        quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == SPARKS {
        texture(fx, TEX_A);
        cell(fx, t, 0x14, sp + 0x80);
        fx.mem.set16(D + 0xB8, 0xA);
        fx.mem.set16(D + 0xB0, 1);
        quads4::ring_quads_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
        fx.mem.set16(D + 0xB8, 0);
        fx.mem.set16(D + 0xB0, 0);
    }
    if f == ARC {
        fx.mem.set16(D + 0xB8, 0x18);
        billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == FAN {
        texture(fx, TEX_A);
        cell(fx, t, 0x14, sp + 0x90);
        fx.mem.set16(D + 0xB8, 1);
        fx.mem.set16(D + 0xBE, 0);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE_A);
        node_position_saving(fx, h, n, sp + 0x50, u64::from(D), Some(drawn), 0x01E0_3E3C);
        fan12(fx, t, 0x20, t + 0x10);
    }
    if f == MOTE_V {
        texture(fx, TEX_B);
        cell(fx, t, 0x14, sp + 0xA0);
        fx.mem.set16(D + 0xB8, 6);
        let v = fx.mem.i16(t + 0xA);
        fx.mem.set16(D + 0xC0, v);
        copy3(fx, t + 0x10, D + 0x20);
        quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x10, BYTES_B);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == TUBE {
        texture(fx, TEX_A);
        cell(fx, t, 0x14, sp + 0xB0);
        let v = fx.mem.i16(D + 0x96) + 0xF;
        fx.mem.set16(D + 0x96, v);
        fx.mem.set16(D + 0x98, 0x1F);
        fx.mem.set16(D + 0x9A, 1);
        let h = fx.self_handle();
        unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_3F78);
        for o in [0x20u32, 0x24, 0x28] {
            fx.mem.write_u32(D + o, 0);
        }
        fx.mem.set16(D + 0xBE, 2);
        let k = slot(fx, t);
        tubetrail::tube_const(fx, D, t, HISTORY + 0x460 * k, 1);
    }
    if f == SPARKS_Z {
        texture(fx, TEX_A);
        cell(fx, t, 0x14, sp + 0xC0);
        fx.mem.set16(D + 0xB8, 0xA);
        fx.mem.set16(D + 0xB0, 1);
        quads4::ring_quads_z(fx, uv::uv_rect, D, t, 0x20, BYTES_C);
        fx.mem.set16(D + 0xB8, 0);
        fx.mem.set16(D + 0xB0, 0);
    }
    if f == ARC_BIG {
        fx.mem.set16(D + 0xB8, 0x18);
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE_A);
        node_position_saving(fx, h, n, sp + 0x50, u64::from(D), Some(drawn), 0x01E0_4088);
        billboard::arc_strips_flat(fx, D, t, 0x21, sp + 0x50);
        fx.mem.set16(D + 0xB8, 0);
    }
    let known = [ORB, MOTE, SPARKS, FAN, MOTE_V, ARC, TUBE, SPARKS_Z, ARC_BIG, LIGHT, 0];
    if !known.contains(&f) {
        panic!("b640: unported draw case {f:#x}");
    }
}

/// 0x1E04EE0(D, t, mode, pos): twelve vertical strip3 pieces of 30
/// degrees from the angle +0x5A - 200 at the radius +0x48 in the x / z
/// plane, half height +0x34, coloured +0x60 / +0x64, UVs the atlas cell
/// 0x14 frame 4, through TRS(angles +0x20, pos) x the view-projection;
/// keyed by their mean w on D+0x9ADC; blend (mode >> 4) [F; b764's petal
/// sprite with the height and the full turn].
fn fan12(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1D0);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x80 + 4 * k, w);
    }
    let d = sp + 0x140;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 1);
    l2::colour_strip3(fx, d, t + 0x64, 2);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    fx.desc_alpha(d, crate::c061::blend_alpha(mode >> 4));
    copy3(fx, sp + 0x80, sp + 0xD0);
    l2::xform_vp(fx, sp + 0xE0, 0, t + 0x20, sp + 0xD0);
    let h = fx.mem.f(t + 0x34);
    fx.mem.setf(sp + 0x94, h);
    fx.mem.setf(sp + 0xA4, h);
    fx.mem.setf(sp + 0xB4, ee::neg(h));
    fx.mem.setf(sp + 0xC4, ee::neg(h));
    let r = fx.mem.f(t + 0x48);
    l2::rect(fx, sp + 0x130, 0x14, 4);
    for k in 0..4 {
        let v = fx.mem.read_u32(sp + 0x130 + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let mut s1 = 0i32;
    for _ in 0..0xC {
        for (e, lo, hi) in [(0, 0xB0u32, 0x90u32), (0x1E, 0xC0, 0xA0)] {
            let a = (fx.mem.i16(t + 0x5A) + e + s1 - 200) as i16 as i32;
            let c = l2::sin_deg(fx, SINES, a, 1);
            let s = l2::sin_deg(fx, SINES, a, 0);
            let (x, z) = (ee::mul(c, r), ee::mul(s, r));
            fx.mem.setf(sp + lo, x);
            fx.mem.setf(sp + lo + 8, z);
            fx.mem.setf(sp + hi, x);
            fx.mem.setf(sp + hi + 8, z);
        }
        if l2::project(fx, sp + 0x90, SPR, sp + 0xE0, d, sp + 0x120) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x120)) as i32;
            let ot = fx.mem.read_u32(OT_AT);
            fx.submit(d, ot, key);
        }
        s1 = (s1 + 0x1E) as i16 as i32;
    }
    fx.leave(0x1D0);
}

/// The spawn's registers: s0..s4 at the call.
type Regs = [u64; 5];

/// 0x1E042C0(D, step, 0): `spawn::task70` on this record's layout
/// (f55f8bcd1468: 130 tasks, colour +0x9AD4, slot +0x9DFA).
fn spawn(fx: &mut Fx, step: u32, r: Regs, ra: u32) -> i32 {
    let p = spawn::pool70_b4("f55f8bcd1468").expect("layout");
    let saves = [(0x50, u64::from(ra)), (0x40, r[4]), (0x30, r[3]), (0x20, r[2]), (0x10, r[1]), (0, r[0])];
    spawn::task70(fx, D, p, step, 0, &saves)
}

/// 0x1E01C70(D, 0) (frame 0x30: ra, the entry's s1 = 0, s0 = D; s1 = D):
/// the update counter +0x9E02 and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3B08), (0x10, 0), (0, u64::from(D))]);
    let p = param as i64 as u64;
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The emitter's s0: the entry's D, 1 after the update-213 loop.
    let mut r: Regs = [u64::from(D), u64::from(D), p, 0, 0];
    if fx.mem.i16(COUNTER) == 0xD5 {
        let r0: Regs = [0, u64::from(D), p, 0, 0];
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE_A);
        node_position_saving(fx, h, s, D + 0x250, 0, Some(u64::from(D)), 0x01E0_1CE0);
        let v = fx.mem.i16(NEXT);
        fx.mem.set16(SLOT, v);
        spawn(fx, ORB, r0, 0x01E0_1D0C);
        spawn(fx, TUBE, r0, 0x01E0_1D20);
        let v = fx.mem.i16(NEXT) + 1;
        fx.mem.set16(NEXT, v);
        if fx.mem.i16(NEXT) >= 0x14 {
            fx.mem.set16(NEXT, 0);
        }
        r[0] = 1;
    }
    if fx.mem.i16(COUNTER) == 0x3B {
        fx.mem.set16(SLOT, 1);
        spawn(fx, ARC_BIG, r, 0x01E0_1DBC);
        fx.mem.set16(SLOT, 2);
        spawn(fx, ARC_BIG, r, 0x01E0_1DE0);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x3B..0x104).contains(&c) {
        spawn(fx, SPARKS_Z, r, 0x01E0_1E18);
        let c = fx.mem.i16(COUNTER);
        if (0xBF..0xD3).contains(&c) {
            for k in 0xB4..0x100u32 {
                let b = fx.mem.read_u8(BYTES_C + k) as i8;
                let b = if fx.mem.i16(YAW) != 0 { b.wrapping_sub(2) } else { b.wrapping_add(1) };
                fx.mem.write_u8(BYTES_C + k, b as u8);
            }
        }
    }
    if fx.mem.i16(COUNTER) == 1 {
        spawn(fx, LIGHT, r, 0x01E0_1EFC);
    }
    fx.leave(0x30);
}

/// +0x63 to `top` before age +6, then out by 11 (ends below 13).
fn hold_out(fx: &mut Fx, t: u32, top: u8) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, top);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xD {
            fx.mem.write_u8(t + 0x63, a - 0xB);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
}

/// 0x1E01F10 (frame 0x30: s0 = the task, s1 = D): the orb at its slot's
/// centre; see the module notes.
fn orb(fx: &mut Fx, t: u32, slot_at: u32, k: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xF);
        fx.mem.set16(t + 0xA, 0xF);
        fx.mem.set16(t + 0x56, 4);
        grey(fx, t + 0x60, 0x80);
        grey(fx, t + 0x64, 0x80);
        let c = CENTRES + 0x10 * slot(fx, t);
        copy3(fx, t + 0x10, c);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let v = rmod(fx, t, 0x01E0_1FD4, 0x168);
        let i = slot(fx, t);
        fx.mem.set16(ANGLES + 2 * i, v);
        let v = rmod(fx, t, 0x01E0_1FFC, 0xDC) + 0x32;
        let i = slot(fx, t);
        let r = ee::div(ee::from_i32(v), 10.0);
        fx.mem.setf(RADII + 4 * i, r);
    }
    if fx.mem.i16(t + 0x54) < 0x52 {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x63, a - 0xE);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        if a >= 0x10 {
            fx.mem.write_u8(t + 0x67, a - 0xE);
        }
    }
    let g = if odd(fx.mem.i16(t + 0x54)) { 0xFF } else { 0xC0 };
    grey(fx, t + 0x60, g);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2138);
    let r: Regs = [u64::from(t), u64::from(D), u64::from(slot_at), u64::from(k), 0];
    let a = fx.mem.i16(t + 0x54);
    if a >= 0x1B && odd(a) {
        let c = CENTRES + 0x10 * slot(fx, t);
        copy3(fx, c, D + 0x250);
        let mut ra = 0x01E0_21D0u32;
        for f in [MOTE, MOTE, SPARKS, SPARKS, ARC, MOTE_V] {
            spawn(fx, f, r, ra);
            ra += 0x14;
        }
    }
    if fx.mem.i16(t + 0x54) < 0x17 {
        let v = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, v);
        let i = slot(fx, t);
        let v = fx.mem.i16(ANGLES + 2 * i) + 0x12;
        fx.mem.set16(ANGLES + 2 * i, v);
    }
    age(fx, t);
    if fx.mem.i16(t + 0x54) < 0x1A {
        for o in [8u32, 0xA] {
            let v = fx.mem.i16(t + o);
            if v < 0x3C {
                fx.mem.set16(t + o, v + 8);
            }
        }
        let c = CENTRES + 0x10 * slot(fx, t);
        let z = fx.mem.f(c + 8);
        let z = if fx.mem.i16(YAW) != 0 { ee::sub(z, 6.5) } else { ee::add(z, 6.5) };
        fx.mem.setf(c + 8, z);
        let c = CENTRES + 0x10 * slot(fx, t);
        let y = ee::sub(fx.mem.f(c + 4), f32::from_bits(0x3ECC_CCCD));
        fx.mem.setf(c + 4, y);
        let c = CENTRES + 0x10 * slot(fx, t);
        copy3(fx, c, D + 0x250);
        if fx.mem.i16(t + 0x54) < 0x16 {
            spawn(fx, FAN, r, 0x01E0_23E8);
        }
    }
}

/// 0x1E02590 (frame 0x20: s0 = the task): a white spark ring (size 15..44,
/// cell 4) turned at random, radius 350..699 growing by 10..19, row 1..8,
/// life 2..4; alpha 0xC0, then out by 11.
fn sparks(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_25B4, 0x1E) + 0xF;
        fx.mem.setf(t + 0x48, ee::from_i32(v));
        fx.mem.set16(t + 0x56, 4);
        grey(fx, t + 0x60, 0xFF);
        for (o, ra) in [(0x20u32, 0x01E0_25F8u32), (0x24, 0x01E0_2614), (0x28, 0x01E0_2630)] {
            let v = rmod(fx, t, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_2654, 0x15E) + 0x15E;
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, t, 0x01E0_2674, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2690, 0xA) + 0xA;
        fx.mem.set16(t + 0xE, v);
        let v = rand_and(fx, t, 0x01E0_26B0, 7) + 1;
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_26D0, 3) + 2;
        fx.mem.set16(t + 6, v);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_out(fx, t, 0xC0);
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
}

/// 0x1E02770 (frame 0x20: s0 = the task): a fan (size 1 growing 1 a
/// update, height 12, cell rand % 7, 4..6 rows, turned a quarter about z
/// and spinning 0x80 a update about y), in by 12 for 9 updates, then out
/// by 9; flickering grey.
fn fan_step(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        let v = rmod(fx, t, 0x01E0_2798, 7);
        fx.mem.set16(t + 0x56, v);
        fx.mem.set16(t + 0x58, 2);
        let v = rmod(fx, t, 0x01E0_27BC, 3) + 4;
        fx.mem.set16(t + 8, v);
        fx.mem.write_u32(t + 0x34, 0x4140_0000);
        fx.mem.set32(t + 0x28, 0x4000);
    }
    if fx.mem.i16(t + 0x54) < 9 {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0xC);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x67);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x67, a - 9);
        }
    }
    let g = if odd(fx.mem.i16(t + 0x54)) { 0x80 } else { 0xC0 };
    grey(fx, t + 0x60, g);
    grey(fx, t + 0x64, g);
    let s = ee::add(fx.mem.f(t + 0x48), 1.0);
    fx.mem.setf(t + 0x48, s);
    let a = fx.mem.read_u32(t + 0x24).wrapping_add(0x80);
    fx.mem.write_u32(t + 0x24, a);
    age(fx, t);
}

/// 0x1E02920 (frame 0x20: s0 = the task): a violet mote ring (b731's
/// 0x1E02BB0 in (0xCF, 0x80, 0xFF), radius 10..59 growing by 10..24, size
/// 40 growing 11 a update, life 4..7; alpha 0xC0, then out by 11).
fn mote_v(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4220_0000);
        let v = rmod(fx, t, 0x01E0_2948, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0xCF, 0x80, 0xFF);
        let v = rmod(fx, t, 0x01E0_2974, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rand_and(fx, t, 0x01E0_2990, 7);
        fx.mem.set16(t + 0xA, v);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = rmod(fx, t, 0x01E0_29B4, 0x32) + 0xA;
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, t, 0x01E0_29D4, 0xF) + 0xA;
        fx.mem.set16(t + 0xE, v);
        let v = rmod(fx, t, 0x01E0_29F4, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rand_and(fx, t, 0x01E0_2A10, 3) + 4;
        fx.mem.set16(t + 6, v);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_out(fx, t, 0xC0);
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    let s = ee::add(fx.mem.f(t + 0x48), 11.0);
    fx.mem.setf(t + 0x48, s);
    age(fx, t);
}

/// The arcs' violet outer flicker.
fn violet(fx: &mut Fx, t: u32) {
    if odd(fx.mem.i16(t + 0x54)) {
        l2::rgb(fx, t + 0x64, 0xC0, 0x80, 0xFF);
    } else {
        l2::rgb(fx, t + 0x64, 0xA0, 0x70, 0xCF);
    }
}

/// 0x1E02AD0 (frame 0x20: s0 = the task): a white / violet arc (radius 30
/// growing 4..43 a update), in by 0x40 for 2 updates, then out by 11; a
/// quarter turn past the camera's angles.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41F0_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        grey(fx, t + 0x60, 0xFF);
        l2::rgb(fx, t + 0x64, 0xC0, 0x80, 0xFF);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    arc_alpha(fx, t, 2, 0x40);
    violet(fx, t);
    let v = rmod(fx, t, 0x01E0_2BE0, 0x28);
    let g = ee::add(4.0, ee::from_i32(v));
    let s = ee::add(fx.mem.f(t + 0x48), g);
    fx.mem.setf(t + 0x48, s);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2C18);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// +0x63 in by `inc` below 0x80 before age `until`, then out by 11 (ends
/// below 13).
fn arc_alpha(fx: &mut Fx, t: u32, until: i32, inc: u8) {
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < until {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + inc);
        }
    } else if a >= 0xD {
        fx.mem.write_u8(t + 0x63, a - 0xB);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E02C50 (frame 0x60 at `sp`: s3 = D, s2 = the task, the point at sp
/// + 0x50): the orb's tube: its slot's centre pushed into the 70-point
/// history +0x3BB0 + 0x460 k (filled on the first update) until age 34;
/// 20 points growing to 65, size 50, in by 0x40 below 0xC0 for 82 updates,
/// then thinning by 2 until below 0; flickering grey.
fn tube_step(fx: &mut Fx, sp: u32, t: u32) {
    let p = sp + 0x50;
    let c = CENTRES + 0x10 * slot(fx, t);
    copy3(fx, c, p);
    if fx.mem.i16(t + 0x58) == 1 {
        grey(fx, t + 0x64, 0xFF);
        let h = HISTORY + 0x460 * slot(fx, t);
        for j in 0..0x46u32 {
            copy3(fx, p, h + 0x10 * j);
        }
        fx.mem.set16(t + 6, 0x14);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.write_u32(t + 0x48, 0x4248_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x52 {
        let a = fx.mem.read_u8(t + 0x63);
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a + 0x40);
        }
    } else {
        let s = fx.mem.f(t + 0x48);
        if s < 0.0 {
            fx.mem.set16(t + 0x58, 0);
        } else {
            fx.mem.setf(t + 0x48, ee::sub(s, 2.0));
        }
    }
    if fx.mem.i16(t + 0x54) < 0x53 {
        let v = fx.mem.i16(t + 0x5A) + 5;
        fx.mem.set16(t + 0x5A, v);
    }
    if fx.mem.i16(t + 0x54) < 0x23 {
        let k = slot(fx, t);
        history(fx, D + 0x4000 + 0x460 * k, 0x46, p);
    }
    let g = if odd(fx.mem.i16(t + 0x54)) { 0xFF } else { 0xC0 };
    grey(fx, t + 0x60, g);
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 6) < 0x41 {
        let v = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, v);
    }
    age(fx, t);
}

/// 0x1E02EE0 (frame 0x30: s0 = the task, s1 = D): a white spark ring at
/// the attacker's node 3 (size 3..10, cell 4) turned at random about z,
/// radius 150..399 growing by 5..14, row 1..8, life 2..4 (alpha 0xC0, then
/// out by 11); from update 196 its turn grows 0x100 a update.
fn sparks_z(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rand_and(fx, t, 0x01E0_2F0C, 7) + 3;
        fx.mem.setf(t + 0x48, ee::from_i32(v));
        fx.mem.set16(t + 0x56, 4);
        grey(fx, t + 0x60, 0xFF);
        let v = rmod(fx, t, 0x01E0_2F54, 0xFFFF);
        fx.mem.set32(t + 0x28, v);
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_2F78, 0xFA) + 0x96;
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, t, 0x01E0_2F98, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2FB4, 0xA) + 5;
        fx.mem.set16(t + 0xE, v);
        let v = rand_and(fx, t, 0x01E0_2FD4, 7) + 1;
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_2FF4, 3) + 2;
        fx.mem.set16(t + 6, v);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_out(fx, t, 0xC0);
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE_A);
    node_position_saving(fx, h, n, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_3078);
    if fx.mem.i16(COUNTER) >= 0xC4 {
        let v = fx.mem.read_u32(t + 0x28).wrapping_add(0x100);
        fx.mem.write_u32(t + 0x28, v);
    }
    let v = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, v);
    let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, v);
    age(fx, t);
}

/// 0x1E030E0 (frame 0x20: s0 = the task): b550's arc (radius 110, white
/// inside, violet flickering outside) in by 12 below 0x80 for 250
/// updates, then out by 11; a quarter turn past the camera's angles.
fn arc_big(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42DC_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        grey(fx, t + 0x60, 0xDF);
        l2::rgb(fx, t + 0x64, 0xFF, 0x80, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    arc_alpha(fx, t, 0xFA, 0xC);
    violet(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_31F4);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E036C0 (frame 0x20: s0 = the task): the stage light +0x50 from 0x80
/// down by 10 to 0x30 for 320 updates, then back up by 10 to 0x80; at the
/// timer's 3 the light 0x80 and the task ends (b611's 0x1E020D0 by age).
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x54) < 0x140 {
        fx.mem.set16(t + 0x50, if v < 0x31 { 0x30 } else { v - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
    }
    age(fx, t);
    if fx.mem.read_u32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
}
