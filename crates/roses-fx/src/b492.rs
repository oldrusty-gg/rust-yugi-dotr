//! BMODULE b492 (card 45), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03880, data area 0x1ED0000, 0xCF30 bytes) [F]
//!: b287's program (b703's 400-task pool of 0x70
//! bytes at D+0x2A0 with the same globals, the same spawn at 0x1E04060 and
//! b550's unit flash at 0x1E04780 stepping the flash tasks 0x1E03410) with
//! these steps: grey rings growing (0x1E02230, b641's) and still
//! (0x1E023E0, b400's) of size 10..24, life 3..5; b432's two-kind arc
//! (0x1E02570: kind 1 blue-grey 80 wide growing by 40); a dim glow
//! (0x1E02790, drawn by the record's own 0x1E048F0: four ST quarters);
//! b703's light columns red (0x1E02880, drawn twice) and white / blue
//! (0x1E02A40); b293's blue ribbon task (0x1E02C00, nodes 0x43 / 0x45 /
//! 0x5E / 0x5C into the four histories to update 0xB9); b590's arc with
//! radius 220 (0x1E02F90); b293's stage light (0x1E030B0); b702's grey
//! frame flash (0x1E031C0, ST from 0 growing by 0.3 to 5 from update 0xBF)
//! and three grey flashing unit flashes (0x1E03410). Update 1 the light,
//! the frame flash and the unit flashes; 0x23 the glow at the attacker (30
//! toward the facing); 0x19 / 0xCB eight red columns at nodes 0x2A / 0x2B
//! (0.7 up); 0x74 / 0x9C at nodes 0x43 / 0x5E an arc and ten growing rings
//! each (0x9C also four white columns and ten blue arcs); 0x56 the ribbon;
//! 0x56..0xB3 a still ring a update at both nodes. Returns 1 on update
//! 0x6A, 4 on 0xF0, 2 on 0xAF, 6 on 0x75..0xA9, 8 once the 0xF5-update
//! timer D+0xCB10 has run out.
use crate::b494::camera_angles_saving;
use crate::blib::{billboard, quads4, spawn as libspawn, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 400;
const POS: u32 = D + 0x250;
const RING: u32 = 0x01E0_2230;
const RING_S: u32 = 0x01E0_23E0;
const ARC_K: u32 = 0x01E0_2570;
const GLOW: u32 = 0x01E0_2790;
const COLUMN_R: u32 = 0x01E0_2880;
const COLUMN_W: u32 = 0x01E0_2A40;
const TRAILS: u32 = 0x01E0_2C00;
const ARC: u32 = 0x01E0_2F90;
const LIGHT: u32 = 0x01E0_30B0;
const FRAME: u32 = 0x01E0_31C0;
/// The flash tasks, run by the unit flash 0x1E04780.
const FLASH: u32 = 0x01E0_3410;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_3B98;
/// The four node histories (20 points of 16 bytes).
const HIST: [u32; 4] = [D + 0xB1D0, D + 0xB310, D + 0xB450, D + 0xB590];
const TEX: u32 = D + 0xCAF0;
const TIMER: u32 = D + 0xCB10;
const COLOUR: u32 = D + 0xCB14;
const LAYER: u32 = D + 0xCB24;
const FLASH_DESC: u32 = D + 0xCB28;
const NODES: u32 = D + 0xCB44;
const SPAWN_A: u32 = D + 0xCBA2;
const SPAWN_K: u32 = D + 0xCBB0;
const FACING: u32 = D + 0xCBB4;
const COUNTER: u32 = D + 0xCBB8;
const BYTES: u32 = D + 0xCBBA;
const TAB: u32 = D + 0xCDBC;
const SPR: u32 = 0x7000_0000;

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

fn node(fx: &mut Fx, k: u32) -> i32 {
    fx.mem.i16(NODES + 2 * k)
}

/// The entry 0x1E03880 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0xCB1C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp, param) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0xCF30);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0xCBB6, param);
    fx.mem.set32(TIMER, 0xF5);
    for (k, n) in [0x43, 0x5E, 0x45, 0x5C, 0x2A, 0x2B].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0xCB54, r);
    for k in 0xE..0x10u32 {
        let t = fx.texture(4, k as i32);
        fx.mem.write_u32(D + 0x298 + 4 * (k - 0xE), t);
    }
    for k in 0..2 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(TEX + 4 * k as u32, t);
    }
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0xB0);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0xB4)), 180.0);
    fx.fix_residue(y, 0x01E0_3A78);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let v = rmod(fx, k, 0x01E0_3AA8, 0x1E);
        fx.mem.set8(BYTES + k, v - 0xF);
    }
    for k in 0..0x1Eu32 {
        let v = fx.rand_from(k, 0x01E0_3AFC);
        fx.mem.set16(D + 0xCB64 + 2 * k, v % 8);
    }
    0
}

