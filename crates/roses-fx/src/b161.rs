//! BMODULE b161 (card 313), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02900, data area 0x1ED0000) [F] (helper C18, 1
//! Nov 2026): a pool of 200 tasks of 0x70 bytes at D+0x2A0 (alive +0x58,
//! step +0x44 through `jalr`, 0x1E028F0 only returns and is skipped;
//! spawned by 0x1E03370 with the colour +0xC394 and the index +0xC3F4 into
//! +0x50), the globals past D+0xC000, drawn with the second library
//! (`ring_quads`, `quad`), the record's waving textured arc strips
//! 0x1E03500 ([`wave_strips`]) and its tube 0x1E03AA0
//! (`tubetrail::tube_with(SINGLE_BLEND)`). The emitter 0x1E01220 counts
//! +0xC3FC: the attacker's node 0x1E followed to +0xC350 (slid to the
//! target's node over 0x9B..0xB9, copied to +0x250 on 0xBA); on 0x1A..0x8D
//! a mote ring (0x1E01970, `ring_quads`) at each of seven nodes (0x1E /
//! 0x2C / 0x3A / 0x48 / 0x55 / 0x62 / 0x79); on 0x1A seven blinking quads
//! (0x1E01B50) riding those nodes; on the odd updates 0x1B..0x79 a trail
//! (0x1E022B0: a 20-point history +0x5A50 + 0x140 k of a wandering head
//! +0xBE50 + 0x10 k) drawn as the tube at all seven nodes, turned by
//! 0x2000 between them; on 0x34..0x8D a closing wave ring (0x1E02120) at
//! node 1, 10 behind or in front by the facing; on 0x70..0x104 a shrinking
//! wave ring (0x1E01F80) at the target's node; from 0xA5 every fourth
//! update the spawn point jittered +-10. Returns 1 on update 0x73, 4 on
//! 0x118, 2 on 0x113, 6 on 0x7E..0x10D, 8 once the 300-update timer
//! +0xC390 has run out. The ring of motes 0x1E01D80 is drawn but never
//! spawned (its step and draw are ported).
use crate::b076::{node_position_saving, request_node_saving};
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, spawn::history, tubetrail, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The step functions a task holds (+0x44), as the record's addresses.
const MOTES: u32 = 0x01E0_1970;
const BLINK: u32 = 0x01E0_1B50;
const MOTES_B: u32 = 0x01E0_1D80;
const WAVE_T: u32 = 0x01E0_1F80;
const WAVE_A: u32 = 0x01E0_2120;
const TRAIL: u32 = 0x01E0_22B0;
const IDLE: u32 = 0x01E0_28F0;
/// The return address of the entry's `jalr` into a step (0x1E02BF8 + 8).
const TASK_RA: u64 = 0x01E0_2C00;
/// The return address of the entry's call into the emitter.
const EMIT_RA: u64 = 0x01E0_2BC0;
/// The scratchpad, where the library projects.
const SPR: u32 = 0x7000_0000;
const SPAWN_AT: u32 = D + 0x250;
const TEX_A: u32 = D + 0x29C;
const TRAILS: u32 = D + 0x5A50;
const HEADS: u32 = D + 0xBE50;
const FOLLOW: u32 = D + 0xC350;
const SLIDE_Y: u32 = D + 0xC364;
const SLIDE_Z: u32 = D + 0xC368;
const TEX: u32 = D + 0xC370;
const TIMER: u32 = D + 0xC390;
const COLOUR: u32 = D + 0xC394;
/// The attacker's node slots 0x1E, 0x2C, 0x3A, 0x48, 0x55, 0x62, 0x79, 1.
const NODES: u32 = D + 0xC3C4;
const NODE_1: u32 = D + 0xC3D2;
const TNODE: u32 = D + 0xC3D4;
const TRAIL_K: u32 = D + 0xC3EA;
const WAVE: u32 = D + 0xC3EC;
const SPIN: u32 = D + 0xC3EE;
const INDEX: u32 = D + 0xC3F4;
const FACING: u32 = D + 0xC3F8;
const COUNTER: u32 = D + 0xC3FC;
const BYTES: u32 = D + 0xC3FE;
const SINES2: u32 = D + 0xC600;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// A trail's head (+0xBE50 + 0x10 k) by its index +0x50.
fn head_of(fx: &mut Fx, t: u32) -> u32 {
    (HEADS as i32 + 0x10 * fx.mem.i16(t + 0x50)) as u32
}

