//! BMODULE b594 (card 289), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02EA0, data area 0x1ED0000) [F]: a pool of 250 tasks of 0x70 bytes at D+0x2A0 (alive +0x58, step
//! +0x44 through `jalr`, the stub 0x1E02E90 skipped; spawn 0x1E03750(D,
//! step, 0) with the colour D+0x7084, the angle D+0x70DA into +0x5A and the
//! index D+0x70F4 into +0x50), no unit flash. Drawn with arcs (0x1E011A0,
//! `billboard::arc_strips`) at the points D+0x7030 + 0x10 k, quads
//! (0x1E00EE0, `billboard::quad`), quad rings turned about y then x
//! (0x1E00920, `billboard::ring_quads_yx` on the bytes D+0x70FE / D+0x71FE /
//! D+0x72FE) and raised turned quads (0x1E01500,
//! `quads4::quad_turned_raised`), all with `uv::uv_rect`. Globals: the
//! point D+0x7030 (the target, then from 0x11D flying to the attacker's
//! node by D+0x7050.. until 0x14F), textures (5, 0..2) D+0x7060.. and (4,
//! 0xF) D+0x29C, timer D+0x7080 = 490, colour D+0x7084, OT D+0x708C, the
//! arc radius D+0x70A4 (2 a update up to 10 x the target's radius, at most
//! 140), nodes D+0x70B8.. (attacker 0x14, from 0x19 every 30 updates eight
//! random target nodes), target nodes D+0x70C8.., angles D+0x70E0..,
//! facing D+0x70F8, param D+0x70FA, counter D+0x70FC. Returns 1 / 4 / 2 /
//! 6 / 8 on 0x8C / 0x1DB / 0x1D1 / 0x97..0x1CB / timer out.
use crate::b494::camera_angles_saving;
use crate::blib::{afterimage, billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 0xFA;
const ARC: u32 = 0x01E0_1F30;
const QUAD: u32 = 0x01E0_2090;
const ARC_B: u32 = 0x01E0_2280;
const QUAD_B: u32 = 0x01E0_23F0;
const RING: u32 = 0x01E0_25E0;
const RING_B: u32 = 0x01E0_2790;
const RAISED: u32 = 0x01E0_2950;
const RING_C: u32 = 0x01E0_2B00;
const WINDOW: u32 = 0x01E0_2CB0;
const STUB: u32 = 0x01E0_2E90;
/// The return address of the entry's `jalr` into a step.
const TASK_RA: u64 = 0x01E0_32A0;
const POS: u32 = D + 0x250;
const POINT: u32 = D + 0x7030;
const VELOCITY: u32 = D + 0x7050;
const TEX: u32 = D + 0x7060;
const TIMER: u32 = D + 0x7080;
const COLOUR: u32 = D + 0x7084;
const OT_AT: u32 = D + 0x708C;
const RADIUS: u32 = D + 0x70A4;
const NODES: u32 = D + 0x70B8;
const TARGET_NODES: u32 = D + 0x70C8;
const ANGLES: u32 = D + 0x70E0;
const ANGLE: u32 = D + 0x70DA;
const INDEX: u32 = D + 0x70F4;
const FACING: u32 = D + 0x70F8;
const COUNTER: u32 = D + 0x70FC;
const BYTES: u32 = D + 0x70FE;
const BYTES_B: u32 = D + 0x71FE;
const BYTES_C: u32 = D + 0x72FE;
const SINES: u32 = D + 0x7400;

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

fn facing(fx: &mut Fx) -> bool {
    fx.mem.i16(FACING) != 0
}

/// The entry 0x1E02EA0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    fx.mem.write_u32(OT_AT, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xD0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x7570);
    l2::sine_table(fx, SINES);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x70FA, param);
    fx.mem.set32(TIMER, 0x1EA);
    let h = fx.self_handle();
    let r = fx.request_node(h, 0x14, 1);
    fx.mem.set16(NODES, r);
    let tex = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x29C, tex);
    for k in 0..3u32 {
        let tex = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, tex);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xC0);
    let v = ee::div(ee::from_i32(fx.mem.i32(sp + 0xC4)), 180.0);
    fx.fix_residue(v, 0x01E0_2FA4);
    fx.mem.set16(FACING, ee::fixsfsi(v));
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    let fills: [(u32, std::ops::Range<u32>, u32, bool, i32, i32); 5] = [
        (BYTES, 0..0x100, 0x01E0_2FCC, false, 0x14, -0xA),
        (BYTES, 100..0xB4, 0x01E0_301C, false, 0x14, 4),
        (BYTES_B, 0..0x100, 0x01E0_3064, false, 0x14, -0xA),
        (BYTES_C, 0..0x100, 0x01E0_30AC, true, 0x28, -0x14),
        (BYTES_C, 100..0xB4, 0x01E0_3104, true, 0x14, 6),
    ];
    for (at, ks, ra, shift, m, add) in fills {
        for k in ks {
            let r = fx.rand_from(D, ra);
            let r = if shift { r >> 4 } else { r };
            let v = fx.div(r, m).1;
            fx.mem.write_u8(at + k, (v + add) as u8);
        }
    }
    for k in 0..8u32 {
        let h = fx.self_handle();
        let n = fx.unit_nodes(h);
        let r = fx.rand_from(D + 2 * k, 0x01E0_3168);
        let mut node = fx.div(r, n).1 as i16 as i32;
        if node == 0 {
            node = 1;
        }
        let h = fx.target_handle();
        let v = if k == 0 { fx.request_node(h, 0x14, 1) } else { fx.request_node(h, node, 0) };
        fx.mem.set16(TARGET_NODES + 2 * k, v);
    }
    for k in 0..8u32 {
        let r = fx.rand_from(k, 0x01E0_3214);
        let v = fx.div(r >> 4, 0x168).1;
        fx.mem.set16(ANGLES + 2 * k, v);
    }
    0
}

