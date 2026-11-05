//! BMODULE b524 (card 91), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02A60, data area 0x1ED0000) [F]: a pool of 200 tasks (0x70 bytes at D+0x2A0, alive +0x58, step
//! +0x44 through `jalr`, the stub 0x1E02A50 skipped) spawned by 0x1E034A0
//! (`spawn::task70`: colour D+0x5DB4, +0x50 from D+0x5EE6) and drawn with
//! the second library (uv_rect 0x1E00080, `billboard::ring_quads`
//! 0x1E00920, `billboard::quad` 0x1E00ED0, `quads4::sparks` 0x1E01190,
//! `billboard::arc_strips` 0x1E01590). The emitter 0x1E018F0 counts
//! D+0x5EEE: on the odd updates 0x3C..0x96 a white spark ring 0x1E02170 at
//! the attacker's node 0x1B (height 40); on 0x3B..0x78 three fliers
//! 0x1E02710 a update from node 0x1C towards node 0x1B; on the odd updates
//! 100..124 at the target's node 1 (jittered by -8..7) a falling orb
//! 0x1E01B30 (index i = D+0x5EE8 into the vectors D+0x5A50 + 16 i, a
//! random drift / 15 times its countdown +0x52 from 50) with its arc ring
//! 0x1E025A0; the orb sheds rings 0x1E01FE0 on its odd ages and once its
//! countdown runs out (age 0x3D on) a burst ring 0x1E023F0 and a growing
//! quad 0x1E02910. The orb's draw (inline in the entry) swings it by
//! +0x48 sin(+0x52) and on its age 1 picks its UV orientation +0xC by the
//! camera eye's x and the target's z (0x1F71A0, 0x1FACE0 / 0x1FAF40).
//! Returns 1 on update 0xAA, 4 on 0xE1, 2 on 0xD7, 6 on 0xB5..0xD1, 8 once
//! the 240-update timer +0x5DB0 has run out.
//!
//! The step 0x1E022A0 is never spawned [F: every `addiu` of 0x22A0 is the
//! entry's draw compare]; only its draw branch is ported.
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b494::camera_angles_saving;
use crate::blib::spawn::{self, Pool70};
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
const POOL: Pool70 = Pool70 { base: 0x2A0, n: TASKS, colour: 0x5DB4, pos: 0x250, slot: 0x5EE6 };
/// The step functions a task holds (+0x44), as the record's addresses.
const ORB: u32 = 0x01E0_1B30;
const RING: u32 = 0x01E0_1FE0;
const SPARKS: u32 = 0x01E0_2170;
const UNSPAWNED: u32 = 0x01E0_22A0;
const BURST: u32 = 0x01E0_23F0;
const ARC: u32 = 0x01E0_25A0;
const FLIER: u32 = 0x01E0_2710;
const GLOW: u32 = 0x01E0_2910;
const STUB: u32 = 0x01E0_2A50;
/// The return address of the entry's `jalr` into a step (0x1E02D48 + 8).
const TASK_RA: u64 = 0x01E0_2D50;
const VECTORS: u32 = D + 0x5A50;
const TEX: u32 = D + 0x5D90;
const TIMER: u32 = D + 0x5DB0;
const OT_AT: u32 = D + 0x5DBC;
const EYE: u32 = D + 0x5DCC;
const NODE_A: u32 = D + 0x5DF0;
const NODE_B: u32 = D + 0x5DF2;
const TNODE: u32 = D + 0x5E00;
const SCALES: u32 = D + 0x5E10;
const ANGLES: u32 = D + 0x5E74;
const SLOT: u32 = D + 0x5EE6;
const NEXT: u32 = D + 0x5EE8;
const FACING: u32 = D + 0x5EEA;
const COUNTER: u32 = D + 0x5EEE;
const BYTES_B: u32 = D + 0x5FF0;
const SINES: u32 = D + 0x60F0;

/// The spawn's registers: s0..s4 at the call.
type Regs = [u64; 5];

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

fn grey(fx: &mut Fx, at: u32, g: i32) {
    l2::rgb(fx, at, g, g, g);
}

