//! CMODULE c061 (program 0x3D), the first of the Exodia finale's three
//! modules (launched with c062 / c063 by the Exodia event 0x26DAA0 once
//! Summoned Lord Exodia's model is in object 0x20), hand-ported from the
//! module (linked at 0x1E00000; entry 0x1E02D00, data area 0x1E10000;
//! 4,608 words, 3,300 of them its own) [F] (helper A, 30 Oct 2026). Its
//! second-effect-library calls are in [`crate::fxlib2`].
//!
//! Launch: the data area cleared (0x6B40 bytes), two degree sine tables
//! (+0xD8, +0x69CC), TEXEFF 14 / 15 (+0x2C8 / +0x2CC), the Lord's effect
//! textures 0..4 (+0x6820..), node slots 0..6 (+0x68A8), the side (+0x68C4
//! = self handle != 0, +0x68C6 its opposite), a countdown of 0x3B9 updates
//! (+0x6840) and 256 random bytes -15..14 (+0x68CA, 100..179 zeroed).
//! Each update: a timeline (0x1E014F0, frame +0x68C8) requests the Lord's
//! nodes, changes its motion, keeps the opponent's units idle and spawns
//! tasks (150 at +0x14A0, 0x70 each: a step function +0x44, state +0x58)
//! and rising trails (30 at +0x2D0, 0x98 each); every live trail and task
//! is stepped, then drawn by its kind (sprite quads on the Lord's nodes,
//! sparks, rings, spline ribbons). Returns 8 when the countdown ends.
//!
//! [F] c061's own timeline spawns only the glow (0x1E01F20), the flash
//! (0x1E02320), the empty step (0x1E02CF0) and the trails; the burst,
//! ring and ribbon steps (0x1E02030 / 0x1E02660 / 0x1E02940) and their
//! draws are in the record but never spawned by it: ported as read, not
//! reached by the parity runs (unverified).
use crate::bus::{Bus, Image};
use crate::ee;
use crate::exe::Fx;
use crate::fxlib2 as l2;
use crate::types::{EffectTextures, EffectUpdate, EffectWorld};
use crate::EffectPort;
use std::sync::Arc;

/// CMODULE program id.
pub const ID: u16 = 0x3D;
/// Its entry in the type-0 table 0x2B8510.
pub const ENTRY: u32 = 0x01E0_2D00;
/// The data area of type-0 task slot 0.
const D: u32 = 0x01E1_0000;
/// Summoned Lord Exodia's object index.
const LORD: i32 = 0x20;
/// The scratchpad 0x204F70 projects into.
const SPR: u32 = 0x7000_0000;
/// The task step functions (their addresses in the module, stored in the
/// task records as the original stores them).
const STEP_GLOW: u32 = 0x01E0_1F20;
const STEP_BURST: u32 = 0x01E0_2030;
const STEP_FLASH: u32 = 0x01E0_2320;
const STEP_RING: u32 = 0x01E0_2660;
const STEP_RIBBON: u32 = 0x01E0_2940;
const STEP_NONE: u32 = 0x01E0_2CF0;


/// A running c061.
pub struct C061 {
    fx: Fx,
    /// The roles' motion after the last call (motion, time, blend).
    motion: [(u8, f32, u8); 2],
}

impl C061 {
    /// The program from the user's executable and CMODULE.MRG (`None`: no
    /// record). `handles`: (self, target) as the executor sets them.
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
        Some(Self { fx, motion: [(0, 0.0, 0); 2] })
    }

    /// Make `role`'s mirrored record answer at object index `index` too
    /// (`roses_ee::bmodule::AttackEffect::place_role`).
    pub fn place_role(&mut self, role: usize, index: u8, motions: &[(u8, u32)]) {
        crate::world::place_role(&mut self.fx.mem, role, index, motions);
    }

    /// The launch (0x1E02D3C..0x1E02E64).
    fn init(&mut self) -> i32 {
        let fx = &mut self.fx;
        fx.enter(0x160);
        fx.mem.write_u32(D + 0x684C, crate::exe::OT);
        fx.mem.write_u32(D + 0x80, crate::exe::OT);
        fx.bzero(D, 0x6B40);
        l2::sine_table(fx, D + 0x69CC);
        l2::sine_table(fx, D + 0xD8);
        for (k, id) in [14, 15].into_iter().enumerate() {
            let t = fx.texture(4, id);
            fx.mem.write_u32(D + 0x2C8 + 4 * k as u32, t);
        }
        for k in 0..5 {
            let t = fx.texture(5, k);
            fx.mem.write_u32(D + 0x6820 + 4 * k as u32, t);
        }
        for k in 0..7 {
            fx.mem.set16(D + 0x68A8 + 2 * k, k as i32);
        }
        let own = fx.self_handle();
        fx.mem.set16(D + 0x68C4, i32::from(own != 0));
        fx.mem.set32(D + 0x6840, 0x3B9);
        let side = fx.mem.i16(D + 0x68C4);
        fx.mem.set16(D + 0x68C6, i32::from(side == 0));
        for k in 0..0x100 {
            let r = fx.rand();
            let (_, m) = fx.div(r, 0x1E);
            fx.mem.set8(D + 0x68CA + k, m - 0xF);
        }
        for k in 0x64..0xB4 {
            fx.mem.write_u8(D + 0x68CA + k, 0);
        }
        fx.leave(0x160);
        0
    }

    /// An update (0x1E02E6C..).
    fn step(&mut self) -> i32 {
        let fx = &mut self.fx;
        let sp = fx.enter(0x160);
        fx.mem.write_u32(D + 0x684C, crate::exe::OT);
        fx.mem.write_u32(D + 0x80, crate::exe::OT);
        timeline(fx);
        for k in 0..30 {
            let rec = D + 0x2D0 + 0x98 * k;
            if fx.mem.i16(rec + 0x88) != 0 {
                trail_step(fx, rec);
            }
        }
        for k in 0..30 {
            let rec = D + 0x2D0 + 0x98 * k;
            if fx.mem.i16(rec + 0x88) != 0 {
                trail_draw(fx, rec, sp + 0xD0, 0);
                trail_draw(fx, rec, sp + 0xD0, 1);
            }
        }
        for k in 0..150 {
            let t = D + 0x14A0 + 0x70 * k;
            if fx.mem.i16(t + 0x58) != 0 {
                let f = fx.mem.read_u32(t + 0x44);
                if f != 0 {
                    run_step(fx, f, t);
                }
            }
        }
        for k in 0..150 {
            let t = D + 0x14A0 + 0x70 * k;
            if fx.mem.i16(t + 0x58) == 0 {
                continue;
            }
            draw_task(fx, sp, t);
        }
        let n = fx.mem.i32(D + 0x6840);
        if n != 0 {
            fx.mem.set32(D + 0x6840, n - 1);
        }
        let n = fx.mem.i32(D + 0x6840);
        fx.leave(0x160);
        if n > 0 {
            0
        } else {
            8
        }
    }
}

/// A rectangle from 0x106960 into the current one (+0x94..+0xA3).
pub(crate) fn set_rect(fx: &mut Fx, at: u32, id: i32, sub: i32) {
    l2::rect(fx, at, id, sub);
    for k in 0..4 {
        let w = fx.mem.read_u32(at + 4 * k);
        fx.mem.write_u32(D + 0x94 + 4 * k, w);
    }
}