/// The last XYZ2 of the last primitive drawn since `before` (0 when none).
fn last_xyz2(fx: &mut Fx, before: usize) -> u32 {
    match fx.out.prims.get(before..).and_then(|p| p.last()) {
        Some(p) => {
            let d = p.desc;
            let n = u32::from(fx.mem.read_u16(d));
            fx.mem.read_u32(d + 8 + 8 * n)
        }
        None => 0,
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    // The entry's s0: the flash tasks counted here (the steps' saved s0);
    // its s1 is 0 until the draw loop.
    let mut flashes = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            0 => continue,
            FLASH => {
                flashes += 1;
                continue;
            }
            _ => {}
        }
        let frame = match f {
            GLOW => 0x30,
            TRAILS => 0x40,
            FRAME => 0x90,
            _ => 0x20,
        };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(flashes))]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(flashes))]);
        }
        match f {
            RING => ring(fx, t, RING, true),
            RING_S => ring(fx, t, RING_S, false),
            ARC_K => arc_k(fx, t),
            GLOW => glow(fx, t),
            COLUMN_R => column(fx, t, ((0xFF, 0xA0, 0xA0), (0xFF, 0x40, 0x40)), (0x4090_0000, 0x4270_0000, 0x41F0_0000), (10.0, 5.0)),
            COLUMN_W => column(fx, t, ((0xFF, 0xFF, 0xFF), (0x40, 0x40, 0xFF)), (0x420C_0000, 0x442F_0000, 0x43FA_0000), (50.0, 30.0)),
            TRAILS => trails(fx, t, s),
            ARC => arc(fx, t),
            LIGHT => light(fx, t),
            FRAME => frame_flash(fx, t, s),
            _ => panic!("b492: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    // a3 as the draws' last library call left it, for the unit flash's
    // first 0x207BB0.
    let mut a3 = 0u32;
    let mut alive = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        alive += 1;
        let f = fx.mem.read_u32(t + 0x44);
        match f {
            RING | RING_S => {
                tex(fx, TEX);
                let c = fx.mem.i16(t + 0x56);
                let (local, site) = if f == RING { (0x70, 0x01E0_3C5C) } else { (0x80, 0x01E0_3D00) };
                cell(fx, sp + local, 0x14, c);
                fx.mem.set16(D + 0xB8, 4);
                let v = fx.rand_from(flashes, site);
                fx.mem.set16(D + 0xBE, v % 4);
                fx.mem.set16(D + 0xB0, 0);
                billboard::ring_quads(fx, uv::uv_rect, D, t, 0x21, BYTES);
                a3 = 0;
                fx.mem.set16(D + 0xB8, 0);
            }
            ARC_K | ARC => {
                fx.mem.set16(D + 0xB8, 0x24);
                let before = fx.out.prims.len();
                billboard::arc_strips(fx, D, t, 0x21, t + 0x10);
                a3 = last_xyz2(fx, before);
                fx.mem.set16(D + 0xB8, 0);
            }
            GLOW => {
                tex(fx, TEX + 4);
                for (o, v) in [(0x94u32, 0), (0x96, 0), (0x98, 0x7F), (0x9A, 0x7F)] {
                    fx.mem.set16(D + o, v);
                }
                fx.mem.set16(D + 0xBE, 0);
                for k in 0..4 {
                    let w = fx.mem.read_u32(t + 0x10 + 4 * k);
                    fx.mem.write_u32(sp + 0x60 + 4 * k, w);
                }
                let saves = [u64::from(flashes), u64::from(alive), u64::from(D), u64::from(t - 0x2A0), 0x01E0_3DE0];
                a3 = quarters(fx, t, sp + 0x60, 0x20, saves);
            }
            COLUMN_R | COLUMN_W => {
                tex(fx, D + 0x29C);
                let local = if f == COLUMN_R { 0x90 } else { 0xA0 };
                cell(fx, sp + local, 0x9A, 0);
                fx.mem.set16(D + 0xBE, 0);
                let before = fx.out.prims.len();
                quads4::sprite_quads(fx, D, t, 0, t + 0x10);
                if f == COLUMN_R {
                    quads4::sprite_quads(fx, D, t, 0, t + 0x10);
                }
                // The last XYZ2 drawn, as after an arc [run].
                a3 = last_xyz2(fx, before);
            }
            TRAILS => {
                for (pair, ra) in [((HIST[0], HIST[1]), 0x01E0_3F00u64), ((HIST[2], HIST[3]), 0x01E0_3F1C)] {
                    let saves = [u64::from(flashes), u64::from(alive), u64::from(D), u64::from(t - 0x2A0), u64::from(k), ra];
                    crate::b703::ribbon(fx, t, pair.0, pair.1, saves);
                }
                // The ribbon leaves a3 = 0 (b256's rule) [run].
                a3 = 0;
            }
            _ => {}
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
        0x6A => 1,
        0xF0 => 4,
        0xAF => 2,
        0x75..=0xA9 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// The atlas cell `id` / `sub` (0x106960 into `r`) copied to D+0x94..+0xA3.
fn cell(fx: &mut Fx, r: u32, id: i32, sub: i32) {
    l2::rect(fx, r, id, sub);
    for k in 0..4 {
        let v = fx.mem.read_u32(r + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, v);
    }
}

fn tex(fx: &mut Fx, at: u32) {
    let w = fx.mem.read_u32(at);
    fx.mem.write_u32(D + 0x88, w);
}

/// 0x1E048F0 (D, task, pos, mode; this record only): a textured quad of
/// half extents (i16 +8, i16 +0xA) drawn as its four quarters (0x109640
/// modes 0..3), ST (0, 0) .. (1, 1) per quarter, coloured +0x60 (Q 1.0),
/// texture D+0x88 (0x207E50 filter 0), blend (mode >> 4) (0: 0x42) at
/// `pos` turned by the task's angles +0x20 (scale 0.5); each keyed by its
/// mean w into D+0x80. `saves`: the caller's s0..s3 and the return
/// address. Returns a3 as the last library call left it (0 after a
/// drawn last quarter [run], the descriptor after a rejected one [H]).
fn quarters(fx: &mut Fx, t: u32, pos: u32, mode: i32, saves: [u64; 5]) -> u32 {
    let sp = fx.enter(0x1C0);
    fx.saved(sp, &[(0, saves[0]), (0x10, saves[1]), (0x20, saves[2]), (0x30, saves[3]), (0x40, saves[4])]);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x50 + 4 * k, w);
    }
    let d = sp + 0x110;
    fx.mem.set16(d + 2, 0x34);
    fx.mem.set16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    for (local, to) in [(0x1A0u32, 0x10u32), (0x1A8, 0x28), (0x1B0, 0x40), (0x1B8, 0x58)] {
        l2::rgbaq(fx, sp + local, t + 0x60);
        let (a, b) = (fx.mem.read_u32(sp + local), fx.mem.read_u32(sp + local + 4));
        fx.mem.write_u32(d + to, a);
        fx.mem.write_u32(d + to + 4, b);
    }
    let b = (mode >> 4) as i16 as i64;
    let alpha = if b == 0 { 0x80_0000_0042u64 } else { ((b << 2) as u64) | 0x80_0000_0040 };
    fx.desc_alpha(d, alpha);
    let tx = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tx, 0);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x50 + 4 * k);
        fx.mem.write_u32(sp + 0xA0 + 4 * k, w);
    }
    l2::set3(fx, sp + 0xB0, 0.5, 0.5, 0.5);
    l2::xform_vp(fx, sp + 0xC0, sp + 0xB0, t + 0x20, sp + 0xA0);
    for (o, v) in [(0x18u32, 0u32), (0x1C, 0), (0x30, 0x3F80_0000), (0x34, 0), (0x48, 0), (0x4C, 0x3F80_0000), (0x60, 0x3F80_0000), (0x64, 0x3F80_0000)] {
        fx.mem.write_u32(d + o, v);
    }
    let ot = |fx: &mut Fx| fx.mem.read_u32(D + 0x80);
    let mut a3 = d;
    for q in 0..4 {
        let (a, b) = (fx.mem.i16(t + 8), fx.mem.i16(t + 0xA));
        l2::quad_ints(fx, sp + 0x60, a, b, q);
        if l2::project(fx, sp + 0x60, SPR, sp + 0xC0, d, sp + 0x100) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x100)) as i32;
            let o = ot(fx);
            fx.submit(d, o, key);
            // a3 = 0 after 0x1A2C70 (b165's rule) [run].
            a3 = 0;
        } else {
            // 0x204F70 rejected: its a3 (the descriptor) [H].
            a3 = d;
        }
    }
    fx.leave(0x1C0);
    a3
}