fn copy(fx: &mut Fx, from: u32, to: u32, n: u32) {
    for k in 0..n {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The entry 0x1E02A60 (a0 = D, a1 = param; s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x6260);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set32(TIMER, 0xF0);
    fx.mem.set16(D + 0x5EEC, param);
    for (k, n) in [0x1B, 0x1C, 0x35, 0x30, 0x2F, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODE_A + 2 * k as u32, r);
    }
    let t = fx.target_handle();
    let r = fx.request_node(t, 1, 1);
    fx.mem.set16(TNODE, r);
    let tex = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, tex);
    for k in 0..4u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(v, 0x01E0_2BF8);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2C20, 0x1E);
        fx.mem.set8(D + 0x5EF0 + k, v - 0xF);
    }
    for k in 100..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_2C74);
        fx.mem.set8(D + 0x5EF0 + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2CC4, 0x28);
        fx.mem.set8(BYTES_B + k, v - 0x14);
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
        if f == 0 || f == STUB {
            continue;
        }
        // The steps' prologues: the entry's s0 = D, s1 = 0.
        let frame = match f {
            FLIER => 0x50,
            ORB | ARC => 0x30,
            RING | SPARKS | BURST | GLOW => 0x20,
            _ => panic!("b524: unknown task step {f:#x}"),
        };
        let s = fx.enter(frame);
        match frame {
            0x50 | 0x30 => fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]),
            _ => fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]),
        }
        match f {
            ORB => orb(fx, t, [u64::from(t), u64::from(D), u64::from(slot), u64::from(k), 0]),
            RING => ring(fx, t),
            SPARKS => sparks(fx, t),
            BURST => burst(fx, t),
            ARC => arc(fx, t),
            FLIER => flier(fx, s, t),
            _ => glow(fx, t),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        let t = slot + 0x2A0;
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
        0xAA => 1,
        0xE1 => 4,
        0xD7 => 2,
        0xB5..=0xD1 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell 0x14 / +0x56 (0x106960 into `r`) copied to D+0x94..+0xA3,
/// with the texture word `tex` in D+0x88.
fn cell(fx: &mut Fx, t: u32, tex: u32, r: u32) {
    let w = fx.mem.read_u32(tex);
    fx.mem.write_u32(D + 0x88, w);
    let sub = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, 0x14, sub);
    copy(fx, r, D + 0x94, 4);
}

/// D+0x94..+0x9A: the 64-wide cell column +0x56 of a 64 x 64 (`h`) cell.
fn column(fx: &mut Fx, t: u32, wh: i32) {
    let sub = fx.mem.i16(t + 0x56);
    fx.mem.set16(D + 0x94, sub << 6);
    fx.mem.set16(D + 0x96, 0);
    fx.mem.set16(D + 0x98, wh);
    fx.mem.set16(D + 0x9A, wh);
}

/// `out` = +0x10 + (`r` sin(D+0x5E74[i]), 0, 0) + D+0x5A50[i] `m`
/// (i = +0x50): a swung, drifted position.
fn swung(fx: &mut Fx, t: u32, out: u32, r: f32, m: f32) {
    let i = fx.mem.i16(t + 0x50) as u32;
    let a = fx.mem.i16(ANGLES + 2 * i);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let x = ee::add(fx.mem.f(t + 0x10), ee::mul(r, s));
    fx.mem.setf(out, x);
    let y = fx.mem.f(t + 0x14);
    fx.mem.setf(out + 4, y);
    let z = fx.mem.f(t + 0x18);
    fx.mem.setf(out + 8, z);
    for c in 0..3 {
        let v = ee::add(fx.mem.f(out + 4 * c), ee::mul(fx.mem.f(VECTORS + 16 * i + 4 * c), m));
        fx.mem.setf(out + 4 * c, v);
    }
}

/// A ring of quads (`ring_quads` on D+0x5FF0, a random UV orientation
/// from the entry's rand returning to `ra`), cell from texture D+0x5D94.
fn rings(fx: &mut Fx, t: u32, r: u32, size: i32, ra: u32) {
    cell(fx, t, D + 0x5D94, r);
    fx.mem.set16(D + 0xB8, size);
    let v = fx.rand_from(D, ra);
    fx.mem.set16(D + 0xBE, v % 4);
    fx.mem.set16(D + 0xB0, 0);
    billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES_B);
    fx.mem.set16(D + 0xB8, 0);
}

