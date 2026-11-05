//! BMODULE b000 (card 0), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E056B0, data area 0x1ED0000) [F] (helper C15, 31
//! Oct 2026). The attacker's nodes 0x7D / 0x81 / 0x78 / 0x7A (+0x1900 ..
//! +0x190C) and 0x79 (+0x18FC): a point (+0x440) on node 0x7D while below
//! update 0xA0, then slid 1 / 0x18, 1 / 0x17 .. of the way to the target
//! (0x104490) feeding a ribbon (`crate::ribbon`, +0x70 / +0xA0, drawn
//! from 0xA0, its points' alpha faded by 5 an update) with a scale (+0x420) rising 0.2 to 5,
//! then 0.5 to 20 (its width +0x7C falling 8 to 0); the scale 0.8 / 1.0 on
//! even / odd updates 0x64..0x9F. Eight jagged strands (`strand::draw`,
//! +0x2A0 + 0x30 k) from the origin of +0x420 to random directions of
//! length 8 (+0x1850 + 0x10 k, 0x1043F0; renewed when the strand's piece
//! counter +0x28, started at -(k + 1), is 0); a disc (+0xDE0, `Fx::disc`)
//! and a quad cloud (+0xE60, `quads_draw`) in the spaces +0xD70 / +0xDF0
//! (scales 1.5 on 0x29, 0.7 on 0x49 then down 0.02 to 0.7 on 0x2A..0x48,
//! up 0.1 to 2 from 0xA1; +0xDF0 up 0.05 to 1 below 0x29; the disc's alpha
//! up 8 to 0xFF below 0x29, 0xFF / 0x80 on even / odd 0x2A..0x48, 0x80
//! then down 4 from 0xB0; the cloud's alpha down 2 from 0x49); from 0x43 a
//! facing quad (`facing::quad`, +0x490 / +0x4F0), rain (`rain::draw`,
//! +0x12C0, 64 drops at 10, drawn at 1.5), 128 sprites (`Fx::sprites`,
//! +0x510 / +0x570: thrown 8 in random directions, slowed x 0.9, rethrown
//! at age 8; half size 0.5, 10 from 0xB6); every update a frame flash
//! (`flash::frame_flash`, +0x18E0: 0x80, 0x60, 0x10) whose alpha is 0x40
//! + 64 sin of an angle advancing 0x800 from 0xC000 below 0x90, up 4 to
//! 0x80 to 0x11D, down 4 after; from 0xB6 a full-window tint (`b262::tint`,
//! +0x18E4, alpha up 8 to 0xFF to 0xD4, down 8 after); from 0xD5 the
//! unit's zoom afterimage (`zoom::sprites`, +0x1840, 10 layers, alpha up 4
//! to 0x40). Returns 1 on update 0xAC, 6 on 0xB6..0x11D, 2 on 0x11E, 8
//! after 0x12E.
//!
//! Library (record addresses): `rain::colours2` 0x1E00080, `colours4`
//! 0x1E00100, `flash::frame_flash` 0x1E001A0, `b262::tint` 0x1E003F0,
//! `zoom::sprites` 0x1E00550, `Fx::disc` 0x1E00C20, `Fx::sprites`
//! 0x1E01180, `facing::quad` 0x1E02A70, `quads_setup` / `quads_draw`
//! 0x1E03300 / 0x1E03460, the ribbon push 0x1E03A70 and its register twin
//! draw 0x1E03BC0 (0511bfa0e724: (ot, g, pts, x)) [`crate::ribbon`],
//! `strand::draw`'s register twin 0x1E04580 (fcab33648d6e), `rain::setup`
//! / `rain::draw` 0x1E04F00 / 0x1E05150.
use crate::b107::approach_down;
use crate::b436::fstep_down;
use crate::blib::{facing, flash, rain, strand, zoom};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const N: u32 = D + 0x18F0;
const LEFT: u32 = D + 0x18EC;
const SLIDE: u32 = D + 0x18E8;
const ANGLE: u32 = D + 0x18F8;
const FLASH: u32 = D + 0x18E0;
const TINT: u32 = D + 0x18E4;
const ZOOM: u32 = D + 0x1840;
const RIBBON: u32 = D + 0x70;
const RIB_PTS: u32 = D + 0xA0;
const POINT: u32 = D + 0x440;
const SPACE: u32 = D + 0x420;
const STRANDS: u32 = D + 0x2A0;
const DIRS: u32 = D + 0x1850;
const DISC: u32 = D + 0xDE0;
const CLOUD: u32 = D + 0xE60;
const QUAD: u32 = D + 0x490;
const LOOK: u32 = D + 0x4F0;
const RAIN: u32 = D + 0x12C0;
const SPRITES: u32 = D + 0x510;
const PARTS: u32 = D + 0x570;
/// The quad cloud's corner template (0x1E03300's `lui` / `addiu`).
const QUAD_TEMPLATE: u32 = 0x01E0_6480;

