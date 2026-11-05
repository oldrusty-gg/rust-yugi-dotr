//! CMODULE c063 (program 0x3F), the last of the Exodia finale's three
//! modules, hand-ported from the module (linked at 0x1E00000; entry
//! 0x1E04AA0, data area 0x1E10000; 6,800 words, of them 2,300 shared
//! with c061 and taken from [`crate::c061`]) [F] (helper A, 30 Oct
//! 2026). Its draws use the second effect library ([`crate::fxlib2`]).
//!
//! Launch: the data area cleared (0x6FB0 bytes), the countdown +0x6B30 =
//! 0x3B9, degree sine tables (+0x6E3C, +0xD8), the side (+0x6C38 = self
//! handle != 0, +0x6C36 the other), node slots 0..6 (+0x6BC8), TEXEFF 14 /
//! 15 (+0x298 / +0x29C), type-5 records 0..5 (+0x6B10..), random byte
//! tables (+0x6C3C, +0x6D3C) and rectangle indices (+0x6BE8), the Lord
//! object 0x20's position (+0x6AF0, z 50 toward the far side).
//! Each update: the timeline (0x1E01690, frame +0x6C3A) spawns tasks
//! (0x1E05730: the first free of 200 at +0x2A0, 0x70 each) and moves the
//! Lord's chest point (+0x5A80, nodes +0x6BCC / +0x6BD4); the tasks are
//! stepped (slots 2..199, then 0 and 1) and drawn by step function
//! (sparks, gouraud rings, sprite rings and bursts, a fan, ribbons round
//! the Lord, a 12-segment ring, full-window tints, the frame-buffer tint
//! 0x207EF0, the stage light 0x1F7F10). Returns 2 at frame 0x366, 8 when
//! the countdown ends.
use crate::bus::{Bus, Image};
use crate::c061::{blend_alpha, scale_tenth, set_rect, shift_points, sparks, sprite_one, sprite_ring, ribbon};
use crate::c062::project_point;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};
use crate::fxlib2 as l2;
use crate::types::{EffectTextures, EffectUpdate, EffectWorld};
use crate::EffectPort;
use std::sync::Arc;

/// CMODULE program id.
pub const ID: u16 = 0x3F;
/// Its entry in the type-0 table 0x2B8510.
pub const ENTRY: u32 = 0x01E0_4AA0;
const D: u32 = 0x01E1_0000;
const LORD: i32 = 0x20;
const SPR: u32 = 0x7000_0000;
const STEP_RING: u32 = 0x01E0_2340;
const STEP_FAN: u32 = 0x01E0_2640;
const STEP_BURST: u32 = 0x01E0_29C0;
const STEP_HALO: u32 = 0x01E0_2D00;
const STEP_GLINT: u32 = 0x01E0_2E00;
const STEP_SPRAY: u32 = 0x01E0_2F40;
const STEP_WAVE: u32 = 0x01E0_31E0;
const STEP_STAR: u32 = 0x01E0_3430;
const STEP_ORBIT: u32 = 0x01E0_35E0;
const STEP_SHOCK: u32 = 0x01E0_3B30;
const STEP_RING12: u32 = 0x01E0_3DB0;
const STEP_SPARKS: u32 = 0x01E0_3F10;
const STEP_DEBRIS: u32 = 0x01E0_4250;
const STEP_WASH: u32 = 0x01E0_4510;
const STEP_LIGHT: u32 = 0x01E0_46E0;
const STEP_TINT: u32 = 0x01E0_47A0;

/// A running c063.
pub struct C063 {
    fx: Fx,
    motion: [(u8, f32, u8); 2],
}

impl C063 {
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

    /// The launch (0x1E04AD4..0x1E04DC4).
    fn init(&mut self) -> i32 {
        let fx = &mut self.fx;
        fx.enter(0x180);
        fx.mem.write_u32(D + 0x6B98, OT);
        fx.mem.write_u32(D + 0x80, OT);
        fx.bzero(D, 0x6FB0);
        fx.mem.set32(D + 0x6B30, 0x3B9);
        l2::sine_table(fx, D + 0x6E3C);
        l2::sine_table(fx, D + 0xD8);
        let own = fx.self_handle();
        fx.mem.set16(D + 0x6C38, i32::from(own != 0));
        fx.mem.set32(D + 0x6B30, 0x3B9);
        for k in 0..7u32 {
            fx.mem.set16(D + 0x6BC8 + 2 * k, k as i32);
        }
        for k in 0..2u32 {
            let t = fx.texture(4, 0xE + k as i32);
            fx.mem.write_u32(D + 0x298 + 4 * k, t);
        }
        for k in 0..6u32 {
            let t = fx.texture(5, k as i32);
            fx.mem.write_u32(D + 0x6B10 + 4 * k, t);
        }
        let side = fx.mem.i16(D + 0x6C38);
        fx.mem.set16(D + 0x6C36, i32::from(side == 0));
        for k in 0..3 {
            fx.mem.write_u32(D + 0x10 + 4 * k, 0x3DCC_CCCD);
        }
        for k in 0..0x100u32 {
            let r = fx.rand();
            let (_, m) = fx.div(r, 0x104);
            fx.mem.set8(D + 0x6C3C + k, m - 0x82);
        }
        for k in 0x64..0xB4u32 {
            let r = fx.rand();
            fx.mem.set8(D + 0x6C3C + k, r % 4 - 2);
        }
        for k in 0xB4..0x100u32 {
            let r = fx.rand();
            let (_, m) = fx.div(r, 0x28);
            fx.mem.set8(D + 0x6C3C + k, m - 0x14);
        }
        for k in 0..0x100u32 {
            let r = fx.rand();
            let (_, m) = fx.div(r, 0x28);
            fx.mem.set8(D + 0x6D3C + k, m - 0x14);
        }
        for k in 0..0x1Eu32 {
            let r = fx.rand();
            fx.mem.set16(D + 0x6BE8 + 2 * k, r % 8);
        }
        for k in 0..3 {
            fx.mem.write_u32(D + 0x5A50 + 4 * k, 0x3F19_999A);
        }
        fx.mem.write_u32(D + 0x5A64, 0x4348_0000);
        fx.object_position(LORD, D + 0x6AF0);
        let z = fx.mem.f(D + 0x6AF8);
        if fx.mem.i16(D + 0x6C38) == 0 {
            fx.mem.setf(D + 0x6AF8, ee::sub(z, 50.0));
        } else {
            fx.mem.setf(D + 0x6AF8, ee::add(z, 50.0));
        }
        fx.leave(0x180);
        0
    }

    /// An update (0x1E04DCC..0x1E05708).
    fn step(&mut self) -> i32 {
        let fx = &mut self.fx;
        let sp = fx.enter(0x180);
        fx.mem.write_u32(D + 0x6B98, OT);
        fx.mem.write_u32(D + 0x80, OT);
        timeline(fx);
        for k in (2..200u32).chain(0..2) {
            let t = D + 0x2A0 + 0x70 * k;
            if fx.mem.i16(t + 0x58) != 0 {
                let f = fx.mem.read_u32(t + 0x44);
                if f != 0 {
                    run_step(fx, f, t, if k >= 2 { 0x01E0_4E10 } else { 0x01E0_4E64 });
                }
            }
        }
        for k in 0..200u32 {
            let t = D + 0x2A0 + 0x70 * k;
            if fx.mem.i16(t + 0x58) != 0 {
                draw_task(fx, sp, t);
            }
        }
        let n = fx.mem.i32(D + 0x6B30);
        if n != 0 {
            fx.mem.set32(D + 0x6B30, n - 1);
        }
        let ret = if fx.mem.i16(D + 0x6C3A) == 0x366 {
            2
        } else if fx.mem.i32(D + 0x6B30) > 0 {
            0
        } else {
            8
        };
        fx.leave(0x180);
        ret
    }
}

/// D+0x250 (the next task's position) = the vec4 at `from`.
fn spawn_at(fx: &mut Fx, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(D + 0x250 + 4 * k, w);
    }
}

/// `at` += `v` (f32).
fn addf(fx: &mut Fx, at: u32, v: f32) {
    let x = fx.mem.f(at);
    fx.mem.setf(at, ee::add(x, v));
}

/// `at` -= `v` (f32).
fn subf(fx: &mut Fx, at: u32, v: f32) {
    let x = fx.mem.f(at);
    fx.mem.setf(at, ee::sub(x, v));
}

