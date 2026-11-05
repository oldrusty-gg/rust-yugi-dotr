//! BMODULE b170 (card 316), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E00C60, data area 0x1ED0000, 0x7A00 bytes) [F]
//!: a pool of 50 tasks of 0x70 bytes at D+0x2A0
//! (alive +0x58, step +0x44 through `jalr`), spawned by 0x1E01090 (b272's
//! spawn on these globals: colour D+0x7624, kind D+0x7682 into +0x50), no
//! textures. Drawn with gouraud rings (`quads4::ring_band` 0x1E00110) and
//! the record's gouraud tube 0x1E011D0 (b577's 2-fin tube recompiled in a
//! 0x310 frame: ALPHA 0x44, the width down by size / 20n a piece, the depth
//! by the update counter D+0xB6) along the 70-point history D+0x18B0 of a
//! point D+0x74A0 that follows node 1's x, node 0x29's y + 1.3 and (to
//! update 0x49) node 1's z, then 6.5 a update along the facing to 0x75.
//! Update 0x49 two violet rings at that point's copy D+0x74B0 / itself and
//! the tube; 0x4A a disc ring at the point. Returns 1 on update 0x5A, 4 on
//! 0xFA, 2 on 0xF0, 6 on 0x65..0xEA, 8 once the 0x136-update timer D+0x7620
//! has run out.
use crate::blib::{quads4, spawn as hist, spline};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const SPR: u32 = 0x7000_0000;
const TASKS: u32 = 50;
const POS: u32 = D + 0x250;
const TUBE: u32 = 0x01E0_0760;
const RING: u32 = 0x01E0_0980;
const DISC: u32 = 0x01E0_0B00;
/// The entry's return address after its `jalr` into a step.
const TASK_RA: u64 = 0x01E0_0E50;
const TRAIL: u32 = D + 0x18B0;
const HEAD: u32 = D + 0x74A0;
const HEAD_B: u32 = D + 0x74B0;
const TIMER: u32 = D + 0x7620;
const COLOUR: u32 = D + 0x7624;
const NODES: u32 = D + 0x7654;
const SPAWN_K: u32 = D + 0x7682;
const FACING: u32 = D + 0x7686;
const COUNTER: u32 = D + 0x768A;
const TAB: u32 = D + 0x788C;

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

fn counter(fx: &mut Fx) -> i32 {
    fx.mem.i16(COUNTER)
}

/// The entry 0x1E00C60 (D, param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    fx.mem.write_u32(D + 0x762C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param, &V_B170) } else { step(fx, sp, param) };
    fx.leave(0x70);
    ret
}

/// The init differences of b170 / b260.
struct V {
    timer: i32,
    /// The first node requested (attacker, cleared).
    node: i32,
    /// Return addresses of 0x1A2B80 and the two byte-table rand loops.
    ras: [u32; 3],
}

const V_B170: V = V { timer: 0x136, node: 0x29, ras: [0x01E0_0D58, 0x01E0_0D7C, 0x01E0_0DC4] };
const V_B260: V = V { timer: 0x104, node: 2, ras: [0x01E0_0CB8, 0x01E0_0CDC, 0x01E0_0D24] };

