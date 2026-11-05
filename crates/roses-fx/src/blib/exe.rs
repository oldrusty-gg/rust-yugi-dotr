//! Executable helpers the shared library calls (not ported in phase 1),
//! as free functions over the [`Fx`] frame (PAL SLES_515.79 addresses
//! [F]).
use crate::bus::Bus;
use crate::exe::{Fx, CAMERA_TABLE};

/// 0x101AF0(p): a delayed countdown of two bytes: with p[0] = 0, 0; with
/// a delay p[1] > 0, p[1] -= 1 and 0; else p[0] -= 1 and 1.
pub fn countdown(fx: &mut Fx, p: u32) -> i32 {
    let n = fx.mem.read_u8(p);
    if n == 0 {
        return 0;
    }
    let delay = fx.mem.read_u8(p + 1);
    if delay != 0 {
        fx.mem.write_u8(p + 1, delay.wrapping_sub(1));
        return 0;
    }
    fx.mem.write_u8(p, n.wrapping_sub(1));
    1
}

/// 0x101BA0(p, out): the node position (0x1F6830) of request slot p[6] on
/// the unit p[5] names (1 self 0x1F65B0, 2 target 0x1F6590) into `out`;
/// `None` for any other p[5] (v0 is left as the caller's then).
pub fn node_source(fx: &mut Fx, p: u32, out: u32) -> Option<i32> {
    let handle = match fx.mem.read_u8(p + 5) {
        1 => fx.self_handle(),
        2 => fx.target_handle(),
        _ => return None,
    };
    let slot = fx.mem.u8(p + 6);
    Some(i32::from(fx.node_position(handle, slot, out)))
}

/// 0x1FAB50(camera, slot, p): only camera 0 (the object at *0x2DF5C0) and
/// slots 0..9; p = 0 clears bit `slot` of obj+0x8AA, p = 1 sets it, any
/// other p copies three quadwords from p into obj+0x600 + 0x30 slot (w of
/// the second set to 1) and sets the bit; 1 when done, else 0.
pub fn camera_slot(fx: &mut Fx, camera: i32, slot: i32, p: u32) -> i32 {
    if camera != 0 {
        return 0;
    }
    let obj = fx.mem.read_u32(CAMERA_TABLE);
    if obj == 0 || !(0..10).contains(&slot) {
        return 0;
    }
    let bits = fx.mem.read_u16(obj + 0x8AA);
    if p == 0 {
        fx.mem.write_u16(obj + 0x8AA, bits & !(1 << slot));
        return 1;
    }
    if p != 1 {
        let t = obj.wrapping_add(0x30 * slot as u32);
        for q in 0..3u32 {
            let (src, dst) = ((p + 0x10 * q) & !15, (t + 0x600 + 0x10 * q) & !15);
            for k in (0..16).step_by(8) {
                let v = fx.mem.read_u64(src + k);
                fx.mem.write_u64(dst + k, v);
            }
        }
        fx.mem.write_u32(t + 0x61C, 0x3F80_0000);
    }
    let bits = fx.mem.read_u16(obj + 0x8AA);
    fx.mem.write_u16(obj + 0x8AA, bits | (1 << slot));
    1
}

/// 0x1F7E20(p): camera 0's slots 4 and 5 from `p` (0x1FAB50 twice); the
/// second's result.
pub fn camera_slots45(fx: &mut Fx, p: u32) -> i32 {
    camera_slot(fx, 0, 4, p);
    camera_slot(fx, 0, 5, p)
}

/// 0x1A7158 cosf(x) [F] (libm's; [`Fx::cosf`] is the executable's table
/// cosine 0x20AD90): |x| <= 0x3F490FD8 straight to the cosine kernel
/// 0x1A6098, else reduced by 0x1A5BF8 and by n & 3: cos, -sin, -cos, sin
/// (the kernels and the reduction as `libm_tan` ports them).
pub fn libm_cosf(fx: &mut Fx, x: f32) -> f32 {
    if x.to_bits() & 0x7FFF_FFFF <= 0x3F49_0FD8 {
        return Fx::kernel_cosf(x, 0.0);
    }
    let (n, y0, y1) = fx.rem_pio2f(x);
    match n & 3 {
        0 => Fx::kernel_cosf(y0, y1),
        1 => crate::ee::neg(Fx::kernel_sinf(y0, y1, 1)),
        2 => crate::ee::neg(Fx::kernel_cosf(y0, y1)),
        _ => Fx::kernel_sinf(y0, y1, 1),
    }
}