/// `at` += (rand % n) - n / 2 (f32).
fn jitter(fx: &mut Fx, at: u32, n: i32) {
    let r = fx.rand();
    let (_, m) = fx.div(r, n);
    addf(fx, at, ee::from_i32(m - n / 2));
}

/// 0x1E01690(D, 0): the timeline (frame D+0x6C3A, counted first).
fn timeline(fx: &mut Fx) {
    let sp = fx.enter(0x50);
    let f = fx.mem.i16(D + 0x6C3A) + 1;
    fx.mem.set16(D + 0x6C3A, f);
    let f = fx.mem.i16(D + 0x6C3A);
    if (0xA7..0xF4).contains(&f) {
        spawn_at(fx, D + 0x6AF0);
        addf(fx, D + 0x254, 120.0);
        fx.mem.set16(D + 0x6C32, 0);
        let f = fx.mem.i16(D + 0x6C3A);
        let (_, m) = fx.div(f, 3);
        if m != 0 {
            spawn(fx, STEP_SPARKS);
        }
        if fx.mem.i16(D + 0x6C3A) & 1 == 1 {
            spawn(fx, STEP_SPARKS);
        }
    }
    let f = fx.mem.i16(D + 0x6C3A);
    if (0xA7..0xFB).contains(&f) {
        spawn_at(fx, D + 0x6AF0);
        addf(fx, D + 0x254, 120.0);
        fx.mem.set16(D + 0x6C32, 1);
        let f = fx.mem.i16(D + 0x6C3A);
        let (_, m) = fx.div(f, 3);
        if m != 1 {
            spawn(fx, STEP_SPARKS);
        }
    }
    let f = fx.mem.i16(D + 0x6C3A);
    let away = fx.mem.i16(D + 0x6C36) != 0;
    if f < 0x164 {
        for k in 0..4 {
            let w = fx.mem.read_u32(D + 0x6AF0 + 4 * k);
            fx.mem.write_u32(D + 0x5A90 + 4 * k, w);
        }
        if away {
            subf(fx, D + 0x5A98, 7.0);
        } else {
            addf(fx, D + 0x5A98, 7.0);
        }
    } else {
        if (0x163..0x16D).contains(&f) {
            let v = f32::from_bits(0x3FE6_6666);
            if away {
                addf(fx, D + 0x5A98, v);
            } else {
                subf(fx, D + 0x5A98, v);
            }
        }
        let f = fx.mem.i16(D + 0x6C3A);
        if (0x186..0x1F5).contains(&f) {
            let v = f32::from_bits(0x3F33_3333);
            if away {
                addf(fx, D + 0x5A98, v);
            } else {
                subf(fx, D + 0x5A98, v);
            }
        }
    }
    if fx.mem.i16(D + 0x6C3A) == 0x12D {
        spawn(fx, STEP_TINT);
    }
    if fx.mem.i16(D + 0x6C3A) == 0x12D {
        spawn(fx, STEP_LIGHT);
    }
    let f = fx.mem.i16(D + 0x6C3A);
    if (0xFA..0x209).contains(&f) {
        let (_, m) = fx.div(f, 3);
        let n = if m == 1 { 2 } else { 1 };
        for _ in 0..n {
            spawn_at(fx, D + 0x6AF0);
            let r = fx.rand();
            let (_, m) = fx.div(r >> 4, 200);
            addf(fx, D + 0x250, ee::from_i32(m - 100));
            subf(fx, D + 0x254, 70.0);
            spawn(fx, STEP_RING);
        }
    }
    let f = fx.mem.i16(D + 0x6C3A);
    if f == 0x271 || (0x2B1..0x362).contains(&f) {
        let slot = fx.mem.i16(D + 0x6BCC);
        fx.object_node(LORD, slot, D + 0x5A80);
        let slot = fx.mem.i16(D + 0x6BD4);
        fx.object_node(LORD, slot, sp + 0x40);
        for k in 0..3 {
            let a = fx.mem.f(D + 0x5A80 + 4 * k);
            let b = fx.mem.f(sp + 0x40 + 4 * k);
            let s = fx.mem.f(D + 0x5A50 + 4 * k);
            fx.mem.setf(D + 0x5A80 + 4 * k, ee::add(a, ee::mul(s, ee::sub(a, b))));
        }
        addf(fx, D + 0x5A84, 25.0);
        if fx.mem.i16(D + 0x6C3A) >= 0x2D7 {
            for k in 0..3 {
                let s = fx.mem.f(D + 0x5A50 + 4 * k);
                if ee_lt(s, 1.0) {
                    fx.mem.setf(D + 0x5A50 + 4 * k, ee::add(s, f32::from_bits(0x3C23_D70A)));
                }
            }
        }
    }
    if fx.mem.i16(D + 0x6C3A) == 0x271 {
        spawn(fx, STEP_GLINT);
        spawn(fx, STEP_HALO);
        fx.mem.set16(D + 0x6C32, 0);
        spawn(fx, STEP_WASH);
        fx.mem.set16(D + 0x6C32, 1);
        spawn(fx, STEP_WASH);
        fx.mem.set16(D + 0x6C32, 0);
        spawn(fx, STEP_SHOCK);
    }
    let f = fx.mem.i16(D + 0x6C3A);
    if (0x271..0x362).contains(&f) {
        spawn_at(fx, D + 0x5A80);
        spawn(fx, STEP_SPRAY);
        fx.mem.set16(D + 0x6C32, 2);
        spawn(fx, STEP_WAVE);
        spawn(fx, STEP_STAR);
    }
    let f = fx.mem.i16(D + 0x6C3A);
    if (0x279..0x362).contains(&f) {
        spawn_at(fx, D + 0x5A80);
        spawn(fx, STEP_RING12);
        let r = fx.mem.f(D + 0x5A64);
        if ee_lt(r, 850.0) {
            fx.mem.setf(D + 0x5A64, ee::add(r, 30.0));
        }
    }
    if fx.mem.i16(D + 0x6C3A) == 0x271 {
        let slot = fx.mem.i16(D + 0x6BCC);
        fx.object_node(LORD, slot, D + 0x5A70);
    }
    if fx.mem.i16(D + 0x6C3A) == 0x271 {
        for k in 0..4 {
            let w = fx.mem.read_u32(D + 0x5A80 + 4 * k);
            fx.mem.write_u32(D + 0x5A70 + 4 * k, w);
        }
    }
    if fx.mem.i16(D + 0x6C3A) == 0x271 {
        spawn_at(fx, D + 0x5A70);
        fx.mem.set32(D + 0x250, 0);
        addf(fx, D + 0x254, 20.0);
        for k in 0..5 {
            fx.mem.set16(D + 0x6C32, k);
            spawn(fx, STEP_FAN);
        }
    }
    if fx.mem.i16(D + 0x6C3A) == 0x16C {
        fx.mem.set16(D + 0x6C26, 0);
        spawn(fx, STEP_BURST);
        fx.mem.set16(D + 0x6C26, 0x5A);
        spawn(fx, STEP_BURST);
    }
    if fx.mem.i16(D + 0x6C3A) == 0x24E {
        fx.mem.set32(D + 0x250, 0);
        fx.mem.write_u32(D + 0x254, 0x428C_0000);
        fx.mem.set32(D + 0x258, 0);
        let mut a = 0i32;
        for k in 0..16 {
            fx.mem.set16(D + 0x6C32, k);
            fx.mem.set16(D + 0x6C26, a);
            spawn(fx, STEP_ORBIT);
            a = (a + 0x2D) as i16 as i32;
        }
    }
    if fx.mem.i16(D + 0x6C3A) == 0x2CB {
        fx.mem.set16(D + 0x6C32, 0);
        spawn(fx, STEP_SHOCK);
        spawn(fx, STEP_SHOCK);
        spawn(fx, STEP_SHOCK);
    }
    let f = fx.mem.i16(D + 0x6C3A);
    if (0x1F4..0x21F).contains(&f) {
        let n = if f < 0x21C { 1 } else { 3 };
        for node in [0x6BCCu32, 0x6BD2] {
            for _ in 0..n {
                let slot = fx.mem.i16(D + node);
                fx.object_node(LORD, slot, D + 0x250);
                if fx.mem.i16(D + 0x6C3A) >= 0x21C {
                    jitter(fx, D + 0x254, 0x14);
                    jitter(fx, D + 0x250, 0x14);
                } else {
                    jitter(fx, D + 0x254, 0xA);
                    jitter(fx, D + 0x250, 0xA);
                }
                spawn(fx, STEP_BURST);
            }
        }
    }
    if fx.mem.i16(D + 0x6C3A) == 0x357 {
        fx.mem.set16(D + 0x6C32, 2);
        spawn(fx, STEP_WASH);
    }
    if fx.mem.i16(D + 0x6C3A) == 0x2F4 {
        fx.mem.set16(D + 0x6C32, 1);
        spawn(fx, STEP_SHOCK);
    }
    let f = fx.mem.i16(D + 0x6C3A);
    if (0x2FE..0x361).contains(&f) {
        let r = fx.rand();
        let (_, m) = fx.div(r, 0x12C);
        fx.mem.setf(D + 0x250, ee::from_i32(m - 0x96));
        fx.mem.write_u32(D + 0x254, 0x41F0_0000);
        let r = fx.rand();
        let (_, m) = fx.div(r, 0x12C);
        fx.mem.setf(D + 0x258, ee::from_i32(m - 0x96));
    }
    fx.leave(0x50);
}