fn init(fx: &mut Fx, sp: u32, param: i32, v: &V) -> i32 {
    fx.bzero(D, 0x7A00);
    l2::sine_table(fx, TAB);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x7688, param);
    fx.mem.set32(TIMER, v.timer);
    for (k, n) in [v.node, 0x1E, 1].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set16(NODES + 2 * k as u32, r);
    }
    let h = fx.target_handle();
    let r = fx.request_node(h, 1, 1);
    fx.mem.set16(D + 0x7664, r);
    let h = fx.self_handle();
    fx.unit_rotation(h, sp + 0x60);
    let y = ee::div(ee::from_i32(fx.mem.i32(sp + 0x64)), 180.0);
    fx.fix_residue(y, v.ras[0]);
    fx.mem.set16(FACING, ee::fixsfsi(y));
    for k in 0..3 {
        fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, v.ras[1]);
        let r = fx.div(r, 0x1E).1;
        fx.mem.set8(D + 0x768C + k, r - 0xF);
    }
    for k in 0..0x100u32 {
        let r = fx.rand_from(D, v.ras[2]);
        let r = fx.div(r, 0x28).1;
        fx.mem.set8(D + 0x778C + k, r - 0x14);
    }
    fx.mem.write_u32(D + 0x7640, 0x437A_0000);
    0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    emit(fx, param);
    let c = counter(fx);
    fx.mem.set16(D + 0xB6, c);
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = fx.mem.read_u32(t + 0x44);
        if f == 0 {
            continue;
        }
        // The step's prologue: ra, the entry's s1 = 0, s0 = D.
        let frame = if f == DISC { 0x20 } else { 0x30 };
        let s = fx.enter(frame);
        if frame == 0x20 {
            fx.saved(s, &[(0x10, TASK_RA), (0, u64::from(D))]);
        } else {
            fx.saved(s, &[(0x20, TASK_RA), (0x10, 0), (0, u64::from(D))]);
        }
        match f {
            TUBE => tube_step(fx, t),
            RING => ring(fx, t),
            DISC => disc(fx, t),
            _ => panic!("b170: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == TUBE {
            tube(fx, &TUBE_B170, t, TRAIL);
        }
        if f(fx) == RING {
            match fx.mem.i16(t + 0x50) {
                0 => copy4(fx, sp + 0x50, HEAD),
                1 => copy4(fx, sp + 0x50, HEAD_B),
                _ => {}
            }
            let mode = if counter(fx) < 0xA0 { 0x21 } else { 0x11 };
            quads4::ring_band(fx, D, t, mode, sp + 0x50);
        }
        if f(fx) == DISC {
            copy4(fx, sp + 0x50, t + 0x10);
            quads4::ring_band(fx, D, t, 0x11, sp + 0x50);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x5A => 1,
        0xFA => 4,
        0xF0 => 2,
        0x65..=0xEA => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E01090(D, step, 0): b272's spawn; its frame's saves: the emitter's
/// s0 = D, the entry's s1 = 0 / s2 = param, the runner's s3 / s4 (0 [H])
/// and the return address `ra`.
fn spawn(fx: &mut Fx, step: u32, param: i32, ra: u32) -> i32 {
    let f = fx.enter(0x60);
    fx.saved(f, &[(0, u64::from(D)), (0x10, 0), (0x20, param as i64 as u64), (0x30, 0), (0x40, 0), (0x50, u64::from(ra))]);
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
            l2::copy3(fx, t + 0x10, POS);
            let s = fx.mem.i16(SPAWN_K);
            fx.mem.set16(t + 0x50, s);
            fx.mem.write_u32(t + 0x40, 0);
            fx.mem.write_u32(t + 0x44, step);
            1
        }
        None => 0,
    };
    fx.leave(0x60);
    ret
}

/// 0x1E00570(D, 0): the update counter, the head point and the emitters
/// (frame 0x50: ra, the entry's s0 = D).
fn emit(fx: &mut Fx, param: i32) {
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x10, 0x01E0_0E18), (0, u64::from(D))]);
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    for (k, out, ra) in [(2u32, sp + 0x20, 0x01E0_05A0u32), (0, sp + 0x30, 0x01E0_05B8)] {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODES + 2 * k);
        crate::b076::node_position_saving(fx, h, slot, out, u64::from(D), Some(0), ra);
    }
    let x = fx.mem.f(sp + 0x20);
    fx.mem.setf(HEAD, x);
    if counter(fx) < 0x4A {
        let z = fx.mem.f(sp + 0x28);
        fx.mem.setf(HEAD + 8, z);
    } else if counter(fx) < 0x76 {
        let z = fx.mem.f(HEAD + 8);
        let by = f32::from_bits(0x40D0_0000);
        let z = if fx.mem.i16(FACING) != 0 { ee::sub(z, by) } else { ee::add(z, by) };
        fx.mem.setf(HEAD + 8, z);
    }
    if counter(fx) < 0xAA {
        copy4(fx, HEAD_B, HEAD);
    }
    if counter(fx) < 0xDC {
        let y = ee::add(f32::from_bits(0x3FA6_6666), fx.mem.f(sp + 0x34));
        fx.mem.setf(HEAD + 4, y);
    }
    if counter(fx) == 0x4A {
        copy4(fx, POS, HEAD);
        spawn(fx, DISC, param, 0x01E0_06C8);
    }
    if counter(fx) == 0x49 {
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, RING, param, 0x01E0_06F0);
        fx.mem.set16(SPAWN_K, 1);
        spawn(fx, RING, param, 0x01E0_070C);
    }
    if counter(fx) == 0x49 {
        fx.mem.set16(SPAWN_K, 0);
        fx.mem.set16(SPAWN_K, 1);
    }
    if counter(fx) == 0x49 {
        spawn(fx, TUBE, param, 0x01E0_074C);
    }
    fx.leave(0x50);
}

