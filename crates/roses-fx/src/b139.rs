//! BMODULE b139 (card 554), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02330, data area 0x1ED0000) [F]: b228's task program (card 377) on a pool of 150 tasks (0x70
//! bytes at +0x2A0, alive +0x58, step +0x44 through `jalr`; spawned by
//! 0x1E02A60 = `spawn::task70` on POOL70_B4 924e92a3a7b1 (colour +0xBD04,
//! +0x50 from +0xC20A)), drawn with the second library (`uv_rect`,
//! `quad_b_twice`, `ring_facing_flat_z`, `ring_band` and the tube
//! `tubetrail::tube_cam`). The emitter 0x1E01360 counts +0xC212: bands
//! 0x1E02070 on updates 97 / 255 / 315 (`ring_band` at the attacker's node
//! 0x4E, then the target's node 1 growing to 400, then a wide one); a grey
//! ring 0x1E01780 at the target's node 1 every update 230..300; on the odd
//! updates 97..299 three (five from 200) coiling tubes 0x1E01910 (a
//! 15-point history +0x4470 + 0xF0 k of a point circling the attacker's
//! node 0x4E, radius +0xBD1C + 4k growing to +0xBEFC + 4k) each with a
//! glow 0x1E01620 at its head (`quad_b_twice`). Returns 1 on update 240,
//! 3 on 335, 6 on 251..329, 8 once the 370-update timer +0xBD00 has run
//! out.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::spawn::{self, history};
use crate::blib::{billboard, quads4, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const GLOW: u32 = 0x01E0_1620;
const RING: u32 = 0x01E0_1780;
const TUBE: u32 = 0x01E0_1910;
const BAND: u32 = 0x01E0_2070;
/// The return address of the entry's `jalr` into a step (0x1E02628 + 8).
const TASK_RA: u64 = 0x01E0_2630;
const HISTORY: u32 = D + 0x4470;
const TEX: u32 = D + 0xBCE0;
const TIMER: u32 = D + 0xBD00;
const RADIUS: u32 = D + 0xBD1C;
const RADIUS_TO: u32 = D + 0xBEFC;
const NODE_A: u32 = D + 0xC0EC;
const TNODE: u32 = D + 0xC0FA;
const ANGLES: u32 = D + 0xC10C;
const SLOT: u32 = D + 0xC20A;
const NEXT: u32 = D + 0xC20C;
const YAW: u32 = D + 0xC20E;
const COUNTER: u32 = D + 0xC212;
const BYTES: u32 = D + 0xC214;
const BYTES_B: u32 = D + 0xC314;
const SINES: u32 = D + 0xC414;

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// rand (the caller's s0 = `s0`) from `ra`, mod `m` (truncated, `div`).
fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn odd(n: i32) -> bool {
    n % 2 == 1
}

fn slot(fx: &mut Fx, t: u32) -> u32 {
    fx.mem.i16(t + 0x50) as u32
}

/// The entry 0x1E02330 (a0 = D, a1 = param; s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    fx.mem.write_u32(D + 0xBD0C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xA0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xC580);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xC210, param);
    fx.mem.set32(TIMER, 0x172);
    for (at, n, clear) in [(NODE_A, 0x4E, 1), (D + 0xC0EE, 0x1C, 0), (D + 0xC0F0, 0xD, 0)] {
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
    let tex = fx.texture(5, 0);
    fx.mem.write_u32(TEX, tex);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x90);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0x94)), 180.0);
    fx.fix_residue(v, 0x01E0_24CC);
    fx.mem.set16(YAW, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_24FC, 0x1E);
        fx.mem.write_u8(BYTES + k, (v - 0xF) as u8);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_2554);
        fx.mem.write_u8(BYTES + k, v.wrapping_rem(4) as u8);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_25AC, 0x28);
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
        // The steps' prologues: the entry's s0 = D, s1 = 0, s2 = the
        // slot, s3 = k.
        let frame = match f {
            TUBE => 0x70,
            BAND => 0x30,
            GLOW | RING => 0x20,
            _ => panic!("b139: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match f {
            TUBE => {
                fx.saved(s, &[(0x50, TASK_RA), (0x40, u64::from(k)), (0x30, u64::from(slot)), (0x20, 0), (0x10, u64::from(D))]);
                // swc1 f20: the runner's (0) [H].
                fx.mem.write_u32(s, 0);
            }
            BAND => fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
        }
        match f {
            GLOW => glow(fx, t),
            RING => ring(fx, t),
            TUBE => tube_step(fx, s, t),
            _ => band(fx, t),
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
        0xF0 => 1,
        0x14F => 3,
        0xFB..=0x149 => 6,
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

/// The entry's draw of one live task by its step (s0 = D, s1 = the live
/// tasks counted so far, this one included).
fn draw(fx: &mut Fx, sp: u32, t: u32, drawn: u64) {
    let f = fx.mem.read_u32(t + 0x44);
    if f == GLOW {
        cell(fx, t, 0x14, sp + 0x60);
        texture(fx, TEX);
        fx.mem.set16(D + 0xBE, 0);
        for o in 0..3 {
            let h = HISTORY + 0xF0 * slot(fx, t);
            let v = fx.mem.read_u32(h + 4 * o);
            fx.mem.write_u32(sp + 0x50 + 4 * o, v);
        }
        billboard::quad_b_twice(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
    }
    if f == RING {
        cell(fx, t, 0x14, sp + 0x70);
        texture(fx, D + 0x260);
        fx.mem.set16(D + 0xB8, 4);
        let v = fx.mem.i16(t + 0xA);
        fx.mem.set16(D + 0xC0, v);
        for o in 0..3 {
            let v = fx.mem.i16(t + 2 * o);
            fx.mem.setf(D + 0x20 + 4 * o, ee::from_i32(v));
        }
        quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES_B);
        fx.mem.set16(D + 0xB8, 0);
    }
    if f == TUBE {
        texture(fx, D + 0x29C);
        cell(fx, t, 0x97, sp + 0x80);
        let h2 = fx.mem.i16(D + 0x9A) >> 1;
        fx.mem.set16(D + 0x9A, h2);
        let v = fx.mem.i16(D + 0x96) + fx.mem.i16(D + 0x9A);
        fx.mem.set16(D + 0x96, v);
        let h = fx.self_handle();
        unit_position_saving(fx, h, sp + 0x50, u64::from(D), 0x01E0_2880);
        for o in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * o);
            fx.mem.write_u32(D + 0x20 + 4 * o, w);
        }
        fx.mem.set16(D + 0xBE, 1);
        let k = slot(fx, t);
        tubetrail::tube_cam(fx, D, t, HISTORY + 0xF0 * k, 0);
    }
    if f == BAND {
        let s1 = Some(drawn);
        match fx.mem.i16(t + 0x50) {
            0 => {
                let h = fx.self_handle();
                let n = fx.mem.i16(NODE_A);
                node_position_saving(fx, h, n, sp + 0x50, u64::from(D), s1, 0x01E0_2908);
            }
            1 => {
                let h = fx.target_handle();
                let n = fx.mem.i16(TNODE);
                node_position_saving(fx, h, n, sp + 0x50, u64::from(D), s1, 0x01E0_2938);
            }
            2 => {
                let h = fx.target_handle();
                let n = fx.mem.i16(TNODE);
                node_position_saving(fx, h, n, sp + 0x50, u64::from(D), s1, 0x01E0_2968);
            }
            _ => {}
        }
        quads4::ring_band(fx, D, t, 0x21, sp + 0x50);
    }
    if f != GLOW && f != RING && f != TUBE && f != BAND && f != 0 {
        panic!("b139: unported draw case {f:#x}");
    }
}