/// 0x1E05730(D, step, 0): a new task (the first free of 200, cleared) at
/// D+0x250 (xyz), angle D+0x6C26, index D+0x6C32.
fn spawn(fx: &mut Fx, step: u32) -> i32 {
    fx.enter(0x50);
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
    let ret = if t == 0 {
        0
    } else {
        for k in 0..3 {
            let w = fx.mem.read_u32(D + 0x250 + 4 * k);
            fx.mem.write_u32(t + 0x10 + 4 * k, w);
        }
        let v = fx.mem.i16(D + 0x6C26);
        fx.mem.set16(t + 0x5A, v);
        let v = fx.mem.i16(D + 0x6C32);
        fx.mem.set16(t + 0x50, v);
        fx.mem.write_u32(t + 0x44, step);
        1
    };
    fx.leave(0x50);
    ret
}

/// The entry's frame top (its sp - 0x10): a step function whose frame
/// saves `ra` there (frames 0x10 / 0x20 / 0x30) leaves the jalr's return;
/// the tint's TEX1 (0x207FC0 keeps its stale bits) reads it [F].
const ENTRY_TOP: u32 = crate::exe::STACK - 0x180 - 0x10;

fn run_step(fx: &mut Fx, f: u32, t: u32, ra: u32) {
    if !matches!(f, STEP_ORBIT | STEP_TINT) {
        fx.mem.write_u64(ENTRY_TOP, u64::from(ra));
    }
    match f {
        STEP_RING => step_ring(fx, t),
        STEP_FAN => step_fan(fx, t),
        STEP_BURST => step_burst(fx, t),
        STEP_HALO => step_halo(fx, t),
        STEP_GLINT => step_glint(fx, t),
        STEP_SPRAY => step_spray(fx, t),
        STEP_WAVE => step_wave(fx, t),
        STEP_STAR => step_star(fx, t),
        STEP_ORBIT => step_orbit(fx, t),
        STEP_SHOCK => step_shock(fx, t),
        STEP_RING12 => step_ring12(fx, t),
        STEP_SPARKS => step_sparks(fx, t),
        STEP_DEBRIS => step_debris(fx, t),
        STEP_WASH => step_wash(fx, t),
        STEP_LIGHT => step_light(fx, t),
        STEP_TINT => step_tint(fx, t),
        _ => {}
    }
}

/// The 16 bytes at `from` into `to`.
fn copy16(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..4 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The xyz at `from` into `to`.
fn copy12(fx: &mut Fx, to: u32, from: u32) {
    for k in 0..3 {
        let w = fx.mem.read_u32(from + 4 * k);
        fx.mem.write_u32(to + 4 * k, w);
    }
}

/// The entry's draw of one live task by its step function
/// (0x1E04E9C..0x1E056A4).
fn draw_task(fx: &mut Fx, sp: u32, t: u32) {
    let f = fx.mem.read_u32(t + 0x44);
    match f {
        STEP_SPARKS => {
            if fx.mem.i16(t + 0x50) == 0 {
                let tex = fx.mem.read_u32(D + 0x29C);
                fx.mem.write_u32(D + 0x88, tex);
                set_rect(fx, sp + 0x60, 0x97, 0);
            }
            if fx.mem.i16(t + 0x50) == 1 {
                let tex = fx.mem.read_u32(D + 0x29C);
                fx.mem.write_u32(D + 0x88, tex);
                let s = fx.mem.i16(t + 0x56);
                set_rect(fx, sp + 0x70, 0x9B, s);
            }
            copy16(fx, sp + 0x50, t + 0x10);
            fx.mem.set16(D + 0xB8, 6);
            sparks(fx, t, 0x20, sp + 0x50);
            fx.mem.set16(D + 0xB8, 0);
        }
        STEP_RING => {
            let s = fx.mem.i16(t + 0x56);
            set_rect(fx, sp + 0x80, 0x14, s);
            fx.mem.set16(D + 0xC0, 0);
            let tex = fx.mem.read_u32(D + 0x6B10);
            fx.mem.write_u32(D + 0x88, tex);
            fx.mem.set16(D + 0xB8, 5);
            copy12(fx, D + 0x20, t + 0x10);
            fx.mem.set16(D + 0xBE, 0);
            sprite_ring(fx, t, 0, D + 0x6C3C);
            fx.mem.set16(D + 0xB8, 0);
        }
        STEP_FAN => {
            let tex = fx.mem.read_u32(D + 0x6B14);
            fx.mem.write_u32(D + 0x88, tex);
            let s = fx.mem.i16(t + 0x56);
            set_rect(fx, sp + 0x90, 0x14, s);
            fx.mem.set16(D + 0xB8, 1);
            fx.mem.set16(D + 0xBE, 0);
            fan(fx, t, 0x20, t + 0x10);
        }
        STEP_BURST => {
            let tex = fx.mem.read_u32(D + 0x6B20);
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
        STEP_HALO => {
            fx.mem.set16(D + 0xB8, 0x24);
            gouraud_ring(fx, t, 0x21, D + 0x5A80);
            fx.mem.set16(D + 0xB8, 0);
        }
        STEP_GLINT => {
            let tex = fx.mem.read_u32(D + 0x6B18);
            fx.mem.write_u32(D + 0x88, tex);
            let v = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0x94, v << 6);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x3F);
            fx.mem.set16(D + 0x9A, 0x3F);
            fx.mem.set16(D + 0xBE, 0);
            sprite_one(fx, t, 0x21, D + 0x5A80);
        }
        STEP_SPRAY => {
            let s = fx.mem.i16(t + 0x56);
            set_rect(fx, sp + 0xA0, 0x14, s);
            fx.mem.set16(D + 0xC0, 0);
            let tex = fx.mem.read_u32(D + 0x6B1C);
            fx.mem.write_u32(D + 0x88, tex);
            fx.mem.set16(D + 0xB8, 3);
            copy12(fx, D + 0x20, t + 0x10);
            fx.mem.set16(D + 0xBE, 0);
            sprite_ring(fx, t, 0x20, D + 0x6D3C);
            fx.mem.set16(D + 0xB8, 0);
        }
        STEP_WAVE => {
            copy16(fx, sp + 0x50, D + 0x5A80);
            fx.mem.set16(D + 0xB8, 0xC);
            gouraud_ring(fx, t, 0x20, sp + 0x50);
            fx.mem.set16(D + 0xB8, 0);
        }
        STEP_STAR => {
            let tex = fx.mem.read_u32(D + 0x6B1C);
            fx.mem.write_u32(D + 0x88, tex);
            copy16(fx, sp + 0x50, D + 0x5A80);
            let s = fx.mem.i16(t + 0x56);
            set_rect(fx, sp + 0xB0, 0x14, s);
            fx.mem.set16(D + 0xB8, 6);
            sparks(fx, t, 0x20, sp + 0x50);
            fx.mem.set16(D + 0xB8, 0);
        }
        STEP_ORBIT => {
            let tex = fx.mem.read_u32(D + 0x6B1C);
            fx.mem.write_u32(D + 0x88, tex);
            set_rect(fx, sp + 0xC0, 0x14, 3);
            fx.mem.set16(D + 0xBE, 0);
            copy12(fx, sp + 0x50, t + 0x10);
            sprite_one(fx, t, 0x20, sp + 0x50);
            let tex = fx.mem.read_u32(D + 0x6B1C);
            fx.mem.write_u32(D + 0x88, tex);
            set_rect(fx, sp + 0xD0, 0x14, 3);
            let x = fx.mem.i16(D + 0x94) + 0xF;
            fx.mem.set16(D + 0x94, x);
            fx.mem.set16(D + 0x98, 1);
            for k in 0..3 {
                fx.mem.set32(D + 0x20 + 4 * k, 0);
            }
            fx.mem.set16(D + 0xBE, 1);
            let idx = fx.mem.i16(t + 0x50);
            let pts = D.wrapping_add((idx * 0xD0) as u32) + 0x5AB0;
            ribbon(fx, t, pts, 0);
        }
        STEP_SHOCK => {
            copy16(fx, sp + 0x50, D + 0x5A80);
            fx.mem.set16(D + 0xB8, 0x24);
            if fx.mem.i16(t + 0x50) == 0 {
                gouraud_ring(fx, t, 0x21, sp + 0x50);
            }
            if fx.mem.i16(t + 0x50) == 1 {
                gouraud_ring(fx, t, 0x21, sp + 0x50);
            }
            fx.mem.set16(D + 0xB8, 0);
        }
        STEP_RING12 => {
            let tex = fx.mem.read_u32(D + 0x29C);
            fx.mem.write_u32(D + 0x88, tex);
            let s = fx.mem.i16(t + 0x56);
            set_rect(fx, sp + 0xE0, 0x97, s);
            copy16(fx, sp + 0x50, D + 0x5A80);
            ring12(fx, t, sp + 0x50, 0x20);
        }
        STEP_DEBRIS => {
            let tex = fx.mem.read_u32(D + 0x6B24);
            fx.mem.write_u32(D + 0x88, tex);
            fx.mem.set16(D + 0xBE, 0);
            let v = fx.mem.i16(t + 0x56);
            fx.mem.set16(D + 0x94, v << 5);
            fx.mem.set16(D + 0x96, 0);
            fx.mem.set16(D + 0x98, 0x20);
            fx.mem.set16(D + 0x9A, 0x40);
            sprite_one(fx, t, 0x21, t + 0x10);
        }
        STEP_WASH => {
            fx.object_position(LORD, sp + 0x50);
            let z = fx.mem.f(sp + 0x58);
            let far = f32::from_bits(0x453B_8000);
            if fx.mem.i16(D + 0x6C36) != 0 {
                fx.mem.setf(sp + 0x58, ee::add(z, far));
            } else {
                fx.mem.setf(sp + 0x58, ee::sub(z, far));
            }
            project_point(fx, sp + 0x50, 0, sp + 0x108, D + 0x6BB4);
            for (kind, mode) in [(0, 1), (1, 0x20), (2, 0x20)] {
                if fx.mem.i16(t + 0x50) == kind {
                    let z = fx.mem.f(D + 0x6BB4);
                    wash(fx, t, mode, z);
                }
            }
        }
        _ => {}
    }
}

