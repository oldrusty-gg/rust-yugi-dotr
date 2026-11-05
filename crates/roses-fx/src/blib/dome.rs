//! Domes of the effect library: a half-circle
//! profile swept round the y axis, drawn as textured quads.
use super::exe::{camera_vp_into, countdown, libm_cosf, mul_mem, node_source, rot_trans_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx};

const PI: u32 = 0x4049_0FDB;

/// Where a compile of the dome keeps its locals: the frame,
/// how much lower than b370's every 0x150.. offset sits (`low`), and its
/// scalars: the record counter, the texture, 4k, the turn, the row count;
/// `cols` b370's column / angle / column-count slots (None: in registers,
/// the angle col x turn by `mult`), `last` b535's slot for the row's
/// previous-column pointer.
pub struct Layout {
    pub frame: u32,
    pub low: u32,
    pub i: u32,
    pub tex: u32,
    pub k4: u32,
    pub turn: u32,
    pub rows: u32,
    pub cols: Option<[u32; 3]>,
    pub last: Option<u32>,
}

/// b370's (c5cb24dcc4c4) 0x580 frame.
pub const B370: Layout = Layout { frame: 0x580, low: 0, i: 0xC0, tex: 0xD0, k4: 0x120, turn: 0xF0, rows: 0x130, cols: Some([0xE0, 0x100, 0x110]), last: None };
/// b535 / b462's (0c07e8f08fe7) 0x550 frame.
pub const B535: Layout = Layout { frame: 0x550, low: 0x30, i: 0xB0, tex: 0xC0, k4: 0xE0, turn: 0xD0, rows: 0xF0, cols: None, last: Some(0x100) };

/// c5cb24dcc4c4 [8 B records; b370 (card 534) 0x1E04F70, 656 words]
/// [F]: `domes(at, recs, ot, n) -> 1` steps and draws `n`
/// records (0x50 bytes; a 0x580 frame, the descriptor at sp + 0x4E0:
/// RGBAQ / ST / XYZF2 x 4). Per record: countdown 0x101AF0; the texture
/// (5, +8); radius +0x14 += +0x18; i32 angles +0x30.. += +0x40..; k =
/// min(+7, 9); the position +0x20 from a node with +5 (0x101BA0); the
/// camera (0x20E2B0) x the record's rotation / translation (0x20C0B0,
/// 0x20B370) at sp + 0x190. The profile: 2k + 1 points at sp + 0x210 (the
/// middle (r, 0, 0), then r (cos, +-sin) of 90 / k steps, libm), their w
/// the stale words there. 4k + 1 columns turned about y by 0x10000 / 4k
/// (0x20BDA0; the last back at 0), each profile point (rows 0..2k, flag
/// 4: 0..k) turned (0x20C540) and projected (0x204D80); a quad between
/// this and the last column, rows j - 1 / j, when all four projected
/// (flag 1: back faces culled by the screen cross). S = +0xC / 4k x the
/// column, T = +0x10 x elapsed + +0xC / 2k x the row, both / w (flag 8:
/// mirrored (1 - v w) / w by count & 3: 3 S, 1 T, 2 both); REGION_CLAMP 0,
/// ALPHA 0x48, colour +0x1C.. with alpha +0x1F (x count / 8 over the last
/// 8 unless +4 bit 0x40; 0 on odd counts with flag 2; flag 4 halves row
/// k - 1's near and row k's far alpha, the other pair 0), the texture
/// (filter 1); keyed by the mean w (0 with +4 bit 0x80) through 0x1A2C70
/// twice. The flags are +0xA.
pub fn domes(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    domes_a3(fx, at, recs, ot, n).0
}

