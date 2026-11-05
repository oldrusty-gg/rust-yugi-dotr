//! BMODULE b134 (card 111), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E029F0) [F]. The entry
//! takes (D, param). b194's task-pool family: 150 tasks of 0x40 at D+0x380
//! (alive +0x36, step +0x24), spawned by 0x1E03750 (b538's spawn at D+0x2D0
//! with the colour D+0x2FC4, +0x30 = the kind D+0x3022), the sines D+0x312C
//! and D+0xD8, timer D+0x2FC0 = 300, the OT at D+0x80 and D+0x2FCC. Nodes:
//! the attacker's 0x37 / 0x39 / 0x4C / 0x4E / 0x10 / 0x12 at D+0x3008..,
//! the target's 1 at D+0x3016; textures (4, 0..2) D+0x340.., (4, 0xB..0xF)
//! D+0x36C.., (5, 0) D+0x2FA0 (0x1E02950); 256 jitter bytes D+0x302A.
//!
//! Six ten-point node histories (D+0x2950 + 0xA0 k, node_history every
//! update; reset to a node on updates 0xB3 / 0xB9 / 0xBF / 0xC9 / 0xCD).
//! The emitter 0x1E016D0 spawns only the object op 0x1E02750 (0x9B), the
//! camera light 0x1E024D0 (0xB4), the claw ribbons 0x1E02080 (kinds 0 / 1 /
//! 2 on 0xB3 + 0xC9 / 0xB9 + 0xCD / 0xBF; inline in the entry), and rings
//! 0x1E01E20 (three a update) with discs 0x1E01FB0 at nodes 0x39 / 0x4E /
//! 0x12 through the slashes (the kind = facing); the record's other steps
//! (0x1E01B90 / 0x1C70 / 0x2140 / 0x21F0 / 0x2300 / 0x2470 / 0x2690 /
//! 0x27F0) and draws (spline::ribbon_tex 0x1E00820, b218's 0x1E03870) are
//! never spawned [F]. Draws: rings 0x1E00DE0 (ring_ticks with the side test
//! on the first vertex's z), discs quads4::fan18, ribbons 0x1E011E0 (five
//! gouraud tubes per claw along its history, offset 3 (sin, kind, cos) of
//! -90 + 15 j). Returns 1 on 0xA9, 4 on 0xE4, 2 on 0xDA, 6 on 0xB4..0xD4,
//! 8 once the timer has run out.
use crate::blib::spawn::node_history;
use crate::blib::spline::spline;
use crate::blib::{exe as bexe, quads4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 0x96;
const POOL: u32 = D + 0x380;
const OT_A: u32 = D + 0x80;
const POS: u32 = D + 0x2D0;
const CAM: u32 = D + 0x2900;
const HIST: u32 = D + 0x2950;
const RING_TEX: u32 = D + 0x2FA0;
const TIMER: u32 = D + 0x2FC0;
const COLOUR: u32 = D + 0x2FC4;
const OTW: u32 = D + 0x2FCC;
const NODE: u32 = D + 0x3008;
const KIND: u32 = D + 0x3022;
const PARAM: u32 = D + 0x3024;
const COUNTER: u32 = D + 0x3026;
const BYTES: u32 = D + 0x302A;
const TAB: u32 = D + 0x312C;
const TAB2: u32 = D + 0xD8;

const RING: u32 = 0x01E0_1E20;
const DISC: u32 = 0x01E0_1FB0;
const CLAW: u32 = 0x01E0_2080;
const LIGHT: u32 = 0x01E0_24D0;
const OBJECT: u32 = 0x01E0_2750;

pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x2D0);
    fx.mem.write_u32(OT_A, OT);
    fx.mem.write_u32(OTW, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0x2D0);
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
    fx.bzero(D, 0x32A0);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, TAB2);
    fx.mem.set16(PARAM, param);
    fx.mem.set32(TIMER, 0x12C);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.set32(D + o, 0);
    }
    for (k, (n, clear)) in [(0x37, 1), (0x39, 0), (0x4C, 0), (0x4E, 0), (0x10, 0), (0x12, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, clear);
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 0);
    fx.mem.set16(D + 0x3016, r);
    // 0x1E02950(0, D): the textures.
    let sp = fx.enter(0x40);
    let _ = sp;
    for k in 0..3u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x340 + 4 * k, t);
    }
    for k in 0xBu32..0x10 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x36C + 4 * (k - 0xB), t);
    }
    let t = fx.texture(5, 0);
    fx.mem.write_u32(RING_TEX, t);
    fx.leave(0x40);
    for k in 0..0x100u32 {
        let r = rmod(fx, 0x1E);
        fx.mem.write_u8(BYTES + k, (r - 0xF) as u8);
    }
    0
}