/// 0x1E00AD0(D, task, mode, pos): D+0xB8 untextured gouraud segments
/// (strip4, colours +0x60 / +0x64) on a circle at `pos` turned by +0x20:
/// inner radius +0x48, outer +0x4C, each from angle a to a + +0x5C
/// degrees, a stepping by +0x5A; blend (mode >> 4), depth mode & 15 (0 the
/// mean w, 1 the far Z at key 0).
fn gouraud_ring(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1F0);
    copy16(fx, sp + 0xA0, pos);
    let span = fx.mem.i16(t + 0x5C);
    let d = sp + 0x160;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    scale_tenth(fx, sp + 0x100);
    copy12(fx, sp + 0xF0, sp + 0xA0);
    l2::xform_vp(fx, sp + 0x110, sp + 0x100, t + 0x20, sp + 0xF0);
    for o in [0xB4u32, 0xC4, 0xD4, 0xE4] {
        fx.mem.set32(sp + o, 0);
    }
    let (inner, outer) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let s5 = mode & 0xF;
    let tab = D + 0xD8;
    let mut a = 0i32;
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0xB8) {
        let s6 = a as i16 as i32;
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
    fx.leave(0x1F0);
}

/// 0x1E010D0(D, task, mode, z): a window-covering gouraud quad (colours
/// +0x60 / +0x64), blend (mode >> 4); mode & 15 = 0 at the far Z, key 0,
/// else keyed by `z`.
fn wash(fx: &mut Fx, t: u32, mode: i32, z: f32) {
    let sp = fx.enter(0xF0);
    let d = sp + 0x60;
    l2::colour_strip4(fx, d, t + 0x60, 1);
    l2::colour_strip4(fx, d, t + 0x64, 2);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    if mode & 0xF == 0 {
        l2::far_z_desc(fx, d, 1);
        fx.submit(d, OT, 0);
    } else {
        let key = ee::fixunssfsi(z) as i32;
        fx.submit(d, OT, key);
    }
    fx.leave(0xF0);
}

/// 0x1E05820(D, task, mode, pos): a fan of 18 textured strip3 segments
/// (20 degrees each from +0x5A - 200, radii +0x48 / +0x4C, colours +0x60
/// / +0x64), each with its own rectangle (0x106960(0x14, D+0x6BE8[k])),
/// turned by +0x20 at `pos` (scale 10), keyed by the mean w.
fn fan(fx: &mut Fx, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1F0);
    copy16(fx, sp + 0x90, pos);
    let d = sp + 0x160;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 1);
    l2::colour_strip3(fx, d, t + 0x64, 2);
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    fx.desc_alpha(d, blend_alpha(mode >> 4));
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xF0 + 4 * k, 0x4120_0000);
    }
    copy12(fx, sp + 0xE0, sp + 0x90);
    l2::xform_vp(fx, sp + 0x100, sp + 0xF0, t + 0x20, sp + 0xE0);
    for o in [0xA4u32, 0xB4, 0xC4, 0xD4] {
        fx.mem.set32(sp + o, 0);
    }
    let (f20, f21) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let tab = D + 0x6E3C;
    let mut s1 = 0i32;
    for k in 0..0x12u32 {
        let id = fx.mem.i16(D + 0x6BE8 + 2 * k);
        set_rect(fx, sp + 0x150, 0x14, id);
        let order = fx.mem.i16(D + 0xBE);
        l2::uv_strip3(fx, d, D + 0x94, order);
        let s6 = (fx.mem.i16(t + 0x5A) + s1 - 0xC8) as i16 as i32;
        let c = l2::sin_deg(fx, tab, s6, 1);
        let s = l2::sin_deg(fx, tab, s6, 0);
        fx.mem.setf(sp + 0xC0, ee::mul(c, f20));
        fx.mem.setf(sp + 0xC8, ee::mul(s, f20));
        fx.mem.setf(sp + 0xA0, ee::mul(c, f21));
        fx.mem.setf(sp + 0xA8, ee::mul(s, f21));
        let s6 = (s1 + fx.mem.i16(t + 0x5A) + 0x14 - 0xC8) as i16 as i32;
        let c = l2::sin_deg(fx, tab, s6, 1);
        let s = l2::sin_deg(fx, tab, s6, 0);
        fx.mem.setf(sp + 0xD0, ee::mul(c, f20));
        fx.mem.setf(sp + 0xD8, ee::mul(s, f20));
        fx.mem.setf(sp + 0xB0, ee::mul(c, f21));
        fx.mem.setf(sp + 0xB8, ee::mul(s, f21));
        if l2::project(fx, sp + 0xA0, SPR, sp + 0x100, d, sp + 0x140) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x140)) as i32;
            let ot = fx.mem.read_u32(D + 0x6B98);
            fx.submit(d, ot, key);
        }
        s1 = (s1 + 0x14) as i16 as i32;
    }
    fx.leave(0x1F0);
}