fn age(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

fn add(fx: &mut Fx, at: u32, by: f32) {
    let v = ee::add(fx.mem.f(at), by);
    fx.mem.setf(at, v);
}

/// +0x60 by the age's parity: even (0x80, 0x10, 0xFF), odd (0x40, 0x10,
/// 0x80).
fn flicker(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x54) % 2 == 0 {
        l2::rgb(fx, t + 0x60, 0x80, 0x10, 0xFF);
    } else {
        l2::rgb(fx, t + 0x60, 0x40, 0x10, 0x80);
    }
}

/// 0x1E00760 (D, task): the tube task: on its first update the 69-point
/// history filled with the head, (0x40, 0x10, 0x80) outer, size 20; +6 up
/// to 0x44 points; alpha 0xC0 to update 0xD6, then down by 7 and the size
/// by 0.5 to 0; flickering; from age 0x10 to 0x4F the size up by 7 (13
/// from update 0x87); to age 0x31 the history takes the head.
fn tube_step(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x63, 0);
        for e in 0..0x45u32 {
            l2::copy3(fx, TRAIL + 0x10 * e, HEAD);
        }
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let n = fx.mem.i16(t + 6);
    if n < 0x44 {
        fx.mem.set16(t + 6, n + 1);
    }
    if counter(fx) < 0xD7 {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 9 {
            fx.mem.write_u8(t + 0x63, a - 7);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let s = fx.mem.f(t + 0x48);
        if !crate::exe::ee_le(s, 0.0) {
            fx.mem.setf(t + 0x48, ee::sub(s, 0.5));
        }
    }
    flicker(fx, t);
    let a = fx.mem.i16(t + 0x54);
    if (0x10..0x50).contains(&a) {
        let by = if counter(fx) < 0x87 { 7.0 } else { 13.0 };
        add(fx, t + 0x48, by);
    }
    if fx.mem.i16(t + 0x54) < 0x32 {
        hist::history(fx, D + 0x1D00, 0x46, HEAD);
    }
    age(fx, t);
}

/// 0x1E00980 (D, task): a violet ring, inner radius 30 growing by 3 to age
/// 0x4F, white inner; in by 0x10 under 0xC0 to update 0x6E, then out by
/// 0xC; flickering, turned to the camera a quarter on.
fn ring(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41F0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
        let v = fx.rand_from(t, 0x01E0_09DC);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    flicker(fx, t);
    let a = fx.mem.read_u8(t + 0x63);
    if counter(fx) < 0x6F {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x10));
        }
    } else if a >= 0xE {
        fx.mem.write_u8(t + 0x63, a - 0xC);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x50 {
        add(fx, t + 0x48, 3.0);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_0AD0);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// 0x1E00B00 (D, task): the disc ring: inner radius 90, white inner, outer
/// flickering (0x40, 0x10, 0x80) / (0x60, 0x10, 0xA0); in by 0x2A under
/// 0x80 to age 0x4F, then out by 8; turned to the camera a quarter on.
fn disc(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
        let v = fx.rand_from(t, 0x01E0_0B54);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x50 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x2A));
        }
    } else if a >= 0xA {
        fx.mem.write_u8(t + 0x63, a - 8);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    fx.mem.write_u32(t + 0x48, 0x42B4_0000);
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
    } else {
        l2::rgb(fx, t + 0x64, 0x60, 0x10, 0xA0);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_0C2C);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// The Lagrange curve (44769f26f5eb's basis 0) through `p` at `t` into