/// The entry 0x1E056B0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xA0);
    ret
}

fn halves(fx: &mut Fx, at: u32, v: &[(u32, i32)]) {
    for &(k, x) in v {
        fx.mem.set16(at + k, x);
    }
}

fn bytes(fx: &mut Fx, at: u32, v: &[(u32, u8)]) {
    for &(k, x) in v {
        fx.mem.write_u8(at + k, x);
    }
}

/// 0x1043F0(out, 8.0) from the entry: its prologue (0x80: the entry's s0 =
/// D at +0, `ra` +0x10) left below the stack pointer.
fn random_dir8(fx: &mut Fx, sp: u32, out: u32, ra: u32) {
    fx.random_dir(out, 8.0);
    fx.saved(sp - 0x80, &[(0, u64::from(D)), (0x10, u64::from(ra))]);
}

fn copy3(fx: &mut Fx, from: u32, to: u32) {
    for c in [0u32, 4, 8] {
        let v = fx.mem.read_u32(from + c);
        fx.mem.write_u32(to + c, v);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1910);
    for x in [0u32, 0x420, 0xDF0, 0xD70, 0x17D0, 0x16D0, 0x1750] {
        fx.xform_init(D + x);
    }
    for (at, node, clear) in [(0x18FCu32, 0x79, 1), (0x1900, 0x7D, 0), (0x1904, 0x81, 0), (0x1908, 0x78, 0), (0x190C, 0x7A, 0)] {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set32(LEFT, 0x12E);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x18F4, param);
    fx.mem.set32(SLIDE, 0x18);
    bytes(fx, FLASH, &[(2, 0x10), (1, 0x60), (0, 0x80), (3, 0)]);
    fx.mem.write_u32(ANGLE, 0xC000);
    bytes(fx, TINT, &[(2, 0x80), (1, 0x80), (0, 0x80), (3, 0)]);
    // The zoom afterimage (+0x1840): flags 0x80400, grey 0x80, scale 20, 10
    // layers of the target.
    fx.mem.write_u32(ZOOM, 0x0008_0400);
    bytes(fx, ZOOM, &[(6, 0x80), (5, 0x80), (4, 0x80), (7, 0)]);
    fx.mem.write_u32(ZOOM + 8, 0x41A0_0000);
    fx.mem.set16(ZOOM + 0xC, 0xA);
    let t = fx.target_handle();
    fx.mem.set16(ZOOM + 0xE, t);
    // Three ring / disc records (+0x1740, +0x17C0, +0xDE0).
    fx.mem.write_u32(D + 0x1740, 0x0008_0000);
    bytes(fx, D + 0x1740, &[(6, 0x60), (5, 0x60), (4, 0xA0), (7, 0xFF)]);
    fx.mem.write_u32(D + 0x1748, 0x3F80_0000);
    halves(fx, D + 0x1740, &[(0xC, 0x24), (0xE, 0)]);
    fx.mem.write_u32(D + 0x17C0, 0x0008_0000);
    bytes(fx, D + 0x17C0, &[(6, 0x70), (5, 0xA0), (4, 0xD0), (7, 0x80)]);
    fx.mem.write_u32(D + 0x17C8, 0x3F80_0000);
    halves(fx, D + 0x17C0, &[(0xC, 0x24), (0xE, 0)]);
    fx.mem.write_u32(DISC, 0x0008_0000);
    bytes(fx, DISC, &[(6, 0x70), (5, 0xA0), (4, 0xD0), (7, 0)]);
    fx.mem.write_u32(DISC + 8, 0x40A0_0000);
    halves(fx, DISC, &[(0xC, 0x20), (0xE, 0)]);
    // The rain (+0x12C0): flags 0x30080001, (0xE0, 0x80, 0x80, 0x80), 64
    // drops at speed 10.
    fx.mem.write_u32(RAIN, 0x3008_0001);
    bytes(fx, RAIN, &[(6, 0x80), (5, 0x80), (4, 0xE0), (7, 0x80)]);
    rain::setup(fx, 0x01E0_4F00, RAIN, 0x40, 10.0);
    // Eight strands (+0x2A0 + 0x30 k) of the attacker's texture 0: far Z,
    // every piece, grey 0x80 alpha 0x40, 16 pieces, jitter 10, cell (0x80,
    // 0x40, 0x20, 0x40), the piece counter -(k + 1); their directions
    // (+0x1850 + 0x10 k) random of length 8.
    fx.texture5(sp + 0x90, 0);
    let tex = fx.mem.read_u32(sp + 0x90);
    for k in 0..8u32 {
        let g = STRANDS + 0x30 * k;
        fx.mem.write_u32(g, tex);
        fx.mem.write_u32(g + 4, 0x0408_0000);
        halves(fx, g, &[(0x10, 0x80), (0x12, 0x40), (0x14, 0x20), (0x16, 0x40)]);
        bytes(fx, g, &[(0xA, 0x80), (9, 0x80), (8, 0x80), (0xB, 0x40)]);
        halves(fx, g, &[(0xC, 0x10), (0x28, -(k as i32 + 1)), (0xE, 0xA)]);
        fx.mem.write_u32(g + 0x20, 0);
        fx.mem.write_u32(g + 0x24, 0);
        random_dir8(fx, sp, DIRS + 0x10 * k, 0x01E0_5968);
    }
    // The sprites (+0x510): 128 of the attacker's texture 1, flags
    // 0x70000003, half size 0.5 growing 15 an age, cell (0, 0, 0x20, 0x20)
    // of 0x80 x 0x40, 8 frames; thrown 8 in random directions, age -(rand
    // % 12), alpha 0x30.
    fx.texture5(sp + 0x90, 1);
    let tex = fx.mem.read_u32(sp + 0x90);
    let g = SPRITES;
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x7000_0003);
    bytes(fx, g, &[(0xA, 0x80), (9, 0x80), (8, 0x80), (0xB, 0)]);
    fx.mem.write_u32(g + 0xC, 0x3F00_0000);
    for k in [0x20u32, 0x24, 0x28] {
        fx.mem.write_u32(g + k, 0);
    }
    halves(fx, g, &[(0x30, 0), (0x32, 0), (0x34, 0x20), (0x36, 0x20), (0x38, 0), (0x3A, 0), (0x3C, 0x80), (0x3E, 0x40)]);
    fx.mem.write_u32(g + 0x40, PARTS);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4170_0000);
    halves(fx, g, &[(0x5A, 0x80), (0x5C, 8), (0x5E, 0)]);
    let mut k = 0;
    while k < fx.mem.i16(g + 0x5A) {
        let p = PARTS + 16 * k as u32;
        random_dir8(fx, sp, sp + 0x60, 0x01E0_5A34);
        copy3(fx, sp + 0x60, p);
        let r = fx.rand_from(D, 0x01E0_5A50);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, h.wrapping_neg());
        fx.mem.write_u8(p + 0xC, 0x30);
        fx.mem.write_u8(p + 0xD, 0);
        k += 1;
    }
    // The quad cloud (+0xE60): TEXEFF 15, 64 quads of size 0.5, cell (0x8E,
    // 0, 0x22, 0x40), (0xD0, 0xA0, 0x70, 0x20), scattered (rand % 0x2000 -
    // 0x1000, rand & 0xFFFF, rand % 0x6000 - 0x3000).
    fx.texture4(sp + 0x90, 0xF);
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.set32(CLOUD + 4, 1);
    halves(fx, CLOUD, &[(0x10, 0x8E), (0x12, 0), (0x14, 0x22), (0x16, 0x40)]);
    bytes(fx, CLOUD, &[(0xA, 0x70), (9, 0xA0), (8, 0xD0), (0xB, 0x20)]);
    fx.quads_setup(CLOUD, tex, 0.5, QUAD_TEMPLATE);
    for k in 0..0x40u32 {
        let p = CLOUD + 0x60 + 16 * k;
        let r = fx.rand_from(D, 0x01E0_5B00);
        fx.mem.set32(p, r.wrapping_rem(0x2000) - 0x1000);
        let r = fx.rand_from(D, 0x01E0_5B20);
        fx.mem.set32(p + 4, r & 0xFFFF);
        let r = fx.rand_from(D, 0x01E0_5B40);
        let (_, h) = fx.div(r, 0x6000);
        fx.mem.set32(p + 8, h - 0x3000);
    }
    // The ribbon (+0x70): the attacker's texture 2, flags 0x8003, grey
    // 0x80, width 80, cell (0x10, 0, 0x10, 0x20) of 0x80 x 0x40, fade 0x10,
    // life 0x32, 32 slots.
    fx.texture5(sp + 0x90, 2);
    let tex = fx.mem.read_u32(sp + 0x90);
    let g = RIBBON;
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 8, 0x8003);
    bytes(fx, g, &[(6, 0x80), (5, 0x80), (4, 0x80), (7, 0x80)]);
    halves(fx, g, &[(0x10, 0x10), (0x12, 0), (0x14, 0x10), (0x16, 0x20), (0x18, 0x10), (0x1A, 0), (0x1C, 0x80), (0x1E, 0x40)]);
    fx.mem.write_u32(g + 0xC, 0x42A0_0000);
    halves(fx, g, &[(0x22, 0x10), (0x24, 0), (0x26, 0x32), (0x28, 0x20), (0x2A, 0), (0x2C, 1)]);
    // The facing quad (+0x490; its look +0x4F0: texture 2, size 5, grey
    // 0x80): flags 0x80001, cell (0x20, 0x20, 0x20, 0x20) of 0x80 x 0x40,
    // scale 1, grey 0x80 alpha 0, turn 0, 0x3E7 at +0x52.
    fx.mem.write_u32(QUAD + 0x38, 0x0008_0001);
    halves(fx, QUAD, &[(0, 0x20), (2, 0x20), (4, 0x20), (6, 0x20), (8, 0x20), (0xA, 0x20), (0xC, 0x80), (0xE, 0x40)]);
    fx.mem.write_u32(QUAD + 0x30, 0x3F80_0000);
    bytes(fx, QUAD, &[(0x36, 0x80), (0x35, 0x80), (0x34, 0x80), (0x37, 0)]);
    halves(fx, QUAD, &[(0x48, 0), (0x4A, 0), (0x4C, 0), (0x4E, 1), (0x50, 0), (0x52, 0x3E7)]);
    for k in [0x18u32, 0x14, 0x10] {
        fx.mem.write_u32(LOOK + k, 0);
    }
    fx.mem.write_u32(LOOK + 8, 0x40A0_0000);
    fx.mem.write_u32(LOOK, tex);
    bytes(fx, LOOK, &[(0xE, 0x80), (0xD, 0x80), (0xC, 0x80), (0xF, 0x80)]);
    for k in [8u32, 4, 0] {
        fx.mem.write_u32(D + 0xDF0 + k, 0x3DCC_CCCD);
    }
    0
}