/// 0x1E06620(D, task, pos, mode): a ring of 12 textured strip3 segments
/// (30 degrees each from +0x5A) between radii 90 cos(+0x50) and
/// 90 cos(+0x50 + 20) at heights 90 sin of them, turned by +0x20 at `pos`
/// (scale +0x48 / 1500), keyed by the mean w.
fn ring12(fx: &mut Fx, t: u32, pos: u32, mode: i32) {
    let sp = fx.enter(0x200);
    copy16(fx, sp + 0xB0, pos);
    let d = sp + 0x170;
    l2::desc_strip3(fx, d);
    l2::colour_strip3(fx, d, t + 0x60, 0);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    let order = fx.mem.i16(D + 0xBE);
    l2::uv_strip3(fx, d, D + 0x94, order);
    let tex = fx.mem.read_u32(D + 0x88);
    fx.desc_texture(d, tex, 1);
    copy12(fx, sp + 0x100, sp + 0xB0);
    let s = ee::div(fx.mem.f(t + 0x48), 1500.0);
    l2::set3(fx, sp + 0x110, s, s, s);
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x20, sp + 0x100);
    let tab = D + 0xD8;
    let a = fx.mem.i16(t + 0x50);
    let y = ee::mul(90.0, l2::sin_deg(fx, tab, a, 0));
    fx.mem.setf(sp + 0xC4, y);
    fx.mem.setf(sp + 0xD4, y);
    let b = (fx.mem.i16(t + 0x50) + 0x14) as i16 as i32;
    let y = ee::mul(90.0, l2::sin_deg(fx, tab, b, 0));
    fx.mem.setf(sp + 0xE4, y);
    fx.mem.setf(sp + 0xF4, y);
    let a = fx.mem.i16(t + 0x50);
    let f20 = ee::mul(90.0, l2::sin_deg(fx, tab, a, 1));
    let b = (fx.mem.i16(t + 0x50) + 0x14) as i16 as i32;
    let f21 = ee::mul(90.0, l2::sin_deg(fx, tab, b, 1));
    let mut s1 = 0i32;
    for k in 0..12 {
        if k == 0 {
            let s3 = (fx.mem.i16(t + 0x5A) + s1) as i16 as i32;
            let v = ee::mul(f21, l2::sin_deg(fx, tab, s3, 1));
            fx.mem.setf(sp + 0xE0, v);
            let v = ee::mul(f21, l2::sin_deg(fx, tab, s3, 0));
            fx.mem.setf(sp + 0xE8, v);
            let v = ee::mul(f20, l2::sin_deg(fx, tab, s3, 1));
            fx.mem.setf(sp + 0xC0, v);
            let v = ee::mul(f20, l2::sin_deg(fx, tab, s3, 0));
            fx.mem.setf(sp + 0xC8, v);
        } else {
            for (to, from) in [(0xE0u32, 0xF0u32), (0xE8, 0xF8), (0xC0, 0xD0), (0xC8, 0xD8)] {
                let w = fx.mem.read_u32(sp + from);
                fx.mem.write_u32(sp + to, w);
            }
        }
        let s3 = (fx.mem.i16(t + 0x5A) + 0x1E + s1) as i16 as i32;
        let v = ee::mul(f21, l2::sin_deg(fx, tab, s3, 1));
        fx.mem.setf(sp + 0xF0, v);
        let v = ee::mul(f21, l2::sin_deg(fx, tab, s3, 0));
        fx.mem.setf(sp + 0xF8, v);
        let v = ee::mul(f20, l2::sin_deg(fx, tab, s3, 1));
        fx.mem.setf(sp + 0xD0, v);
        let v = ee::mul(f20, l2::sin_deg(fx, tab, s3, 0));
        fx.mem.setf(sp + 0xD8, v);
        if l2::project(fx, sp + 0xC0, SPR, sp + 0x120, d, sp + 0x160) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
            let ot = fx.mem.read_u32(D + 0x80);
            fx.submit(d, ot, key);
        }
        s1 = (s1 + 0x1E) as i16 as i32;
    }
    fx.leave(0x200);
}

/// The byte at `at` + `step` while below `cap`.
fn rise(fx: &mut Fx, at: u32, cap: i32, step: i32) {
    let a = fx.mem.u8(at);
    if a < cap {
        fx.mem.set8(at, a + step);
    }
}

/// The byte at `at` - `step` while at least `floor`; else the task ends
/// (+0x58 = 0).
fn sink(fx: &mut Fx, t: u32, at: u32, floor: i32, step: i32) {
    let a = fx.mem.u8(at);
    if a >= floor {
        fx.mem.set8(at, a - step);
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
}

/// The byte at `at` - `step` while at least `floor` (the task kept).
fn sink_only(fx: &mut Fx, at: u32, floor: i32, step: i32) {
    let a = fx.mem.u8(at);
    if a >= floor {
        fx.mem.set8(at, a - step);
    }
}

/// rand % n (the div's remainder).
fn rand_mod(fx: &mut Fx, n: i32) -> i32 {
    let r = fx.rand();
    fx.div(r, n).1
}

/// (rand >> 4) % n.
fn rand4_mod(fx: &mut Fx, n: i32) -> i32 {
    let r = fx.rand();
    fx.div(r >> 4, n).1
}

/// A random spin step: (rand & 0x7FF) - 0x400.
fn rand_spin(fx: &mut Fx) -> i32 {
    (fx.rand() % 0x800) - 0x400
}

/// The three random drifts (rand % 40 - 20) at +0x30 / +0x34 / +0x38.
fn drift40(fx: &mut Fx, t: u32) {
    for o in [0x30u32, 0x34, 0x38] {
        let m = rand_mod(fx, 0x28);
        fx.mem.setf(t + o, ee::from_i32(m - 0x14));
    }
}

/// The three random drifts (rand % 100 - 50) / 60 at +0x34 / +0x30 /
/// +0x38, and the spin (+0x5C, angles +0x20, steps +4 / +2 / +0, the
/// rectangle order +0x52).
fn drift_spin(fx: &mut Fx, t: u32) {
    for o in [0x34u32, 0x30, 0x38] {
        let m = rand_mod(fx, 0x64);
        fx.mem.setf(t + o, ee::from_i32(m - 0x32));
    }
    for o in [0x34u32, 0x30, 0x38] {
        let v = fx.mem.f(t + o);
        fx.mem.setf(t + o, ee::div(v, 60.0));
    }
    let m = rand_mod(fx, 0x168);
    fx.mem.set16(t + 0x5C, m);
    l2::camera_angles_to(fx, t + 0x20);
    for o in [4u32, 2, 0] {
        let v = rand_spin(fx);
        fx.mem.set16(t + o, v);
    }
    let r = fx.rand();
    fx.mem.set16(t + 0x52, (r >> 4) % 4);
    fx.mem.set16(t + 6, 0xA);
}

/// The position drifting by +0x30.. and falling 1.5.
fn fall(fx: &mut Fx, t: u32) {
    for k in 0..3 {
        let d = fx.mem.f(t + 0x30 + 4 * k);
        addf(fx, t + 0x10 + 4 * k, d);
    }
    subf(fx, t + 0x14, 1.5);
}

fn bump(fx: &mut Fx, t: u32) {
    let n = fx.mem.i16(t + 0x54) + 1;
    fx.mem.set16(t + 0x54, n);
}

/// The common random set-up of 0x1E02340 / 0x1E02F40 [F].
fn sprite_setup(fx: &mut Fx, t: u32, spray: bool) {
    let m = rand_mod(fx, 0x168);
    fx.mem.set16(t + 0x5A, m);
    let m = rand_mod(fx, 0x168);
    fx.mem.set16(t + 0x5C, m);
    let r = fx.rand();
    fx.mem.set16(t + 0xA, if r % 2 != 0 { 2 } else { 0 });
    let v = fx.mem.i16(t + 0x50);
    fx.mem.set16(t + 0xC, v);
    let m = if spray { rand4_mod(fx, 0x64) + 0x46 } else { rand_mod(fx, 0x15E) + 0x32 };
    fx.mem.set16(t + 0x50, m);
    let m = rand_mod(fx, 0x15E);
    fx.mem.set16(t + 0x52, m + 0x64);
    let m = if spray { rand_mod(fx, 0xA) + 5 } else { rand_mod(fx, 0x14) + 0xA };
    fx.mem.set16(t + 0xE, m);
    let m = if spray { rand4_mod(fx, 0x23) } else { rand_mod(fx, 0x23) };
    fx.mem.set16(t + 8, m);
    let m = rand_mod(fx, 5);
    fx.mem.set16(t + 6, m + 3);
    drift40(fx, t);
    fx.mem.set16(t + 0x58, 2);
}

/// 0x1E02340: a rising sprite ring (the Lord's aura).
fn step_ring(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x43F0_0000);
        let r = fx.rand();
        fx.mem.set16(t + 0x56, (r >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        sprite_setup(fx, t, false);
    }
    if fx.mem.i16(t + 0x54) == 0x21 {
        l2::rgb(fx, t + 0x60, 0x30, 0x30, 0x30);
    }
    if fx.mem.i16(D + 0x6C3A) < 0x1F4 && fx.mem.i16(t + 0x54) < 0x35 {
        rise(fx, t + 0x63, 0x80, 0x80);
    } else {
        sink(fx, t, t + 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        addf(fx, t + 0x14, 6.0);
    }
    let v = ee::add(fx.mem.f(D + 0x5A98), fx.mem.f(t + 0x38));
    fx.mem.setf(t + 0x18, v);
    bump(fx, t);
    fx.leave(0x30);
}

/// 0x1E02640: a fan widening from the Lord's chest.
fn step_fan(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        let k = fx.mem.i16(t + 0x50);
        let r: Option<f32> = match k {
            0 => Some(10.0),
            1 => Some(15.0),
            2 => Some(20.0),
            3 => Some(25.0),
            4 => Some(30.0),
            _ => None,
        };
        if let Some(r) = r {
            fx.mem.setf(t + 0x48, r);
            fx.mem.setf(t + 0x4C, ee::sub(r, 5.0));
        }
        let m = rand_mod(fx, 7);
        fx.mem.set16(t + 0x56, m);
        fx.mem.set16(t + 0x58, 2);
        let m = rand_mod(fx, 3);
        fx.mem.set16(t + 8, m + 4);
        let life = match fx.mem.i16(t + 0x50) {
            0 => Some(0x46),
            1 => Some(0x3C),
            2 => Some(0x32),
            3 => Some(0x28),
            4 => Some(0x1E),
            _ => None,
        };
        if let Some(n) = life {
            fx.mem.set16(t + 6, n);
        }
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        rise(fx, t + 0x63, 0x80, 9);
        rise(fx, t + 0x67, 0x80, 9);
    } else {
        sink(fx, t, t + 0x63, 7, 5);
        sink_only(fx, t + 0x67, 7, 5);
        let w = ee::add(fx.mem.f(t + 0x48), 1.5);
        fx.mem.setf(t + 0x48, w);
        fx.mem.setf(t + 0x4C, ee::sub(w, 6.0));
    }
    let c = if fx.mem.i16(t + 0x54) & 1 == 1 { 0x50 } else { 0x60 };
    l2::rgb(fx, t + 0x60, c, c, c);
    l2::rgb(fx, t + 0x64, c, c, c);
    if fx.mem.i16(t + 0x54) < 0xA {
        let v = if fx.mem.i16(D + 0x6C36) != 0 { 0 } else { 0x8000 };
        fx.mem.set32(t + 0x24, v);
    } else {
        let a = fx.mem.i32(t + 0x24);
        let d = if fx.mem.i16(t + 0x50) & 1 == 1 { 0x80 } else { -0x80 };
        fx.mem.set32(t + 0x24, a.wrapping_add(d));
    }
    bump(fx, t);
    fx.leave(0x30);
}

/// 0x1E029C0: a burst sprite flying off and growing.
fn step_burst(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0xC8);
        fx.mem.set16(t + 0xA, 0xC8);
        let m = rand4_mod(fx, 3);
        fx.mem.set16(t + 0x56, m);
        if fx.mem.i16(D + 0x6C3A) & 1 == 1 {
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0x40);
        } else {
            l2::rgb(fx, t + 0x60, 0xFF, 0x80, 0x40);
        }
        fx.mem.set16(t + 0x58, 2);
        drift_spin(fx, t);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        rise(fx, t + 0x63, 0x80, 0x40);
    } else {
        sink(fx, t, t + 0x63, 0xB, 9);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        let s = fx.mem.i16(t + 8);
        if s < 0x12C {
            fx.mem.set16(t + 8, s + 5);
        }
        let s = fx.mem.i16(t + 8);
        fx.mem.set16(t + 0xA, s);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        fall(fx, t);
    }
    for (a, s) in [(0x28u32, 4u32), (0x24, 2), (0x20, 0)] {
        let v = fx.mem.i32(t + a).wrapping_add(fx.mem.i16(t + s));
        fx.mem.set32(t + a, v);
    }
    l2::camera_angles_to(fx, t + 0x20);
    bump(fx, t);
    fx.leave(0x30);
}