/// The entry's draw of one live task (frame locals at `sp`).
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let p = sp + 0x60;
    if fx.mem.read_u32(t + 0x44) == ORB {
        cell(fx, t, TEX, sp + 0x80);
        column(fx, t, 0x40);
        let r = fx.mem.f(t + 0x48);
        let m = ee::from_i32(fx.mem.i16(t + 0x52));
        swung(fx, t, p, r, m);
        if fx.mem.i16(t + 0x54) == 1 {
            let h = fx.target_handle();
            unit_position_saving(fx, h, sp + 0x70, u64::from(D), 0x01E0_2EDC);
            let eye = fx.camera_eye(0);
            fx.mem.write_u32(EYE, eye);
            let at = fx.camera_at30(0);
            fx.mem.write_u32(D + 0x5DD0, at);
            let turned = fx.mem.i16(FACING) != 0;
            let z = fx.mem.f(sp + 0x78);
            let z = if turned { ee::sub(z, 30.0) } else { ee::add(z, 30.0) };
            fx.mem.setf(sp + 0x78, z);
            let e = fx.mem.read_u32(EYE);
            let ex = fx.mem.f(e);
            let near = fx.mem.f(sp + 0x78) <= fx.mem.f(p + 8);
            let flip = if turned { ex < 0.0 } else { ex <= 0.0 };
            fx.mem.set16(t + 0xC, if near == flip { 2 } else { 0 });
        }
        let o = fx.mem.i16(t + 0xC);
        fx.mem.set16(D + 0xBE, o);
        billboard::quad(fx, uv::uv_rect, D, t, 0x20, p);
    }
    if fx.mem.read_u32(t + 0x44) == RING {
        rings(fx, t, sp + 0x90, 4, 0x01E0_30A0);
    }
    if fx.mem.read_u32(t + 0x44) == SPARKS {
        let w = fx.mem.read_u32(D + 0x5D98);
        fx.mem.write_u32(D + 0x88, w);
        for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x1F), (0x9A, 0x3F), (0xB8, 0)] {
            fx.mem.set16(D + o, v);
        }
        copy(fx, t + 0x10, D + 0x20, 3);
        fx.mem.set16(D + 0xB8, 0x24);
        fx.mem.set16(D + 0xBE, 0);
        quads4::sparks(fx, D, t, 0x20, t + 0x10);
    }
    if fx.mem.read_u32(t + 0x44) == UNSPAWNED {
        fx.mem.set16(D + 0xB8, 0x24);
        billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    if fx.mem.read_u32(t + 0x44) == BURST {
        rings(fx, t, sp + 0xA0, 0xC, 0x01E0_31E0);
    }
    if fx.mem.read_u32(t + 0x44) == ARC {
        fx.mem.set16(D + 0xB8, 0x24);
        let i = fx.mem.i16(t + 0x50) as u32;
        let m = ee::from_i32(fx.mem.i16(SCALES + 2 * i));
        swung(fx, t, p, 5.0, m);
        billboard::arc_strips(fx, D, t, 0x21, p);
        fx.mem.set16(D + 0xB8, 0);
    }
    if fx.mem.read_u32(t + 0x44) == FLIER {
        cell(fx, t, D + 0x5D94, sp + 0xB0);
        fx.mem.set16(D + 0xBE, 0);
        billboard::quad(fx, uv::uv_rect, D, t, 0x20, t + 0x10);
    }
    if fx.mem.read_u32(t + 0x44) == GLOW {
        let w = fx.mem.read_u32(D + 0x5D9C);
        fx.mem.write_u32(D + 0x88, w);
        column(fx, t, 0x3F);
        fx.mem.set16(D + 0xBE, 0);
        billboard::quad(fx, uv::uv_rect, D, t, 0x21, t + 0x10);
    }
}

/// 0x1E018F0(D, 0) (frame 0x20: ra, the entry's s0 = D): the update
/// counter +0x5EEE and the spawns.
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, 0x01E0_2D10), (0, u64::from(D))]);
    let r: Regs = [u64::from(D), 0, param as i64 as u64, 0, 0];
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let n = fx.mem.i16(COUNTER);
    if (100..0x7D).contains(&n) && odd(n) {
        let h = fx.target_handle();
        let s = fx.mem.i16(TNODE);
        node_position_saving(fx, h, s, D + 0x250, u64::from(D), Some(0), 0x01E0_195C);
        for (o, ra) in [(0x250u32, 0x01E0_1964u32), (0x254, 0x01E0_1998), (0x258, 0x01E0_19CC)] {
            let v = fx.rand_from(D, ra) % 16 - 8;
            let x = ee::add(fx.mem.f(D + o), ee::from_i32(v));
            fx.mem.setf(D + o, x);
        }
        let v = fx.mem.i16(NEXT);
        fx.mem.set16(SLOT, v);
        spawn(fx, ORB, r, 0x01E0_1A14);
        spawn(fx, ARC, r, 0x01E0_1A28);
        let v = fx.mem.i16(NEXT) + 1;
        fx.mem.set16(NEXT, v);
    }
    let n = fx.mem.i16(COUNTER);
    if n >= 0x3C && odd(n) && n < 0x97 {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE_A);
        node_position_saving(fx, h, s, D + 0x250, u64::from(D), Some(0), 0x01E0_1A88);
        fx.mem.write_u32(D + 0x254, 0x4220_0000);
        fx.mem.set16(SLOT, 0);
        spawn(fx, SPARKS, r, 0x01E0_1AA8);
    }
    let n = fx.mem.i16(COUNTER);
    if (0x3B..0x79).contains(&n) {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE_B);
        node_position_saving(fx, h, s, D + 0x250, u64::from(D), Some(0), 0x01E0_1ADC);
        for ra in [0x01E0_1AF0u32, 0x01E0_1B04, 0x01E0_1B18] {
            spawn(fx, FLIER, r, ra);
        }
    }
    fx.leave(0x20);
}

