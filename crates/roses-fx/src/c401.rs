//! CMODULE c401 (program 0x191), the Stonehenge / Simon story light: a
//! full-screen additive flash, a scene light and 32 textured rays, hand-
//! ported from the module (linked at 0x1E00000; entry 0x1E00080, data area
//! 0x1E10000; one function of 736 words) [F] (helper A, 30 Oct 2026).
//!
//! Launch (`entry(data, param >= 0)`): the data area cleared (0x9A0
//! bytes), TEXEVE records 0x18..0x1A (0x202530(8, ..)) at +8 / +0xC /
//! +0x10, the rays' origin (0, 800, 0, 1) at +0x20 and target (0, 0, 0, 1)
//! at +0x30. Each update (`param < 0`), frame f = +0 (counted after):
//! - f == 0x20 starts the flash: a sprite (0x700, 0x700)..(0x980, 0x8C0)
//!   colour (0xFF, 0xDC, 0xC8) with alpha 128 (48 - t) / 48 for 49
//!   updates (ALPHA preset 1, OT key 0).
//! - f == 0x1F starts the light: camera light slot 7 (0x1FAB50) with
//!   (1, 100, 100, 100), (0, 20, 0, 1), (10, 500, (48 - t) / 48), cleared
//!   after 49 updates.
//! - f == 0x10 launches the rays (record +0x50.., one record): per ray a
//!   random length 20..40 (+0x84 + 4 k) and width 7.5..15 (+0x104 + 4 k),
//!   a matrix pointing from the origin to the target (0x100E40) turned by
//!   a random angle about z (0x100E00).
//! - while the rays run (64 updates): 32 rays of up to 32 textured
//!   segments (TEXEVE 0x1A, additive, CLAMP 5), each segment's width from
//!   0x100D50's tan falloff, alphas fading along the ray and with time.
//! - returns 8 when f reaches 0x71.
use crate::bus::{Bus, Image};
use crate::ee;
use crate::exe::{Fx, OT};
use crate::types::{EffectTextures, EffectUpdate, EffectWorld};
use crate::EffectPort;
use std::sync::Arc;

/// CMODULE program id.
pub const ID: u16 = 0x191;
/// Its entry in the type-0 table 0x2B8510.
pub const ENTRY: u32 = 0x01E0_0080;
/// The data area of type-0 task slot 0.
const D: u32 = 0x01E1_0000;
/// The frame size of the entry.
const FRAME: u32 = 0x2E0;
/// 0x205210's output vectors (a global scratch the module passes).
const PROJECTED: u32 = 0x002D_3600;

/// A running c401.
pub struct C401 {
    fx: Fx,
}

impl C401 {
    /// The program from the user's executable and CMODULE.MRG (`None`: no
    /// record 401). `textures`: TEXEVE 0x18..0x1A as type 8 (`other`).
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

    /// The launch: 0x1E00080 with `param` >= 0 (0x1E000F0..0x1E001A4).
    fn init(&mut self) -> i32 {
        let fx = &mut self.fx;
        fx.enter(FRAME);
        fx.bzero(D, 0x9A0);
        fx.mem.set32(D, 0);
        fx.mem.set32(D + 4, 0);
        for (k, id) in [0x18, 0x19, 0x1A].into_iter().enumerate() {
            let t = fx.texture(8, id);
            fx.mem.write_u32(D + 8 + 4 * k as u32, t);
        }
        for (o, v) in [(0x20u32, 0u32), (0x24, 0x4448_0000), (0x28, 0), (0x2C, 0x3F80_0000), (0x30, 0), (0x34, 0), (0x38, 0), (0x3C, 0x3F80_0000), (0x40, 0)] {
            fx.mem.write_u32(D + o, v);
        }
        fx.mem.write_u8(D + 0x50, 0);
        fx.mem.write_u8(D + 0x990, 0);
        fx.mem.write_u8(D + 0x998, 0);
        fx.leave(FRAME);
        0
    }