/// 0x1E02D00: the white halo (gouraud ring of radius 300) spinning.
fn step_halo(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4396_0000);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        l2::rgb(fx, t + 0x64, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 6, 0x109);
        fx.mem.set16(t + 0x5A, 0xA);
        fx.mem.set16(t + 0x5C, 0xA);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        rise(fx, t + 0x63, 0xA0, 0xD);
    } else {
        sink(fx, t, t + 0x63, 0xE, 0xC);
    }
    l2::camera_angles_to(fx, t + 0x20);
    let v = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
    fx.mem.set32(t + 0x20, v);
    bump(fx, t);
    fx.leave(0x20);
}

/// 0x1E02E00: the glint sprite on the Lord's chest, flickering.
fn step_glint(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x12C);
        fx.mem.set16(t + 0xA, 0x12C);
        let r = fx.rand();
        fx.mem.set16(t + 0x56, r % 2);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
        l2::rgb(fx, t + 0x64, 0x80, 0x80, 0x80);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
    }
    if fx.mem.i16(t + 0x54) < 0x109 {
        rise(fx, t + 0x63, 0xC0, 0x13);
        rise(fx, t + 0x67, 0xC0, 0x13);
    } else {
        sink(fx, t, t + 0x63, 0x10, 0xE);
        // [F] 0x1E02EDC reads +0x63 again (the alpha falls twice).
        sink_only(fx, t + 0x63, 0x10, 0xE);
    }
    let v = i32::from(fx.mem.i16(t + 0x56) == 0);
    fx.mem.set16(t + 0x56, v);
    l2::camera_angles_to(fx, t + 0x20);
    bump(fx, t);
    fx.leave(0x20);
}

/// 0x1E02F40: a spray sprite ring at the chest.
fn step_spray(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x4248_0000);
        let r = fx.rand();
        fx.mem.set16(t + 0x56, (r >> 4) % 8);
        l2::rgb(fx, t + 0x60, 0x60, 0x60, 0x60);
        sprite_setup(fx, t, true);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        rise(fx, t + 0x63, 0xC0, 0xC0);
    } else {
        sink(fx, t, t + 0x63, 0xD, 0xB);
    }
    let a = fx.mem.i16(t + 0x5A) + 3;
    fx.mem.set16(t + 0x5A, a);
    if fx.mem.i16(t + 0x54) != 0 {
        let r = fx.mem.i16(t + 0x50) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x50, r);
    }
    bump(fx, t);
    fx.leave(0x20);
}

/// 0x1E031E0: a gouraud wave ring growing by 30.
fn step_wave(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set32(t + 0x48, 0);
        if fx.mem.i16(t + 0x50) != 2 {
            l2::rgb(fx, t + 0x60, 0x10, 0x10, 0x80);
            l2::rgb(fx, t + 0x64, 0x30, 0x30, 0x30);
        } else {
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x10);
            l2::rgb(fx, t + 0x64, 0x10, 0x10, 8);
        }
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xE, k);
        let k = fx.mem.i16(t + 0x50);
        if k == 0 || k == 2 {
            for o in [0x20u32, 0x24, 0x28] {
                let m = rand_mod(fx, 0xFFFF);
                fx.mem.set32(t + o, m);
            }
        } else {
            fx.mem.set32(t + 0x20, 0x4000);
            fx.mem.set32(t + 0x24, 0);
            fx.mem.set32(t + 0x28, 0);
        }
        fx.mem.set16(t + 0x5A, 0x1E);
        fx.mem.set16(t + 0x5C, 0x1E);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 6, 0);
        let n = if fx.mem.i16(t + 0x50) == 2 { 0x1E } else { 0x12 };
        fx.mem.set16(t + 8, n);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 8) {
        rise(fx, t + 0x67, 0x80, 0xC);
    } else if fx.mem.i16(t + 0x50) != 2 {
        sink(fx, t, t + 0x67, 0xB, 9);
    } else {
        sink(fx, t, t + 0x67, 9, 7);
    }
    if fx.mem.i16(t + 0xE) != 2 {
        let r = fx.mem.f(t + 0x48);
        fx.mem.setf(t + 0x4C, ee::sub(r, ee::mul(0.5, r)));
    }
    if fx.mem.i16(t + 6) < 0 {
        fx.mem.set32(t + 0x4C, 0);
    }
    if fx.mem.i16(t + 0x54) != 0 {
        addf(fx, t + 0x48, 30.0);
    }
    bump(fx, t);
    fx.leave(0x20);
}

