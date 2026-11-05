//! BMODULE b206 (card 276), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E01DB0, data area 0x1ED0000) [F]: b219's family (a pool of 150 tasks of 0x70 bytes at D+0x2A0,
//! D+0x4Cxx counters, the bytes at D+0x4C80) drawn with bands
//! (853ed233d9de at 0x1E00920, `quads4::ring_band_fy`) and flat facing
//! rings turned about z (2f5a28a746a2 at 0x1E00D80,
//! `quads4::ring_facing_flat_z`), plus b232's unit flash (0x1E02460 /
//! 0x1E01B80, the target drawn into the render target in a packet layer
//! under a sweeping frame sprite). On the odd updates 64..275 a widening
//! band leaves the attacker's node 0x22 (2 to the facing, x 0); below 301
//! a band pulsing with the target's radius 50 behind the target's node 1;
//! on 56 two rings at the attacker's nodes 0x3A / 0x46; on 100 the two
//! flash tasks. Returns 1 on update 140, 4 on 320, 2 on 310, 6 on
//! 151..299, 8 once the 330-update timer +0x4C10 has run out.
use crate::blib::{quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const WAVE: u32 = 0x01E0_1550;
const PULSE: u32 = 0x01E0_1770;
const RING: u32 = 0x01E0_19D0;
const FLASH: u32 = 0x01E0_1B80;
/// The return address of the entry's `jalr` into a step (0x1E02060 + 8).
const TASK_RA: u64 = 0x01E0_2068;
const TIMER: u32 = D + 0x4C10;
const COLOUR: u32 = D + 0x4C14;
const LAYER: u32 = D + 0x4C24;
const FLASH_DESC: u32 = D + 0x4C28;
const NODES: u32 = D + 0x4C48;
const TARGET_NODE: u32 = D + 0x4C56;
const SLOT: u32 = D + 0x4C76;
const NEXT: u32 = D + 0x4C78;
const FACING: u32 = D + 0x4C7A;
const COUNTER: u32 = D + 0x4C7E;
const BYTES: u32 = D + 0x4C80;
const BYTES2: u32 = D + 0x4D80;
const SINES: u32 = D + 0xD8;

/// The entry 0x1E01DB0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    fx.mem.write_u32(D + 0x4C1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4FF0);
    l2::sine_table(fx, D + 0x4E80);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x4C7C, param);
    fx.mem.set32(TIMER, 0x14A);
    for (k, node) in [0x22, 0x3A, 0x46].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(TARGET_NODE, r);
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x288 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 0);
    fx.mem.write_u32(D + 0x260, t);
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x4BF0 + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x80);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x84)), 180.0);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    // The entry's s0 is D here (rand's frame).
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_1F3C);
        let (_, h) = fx.div(v, 0x18);
        fx.mem.set8(BYTES + k, h - 0xC);
    }
    for k in 0x64..0xB4u32 {
        let v = fx.rand_from(D, 0x01E0_1F8C);
        fx.mem.set8(BYTES + k, v % 4);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(D, 0x01E0_1FDC);
        let (_, h) = fx.div(v, 0x28);
        fx.mem.set8(BYTES2 + k, h - 0x14);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => {}
            FLASH => flashes += 1,
            WAVE => wave(fx, t, flashes),
            PULSE => pulse(fx, t, flashes),
            RING => crate::b082::ring_v(fx, t, RING, TASK_RA, 0x4270_0000, 0xC0, (0xA, 5), (0xF, 9), (0xB, 9)),
            _ => panic!("b206: unknown task step {f:#x}"),
        }
    }
    // a3 as the draws leave it for the unit flash's first TEXFLUSH: 0 (the
    // bands here key by the mean w, unlike b362's far-Z band) [run].
    let a3 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x44) == WAVE {
            for k in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x60 + 4 * k, w);
            }
            let a = fx.mem.i16(t + 0x5C);
            let c = l2::sin_deg(fx, SINES, a, 1);
            let z = ee::add(fx.mem.f(sp + 0x68), ee::mul(fx.mem.f(t + 0x30), c));
            fx.mem.setf(sp + 0x68, z);
            let a = fx.mem.i16(t + 0x5C);
            let s = l2::sin_deg(fx, SINES, a, 0);
            let x = ee::add(fx.mem.f(sp + 0x60), ee::mul(fx.mem.f(t + 0x30), s));
            fx.mem.setf(sp + 0x60, x);
            quads4::ring_band_fy(fx, D, t, 0x20, sp + 0x60);
        }
        if fx.mem.read_u32(t + 0x44) == PULSE {
            for k in 0..4 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x60 + 4 * k, w);
            }
            quads4::ring_band_fy(fx, D, t, 0x20, sp + 0x60);
        }
        if fx.mem.read_u32(t + 0x44) == RING {
            let sub = fx.mem.i16(t + 0x56);
            crate::b615::cell(fx, sp + 0x70, 0x14, sub);
            let w = fx.mem.read_u32(D + 0x4BF4);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0xB8, 0xA);
            let o = fx.mem.i16(t + 0xA);
            fx.mem.set16(D + 0xC0, o);
            for k in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(D + 0x20 + 4 * k, w);
            }
            quads4::ring_facing_flat_z(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
    }
    if flashes != 0 {
        unit_flash(fx, a3);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x8C => 1,
        0x140 => 4,
        0x136 => 2,
        0x97..=0x12B => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

fn spawn(fx: &mut Fx, step: u32, saves: &[(u32, u64)]) {
    crate::b615::spawn_at(fx, step, COLOUR, SLOT, saves);
}

/// 0x1E012D0(D, 0): the update counter and the emitter (frame 0x40: ra,
/// the entry's s2 = the parameter, s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    let p = param as i64 as u64;
    fx.saved(sp, &[(0x30, 0x01E0_2028), (0x20, p), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    fx.node_position(h, slot, D + 0x250);
    let z = fx.mem.f(D + 0x258);
    let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 2.0) } else { ee::add(z, 2.0) };
    fx.mem.setf(D + 0x258, z);
    fx.mem.write_u32(D + 0x250, 0);
    // The emitter's s1 / s0 as the later spawns save them.
    let (mut s1, mut s0) = (0u64, u64::from(D));
    let c = fx.mem.i16(COUNTER);
    if (0x40..0x114).contains(&c) && c % 2 == 1 {
        let s = fx.mem.i16(NEXT);
        fx.mem.set16(SLOT, s);
        spawn(fx, WAVE, &[(0x50, 0x01E0_13C0), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, 1), (0, 0)]);
        let s = fx.mem.i16(NEXT) + 1;
        fx.mem.set16(NEXT, s);
        if fx.mem.i16(NEXT) >= 0x78 {
            fx.mem.set16(NEXT, 0);
        }
        (s1, s0) = (1, 1);
    }
    let h = fx.target_handle();
    let slot = fx.mem.i16(TARGET_NODE);
    fx.node_position(h, slot, D + 0x250);
    let z = fx.mem.f(D + 0x258);
    let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 50.0) } else { ee::sub(z, 50.0) };
    fx.mem.setf(D + 0x258, z);
    let saves = |ra: u32| [(0x50, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, u64::from(D)), (0x10, s1), (0, s0)];
    if fx.mem.i16(COUNTER) < 0x12D {
        spawn(fx, PULSE, &saves(0x01E0_1484));
    }
    if fx.mem.i16(COUNTER) == 0x64 {
        fx.mem.set16(SLOT, 0);
        spawn(fx, FLASH, &saves(0x01E0_14AC));
        fx.mem.set16(SLOT, 1);
        spawn(fx, FLASH, &saves(0x01E0_14C8));
    }
    if fx.mem.i16(COUNTER) == 0x38 {
        for (k, ra) in [(1u32, 0x01E0_1508u32), (2, 0x01E0_1538)] {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 * k);
            fx.node_position(h, slot, D + 0x250);
            fx.mem.set16(SLOT, 0);
            spawn(fx, RING, &saves(ra));
        }
    }
    fx.leave(0x40);
}