/// 0x101C80(spec, out): the ALPHA selectors (A, B, C, D bytes) of blend
/// mode spec[5]: 1 (0, 1, 0, 1), 2 (0, 2, 0, 1), 3 (1, 0, 0, 1), 4 (2, 0,
/// 0, 1); any other leaves `out` as it was.
pub fn alpha_mode(fx: &mut Fx, spec: u32, out: u32) {
    let v: [u8; 4] = match fx.mem.read_u8(spec + 5) {
        1 => [0, 1, 0, 1],
        2 => [0, 2, 0, 1],
        3 => [1, 0, 0, 1],
        4 => [2, 0, 0, 1],
        _ => return,
    };
    for (k, b) in v.into_iter().enumerate() {
        fx.mem.write_u8(out + k as u32, b);
    }
}

/// 0x101D20(desc, spec, rh2): a textured strip's texture from a sprite
/// spec (i32 u0 +8, u1 +0xC, v0 +0x10, v1 +0x14; blend mode +5): flag
/// 0x800, the four UVs (u0 v0, u1 v0, u0 v1, u1 v1), REGION_CLAMP
/// u0..u1-1 x v0..v1-1 (0x208400), TEX0 / TEX1 from `rh2` with filter 1
/// (0x207E50), ALPHA from the mode's selectors (0x101C80 into the stack
/// local sp + 0x4C of a 0x50 frame; stale for an unknown mode).
pub fn tex_rect(fx: &mut Fx, d: u32, spec: u32, rh2: u32) {
    tex_rect_a3(fx, d, spec, rh2);
}

/// [`tex_rect`] and the a3 it leaves: the descriptor's flags
/// before 0x207E20 sets 0x200 in them [F 0x101D20 / 0x207E20].
pub fn tex_rect_a3(fx: &mut Fx, d: u32, spec: u32, rh2: u32) -> u32 {
    let sp = fx.enter(0x50);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    for (off, field) in [(0x18u32, 8u32), (0x1A, 0x10), (0x28, 0xC), (0x2A, 0x10), (0x38, 8), (0x3A, 0x14), (0x48, 0xC), (0x4A, 0x14)] {
        let v = fx.mem.i32(spec + field);
        fx.set_uv(d + off, v);
    }
    let (u0, u1, v0, v1) = (fx.mem.i32(spec + 8), fx.mem.i32(spec + 0xC), fx.mem.i32(spec + 0x10), fx.mem.i32(spec + 0x14));
    let clamp = ((i64::from(u0) << 4) as u64 | 0xA) | ((i64::from(u1.wrapping_sub(1)) << 14) as u64) | ((i64::from(v0) << 24) as u64) | ((i64::from(v1.wrapping_sub(1)) << 34) as u64);
    fx.desc_clamp(d, clamp);
    fx.desc_texture(d, rh2, 1);
    alpha_mode(fx, spec, sp + 0x4C);
    let b = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x4C + k));
    let alpha = b(fx, 0) | (b(fx, 1) << 2) | (b(fx, 2) << 4) | (b(fx, 3) << 6);
    let a3 = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, alpha);
    fx.leave(0x50);
    a3
}

/// 0x20E2B0(out): the current camera's view-projection into `out`.
pub fn camera_vp_into(fx: &mut Fx, out: u32) {
    let m = fx.camera_vp();
    fx.write_m4(out, &m);
}

/// 0x20C0B0(out, 0, 0, trans): the translation matrix of the vec3 at
/// `trans` into `out`.
pub fn translation_into(fx: &mut Fx, out: u32, trans: u32) {
    let t = [fx.mem.f(trans), fx.mem.f(trans + 4), fx.mem.f(trans + 8)];
    let m = fx.trs(None, None, Some(t));
    fx.write_m4(out, &m);
}

/// 0x20B370(a, b) with both in memory: `a` = `b` x `a`.
pub fn mul_mem(fx: &mut Fx, a: u32, b: u32) {
    let m = fx.read_m4(b);
    fx.mul_into(Some(a), &m);
}

/// The unsigned conversion the records use for a u32 (cvt.s.w, or for
/// bit 31 set the halved value rounded to odd, converted and doubled).
pub fn cvt_u32(v: u32) -> f32 {
    if (v as i32) < 0 {
        let h = (v >> 1) | (v & 1);
        let f = crate::ee::from_i32(h as i32);
        crate::ee::add(f, f)
    } else {
        crate::ee::from_i32(v as i32)
    }
}

/// 0x101EE0(desc, spec, rh2): as [`tex_rect`] for a strip of RGBAQ / UV
/// / XYZF2 vertices (UVs at +0x18, +0x30, +0x48, +0x60: u0 v0, u1 v0, u0
/// v1, u1 v1), ALPHA only when the spec's blend mode +5 is not 0.
pub fn tex_rect_strip(fx: &mut Fx, d: u32, spec: u32, rh2: u32) {
    tex_rect_strip_a3(fx, d, spec, rh2);
}