/// A trail's history (20 points of 16 bytes) by its index +0x50.
fn trail_of(fx: &mut Fx, t: u32) -> u32 {
    (TRAILS as i32 + 0x140 * fx.mem.i16(t + 0x50)) as u32
}

/// The entry 0x1E02900 (a0 = D, a1 = param; s0 = D, s1 = 0, s2 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xE0);
    fx.mem.write_u32(D + 0xC39C, crate::exe::OT);
    fx.mem.write_u32(D + 0x80, crate::exe::OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xE0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xC770);
    l2::sine_table(fx, SINES2);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xC3FA, param);
    fx.mem.set32(TIMER, 0x12C);
    for (k, node) in [0x1E, 0x2C, 0x3A, 0x48, 0x55, 0x62, 0x79, 1].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    // The last request's frame (s0 = D, s1 = 0, s2 = the parameter).
    let h = fx.target_handle();
    let r = request_node_saving(fx, h, 1, 1, [Some(u64::from(D)), Some(0), Some(param as i64 as u64)], 0x01E0_2A98);
    fx.mem.set16(TNODE, r);
    for k in 0..2u32 {
        let t = fx.texture(4, 0xE + k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * k, t);
    }
    for k in 0..4u32 {
        let t = fx.texture(5, k as i32);
        fx.mem.write_u32(TEX + 4 * k, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xD0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xD4)), 180.0);
    fx.fix_residue(y, 0x01E0_2B38);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, D, 0x01E0_2B68, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
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
        match fx.mem.read_u32(t + 0x44) {
            0 | IDLE => {}
            MOTES => motes(fx, t),
            BLINK => blink(fx, t),
            MOTES_B => motes_b(fx, t),
            WAVE_T => wave_t(fx, t),
            WAVE_A => wave_a(fx, t),
            TRAIL => trail(fx, t),
            f => panic!("b161: unknown task step {f:#x}"),
        }
    }
    // The entry's s1: the live tasks counted so far.
    let mut live = 0i32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        live = (live + 1) as i16 as i32;
        let s1 = Some(live as i64 as u64);
        let sub = fx.mem.i16(t + 0x56);
        if fx.mem.read_u32(t + 0x44) == MOTES {
            let w = fx.mem.read_u32(TEX);
            fx.mem.write_u32(D + 0x88, w);
            cell(fx, sp + 0x80, 0x14, sub);
            fx.mem.set16(D + 0xB8, 2);
            fx.mem.set16(D + 0xB0, 0);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        if fx.mem.read_u32(t + 0x44) == BLINK {
            let w = fx.mem.read_u32(TEX + 8);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0x90, 0x9B, sub);
            fx.mem.set16(D + 0xBE, 5);
            let sub = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0x94, sub << 6);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x3F);
            fx.mem.set16(D + 0x9A, 0x3F);
            for c in 0..3 {
                let w = fx.mem.read_u32(t + 0x10 + 4 * c);
                fx.mem.write_u32(sp + 0x70 + 4 * c, w);
            }
            billboard::quad(fx, uv::uv_rect, D, t, 0x20, sp + 0x70);
        }
        if fx.mem.read_u32(t + 0x44) == MOTES_B {
            let w = fx.mem.read_u32(TEX_A);
            fx.mem.write_u32(D + 0x88, w);
            let sub = fx.mem.i16(t + 0x56);
            cell(fx, sp + 0xA0, 0x97, sub);
            fx.mem.set16(D + 0xB8, 0xA);
            fx.mem.set16(D + 0xB0, 1);
            billboard::ring_quads(fx, uv::uv_rect, D, t, 0x20, BYTES);
            fx.mem.set16(D + 0xB8, 0);
            fx.mem.set16(D + 0xB0, 0);
        }
        for (f, r, mode) in [(WAVE_T, 0xB0u32, 0x21), (WAVE_A, 0xC0, 0x20)] {
            if fx.mem.read_u32(t + 0x44) == f {
                let sub = fx.mem.i16(t + 0x56);
                cell(fx, sp + r, 0x97, sub);
                let w = fx.mem.read_u32(TEX + 4);
                fx.mem.write_u32(D + 0x88, w);
                fx.mem.set16(D + 0x94, 0);
                fx.mem.set16(D + 0x96, 0);
                fx.mem.set16(D + 0x98, 0x1F);
                fx.mem.set16(D + 0x9A, 0x3F);
                fx.mem.set16(D + 0xB8, 0x48);
                wave_strips(fx, D, t, mode, t + 0x10);
                fx.mem.set16(D + 0xB8, 0);
            }
        }
        if fx.mem.read_u32(t + 0x44) == TRAIL {
            let w = fx.mem.read_u32(TEX + 0xC);
            fx.mem.write_u32(D + 0x88, w);
            fx.mem.set16(D + 0x94, 0);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x7F);
            fx.mem.set16(D + 0x9A, 0x3F);
            let ras = [0x01E0_2F90u32, 0x01E0_3000, 0x01E0_306C, 0x01E0_30D8, 0x01E0_3144, 0x01E0_31B0, 0x01E0_321C];
            let mut keep = 0u32;
            for (j, ra) in ras.into_iter().enumerate() {
                let h = fx.self_handle();
                let slot = fx.mem.i16(NODES + 2 * j as u32);
                node_position_saving(fx, h, slot, sp + 0x70, u64::from(D), s1, ra);
                if j == 0 {
                    fx.mem.set16(D + 0xBE, 1);
                }
                for c in 0..3 {
                    let w = fx.mem.read_u32(sp + 0x70 + 4 * c);
                    fx.mem.write_u32(D + 0x20 + 4 * c, w);
                }
                if j > 0 {
                    let a = fx.mem.read_u32(t + 0x20);
                    keep = a;
                    fx.mem.write_u32(t + 0x20, a.wrapping_add(0x2000));
                }
                let a = trail_of(fx, t);
                tubetrail::tube_with(fx, &tubetrail::SINGLE_BLEND, D, t, a, 0x21);
            }
            // The last turn is undone: the angle ends 0xA000 on.
            fx.mem.write_u32(t + 0x20, keep);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left.wrapping_sub(1));
    }
    match fx.mem.i16(COUNTER) {
        0x73 => 1,
        0x118 => 4,
        0x113 => 2,
        0x7E..=0x10D => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
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

/// 0x1E03500(D, task, mode, pos): D+0xB8 textured strip3 strips (colours
/// +0x60 inner / +0x64 outer, the UVs D+0x94, texture D+0x88, blend mode
/// >> 4) about `pos` (scale 0.1, the task's angles +0x20) between radius
/// +0x48 and +0x4C in the plane y = 0, the k-th from angle s + a_k to s +
/// a_k + span (s +0x5A, span +0x5C, a_k from +0xC3EE + the task's +0x52
/// stepping by s), each edge pushed out by the wave w sin(+0xC3EC) (w =
/// size / 10 for the index +0x50 0, / 20 for 1; +0xC3EC stepping 20 / 40
/// an edge from 0 a draw) on the record's sines +0xC600; depth mode & 15
/// (0 the mean w, 1 the far Z at key 0, else nothing drawn) into D+0x80's
/// ordering table [F].
fn wave_strips(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    use ee::{add, div, mul};
    let sp = fx.enter(0x220);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    let span = fx.mem.i16(t + 0x5C);
    let d = sp + 0x170;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 0);
    l2::uv_strip3(fx, d, d0 + 0x94, 0);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for (local, c, to) in [(0x200u32, 0x60u32, 0x10u32), (0x208, 0x64, 0x40), (0x210, 0x60, 0x28), (0x218, 0x64, 0x58)] {
        crate::c061::rgbaq_at(fx, sp + local, t + c, d + to);
    }
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * k);
        fx.mem.write_u32(sp + 0x110 + 4 * k, 0x3DCC_CCCD);
        fx.mem.write_u32(sp + 0x100 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x20, sp + 0x100);
    for o in [0xC4u32, 0xD4, 0xE4, 0xF4] {
        fx.mem.set32(sp + o, 0);
    }
    let (inner, outer) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    fx.mem.set16(WAVE, 0);
    let depth = mode & 0xF;
    let mut a = (fx.mem.i16(SPIN) + fx.mem.i16(t + 0x52)) as i16 as i32;
    // f20: the wave, kept across strips (set on every edge for the
    // indices 0 / 1 the record spawns).
    let mut f20 = 0.0f32;
    let wave = |fx: &mut Fx, f20: &mut f32| {
        let kind = fx.mem.i16(t + 0x50);
        let by = match kind {
            0 => Some(10.0),
            1 => Some(20.0),
            _ => None,
        };
        if let Some(by) = by {
            let w = fx.mem.i16(WAVE);
            let s = l2::sin_deg(fx, SINES2, w, 0);
            *f20 = mul(div(fx.mem.f(t + 0x48), by), s);
        }
    };
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let s4 = a;
        let s = (fx.mem.i16(t + 0x5A) + s4) as i16 as i32;
        wave(fx, &mut f20);
        match fx.mem.i16(t + 0x50) {
            0 => {
                let w = fx.mem.i16(WAVE) + 0x14;
                fx.mem.set16(WAVE, w);
            }
            1 => {
                let w = fx.mem.i16(WAVE) + 0x28;
                fx.mem.set16(WAVE, w);
            }
            _ => {}
        }
        for (r, x, z) in [(inner, 0xC0u32, 0xC8u32), (outer, 0xE0, 0xE8)] {
            let c = l2::sin_deg(fx, SINES2, s, 1);
            fx.mem.setf(sp + x, add(f20, mul(r, c)));
            let sn = l2::sin_deg(fx, SINES2, s, 0);
            fx.mem.setf(sp + z, add(f20, mul(r, sn)));
        }
        wave(fx, &mut f20);
        let e = (s4 + (fx.mem.i16(t + 0x5A) + span)) as i16 as i32;
        for (r, x, z) in [(inner, 0xD0u32, 0xD8u32), (outer, 0xF0, 0xF8)] {
            let c = l2::sin_deg(fx, SINES2, e, 1);
            fx.mem.setf(sp + x, add(f20, mul(r, c)));
            let sn = l2::sin_deg(fx, SINES2, e, 0);
            fx.mem.setf(sp + z, add(f20, mul(r, sn)));
        }
        if depth <= 1 && l2::project(fx, sp + 0xC0, SPR, sp + 0x120, d, sp + 0x160) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            if depth == 0 {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                fx.submit(d, ot, key);
            } else {
                l2::far_z_desc(fx, d, 1);
                fx.submit(d, ot, 0);
            }
        }
        k += 1;
        a = (a + fx.mem.i16(t + 0x5A)) as i16 as i32;
    }
    fx.leave(0x220);
}

