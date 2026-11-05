//! de5cc68242f9 [1 B record; b223 0x1E03160, 480 words] (helper C9, 31
//! Oct 2026): `plank::draw(ot, g, x)`, a textured band between two points
//! [F].
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};
use crate::fxlib::{ALPHA_42, ALPHA_88};

/// RGBAQ / ST / XYZF2 x 4 (a textured strip, one colour each).
const REGS_TEX4: u64 = 0x4214_2142_1421;

/// `draw(ot, g, x)`: the group `g` (+0 texture, +4 flags, +8 colour B, G,
/// R, A, +0xC the half width, +0x10.. the UVs: i16 x, y, w, h with flag 0,
/// else float s, t, ds, dt; +0x20 / +0x30 the two points, +0x40 the
/// widening direction) as a textured strip (12 values, flags 0x34;
/// ALPHA 0x88 / 0x42 / 0x48 by flags 0xB / 0xA) from each point pushed
/// out by the half width (along +0x40 normalized (0x20C7E0) with flag
/// 0x19, else up) to the point itself, the far side's alpha 0; drawn
/// twice (the other side pushed the other way) unless flag 0x1A. Each
/// vertex projected through `x`'s matrix +0x30 (0x204D80, flags 0x14..
/// 0x17); texture with flag 0x18 unfiltered, else filtered (with flag 0
/// a REGION_CLAMP 0x208400 of the atlas rectangle); with flag 0x13 the
/// far Z at key 0 (whatever was projected), else when all four were
/// projected at their mean w on `ot`. A 0x160 frame.
pub fn draw(fx: &mut Fx, ot: u32, g: u32, x: u32) -> i32 {
    let sp = fx.enter(0x160);
    let d = sp + 0xD0;
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    fx.alpha_for(d, g + 4, &[(0xB, ALPHA_88), (0xA, ALPHA_42)]);
    let v = [sp + 0xE0, sp + 0xF8, sp + 0x110, sp + 0x128];
    let (r, gr, b, a) = (fx.mem.read_u8(g + 0xA), fx.mem.read_u8(g + 9), fx.mem.read_u8(g + 8), fx.mem.read_u8(g + 0xB));
    for (k, &c) in v.iter().enumerate() {
        fx.mem.write_u8(c, r);
        fx.mem.write_u8(c + 1, gr);
        fx.mem.write_u8(c + 2, b);
        fx.mem.write_u8(c + 3, if k < 2 { a } else { 0 });
        fx.mem.write_u32(c + 4, 0x3F80_0000);
    }
    let st = [sp + 0xE8, sp + 0x100, sp + 0x118, sp + 0x130];
    if bit(fx.mem.read_u32(g + 4), 0) {
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        let (u, vv) = (fx.mem.i16(g + 0x10), fx.mem.i16(g + 0x12));
        let (w, h) = (fx.mem.i16(g + 0x14), fx.mem.i16(g + 0x16));
        for (k, &c) in st.iter().enumerate() {
            let uu = if k & 1 == 1 { u + w - 1 } else { u };
            let vvv = if k >= 2 { vv + h - 1 } else { vv };
            fx.set_uv(c, uu);
            fx.set_uv(c + 2, vvv);
        }
    } else {
        let (s, t) = (fx.mem.f(g + 0x10), fx.mem.f(g + 0x14));
        for (k, &c) in st.iter().enumerate() {
            let ss = if k & 1 == 1 { ee::add(s, fx.mem.f(g + 0x18)) } else { s };
            let tt = if k >= 2 { ee::add(t, fx.mem.f(g + 0x1C)) } else { t };
            fx.mem.setf(c, ss);
            fx.mem.setf(c + 4, tt);
        }
    }
    let passes = if bit(fx.mem.read_u32(g + 4), 0x1A) { 1 } else { 2 };
    let off = if bit(fx.mem.read_u32(g + 4), 0x19) {
        crate::blib::trail::normalize3(fx, g + 0x40);
        let w = fx.mem.f(g + 0xC);
        [ee::mul(fx.mem.f(g + 0x40), w), ee::mul(fx.mem.f(g + 0x44), w), ee::mul(fx.mem.f(g + 0x48), w)]
    } else {
        [0.0, fx.mem.f(g + 0xC), 0.0]
    };
    let xyz = [sp + 0xF0, sp + 0x108, sp + 0x120, sp + 0x138];
    let ws = [sp + 0xB0, sp + 0xB4, sp + 0xB8, sp + 0xBC];
    let m = fx.read_m4(x + 0x30);
    for pass in 0..passes {
        for (k, p) in [(0usize, g + 0x20), (1, g + 0x30)] {
            for c in 0..3usize {
                let q = fx.mem.f(p + 4 * c as u32);
                let q = if pass != 0 { ee::sub(q, off[c]) } else { ee::add(q, off[c]) };
                fx.mem.setf(sp + 0xC0 + 4 * c as u32, q);
            }
            if fx.project_at(sp + 0xC0, &m, xyz[k], ws[k]) {
                fx.flag(g + 4, 0x14 + k as u32);
            }
        }
        for (k, p) in [(2usize, g + 0x20), (3, g + 0x30)] {
            if fx.project_at(p, &m, xyz[k], ws[k]) {
                fx.flag(g + 4, 0x14 + k as u32);
            }
        }
        let flags = fx.mem.read_u32(g + 4);
        if bit(flags, 0x18) {
            let tex = fx.mem.read_u32(g);
            fx.desc_texture(d, tex, 0);
        } else {
            if bit(flags, 0) {
                let (u, vv) = (fx.mem.i16(g + 0x10), fx.mem.i16(g + 0x12));
                let (w, h) = (fx.mem.i16(g + 0x14), fx.mem.i16(g + 0x16));
                let c = ((u as i64) << 4) as u64 | 0xA | (((u + w - 1) as i64 as u64) << 14) | ((vv as i64 as u64) << 24) | (((vv + h - 1) as i64 as u64) << 34);
                fx.desc_clamp(d, c);
            }
            let tex = fx.mem.read_u32(g);
            fx.desc_texture(d, tex, 1);
        }
        if bit(fx.mem.read_u32(g + 4), 0x13) {
            fx.far_z(&xyz);
            fx.submit(d, ot, 0);
        } else if fx.all_projected(g + 4) {
            let key = fx.mean_key4(ws);
            fx.submit(d, ot, key);
        }
        fx.clear_projected(g + 4);
    }
    fx.leave(0x160);
    0
}