/// The spawn's registers: s0..s4 at the call.
type Regs = [u64; 5];

/// 0x1E02A60(D, step, 0): `spawn::task70` on this record's layout
/// (924e92a3a7b1: 150 tasks, colour +0xBD04, slot +0xC20A).
fn spawn(fx: &mut Fx, step: u32, r: Regs, ra: u32) -> i32 {
    let p = spawn::pool70_b4("924e92a3a7b1").expect("layout");
    let saves = [(0x50, u64::from(ra)), (0x40, r[4]), (0x30, r[3]), (0x20, r[2]), (0x10, r[1]), (0, r[0])];
    spawn::task70(fx, D, p, step, 0, &saves)
}

/// 0x1E01360(D, 0) (frame 0x50: ra, the entry's s2 = param, s1 = 0, s0 =
/// D): the update counter +0xC212, the bands, rings and tubes.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2600), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let r: Regs = [u64::from(D), 0, u64::from(D), 0, 0];
    for (c, s, ra) in [(0x61, 0, 0x01E0_13C4u32), (0xFF, 1, 0x01E0_1400), (0x13B, 2, 0x01E0_143C)] {
        if fx.mem.i16(COUNTER) == c {
            fx.mem.set16(SLOT, s);
            spawn(fx, BAND, r, ra);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0xE6..0x12D).contains(&c) {
        fx.mem.set16(SLOT, 0);
        let h = fx.target_handle();
        let n = fx.mem.i16(TNODE);
        node_position_saving(fx, h, n, sp + 0x40, u64::from(D), Some(0), 0x01E0_1488);
        for o in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * o);
            fx.mem.write_u32(D + 0x250 + 4 * o, w);
        }
        spawn(fx, RING, r, 0x01E0_14B4);
    }
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE_A);
    node_position_saving(fx, h, n, D + 0xBC70, u64::from(D), Some(0), 0x01E0_14D8);
    let h = fx.self_handle();
    unit_position_saving(fx, h, D + 0x250, u64::from(D), 0x01E0_14EC);
    let c = fx.mem.i16(COUNTER);
    if (0x61..0x12D).contains(&c) && odd(c) {
        let count = if c < 0xC8 { 3 } else { 5 };
        for j in 0..count {
            let r: Regs = [j as u64, count as u64, u64::from(D), 0, 0];
            let v = fx.mem.i16(NEXT);
            fx.mem.set16(SLOT, v);
            spawn(fx, TUBE, r, 0x01E0_1584);
            spawn(fx, GLOW, r, 0x01E0_1598);
            let n = fx.mem.i16(NEXT) + 1;
            fx.mem.set16(NEXT, n);
            if fx.mem.i16(NEXT) >= 0x78 {
                fx.mem.set16(NEXT, 0);
            }
        }
    }
    fx.leave(0x50);
}

