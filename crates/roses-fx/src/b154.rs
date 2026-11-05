//! BMODULE b154 (card 236), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E026A0, data area 0x1ED0000) [F], the first library's family (b043 / b416 / b262): two glowing
//! ribbons from the attacker's node pairs 0x1B / 0x19 (+0x70) and 0x24 /
//! 0x22 (+0x190) on 0x1F..0x2A and 0x50..0x78 (the record's 0x1E02200:
//! eight-point histories of each node and two gouraud strips a segment
//! from the pair's midpoint out to either node, alpha falling by an eighth
//! a segment, in a packet layer); at the target from 0x57 a flashing disc
//! (+0x6A0 on +0x630, to 0x7B) and a spark burst (+0x7A0 on +0x730,
//! `trail::sparks`, each spark sped up by 1.2 and thrown again from 4 up
//! once above 30); from 0x52 between the attacker's nodes 0x19 / 0x22 a
//! second disc (+0x720 on +0x6B0, to 0x7B), a second burst (+0x1040 on
//! +0xFD0) and two growing particle clouds (+0x4E0 on +0x470 at the
//! target, by 1.01; +0x320 on +0x2B0, by 1.02), the stage light dimming
//! to 0x20 by 8 and back up to 0x80 by 8. Returns 1 on update 0x48, 6 on
//! 0x52..0x7B, 2 on 0x7C, 8 once the 165-update timer +0x1880 has run out.
//!
//! Library: 0x1E00150 `colours4`, 0x1E001F0 `particle_quads`, 0x1E00D80
//! `spawn::scatter`, 0x1E00F40 `trail::sparks`, 0x1E01CA0 `disc`; the
//! record's own 0x1E00080 (strip colours) and 0x1E02200 (the ribbon).
use crate::blib;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const TIMER: u32 = D + 0x1880;
const COUNTER: u32 = D + 0x1884;
const FLAGS: u32 = D + 0x188C;
const NODES: u32 = D + 0x1890;
const LIGHT: u32 = D + 0x18A4;

/// The entry 0x1E026A0 (s0 = the OT, s2 = D).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xF0);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xF0);
    ret
}

fn halves(fx: &mut Fx, at: u32, xs: &[(u32, i32)]) {
    for &(k, x) in xs {
        fx.mem.set16(at + k, x);
    }
}

fn bytes(fx: &mut Fx, at: u32, xs: &[(u32, u8)]) {
    for &(k, x) in xs {
        fx.mem.write_u8(at + k, x);
    }
}

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// A particle cloud's particles (count +0x130) round its origin (+0x10 /
/// +0x14 / +0x18): x + rand % 10 - 5, y + rand % 8 - 2, ticks -k x +0x13A,
/// frame 0.
fn scatter(fx: &mut Fx, g: u32) {
    for k in 0..0x10u32 {
        let p = g + 0x10 * k;
        let r = fx.rand();
        let (_, h) = fx.div(r, 0xA);
        let x = ee::add(fx.mem.f(g + 0x10), ee::from_i32(h - 5));
        fx.mem.setf(p + 0x30, x);
        let r = fx.rand();
        let y = ee::add(fx.mem.f(g + 0x14), ee::from_i32(r % 8 - 2));
        fx.mem.setf(p + 0x34, y);
        let z = fx.mem.read_u32(g + 0x18);
        fx.mem.write_u32(p + 0x38, z);
        let t = fx.mem.i16(g + 0x13A).wrapping_neg().wrapping_mul(k as i32);
        fx.mem.set16(p + 0x3E, t);
        fx.mem.set16(p + 0x3C, 0);
        let n = fx.mem.i16(g + 0x140) - 1;
        fx.mem.set16(g + 0x13E, n);
    }
}

