//! CMODULE c620 (program 0x26C, `c620.bin`), the leaders' appearance at
//! the duel's start and in the Exodia event, hand-ported from the module
//! (linked at 0x1E00000; entry 0x1E04190, data area 0x1E10000) [F]. Its
//! effect-library routines are in [`crate::fxlib`].
//!
//! Launch (`entry(data, param >= 0)`): the data area cleared (0x3740
//! bytes); the target (row << 8 | column, 0x1F6590) picks the cell; nine
//! transform records anchored on the cell's centre (0x1E2370); TEXEFF
//! record 15 (0x103260); a glow quad (+0xE0), two discs (+0x1E0, +0x260),
//! two quad groups (+0x2E0, +0x740), six rings (+0xBA0.., +0x2CE0,
//! +0x3150), a rising particle column (+0x35C0) and 256 sparkles (+0x1C10).
//! Each update (`param < 0`): the transforms rebuilt with the camera, the
//! layers drawn, alphas faded in for 20 updates then out, the column from
//! update 40; returns 2 on its 40th update (the leader shows) and 8 when
//! the countdown of 72 ends [emu: `modes/older/c620/run1.jsonl`].
use crate::bus::{Bus, Image};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::types::{EffectTextures, EffectUpdate, EffectWorld};
use crate::EffectPort;
use std::sync::Arc;

/// CMODULE program id.
pub const ID: u16 = 0x26C;
/// Its entry in the type-0 table 0x2B8510.
pub const ENTRY: u32 = 0x01E0_4190;
/// The data area of type-0 task slot 0.
const D: u32 = 0x01E1_0000;
/// The quad template the record keeps at 0x1E04FC0 (rodata).
const QUAD_TEMPLATE: u32 = 0x01E0_4FC0;
/// The glow quad's corner template 0x1E04F80 (rodata; its xyz are then
/// overwritten).
const GLOW_TEMPLATE: u32 = 0x01E0_4F80;

/// A running c620.
pub struct C620 {
    fx: Fx,
}

impl C620 {
    /// The program from the user's executable and CMODULE.MRG (`None`: no
    /// record 620). `handles`: (self, target) as the executor sets them; the
    /// target is the leader's cell (row << 8 | column).
    pub fn load(executable: &[u8], cmodule: &[u8], handles: [u16; 2], textures: &EffectTextures) -> Option<Self> {
        let image = Arc::new(Image::from_elf(executable)?);
        Self::load_with(image, cmodule, handles, textures)
    }

    /// As [`Self::load`] with the executable's image already parsed.
    pub fn load_with(image: Arc<Image>, cmodule: &[u8], handles: [u16; 2], textures: &EffectTextures) -> Option<Self> {
        let start = usize::from(ID) * 0x8000;
        let record = cmodule.get(start..start + 0x8000)?;
        if record.get(..4) != Some(b"MWo3") {
            return None;
        }
        let mut fx = Fx::new(image, record, ID, handles, textures);
        fx.mem.write_u32(crate::world::LINKED_TYPE, 0);
        Some(Self { fx })
    }

