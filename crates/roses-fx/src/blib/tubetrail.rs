//! fcbd383137cf [4 B records; b731 (card 51) 0x1E04900, 720 words]
//!: b258's trail tube (0x1E04FE0) compiled for the 0x70-byte
//! tasks, each segment drawn in two halves.
use crate::blib::spline;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

/// The scratchpad, where the original projects.
const SPR: u32 = 0x7000_0000;

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The Lagrange curve (44769f26f5eb's basis 0) through `p` at `t` into
/// `out` (xyz), summed as the original's mula / madd / adda chain.
fn curve(fx: &mut Fx, out: u32, p: [u32; 4], t: f32) {
    use ee::{add, mul};
    let w = spline::weights(t, 0).unwrap_or([0.0; 4]);
    for c in 0..3u32 {
        let v: [f32; 4] = std::array::from_fn(|i| fx.mem.f(p[i] + 4 * c));
        let acc = add(mul(v[2], w[2]), add(mul(v[0], w[0]), mul(v[1], w[1])));
        fx.mem.setf(out + 4 * c, add(acc, mul(v[3], w[3])));
    }
}

/// `tube(D, task, trail, mode)`: a textured tube along the trail `trail`
/// (n = i16 task +6 points of 16 bytes; the first segment's point before
/// is the first point): each of the n - 1 segments in two halves (the
/// curve at t = 0 / 0.5 / 1, the start of each half the previous end),
/// two strip3 quads a half (vertical, then horizontal with half the z
/// offset) of half width size +0x48 / 10 narrowing by size / 20n a half,
/// turned by the task's angles +0x20 at D+0x20 (no scale); colours +0x60 /
/// +0x64, the later pair's alpha down by alpha / n a half; UVs D+0x94 in
/// orientation D+0xBE, texture D+0x88; depth `mode` (0 the mean w, 1 the
/// far Z at key 0, 2 key -1, else nothing projected) into the executable's
/// ordering table (0x206E40) [F].
pub fn tube(fx: &mut Fx, d0: u32, t: u32, a: u32, mode: i32) {
    tube_with(fx, &TRAIL, d0, t, a, mode);
}

/// How a compile of the tube lays out its frame and cuts its segments.
pub struct TubeLayout {
    /// The frame size.
    pub frame: u32,
    /// How far below fcbd383137cf's every local from sp + 0x100 up sits
    /// (sp + 0xD0..0xE0 stay; the half counter at sp + 0xF0 only when 0).
    pub low: u32,
    /// The curve parameter's step (clamped to 1 from 0.9).
    pub step: f32,
    /// Pieces a segment (the loop on the step).
    pub pieces: i32,
    /// Quads a piece (1: the vertical one only).
    pub quads: i32,
    /// The highest depth mode compiled (`None`: always the mean w).
    pub modes: Option<i32>,
    /// Whether `mode` is a3's low nibble and a3 >> 4 the blend (0x207E20:
    /// 0x42 for 0, else (k << 2) | 0x40).
    pub blend: bool,
    /// The stack slot (before `low`) of n x `n_mul`, the alpha's divisor.
    pub n_slot: u32,
    pub n_mul: i32,
    /// The stack slot (before `low`) of n - 1.
    pub last_slot: u32,
    /// The width's divisor per n (the width falls by size / (width n) a
    /// piece).
    pub width: f32,
    /// Whether the segments start at the task's +0xE (else 0).
    pub start_e: bool,
    /// Whether the piece counter lives at sp + 0xF0.
    pub piece_counter: bool,
}

/// fcbd383137cf's layout.
pub const TRAIL: TubeLayout = TubeLayout { frame: 0x350, low: 0, step: 0.5, pieces: 2, quads: 2, modes: Some(2), blend: false, n_slot: 0x110, n_mul: 1, last_slot: 0x100, width: 2.0, start_e: false, piece_counter: true };

/// d3f1c1562bfe [cards 44 0x1E04240 / 670 0x1E03DE0, 633 words] (helper
/// C18): the tube compiled with one piece a segment (the step 1.5, so the
/// curve at t = 0 and 1 only), the vertical quad only, always keyed by the
/// mean w (a3 unused), in a 0x340 frame with the locals from sp + 0x100 up
/// 0x10 lower and no piece counter on the stack [F].
pub const SINGLE: TubeLayout = TubeLayout { frame: 0x340, low: 0x10, step: 1.5, pieces: 1, quads: 1, modes: None, blend: false, n_slot: 0x110, n_mul: 1, last_slot: 0x100, width: 2.0, start_e: false, piece_counter: false };

/// 388b9b4a911b [card 313 0x1E03AA0, 723 words]: the tube in
/// fcbd383137cf's frame with one piece a segment (the step 1.5), both
/// quads, the blend from a3 >> 4 and depth modes a3 & 0xF 0 / 1 only, the
/// segments from the task's +0xE, the alpha down by alpha / 3n (3n at sp +
/// 0x100, n - 1 at sp + 0x110) and the width by size / 8n [F].
pub const SINGLE_BLEND: TubeLayout = TubeLayout { frame: 0x350, low: 0, step: 1.5, pieces: 1, quads: 2, modes: Some(1), blend: true, n_slot: 0x100, n_mul: 3, last_slot: 0x110, width: 8.0, start_e: true, piece_counter: true };

/// a7bb10cd678f [cards 377 0x1E03110 / 467 0x1E042F0, 717 words] (helper
/// E1): fcbd383137cf's tube (two pieces a segment, both quads, depth modes
/// 0..2) with SINGLE_BLEND's divisors: 3n at sp + 0x100 (the alpha falls
/// by alpha / 3n a piece), n - 1 at sp + 0x110, the width by size / 8n [F].
pub const TRAIL3: TubeLayout = TubeLayout { frame: 0x350, low: 0, step: 0.5, pieces: 2, quads: 2, modes: Some(2), blend: false, n_slot: 0x100, n_mul: 3, last_slot: 0x110, width: 8.0, start_e: false, piece_counter: true };

/// [`tube`] as compiled with layout `l`.
pub fn tube_with(fx: &mut Fx, l: &TubeLayout, d0: u32, t: u32, a: u32, mode: i32) {
    use ee::{add, div, sub};
    let sp = fx.enter(l.frame);
    let lo = |o: u32| o - l.low;
    fx.mem.write_u32(sp + lo(0x120), a);
    fx.mem.write_u32(sp + lo(0x12C), t);
    let mut f30 = div(fx.mem.f(t + 0x48), 10.0);
    let n = fx.mem.i16(t + 6);
    let d = sp + lo(0x298);
    l2::desc_strip3(fx, d);
    fx.mem.setf(sp + 0xD0, f30);
    if l.blend {
        let k = (mode >> 4) as i16 as i64;
        let alpha = if k == 0 { 0x80_0000_0042 } else { ((k << 2) as u64) | 0x80_0000_0040 };
        fx.desc_alpha(d, alpha);
    }
    for (local, c, to) in [(0x328u32, 0x60u32, 0x10u32), (0x330, 0x64, 0x28), (0x338, 0x60, 0x40), (0x340, 0x64, 0x58)] {
        crate::c061::rgbaq_at(fx, sp + lo(local), t + c, d + to);
    }
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    let colour = sp + lo(0x34C);
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(colour + k, v);
    }
    let pa = sp + lo(0x150);
    copy4(fx, sp + lo(0x280), a);
    copy4(fx, pa, sp + lo(0x280));
    copy4(fx, pa + 0x10, sp + lo(0x280));
    copy4(fx, pa + 0x20, a + 0x10);
    copy4(fx, pa + 0x30, a + 0x20);
    for k in 0..3 {
        let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
        fx.mem.write_u32(sp + lo(0x1E0) + 4 * k, w);
    }
    l2::xform_vp(fx, sp + lo(0x200), 0, t + 0x20, sp + lo(0x1E0));
    let w = fx.mem.read_u32(pa + 0x10);
    fx.mem.write_u32(sp + 0xD4, w);
    fx.mem.set32(sp + lo(l.n_slot), n * l.n_mul);
    let w = fx.mem.read_u32(pa + 0x20);
    fx.mem.write_u32(sp + 0xD8, w);
    let mut seg = if l.start_e { fx.mem.i16(t + 0xE) } else { 0 };
    fx.mem.set32(sp + 0xE0, seg);
    fx.mem.setf(sp + 0xDC, ee::mul(l.width, ee::from_i32(n)));
    fx.mem.set32(sp + lo(l.last_slot), n - 1);
    let v = sp + lo(0x1A0);
    let (mode, top) = match l.modes {
        None => (0, 0),
        Some(m) if l.blend => (mode & 0xF, m),
        Some(m) => (mode, m),
    };
    while seg < n - 1 {
        let mut f29 = 0.0f32;
        if l.piece_counter {
            fx.mem.set32(sp + 0xF0, 0);
        }
        let p = (a as i32 + 0x10 * (seg as i16 as i32)) as u32;
        let pts = if seg == 0 { [pa, pa + 0x10, pa + 0x20, pa + 0x30] } else { [p - 0x10, p, p + 0x10, p + 0x20] };
        for sub_ in 0..l.pieces {
            if seg == 0 && sub_ == 0 {
                curve(fx, sp + lo(0x260), pts, f29);
                copy4(fx, sp + lo(0x130), sp + lo(0x260));
            } else {
                copy4(fx, sp + lo(0x130), sp + lo(0x190));
            }
            f29 = add(f29, l.step);
            if !(f29 < f32::from_bits(0x3F66_6666)) {
                f29 = 1.0;
            }
            let e = lo(if seg == 0 { 0x270 } else { 0x250 });
            curve(fx, sp + e, pts, f29);
            copy4(fx, sp + lo(0x140), sp + e);
            copy4(fx, sp + lo(0x190), sp + lo(0x140));
            l2::colour_strip3(fx, d, colour, 1);
            let al = i32::from(fx.mem.read_u8(t + 0x63));
            let q = fx.div(al, n * l.n_mul).0 as i16 as i32;
            let r = i32::from(fx.mem.read_u8(colour + 3)) - q;
            fx.mem.write_u8(colour + 3, if r > 0 { r as u8 } else { 0 });
            l2::colour_strip3(fx, d, colour, 2);
            let f31 = f30;
            f30 = sub(f30, div(fx.mem.f(sp + 0xD0), fx.mem.f(sp + 0xDC)));
            for s0 in 0..l.quads {
                l2::copy3(fx, v, sp + lo(0x130));
                l2::copy3(fx, v + 0x10, sp + lo(0x130));
                l2::copy3(fx, v + 0x20, sp + lo(0x140));
                l2::copy3(fx, v + 0x30, sp + lo(0x140));
                let bump = |fx: &mut Fx, at: u32, by: f32, up: bool| {
                    let x = fx.mem.f(v + at);
                    fx.mem.setf(v + at, if up { add(x, by) } else { sub(x, by) });
                };
                if s0 == 0 {
                    bump(fx, 0x14, f31, true);
                    bump(fx, 0x34, f30, true);
                    bump(fx, 0x4, f31, false);
                    bump(fx, 0x24, f30, false);
                } else {
                    let h2 = div(f31, 2.0);
                    bump(fx, 0x10, f31, true);
                    let h1 = div(f30, 2.0);
                    bump(fx, 0x30, f30, true);
                    bump(fx, 0x18, h2, true);
                    bump(fx, 0x38, h1, true);
                    bump(fx, 0x0, f31, false);
                    bump(fx, 0x20, f30, false);
                    bump(fx, 0x8, h2, false);
                    bump(fx, 0x28, h1, false);
                }
                if (0..=top).contains(&mode) && l2::project(fx, v, SPR, sp + lo(0x200), d, sp + lo(0x1F0)) {
                    match mode {
                        0 => {
                            let key = ee::fixunssfsi(fx.mem.f(sp + lo(0x1F0))) as i32;
                            fx.submit(d, OT, key);
                        }
                        1 => {
                            l2::far_z_desc(fx, d, 0);
                            fx.submit(d, OT, 0);
                        }
                        _ => fx.submit(d, OT, -1),
                    }
                }
            }
            if l.piece_counter {
                fx.mem.set32(sp + 0xF0, sub_ + 1);
            }
        }
        seg += 1;
        fx.mem.set32(sp + 0xE0, seg);
    }
    fx.leave(l.frame);
}