/// [`tex_rect_strip`] and the a3 it leaves [F 0x101EE0 /
/// 0x207E50 / 0x207E20]: with a blend, the descriptor's flags before
/// 0x207E20 sets 0x200 in them; else 0x207E50's: the TEX0 low word
/// (RH2 + 0x30) with flag 0x20 and a texture, otherwise desc + 0x10.
pub fn tex_rect_strip_a3(fx: &mut Fx, d: u32, spec: u32, rh2: u32) -> u32 {
    let sp = fx.enter(0x50);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    for (off, field) in [(0x18u32, 8u32), (0x1A, 0x10), (0x30, 0xC), (0x32, 0x10), (0x48, 8), (0x4A, 0x14), (0x60, 0xC), (0x62, 0x14)] {
        let v = fx.mem.i32(spec + field);
        fx.set_uv(d + off, v);
    }
    let (u0, u1, v0, v1) = (fx.mem.i32(spec + 8), fx.mem.i32(spec + 0xC), fx.mem.i32(spec + 0x10), fx.mem.i32(spec + 0x14));
    let clamp = ((i64::from(u0) << 4) as u64 | 0xA) | ((i64::from(u1.wrapping_sub(1)) << 14) as u64) | ((i64::from(v0) << 24) as u64) | ((i64::from(v1.wrapping_sub(1)) << 34) as u64);
    fx.desc_clamp(d, clamp);
    let mut a3 = if fx.mem.read_u16(d + 2) & 0x20 != 0 && rh2 != 0 { fx.mem.read_u32(rh2 + 0x30) } else { d + 0x10 };
    fx.desc_texture(d, rh2, 1);
    if fx.mem.read_u8(spec + 5) != 0 {
        alpha_mode(fx, spec, sp + 0x4C);
        let b = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x4C + k));
        let alpha = b(fx, 0) | (b(fx, 1) << 2) | (b(fx, 2) << 4) | (b(fx, 3) << 6);
        a3 = u32::from(fx.mem.read_u16(d + 2));
        fx.desc_alpha(d, alpha);
    }
    fx.leave(0x50);
    a3
}

/// The jump table of 0x1020B0 (10 handlers) and what each handler sets:
/// (handler, cell width, cell height, columns, rows) [F 0x1020D8..
/// 0x10219C]. The table itself is read from the user's executable.
const ATLAS_TABLE: u32 = 0x002B_8360;
const ATLAS_LAYOUTS: [(u32, i32, i32, i32, i32); 10] = [
    (0x0010_20D8, 0x20, 0x20, 4, 2),
    (0x0010_20EC, 0x20, 0x20, 2, 4),
    (0x0010_2100, 0x20, 0x20, 4, 4),
    (0x0010_2114, 0x20, 0x40, 8, 1),
    (0x0010_2128, 0x40, 0x40, 2, 1),
    (0x0010_213C, 0x40, 0x40, 2, 2),
    (0x0010_2150, 0x40, 0x40, 1, 1),
    (0x0010_2164, 0x20, 0x80, 1, 1),
    (0x0010_2178, 0x80, 0x40, 1, 1),
    (0x0010_218C, 0x80, 0x80, 1, 1),
];

/// 0x1020B0(spec, cell, layout): the rectangle of atlas cell `cell` of
/// layout 0..9 (cell size, columns x rows by the executable's jump table)
/// into the spec's u0 +8, u1 +0xC, v0 +0x10, v1 +0x14; a cell past the
/// last zeroes them. `None` for a layout outside 0..9 or a table entry not
/// one of the ten handlers (the original then uses whatever registers
/// held).
pub fn atlas_cell(fx: &mut Fx, spec: u32, cell: i32, layout: i32) -> Option<()> {
    if !(0..10).contains(&layout) {
        return None;
    }
    let handler = fx.mem.read_u32(ATLAS_TABLE + 4 * layout as u32);
    let &(_, w, h, cols, rows) = ATLAS_LAYOUTS.iter().find(|l| l.0 == handler)?;
    if fx.mult(cols, rows) - 1 < cell {
        for o in [0x14u32, 0x10, 0xC, 8] {
            fx.mem.write_u32(spec + o, 0);
        }
        return Some(());
    }
    let (row, col) = fx.div(cell, cols);
    let x = fx.mult(w, col);
    fx.mem.set32(spec + 8, x);
    let v = fx.mem.i32(spec + 8).wrapping_add(w);
    fx.mem.set32(spec + 0xC, v);
    fx.mem.set32(spec + 0x10, h.wrapping_mul(row));
    let v = fx.mem.i32(spec + 0x10).wrapping_add(h);
    fx.mem.set32(spec + 0x14, v);
    Some(())
}

