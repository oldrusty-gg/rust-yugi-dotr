//! The second effect library, in the executable at 0x106960..0x10A340
//! (used by the Exodia finale c061..c063 and ~250 BMODULE records [F scan,
//! INVENTORY.md]), hand-ported as free functions over the port frame
//! [`Fx`] (helper A, 30 Oct 2026). Addresses are the PAL executable's [F];
//! float operations go through [`crate::ee`] in the original's order.
//! Locals the originals keep on the stack are kept at the original stack
//! addresses (`Fx::enter`), so a copy of a partly written local carries
//! the bytes the original would carry.
//!
//! Vocabulary: a *strip3* descriptor holds 12 values RGBAQ / ST / XYZF2 x
//! 4 (vertex k's values at +0x10 + 0x18 k); a *strip4* one holds 16
//! values RGBAQ / ST(UV) / ... at +0x10 + 0x10 k (the module-linked
//! routines); `rect` is an i16 (x, y, w, h).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, M4, V4};

/// 0x106960(out, id, sub): the texture-atlas rectangle of effect sprite
/// `id` frame `sub` (x, y, w, h as i16 at +0..+6) written into the
/// function's stack local, whose 16 bytes are copied to `out` (bytes 8..15
/// and the fields a case does not set are what the stack held).
pub fn rect(fx: &mut Fx, out: u32, id: i32, sub: i32) {
    let sp = fx.enter(0x10);
    let s = sub as i16 as i32;
    // Fields set: (x, y, w, h), None = not written.
    let f = |x: i32, y: i32, w: i32, h: i32| [Some(x), Some(y), Some(w), Some(h)];
    let wh = |w: i32, h: i32| [None, None, Some(w), Some(h)];
    let v: [Option<i32>; 4] = match id as i16 {
        0x8C if s == 0 => f(1, 1, 0x1E, 0x7E),
        0x8D => match s {
            0 => f(0, 0x80, 0x1F, 0x7F),
            1 => f(0x20, 0x80, 0x1F, 0x7F),
            2 => f(0x40, 0x80, 0x1F, 0x7F),
            _ => wh(0x1F, 0x7F),
        },
        0x8E if s < 4 => f(0x21, (sub << 5) + 1, 0x1E, 0x1E),
        0x8E => f(0x41, ((sub - 4) << 5) + 1, 0x1E, 0x1E),
        0x8F => f(0x60, 0xF0, 0x9F, 0xF),
        0x96 => f(44 * sub + 10, 0, 0x2B, 0x7F),
        0x97 => f(0x8F, 0, 0x1F, 0x7F),
        0x98 => f(0xB0, 0, 0x3B, 0x1F),
        0x99 => f(0xF1, sub << 4, 0xE, 0xF),
        0x9A => f((sub << 5) + 0xB1, 0x21, 0x1E, 0x1E),
        0x9B if s < 2 => f((sub << 5) + 0xB1, 0x41, 0x1E, 0x1E),
        0x9B => f(((sub - 2) << 5) + 0xB1, 0x61, 0x1E, 0x1E),
        0x9C => f(0x80, 0x81, 0x7F, 0x3F),
        0x9D if s < 4 => f((sub << 5) + 0x80, 0xC1, 0x1F, 0x1F),
        0x9D => f(((sub - 4) << 5) + 0x80, 0xE1, 0x1F, 0x1F),
        0x9E => f(1, 10 * sub + 0xD1, 8, 8),
        0x9F if s < 2 => f(59 * sub + 0xB, 0x81, 0x38, 0x3F),
        0x9F => f(59 * (sub - 2) + 0xB, 0xC1, 0x38, 0x3F),
        0x64 => match s {
            0 => f(1, 1, 0x12, 0x1F),
            1 => f(0x15, 1, 0x12, 0x1F),
            2 => f(0x29, 1, 0x12, 0x1F),
            3 => f(0x3D, 1, 0x12, 0x1F),
            4 => f(1, 0x21, 0x12, 0x1F),
            5 => f(0x15, 0x21, 0x12, 0x1F),
            6 => f(0x29, 0x21, 0x12, 0x1F),
            7 => f(0x3D, 0x21, 0x12, 0x1F),
            _ => wh(0x12, 0x1F),
        },
        0x65 => f(0x50, 0, 0x2F, 0x3F),
        0x6E | 0x3C => f(sub << 6, 0, 0x3F, 0x3F),
        0x3D => match s {
            0 => f(0x21, 0, 0x1F, 0x1F),
            1 => f(0x21, 0x21, 0x1F, 0x1F),
            2 => f(0x41, 0, 0x1F, 0x1F),
            3 => f(0x41, 0x21, 0x1F, 0x1F),
            _ => wh(0x1F, 0x1F),
        },
        0x5A => f(sub << 6, 1, 0x3F, 0x3F),
        0x14 if s < 4 => f(sub << 5, 1, 0x1F, 0x1F),
        0x14 => f((sub - 4) << 5, 0x21, 0x1F, 0x1F),
        0x82 => match s {
            0 => f(1, 0x41, 0x12, 0x1F),
            1 => f(0x15, 0x41, 0x12, 0x1F),
            2 => f(0x29, 0x41, 0x12, 0x1F),
            3 => f(0x3D, 0x41, 0x12, 0x1F),
            4 => f(1, 0x61, 0x12, 0x1F),
            5 => f(0x15, 0x61, 0x12, 0x1F),
            6 => f(0x29, 0x61, 0x12, 0x1F),
            7 => f(0x3D, 0x61, 0x12, 0x1F),
            _ => wh(0x12, 0x1F),
        },
        0x83 if s < 2 => f(24 * sub + 0x51, 0x51, 0x17, 0x16),
        0x83 => f(24 * (sub - 2) + 0x51, 0x51, 0x17, 0x16),
        0x50 => f(1, 10 * sub + 1, 8, 8),
        0x0A if s < 5 => f(24 * sub + 1, 1, 0x17, 0x1F),
        0x0A => f(24 * (sub - 5) + 1, 0x21, 0x17, 0x1F),
        0x1E if s < 4 => f(sub << 4, 0, 0x10, 0x10),
        0x1E if s < 8 => f((sub - 4) << 4, 0x10, 0x10, 0x10),
        0x1E if s < 12 => f((sub - 8) << 4, 0x20, 0x10, 0x10),
        0x1E if s < 16 => f((sub - 12) << 4, 0x30, 0x10, 0x10),
        0x1E => wh(0x10, 0x10),
        _ => [None; 4],
    };
    for (k, x) in v.into_iter().enumerate() {
        if let Some(x) = x {
            fx.mem.set16(sp + 2 * k as u32, x);
        }
    }
    for k in 0..4 {
        let w = fx.mem.read_u32(sp + 4 * k);
        fx.mem.write_u32(out + 4 * k, w);
    }
    fx.leave(0x10);
}