/// A spark burst `g` (points at `g` + 0x30): flags 0x2000_0003, colour
/// (0x40, 0x40, 0xA0, 0x80), width 0.5, length 5, cell (0xB0, 0x40, 0x20,
/// 0x20), speed 5, 128 sparks, thrown by `spawn::scatter` (life 12, 4).
fn burst(fx: &mut Fx, g: u32, tex: u32) {
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x2000_0003);
    bytes(fx, g, &[(0xA, 0xA0), (9, 0x40), (8, 0x40), (0xB, 0x80)]);
    fx.mem.write_u32(g + 0x20, 0x3F00_0000);
    fx.mem.write_u32(g + 0x24, 0x40A0_0000);
    halves(fx, g, &[(0x10, 0xB0), (0x12, 0x40), (0x14, 0x20), (0x16, 0x20), (0x18, 0xB0), (0x1A, 0x40), (0x1C, 0x100), (0x1E, 0x100)]);
    fx.mem.write_u32(g + 0xC, g + 0x30);
    fx.mem.write_u32(g + 0x28, 0x40A0_0000);
    fx.mem.set16(g + 0x2C, 0x80);
    fx.mem.write_u8(g + 0x2E, 2);
    fx.mem.write_u8(g + 0x2F, 0);
    blib::spawn::scatter(fx, g, 0xC, 4.0);
}

/// Both ribbon histories (8 points of xyz at +0x10 / +0x90) set to `a` /
/// `b`.
fn fill(fx: &mut Fx, g: u32, a: u32, b: u32) {
    for k in 0..8u32 {
        for (to, from) in [(0x10u32, a), (0x90, b)] {
            for c in 0..3 {
                let w = fx.mem.read_u32(from + 4 * c);
                fx.mem.write_u32(g + to + 0x10 * k + 4 * c, w);
            }
        }
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x18B0);
    for x in [0u32, 0x2B0, 0x470, 0x630, 0x6B0, 0x730, 0xFD0] {
        fx.xform_init(D + x);
    }
    for (k, node) in [0x1B, 0x19, 0x24, 0x22].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set32(NODES + 4 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set32(NODES + 0x10, r);
    fx.mem.set32(TIMER, 0xA5);
    fx.mem.set32(COUNTER, 1);
    fx.mem.set32(D + 0x1888, param);
    fx.mem.set32(FLAGS, 0);
    fx.mem.write_u8(LIGHT, 0x80);
    for g in [D + 0x6A0, D + 0x720] {
        fx.mem.write_u32(g, 0x8_0000);
        bytes(fx, g, &[(6, 0xA0), (5, 0x40), (4, 0x40), (7, 0xFF)]);
        fx.mem.write_u32(g + 8, 0x4120_0000);
        fx.mem.set16(g + 0xC, 0x24);
        fx.mem.set16(g + 0xE, 0);
    }
    fx.texture4(sp + 0xE0, 8);
    // Cloud A (+0x4E0): 16 quads, size 0.5, scaled.
    let g = D + 0x4E0;
    let tex = fx.mem.read_u32(sp + 0xE0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x8_0085);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0), (0x2A, 0x3C), (0x24, 0xA), (0x26, 0xA), (0x2C, 0x80), (0x2E, 0x80)]);
    fx.mem.write_u32(g + 0xC, 0x3F00_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 1), (0x13C, 0), (0x140, 6), (0x136, 0xA), (0x138, 0xA)]);
    bytes(fx, g, &[(0xA, 0x80), (9, 0x80), (8, 0x80), (0xB, 0x80)]);
    scatter(fx, g);
    fx.texture4(sp + 0xE0, 0xF);
    // Cloud B (+0x320): 16 quads, size 6.
    let g = D + 0x320;
    let tex = fx.mem.read_u32(sp + 0xE0);
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x8_0481);
    for k in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x28, 0xF0), (0x2A, 0), (0x24, 0x10), (0x26, 0x10), (0x2C, 0x100), (0x2E, 0x100)]);
    fx.mem.write_u32(g + 0xC, 0x40C0_0000);
    halves(fx, g, &[(0x130, 0x10), (0x13A, 3), (0x13C, 2), (0x140, 8), (0x136, 0xA), (0x138, 0xA)]);
    bytes(fx, g, &[(0xA, 0x40), (9, 0x80), (8, 0x60), (0xB, 0x30)]);
    scatter(fx, g);
    let tex = fx.mem.read_u32(sp + 0xE0);
    burst(fx, D + 0x7A0, tex);
    let tex = fx.mem.read_u32(sp + 0xE0);
    burst(fx, D + 0x1040, tex);
    for (g, na, nb, a, b) in [(D + 0x70, 0u32, 4u32, sp + 0xA0, sp + 0xB0), (D + 0x190, 8, 0xC, sp + 0xC0, sp + 0xD0)] {
        let tex = fx.mem.read_u32(sp + 0xE0);
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 4, 1);
        halves(fx, g, &[(0x110, 0x8E), (0x112, 0x54), (0x114, 0x20), (0x116, 1)]);
        bytes(fx, g, &[(0xA, 0x20), (9, 0x20), (8, 0x60), (0xB, 0xFF)]);
        for (slot, out) in [(na, a), (nb, b)] {
            let h = fx.self_handle();
            let s = fx.mem.i32(NODES + slot);
            fx.node_position(h, s, out);
        }
        fill(fx, g, a, b);
    }
    0
}