/// `out` (xyz), summed as the original's mula / madd / adda chain
/// (`tubetrail`'s).
fn curve(fx: &mut Fx, out: u32, p: [u32; 4], t: f32) {
    use ee::{add, mul};
    let w = spline::weights(t, 0).unwrap_or([0.0; 4]);
    for c in 0..3u32 {
        let v: [f32; 4] = std::array::from_fn(|i| fx.mem.f(p[i] + 4 * c));
        let acc = add(mul(v[2], w[2]), add(mul(v[0], w[0]), mul(v[1], w[1])));
        fx.mem.setf(out + 4 * c, add(acc, mul(v[3], w[3])));
    }
}

/// How a compile of the record's gouraud tube lays out its frame.
struct Tube {
    frame: u32,
    /// The locals from sp + 0xE0 up this much lower (the trail at sp +
    /// 0x100 - low, n - 1 at sp + 0xF0 - low).
    low: u32,
    /// The piece counter's slot (b260's sp + 0xE0), else none.
    piece_slot: Option<u32>,
    /// Fins a piece: up, down, then (4) +x, -x.
    fins: i32,
    /// Keyed by the mean w (else the far Z at key 0) for the update
    /// counter D+0xB6 and the segment.
    keyed: fn(i32, i32) -> bool,
}

/// b170's 0x1E011D0: 0x310 frame, 2 fins, depth below 0x68 the mean w,
/// below 0x8C the far Z for the first 27 segments (then the mean w), else
/// the far Z.
const TUBE_B170: Tube = Tube { frame: 0x310, low: 0x10, piece_slot: None, fins: 2, keyed: |c, seg| c < 0x68 || (c < 0x8C && seg >= 0x1B) };

/// b260's 0x1E01220: 0x320 frame (b577's locals), the piece counter at
/// sp + 0xE0, 4 fins, depth below 0x60 the mean w, else the far Z.
const TUBE_B260: Tube = Tube { frame: 0x320, low: 0, piece_slot: Some(0xE0), fins: 4, keyed: |c, _| c < 0x60 };

/// 0x1E011D0 / b260's 0x1E01220 (D, task, trail, _):
/// `tubetrail::tube_gouraud` recompiled (layout `l`): ALPHA 0x44, the half
/// width down by size / 20n a piece and the depth by the update counter
/// D+0xB6 (a3 unused) [F].
fn tube(fx: &mut Fx, l: &Tube, t: u32, a: u32) {
    use ee::{add, div, sub};
    let sp = fx.enter(l.frame);
    let o = |x: u32| sp + x - l.low;
    let mut f30 = div(fx.mem.f(t + 0x48), 10.0);
    fx.mem.write_u32(o(0x100), a);
    let n = fx.mem.i16(t + 6);
    let d = o(0x270);
    fx.mem.setf(sp + 0xD0, f30);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for (local, c, to) in [(0x300u32, 0x60u32, 0x10u32), (0x308, 0x64, 0x20), (0x310, 0x60, 0x30), (0x318, 0x64, 0x40)] {
        crate::c061::rgbaq_at(fx, o(local), t + c, d + to);
    }
    fx.desc_alpha(d, 0x80_0000_0044);
    copy4(fx, o(0x260), a);
    copy4(fx, o(0x130), o(0x260));
    copy4(fx, o(0x140), o(0x260));
    copy4(fx, o(0x150), a + 0x10);
    copy4(fx, o(0x160), a + 0x20);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x20 + 4 * k);
        fx.mem.write_u32(o(0x1C0) + 4 * k, w);
    }
    l2::xform_vp(fx, o(0x1E0), 0, t + 0x20, o(0x1C0));
    let w = fx.mem.read_u32(o(0x140));
    fx.mem.write_u32(sp + 0xD4, w);
    let w = fx.mem.read_u32(o(0x150));
    fx.mem.write_u32(sp + 0xD8, w);
    fx.mem.set32(o(0xF0), n - 1);
    fx.mem.setf(sp + 0xDC, ee::mul(2.0, ee::from_i32(n)));
    let first = [o(0x130), o(0x140), o(0x150), o(0x160)];
    let v = o(0x180);
    let mut seg = 0i32;
    while seg < n - 1 {
        let mut f29 = 0.0f32;
        let p = (a as i32 + 0x10 * (seg as i16 as i32)) as u32;
        let pts = if seg == 0 { first } else { [p - 0x10, p, p + 0x10, p + 0x20] };
        if let Some(at) = l.piece_slot {
            fx.mem.set32(sp + at, 0);
        }
        for piece in 0..2 {
            if seg == 0 && piece == 0 {
                curve(fx, o(0x240), pts, f29);
                copy4(fx, o(0x110), o(0x240));
            } else {
                copy4(fx, o(0x110), o(0x170));
            }
            f29 = add(f29, 0.5);
            if !(f29 < f32::from_bits(0x3F66_6666)) {
                f29 = 1.0;
            }
            let e = if seg == 0 { o(0x250) } else { o(0x230) };
            curve(fx, e, pts, f29);
            copy4(fx, o(0x120), e);
            copy4(fx, o(0x170), o(0x120));
            let f31 = f30;
            f30 = sub(f30, div(fx.mem.f(sp + 0xD0), fx.mem.f(sp + 0xDC)));
            for fin in 0..l.fins {
                l2::copy3(fx, v, o(0x110));
                l2::copy3(fx, v + 0x10, o(0x110));
                l2::copy3(fx, v + 0x20, o(0x120));
                l2::copy3(fx, v + 0x30, o(0x120));
                let bump = |fx: &mut Fx, at: u32, by: f32, up: bool| {
                    let x = fx.mem.f(v + at);
                    fx.mem.setf(v + at, if up { add(x, by) } else { sub(x, by) });
                };
                let (axis, up) = match fin {
                    0 => (4, true),
                    1 => (4, false),
                    2 => (0, true),
                    _ => (0, false),
                };
                bump(fx, 0x10 + axis, f31, up);
                bump(fx, 0x30 + axis, f30, up);
                let c = fx.mem.i16(D + 0xB6);
                if l2::project(fx, v, SPR, o(0x1E0), d, o(0x1D0)) {
                    if (l.keyed)(c, seg) {
                        let key = ee::fixunssfsi(fx.mem.f(o(0x1D0))) as i32;
                        fx.submit(d, OT, key);
                    } else {
                        l2::far_z_desc(fx, d, 1);
                        fx.submit(d, OT, 0);
                    }
                }
            }
            if let Some(at) = l.piece_slot {
                fx.mem.set32(sp + at, piece + 1);
            }
        }
        seg += 1;
    }
    fx.leave(l.frame);
}

