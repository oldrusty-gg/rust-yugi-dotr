//! Cones of the effect library: a textured band
//! between two horizontal rings of different heights (a lampshade), drawn
//! quadrant by quadrant.
use super::exe::{alpha_mode, camera_vp_into, countdown, libm_cosf, mul_mem, node_source, rot_trans_into, tex_rect_strip};
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

const PI: u32 = 0x4049_0FDB;
/// RGBAQ / ST / XYZF2 x 4 (a textured strip).
const REGS_TEX4: u64 = 0x4214_2142_1421;

/// 5b100f042ffd [5 B records; b032 0x1E02870, 540 words] [F]:
/// `cones(at, recs, ot, n) -> 1` steps and draws `n` records (0x48 bytes;
/// a 0x410 frame, the descriptor at sp + 0x378). The function's own table
/// of four 0x18-byte sprite specs (record-relative `lui` / `addiu` at
/// +0x4C / +0x50) is copied to sp + 0x300; spec +8 of a record picks one
/// (its texture 0x202530(spec +0, spec +4)). Per record: countdown
/// 0x101AF0; height +0x18 += +0x1C; the position +0x28 from a node with +5
/// (0x101BA0); through the camera x (angles +0x38, position +0x28)
/// (0x20E2B0, 0x20C0B0, 0x20B370); with k = min(+7, 9) the k + 1 points of
/// a quarter turn (libm sinf / cosf) of two rings: radius +0xC (+0x18 +
/// +0x14) / +0x10 at height +0x18 + +0x14 and +0xC +0x18 / +0x10 at +0x18
/// (sp + 0x170 / + 0x210); colours +0x22.. / +0x26.., alphas +0x23 / +0x27
/// x count / 8 over the last 8. Each step draws four quads (the quadrants:
/// x, -x, then z negated too, then x back), skipped when a vertex fails to
/// project (0x204D80); specs 1..3 STQ 0..1 over (0, +0xA) x (v0, v1) with
/// a REGION_REPEAT clamp from +9 (0x208400), ALPHA by 0x101C80 / 0x207E20
/// and the texture (0x207E50, filter 1); another spec 0x101EE0's UVs;
/// keyed by the mean w (0x1A2C70).
pub fn cones(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    cones_a3(fx, at, recs, ot, n).0
}

/// Where a compilation of [`cones`] keeps its frame, table and `jal
/// 0x1A2C70`.
pub struct ConeLayout {
    /// The frame size (the locals are caller-relative the same in both).
    pub frame: u32,
    /// The spec table's `lui` / `addiu` offsets in the function.
    pub table: (u32, u32),
    /// The 0x1A2C70 return address's offset in the function.
    pub fix: u32,
    /// The counters i / k / j (record, steps, ring step) stored at these
    /// frame offsets (`None`: in registers).
    pub counters: Option<(u32, u32, Option<u32>)>,
}

/// 5b100f042ffd's layout (b106 0x1E02F30).
pub const CONES: ConeLayout = ConeLayout { frame: 0x410, table: (0x4C, 0x50), fix: 0x7B8, counters: Some((0xC0, 0xD0, None)) };

/// 818ce78a7471 [8 B records; b199 0x1E00440, 628 words]
/// [F]: [`cones`] compiled with a 0x3C0 frame (no fp / f24 saves), every
/// local 0x50 lower in the frame (so at the same caller-relative
/// address), the counters i / k and the record pointer in registers, the
/// spec table at +0x44 / +0x48 and the 0x1A2C70 return at +0x930; the
/// cosf angle recomputed (the same value).
pub const CONES_B: ConeLayout = ConeLayout { frame: 0x3C0, table: (0x44, 0x48), fix: 0x930, counters: None };