/// The atlas cell `id` (0x106960 into `r`, frame +0x56) at D+0x94..+0xA3.
fn cell(fx: &mut Fx, t: u32, r: u32, id: i32) {
    let frame = fx.mem.i16(t + 0x56);
    l2::rect(fx, r, id, frame);
    copy4(fx, D + 0x94, r);
}

fn copy3(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(slot + 0x2E4);
        if f == 0 || f == STUB {
            continue;
        }
        run_task(fx, f, slot + 0x2A0, [u64::from(slot), u64::from(D), u64::from(k), 0]);
    }
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        draw(fx, sp, slot + 0x2A0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x8C => 1,
        0x1DB => 4,
        0x1D1 => 2,
        0x97..=0x1CB => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The entry's draw case for task `t`.
fn draw(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    if f == ARC || f == ARC_B {
        fx.mem.set16(D + 0xB8, 0x24);
        let k = fx.mem.i16(t + 0x50);
        let at = (POINT as i32 + 0x10 * k) as u32;
        billboard::arc_strips(fx, D, t, 0x21, at);
        fx.mem.set16(D + 0xB8, 0);
    }
    for (g, ti, r) in [(QUAD, 0u32, 0x60u32), (QUAD_B, 4, 0x70)] {
        if f == g {
            tex(fx, TEX + ti);
            cell(fx, t, sp + r, 0x14);
            fx.mem.set16(D + 0xBE, 0);
            copy3(fx, sp + 0x50, t + 0x10);
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x50);
        }
    }
    for (g, r, mode, table) in [(RING, 0x80u32, 0x20, BYTES), (RING_B, 0x90, 0x21, BYTES_B)] {
        if f == g {
            tex(fx, TEX + 8);
            cell(fx, t, sp + r, 0x14);
            ring(fx, t, mode, table);
        }
    }
    if f == RAISED {
        cell(fx, t, sp + 0xA0, 0x14);
        tex(fx, TEX + 8);
        fx.mem.set16(D + 0xBE, 5);
        copy3(fx, sp + 0x50, t + 0x10);
        quads4::quad_turned_raised(fx, uv::uv_rect, D, t, 0x21, sp + 0x50);
    }
    if f == RING_C {
        tex(fx, TEX + 8);
        cell(fx, t, sp + 0xB0, 0x14);
        ring(fx, t, 0x20, BYTES_C);
    }
}

fn ring(fx: &mut Fx, t: u32, mode: i32, table: u32) {
    fx.mem.set16(D + 0xB8, 4);
    fx.mem.set16(D + 0xB0, 0);
    billboard::ring_quads_yx(fx, uv::uv_rect, D, t, mode, table);
    fx.mem.set16(D + 0xB8, 0);
    fx.mem.set16(D + 0xB0, 0);
}

