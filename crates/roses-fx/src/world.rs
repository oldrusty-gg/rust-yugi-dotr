//! The game state an effect program reads, written into its memory before
//! each call, and the primitive descriptors it hands the GS: shared by the
//! interpreter runner (`roses_ee::bmodule::AttackEffect`, the oracle) and
//! the hand ports, so both see the same bytes. Moved from
//! `roses_ee::bmodule` (30 Oct 2026); research in
//! `duel/the project notes.
use crate::bus::Bus;
use crate::types::*;
use std::collections::BTreeMap;

/// Object table and the two arena objects (indices and records as in the
/// footage: attacker 25 at 0x1FE42D0, defender 20 at 0x1FE3830).
pub const OBJECT_TABLE: u32 = 0x002D_E2C0;
pub const OBJECT_INDEX: [u8; 2] = [25, 20];
pub const OBJECT_RECORD: [u32; 2] = [0x01FE_42D0, 0x01FE_3830];
pub const OBJECT_BYTES: u32 = 0x220;
pub const LINKED_TYPE: u32 = 0x002D_F5A0;
pub const CAMERA_POINTER: u32 = 0x002D_F5C0;
pub const CAMERA_OBJECT: u32 = 0x01FE_04E0;
pub const CAMERA_STACK_INDEX: u32 = 0x002E_D6C0;
pub const CAMERA_RECORD: u32 = 0x002E_D720;
pub const PACKET_POINTER: u32 = 0x002E_9650;
pub const PACKET_SCRATCH: u32 = 0x01F4_0000;
/// Effect textures are copied here (RH2 records with patched TEX0).
pub const TEXTURE_AREA: u32 = 0x01A0_0000;
pub const TEXTURE_AREA_END: u32 = 0x01DF_0000;
/// The display frame counter (its parity picks the draw buffer).
pub const FRAME_COUNTER: u32 = 0x002E_963C;