    /// The launch: 0x1E04190 with `param` >= 0.
    fn init(&mut self) -> i32 {
        let fx = &mut self.fx;
        let sp = fx.enter(0xC0);
        fx.bzero(D, 0x3740);
        fx.mem.set32(D + 0x3718, 0x48);
        fx.mem.set32(D + 0x371C, 1);
        fx.mem.set32(D + 0x3720, 0);
        fx.mem.set32(D + 0x372C, 0x80);
        let t = fx.mem.i32(crate::exe::TARGET_HANDLE);
        fx.mem.set32(D + 0x3730, t & 0xFF);
        fx.mem.set32(D + 0x3734, t >> 8);
        fx.mem.set32(D + 0x3710, t);
        for x in [0u32, 0x70, 0x1BA0, 0x270, 0x2C70, 0x30E0, 0x3550, 0x170, 0x1F0] {
            fx.xform_init(D + x);
        }
        for at in [0x90u32, 0x1BC0, 0x290, 0x2C90, 0x3100, 0x3570, 0x190, 0x210] {
            let (col, row) = (fx.mem.i32(D + 0x3730), fx.mem.i32(D + 0x3734));
            let v = Fx::cell_position(col, row, 0);
            fx.write_v4(D + at, v);
        }
        for k in 0..3 {
            fx.mem.write_u8(D + 0x3714 + k, 0x80);
        }
        fx.mem.write_u8(D + 0x3717, 0xFF);
        // The glow quad (+0xE0): no flags, colour (0xE0, 0xB0, 0x70) alpha 0.
        fx.mem.set32(D + 0xE4, 0);
        colour(fx, D + 0xE8, 0);
        for k in 0..4u32 {
            let v = fx.read_v4(GLOW_TEMPLATE + 16 * k);
            fx.write_v4(sp + 0x80 + 16 * k, v);
        }
        let (n, p) = (f32::from_bits(0xC185_47AE), f32::from_bits(0x4185_47AE));
        for (k, (x, y, z)) in [(n, 0.0, p), (p, 0.0, p), (p, -0.0, n), (n, -0.0, n)].into_iter().enumerate() {
            let a = sp + 0x80 + 16 * k as u32;
            fx.mem.setf(a, x);
            fx.mem.setf(a + 4, y);
            fx.mem.setf(a + 8, z);
        }
        for k in 0..4u32 {
            for c in 0..3u32 {
                let v = fx.mem.f(sp + 0x80 + 16 * k + 4 * c);
                fx.mem.setf(D + 0xF0 + 16 * k + 4 * c, v);
            }
        }
        // Discs: +0x1E0 (flags +0, radius 50, 36 triangles), +0x260 (far
        // Z, radius 20).
        fx.mem.set32(D + 0x1E0, 0);
        colour(fx, D + 0x1E4, 0);
        fx.mem.setf(D + 0x1E8, 50.0);
        fx.mem.set16(D + 0x1EC, 0x24);
        fx.mem.set16(D + 0x1EE, 0);
        fx.mem.set32(D + 0x260, 0x8_0000);
        colour(fx, D + 0x264, 0);
        fx.mem.setf(D + 0x268, 20.0);
        fx.mem.set16(D + 0x26C, 0x24);
        fx.mem.set16(D + 0x26E, 0);
        let tex = fx.texture(4, 15);
        fx.mem.write_u32(sp + 0x70, tex);
        // Two quad groups (+0x2E0, +0x740): FST cell (142, 0, 34, 64).
        for s in 0..2u32 {
            let g = D + 0x460 * s + 0x2E0;
            fx.mem.set32(g + 4, 0x4_0001);
            rect(fx, g + 0x10, 0x8E, 0, 0x22, 0x40);
            colour(fx, g + 8, 0);
            let tex = fx.mem.read_u32(sp + 0x70);
            fx.quads_setup(g, tex, 2.0, QUAD_TEMPLATE);
            for k in 0..64u32 {
                let q = D + 0x460 * s + 16 * k;
                fx.mem.set32(q + 0x340, 0x4000);
                let r = fx.rand() & 0xFFFF;
                fx.mem.set32(q + 0x344, r);
                fx.mem.set32(q + 0x348, 0);
            }
        }
        // Rings.
        fx.mem.set32(D + 0x2CE4, 0);
        rect(fx, D + 0x2CF0, 0x8E, 0, 0x20, 0x40);
        colour(fx, D + 0x2CE8, 0);
        let tex = fx.mem.read_u32(sp + 0x70);
        fx.ring_setup(D + 0x2CE0, tex, 10.0, 10.0, 0.0, 100.0);
        fx.mem.set32(D + 0x3154, 0);
        rect(fx, D + 0x3160, 0x8E, 0, 0x20, 0x80);
        colour(fx, D + 0x3158, 0x20);
        let tex = fx.mem.read_u32(sp + 0x70);
        fx.ring_setup(D + 0x3150, tex, 3.0, 3.0, -20.0, 40.0);
        for s in 0..4u32 {
            let g = D + 0x400 * s + 0xBA0;
            fx.mem.set32(g + 4, 0);
            rect(fx, g + 0x10, 0x8E, 0, 0x20, 0x80);
            colour(fx, g + 8, 0);
            let r = ee::mul(6.0, ee::from_i32(s as i32 + 1));
            let tex = fx.mem.read_u32(sp + 0x70);
            fx.ring_setup(g, tex, r, ee::sub(r, 1.0), 0.0, 0.0);
        }
        // The particle column (+0x35C0): 16 particles, frames 8 x 8 cells of
        // 10 x 10 from (0, 228) of a 256 x 256 sheet.
        let tex = fx.mem.read_u32(sp + 0x70);
        fx.mem.write_u32(D + 0x35C0, tex);
        fx.mem.set32(D + 0x35C4, 0x8_0005);
        fx.mem.set32(D + 0x35D0, 0);
        fx.mem.set32(D + 0x35D4, 0);
        fx.mem.set32(D + 0x35D8, 0);
        for (o, v) in [(0x35E0u32, 0), (0x35E2, 0xE4), (0x35E4, 0xA), (0x35E6, 0xA), (0x35E8, 0), (0x35EA, 0xE4), (0x35EC, 0x100), (0x35EE, 0x100)] {
            fx.mem.set16(D + o, v);
        }
        fx.mem.setf(D + 0x35CC, f32::from_bits(0x3F4C_CCCD));
        fx.mem.set16(D + 0x36F0, 0x10);
        fx.mem.set16(D + 0x36FA, 1);
        fx.mem.set16(D + 0x36FC, 0);
        fx.mem.set16(D + 0x3700, 2);
        fx.mem.set16(D + 0x36F6, 0xA);
        fx.mem.set16(D + 0x36F8, 0xA);
        colour(fx, D + 0x35C8, 0x80);
        for k in 0..8u32 {
            let p = D + 16 * k;
            let r = fx.rand();
            let (_, m) = fx.div(r, 20);
            fx.mem.setf(p + 0x35F0, ee::from_i32(m - 10));
            let r = fx.rand();
            let (_, m) = fx.div(r, 20);
            fx.mem.setf(p + 0x35F4, ee::from_i32(m - 10));
            let t = fx.mem.i16(D + 0x36FA).wrapping_neg().wrapping_mul(k as i32);
            fx.mem.set16(p + 0x35FE, t);
            fx.mem.set16(p + 0x35FC, 0);
            let back = fx.mem.i16(D + 0x3700);
            fx.mem.set16(D + 0x36FE, back - 1);
        }
        // 256 sparkles (+0x1C10) at *(+0x1C50) = +0x1C70.
        let tex = fx.mem.read_u32(sp + 0x70);
        fx.mem.write_u32(D + 0x1C10, tex);
        fx.mem.write_u32(D + 0x1C14, 0x5000_0003);
        colour(fx, D + 0x1C18, 0x80);
        fx.mem.setf(D + 0x1C1C, 0.5);
        fx.mem.set32(D + 0x1C30, 0);
        fx.mem.set32(D + 0x1C34, 0x400);
        fx.mem.set32(D + 0x1C38, 0);
        for (o, v) in [(0x1C40u32, 0xB0), (0x1C42, 0x60), (0x1C44, 0x20), (0x1C46, 0x20), (0x1C48, 0xB0), (0x1C4A, 0x60), (0x1C4C, 0x80), (0x1C4E, 0x40)] {
            fx.mem.set16(D + o, v);
        }
        fx.mem.write_u32(D + 0x1C50, D + 0x1C70);
        fx.mem.set32(D + 0x1C54, 0);
        fx.mem.set32(D + 0x1C58, 0);
        fx.mem.setf(D + 0x1C5C, 1.0);
        fx.mem.set16(D + 0x1C6A, 0x100);
        fx.mem.set16(D + 0x1C6C, 2);
        fx.mem.set16(D + 0x1C6E, 0);
        fx.sprites_spawn(D + 0x1C10, 1.0);
        let mut k = 0;
        while k < fx.mem.i16(D + 0x1C6A) {
            let p = D + 16 * k as u32;
            let r = fx.rand();
            let (_, m) = fx.div(r, 0x22);
            fx.mem.set16(p + 0x1C7E, m.wrapping_neg());
            fx.mem.write_u8(p + 0x1C7C, 0x80);
            fx.mem.write_u8(p + 0x1C7D, u8::from(k & 1 != 0));
            let y = ee::sub(fx.mem.f(p + 0x1C74), 50.0);
            fx.mem.setf(p + 0x1C74, y);
            k += 1;
        }
        for at in [0x194u32, 0x294, 0x94] {
            fx.mem.setf(D + at, -1.0);
        }
        fx.mem.set32(D + 0x180, 0x4000);
        for at in [0x3104u32, 0x3574, 0x1BC4] {
            fx.mem.setf(D + at, 50.0);
        }
        for at in [0x2C78u32, 0x2C70, 0x1F8, 0x1F4, 0x1F0] {
            fx.mem.setf(D + at, f32::from_bits(0x3DCC_CCCD));
        }
        fx.leave(0xC0);
        0
    }