    /// An update: 0x1E00080 with `param` < 0 (0x1E001AC..).
    fn step(&mut self) -> i32 {
        let fx = &mut self.fx;
        let sp = fx.enter(FRAME);
        if fx.mem.i32(D) == 0x20 {
            fx.mem.write_u8(D + 0x990, 1);
            fx.mem.set32(D + 0x994, 0);
        }
        if fx.mem.read_u8(D + 0x990) == 1 {
            flash(fx, sp);
        }
        if fx.mem.i32(D) == 0x1F {
            fx.mem.write_u8(D + 0x998, 1);
            fx.mem.set32(D + 0x99C, 0);
        }
        if fx.mem.read_u8(D + 0x998) == 1 {
            light(fx, sp);
        }
        if fx.mem.i32(D) == 0x10 {
            launch_rays(fx);
        }
        // The strip descriptor (sp + 0x218): 12 values RGBAQ / ST / XYZF2
        // x 4, flags 0x34.
        let d = sp + 0x218;
        fx.mem.set16(d + 2, 0x34);
        fx.mem.set16(d, 0xC);
        fx.mem.write_u64(d + 8, 0x4214_2142_1421);
        fx.mem.set32(d + 4, 0);
        let a = fx.alpha_preset(1);
        fx.desc_alpha(d, a);
        let tex = fx.mem.read_u32(D + 0x10);
        fx.desc_texture(d, tex, 1);
        fx.desc_clamp(d, 5);
        fx.mem.set32(sp + 0x110, 0);
        fx.mem.set32(sp + 0x100, 0);
        let mut s6 = D;
        loop {
            if fx.mem.read_u8(s6 + 0x50) != 0 {
                rays(fx, sp, s6);
                let n = fx.mem.i32(s6 + 0x54) + 1;
                fx.mem.set32(s6 + 0x54, n);
                if n >= 0x41 {
                    fx.mem.write_u8(s6 + 0x50, 0);
                }
            }
            let v = fx.mem.i32(sp + 0x100).wrapping_add(0x15B3);
            fx.mem.set32(sp + 0x100, v);
            let j = fx.mem.i32(sp + 0x110) + 1;
            fx.mem.set32(sp + 0x110, j);
            s6 += 0x940;
            if j > 0 {
                break;
            }
        }
        let t = fx.mem.i32(D + 4) + 1;
        fx.mem.set32(D + 4, t);
        let f = fx.mem.i32(D) + 1;
        fx.mem.set32(D, f);
        fx.leave(FRAME);
        if f >= 0x71 {
            8
        } else {
            0
        }
    }
}