/// 0x1E03430: a white star of sparks growing by 55.
fn step_star(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.write_u32(t + 0x48, 0x41A0_0000);
        fx.mem.set32(t + 0x4C, 0);
        l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
        fx.mem.set16(t + 0x50, 0);
        fx.mem.set16(t + 0x5A, 0x3C);
        fx.mem.set16(t + 0x5C, 3);
        fx.mem.set16(t + 0x56, 4);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for o in [0x20u32, 0x24, 0x28] {
            let m = rand_mod(fx, 0xFFFF);
            fx.mem.set32(t + o, m);
        }
    }
    if fx.mem.i16(t + 0x54) < 7 {
        rise(fx, t + 0x63, 0xC0, 0x40);
    } else {
        sink(fx, t, t + 0x63, 0xA, 8);
    }
    if fx.mem.i16(t + 0x54) & 1 == 1 {
        l2::rgb(fx, t + 0x64, 0xA0, 0xA0, 0x60);
    } else {
        l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0x60);
    }
    let r = ee::add(fx.mem.f(t + 0x48), 55.0);
    fx.mem.setf(t + 0x48, r);
    fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3E4C_CCCD), r));
    bump(fx, t);
    fx.leave(0x20);
}

/// 0x1E035E0: a ribbon head circling the Lord (radius +0x4C shrinking by
/// 2.6, angle +0x5A by +-20 / +-10), rising to the chest, trailing 13
/// points (D+0x5AB0 + 0xD0 index) and dropping spray / bursts.
fn step_orbit(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x40);
    let tab = D + 0x6E3C;
    let pts = |fx: &mut Fx| D.wrapping_add((fx.mem.i16(t + 0x50) * 0xD0) as u32) + 0x5AB0;
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
        fx.mem.write_u32(t + 0x4C, 0x42C8_0000);
        fx.mem.write_u32(t + 0x14, 0x41F0_0000);
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, tab, a, 0);
        let v = ee::mul(fx.mem.f(t + 0x4C), s);
        fx.mem.setf(t + 0x18, v);
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, tab, a, 1);
        let v = ee::mul(fx.mem.f(t + 0x4C), c);
        fx.mem.setf(t + 0x10, v);
        let base = pts(fx);
        for k in 0..13u32 {
            copy12(fx, base + 0x10 * k, t + 0x10);
        }
    }
    if fx.mem.i16(t + 0x54) < 0x12C {
        rise(fx, t + 0x63, 0xC0, 0x13);
        rise(fx, t + 0x67, 0xC0, 0x13);
    } else {
        sink(fx, t, t + 0x63, 0x10, 0xE);
        sink_only(fx, t + 0x63, 0x10, 0xE);
    }
    let v = i32::from(fx.mem.i16(t + 0x56) == 0);
    fx.mem.set16(t + 0x56, v);
    if fx.mem.i16(t + 0x54) != 0 {
        if fx.mem.i16(D + 0x6C3A) >= 0x272 {
            let y = fx.mem.f(t + 0x14);
            if !ee_le(fx.mem.f(D + 0x5A84), y) {
                fx.mem.setf(t + 0x14, ee::add(y, 5.0));
            }
        } else {
            addf(fx, t + 0x14, 6.0);
        }
        let a = fx.mem.i16(t + 0x5A);
        let s = l2::sin_deg(fx, tab, a, 0);
        let v = ee::mul(fx.mem.f(t + 0x4C), s);
        fx.mem.setf(t + 0x18, v);
        let a = fx.mem.i16(t + 0x5A);
        let c = l2::sin_deg(fx, tab, a, 1);
        let v = ee::mul(fx.mem.f(t + 0x4C), c);
        fx.mem.setf(t + 0x10, v);
        let r = fx.mem.f(t + 0x4C);
        if !ee_le(r, 0.0) {
            fx.mem.setf(t + 0x4C, ee::sub(r, f32::from_bits(0x4026_6666)));
        } else {
            fx.mem.set16(t + 0x54, 0x190);
        }
        let k = fx.mem.i16(t + 0x50);
        let step = if k < 8 { 0x14 } else { 0xA };
        let a = fx.mem.i16(t + 0x5A);
        fx.mem.set16(t + 0x5A, if k & 1 != 0 { a + step } else { a - step });
    }
    fx.object_position(LORD, sp + 0x30);
    let lz = fx.mem.f(sp + 0x38);
    let z = fx.mem.f(t + 0x18);
    if fx.mem.i16(D + 0x6C36) != 0 {
        fx.mem.setf(t + 0x18, ee::add(z, ee::sub(lz, 85.0)));
    } else {
        fx.mem.setf(t + 0x18, ee::add(z, ee::add(85.0, lz)));
    }
    let lx = fx.mem.f(sp + 0x30);
    addf(fx, t + 0x10, lx);
    if fx.mem.i16(t + 0x54) < 0x12C {
        copy16(fx, D + 0x250, t + 0x10);
        let n = fx.mem.i16(t + 0x54);
        if n & 1 == 1 {
            if fx.mem.i16(t + 0x50) & 1 == 1 && n >= 6 {
                spawn(fx, STEP_SPRAY);
            }
            if fx.mem.i16(t + 0x50) & 1 == 0 && fx.mem.i16(t + 0x54) >= 6 {
                spawn(fx, STEP_BURST);
            }
        }
    }
    let c = if fx.mem.i16(t + 0x54) & 1 == 0 { 0xC0 } else { 0xFF };
    l2::rgb(fx, t + 0x60, c, c, c);
    let base = pts(fx);
    shift_points(fx, base + 0xC0, 0xD, t + 0x10);
    l2::camera_angles_to(fx, t + 0x20);
    if fx.mem.i16(t + 0x54) != 0 {
        let n = fx.mem.i16(t + 6);
        if n < 0xC {
            fx.mem.set16(t + 6, n + 1);
        }
    }
    bump(fx, t);
    fx.leave(0x40);
}

/// 0x1E03B30: a gouraud shock ring (index 0: radius 800, 1: 1000) facing
/// the camera, growing by 70.
fn step_shock(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set32(t + 0x48, 0);
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x80);
            l2::rgb(fx, t + 0x64, 0x20, 0x20, 0x20);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xC0, 0xC0, 0xC0);
            l2::rgb(fx, t + 0x64, 0x20, 0x20, 0x20);
        }
        let k = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0xE, k);
        for (k, r) in [(0, 0x4448_0000u32), (1, 0x447A_0000)] {
            if fx.mem.i16(t + 0x50) == k {
                l2::camera_angles_to(fx, t + 0x20);
                let v = fx.mem.i32(t + 0x20).wrapping_add(0x4000);
                fx.mem.set32(t + 0x20, v);
                fx.mem.write_u32(t + 0x48, r);
            }
        }
        fx.mem.set16(t + 0x5A, 0x14);
        fx.mem.set16(t + 0x5C, 0x14);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x63, 0);
        fx.mem.set16(t + 6, 0);
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.set16(t + 8, 8);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.set16(t + 8, 8);
        }
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 8) {
        if fx.mem.i16(t + 0x50) == 0 {
            rise(fx, t + 0x63, 0x80, 0x2A);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            rise(fx, t + 0x63, 0xC0, 0x40);
        }
    } else {
        if fx.mem.i16(t + 0x50) == 0 {
            sink(fx, t, t + 0x63, 0xB, 9);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            sink(fx, t, t + 0x63, 0x11, 0xF);
        }
    }
    for k in [0, 1] {
        if fx.mem.i16(t + 0x50) == k && fx.mem.i16(t + 0x54) != 0 {
            addf(fx, t + 0x48, 70.0);
        }
    }
    bump(fx, t);
    fx.leave(0x20);
}

/// 0x1E03DB0: the 12-segment ring (radius D+0x5A64) turning upward by a
/// random 0..6 degrees each update until 270.
fn step_ring12(fx: &mut Fx, t: u32) {
    fx.enter(0x30);
    if fx.mem.i16(t + 0x58) == 1 {
        for o in [0x20u32, 0x24, 0x28] {
            let m = rand_mod(fx, 0xFFFF);
            fx.mem.set32(t + o, m);
        }
        fx.mem.set16(t + 0x56, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x80, 0x40);
        fx.mem.set16(t + 0x50, 0x5A);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x63, 0);
        let r = fx.mem.read_u32(D + 0x5A64);
        fx.mem.write_u32(t + 0x48, r);
    }
    if fx.mem.i16(t + 0x54) < 8 {
        rise(fx, t + 0x63, 0x80, 0x80);
    } else {
        sink(fx, t, t + 0x63, 0xB, 9);
    }
    bump(fx, t);
    if fx.mem.i16(t + 0x50) < 0x10E {
        let r = fx.rand();
        let (_, m) = fx.div(r, 7);
        let a = fx.mem.i16(t + 0x50);
        fx.mem.set16(t + 0x50, a + (m as i16 as i32));
    } else {
        fx.mem.set16(t + 0x58, 0);
    }
    fx.leave(0x30);
}

