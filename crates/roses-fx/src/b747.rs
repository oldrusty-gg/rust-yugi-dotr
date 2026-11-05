//! BMODULE b747 (card 363), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02F80, data area 0x1ED0000) [F] (helper A16, 31
//! Oct 2026): b491 / b247's task pool (150 x 0x70 at +0x2A0, alive +0x58,
//! step +0x44 through `jalr`; spawned by 0x1E03A10, b494's spawn with the
//! index +0x4512) on the second library (`uv_rect`, `ring_quads`,
//! `ring_band_fy`, `arc_strips_flat`, `sprite_quads` and the turned quad
//! 8bce3c06cdde at 0x1E01A00, `billboard::quad_b_turned`), with b491's
//! unit flash (0x1E03B60: the attacker drawn into the target in a packet
//! layer, then every flash task 0x1E02D20 stepped). The attacker's node
//! 0x1A is followed to +0x4470 (slid to the target's first node over
//! updates 155..186, copied to +0x250 on 186); on update 40 two sprite
//! clusters (0x1E026D0, `sprite_quads` at the four nodes) and a quad
//! cluster (0x1E02860, six turned quads at the same nodes); from update
//! 100 the nodes are 0x11 / 0x1C / 0x26; on 109..360 every third update a
//! green sway band (0x1E02170, `ring_band_fy`) at one of the three nodes
//! (by update % 3); on 109 the unit flash (0x1E02D20) and a full-window
//! frame tint (0x1E02B40, (0x78, 0x80, 0x78)); on 160..380 every fourth
//! update an arc (0x1E02420, `arc_strips_flat`, 24 strips) and a grey
//! mote ring (0x1E02500, `ring_quads` of 10 from the bytes +0x461C) at the
//! target's first node jittered +-8 / +-5. The ring-of-quads step 0x1E02940
//! is never spawned (and its draw case submits nothing): not ported.
//! Returns 1 on update 121, 4 on 405, 2 on 395, 6 on 132..389, 8 once the
//! 419-update timer +0x44B0 has run out.
use crate::b076::{node_position_saving, request_node_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 150;
/// The step functions a task holds (+0x44), as the record's addresses.
const BAND: u32 = 0x01E0_2170;
const ARC: u32 = 0x01E0_2420;
const MOTE: u32 = 0x01E0_2500;
const SPRITES: u32 = 0x01E0_26D0;
const QUADS: u32 = 0x01E0_2860;
const RINGQ: u32 = 0x01E0_2940;
const TINT: u32 = 0x01E0_2B40;
const FLASH: u32 = 0x01E0_2D20;
/// The return address of the entry's `jalr` into a step (0x1E031E0 + 8).
const TASK_RA: u64 = 0x01E0_31E8;
/// The return address of the entry's call into the emitter 0x1E01D00.
const EMIT_RA: u64 = 0x01E0_31A8;
/// The return address of the unit flash's call into a flash step.
const FLASH_RA: u64 = 0x01E0_3C2C;
const SPAWN_AT: u32 = D + 0x250;
const TEX_A: u32 = D + 0x29C;
const FOLLOW: u32 = D + 0x4470;
const SLIDE: u32 = D + 0x4484;
const TEX: u32 = D + 0x4490;
const TIMER: u32 = D + 0x44B0;
const COLOUR: u32 = D + 0x44B4;
const LAYER: u32 = D + 0x44C4;
const FLASH_DESC: u32 = D + 0x44C8;
const NODE_A: u32 = D + 0x44E4;
const NODE_B: u32 = D + 0x44E6;
const NODE_C: u32 = D + 0x44E8;
const NODE_D: u32 = D + 0x44EA;
const TNODE: u32 = D + 0x44F4;
const SPIN: u32 = D + 0x4506;
const ANGLE: u32 = D + 0x450C;
const INDEX: u32 = D + 0x4512;
const FACING: u32 = D + 0x4516;
const COUNTER: u32 = D + 0x451A;
const BYTES: u32 = D + 0x451C;
const BYTES2: u32 = D + 0x461C;
const SINES: u32 = D + 0xD8;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// The entry 0x1E02F80 (a0 = D, a1 = param; s0 = D, s1 = s2 = 0, s3 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0x44BC, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x4890);
    l2::sine_table(fx, D + 0x471C);
    l2::sine_table(fx, SINES);
    fx.mem.set16(D + 0x4518, param);
    fx.mem.set32(TIMER, 0x1A3);
    for (node, clear, at) in [(0x1A, 1, NODE_A), (0x1B, 0, NODE_B), (0x11, 0, NODE_C), (0x27, 0, NODE_D)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set16(at, r);
    }
    // The last request's frame is a later frame tint's missing-texture
    // slot (A15's lesson): s0 = D, s1 = s2 = 0.
    let h = fx.target_handle();
    let r = request_node_saving(fx, h, 1, 1, [Some(u64::from(D)), Some(0), Some(0)], 0x01E0_305C);
    fx.mem.set16(TNODE, r);
    let t = fx.texture(4, 0xF);
    fx.mem.write_u32(TEX_A, t);
    for k in 0..2u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_30EC);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_3114, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_315C, 0x28);
        fx.mem.set8(BYTES2 + k, v - 0x14);
    }
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s1: the flash tasks counted so far (saved by the steps).
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
            BAND => band(fx, t, flashes),
            ARC => arc(fx, t, flashes),
            MOTE => mote(fx, t),
            SPRITES => sprites(fx, t),
            QUADS => quads(fx, t),
            TINT => tint(fx, t, flashes),
            _ => panic!("b747: unknown task step {f:#x}"),
        }
    }
    // a3 as the draws' last library call left it, for the unit flash's
    // first TEXFLUSH (b491's lesson): 0 after every draw here, the band
    // included (ring_band_fy at depth mode 0 keys by the mean w; b491's
    // ring_band_f at mode 1 left the last XYZ2) [run].
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        let at_task = |fx: &mut Fx| {
            for k in 0..4 {
                let v = fx.mem.read_u32(t + 0x10 + 4 * k);
                fx.mem.write_u32(sp + 0x60 + 4 * k, v);
            }
        };
        if f == BAND {
            at_task(fx);
            let kind = fx.mem.i16(t + 0x50);
            let deg = fx.mem.i16(t + 0x5C);
            if kind == 1 {
                let c = l2::sin_deg(fx, SINES, deg, 1);
                let v = ee::add(fx.mem.f(sp + 0x64), ee::mul(3.0, ee::mul(fx.mem.f(t + 0x38), c)));
                fx.mem.setf(sp + 0x64, v);
            }
            let kind = fx.mem.i16(t + 0x50);
            if kind == 0 {
                let deg = fx.mem.i16(t + 0x5C);
                let c = l2::sin_deg(fx, SINES, deg, 1);
                let v = ee::add(fx.mem.f(sp + 0x60), ee::mul(6.0, ee::mul(fx.mem.f(t + 0x38), c)));
                fx.mem.setf(sp + 0x60, v);
            }
            let kind = fx.mem.i16(t + 0x50);
            if kind == 2 {
                let deg = fx.mem.i16(t + 0x5C);
                let c = l2::sin_deg(fx, SINES, deg, 1);
                let v = ee::sub(fx.mem.f(sp + 0x60), ee::mul(6.0, ee::mul(fx.mem.f(t + 0x38), c)));
                fx.mem.setf(sp + 0x60, v);
            }
            quads4::ring_band_fy(fx, D, t, 0x20, sp + 0x60);
        }
        if fx.mem.read_u32(t + 0x44) == ARC {
            fx.mem.set16(D + 0xB8, 0x18);
            billboard::arc_strips_flat(fx, D, t, 0x21, t + 0x10);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x44) == MOTE {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x70, 0x14, sub);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES2);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == SPRITES {
            let w = fx.mem.read_u32(TEX_A);
            fx.mem.write_u32(D + 0x88, w);
            cluster(fx, sp, t, flashes, 0x80, 0x9A, [0x01E0_3434, 0x01E0_34A8, 0x01E0_34D4, 0x01E0_353C, 0x01E0_35A4, 0x01E0_360C], |fx, t| quads4::sprite_quads(fx, D, t, 0, sp + 0x60));
        }
        if fx.mem.read_u32(t + 0x44) == QUADS {
            let w = fx.mem.read_u32(TEX + 4);
            fx.mem.write_u32(D + 0x88, w);
            cluster(fx, sp, t, flashes, 0x90, 0x14, [0x01E0_368C, 0x01E0_3700, 0x01E0_372C, 0x01E0_3794, 0x01E0_37FC, 0x01E0_3864], |fx, t| billboard::quad_b_turned(fx, uv::uv_rect, D, t, 0x21, sp + 0x60));
        }
        if fx.mem.read_u32(t + 0x44) == RINGQ {
            // The draw case sets the texture, cell and counts but submits
            // nothing (no call compiled); the step is never spawned.
            let w = fx.mem.read_u32(TEX_A);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xA0, 0x97, sub);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
    }
    if flashes != 0 {
        unit_flash(fx, 0);
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x79 => 1,
        0x195 => 4,
        0x18B => 2,
        0x84..=0x185 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The six draws of a sprite / quad cluster at the attacker's nodes (the
/// position at sp + 0x60, the atlas cell `id` into `r`): node A, node B,
/// node C x - 1 (y + 0.5 when facing) / x + 1 (y + 0.5 when not), node D
/// x - 1 (y + 0.5 when not facing) / x + 1 (y + 0.5 when facing).
/// The node positions are read from the entry's depth, so their frames
/// (s0 = D, s1 = the flashes, `ra`) are the quads' missing-texture TEX1
/// (sp - 0x30 [run: D]) and a later frame tint's (sp - 0x10).
#[allow(clippy::too_many_arguments)]
fn cluster(fx: &mut Fx, sp: u32, t: u32, flashes: u32, r: u32, id: i32, ras: [u32; 6], draw: impl Fn(&mut Fx, u32)) {
    let s1 = Some(u64::from(flashes));
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_A);
    node_position_saving(fx, h, slot, sp + 0x60, u64::from(D), s1, ras[0]);
    cell(fx, sp + r, id, 0);
    fx.mem.set16(D + 0xBE, 0);
    fx.mem.write_u32(sp + 0x6C, 0);
    draw(fx, t);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODE_B);
    node_position_saving(fx, h, slot, sp + 0x60, u64::from(D), s1, ras[1]);
    draw(fx, t);
    for (k, (node, dx, when_facing)) in [(NODE_C, -1.0f32, true), (NODE_C, 1.0, false), (NODE_D, -1.0, false), (NODE_D, 1.0, true)].into_iter().enumerate() {
        let h = fx.self_handle();
        let slot = fx.mem.i16(node);
        node_position_saving(fx, h, slot, sp + 0x60, u64::from(D), s1, ras[k + 2]);
        let x = ee::add(fx.mem.f(sp + 0x60), dx);
        fx.mem.setf(sp + 0x60, x);
        if (fx.mem.i16(FACING) != 0) == when_facing {
            let y = ee::add(fx.mem.f(sp + 0x64), 0.5);
            fx.mem.setf(sp + 0x64, y);
        }
        draw(fx, t);
    }
}