/// 0x1E04060(D, step, 0): b287's spawn (the same function); its frame's
/// saves the emitter's s0 = D / s1, the entry's s2 = D / s3 = param, the
/// runner's s4 (0 [H]) and the return address `ra`.
fn spawn(fx: &mut Fx, step: u32, s1: u32, param: i32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(s1)), (0x20, u64::from(D)), (0x30, param as i64 as u64), (0x40, 0), (0x50, u64::from(ra))]);
    let ret = match (0..TASKS).find(|&k| fx.mem.i16(D + 0x2F8 + 0x70 * k) == 0) {
        Some(k) => {
            let t = D + 0x2A0 + 0x70 * k;
            fx.bzero(t, 0x70);
            fx.mem.set16(t + 0x58, 1);
            for o in [0x60u32, 0x64] {
                let (r, g, b) = (fx.mem.read_u8(COLOUR), fx.mem.read_u8(COLOUR + 1), fx.mem.read_u8(COLOUR + 2));
                l2::rgb(fx, t + o, i32::from(r), i32::from(g), i32::from(b));
                let a = fx.mem.read_u8(COLOUR + 3);
                fx.mem.write_u8(t + o + 3, a);
            }
            l2::to_i16x4(fx, t, POS, 0);
            for k in 0..3 {
                let w = fx.mem.read_u32(POS + 4 * k);
                fx.mem.write_u32(t + 0x10 + 4 * k, w);
            }
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x50, s);
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x52, s);
            let a = fx.mem.i16(SPAWN_A);
            fx.mem.set16(t + 0x5A, a);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