/// The entry's draw of one live task by its step function
/// (0x1E02F4C..0x1E03338).
fn draw_task(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_GLOW {
        let slot = fx.mem.i16(D + 0x68AA);
        fx.object_node(LORD, slot, sp + 0x60);
        let tex = fx.mem.read_u32(D + 0x2CC);
        fx.mem.write_u32(D + 0x88, tex);
        set_rect(fx, sp + 0x70, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        fx.mem.set32(sp + 0x6C, 0);
        sprite_quads(fx, t, 0, sp + 0x60);
        sprite_quads(fx, t, 0, sp + 0x60);
        let slot = fx.mem.i16(D + 0x68A8);
        fx.object_node(LORD, slot, sp + 0x60);
        fx.mem.set32(sp + 0x6C, 0);
        sprite_quads(fx, t, 0, sp + 0x60);
        sprite_quads(fx, t, 0, sp + 0x60);
    }
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_BURST {
        let tex = fx.mem.read_u32(D + 0x6830);
        fx.mem.write_u32(D + 0x88, tex);
        let o = fx.mem.i16(t + 0x52);
        fx.mem.set16(D + 0xBE, o);
        let v = fx.mem.i16(t + 0x56);
        if v < 2 {
            fx.mem.set16(D + 0x94, v << 6);
            fx.mem.set16(D + 0x96, 0);
        } else {
            fx.mem.set16(D + 0x94, (v - 2) << 6);
            fx.mem.set16(D + 0x96, 0x40);
        }
        fx.mem.set16(D + 0x98, 0x40);
        fx.mem.set16(D + 0x9A, 0x40);
        sprite_one(fx, t, 0x21, t + 0x10);
    }
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_FLASH {
        if fx.mem.i16(t + 0x50) == 0 {
            let tex = fx.mem.read_u32(D + 0x2CC);
            fx.mem.write_u32(D + 0x88, tex);
            set_rect(fx, sp + 0x80, 0x97, 0);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            let tex = fx.mem.read_u32(D + 0x2CC);
            fx.mem.write_u32(D + 0x88, tex);
            let s = fx.mem.i16(t + 0x56);
            set_rect(fx, sp + 0x90, 0x9B, s);
        }
        for k in 0..4 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(sp + 0x60 + 4 * k, w);
        }
        fx.mem.set16(D + 0xB8, 6);
        sparks(fx, t, 0x20, sp + 0x60);
        fx.mem.set16(D + 0xB8, 0);
    }
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_RING {
        let s = fx.mem.i16(t + 0x56);
        set_rect(fx, sp + 0xA0, 0x14, s);
        fx.mem.set16(D + 0xC0, 0);
        let tex = fx.mem.read_u32(D + 0x6820);
        fx.mem.write_u32(D + 0x88, tex);
        fx.mem.set16(D + 0xB8, 5);
        for k in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(D + 0x20 + 4 * k, w);
        }
        fx.mem.set16(D + 0xBE, 0);
        sprite_ring(fx, t, 0x10, D + 0x68CA);
        fx.mem.set16(D + 0xB8, 0);
    }
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_RIBBON {
        let tex = fx.mem.read_u32(D + 0x682C);
        fx.mem.write_u32(D + 0x88, tex);
        set_rect(fx, sp + 0xB0, 0x14, 3);
        fx.mem.set16(D + 0xBE, 0);
        for k in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(sp + 0x60 + 4 * k, w);
        }
        sprite_one(fx, t, 0x20, sp + 0x60);
        let tex = fx.mem.read_u32(D + 0x682C);
        fx.mem.write_u32(D + 0x88, tex);
        set_rect(fx, sp + 0xC0, 0x14, 3);
        let x = fx.mem.i16(D + 0x94) + 0xF;
        fx.mem.set16(D + 0x94, x);
        fx.mem.set16(D + 0x98, 1);
        for k in 0..3 {
            fx.mem.set32(D + 0x20 + 4 * k, 0);
        }
        fx.mem.set16(D + 0xBE, 1);
        let idx = fx.mem.i16(t + 0x50);
        let pts = D.wrapping_add((idx * 0xD0) as u32) + 0x57E0;
        ribbon(fx, t, pts, 0);
    }
}

/// A task's step function by its stored address.
fn run_step(fx: &mut Fx, f: u32, t: u32) {
    match f {
        STEP_GLOW => step_glow(fx, t),
        STEP_BURST => step_burst(fx, t),
        STEP_FLASH => step_flash(fx, t),
        STEP_RING => step_ring(fx, t),
        STEP_RIBBON => step_ribbon(fx, t),
        _ => {}
    }
}

/// 0x1E00080(dst, n, _, src): a trail of `n` points ending at `dst`
/// shifted one back (point k = point k - 1, from the last), point 0 =
/// `src` (xyz).
pub(crate) fn shift_points(fx: &mut Fx, dst: u32, n: i32, src: u32) {
    let mut a = dst;
    let mut left = n - 1;
    let mut k = 0i32;
    while k < n {
        if left == 0 {
            l2::copy3(fx, a, src);
        } else {
            l2::copy3(fx, a, a - 0x10);
        }
        k = (k + 1) as i16 as i32;
        a = a.wrapping_sub(0x10);
        left -= 1;
    }
}

/// 0x1E00110(d, rect, order) [3a1a2828e348: 8 B, 3 C records]: the UVs of
/// a 4-vertex strip with values UV / XYZ (+0x18 + 0x10 k) in one of four
/// orders.
pub(crate) fn uv_strip4(fx: &mut Fx, d: u32, r: u32, order: i32) {
    let (x, y, w, h) = (fx.mem.i16(r), fx.mem.i16(r + 2), fx.mem.i16(r + 4), fx.mem.i16(r + 6));
    let (x1, y1) = (x + w, y + h);
    let at: [u32; 4] = match order {
        0 => [0x18, 0x28, 0x38, 0x48],
        1 => [0x48, 0x38, 0x28, 0x18],
        2 => [0x28, 0x18, 0x48, 0x38],
        3 => [0x38, 0x48, 0x18, 0x28],
        _ => return,
    };
    for (k, (u, v)) in [(x, y), (x1, y), (x, y1), (x1, y1)].into_iter().enumerate() {
        fx.set_uv(d + at[k], u);
        fx.set_uv(d + at[k] + 2, v);
    }
}

/// The ALPHA of a blend selector: 0 -> 0x42, else (n << 2) | 0x40.
pub(crate) fn blend_alpha(n: i32) -> u64 {
    if n == 0 {
        crate::fxlib::ALPHA_42
    } else {
        ((i64::from(n) << 2) as u64) | 0x80_0000_0040
    }
}

/// A 9-value strip header (RGBAQ then UV / XYZ x 4, flags 0x824: FST).
pub(crate) fn desc_uv4(fx: &mut Fx, d: u32) {
    fx.mem.set16(d + 2, 0x24);
    fx.mem.set16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.set32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// The 8-byte RGBAQ of `c` (0x108020 through `local`) at `to`.
pub(crate) fn rgbaq_at(fx: &mut Fx, local: u32, c: u32, to: u32) {
    l2::rgbaq(fx, local, c);
    let (a, b) = (fx.mem.read_u32(local), fx.mem.read_u32(local + 4));
    fx.mem.write_u32(to, a);
    fx.mem.write_u32(to + 4, b);
}

/// The 0.1 (0x3DCCCCCD) scale vector at `at`.
pub(crate) fn scale_tenth(fx: &mut Fx, at: u32) {
    for k in 0..3 {
        fx.mem.write_u32(at + 4 * k, 0x3DCC_CCCD);
    }
}

/// 0x1E00560(D, task, mode, pos): four textured quads (strip3, FST) of
/// the task's size (+0x30 / +0x34) turned by its angle +0x5A about z,
/// billboarded at `pos` (scale 0.05), coloured +0x64 / +0x60; keyed by
/// pos.w (always 0 [F callers]).
pub(crate) fn sprite_quads(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x210);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x70 + 4 * k, w);
    }
    let d = sp + 0x140;
    l2::desc_strip3(fx, d);
    match mode >> 4 {
        0 => {
            rgbaq_at(fx, sp + 0x1D0, t + 0x64, d + 0x10);
            rgbaq_at(fx, sp + 0x1D8, t + 0x64, d + 0x28);
            rgbaq_at(fx, sp + 0x1E0, t + 0x64, d + 0x40);
            rgbaq_at(fx, sp + 0x1E8, t + 0x60, d + 0x58);
        }
        1 => {
            rgbaq_at(fx, sp + 0x1F0, t + 0x64, d + 0x10);
            rgbaq_at(fx, sp + 0x1F8, t + 0x64, d + 0x28);
            rgbaq_at(fx, sp + 0x200, t + 0x60, d + 0x40);
            rgbaq_at(fx, sp + 0x208, t + 0x60, d + 0x58);
        }
        _ => {}
    }
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xD0 + 4 * k, 0x3D4C_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x130);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x70 + 4 * k);
        fx.mem.write_u32(sp + 0xC0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xF0, sp + 0xD0, sp + 0x130, sp + 0xC0);
    let a = fx.mem.i16(t + 0x5A);
    let depth = fx.mem.f(sp + 0x7C);
    fx.mem.set32(sp + 0x138, a);
    fx.mem.set32(sp + 0x134, 0);
    fx.mem.set32(sp + 0x130, 0);
    for k in 0..4 {
        let w = ee::fixsfsi(fx.mem.f(t + 0x30));
        let h = ee::fixsfsi(fx.mem.f(t + 0x34));
        l2::quad_ints(fx, sp + 0x80, w, h, k);
        l2::rotate4(fx, sp + 0x80, sp + 0x130, 2, D + 0xD8);
        if l2::project(fx, sp + 0x80, SPR, sp + 0xF0, d, sp + 0xE0) {
            l2::far_z(fx, &[d + 0x68, d + 0x50, d + 0x38, d + 0x20]);
            let key = ee::fixunssfsi(depth) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x210);
}