/// 0x1E034A0(D, step, 0): `spawn::task70` on this record's layout.
fn spawn(fx: &mut Fx, step: u32, r: Regs, ra: u32) -> i32 {
    let saves = [(0x50, u64::from(ra)), (0x40, r[4]), (0x30, r[3]), (0x20, r[2]), (0x10, r[1]), (0, r[0])];
    spawn::task70(fx, D, POOL, step, 0, &saves)
}

/// Alpha byte `a`: + `by` below `cap`.
fn up(fx: &mut Fx, a: u32, cap: u8, by: u8) {
    let v = fx.mem.read_u8(a);
    if v < cap {
        fx.mem.write_u8(a, v.wrapping_add(by));
    }
}

/// Alpha byte `a`: - `by` from `floor` up, else the task ends (`end`).
fn down(fx: &mut Fx, t: u32, a: u32, floor: u8, by: u8, end: bool) {
    let v = fx.mem.read_u8(a);
    if v >= floor {
        fx.mem.write_u8(a, v.wrapping_sub(by));
    } else if end {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// D+0x250 = +0x10.. plus D+0x5A50[i] times `m` (i = +0x50).
fn drift_point(fx: &mut Fx, t: u32) {
    copy(fx, t + 0x10, D + 0x250, 4);
    let m = ee::from_i32(fx.mem.i16(t + 0x52));
    for c in 0..3 {
        let i = fx.mem.i16(t + 0x50) as u32;
        let v = ee::add(fx.mem.f(D + 0x250 + 4 * c), ee::mul(fx.mem.f(VECTORS + 16 * i + 4 * c), m));
        fx.mem.setf(D + 0x250 + 4 * c, v);
    }
}

/// 0x1E01B30: the falling orb (frame 0x30, s1 = D): size 20 growing to 30
/// every third update, swing 5, cell 2..7 cycling 0..2, a random drift
/// D+0x5A50[i] / 15 times its countdown +0x52 (50, down by 1 a update to
/// 20, then from age 0x3D on to 0 and below: the burst); in by 10 until age
/// 0x50, then out by 9. `r`: s0..s4 at its spawns.
fn orb(fx: &mut Fx, t: u32, r: Regs) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x14);
        fx.mem.set16(t + 0xA, 0x14);
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        let v = rmod(fx, t, 0x01E0_1B6C, 6);
        grey(fx, t + 0x60, 0x80);
        fx.mem.set16(t + 0x56, v + 2);
        let i = fx.mem.i16(t + 0x50) as u32;
        let v = rmod(fx, t, 0x01E0_1B9C, 100);
        fx.mem.setf(VECTORS + 16 * i, ee::from_i32(v - 0x32));
        let i = fx.mem.i16(t + 0x50) as u32;
        let v = rmod(fx, t, 0x01E0_1BD0, 0x28);
        fx.mem.setf(VECTORS + 16 * i + 4, ee::from_i32(v));
        let i = fx.mem.i16(t + 0x50) as u32;
        let v = rmod(fx, t, 0x01E0_1C00, 100);
        fx.mem.setf(VECTORS + 16 * i + 8, ee::from_i32(v - 0x32));
        for c in 0..3 {
            let i = fx.mem.i16(t + 0x50) as u32;
            let a = VECTORS + 16 * i + 4 * c;
            let v = ee::div(fx.mem.f(a), 15.0);
            fx.mem.setf(a, v);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.set16(t + 0x52, 0x32);
        let v = rmod(fx, t, 0x01E0_1C94, 0x168);
        fx.mem.set16(t + 0x5C, v);
        let v = fx.rand_from(t, 0x01E0_1CB0);
        let o = (v >> 4) % 2;
        fx.mem.set16(t + 0xC, if o == 0 { 0 } else { 2 });
    }
    if fx.mem.i16(t + 0x54) < 0x50 {
        up(fx, t + 0x63, 0x80, 0xA);
    } else {
        down(fx, t, t + 0x63, 0xB, 9, true);
    }
    let c = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, c);
    if fx.mem.i16(t + 0x56) >= 3 {
        fx.mem.set16(t + 0x56, 0);
    }
    if fx.mem.i16(t + 0x54) % 3 == 1 {
        for o in [8u32, 0xA] {
            let w = fx.mem.i16(t + o);
            if w < 0x1E {
                fx.mem.set16(t + o, w + 1);
            }
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        drift_point(fx, t);
        if odd(fx.mem.i16(t + 0x54)) {
            spawn(fx, RING, r, 0x01E0_1E78);
        }
        let v = fx.mem.i16(t + 0x52);
        if v >= 0x15 {
            fx.mem.set16(t + 0x52, v - 1);
        } else if fx.mem.i16(t + 0x54) >= 0x3D {
            fx.mem.set16(t + 0x52, v - 1);
            if v == 0 {
                drift_point(fx, t);
                spawn(fx, BURST, r, 0x01E0_1F60);
                spawn(fx, GLOW, r, 0x01E0_1F74);
            }
        }
    }
    let i = fx.mem.i16(t + 0x50) as u32;
    let v = fx.mem.i16(t + 0x52);
    fx.mem.set16(SCALES + 2 * i, v);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1F90);
    age(fx, t);
    let v = fx.mem.i16(t + 0x5C) + 5;
    fx.mem.set16(t + 0x5C, v);
    let i = fx.mem.i16(t + 0x50) as u32;
    let v = fx.mem.i16(t + 0x52);
    fx.mem.set16(ANGLES + 2 * i, v);
}

