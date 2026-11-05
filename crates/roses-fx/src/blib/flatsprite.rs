//! de3814e8ae4c (b143 0x1E009C0; 6 B records): `sprites(ot, g, x) -> 0`,
//! a one-descriptor cousin of [`Fx::sprites`]
//! [F]. The sprite group `g` (particles at *(g+0x40): x y z, alpha +0xC,
//! frame +0xD, age +0xE; count +0x5A) drawn as screen-aligned squares
//! only: half size 20 x 0x1018E0(+0xC, w) (plus +0x50 x age, bit 0x1B, not
//! scaled by the size), every particle bobbing by +0x54 x sin(+0x58 x age)
//! (no flag), UVs of a cell (bit 0, `u + w - 1` / `v + h - 1`) or a float
//! rectangle, per particle frames unless bit 3, the group turned by the
//! angles +0x20 first (bit 0x1C); frames step every +0x5E updates over
//! +0x5C frames with no respawn. Blends 0xC / 0xB / 0xA else 0x48,
//! REGION_CLAMP (bit 0, unless 0x18), far Z (bit 0x13) or the mean w as
//! the key. A 0x230 frame: the descriptor at sp + 0x1B8 (9 regs, prim
//! 0x24).
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};
use crate::fxlib::{ALPHA_42, ALPHA_44, ALPHA_88};

/// Bits 0x14..0x17: the four vertices projected.
const PROJECTED: u32 = 0x00F0_0000;

/// The cell's integer UVs (12.4, `u + w - 1`) into the four UV slots.
fn uv_cell(fx: &mut Fx, g: u32, d: u32) {
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let (u, v, w, h) = (fx.mem.i16(g + 0x30), fx.mem.i16(g + 0x32), fx.mem.i16(g + 0x34), fx.mem.i16(g + 0x36));
    let uv = d + 0x18;
    for (k, (x, y)) in [(u, v), (u + w - 1, v), (u, v + h - 1), (u + w - 1, v + h - 1)].into_iter().enumerate() {
        let a = uv + 0x10 * k as u32;
        fx.set_uv(a, x);
        fx.set_uv(a + 2, y);
    }
}

/// The float rectangle (+0x30 s, +0x34 t, +0x38 w, +0x3C h) as ST.
fn uv_float(fx: &mut Fx, g: u32, d: u32) {
    let uv = d + 0x18;
    let (s, t) = (fx.mem.f(g + 0x30), fx.mem.f(g + 0x34));
    let sw = ee::add(fx.mem.f(g + 0x30), fx.mem.f(g + 0x38));
    let th = ee::add(fx.mem.f(g + 0x34), fx.mem.f(g + 0x3C));
    for (k, (x, y)) in [(s, t), (sw, t), (s, th), (sw, th)].into_iter().enumerate() {
        let a = uv + 0x10 * k as u32;
        fx.mem.setf(a, x);
        fx.mem.setf(a + 4, y);
    }
}