/// 0x1E008D0(D, task, mode, pos): one textured quad (9 values, FST) of
/// the task's size (+8 / +0xA) turned by its angles +0x20, at `pos`
/// (scale 0.1), coloured +0x60; blend (mode >> 4) & 15, depth mode & 15
/// (0 the mean w, 1 the far Z at key 0, else key -1).
pub(crate) fn sprite_one(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1B0);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x50 + 4 * k, w);
    }
    let d = sp + 0x118;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) & 0xF));
    rgbaq_at(fx, sp + 0x1A8, t + 0x60, d + 0x10);
    let order = fx.mem.i16(D + 0xBE);
    uv_strip4(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    scale_tenth(fx, sp + 0xB0);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x50 + 4 * k);
        fx.mem.write_u32(sp + 0xA0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, t + 0x20, sp + 0xA0);
    let (w, h) = (ee::from_i32(fx.mem.i16(t + 8)), ee::from_i32(fx.mem.i16(t + 0xA)));
    l2::quad_xy(fx, sp + 0x60, w, h);
    if l2::project(fx, sp + 0x60, SPR, sp + 0xD0, d, sp + 0xC0) {
        let ot = fx.mem.read_u32(D + 0x80);
        match mode & 0xF {
            0 => {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0xC0)) as i32;
                fx.submit(d, ot, key);
            }
            1 => {
                l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                fx.submit(d, ot, 0);
            }
            _ => fx.submit(d, ot, -1),
        }
    }
    fx.leave(0x1B0);
}

/// The camera angle at `a` (16-bit) as negated degrees (the original's
/// multiply by 0xB60B60B7: a / 182).
pub(crate) fn to_degrees_neg(fx: &mut Fx, a: u32) {
    let v = fx.mem.i32(a);
    let hi = ((i64::from(0xB60B_60B7u32 as i32) * i64::from(v)) >> 32) as i32;
    let q = (hi.wrapping_add(v) >> 7).wrapping_add(((v as u32) >> 31) as i32);
    fx.mem.set32(a, q.wrapping_neg());
}

/// 0x1E00B90(D, task, mode, table): a ring of D+0xB8 textured quads (9
/// values, FST) of size +0x48 facing the camera at D+0x20, each pushed out
/// by the random bytes `table` [task +8 + k (+0x64, +0xB4)] / 20 times the
/// radius +0x50 + k / 2 and turned by +0x5A degrees.
pub(crate) fn sprite_ring(fx: &mut Fx, t: u32, mode: i32, table: u32) {
    let sp = fx.enter(0x220);
    let d = sp + 0x188;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    rgbaq_at(fx, sp + 0x218, t + 0x60, d + 0x10);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    scale_tenth(fx, sp + 0x100);
    l2::camera_angles_to(fx, sp + 0x160);
    to_degrees_neg(fx, sp + 0x164);
    to_degrees_neg(fx, sp + 0x160);
    let v = fx.mem.i16(t + 0x5C);
    fx.mem.set32(sp + 0x168, v);
    let v = fx.mem.i16(t + 0x5A);
    fx.mem.set32(sp + 0x174, v);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x100, 0, sp + 0xF0);
    let order = fx.mem.i16(D + 0xC0);
    uv_strip4(fx, d, D + 0x94, order);
    let s4 = mode & 0xF;
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xB8) {
        let size = fx.mem.f(t + 0x48);
        l2::quad_xy(fx, sp + 0xA0, size, size);
        l2::rotate4(fx, sp + 0xA0, sp + 0x160, 2, D + 0xD8);
        l2::rotate4(fx, sp + 0xA0, sp + 0x160, 1, D + 0xD8);
        l2::rotate4(fx, sp + 0xA0, sp + 0x160, 0, D + 0xD8);
        let s6 = table.wrapping_add(k as u32);
        let half = k >> 1;
        let byte = |fx: &mut Fx, off: u32| {
            let i = fx.mem.i16(t + 8);
            ee::from_i32(fx.mem.i8(s6.wrapping_add(i as u32) + off))
        };
        let radius = |fx: &mut Fx| ee::from_i32(fx.mem.i16(t + 0x50) + half);
        let b = byte(fx, 0);
        let r = radius(fx);
        fx.mem.setf(sp + 0xE0, ee::mul(ee::div(b, 20.0), r));
        let b = byte(fx, 0x64);
        let r = radius(fx);
        fx.mem.setf(sp + 0xE4, ee::mul(ee::div(b, 20.0), r));
        fx.mem.set32(sp + 0xE8, 0);
        l2::rotate_deg(fx, sp + 0xE0, sp + 0x170, 0, D + 0xD8);
        for (o, from) in [(0u32, 0xE0u32), (4, 0xE4)] {
            let f2 = fx.mem.f(sp + from);
            for c in [0xA0u32, 0xB0, 0xC0, 0xD0] {
                let v = ee::add(fx.mem.f(sp + c + o), f2);
                fx.mem.setf(sp + c + o, v);
            }
        }
        let r = radius(fx);
        let b = byte(fx, 0xB4);
        let z = ee::mul(ee::div(b, 20.0), r);
        fx.mem.setf(sp + 0xE8, z);
        for c in [0xA8u32, 0xB8, 0xC8, 0xD8] {
            let v = ee::add(fx.mem.f(sp + c), z);
            fx.mem.setf(sp + c, v);
        }
        if s4 <= 2 && l2::project(fx, sp + 0xA0, SPR, sp + 0x120, d, sp + 0x110) {
            let ot = fx.mem.read_u32(D + 0x80);
            match s4 {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x110)) as i32;
                    fx.submit(d, ot, key);
                }
                1 => {
                    l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x220);
}

/// 0x1E01100(D, task, mode, pos): D+0xB8 sparks (strip3, FST) on a
/// circle at `pos` turned by +0x20: inner radius +0x48, outer +0x4C,
/// each from angle a to a + +0x5C degrees, a stepping by +0x5A.
pub(crate) fn sparks(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x210);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xA0 + 4 * k, w);
    }
    let span = fx.mem.i16(t + 0x5C);
    let d = sp + 0x160;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    let blend = (mode >> 4) as i16 as i32;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 0);
    fx.desc_alpha(d, blend_alpha(blend));
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    rgbaq_at(fx, sp + 0x1F0, t + 0x60, d + 0x10);
    rgbaq_at(fx, sp + 0x1F8, t + 0x64, d + 0x40);
    rgbaq_at(fx, sp + 0x200, t + 0x60, d + 0x28);
    rgbaq_at(fx, sp + 0x208, t + 0x64, d + 0x58);
    scale_tenth(fx, sp + 0x100);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0xA0 + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x110, sp + 0x100, t + 0x20, sp + 0xF0);
    for o in [0xB4u32, 0xC4, 0xD4, 0xE4] {
        fx.mem.set32(sp + o, 0);
    }
    let (inner, outer) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let s5 = mode & 0xF;
    let mut a = 0i32;
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xB8) {
        let s6 = a as i16 as i32;
        let tab = D + 0xD8;
        let c = l2::sin_deg(fx, tab, s6, 1);
        fx.mem.setf(sp + 0xB0, ee::mul(inner, c));
        let s = l2::sin_deg(fx, tab, s6, 0);
        fx.mem.setf(sp + 0xB8, ee::mul(inner, s));
        let c = l2::sin_deg(fx, tab, s6, 1);
        fx.mem.setf(sp + 0xD0, ee::mul(outer, c));
        let s = l2::sin_deg(fx, tab, s6, 0);
        fx.mem.setf(sp + 0xD8, ee::mul(outer, s));
        let e = (span + s6) as i16 as i32;
        let c = l2::sin_deg(fx, tab, e, 1);
        fx.mem.setf(sp + 0xC0, ee::mul(inner, c));
        let s = l2::sin_deg(fx, tab, e, 0);
        fx.mem.setf(sp + 0xC8, ee::mul(inner, s));
        let c = l2::sin_deg(fx, tab, e, 1);
        fx.mem.setf(sp + 0xE0, ee::mul(outer, c));
        let s = l2::sin_deg(fx, tab, e, 0);
        fx.mem.setf(sp + 0xE8, ee::mul(outer, s));
        if s5 <= 1 && l2::project(fx, sp + 0xB0, SPR, sp + 0x110, d, sp + 0x150) {
            let ot = fx.mem.read_u32(D + 0x80);
            if s5 == 0 {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x150)) as i32;
                fx.submit(d, ot, key);
            } else {
                l2::far_z_desc(fx, d, 1);
                fx.submit(d, ot, 0);
            }
        }
        k += 1;
        a = (a + fx.mem.i16(t + 0x5A)) as i16 as i32;
    }
    fx.leave(0x210);
}