/// The atlas cell `id` frame `sub` (0x106960 into `r`) copied to
/// D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

/// 0x1E03A10(D, step, 0): the first free task cleared and set off at
/// +0x250 with the colour +0x44B4 twice, angle +0x4506 (+0x5A), index
/// +0x4512 (+0x50). `saves`: its prologue's (0x60: ra, s4 .. s0).
fn spawn(fx: &mut Fx, f: u32, saves: &[(u32, u64)]) -> i32 {
    let sp = fx.enter(0x60);
    fx.saved(sp, saves);
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
            l2::to_i16x4(fx, t, SPAWN_AT, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(SPAWN_AT + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let a = fx.mem.i16(SPIN);
            fx.mem.set16(t + 0x5A, a);
            let a = fx.mem.i16(INDEX);
            fx.mem.set16(t + 0x50, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, f);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E01D00(D, 0): the update counter, the followed node, the spawns and
/// the angle's turn. Frame 0x40 (ra, the entry's s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x10, EMIT_RA), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The spawns' saves: s0 = D, the entry's s1 = s2 = 0, s3 = the
    // parameter, s4 the runner's (0 [H, as b491]).
    let p = param as i64 as u64;
    let saves = |ra: u32| [(0x50u32, u64::from(ra)), (0x40, 0), (0x30, p), (0x20, 0), (0x10, 0), (0, u64::from(D))];
    let c = fx.mem.i16(COUNTER);
    if c < 0x9B {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE_A);
        node_position_saving(fx, h, slot, FOLLOW, u64::from(D), Some(0), 0x01E0_1D44);
    } else {
        if c == 0x9B {
            let h = fx.target_handle();
            let slot = fx.mem.i16(TNODE);
            node_position_saving(fx, h, slot, sp + 0x20, u64::from(D), Some(0), 0x01E0_1D70);
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x28), fx.mem.f(FOLLOW + 8)), 32.0);
            fx.mem.setf(SLIDE + 4, v);
            let v = ee::div(ee::sub(fx.mem.f(sp + 0x24), fx.mem.f(FOLLOW + 4)), 32.0);
            fx.mem.setf(SLIDE, v);
        }
        if fx.mem.i16(COUNTER) < 0xBA {
            let v = ee::add(fx.mem.f(FOLLOW + 8), fx.mem.f(SLIDE + 4));
            fx.mem.setf(FOLLOW + 8, v);
            let v = ee::add(fx.mem.f(FOLLOW + 4), fx.mem.f(SLIDE));
            fx.mem.setf(FOLLOW + 4, v);
        }
    }
    if fx.mem.i16(COUNTER) == 0xBA {
        for k in 0..4 {
            let w = fx.mem.read_u32(FOLLOW + 4 * k);
            fx.mem.write_u32(SPAWN_AT + 4 * k, w);
        }
        fx.mem.set16(INDEX, 0);
    }
    if fx.mem.i16(COUNTER) == 0x64 {
        for (node, clear, at) in [(0x11, 1, NODE_A), (0x1C, 0, NODE_B), (0x26, 0, NODE_C)] {
            let h = fx.self_handle();
            let r = fx.request_node(h, node, clear);
            fx.mem.set16(at, r);
        }
    }
    if fx.mem.i16(COUNTER) == 0x28 {
        fx.mem.set16(INDEX, 0);
        fx.mem.set16(SPIN, 0);
        spawn(fx, SPRITES, &saves(0x01E0_1E8C));
        fx.mem.set16(INDEX, 1);
        fx.mem.set16(SPIN, 0x5A);
        spawn(fx, SPRITES, &saves(0x01E0_1EB0));
        spawn(fx, QUADS, &saves(0x01E0_1EC4));
    }
    let c = fx.mem.i16(COUNTER);
    if (0x6D..0x169).contains(&c) {
        let (_, r) = fx.div(c, 3);
        let (node, ra, ra_pos) = match r {
            0 => (NODE_A, 0x01E0_1F28, 0x01E0_1F14),
            1 => (NODE_B, 0x01E0_1F68, 0x01E0_1F54),
            _ => (NODE_C, 0x01E0_1FA8, 0x01E0_1F94),
        };
        fx.mem.set16(INDEX, r);
        let h = fx.self_handle();
        let slot = fx.mem.i16(node);
        node_position_saving(fx, h, slot, SPAWN_AT, u64::from(D), Some(0), ra_pos);
        spawn(fx, BAND, &saves(ra));
    }
    if fx.mem.i16(COUNTER) == 0x6D {
        spawn(fx, FLASH, &saves(0x01E0_1FCC));
    }
    if fx.mem.i16(COUNTER) == 0x6D {
        spawn(fx, TINT, &saves(0x01E0_1FF0));
    }
    let c = fx.mem.i16(COUNTER);
    if (0x9F..0x17D).contains(&c) && c % 4 == 0 {
        let h = fx.target_handle();
        let slot = fx.mem.i16(TNODE);
        node_position_saving(fx, h, slot, sp + 0x20, u64::from(D), Some(0), 0x01E0_2040);
        for k in 0..4 {
            let w = fx.mem.read_u32(sp + 0x20 + 4 * k);
            fx.mem.write_u32(SPAWN_AT + 4 * k, w);
        }
        for (o, ra, m, half) in [(0u32, 0x01E0_2068u32, 16, 8), (4, 0x01E0_209C, 16, 8), (8, 0x01E0_20D0, 10, 5)] {
            let v = rmod(fx, D, ra, m);
            let p = ee::add(fx.mem.f(SPAWN_AT + o), ee::from_i32(v - half));
            fx.mem.setf(SPAWN_AT + o, p);
        }
        spawn(fx, ARC, &saves(0x01E0_210C));
        spawn(fx, MOTE, &saves(0x01E0_2120));
    }
    let a = fx.mem.i16(ANGLE) + 2;
    fx.mem.set16(ANGLE, a);
    let a = fx.mem.i16(ANGLE);
    if a >= 0x168 {
        let (_, h) = fx.div(a, 0x168);
        fx.mem.set16(ANGLE, h);
    }
    fx.leave(0x40);
}