/// In by 0x40 to 0xC0 before age +6, then out by 14 (ends below 16).
fn hold_out(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        up(fx, t + 0x63, 0xC0, 0x40);
    } else {
        down(fx, t, t + 0x63, 0x10, 0xE, true);
    }
}

/// 0x1E01FE0: a ring of quads the orb sheds (frame 0x20): size 2..4, cell
/// 0..2, radius +0x50 10..49, life 3..5.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2004, 3);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 2));
        let v = rmod(fx, t, 0x01E0_2030, 3);
        grey(fx, t + 0x60, 0x80);
        fx.mem.set16(t + 0x56, v);
        let v = rmod(fx, t, 0x01E0_205C, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_207C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_2098, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, 0x01E0_20B8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_20D4, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_out(fx, t);
    age(fx, t);
}

/// 0x1E023F0: the burst ring (frame 0x20): size 8..21, radius +0x50 3..7
/// growing by +0xE (16..35) a update, life 3..5.
fn burst(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_2414, 0xE);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 8));
        let v = rmod(fx, t, 0x01E0_2440, 3);
        grey(fx, t + 0x60, 0x80);
        fx.mem.set16(t + 0x56, v);
        let v = rmod(fx, t, 0x01E0_246C, 5);
        fx.mem.set16(t + 0x50, v + 3);
        let v = rmod(fx, t, 0x01E0_248C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_24A8, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, 0x01E0_24C8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, 0x01E0_24E4, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    hold_out(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        let v = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    age(fx, t);
}

/// 0x1E02170: a white spark ring (frame 0x20): radius 100 (inner 58 less)
/// growing by 35 a update, full for 5 updates, then out by 14.
fn sparks(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        fx.mem.setf(t + 0x4C, ee::sub(100.0, 58.0));
        grey(fx, t + 0x60, 0x80);
        grey(fx, t + 0x64, 0x80);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.write_u32(t + 0x34, 0x43AF_0000);
        let v = ee::div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 5 {
        fx.mem.write_u8(t + 0x63, 0x80);
        fx.mem.write_u8(t + 0x67, 0x80);
    } else {
        down(fx, t, t + 0x63, 0x10, 0xE, true);
        down(fx, t, t + 0x67, 0x10, 0xE, false);
    }
    let r = ee::add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::sub(r, 58.0));
    age(fx, t);
}