/// The particle's frame `f` into the cell origin (+0x30 / +0x32): by
/// columns (bit 1) or rows of the sheet +0x3C / +0x3E.
fn frame_cell(fx: &mut Fx, g: u32, pa: u32) {
    let flags = fx.mem.read_u32(g + 4);
    if bit(flags, 1) {
        let (w, sheet, ox) = (fx.mem.i16(g + 0x34), fx.mem.i16(g + 0x3C), fx.mem.i16(g + 0x38));
        let (cols, _) = fx.div(sheet, w);
        let f = i32::from(fx.mem.read_u8(pa + 0xD));
        let (_, col) = fx.div(f, cols);
        let wc = fx.mult(w, col);
        fx.mem.set16(g + 0x30, ox.wrapping_add(wc));
        let (sheet, w, h, oy) = (fx.mem.i16(g + 0x3C), fx.mem.i16(g + 0x34), fx.mem.i16(g + 0x36), fx.mem.i16(g + 0x3A));
        let f = i32::from(fx.mem.read_u8(pa + 0xD));
        let cols = div1(sheet, w).0;
        let row = div1(f, cols).0;
        fx.mem.set16(g + 0x32, oy.wrapping_add(h.wrapping_mul(row)));
    } else {
        let (sheet, h, w, ox) = (fx.mem.i16(g + 0x3E), fx.mem.i16(g + 0x36), fx.mem.i16(g + 0x34), fx.mem.i16(g + 0x38));
        let (rows, _) = fx.div(sheet, h);
        let f = i32::from(fx.mem.read_u8(pa + 0xD));
        let (c, _) = fx.div(f, rows);
        let wc = fx.mult(w, c);
        fx.mem.set16(g + 0x30, ox.wrapping_add(wc));
        let (h, sheet, oy) = (fx.mem.i16(g + 0x36), fx.mem.i16(g + 0x3E), fx.mem.i16(g + 0x3A));
        let f = i32::from(fx.mem.read_u8(pa + 0xD));
        let rows = div1(sheet, h).0;
        let (_, r) = fx.div(f, rows);
        fx.mem.set16(g + 0x32, oy.wrapping_add(h.wrapping_mul(r)));
    }
}

/// The particle's frame into the float rectangle (+0x30 / +0x34).
fn frame_float(fx: &mut Fx, g: u32, pa: u32) {
    let flags = fx.mem.read_u32(g + 4);
    if bit(flags, 1) {
        let f20 = fx.mem.f(g + 0x38);
        let f = i32::from(fx.mem.read_u8(pa + 0xD));
        let n = ee::fixsfsi(ee::div(1.0, f20)) as i16 as i32;
        let (_, r) = fx.div(f, n);
        fx.mem.setf(g + 0x30, ee::mul(f20, ee::from_i32(r)));
        let f = i32::from(fx.mem.read_u8(pa + 0xD));
        let n2 = ee::fixsfsi(ee::div(1.0, fx.mem.f(g + 0x38))) as i16 as i32;
        let h = fx.mem.f(g + 0x3C);
        let (q, _) = fx.div(f, n2);
        fx.mem.setf(g + 0x34, ee::mul(h, ee::from_i32(q)));
    } else {
        let f = i32::from(fx.mem.read_u8(pa + 0xD));
        let n = ee::fixsfsi(ee::div(1.0, fx.mem.f(g + 0x3C))) as i16 as i32;
        let w = fx.mem.f(g + 0x38);
        let (q, _) = fx.div(f, n);
        fx.mem.setf(g + 0x30, ee::mul(w, ee::from_i32(q)));
        let f20 = fx.mem.f(g + 0x3C);
        let f = i32::from(fx.mem.read_u8(pa + 0xD));
        let n2 = ee::fixsfsi(ee::div(1.0, f20)) as i16 as i32;
        let (_, r) = fx.div(f, n2);
        fx.mem.setf(g + 0x34, ee::mul(f20, ee::from_i32(r)));
    }
}

/// div1 (pipeline 1): quotient and remainder (by zero 0, 0).
fn div1(a: i32, b: i32) -> (i32, i32) {
    if b == 0 {
        (0, 0)
    } else {
        (a.wrapping_div(b), a.wrapping_rem(b))
    }
}

