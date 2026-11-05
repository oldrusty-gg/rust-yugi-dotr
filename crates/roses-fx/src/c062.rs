//! CMODULE c062 (program 0x3E), the second of the Exodia finale's three
//! modules, hand-ported from the module (linked at 0x1E00000; entry
//! 0x1E04450, data area 0x1E10000; 5,632 words, 4,776 of them its own) [F]
//! (helper A, 30 Oct 2026). It shares c061's sprite routines
//! ([`crate::c061`]) and the second effect library ([`crate::fxlib2`]).
//!
//! Launch: the data area cleared (0x6CB0 bytes), degree sine tables
//! (+0xD8, +0x6B40), TEXEFF 15 (+0x29C), TEXEFF... type 3 record 0
//! (+0x685C), the side (+0x693A = self handle != 0, +0x6938 the other),
//! random byte tables (+0x693E, +0x6A3E), the five limbs (0x1E039C0: the
//! units the target handle packs as slots k0..k3 plus the leader, their
//! board card objects lifted and turned), the Lord object 0x20 lowered by
//! 25, five target points (+0x67B0) 80 above the far side.
//! Each update (frame +0x693C): 200 tasks (+0x2A0, 0x70 each) stepped and
//! drawn by kind (beams between the limbs, ring trails, flashes, sparks,
//! sparkle rings, a frame-buffer tint (0x207EF0)); the opponent's units
//! fade; the limbs fly to the targets and round them (0x1E03BC0) through
//! the stage counter +0x68FC, which spawns the effects. Returns 8 when the
//! countdown of 0x3B9 ends.
//!
//! [F] The limbs' card objects are the board's (unit records 0x2DB990 +4);
//! the effect runner does not mirror them, so in the game as in the
//! parity runs 0x1F6D00 finds none and the limbs' machine waits
//! (the parity test places card objects to reach it).
use crate::bus::{Bus, Image};
use crate::c061::{blend_alpha, desc_uv4, fade, rgbaq_at, scale_tenth, set_rect, sprite_quads, to_degrees_neg, uv_strip4};
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;
use crate::types::{EffectTextures, EffectUpdate, EffectWorld};
use crate::EffectPort;
use std::sync::Arc;

/// CMODULE program id.
pub const ID: u16 = 0x3E;
/// Its entry in the type-0 table 0x2B8510.
pub const ENTRY: u32 = 0x01E0_4450;
const D: u32 = 0x01E1_0000;
const LORD: i32 = 0x20;
const SPR: u32 = 0x7000_0000;
/// The limbs' index pairs (5 x 2 i32) and connection table (i16 pairs,
/// then the order) in the module's rodata.
const BEAM_PAIRS: u32 = 0x01E0_5880;
const LINK_PAIRS: u32 = 0x01E0_58B0;
const LINK_ORDER: u32 = 0x01E0_58C8;
const STEP_BEAMS: u32 = 0x01E0_24F0;
const STEP_LINKS: u32 = 0x01E0_2650;
const STEP_TRAILS: u32 = 0x01E0_2780;
const STEP_FLASH: u32 = 0x01E0_2B20;
const STEP_SPARKS: u32 = 0x01E0_2E40;
const STEP_SPARKLE: u32 = 0x01E0_3070;
const STEP_RING: u32 = 0x01E0_3210;
const STEP_GLOW: u32 = 0x01E0_33E0;
const STEP_TINT: u32 = 0x01E0_3550;

/// A running c062.
pub struct C062 {
    fx: Fx,
    motion: [(u8, f32, u8); 2],
}

impl C062 {
    pub fn load(executable: &[u8], cmodule: &[u8], handles: [u16; 2], textures: &EffectTextures) -> Option<Self> {
        let image = Arc::new(Image::from_elf(executable)?);
        Self::load_with(image, cmodule, handles, textures)
    }

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

    /// As `roses_ee::bmodule::AttackEffect::place_role`.
    pub fn place_role(&mut self, role: usize, index: u8, motions: &[(u8, u32)]) {
        crate::world::place_role(&mut self.fx.mem, role, index, motions);
    }

    /// The launch (0x1E04494..0x1E04798); `param` the launch parameter.
    fn init(&mut self, param: i32) -> i32 {
        let fx = &mut self.fx;
        let sp = fx.enter(0x140);
        fx.mem.write_u32(D + 0x684C, OT);
        fx.mem.write_u32(D + 0x80, OT);
        fx.bzero(D, 0x6CB0);
        l2::sine_table(fx, D + 0x6B40);
        l2::sine_table(fx, D + 0xD8);
        fx.mem.set16(D + 0x693A, param);
        fx.mem.set32(D + 0x6840, 0x3B9);
        let t = fx.texture(4, 15);
        fx.mem.write_u32(D + 0x29C, t);
        let t = fx.texture(3, 0);
        fx.mem.write_u32(D + 0x685C, t);
        let own = fx.self_handle();
        fx.mem.set16(D + 0x693A, i32::from(own != 0));
        let side = fx.mem.i16(D + 0x693A);
        fx.mem.set16(D + 0x6938, i32::from(side == 0));
        for k in 0..3 {
            fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
        }
        for k in 0..0x100u32 {
            let r = fx.rand();
            let (_, m) = fx.div(r, 0x1E);
            fx.mem.set8(D + 0x693E + k, m - 0xF);
        }
        for k in 0x64..0xB4u32 {
            let r = fx.rand();
            fx.mem.set8(D + 0x693E + k, r % 4);
        }
        for k in 0xB4..0x100u32 {
            let r = fx.rand();
            fx.mem.set8(D + 0x693E + k, (r & 0xF) - 8);
        }
        for k in 0..0x100u32 {
            let r = fx.rand();
            let (_, m) = fx.div(r, 0x28);
            fx.mem.set8(D + 0x6A3E + k, m - 0x14);
        }
        limbs_setup(fx);
        fx.object_position(LORD, sp + 0x70);
        fx.mem.write_u32(sp + 0x74, 0xC1C8_0000);
        fx.object_set_position(LORD, sp + 0x70);
        let side = fx.mem.i16(D + 0x693A);
        if side == 0 {
            fx.mem.write_u32(D + 0x6808, 0x43AF_0000);
        } else if side == 1 {
            fx.mem.write_u32(D + 0x6808, 0xC3AF_0000);
        }
        let x = fx.mem.f(D + 0x6808);
        if side == 0 {
            let f1 = ee::sub(x, 50.0);
            fx.mem.setf(D + 0x6808, f1);
            fx.mem.setf(D + 0x6818, ee::sub(f1, 30.0));
        } else if side == 1 {
            let f1 = ee::add(x, 50.0);
            fx.mem.setf(D + 0x6808, f1);
            fx.mem.setf(D + 0x6818, ee::add(30.0, f1));
        }
        fx.mem.write_u32(D + 0x6814, 0x42C8_0000);
        for k in 0..5u32 {
            let z = fx.mem.f(D + 0x6818);
            l2::set3(fx, D + 0x67B0 + 0x10 * k, 0.0, 80.0, z);
        }
        fx.leave(0x140);
        0
    }

    /// An update (0x1E0479C..).
    fn step(&mut self) -> i32 {
        let fx = &mut self.fx;
        let sp = fx.enter(0x140);
        fx.mem.write_u32(D + 0x684C, OT);
        fx.mem.write_u32(D + 0x80, OT);
        let f = fx.mem.i16(D + 0x693C) + 1;
        fx.mem.set16(D + 0x693C, f);
        if f == 1 {
            fx.mem.set16(D + 0x6934, 0);
            spawn(fx, STEP_FLASH, 0x01E0_47D0);
            spawn(fx, STEP_TINT, 0x01E0_47E4);
        }
        for k in 0..200u32 {
            let t = D + 0x2A0 + 0x70 * k;
            if fx.mem.i16(t + 0x58) != 0 {
                let f = fx.mem.read_u32(t + 0x44);
                if f != 0 {
                    run_step(fx, f, t);
                }
            }
        }
        for k in 0..200u32 {
            let t = D + 0x2A0 + 0x70 * k;
            if fx.mem.i16(t + 0x58) != 0 {
                draw_task(fx, sp, t);
            }
        }
        if fx.mem.i16(D + 0x693C) < 0xA {
            fade_board(fx);
        }
        for k in 0..5u32 {
            limb_step(fx, D + 0x5A50 + 0x70 * k, k);
        }
        stages(fx, sp);
        if fx.mem.i16(D + 0x693C) >= 0x369 {
            for k in 0..0x16i32 {
                let h = if k < 0xB { k } else { k + 0xF5 };
                card_fade_in(fx, h, 0x18);
                card_grow(fx, h, f32::from_bits(0x3E19_999A));
                saved_ra(fx, 0x01E0_5468);
                fx.card_set_rotation(h, D + 0x6460 + 0x10 * k as u32);
            }
        }
        let n = fx.mem.i16(D + 0x6924);
        if n != 0 {
            fx.mem.set16(D + 0x6924, n - 1);
        }
        let n = fx.mem.i32(D + 0x6840);
        if n != 0 {
            fx.mem.set32(D + 0x6840, n - 1);
        }
        let n = fx.mem.i32(D + 0x6840);
        fx.leave(0x140);
        if n > 0 {
            0
        } else {
            8
        }
    }
}

/// The opponent's units shrink (0x107A60) and fade (0x107B60) for the
/// first 9 updates, the five limbs excepted (0x1E04BF4..).
fn fade_board(fx: &mut Fx) {
    for k in 0..0x16i32 {
        let h = if k < 0xB { k } else { k + 0xF5 };
        let side = fx.mem.i16(D + 0x693A);
        let mine = (side == 0 && k < 0xB) || (side == 1 && (0xB..0x16).contains(&k));
        if mine {
            card_shrink(fx, h);
        }
        let limb = (0..5u32).any(|i| fx.mem.i32(D + 0x687C + 4 * i) == h);
        if !limb && mine {
            card_fade_out(fx, h);
        }
    }
}