/// 0x1E014F0(D): the timeline (frame D+0x68C8, counted first).
fn timeline(fx: &mut Fx) {
    let sp = fx.enter(0x70);
    fx.mem.write_u8(D + 0x6844, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(D + 0x6844 + k, 0);
    }
    let t = fx.mem.i16(D + 0x68C8) + 1;
    fx.mem.set16(D + 0x68C8, t);
    let frame = |fx: &mut Fx| fx.mem.i16(D + 0x68C8);
    let requests = |fx: &mut Fx, nodes: &[i32]| {
        for (slot, &n) in nodes.iter().enumerate() {
            fx.object_request_node(LORD, n, slot as i32);
        }
    };
    if frame(fx) == 0x14A {
        requests(fx, &[0xE, 0xF]);
    }
    if frame(fx) == 0x190 {
        requests(fx, &[0x23, 0x24, 0x27, 0x4C, 0x4D, 0x50]);
    }
    if frame(fx) == 0x244 {
        requests(fx, &[0x74, 0x81, 0x24, 0x50, 0xE, 0xF, 0x23]);
    }
    if frame(fx) == 0x12C {
        fx.change_motion(LORD, 2, 6);
    }
    let t = frame(fx);
    if (0x297..0x324).contains(&t) {
        let side = fx.mem.i16(D + 0x68C4);
        let units = match side {
            0 => Some(0x100..0x106),
            1 => Some(0..6),
            _ => None,
        };
        if let Some(units) = units {
            for h in units {
                fx.unit_flags(h, 0);
                fx.keep_motion4(h);
            }
        }
    }
    if frame(fx) == 0x12D {
        spawn_task(fx, STEP_NONE, 0);
    }
    if frame(fx) == 0x2F1 {
        let mut v = -0x1E;
        for k in 0..10u32 {
            fx.mem.setf(D + 0x5714 + 0x10 * k, ee::from_i32(v));
            v += 0x11;
        }
        fx.mem.set32(D + 0x6854, 0);
        for k in 1..10u32 {
            fx.mem.write_u32(D + 0x6854 + 4 * k, 0x4040_0000);
        }
        for k in 0..10u32 {
            let r = fx.rand();
            let (_, m) = fx.div(r, 0x168);
            fx.mem.set16(D + 0x6894 + 2 * k, m);
        }
        for k in 0..3 {
            fx.mem.write_u8(D + 0x6844 + k, 0x20);
        }
        fx.mem.set32(D + 0x250, 0);
        fx.mem.write_u32(D + 0x254, 0x41F0_0000);
        fx.mem.set32(D + 0x258, 0);
        fx.mem.set16(D + 0x68BE, 1);
        fx.mem.set32(D + 0x6880, 0);
        fx.mem.set16(D + 0x68C2, 1);
        fx.mem.set16(D + 0x68BC, 0);
        spawn_trail(fx);
    }
    let slot = fx.mem.i16(D + 0x68AA);
    fx.object_node(LORD, slot, sp + 0x60);
    for k in 0..10u32 {
        let tab = D + 0x69CC;
        let a = fx.mem.i16(D + 0x6894 + 2 * k);
        let c = l2::sin_deg(fx, tab, a, 1);
        let r = fx.mem.f(D + 0x6854 + 4 * k);
        fx.mem.setf(D + 0x5710 + 0x10 * k, ee::mul(r, c));
        let a = fx.mem.i16(D + 0x6894 + 2 * k);
        let s = l2::sin_deg(fx, tab, a, 0);
        let r = fx.mem.f(D + 0x6854 + 4 * k);
        fx.mem.setf(D + 0x5718 + 0x10 * k, ee::mul(r, s));
        let a = fx.mem.i16(D + 0x6894 + 2 * k) + 4;
        fx.mem.set16(D + 0x6894 + 2 * k, a);
        let a = fx.mem.i16(D + 0x6894 + 2 * k);
        if a >= 0x168 {
            let (_, m) = fx.div(a, 0x168);
            fx.mem.set16(D + 0x6894 + 2 * k, m);
        }
    }
    let t = frame(fx);
    if t == 0x271 || (0x2B1..0x371).contains(&t) {
        let s = fx.mem.i16(D + 0x68AC);
        fx.object_node(LORD, s, D + 0x56A0);
        let s = fx.mem.i16(D + 0x68B4);
        fx.object_node(LORD, s, sp + 0x60);
        for k in 0..3u32 {
            let p = fx.mem.f(D + 0x56A0 + 4 * k);
            let n = fx.mem.f(sp + 0x60 + 4 * k);
            let f = fx.mem.f(D + 0x5670 + 4 * k);
            fx.mem.setf(D + 0x56A0 + 4 * k, ee::add(p, ee::mul(f, ee::sub(p, n))));
        }
        let y = ee::add(fx.mem.f(D + 0x56A4), 25.0);
        fx.mem.setf(D + 0x56A4, y);
        if frame(fx) >= 0x2D7 {
            for k in 0..3u32 {
                let f = fx.mem.f(D + 0x5670 + 4 * k);
                if crate::exe::ee_lt(f, 1.0) {
                    fx.mem.setf(D + 0x5670 + 4 * k, ee::add(f, f32::from_bits(0x3C23_D70A)));
                }
            }
        }
    }
    for k in 0..4u32 {
        let w = fx.mem.read_u32(D + 0x56A0 + 4 * k);
        fx.mem.write_u32(D + 0x57B0 + 4 * k, w);
    }
    if frame(fx) == 0x16C {
        fx.mem.set16(D + 0x68BC, 0);
        spawn_task(fx, STEP_GLOW, 0);
        fx.mem.set16(D + 0x68BC, 0x5A);
        spawn_task(fx, STEP_GLOW, 0);
    }
    if frame(fx) == 0x2F5 {
        for k in 0..3u32 {
            let w = fx.mem.read_u32(D + 0x57B0 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        let mut a = 0i32;
        for k in 0..0x10 {
            fx.mem.set16(D + 0x68C2, k);
            fx.mem.set16(D + 0x68BC, a);
            a = (a + 0x2D) as i16 as i32;
        }
    }
    let t = frame(fx);
    if (0x30C..0x35D).contains(&t) {
        for k in 0..3u32 {
            let w = fx.mem.read_u32(D + 0x57B0 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        fx.mem.set16(D + 0x68C2, 0);
        spawn_task(fx, STEP_FLASH, 0);
    }
    fx.leave(0x70);
}

/// 0x1E033A0(D, ..): a new trail record (the first free of 30) at D+0x250
/// following D+0x6850, height D+0x6880; 1, or 0 when all are in use.
fn spawn_trail(fx: &mut Fx) -> i32 {
    fx.enter(0x40);
    let mut rec = 0;
    for k in 0..30u32 {
        let r = D + 0x2D0 + 0x98 * k;
        if fx.mem.i16(r + 0x88) == 0 {
            fx.bzero(r, 0x98);
            fx.mem.set16(r + 0x88, 1);
            rec = r;
            break;
        }
    }
    if rec == 0 {
        fx.leave(0x40);
        return 0;
    }
    for k in 0..4 {
        let w = fx.mem.read_u32(D + 0x250 + 4 * k);
        fx.mem.write_u32(rec + 0x30 + 4 * k, w);
    }
    let v = fx.mem.i16(D + 0x68BC);
    fx.mem.set16(rec + 0x92, v);
    let prev = fx.mem.read_u32(D + 0x6850);
    fx.mem.write_u32(rec + 0x70, prev);
    let v = fx.mem.i16(D + 0x68C2);
    fx.mem.set16(rec + 0x8E, v);
    let h = fx.mem.f(D + 0x6880);
    fx.mem.setf(rec + 4, h);
    fx.mem.setf(rec + 0x14, ee::sub(h, 18.0));
    let prev = fx.mem.read_u32(D + 0x6850);
    fx.mem.write_u32(rec + 0x70, prev);
    if prev != 0 {
        fx.mem.write_u32(prev + 0x74, rec);
    }
    fx.mem.set16(rec + 0x94, 1);
    fx.mem.set16(rec + 0x96, 1);
    fx.mem.write_u32(D + 0x6850, rec);
    fx.leave(0x40);
    1
}

/// 0x1E03510(D, step, arg): a new task (the first free of 150) with the
/// colour D+0x6844, at D+0x250, angle D+0x68BC, index D+0x68C2; 1, or 0.
fn spawn_task(fx: &mut Fx, step: u32, arg: u32) -> i32 {
    fx.enter(0x60);
    let mut t = 0;
    for k in 0..150u32 {
        let r = D + 0x14A0 + 0x70 * k;
        if fx.mem.i16(r + 0x58) == 0 {
            fx.bzero(r, 0x70);
            fx.mem.set16(r + 0x58, 1);
            t = r;
            break;
        }
    }
    if t == 0 {
        fx.leave(0x60);
        return 0;
    }
    let (r, g, b) = (fx.mem.u8(D + 0x6844), fx.mem.u8(D + 0x6845), fx.mem.u8(D + 0x6846));
    l2::rgb(fx, t + 0x60, r, g, b);
    let a = fx.mem.read_u8(D + 0x6847);
    fx.mem.write_u8(t + 0x63, a);
    let (r, g, b) = (fx.mem.u8(D + 0x6844), fx.mem.u8(D + 0x6845), fx.mem.u8(D + 0x6846));
    l2::rgb(fx, t + 0x64, r, g, b);
    let a = fx.mem.read_u8(D + 0x6847);
    fx.mem.write_u8(t + 0x67, a);
    l2::to_i16x4(fx, t, D + 0x250, 0);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x250 + 4 * k);
        fx.mem.write_u32(t + 0x10 + 4 * k, w);
    }
    let v = fx.mem.i16(D + 0x68BC);
    fx.mem.set16(t + 0x5A, v);
    let v = fx.mem.i16(D + 0x68C2);
    fx.mem.set16(t + 0x50, v);
    fx.mem.write_u32(t + 0x40, arg);
    fx.mem.write_u32(t + 0x44, step);
    fx.leave(0x60);
    1
}

/// 0x1E01BA0(D, rec): a trail rising round the ten spinning points
/// D+0x5710 (+0x10 each): its ends +0x40 / +0x50 snapped to the first
/// segment above its height +4, widths +0x8A / +0x8C grown, the head
/// rising to 80 then the tail +0x14 following; at the end the record is
/// freed, a live one spawns the next (once).
fn trail_step(fx: &mut Fx, rec: u32) {
    let sp = fx.enter(0x90);
    fx.mem.set32(sp + 0x64, 0);
    l2::rgb(fx, rec + 0x80, 0x78, 0x78, 0x20);
    l2::rgb(fx, rec + 0x84, 0x78, 0x78, 0x20);
    for k in 0..3 {
        fx.mem.set32(rec + 0x30 + 4 * k, 0);
    }
    if crate::exe::ee_lt(fx.mem.f(rec + 4), 100.0) {
        for a in [rec + 0x83, rec + 0x87] {
            let v = fx.mem.u8(a);
            if v < 0x50 {
                fx.mem.set8(a, v + 0xA);
            }
        }
    } else {
        for a in [rec + 0x83, rec + 0x87] {
            let v = fx.mem.u8(a);
            if v >= 0x11 {
                fx.mem.set8(a, v - 0xE);
            }
        }
    }
    let mut s0 = 1i32;
    while s0 < 8 {
        let p = D + 0x10 * s0 as u32;
        if crate::exe::ee_le(fx.mem.f(p + 0x5714), fx.mem.f(rec + 4)) {
            s0 += 1;
            continue;
        }
        let x = fx.mem.read_u32(D + 0x5710);
        fx.mem.write_u32(sp + 0x60, x);
        let z = fx.mem.read_u32(D + 0x5718);
        fx.mem.write_u32(sp + 0x68, z);
        let b = p + 0x5710;
        let a = D + 0x10 * (s0 - 1) as u32 + 0x5710;
        l2::segment_point(fx, sp + 0x70, sp + 0x60, rec, a, b);
        for k in 0..4 {
            let w = fx.mem.read_u32(sp + 0x70 + 4 * k);
            fx.mem.write_u32(rec + 0x40 + 4 * k, w);
        }
        l2::segment_point(fx, sp + 0x80, sp + 0x60, rec, a, b);
        for k in 0..4 {
            let w = fx.mem.read_u32(sp + 0x80 + 4 * k);
            fx.mem.write_u32(rec + 0x50 + 4 * k, w);
        }
        let w = fx.mem.i16(rec + 0x8A);
        if 20 * s0 >= w {
            fx.mem.set16(rec + 0x8A, w + 7);
        }
        let w = fx.mem.i16(rec + 0x8C);
        if 19 * s0 >= w {
            fx.mem.set16(rec + 0x8C, w + 6);
        }
        break;
    }
    let head = fx.mem.f(rec + 4);
    if crate::exe::ee_le(head, 80.0) {
        let head = ee::add(head, 4.0);
        fx.mem.setf(rec + 4, head);
        let tail = fx.mem.f(rec + 0x14);
        if !crate::exe::ee_le(ee::sub(head, tail), 18.0) {
            fx.mem.setf(rec + 0x14, ee::add(tail, 4.0));
        }
    } else {
        let tail = ee::add(fx.mem.f(rec + 0x14), 4.0);
        fx.mem.setf(rec + 0x14, tail);
        if !crate::exe::ee_le(tail, fx.mem.f(rec + 4)) {
            let next = fx.mem.read_u32(rec + 0x74);
            if next != 0 {
                fx.mem.set32(next + 0x70, 0);
            }
            fx.mem.set16(rec + 0x88, 0);
        }
    }
    if fx.mem.i16(rec + 0x88) == 1 && !crate::exe::ee_le(fx.mem.f(rec + 0x14), 0.0) {
        fx.mem.write_u32(D + 0x687C, 0x4160_0000);
        fx.mem.write_u32(D + 0x6850, rec);
        fx.mem.set16(D + 0x68BE, 1);
        let tail = fx.mem.read_u32(rec + 0x14);
        fx.mem.write_u32(D + 0x6880, tail);
        fx.mem.set16(D + 0x68C2, 1);
        let a = fx.mem.i16(rec + 0x92) - 0xF;
        fx.mem.set16(D + 0x68BC, a);
        if fx.mem.i16(D + 0x68C8) < 0x370 && spawn_trail(fx) != 0 {
            fx.mem.set16(rec + 0x88, 2);
        }
    }
    fx.leave(0x90);
}

/// 0x1E03660(D, rec, desc, pass): pass 0 the trail's strip3 header and
/// colours (+0x80 / +0x84); pass 1 its 12 slices (30 degrees each)
/// between the previous record's (+0x70) ring and its own, drawn through
/// four nested scales (TEXEFF 15 rects 0x97 / 0x96).
fn trail_draw(fx: &mut Fx, rec: u32, d: u32, pass: i32) {
    let sp = fx.enter(0x370);
    let prev = fx.mem.read_u32(rec + 0x70);
    if pass == 0 {
        l2::desc_strip3(fx, d);
        rgbaq_at(fx, sp + 0x350, rec + 0x80, d + 0x10);
        rgbaq_at(fx, sp + 0x358, rec + 0x80, d + 0x28);
        rgbaq_at(fx, sp + 0x360, rec + 0x84, d + 0x40);
        rgbaq_at(fx, sp + 0x368, rec + 0x84, d + 0x58);
        fx.leave(0x370);
        return;
    }
    let yaw = if fx.mem.i16(D + 0x68C6) != 0 { 0xC000 } else { 0x4000 };
    fx.mem.set32(sp + 0x240, yaw);
    fx.mem.set32(sp + 0x244, 0);
    fx.mem.set32(sp + 0x248, 0);
    let scale = |fx: &mut Fx, xz: u32, y: u32| {
        fx.mem.write_u32(sp + 0xE0, xz);
        fx.mem.write_u32(sp + 0xE4, y);
        fx.mem.write_u32(sp + 0xE8, xz);
    };
    scale(fx, 0x4080_0000, 0x4000_0000);
    let tex = fx.mem.read_u32(D + 0x2C8);
    fx.desc_texture(d, tex, 1);
    l2::rect(fx, sp + 0xF0, 0x8C, 0);
    l2::uv_strip3(fx, d, sp + 0xF0, 0);
    l2::xform_vp(fx, sp + 0x100, sp + 0xE0, sp + 0x240, D + 0x57B0);
    for (k, (xz, y)) in [(0x4040_0000u32, 0x4140_0000u32), (0x4000_0000, 0x4150_0000), (0x3F80_0000, 0x4160_0000), (0x3F00_0000, 0x4170_0000)].into_iter().enumerate() {
        scale(fx, xz, y);
        let tmp = sp + 0x250 + 0x40 * k as u32;
        l2::xform_vp(fx, tmp, sp + 0xE0, sp + 0x240, D + 0x57B0);
        for i in 0..16 {
            let w = fx.mem.read_u32(tmp + 4 * i);
            fx.mem.write_u32(sp + 0x140 + 0x40 * k as u32 + 4 * i, w);
        }
    }
    let tab = D + 0x69CC;
    let mut s1 = 0i32;
    for _ in 0..12 {
        let ring = |fx: &mut Fx, src: u32, width: u32, base: i32, at: [u32; 4]| {
            let a = (base + fx.mem.i16(src + 0x92)) as i16 as i32;
            let c = l2::sin_deg(fx, tab, a, 1);
            let w = ee::from_i32(fx.mem.i16(src + width));
            fx.mem.setf(sp + at[0], ee::mul(w, c));
            let s = l2::sin_deg(fx, tab, a, 0);
            let w = ee::from_i32(fx.mem.i16(src + width));
            fx.mem.setf(sp + at[1], ee::mul(w, s));
            let a = (base + fx.mem.i16(src + 0x92) - 0x1E) as i16 as i32;
            let c = l2::sin_deg(fx, tab, a, 1);
            let w = ee::from_i32(fx.mem.i16(src + width));
            fx.mem.setf(sp + at[2], ee::mul(w, c));
            let s = l2::sin_deg(fx, tab, a, 0);
            let w = ee::from_i32(fx.mem.i16(src + width));
            fx.mem.setf(sp + at[3], ee::mul(w, s));
        };
        if prev != 0 {
            ring(fx, prev, 0x8C, s1, [0xA0, 0xA8, 0xB0, 0xB8]);
            let tail = fx.mem.read_u32(prev + 0x14);
            fx.mem.write_u32(rec + 4, tail);
            l2::drift(fx, sp + 0xA0, prev + 0x40, rec + 0x40);
        } else {
            ring(fx, rec, 0x8A, s1 as i16 as i32, [0xA0, 0xA8, 0xB0, 0xB8]);
        }
        ring(fx, rec, 0x8C, s1, [0xC0, 0xC8, 0xD0, 0xD8]);
        let (h, t) = (fx.mem.read_u32(rec + 4), fx.mem.read_u32(rec + 0x14));
        fx.mem.write_u32(sp + 0xA4, h);
        fx.mem.write_u32(sp + 0xB4, h);
        fx.mem.write_u32(sp + 0xC4, t);
        fx.mem.write_u32(sp + 0xD4, t);
        for k in 0..4u32 {
            let a = sp + 0xA4 + 0x10 * k;
            if crate::exe::ee_lt(fx.mem.f(a), 0.0) {
                fx.mem.set32(a, 0);
            }
        }
        l2::add_rows(fx, sp + 0xA0, rec + 0x40);
        l2::drift(fx, sp + 0xC0, rec + 0x40, rec + 0x50);
        for s3 in 0..4u32 {
            let m = sp + 0x100 + 0x40 * s3;
            for id in [0x97, 0x96] {
                let tex = fx.mem.read_u32(D + 0x2CC);
                fx.desc_texture(d, tex, 1);
                l2::rect(fx, sp + 0xF0, id, 0);
                l2::uv_strip3(fx, d, sp + 0xF0, 0);
                if l2::project(fx, sp + 0xA0, SPR, m, d, D + 0x6884) {
                    let key = ee::fixunssfsi(fx.mem.f(D + 0x6884)) as i32;
                    let ot = fx.mem.read_u32(D + 0x684C);
                    fx.submit(d, ot, key);
                }
            }
        }
        s1 = (s1 + 0x1E) as i16 as i32;
    }
    fx.leave(0x370);
}

/// The cubic B-spline weights at t (b0..b3) as 0x1E03D80 computes them.
fn bspline(t: f32) -> [f32; 4] {
    let f9 = ee::sub(t, 1.0);
    let f7 = ee::sub(t, 2.0);
    let f10 = ee::add(1.0, t);
    let f2 = ee::mul(f9, f7);
    let b0 = ee::div(ee::mul(ee::neg(t), f2), 6.0);
    let f2 = ee::mul(t, f9);
    let f2 = ee::mul(f10, f2);
    let b3 = ee::div(f2, 6.0);
    let f1 = ee::mul(f10, f9);
    let f1 = ee::mul(f7, f1);
    let b1 = ee::div(f1, 2.0);
    let f7 = ee::mul(t, f7);
    let f2 = ee::mul(ee::neg(f10), f7);
    let b2 = ee::div(f2, 2.0);
    [b0, b1, b2, b3]
}

/// One spline coordinate: ((p0 b0 + p1 b1) + p2 b2) + p3 b3 in the
/// original's ACC order.
fn spline_axis(p: [f32; 4], b: [f32; 4]) -> f32 {
    let acc = ee::mul(p[0], b[0]);
    let f6 = ee::add(acc, ee::mul(p[1], b[1]));
    let acc = ee::add(ee::mul(p[2], b[2]), f6);
    ee::add(acc, ee::mul(p[3], b[3]))
}

/// 0x1E03D80(D, task, pts, mode): a ribbon through the task's trail
/// points `pts` (+6 of them, 0x10 each) as a cubic B-spline, two segments
/// per span, width +0x48 / 10 tapering, alpha fading (+0x63 / 2n per
/// step); depth mode 0 (mean w) or 1 (far Z, key 0).
pub(crate) fn ribbon(fx: &mut Fx, t: u32, pts: u32, mode: i32) {
    let sp = fx.enter(0x350);
    let mut f30 = ee::div(fx.mem.f(t + 0x48), 10.0);
    let ot = crate::exe::OT;
    let d = sp + 0x298;
    let n = fx.mem.i16(t + 6);
    fx.mem.setf(sp + 0xD0, f30);
    l2::desc_strip3(fx, d);
    rgbaq_at(fx, sp + 0x328, t + 0x60, d + 0x10);
    rgbaq_at(fx, sp + 0x330, t + 0x64, d + 0x28);
    rgbaq_at(fx, sp + 0x338, t + 0x60, d + 0x40);
    rgbaq_at(fx, sp + 0x340, t + 0x64, d + 0x58);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    let rgba = sp + 0x34C;
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(rgba + k, b);
    }
    let copy16 = |fx: &mut Fx, to: u32, from: u32| {
        for k in 0..4 {
            let w = fx.mem.read_u32(from + 4 * k);
            fx.mem.write_u32(to + 4 * k, w);
        }
    };
    copy16(fx, sp + 0x280, pts);
    copy16(fx, sp + 0x150, sp + 0x280);
    copy16(fx, sp + 0x160, sp + 0x280);
    copy16(fx, sp + 0x170, pts + 0x10);
    copy16(fx, sp + 0x180, pts + 0x20);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0x1E0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x200, 0, 0, sp + 0x1E0);
    let p0x = fx.mem.f(sp + 0x160);
    fx.mem.setf(sp + 0xD4, p0x);
    fx.mem.set32(sp + 0x100, n * 2);
    fx.mem.set32(sp + 0x110, n - 1);
    let p1x = fx.mem.f(sp + 0x170);
    fx.mem.setf(sp + 0xD8, p1x);
    let first: [[f32; 4]; 3] = std::array::from_fn(|c| {
        let o = 4 * c as u32;
        [fx.mem.f(sp + 0x150 + o), fx.mem.f(sp + 0x160 + o), fx.mem.f(sp + 0x170 + o), fx.mem.f(sp + 0x180 + o)]
    });
    let steps = ee::mul(2.5, ee::from_i32(n));
    fx.mem.setf(sp + 0xDC, steps);
    let mut i = 0i32;
    while i < fx.mem.i32(sp + 0x110) {
        let mut f29 = 0.0f32;
        let s3 = pts.wrapping_add((i << 4) as i16 as i32 as u32);
        for j in 0..2 {
            let spline_at = |fx: &mut Fx, f29: f32, out: u32| {
                let b = bspline(f29);
                if i == 0 {
                    // P-1 = P0 (the first point twice); x from the saved
                    // locals, y and z from the registers the code keeps.
                    let x = spline_axis([fx.mem.f(sp + 0x150), fx.mem.f(sp + 0xD4), fx.mem.f(sp + 0xD8), first[0][3]], b);
                    fx.mem.setf(out, x);
                    fx.mem.setf(out + 4, spline_axis(first[1], b));
                    fx.mem.setf(out + 8, spline_axis(first[2], b));
                } else {
                    for c in 0..3u32 {
                        let p = [fx.mem.f(s3 - 0x10 + 4 * c), fx.mem.f(s3 + 4 * c), fx.mem.f(s3 + 0x10 + 4 * c), fx.mem.f(s3 + 0x20 + 4 * c)];
                        fx.mem.setf(out + 4 * c, spline_axis(p, b));
                    }
                }
            };
            if i == 0 && j == 0 {
                spline_at(fx, f29, sp + 0x260);
                copy16(fx, sp + 0x130, sp + 0x260);
            } else {
                copy16(fx, sp + 0x130, sp + 0x190);
            }
            f29 = ee::add(f29, 0.5);
            if !crate::exe::ee_lt(f29, f32::from_bits(0x3F66_6666)) {
                f29 = 1.0;
            }
            let end = if i == 0 { sp + 0x270 } else { sp + 0x250 };
            spline_at(fx, f29, end);
            copy16(fx, sp + 0x140, end);
            copy16(fx, sp + 0x190, sp + 0x140);
            l2::colour_strip3(fx, d, rgba, 1);
            let a = i32::from(fx.mem.read_u8(rgba + 3));
            let alpha = fx.mem.u8(t + 0x63);
            let n2 = fx.mem.i32(sp + 0x100);
            let (q, _) = fx.div(alpha, n2);
            let v = a - (q as i16 as i32);
            fx.mem.write_u8(rgba + 3, if v > 0 { v as u8 } else { 0 });
            l2::colour_strip3(fx, d, rgba, 2);
            let f31 = f30;
            f30 = ee::sub(f30, ee::div(fx.mem.f(sp + 0xD0), fx.mem.f(sp + 0xDC)));
            for s0 in 0..2 {
                l2::copy3(fx, sp + 0x1A0, sp + 0x130);
                l2::copy3(fx, sp + 0x1B0, sp + 0x130);
                l2::copy3(fx, sp + 0x1C0, sp + 0x140);
                l2::copy3(fx, sp + 0x1D0, sp + 0x140);
                let addf = |fx: &mut Fx, a: u32, v: f32, sub: bool| {
                    let x = fx.mem.f(sp + a);
                    fx.mem.setf(sp + a, if sub { ee::sub(x, v) } else { ee::add(x, v) });
                };
                if s0 == 0 {
                    addf(fx, 0x1B4, f31, false);
                    addf(fx, 0x1D4, f30, false);
                    addf(fx, 0x1A4, f31, true);
                    addf(fx, 0x1C4, f30, true);
                } else {
                    let f2 = ee::div(f31, 2.0);
                    addf(fx, 0x1B0, f31, false);
                    let f1 = ee::div(f30, 2.0);
                    addf(fx, 0x1D0, f30, false);
                    addf(fx, 0x1B8, f2, false);
                    addf(fx, 0x1D8, f1, false);
                    addf(fx, 0x1A0, f31, true);
                    addf(fx, 0x1C0, f30, true);
                    addf(fx, 0x1A8, f2, true);
                    addf(fx, 0x1C8, f1, true);
                }
                if mode <= 1 && l2::project(fx, sp + 0x1A0, SPR, sp + 0x200, d, sp + 0x1F0) {
                    if mode == 0 {
                        let key = ee::fixunssfsi(fx.mem.f(sp + 0x1F0)) as i32;
                        fx.submit(d, ot, key);
                    } else {
                        l2::far_z(fx, &[d + 0x68, d + 0x50, d + 0x38, d + 0x20]);
                        fx.submit(d, ot, 0);
                    }
                }
            }
            let _ = j;
        }
        i += 1;
    }
    fx.leave(0x350);
}