/// b88644fdf000 [card 554 0x1E02BF0, 780 words]: a deep
/// recompile of [`TRAIL3`]'s tube in a 0x370 frame: the trail's points in
/// world space (the view-projection alone at sp + 0x200: no D+0x20
/// translation, no turn), the camera's angles (0x205580) at sp + 0x1F0
/// with the y slot replaced by its negated degrees (/ 180 by the
/// 0xB60B60B7 multiply; LO / HI left by it), the mean w at sp + 0x1E0, the
/// colours per vertex through 0x108020 (sp + 0x368 for the first / third
/// vertices, sp + 0x36C for the second / fourth with the first's alpha, the
/// alpha falling by alpha / 3n a piece), only the horizontal quad of each
/// piece (the quad counter starts at 1) and depth modes 0 / 1 submitted
/// twice (mode 2 once, key -1; else nothing projected) into the
/// executable's ordering table [F].
pub fn tube_cam(fx: &mut Fx, d0: u32, t: u32, a: u32, mode: i32) {
    use ee::{add, div, sub};
    let sp = fx.enter(0x370);
    fx.mem.write_u32(sp + 0x120, a);
    fx.mem.write_u32(sp + 0x12C, t);
    let mut f30 = div(fx.mem.f(t + 0x48), 10.0);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x298;
    l2::desc_strip3(fx, d);
    fx.mem.setf(sp + 0xD0, f30);
    for (local, c, to) in [(0x328u32, 0x60u32, 0x10u32), (0x330, 0x64, 0x28), (0x338, 0x60, 0x40), (0x340, 0x64, 0x58)] {
        crate::c061::rgbaq_at(fx, sp + local, t + c, d + to);
    }
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    let (ca, cb) = (sp + 0x368, sp + 0x36C);
    for c in [ca, cb] {
        for k in 0..4 {
            let v = fx.mem.read_u8(t + 0x60 + k);
            fx.mem.write_u8(c + k, v);
        }
    }
    let pa = sp + 0x150;
    copy4(fx, sp + 0x280, a);
    copy4(fx, pa, sp + 0x280);
    copy4(fx, pa + 0x10, sp + 0x280);
    copy4(fx, pa + 0x20, a + 0x10);
    copy4(fx, pa + 0x30, a + 0x20);
    l2::xform_vp(fx, sp + 0x200, 0, 0, 0);
    l2::camera_angles_to(fx, sp + 0x1F0);
    let y = fx.mem.i32(sp + 0x1F4);
    let hi = {
        fx.mult(0xB60B_60B7u32 as i32, y);
        fx.hilo().1
    };
    let q = (hi.wrapping_add(y) >> 7).wrapping_add((y as u32 >> 31) as i32);
    fx.mem.set32(sp + 0x1F4, q.wrapping_neg());
    fx.mem.set32(sp + 0x100, n * 3);
    fx.mem.set32(sp + 0x110, n - 1);
    fx.mem.setf(sp + 0xDC, ee::mul(8.0, ee::from_i32(n)));
    let w = fx.mem.read_u32(pa + 0x10);
    fx.mem.write_u32(sp + 0xD4, w);
    let w = fx.mem.read_u32(pa + 0x20);
    fx.mem.write_u32(sp + 0xD8, w);
    let mut seg = 0;
    fx.mem.set32(sp + 0xE0, seg);
    let v = sp + 0x1A0;
    while seg < n - 1 {
        let mut f29 = 0.0f32;
        fx.mem.set32(sp + 0xF0, 0);
        let p = (a as i32 + 0x10 * (seg as i16 as i32)) as u32;
        let pts = if seg == 0 { [pa, pa + 0x10, pa + 0x20, pa + 0x30] } else { [p - 0x10, p, p + 0x10, p + 0x20] };
        for sub_ in 0..2 {
            if seg == 0 && sub_ == 0 {
                curve(fx, sp + 0x260, pts, f29);
                copy4(fx, sp + 0x130, sp + 0x260);
            } else {
                copy4(fx, sp + 0x130, sp + 0x190);
            }
            f29 = add(f29, 0.5);
            if !(f29 < f32::from_bits(0x3F66_6666)) {
                f29 = 1.0;
            }
            let e = if seg == 0 { 0x270 } else { 0x250 };
            curve(fx, sp + e, pts, f29);
            copy4(fx, sp + 0x140, sp + e);
            copy4(fx, sp + 0x190, sp + 0x140);
            l2::rgbaq(fx, sp + 0x348, ca);
            for (k, o) in [(0u32, 0x10u32), (4, 0x14)] {
                let w = fx.mem.read_u32(sp + 0x348 + k);
                fx.mem.write_u32(d + o, w);
            }
            let al = fx.mem.read_u8(ca + 3);
            fx.mem.write_u8(cb + 3, al);
            l2::rgbaq(fx, sp + 0x350, cb);
            for (k, o) in [(0u32, 0x28u32), (4, 0x2C)] {
                let w = fx.mem.read_u32(sp + 0x350 + k);
                fx.mem.write_u32(d + o, w);
            }
            let al = i32::from(fx.mem.read_u8(t + 0x63));
            let q = fx.div(al, n * 3).0 as i16 as i32;
            let r = i32::from(fx.mem.read_u8(ca + 3)) - q;
            fx.mem.write_u8(ca + 3, if r > 0 { r as u8 } else { 0 });
            l2::rgbaq(fx, sp + 0x358, ca);
            for (k, o) in [(0u32, 0x40u32), (4, 0x44)] {
                let w = fx.mem.read_u32(sp + 0x358 + k);
                fx.mem.write_u32(d + o, w);
            }
            let al = fx.mem.read_u8(ca + 3);
            fx.mem.write_u8(cb + 3, al);
            l2::rgbaq(fx, sp + 0x360, cb);
            let f31 = f30;
            for (k, o) in [(0u32, 0x58u32), (4, 0x5C)] {
                let w = fx.mem.read_u32(sp + 0x360 + k);
                fx.mem.write_u32(d + o, w);
            }
            f30 = sub(f30, div(fx.mem.f(sp + 0xD0), fx.mem.f(sp + 0xDC)));
            // s0 starts at 1: the horizontal quad only.
            for s0 in 1..2 {
                l2::copy3(fx, v, sp + 0x130);
                l2::copy3(fx, v + 0x10, sp + 0x130);
                l2::copy3(fx, v + 0x20, sp + 0x140);
                l2::copy3(fx, v + 0x30, sp + 0x140);
                let bump = |fx: &mut Fx, at: u32, by: f32, up: bool| {
                    let x = fx.mem.f(v + at);
                    fx.mem.setf(v + at, if up { add(x, by) } else { sub(x, by) });
                };
                if s0 == 0 {
                    bump(fx, 0x14, f31, true);
                    bump(fx, 0x34, f30, true);
                    bump(fx, 0x4, f31, false);
                    bump(fx, 0x24, f30, false);
                } else {
                    let h2 = div(f31, 2.0);
                    bump(fx, 0x10, f31, true);
                    let h1 = div(f30, 2.0);
                    bump(fx, 0x30, f30, true);
                    bump(fx, 0x18, h2, true);
                    bump(fx, 0x38, h1, true);
                    bump(fx, 0x0, f31, false);
                    bump(fx, 0x20, f30, false);
                    bump(fx, 0x8, h2, false);
                    bump(fx, 0x28, h1, false);
                }
                if (0..=2).contains(&mode) && l2::project(fx, v, SPR, sp + 0x200, d, sp + 0x1E0) {
                    match mode {
                        0 => {
                            for _ in 0..2 {
                                let key = ee::fixunssfsi(fx.mem.f(sp + 0x1E0)) as i32;
                                fx.submit(d, OT, key);
                            }
                        }
                        1 => {
                            l2::far_z_desc(fx, d, 0);
                            fx.submit(d, OT, 0);
                            fx.submit(d, OT, 0);
                        }
                        _ => fx.submit(d, OT, -1),
                    }
                }
            }
            fx.mem.set32(sp + 0xF0, sub_ + 1);
        }
        seg += 1;
        fx.mem.set32(sp + 0xE0, seg);
    }
    fx.leave(0x370);
}

