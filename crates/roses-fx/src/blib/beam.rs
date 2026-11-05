//! Beads along a path of the effect library: textured sprites (or
//! stretched segments) placed by a table or a spiral from a record's
//! origin.
use super::exe::{atlas_cell, camera_vp_into, countdown, cvt_u32, libm_cosf, mul_mem, node_source, rot_trans_into, tex_rect};
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx};

const PI: u32 = 0x4049_0FDB;
const ASPECT: u32 = 0x3FAA_3D71;

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v) as i32;
    fx.mem.set16(a, x);
}

/// 0x20C850(v): |v.xyz| (libm sqrtf of ((x x + y y) + z z)).
pub fn length3(fx: &mut Fx, v: u32) -> f32 {
    let (x, y, z) = (fx.mem.f(v), fx.mem.f(v + 4), fx.mem.f(v + 8));
    let acc = ee::add(ee::mul(x, x), ee::mul(y, y));
    fx.sqrtf(ee::add(acc, ee::mul(z, z)))
}

/// f83869e0dfef [51 B records; b059 0x1E00410, 884 words] [F]:
/// `beads(g, ot, n) -> 1` steps and draws `n` records (0x58 bytes from
/// `g`; a 0x280 frame, the descriptor at sp + 0x200). Four sprite specs
/// (0x60 bytes, record-relative `lui` / `addiu` at +0x48 / +0x4C) are
/// copied to sp + 0x180; flag 4 uses an own-slot texture +0xA with atlas
/// cell +0xB (flag 8: from the length left, the last 4 counts counting
/// down) of layout 0 and blend +0xC, else spec +0xA. Per record: countdown,
/// origin +0x28 += +0x38, the origin from a node, the matrix camera x
/// (angles +0x48, origin) as `ribbon`; colour +0x24..+0x26, alpha +0x27
/// faded over the first 8 counts; then +8 beads along the path (flag 1:
/// table deltas x +0x14 / 255; else a 17-degree spiral, radius table x
/// +0x18 / 255 scaled by the next table byte / 255, height that x +0x14)
/// scaled by the length left; size +0x1C + +0x20 x left, divided by (1 +
/// |step| / (+0x14 / +0xD)) when +0xD >= 2 (0x20C850). Flag 2: a segment
/// to 4 steps further (two projections, widened across the major axis);
/// else a billboard (y x 1.33). Key: the bead's depth.
pub fn beads(fx: &mut Fx, at: u32, g: u32, ot: u32, n: i32) -> i32 {
    beads_a3(fx, at, g, ot, n).0
}

/// [`beads`] and the a3 it leaves (helper B10, for a later boundary call
/// that reports it): the specs' address, then per drawing record the last
/// bead's: the projection's depth pointer sp + 0x278 (+ 0x27C for a
/// segment's second end) when it failed or fell outside the window, the
/// z mask 0xFFFFFFFF (low half of 0xFF000000_FFFFFFFF) when drawn [F
/// 0x204D80 / 0x101600 / 0x1018E0 leave a3 with a matrix given; run b626].
/// [H] a record with no beads (+8 = 0) leaves what its tex_rect 0x1020B0
/// / 0x101D20 left; not modelled (the previous value is kept).
pub fn beads_a3(fx: &mut Fx, at: u32, g: u32, ot: u32, n: i32) -> (i32, u32) {
    let src = rel(fx, at, 0x48, 0x4C);
    let mut a3 = src;
    let r = beads_body(fx, src, g, ot, n, &mut a3);
    (r, a3)
}

/// ad08bbed1db5 [12 B records; b110 0x1E02F40, 800 words] and
/// 4c6a2da07857 [11 B records; b012 0x1E05050, 964 words]
/// [F]: the same routine compiled differently (counters in registers;
/// 4c6a's 0x240 frame puts the locals at [`beads`]' addresses, sp + 0x40
/// = its sp): the specs' `lui` / `addiu` at `hi` / `lo` (+0x34 / +0x3C,
/// +0x40 / +0x44).
pub fn beads_at(fx: &mut Fx, at: u32, hi: u32, lo: u32, g: u32, ot: u32, n: i32) -> i32 {
    let src = rel(fx, at, hi, lo);
    let mut a3 = src;
    beads_body(fx, src, g, ot, n, &mut a3)
}

fn beads_body(fx: &mut Fx, src: u32, g: u32, ot: u32, n: i32, a3: &mut u32) -> i32 {
    let sp = fx.enter(0x280);
    fx.mem.write_u32(sp + 0xC8, ot);
    fx.mem.set32(sp + 0xC4, n);
    for k in (0..0x60u32).step_by(8) {
        let v = fx.mem.read_u64((src & !15) + k);
        fx.mem.write_u64(((sp + 0x180) & !15) + k, v);
    }
    let mut g = g;
    let mut i = 0;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < n {
        if i != 0 {
            g += 0x58;
        }
        one(fx, sp, g, a3);
        i += 1;
        fx.mem.set32(sp + 0xB0, i);
    }
    fx.leave(0x280);
    1
}