/// `regs`: the run loop's s0 (the slot), s1 (D), s2 (its index), s3 (0)
/// at the `jalr`.
fn run_task(fx: &mut Fx, f: u32, t: u32, regs: [u64; 4]) {
    match f {
        ARC => arc(fx, t, regs),
        QUAD => quad(fx, t, regs, 0x19, (0x1E, 0x19), true, [0x01E0_20C0, 0x01E0_20E4, 0x01E0_225C]),
        ARC_B => arc_b(fx, t, regs),
        QUAD_B => quad(fx, t, regs, 0xA, (0x14, 0xA), false, [0x01E0_2420, 0x01E0_2444, 0x01E0_25BC]),
        RING => ring_step(fx, t, regs, 0x30, false, (0x64, 0x64), 6, 0x01E0_2630),
        RING_B => ring_step(fx, t, regs, 0x20, true, (0x32, 0x28), 6, 0x01E0_27D8),
        RAISED => raised(fx, t, regs),
        RING_C => ring_step(fx, t, regs, 0x30, false, (0xC8, 0xC8), 0x14, 0x01E0_2B50),
        WINDOW => window(fx, t, regs),
        _ => panic!("b594: unknown task step {f:#x}"),
    }
}

/// A step's prologue: `frame` bytes, the caller's s0.. (`n` of `regs`) at
/// +0x10 k, ra at `ra_at`.
fn enter_step(fx: &mut Fx, frame: u32, regs: [u64; 4], n: usize, ra_at: u32) -> u32 {
    let sp = fx.enter(frame);
    for k in 0..n {
        fx.saved(sp + 0x10 * k as u32, &[(0, regs[k])]);
    }
    fx.saved(sp, &[(ra_at, TASK_RA)]);
    sp
}

/// 0x1E03750(D, step, 0) with the caller's s0..s4 = `regs`.
fn spawn(fx: &mut Fx, f: u32, regs: [u64; 5], ra: u32) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, &[(0x50, u64::from(ra)), (0x40, regs[4]), (0x30, regs[3]), (0x20, regs[2]), (0x10, regs[1]), (0, regs[0])]);
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
            l2::to_i16x4(fx, t, POS, 0);
            // 0x107390's `sq ra` upper half (0) (b232's).
            fx.mem.write_u64(sp - 8, 0);
            copy3(fx, t + 0x10, POS);
            let a = fx.mem.i16(ANGLE);
            fx.mem.set16(t + 0x5A, a);
            let i = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x50, i);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// The emitter's registers for its callees' saves: s0 (the entry's param,
/// then a node pointer), s1 = D, the runner's s2 (0 [H]) and s3 (0).
struct Regs {
    s0: u64,
}

impl Regs {
    fn spawn(&self, fx: &mut Fx, f: u32, ra: u32) {
        spawn(fx, f, [self.s0, u64::from(D), 0, 0, 0], ra);
    }

    fn node(&self, fx: &mut Fx, h: i32, slot: i32, out: u32, ra: u32) {
        crate::b076::node_position_saving(fx, h, slot, out, self.s0, Some(u64::from(D)), ra);
    }
}

/// D+0x250 = the attacker's node D+0x70B8 (0x1F65B0's handle).
fn at_node(fx: &mut Fx, r: &Regs, ra: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i16(NODES);
    r.node(fx, h, s, POS, ra);
}