/// 0x1E03370(D, step, 0): the first free task of 200 cleared and set off at
/// +0x250 with the colour +0xC394 twice and the index +0xC3F4 (+0x50).
/// `saves`: its prologue's (0x60: ra, s4 .. s0).
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

/// 0x1E01220(D, 0): the update counter, the followed node and the spawns.
/// Frame 0x50 (ra, the entry's s1 = 0, s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    use ee::{add, div, from_i32, sub};
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x20, EMIT_RA), (0x10, 0), (0, u64::from(D))]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    // The spawns' saves: s0 = D (0 in the trail loop), s1 = D, s2 = the
    // parameter, s3 / s4 the runner's (0 [H, as b747]).
    let p = param as i64 as u64;
    let saves = |ra: u32, s0: u64| [(0x50u32, u64::from(ra)), (0x40, 0), (0x30, 0), (0x20, p), (0x10, u64::from(D)), (0, s0)];
    let dd = u64::from(D);
    let s1 = Some(dd);
    let c = fx.mem.i16(COUNTER);
    if c < 0x9B {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES);
        node_position_saving(fx, h, slot, FOLLOW, dd, s1, 0x01E0_128C);
    } else {
        if c == 0x9B {
            let h = fx.target_handle();
            let slot = fx.mem.i16(TNODE);
            node_position_saving(fx, h, slot, sp + 0x30, dd, s1, 0x01E0_12C0);
            let v = div(sub(fx.mem.f(sp + 0x38), fx.mem.f(FOLLOW + 8)), 32.0);
            fx.mem.setf(SLIDE_Z, v);
            let v = div(sub(fx.mem.f(sp + 0x34), fx.mem.f(FOLLOW + 4)), 32.0);
            fx.mem.setf(SLIDE_Y, v);
        }
        if fx.mem.i16(COUNTER) < 0xBA {
            let v = add(fx.mem.f(FOLLOW + 8), fx.mem.f(SLIDE_Z));
            fx.mem.setf(FOLLOW + 8, v);
            let v = add(fx.mem.f(FOLLOW + 4), fx.mem.f(SLIDE_Y));
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
    let c = fx.mem.i16(COUNTER);
    if (0x34..0x8E).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE_1);
        node_position_saving(fx, h, slot, SPAWN_AT, dd, s1, 0x01E0_1408);
        let z = fx.mem.f(SPAWN_AT + 8);
        let z = if fx.mem.i16(FACING) != 0 { add(z, 10.0) } else { sub(z, 10.0) };
        fx.mem.setf(SPAWN_AT + 8, z);
        fx.mem.set16(INDEX, 0);
        spawn(fx, WAVE_A, &saves(0x01E0_1470, dd));
        let a = fx.mem.i16(SPIN) + 3;
        fx.mem.set16(SPIN, a);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x70..0x105).contains(&c) {
        let h = fx.target_handle();
        let slot = fx.mem.i16(TNODE);
        node_position_saving(fx, h, slot, SPAWN_AT, dd, s1, 0x01E0_14D0);
        fx.mem.set16(INDEX, 1);
        spawn(fx, WAVE_T, &saves(0x01E0_14F4, dd));
        let a = fx.mem.i16(SPIN) + 3;
        fx.mem.set16(SPIN, a);
    }
    let c = fx.mem.i16(COUNTER);
    if (0xA5..0x111).contains(&c) && c.wrapping_rem(4) == 1 {
        let h = fx.target_handle();
        let slot = fx.mem.i16(TNODE);
        node_position_saving(fx, h, slot, SPAWN_AT, dd, s1, 0x01E0_1574);
        for (o, site) in [(0u32, 0x01E0_157Cu32), (4, 0x01E0_15B0)] {
            let v = rmod(fx, D, site, 0x14);
            let x = add(fx.mem.f(SPAWN_AT + o), from_i32(v - 0xA));
            fx.mem.setf(SPAWN_AT + o, x);
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x1A..0x8E).contains(&c) {
        let ras = [(0x01E0_1624u32, 0x01E0_1638u32), (0x01E0_1658, 0x01E0_166C), (0x01E0_168C, 0x01E0_16A0), (0x01E0_16C0, 0x01E0_16D4), (0x01E0_16F4, 0x01E0_1708), (0x01E0_1728, 0x01E0_173C), (0x01E0_175C, 0x01E0_1770)];
        for (j, (ra_pos, ra)) in ras.into_iter().enumerate() {
            let h = fx.self_handle();
            let slot = fx.mem.i16(NODES + 2 * j as u32);
            node_position_saving(fx, h, slot, SPAWN_AT, dd, s1, ra_pos);
            spawn(fx, MOTES, &saves(ra, dd));
        }
    }
    if fx.mem.i16(COUNTER) == 0x1A {
        for (i, ra) in [0x01E0_17A8u32, 0x01E0_17CC, 0x01E0_17F0, 0x01E0_1814, 0x01E0_1838, 0x01E0_185C, 0x01E0_1880].into_iter().enumerate() {
            fx.mem.set16(INDEX, i as i32);
            spawn(fx, BLINK, &saves(ra, dd));
        }
    }
    let c = fx.mem.i16(COUNTER);
    if (0x1B..0x7A).contains(&c) && c.wrapping_rem(2) == 1 {
        let k = fx.mem.i16(TRAIL_K);
        fx.mem.set16(INDEX, k);
        spawn(fx, TRAIL, &saves(0x01E0_18F8, 0));
        let k = fx.mem.i16(TRAIL_K) + 1;
        fx.mem.set16(TRAIL_K, k);
        if fx.mem.i16(TRAIL_K) >= 0x50 {
            fx.mem.set16(TRAIL_K, 0);
        }
    }
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The alpha +0x63's fade: in by `up.1` below `up.0` while `rising`, else
/// out by `down.1` while at least `down.0`, else the task ends.
fn fade(fx: &mut Fx, t: u32, rising: bool, up: (u8, u8), down: (u8, u8)) {
    let a = fx.mem.read_u8(t + 0x63);
    if rising {
        if a < up.0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(up.1));
        }
    } else if a >= down.0 {
        fx.mem.write_u8(t + 0x63, a - down.1);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// A 0x20-frame step's prologue (ra, the entry's s0 = D).
fn frame20(fx: &mut Fx) {
    let sp = fx.enter(0x20);
    fx.saved(sp, &[(0x10, TASK_RA), (0, u64::from(D))]);
}

/// A 0x30-frame step's prologue (ra, the entry's s1 = 0, s0 = D).
fn frame30(fx: &mut Fx) -> u32 {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
    sp
}

/// The task ends once the update counter is at 0x79.
fn end_at_79(fx: &mut Fx, t: u32) {
    if fx.mem.i16(COUNTER) >= 0x79 {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// 0x1E01970 (D, task): a white mote ring (`ring_quads` of 2 from the bytes
/// +0xC3FE): size 2, cell 4, random angles, radius 3..12, angle +0x5A
/// random, +0xE 1..3, row 1..8, life 1..3; in by 0x40 under 0xC0 during the
/// life, then out by 0x13; ended from update 0x79.
fn motes(fx: &mut Fx, t: u32) {
    frame30(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4000_0000);
        fx.mem.set16(t + 0x56, 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        for (o, site) in [(0x20u32, 0x01E0_19C0u32), (0x24, 0x01E0_19DC), (0x28, 0x01E0_19F8)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        let v = rmod(fx, t, 0x01E0_1A14, 0xA);
        fx.mem.set16(t + 0x50, v + 3);
        let v = rmod(fx, t, 0x01E0_1A34, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_1A50, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, 0x01E0_1A70);
        fx.mem.set16(t + 8, (v & 7) + 1);
        let v = rmod(fx, t, 0x01E0_1A90, 3);
        fx.mem.set16(t + 6, v + 1);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, rising, (0xC0, 0x40), (0x15, 0x13));
    end_at_79(fx, t);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E01B50 (D, task): a quad (`quad`, 10 x 10, cell 0x9B frame +0x56)
/// riding the attacker's node by the index +0x50 (+0xC3C4 + 2 i), turned by
/// the camera's angles; grey 0x80 / black blinking by the age (the odd
/// indices out of phase), the frame +0x56 the age's parity; in by 0x60
/// under 0xC0 for 15 updates, out by 9 from update 0x7A.
fn blink(fx: &mut Fx, t: u32) {
    frame30(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xA);
        fx.mem.set16(t + 0xA, 0xA);
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0xF {
        fade(fx, t, true, (0xC0, 0x60), (0, 0));
    } else if fx.mem.i16(COUNTER) >= 0x7A {
        fade(fx, t, false, (0, 0), (0xB, 9));
    }
    let odd = fx.mem.i16(t + 0x50).wrapping_rem(2) == 1;
    let q = fx.mem.i16(t + 0x54).wrapping_rem(4);
    let (on, off) = if odd { (1, 3) } else { (3, 1) };
    if q == on {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
    } else if q == off {
        l2::rgb(fx, t + 0x60, 0, 0, 0);
    }
    let f = i32::from(fx.mem.i16(t + 0x54).wrapping_rem(2) != 0);
    fx.mem.set16(t + 0x56, f);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_1D28);
    age(fx, t);
    let h = fx.self_handle();
    let i = fx.mem.i16(t + 0x50);
    let slot = fx.mem.i16((NODES as i32 + 2 * i) as u32);
    node_position_saving(fx, h, slot, t + 0x10, u64::from(t), Some(u64::from(D)), 0x01E0_1D5C);
    fx.leave(0x30);
}

/// 0x1E01D80 (D, task): a red mote ring (`ring_quads` of 10, cell 0x97):
/// size 2..6, frame rand & 3, random angles, +0x52 counting from 1, radius
/// 10..39 growing by 1..3, row 1..8, life 2..4; alpha 0x60 during the
/// life, then out by 11. Never spawned.
fn motes_b(fx: &mut Fx, t: u32) {
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, 0x01E0_1DA4, 5);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 2));
        let v = fx.rand_from(t, 0x01E0_1DD0);
        fx.mem.set16(t + 0x56, v & 3);
        l2::rgb(fx, t + 0x60, 0x80, 0x30, 0x30);
        for (o, site) in [(0x20u32, 0x01E0_1E04u32), (0x24, 0x01E0_1E20), (0x28, 0x01E0_1E3C)] {
            let v = rmod(fx, t, site, 0xFFFF);
            fx.mem.set32(t + o, v);
        }
        fx.mem.set16(t + 0x52, 1);
        let v = rmod(fx, t, 0x01E0_1E60, 0x1E);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, 0x01E0_1E80, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, 0x01E0_1E9C, 3);
        fx.mem.set16(t + 0xE, v + 1);
        let v = fx.rand_from(t, 0x01E0_1EBC);
        fx.mem.set16(t + 8, (v & 7) + 1);
        let v = rmod(fx, t, 0x01E0_1EDC, 3);
        fx.mem.set16(t + 6, v + 2);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fx.mem.write_u8(t + 0x63, 0x60);
    } else {
        fade(fx, t, false, (0, 0), (0xD, 0xB));
    }
    let a = fx.mem.i16(t + 0x52) + 1;
    fx.mem.set16(t + 0x52, a);
    let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x50, r);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E01F80 (D, task): a grey wave ring (`wave_strips` x 72, mode 0x21)