/// [`domes`] and the a3 it leaves (helper B18, for a later boundary call
/// that reports it) [F b758 0x1E03FC8 / 0x1E04230 / 0x1E04298 / 0x1E042E0;
/// run b758]: per counting record 0x20C0B0's translation pointer +0x20,
/// then per row the projection's sp + 0x548; a back-face test (flag 1)
/// leaves the halfword it loads (u16 sp + 0x502 on row 1, else sp +
/// 0x532), a drawn quad 0x207E50's (the TEX0 low word RH2 + 0x30 with a
/// texture, else desc + 0x10). `None` when no record counted.
pub fn domes_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    domes_at(fx, at, recs, ot, n, &B370)
}

/// 0c07e8f08fe7 [2 B records; b535 (card 457) 0x1E01500 / b462 (card 537)
/// 0x1E042D0, 676 words] [F]: [`domes`] compiled in a 0x550
/// frame (every 0x150.. local 0x30 lower, the descriptor at sp + 0x4B0;
/// the record counter sp + 0xB0, the texture sp + 0xC0, 4k sp + 0xE0, the
/// turn sp + 0xD0, the rows sp + 0xF0; column and angle in registers, the
/// angle col x turn; the row's previous-column pointer saved at sp +
/// 0x100). Its a3 is not modelled (b370's rule is [`domes_a3`]'s).
pub fn domes_550(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    domes_at(fx, at, recs, ot, n, &B535).0
}

/// [`domes_a3`] in the layout `l`.
pub fn domes_at(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, l: &Layout) -> (i32, Option<u32>) {
    let mut a3 = None;
    let sp = fx.enter(l.frame);
    let b = sp - l.low;
    fx.mem.set32(b + 0x144, n);
    fx.mem.write_u32(b + 0x148, ot);
    let mut g = recs;
    let mut i = 0;
    fx.mem.set32(sp + l.i, i);
    while i < fx.mem.i32(b + 0x144) {
        if i != 0 {
            g += 0x50;
        }
        dome(fx, at, sp, g, l, &mut a3);
        i = fx.mem.i32(sp + l.i) + 1;
        fx.mem.set32(sp + l.i, i);
    }
    fx.leave(l.frame);
    (1, a3)
}