/// 2d95e82e7206 [2 B records; b131 0x1E05DE0, 516 words]
/// [F]: [`cones`] compiled with the same 0x410 frame and locals (the ra at
/// sp + 0xA0, no f24 save) but the counters i / k / j at sp + 0xB0 / 0xC0
/// / 0xD0 (j, the ring step, zeroed with the colours and counted up to k),
/// the spec table at +0x34 / +0x3C and the 0x1A2C70 return at +0x758; the
/// quadrant counter in a register.
pub const CONES_C: ConeLayout = ConeLayout { frame: 0x410, table: (0x34, 0x3C), fix: 0x758, counters: Some((0xB0, 0xC0, Some(0xD0))) };

/// The 2d95e82e7206 compilation of [`cones`] ([`CONES_C`]).
pub fn cones_c(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    cones_with(fx, at, recs, ot, n, &CONES_C).0
}

/// The 818ce78a7471 compilation of [`cones`] ([`CONES_B`]).
pub fn cones_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    cones_with(fx, at, recs, ot, n, &CONES_B).0
}

/// [`cones`] and the a3 it leaves (helper B9, for a later boundary call
/// that reports it) [F; run in b106]: the table's address, else after a
/// drawn record's last quad the descriptor flags 0x207E20 loaded (spec
/// kinds 0 / 4..: 0x101EE0's last call) or 0x207E50's (TEX0's low half,
/// else the descriptor + 0x10), or after a failed projection the camera
/// table pointer 0x20E2B0 leaves (0x2ED760 + 0x1A0 x the camera index).
/// Not modelled (left as the last value): a record with no steps (+7 =
/// 0), which runs only the setup calls.
pub fn cones_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, u32) {
    cones_with(fx, at, recs, ot, n, &CONES)
}

/// [`cones_a3`] compiled with layout `l`; `sp` below is the 0x410-frame
/// base (the locals' caller-relative addresses).
pub fn cones_with(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, l: &ConeLayout) -> (i32, u32) {
    let table = rel(fx, at, l.table.0, l.table.1);
    let mut a3 = table;
    let sp = fx.enter(l.frame) + l.frame - 0x410;
    for k in (0..0x60u32).step_by(8) {
        let v = fx.mem.read_u64((table & !15) + k);
        fx.mem.write_u64(sp + 0x300 + k, v);
    }
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x48;
        }
        if let Some((ci, _, _)) = l.counters {
            fx.mem.set32(sp + ci, i);
        }
        if let Some(v) = one(fx, at, sp, g, ot, l) {
            a3 = v;
        }
        i += 1;
    }
    if let Some((ci, _, _)) = l.counters {
        fx.mem.set32(sp + ci, i);
    }
    fx.leave(l.frame);
    (1, a3)
}

fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, l: &ConeLayout) -> Option<u32> {
    if countdown(fx, g) == 0 {
        return None;
    }
    let mut a3 = None;
    let e = fx.mem.read_u8(g + 8) as u32;
    let spec = sp + 0x300 + 0x18 * e;
    let (kind, id) = (fx.mem.i32(spec), fx.mem.u8(spec + 4));
    let tex = fx.texture(kind, id);
    let v = ee::add(fx.mem.f(g + 0x18), fx.mem.f(g + 0x1C));
    fx.mem.setf(g + 0x18, v);
    node_source(fx, g, g + 0x28);
    camera_vp_into(fx, sp + 0x130);
    rot_trans_into(fx, sp + 0xF0, g + 0x38, g + 0x28);
    mul_mem(fx, sp + 0x130, sp + 0xF0);
    let mut k = fx.mem.u8(g + 7);
    if k >= 10 {
        k = 9;
    }
    if let Some((_, ck, _)) = l.counters {
        fx.mem.set32(sp + ck, k);
    }
    let step = ee::div(90.0, ee::from_i32(k));
    let (h, rise, a, b) = (fx.mem.f(g + 0x18), fx.mem.f(g + 0x14), fx.mem.f(g + 0xC), fx.mem.f(g + 0x10));
    let top = ee::add(h, rise);
    let r0 = ee::div(ee::mul(a, top), b);
    let r1 = ee::div(ee::mul(a, h), b);
    for j in 0..=k {
        let ang = ee::div(ee::mul(f32::from_bits(PI), ee::mul(step, ee::from_i32(j))), 180.0);
        let s = fx.libm_sinf(ang);
        let c = libm_cosf(fx, ang);
        let p = sp + 16 * j as u32;
        fx.mem.setf(p + 0x170, ee::mul(r0, c));
        fx.mem.setf(p + 0x178, ee::mul(r0, s));
        let y = ee::add(fx.mem.f(g + 0x18), fx.mem.f(g + 0x14));
        fx.mem.setf(p + 0x174, y);
        fx.mem.setf(p + 0x210, ee::mul(r1, c));
        fx.mem.setf(p + 0x218, ee::mul(r1, s));
        let y = fx.mem.read_u32(g + 0x18);
        fx.mem.write_u32(p + 0x214, y);
    }
    let d = sp + 0x378;
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    for (src, o) in [(0x22u32, 0u32), (0x21, 1), (0x20, 2)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + 0x3A0 + o, v);
        fx.mem.write_u8(sp + 0x388 + o, v);
        let v = fx.mem.read_u8(g + src + 4);
        fx.mem.write_u8(sp + 0x3D0 + o, v);
        fx.mem.write_u8(sp + 0x3B8 + o, v);
    }
    for o in [0x3D4u32, 0x3BC, 0x3A4, 0x38C] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let c = fx.mem.u8(g);
    let (inner, outer) = if c < 8 {
        let a = fx.mem.u8(g + 0x23) >> 3;
        let inner = fx.mult(c, a);
        (inner, (fx.mem.u8(g + 0x27) >> 3).wrapping_mul(fx.mem.u8(g)))
    } else {
        (fx.mem.u8(g + 0x23), fx.mem.u8(g + 0x27))
    };
    fx.mem.set8(sp + 0x3A3, inner);
    fx.mem.set8(sp + 0x38B, inner);
    fx.mem.set8(sp + 0x3D3, outer);
    fx.mem.set8(sp + 0x3BB, outer);
    let y = fx.mem.read_u32(sp + 0x174);
    fx.mem.write_u32(sp + 0x2C4, y);
    fx.mem.write_u32(sp + 0x2B4, y);
    let y = fx.mem.read_u32(sp + 0x214);
    fx.mem.write_u32(sp + 0x2E4, y);
    fx.mem.write_u32(sp + 0x2D4, y);
    let cj = l.counters.and_then(|c| c.2);
    if let Some(cj) = cj {
        fx.mem.set32(sp + cj, 0);
    }
    for j in 0..k as u32 {
        if let Some(cj) = cj {
            fx.mem.write_u32(sp + cj, j);
        }
        let p = sp + 16 * j;
        // (vertex input, ring point) for x (+0) and z (+8).
        let pairs = [(0x2B0u32, p + 0x170), (0x2C0, p + 0x180), (0x2D0, p + 0x210), (0x2E0, p + 0x220)];
        for q in 0..4 {
            for &(v, src) in &pairs {
                match q {
                    0 => {
                        let x = fx.mem.read_u32(src);
                        fx.mem.write_u32(sp + v, x);
                    }
                    1 => {
                        let x = ee::neg(fx.mem.f(src));
                        fx.mem.setf(sp + v, x);
                    }
                    2 => {
                        let z = ee::neg(fx.mem.f(src + 8));
                        fx.mem.setf(sp + v + 8, z);
                    }
                    _ => {
                        let x = fx.mem.read_u32(src);
                        fx.mem.write_u32(sp + v, x);
                    }
                }
            }
            if q == 0 {
                for &(v, src) in &pairs {
                    let z = fx.mem.read_u32(src + 8);
                    fx.mem.write_u32(sp + v + 8, z);
                }
            }
            a3 = Some(quad(fx, at, sp, g, ot, spec, tex, d, l.fix));
        }
    }
    if let Some(cj) = cj {
        fx.mem.set32(sp + cj, k);
    }
    a3
}