fn node_pos(fx: &mut Fx, k: u32) {
    let h = fx.self_handle();
    let slot = node(fx, k);
    fx.node_position(h, slot, POS);
}

/// Four columns (angles 0 / 90 kind 0, 45 / 135 kind 1) of step `step`.
fn columns(fx: &mut Fx, step: u32, s1: u32, param: i32, ras: [u32; 4]) {
    for ((a, k), ra) in [(0, 0), (0x5A, 0), (0x2D, 1), (0x87, 1)].into_iter().zip(ras) {
        fx.mem.set16(SPAWN_A, a);
        fx.mem.set16(SPAWN_K, k);
        spawn(fx, step, s1, param, ra);
    }
}

/// 0x1E01A70(D, 0): the update counter and the emitters (frame 0x30: ra,
/// the entry's s1 = 0, s0 = 0; then s0 = D, s1 a loop count).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x30);
    fx.saved(sp, &[(0x20, 0x01E0_3B58), (0x10, 0), (0, 0)]);
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    let c = |fx: &mut Fx| fx.mem.i16(COUNTER);
    if c(fx) == 1 {
        spawn(fx, LIGHT, 0, param, 0x01E0_1AC4);
    }
    if c(fx) == 1 {
        spawn(fx, FRAME, 0, param, 0x01E0_1AF0);
    }
    for ra in [0x01E0_1B1C, 0x01E0_1B48, 0x01E0_1B74] {
        if c(fx) == 1 {
            spawn(fx, FLASH, 0, param, ra);
        }
    }
    if c(fx) == 0x23 {
        let h = fx.self_handle();
        fx.unit_position(h, POS);
        // 0x1F71A0's frame below the emitter's (its saved ra, s0).
        fx.saved(sp - 0x20, &[(0x10, 0x01E0_1BA0), (0, u64::from(D))]);
        let z = fx.mem.f(POS + 8);
        let z = if fx.mem.i16(FACING) != 0 { ee::add(z, 30.0) } else { ee::sub(z, 30.0) };
        fx.mem.setf(POS + 8, z);
        spawn(fx, GLOW, 0, param, 0x01E0_1BFC);
    }
    if c(fx) == 0x19 || c(fx) == 0xCB {
        for (k, ras) in [(4, [0x01E0_1C84, 0x01E0_1CB4, 0x01E0_1CE8, 0x01E0_1D1C]), (5, [0x01E0_1D80, 0x01E0_1DB0, 0x01E0_1DE4, 0x01E0_1E18])] {
            node_pos(fx, k);
            let y = ee::sub(fx.mem.f(POS + 4), f32::from_bits(0x3F33_3333));
            fx.mem.setf(POS + 4, y);
            columns(fx, COLUMN_R, 0, param, ras);
        }
    }
    if c(fx) == 0x74 || c(fx) == 0x9C {
        let mut s1 = 0u32;
        let sites = [
            (0x01E0_1E70u32, 0x01E0_1E8Cu32, [0x01E0_1EF0u32, 0x01E0_1F20, 0x01E0_1F54, 0x01E0_1F88], 0x01E0_1FA4u32),
            (0x01E0_1FFC, 0x01E0_2018, [0x01E0_2080, 0x01E0_20B0, 0x01E0_20E4, 0x01E0_2118], 0x01E0_2134),
        ];
        for (k, (ra_arc, ra_ring, ras_col, ra_arcs)) in sites.into_iter().enumerate() {
            node_pos(fx, k as u32);
            spawn(fx, ARC, s1, param, ra_arc);
            for i in 0..0xA {
                spawn(fx, RING, i, param, ra_ring);
            }
            s1 = 0xA;
            if c(fx) == 0x9C {
                columns(fx, COLUMN_W, s1, param, ras_col);
                for i in 0..0xA {
                    spawn(fx, ARC_K, i, param, ra_arcs);
                }
            }
        }
    }
    if c(fx) == 0x56 {
        spawn(fx, TRAILS, 0, param, 0x01E0_2184);
    }
    if (0x56..0xB4).contains(&c(fx)) {
        node_pos(fx, 0);
        spawn(fx, RING_S, 0, param, 0x01E0_21DC);
        node_pos(fx, 1);
        spawn(fx, RING_S, 0, param, 0x01E0_2210);
    }
    fx.leave(0x30);
}