fn dome(fx: &mut Fx, at: u32, sp: u32, g: u32, l: &Layout, a3: &mut Option<u32>) {
    let b = sp - l.low;
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 8);
    let t = fx.texture(5, id);
    fx.mem.write_u32(sp + l.tex, t);
    let r = ee::add(fx.mem.f(g + 0x14), fx.mem.f(g + 0x18));
    fx.mem.setf(g + 0x14, r);
    for c in 0..3u32 {
        let v = fx.mem.i32(g + 0x30 + 4 * c).wrapping_add(fx.mem.i32(g + 0x40 + 4 * c));
        fx.mem.set32(g + 0x30 + 4 * c, v);
    }
    let mut k = fx.mem.u8(g + 7);
    if k >= 0xA {
        k = 9;
    }
    let step = ee::div(90.0, ee::from_i32(k));
    node_source(fx, g, g + 0x20);
    camera_vp_into(fx, b + 0x190);
    rot_trans_into(fx, b + 0x150, g + 0x30, g + 0x20);
    *a3 = Some(g + 0x20);
    mul_mem(fx, b + 0x190, b + 0x150);
    let prof = b + 0x210;
    for s1 in 1..=k {
        let a = ee::div(ee::mul(f32::from_bits(PI), ee::mul(step, ee::from_i32(s1))), 180.0);
        let c = libm_cosf(fx, a);
        let lo = prof + 16 * (k - s1) as u32;
        let hi = prof + 16 * (k + s1) as u32;
        let x = ee::mul(fx.mem.f(g + 0x14), c);
        fx.mem.setf(lo, x);
        fx.mem.setf(hi, x);
        let s = fx.libm_sinf(a);
        let y = ee::mul(fx.mem.f(g + 0x14), s);
        fx.mem.setf(hi + 4, y);
        fx.mem.setf(lo + 4, ee::neg(y));
        fx.mem.write_u32(lo + 8, 0);
        fx.mem.write_u32(hi + 8, 0);
    }
    let mid = prof + 16 * k as u32;
    let r = fx.mem.read_u32(g + 0x14);
    fx.mem.write_u32(mid, r);
    fx.mem.write_u32(mid + 8, 0);
    fx.mem.write_u32(mid + 4, 0);
    fx.mem.set32(sp + l.k4, 4 * k);
    fx.mem.write_u32(b + 0x4D8, 0);
    fx.mem.write_u32(b + 0x4D0, 0);
    let (turn, _) = fx.div(0x1_0000, 4 * k);
    fx.mem.set32(sp + l.turn, turn);
    let e = fx.mem.u8(g + 2) - fx.mem.u8(g);
    let su = ee::div(fx.mem.f(g + 0xC), ee::from_i32(4 * k));
    fx.mem.set32(sp + l.rows, 2 * k);
    let t0 = ee::mul(fx.mem.f(g + 0x10), ee::from_i32(e));
    let tu = ee::div(fx.mem.f(g + 0xC), ee::from_i32(2 * k));
    if fx.mem.u8(g + 0xA) & 4 != 0 {
        fx.mem.set32(sp + l.rows, k);
    }
    let Some([c_at, a_at, n_at]) = l.cols else {
        // b535: column / angle in registers, the angle col x turn.
        let mut col = 0;
        while col < fx.mem.i32(sp + l.k4) + 1 {
            let a = if col == fx.mem.i32(sp + l.k4) { 0 } else { {
                let t = fx.mem.i32(sp + l.turn);
                fx.mult(col, t)
            } };
            fx.mem.set32(b + 0x4D4, a);
            let rot = [fx.mem.i32(b + 0x4D0), fx.mem.i32(b + 0x4D4), fx.mem.i32(b + 0x4D8)];
            let m = fx.rotation_matrix(rot);
            fx.write_m4(b + 0x490, &m);
            column(fx, at, sp, g, k, col, su, t0, tu, l, a3);
            col += 1;
        }
        return;
    };
    fx.mem.set32(sp + c_at, 0);
    fx.mem.set32(sp + a_at, 0);
    fx.mem.set32(sp + n_at, 4 * k + 1);
    let mut col = 0;
    while col < fx.mem.i32(sp + n_at) {
        let a = if col == fx.mem.i32(sp + l.k4) { 0 } else { fx.mem.i32(sp + a_at) };
        fx.mem.set32(b + 0x4D4, a);
        let rot = [fx.mem.i32(b + 0x4D0), fx.mem.i32(b + 0x4D4), fx.mem.i32(b + 0x4D8)];
        let m = fx.rotation_matrix(rot);
        fx.write_m4(b + 0x490, &m);
        column(fx, at, sp, g, k, col, su, t0, tu, l, a3);
        let a = fx.mem.i32(sp + a_at).wrapping_add(fx.mem.i32(sp + l.turn));
        fx.mem.set32(sp + a_at, a);
        col = fx.mem.i32(sp + c_at) + 1;
        fx.mem.set32(sp + c_at, col);
    }
}