/// The stage counter's spawns and moves (0x1E04D7C..0x1E05404).
fn stages(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D + 0x68FC) == 5 {
        for k in 0..5u32 {
            fx.mem.set32(D + 0x5AB4 + 0x70 * k, 2);
        }
        spawn(fx, STEP_TRAILS, 0x01E0_4DD8);
        let n = fx.mem.i32(D + 0x68FC) + 1;
        fx.mem.set32(D + 0x68FC, n);
    }
    let point = |fx: &mut Fx, k: i32| {
        for w in 0..4u32 {
            let v = fx.mem.read_u32(D + 0x67B0 + 0x10 * k as u32 + 4 * w);
            fx.mem.write_u32(D + 0x250 + 4 * w, v);
        }
    };
    let glows = |fx: &mut Fx, ra: [u32; 4]| {
        fx.mem.set16(D + 0x6934, 0);
        fx.mem.set16(D + 0x6928, 0);
        spawn(fx, STEP_GLOW, ra[0]);
        fx.mem.set16(D + 0x6928, 0x5A);
        spawn(fx, STEP_GLOW, ra[1]);
        fx.mem.set16(D + 0x6934, 1);
        fx.mem.set16(D + 0x6928, 0x2D);
        spawn(fx, STEP_GLOW, ra[2]);
        fx.mem.set16(D + 0x6928, 0x87);
        spawn(fx, STEP_GLOW, ra[3]);
    };
    let targets = |fx: &mut Fx, x: u32| {
        fx.mem.write_u32(sp + 0x70, x);
        let y = fx.mem.read_u32(D + 0x6814);
        fx.mem.write_u32(sp + 0x74, y);
        let z = fx.mem.read_u32(D + 0x6818);
        fx.mem.write_u32(sp + 0x78, z);
        let side = fx.mem.i16(D + 0x693A);
        target_points(fx, D + 0x67B0, sp + 0x70, side);
    };
    let a = fx.mem.i32(D + 0x68FC);
    if a == 0xB {
        point(fx, 0);
        spawn(fx, STEP_BEAMS, 0x01E0_4E28);
        spawn(fx, STEP_LINKS, 0x01E0_4E3C);
        fx.mem.set16(D + 0x6934, 1);
        spawn(fx, STEP_FLASH, 0x01E0_4E5C);
        fx.mem.set16(D + 0x6934, 0);
        spawn(fx, STEP_SPARKS, 0x01E0_4E74);
        glows(fx, [0x01E0_4E90, 0x01E0_4EAC, 0x01E0_4ED0, 0x01E0_4EEC]);
        targets(fx, 0x42C8_0000);
        fx.mem.set16(D + 0x6934, 0);
        fx.mem.set16(D + 0x6924, 0xA);
        fx.mem.set32(D + 0x68FC, 0xC);
    } else if a == 0x11 {
        fx.mem.set32(D + 0x68FC, 0x12);
    } else if (0x12..0x29).contains(&a) {
        let (index, first) = if a >= 0x26 {
            (1, 0x26)
        } else if a >= 0x1F {
            (3, 0x1F)
        } else if a >= 0x1C {
            (4, 0x1C)
        } else if a >= 0x19 {
            (2, 0x19)
        } else {
            (0, 0x12)
        };
        fx.mem.set16(D + 0x6934, index);
        let s1 = if fx.mem.i32(D + 0x68FC) == first { 1 } else { 2 };
        let at = fx.mem.i16(D + 0x6934);
        if s1 == 2 {
            point(fx, at);
            spawn(fx, STEP_RING, 0x01E0_50F4);
        }
        if s1 == 1 {
            point(fx, at);
            spawn(fx, STEP_RING, 0x01E0_5140);
            glows(fx, [0x01E0_515C, 0x01E0_5178, 0x01E0_519C, 0x01E0_51B8]);
        }
        let n = fx.mem.i32(D + 0x68FC) + 1;
        fx.mem.set32(D + 0x68FC, n);
    } else if (0x29..0x3D).contains(&a) {
        let n = fx.mem.i32(D + 0x68FC) + 1;
        fx.mem.set32(D + 0x68FC, n);
    } else if a == 0x3D {
        targets(fx, 0x4316_0000);
        let n = fx.mem.i32(D + 0x68FC) + 1;
        fx.mem.set32(D + 0x68FC, n);
    } else if a == 0x43 {
        fx.mem.set16(D + 0x6924, 0xA);
        for k in 0..5 {
            fx.mem.set16(D + 0x6934, k);
        }
        let n = fx.mem.i32(D + 0x68FC) + 1;
        fx.mem.set32(D + 0x68FC, n);
    }
    let v = fx.mem.i32(D + 0x68FC);
    if v < 0x2F {
        return;
    }
    if v == 0x2F {
        fx.mem.set32(D + 0x250, 0);
        fx.mem.write_u32(D + 0x254, 0x42C8_0000);
        fx.mem.set32(D + 0x258, 0);
    }
    fx.mem.set32(D + 0x250, 0);
    let y = fx.mem.read_u32(D + 0x6814);
    fx.mem.write_u32(D + 0x254, y);
    let z = fx.mem.read_u32(D + 0x6818);
    fx.mem.write_u32(D + 0x258, z);
    let f = fx.mem.i16(D + 0x693C);
    if f < 0x104 && f % 2 == 0 {
        spawn(fx, STEP_SPARKLE, 0x01E0_5308);
    }
    if fx.mem.i16(D + 0x693C) < 0x104 {
        let r = entry_rand(fx, 0x01E0_5320);
        let (_, m) = fx.div(r, 0xC8);
        fx.mem.setf(D + 0x250, ee::from_i32(m - 0x64));
        let r = entry_rand(fx, 0x01E0_534C);
        let y = fx.mem.f(D + 0x254);
        let (_, m) = fx.div(r, 0x96);
        fx.mem.setf(D + 0x254, ee::add(y, ee::from_i32(m - 0x4B)));
        fx.mem.set16(D + 0x6934, 1);
        let f = fx.mem.i16(D + 0x693C);
        let (_, m) = fx.div(f, 3);
        if m != 1 {
            spawn(fx, STEP_SPARKS, 0x01E0_53B4);
        }
    }
    if fx.mem.i16(D + 0x693C) == 0xFA {
        fx.mem.set16(D + 0x6934, 2);
        spawn(fx, STEP_FLASH, 0x01E0_53E0);
        fx.mem.set32(D + 0x250, 0);
        let y = fx.mem.read_u32(D + 0x6814);
        fx.mem.write_u32(D + 0x254, y);
        let z = fx.mem.read_u32(D + 0x6818);
        fx.mem.write_u32(D + 0x258, z);
        spawn(fx, STEP_SPARKS, 0x01E0_5408);
    }
}

/// The entry's draw of one live task by its step function
/// (0x1E0484C..0x1E04BB8).
fn draw_task(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_BEAMS {
        beams(fx, t, sp + 0xB0, 0);
        beams(fx, t, sp + 0xB0, 1);
    }
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_LINKS {
        links(fx, t, sp + 0xB0, 0);
        links(fx, t, sp + 0xB0, 1);
    }
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_TRAILS {
        trails(fx, t, sp + 0xB0, 0);
        trails(fx, t, sp + 0xB0, 1);
    }
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_FLASH {
        let p = [fx.mem.f(t + 0x10), fx.mem.f(t + 0x14), fx.mem.f(t + 0x18)];
        for (i, v) in p.into_iter().enumerate() {
            fx.mem.setf(sp + 0x70 + 4 * i as u32, v);
        }
        project_point(fx, sp + 0x70, 0, sp + 0xC8, D + 0x6868);
        let w = fx.mem.f(D + 0x6868);
        match fx.mem.i16(t + 0x50) {
            0 => flash(fx, t, 1, w),
            1 | 2 => flash(fx, t, 0x20, w),
            _ => {}
        }
    }
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_SPARKS {
        fx.mem.set16(D + 0xB8, 0x24);
        sparks(fx, t, 0x21, t + 0x10);
        fx.mem.set16(D + 0xB8, 0);
    }
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_SPARKLE {
        let tex = fx.mem.read_u32(D + 0x29C);
        fx.mem.write_u32(D + 0x88, tex);
        let s = fx.mem.i16(t + 0x56);
        set_rect(fx, sp + 0x80, 0x9B, s);
        fx.mem.set16(D + 0xB8, 5);
        let r = entry_rand(fx, 0x01E0_4A68);
        fx.mem.set16(D + 0xBE, r % 4);
        fx.mem.set16(D + 0xB0, 0);
        sparkles(fx, t, 0x21, D + 0x6A3E);
        fx.mem.set16(D + 0xB8, 0);
    }
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_RING {
        let s = fx.mem.i16(t + 0x56);
        set_rect(fx, sp + 0x90, 0x9B, s);
        let tex = fx.mem.read_u32(D + 0x29C);
        fx.mem.write_u32(D + 0x88, tex);
        fx.mem.set16(D + 0xB8, 0xF);
        fx.mem.set16(D + 0xBE, 0);
        for i in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * i);
            fx.mem.write_u32(D + 0x20 + 4 * i, w);
        }
        ring(fx, t, 0x20, D + 0x6A3E);
        fx.mem.set16(D + 0xB8, 0);
    }
    let f = fx.mem.read_u32(t + 0x44);
    if f == STEP_GLOW {
        let tex = fx.mem.read_u32(D + 0x29C);
        fx.mem.write_u32(D + 0x88, tex);
        set_rect(fx, sp + 0xA0, 0x9A, 0);
        fx.mem.set16(D + 0xBE, 0);
        fx.mem.set32(sp + 0x7C, 0);
        for i in 0..3 {
            let w = fx.mem.read_u32(t + 0x10 + 4 * i);
            fx.mem.write_u32(sp + 0x70 + 4 * i, w);
        }
        sprite_quads(fx, t, 0, sp + 0x70);
    }
}