/// Byte `at` up by `up` while under `cap`.
fn fade_in(fx: &mut Fx, t: u32, at: u32, cap: u8, up: u8) {
    let a = fx.mem.read_u8(t + at);
    if a < cap {
        fx.mem.write_u8(t + at, a.wrapping_add(up));
    }
}

/// Byte `at` down by `down` while at least `floor`, else the task ends.
fn fade_out(fx: &mut Fx, t: u32, at: u32, floor: u8, down: u8) {
    let a = fx.mem.read_u8(t + at);
    if a >= floor {
        fx.mem.write_u8(t + at, a.wrapping_sub(down));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// 0x1E02230 (`grows`) / 0x1E023E0 (`f`) (D, task): a grey ring of quads
/// (b641's / b400's): size 10..24, cell row 0..2, radius 10..49 (growing
/// by +0xE 16..35 a update), a random angle, row 0..34, life 3..5; in by
/// 0x40 under 0xC0, out by 14.
fn ring(fx: &mut Fx, t: u32, f: u32, grows: bool) {
    if fx.mem.i16(t + 0x58) == 1 {
        let v = rmod(fx, t, f + 0x24, 0xF);
        fx.mem.setf(t + 0x48, ee::from_i32(v + 0xA));
        let v = rmod(fx, t, f + 0x50, 3);
        fx.mem.set16(t + 0x56, v);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        let v = rmod(fx, t, f + 0x7C, 0x28);
        fx.mem.set16(t + 0x50, v + 0xA);
        let v = rmod(fx, t, f + 0x9C, 0x168);
        fx.mem.set16(t + 0x5A, v);
        let v = rmod(fx, t, f + 0xB8, 0x14);
        fx.mem.set16(t + 0xE, v + 0x10);
        let v = rmod(fx, t, f + 0xD8, 0x23);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, f + 0xF4, 3);
        fx.mem.set16(t + 6, v + 3);
        fx.mem.set16(t + 0x58, 2);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x40));
        }
    } else if a >= 0x10 {
        fx.mem.write_u8(t + 0x63, a - 0xE);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if grows && fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    age(fx, t);
}