/// Column `col`: rows 0..=sp[0x130], quads back to the last column (its
/// XYZ at sp + 0x350, projected flags sp + 0x3F0, w sp + 0x440 per row).
#[allow(clippy::too_many_arguments)]
fn column(fx: &mut Fx, at: u32, sp: u32, g: u32, k: i32, col: i32, su: f32, t0: f32, tu: f32, l: &Layout, a3: &mut Option<u32>) {
    let b = sp - l.low;
    let rows = fx.mem.i32(sp + l.rows) + 1;
    let s_here = ee::mul(su, ee::from_i32(col));
    let s_last = ee::mul(su, ee::from_i32(col - 1));
    let rot = fx.read_m4(b + 0x490);
    let d = b + 0x4E0;
    let (mut fp, mut f23, mut s2) = (0u32, 0.0f32, 0u32);
    let mut j = 0i32;
    while j < rows {
        let (s4, s5, s6) = (b + 0x350 + 8 * j as u32, b + 0x3F0 + 4 * j as u32, b + 0x440 + 4 * j as u32);
        fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
        let v = fx.read_v4(b + 0x210 + 16 * j as u32);
        let o = fx.transform(Some(&rot), v);
        fx.write_v4(b + 0x1D0, o);
        let m = fx.read_m4(b + 0x190);
        s2 = u32::from(fx.project_at(b + 0x1D0, &m, b + 0x500, b + 0x57C));
        *a3 = Some(b + 0x548);
        let v = fx.mem.read_u64(s4);
        fx.mem.write_u64(b + 0x518, v);
        let v = fx.mem.read_u64(b + 0x570);
        fx.mem.write_u64(b + 0x530, v);
        if let Some(o) = l.last {
            fx.mem.write_u32(sp + o, s4.wrapping_sub(8));
        }
        let v = fx.mem.read_u64(s4.wrapping_sub(8));
        fx.mem.write_u64(b + 0x548, v);
        if col != 0 && j != 0 && s2 != 0 && fp != 0 && fx.mem.read_u32(s5) != 0 && fx.mem.read_u32(s5 - 4) != 0 {
            quad(fx, at, sp, g, k, j, s_here, s_last, t0, tu, f23, s6, l, a3);
        }
        if j != 0 {
            let v = fx.mem.read_u64(b + 0x570);
            fx.mem.write_u64(s4 - 8, v);
            fx.mem.setf(s6 - 4, f23);
            fx.mem.write_u32(s5 - 4, fp);
        }
        let v = fx.mem.read_u64(b + 0x500);
        fx.mem.write_u64(b + 0x570, v);
        f23 = fx.mem.f(b + 0x57C);
        fp = s2;
        j += 1;
    }
    let v = fx.mem.read_u64(b + 0x500);
    fx.mem.write_u64(b + 0x348 + 8 * j as u32, v);
    fx.mem.write_u32(b + 0x3EC + 4 * j as u32, s2);
    let w = fx.mem.read_u32(b + 0x57C);
    fx.mem.write_u32(b + 0x43C + 4 * j as u32, w);
}

/// (1 - v w) / w: a coordinate mirrored.
fn mirror(fx: &mut Fx, a: u32, w: f32) {
    let v = ee::div(ee::sub(1.0, ee::mul(fx.mem.f(a), w)), w);
    fx.mem.setf(a, v);
}