/// 0x204D80(v, m, xyz, w): the vec at `v` (w forced 1) through `m` (the
/// camera's view-projection, copied to the frame's sp + 0x60, when 0)
/// with VU0, divided by w; on screen the XYZ2 value merged at `xyz` and w
/// at `w` (when not 0). The frame's locals (sp + 0xA0: x / w, y / w, z /
/// w, w) are kept as the original leaves them.
pub(crate) fn project_point(fx: &mut Fx, v: u32, m: u32, xyz: u32, w: u32) -> bool {
    let sp = fx.enter(0xB0);
    for k in 0..4 {
        let x = fx.mem.read_u32(v + 4 * k);
        fx.mem.write_u32(sp + 0xA0 + 4 * k, x);
    }
    let mm = if m == 0 {
        fx.camera_vp_to(sp + 0x60);
        fx.read_m4(sp + 0x60)
    } else {
        fx.read_m4(m)
    };
    fx.mem.write_u32(sp + 0xAC, 0x3F80_0000);
    let p = fx.read_v4(sp + 0xA0);
    fx.push(Some(mm));
    let o = fx.transform(None, p);
    fx.pop();
    fx.write_v4(sp + 0xA0, o);
    let q = ee::div(1.0, o[3]);
    let x = ee::mul(o[0], q);
    fx.mem.setf(sp + 0xA0, x);
    let y = ee::mul(o[1], q);
    fx.mem.setf(sp + 0xA4, y);
    let z = ee::mul(o[2], q);
    fx.mem.setf(sp + 0xA8, z);
    let lim = f32::from_bits(0x457F_F000);
    let ok = !(ee_lt(z, 1.0) || !ee_le(z, f32::from_bits(0x4B7F_FFFF)) || ee_lt(x, 0.0) || !ee_le(x, lim) || ee_lt(y, 0.0) || !ee_le(y, lim));
    if ok {
        let x16 = u64::from(ee::fixunssfsi(ee::mul(16.0, x)) as u16);
        fx.mem.write_u16(xyz, x16 as u16);
        let y16 = ee::fixunssfsi(ee::mul(16.0, y)) as u16;
        fx.mem.write_u16(xyz + 2, y16);
        let fz = u64::from(ee::fixunssfsi(z) & 0xFF_FFFF);
        let old = fx.mem.read_u64(xyz);
        fx.mem.write_u64(xyz, (old & 0xFF00_0000_FFFF_FFFF) | (fz << 32));
        if w != 0 {
            fx.mem.setf(w, o[3]);
        }
    }
    fx.leave(0xB0);
    ok
}

fn run_step(fx: &mut Fx, f: u32, t: u32) {
    // Every step function but the tint saves `ra` (the jalr's return,
    // 0x1E04814) at the entry's frame top.
    if matches!(f, STEP_BEAMS | STEP_LINKS | STEP_TRAILS | STEP_FLASH | STEP_SPARKS | STEP_SPARKLE | STEP_RING | STEP_GLOW) {
        saved_ra(fx, 0x01E0_4814);
    }
    match f {
        STEP_BEAMS => step_beams(fx, t),
        STEP_LINKS => step_links(fx, t),
        STEP_TRAILS => step_trails(fx, t),
        STEP_FLASH => step_flash(fx, t),
        STEP_SPARKS => step_sparks(fx, t),
        STEP_SPARKLE => step_sparkle(fx, t),
        STEP_RING => step_ring(fx, t),
        STEP_GLOW => step_glow(fx, t),
        STEP_TINT => step_tint(fx, t),
        _ => {}
    }
}

/// The entry's frame top (its sp - 0x10): where a callee's prologue
/// saves `ra` (spawn, the step functions, 0x1F6E50) or rand saves s0 / ra;
/// the tint's TEX1 (0x207FC0 keeps its stale bits) reads it [F].
const ENTRY_TOP: u32 = crate::exe::STACK - 0x140 - 0x10;

/// A direct callee's saved `ra` (low 64 bits) at [`ENTRY_TOP`].
fn saved_ra(fx: &mut Fx, ra: u32) {
    fx.mem.write_u64(ENTRY_TOP, u64::from(ra));
}

/// rand called from the entry: its frame saves s0 (= D) and `ra`.
fn entry_rand(fx: &mut Fx, ra: u32) -> i32 {
    fx.mem.write_u64(ENTRY_TOP, u64::from(D));
    fx.mem.write_u64(ENTRY_TOP + 8, u64::from(ra));
    fx.rand()
}

/// 0x1E05500(D, step, 0): a new task (first free of 200) with the colour
/// D+0x6844, at D+0x250, angle D+0x6928, index D+0x6934.
fn spawn(fx: &mut Fx, step: u32, ra: u32) -> i32 {
    saved_ra(fx, ra);
    fx.enter(0x60);
    let mut t = 0;
    for k in 0..200u32 {
        let r = D + 0x2A0 + 0x70 * k;
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
    for o in [0x60u32, 0x64] {
        let (r, g, b) = (fx.mem.u8(D + 0x6844), fx.mem.u8(D + 0x6845), fx.mem.u8(D + 0x6846));
        l2::rgb(fx, t + o, r, g, b);
        let a = fx.mem.read_u8(D + 0x6847);
        fx.mem.write_u8(t + o + 3, a);
    }
    l2::to_i16x4(fx, t, D + 0x250, 0);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x250 + 4 * k);
        fx.mem.write_u32(t + 0x10 + 4 * k, w);
    }
    let v = fx.mem.i16(D + 0x6928);
    fx.mem.set16(t + 0x5A, v);
    let v = fx.mem.i16(D + 0x6934);
    fx.mem.set16(t + 0x50, v);
    fx.mem.set32(t + 0x40, 0);
    fx.mem.write_u32(t + 0x44, step);
    fx.leave(0x60);
    1
}

/// A limb's position: its record +0 plus +0x10 (xyz) into `out` (set3).
fn limb_pos(fx: &mut Fx, rec: u32, out: u32) {
    let x = ee::add(fx.mem.f(rec), fx.mem.f(rec + 0x10));
    let y = ee::add(fx.mem.f(rec + 4), fx.mem.f(rec + 0x14));
    let z = ee::add(fx.mem.f(rec + 8), fx.mem.f(rec + 0x18));
    l2::set3(fx, out, x, y, z);
}

/// 0x1E004D0(D, task, desc, pass): beams from each pair of limbs
/// (rodata 0x1E05880) to their centroid, gouraud quads (pass 0 the
/// header).
fn beams(fx: &mut Fx, t: u32, d: u32, pass: i32) {
    let sp = fx.enter(0x110);
    for k in 0..10 {
        let w = fx.mem.read_u32(BEAM_PAIRS + 4 * k);
        fx.mem.write_u32(sp + 0xE0 + 4 * k, w);
    }
    if pass == 0 {
        l2::desc_gouraud4(fx, d, 0);
        fx.leave(0x110);
        return;
    }
    let sum = |fx: &mut Fx, o: u32| {
        let r = |fx: &mut Fx, a: u32| fx.mem.f(D + a + o);
        let mut s = ee::add(r(fx, 0x5A50), r(fx, 0x5AC0));
        for a in [0x5B30u32, 0x5BA0, 0x5C10, 0x5A60, 0x5AD0, 0x5B40, 0x5BB0, 0x5C20] {
            s = ee::add(r(fx, a), s);
        }
        ee::div(s, 5.0)
    };
    let cx = sum(fx, 0);
    let cz = sum(fx, 8);
    let cy = sum(fx, 4);
    for s0 in 0..5u32 {
        for s1 in 0..2u32 {
            let idx = fx.mem.i32(sp + 0xE0 + 8 * s0 + 4 * s1);
            let rec = D.wrapping_add((idx * 0x70) as u32) + 0x5A50;
            limb_pos(fx, rec, sp + 0xA0 + 0x10 * s1);
        }
        for (o, v) in [(0xD0u32, cx), (0xC0, cx), (0xD4, cy), (0xC4, cy), (0xD8, cz), (0xC8, cz)] {
            fx.mem.setf(sp + o, v);
        }
        l2::colour_strip4(fx, d, t + 0x60, 0);
        let ot = fx.mem.read_u32(D + 0x684C);
        l2::project_submit(fx, sp + 0xA0, d, 0, ot);
    }
    fx.leave(0x110);
}