/// The history at `at` (9 points) set to node slot `slot`'s position.
fn reset_hist(fx: &mut Fx, at: u32, slot: u32, local: u32) {
    let h = fx.self_handle();
    let n = fx.mem.i16(NODE + slot);
    fx.node_position(h, n, local);
    for k in 0..9u32 {
        for c in 0..3 {
            let w = fx.mem.read_u32(local + 4 * c);
            fx.mem.write_u32(at + 0x10 * k + 4 * c, w);
        }
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    // (update, history, node slot, local) as the entry checks them.
    const RESETS: [(i32, u32, u32, u32); 10] = [
        (0xB3, 0x2950, 2, 0x190),
        (0xB3, 0x2B30, 0, 0x1A0),
        (0xB9, 0x29F0, 6, 0x1B0),
        (0xB9, 0x2BD0, 4, 0x1C0),
        (0xBF, 0x2A90, 0xA, 0x1D0),
        (0xBF, 0x2C70, 8, 0x1E0),
        (0xC9, 0x29F0, 2, 0x1F0),
        (0xC9, 0x2B30, 0, 0x200),
        (0xCD, 0x2A90, 6, 0x210),
        (0xCD, 0x2C70, 4, 0x220),
    ];
    for (c, at, slot, local) in RESETS {
        if fx.mem.i16(COUNTER) == c {
            reset_hist(fx, D + at, slot, sp + local);
        }
    }
    for (top, slot) in [(0x29E0u32, 2u32), (0x2A80, 6), (0x2B20, 0xA), (0x2BC0, 0), (0x2C60, 4), (0x2D00, 8)] {
        let n = fx.mem.i16(NODE + slot);
        node_history(fx, D + top, 0xA, n);
    }
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        // The steps' prologue saves (the entry's s0 = D): 0x20 frames ra +
        // 0x10, s0 = the task; the light's 0x30 ra + 0x20, s1 = D, s0. A
        // ring drawn with texture (5, 0) missing keeps the upper half of
        // the s0 save as its TEX0.
        let low = fx.sp;
        match fx.mem.read_u32(t + 0x24) {
            RING => {
                ring(fx, t);
                fx.saved(low - 0x20, &[(0x10, 0x01E0_2FA8), (0, u64::from(t))]);
            }
            DISC => {
                disc(fx, t);
                fx.saved(low - 0x20, &[(0x10, 0x01E0_2FC8), (0, u64::from(t))]);
            }
            CLAW => claw(fx, t),
            LIGHT => {
                light(fx, t);
                fx.saved(low - 0x30, &[(0x20, 0x01E0_31C8), (0x10, u64::from(D)), (0, u64::from(t))]);
            }
            OBJECT => object(fx, sp, t),
            _ => {}
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x90);
    for k in 0..TASKS {
        let t = POOL + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        match fx.mem.read_u32(t + 0x24) {
            RING => {
                let w = fx.mem.read_u32(RING_TEX);
                fx.mem.write_u32(D + 0x88, w);
                let c = fx.mem.i16(t + 0x34);
                l2::rect(fx, sp + 0x160, 0x14, c);
                for k in 0..4 {
                    let w = fx.mem.read_u32(sp + 0x160 + 4 * k);
                    fx.mem.write_u32(D + 0x94 + 4 * k, w);
                }
                fx.mem.set16(D + 0xB8, 1);
                let side = fx.mem.i16(t + 0x30);
                ring_ticks_z(fx, t, side);
                fx.mem.set16(D + 0xB8, 0);
            }
            DISC => quads4::fan18(fx, D, t, sp + 0x238),
            CLAW => {
                let kind = fx.mem.i16(t + 0x30);
                if kind == 0 {
                    fx.mem.set32(D + 4, 0);
                }
                if kind == 1 {
                    fx.mem.write_u32(D + 4, 0x3F80_0000);
                }
                let mut a = -0x5Ai32;
                for _ in 0..5 {
                    let s = ee::mul(3.0, l2::sin_deg(fx, TAB, a as i16 as i32, 0));
                    fx.mem.setf(D, s);
                    let c = ee::mul(3.0, l2::sin_deg(fx, TAB, a as i16 as i32, 1));
                    fx.mem.setf(D + 8, c);
                    let kind = fx.mem.i16(t + 0x30);
                    if (0..3).contains(&kind) {
                        tube(fx, t, HIST + 0xA0 * kind as u32, D);
                    }
                    a += 0xF;
                }
            }
            _ => {}
        }
    }
    let n = fx.mem.i32(TIMER);
    if n != 0 {
        fx.mem.set32(TIMER, n - 1);
    }
    match fx.mem.i16(COUNTER) {
        0xA9 => 1,
        0xE4 => 4,
        0xDA => 2,
        0xB4..=0xD4 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E03750(D, step, 0): the first free task from 0 (b538's spawn).
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

/// 0x1E016D0(D, 0) (frame 0x70): the spawn colour (0x80, 0, 0, 0), the
/// update counter and the spawns.
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x70);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let c = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, c);
    if c == 0x9B {
        spawn(fx, OBJECT);
    }
    if fx.mem.i16(COUNTER) == 0xB4 {
        spawn(fx, LIGHT);
    }
    for (kind, us) in [(0, [0xB3, 0xC9]), (1, [0xB9, 0xCD]), (2, [0xBF, 0xBF])] {
        let c = fx.mem.i16(COUNTER);
        if us.contains(&c) {
            fx.mem.set16(KIND, kind);
            spawn(fx, CLAW);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if c == 0xBA || c == 0xBB {
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE + 2);
        fx.node_position(h, n, sp + 0x40);
        let h = fx.target_handle();
        fx.unit_position(h, sp + 0x50);
        for (to, from) in [(0u32, 0x40u32), (4, 0x44), (8, 0x58)] {
            let w = fx.mem.read_u32(sp + from);
            fx.mem.write_u32(POS + to, w);
        }
        fx.mem.set16(KIND, 1);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x60);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x64)), 180.0);
    let yaw = fix(fx, y);
    let slashes: [(u32, &[i32]); 3] = [
        (2, &[0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD]),
        (6, &[0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xCD, 0xCE, 0xCF, 0xD0, 0xD1]),
        (0xA, &[0xBF, 0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7]),
    ];
    for (slot, us) in slashes {
        let c = fx.mem.i16(COUNTER);
        if !us.contains(&c) {
            continue;
        }
        let h = fx.self_handle();
        let n = fx.mem.i16(NODE + slot);
        fx.node_position(h, n, sp + 0x40);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
            fx.mem.write_u32(POS + 4 * k, w);
        }
        fx.mem.set16(KIND, i32::from(yaw == 0));
        for _ in 0..3 {
            spawn(fx, RING);
        }
        spawn(fx, DISC);
    }
    fx.leave(0x70);
}