/// b260 (card 329), the entry 0x1E00BC0: b170's globals,
/// init (timer 0x104, the first node 2) and spawn (0x1E010E0), the emitter
/// inline: update 0x4A a disc ring 0x1E00A30 at the head, 0x49 the tube
/// task 0x1E004E0 and one red ring 0x1E008B0 (kind 0). The tube task
/// itself moves the head: its 70-point history is laid each update along
/// node 2 -> the target's node 1 (a thirtieth a point, +0x50 points out,
/// growing to age 0x31), the head its first point, its copy D+0x74B0
/// node 2. Drawn: discs, then the tube (4 fins, keyed below update 0x60)
/// and rings (at D+0x74B0 whatever the kind; mode 0x10 below 0x5C, else
/// 0x11). Returns 1 on 0x55, 4 on 0xFF, 2 on 0xF0, 6 on 0x60..0xEA, 8 once
/// the timer has run out [F].
pub fn program_b260(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    fx.mem.write_u32(D + 0x762C, OT);
    fx.mem.write_u32(D + 0x80, OT);
    let ret = if param >= 0 { init(fx, sp, param, &V_B260) } else { step_b260(fx, sp, param) };
    fx.leave(0x70);
    ret
}

const TUBE_V: u32 = 0x01E0_04E0;
const RING_V: u32 = 0x01E0_08B0;
const DISC_V: u32 = 0x01E0_0A30;