/// 0x107390(out, v, half): the four floats at `v` as i16 (0x1A2B80) at
/// `out` (half 0) or `out` + 8.
pub fn to_i16x4(fx: &mut Fx, out: u32, v: u32, half: i32) {
    let o = if half == 0 { out } else { out + 8 };
    for k in 0..4 {
        let x = ee::fixsfsi(fx.mem.f(v + 4 * k));
        fx.mem.set16(o + 2 * k, x);
    }
}

/// 0x1076B0(out, x, y, z).
pub fn set3(fx: &mut Fx, out: u32, x: f32, y: f32, z: f32) {
    fx.mem.setf(out, x);
    fx.mem.setf(out + 4, y);
    fx.mem.setf(out + 8, z);
}

/// 0x107880(m, v): each of the four rows' xyz += v's.
pub fn add_rows(fx: &mut Fx, m: u32, v: u32) {
    for r in 0..4 {
        fx.add3(m + 0x10 * r, v);
    }
}

/// 0x107950(m, axis, f): column `axis` (0, 1, else 2) of the four rows += f.
pub fn add_column(fx: &mut Fx, m: u32, axis: i32, f: f32) {
    let c = match axis {
        0 => 0,
        _ if axis as i16 == 1 => 4,
        _ => 8,
    };
    for r in 0..4 {
        let a = m + 0x10 * r + c;
        let v = ee::add(fx.mem.f(a), f);
        fx.mem.setf(a, v);
    }
}

/// 0x107A10(a, b, dir): dir 0: a = b, else b = a (xyz).
pub fn copy3_dir(fx: &mut Fx, a: u32, b: u32, dir: i32) {
    let (to, from) = if dir == 0 { (a, b) } else { (b, a) };
    copy3(fx, to, from);
}