/// One quad; returns the a3 it leaves (see [`cones_a3`]).
#[allow(clippy::too_many_arguments)]
fn quad(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, spec: u32, tex: u32, d: u32, fix: u32) -> u32 {
    for (v, xyz, w) in [(0x2B0u32, 0x398u32, 0x2F0u32), (0x2C0, 0x3B0, 0x2F4), (0x2D0, 0x3C8, 0x2F8), (0x2E0, 0x3E0, 0x2FC)] {
        let m = fx.read_m4(sp + 0x130);
        if !fx.project_at(sp + v, &m, sp + xyz, sp + w) {
            let cam = fx.mem.read_u32(0x002E_D6C0);
            return 0x002E_D760u32.wrapping_add(cam.wrapping_mul(0x1A0));
        }
    }
    let a3;
    for o in [0x3E7u32, 0x3CF, 0x3B7, 0x39F] {
        fx.mem.write_u8(sp + o, 0);
    }
    for k in 0..6u32 {
        let v = fx.mem.read_u32(spec + 4 * k);
        fx.mem.write_u32(sp + 0x360 + 4 * k, v);
    }
    let e = fx.mem.u8(g + 8);
    if (1..4).contains(&e) {
        let wrap = u64::from(fx.mem.read_u8(g + 9));
        let (u0, v0, v1) = (fx.mem.i32(sp + 0x368), fx.mem.i32(sp + 0x370), fx.mem.i32(sp + 0x374));
        let clamp = ((wrap << 4) | 0xB) | ((i64::from(u0) << 14) as u64) | ((i64::from(v0) << 24) as u64) | ((i64::from(v1.wrapping_sub(1)) << 34) as u64);
        fx.desc_clamp(d, clamp);
        fx.mem.write_u32(sp + 0x368, 0);
        let b = fx.mem.u8(g + 0xA);
        fx.mem.set32(sp + 0x36C, b);
        let w: [f32; 4] = std::array::from_fn(|k| fx.mem.f(sp + 0x2F0 + 4 * k as u32));
        for (k, o) in [0x38Cu32, 0x3A4, 0x3BC, 0x3D4].into_iter().enumerate() {
            fx.mem.setf(sp + o, ee::div(1.0, w[k]));
        }
        let t = |fx: &mut Fx, o: u32| ee::div(ee::from_i32(fx.mem.i32(sp + o)), 256.0);
        let (s0, t0, s1, t1) = (t(fx, 0x368), t(fx, 0x370), t(fx, 0x36C), t(fx, 0x374));
        for (o, v) in [(0x390u32, ee::div(s0, w[0])), (0x394, ee::div(t0, w[0])), (0x3A8, ee::div(s1, w[1])), (0x3AC, ee::div(t0, w[1])), (0x3C0, ee::div(s0, w[2])), (0x3C4, ee::div(t1, w[2])), (0x3D8, ee::div(s1, w[3])), (0x3DC, ee::div(t1, w[3]))] {
            fx.mem.setf(sp + o, v);
        }
        alpha_mode(fx, sp + 0x360, sp + 0x40C);
        let s = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x40C + k));
        let a = s(fx, 0) | (s(fx, 1) << 2) | (s(fx, 2) << 4) | (s(fx, 3) << 6);
        fx.desc_alpha(d, a);
        fx.desc_texture(d, tex, 1);
        a3 = if fx.mem.read_u16(d + 2) & 0x20 != 0 && tex != 0 { fx.mem.read_u32(tex + 0x30) } else { d + 0x10 };
    } else {
        let f0 = fx.mem.read_u16(d + 2);
        tex_rect_strip(fx, d, sp + 0x360, tex);
        let f = fx.mem.read_u16(d + 2);
        a3 = u32::from(f ^ ((f ^ f0) & 0x200));
    }
    let (w0, w1, w2, w3) = (fx.mem.f(sp + 0x2F0), fx.mem.f(sp + 0x2F4), fx.mem.f(sp + 0x2F8), fx.mem.f(sp + 0x2FC));
    let depth = ee::div(ee::add(w3, ee::add(w2, ee::add(w0, w1))), 4.0);
    fx.fix_residue(depth, at + fix);
    fx.submit(d, ot, ee::fixunssfsi(depth) as i32);
    a3
}