fn set_scale(fx: &mut Fx, x: u32, v: u32) {
    for k in [8u32, 4, 0] {
        fx.mem.write_u32(x + k, v);
    }
}

/// The attacker's node of the request at D + `slot` into `out`.
fn node(fx: &mut Fx, slot: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(D + slot);
    fx.node_position(h, s, out);
}

/// 0x1039A0(x) from the entry, with its frame (0x80: s0 = D +0, s1 = the
/// OT +0x10, `ra` +0x20, the camera angles +0x30, the TRS matrix +0x40,
/// 0x20C0B0's output): the last one's translation row (sp - 0x10) is the
/// flash's textured quad's TEX1 residue [run].
fn billboard(fx: &mut Fx, sp: u32, x: u32, ra: u32) {
    fx.xform_billboard(x);
    let f = sp - 0x80;
    fx.saved(f, &[(0, u64::from(D)), (0x10, u64::from(OT)), (0x20, u64::from(ra))]);
    let a = fx.camera_angles();
    for k in 0..4u32 {
        fx.mem.write_u32(f + 0x30 + 4 * k, a[k as usize] as u32);
    }
    let sc: [f32; 3] = std::array::from_fn(|k| fx.mem.f(x + 4 * k as u32));
    let rot: [i32; 3] = std::array::from_fn(|k| fx.mem.i32(x + 0x10 + 4 * k as u32));
    let tr: [f32; 3] = std::array::from_fn(|k| fx.mem.f(x + 0x20 + 4 * k as u32));
    let m = fx.trs(Some(sc), Some(rot), Some(tr));
    fx.write_m4(f + 0x40, &m);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let t = fx.target_handle();
    fx.unit_point(1, t, D + 0x17F0);
    node(fx, 0x1900, D + 0xD90);
    node(fx, 0x1908, D + 0x16F0);
    node(fx, 0x190C, D + 0x1770);
    let n = fx.mem.i32(N);
    if n < 0xA0 {
        node(fx, 0x1900, POINT);
        node(fx, 0x1900, sp + 0x70);
        node(fx, 0x1904, sp + 0x80);
        fx.look_angles(sp + 0x70, sp + 0x80, D + 0xE00);
        fx.midpoint(sp + 0x70, sp + 0x80, D + 0xE10);
        if n >= 0x64 {
            set_scale(fx, SPACE, if n & 1 != 0 { 0x3F80_0000 } else { 0x3F4C_CCCD });
        }
    } else {
        if n == 0xA0 {
            node(fx, 0x1904, sp + 0x70);
            let y = ee::add(fx.mem.f(sp + 0x74), 1.0);
            fx.mem.setf(sp + 0x74, y);
            let y = ee::add(fx.mem.f(POINT + 4), 1.0);
            fx.mem.setf(POINT + 4, y);
            fx.ribbon_push(RIBBON, RIB_PTS, sp + 0x70);
            set_scale(fx, SPACE, 0x3F80_0000);
        } else {
            let t = fx.target_handle();
            fx.unit_position(t, D + 0x18D0);
        }
        let s = fx.mem.i32(SLIDE);
        fx.approach_by(POINT, D + 0x18D0, s);
        if fx.mem.i32(SLIDE) != 0 {
            fx.approach(SPACE, 5.0, f32::from_bits(0x3E4C_CCCD));
            let s = fx.mem.i32(SLIDE) - 1;
            fx.mem.set32(SLIDE, s);
            fx.ribbon_push(RIBBON, RIB_PTS, POINT);
        } else {
            fx.approach(SPACE, 20.0, 0.5);
            let t = fx.target_handle();
            fx.unit_position(t, POINT);
            fstep_down(fx, RIBBON + 0xC, 0.0, 8.0);
        }
    }
    let n = fx.mem.i32(N);
    if n == 0x29 {
        set_scale(fx, D + 0xD70, 0x3FC0_0000);
    }
    if n == 0x49 {
        set_scale(fx, D + 0xD70, 0x3F33_3333);
    }
    fx.xform_world(D);
    billboard(fx, sp, SPACE, 0x01E0_5F34);
    billboard(fx, sp, D + 0xD70, 0x01E0_5F3C);
    fx.xform_world(D + 0xDF0);
    billboard(fx, sp, D + 0x17D0, 0x01E0_5F4C);
    billboard(fx, sp, D + 0x16D0, 0x01E0_5F54);
    billboard(fx, sp, D + 0x1750, 0x01E0_5F5C);
    if n >= 0xB6 {
        crate::b262::tint(fx, TINT);
        if n < 0xD5 {
            fx.step_up(TINT + 3, 0xFF, 8);
        } else {
            fx.step_down(TINT + 3, 0, 8);
        }
    }
    if n >= 0xD5 {
        zoom::sprites(fx, 0x01E0_0550, ZOOM, POINT);
        fx.step_up(ZOOM + 7, 0x40, 4);
    }
    // The flash's alpha, and the a1..a3 its first TEXFLUSH reports: below
    // 0x90 0x1039A0(D + 0x1750)'s leftovers (its 0x20B370 call: sp - 0x40,
    // x + 0x10, x + 0x20; the table sine and 0x1A2B80 leave them [run]);
    // else the alpha steps' (cap / floor, step, the byte before) [run b648].
    let a123 = if n < 0x90 {
        let a = fx.mem.i32(ANGLE).wrapping_add(0x800);
        fx.mem.set32(ANGLE, a);
        let a = a & 0xFFFF;
        let s = fx.table_trig(a, false);
        let f = ee::mul(64.0, s);
        let v = ee::fixsfsi(f);
        // 0x1A2B80's frame: its ra at sp - 0x10 (the flash's textured
        // quad's TEX1 residue).
        fx.fix_residue(f, 0x01E0_6028);
        fx.mem.write_u8(FLASH + 3, ((v & 0xFF) + 0x40) as u8);
        [sp - 0x40, D + 0x1760, D + 0x1770]
    } else if n < 0x11E {
        let before = u32::from(fx.mem.read_u8(FLASH + 3));
        fx.step_up(FLASH + 3, 0x80, 4);
        [0x80, 4, before]
    } else {
        let before = u32::from(fx.mem.read_u8(FLASH + 3));
        fx.step_down(FLASH + 3, 0, 4);
        [0, 4, before]
    };
    flash::frame_flash(fx, FLASH, a123);
    if n < 0x29 {
        fx.approach(D + 0xDF0, 1.0, f32::from_bits(0x3D4C_CCCD));
        fx.step_up(DISC + 7, 0xFF, 8);
    } else if n < 0x2A {
        // Update 0x29: nothing.
    } else if n < 0x49 {
        approach_down(fx, D + 0xD70, f32::from_bits(0x3F33_3333), f32::from_bits(0x3CA3_D70A));
        fx.mem.write_u8(DISC + 7, if n & 1 != 0 { 0x80 } else { 0xFF });
    } else {
        fx.step_down(CLOUD + 0xB, 0, 2);
    }
    if n >= 0xA1 {
        fx.mem.write_u8(DISC + 7, 0x80);
        fx.approach(D + 0xD70, 2.0, f32::from_bits(0x3DCC_CCCD));
        if n >= 0xB0 {
            fx.step_down(DISC + 7, 0, 4);
        }
    }
    fx.disc(DISC, D + 0xD70);
    fx.quads_draw(CLOUD, D + 0xDF0);
    if n >= 0x43 {
        facing::quad(fx, OT, QUAD, LOOK, SPACE);
        fx.step_up(QUAD + 0x37, 0x80, 8);
        rain::draw(fx, 0x01E0_5150, OT, RAIN, SPACE, 1.5);
        // The rain's prologue (0x110: s0 = D, s1 = the OT, s2 = the
        // parameter -1 as 64 bits, s3..fp the runner's 0, ra +0xA0): the
        // sprites' missing-texture TEX0 / TEX1 are s5's upper half and s6
        // [run].
        fx.saved(sp - 0x110, &[(0x10, u64::from(D)), (0x20, u64::from(OT)), (0x30, u64::MAX), (0x40, 0), (0x50, 0), (0x60, 0), (0x70, 0), (0x80, 0), (0x90, 0), (0xA0, 0x01E0_6218)]);
        fx.sprites(SPRITES, SPACE);
        fx.step_up(SPRITES + 0xB, 0x80, 3);
        if n >= 0xB6 {
            fx.mem.write_u32(SPRITES + 0xC, 0x4120_0000);
        }
        let mut k = 0;
        while k < fx.mem.i16(SPRITES + 0x5A) {
            let p = PARTS + 16 * k as u32;
            k += 1;
            if fx.mem.i16(p + 0xE) >= 0 {
                for c in [0u32, 4, 8] {
                    let v = ee::mul(fx.mem.f(p + c), f32::from_bits(0x3F66_6666));
                    fx.mem.setf(p + c, v);
                }
            }
            if fx.mem.i16(p + 0xE) < 8 {
                continue;
            }
            random_dir8(fx, sp, sp + 0x60, 0x01E0_62C8);
            copy3(fx, sp + 0x60, p);
            fx.mem.set16(p + 0xE, 0);
        }
        for k in [0x78u32, 0x74, 0x70] {
            fx.mem.write_u32(sp + k, 0);
        }
        for k in 0..8u32 {
            let g = STRANDS + 0x30 * k;
            strand::draw(fx, 0x01E0_4580, OT, g, SPACE, sp + 0x70, DIRS + 0x10 * k);
            if fx.mem.i16(g + 0x28) == 0 {
                random_dir8(fx, sp, DIRS + 0x10 * k, 0x01E0_634C);
            }
        }
    }
    if n >= 0xA0 {
        fx.ribbon_draw(RIBBON, RIB_PTS, D);
        let mut k = 0;
        while k < fx.mem.i16(RIBBON + 0x2A) {
            fx.step_down(RIB_PTS + 0xC + 16 * k as u32, 0, 5);
            k += 1;
        }
    }
    let n = n + 1;
    fx.mem.set32(N, n);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if n == 0x11E {
        2
    } else if n == 0xAC {
        1
    } else if (0xB6..0x11E).contains(&n) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