/// 0x1E01840(D, 0) (frame 0x70: the entry's s0 = param, s1 = D, s2, s3 = 0,
/// ra).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x70);
    let mut r = Regs { s0: param as i64 as u64 };
    fx.saved(sp, &[(0x40, 0x01E0_3260), (0x30, 0), (0x20, 0), (0x10, u64::from(D)), (0, r.s0)]);
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    let c = counter(fx);
    if (0x8C..0x15F).contains(&c) {
        let h = fx.target_handle();
        let rad = afterimage::unit_radius(fx, h);
        let v = fx.mem.f(RADIUS);
        if crate::exe::ee_lt(v, ee::mul(10.0, rad)) {
            fx.mem.setf(RADIUS, ee::add(v, 2.0));
        }
        let v = fx.mem.f(RADIUS);
        if !crate::exe::ee_le(v, 140.0) {
            fx.mem.write_u32(RADIUS, 0x430C_0000);
        }
    }
    if counter(fx) == 1 {
        r.spawn(fx, WINDOW, 0x01E0_1910);
    }
    let c = counter(fx);
    if c < 0x11D {
        if c == 1 {
            let h = fx.target_handle();
            let s = fx.mem.i16(TARGET_NODES);
            r.node(fx, h, s, POINT, 0x01E0_1944);
            let h = fx.target_handle();
            fx.unit_position(h, POINT);
            let z = fx.mem.f(POINT + 8);
            let z = if facing(fx) { ee::add(z, 17.0) } else { ee::sub(z, 17.0) };
            fx.mem.setf(POINT + 8, z);
            fx.mem.set16(INDEX, 0);
            r.spawn(fx, ARC, 0x01E0_19B0);
        }
    } else {
        if c == 0x11D {
            let h = fx.self_handle();
            let s = fx.mem.i16(NODES);
            r.node(fx, h, s, sp + 0x50, 0x01E0_19DC);
            for o in [8u32, 4, 0] {
                let v = ee::div(ee::sub(fx.mem.f(sp + 0x50 + o), fx.mem.f(POINT + o)), 50.0);
                fx.mem.setf(VELOCITY + o, v);
            }
        }
        if counter(fx) < 0x14F {
            for o in [8u32, 4, 0] {
                let v = ee::add(fx.mem.f(POINT + o), fx.mem.f(VELOCITY + o));
                fx.mem.setf(POINT + o, v);
            }
        }
    }
    let c = counter(fx);
    if (0x19..0x14F).contains(&c) {
        if c.wrapping_rem(0x1E) == 1 {
            for k in 0..8u32 {
                r.s0 = u64::from(D + 2 * k);
                let h = fx.target_handle();
                let n = fx.unit_nodes(h);
                let v = fx.rand_from(D + 2 * k, 0x01E0_1AC0);
                let mut node = fx.div(v, n).1 as i16 as i32;
                if node == 0 {
                    node = 1;
                }
                let h = fx.target_handle();
                let v = fx.request_node(h, node, i32::from(k == 0));
                fx.mem.set16(NODES + 2 * k, v);
            }
            r.s0 = u64::from(D + 0x10);
        } else if c.wrapping_rem(4) == 0 {
            for k in 0..8u32 {
                r.s0 = u64::from(D + 2 * k);
                let h = fx.target_handle();
                let s = fx.mem.i16(NODES + 2 * k);
                r.node(fx, h, s, sp + 0x50, 0x01E0_1BB0);
                copy4(fx, POS, sp + 0x50);
                fx.mem.set16(INDEX, k as i32);
                r.spawn(fx, QUAD, 0x01E0_1BEC);
            }
            r.s0 = u64::from(D + 0x10);
        }
    }
    let c = counter(fx);
    if (0x19..0x14F).contains(&c) {
        fx.mem.set16(INDEX, 0);
        r.spawn(fx, ARC_B, 0x01E0_1C4C);
    }
    let c = counter(fx);
    if (0x151..0x1C3).contains(&c) {
        for (dx, ra_n, ra_s) in [(1.0f32, 0x01E0_1C80u32, 0x01E0_1D0Cu32), (-1.0, 0x01E0_1D24, 0x01E0_1DB0)] {
            at_node(fx, &r, ra_n);
            let x = fx.mem.f(POS);
            let x = if dx > 0.0 { ee::add(x, 1.0) } else { ee::sub(x, 1.0) };
            fx.mem.setf(POS, x);
            let y = ee::sub(fx.mem.f(POS + 4), f32::from_bits(0x3F99_999A));
            fx.mem.setf(POS + 4, y);
            let z = fx.mem.f(POS + 8);
            let d = f32::from_bits(0x3F33_3333);
            let z = if facing(fx) { ee::add(z, d) } else { ee::sub(z, d) };
            fx.mem.setf(POS + 8, z);
            r.spawn(fx, QUAD_B, ra_s);
        }
    }
    let c = counter(fx);
    if (0x61..0x141).contains(&c) {
        at_node(fx, &r, 0x01E0_1DE4);
        let y = ee::sub(fx.mem.f(POS + 4), 0.5);
        fx.mem.setf(POS + 4, y);
        r.spawn(fx, RING, 0x01E0_1E0C);
    }
    let c = counter(fx);
    if (0x61..0x141).contains(&c) {
        copy4(fx, POS, POINT);
        r.spawn(fx, RING_C, 0x01E0_1E5C);
    }
    let c = counter(fx);
    if (0x58..0x141).contains(&c) {
        at_node(fx, &r, 0x01E0_1E90);
        let y = ee::sub(fx.mem.f(POS + 4), 0.5);
        fx.mem.setf(POS + 4, y);
        r.spawn(fx, RAISED, 0x01E0_1EB8);
    }
    let c = counter(fx);
    if (0x89..0x141).contains(&c) {
        copy4(fx, POS, POINT);
        r.spawn(fx, RING_B, 0x01E0_1F08);
    }
    fx.leave(0x70);
}