/// closing from radius 900 by 25..59.9 a update (the outer 65 inside),
/// turned by the camera's angles (the first + 0x4000), start angle +0x52
/// random, steps 5; alpha 0x80 for 2 updates, then out by 14 below radius
/// 180; ended below 120.
fn wave_t(fx: &mut Fx, t: u32) {
    use ee::{div, from_i32, sub};
    frame20(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4461_0000);
        fx.mem.setf(t + 0x4C, sub(900.0, 58.0));
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x40);
        let v = rmod(fx, t, 0x01E0_1FE8, 0x168);
        fx.mem.set16(t + 0x52, v);
        fx.mem.set16(t + 0x5A, 5);
        fx.mem.set16(t + 0x5C, 5);
        let v = rmod(fx, t, 0x01E0_2010, 0x15E);
        fx.mem.setf(t + 0x34, from_i32(v + 0xFA));
        let v = div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 2 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        if fx.mem.f(t + 0x48) < 180.0 {
            let a = fx.mem.read_u8(t + 0x63);
            if a >= 0x10 {
                fx.mem.write_u8(t + 0x63, a - 0xE);
            }
        }
        if fx.mem.f(t + 0x48) < 120.0 {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let r = sub(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, sub(r, 65.0));
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_20F4);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
    fx.leave(0x20);
}