/// 0x107FD0(p, r, g, b): three bytes.
pub fn rgb(fx: &mut Fx, p: u32, r: i32, g: i32, b: i32) {
    fx.mem.set8(p, r);
    fx.mem.set8(p + 1, g);
    fx.mem.set8(p + 2, b);
}

/// 0x108020(out, c): out = the colour's 4 bytes, out + 4 = 1.0 (through
/// a stack local).
pub fn rgbaq(fx: &mut Fx, out: u32, c: u32) {
    let sp = fx.enter(0x10);
    for k in 0..4 {
        let b = fx.mem.read_u8(c + k);
        fx.mem.write_u8(sp + 8 + k, b);
    }
    fx.mem.write_u32(sp + 0xC, 0x3F80_0000);
    let (a, b) = (fx.mem.read_u32(sp + 8), fx.mem.read_u32(sp + 0xC));
    fx.leave(0x10);
    fx.mem.write_u32(out, a);
    fx.mem.write_u32(out + 4, b);
}

/// The RGBAQ values of a descriptor's vertices `ks` (stride `stride`) =
/// colour `c`, Q 1.0, through the stack locals of 0x108070 / 0x108270.
fn rgbaq_vertices(fx: &mut Fx, d: u32, c: u32, stride: u32, ks: &[(u32, u32)]) {
    let sp = fx.enter(0x40);
    for &(k, local) in ks {
        for i in 0..4 {
            let b = fx.mem.read_u8(c + i);
            fx.mem.write_u8(sp + local + i, b);
        }
        fx.mem.write_u32(sp + local + 4, 0x3F80_0000);
        let (a, b) = (fx.mem.read_u32(sp + local), fx.mem.read_u32(sp + local + 4));
        fx.mem.write_u32(d + 0x10 + stride * k, a);
        fx.mem.write_u32(d + 0x14 + stride * k, b);
    }
    fx.leave(0x40);
}

/// 0x108070(d, c, which): a strip3's RGBAQ = colour `c`: which 0 all four
/// vertices, 1 vertices 0, 1, 2 vertices 2, 3.
pub fn colour_strip3(fx: &mut Fx, d: u32, c: u32, which: i32) {
    match which {
        0 => rgbaq_vertices(fx, d, c, 0x18, &[(0, 0), (1, 8), (2, 0x10), (3, 0x18)]),
        1 => rgbaq_vertices(fx, d, c, 0x18, &[(0, 0x20), (1, 0x28)]),
        2 => rgbaq_vertices(fx, d, c, 0x18, &[(2, 0x30), (3, 0x38)]),
        _ => {
            fx.enter(0x40);
            fx.leave(0x40);
        }
    }
}

/// 0x108270(d, c, which): as [`colour_strip3`] for a 4-vertex strip with
/// RGBAQ at +0x10 + 0x10 k.
pub fn colour_strip4(fx: &mut Fx, d: u32, c: u32, which: i32) {
    match which {
        0 => rgbaq_vertices(fx, d, c, 0x10, &[(0, 0), (1, 8), (2, 0x10), (3, 0x18)]),
        1 => rgbaq_vertices(fx, d, c, 0x10, &[(0, 0x20), (1, 0x28)]),
        2 => rgbaq_vertices(fx, d, c, 0x10, &[(2, 0x30), (3, 0x38)]),
        _ => {
            fx.enter(0x40);
            fx.leave(0x40);
        }
    }
}

/// 0x1084F0(t): t[k] = sinf(pi k / 180) for k = 0..90 (the degree table
/// 0x108910 reads).
pub fn sine_table(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x30);
    for k in 0..0x5B {
        let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), ee::from_i32(k)), 180.0);
        let s = fx.libm_sinf(a);
        fx.mem.setf(t + 4 * k as u32, s);
    }
    let _ = sp;
    fx.leave(0x30);
}

/// [`sine_table`] with its frame left below the caller's sp (0x30: the
/// caller's s0 +0, s1 +0x10, `ra` +0x20) [run b247: the second call's ra
/// is a later frame tint's missing-texture TEX1].
pub fn sine_table_saving(fx: &mut Fx, t: u32, s0: u64, s1: u64, ra: u32) {
    sine_table(fx, t);
    let f = fx.sp - 0x30;
    fx.saved(f, &[(0, s0), (0x10, s1), (0x20, u64::from(ra))]);
}