/// 0x1E01620 (frame 0x20: s0 = the task): a tube's glow (size 10, grey),
/// in by 0x2A for 27 updates, then out by 0x22 twice, flickering, facing
/// the camera.
fn glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 0xA);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x1B {
        for o in [0x63u32, 0x67] {
            let a = fx.mem.read_u8(t + o);
            if a < 0x80 {
                fx.mem.write_u8(t + o, a + 0x2A);
            }
        }
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x24 {
            fx.mem.write_u8(t + 0x63, a - 0x22);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0x24 {
            fx.mem.write_u8(t + 0x63, a - 0x22);
        }
    }
    let g = if odd(fx.mem.i16(t + 0x54)) { 0x80 } else { 0x60 };
    l2::rgb(fx, t + 0x60, g, g, g);
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_1758);
    age(fx, t);
}

/// 0x1E01780 (frame 0x20: s0 = the task): a dark ring (size 30 growing 8 a
/// update, 4 quads by `ring_facing_flat_z`) rising 1 a update, radius rand
/// % 100 + 100 growing by rand % 30 + 16, life rand % 5 + 3.
fn ring(fx: &mut Fx, t: u32) {
    let s0 = t;
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41F0_0000);
        let v = rmod(fx, s0, 0x01E0_17A8, 7);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x20);
        let v = fx.rand_from(s0, 0x01E0_17D4) & 7;
        fx.mem.set16(t + 0xA, v);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        let v = rmod(fx, s0, 0x01E0_17F8, 0x64) + 0x64;
        fx.mem.set16(t + 0x50, v);
        let v = rmod(fx, s0, 0x01E0_1818, 0x1E) + 0x10;
        fx.mem.set16(t + 0xE, v);
        let v = rmod(fx, s0, 0x01E0_1838, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, s0, 0x01E0_1854, 5) + 3;
        fx.mem.set16(t + 6, v);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 0xA {
            fx.mem.write_u8(t + 0x63, a - 8);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let s = ee::add(fx.mem.f(t + 0x48), 8.0);
        fx.mem.setf(t + 0x48, s);
    }
    let y = fx.mem.i16(t + 2) + 1;
    fx.mem.set16(t + 2, y);
    age(fx, t);
}