/// The alpha byte `a` down by `down.1` while at least `down.0`, else the
/// task ends.
fn fade_out(fx: &mut Fx, t: u32, a: u32, down: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if v >= down.0 {
        fx.mem.write_u8(t + a, v.wrapping_sub(down.1));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The alpha byte `a` up by `up.1` while below `up.0`.
fn fade_in(fx: &mut Fx, t: u32, a: u32, up: (u8, u8)) {
    let v = fx.mem.read_u8(t + a);
    if v < up.0 {
        fx.mem.write_u8(t + a, v.wrapping_add(up.1));
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn young(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6)
}

fn first(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x58) == 1
}

fn odd(fx: &mut Fx, t: u32) -> bool {
    fx.mem.i16(t + 0x54).wrapping_rem(2) == 1
}

/// rand % m (HI) from a step's rand returning to `ra` (s0 = the task).
fn rand_mod(fx: &mut Fx, t: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(t, ra);
    fx.div(v, m).1
}

/// The arcs' camera turn: 0x205580 into +0x20, then +0x4000.
fn face(fx: &mut Fx, t: u32, ra: u32) {
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), ra);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
}

/// 0x1E01F30 (frame 0x30): the arc at the point (radius D+0x70A4), grey
/// over green, red / dark red by age parity; in by 0x40 below 0x80 for 4
/// updates, out by 0xB from update 0x1E6.
fn arc(fx: &mut Fx, t: u32, regs: [u64; 4]) {
    enter_step(fx, 0x30, regs, 2, 0x20);
    if first(fx, t) {
        let v = fx.mem.read_u32(RADIUS);
        fx.mem.write_u32(t + 0x4C, v);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0xFF, 0x80);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x67, (0x80, 0x40));
    } else if counter(fx) >= 0x1E6 {
        fade_out(fx, t, 0x67, (0xD, 0xB));
    }
    if odd(fx, t) {
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
    } else {
        l2::rgb(fx, t + 0x60, 0xC0, 0x80, 0x80);
    }
    age(fx, t);
    let v = fx.mem.read_u32(RADIUS);
    fx.mem.write_u32(t + 0x4C, v);
    face(fx, t, 0x01E0_206C);
    fx.leave(0x30);
}