/// 0x1086B0(a, b, c): a.x += b.x - c.x, a.z += b.z - c.z, and the same
/// for the vector at a + 0x10.
pub fn drift(fx: &mut Fx, a: u32, b: u32, c: u32) {
    for (o, k) in [(0u32, 0u32), (8, 8), (0x10, 0), (0x18, 8)] {
        let d = ee::sub(fx.mem.f(b + k), fx.mem.f(c + k));
        let v = ee::add(fx.mem.f(a + o), d);
        fx.mem.setf(a + o, v);
    }
}

/// The angle in degrees as 0x108910 folds it into 0..359 (i16 steps).
fn fold360(a: i32) -> i32 {
    let mut a = a;
    if (a as i16) >= 360 {
        a = (a % 360) as i16 as i32;
    }
    if (a as i16 as i32) < -359 {
        a = (a % 360) as i16 as i32;
    }
    if (a as i16) < 0 {
        a = (a + 360) as i16 as i32;
    }
    a as i16 as i32
}

/// 0x108910(t, deg, cos): sine (cos 0) or cosine of an angle in degrees
/// from the 91-entry table `t` (0x1084F0), by quadrant.
pub fn sin_deg(fx: &mut Fx, t: u32, deg: i32, cos: i32) -> f32 {
    let a = fold360(deg);
    let at = |fx: &mut Fx, i: i32| fx.mem.f(t.wrapping_add((4 * i) as u32));
    let neg = |v: f32| ee::mul(v, -1.0);
    if cos == 0 {
        if a < 90 {
            at(fx, a)
        } else if a < 180 {
            at(fx, 90 - (a - 90) as i16 as i32)
        } else if a < 270 {
            neg(at(fx, (a - 180) as i16 as i32))
        } else {
            neg(at(fx, 90 - (a - 270) as i16 as i32))
        }
    } else if a < 90 {
        at(fx, 90 - a)
    } else if a < 180 {
        neg(at(fx, (a - 90) as i16 as i32))
    } else if a < 270 {
        neg(at(fx, 90 - (a - 180) as i16 as i32))
    } else {
        at(fx, (a - 270) as i16 as i32)
    }
}

/// The UV fields (14 bits, 12.4) of the u16 pair at `a`.
fn uv(fx: &mut Fx, a: u32, u: i32, v: i32) {
    fx.set_uv(a, u);
    fx.set_uv(a + 2, v);
}

/// 0x108DE0(d, rect, order): the UVs of a strip3 (values 1, 4, 7, 10 at
/// +0x18 + 0x18 k) from the rectangle, corners in one of four orders (0,
/// 1, 10, else).
pub fn uv_strip3(fx: &mut Fx, d: u32, r: u32, order: i32) {
    let (x, y, w, h) = (fx.mem.i16(r), fx.mem.i16(r + 2), fx.mem.i16(r + 4), fx.mem.i16(r + 6));
    let (x1, y1) = (x + w, y + h);
    let c: [(i32, i32); 4] = match order {
        0 => [(x, y), (x1, y), (x, y1), (x1, y1)],
        1 => [(x, y1), (x, y), (x1, y1), (x1, y)],
        10 => [(x1, y), (x1, y1), (x, y), (x, y1)],
        _ => [(x1, y), (x, y), (x1, y1), (x, y1)],
    };
    for (k, (u, v)) in c.into_iter().enumerate() {
        uv(fx, d + 0x18 + 0x18 * k as u32, u, v);
    }
}