/// The routine (see the module doc).
pub fn sprites(fx: &mut Fx, ot: u32, g: u32, x: u32) -> i32 {
    let sp = fx.enter(0x230);
    let d = sp + 0x1B8;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    if fx.mem.i16(g + 0x5E) == 0 {
        fx.mem.set16(g + 0x5E, 1);
    }
    fx.alpha_for(d, g + 4, &[(0xC, ALPHA_44), (0xB, ALPHA_88), (0xA, ALPHA_42)]);
    for (o, s) in [(0u32, 0xAu32), (1, 9), (2, 8), (3, 0xB)] {
        let b = fx.mem.read_u8(g + s);
        fx.mem.write_u8(d + 0x10 + o, b);
    }
    fx.mem.write_f32(d + 0x14, 1.0);
    if bit(fx.mem.read_u32(g + 4), 3) {
        if bit(fx.mem.read_u32(g + 4), 0) {
            uv_cell(fx, g, d);
        } else {
            uv_float(fx, g, d);
        }
    }
    if bit(fx.mem.read_u32(g + 4), 0x1C) {
        let rot = [fx.mem.i32(g + 0x20), fx.mem.i32(g + 0x24), fx.mem.i32(g + 0x28)];
        let r = fx.rotation_matrix(rot);
        fx.write_m4(sp + 0x150, &r);
        let mut k = 0;
        while k < fx.mem.i16(g + 0x5A) {
            let p = fx.mem.read_u32(g + 0x40).wrapping_add((16 * k) as u32);
            for c in 0..3u32 {
                let v = fx.mem.f(p + 4 * c);
                fx.mem.setf(sp + 0x1A0 + 4 * c, v);
            }
            let vin = fx.read_v4(sp + 0x1A0);
            let o = fx.transform(Some(&r), vin);
            fx.write_v4(sp + 0x190, o);
            let p = fx.mem.read_u32(g + 0x40).wrapping_add((16 * k) as u32);
            for c in 0..3u32 {
                fx.mem.setf(p + 4 * c, o[c as usize]);
            }
            k += 1;
        }
    }
    let (s1, s2, s3, s4) = (sp + 0x1D8, sp + 0x1E8, sp + 0x1F8, sp + 0x208);
    let ws = [sp + 0x140, sp + 0x144, sp + 0x148, sp + 0x14C];
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let off = (16 * k) as u32;
        let p = fx.mem.read_u32(g + 0x40).wrapping_add(off);
        let a = fx.mem.read_u8(p + 0xC);
        let cap = fx.mem.read_u8(g + 0xB);
        fx.mem.write_u8(d + 0x13, if a < cap { a } else { cap });
        if !bit(fx.mem.read_u32(g + 4), 3) {
            let pa = fx.mem.read_u32(g + 0x40).wrapping_add(off);
            if bit(fx.mem.read_u32(g + 4), 0) {
                frame_cell(fx, g, pa);
                uv_cell(fx, g, d);
            } else {
                frame_float(fx, g, pa);
                uv_float(fx, g, d);
            }
        }
        let rh2 = fx.mem.read_u32(g);
        if bit(fx.mem.read_u32(g + 4), 0x18) {
            fx.desc_texture(d, rh2, 0);
        } else {
            if bit(fx.mem.read_u32(g + 4), 0) {
                fx.region_clamp(d, g + 0x30, g + 0x34, g + 0x32, g + 0x36);
            }
            let rh2 = fx.mem.read_u32(g);
            fx.desc_texture(d, rh2, 1);
        }
        let p = fx.mem.read_u32(g + 0x40).wrapping_add(off);
        let (px, py) = (fx.mem.f(p), fx.mem.f(p + 4));
        fx.mem.setf(sp + 0x190, px);
        fx.mem.setf(sp + 0x194, py);
        let age = fx.mem.i16(p + 0xE);
        let freq = fx.mem.i16(g + 0x58);
        let fa = fx.mult(freq, age);
        let s = fx.tsin(fa);
        let bob = ee::mul(fx.mem.f(g + 0x54), s);
        let p = fx.mem.read_u32(g + 0x40).wrapping_add(off);
        let z = ee::sub(fx.mem.f(p + 8), bob);
        fx.mem.setf(sp + 0x198, z);
        let m = fx.read_m4(x + 0x30);
        if fx.project_at(sp + 0x190, &m, s1, ws[0]) && Fx::on_screen(fx.mem.read_u64(s1)) {
            let v = fx.mem.read_u32(g + 4);
            fx.mem.write_u32(g + 4, v | PROJECTED);
        }
        let w = fx.mem.f(ws[0]);
        for &a in &ws[1..] {
            fx.mem.setf(a, w);
        }
        let w = fx.mem.f(ws[0]);
        let size = fx.mem.f(g + 0xC);
        let mut f20 = ee::mul(20.0, fx.view_scale(size, w));
        if bit(fx.mem.read_u32(g + 4), 0x1B) {
            let p = fx.mem.read_u32(g + 0x40).wrapping_add(off);
            let grow = ee::mul(fx.mem.f(g + 0x50), ee::from_i32(fx.mem.i16(p + 0xE)));
            f20 = ee::add(f20, grow);
        }
        let cx = |fx: &mut Fx| ee::from_i32(i32::from(fx.mem.read_u16(s1)));
        let cy = |fx: &mut Fx| ee::from_i32(i32::from(fx.mem.read_u16(s1 + 2)));
        let v = cx(fx);
        fx.mem.write_u16(s2, ee::fixunssfsi(ee::add(v, f20)) as u16);
        let v = cy(fx);
        fx.mem.write_u16(s2 + 2, ee::fixunssfsi(ee::sub(v, f20)) as u16);
        let v = cx(fx);
        fx.mem.write_u16(s3, ee::fixunssfsi(ee::sub(v, f20)) as u16);
        let v = cy(fx);
        fx.mem.write_u16(s3 + 2, ee::fixunssfsi(ee::add(v, f20)) as u16);
        let v = cx(fx);
        fx.mem.write_u16(s4, ee::fixunssfsi(ee::add(v, f20)) as u16);
        let v = cy(fx);
        fx.mem.write_u16(s4 + 2, ee::fixunssfsi(ee::add(v, f20)) as u16);
        let v = cx(fx);
        fx.mem.write_u16(s1, ee::fixunssfsi(ee::sub(v, f20)) as u16);
        let v = cy(fx);
        fx.mem.write_u16(s1 + 2, ee::fixunssfsi(ee::sub(v, f20)) as u16);
        let z = fx.xyz_z(s1);
        fx.set_xyz_z(s4, z);
        let z = fx.xyz_z(s4);
        fx.set_xyz_z(s3, z);
        let z = fx.xyz_z(s3);
        fx.set_xyz_z(s2, z);
        if fx.all_projected(g + 4) {
            if bit(fx.mem.read_u32(g + 4), 0x13) {
                fx.far_z(&[s1, s2, s3, s4]);
                let p = fx.mem.read_u32(g + 0x40).wrapping_add(off);
                if fx.mem.i16(p + 0xE) >= 0 {
                    fx.submit(d, ot, 0);
                }
            } else {
                let p = fx.mem.read_u32(g + 0x40).wrapping_add(off);
                if fx.mem.i16(p + 0xE) >= 0 {
                    let key = fx.mean_key4(ws);
                    fx.submit(d, ot, key);
                }
            }
        }
        fx.clear_projected(g + 4);
        if !bit(fx.mem.read_u32(g + 4), 3) {
            let p = fx.mem.read_u32(g + 0x40).wrapping_add(off);
            let age = fx.mem.i16(p + 0xE).wrapping_add(1);
            fx.mem.set16(p + 0xE, age);
            let p = fx.mem.read_u32(g + 0x40).wrapping_add(off);
            let period = fx.mem.i16(g + 0x5E);
            let age = fx.mem.i16(p + 0xE);
            let (_, r) = fx.div(age, period);
            if r == 0 {
                let f = fx.mem.read_u8(p + 0xD).wrapping_add(1);
                fx.mem.write_u8(p + 0xD, f);
                let p = fx.mem.read_u32(g + 0x40).wrapping_add(off);
                if fx.mem.i16(g + 0x5C) == i32::from(fx.mem.read_u8(p + 0xD)) {
                    fx.mem.write_u8(p + 0xD, 0);
                }
            }
        }
        k += 1;
    }
    fx.leave(0x230);
    0
}