/// 0x1E02090 / 0x1E023F0 (frame 0x30): a blue quad (life +0xA = rand %
/// `life.0` + `life.1`, cell (rand >> 4) % 8 cycling, turn the angle of its
/// node +0x50) rising 0.6 (`up`) or sinking 0.2, growing by 0.1 below 0.8;
/// in by 0x40 below 0x80 for 5 updates, then out by 9.
fn quad(fx: &mut Fx, t: u32, regs: [u64; 4], e8: i32, life: (i32, i32), up: bool, ras: [u32; 3]) {
    enter_step(fx, 0x30, regs, 2, 0x20);
    if first(fx, t) {
        fx.mem.set16(t + 8, e8);
        let r = rand_mod(fx, t, ras[0], life.0);
        fx.mem.set16(t + 0xA, r + life.1);
        fx.mem.write_u32(t + 0x48, 0);
        let r = fx.rand_from(t, ras[1]);
        fx.mem.set16(t + 0x56, (r >> 4).wrapping_rem(8));
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let k = fx.mem.i16(t + 0x50);
        let a = fx.mem.i16((ANGLES as i32 + 2 * k) as u32);
        fx.mem.set16(t + 0x5C, a);
    }
    if fx.mem.i16(t + 0x54) < 5 {
        fade_in(fx, t, 0x63, (0x80, 0x40));
    } else {
        fade_out(fx, t, 0x63, (0xB, 9));
    }
    if odd(fx, t) {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0x70, 0x70, 0xC0);
    }
    let v = fx.mem.i16(t + 0x56) + 1;
    fx.mem.set16(t + 0x56, v);
    if fx.mem.i16(t + 0x56) >= 8 {
        fx.mem.set16(t + 0x56, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let y = fx.mem.f(t + 0x14);
        let y = if up { ee::add(y, f32::from_bits(0x3F19_999A)) } else { ee::sub(y, f32::from_bits(0x3E4C_CCCD)) };
        fx.mem.setf(t + 0x14, y);
        let s = fx.mem.f(t + 0x48);
        if crate::exe::ee_lt(s, f32::from_bits(0x3F4C_CCCD)) {
            fx.mem.setf(t + 0x48, ee::add(s, f32::from_bits(0x3DCC_CCCD)));
        }
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), ras[2]);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02280 (frame 0x30): the red arc at the point, inner radius 5 + rand
/// % (D+0x70A4 as an integer), grey; in by 0x40 below 0x80 for 4 updates,
/// then out by 0xB.
fn arc_b(fx: &mut Fx, t: u32, regs: [u64; 4]) {
    enter_step(fx, 0x30, regs, 2, 0x20);
    let v = fx.mem.f(RADIUS);
    fx.fix_residue(v, 0x01E0_229C);
    let m = ee::fixsfsi(v) as i16 as i32;
    if first(fx, t) {
        let r = fx.rand_from(t, 0x01E0_22B8);
        let h = fx.div(r, m).1;
        fx.mem.setf(t + 0x4C, ee::add(5.0, ee::from_i32(h)));
        l2::rgb(fx, t + 0x64, 0x50, 0x50, 0x50);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x67, (0x80, 0x40));
    } else {
        fade_out(fx, t, 0x67, (0xD, 0xB));
    }
    if odd(fx, t) {
        l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
    } else {
        l2::rgb(fx, t + 0x60, 0xC0, 0x30, 0x30);
    }
    age(fx, t);
    face(fx, t, 0x01E0_23C8);
    fx.leave(0x30);
}