fn step_b260(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = counter(fx) + 1;
    fx.mem.set16(COUNTER, n);
    if counter(fx) == 0x4A {
        spawn(fx, DISC_V, param, 0x01E0_0D9C);
    }
    if counter(fx) == 0x49 {
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, TUBE_V, param, 0x01E0_0DC4);
    }
    if counter(fx) == 0x49 {
        fx.mem.set16(SPAWN_K, 0);
        spawn(fx, RING_V, param, 0x01E0_0DEC);
        fx.mem.set16(SPAWN_K, 1);
    }
    let c = counter(fx);
    fx.mem.set16(D + 0xB6, c);
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
        // The step's prologue: the entry's s0 = D, s1 = 0, s2 = the slot.
        let frame = match f {
            TUBE_V => 0x80,
            DISC_V => 0x20,
            _ => 0x30,
        };
        let s = fx.enter(frame);
        match frame {
            0x20 => fx.saved(s, &[(0x10, 0x01E0_0E30), (0, u64::from(D))]),
            0x30 => fx.saved(s, &[(0x20, 0x01E0_0E30), (0x10, 0), (0, u64::from(D))]),
            _ => {
                fx.saved(s, &[(0x40, 0x01E0_0E30), (0x30, u64::from(slot)), (0x20, 0), (0x10, u64::from(D))]);
                // swc1 f20: the runner's (0) [H].
                fx.mem.write_u32(s, 0);
            }
        }
        match f {
            TUBE_V => tube_b260(fx, t, s),
            RING_V => ring_b260(fx, t),
            DISC_V => disc_b260(fx, t),
            _ => panic!("b260: unknown task step {f:#x}"),
        }
        fx.leave(frame);
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) != 0 && fx.mem.read_u32(t + 0x44) == DISC_V {
            copy4(fx, sp + 0x50, HEAD);
            quads4::ring_band(fx, D, t, 0x11, sp + 0x50);
        }
    }
    for k in 0..TASKS {
        let t = D + 0x2A0 + 0x70 * k;
        if fx.mem.i16(t + 0x58) == 0 {
            continue;
        }
        let f = |fx: &mut Fx| fx.mem.read_u32(t + 0x44);
        if f(fx) == TUBE_V {
            tube(fx, &TUBE_B260, t, TRAIL);
        }
        if f(fx) == RING_V {
            match fx.mem.i16(t + 0x50) {
                0 => copy4(fx, sp + 0x50, HEAD),
                1 => copy4(fx, sp + 0x50, HEAD_B),
                _ => {}
            }
            copy4(fx, sp + 0x50, HEAD_B);
            let mode = if counter(fx) < 0x5C { 0x10 } else { 0x11 };
            quads4::ring_band(fx, D, t, mode, sp + 0x50);
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match counter(fx) {
        0x55 => 1,
        0xFF => 4,
        0xF0 => 2,
        0x60..=0xEA => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// b260's 0x1E004E0 (D, task; frame `sp`): b170's tube task in red
/// ((0x80, 0x10, 0x10) outer, (0xFF / 0xE0, 0x10, 0x10) flickering); +0x50
/// counts to age 0x31; each update the history laid along node 2 -> the
/// target's node 1 / 30 (point k at +0x50 - k (not below 0) thirtieths of
/// the way while k < +6, the rest at node 2), the head its first point.
fn tube_b260(fx: &mut Fx, t: u32, sp: u32) {
    use ee::{div, mul, sub};
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 6, 8);
        fx.mem.write_u8(t + 0x63, 0);
        for e in 0..0x45u32 {
            l2::copy3(fx, TRAIL + 0x10 * e, HEAD);
        }
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0x10);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let n = fx.mem.i16(t + 6);
    if n < 0x44 {
        fx.mem.set16(t + 6, n + 1);
    }
    if counter(fx) < 0xD7 {
        fx.mem.write_u8(t + 0x63, 0xC0);
    } else {
        let a = fx.mem.read_u8(t + 0x63);
        if a >= 9 {
            fx.mem.write_u8(t + 0x63, a - 7);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let s = fx.mem.f(t + 0x48);
        if !crate::exe::ee_le(s, 0.0) {
            fx.mem.setf(t + 0x48, sub(s, 0.5));
        }
    }
    if fx.mem.i16(t + 0x54) % 2 == 0 {
        l2::rgb(fx, t + 0x60, 0xFF, 0x10, 0x10);
    } else {
        l2::rgb(fx, t + 0x60, 0xE0, 0x10, 0x10);
    }
    let a = fx.mem.i16(t + 0x54);
    if (0x10..0x50).contains(&a) {
        let by = if counter(fx) < 0x87 { 7.0 } else { 13.0 };
        add(fx, t + 0x48, by);
    }
    if fx.mem.i16(t + 0x54) < 0x32 {
        let m = fx.mem.i16(t + 0x50) + 1;
        fx.mem.set16(t + 0x50, m);
    }
    let f20 = ee::from_i32(fx.mem.i16(t + 0x50));
    let (p, q) = (sp + 0x50, sp + 0x60);
    let h = fx.self_handle();
    let slot = fx.mem.i16(NODES);
    crate::b076::node_position_saving(fx, h, slot, p, u64::from(D), Some(u64::from(t)), 0x01E0_06FC);
    let h = fx.target_handle();
    let slot = fx.mem.i16(D + 0x7664);
    crate::b076::node_position_saving(fx, h, slot, q, u64::from(q), Some(u64::from(t)), 0x01E0_0718);
    copy4(fx, HEAD_B, p);
    for c in [0u32, 8, 4] {
        let v = div(sub(fx.mem.f(q + c), fx.mem.f(p + c)), 30.0);
        fx.mem.setf(q + c, v);
    }
    for k in 0..0x45i32 {
        let e = TRAIL + 0x10 * k as u32;
        fx.mem.write_u32(e, 0);
        if k < fx.mem.i16(t + 6) {
            let mut f3 = sub(f20, ee::from_i32(k));
            if crate::exe::ee_lt(f3, 0.0) {
                f3 = 0.0;
            }
            for c in [8u32, 0, 4] {
                let v = ee::add(fx.mem.f(p + c), mul(fx.mem.f(q + c), f3));
                fx.mem.setf(e + c, v);
            }
        } else {
            for c in [0u32, 8, 4] {
                let v = fx.mem.read_u32(p + c);
                fx.mem.write_u32(e + c, v);
            }
        }
    }
    copy4(fx, HEAD, TRAIL);
    age(fx, t);
}

/// b260's 0x1E008B0 (D, task): b170's ring in red (inner radius 110 growing
/// by 3 to age 0xB3, outer (0x80, 0x10, 0x10), inner (0xFF / 0xD0, 0x10,
/// 0x10) flickering), in to update 0xEF, then out by 0xC.
fn ring_b260(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x42DC_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0x10);
        let v = fx.rand_from(t, 0x01E0_090C);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) % 2 == 0 {
        l2::rgb(fx, t + 0x60, 0xFF, 0x10, 0x10);
    } else {
        l2::rgb(fx, t + 0x60, 0xD0, 0x10, 0x10);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if counter(fx) < 0xF0 {
        if a < 0xC0 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x10));
        }
    } else if a >= 0xE {
        fx.mem.write_u8(t + 0x63, a - 0xC);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    if fx.mem.i16(t + 0x54) < 0xB4 {
        add(fx, t + 0x48, 3.0);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), u64::from(D), 0x01E0_0A00);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}

