//! A pair of turned textured plates of the effect library (helper C3, 31
//! Oct 2026).
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};
use crate::fxlib::{ALPHA_42, ALPHA_44, ALPHA_88};

/// RGBAQ / ST / XYZF2 x 4 (a textured strip).
const REGS_TEX4: u64 = 0x4214_2142_1421;

/// bd239d3ca267 [16 B records; b642 0x1E01300, 356 words] [F]:
/// `plates(ot, g, x) -> 0`: two textured quads in the plane z of the
/// record's template (8 vec4 at the record-relative `lui` / `addiu` +0x34 /
/// +0x38, copied to sp + 0xA0), half sizes (+0xC, +0x10) and (+0x18, +0x14)
/// (x, y of the corners -+, ++, --, +-), turned by the angles +0x30
/// (0x20BDA0, sp + 0x1A0; 0x20C540 into sp + 0x120), projected through `x`
/// + 0x30 (0x204D80, vertex bits 0x14..0x17). Descriptor at sp + 0x1F0:
/// blend by flags 0xC / 0xB / 0xA (0x44 / 0x88 / 0x42, else 0x48), colour
/// +8 (0x1E00080 colours4), FST UVs from the rectangle +0x20 (0x104540
/// order 0), REGION_CLAMP from it (0x208400), texture +0 (0x207E50 filter
/// 1); drawn when all four projected: flag 0x13 far Z key 0, else the
/// mean-w key (0x1A2C70). `g`: +0 RH2, +4 flags, +8 colour, +0xC.. sizes,
/// +0x20 rectangle (i16 x, y, w, h), +0x30 angles (i32 x 3).
pub fn plates(fx: &mut Fx, at: u32, ot: u32, g: u32, x: u32) -> i32 {
    let template = rel(fx, at, 0x34, 0x38);
    let sp = fx.enter(0x280);
    for k in 0..8u32 {
        let v = fx.read_v4(template + 16 * k);
        fx.write_v4(sp + 0xA0 + 16 * k, v);
    }
    let corners = |fx: &mut Fx, base: u32, w: u32, h: u32| {
        for (k, (sx, sy)) in [(true, false), (false, false), (true, true), (false, true)].into_iter().enumerate() {
            let a = base + 16 * k as u32;
            let vx = fx.mem.f(g + w);
            fx.mem.setf(a, if sx { ee::neg(vx) } else { vx });
            let vy = fx.mem.f(g + h);
            fx.mem.setf(a + 4, if sy { ee::neg(vy) } else { vy });
        }
    };
    corners(fx, sp + 0xA0, 0xC, 0x10);
    corners(fx, sp + 0xE0, 0x18, 0x14);
    let rot = [fx.mem.i32(g + 0x30), fx.mem.i32(g + 0x34), fx.mem.i32(g + 0x38)];
    let r = fx.rotation_matrix(rot);
    fx.write_m4(sp + 0x1A0, &r);
    for k in 0..8u32 {
        let v = fx.read_v4(sp + 0xA0 + 16 * k);
        let o = fx.transform(Some(&r), v);
        fx.write_v4(sp + 0x120 + 16 * k, o);
    }
    let d = sp + 0x1F0;
    fx.desc_init(d, 12, 0x34, REGS_TEX4);
    fx.alpha_for(d, g + 4, &[(0xC, ALPHA_44), (0xB, ALPHA_88), (0xA, ALPHA_42)]);
    fx.colours4(d, g + 8);
    fx.uv_rect(d, g + 0x20, 0);
    let xyz = [sp + 0x210, sp + 0x228, sp + 0x240, sp + 0x258];
    let ws = [sp + 0x1E0, sp + 0x1E4, sp + 0x1E8, sp + 0x1EC];
    for q in 0..2u32 {
        let m = fx.read_m4(x + 0x30);
        for i in 0..4u32 {
            if fx.project_at(sp + 0x120 + 0x40 * q + 16 * i, &m, xyz[i as usize], ws[i as usize]) {
                fx.flag(g + 4, 0x14 + i);
            }
        }
        let (rx, ry, rw, rh) = (fx.mem.i16(g + 0x20), fx.mem.i16(g + 0x22), fx.mem.i16(g + 0x24), fx.mem.i16(g + 0x26));
        let clamp = ((rx as i64) << 4) as u64 | 0xA | (((rx + rw - 1) as i64 as u64) << 14) | ((ry as i64 as u64) << 24) | (((ry + rh - 1) as i64 as u64) << 34);
        fx.desc_clamp(d, clamp);
        let rh2 = fx.mem.read_u32(g);
        fx.desc_texture(d, rh2, 1);
        if fx.all_projected(g + 4) {
            if bit(fx.mem.read_u32(g + 4), 0x13) {
                fx.far_z(&xyz);
                fx.submit(d, ot, 0);
            } else {
                let key = fx.mean_key4(ws);
                fx.submit(d, ot, key);
            }
        }
        fx.clear_projected(g + 4);
    }
    fx.leave(0x280);
    0
}