fn age(fx: &mut Fx, t: u32) -> i32 {
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    n
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

/// 0x1E01E20 (D, task): a white ring (size -10 growing by +8 = rand % 3 +
/// 4, turned (90, rand % 360, rand % 360)), grey on even ages.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, -0xA);
        fx.mem.set32(t + 0x10, 0x5A);
        let r = rmod(fx, 0x168);
        fx.mem.set32(t + 0x14, r);
        let r = rmod(fx, 0x168);
        fx.mem.set32(t + 0x18, r);
        fx.mem.set16(t + 0x34, 0);
        fx.mem.set16(t + 0x36, 2);
        let r = rmod(fx, 3);
        fx.mem.set16(t + 8, r + 4);
        fx.mem.set16(t + 6, 0x14);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    }
    if fx.mem.i16(t + 0x32) < 2 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a < 0x80 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else if fx.mem.i16(t + 0x38) >= 0xB {
        fade_out(fx, t, 0x15, 0x13);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
    } else {
        l2::rgb(fx, t + 0x28, 0xC0, 0xC0, 0xC0);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let s = fx.mem.i16(t + 0x38) + fx.mem.i16(t + 8);
        fx.mem.set16(t + 0x38, s);
    }
    age(fx, t);
}

/// 0x1E01FB0 (D, task): a disc (radius 0x82, grey centre, red rim of alpha
/// 0), alpha 0x80 on its first update, then out by 0xF.
fn disc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x82);
        l2::rgb(fx, t + 0x28, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x2C, 0x80, 0x10, 0x10);
        let r = rmod(fx, 0x168);
        fx.mem.set16(t + 0x3A, r);
        fx.mem.set16(t + 0x36, 2);
        fx.mem.write_u8(t + 0x2F, 0);
    }
    if fx.mem.i16(t + 0x32) <= 0 {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x11, 0xF);
    }
    age(fx, t);
}