/// The flash sprite (0x1E001E4..0x1E00328).
fn flash(fx: &mut Fx, sp: u32) {
    let d = sp + 0x2A8;
    fx.mem.set16(d + 2, 6);
    fx.mem.set16(d, 3);
    fx.mem.write_u64(d + 8, 0x441);
    fx.mem.set32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let t = fx.mem.i32(D + 0x994);
    let alpha = ee::fixunssfsi(ee::mul(128.0, ee::div(ee::from_i32(0x30 - t), 48.0)));
    fx.mem.write_u8(sp + 0x2B8, 0xC8);
    fx.mem.write_u8(sp + 0x2B9, 0xDC);
    fx.mem.write_u8(sp + 0x2BA, 0xFF);
    fx.mem.write_u8(sp + 0x2BB, alpha as u8);
    fx.mem.write_u32(sp + 0x2BC, 0x3F80_0000);
    for (at, x, y) in [(sp + 0x2C0, 0x7000u16, 0x7000u16), (sp + 0x2C8, 0x9800, 0x8C00)] {
        fx.mem.write_u16(at, x);
        fx.mem.write_u16(at + 2, y);
        let v = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (v & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFF7u64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    fx.submit(d, OT, 0);
    let t = fx.mem.i32(D + 0x994) + 1;
    fx.mem.set32(D + 0x994, t);
    if t >= 0x31 {
        fx.mem.write_u8(D + 0x990, 0);
    }
}

/// The scene light, camera light slot 7 (0x1E00364..0x1E00418).
fn light(fx: &mut Fx, sp: u32) {
    for (o, v) in [(0x1E4u32, 100.0f32), (0x1E8, 100.0), (0x1EC, 100.0), (0x1E0, 1.0), (0x1F0, 0.0), (0x1F4, 20.0), (0x1F8, 0.0), (0x1FC, 1.0), (0x200, 10.0), (0x204, 500.0)] {
        fx.mem.setf(sp + o, v);
    }
    let t = fx.mem.i32(D + 0x99C);
    let f = ee::div(ee::from_i32(0x30 - t), 48.0);
    fx.mem.setf(sp + 0x208, f);
    fx.camera_light(0, 7, sp + 0x1E0);
    let t = fx.mem.i32(D + 0x99C) + 1;
    fx.mem.set32(D + 0x99C, t);
    if t >= 0x31 {
        fx.mem.write_u8(D + 0x998, 0);
        fx.camera_light(0, 7, 0);
    }
}

/// The rays' launch (0x1E00430..0x1E00634): record +0x940 x (+0x40).
fn launch_rays(fx: &mut Fx) {
    let idx = fx.mem.i32(D + 0x40);
    let r = D.wrapping_add((idx.wrapping_mul(0x940)) as u32);
    fx.mem.write_u8(r + 0x50, 1);
    fx.mem.set32(r + 0x54, 0);
    for k in 0..4 {
        let v = fx.mem.read_u32(D + 0x20 + 4 * k);
        fx.mem.write_u32(r + 0x60 + 4 * k, v);
    }
    fx.vu_sub4(r + 0x70, D + 0x30, r + 0x60);
    let x2 = Fx::powf2(fx.mem.f(r + 0x70));
    let y2 = Fx::powf2(fx.mem.f(r + 0x74));
    let z2 = Fx::powf2(fx.mem.f(r + 0x78));
    let len = fx.sqrtf(ee::add(z2, ee::add(x2, y2)));
    fx.mem.setf(r + 0x80, len);
    let byte = |fx: &mut Fx| ((fx.rand() >> 23) & 0xFF) as i32;
    for k in 0..32u32 {
        let v = byte(fx);
        let f = ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(v)), 256.0));
        fx.mem.setf(r + 4 * k + 0x84, ee::mul(40.0, f));
        let v = byte(fx);
        let f = ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(v)), 256.0));
        fx.mem.setf(r + 4 * k + 0x104, ee::mul(15.0, f));
        let m = r + 0x40 * k + 0x190;
        fx.point_along(m, r + 0x70);
        let v = byte(fx);
        fx.rotate_axis(m, (v << 16) >> 8, 2);
    }
    let n = fx.mem.i32(D + 0x40) + 1;
    fx.mem.set32(D + 0x40, n);
    if n > 0 {
        fx.mem.set32(D + 0x40, 0);
    }
}