/// 0x1E03F10: the gathering sparks (index 0: a spinning ring of radius
/// 20 growing by 80; 1: one of 3000 shrinking by 250).
fn step_sparks(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            fx.mem.write_u32(t + 0x48, 0x41A0_0000);
            fx.mem.set32(t + 0x4C, 0);
            l2::rgb(fx, t + 0x60, 0x80, 0x80, 0xFF);
            fx.mem.set16(t + 0x5A, 0x3C);
            fx.mem.set16(t + 0x5C, 3);
            for o in [4u32, 2, 0] {
                let v = rand_spin(fx);
                fx.mem.set16(t + o, v);
            }
            fx.mem.set16(t + 6, 0xF);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            fx.mem.write_u32(t + 0x48, 0x453B_8000);
            fx.mem.set32(t + 0x4C, 0);
            l2::rgb(fx, t + 0x60, 0xFF, 0xFF, 0xFF);
            fx.mem.set16(t + 0x5A, 0x3C);
            fx.mem.set16(t + 0x5C, 2);
            fx.mem.set16(t + 0x56, 3);
            fx.mem.set16(t + 6, 7);
        }
        fx.mem.set16(t + 0x56, 4);
        fx.mem.set16(t + 0x58, 2);
        fx.mem.write_u8(t + 0x67, 0);
        for o in [0x20u32, 0x24, 0x28] {
            let m = rand_mod(fx, 0xFFFF);
            fx.mem.set32(t + o, m);
        }
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        rise(fx, t + 0x63, 0xC0, 0x40);
    } else {
        sink(fx, t, t + 0x63, 0xA, 8);
    }
    if fx.mem.i16(t + 0x50) == 0 {
        if fx.mem.i16(t + 0x54) & 1 == 1 {
            l2::rgb(fx, t + 0x64, 0xA0, 0xA0, 0x60);
        } else {
            l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0x60);
        }
        let r = ee::add(fx.mem.f(t + 0x48), 80.0);
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, ee::mul(f32::from_bits(0x3E4C_CCCD), r));
        for (a, s) in [(0x20u32, 0u32), (0x24, 2), (0x28, 4)] {
            let v = fx.mem.i32(t + a).wrapping_add(fx.mem.i16(t + s));
            fx.mem.set32(t + a, v);
        }
    }
    if fx.mem.i16(t + 0x50) == 1 {
        if fx.mem.i16(t + 0x54) & 1 == 1 {
            l2::rgb(fx, t + 0x64, 0x60, 0x60, 0xA0);
        } else {
            l2::rgb(fx, t + 0x64, 0x60, 0x60, 0xC0);
        }
        let r = ee::sub(fx.mem.f(t + 0x48), 250.0);
        fx.mem.setf(t + 0x48, r);
        fx.mem.setf(t + 0x4C, ee::mul(0.5, r));
    }
    bump(fx, t);
    fx.leave(0x20);
}

/// 0x1E04250: debris sprites thrown off the Lord.
fn step_debris(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 8, 0x190);
        let s = fx.mem.i16(t + 8) << 2;
        fx.mem.set16(t + 0xA, s);
        let m = rand4_mod(fx, 3);
        fx.mem.set16(t + 0x56, m);
        l2::rgb(fx, t + 0x60, 0x40, 0x40, 0x40);
        fx.mem.set16(t + 0x58, 2);
        drift_spin(fx, t);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        rise(fx, t + 0x63, 0x80, 0x40);
    } else {
        sink(fx, t, t + 0x63, 0xB, 9);
    }
    let v = fx.mem.i16(t + 0x56);
    fx.mem.set16(t + 0x56, if v < 7 { v + 1 } else { 0 });
    if fx.mem.i16(t + 0x54) != 0 {
        fall(fx, t);
    }
    let v = fx.mem.i32(t + 0x24).wrapping_add(fx.mem.i16(t + 2));
    fx.mem.set32(t + 0x24, v);
    l2::camera_angles_to(fx, t + 0x20);
    bump(fx, t);
    fx.leave(0x20);
}

/// 0x1E04510: a window wash (index 0 white, 1 gold, 2 pale violet).
fn step_wash(fx: &mut Fx, t: u32) {
    fx.enter(0x20);
    if fx.mem.i16(t + 0x58) == 1 {
        if fx.mem.i16(t + 0x50) == 0 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xF0, 0xF0);
            l2::rgb(fx, t + 0x64, 0xF0, 0xF0, 0xF0);
            fx.mem.set16(t + 6, 0x14);
        }
        if fx.mem.i16(t + 0x50) == 1 {
            l2::rgb(fx, t + 0x60, 0xD0, 0xD0, 0x80);
            l2::rgb(fx, t + 0x64, 0xC0, 0xC0, 0x80);
            fx.mem.set16(t + 6, 0x14);
        }
        if fx.mem.i16(t + 0x50) == 2 {
            l2::rgb(fx, t + 0x60, 0xF0, 0xE0, 0xFF);
            l2::rgb(fx, t + 0x64, 0xF0, 0xE0, 0xFF);
            fx.mem.set16(t + 6, 0x1E);
        }
        fx.mem.set16(t + 0x58, 2);
    }
    if fx.mem.i16(t + 0x54) < fx.mem.i16(t + 6) {
        if fx.mem.i16(t + 0x50) != 2 {
            rise(fx, t + 0x63, 0x80, 7);
            rise(fx, t + 0x67, 0x80, 7);
        } else {
            rise(fx, t + 0x63, 0xC0, 0xC);
            rise(fx, t + 0x67, 0xC0, 0xC);
        }
    } else {
        sink(fx, t, t + 0x63, 6, 4);
        let a = fx.mem.u8(t + 0x67);
        fx.mem.set8(t + 0x67, if a >= 6 { a - 4 } else { 0 });
    }
    bump(fx, t);
    fx.leave(0x20);
}

/// 0x1E046E0: the stage light (0x1F7F10) dimmed to 0x30 by 10 a frame,
/// back to 0x80 when the countdown is below 31.
fn step_light(fx: &mut Fx, t: u32) {
    fx.enter(0x10);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set16(t + 0x50, 0x80);
        fx.mem.set16(t + 0x58, 2);
    }
    let v = fx.mem.i16(t + 0x50);
    if fx.mem.i32(D + 0x6B30) >= 0x1F {
        fx.mem.set16(t + 0x50, if v < 0x31 { 0x30 } else { v - 0xA });
    } else {
        fx.mem.set16(t + 0x50, if v < 0x80 { v + 0xA } else { 0x80 });
        if fx.mem.i16(t + 0x50) >= 0x81 {
            fx.mem.set16(t + 0x50, 0x80);
        }
    }
    bump(fx, t);
    let v = fx.mem.u8(t + 0x50);
    fx.set_stage_light(v);
    fx.leave(0x10);
}

/// 0x1E047A0: the frame-buffer tint (a window sprite over the frame drawn,
/// 0x207EF0, ALPHA 0x44), its colour stepping with the timeline.
fn step_tint(fx: &mut Fx, t: u32) {
    let sp = fx.enter(0x90);
    if fx.mem.i16(t + 0x58) == 1 {
        fx.mem.set32(t + 0x48, 0);
        l2::rgb(fx, t + 0x60, 0x80, 0x78, 0x78);
        fx.mem.set16(t + 0x58, 2);
    }
    for (f, c) in [(0x1F8, [0xB8, 0x80, 0x80]), (0x271, [0x90, 0x90, 0x90]), (0x27B, [0x80, 0x80, 0x80]), (0x2CB, [0xB8, 0x80, 0xC8])] {
        if fx.mem.i16(D + 0x6C3A) == f {
            l2::rgb(fx, t + 0x60, c[0], c[1], c[2]);
        }
    }
    if fx.mem.i16(D + 0x6C3A) == 0x2DF {
        fx.mem.write_u32(t + 0x48, 0x4040_0000);
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
    fx.mem.setf(sp + 0x50, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x54, ee::div(s, 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x60, ee::div(ee::sub(512.0, s), 512.0));
    let s = fx.mem.f(t + 0x48);
    fx.mem.setf(sp + 0x64, ee::div(ee::sub(512.0, s), 512.0));
    // [F] The three ALPHA branches (frames < 0x271, < 0x27C, after) are
    // the same 0x44.
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, crate::fxlib::ALPHA_44);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.submit(d, OT, 0);
    if fx.mem.i32(D + 0x6B30) >= 0x15 {
        rise(fx, t + 0x63, 0x48, 0xB);
    } else {
        sink(fx, t, t + 0x63, 0xB, 9);
    }
    bump(fx, t);
    fx.leave(0x90);
}

impl EffectPort for C063 {
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