/// 0x1E01F20: the Lord's glow (two sprite groups on nodes 1 and 0):
/// size 8 x 300 growing by 26, alpha up by 0x40 for 4 updates then down
/// by 9 (ending at 10 or less), turning 6 per update.
fn step_glow(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0x70, 0x70, 0x70);
        l2::rgb(fx, t + 0x64, 0xFF, 0x20, 0x20);
        fx.mem.write_u32(t + 0x30, 0x4100_0000);
        fx.mem.write_u32(t + 0x34, 0x4396_0000);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 4;
    fade(fx, t, rising, 0x80, 0x40, 0xB, 9);
    if fx.mem.i16(t + 0x54) != 0 {
        let h = ee::add(fx.mem.f(t + 0x34), 26.0);
        fx.mem.setf(t + 0x34, h);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
    fx.leave(0x20);
}

/// The alpha byte +0x63: while `rising`, + `up` when below `cap`; else -
/// `down` while at least `floor`, the task ended (+0x58 = 0) below it.
pub(crate) fn fade(fx: &mut Fx, t: u32, rising: bool, cap: i32, up: i32, floor: i32, down: i32) {
    let a = fx.mem.u8(t + 0x63);
    if rising {
        if a < cap {
            fx.mem.set8(t + 0x63, a + up);
        }
    } else if a >= floor {
        fx.mem.set8(t + 0x63, a - down);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The 11-bit random angle step (rand & 0x7FF) - 0x400.
pub(crate) fn rand_step(fx: &mut Fx) -> i32 {
    let r = fx.rand();
    (r & 0x7FF) - 0x400
}

/// 0x1E02030: a burst sprite flying off at a random velocity (-50..49) /
/// 60, falling 1.5 per update, growing to 300, spinning randomly; its
/// angles are then reset to the camera's (0x205580).
fn step_burst(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xC8);
        let v = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, v);
        let r = fx.rand();
        let (_, m) = fx.div(r >> 4, 3);
        fx.mem.set16(t + 0x56, m);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x40);
        fx.mem.set16(t + 0x58, 2);
        for o in [0x34u32, 0x30, 0x38] {
            let r = fx.rand();
            let (_, m) = fx.div(r, 0x64);
            fx.mem.setf(t + o, ee::from_i32(m - 0x32));
        }
        for o in [0x34u32, 0x30, 0x38] {
            let v = ee::div(fx.mem.f(t + o), 60.0);
            fx.mem.setf(t + o, v);
        }
        let r = fx.rand();
        let (_, m) = fx.div(r, 0x168);
        fx.mem.set16(t + 0x5C, m);
        l2::camera_angles_to(fx, t + 0x20);
        for o in [4u32, 2, 0] {
            let v = rand_step(fx);
            fx.mem.set16(t + o, v);
        }
        let r = fx.rand();
        let v = r >> 4;
        fx.mem.set16(t + 0x52, v % 4);
        fx.mem.set16(t + 6, 0xA);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, rising, 0x80, 0x40, 0xB, 9);
    if fx.mem.i16(t + 0x54) != 0 {
        let w = fx.mem.i16(t + 8);
        if w < 0x12C {
            fx.mem.set16(t + 8, w + 5);
        }
        let w = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, w);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        for (p, v) in [(0x10u32, 0x30u32), (0x14, 0x34), (0x18, 0x38)] {
            let x = ee::add(fx.mem.f(t + p), fx.mem.f(t + v));
            fx.mem.setf(t + p, x);
        }
        let y = ee::sub(fx.mem.f(t + 0x14), 1.5);
        fx.mem.setf(t + 0x14, y);
    }
    for (a, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.i32(t + a).wrapping_add(fx.mem.i16(t + s));
        fx.mem.set32(t + a, v);
    }
    l2::camera_angles_to(fx, t + 0x20);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E02320: a flash of sparks: kind +0x50 0 (radius 20 growing by 180,
/// pulsing yellow) or 1 (radius 3000 shrinking by 250, pulsing blue).
fn step_flash(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x48, 0x41A0_0000);
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
            fx.mem.set32(t + 0x4C, 0);
            fx.mem.set16(t + 0x5A, 0x3C);
            fx.mem.set16(t + 0x5C, 3);
            for o in [4u32, 2, 0] {
                let v = rand_step(fx);
                fx.mem.set16(t + o, v);
            }
            fx.mem.set16(t + 6, 0xF);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x48, 0x453B_8000);
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
            fx.mem.set32(t + 0x4C, 0);
            fx.mem.set16(t + 0x5A, 0x3C);
            fx.mem.set16(t + 0x5C, 2);
            fx.mem.set16(t + 0x56, 3);
            fx.mem.set16(t + 6, 7);
        }
        fx.mem.set16(t + 0x56, 4);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for o in [0x20u32, 0x24, 0x28] {
            let r = fx.rand();
            let (_, m) = fx.div(r, 0xFFFF);
            fx.mem.set32(t + o, m);
        }
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, rising, 0xC0, 0x40, 0xA, 8);
    let odd = |fx: &mut Fx| fx.mem.i16(t + 0x54) % 2 == 1;
    if fx.mem.i16(t + 0x50) == 0 {
        if odd(fx) {
            l2::rgb(fx, t + 0x64, 0xA0, 0xA0, 0x60);
        } else {
            l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0x60);
        }
        let r = ee::add(fx.mem.f(t + 0x48), 180.0);
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3E4C_CCCD), r));
        for (a, s) in [(0x20u32, 0u32), (0x24, 2), (0x28, 4)] {
            let v = fx.mem.i32(t + a).wrapping_add(fx.mem.i16(t + s));
            fx.mem.set32(t + a, v);
        }
    }
    if fx.mem.i16(t + 0x50) == 1 {
        if odd(fx) {
            l2::rgb(fx, t + 0x64, 0x60, 0x60, 0xA0);
        } else {
            l2::rgb(fx, t + 0x64, 0x60, 0x60, 0xC0);
        }
        let r = ee::sub(fx.mem.f(t + 0x48), 250.0);
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, ee::mul(0.5, r));
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E02660: a ring of random sprites rising 3.3 per update at the
/// Lord's height (D+0x56B8), random angles, sizes and radii.
fn step_ring(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4396_0000);
        let r = fx.rand();
        fx.mem.set16(t + 0x56, (r >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x60);
        for o in [0x5Au32, 0x5C] {
            let r = fx.rand();
            let (_, m) = fx.div(r, 0x168);
            fx.mem.set16(t + o, m);
        }
        let r = fx.rand();
        fx.mem.set16(t + 0xA, if r % 2 != 0 { 2 } else { 0 });
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xC, v);
        for (o, m, add) in [(0x50u32, 0x15E, 0x32), (0x52, 0x15E, 0x64), (0xE, 0x14, 0xA), (8, 0x23, 0), (6, 5, 3)] {
            let r = fx.rand();
            let (_, x) = fx.div(r, m);
            fx.mem.set16(t + o, x + add);
        }
        for o in [0x30u32, 0x34, 0x38] {
            let r = fx.rand();
            let (_, x) = fx.div(r, 0x28);
            fx.mem.setf(t + o, ee::from_i32(x - 0x14));
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(D + 0x68C8) < 0x1F4 {
        let rising = fx.mem.i16(t + 0x54) < 0x37;
        fade(fx, t, rising, 0xC0, 0xC0, 0xD, 0xB);
    } else {
        fade(fx, t, false, 0, 0, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let y = ee::add(fx.mem.f(t + 0x14), f32::from_bits(0x4053_3333));
        fx.mem.setf(t + 0x14, y);
    }
    let z = ee::add(fx.mem.f(D + 0x56B8), fx.mem.f(t + 0x38));
    fx.mem.setf(t + 0x18, z);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E02940: a ribbon head wandering (speed +0x4C = 4, turning +0x5A by
/// 20 or 10 per update by its index's parity), sinking or rising 8 per
/// update by side, its 13-point trail (D+0x57E0 + 0xD0 index) following;
/// it leads the spawn point D+0x250 for 300 updates.
fn step_ribbon(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    let trail = |fx: &mut Fx| D.wrapping_add((fx.mem.i16(t + 0x50) * 0xD0) as u32);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x64);
        fx.mem.set16(t + 0xA, 0x64);
        let r = fx.rand();
        fx.mem.set16(t + 0x56, r % 2);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        fx.mem.write_u32(t + 0x48, 0x4150_0000);
        fx.mem.write_u32(t + 0x4C, 0x4080_0000);
        let base = trail(fx) + 0x57E0;
        for k in 0..13u32 {
            l2::copy3(fx, base + 0x10 * k, t + 0x10);
        }
    }
    if fx.mem.i16(t + 0x54) < 0x12C {
        for a in [t + 0x63, t + 0x67] {
            let v = fx.mem.u8(a);
            if v < 0xC0 {
                fx.mem.set8(a, v + 0x13);
            }
        }
    } else {
        let v = fx.mem.u8(t + 0x63);
        if v < 0x10 {
            fx.mem.set16(t + 0x58, 0);
        } else {
            fx.mem.set8(t + 0x63, v - 0xE);
        }
        let v = fx.mem.u8(t + 0x63);
        if v >= 0x10 {
            fx.mem.set8(t + 0x63, v - 0xE);
        }
    }
    let f = fx.mem.i16(t + 0x56);
    fx.mem.set16(t + 0x56, i32::from(f == 0));
    if fx.mem.i16(t + 0x54) != 0 {
        let tab = D + 0x69CC;
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, tab, a, 0);
        let y = ee::add(fx.mem.f(t + 0x14), ee::mul(fx.mem.f(t + 0x4C), s));
        fx.mem.setf(t + 0x14, y);
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, tab, a, 1);
        let x = ee::add(fx.mem.f(t + 0x10), ee::mul(fx.mem.f(t + 0x4C), c));
        fx.mem.setf(t + 0x10, x);
        let idx = fx.mem.i16(t + 0x50);
        let step = if idx < 8 { 0x14 } else { 0xA };
        let a = fx.mem.i16(t + 0x5A);
        fx.mem.set16(t + 0x5A, if idx % 2 != 0 { a + step } else { a - step });
    }
    let z = fx.mem.f(t + 0x18);
    let z = if fx.mem.i16(D + 0x68C6) != 0 { ee::sub(z, 8.0) } else { ee::add(z, 8.0) };
    fx.mem.setf(t + 0x18, z);
    if fx.mem.i16(t + 0x54) < 0x12C {
        for k in 0..4 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
    }
    if fx.mem.i16(t + 0x54) % 2 == 0 {
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
    } else {
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
    }
    let last = trail(fx) + 0x58A0;
    shift_points(fx, last, 0xD, t + 0x10);
    l2::camera_angles_to(fx, t + 0x20);
    if fx.mem.i16(t + 0x54) != 0 && fx.mem.i16(t + 6) < 0xC {
        let v = fx.mem.i16(t + 6) + 1;
        fx.mem.set16(t + 6, v);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

impl EffectPort for C061 {
    fn update(&mut self, world: &dyn EffectWorld) -> EffectUpdate {
        if self.fx.finished {
            return EffectUpdate { ret: 8, ..Default::default() };
        }
        self.fx.begin(world);
        let ret = if self.fx.calls == 0 { self.init() } else { self.step() };
        for role in 0..2 {
            let o = crate::world::OBJECT_RECORD[role];
            let m = &mut self.fx.mem;
            self.motion[role] = (m.read_u8(o + 0x1E0), m.read_f32(o + 0x19C), m.read_u8(o + 0x1FB));
        }
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
    fn object_motion(&self, role: usize) -> Option<(u8, f32, u8)> {
        self.motion.get(role).copied()
    }
    fn bus(&mut self) -> Option<&mut dyn Bus> {
        Some(&mut self.fx.mem)
    }
}