/// 0x1E025E0 / 0x1E02790 / 0x1E02B00: a white quad ring (cell 4, size 5 or
/// 10, radius rand % `radius.0` + `radius.1`, turn rand % 360, row rand %
/// 8 + 1, life rand % 3 + 2); `random` (0x1E02790) turned randomly about
/// all axes with +0x5C 90, else +0x5C +-90 by the facing and the radius
/// shrinking by rand % `shrink` + 4 (+0xD for 0x1E02B00) while positive;
/// in by 0x2A below 0x80, then out by 0xB.
#[allow(clippy::too_many_arguments)]
fn ring_step(fx: &mut Fx, t: u32, regs: [u64; 4], frame: u32, random: bool, radius: (i32, i32), shrink: i32, ra0: u32) {
    let n = if frame == 0x20 { 1 } else { 2 };
    enter_step(fx, frame, regs, n, frame - 0x10);
    if first(fx, t) {
        let big = shrink == 0x14;
        fx.mem.write_u32(t + 0x48, if big { 0x4120_0000 } else { 0x40A0_0000 });
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        let ras = [ra0, ra0 + 0x20, ra0 + 0x60, ra0 + 0x80, ra0 + 0xA0];
        if random {
            // ras 0x1E027D8 / 0x1E027F4 / 0x1E02810, then 0x1E0282C..
            for (o, d) in [(0x20u32, 0u32), (0x24, 0x1C), (0x28, 0x38)] {
                let v = rand_mod(fx, t, ra0 + d, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
            let v = rand_mod(fx, t, 0x01E0_282C, radius.0);
            fx.mem.set16(t + 0x50, v + radius.1);
            let v = rand_mod(fx, t, 0x01E0_284C, 0x168);
            fx.mem.set16(t + 0x5A, v);
            fx.mem.set16(t + 0x5C, 0x5A);
            let v = rand_mod(fx, t, 0x01E0_2870, 6);
            fx.mem.set16(t + 0xE, v + 4);
            let v = fx.rand_from(t, 0x01E0_2890);
            fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
            let v = rand_mod(fx, t, 0x01E0_28B0, 3);
            fx.mem.set16(t + 6, v + 2);
        } else {
            let v = rand_mod(fx, t, ras[0], radius.0);
            fx.mem.set16(t + 0x50, v + radius.1);
            let v = rand_mod(fx, t, ras[1], 0x168);
            fx.mem.set16(t + 0x5A, v);
            let b = if facing(fx) { 0x5A } else { -0x5A };
            fx.mem.set16(t + 0x5C, b);
            let v = rand_mod(fx, t, ras[2], shrink);
            fx.mem.set16(t + 0xE, v + if big { 0xD } else { 4 });
            let v = fx.rand_from(t, ras[3]);
            fx.mem.set16(t + 8, v.wrapping_rem(8) + 1);
            let v = rand_mod(fx, t, ras[4], 3);
            fx.mem.set16(t + 6, v + 2);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if young(fx, t) {
        fade_in(fx, t, 0x63, (0x80, 0x2A));
    } else {
        fade_out(fx, t, 0x63, (0xD, 0xB));
    }
    if !random && fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 0x50) > 0 {
        let v = fx.mem.i16(t + 0x50) - fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, v);
    }
    age(fx, t);
    fx.leave(frame);
}

/// 0x1E02950 (frame 0x30): a white raised quad (+8 3, +0xA 0x54, cell 4)
/// dropping from 700 by 30, turned rand % 360 / rand % 60 - 30 and +-0x4000
/// about x by the facing; in by 0x2A below 0x80 for 3 updates, then out by
/// 9 twice a update.
fn raised(fx: &mut Fx, t: u32, regs: [u64; 4]) {
    enter_step(fx, 0x30, regs, 2, 0x20);
    if first(fx, t) {
        fx.mem.set16(t + 8, 3);
        fx.mem.set16(t + 0xA, 0x54);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.write_u32(t + 0x34, 0x442F_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        let r = rand_mod(fx, t, 0x01E0_29CC, 0x168);
        fx.mem.set16(t + 0x5A, r);
        let r = rand_mod(fx, t, 0x01E0_29E8, 0x3C);
        fx.mem.set16(t + 0x5C, r - 0x1E);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        fade_in(fx, t, 0x63, (0x80, 0x2A));
        fade_in(fx, t, 0x67, (0x80, 0x2A));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let d = fx.mem.f(t + 0x34);
        if !crate::exe::ee_le(d, 30.0) {
            fx.mem.setf(t + 0x34, ee::sub(d, 30.0));
        } else {
            fade_out(fx, t, 0x63, (0xB, 9));
            let v = fx.mem.read_u8(t + 0x63);
            if v >= 0xB {
                fx.mem.write_u8(t + 0x63, v - 9);
            }
        }
    }
    fx.mem.write_u32(t + 0x20, 0);
    let a = if facing(fx) { -0x4000 } else { 0x4000 };
    fx.mem.set32(t + 0x20, a);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E02CB0 (frame 0x90): a grey window sprite (0x78, ALPHA 0x44, key 0),
/// in by 8 below 0x50 until update 0x163, then out by 7.
fn window(fx: &mut Fx, t: u32, regs: [u64; 4]) {
    let sp = enter_step(fx, 0x90, regs, 2, 0x20);
    if first(fx, t) {
        fx.mem.write_u32(t + 0x48, 0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..3u32 {
        fx.mem.write_u8(sp + 0x48 + k, 0x78);
    }
    let a = fx.mem.read_u8(t + 0x63);
    fx.mem.write_u8(sp + 0x4B, a);
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    for o in [0x50u32, 0x54] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(s, 512.0));
    }
    for o in [0x60u32, 0x64] {
        let s = fx.mem.f(t + 0x48);
        fx.mem.setf(sp + o, ee::div(ee::sub(512.0, s), 512.0));
    }
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, 0x80_0000_0044);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if counter(fx) < 0x163 {
        fade_in(fx, t, 0x63, (0x50, 8));
    } else {
        fade_out(fx, t, 0x63, (9, 7));
    }
    age(fx, t);
    fx.leave(0x90);
}