/// b260's 0x1E00A30 (D, task): b170's disc ring growing by 3 below 300 (from
/// 90), outer (0x80 / 0xA0, 0x10, 0x10) by the age's parity, in to age
/// 0x77.
fn disc_b260(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40A0_0000);
        fx.mem.write_u32(t + 0x48, 0x42B4_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0x40, 0x10, 0x80);
        let v = fx.rand_from(t, 0x01E0_0A8C);
        let r = fx.div(v, 0x168).1;
        fx.mem.set16(t + 0x5A, r);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let a = fx.mem.read_u8(t + 0x63);
    if fx.mem.i16(t + 0x54) < 0x78 {
        if a < 0x80 {
            fx.mem.write_u8(t + 0x63, a.wrapping_add(0x2A));
        }
    } else if a >= 0xA {
        fx.mem.write_u8(t + 0x63, a - 8);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    let r = fx.mem.f(t + 0x48);
    if crate::exe::ee_lt(r, 300.0) {
        fx.mem.setf(t + 0x48, ee::add(r, 3.0));
    }
    if fx.mem.i16(t + 0x54) % 2 == 1 {
        l2::rgb(fx, t + 0x64, 0x80, 0x10, 0x10);
    } else {
        l2::rgb(fx, t + 0x64, 0xA0, 0x10, 0x10);
    }
    crate::b494::camera_angles_saving(fx, t + 0x20, u64::from(t), 0, 0x01E0_0B90);
    let v = fx.mem.read_u32(t + 0x20).wrapping_add(0x4000);
    fx.mem.write_u32(t + 0x20, v);
    age(fx, t);
}