/// The 32 rays of record `s6` (0x1E006B0..0x1E00B1C).
fn rays(fx: &mut Fx, sp: u32, s6: u32) {
    let t = ee::from_i32(fx.mem.i32(s6 + 0x54));
    let grow = Fx::ratio_below(t, 16.0);
    let mut segs = ee::fixsfsi(ee::mul(32.0, grow));
    if segs > 0x20 {
        segs = 0x20;
    }
    let f3 = ee::div(ee::from_i32(0x40 - fx.mem.i32(s6 + 0x54)), 64.0);
    let mut s5 = fx.mem.i32(sp + 0x100);
    fx.mem.write_u32(sp + 0xF0, s6 + 0x60);
    let f25 = ee::mul(100.0, f3);
    let f26 = ee::add(0.5, ee::mul(0.5, f3));
    let f27 = ee::add(1.0, ee::mul(0.5, ee::sub(1.0, f3)));
    let d = sp + 0x218;
    for s0 in 0..32i32 {
        let fp = s6 + 0x40 * s0 as u32;
        let s4 = s6 + 4 * s0 as u32;
        fx.camera_vp_to(sp + 0x120);
        let origin = fx.mem.read_u32(sp + 0xF0);
        fx.translate_rows(sp + 0x160, fp + 0x190, origin);
        let m = fx.read_m4(sp + 0x160);
        fx.mul_into(Some(sp + 0x120), &m);
        let f21 = ee::div(ee::mul(32.0, ee::from_i32(s0 & 3)), 128.0);
        if segs <= 0 {
            s5 = s5.wrapping_add(0x411A);
            continue;
        }
        let f24 = ee::add(0.25, f21);
        let mut s3 = 0i32;
        for s1 in 0..segs {
            let f22 = ee::mul(400.0, ee::sub(1.0, fx.tan_falloff(s1, 32)));
            let f23 = ee::mul(400.0, ee::sub(1.0, fx.tan_falloff(s1 + 1, 32)));
            let a0 = ee::fixunssfsi(ee::div(ee::mul(f25, ee::from_i32(s1)), 32.0));
            colour(fx, sp + 0x228, a0 as u8);
            copy8(fx, sp + 0x228, sp + 0x240);
            let a1 = ee::fixunssfsi(ee::div(ee::mul(f25, ee::from_i32(s1 + 1)), 32.0));
            colour(fx, sp + 0x258, a1 as u8);
            copy8(fx, sp + 0x258, sp + 0x270);
            if s1 == segs - 1 {
                colour(fx, sp + 0x258, 0);
                copy8(fx, sp + 0x258, sp + 0x270);
            }
            let v1 = (s1 + s0) & 7;
            let t0 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(v1)), 8.0), 128.0);
            fx.mem.setf(sp + 0x230, f21);
            fx.mem.setf(sp + 0x234, t0);
            fx.mem.setf(sp + 0x248, f24);
            fx.mem.setf(sp + 0x24C, t0);
            fx.mem.setf(sp + 0x260, f21);
            let t1 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(v1 + 1)), 8.0), 128.0);
            fx.mem.setf(sp + 0x264, t1);
            fx.mem.setf(sp + 0x278, f24);
            fx.mem.setf(sp + 0x27C, t1);
            let len = fx.mem.f(s6 + 0x80);
            let f29 = ee::mul(fx.mem.f(s4 + 0x84), f26);
            let f28 = ee::div(ee::mul(len, ee::from_i32(s1 + 1)), 32.0);
            let f20 = ee::div(ee::mul(len, ee::from_i32(s1)), 32.0);
            let c0 = fx.tcos(((s3 >> 5).wrapping_add(s5)) & 0xFFFF);
            let f30 = ee::mul(f27, ee::mul(fx.mem.f(s4 + 0x104), c0));
            let c1 = fx.tcos(((((s1 + 1) << 17) >> 5).wrapping_add(s5)) & 0xFFFF);
            let f0 = ee::mul(fx.mem.f(s4 + 0x104), c1);
            let f3 = ee::neg(f29);
            let f1 = ee::mul(f27, f0);
            let v = sp + 0x1A0;
            for (k, (x, y, z)) in [(f30, ee::add(f3, f22), f20), (f30, ee::add(f29, f22), f20), (f1, ee::add(f3, f23), f28), (f1, ee::add(f29, f23), f28)].into_iter().enumerate() {
                let a = v + 16 * k as u32;
                fx.mem.setf(a, x);
                fx.mem.setf(a + 4, y);
                fx.mem.setf(a + 8, z);
                fx.mem.write_u32(a + 12, 0x3F80_0000);
            }
            let m = fx.read_m4(sp + 0x120);
            if fx.project_desc(v, PROJECTED, Some(&m), d, sp + 0x2DC) {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x2DC)) as i32;
                fx.submit(d, OT, key);
            }
            s3 = s3.wrapping_add(0x2_0000);
        }
        s5 = s5.wrapping_add(0x411A);
    }
}

/// RGBAQ (0x80, 0x80, 0x80, `a`), Q 1.0 at `at`.
fn colour(fx: &mut Fx, at: u32, a: u8) {
    fx.mem.write_u8(at, 0x80);
    fx.mem.write_u8(at + 1, 0x80);
    fx.mem.write_u8(at + 2, 0x80);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

fn copy8(fx: &mut Fx, from: u32, to: u32) {
    let (a, b) = (fx.mem.read_u32(from), fx.mem.read_u32(from + 4));
    fx.mem.write_u32(to, a);
    fx.mem.write_u32(to + 4, b);
}

impl EffectPort for C401 {
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