    /// An update: 0x1E04190 with `param` < 0 (0x1E048FC..).
    fn step(&mut self) -> i32 {
        let fx = &mut self.fx;
        let sp = fx.enter(0xC0);
        let r = fx.mem.i32(D + 0x2C84).wrapping_add(0x400);
        fx.mem.set32(D + 0x2C84, r);
        for x in [0u32, 0x70, 0x270, 0x1BA0, 0x2C70, 0x30E0, 0x170] {
            fx.xform_world(D + x);
        }
        fx.xform_billboard(D + 0x3550);
        fx.xform_billboard(D + 0x1F0);
        fx.disc(D + 0x1E0, D + 0x170);
        glow(fx, D + 0xE0, D + 0x70);
        for s in 0..2u32 {
            let g = D + 0x460 * s;
            fx.quads_draw(g + 0x2E0, D + 0x270);
            for k in 0..64u32 {
                let a = g + 0x344 + 16 * k;
                let v = fx.mem.i32(a);
                fx.mem.set32(a, if s == 0 { v.wrapping_add(0x20) } else { v.wrapping_sub(0x20) });
            }
        }
        for s in 0..4u32 {
            fx.ring_draw(D + 0x400 * s + 0xBA0, D + 0x270);
        }
        if fx.mem.i32(D + 0x371C) >= 20 {
            fx.step_down(D + 0x1E7, 0, 8);
            fx.step_down(D + 0xEB, 0, 0x20);
            for s in 0..2u32 {
                fx.step_down(D + 0x460 * s + 0x2EB, 0, 2);
            }
            for s in 0..4u32 {
                fx.step_down(D + 0x400 * s + 0xBAB, 0, 4);
            }
            fx.approach(D + 0x270, 4.0, f32::from_bits(0x3D4C_CCCD));
            fx.approach(D + 0x170, 4.0, f32::from_bits(0x3D4C_CCCD));
        } else {
            fx.step_up(D + 0x1E7, 0xFF, 0x10);
            fx.step_up(D + 0xEB, 0xFF, 0x20);
            for s in 0..2u32 {
                fx.step_up(D + 0x460 * s + 0x2EB, 0x20, 4);
            }
            for s in 0..4u32 {
                fx.step_up(D + 0x400 * s + 0xBAB, 0x40, 4);
            }
        }
        if fx.mem.i32(D + 0x371C) >= 40 {
            fx.mem.set32(sp + 0x68, 0);
            fx.mem.set32(sp + 0x64, 0);
            fx.mem.set32(sp + 0x60, 0);
            fx.particle_quads(D + 0x3550, D + 0x35C0, sp + 0x60);
            fx.step_down(D + 0x35CB, 0, 4);
        }
        if fx.mem.i32(D + 0x371C) >= 20 {
            fx.disc(D + 0x260, D + 0x1F0);
            let step = f32::from_bits(0x3D4C_CCCD);
            let grow = |fx: &mut Fx, ats: &[u32]| {
                for &a in ats {
                    let v = ee::add(fx.mem.f(D + a), step);
                    fx.mem.setf(D + a, v);
                }
            };
            if crate::exe::ee_lt(fx.mem.f(D + 0x1F0), 1.0) {
                if fx.step_up(D + 0x267, 0xFF, 0x10) == 0 {
                    grow(fx, &[0x1F0, 0x1F8, 0x1F4]);
                }
            } else {
                fx.step_down(D + 0x267, 0, 0x10);
                grow(fx, &[0x1F0, 0x1F8, 0x1F4]);
            }
            fx.ring_draw(D + 0x2CE0, D + 0x2C70);
            if crate::exe::ee_lt(fx.mem.f(D + 0x2C70), 1.0) {
                if fx.step_up(D + 0x2CEB, 0x20, 2) == 0 {
                    grow(fx, &[0x2C70, 0x2C78]);
                }
            } else {
                fx.step_down(D + 0x2CEB, 0, 2);
                grow(fx, &[0x2C70, 0x2C78]);
                fx.mem.setf(D + 0x2C74, 1.0);
            }
            fx.sprites(D + 0x1C10, D + 0x1BA0);
            if fx.mem.i32(D + 0x371C) >= 40 {
                fx.step_down(D + 0x1C1B, 0, 8);
            }
            let mut k = 0;
            while k < fx.mem.i16(D + 0x1C6A) {
                let p = D + 16 * k as u32;
                if fx.mem.i16(p + 0x1C7E) >= 0 {
                    let (_, m) = fx.div(k, 3);
                    let dy = match m {
                        0 => 1.0,
                        1 => 2.0,
                        _ => 3.0,
                    };
                    let y = ee::add(fx.mem.f(p + 0x1C74), dy);
                    fx.mem.setf(p + 0x1C74, y);
                    if !crate::exe::ee_le(fx.mem.f(p + 0x1C74), 0.0) {
                        fx.step_down(p + 0x1C7C, 0, 8);
                    }
                    if !crate::exe::ee_le(fx.mem.f(p + 0x1C74), 50.0) {
                        let a = fx.rand() & 0xFFFF;
                        let c = fx.tcos(a);
                        let z = ee::mul(fx.mem.f(D + 0x1C5C), c);
                        fx.mem.setf(p + 0x1C78, z);
                        let s = fx.tsin(a);
                        let x = ee::mul(fx.mem.f(D + 0x1C5C), s);
                        fx.mem.setf(p + 0x1C70, x);
                        fx.mem.setf(p + 0x1C74, -50.0);
                        fx.mem.write_u8(p + 0x1C7C, 0x80);
                    }
                }
                k += 1;
            }
        }
        let n = fx.mem.i32(D + 0x371C) + 1;
        fx.mem.set32(D + 0x371C, n);
        let left = fx.mem.i32(D + 0x3718) - 1;
        fx.mem.set32(D + 0x3718, left);
        fx.leave(0xC0);
        if n == 40 {
            2
        } else if left > 0 {
            0
        } else {
            8
        }
    }
}