/// 0x101940(v, out): the angles that point along `v` (i32 x3, 16-bit):
/// pitch 0x1A2B80(65536 atan2f(sqrtf(x x + z z), y) / 2 pi) & 0xFFFF, yaw
/// the same of atan2f(x, z), roll 0.
pub fn aim_angles(fx: &mut Fx, v: u32, out: u32) {
    let (x, y, z) = (fx.mem.f(v), fx.mem.f(v + 4), fx.mem.f(v + 8));
    let r = fx.sqrtf(crate::ee::add(crate::ee::mul(x, x), crate::ee::mul(z, z)));
    let a = fx.atan2f(r, y);
    let t = crate::ee::div(a, f32::from_bits(0x40C9_0FDB));
    let p = crate::ee::fixsfsi(crate::ee::mul(65536.0, t)) & 0xFFFF;
    fx.mem.set32(out, p);
    let (x, z) = (fx.mem.f(v), fx.mem.f(v + 8));
    let a = fx.atan2f(x, z);
    let t = crate::ee::div(a, f32::from_bits(0x40C9_0FDB));
    let q = crate::ee::fixsfsi(crate::ee::mul(65536.0, t)) & 0xFFFF;
    fx.mem.set32(out + 4, q);
    fx.mem.set32(out + 8, 0);
}

/// 0x20C0B0(out, 0, rot, trans): rotation (i32 angles at `rot`) and
/// translation (vec3 at `trans`) into `out`.
pub fn rot_trans_into(fx: &mut Fx, out: u32, rot: u32, trans: u32) {
    let r = [fx.mem.i32(rot), fx.mem.i32(rot + 4), fx.mem.i32(rot + 8)];
    let t = [fx.mem.f(trans), fx.mem.f(trans + 4), fx.mem.f(trans + 8)];
    let m = fx.trs(None, Some(r), Some(t));
    fx.write_m4(out, &m);
}

/// The jump table of 0x1022D0 (10 handlers) and each handler's mask
/// [F 0x1022F8..0x102318]; the table is read from the user's executable.
const FRAME_MASK_TABLE: u32 = 0x002B_83C0;
const FRAME_MASKS: [(u32, i32); 5] = [(0x0010_22F8, 7), (0x0010_2300, 1), (0x0010_2308, 0xF), (0x0010_2310, 3), (0x0010_2318, 0)];

/// 0x1022D0(kind, v): v masked by the frame count of animation kind
/// 0..9 (by the executable's jump table: 7, 1, 0xF, 3 or 0); 0 for
/// another kind (unsigned compare) or an unknown handler.
pub fn frame_mask(fx: &mut Fx, kind: i32, v: i32) -> i32 {
    if kind as u32 >= 10 {
        return 0;
    }
    let handler = fx.mem.read_u32(FRAME_MASK_TABLE + 4 * kind as u32);
    FRAME_MASKS.iter().find(|m| m.0 == handler).map_or(0, |m| v & m.1)
}

/// 0x101C30(p, stride, n): the first of `n` records (`stride` bytes from
/// `p`) whose byte 0 is 0, else -1.
pub fn first_free(fx: &mut Fx, p: u32, stride: i32, n: i32) -> i32 {
    let mut i = 0;
    while i < n {
        if fx.mem.read_u8(p.wrapping_add(i.wrapping_mul(stride) as u32)) == 0 {
            return i;
        }
        i += 1;
    }
    -1
}

/// 0x101AD0(p, count, delay, c): p[2] = p[0] = count, p[1] = delay, p[3]
/// = c (a countdown's start).
pub fn set_counter4(fx: &mut Fx, p: u32, count: i32, delay: i32, c: i32) {
    fx.mem.set8(p + 2, count);
    fx.mem.set8(p, count);
    fx.mem.set8(p + 1, delay);
    fx.mem.set8(p + 3, c);
}

/// 0x101B40(p, count, delay): p[2] = p[0] = count, p[1] = delay.
pub fn set_counter(fx: &mut Fx, p: u32, count: i32, delay: i32) {
    fx.mem.set8(p + 2, count);
    fx.mem.set8(p, count);
    fx.mem.set8(p + 1, delay);
}

/// 0x100F60(out, a, b, c) [F]: m = (a + b) x 0.5 at sp +
/// 0x30 of its 0x50 frame, (c - m) / 3 at sp + 0x40, out = that + m (the
/// point a third of the way from the midpoint of a / b to c).
pub fn third_point(fx: &mut Fx, out: u32, a: u32, b: u32, c: u32) {
    let s = fx.sp - 0x50;
    fx.vu_add4(s + 0x30, a, b);
    fx.vu_scale4(s + 0x30, s + 0x30, 0.5);
    fx.vu_sub4(s + 0x40, c, s + 0x30);
    fx.vu_scale4(s + 0x40, s + 0x40, f32::from_bits(0x3EAA_AAAB));
    fx.vu_add4(out, s + 0x40, s + 0x30);
}