/// 0x1E00730(D, task, desc, pass): the links between limbs (rodata
/// pairs 0x1E058B0, order 0x1E058C8) as gouraud quads, twice each (the
/// two colour orders, widened by +0x48 or 1.75 x for kinds 3, 4), every
/// quad on screen submitted four times [F].
fn links(fx: &mut Fx, t: u32, d: u32, pass: i32) {
    let sp = fx.enter(0x220);
    let f20 = fx.mem.f(t + 0x48);
    for k in 0..5 {
        let w = fx.mem.read_u32(LINK_PAIRS + 4 * k);
        fx.mem.write_u32(sp + 0x110 + 4 * k, w);
    }
    let w = fx.mem.read_u64(LINK_ORDER);
    fx.mem.write_u64(sp + 0x150, w);
    let h = fx.mem.read_u16(LINK_ORDER + 8);
    fx.mem.write_u16(sp + 0x158, h);
    if pass == 0 {
        l2::desc_gouraud4(fx, d, 0);
        fx.leave(0x220);
        return;
    }
    let f21 = ee::mul(1.75, f20);
    let mut fp = sp + 0x150;
    fx.mem.set16(sp + 0xB0, 0);
    let copy16 = |fx: &mut Fx, to: u32, from: u32| {
        for k in 0..4 {
            let w = fx.mem.read_u32(from + 4 * k);
            fx.mem.write_u32(to + 4 * k, w);
        }
    };
    while fx.mem.i16(sp + 0xB0) < fx.mem.i32(D + 0x6900) {
        let a = fx.mem.i16(fp);
        let i0 = fx.mem.i16(sp.wrapping_add((4 * a) as u32) + 0x110);
        limb_pos(fx, D.wrapping_add((i0 * 0x70) as u32) + 0x5A50, sp + 0x100);
        copy16(fx, sp + 0x130, sp + 0x100);
        copy16(fx, sp + 0xC0, sp + 0x130);
        copy16(fx, sp + 0xD0, sp + 0x130);
        let a = fx.mem.i16(fp);
        let i1 = fx.mem.i16(sp.wrapping_add((4 * a) as u32) + 0x112);
        limb_pos(fx, D.wrapping_add((i1 * 0x70) as u32) + 0x5A50, sp + 0x100);
        copy16(fx, sp + 0x140, sp + 0x100);
        copy16(fx, sp + 0xE0, sp + 0x140);
        copy16(fx, sp + 0xF0, sp + 0x140);
        let z = if fx.mem.i16(D + 0x693A) != 0 { -5.0 } else { 5.0 };
        l2::add_column(fx, sp + 0xC0, 2, z);
        let s6 = fx.mem.i16(fp);
        for s4 in 0..2 {
            let s5 = s4;
            let colours = |fx: &mut Fx, base: u32, first: u32, second: u32| {
                rgbaq_at(fx, sp + base, t + first, d + 0x10);
                rgbaq_at(fx, sp + base - 8, t + second, d + 0x20);
                rgbaq_at(fx, sp + base - 0x10, t + first, d + 0x30);
                rgbaq_at(fx, sp + base - 0x18, t + second, d + 0x40);
            };
            let sub = |fx: &mut Fx, a: u32, v: f32| {
                let x = ee::sub(fx.mem.f(sp + a), v);
                fx.mem.setf(sp + a, x);
            };
            if s6 == 0 || s6 == 1 {
                if s5 == 0 {
                    colours(fx, 0x178, 0x60, 0x64);
                    sub(fx, 0xD0, f20);
                    sub(fx, 0xF0, f20);
                } else {
                    colours(fx, 0x198, 0x64, 0x60);
                    l2::add_column(fx, sp + 0xC0, 0, f20);
                }
            }
            if s6 == 2 {
                if s5 == 0 {
                    colours(fx, 0x1B8, 0x60, 0x64);
                    sub(fx, 0xD4, f20);
                    sub(fx, 0xF4, f20);
                } else {
                    colours(fx, 0x1D8, 0x64, 0x60);
                    l2::add_column(fx, sp + 0xC0, 1, f20);
                }
            }
            if s6 == 3 || s6 == 4 {
                if s5 == 0 {
                    colours(fx, 0x1F8, 0x60, 0x64);
                    sub(fx, 0xD0, f21);
                    sub(fx, 0xF0, f21);
                } else {
                    colours(fx, 0x218, 0x64, 0x60);
                    l2::add_column(fx, sp + 0xC0, 0, f21);
                }
            }
            if l2::project(fx, sp + 0xC0, SPR, 0, d, D + 0x686C) {
                for _ in 0..4 {
                    let key = ee::fixunssfsi(fx.mem.f(D + 0x686C)) as i32;
                    let ot = fx.mem.read_u32(D + 0x684C);
                    fx.submit(d, ot, key);
                }
            }
        }
        let n = fx.mem.i16(sp + 0xB0) + 1;
        fx.mem.set16(sp + 0xB0, n);
        fp += 2;
    }
    fx.leave(0x220);
}

/// 0x1E00E30(D, task, desc, pass): the limbs' trails (6 points each,
/// D+0x65D0 + 0x60 limb) as small textured quads (type 3 texture 0),
/// three per limb before stage 0xC, five after.
fn trails(fx: &mut Fx, t: u32, d: u32, pass: i32) {
    let sp = fx.enter(0x280);
    let mut n = 3;
    if pass == 0 {
        fx.mem.set16(sp + 0xC0, 0x100);
        fx.mem.set16(sp + 0xC2, 0);
        fx.mem.set16(sp + 0xC4, 0x100);
        fx.mem.set16(sp + 0xC6, 0x180);
        l2::desc_strip3(fx, d);
        l2::colour_strip3(fx, d, t + 0x60, 1);
        l2::colour_strip3(fx, d, t + 0x64, 2);
        l2::uv_strip3(fx, d, sp + 0xC0, 3);
        let tex = fx.mem.read_u32(D + 0x685C);
        fx.desc_texture(d, tex, 1);
        fx.leave(0x280);
        return;
    }
    if fx.mem.i32(D + 0x68FC) >= 0xC {
        n = 5;
    }
    l2::quad_xy(fx, sp + 0x80, f32::from_bits(0x414C_CCCD), f32::from_bits(0x4193_3333));
    fx.mem.set32(sp + 0x110, 0);
    fx.mem.set32(sp + 0x114, 0);
    let z = if fx.mem.i16(D + 0x693A) != 0 { 0xC349_0000u32 } else { 0x4349_0000 };
    fx.mem.write_u32(sp + 0x118, z);
    project_point(fx, sp + 0x110, 0, d + 0x20, sp + 0x27C);
    for limb in 0..5u32 {
        for k in 0..n as u32 {
            let p = D + 0x60 * limb + 0x10 * k + 0x65D0;
            for i in 0..4 {
                let w = fx.mem.read_u32(p + 4 * i);
                fx.mem.write_u32(sp + 0xD0 + 4 * i, w);
            }
            fx.camera_vp_to(sp + 0x120);
            let tr = [fx.mem.f(sp + 0xD0), fx.mem.f(sp + 0xD4), fx.mem.f(sp + 0xD8)];
            let m = fx.trs(None, None, Some(tr));
            fx.write_m4(sp + 0x220, &m);
            fx.mul_into(Some(sp + 0x120), &m);
            if l2::project(fx, sp + 0x80, SPR, sp + 0x120, d, sp + 0x260) {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x260)) as i32;
                let ot = fx.mem.read_u32(D + 0x684C);
                fx.submit(d, ot, key);
            }
        }
    }
    fx.leave(0x280);
}

/// A u16 as the code converts it (cvt.s.w, halved and doubled for the
/// top bit: never for a u16).
fn u16_f32(v: u16) -> f32 {
    ee::from_i32(i32::from(v))
}

/// 0x1E01050(D, task, mode, table): a ring of D+0xB8 sparkles (9-value
/// quads, FST) at D+0x20 pushed out by the random table, each widened on
/// screen by +0x48 / (w / (distance / 10)) (mode 0) [F].
fn ring(fx: &mut Fx, t: u32, mode: i32, table: u32) {
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
    let v = fx.mem.i16(t + 0x5A);
    fx.mem.set32(sp + 0x174, v);
    for k in 0..3 {
        let w = fx.mem.read_u32(D + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x100, 0, sp + 0xF0);
    let order = fx.mem.i16(D + 0xC0);
    uv_strip4(fx, d, D + 0x94, order);
    let mut dist = fx.camera_distance();
    if crate::libm_tan::ee_eq(0.0, dist) {
        dist = 400.0;
    }
    let s2 = mode & 0xF;
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xB8) {
        let mut size = fx.mem.f(t + 0x48);
        l2::quad_xy(fx, sp + 0xA0, 0.0, 0.0);
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
        let r = radius(fx);
        let b = byte(fx, 0x64);
        fx.mem.setf(sp + 0xE4, ee::mul(ee::div(b, 20.0), r));
        let r = radius(fx);
        let b = byte(fx, 0xB4);
        fx.mem.setf(sp + 0xE8, ee::mul(ee::div(b, 20.0), r));
        l2::rotate_deg(fx, sp + 0xE0, sp + 0x170, 0, D + 0xD8);
        for (o, from) in [(0u32, 0xE0u32), (4, 0xE4), (8, 0xE8)] {
            let f1 = fx.mem.f(sp + from);
            for c in [0xA0u32, 0xB0, 0xC0, 0xD0] {
                let v = ee::add(fx.mem.f(sp + c + o), f1);
                fx.mem.setf(sp + c + o, v);
            }
        }
        if s2 <= 2 && l2::project(fx, sp + 0xA0, SPR, sp + 0x120, d, sp + 0x110) {
            let ot = fx.mem.read_u32(D + 0x80);
            match s2 {
                0 => {
                    let f1 = ee::div(dist, 10.0);
                    let f0 = ee::div(fx.mem.f(sp + 0x110), f1);
                    size = ee::div(size, f0);
                    for (o, plus) in [(0x20u32, false), (0x22, true), (0x30, true), (0x32, true), (0x40, false), (0x42, false), (0x50, true), (0x52, false)] {
                        let u = u16_f32(fx.mem.read_u16(d + o));
                        let v = if plus { ee::add(u, size) } else { ee::sub(u, size) };
                        fx.mem.write_u16(d + o, ee::fixunssfsi(v) as u16);
                    }
                    l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                    fx.submit(d, ot, 0);
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

/// 0x1E01870(D, task, mode, pos): D+0xB8 gouraud sparks on a circle at
/// `pos` turned by +0x20 (inner +0x48, outer +0x4C, from angle a + +0x5A
/// over +0x5C degrees).
fn sparks(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x200);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    let span = fx.mem.i16(t + 0x5C);
    let d = sp + 0x170;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * k);
        fx.mem.write_u32(sp + 0x100 + 4 * k, w);
    }
    scale_tenth(fx, sp + 0x110);
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x20, sp + 0x100);
    for o in [0xC4u32, 0xD4, 0xE4, 0xF4] {
        fx.mem.set32(sp + o, 0);
    }
    let (inner, outer) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let s7 = mode & 0xF;
    let tab = D + 0xD8;
    let mut a = 0i32;
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xB8) {
        let s4 = a as i16 as i32;
        let e1 = (fx.mem.i16(t + 0x5A) + s4) as i16 as i32;
        let c = l2::sin_deg(fx, tab, e1, 1);
        fx.mem.setf(sp + 0xC0, ee::mul(inner, c));
        let s = l2::sin_deg(fx, tab, e1, 0);
        fx.mem.setf(sp + 0xC8, ee::mul(inner, s));
        let c = l2::sin_deg(fx, tab, e1, 1);
        fx.mem.setf(sp + 0xE0, ee::mul(outer, c));
        let s = l2::sin_deg(fx, tab, e1, 0);
        fx.mem.setf(sp + 0xE8, ee::mul(outer, s));
        let e2 = (s4 + fx.mem.i16(t + 0x5A) + span) as i16 as i32;
        let c = l2::sin_deg(fx, tab, e2, 1);
        fx.mem.setf(sp + 0xD0, ee::mul(inner, c));
        let s = l2::sin_deg(fx, tab, e2, 0);
        fx.mem.setf(sp + 0xD8, ee::mul(inner, s));
        let c = l2::sin_deg(fx, tab, e2, 1);
        fx.mem.setf(sp + 0xF0, ee::mul(outer, c));
        let s = l2::sin_deg(fx, tab, e2, 0);
        fx.mem.setf(sp + 0xF8, ee::mul(outer, s));
        if s7 <= 1 && l2::project(fx, sp + 0xC0, SPR, sp + 0x120, d, sp + 0x160) {
            let ot = fx.mem.read_u32(D + 0x80);
            if s7 == 0 {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                fx.submit(d, ot, key);
            } else {
                l2::far_z_desc(fx, d, 1);
                fx.submit(d, ot, 0);
            }
        }
        k += 1;
        a = (a + fx.mem.i16(t + 0x5A)) as i16 as i32;
    }
    fx.leave(0x200);
}