/// Colour bytes (B 0xE0, G 0xB0, R 0x70, A `a`) at `at`.
fn colour(fx: &mut Fx, at: u32, a: u8) {
    fx.mem.write_u8(at + 2, 0x70);
    fx.mem.write_u8(at + 1, 0xB0);
    fx.mem.write_u8(at, 0xE0);
    fx.mem.write_u8(at + 3, a);
}

/// An i16 rectangle (x, y, w, h).
fn rect(fx: &mut Fx, at: u32, x: i32, y: i32, w: i32, h: i32) {
    fx.mem.set16(at, x);
    fx.mem.set16(at + 2, y);
    fx.mem.set16(at + 4, w);
    fx.mem.set16(at + 6, h);
}

/// c620 0x1E00120 (2 records: c620 and one more C module): the glow quad
/// `g` (corners +0x10 + 16 k) drawn as four gouraud strips from each
/// corner pushed out by 1/6 to the corner itself, alpha +0xB at the outer
/// edge (bit 0xC: random colours inside).
fn glow(fx: &mut Fx, g: u32, x: u32) {
    let sp = fx.enter(0x120);
    let d = sp + 0xC8;
    fx.desc_init(d, 8, 0x14, crate::fxlib::REGS_STRIP4);
    let flags = fx.mem.read_u32(g + 4);
    let a = if crate::exe::bit(flags, 0xB) {
        crate::fxlib::ALPHA_88
    } else if crate::exe::bit(flags, 0xA) {
        crate::fxlib::ALPHA_42
    } else {
        crate::fxlib::ALPHA_ADD
    };
    fx.desc_alpha(d, a);
    for base in [0xD8u32, 0xE8] {
        fx.mem.write_u8(sp + base, 0);
        fx.mem.write_u8(sp + base + 1, 0);
        fx.mem.write_u8(sp + base + 2, 0);
        let ga = fx.mem.read_u8(g + 0xB);
        fx.mem.write_u8(sp + base + 3, ga);
        fx.mem.setf(sp + base + 4, 1.0);
    }
    if crate::exe::bit(fx.mem.read_u32(g + 4), 0xC) {
        for base in [0xF8u32, 0x108] {
            for c in 0..3 {
                let r = (fx.rand() & 0x7F) as u8;
                fx.mem.write_u8(sp + base + c, r);
            }
            let ga = fx.mem.read_u8(g + 0xB);
            fx.mem.write_u8(sp + base + 3, ga);
            fx.mem.setf(sp + base + 4, 1.0);
        }
    } else {
        for base in [0xF8u32, 0x108] {
            let (ca, c9, c8, cb) = (fx.mem.read_u8(g + 0xA), fx.mem.read_u8(g + 9), fx.mem.read_u8(g + 8), fx.mem.read_u8(g + 0xB));
            fx.mem.write_u8(sp + base, ca);
            fx.mem.write_u8(sp + base + 1, c9);
            fx.mem.write_u8(sp + base + 2, c8);
            fx.mem.write_u8(sp + base + 3, cb);
            fx.mem.setf(sp + base + 4, 1.0);
        }
    }
    let m = fx.read_m4(x + 0x30);
    let xyz = [sp + 0xE0, sp + 0xF0, sp + 0x100, sp + 0x110];
    let ws = [sp + 0xA0, sp + 0xA4, sp + 0xA8, sp + 0xAC];
    let v = sp + 0xB0;
    for k in (0..4i32).rev() {
        let prev = if k == 0 { 3 } else { k - 1 };
        let pk = g + 0x10 + 16 * k as u32;
        let pp = g + 0x10 + 16 * prev as u32;
        for (i, (p, out)) in [(pk, true), (pp, true), (pk, false), (pp, false)].into_iter().enumerate() {
            for c in 0..3u32 {
                let a = fx.mem.f(p + 4 * c);
                let val = if out { ee::add(a, ee::div(a, 6.0)) } else { a };
                fx.mem.setf(v + 4 * c, val);
            }
            if fx.project_at(v, &m, xyz[i], ws[i]) {
                let f = fx.mem.read_u32(g + 4);
                fx.mem.write_u32(g + 4, f | (0x10_0000 << i));
            }
        }
        let f = fx.mem.read_u32(g + 4);
        if f & 0xF0_0000 == 0xF0_0000 {
            if crate::exe::bit(f, 0x13) {
                let mut z = 0xFF_FFFF;
                for &a in xyz.iter().rev() {
                    fx.set_xyz_z(a, z);
                    z = fx.xyz_z(a);
                }
                fx.submit(d, OT, 0);
            } else {
                let (w0, w1, w2, w3) = (fx.mem.f(ws[0]), fx.mem.f(ws[1]), fx.mem.f(ws[2]), fx.mem.f(ws[3]));
                let s = ee::add(w3, ee::add(w2, ee::add(w0, w1)));
                let key = ee::fixunssfsi(ee::div(s, 4.0)) as i32;
                fx.submit(d, OT, key);
            }
        }
        let f = fx.mem.read_u32(g + 4);
        fx.mem.write_u32(g + 4, f & 0xFF0F_FFFF);
    }
    fx.leave(0x120);
}

impl EffectPort for C620 {
    fn update(&mut self, world: &dyn EffectWorld) -> EffectUpdate {
        if self.fx.finished {
            return EffectUpdate { ret: 8, ..Default::default() };
        }
        self.fx.begin(world);
        let ret = if self.fx.calls == 0 { self.init() } else { self.step() };
        self.fx.end(ret)
    }
    fn calls(&self) -> usize {
        self.fx.calls
    }
    fn finished(&self) -> bool {
        self.fx.finished
    }
    fn rand_state(&mut self) -> u64 {
        self.fx.rand_state()
    }
    fn set_rand_state(&mut self, state: u64) {
        self.fx.set_rand_state(state);
    }
    fn bus(&mut self) -> Option<&mut dyn Bus> {
        Some(&mut self.fx.mem)
    }
}