/// 0x1E02120 (D, task): a grey wave ring (`wave_strips` x 72, mode 0x20)
/// opening from radius 20 by 10..34.9 a update (the outer 30 inside),
/// turned by the camera's angles (the first + 0x4000), start angle +0x52
/// random, steps 5; alpha 0x80 for 2 updates, then out by 14; ended from
/// update 0x79.
fn wave_a(fx: &mut Fx, t: u32) {
    use ee::{add, div, from_i32, sub};
    frame30(fx);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.setf(t + 0x4C, sub(20.0, 58.0));
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        l2::rgb(fx, t + 0x64, 0x40, 0x40, 0x40);
        let v = rmod(fx, t, 0x01E0_2190, 0x168);
        fx.mem.set16(t + 0x52, v);
        fx.mem.set16(t + 0x5A, 5);
        fx.mem.set16(t + 0x5C, 5);
        let v = rmod(fx, t, 0x01E0_21B8, 0xFA);
        fx.mem.setf(t + 0x34, from_i32(v + 0x64));
        let v = div(fx.mem.f(t + 0x34), 10.0);
        fx.mem.setf(t + 0x34, v);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 2 {
        fx.mem.write_u8(t + 0x63, 0x80);
    } else {
        fade(fx, t, false, (0, 0), (0x10, 0xE));
    }
    let r = add(fx.mem.f(t + 0x48), fx.mem.f(t + 0x34));
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, sub(r, 30.0));
    end_at_79(fx, t);
    camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_2280);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
    fx.leave(0x30);
}