/// The spark respawn: each live spark (life +0xE >= 0) of the burst `g`
/// sped up by 1.2; one above 30 thrown again from 4 up along a random
/// angle at 0.1 (rand % (10 x speed)), its life -(rand % 12). `ras`: the
/// return addresses of 0x1A2B80, the three rands, the cos / sin (not
/// modelled).
fn respawn(fx: &mut Fx, g: u32, ras: [u32; 6]) {
    let mut k = 0i32;
    while k < fx.mem.i16(g + 0x2C) {
        let p = g + 0x30 + 0x10 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        let s = f32::from_bits(0x3F99_999A);
        for c in 0..3u32 {
            let v = ee::mul(fx.mem.f(p + 4 * c), s);
            fx.mem.setf(p + 4 * c, v);
        }
        if ee_le(fx.mem.f(p + 4), 30.0) {
            continue;
        }
        let v = ee::mul(10.0, fx.mem.f(g + 0x28));
        fx.fix_residue(v, ras[0]);
        let n = ee::fixsfsi(v);
        let a = fx.rand_from(OT, ras[1]) & 0xFFFF;
        let r = fx.rand_from(OT, ras[2]);
        let (_, h) = fx.div(r, n);
        fx.mem.write_u32(p + 4, 0x4080_0000);
        let f20 = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
        let c = fx.table_trig(a, true);
        fx.mem.setf(p + 8, ee::mul(f20, c));
        let sn = fx.table_trig(a, false);
        fx.mem.setf(p, ee::mul(f20, sn));
        let r = fx.rand_from(OT, ras[5]);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let node = |fx: &mut Fx, slot: u32, out: u32| {
        let h = fx.self_handle();
        let s = fx.mem.i32(NODES + slot);
        fx.node_position(h, s, out);
    };
    node(fx, 0, sp + 0xA0);
    node(fx, 4, sp + 0xB0);
    node(fx, 8, sp + 0xC0);
    node(fx, 0xC, sp + 0xD0);
    if fx.mem.i32(COUNTER) == 0x52 {
        let h = fx.target_handle();
        fx.unit_position(h, D + 0x490);
        fx.midpoint(sp + 0xB0, sp + 0xD0, sp + 0x90);
        copy4(fx, D + 0x2D0, sp + 0x90);
        fx.look_angles(sp + 0xA0, sp + 0xB0, D + 0xFE0);
        copy4(fx, D + 0x6D0, sp + 0xB0);
        copy4(fx, D + 0xFF0, D + 0x6D0);
    }
    if fx.mem.i32(COUNTER) == 0x57 {
        fx.look_angles(sp + 0xC0, sp + 0xD0, D + 0x740);
        copy4(fx, D + 0x650, sp + 0xD0);
        copy4(fx, D + 0x750, D + 0x650);
    }
    // s0 = the OT, s1 = sp + 0xD0, s2 = D, s3 = sp + 0xB0.
    let regs = [Some(OT), Some(sp + 0xD0), Some(D), Some(sp + 0xB0)];
    fx.xform_world_saving(D, 0x01E0_2ECC, regs);
    fx.xform_billboard_saving(D + 0x2B0, 0x01E0_2ED4, regs);
    fx.xform_billboard_saving(D + 0x470, 0x01E0_2EDC, regs);
    fx.xform_billboard_saving(D + 0x630, 0x01E0_2EE4, regs);
    fx.xform_billboard_saving(D + 0x6B0, 0x01E0_2EEC, regs);
    fx.xform_world_saving(D + 0x730, 0x01E0_2EF4, regs);
    fx.xform_world_saving(D + 0xFD0, 0x01E0_2EFC, regs);
    let c = fx.mem.i32(COUNTER);
    if (0x1F..0x2B).contains(&c) || (0x50..0x79).contains(&c) {
        ribbon(fx, D + 0x70, D, sp + 0xA0, sp + 0xB0);
        ribbon(fx, D + 0x190, D, sp + 0xC0, sp + 0xD0);
    } else {
        fill(fx, D + 0x70, sp + 0xA0, sp + 0xB0);
        fill(fx, D + 0x190, sp + 0xC0, sp + 0xD0);
    }
    let c = fx.mem.i32(COUNTER);
    if c >= 0x57 {
        if c < 0x7C {
            fx.disc(D + 0x6A0, D + 0x630);
            let odd = fx.mem.i32(COUNTER) % 2 == 1;
            fx.mem.write_u8(D + 0x6A7, if odd { 0xFF } else { 0x80 });
        }
        blib::trail::sparks(fx, OT, D + 0x7A0, D + 0x730);
        if fx.mem.i32(COUNTER) >= 0x7D {
            fx.step_down(D + 0x7AB, 0, 0x10);
        }
        respawn(fx, D + 0x7A0, [0x01E0_3100, 0x01E0_3108, 0x01E0_3124, 0x01E0_3158, 0x01E0_3168, 0x01E0_3174]);
    }
    let c = fx.mem.i32(COUNTER);
    if c >= 0x52 {
        if c < 0x7C || fx.mem.read_u8(LIGHT) != 0x80 {
            let l = fx.mem.read_u8(LIGHT);
            fx.set_stage_light(i32::from(l));
            let f = fx.mem.read_u32(FLAGS);
            if !bit(f, 0x1F) {
                if fx.step_down(LIGHT, 0x20, 8) == 0 {
                    let f = fx.mem.read_u32(FLAGS);
                    fx.mem.write_u32(FLAGS, f | 0x8000_0000);
                }
            } else if fx.step_up(LIGHT, 0x80, 8) == 0 {
                let f = fx.mem.read_u32(FLAGS);
                fx.mem.write_u32(FLAGS, f & 0x7FFF_FFFF);
            }
        }
        if fx.mem.i32(COUNTER) < 0x7C {
            fx.disc(D + 0x720, D + 0x6B0);
            let odd = fx.mem.i32(COUNTER) % 2 != 0;
            fx.mem.write_u8(D + 0x727, if odd { 0x80 } else { 0xFF });
        }
        blib::trail::sparks(fx, OT, D + 0x1040, D + 0xFD0);
        if fx.mem.i32(COUNTER) >= 0x7D {
            fx.step_down(D + 0x104B, 0, 0x10);
        }
        respawn(fx, D + 0x1040, [0x01E0_3344, 0x01E0_334C, 0x01E0_3368, 0x01E0_339C, 0x01E0_33AC, 0x01E0_33B8]);
        for (s, x, g) in [(0x3F81_47AEu32, 0x470u32, 0x4E0u32), (0x3F82_8F5C, 0x2B0, 0x320)] {
            for o in [0x88u32, 0x84, 0x80] {
                fx.mem.write_u32(sp + o, s);
            }
            fx.particle_quads(D + x, D + g, sp + 0x80);
            fx.step_down(D + g + 0xB, 0, 2);
        }
    }
    let n = fx.mem.i32(COUNTER) + 1;
    fx.mem.set32(COUNTER, n);
    let left = fx.mem.i32(TIMER) - 1;
    fx.mem.set32(TIMER, left);
    let c = fx.mem.i32(COUNTER);
    if c == 0x7C {
        2
    } else if c == 0x48 {
        1
    } else if (0x52..0x7C).contains(&c) {
        6
    } else if fx.mem.i32(TIMER) > 0 {
        0
    } else {
        8
    }
}

/// 0x1E00080(d, c, k): a ribbon strip's colours: (c+2, c+1, c+0) at all
/// four vertices, alpha c+3 - k (c+3 >> 3) (0 for k = 0), c+3 - (k + 1)
/// (c+3 >> 3) (0 for k = 6), 0, 0; Q 1.
pub(crate) fn strip_colours(fx: &mut Fx, d: u32, c: u32, k: i32) {
    let a = i32::from(fx.mem.read_u8(c + 3));
    let step = a >> 3;
    for v in 0..4u32 {
        let at = d + 0x10 + 0x10 * v;
        for (o, from) in [(0u32, 2u32), (1, 1), (2, 0)] {
            let b = fx.mem.read_u8(c + from);
            fx.mem.write_u8(at + o, b);
        }
        // The record's `mult` leaves LO / HI for a later division by zero
        // (trail::sparks' frame step) [run].
        let al = match v {
            0 if k != 0 => a - fx.mult(k, step),
            1 if k != 6 => a - (step + fx.mult(k, step)),
            _ => 0,
        };
        fx.mem.write_u8(at + 3, al as u8);
        fx.mem.write_u32(at + 4, 0x3F80_0000);
    }
}

/// 0x1E02200(g, x, a, b) (frame 0x140): both histories (+0x10 / +0x90, 8
/// points newest first) shift up one, point 0 = `a` / `b` (xyz); in a
/// packet layer, for each of the 7 segments two untextured gouraud strips
/// (mid k, mid k + 1, A k, A k + 1) and (mid k, mid k + 1, B k, B k + 1),
/// mid the histories' midpoint (0x1036E0), coloured by 0x1E00080, the
/// corners through `x` + 0x30 (0x204D80, w at sp + 0xD0..0xDC, corner bits
/// 0x14..0x17 of +4), drawn with no ordering table keyed by the mean w
/// when all four project; corner bits cleared.
pub(crate) fn ribbon(fx: &mut Fx, g: u32, x: u32, a: u32, b: u32) {
    let sp = fx.enter(0x140);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xAC, layer);
    let mut k = 6i32;
    while k >= 0 {
        let v = g + 0x60 - 0x10 * (6 - k) as u32;
        for o in [0x10u32, 0x90] {
            copy4(fx, v + o + 0x10, v + o);
        }
        if k == 0 {
            for c in 0..3 {
                let w = fx.mem.read_u32(a + 4 * c);
                fx.mem.write_u32(v + 0x10 + 4 * c, w);
                let w = fx.mem.read_u32(b + 4 * c);
                fx.mem.write_u32(v + 0x90 + 4 * c, w);
            }
        }
        k -= 1;
    }
    let d = sp + 0xE8;
    fx.desc_init(d, 8, 0x14, 0x4141_4141);
    fx.desc_alpha(d, 0x80_0000_0048);
    for k in 0..7u32 {
        let (p0, p1) = (g + 0x10 * k, g + 0x10 * (k + 1));
        fx.midpoint(p0 + 0x10, p0 + 0x90, sp + 0xB0);
        fx.midpoint(p1 + 0x10, p1 + 0x90, sp + 0xC0);
        for side in [0x10u32, 0x90] {
            strip_colours(fx, d, g + 8, k as i32);
            let corners = [(sp + 0xB0, sp + 0x100, sp + 0xD0), (sp + 0xC0, sp + 0x110, sp + 0xD4), (p0 + side, sp + 0x120, sp + 0xD8), (p1 + side, sp + 0x130, sp + 0xDC)];
            for (bitn, (v, xyz, w)) in (0x14u32..).zip(corners) {
                let m = fx.read_m4(x + 0x30);
                if fx.project_at(v, &m, xyz, w) {
                    fx.flag(g + 4, bitn);
                }
            }
            let flags = fx.mem.read_u32(g + 4);
            if (0x14..0x18).all(|n| bit(flags, n)) {
                let s = ee::add(fx.mem.f(sp + 0xD0), fx.mem.f(sp + 0xD4));
                let s = ee::add(fx.mem.f(sp + 0xD8), s);
                let s = ee::add(fx.mem.f(sp + 0xDC), s);
                let key = ee::fixunssfsi(ee::div(s, 4.0)) as i32;
                fx.submit(d, 0, key);
            }
            let f = fx.mem.read_u32(g + 4) & 0xFF0F_FFFF;
            fx.mem.write_u32(g + 4, f);
        }
    }
    let layer = fx.mem.read_u32(sp + 0xAC);
    fx.layer_end(layer, OT, 0);
    fx.leave(0x140);
}