fn one(fx: &mut Fx, sp: u32, g: u32, a3: &mut u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let tex = if fx.mem.u8(g + 4) & 4 != 0 {
        let id = fx.mem.u8(g + 0xA);
        fx.texture(5, id)
    } else {
        let s = sp + 0x180 + 0x18 * fx.mem.u8(g + 0xA) as u32;
        let (kind, id) = (fx.mem.i32(s), fx.mem.u8(s + 4));
        fx.texture(kind, id)
    };
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(g + 0x28 + 4 * k), fx.mem.f(g + 0x38 + 4 * k));
        fx.mem.setf(g + 0x28 + 4 * k, v);
    }
    node_source(fx, g, g + 0x28);
    camera_vp_into(fx, sp + 0x110);
    rot_trans_into(fx, sp + 0xD0, g + 0x48, g + 0x28);
    mul_mem(fx, sp + 0x110, sp + 0xD0);
    let mut step = fx.mem.u8(g + 9);
    let mut turn = 0.0f32;
    let left = fx.mem.u8(g + 2) - fx.mem.u8(g);
    let d = sp + 0x200;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (src, dst) in [(0x26u32, 0x210u32), (0x25, 0x211), (0x24, 0x212)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + dst, v);
    }
    fx.mem.write_u32(sp + 0x214, 0x3F80_0000);
    let f = fx.mem.u8(g + 4);
    if f & 4 != 0 {
        let mut cell = fx.mem.u8(g + 0xB);
        if f & 8 != 0 {
            cell = left & 7;
            if left < 3 {
                cell = left;
            }
            let c = fx.mem.u8(g);
            if c < 4 {
                cell = 7 - c;
            }
        }
        let b = fx.mem.read_u8(g + 0xC);
        fx.mem.write_u8(sp + 0x1E5, b);
        atlas_cell(fx, sp + 0x1E0, cell, 0);
        tex_rect(fx, d, sp + 0x1E0, tex);
    } else {
        let s = sp + 0x180 + 0x18 * fx.mem.u8(g + 0xA) as u32;
        tex_rect(fx, d, s, tex);
    }
    let c = fx.mem.u8(g);
    if c < 8 {
        let k = fx.mem.u8(g + 0x27) >> 3;
        let v = fx.mult(k, c);
        fx.mem.set8(sp + 0x213, v);
    } else {
        let v = fx.mem.read_u8(g + 0x27);
        fx.mem.write_u8(sp + 0x213, v);
    }
    let mut s = 0;
    while s < fx.mem.u8(g + 8) {
        if fx.mem.u8(g + 4) & 1 != 0 {
            let t = fx.mem.read_u32(g + 0x10).wrapping_add((step & 0xFF) as u32);
            for (k, (a, b)) in [(0u32, 1u32), (2, 3), (4, 5)].into_iter().enumerate() {
                let dlt = fx.mem.u8(t + a) - fx.mem.u8(t + b);
                let v = ee::div(ee::mul(fx.mem.f(g + 0x14), ee::from_i32(dlt)), 255.0);
                fx.mem.setf(sp + 0x170 + 4 * k as u32, v);
            }
            step = (step + 6) & 0xFF;
        } else {
            turn = ee::add(turn, 17.0);
            if !ee_le(turn, 360.0) {
                turn = ee::sub(turn, 360.0);
            }
            let t = fx.mem.read_u32(g + 0x10);
            let a = cvt_u32(fx.mem.read_u8(t.wrapping_add((step & 0xFF) as u32)) as u32);
            let h = ee::div(a, 255.0);
            let b = cvt_u32(fx.mem.read_u8(t.wrapping_add(((step + 1) & 0xFF) as u32)) as u32);
            let r = ee::div(ee::mul(fx.mem.f(g + 0x18), b), 255.0);
            step = (step + 2) & 0xFF;
            let ang = ee::div(ee::mul(f32::from_bits(PI), turn), 180.0);
            let cs = libm_cosf(fx, ang);
            let r = ee::mul(h, r);
            fx.mem.setf(sp + 0x170, ee::mul(r, cs));
            let sn = fx.libm_sinf(ang);
            fx.mem.setf(sp + 0x178, ee::mul(r, sn));
            let v = ee::mul(h, fx.mem.f(g + 0x14));
            fx.mem.setf(sp + 0x174, v);
        }
        let mut size = ee::add(fx.mem.f(g + 0x1C), ee::mul(fx.mem.f(g + 0x20), ee::from_i32(left)));
        let beads = fx.mem.u8(g + 0xD);
        if beads >= 2 {
            let unit = ee::div(fx.mem.f(g + 0x14), ee::from_i32(beads));
            let len = length3(fx, sp + 0x170);
            size = ee::div(size, ee::add(1.0, ee::div(len, unit)));
        }
        let l = ee::from_i32(left);
        for k in 0..3u32 {
            let v = ee::mul(fx.mem.f(sp + 0x170 + 4 * k), l);
            fx.mem.setf(sp + 0x150 + 4 * k, v);
        }
        let m = fx.read_m4(sp + 0x110);
        *a3 = sp + 0x278;
        if fx.project_at(sp + 0x150, &m, sp + 0x220, sp + 0x278) && Fx::on_screen(fx.mem.read_u64(sp + 0x220)) {
            bead(fx, sp, g, size, a3);
        }
        s += 1;
    }
}