/// The entry's inline step of 0x1E02080 (the claws): red, size 0xD, life
/// +0xE = 8 at alpha 0x80, then out by 0x1D; +6 = 9 points.
fn claw(fx: &mut Fx, t: u32) {
    l2::rgb(fx, t + 0x28, 0x80, 0x20, 0x20);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0xD);
        fx.mem.set16(t + 0xE, 8);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 0xE) {
        fx.mem.write_u8(t + 0x2B, 0x80);
    } else {
        fade_out(fx, t, 0x1F, 0x1D);
    }
    fx.mem.set16(t + 6, 9);
    age(fx, t);
}

/// The entry's inline step of 0x1E02750: the object op 0x1F8380(10, 0, ..)
/// (record literals 0x1E03F00 / 0x1E03F08) every update until age 0x50.
fn object(fx: &mut Fx, sp: u32, t: u32) {
    for (to, from) in [(0x2CCu32, 0x3F00u32), (0x2C8, 0x3F08)] {
        let w = fx.mem.read_u32(0x01E0_0000 + from);
        fx.mem.write_u32(sp + to, w);
    }
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x36, 2);
    }
    fx.boundary(0x001F_8380, "object op 0x1F8380", [0xA, 0, sp + 0x2CC, sp + 0x2C8]);
    if fx.mem.i16(t + 0x32) >= 0x50 {
        fx.mem.set16(t + 0x36, 0);
    }
    age(fx, t);
}