/// 0x109230(v, rot, axis, t): `v` rotated about y (axis 0, angle rot[1]),
/// x (1, rot[0]) or z (2, rot[2]) in degrees with the table `t`, through
/// a stack local whose w the original never writes (copied as it is).
pub fn rotate_deg(fx: &mut Fx, v: u32, rot: u32, axis: i32, t: u32) {
    let sp = fx.enter(0x80);
    for k in 0..4 {
        let w = fx.mem.read_u32(rot + 4 * k);
        fx.mem.write_u32(sp + 0x60 + 4 * k, w);
    }
    let copy_back = |fx: &mut Fx| {
        for k in 0..4 {
            let w = fx.mem.read_u32(sp + 0x70 + 4 * k);
            fx.mem.write_u32(v + 4 * k, w);
        }
    };
    if axis == 0 {
        let a = fx.mem.i32(sp + 0x64) as i16 as i32;
        let s = sin_deg(fx, t, a, 0);
        let f20 = ee::mul(fx.mem.f(v + 8), ee::neg(s));
        let c = sin_deg(fx, t, a, 1);
        let x = ee::add(ee::mul(fx.mem.f(v), c), f20);
        fx.mem.setf(sp + 0x70, x);
        let y = fx.mem.f(v + 4);
        fx.mem.setf(sp + 0x74, y);
        let s = sin_deg(fx, t, a, 0);
        let f20 = ee::mul(fx.mem.f(v), s);
        let c = sin_deg(fx, t, a, 1);
        let z = ee::add(f20, ee::mul(fx.mem.f(v + 8), c));
        fx.mem.setf(sp + 0x78, z);
        copy_back(fx);
    }
    if axis == 1 {
        let a = fx.mem.i16(sp + 0x60);
        let x = fx.mem.f(v);
        fx.mem.setf(sp + 0x70, x);
        let c = sin_deg(fx, t, a, 1);
        let f20 = ee::mul(fx.mem.f(v + 4), c);
        let s = sin_deg(fx, t, a, 0);
        let y = ee::add(f20, ee::mul(fx.mem.f(v + 8), s));
        fx.mem.setf(sp + 0x74, y);
        let s = sin_deg(fx, t, a, 0);
        let f20 = ee::mul(fx.mem.f(v + 4), ee::neg(s));
        let c = sin_deg(fx, t, a, 1);
        let z = ee::add(f20, ee::mul(fx.mem.f(v + 8), c));
        fx.mem.setf(sp + 0x78, z);
        copy_back(fx);
    }
    if axis == 2 {
        let a = fx.mem.i32(sp + 0x68) as i16 as i32;
        let c = sin_deg(fx, t, a, 1);
        let f20 = ee::mul(fx.mem.f(v), c);
        let s = sin_deg(fx, t, a, 0);
        let x = ee::add(f20, ee::mul(fx.mem.f(v + 4), s));
        fx.mem.setf(sp + 0x70, x);
        let s = sin_deg(fx, t, a, 0);
        let f20 = ee::mul(fx.mem.f(v), ee::neg(s));
        let c = sin_deg(fx, t, a, 1);
        let y = ee::add(f20, ee::mul(fx.mem.f(v + 4), c));
        fx.mem.setf(sp + 0x74, y);
        let z = fx.mem.f(v + 8);
        fx.mem.setf(sp + 0x78, z);
        copy_back(fx);
    }
    fx.leave(0x80);
}

/// 0x1095C0(out, w, h): a quad's corners (-w, h), (w, h), (-w, -h), (w,
/// -h) in xy, z 0.
pub fn quad_xy(fx: &mut Fx, out: u32, w: f32, h: f32) {
    let (nw, nh) = (ee::neg(w), ee::neg(h));
    for (k, (x, y)) in [(nw, h), (w, h), (nw, nh), (w, nh)].into_iter().enumerate() {
        let a = out + 0x10 * k as u32;
        fx.mem.setf(a, x);
        fx.mem.setf(a + 4, y);
        fx.mem.set32(a + 8, 0);
    }
}

/// 0x109640(out, a, b, mode): a quad from integer extents: rows (x0, y0,
/// 0), (0, y0, 0), (x0, 0, 0), (0, 0, 0) with (x0, y0) = (-a, b), (a, b),
/// (-a, -b), (a, -b) for modes 0..3 (a, b as i16 where negated).
pub fn quad_ints(fx: &mut Fx, out: u32, a: i32, b: i32, mode: i32) {
    let (x, y) = match mode as i16 {
        0 => (ee::from_i32((a as i16 as i32).wrapping_neg()), ee::from_i32(b)),
        1 => (ee::from_i32(a), ee::from_i32(b)),
        2 => (ee::from_i32((a as i16 as i32).wrapping_neg()), ee::from_i32((b as i16 as i32).wrapping_neg())),
        3 => (ee::from_i32(a), ee::from_i32((b as i16 as i32).wrapping_neg())),
        _ => return,
    };
    fx.mem.setf(out, x);
    fx.mem.setf(out + 4, y);
    fx.mem.set32(out + 8, 0);
    fx.mem.set32(out + 0x10, 0);
    fx.mem.setf(out + 0x14, y);
    fx.mem.set32(out + 0x18, 0);
    fx.mem.setf(out + 0x20, x);
    fx.mem.set32(out + 0x24, 0);
    fx.mem.set32(out + 0x28, 0);
    fx.mem.set32(out + 0x30, 0);
    fx.mem.set32(out + 0x34, 0);
    fx.mem.set32(out + 0x38, 0);
}