/// 0x1E01910 (frame 0x70 at `sp`: s3 = D, s2 = the task, f20): a coiling
/// tube: its head circles the attacker's node 0x4E (radius +0x2C =
/// +0xBD1C + 4k scaled by cos +0x5C, at angles +0xC10C + 2k / +0x5A) and
/// is pushed into the 15-point history +0x4470 + 0xF0 k; 2 points growing
/// to 7; in by 0x15 for 10 (17 from update 191) updates, then out; from
/// update 191 the centre flies 5 along z by the facing and the width grows.
fn tube_step(fx: &mut Fx, sp: u32, t: u32) {
    let (pt_x, pt_y, pt_z) = (sp + 0x60, sp + 0x64, sp + 0x68);
    let mut s0 = u64::from(D);
    let mut s1 = 0u64;
    if fx.mem.i16(t + 0x58) == 1 {
        let r = rmod(fx, D, 0x01E0_1948, 0x32);
        let v = fx.mem.i16(D + 0xC206) + r + 5;
        fx.mem.set16(t + 0xA, v);
        let r = fx.rand_from(D, 0x01E0_1970) & 1;
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 8, r);
        if fx.mem.i16(COUNTER) < 0xC8 {
            let v = rmod(fx, D, 0x01E0_19BC, 0x168);
            fx.mem.set16(t + 0x5C, v);
        } else {
            let v = if fx.mem.i16(YAW) != 0 { 0 } else { 0x5A };
            fx.mem.set16(t + 0x5C, v);
        }
        let v = rmod(fx, D, 0x01E0_1A04, 0x168);
        let k = slot(fx, t);
        fx.mem.set16(ANGLES + 2 * k, v);
        if fx.mem.i16(COUNTER) < 0xC8 {
            fx.mem.write_u32(RADIUS_TO + 4 * k, 0x41F0_0000);
        } else {
            let v = rmod(fx, D, 0x01E0_1A68, 0x12C);
            let k = slot(fx, t);
            let r = ee::div(ee::from_i32(v), 10.0);
            fx.mem.setf(RADIUS_TO + 4 * k, r);
        }
        let k = slot(fx, t);
        fx.mem.write_u32(RADIUS + 4 * k, 0);
        let a = fx.mem.i16(t + 0x5C);
        let c = l2::sin_deg(fx, SINES, a, 1);
        let v = ee::mul(0.0, c);
        fx.fix_residue(v, 0x01E0_1AFC);
        fx.mem.set32(t + 0x2C, ee::fixsfsi(v));
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE_A);
        node_position_saving(fx, h, n, pt_x, s0, Some(s1), 0x01E0_1B1C);
        fx.mem.set16(t + 0x5A, 0x5A);
        coil_add(fx, t, pt_x);
        s0 = u64::from(pt_y);
        s1 = u64::from(pt_z);
        let k = slot(fx, t);
        let h = HISTORY + 0xF0 * k;
        for j in 0..0xFu32 {
            for o in 0..3 {
                let w = fx.mem.read_u32(pt_x + 4 * o);
                fx.mem.write_u32(h + 0x10 * j + 4 * o, w);
            }
        }
        fx.mem.set16(t + 6, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 8, 0);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.write_u32(t + 0x48, 0x4020_0000);
        fx.mem.set16(t + 0x58, 2);
        for (o, ra) in [(0x20u32, 0x01E0_1C38u32), (0x24, 0x01E0_1C54), (0x28, 0x01E0_1C70)] {
            let v = rmod(fx, pt_y, ra, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE_A);
        node_position_saving(fx, h, n, t + 0x10, s0, Some(s1), 0x01E0_1CA4);
    }
    let late = fx.mem.i16(COUNTER) >= 0xBF;
    if !late {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE_A);
        node_position_saving(fx, h, n, t + 0x10, s0, Some(s1), 0x01E0_1CDC);
    } else if fx.mem.i16(t + 0x54) >= 2 {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(YAW) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(t + 0x18, z);
        let s = ee::add(fx.mem.f(t + 0x48), f32::from_bits(0x3E99_999A));
        fx.mem.setf(t + 0x48, s);
    }
    let until = if fx.mem.i16(COUNTER) < 0xBF { 0xA } else { 0x11 };
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < until {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0x15);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let v = fx.mem.i16(t + 0x5A) + 0xC;
    fx.mem.set16(t + 0x5A, v);
    let v = fx.mem.i16(t + 0x5A);
    if v >= 0x168 {
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
    }
    // sp + 0x64 += i16 +8 (overwritten below).
    let y = ee::add(fx.mem.f(pt_y), ee::from_i32(fx.mem.i16(t + 8)));
    fx.mem.setf(pt_y, y);
    coil_set(fx, t, pt_x);
    for o in 0..3 {
        let v = ee::add(fx.mem.f(pt_x + 4 * o), fx.mem.f(t + 0x10 + 4 * o));
        fx.mem.setf(pt_x + 4 * o, v);
    }
    let k = slot(fx, t);
    history(fx, D + 0x4550 + 0xF0 * k, 0xF, pt_x);
    let (r, g, b) = if odd(fx.mem.i16(t + 0x54)) { (0x80, 0x20, 0x40) } else { (0x70, 0x10, 0x30) };
    l2::rgb(fx, t + 0x60, r, g, b);
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 6) < 7 {
        let v = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, v);
    }
    let a = fx.mem.i16(t + 0x5C);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let k = slot(fx, t);
    let v = ee::mul(fx.mem.f(RADIUS + 4 * k), c);
    fx.fix_residue(v, 0x01E0_1F94);
    fx.mem.set32(t + 0x2C, ee::fixsfsi(v));
    let v = fx.mem.i16(t + 0x5C) + 6;
    fx.mem.set16(t + 0x5C, v);
    let k = slot(fx, t);
    let v = fx.mem.i16(ANGLES + 2 * k) + 6;
    fx.mem.set16(ANGLES + 2 * k, v);
    if fx.mem.i16(t + 0x54) < 0xC {
        let k = slot(fx, t);
        let r = fx.mem.f(RADIUS + 4 * k);
        if r < fx.mem.f(RADIUS_TO + 4 * k) {
            fx.mem.setf(RADIUS + 4 * k, ee::add(r, 2.5));
        }
    }
    age(fx, t);
}