/// 0x1E024D0 (D, task) (frame 0x30): camera 0's slots 4 / 5 (0x1F7E20)
/// from D+0x2900 = (128, 6, 0 / 3 by age parity, 0), the task's point,
/// (0, 150, w): w up by 0.2 to 1 for 30 updates, then down by 0.1 (the
/// task over once at most 0.1); cleared on its last update; over on timer 1.
fn light(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    let _ = sp;
    if fx.mem.i16(t + 0x36) == 1 {
        for (o, v) in [(0u32, 0x4300_0000u32), (4, 0x40C0_0000), (8, 0), (0xC, 0), (0x20, 0), (0x24, 0x4316_0000), (0x28, 0)] {
            fx.mem.write_u32(CAM + o, v);
        }
        for k in 0..3 {
            let v = ee::from_i32(fx.mem.i16(t + 2 * k));
            fx.mem.setf(CAM + 0x10 + 4 * k, v);
        }
        fx.mem.set16(t + 0x36, 2);
        fx.mem.set16(t + 0x30, 0);
    }
    let w = fx.mem.f(CAM + 0x28);
    if fx.mem.i16(t + 0x32) < 0x1E {
        if w < 1.0 {
            fx.mem.setf(CAM + 0x28, ee::add(w, f32::from_bits(0x3E4C_CCCD)));
        }
    } else if w <= f32::from_bits(0x3DCC_CCCD) {
        fx.mem.set16(t + 0x36, 0);
    } else {
        fx.mem.setf(CAM + 0x28, ee::sub(w, f32::from_bits(0x3DCC_CCCD)));
    }
    fx.mem.write_u32(CAM + 4, 0x40C0_0000);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        fx.mem.write_u32(CAM + 8, 0);
    } else {
        fx.mem.write_u32(CAM + 8, 0x4040_0000);
    }
    let p = if fx.mem.i16(t + 0x36) == 0 { 0 } else { CAM };
    bexe::camera_slots45(fx, p);
    if fx.mem.i32(TIMER) == 1 {
        fx.mem.set16(t + 0x36, 0);
    }
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E00DE0 (D, task, side) (frame 0x1E0): quads4::ring_ticks (b477's
/// 0x1E01210) with the side test on the first vertex's z: nine strip3
/// ticks (2 degrees wide every 40 from +0x3A between radius +0x38 and that
/// + 20) turned about x, y, z by +0x10 (D+0xD8's sines) at the task's
/// point; colour +0x28, UVs D+0x94 in orientation D+0xBE, texture D+0x88;
/// drawn D+0xB8 + 1 times at its mean w on D+0x80 when on `side`'s side.
fn ring_ticks_z(fx: &mut Fx, t: u32, side: i32) {
    let sp = fx.enter(0x1E0);
    let d = sp + 0x150;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x28, 0);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3u32 {
        let v = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xE0 + 4 * k, v);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, 0x3F80_0000);
    }
    l2::xform_vp(fx, sp + 0x100, sp + 0xF0, 0, sp + 0xE0);
    for o in [0xA4u32, 0xB4, 0xC4, 0xD4] {
        fx.mem.set32(sp + o, 0);
    }
    let mut a = 0i32;
    for _ in 0..9 {
        let s = (fx.mem.i16(t + 0x3A) + a) as i16 as i32;
        let e = (a + (fx.mem.i16(t + 0x3A) + 2)) as i16 as i32;
        for (deg, inner, outer) in [(s, 0xC0u32, 0xA0u32), (e, 0xD0, 0xB0)] {
            for (v, wide) in [(inner, 0), (outer, 0x14)] {
                let c = l2::sin_deg(fx, TAB2, deg, 1);
                let r = ee::from_i32(fx.mem.i16(t + 0x38) + wide);
                fx.mem.setf(sp + v, ee::mul(r, c));
                let sn = l2::sin_deg(fx, TAB2, deg, 0);
                let r = ee::from_i32(fx.mem.i16(t + 0x38) + wide);
                fx.mem.setf(sp + v + 8, ee::mul(r, sn));
            }
        }
        for axis in 0..3 {
            l2::rotate4(fx, sp + 0xA0, t + 0x10, axis, TAB2);
        }
        let z = fx.mem.f(sp + 0xA8);
        let shown = if side == 0 { z < 0.0 } else { !(z <= 0.0) };
        if shown && l2::project(fx, sp + 0xA0, SPR, sp + 0x100, d, sp + 0x140) {
            let mut k = 0;
            while k < fx.mem.i16(D + 0xB8) + 1 {
                let key = fixu(fx, sp + 0x140);
                let ot = fx.mem.read_u32(OT_A);
                fx.submit(d, ot, key);
                k += 1;
            }
        }
        a = (a + 0x28) as i16 as i32;
    }
    fx.leave(0x1E0);
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// 0x1E011E0 (D, task, hist, off) (frame 0x290): a gouraud tube along the
/// history `hist` (+6 points, spline basis 0, four steps a segment, the
/// first segment on copies h0 h0 h1 h2) moved by the vec at `off`: per
/// step two flat quads (z +- r, then x +- r; r = +0x38 / 10) through the
/// camera at the mean w on the OT; the far pair's alpha down by +0x2B / 3n
/// a step (clamped at 0).
fn tube(fx: &mut Fx, t: u32, hist: u32, off: u32) {
    let sp = fx.enter(0x290);
    copy4(fx, sp + 0x110, off);
    let r = ee::div(ee::from_i32(fx.mem.i16(t + 0x38)), 10.0);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x1F8;
    l2::fullscreen4(fx, d);
    l2::colour_strip4(fx, d, t + 0x28, 0);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    let c = sp + 0x28C;
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x28 + k);
        fx.mem.write_u8(c + k, b);
    }
    let div = 3 * n;
    copy4(fx, sp + 0x140, hist);
    copy4(fx, sp + 0x150, hist);
    copy4(fx, sp + 0x160, hist + 0x10);
    copy4(fx, sp + 0x170, hist + 0x20);
    let (oy, oz) = (fx.mem.f(sp + 0x114), fx.mem.f(sp + 0x118));
    let mut seg = 0i32;
    while seg < n - 1 {
        let mut f20 = 0.0f32;
        let pts = if seg != 0 { hist.wrapping_add((0x10 * (seg as i16 as i32 - 1)) as u32) } else { sp + 0x140 };
        let far = if seg != 0 { sp + 0x1D0 } else { sp + 0x1E0 };
        for _ in 0..4 {
            spline(fx, sp + 0x120, pts, f20, 0, [0.0; 4]);
            f20 = ee::add(f20, 0.25);
            spline(fx, far, pts, f20, 0, [0.0; 4]);
            copy4(fx, sp + 0x130, far);
            l2::colour_strip4(fx, d, c, 1);
            let al = i32::from(fx.mem.read_u8(t + 0x2B));
            let q = fx.div(al, div).0 as i16 as i32;
            let v = i32::from(fx.mem.read_u8(c + 3)) - q;
            fx.mem.write_u8(c + 3, if v > 0 { v as u8 } else { 0 });
            l2::colour_strip4(fx, d, c, 2);
            for s0 in 0..2 {
                let (p, q) = (sp + 0x120, sp + 0x130);
                for (v, src) in [(0x180u32, p), (0x190, p), (0x1A0, q), (0x1B0, q)] {
                    for k in 0..3 {
                        let w = fx.mem.read_u32(src + 4 * k);
                        fx.mem.write_u32(sp + v + 4 * k, w);
                    }
                }
                let ox = fx.mem.f(sp + 0x110);
                l2::add_column(fx, sp + 0x180, 0, ox);
                l2::add_column(fx, sp + 0x180, 1, oy);
                l2::add_column(fx, sp + 0x180, 2, oz);
                if s0 == 0 {
                    for o in [0x188u32, 0x1A8, 0x198, 0x1B8] {
                        let v = ee::add(fx.mem.f(sp + o), r);
                        fx.mem.setf(sp + o, v);
                    }
                } else {
                    for (o, plus) in [(0x180u32, true), (0x1A0, true), (0x190, false), (0x1B0, false)] {
                        let v = if plus { ee::add(fx.mem.f(sp + o), r) } else { ee::sub(fx.mem.f(sp + o), r) };
                        fx.mem.setf(sp + o, v);
                    }
                }
                if l2::project(fx, sp + 0x180, SPR, 0, d, sp + 0x1C0) {
                    let key = fixu(fx, sp + 0x1C0);
                    fx.submit(d, OT, key);
                }
            }
        }
        seg += 1;
    }
    fx.leave(0x290);
}