/// 0x1E02570 (D, task): b432's arc (`arc_strips`, angle 10 spanning 10) by
/// the kind +0x50: 0 a white over red (0xFF, 0x40, 0x40) arc of radius 220
/// for 14 updates, out by 9; 1 a blue-grey (0x20, 0x20, 0x68) over dark
/// (0x18) arc at random angles, 80 wide (the inner 0.6 of it) growing by
/// 40, for 8 updates, out by 15; in by 0x60 under 0xC0 during the life.
fn arc_k(fx: &mut Fx, t: u32) {
    let k06 = f32::from_bits(0x3F19_999A);
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x4C, 0x435C_0000);
            fx.mem.set16(t + 6, 0xE);
            l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
            l2::rgb(fx, t + 0x60, 0xFF, 0x40, 0x40);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.set16(t + 6, 8);
            l2::rgb(fx, t + 0x64, 0x20, 0x20, 0x68);
            l2::rgb(fx, t + 0x60, 0x18, 0x18, 0x18);
            for (o, site) in [(0x20u32, 0x01E0_2618u32), (0x24, 0x01E0_2634), (0x28, 0x01E0_2650)] {
                let v = rmod(fx, t, site, 0xFFFF);
                fx.mem.set32(t + o, v);
            }
            fx.mem.write_u32(t + 0x48, 0x42A0_0000);
            let r = fx.mem.f(t + 0x48);
            fx.mem.setf(t + 0x4C, ee::mul(k06, r));
        }
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        fade_in(fx, t, 0x67, 0xC0, 0x60);
    } else {
        if fx.mem.i16(t + 0x50) == 0 {
            fade_out(fx, t, 0x67, 0xB, 9);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fade_out(fx, t, 0x67, 0x11, 0xF);
        }
    }
    if fx.mem.i16(t + 0x50) == 1 {
        let r = ee::add(fx.mem.f(t + 0x48), 40.0);
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, ee::mul(k06, r));
    }
    age(fx, t);
}

/// 0x1E02790 (D, task): a dim grey glow 30 x 30 (+0x34 90, +0x48 1.0):
/// in by 12 under 0x80 until update 0xC8, then out by 9; turned 0x200 a
/// update about z (+0x28).
fn glow(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x56, 0);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.write_u32(t + 0x34, 0x42B4_0000);
        fx.mem.set16(t + 8, 0x1E);
        fx.mem.set16(t + 0xA, 0x1E);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.write_u32(t + 0x48, 0x3F80_0000);
    }
    if fx.mem.i16(COUNTER) < 0xC8 {
        fade_in(fx, t, 0x63, 0x80, 0xC);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    let v = fx.mem.read_u32(t + 0x28).wrapping_add(0x200);
    fx.mem.write_u32(t + 0x28, v);
    age(fx, t);
}