/// 0x1E022B0 (D, task): a trail. Its head (+0xBE50 + 0x10 k, k = +0x50)
/// starts at the origin, jittered by a random +-1 and the 20-point history
/// (+0x5A50 + 0x140 k) filled with it; size 1..3.9, sway +0x4C (1..4) / 20
/// along the sines of +0x5A (random) and +0x5C (random), life 3..17; the
/// tube's points +6 rising by 1 to 15 for 7 updates, then falling to 0;
/// alpha 0xC0 for the life, then out by 0x1D; grey 0x60 / 0x40 by the
/// age's parity; every update a new +-10 / 20 drift; the head pushed into
/// the history for 7 updates; ended from update 0x79.
fn trail(fx: &mut Fx, t: u32) {
    use ee::{add, div, from_i32, mul};
    frame30(fx);
    let jitter = |fx: &mut Fx, sites: [u32; 3]| {
        for (o, site) in [0x30u32, 0x34, 0x38].into_iter().zip(sites) {
            let v = rmod(fx, t, site, 0x14);
            fx.mem.setf(t + o, from_i32(v - 0xA));
        }
    };
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 6, 1);
        fx.mem.write_u8(t + 0x63, 0);
        for c in 0..3 {
            let h = head_of(fx, t);
            fx.mem.write_u32(h + 4 * c, 0);
        }
        jitter(fx, [0x01E0_2328, 0x01E0_2354, 0x01E0_2380]);
        for (c, o) in [(0u32, 0x30u32), (4, 0x34), (8, 0x38)] {
            let by = div(fx.mem.f(t + o), 10.0);
            let h = head_of(fx, t);
            let v = add(fx.mem.f(h + c), by);
            fx.mem.setf(h + c, v);
        }
        let h = head_of(fx, t);
        let a = trail_of(fx, t);
        for i in 0..0x14u32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(h + 4 * c);
                fx.mem.write_u32(a + 0x10 * i + 4 * c, w);
            }
        }
        let v = rmod(fx, t, 0x01E0_24AC, 0x1E);
        fx.mem.setf(t + 0x48, from_i32(v + 0xA));
        let v = div(fx.mem.f(t + 0x48), 10.0);
        fx.mem.setf(t + 0x48, v);
        let v = fx.rand_from(t, 0x01E0_24EC);
        fx.mem.setf(t + 0x4C, from_i32((v & 3) + 1));
        let v = div(fx.mem.f(t + 0x4C), 20.0);
        fx.mem.setf(t + 0x4C, v);
        let v = rmod(fx, t, 0x01E0_2538, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.rand_from(t, 0x01E0_2554);
        let (_, r) = fx.div(v >> 4, 0x168);
        fx.mem.set16(t + 0x5C, r);
        let v = rmod(fx, t, 0x01E0_2574, 0xF);
        fx.mem.set16(t + 8, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let n = fx.mem.i16(t + 6);
    if fx.mem.i16(t + 0x54) < 7 {
        if n < 0xF {
            fx.mem.set16(t + 6, n + 1);
        }
    } else if n > 0 {
        fx.mem.set16(t + 6, n - 1);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 8) {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        fade(fx, t, false, (0, 0), (0x1F, 0x1D));
    }
    let g = if fx.mem.i16(t + 0x54).wrapping_rem(2) == 0 { 0x60 } else { 0x40 };
    l2::rgb(fx, t + 0x60, g, g, g);
    jitter(fx, [0x01E0_2670, 0x01E0_269C, 0x01E0_26C8]);
    if fx.mem.i16(t + 0x54) != 0 {
        for (c, deg_at, cos) in [(8u32, 0x5Au32, 0), (0, 0x5A, 1), (4, 0x5C, 0)] {
            let deg = fx.mem.i16(t + deg_at);
            let s = l2::sin_deg(fx, SINES2, deg, cos);
            let by = mul(fx.mem.f(t + 0x4C), s);
            let h = head_of(fx, t);
            let v = add(fx.mem.f(h + c), by);
            fx.mem.setf(h + c, v);
        }
        for (c, o) in [(0u32, 0x30u32), (4, 0x34), (8, 0x38)] {
            let by = div(fx.mem.f(t + o), 20.0);
            let h = head_of(fx, t);
            let v = add(fx.mem.f(h + c), by);
            fx.mem.setf(h + c, v);
        }
    }
    if fx.mem.i16(t + 0x54) < 7 {
        let a = trail_of(fx, t);
        let h = head_of(fx, t);
        history(fx, a + 0x130, 0x14, h);
    }
    end_at_79(fx, t);
    age(fx, t);
    fx.leave(0x30);
}