/// 0x1E01550 (D, task): a widening band (frame 0x30: ra, the entry's s1 =
/// the flashes counted so far, s0 = D): inner radius +0x48 from 15 by 7.5,
/// its width +0x34 by 1.6 below 50 (outer +0x4C = inner - width), sway
/// +0x30 by 0.2 below 10 (+0x5C by 9); white, grey 0x60 on even ages; the
/// outer alpha in by 0x40 under 0x80 for 35 ages, then out by 9; angles
/// (0x4000, 0, 0); 5 to the facing a update.
fn wave(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(flashes)), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4170_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade67(fx, t, 0x23);
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
    } else {
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x60);
    }
    fx.mem.write_u32(t + 0x20, 0x4000);
    let w = fx.mem.f(t + 0x34);
    if w < 50.0 {
        fx.mem.setf(t + 0x34, ee::add(w, f32::from_bits(0x3FCC_CCCD)));
    }
    let s = fx.mem.f(t + 0x30);
    if s < 10.0 {
        fx.mem.setf(t + 0x30, ee::add(s, f32::from_bits(0x3E4C_CCCD)));
    }
    let r = ee::add(fx.mem.f(t + 0x48), 7.5);
    fx.mem.setf(t + 0x48, r);
    let o = ee::sub(r, fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x4C, o);
    let a = fx.mem.i16(t + 0x5C) + 9;
    fx.mem.set16(t + 0x5C, a);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    if fx.mem.i16(t + 0x54) != 0 {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(t + 0x18, z);
    }
    fx.leave(0x30);
}