/// 0x1E01BD0(D, task, mode, table): D+0xB8 sparkles (9-value quads)
/// round the task, pushed out by the table (+0, +100, +180), facing the
/// camera (D+0xB0 0) or flat (1).
fn sparkles(fx: &mut Fx, t: u32, mode: i32, table: u32) {
    let sp = fx.enter(0x210);
    let d = sp + 0x178;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha(mode >> 4));
    rgbaq_at(fx, sp + 0x208, t + 0x60, d + 0x10);
    uv_strip4(fx, d, D + 0x94, 0);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    l2::camera_angles_to(fx, sp + 0x150);
    to_degrees_neg(fx, sp + 0x154);
    to_degrees_neg(fx, sp + 0x150);
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(sp + 0xE0 + 4 * k, w);
    }
    let kind = fx.mem.i16(D + 0xB0);
    if kind == 0 || kind == 1 {
        l2::xform_vp(fx, sp + 0x110, D + 0x10, t + 0x20, sp + 0xE0);
    }
    let s3 = mode & 0xF;
    let a = fx.mem.i16(t + 0x5A);
    fx.mem.set32(sp + 0x164, a);
    let mut k = 0i32;
    let mut s2 = D;
    while k < fx.mem.i16(D + 0xB8) {
        let s4 = k as i16 as i32;
        let kind = fx.mem.i16(D + 0xB0);
        let size = fx.mem.f(t + 0x48);
        if kind == 0 {
            let order = fx.mem.i16(s2 + 0xC0);
            uv_strip4(fx, d, D + 0x94, order);
            l2::quad_xy(fx, sp + 0xA0, size, size);
            l2::rotate4(fx, sp + 0xA0, sp + 0x150, 1, D + 0xD8);
            l2::rotate4(fx, sp + 0xA0, sp + 0x150, 0, D + 0xD8);
        } else if kind == 1 {
            l2::quad_xy(fx, sp + 0xA0, size, size);
        }
        let base = table.wrapping_add(s4 as u32);
        let half = s4 >> 1;
        for (j, off) in [(0u32, 0u32), (1, 100), (2, 180)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::from_i32(fx.mem.i8(base.wrapping_add(i as u32) + off));
            let r = ee::from_i32(fx.mem.i16(t + 0x50) + half);
            fx.mem.setf(sp + 0xF0 + 4 * j, ee::mul(ee::div(b, 20.0), r));
        }
        if fx.mem.i16(D + 0xB0) == 0 {
            l2::rotate_deg(fx, sp + 0xF0, sp + 0x160, 0, D + 0xD8);
        }
        for axis in 0..3 {
            let v = fx.mem.f(sp + 0xF0 + 4 * axis as u32);
            l2::add_column(fx, sp + 0xA0, axis, v);
        }
        if fx.mem.i16(D + 0xB0) == 1 {
            fx.mem.set32(sp + 0xA4, 0);
            fx.mem.set32(sp + 0xB4, 0);
            let s = fx.mem.f(t + 0x48);
            fx.mem.setf(sp + 0xA0, s);
            let s = fx.mem.f(t + 0x48);
            fx.mem.setf(sp + 0xB0, ee::neg(s));
            fx.mem.set32(sp + 0xA8, 0);
            fx.mem.set32(sp + 0xB8, 0);
        }
        if s3 <= 2 && l2::project(fx, sp + 0xA0, SPR, sp + 0x110, d, sp + 0x100) {
            let ot = fx.mem.read_u32(D + 0x80);
            match s3 {
                0 => {
                    let key = ee::fixunssfsi(fx.mem.f(sp + 0x100)) as i32;
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
        s2 += 2;
    }
    fx.leave(0x210);
}

/// 0x1E05650(D, task, mode, w): a full-window gouraud flash quad of size
/// +0x48 at the task, keyed by `w` (mode & 15 != 0) or at the far Z (0).
fn flash(fx: &mut Fx, t: u32, mode: i32, w: f32) {
    let sp = fx.enter(0x190);
    let ot = OT;
    let d = sp + 0x100;
    l2::colour_strip4(fx, d, t + 0x60, 1);
    l2::colour_strip4(fx, d, t + 0x64, 2);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    for k in 0..3 {
        let v = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(sp + 0xA0 + 4 * k, v);
    }
    l2::xform_vp(fx, sp + 0xC0, 0, 0, sp + 0xA0);
    let s = fx.mem.f(t + 0x48);
    l2::quad_xy(fx, sp + 0x60, s, s);
    if l2::project(fx, sp + 0x60, SPR, sp + 0xC0, d, sp + 0xB0) {
        if mode & 0xF == 0 {
            l2::far_z(fx, &[d + 0x48, d + 0x38, d + 0x28, d + 0x18]);
            fx.submit(d, ot, 0);
        } else {
            let key = ee::fixunssfsi(w) as i32;
            fx.submit(d, ot, key);
        }
    }
    fx.leave(0x190);
}

/// 0x1E024F0: the beams' colour cycle (G down, then R, then B up and
/// down), alpha in for 15 updates, out from 50; ends at frame 0x10E.
fn step_beams(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x54) < 0xF {
        let a = fx.mem.u8(t + 0x63);
        if a < 0x80 {
            fx.mem.set8(t + 0x63, a + 8);
        }
        if fx.mem.i16(t + 0x54) == 0 {
            l2::rgb(fx, t + 0x60, 0x80, 0x60, 0);
        }
    } else if fx.mem.i16(t + 0x54) >= 0x32 {
        let a = fx.mem.u8(t + 0x63);
        if a >= 0x10 {
            fx.mem.set8(t + 0x63, a - 5);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    match fx.mem.i16(t + 0x50) {
        0 => {
            let g = fx.mem.u8(t + 0x61);
            if g >= 3 {
                fx.mem.set8(t + 0x61, g - 1);
            } else {
                fx.mem.set16(t + 0x50, 1);
            }
        }
        1 => {
            fx.mem.write_u8(t + 0x62, 0);
            let r = fx.mem.u8(t + 0x60);
            if r >= 3 {
                fx.mem.set8(t + 0x60, r - 1);
            } else {
                fx.mem.set16(t + 0x50, 2);
                fx.mem.write_u8(t + 0x62, 0xFF);
            }
        }
        2 => {
            let b = fx.mem.u8(t + 0x62);
            if b >= 4 {
                fx.mem.set8(t + 0x62, b - 2);
            }
        }
        _ => {}
    }
    if fx.mem.i16(D + 0x693C) >= 0x10E {
        fx.mem.set16(t + 0x58, 0);
    }
    fx.leave(0x30);
}

/// 0x1E02650: the links grow to 7 wide, flicker (0xAF / 0x8F) until
/// frame 0x10E, then fade; one more link per update up to 5.
fn step_links(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    let w = fx.mem.f(t + 0x48);
    if ee_lt(w, 7.0) {
        fx.mem.setf(t + 0x48, ee::add(w, 1.0));
    }
    l2::rgb(fx, t + 0x60, 0x4F, 0x4F, 0x4F);
    l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
    if fx.mem.i16(D + 0x693C) < 0x10E {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u8(t + 0x63, 0xAF);
            fx.mem.write_u8(t + 0x67, 0x10);
            fx.mem.set16(t + 0x50, 1);
        } else {
            fx.mem.write_u8(t + 0x63, 0x8F);
            fx.mem.write_u8(t + 0x67, 0x10);
            fx.mem.set16(t + 0x50, 0);
        }
    } else {
        let a = fx.mem.u8(t + 0x63);
        if a >= 0xB {
            fx.mem.set8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
        let b = fx.mem.u8(t + 0x67);
        if b >= 4 {
            fx.mem.set8(t + 0x67, b - 2);
        }
    }
    let n = fx.mem.i32(D + 0x6900);
    if n < 5 {
        fx.mem.set32(D + 0x6900, n + 1);
    }
    fx.leave(0x30);
}

/// A limb's trail head: its position plus D+0x68BC (limb 0's radius) at
/// its angle (+0x24) on the 0x6B40 table, z one toward the far side.
fn trail_head(fx: &mut Fx, rec: u32, out: u32) {
    let a = fx.mem.i16(rec + 0x24);
    let c = l2::sin_deg(fx, D + 0x6B40, a, 1);
    let r = fx.mem.f(D + 0x68BC);
    let x = ee::add(ee::add(fx.mem.f(rec), fx.mem.f(rec + 0x10)), ee::mul(r, c));
    fx.mem.setf(out, x);
    let s = l2::sin_deg(fx, D + 0x6B40, a, 0);
    let r = fx.mem.f(D + 0x68BC);
    let y = ee::add(ee::add(fx.mem.f(rec + 4), fx.mem.f(rec + 0x14)), ee::mul(r, s));
    fx.mem.setf(out + 4, y);
    let z = ee::add(fx.mem.f(rec + 8), fx.mem.f(rec + 0x18));
    let z = if fx.mem.i16(D + 0x693A) == 0 { ee::add(1.0, z) } else { ee::sub(z, 1.0) };
    fx.mem.setf(out + 8, z);
}

/// 0x1E02780: the limbs' trails (each limb's 6 points shifted, the head
/// from [`trail_head`]); the first update fills all from limb 0 [F: the
/// record pointer is not stepped there]; ends at frame 0xD9.
fn step_trails(fx: &mut Fx, t: u32) {
    fx.enter(0x80);
    if fx.mem.i16(t + 0x58) == 1 {
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x20);
        l2::rgb(fx, t + 0x64, 0x20, 0x20, 0x20);
        for limb in 0..5u32 {
            let p = D + 0x60 * limb + 0x65D0;
            trail_head(fx, D + 0x5A50, p);
            for k in 1..5u32 {
                for i in 0..4 {
                    let w = fx.mem.read_u32(p + 4 * i);
                    fx.mem.write_u32(p + 0x10 * k + 4 * i, w);
                }
            }
        }
        fx.mem.set16(t + 0x58, 2);
    }
    let w = fx.mem.f(t + 0x48);
    if ee_lt(w, 7.0) {
        fx.mem.setf(t + 0x48, ee::add(w, 1.0));
    }
    fx.mem.write_u8(t + 0x63, 0x80);
    fx.mem.write_u8(t + 0x67, 0x80);
    for limb in 0..5u32 {
        let p = D + 0x60 * limb + 0x65D0;
        for k in (1..6u32).rev() {
            for i in 0..4 {
                let w = fx.mem.read_u32(p + 0x10 * (k - 1) + 4 * i);
                fx.mem.write_u32(p + 0x10 * k + 4 * i, w);
            }
        }
        trail_head(fx, D + 0x5A50 + 0x70 * limb, p);
    }
    if fx.mem.i16(D + 0x693C) >= 0xD9 {
        fx.mem.set16(t + 0x58, 0);
    }
    fx.leave(0x80);
}

/// 0x1E02B20: a flash at the far side (D+0x6808, 70 up) of kind +0x50
/// (white for 22 updates; warm for 15 / 25), sliding 4 per update from
/// frame 0x10F.
fn step_flash(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        let z = fx.mem.read_u32(D + 0x6808);
        fx.mem.write_u32(t + 0x18, z);
        fx.mem.set32(t + 0x10, 0);
        fx.mem.write_u32(t + 0x14, 0x428C_0000);
        let kind = fx.mem.i16(t + 0x50);
        if kind == 0 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
            l2::rgb(fx, t + 0x64, 0xF0, 0xF0, 0xF0);
            fx.mem.set16(t + 6, 0x16);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xD0, 0xA0);
            l2::rgb(fx, t + 0x64, 0xF0, 0xA0, 0xA0);
            fx.mem.set16(t + 6, 0xF);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xD0, 0xA0);
            l2::rgb(fx, t + 0x64, 0xF0, 0xA0, 0xA0);
            fx.mem.set16(t + 6, 0x19);
        }
        fx.mem.write_u32(t + 0x48, 0x43FA_0000);
        fx.mem.set16(t + 0x58, 2);
    }
    let up = |fx: &mut Fx, a: u32, by: i32| {
        let v = fx.mem.u8(a);
        if v < 0x80 {
            fx.mem.set8(a, v + by);
        }
    };
    let down = |fx: &mut Fx, ends: bool| {
        let v = fx.mem.u8(t + 0x63);
        if v >= 6 {
            fx.mem.set8(t + 0x63, v - 4);
        } else {
            let _ = ends;
            fx.mem.set16(t + 0x58, 0);
        }
        let v = fx.mem.u8(t + 0x67);
        if v >= 6 {
            fx.mem.set8(t + 0x67, v - 4);
        } else {
            fx.mem.write_u8(t + 0x67, 0);
        }
    };
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if fx.mem.i16(t + 0x50) != 2 {
            up(fx, t + 0x63, 0x12);
            up(fx, t + 0x67, 0x12);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            up(fx, t + 0x63, 7);
            up(fx, t + 0x67, 7);
        }
    } else {
        if fx.mem.i16(t + 0x50) == 1 {
            down(fx, true);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            down(fx, true);
        }
        if fx.mem.i16(t + 0x50) == 0 && fx.mem.i16(D + 0x693C) >= 0x141 {
            down(fx, true);
        }
    }
    if fx.mem.i16(D + 0x693C) >= 0x10F {
        let z = fx.mem.f(t + 0x18);
        if fx.mem.i16(D + 0x693A) == 0 {
            fx.mem.setf(t + 0x18, ee::add(z, 4.0));
        }
        let z = fx.mem.f(t + 0x18);
        if fx.mem.i16(D + 0x693A) == 1 {
            fx.mem.setf(t + 0x18, ee::sub(z, 4.0));
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x30);
}