fn bead(fx: &mut Fx, sp: u32, g: u32, size: f32, a3: &mut u32) {
    let w = fx.mem.f(sp + 0x278);
    let half = fx.view_scale(size, w);
    let f = |v: i32| ee::from_i32(v);
    if fx.mem.u8(g + 4) & 2 != 0 {
        for k in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0x150 + 4 * k), ee::mul(4.0, fx.mem.f(sp + 0x170 + 4 * k)));
            fx.mem.setf(sp + 0x160 + 4 * k, v);
        }
        let m = fx.read_m4(sp + 0x110);
        *a3 = sp + 0x27C;
        if !fx.project_at(sp + 0x160, &m, sp + 0x240, sp + 0x27C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x240)) {
            return;
        }
        let w2 = fx.mem.f(sp + 0x27C);
        let half2 = fx.view_scale(size, w2);
        let (y1, y0) = (fx.mem.u16(sp + 0x242), fx.mem.u16(sp + 0x222));
        let dy = (y1 - y0).abs();
        let (x1, x0) = (fx.mem.u16(sp + 0x240), fx.mem.u16(sp + 0x220));
        let dx = (x1 - x0).abs();
        if dy < dx {
            fx.mem.set16(sp + 0x230, x0);
            let v = fx.mem.u16(sp + 0x240);
            fx.mem.set16(sp + 0x250, v);
            let y = fx.mem.u16(sp + 0x222);
            fix16(fx, sp + 0x232, ee::add(f(y), half));
            let y = fx.mem.u16(sp + 0x222);
            fix16(fx, sp + 0x222, ee::sub(f(y), half));
            let y = fx.mem.u16(sp + 0x242);
            fix16(fx, sp + 0x252, ee::add(f(y), half2));
            let y = fx.mem.u16(sp + 0x242);
            fix16(fx, sp + 0x242, ee::sub(f(y), half2));
        } else {
            fx.mem.set16(sp + 0x232, y0);
            let v = fx.mem.u16(sp + 0x242);
            fx.mem.set16(sp + 0x252, v);
            let x = fx.mem.u16(sp + 0x220);
            fix16(fx, sp + 0x230, ee::add(f(x), half));
            let x = fx.mem.u16(sp + 0x220);
            fix16(fx, sp + 0x220, ee::sub(f(x), half));
            let x = fx.mem.u16(sp + 0x240);
            fix16(fx, sp + 0x250, ee::add(f(x), half2));
            let x = fx.mem.u16(sp + 0x240);
            fix16(fx, sp + 0x240, ee::sub(f(x), half2));
        }
        let z = fx.xyz_z(sp + 0x220);
        fx.set_xyz_z(sp + 0x230, z);
        let z = fx.xyz_z(sp + 0x240);
        fx.set_xyz_z(sp + 0x250, z);
    } else {
        let x = fx.mem.u16(sp + 0x220);
        fix16(fx, sp + 0x230, ee::add(f(x), half));
        let hy = ee::mul(f32::from_bits(ASPECT), half);
        let y = fx.mem.u16(sp + 0x222);
        fix16(fx, sp + 0x232, ee::sub(f(y), hy));
        fix16(fx, sp + 0x240, ee::sub(f(x), half));
        fix16(fx, sp + 0x242, ee::add(f(y), hy));
        fix16(fx, sp + 0x250, ee::add(f(x), half));
        fix16(fx, sp + 0x252, ee::add(f(y), hy));
        fix16(fx, sp + 0x220, ee::sub(f(x), half));
        fix16(fx, sp + 0x222, ee::sub(f(y), hy));
        let z = fx.xyz_z(sp + 0x220);
        fx.set_xyz_z(sp + 0x250, z);
        let z = fx.xyz_z(sp + 0x250);
        fx.set_xyz_z(sp + 0x240, z);
        let z = fx.xyz_z(sp + 0x240);
        fx.set_xyz_z(sp + 0x230, z);
    }
    for o in [0x257u32, 0x247, 0x237, 0x227] {
        fx.mem.write_u8(sp + o, 1);
    }
    *a3 = 0xFFFF_FFFF;
    let key = ee::fixunssfsi(fx.mem.f(sp + 0x278)) as i32;
    let ot = fx.mem.read_u32(sp + 0xC8);
    fx.submit(sp + 0x200, ot, key);
}