/// The alpha byte `o`'s fade: in by `up.1` below `up.0` while `rising`,
/// else out by `down.1` while at least `down.0`, else the task ends.
fn fade(fx: &mut Fx, t: u32, o: u32, rising: bool, up: (u8, u8), down: (u8, u8)) {
    let a = fx.mem.read_u8(t + o);
    if rising {
        if a < up.0 {
            fx.mem.write_u8(t + o, a + up.1);
        }
    } else if a >= down.0 {
        fx.mem.write_u8(t + o, a - down.1);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// A 0x20-frame step's prologue (ra, the entry's s0 = D).
fn frame20(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
}

/// 0x1E02170 (D, task; frame 0x30: ra, the entry's s1 = the flashes so
/// far, s0 = D): a green sway band (`ring_band_fy`; the inner colour
/// flickering (0x80, 0xFF, 0x80) / (0x60, 0xC0, 0x60), outer grey 0x80,
/// alpha +0x67 in by 0x40 under 0x80 for 35 updates, then out by 9) from
/// radius 7 widening by 6 (the inner 13 / 14 less by the facing), turned
/// 0x4000, at the angle slot +0x450C turning 9 a update, swaying to +-50
/// in +0x34 by 1.6 and up to 10 in +0x38 by 0.2, sinking / rising 5 in z
/// by the facing.
fn band(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(flashes)), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40E0_0000);
        l2::rgb(fx, t + 0x60, 0x80, 0xFF, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        let a = fx.mem.i16(ANGLE);
        fx.mem.set16(t + 0x5C, a);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 0x23;
    fade(fx, t, 0x67, rising, (0x80, 0x40), (0xB, 9));
    if fx.mem.i16(t + 0x54).wrapping_rem(2) == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0xFF, 0x80);
    } else {
        l2::rgb(fx, t + 0x60, 0x60, 0xC0, 0x60);
    }
    fx.mem.write_u32(t + 0x20, 0x4000);
    let facing = fx.mem.i16(FACING) != 0;
    let x = fx.mem.f(t + 0x34);
    if facing {
        if x < 50.0 {
            fx.mem.setf(t + 0x34, ee::add(x, f32::from_bits(0x3FCC_CCCD)));
        }
    } else if !(x <= -50.0) {
        fx.mem.setf(t + 0x34, ee::sub(x, f32::from_bits(0x3FCC_CCCD)));
    }
    let y = fx.mem.f(t + 0x38);
    if y < 10.0 {
        fx.mem.setf(t + 0x38, ee::add(y, f32::from_bits(0x3E4C_CCCD)));
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = ee::add(fx.mem.f(t + 0x48), 6.0);
        fx.mem.setf(t + 0x48, r);
        let less = if fx.mem.i16(FACING) != 0 { 13.0 } else { 14.0 };
        let r = ee::sub(fx.mem.f(t + 0x48), less);
        fx.mem.setf(t + 0x4C, r);
    }
    let a = fx.mem.i16(t + 0x5C) + 9;
    fx.mem.set16(t + 0x5C, a);
    age(fx, t);
    if fx.mem.i16(t + 0x54) != 0 {
        let z = fx.mem.f(t + 0x18);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, 5.0) } else { ee::add(z, 5.0) };
        fx.mem.setf(t + 0x18, z);
    }
    fx.leave(0x30);
}