/// 0x1E02880 / 0x1E02A40 (D, task): b703's light column (0x1E02840) with
/// colours `rgb` (+0x60 / +0x64), kind 0 / 1 / 2 width / height
/// (`wh.0`, `wh.1`) / (`wh.0`, `wh.2`) / (8, 400), kinds 0 / 1 growing by
/// `dh`; in by 0x40 under 0x80 for 4 updates, then out by 9; turned by 6
/// a update.
fn column(fx: &mut Fx, t: u32, rgb: ((i32, i32, i32), (i32, i32, i32)), wh: (u32, u32, u32), dh: (f32, f32)) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, rgb.0 .0, rgb.0 .1, rgb.0 .2);
        l2::rgb(fx, t + 0x64, rgb.1 .0, rgb.1 .1, rgb.1 .2);
        for (kind, w, h) in [(0, wh.0, wh.1), (1, wh.0, wh.2), (2, 0x4100_0000, 0x43C8_0000)] {
            if fx.mem.i16(t + 0x50) == kind {
                fx.mem.write_u32(t + 0x30, w);
                fx.mem.write_u32(t + 0x34, h);
            }
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.set16(t + 0x52, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x63, 0x80, 0x40);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    for (kind, dh) in [(0, dh.0), (1, dh.1)] {
        if fx.mem.i16(t + 0x50) == kind && fx.mem.i16(t + 0x54) != 0 {
            add(fx, t + 0x34, dh);
        }
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let c = fx.mem.i16(t + 0x52);
        if c < 0x19 {
            fx.mem.set16(t + 0x52, c + 1);
        }
    }
    age(fx, t);
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
}

/// 0x1E02C00 (D, task): b293's ribbon task (0x1E02780) in blue (0x20,
/// 0x20, 0xC0) with the histories on nodes 0 / 2 / 1 / 3: in by 0x3C
/// under 0xF0 to update 0xB7, then out by 9; to update 0xB8 each history
/// takes its node's point.
fn trails(fx: &mut Fx, t: u32, sp: u32) {
    l2::rgb(fx, t + 0x60, 0x20, 0x20, 0xC0);
    let nodes = [0u32, 2, 1, 3];
    let ras_fill = [0x01E0_2C68u32, 0x01E0_2CD4, 0x01E0_2D44, 0x01E0_2DB4];
    let ras_step = [0x01E0_2E98u32, 0x01E0_2ED0, 0x01E0_2F08, 0x01E0_2F40];
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        fx.mem.set16(t + 0xE, 0x1E);
        for k in 0..4usize {
            let h = fx.self_handle();
            let slot = node(fx, nodes[k]);
            crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ras_fill[k]);
            for e in 0..0x13u32 {
                l2::copy3(fx, HIST[k] + 0x10 * e, sp + 0x30);
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(COUNTER) < 0xB8 {
        fade_in(fx, t, 0x63, 0xF0, 0x3C);
    } else {
        fade_out(fx, t, 0x63, 0xB, 9);
    }
    if fx.mem.i16(COUNTER) < 0xB9 {
        for k in 0..4usize {
            let h = fx.self_handle();
            let slot = node(fx, nodes[k]);
            crate::b076::node_position_saving(fx, h, slot, sp + 0x30, u64::from(t), Some(u64::from(D)), ras_step[k]);
            libspawn::history(fx, HIST[k] + 0x130, 0x14, sp + 0x30);
        }
    }
    fx.mem.set16(t + 6, 0xA);
    age(fx, t);
}

/// 0x1E02F90 (D, task): b590's arc (0x1E02330) with outer radius 220
/// growing by 0.4, inner (0x80, 0x80, 0xFF), outer 0xC0 grey in by 0x30
/// under 0xC0 for 4 updates, then out by 15; turned by the camera.
fn arc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x4C, 0x435C_0000);
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
        fx.mem.set16(t + 0x52, 0);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 4 {
        fade_in(fx, t, 0x67, 0xC0, 0x30);
    } else {
        fade_out(fx, t, 0x67, 0x11, 0xF);
    }
    camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_3050);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    if fx.mem.i16(t + 0x54) != 0 {
        add(fx, t + 0x4C, f32::from_bits(0x3ECC_CCCD));
    }
    age(fx, t);
}