/// The outer alpha +0x67: in by 0x40 under 0x80 while the age is below
/// `ages`, then out by 9 (the task gone below 0xB).
fn fade67(fx: &mut Fx, t: u32, ages: i32) {
    let a = fx.mem.read_u8(t + 0x67);
    if fx.mem.i16(t + 0x54) < ages {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x67, a + 0x40);
        }
    } else if a < 0xB {
        fx.mem.set16(t + 0x58, 0);
    } else {
        fx.mem.write_u8(t + 0x67, a - 9);
    }
}

/// 0x1E01770 (D, task): a pulsing band (frame 0x40: ra, the entry's s1 =
/// the flashes counted so far, s0 = D, f20): red (0x80, 0x10, 0x10),
/// brighter (0xC0) on even ages; the outer alpha in by 0x40 for 15 ages,
/// then out by 9; inner radius 12 r sin(+0x5C) (r the target's radius,
/// 0x1F6A70), width +0x34 by 3.8 below 40; +0x5C by 9, gone past 180;
/// 2.5 + rand % 4 to the facing a update.
fn pulse(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, TASK_RA), (0x20, u64::from(flashes)), (0x10, u64::from(D))]);
    let h = fx.target_handle();
    let r = crate::blib::afterimage::unit_radius(fx, h);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4170_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0x10, 0x10);
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0x10);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    fade67(fx, t, 0xF);
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x10, 0x10);
    } else {
        l2::rgb(fx, t + 0x60, 0xC0, 0x10, 0x10);
    }
    fx.mem.write_u32(t + 0x20, 0x4000);
    let w = fx.mem.f(t + 0x34);
    if w < 40.0 {
        fx.mem.setf(t + 0x34, ee::add(w, f32::from_bits(0x4073_3333)));
    }
    let a = fx.mem.i16(t + 0x5C);
    let s = l2::sin_deg(fx, SINES, a, 0);
    let inner = ee::mul(ee::mul(12.0, r), s);
    fx.mem.setf(t + 0x48, inner);
    let o = ee::sub(inner, fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x4C, o);
    let a = fx.mem.i16(t + 0x5C) + 9;
    fx.mem.set16(t + 0x5C, a);
    if fx.mem.i16(t + 0x5C) >= 0xB5 {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    let toward = fx.mem.i16(FACING) != 0;
    let v = fx.rand_from(t, if toward { 0x01E0_1938 } else { 0x01E0_1978 });
    let by = ee::add(2.5, ee::from_i32(v % 4));
    let z = fx.mem.f(t + 0x18);
    fx.mem.setf(t + 0x18, if toward { ee::sub(z, by) } else { ee::add(z, by) });
    fx.leave(0x40);
}

/// 0x1E02460(D): b232's unit flash (0x1E01ED0) on this layout: in a packet
/// layer (0x2078A0, kept at D+0x4C24) the target drawn into the render
/// target (0x1F8270 filter 0, its TEX0 / TEX1 into the sprite descriptor at
/// sp + 0x58, kept at D+0x4C28; ALPHA 0x48), TEXFLUSH before and after;
/// every flash task stepped (0x1E01B80); the layer linked into the
/// ordering table at key 0 when any task lives.
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xB0);
    let d = sp + 0x58;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    // a0..a3 at the first 0x207BB0: 5, the descriptor, 0x26, the caller's.
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let t = fx.target_handle();
    let (_, regs) = fx.unit_draw(t, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    let mut live = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        live += 1;
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, [k, live, slot]);
        }
    }
    if live != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}

/// 0x1E01B80: b232's flash sprite (0x1E01120) with grey 0x60, submitted
/// once, x 15 cos(+0x5A) and the alpha rising for 200 updates. `regs` the
/// caller's s0..s2.
fn flash(fx: &mut Fx, t: u32, regs: [u32; 3]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_2534), (0x20, u64::from(regs[2])), (0x10, u64::from(regs[1])), (0, u64::from(regs[0]))]);
    let x0 = fx.mem.i16(t + 8);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x60);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.set16(t + 0x5A, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.set16(t + 0x5A, 0xB4);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, x, y) in [(0x20u32, x0 + 0x700, 0x6FF8u16), (0x30, x0 + 0x900, 0x8FF8)] {
        fx.mem.write_u16(d + at, ((x << 4) - 8) as u16);
        fx.mem.write_u16(d + at + 2, y);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    for (o, v) in [(0x18u32, 0u32), (0x1C, 0), (0x28, 0x3F80_0000), (0x2C, 0x3F80_0000)] {
        fx.mem.write_u32(d + o, v);
    }
    fx.submit(d, 0, 0);
    let a = fx.mem.i16(t + 0x5A);
    let c = l2::sin_deg(fx, D + 0x4E80, a, 1);
    let v = ee::mul(15.0, c);
    fx.fix_residue(v, 0x01E0_1D24);
    fx.mem.set16(t + 8, ee::fixsfsi(v));
    let a = fx.mem.i16(t + 0x5A) + 0x19;
    fx.mem.set16(t + 0x5A, a);
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xC8 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a + 0xA);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x40);
}