/// 0x1E02E40: sparks of kind +0x50 (radius +0x4C 600 fixed, or growing by
/// 10 from 600 / 1800), facing the camera turned a quarter.
fn step_sparks(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set32(t + 0x48, 0);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
            l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
            fx.mem.set16(t + 6, 0xC);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
            l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x80);
            fx.mem.set16(t + 6, 5);
            fx.mem.write_u32(t + 0x4C, 0x4416_0000);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
            l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
            fx.mem.set16(t + 6, 0xA);
            fx.mem.write_u32(t + 0x4C, 0x44E1_0000);
        }
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        let a = fx.mem.u8(t + 0x67);
        if a < 0xA0 {
            fx.mem.set8(t + 0x67, a + 0x50);
        }
    } else if fx.mem.i16(t + 0x50) != 1 {
        let a = fx.mem.u8(t + 0x67);
        if a >= 0xE {
            fx.mem.set8(t + 0x67, a - 0xC);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    } else {
        let a = fx.mem.u8(t + 0x67);
        if a >= 9 {
            fx.mem.set8(t + 0x67, a - 7);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    if fx.mem.i16(t + 0x50) == 0 {
        fx.mem.write_u32(t + 0x4C, 0x4416_0000);
    }
    for kind in [1, 2] {
        if fx.mem.i16(t + 0x50) == kind {
            let r = ee::add(fx.mem.f(t + 0x4C), 10.0);
            fx.mem.setf(t + 0x4C, r);
        }
    }
    l2::camera_angles_to(fx, t + 0x20);
    let a = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, a);
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E03070: a random sparkle cluster (radius +0x50 3000..4999 shrinking
/// by +0xE).
fn step_sparkle(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x420C_0000);
        let r = fx.rand();
        fx.mem.set16(t + 0x56, (r >> 4) % 4);
        l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x80);
        for (o, m, add) in [(0x50u32, 0x7D0, 0xBB8), (0x5A, 0x168, 0), (0xE, 0xC8, 0xC8), (8, 0x23, 0), (6, 3, 3)] {
            let r = fx.rand();
            let (_, x) = fx.div(r, m);
            fx.mem.set16(t + o, x + add);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, rising, 0xC0, 0x40, 0x10, 0xE);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50);
        if r >= 0x1F {
            let e = fx.mem.i16(t + 0xE);
            fx.mem.set16(t + 0x50, r - e);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E03210: a sparkle ring growing (+0x48 by 1, radius +0x50 by +0xE).
fn step_ring(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4448_0000);
        let r = fx.rand();
        fx.mem.set16(t + 0x56, (r >> 4) % 4);
        l2::rgb(fx, t + 0x60, 0x20, 0x20, 0x80);
        let v = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xA, v);
        for (o, m, add) in [(0x50u32, 0xF, 1), (0x5A, 0x168, 0), (0xE, 0x50, 0x50), (8, 0x23, 0), (6, 0xA, 8)] {
            let r = fx.rand();
            let (_, x) = fx.div(r, m);
            fx.mem.set16(t + o, x + add);
        }
        let r = fx.rand();
        fx.mem.set16(t + 0xC, r % 2);
        fx.mem.set16(t + 0x58, 2);
    }
    let rising = fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6);
    fade(fx, t, rising, 0x80, 0x2A, 0xD, 0xB);
    if fx.mem.i16(t + 0x54) != 0 {
        let s = ee::add(fx.mem.f(t + 0x48), 1.0);
        fx.mem.setf(t + 0x48, s);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x20);
}

/// 0x1E033E0: a tall glow column (200 x 2000 or 1200, growing 140 / 80).
fn step_glow(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x40C0_0000);
        l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
        l2::rgb(fx, t + 0x64, 0xC0, 0x10, 0x10);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x30, 0x4348_0000);
            fx.mem.write_u32(t + 0x34, 0x44FA_0000);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x30, 0x4348_0000);
            fx.mem.write_u32(t + 0x34, 0x4496_0000);
        }
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    let rising = fx.mem.i16(t + 0x54) < 0xC;
    fade(fx, t, rising, 0x80, 0x40, 9, 7);
    if fx.mem.i16(t + 0x50) == 0 && fx.mem.i16(t + 0x54) != 0 {
        let h = ee::add(fx.mem.f(t + 0x34), 140.0);
        fx.mem.setf(t + 0x34, h);
    }
    if fx.mem.i16(t + 0x50) == 1 && fx.mem.i16(t + 0x54) != 0 {
        let h = ee::add(fx.mem.f(t + 0x34), 80.0);
        fx.mem.setf(t + 0x34, h);
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    let a = fx.mem.i16(t + 0x5A) + 6;
    fx.mem.set16(t + 0x5A, a);
    fx.leave(0x20);
}