#[allow(clippy::too_many_arguments)]
fn quad(fx: &mut Fx, at: u32, sp: u32, g: u32, k: i32, j: i32, s_here: f32, s_last: f32, t0: f32, tu: f32, w2: f32, s6: u32, l: &Layout, a3: &mut Option<u32>) {
    let tex = fx.mem.read_u32(sp + l.tex);
    let sp = sp - l.low;
    let flags = fx.mem.u8(g + 0xA);
    if flags & 1 != 0 {
        let u = |fx: &mut Fx, o: u32| fx.mem.u16(sp + o);
        *a3 = Some(fx.mem.u16(sp + if j == 1 { 0x502 } else { 0x532 }) as u32);
        let (a, b) = if j == 1 {
            let (x0, y0) = (u(fx, 0x500), u(fx, 0x502));
            let (dy, dx) = (u(fx, 0x532) - y0, u(fx, 0x518) - x0);
            let p = fx.mult(dy, dx);
            (p, (u(fx, 0x51A) - y0).wrapping_mul(u(fx, 0x530) - x0))
        } else {
            let (x2, y2) = (u(fx, 0x530), u(fx, 0x532));
            let (dy, dx) = (u(fx, 0x54A) - y2, u(fx, 0x500) - x2);
            let p = fx.mult(dy, dx);
            (p, (u(fx, 0x502) - y2).wrapping_mul(u(fx, 0x548) - x2))
        };
        if !ee_le(ee::from_i32(a.wrapping_sub(b)), 0.0) {
            return;
        }
    }
    let w0 = fx.mem.f(sp + 0x57C);
    let w1 = fx.mem.f(s6);
    let w3 = fx.mem.f(s6 - 4);
    let t_here = ee::add(t0, ee::mul(tu, ee::from_i32(j)));
    let t_last = ee::add(t0, ee::mul(tu, ee::from_i32(j - 1)));
    for (o, v, w) in [(0x4F4u32, 1.0f32, w0), (0x4F8, s_here, w0), (0x4FC, t_here, w0), (0x50C, 1.0, w1), (0x510, s_last, w1), (0x514, t_here, w1), (0x524, 1.0, w2), (0x528, s_here, w2), (0x52C, t_last, w2), (0x53C, 1.0, w3), (0x540, s_last, w3), (0x544, t_last, w3)] {
        fx.mem.setf(sp + o, ee::div(v, w));
    }
    if flags & 8 != 0 {
        let ws = [w0, w1, w2, w3];
        let m = fx.mem.u8(g) & 3;
        if m == 3 || m == 2 {
            for (v, o) in [0x4F8u32, 0x510, 0x528, 0x540].into_iter().enumerate() {
                mirror(fx, sp + o, ws[v]);
            }
        }
        if m == 1 || m == 2 {
            for (v, o) in [0x4FCu32, 0x514, 0x52C, 0x544].into_iter().enumerate() {
                mirror(fx, sp + o, ws[v]);
            }
        }
    }
    let d = sp + 0x4E0;
    fx.desc_clamp(d, 0);
    fx.desc_alpha(d, 0x48);
    let count = fx.mem.u8(g);
    let alpha = if fx.mem.u8(g + 4) & 0x40 == 0 && count < 8 {
        let a = fx.mem.u8(g + 0x1F) >> 3;
        fx.mult(a, count) & 0xFF
    } else {
        fx.mem.u8(g + 0x1F)
    };
    let a = if fx.mem.u8(g + 0xA) & 2 != 0 && fx.mem.u8(g) & 1 != 0 { 0 } else { alpha };
    for o in [0x53Bu32, 0x523, 0x50B, 0x4F3] {
        fx.mem.set8(sp + o, a);
    }
    if fx.mem.u8(g + 0xA) & 4 != 0 {
        let half = (alpha & 0xFF) >> 1;
        if j == k - 1 {
            fx.mem.set8(sp + 0x50B, half);
            fx.mem.set8(sp + 0x4F3, half);
        }
        if j == k {
            fx.mem.write_u8(sp + 0x50B, 0);
            fx.mem.write_u8(sp + 0x4F3, 0);
            fx.mem.set8(sp + 0x53B, half);
            fx.mem.set8(sp + 0x523, half);
        }
    }
    for c in 0..3u32 {
        let v = fx.mem.read_u8(g + 0x1E - c);
        for o in [0x538u32, 0x520, 0x508, 0x4F0] {
            fx.mem.write_u8(sp + o + c, v);
        }
    }
    for o in [0x54Fu32, 0x537, 0x51F, 0x507] {
        fx.mem.write_u8(sp + o, 0);
    }
    let t = tex;
    *a3 = Some(if fx.mem.read_u16(d + 2) & 0x20 != 0 && t != 0 { fx.mem.read_u32(t + 0x30) } else { d + 0x10 });
    fx.desc_texture(d, t, 1);
    let f = if fx.mem.u8(g + 4) & 0x80 != 0 {
        0.0
    } else {
        let w = ee::div(ee::add(w3, ee::add(w1, ee::add(w0, w2))), 4.0);
        fx.fix_residue(w, at + 0x8AC);
        let k = ee::fixunssfsi(w);
        if (k as i32) < 0 {
            let h = ee::from_i32(((k >> 1) | (k & 1)) as i32);
            ee::add(h, h)
        } else {
            ee::from_i32(k as i32)
        }
    };
    fx.fix_residue(f, at + 0x8E4);
    let key = ee::fixunssfsi(f) as i32;
    let ot = fx.mem.read_u32(sp + 0x148);
    fx.submit(d, ot, key);
}