/// 0x1097D0(out, scale, rot, trans): out = TRS (0x20C0B0 with the
/// pointers' contents) x the camera's view-projection, through the
/// original's stack locals (sp + 0x90 the product, sp + 0x50 the TRS).
pub fn xform_vp(fx: &mut Fx, out: u32, scale: u32, rot: u32, trans: u32) {
    let sp = fx.enter(0xD0);
    fx.camera_vp_to(sp + 0x90);
    let s = (scale != 0).then(|| std::array::from_fn(|k| fx.mem.f(scale + 4 * k as u32)));
    let r = (rot != 0).then(|| std::array::from_fn(|k| fx.mem.i32(rot + 4 * k as u32)));
    let t = (trans != 0).then(|| std::array::from_fn(|k| fx.mem.f(trans + 4 * k as u32)));
    let m = fx.trs(s, r, t);
    fx.write_m4(sp + 0x50, &m);
    fx.mul_into(Some(sp + 0x90), &m);
    for k in 0..16 {
        let w = fx.mem.read_u32(sp + 0x90 + 4 * k);
        fx.mem.write_u32(out + 4 * k, w);
    }
    fx.leave(0xD0);
}

/// 0x204F70(in, out, m, desc, mean_w): the descriptor's XYZF2 vertices
/// (`in`, vec4s, w set to 1) through `m` (the camera's view-projection
/// when 0) with VU0 (0x20C670) into `out`, then per vertex x y z / w;
/// rejected (false) when z < 1, z > 16777215, x or y outside [0, 4095];
/// X / Y = 16 x / 16 y and Z through 0x1A2C70 into the register values; the
/// mean w to `mean_w` when not 0 [F].
pub fn project(fx: &mut Fx, input: u32, out: u32, m: u32, desc: u32, mean_w: u32) -> bool {
    let sp = fx.enter(0xF0);
    let n = u32::from(fx.mem.read_u16(desc));
    let regs = fx.mem.read_u64(desc + 8);
    let m: M4 = if m == 0 {
        fx.camera_vp_to(sp + 0xB0);
        fx.read_m4(sp + 0xB0)
    } else {
        fx.read_m4(m)
    };
    let mut count = 0u32;
    for i in 0..n {
        if (regs >> (4 * i)) & 15 == 4 {
            fx.mem.write_u32(input + 16 * count + 12, 0x3F80_0000);
            count += 1;
        }
    }
    fx.push(Some(m));
    for k in 0..count {
        let v: V4 = fx.read_v4((input + 16 * k) & !15);
        let o = Fx::vu_apply(&fx.cur, v);
        fx.write_v4((out + 16 * k) & !15, o);
    }
    fx.pop();
    let mut sum_w = 0.0f32;
    let mut v = out;
    let lim = f32::from_bits(0x457F_F000);
    let mut ok = true;
    for i in 0..n {
        if (regs >> (4 * i)) & 15 != 4 {
            continue;
        }
        let value = desc + 0x10 + 8 * i;
        let w = fx.mem.f(v + 12);
        let q = ee::div(1.0, w);
        let x = ee::mul(fx.mem.f(v), q);
        fx.mem.setf(v, x);
        let y = ee::mul(fx.mem.f(v + 4), q);
        fx.mem.setf(v + 4, y);
        let z = ee::mul(fx.mem.f(v + 8), q);
        fx.mem.setf(v + 8, z);
        if ee_lt(z, 1.0) || !ee_le(z, f32::from_bits(0x4B7F_FFFF)) || ee_lt(x, 0.0) || !ee_le(x, lim) || ee_lt(y, 0.0) || !ee_le(y, lim) {
            ok = false;
            break;
        }
        // 0x1A2C70's frames (0x30 below this one) as the original leaves
        // them: 16 x from 0x20513C, 16 y from 0x205154, z from 0x205160.
        let (x16, y16) = (ee::mul(16.0, x), ee::mul(16.0, y));
        fx.fix_residue(x16, 0x0020_513C);
        fx.mem.write_u16(value, ee::fixunssfsi(x16) as u16);
        fx.fix_residue(y16, 0x0020_5154);
        fx.mem.write_u16(value + 2, ee::fixunssfsi(y16) as u16);
        fx.fix_residue(z, 0x0020_5160);
        let fz = u64::from(ee::fixunssfsi(z) & 0xFF_FFFF);
        let old = fx.mem.read_u64(value);
        fx.mem.write_u64(value, (old & 0xFF00_0000_FFFF_FFFF) | (fz << 32));
        sum_w = ee::add(sum_w, fx.mem.f(v + 12));
        v += 16;
    }
    if ok && mean_w != 0 {
        let mean = ee::div(sum_w, ee::from_i32(count as i32));
        fx.mem.setf(mean_w, mean);
    }
    fx.leave(0xF0);
    ok
}