/// 0x1E03550: the frame-buffer tint: a window sprite (0x6FF8..0x8FF8)
/// sampling the frame (0x207EF0) at ST from +0x48 / 512, colour +0x60,
/// ALPHA 0x44, key 0; reddening from frame 0xF1, fading after 0x118.
fn step_tint(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0xA0, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let d = sp + 0x38;
    fx.mem.set16(d + 2, 0x26);
    fx.mem.set16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.set32(d + 4, 0);
    for k in 0..4 {
        let b = fx.mem.read_u8(t + 0x60 + k);
        fx.mem.write_u8(sp + 0x48 + k, b);
    }
    fx.mem.write_u32(sp + 0x4C, 0x3F80_0000);
    for (at, v) in [(sp + 0x58, 0x6FF8u16), (sp + 0x68, 0x8FF8)] {
        fx.mem.write_u16(at, v);
        fx.mem.write_u16(at + 2, v);
        let x = fx.mem.read_u64(at);
        fx.mem.write_u64(at, (x & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFFFu64 << 32));
        fx.mem.write_u8(at + 7, 0);
    }
    let s = fx.mem.f(t + 0x48);
    let s0 = ee::div(ee::add(s, 0.0), 512.0);
    fx.mem.setf(sp + 0x50, s0);
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(ee::add(s, 0.0), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(ee::add(512.0, s), 0.0), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(ee::add(512.0, s), 0.0), 512.0));
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i16(D + 0x693C) == 0xF1 {
        l2::rgb(fx, t + 0x60, 0x90, 0x70, 0x80);
    }
    if fx.mem.i16(t + 0x54) < 0x118 {
        if fx.mem.i16(D + 0x693C) < 0xD8 {
            let a = fx.mem.u8(t + 0x63);
            if a < 0x50 {
                fx.mem.set8(t + 0x63, a + 0xA);
            }
        }
        if fx.mem.i16(D + 0x693C) >= 0xD9 {
            let a = fx.mem.u8(t + 0x63);
            if a >= 0x41 {
                fx.mem.set8(t + 0x63, a - 6);
            }
        }
    } else {
        let a = fx.mem.u8(t + 0x63);
        if a >= 0xB {
            fx.mem.set8(t + 0x63, a - 9);
        } else {
            fx.mem.set16(t + 0x58, 0);
        }
    }
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
    fx.leave(0x90);
}

/// 0x1E037C0(D, pts, base, side): five points round `base` (x the
/// radius, y the height, z the depth) at angles 90, 18 / 162, 162 / 18,
/// -54 / 234, 234 / -54 degrees by side.
fn target_points(fx: &mut Fx, pts: u32, base: u32, side: i32) {
    let sp = fx.enter(0x90);
    for k in 0..4 {
        let w = fx.mem.read_u32(base + 4 * k);
        fx.mem.write_u32(sp + 0x80 + 4 * k, w);
    }
    let z = fx.mem.f(sp + 0x88);
    let y = fx.mem.f(sp + 0x84);
    let mut p = pts;
    for k in 0..5 {
        l2::set3(fx, p, 0.0, 0.0, z);
        let a = match k {
            0 => 0x5A,
            1 if side == 0 => 0x12,
            1 => 0xA2,
            2 if side == 0 => 0xA2,
            2 => 0x12,
            3 if side == 0 => -0x36,
            3 => 0xEA,
            _ if side == 0 => 0xEA,
            _ => -0x36,
        };
        let s = l2::sin_deg(fx, D + 0xD8, a, 0);
        let r = fx.mem.f(sp + 0x80);
        fx.mem.setf(p + 4, ee::add(y, ee::mul(r, s)));
        let c = l2::sin_deg(fx, D + 0xD8, a, 1);
        let r = fx.mem.f(sp + 0x80);
        fx.mem.setf(p, ee::mul(r, c));
        p += 0x10;
    }
    fx.leave(0x90);
}

/// 0x1E039C0(D): the limbs' handles from the target (slots k0..k3 packed
/// 9/6/3/0 and the leader 0 of the side), every unit of both sides
/// flagged (0x1F6B80(h, 1)) with its card's rotation saved (+0x6460),
/// then each limb's record from its card object (0x1E03AD8..).
fn limbs_setup(fx: &mut Fx) {
    let sp = fx.enter(0x60);
    let t = fx.target_handle();
    let side = fx.mem.i16(D + 0x693A);
    let base = match side {
        0 => Some(0),
        1 => Some(0x100),
        _ => None,
    };
    if let Some(b) = base {
        fx.mem.set32(D + 0x687C, b);
        fx.mem.set32(D + 0x6884, ((t >> 6) & 7) + b);
        fx.mem.set32(D + 0x6880, ((t >> 9) & 7) + b);
        fx.mem.set32(D + 0x688C, (t & 7) + b);
        fx.mem.set32(D + 0x6888, ((t >> 3) & 7) + b);
    }
    for k in 0..0x16i32 {
        let h = if k < 0xB { k } else { k + 0xF5 };
        fx.unit_flags(h, 1);
        fx.card_rotation(h, D + 0x6460 + 0x10 * k as u32);
    }
    for k in 0..5u32 {
        let h = fx.mem.i32(D + 0x687C + 4 * k);
        let rec = D + 0x5A50 + 0x70 * k;
        fx.mem.set32(rec + 0x64, 0);
        let r = fx.rand();
        let (_, m) = fx.div(r, 0x168);
        fx.mem.set32(rec + 0x24, m);
        fx.mem.set32(D + 0x689C + 4 * k, 0x1E);
        fx.card_position(h, rec);
        fx.mem.set32(rec + 4, 0);
        fx.card_set_position(h, rec);
        fx.card_rotation(h, rec + 0x30);
        fx.mem.set32(rec + 0x34, 0);
        fx.mem.set32(rec + 0x30, 0x4000);
        fx.card_set_rotation(h, rec + 0x30);
        fx.card_colour(h, rec + 0x60);
        fx.mem.write_u8(rec + 0x63, 0x80);
        for i in 0..3 {
            fx.mem.write_u32(rec + 0x40 + 4 * i, 0x3DCC_CCCD);
        }
        fx.card_set_scale(h, rec + 0x40);
    }
    let _ = sp;
    fx.leave(0x60);
}

/// One rotation word stepped by 0x700 toward 0 (from 0x8000 up toward
/// 0x10000) as 0x1E03D9C / 0x1E03E28 do; `true` when it arrived (set 0).
fn turn_home(v: i32) -> (i32, bool) {
    if v >= 0x8001 {
        if v < 0xF800 {
            (v + 0x700, false)
        } else {
            (0, true)
        }
    } else if v >= 0x8000 || v < 0x801 {
        (0, true)
    } else {
        (v - 0x700, false)
    }
}