/// 0x1E02420 (D, task): an arc (`arc_strips_flat`; white / pale green,
/// radius 50, the outer +0x4C 0, angle 15 spanning 15) turned by the
/// camera's angles (the first + 0x4000); alpha 0xC0 for 3 updates, then
/// out by 11.
fn arc(fx: &mut Fx, t: u32, flashes: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4248_0000);
        fx.mem.write_u32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0xFF, 0x80);
        fx.mem.set16(t + 0x5A, 0xF);
        fx.mem.set16(t + 0x5C, 0xF);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 3 {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0xD, 0xB));
    }
    // 0x205580's saves: the step's s0 = the task, s1 the entry's.
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(flashes), 0x01E0_24CC);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02500 (D, task): a grey mote ring (`ring_quads` of 10 from the
/// bytes +0x461C): size 15..24, cell 4, random angles, radius 10..59
/// growing by 20..49, row 1..8, life 2..4; alpha 0x60 during the life,
/// then out by 11.
fn mote(fx: &mut Fx, t: u32) {
    frame20(fx);
    let f = MOTE;
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 0xA);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xF));
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        for (o, site) in [(0x20u32, f + 0x68), (0x24, f + 0x84), (0x28, f + 0xA0)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = rmod(fx, t, f + 0xBC, 0x32);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, f + 0xDC, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xF8, 0x1E);
        fx.mem.set16(t + 0xE, v + 0x14);
        let v = rmod(fx, t, f + 0x118, 8);
        fx.mem.set16(t + 8, v + 1);
        let v = rmod(fx, t, f + 0x138, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        fade(fx, t, 0x63, false, (0, 0), (0xD, 0xB));
    }
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E026D0 (D, task): a sprite cluster (`sprite_quads`, grey 0xC0 / green
/// (0x10, 0xFF, 0x10), +0x48 6): by the index +0x50, 0: size 4.5 x 10
/// growing 8 a update, angle 0; 1: 4 x 5 growing 5, angle 90; the angle
/// turning 6 a update; in by 0x40 under 0x80 for 4 updates, then out by 9.
fn sprites(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x64, 0x10, 0xFF, 0x10);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x30, 0x4090_0000);
            fx.mem.write_u32(t + 0x34, 0x4120_0000);
            fx.mem.set16(t + 0x5A, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x30, 0x4080_0000);
            fx.mem.write_u32(t + 0x34, 0x40A0_0000);
            fx.mem.set16(t + 0x5A, 0x5A);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 4;
    fade(fx, t, 0x63, rising, (0x80, 0x40), (0xB, 9));
    if fx.mem.i16(t + 0x50) == 0 && fx.mem.i16(t + 0x54) != 0 {
        let h = ee::add(fx.mem.f(t + 0x34), 8.0);
        fx.mem.setf(t + 0x34, h);
    }
    if fx.mem.i16(t + 0x50) == 1 && fx.mem.i16(t + 0x54) != 0 {
        let h = ee::add(fx.mem.f(t + 0x34), 5.0);
        fx.mem.setf(t + 0x34, h);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let a = fx.mem.i16(t + 0x5A) + 6;
        fx.mem.set16(t + 0x5A, a);
    }
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02860 (D, task): the quad cluster (`quad_b_turned`, white, size 5,
/// +0x48 100, cell 4); in by 0x60 under 0xC0 for 4 updates, then out by
/// 14.
fn quads(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 8, 5);
        fx.mem.set16(t + 0xA, 5);
        fx.mem.write_u32(t + 0x48, 0x42C8_0000);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 4;
    fade(fx, t, 0x63, rising, (0xC0, 0x60), (0x10, 0xE));
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02B40 (D, task; frame 0x90: ra, the entry's s1 = the flashes so
/// far, s0 = D): a window sprite (0x6FF8 .. 0x8FF8) sampling the frame
/// (0x207EF0, filter 1) at ST inset by +0x48 (1) / 512, colour (0x78,
/// 0x80, 0x78, alpha +0x63), ALPHA 0x44, key 0 on a fresh ordering table;
/// in by 8 below 0x50 until update 395, then out by 7.
fn tint(fx: &mut Fx, t: u32, flashes: u32) {
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, u64::from(flashes)), (0, u64::from(D))]);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for (k, c) in [0x78u8, 0x80, 0x78].into_iter().enumerate() {
        fx.mem.write_u8(sp + 0x48 + k as u32, c);
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
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    let rising = fx.mem.i16(COUNTER) < 0x18B;
    fade(fx, t, 0x63, rising, (0x50, 8), (9, 7));
    age(fx, t);
    fx.leave(0x90);
}