/// The coil offsets f20 = +0x2C: y += f20 sin(+0xC10C + 2k), z += f20
/// sin(+0x5A), x += f20 cos(+0x5A) (the first update's form).
fn coil_add(fx: &mut Fx, t: u32, p: u32) {
    let f20 = ee::from_i32(fx.mem.i32(t + 0x2C));
    let k = slot(fx, t);
    let a = fx.mem.i16(ANGLES + 2 * k);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let y = ee::add(fx.mem.f(p + 4), ee::mul(f20, s));
    fx.mem.setf(p + 4, y);
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let z = ee::add(fx.mem.f(p + 8), ee::mul(f20, s));
    fx.mem.setf(p + 8, z);
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    let x = ee::add(fx.mem.f(p), ee::mul(f20, c));
    fx.mem.setf(p, x);
}

/// The coil offsets set (not added): y = f20 sin(+0xC10C + 2k), z = f20
/// sin(+0x5A), x = f20 cos(+0x5A).
fn coil_set(fx: &mut Fx, t: u32, p: u32) {
    let f20 = ee::from_i32(fx.mem.i32(t + 0x2C));
    let k = slot(fx, t);
    let a = fx.mem.i16(ANGLES + 2 * k);
    let s = l2::sin_deg(fx, SINES, a, 0);
    fx.mem.setf(p + 4, ee::mul(f20, s));
    let a = fx.mem.i16(t + 0x5A);
    let s = l2::sin_deg(fx, SINES, a, 0);
    fx.mem.setf(p + 8, ee::mul(f20, s));
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, SINES, a, 1);
    fx.mem.setf(p, ee::mul(f20, c));
}