/// 0x1E03BC0(D, rec, k): limb k's machine (+0x64): 0 turn its card home
/// (x to 0; y to 0x8000 or 0 by side; z to 0), 1 (by 0x1E04D7C) / 2 fly
/// to target k in 28 steps then shrink the radius, 3 / 5 wait for stages
/// 0xC / 0x3E then aim again, 4 / 6 fly; the card follows at its angle +
/// radius; all parked at frame 0x369.
fn limb_step(fx: &mut Fx, rec: u32, k: u32) {
    let sp = fx.enter(0x100);
    let h = fx.mem.i32(D + 0x687C + 4 * k);
    let pt = D + 0x10 * k + 0x67B0;
    let radius = D + 4 * k + 0x68BC;
    if fx.mem.i16(D + 0x6924) != 0 {
        fx.leave(0x100);
        return;
    }
    if fx.card_exists(h) != 0 {
        l2::copy3_dir(fx, rec, sp + 0xC0, 1);
        let dx = ee::sub(fx.mem.f(pt), ee::add(fx.mem.f(rec), fx.mem.f(rec + 0x10)));
        let f3 = ee::add(fx.mem.f(rec + 8), fx.mem.f(rec + 0x18));
        let f0 = ee::add(fx.mem.f(rec + 4), fx.mem.f(rec + 0x14));
        let dz = ee::sub(fx.mem.f(pt + 8), f3);
        let dy = ee::sub(fx.mem.f(pt + 4), f0);
        let ax = fx.sqrtf(ee::mul(dx, dx));
        let az = fx.sqrtf(ee::mul(dz, dz));
        let ay = fx.sqrtf(ee::mul(dy, dy));
        let (f20, f22, f21, f23) = (ax, ax, az, az);
        let mut s1 = 0i32;
        let near = |v: f32| ee_le(v, 2.0);
        let step_axis = |fx: &mut Fx, close: bool, p: u32, v: u32, s1: &mut i32| {
            if !close {
                let x = ee::add(fx.mem.f(rec + p), fx.mem.f(rec + v));
                fx.mem.setf(rec + p, x);
            } else {
                *s1 = (*s1 + 1) as i16 as i32;
            }
        };
        let aim = |fx: &mut Fx, at: u32| {
            l2::spread(fx, sp + at, pt, rec, 28.0);
            for i in 0..4 {
                let w = fx.mem.read_u32(sp + at + 4 * i);
                fx.mem.write_u32(rec + 0x50 + 4 * i, w);
            }
            let n = fx.mem.i32(rec + 0x64) + 1;
            fx.mem.set32(rec + 0x64, n);
        };
        let bump_stage = |fx: &mut Fx| {
            let n = fx.mem.i32(D + 0x68FC) + 1;
            fx.mem.set32(D + 0x68FC, n);
        };
        match fx.mem.i32(rec + 0x64) {
            0 => {
                fx.card_rotation(h, rec + 0x30);
                let v = fx.mem.i32(rec + 0x30);
                if v < 0x801 {
                    s1 += 1;
                    fx.mem.set32(rec + 0x30, 0);
                } else {
                    fx.mem.set32(rec + 0x30, v - 0x700);
                }
                let v = fx.mem.i32(rec + 0x34);
                if fx.mem.i16(D + 0x693A) == 0 {
                    if v < 0x7800 {
                        fx.mem.set32(rec + 0x34, v + 0x700);
                    } else if v < 0x8801 {
                        fx.mem.set32(rec + 0x34, 0x8000);
                        s1 += 1;
                    } else {
                        fx.mem.set32(rec + 0x34, v - 0x700);
                    }
                } else {
                    let (n, done) = turn_home(v);
                    fx.mem.set32(rec + 0x34, n);
                    if done {
                        s1 += 1;
                    }
                }
                let side = fx.mem.i16(D + 0x693A);
                if side == 0 || side == 1 {
                    let v = fx.mem.i32(rec + 0x38);
                    let (n, done) = turn_home(v);
                    fx.mem.set32(rec + 0x38, n);
                    if done {
                        s1 += 1;
                    }
                }
                fx.card_set_rotation(h, rec + 0x30);
                if s1 == 3 {
                    let n = fx.mem.i32(rec + 0x64) + 1;
                    fx.mem.set32(rec + 0x64, n);
                    bump_stage(fx);
                    for (o, p) in [(0x50u32, 0u32), (0x58, 8), (0x54, 4)] {
                        let v = ee::div(ee::sub(fx.mem.f(pt + p), fx.mem.f(rec + p)), 28.0);
                        fx.mem.setf(rec + o, v);
                    }
                }
            }
            2 => {
                step_axis(fx, near(f20), 0x10, 0x50, &mut s1);
                step_axis(fx, near(f21), 0x18, 0x58, &mut s1);
                if !near(ay) {
                    let y = ee::add(fx.mem.f(rec + 0x14), fx.mem.f(rec + 0x54));
                    fx.mem.setf(rec + 0x14, y);
                } else if s1 == 2 {
                    s1 = 3;
                    let r = fx.mem.f(radius);
                    if !ee_le(r, 0.0) {
                        fx.mem.setf(radius, ee::sub(r, 0.5));
                    } else {
                        fx.mem.set32(radius, 0);
                        bump_stage(fx);
                        fx.mem.set32(rec + 0x64, 3);
                        for (o, p) in [(0x10u32, 0u32), (0x14, 4), (0x18, 8)] {
                            let v = ee::sub(fx.mem.f(pt + p), fx.mem.f(rec + p));
                            fx.mem.setf(rec + o, v);
                        }
                    }
                }
                if s1 != 3 {
                    let r = ee::add(fx.mem.f(radius), f32::from_bits(0x3F4C_CCCD));
                    fx.mem.setf(radius, r);
                }
            }
            3 => {
                if fx.mem.i32(D + 0x68FC) == 0xC {
                    aim(fx, 0xE0);
                }
            }
            4 => {
                step_axis(fx, near(f20), 0x10, 0x50, &mut s1);
                step_axis(fx, near(f21), 0x18, 0x58, &mut s1);
                if !near(ay) {
                    let y = ee::add(fx.mem.f(rec + 0x14), fx.mem.f(rec + 0x54));
                    fx.mem.setf(rec + 0x14, y);
                } else if s1 == 2 {
                    fx.mem.set32(radius, 0);
                    bump_stage(fx);
                    let n = fx.mem.i32(rec + 0x64) + 1;
                    fx.mem.set32(rec + 0x64, n);
                }
            }
            5 => {
                if fx.mem.i32(D + 0x68FC) == 0x3E {
                    aim(fx, 0xF0);
                }
            }
            6 => {
                if !near(f22) {
                    let x = ee::add(fx.mem.f(rec + 0x10), fx.mem.f(rec + 0x50));
                    fx.mem.setf(rec + 0x10, x);
                }
                if !near(f23) {
                    let z = ee::add(fx.mem.f(rec + 0x18), fx.mem.f(rec + 0x58));
                    fx.mem.setf(rec + 0x18, z);
                }
                if !near(ay) {
                    let y = ee::add(fx.mem.f(rec + 0x14), fx.mem.f(rec + 0x54));
                    fx.mem.setf(rec + 0x14, y);
                }
            }
            _ => {}
        }
        if fx.mem.i16(D + 0x693C) == 0x127 {
            for i in 0..3 {
                fx.mem.set32(rec + 0x40 + 4 * i, 0);
            }
            fx.card_set_scale(h, rec + 0x40);
        }
        fx.add3(sp + 0xC0, rec + 0x10);
        let a = fx.mem.i32(rec + 0x24).wrapping_add(fx.mem.i32(D + 0x689C + 4 * k));
        fx.mem.set32(rec + 0x24, a);
        let a = a as i16 as i32;
        let c = l2::sin_deg(fx, D + 0x6B40, a, 1);
        let r = fx.mem.f(radius);
        let x = ee::add(fx.mem.f(sp + 0xC0), ee::mul(r, c));
        fx.mem.setf(sp + 0xC0, x);
        let s = l2::sin_deg(fx, D + 0x6B40, a, 0);
        let r = fx.mem.f(radius);
        let y = ee::add(fx.mem.f(sp + 0xC4), ee::mul(r, s));
        fx.mem.setf(sp + 0xC4, y);
        l2::copy3_dir(fx, sp + 0xD0, sp + 0xC0, 0);
        fx.card_set_position(h, sp + 0xD0);
    }
    if fx.mem.i16(D + 0x693C) >= 0x369 {
        fx.mem.set32(rec + 4, 0);
        fx.card_set_position(h, rec);
        fx.mem.write_u8(rec + 0x63, 0x80);
        fx.card_set_colour(h, rec + 0x60);
        fx.mem.set32(sp + 0xB0, 0x4000);
        fx.mem.set32(sp + 0xB4, 0);
        fx.mem.set32(sp + 0xB8, 0);
        fx.card_set_rotation(h, sp + 0xB0);
    }
    fx.leave(0x100);
}

/// 0x107A60(h, 0, 0, on): a unit drawn (0x1F70D0) with `on` shrinks by
/// 0.18 per axis (floor 0) through its object's scale (0x1F7550 / 0x1F74C0).
fn card_shrink(fx: &mut Fx, h: i32) {
    let sp = fx.enter(0x40);
    if fx.unit_drawn(h) != 0 {
        fx.unit_scale(h, sp + 0x30);
        for k in 0..3u32 {
            let v = ee::sub(fx.mem.f(sp + 0x30 + 4 * k), f32::from_bits(0x3E38_51EC));
            fx.mem.setf(sp + 0x30 + 4 * k, v);
            if ee_lt(v, 0.0) {
                fx.mem.set32(sp + 0x30 + 4 * k, 0);
            }
        }
        fx.unit_set_scale(h, sp + 0x30);
    }
    fx.leave(0x40);
}

/// 0x107B60(h): the card's alpha down by 0x13 (0 below 0x15); 1 when out.
fn card_fade_out(fx: &mut Fx, h: i32) -> i32 {
    let sp = fx.enter(0x40);
    if fx.card_exists(h) == 0 {
        fx.leave(0x40);
        return 0;
    }
    fx.card_colour(h, sp + 0x3C);
    let a = fx.mem.u8(sp + 0x3F);
    fx.mem.set8(sp + 0x3F, if a >= 0x15 { a - 0x13 } else { 0 });
    fx.card_set_colour(h, sp + 0x3C);
    let r = i32::from(fx.mem.read_u8(sp + 0x3F) == 0);
    fx.leave(0x40);
    r
}

/// 0x107C50(h, 0, f): a drawn unit's scale + f per axis, capped at 1.
fn card_grow(fx: &mut Fx, h: i32, f: f32) {
    let sp = fx.enter(0x40);
    if fx.unit_drawn(h) != 0 {
        fx.unit_scale(h, sp + 0x30);
        for k in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0x30 + 4 * k), f);
            fx.mem.setf(sp + 0x30 + 4 * k, v);
            if !ee_le(v, 1.0) {
                fx.mem.write_u32(sp + 0x30 + 4 * k, 0x3F80_0000);
            }
        }
        fx.unit_set_scale(h, sp + 0x30);
    }
    fx.leave(0x40);
}

/// 0x107F30(h, add): the card's alpha up by `add` (wrapping, capped at
/// 0x80); 1 when full.
fn card_fade_in(fx: &mut Fx, h: i32, add: i32) -> i32 {
    let sp = fx.enter(0x50);
    if fx.card_exists(h) == 0 {
        fx.leave(0x50);
        return 0;
    }
    fx.card_colour(h, sp + 0x4C);
    let a = fx.mem.u8(sp + 0x4F) + add;
    fx.mem.set8(sp + 0x4F, a);
    if fx.mem.u8(sp + 0x4F) >= 0x80 {
        fx.mem.write_u8(sp + 0x4F, 0x80);
    }
    fx.card_set_colour(h, sp + 0x4C);
    let r = i32::from(fx.mem.u8(sp + 0x4F) >= 0x80);
    fx.leave(0x50);
    r
}

impl EffectPort for C062 {
    fn update(&mut self, world: &dyn EffectWorld) -> EffectUpdate {
        if self.fx.finished {
            return EffectUpdate { ret: 8, ..Default::default() };
        }
        self.fx.begin(world);
        let ret = if self.fx.calls == 0 { self.init(0) } else { self.step() };
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