/// 0x1E030B0 (D, task): b293's stage light (0x1E02C10, word for word).
fn light(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < 0x97 {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x31 { 0x30 } else { l - 0xA });
    } else if fx.mem.i32(TIMER) < 0x1E {
        let l = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, if l < 0x80 { l + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    age(fx, t);
    if fx.mem.i32(TIMER) == 3 {
        fx.set_stage_light(0x80);
        fx.mem.set16(t + 0x58, 0);
    } else {
        let l = fx.mem.read_u8(t + 0x50);
        fx.set_stage_light(i32::from(l));
    }
}

/// 0x1E031C0 (D, task; frame 0x90): b702's frame flash (0x1E02AD0) in grey
/// (0x80), ST from +0x48 = 0 growing by 0.3 to 5 from update 0xBF: in by
/// 0x10 under 0x40 until update 0xC7, then out by 4.
fn frame_flash(fx: &mut Fx, t: u32, sp: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    for k in 0..4 {
        let c = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, c);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x50, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(512.0, s), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(512.0, s), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(COUNTER) >= 0xBF {
        let s = fx.mem.f(t + 0x48);
        if s < 5.0 {
            fx.mem.setf(t + 0x48, ee::add(s, f32::from_bits(0x3E99_999A)));
        }
    }
    if fx.mem.i16(COUNTER) < 0xC8 {
        fade_in(fx, t, 0x63, 0x40, 0x10);
    } else {
        fade_out(fx, t, 0x63, 6, 4);
    }
    age(fx, t);
}

/// 0x1E03410(D, t): b287's flash step (0x1E02EB0) in a grey flashing by
/// the age % 8 (0x60 0x50 0x40 0x30 0x20 0x30 0x40 0x50): alpha up by 9
/// below 0x80 for 220 updates, then down by 7 (the task ends under 9).
/// `regs` the unit flash's s0 / s1 (the slot index, the live count).
fn flash(fx: &mut Fx, t: u32, regs: [u32; 2]) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x30, 0x01E0_488C), (0x20, u64::from(regs[1])), (0x10, u64::from(regs[0]))]);
    // swc1 f21 / f20: the runner's (0) [H].
    fx.mem.write_u32(sp + 4, 0);
    fx.mem.write_u32(sp, 0);
    let w = ee::div(ee::from_i32(fx.mem.i16(t + 8)), 10.0);
    let h = ee::div(ee::from_i32(fx.mem.i16(t + 0xA)), 10.0);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u8(t + 0x63, 0);
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x60);
        fx.mem.set16(t + 0x58, 2);
    }
    let g = [0x60, 0x50, 0x40, 0x30, 0x20, 0x30, 0x40, 0x50];
    for (k, v) in g.into_iter().enumerate() {
        if fx.mem.i16(t + 0x54) % 8 == k as i32 {
            l2::rgb(fx, t + 0x60, v, v, v);
        }
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
    fx.mem.setf(d + 0x18, ee::div(w, 512.0));
    fx.mem.setf(d + 0x1C, ee::div(h, 512.0));
    fx.mem.setf(d + 0x28, ee::div(ee::sub(512.0, w), 512.0));
    fx.mem.setf(d + 0x2C, ee::div(ee::sub(512.0, h), 512.0));
    fx.submit(d, 0, 0);
    if fx.mem.i16(t + 0x54) < 0xDC {
        fade_in(fx, t, 0x63, 0x80, 9);
    } else {
        fade_out(fx, t, 0x63, 9, 7);
    }
    age(fx, t);
    fx.leave(0x40);
}

/// 0x1E04780(D): b550's unit flash (0x1E05CF0) on this record's globals
/// and 400 tasks: in a packet layer (0x2078A0) the attacker drawn into
/// the target (0x1F8270 filter 0; ALPHA 0x48), TEXFLUSH before and after;
/// every flash task stepped (0x1E03410); the layer linked into the
/// ordering table at key 0 when any task was alive.
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
    fx.texflush([5, d, 0x26, a3]);
    fx.desc_alpha(d, 0x80_0000_0048);
    let h = fx.self_handle();
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x98, sp + 0xA0);
    fx.texflush(regs);
    let mut alive = 0u32;
    for k in 0..TASKS {
        let slot = D + 0x70 * k;
        if fx.mem.i16(slot + 0x2F8) == 0 {
            continue;
        }
        alive += 1;
        if fx.mem.read_u32(slot + 0x2E4) == FLASH {
            flash(fx, slot + 0x2A0, [k, alive]);
        }
    }
    if alive != 0 {
        let layer = fx.mem.read_u32(LAYER);
        fx.layer_end(layer, OT, 0);
    }
    fx.leave(0xB0);
}