/// 576188425b73 [card 202 0x1E04450, 676 words]: [`TRAIL`]'s
/// tube (two pieces a segment, both quads, depth modes 0..2 submitted once,
/// the D+0x20 translation and the task's turn) in a 0x340 frame with the
/// locals from sp + 0x100 up 0x10 lower (the task kept in a register, n - 1
/// at sp + 0x100, no n slot) and neither narrowing nor fading: the half
/// width size +0x48 / 10 and the colour +0x60 the same for every piece
/// (0x108070 for both vertex pairs) [F].
pub fn tube_const(fx: &mut Fx, d0: u32, t: u32, a: u32, mode: i32) {
    use ee::{add, div, sub};
    let sp = fx.enter(0x340);
    fx.mem.write_u32(sp + 0x110, a);
    let f31 = div(fx.mem.f(t + 0x48), 10.0);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x288;
    l2::desc_strip3(fx, d);
    for (local, c, to) in [(0x318u32, 0x60u32, 0x10u32), (0x320, 0x64, 0x28), (0x328, 0x60, 0x40), (0x330, 0x64, 0x58)] {
        crate::c061::rgbaq_at(fx, sp + local, t + c, d + to);
    }
    let order = fx.mem.i16(d0 + 0xBE);
    l2::uv_strip3(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    let colour = sp + 0x33C;
    for k in 0..4 {
        let v = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(colour + k, v);
    }
    let pa = sp + 0x140;
    copy4(fx, sp + 0x270, a);
    copy4(fx, pa, sp + 0x270);
    copy4(fx, pa + 0x10, sp + 0x270);
    copy4(fx, pa + 0x20, a + 0x10);
    copy4(fx, pa + 0x30, a + 0x20);
    for k in 0..3 {
        let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0x1D0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x1F0, 0, t + 0x20, sp + 0x1D0);
    let w = fx.mem.read_u32(pa + 0x10);
    fx.mem.write_u32(sp + 0xDC, w);
    fx.mem.set32(sp + 0x100, n - 1);
    let mut seg = 0;
    fx.mem.set32(sp + 0xE0, seg);
    let v = sp + 0x190;
    while seg < n - 1 {
        let mut f30 = 0.0f32;
        fx.mem.set32(sp + 0xF0, 0);
        let p = (a as i32 + 0x10 * (seg as i16 as i32)) as u32;
        let pts = if seg == 0 { [pa, pa + 0x10, pa + 0x20, pa + 0x30] } else { [p - 0x10, p, p + 0x10, p + 0x20] };
        for sub_ in 0..2 {
            if seg == 0 && sub_ == 0 {
                curve(fx, sp + 0x250, pts, f30);
                copy4(fx, sp + 0x120, sp + 0x250);
            } else {
                copy4(fx, sp + 0x120, sp + 0x180);
            }
            f30 = add(f30, 0.5);
            if !(f30 < f32::from_bits(0x3F66_6666)) {
                f30 = 1.0;
            }
            let e = if seg == 0 { 0x260 } else { 0x240 };
            curve(fx, sp + e, pts, f30);
            copy4(fx, sp + 0x130, sp + e);
            copy4(fx, sp + 0x180, sp + 0x130);
            l2::colour_strip3(fx, d, colour, 1);
            l2::colour_strip3(fx, d, colour, 2);
            for s0 in 0..2 {
                l2::copy3(fx, v, sp + 0x120);
                l2::copy3(fx, v + 0x10, sp + 0x120);
                l2::copy3(fx, v + 0x20, sp + 0x130);
                l2::copy3(fx, v + 0x30, sp + 0x130);
                let bump = |fx: &mut Fx, at: u32, by: f32, up: bool| {
                    let x = fx.mem.f(v + at);
                    fx.mem.setf(v + at, if up { add(x, by) } else { sub(x, by) });
                };
                if s0 == 0 {
                    bump(fx, 0x14, f31, true);
                    bump(fx, 0x34, f31, true);
                    bump(fx, 0x4, f31, false);
                    bump(fx, 0x24, f31, false);
                } else {
                    let h = div(f31, 2.0);
                    bump(fx, 0x10, f31, true);
                    bump(fx, 0x30, f31, true);
                    bump(fx, 0x18, h, true);
                    bump(fx, 0x38, h, true);
                    bump(fx, 0x0, f31, false);
                    bump(fx, 0x20, f31, false);
                    bump(fx, 0x8, h, false);
                    bump(fx, 0x28, h, false);
                }
                if (0..=2).contains(&mode) && l2::project(fx, v, SPR, sp + 0x1F0, d, sp + 0x1E0) {
                    match mode {
                        0 => {
                            let key = ee::fixunssfsi(fx.mem.f(sp + 0x1E0)) as i32;
                            fx.submit(d, OT, key);
                        }
                        1 => {
                            l2::far_z_desc(fx, d, 0);
                            fx.submit(d, OT, 0);
                        }
                        _ => fx.submit(d, OT, -1),
                    }
                }
            }
            fx.mem.set32(sp + 0xF0, sub_ + 1);
        }
        seg += 1;
        fx.mem.set32(sp + 0xE0, seg);
    }
    fx.leave(0x340);
}

/// How a compile of the gouraud tube cuts its quads.
pub struct GouraudLayout {
    /// Fins a piece (quads from the centre line pushed out by the width:
    /// up, down, then (4) left, right).
    pub fins: i32,
    /// The descriptor's ALPHA (0x207E20).
    pub alpha: u64,
}

/// 9769823e2004 [card 90 0x1E044B0, 660 words]: four fins.
pub const GOURAUD4: GouraudLayout = GouraudLayout { fins: 4, alpha: 0x80_0000_0048 };

/// 3cde88827d7d [card 93 0x1E02B30, 660 words]: the up /
/// down fins only (the loop on 2; every other word the same).
pub const GOURAUD2: GouraudLayout = GouraudLayout { fins: 2, alpha: 0x80_0000_0048 };

/// 0efc7dca7954 [card 300 0x1E03750, 652 words]: [`GOURAUD2`]
/// with ALPHA 0x44 (the one word that differs).
pub const GOURAUD2_44: GouraudLayout = GouraudLayout { fins: 2, alpha: 0x80_0000_0044 };

/// `tube_gouraud(D, task, trail, mode)`: the trail tube
/// compiled untextured: an additive gouraud strip4 (0x109BC0 (d, 0) /
/// 0x109DD0) coloured +0x60 / +0x64 / +0x60 / +0x64 (0x108020), ALPHA
/// `l.alpha` (0x48; 0x44 for [`GOURAUD2_44`]); each of the n - 1 segments in two pieces (t += 0.5, clamped to 1
/// from 0.9; the Lagrange curve inline, the first segment's point before
/// the first point), each piece `fins` quads with two corners on the
/// piece's start / end and two pushed out by the half width (size +0x48
/// / 10, down by size / 80n a piece); turned by the task's angles +0x20
/// at D+0x20 (0x1097D0 into sp + 0x1E0); depth a3 (0 the mean w, 1 the
/// far Z at key 0, 2 key -1, else nothing projected). A 0x320 frame, the
/// descriptor at sp + 0x270 [F].
pub fn tube_gouraud(fx: &mut Fx, l: &GouraudLayout, d0: u32, t: u32, a: u32, mode: i32) {
    use ee::{add, div, sub};
    let sp = fx.enter(0x320);
    let mut f30 = div(fx.mem.f(t + 0x48), 10.0);
    fx.mem.write_u32(sp + 0x100, a);
    let n = fx.mem.i16(t + 6);
    let d = sp + 0x270;
    fx.mem.setf(sp + 0xD0, f30);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    for (local, c, to) in [(0x300u32, 0x60u32, 0x10u32), (0x308, 0x64, 0x20), (0x310, 0x60, 0x30), (0x318, 0x64, 0x40)] {
        crate::c061::rgbaq_at(fx, sp + local, t + c, d + to);
    }
    fx.desc_alpha(d, l.alpha);
    copy4(fx, sp + 0x260, a);
    copy4(fx, sp + 0x130, sp + 0x260);
    copy4(fx, sp + 0x140, sp + 0x260);
    copy4(fx, sp + 0x150, a + 0x10);
    copy4(fx, sp + 0x160, a + 0x20);
    for k in 0..3 {
        let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0x1C0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x1E0, 0, t + 0x20, sp + 0x1C0);
    let w = fx.mem.read_u32(sp + 0x140);
    fx.mem.write_u32(sp + 0xD4, w);
    let w = fx.mem.read_u32(sp + 0x150);
    fx.mem.write_u32(sp + 0xD8, w);
    fx.mem.set32(sp + 0xE0, 0);
    fx.mem.set32(sp + 0xF0, n - 1);
    fx.mem.setf(sp + 0xDC, ee::mul(8.0, ee::from_i32(n)));
    let first = [sp + 0x130, sp + 0x140, sp + 0x150, sp + 0x160];
    let v = sp + 0x180;
    let mut seg = 0i32;
    while seg < n - 1 {
        let mut f29 = 0.0f32;
        let p = (a as i32 + 0x10 * (seg as i16 as i32)) as u32;
        let pts = if seg == 0 { first } else { [p - 0x10, p, p + 0x10, p + 0x20] };
        for piece in 0..2 {
            if seg == 0 && piece == 0 {
                curve(fx, sp + 0x240, pts, f29);
                copy4(fx, sp + 0x110, sp + 0x240);
            } else {
                copy4(fx, sp + 0x110, sp + 0x170);
            }
            f29 = add(f29, 0.5);
            if !(f29 < f32::from_bits(0x3F66_6666)) {
                f29 = 1.0;
            }
            let e = if seg == 0 { 0x250 } else { 0x230 };
            curve(fx, sp + e, pts, f29);
            copy4(fx, sp + 0x120, sp + e);
            copy4(fx, sp + 0x170, sp + 0x120);
            let f31 = f30;
            f30 = sub(f30, div(fx.mem.f(sp + 0xD0), fx.mem.f(sp + 0xDC)));
            for fin in 0..l.fins {
                l2::copy3(fx, v, sp + 0x110);
                l2::copy3(fx, v + 0x10, sp + 0x110);
                l2::copy3(fx, v + 0x20, sp + 0x120);
                l2::copy3(fx, v + 0x30, sp + 0x120);
                let bump = |fx: &mut Fx, at: u32, by: f32, up: bool| {
                    let x = fx.mem.f(v + at);
                    fx.mem.setf(v + at, if up { add(x, by) } else { sub(x, by) });
                };
                match fin {
                    0 => {
                        bump(fx, 0x14, f31, true);
                        bump(fx, 0x34, f30, true);
                    }
                    1 => {
                        bump(fx, 0x14, f31, false);
                        bump(fx, 0x34, f30, false);
                    }
                    2 => {
                        bump(fx, 0x10, f31, false);
                        bump(fx, 0x30, f30, false);
                    }
                    _ => {
                        bump(fx, 0x10, f31, true);
                        bump(fx, 0x30, f30, true);
                    }
                }
                if (0..=2).contains(&mode) && l2::project(fx, v, SPR, sp + 0x1E0, d, sp + 0x1D0) {
                    match mode {
                        0 => {
                            let key = ee::fixunssfsi(fx.mem.f(sp + 0x1D0)) as i32;
                            fx.submit(d, OT, key);
                        }
                        1 => {
                            l2::far_z_desc(fx, d, 1);
                            fx.submit(d, OT, 0);
                        }
                        _ => fx.submit(d, OT, -1),
                    }
                }
            }
        }
        seg += 1;
        fx.mem.set32(sp + 0xE0, seg);
    }
    fx.leave(0x320);
}