/// 0x109870(in, desc, m, ot): [`project`] into the scratchpad, submitted
/// at the mean w's key when on screen.
pub fn project_submit(fx: &mut Fx, input: u32, desc: u32, m: u32, ot: u32) {
    let sp = fx.enter(0x40);
    if project(fx, input, 0x7000_0000, m, desc, sp + 0x30) {
        let key = ee::fixunssfsi(fx.mem.f(sp + 0x30)) as i32;
        fx.submit(desc, ot, key);
    }
    fx.leave(0x40);
}

/// 0x1099E0(vs, rot, axis, t): the four vectors at `vs` rotated by
/// [`rotate_deg`] (the angles copied to a stack local first).
pub fn rotate4(fx: &mut Fx, vs: u32, rot: u32, axis: i32, t: u32) {
    let sp = fx.enter(0x50);
    for k in 0..4 {
        let w = fx.mem.read_u32(rot + 4 * k);
        fx.mem.write_u32(sp + 0x40 + 4 * k, w);
    }
    for k in 0..4 {
        rotate_deg(fx, vs + 0x10 * k, sp + 0x40, axis, t);
    }
    fx.leave(0x50);
}

/// 0x109B50(d): a strip3 header (12 values, flags 0x834: FST), additive.
pub fn desc_strip3(fx: &mut Fx, d: u32) {
    fx.enter(0x10);
    fx.mem.set16(d + 2, 0x34);
    fx.mem.set16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.set32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    fx.desc_alpha(d, crate::fxlib::ALPHA_ADD);
    fx.leave(0x10);
}

/// 0x109BC0(d, blend): an untextured 4-vertex gouraud strip (8 values,
/// flags 0x14), additive (blend 0) or 0x42; the Qs (+0x14 + 0x10 k) and
/// alphas (+0x1F + 0x10 k) cleared.
pub fn desc_gouraud4(fx: &mut Fx, d: u32, blend: i32) {
    fx.enter(0x20);
    fx.mem.set16(d + 2, 0x14);
    fx.mem.set16(d, 8);
    fx.mem.write_u64(d + 8, 0x4141_4141);
    fx.mem.set32(d + 4, 0);
    fx.desc_alpha(d, if blend == 0 { crate::fxlib::ALPHA_ADD } else { crate::fxlib::ALPHA_42 });
    for o in [0x44u32, 0x34, 0x24, 0x14] {
        fx.mem.set32(d + o, 0);
    }
    for o in [0x1Fu32, 0x2F, 0x3F, 0x4F] {
        fx.mem.write_u8(d + o, 0);
    }
    fx.leave(0x20);
}

/// The far Z (0xFFFFFF) into the XYZ values at `xyz` (last first), the
/// chain the originals merge.
pub fn far_z(fx: &mut Fx, xyz: &[u32]) {
    let mut z = 0xFF_FFFFu64;
    for &a in xyz {
        fx.set_xyz_z(a, z);
        z = fx.xyz_z(a);
    }
}

/// 0x109C60(d, kind): the far Z on a strip3's (kind 0: +0x68, +0x50,
/// +0x38, +0x20) or a strip4's (+0x48, +0x38, +0x28, +0x18) vertices.
pub fn far_z_desc(fx: &mut Fx, d: u32, kind: i32) {
    if kind == 0 {
        far_z(fx, &[d + 0x68, d + 0x50, d + 0x38, d + 0x20]);
    } else {
        far_z(fx, &[d + 0x48, d + 0x38, d + 0x28, d + 0x18]);
    }
}