/// Decode a primitive descriptor at `desc` (0x206F90's input): `+0 u16 n`
/// register count, `+2 u16 flags`, `+8 u64` 4-bit GS register ids, `+0x10`
/// n u64 register-format values, then ALPHA (0x200), TEX0 / TEX1 (0x20),
/// CLAMP (0x20 and 0x4000) [F 0x206F90].
pub fn decode_descriptor(cpu: &mut (impl Bus + ?Sized), desc: u32) -> Option<GsPrim> {
    let n = u32::from(cpu.read_u16(desc));
    let flags = cpu.read_u16(desc + 2);
    if flags & 0xF == 7 || n > 64 {
        return None;
    }
    let regs = cpu.read_u64(desc + 8);
    let values = desc + 0x10;
    // Fixed slots after the values: ALPHA, TEX0, TEX1, CLAMP.
    let extra = values + 8 * n;
    let alpha = (flags & 0x200 != 0).then(|| cpu.read_u64(extra));
    let (tex0, tex1, clamp) = if flags & 0x20 != 0 {
        let t = cpu.read_u64(extra + 8);
        let t1 = cpu.read_u64(extra + 16);
        let c = (flags & 0x4000 != 0).then(|| cpu.read_u64(extra + 24));
        (Some(t), Some(t1), c)
    } else {
        (None, None, None)
    };
    let mut prim = GsPrim {
        flags,
        kind: (flags & 7) as u8,
        vertices: Vec::new(),
        alpha,
        tex0,
        tex1,
        clamp,
        texture: tex0.and_then(|t| TextureKey::from_tbp((t & 0x3FFF) as u16)),
        buffered: false,
        desc,
        ot_key: None,
        layer: None,
        target: None,
        target_pre: false,
        post_copy: false,
        drawn_buffer: false,
    };
    let (mut rgba, mut q, mut uv) = ([0x80u8; 4], 1.0f32, [0.0f32; 2]);
    for i in 0..n {
        let mut id = ((regs >> (4 * i)) & 15) as u8;
        if flags & 0x800 != 0 && id == 2 {
            id = 3;
        }
        let v = cpu.read_u64(values + 8 * i);
        match id {
            0 => prim.kind = (v & 7) as u8,
            1 => {
                rgba = [v as u8, (v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8];
                q = f32::from_bits((v >> 32) as u32);
            }
            2 => uv = [f32::from_bits(v as u32), f32::from_bits((v >> 32) as u32)],
            3 => uv = [(v & 0x3FFF) as f32 / 16.0, ((v >> 16) & 0x3FFF) as f32 / 16.0],
            4 | 5 => prim.vertices.push(GsVertex {
                x: (v & 0xFFFF) as f32 / 16.0 - 1792.0,
                y: ((v >> 16) & 0xFFFF) as f32 / 16.0 - 1792.0,
                z: if id == 4 { ((v >> 32) & 0xFF_FFFF) as u32 } else { (v >> 32) as u32 },
                rgba,
                q,
                uv,
            }),
            _ => {}
        }
    }
    Some(prim)
}

/// Copy the RH2 records into the texture area with this shim's TBP0 /
/// CBP in TEX0 (+0x30) and the record's own address at +0x0C.
pub fn place_textures(bus: &mut (impl Bus + ?Sized), textures: &EffectTextures) -> BTreeMap<(i32, i32), u32> {
    let mut placed = BTreeMap::new();
    let mut at = TEXTURE_AREA;
    let all = textures
        .texeff
        .iter()
        .enumerate()
        .map(|(i, t)| (4, i as i32, TextureKey::TexEff(i as u8), t.as_ref()))
        .chain(textures.own.iter().enumerate().map(|(i, t)| (5, i as i32, TextureKey::Own(i as u8), t.as_ref())))
        .chain(textures.other.iter().take(8).enumerate().map(|(i, (kind, id, t))| (*kind, *id, TextureKey::Own(i as u8), Some(t))));
    for (kind, id, key, bytes) in all {
        let Some(bytes) = bytes else { continue };
        if bytes.len() < 0x60 || bytes.get(..4) != Some(b"RH2\0") || at + bytes.len() as u32 > TEXTURE_AREA_END {
            continue;
        }
        bus.load(at, bytes);
        bus.write_u32(at + 0x0C, at);
        let tex0 = bus.read_u64(at + 0x30);
        let tbp = u64::from(key.tbp());
        let tex0 = (tex0 & !0x3FFF & !(0x3FFF << 37)) | tbp | ((tbp + 0x20) << 37);
        bus.write_u64(at + 0x30, tex0);
        placed.insert((kind, id), at);
        at = (at + bytes.len() as u32 + 0x3F) & !0x3F;
    }
    placed
}

/// State that does not change during the effect.
pub fn static_state(bus: &mut (impl Bus + ?Sized), model: u16) {
    let c = bus;
    c.write_u32(LINKED_TYPE, 1);
    c.write_u32(LINKED_TYPE + 4, u32::from(model));
    c.write_u32(CAMERA_POINTER, CAMERA_OBJECT);
    c.write_u32(CAMERA_STACK_INDEX, 0);
    // The duel's fade bytes and the stage light (0x80 each).
    for k in 0..3 {
        c.write_u8(STAGE_LIGHT - 2 + k, 0x80);
    }
    for role in 0..2 {
        let o = OBJECT_RECORD[role];
        for k in (0..OBJECT_BYTES).step_by(4) {
            c.write_u32(o + k, 0);
        }
        c.write_u32(OBJECT_TABLE + 4 * u32::from(OBJECT_INDEX[role]), o);
        for k in 0..4 {
            c.write_f32(o + 0x30 + 4 * k, 1.0);
        }
        // Light scales and the RGBA multiplier (0x80 each) [emu].
        c.write_f32(o + 0x120, 0.8);
        c.write_f32(o + 0x124, 0.64);
        c.write_f32(o + 0x128, 1.0);
        c.write_u32(o + 0x12C, 0x8080_8080);
        for s in 0..8 {
            c.write_u16(o + 0x1B0 + 2 * s, 0x7FFF);
        }
        c.write_u8(o + 0x1F2, OBJECT_INDEX[role]);
        c.write_f32(o + 0x1E8, 1.0);
        c.write_u8(o + 0x20C, 0xE2);
    }
    // The GS draw environment the frame-buffer helpers read (0x209640 /
    // 0x2094B0, called by 0x207FC0 / 0x207EF0 / 0x2095A0 in 307 modules):
    // FRAME_1 of both display contexts, the frame's width / height /
    // PSM and 0x2E9638 [emu: footage RAM 044 / 064, equal in both].
    for (a, v) in [
        (0x002E_9430u32, 0x0A08_0040u32),
        (0x002E_9438, 0x4C),
        (0x002E_9520, 0x0A08_0000),
        (0x002E_9528, 0x4C),
        (0x002E_960C, 512),
        (0x002E_9610, 512),
        (0x002E_9620, 0x0A),
        (0x002E_9628, 0x31),
        (0x002E_962C, 1),
        (0x002E_9638, 0x2000),
        // The GS offsets 0x209580 hands 0x20E160 [emu footage 028].
        (0x002E_9630, 0x800),
        (0x002E_9634, 0x800),
        // The pixel aspect 0x209490 returns (1.4285710) and its mode.
        (0x002E_9614, 0x3FB6_DB6A),
        (0x002E_9618, 2),
    ] {
        c.write_u32(a, v);
    }
    // Camera record 0's constants that 0x20E160 builds a projection from
    // when a module sets a camera of its own (0x20DAD0, e.g. card 677's
    // pre-model callback): window centre +0x150 / +0x154, aspect +0x158 /
    // +0x15C, window +0x164 / +0x168, +0x16C, the clip ranges +0x170..0x180,
    // the Z max +0x184 [emu footage 028; +0x160 the distance is mirrored].
    let r = CAMERA_RECORD;
    for (k, v) in [
        (0x150u32, 0x4380_0000u32),
        (0x154, 0x4380_0000),
        (0x158, 0x3F80_0000),
        (0x15C, 0x3F70_A3D7),
        (0x164, 0x4400_0000),
        (0x168, 0x4400_0000),
        (0x16C, 0x3F80_0000),
        (0x170, 0x4580_0000),
        (0x174, 0x44FA_0000),
        (0x178, 0x457A_0000),
        (0x17C, 0x44FA_0000),
        (0x180, 0x457A_0000),
        (0x184, 0x00FF_FFFF),
    ] {
        c.write_u32(r + k, v);
    }
    // Camera object constants [emu footage 044 +0x48..+0x6C].
    let cam = CAMERA_OBJECT;
    for (k, v) in [(0x48, 1.0f32), (0x4C, 0.94), (0x50, 1.0), (0x54, 4096.0), (0x64, 0.5), (0x68, 0.5), (0x6C, 0.5)] {
        c.write_f32(cam + k, v);
    }
}

/// Make `role`'s mirrored object record answer at object index `index`
/// too (a C module that reads an object by index: the Exodia finale
/// c061..c063 read object 0x20, Summoned Lord Exodia, through 0x1F31E0 /
/// 0x1F3FE0 / 0x1F40D0 / 0x1F2760 [F]). Its node requests are filled for
/// all 8 request slots (0x1F3FE0 writes the key only), and each of
/// `motions` (motion, frame count) gets a non-null motion pointer
/// (+0x178 + 4 m), which ChangeMotion 0x1F2770 requires; the motion a
/// module sets is read back with [`Self::object_motion`].
pub fn place_role(bus: &mut (impl Bus + ?Sized), role: usize, index: u8, motions: &[(u8, u32)]) {
    let o = OBJECT_RECORD[role];
    let c = bus;
    c.write_u32(OBJECT_TABLE + 4 * u32::from(index), o);
    c.write_u8(o + 0x1F2, index);
    c.write_u8(o + 0x1F9, 8);
    for &(m, frames) in motions.iter().filter(|m| m.0 < 8) {
        c.write_u32(o + 0x178 + 4 * u32::from(m), 1);
        c.write_u32(o + 0x1C0 + 4 * u32::from(m), frames);
    }
}

/// The duel's unit records (0x1DE3A0: 0x5C bytes per slot, 0x3F4 per side,
/// sides 0..2, slots 0..10), the battle's role handles (0x1E64C0) and the
/// active side (0x1DF7C0) [F]; written only when the world has a battle
///.
pub const UNIT_RECORDS: u32 = 0x002D_B990;
pub const BATTLE_ROLES: u32 = 0x002D_E298;
pub const ACTIVE_SIDE: u32 = 0x002D_B8F4;
/// Two 0x10-byte card records the mirrored units point at (+8 / +0xA the
/// printed ATK / DEF): in the gap between the texture area and the record.
pub const BATTLE_CARDS: u32 = 0x01DF_FF00;

/// The unit record of (side, slot) (0x1DE3A0), None off the board.
pub fn unit_record(side: i32, slot: i32) -> Option<u32> {
    ((0..3).contains(&side) && (0..11).contains(&slot)).then(|| UNIT_RECORDS + 0x3F4 * side as u32 + 0x5C * slot as u32)
}

/// Write `b` as the 0x1F8100 closure reads it: every board record cleared
/// and marked absent (+0x45 = 0xFF), the roles' units present (+0x45 = 0)
/// with a card record of printed ATK / DEF min(max(v, 0), 0x1FFF) and the
/// rest as the s16 modifiers +0x38 / +0x3A, defence +0x57 bit 3, role 1's
/// cell at +0x40 / +0x41 (bit 6 clear); +0x3C (card 0x2A3's leader copy)
/// 0, no field units: so 0x267A00's bonus is 0 [run: the fnparity row]
/// and the closure's stats are the given ones.
pub fn mirror_battle(bus: &mut (impl Bus + ?Sized), b: &BattleState) {
    let c = bus;
    for side in 0..3 {
        for slot in 0..11 {
            let r = UNIT_RECORDS + 0x3F4 * side + 0x5C * slot;
            for k in (0..0x5C).step_by(4) {
                c.write_u32(r + k, 0);
            }
            c.write_u8(r + 0x45, 0xFF);
        }
    }
    let split = |v: i32| {
        let printed = v.clamp(0, 0x1FFF);
        (printed as u16, (v - printed).clamp(-0x8000, 0x7FFF) as i16 as u16)
    };
    for role in 0..2 {
        let h = b.roles[role];
        let Some(r) = unit_record(i32::from(h >> 8), i32::from(h & 0xFF)) else { continue };
        for k in (0..0x5C).step_by(4) {
            c.write_u32(r + k, 0);
        }
        c.write_u8(r + 0x45, 0xFF);
        let Some(u) = b.units[role] else { continue };
        let card = BATTLE_CARDS + 0x10 * role as u32;
        for k in (0..0x10).step_by(4) {
            c.write_u32(card + k, 0);
        }
        let (pa, ma) = split(u.attack);
        let (pd, md) = split(u.defence);
        c.write_u16(card + 8, pa);
        c.write_u16(card + 0xA, pd);
        c.write_u32(r, card);
        c.write_u16(r + 0x38, ma);
        c.write_u16(r + 0x3A, md);
        if role == 1 {
            c.write_u8(r + 0x40, b.cell.0);
            c.write_u8(r + 0x41, b.cell.1);
        }
        c.write_u8(r + 0x45, 0);
        c.write_u8(r + 0x57, if u.defence_position { 8 } else { 0 });
    }
    c.write_u32(BATTLE_ROLES, u32::from(b.roles[0]));
    c.write_u32(BATTLE_ROLES + 4, u32::from(b.roles[1]));
    c.write_u8(ACTIVE_SIDE, b.active_side);
}

/// The battle an image written by [`mirror_battle`] holds; `None` for the
/// untouched zero image (a role's record present with no card), which
/// 0x1F8100 answers 1. Research / the fnparity row only.
pub fn battle_in(bus: &mut (impl Bus + ?Sized)) -> Option<BattleState> {
    let c = bus;
    let roles = [c.read_u32(BATTLE_ROLES) as u16, c.read_u32(BATTLE_ROLES + 4) as u16];
    let mut units = [None; 2];
    let mut cell = (0, 0);
    for role in 0..2 {
        let h = roles[role];
        let Some(r) = unit_record(i32::from(h >> 8), i32::from(h & 0xFF)) else { continue };
        if role == 1 {
            cell = (c.read_u8(r + 0x40), c.read_u8(r + 0x41));
        }
        if c.read_u8(r + 0x45) >= 0xB {
            continue;
        }
        let card = c.read_u32(r);
        if card == 0 {
            return None;
        }
        units[role] = Some(BattleUnit {
            attack: i32::from(c.read_u16(card + 8) & 0x1FFF) + i32::from(c.read_u16(r + 0x38) as i16),
            defence: i32::from(c.read_u16(card + 0xA) & 0x1FFF) + i32::from(c.read_u16(r + 0x3A) as i16),
            defence_position: c.read_u8(r + 0x57) & 8 != 0,
        });
    }
    Some(BattleState { roles, active_side: c.read_u8(ACTIVE_SIDE), cell, units })
}

/// Write the battle's state into the image before a call.
pub fn mirror(bus: &mut (impl Bus + ?Sized), world: &dyn EffectWorld, calls: usize) {
    let c = bus;
    // Camera object and record 0.
    let cam = world.camera();
    let d = sub(cam.eye, cam.target);
    let dist = dot(d, d).sqrt();
    let (yaw, pitch) = if dist > 0.0 {
        (d[2].atan2(d[0]).rem_euclid(std::f32::consts::TAU), (d[1] / dist).clamp(-1.0, 1.0).asin())
    } else {
        (0.0, 0.0)
    };
    let dir = normalize(sub(cam.target, cam.eye)).unwrap_or([0.0, 0.0, 1.0]);
    let o = CAMERA_OBJECT;
    for (k, v) in [dist, yaw, pitch].into_iter().enumerate() {
        c.write_f32(o + 4 * k as u32, v);
    }
    for k in 0..3 {
        c.write_f32(o + 0x10 + 4 * k as u32, dir[k]);
        c.write_f32(o + 0x20 + 4 * k as u32, cam.eye[k]);
        c.write_f32(o + 0x30 + 4 * k as u32, cam.target[k]);
    }
    c.write_f32(o + 0x3C, 1.0);
    c.write_f32(o + 0x40, cam.roll);
    c.write_f32(o + 0x44, cam.distance);
    let view = cam.view();
    let proj = cam.projection();
    let vp = mat_mul(&view, &proj);
    let r = CAMERA_RECORD;
    for (base, m) in [(0u32, &vp), (0x40, &view), (0x80, &proj), (0xC0, &vp)] {
        for i in 0..4 {
            for j in 0..4 {
                c.write_f32(r + base + 16 * i as u32 + 4 * j as u32, m[i][j]);
            }
        }
    }
    for k in 0..3 {
        c.write_f32(r + 0x100 + 4 * k as u32, cam.eye[k]);
        c.write_f32(r + 0x110 + 4 * k as u32, dir[k]);
    }
    c.write_f32(r + 0x160, cam.distance);
    // Units.
    for role in 0..2 {
        let u = world.unit(role);
        let o = OBJECT_RECORD[role];
        for k in 0..3 {
            c.write_f32(o + 4 * k as u32, u.position[k]);
            c.write_u32(o + 0x10 + 4 * k as u32, u.rotation[k] as u32);
            c.write_f32(o + 0x80 + 4 * k as u32, u.animated[k]);
        }
        c.write_f32(o + 0x0C, 1.0);
        c.write_f32(o + 0x8C, 1.0);
        for k in 0..3 {
            c.write_f32(o + 0x90 + 4 * k as u32, u.bounds[k]);
        }
        c.write_f32(o + 0x9C, 1.0);
        c.write_f32(o + 0x1E4, u.bounds[3]);
        // The battle writes the alpha only while a unit fades; the
        // modules' own tint writes (+0x12C..) persist otherwise.
        if u.alpha != 0x80 {
            c.write_u8(o + 0x12F, u.alpha);
        }
        c.write_f32(o + 0x198, u.speed);
        c.write_f32(o + 0x19C, u.time);
        for m in 0..8 {
            c.write_u32(o + 0x1C0 + 4 * m as u32, u.frames[m]);
        }
        c.write_u8(o + 0x1E0, u.motion);
        c.write_u8(o + 0x1E1, 0xFF);
        let mode = c.read_u8(o + 0x1F7) & 0x3F | (((u.play_mode as u8) & 3) << 6);
        c.write_u8(o + 0x1F7, mode);
        let flags = c.read_u8(o + 0x20D) & !1 | u8::from(u.frozen);
        c.write_u8(o + 0x20D, flags);
        c.write_u16(o + 0x1F4, u.model);
        c.write_u16(o + 0x206, u.model);
        c.write_u8(o + 0x1FE, u.nodes);
        // Node results for the registered requests.
        let count = c.read_u8(o + 0x1F9).min(8);
        for s in 0..u32::from(count) {
            let key = c.read_u16(o + 0x1B0 + 2 * s);
            if key & 0x7FFF == 0x7FFF {
                continue;
            }
            match world.node(role, key as u8) {
                Some(p) => {
                    for k in 0..3 {
                        c.write_f32(o + 0xA0 + 16 * s + 4 * k as u32, p[k]);
                    }
                    c.write_f32(o + 0xAC + 16 * s, 1.0);
                    c.write_u16(o + 0x1B0 + 2 * s, key | 0x8000);
                }
                None => c.write_u16(o + 0x1B0 + 2 * s, key & 0x7FFF),
            }
        }
    }
    // The battle the outcome boundary reads, only when given.
    if let Some(b) = world.battle() {
        mirror_battle(c, &b);
    }
    c.write_u32(PACKET_POINTER, PACKET_SCRATCH);
    // The display frame counter (its parity picks the draw buffer).
    c.write_u32(0x002E_963C, world.frame_counter().unwrap_or(calls as u32));
}