/// 0x1E02070 (frame 0x30: s0 = the task, s1 = D): a band (b228's
/// 0x1E024B0) by +0x50: 0 at the attacker's node 0x4E and 1 at the
/// target's node 1 (inner radius 40, the second growing 15 a update to
/// 400; in by 0x10 for 12 updates, flickering violet outside, out from
/// update 321), 2 a wide one at the target (radius 840, white / brown, in
/// by 0x20 below 0xC0 for 13 updates, then out); a quarter turn past the
/// camera's angles.
fn band(fx: &mut Fx, t: u32) {
    let which = |fx: &mut Fx| fx.mem.i16(t + 0x50);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4220_0000);
        if which(fx) == 2 {
            fx.mem.write_u32(t + 0x48, 0x4452_0000);
        }
        if which(fx) < 2 {
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
            l2::rgb(fx, t + 0x64, 0xF0, 0x80, 0x10);
        } else {
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
            l2::rgb(fx, t + 0x64, 0x80, 0x40, 0x10);
        }
        let v = rmod(fx, t, 0x01E0_2124, 0x168);
        fx.mem.set16(t + 0x5A, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if which(fx) < 2 && fx.mem.i16(t + 0x54) < 0xC {
        let a = fx.mem.read_u8(t + 0x63);
        fx.mem.write_u8(t + 0x63, a.wrapping_add(0x10));
    }
    if which(fx) == 2 {
        let a = fx.mem.read_u8(t + 0x63);
        if fx.mem.i16(t + 0x54) < 0xD {
            if a < 0xC0 {
                fx.mem.write_u8(t + 0x63, a + 0x20);
            }
        } else if a >= 0xB {
            fx.mem.write_u8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if which(fx) < 2 {
        let (r, g, b) = if odd(fx.mem.i16(t + 0x54)) { (0xF0, 8, 0x80) } else { (0x80, 8, 0x40) };
        l2::rgb(fx, t + 0x64, r, g, b);
    }
    let late_out = |fx: &mut Fx| {
        if fx.mem.i16(COUNTER) >= 0x141 {
            let a = fx.mem.read_u8(t + 0x63);
            if a >= 0x11 {
                fx.mem.write_u8(t + 0x63, a - 0xF);
            } else {
                fx.mem.set16(t + 0x58, 0);
            }
        }
    };
    if which(fx) == 1 {
        let r = fx.mem.f(t + 0x48);
        if r < 400.0 {
            fx.mem.setf(t + 0x48, ee::add(r, 15.0));
        }
        late_out(fx);
    }
    if which(fx) == 0 {
        late_out(fx);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2300);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}