/// 0x1E025A0: the orb's arc ring (frame 0x30, s1 = D): radius 60, in by
/// 10 once the orb's countdown is below 21 until age 0x46, then out by 11;
/// white / yellow-white flicker, spun by 0x4000 a update.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x4270_0000);
        fx.mem.write_u32(t + 0x4C, 0x4270_0000);
        grey(fx, t + 0x64, 0xFF);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x46 {
        let i = fx.mem.i16(t + 0x50) as u32;
        if fx.mem.i16(SCALES + 2 * i) < 0x15 {
            up(fx, t + 0x67, 0x80, 0xA);
        }
    } else {
        down(fx, t, t + 0x67, 0xD, 0xB, true);
    }
    let g = if odd(fx.mem.i16(t + 0x54)) { 0xFF } else { 0xC0 };
    l2::rgb(fx, t + 0x60, g, g, 0x80);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_26DC);
    let a = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, a);
    age(fx, t);
}

/// 0x1E02710: a flier (frame 0x50 at `sp`, s1 = D): size 12, cell 2..7
/// cycling 0..3, flying from the attacker's node 0x1B towards node 0x1C
/// (the difference / 10, / 10, / 4, y raised by rand % 15 - 4); in by
/// 0x40 until age 15, then out by 9.
fn flier(fx: &mut Fx, sp: u32, t: u32) {
    let mut s1 = u64::from(D);
    if fx.mem.i16(t + 0x58) == 1 {
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE_A);
        node_position_saving(fx, h, s, sp + 0x30, u64::from(t), Some(u64::from(D)), 0x01E0_274C);
        let h = fx.self_handle();
        let s = fx.mem.i16(NODE_B);
        s1 = u64::from(sp + 0x40);
        node_position_saving(fx, h, s, sp + 0x40, u64::from(t), Some(s1), 0x01E0_2768);
        fx.mem.set16(t + 8, 0xC);
        fx.mem.set16(t + 0xA, 0xC);
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        let v = rmod(fx, t, 0x01E0_2780, 6);
        grey(fx, t + 0x60, 0x80);
        fx.mem.set16(t + 0x56, v + 2);
        let x = ee::sub(fx.mem.f(sp + 0x40), fx.mem.f(sp + 0x30));
        fx.mem.setf(t + 0x30, x);
        let v = rmod(fx, t, 0x01E0_27BC, 0xF);
        let y = ee::sub(fx.mem.f(sp + 0x44), fx.mem.f(sp + 0x34));
        let y = ee::sub(ee::add(y, ee::from_i32(v)), 4.0);
        fx.mem.setf(t + 0x34, y);
        let z = ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(sp + 0x38));
        fx.mem.setf(t + 0x38, z);
        for (o, by) in [(0x30u32, 10.0), (0x34, 10.0), (0x38, 4.0)] {
            let v = ee::div(fx.mem.f(t + o), by);
            fx.mem.setf(t + o, v);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0xF {
        up(fx, t + 0x63, 0x80, 0x40);
    } else {
        down(fx, t, t + 0x63, 0xB, 9, true);
    }
    let c = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, c);
    if fx.mem.i16(t + 0x56) >= 4 {
        fx.mem.set16(t + 0x56, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for c in 0..3 {
            let v = ee::add(fx.mem.f(t + 0x10 + 4 * c), fx.mem.f(t + 0x30 + 4 * c));
            fx.mem.setf(t + 0x10 + 4 * c, v);
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), s1, 0x01E0_28E8);
    age(fx, t);
}

/// 0x1E02910: the burst's glow quad (frame 0x20): size 80 growing by 1,
/// cell 0 stepping on the odd ages from 2 to 5; in by 0x60 until age 5,
/// then out by 9.
fn glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x50);
        fx.mem.set16(t + 0xA, 0x50);
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        grey(fx, t + 0x60, 0x80);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.i16(t + 0x54);
    if a < 5 {
        up(fx, t + 0x63, 0xC0, 0x60);
    } else {
        down(fx, t, t + 0x63, 0xB, 9, true);
    }
    let a = fx.mem.i16(t + 0x54);
    if a >= 2 && odd(a) {
        let c = fx.mem.i16(t + 0x56) + 1;
        fx.mem.set16(t + 0x56, c);
    }
    if fx.mem.i16(t + 0x56) >= 6 {
        fx.mem.set16(t + 0x56, 5);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for o in [8u32, 0xA] {
            let w = fx.mem.i16(t + o) + 1;
            fx.mem.set16(t + o, w);
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_2A30);
    age(fx, t);
}