/// 0x109DD0(d): a gouraud strip4 covering the window (0x700..0x900 in
/// both axes, 12.4), Qs cleared, far Z, F 0.
pub fn fullscreen4(fx: &mut Fx, d: u32) {
    let (lo, hi) = ((0x800 - 0x100) << 4, (0x100 + 0x800) << 4);
    for o in [0x44u32, 0x34, 0x24, 0x14] {
        fx.mem.set32(d + o, 0);
    }
    for (k, (x, y)) in [(lo, lo), (hi, lo), (lo, hi), (hi, hi)].into_iter().enumerate() {
        let a = d + 0x18 + 0x10 * k as u32;
        fx.mem.set16(a, x);
        fx.mem.set16(a + 2, y);
        let v = fx.mem.read_u64(a);
        fx.mem.write_u64(a, (v & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(a + 7, 0);
    }
}

/// 0x10A1B0(out, p, q, a, b): with A = a + p, B = b + p (x, z; y as given),
/// t = ((0 - A.x) dx + (q.y - A.y) dy + (0 - A.z) dz) / |B - A|^2 and out
/// = (A.x + t dx, 0, A.z + t dz) through a stack local (w as the stack
/// held); a zero length gives (0, stale...) [F: the x and z of the point
/// read as 0, the register the code reuses].
pub fn segment_point(fx: &mut Fx, out: u32, p: u32, q: u32, a: u32, b: u32) {
    let sp = fx.enter(0x10);
    fx.mem.set32(sp, 0);
    let f7 = ee::add(fx.mem.f(a), fx.mem.f(p));
    let f8 = fx.mem.f(a + 4);
    let f2 = ee::add(fx.mem.f(b), fx.mem.f(p));
    let f9 = ee::add(fx.mem.f(a + 8), fx.mem.f(p + 8));
    let f0 = ee::add(fx.mem.f(b + 8), fx.mem.f(p + 8));
    let dx = ee::sub(f2, f7);
    let dy = ee::sub(fx.mem.f(b + 4), f8);
    let dz = ee::sub(f0, f9);
    let acc = ee::add(ee::mul(dx, dx), ee::mul(dy, dy));
    let len2 = ee::add(acc, ee::mul(dz, dz));
    if !crate::libm_tan::ee_eq(0.0, len2) {
        let qy = fx.mem.f(q + 4);
        let f0 = ee::mul(dy, ee::sub(qy, f8));
        let f1 = ee::mul(dx, ee::sub(0.0, f7));
        let acc = ee::add(f1, f0);
        let f0 = ee::add(acc, ee::mul(dz, ee::sub(0.0, f9)));
        let t = ee::div(f0, len2);
        fx.mem.setf(sp, ee::add(f7, ee::mul(dx, t)));
        fx.mem.set32(sp + 4, 0);
        fx.mem.setf(sp + 8, ee::add(f9, ee::mul(dz, t)));
    }
    for k in 0..4 {
        let w = fx.mem.read_u32(sp + 4 * k);
        fx.mem.write_u32(out + 4 * k, w);
    }
    fx.leave(0x10);
}

/// 0x10A2B0(out, a, b, f): out = (a - (b + b')) / f per axis (b' = the
/// vector at b + 0x10), w 0.
pub fn spread(fx: &mut Fx, out: u32, a: u32, b: u32, f: f32) {
    let sp = fx.enter(0x10);
    for k in [0u32, 8, 4] {
        let s = ee::add(fx.mem.f(b + k), fx.mem.f(b + 0x10 + k));
        let v = ee::div(ee::sub(fx.mem.f(a + k), s), f);
        fx.mem.setf(sp + k, v);
    }
    fx.mem.set32(sp + 0xC, 0);
    for k in 0..4 {
        let w = fx.mem.read_u32(sp + 4 * k);
        fx.mem.write_u32(out + 4 * k, w);
    }
    fx.leave(0x10);
}

/// 0x10A340(a, b): a = b (xyz).
pub fn copy3(fx: &mut Fx, a: u32, b: u32) {
    for k in 0..3 {
        let v = fx.mem.read_u32(b + 4 * k);
        fx.mem.write_u32(a + 4 * k, v);
    }
}

/// 0x205580(out): the camera's angles (0x1FC1C0) as four words.
pub fn camera_angles_to(fx: &mut Fx, out: u32) {
    let a = fx.camera_angles();
    for (k, v) in a.into_iter().enumerate() {
        fx.mem.set32(out + 4 * k as u32, v);
    }
}