/// 0x1E02D20(D, t): the flash's frame sprite (the target over the window,
/// pale green (0xC0, 0xFF, 0xC0), UVs from +8 / 10 and +0xA / 10 over
/// 512, the bottom edge at (512 - 3 (+0xA / 10)) / 512) into the
/// descriptor at +0x44C8, submitted once with no ordering table; for 200
/// updates the alpha swings up by 8 to 0x80 and down by 11 to 12 (+0x50
/// the direction), then out by 9. `k` the unit flash's s1 (the slot index).
fn flash(fx: &mut Fx, t: u32, k: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, FLASH_RA), (0x20, u64::from(k)), (0x10, u64::from(D))]);
    // swc1 f20 / f21: the runner's (0) [H, as b491].
    fx.mem.write_u32(sp, 0);
    fx.mem.write_u32(sp + 4, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0xC0, 0xFF, 0xC0);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = fx.mem.read_u32(FLASH_DESC);
    for k in 0..4u32 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(d + 0x10 + k, b);
    }
    fx.mem.write_u32(d + 0x14, 0x3F80_0000);
    for (at, xy) in [(0x20u32, 0x6FF8u16), (0x30, 0x8FF8)] {
        fx.mem.write_u16(d + at, xy);
        fx.mem.write_u16(d + at + 2, xy);
        let v = fx.mem.read_u64(d + at);
        fx.mem.write_u64(d + at, (v & 0xFF00_0000_FFFF_FFFF) | 0x00FF_FFFF_0000_0000);
        fx.mem.write_u8(d + at + 7, 0);
    }
    let bottom = ee::div(ee::sub(512.0, ee::mul(3.0, h)), 512.0);
    for (o, v) in [(0x18u32, ee::div(w, 512.0)), (0x1C, ee::div(h, 512.0)), (0x28, ee::div(ee::sub(512.0, w), 512.0)), (0x2C, bottom)] {
        fx.mem.setf(d + o, v);
    }
    fx.submit(d, 0, 0);
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0xC8 {
        if fx.mem.i16(t + 0x50) == 0 {
            if a < 0x80 {
                fx.mem.write_u8(t + 0x63, a + 8);
            } else {
                fx.mem.set16(t + 0x50, 1);
            }
        } else if a >= 0xD {
            fx.mem.write_u8(t + 0x63, a - 0xB);
        } else {
            fx.mem.set16(t + 0x50, 0);
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x63, a - 9);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E03B60(D): in a packet layer (0x2078A0) the attacker drawn into the
/// target (0x1F8270 filter 0; ALPHA 0x48), TEXFLUSH before and after;
/// every flash task stepped (0x1E02D20); the layer linked into the
/// ordering table at key 0 (b491's 0x1E03F90).
fn unit_flash(fx: &mut Fx, a3: u32) {
    let sp = fx.enter(0xA0);
    let d = sp + 0x48;
    fx.mem.write_u32(FLASH_DESC, d);
    let layer = fx.layer_begin();
    fx.mem.write_u32(LAYER, layer);
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x88, sp + 0x90);
    fx.texflush(regs);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) != 0 && fx.mem.read_u32(t + 0x44) == FLASH {
            flash(fx, t, k);
        }
    }
    let layer = fx.mem.read_u32(LAYER);
    fx.layer_end(layer, OT, 0);
    fx.leave(0xA0);
}
