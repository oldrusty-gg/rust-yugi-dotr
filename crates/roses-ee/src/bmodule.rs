//! Drawing attack effects: a stateful runner for a BMODULE program whose
//! boundary is the port's own battle state (option A of
//! `duel/EFFECTS-DRAWING-AUDIT.md`; research in
//! `duel/the project notes).
//!
//! The module and every executable helper it calls run as original code in
//! the interpreter, from the user's executable and BMODULE record. What the
//! game's other systems would have left in memory is mirrored into the
//! emulated image before each update from an [`EffectWorld`] (the camera,
//! the two unit objects, their skeleton node results); what the module
//! draws is captured at the primitive descriptor 0x206F30 / 0x206F90 as GS
//! screen-space primitives ([`GsPrim`]).
//!
//! State a B module reads [emu: load/store traces of B220 / B125 from the
//! footage RAM, `examples/trace_state.rs`]:
//! - handle (side << 8 | slot) -> object index 0x1E25E0 (hooked here), the
//!   object table 0x2DE2C0, the self / target handles 0x2DF5A8 / 0x2DF5AC
//!   (set by 0x1F65A0 / 0x1F6580) and the linked module type 0x2DF5A0
//!   (mixed into node-request keys);
//! - unit object: position +0x00, animated position +0x80, node requests
//!   +0x1B0 (8 u16 keys, bit 0x8000 = result valid, count +0x1F9) and their
//!   results +0xA0 + 16 * slot, own index +0x1F2, node count +0x1FE, flags
//!   +0x20C (bit 5 = node results available) / +0x20D;
//! - camera: the object at *0x2DF5C0 (+0x44 projection distance), the
//!   camera stack index 0x2ED6C0 and record 0x2ED720 (+0x00 view-projection,
//!   +0x40 view, +0x80 projection, row vectors);
//! - textures: 0x202530(type, id) -> RH2 header (hooked here: type 4 =
//!   TEXEFF record, type 5 = the attacker's effect slot), whose TEX0 at +0x30
//!   is copied into descriptors.
//!
//! Mirrored values follow the footage RAM (battle-pine 032 / 044 / 060):
//! arena objects are not recentred (obj+0x1FF = 0), node results are the
//! node origin under the object matrix at the current motion time, flags
//! +0x20C = 0xE2.
use crate::{Cpu, Hook, Stop};
pub use roses_fx::types::*;
pub use roses_fx::world::{OBJECT_INDEX, OBJECT_RECORD};
use roses_fx::world::{CAMERA_RECORD, LINKED_TYPE};
use std::collections::BTreeMap;

pub use crate::effects::{DATA_AREA, LINK_ADDRESS, RECORD_BYTES};

const ENTRY_TABLE: u32 = 0x002B_94C0;
/// CMODULE entries by program id (0x1F5B10 for type 0) [F].
const CMODULE_ENTRY_TABLE: u32 = 0x002B_8510;
/// Type-0 task slot 0's data area (0x1E10000 + 0x8000 slot) [F].
const CMODULE_DATA_AREA: u32 = 0x01E1_0000;
const SET_TARGET: u32 = 0x001F_6580;
const SET_SELF: u32 = 0x001F_65A0;
const STACK: u32 = 0x01F0_0000;
const STEPS_PER_CALL: u64 = 300_000_000;

/// Object table and the two arena objects (indices and records as in the
/// footage: attacker 25 at 0x1FE42D0, defender 20 at 0x1FE3830).
const LAYER_SCRATCH: u32 = 0x01F9_0000;
/// Effect textures are copied here (RH2 records with patched TEX0).

/// Boundary functions a module may call that this shim does not
/// implement: they are answered (`Some`) or run as original code (`None`)
/// and every call is reported in [`EffectUpdate::boundary`].
const BOUNDARY: &[(u32, &str, Option<u64>)] = &[
    (0x001F_8270, "unit model extra draw 0x1F8270", None),
    (0x001F_8280, "unit model extra draw 0x1F8280", None),
    (0x001F_82B0, "unit model extra draw 0x1F82B0", None),
    (0x001F_8380, "object op 0x1F8380", Some(0)),
    (0x001F_83B0, "object op 0x1F83B0", Some(0)),
    (0x0020_79D0, "previous-frame copy 0x2079D0", None),
    (0x0019_A410, "printf", Some(0)),
    (0x001A_9080, "0x1A9080", Some(0)),
    (0x0020_7BB0, "packet writer 0x207BB0", None),
    (0x0020_7EF0, "packet writer 0x207EF0", None),
    (0x0020_7F50, "packet writer 0x207F50", None),
    (0x0020_7FC0, "packet writer 0x207FC0", None),
    (0x0020_95A0, "packet writer 0x2095A0", None),
    (0x0020_96C0, "packet writer 0x2096C0", None),
    (0x0020_EA20, "packet writer 0x20EA20", None),
    (0x0020_DAD0, "camera set 0x20DAD0", None),
    (0x0020_E3D0, "0x20E3D0", None),
    (0x0020_D660, "camera copy 0x20D660", None),
    (0x001F_84D0, "target flag 0x1F84D0", None),
    (0x001F_8100, "outcome 0x1F8100", None),
    (0x001F_8470, "camera track 0x1F8470", None),
];

/// The runner's name for a boundary call at `pc` (reported in
/// [`EffectUpdate::boundary`]), if it is one (helper C6: the
/// function-level harness compares them).
pub fn boundary_name(pc: u32) -> Option<&'static str> {
    BOUNDARY.iter().find(|b| b.0 == pc).map(|b| b.1)
}

/// The camera a pre-model callback left (see [`UnitRender::pre_vp`]): kept
/// when it differs from the battle camera's record.
fn record_pre_vp(c: &mut Cpu, out: &mut EffectUpdate) {
    let index = c.read_u32(0x002E_D6C0);
    let r = CAMERA_RECORD.wrapping_add(0x1A0u32.wrapping_mul(index));
    let vp: [[f32; 4]; 4] = std::array::from_fn(|i| std::array::from_fn(|k| c.read_f32(r + 16 * i as u32 + 4 * k as u32)));
    if std::env::var_os("ROSES_BMODULE_PRE_VP").is_some() {
        let mut dump = |o: u32, n: u32| (0..n).map(|k| c.read_f32(r + o + 4 * k)).collect::<Vec<_>>();
        let (a, b, d, e, f) = (dump(0, 16), dump(0x40, 16), dump(0x80, 16), dump(0x100, 8), dump(0x160, 8));
        eprintln!("pre callback camera: index {index} vp {a:?} +0x40 {b:?} +0x80 {d:?} +0x100 {e:?} +0x160 {f:?}");
    }
    if let Some(u) = out.unit_renders.last_mut() {
        u.pre_vp = Some(vp);
    }
}

/// Where a module's unit-target callback returns to (never executed: the
/// hook restores 0x1EF8F0's return address).
const TARGET_CALLBACK_RETURN: u32 = 0x0DEA_DBF0;

/// 0x1A2C70 (float f12 -> unsigned int) and 0x1A2B80 (-> int), the
/// executable's soft-float conversions; together with their unpack 0x1A2B20
/// a third of a heavy module's instructions [run: B220 profile].
const FIXUNSSFSI: u32 = 0x001A_2C70;
const FIXSFSI: u32 = 0x001A_2B80;
/// 0x20ABF0 / 0x20ACC0 (table cosine / sine of a 16-bit angle) and the VU0
/// matrix stack 0x20B260 / 0x20B2C0, run natively (`Cpu::native_*`).
const TABLE_COS: u32 = 0x0020_ABF0;
const TABLE_SIN: u32 = 0x0020_ACC0;
const MATRIX_PUSH: u32 = 0x0020_B260;
const MATRIX_POP: u32 = 0x0020_B2C0;
/// 0x205210, a descriptor's vertices projected in place (`Cpu::native_project`):
/// over half of the heaviest modules' instructions [run: b222 / b765].
const PROJECT: u32 = 0x0020_5210;

/// Every address the runner answers or counts (a fast pre-check for the
/// hook called at each control transfer).
fn is_hooked(pc: u32) -> bool {
    pc == TARGET_CALLBACK_RETURN
        || (0x0019_A410..=0x0020_EA20).contains(&pc)
        && (matches!(pc, 0x0020_6F30 | 0x0020_6F90 | 0x001E_25E0 | 0x0020_2530 | 0x0020_78A0 | 0x0020_7910 | 0x001E_F6F0 | 0x001E_F8F0 | FIXUNSSFSI | FIXSFSI | TABLE_COS | TABLE_SIN | MATRIX_PUSH | MATRIX_POP | PROJECT) || BOUNDARY.iter().any(|b| b.0 == pc))
}

/// 0x1A2C70 as its code computes it [F]: zero exponent field, a sign or an
/// exponent below 0 give 0; 32 and above 0xFFFFFFFF; else the 1.23 mantissa
/// at bit 30 shifted right by 30 - e (left by 1 for e = 31), truncating.
pub fn fixunssfsi(bits: u32) -> u32 {
    let field = (bits >> 23) & 0xFF;
    if field == 0 || bits >> 31 != 0 {
        return 0;
    }
    let e = field as i32 - 127;
    if e < 0 {
        return 0;
    }
    if e >= 32 {
        return u32::MAX;
    }
    let m = ((bits & 0x7F_FFFF) << 7) | 0x4000_0000;
    if e < 31 { m >> (30 - e) } else { m << 1 }
}

/// 0x1A2B80 [F]: zero exponent field or e < 0 give 0; e >= 31 saturates to
/// 0x7FFFFFFF / 0x80000000 by sign; else the mantissa shifted right by
/// 30 - e, negated for a sign.
pub fn fixsfsi(bits: u32) -> i32 {
    let field = (bits >> 23) & 0xFF;
    let negative = bits >> 31 != 0;
    if field == 0 {
        return 0;
    }
    let e = field as i32 - 127;
    if e < 0 {
        return 0;
    }
    if e >= 31 {
        return if negative { i32::MIN } else { i32::MAX };
    }
    let v = ((((bits & 0x7F_FFFF) << 7) | 0x4000_0000) >> (30 - e)) as i32;
    if negative { v.wrapping_neg() } else { v }
}

pub use roses_fx::world::decode_descriptor;

/// A launched attack effect.
pub struct AttackEffect {
    pub cpu: Cpu,
    entry: u32,
    /// The data area handed to every call: 0x1ED0000 for a B module (type
    /// 1), 0x1E10000 + 0x8000 slot for a C module (type 0, slot 0 here).
    data_area: u32,
    pub card: u16,
    pub model: u16,
    /// Calls made (the launch is call 0).
    pub calls: usize,
    pub finished: bool,
    handles: [u16; 2],
    textures: BTreeMap<(i32, i32), u32>,
    /// Native versions of hot pure helpers (exact; see [`fixunssfsi`]).
    pub hle: bool,
    /// What the offscreen block at TBP 0x2000 last received: a unit drawn
    /// there (0x1EF6F0 / 0x1EF8F0) or a copy of the frame (0x207FC0 with a
    /// destination); a later primitive sampling it without a new write in
    /// its update reads what it holds [H].
    rt: Option<RtContent>,
    /// Destinations of frame copies seen (other than 0x2000).
    copies: Vec<u16>,
    /// Research: consulted at every control transfer before the runner's
    /// own hooks (`Some` answers it); the function-level parity harness
    /// (`crate::fnparity`) watches library calls with it. `None` normally.
    pub probe: Option<Probe>,
}

/// A [`AttackEffect::probe`].
pub type Probe = Box<dyn FnMut(&mut Cpu, u32) -> Option<Hook> + Send>;

/// The content of the offscreen block at [`UNIT_RT_TBP`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RtContent {
    Unit(u8),
    /// The frame buffer copied there (GS local-to-local, 0x207FC0).
    FrameCopy,
}

impl RtContent {
    fn key(self) -> TextureKey {
        match self {
            Self::Unit(role) => TextureKey::UnitRt(role),
            // A copy of the frame being drawn up to the copy; sampled as the
            // previous displayed frame (the blur history) [H: one frame late].
            Self::FrameCopy => TextureKey::Frame,
        }
    }
}

/// The type-1 entry of `card` and its model index, from the executable
/// image in `cpu`.
pub fn entry_of(cpu: &mut Cpu, card: u16) -> Option<(u16, u32)> {
    let model = crate::effects::model_index(cpu, card)?;
    let entry = cpu.read_u32(ENTRY_TABLE + 4 * u32::from(model));
    (LINK_ADDRESS..LINK_ADDRESS + RECORD_BYTES as u32).contains(&entry).then_some((model, entry))
}

/// newlib rand's state pointer: the state is the u64 at *(0x28ED18) + 0xA8
/// (0x19AE88) [F].
pub const RAND_REENT: u32 = 0x0028_ED18;

impl AttackEffect {
    /// The game's rand state as the module sees it.
    pub fn rand_state(&mut self) -> u64 {
        let p = self.cpu.read_u32(RAND_REENT);
        self.cpu.read_u64(p.wrapping_add(0xA8))
    }

    /// Put the game's rand state where the module's draws read it.
    pub fn set_rand_state(&mut self, state: u64) {
        let p = self.cpu.read_u32(RAND_REENT);
        self.cpu.write_u64(p.wrapping_add(0xA8), state);
    }

    /// Load the executable and `card`'s record (no call yet). `handles`:
    /// the attacker's and the defender's (side << 8 | slot).
    pub fn load(executable: &[u8], bmodule: &[u8], card: u16, handles: [u16; 2], textures: &EffectTextures) -> Result<Option<Self>, Stop> {
        let mut cpu = Cpu::new();
        cpu.load_elf(executable)?;
        let start = usize::from(card) * RECORD_BYTES;
        let Some(record) = bmodule.get(start..start + RECORD_BYTES) else {
            return Ok(None);
        };
        if record.get(..4) != Some(b"MWo3") {
            return Ok(None);
        }
        let Some((model, entry)) = entry_of(&mut cpu, card) else {
            return Ok(None);
        };
        cpu.load(LINK_ADDRESS, record);
        let mut effect = Self { cpu, entry, data_area: DATA_AREA, card, model, calls: 0, finished: false, handles, textures: BTreeMap::new(), hle: true, rt: None, copies: Vec::new(), probe: None };
        effect.place_textures(textures);
        effect.static_state();
        Ok(Some(effect))
    }

    /// Load the executable and CMODULE program `id` (a type-0 task, as
    /// `0x1F5B00(0, id, param, target, self)` launches it: entry from the
    /// table 0x2B8510, data area 0x1E10000 = slot 0) [F 0x1F5B10]. `handles`:
    /// (self, target) as the executor sets them (0x1F65A0 / 0x1F6580); a
    /// module reads the target with 0x1F6590 (c620: the leader's cell, row
    /// << 8 | column). The image's other state is the executable's own.
    pub fn load_cmodule(executable: &[u8], cmodule: &[u8], id: u16, handles: [u16; 2], textures: &EffectTextures) -> Result<Option<Self>, Stop> {
        let mut cpu = Cpu::new();
        cpu.load_elf(executable)?;
        let start = usize::from(id) * RECORD_BYTES;
        let Some(record) = cmodule.get(start..start + RECORD_BYTES) else {
            return Ok(None);
        };
        if record.get(..4) != Some(b"MWo3") {
            return Ok(None);
        }
        let entry = cpu.read_u32(CMODULE_ENTRY_TABLE + 4 * u32::from(id));
        if !(LINK_ADDRESS..LINK_ADDRESS + RECORD_BYTES as u32).contains(&entry) {
            return Ok(None);
        }
        cpu.load(LINK_ADDRESS, record);
        let mut effect = Self { cpu, entry, data_area: CMODULE_DATA_AREA, card: id, model: id, calls: 0, finished: false, handles, textures: BTreeMap::new(), hle: true, rt: None, copies: Vec::new(), probe: None };
        effect.place_textures(textures);
        effect.static_state();
        effect.cpu.write_u32(LINKED_TYPE, 0);
        Ok(Some(effect))
    }

    fn place_textures(&mut self, textures: &EffectTextures) {
        self.textures = roses_fx::world::place_textures(&mut self.cpu, textures);
    }

    /// State that does not change during the effect.
    fn static_state(&mut self) {
        roses_fx::world::static_state(&mut self.cpu, self.model);
    }

    /// Make `role`'s mirrored object record answer at object index `index`
    /// too (a C module that reads an object by index: the Exodia finale
    /// c061..c063 read object 0x20, Summoned Lord Exodia, through 0x1F31E0 /
    /// 0x1F3FE0 / 0x1F40D0 / 0x1F2760 [F]). Its node requests are filled for
    /// all 8 request slots (0x1F3FE0 writes the key only), and each of
    /// `motions` (motion, frame count) gets a non-null motion pointer
    /// (+0x178 + 4 m), which ChangeMotion 0x1F2770 requires; the motion a
    /// module sets is read back with [`Self::object_motion`].
    pub fn place_role(&mut self, role: usize, index: u8, motions: &[(u8, u32)]) {
        roses_fx::world::place_role(&mut self.cpu, role, index, motions);
    }

    /// `role`'s motion after the last call: (motion obj+0x1E0, time
    /// obj+0x19C, blend updates obj+0x1FB).
    pub fn object_motion(&mut self, role: usize) -> (u8, f32, u8) {
        let o = OBJECT_RECORD[role];
        (self.cpu.read_u8(o + 0x1E0), self.cpu.read_f32(o + 0x19C), self.cpu.read_u8(o + 0x1FB))
    }

    /// Write the battle's state into the image before a call.
    pub fn mirror(&mut self, world: &dyn EffectWorld) {
        roses_fx::world::mirror(&mut self.cpu, world, self.calls);
    }

    /// The launch (`entry(data, param)`) or the next update
    /// (`entry(data, -1)`), after mirroring `world`.
    pub fn update(&mut self, world: &dyn EffectWorld) -> Result<EffectUpdate, Stop> {
        if self.finished {
            return Ok(EffectUpdate { ret: 8, ..Default::default() });
        }
        self.mirror(world);
        let mut out = EffectUpdate::default();
        let handles = self.handles;
        let textures = self.textures.clone();
        let hle = self.hle;
        let mut rt = self.rt;
        let mut copies = std::mem::take(&mut self.copies);
        let mut open_layers: Vec<u32> = Vec::new();
        let mut next_layer = 0u32;
        // A module callback 0x1EF8F0 runs into a unit's target: (role, the
        // caller's return address).
        // (role, return address, the post callback still to run, running the
        // pre-model one)
        let mut in_target: Option<(u8, u64, u32, bool)> = None;
        // The buffer this frame draws into (0x209640(1) with the counter the
        // mirror wrote: odd -> TBP 0).
        let drawn_tbp: u16 = if self.cpu.read_u32(0x002E_963C) & 1 == 1 { 0 } else { 0x800 };
        // Research: every hooked call with the primitives emitted so far.
        let trace_calls = std::env::var_os("ROSES_BMODULE_TRACE_CALLS").is_some_and(|v| v.to_str().and_then(|v| v.parse::<usize>().ok()) == Some(self.calls));
        let mut probe = self.probe.take();
        let mut hooks = |c: &mut Cpu, pc: u32| -> Hook {
            if let Some(h) = probe.as_mut().and_then(|p| p(c, pc)) {
                return h;
            }
            if !is_hooked(pc) {
                return Hook::Run;
            }
            if hle {
                match pc {
                    FIXUNSSFSI | FIXSFSI => {
                        let (sp, bits, ra) = (c.reg(29) as u32, c.fpr[12], c.reg(31));
                        c.native_fix_frame(sp, bits, ra);
                        let v = if pc == FIXSFSI { fixsfsi(bits) as i64 as u64 } else { fixunssfsi(bits) as i32 as i64 as u64 };
                        return Hook::Return(Some(v));
                    }
                    TABLE_COS | TABLE_SIN => {
                        let a = c.reg(4) as u32;
                        c.native_table_trig(a, pc == TABLE_COS);
                        return Hook::Return(None);
                    }
                    MATRIX_PUSH => {
                        let m = c.reg(4) as u32;
                        c.native_matrix_push(m);
                        return Hook::Return(None);
                    }
                    MATRIX_POP => {
                        c.native_matrix_pop();
                        return Hook::Return(None);
                    }
                    PROJECT => {
                        let (a0, a1, a2, a3, t0) = (c.reg(4) as u32, c.reg(5) as u32, c.reg(6) as u32, c.reg(7) as u32, c.reg(8) as u32);
                        let v = c.native_project(a0, a1, a2, a3, t0);
                        return Hook::Return(Some(v));
                    }
                    _ => {}
                }
            }
            if trace_calls {
                eprintln!("bmodule call trace: pc {pc:#x} ra {:#x} a0..a3 {:#x} {:#x} {:#x} {:#x} prims {}", c.reg(31), c.reg(4), c.reg(5), c.reg(6), c.reg(7), out.prims.len());
            }
            if pc == TARGET_CALLBACK_RETURN {
                match in_target.take() {
                    // The pre-model callback returned: the post one gets its
                    // packet pointer.
                    Some((role, ra, next, pre_phase)) if next != 0 => {
                        if pre_phase {
                            record_pre_vp(c, &mut out);
                        }
                        in_target = Some((role, ra, 0, false));
                        c.gpr[4][0] = c.gpr[2][0];
                        c.gpr[31][0] = u64::from(TARGET_CALLBACK_RETURN);
                        return Hook::Jump(next);
                    }
                    Some((_, ra, _, pre_phase)) => {
                        if pre_phase {
                            record_pre_vp(c, &mut out);
                        }
                        c.gpr[31][0] = ra
                    }
                    None => {}
                }
                return Hook::Return(None);
            }
            match pc {
                0x0020_6F30 | 0x0020_6F90 => {
                    // 0x206F30(desc, ot, key); 0x206F90(packet, desc, ot, key).
                    let buffered = pc == 0x0020_6F90;
                    let a = if buffered { 5 } else { 4 };
                    let (desc, ot, key) = (c.reg(a) as u32, c.reg(a + 1) as u32, c.reg(a + 2) as u32 as i32);
                    if let Some(mut p) = decode_descriptor(c, desc) {
                        p.buffered = buffered;
                        // 0x206FF0..0x207040: flag 0x8000 links at the last
                        // entry (drawn first); other keys are clamped
                        // (unsigned) to the one before it [F].
                        p.ot_key = (ot != 0).then(|| if p.flags & 0x8000 != 0 { OT_ENTRIES as i32 - 1 } else { (key as u32).min(OT_ENTRIES - 2) as i32 });
                        p.layer = open_layers.last().copied();
                        p.target = in_target.map(|t| t.0);
                        p.target_pre = in_target.is_some_and(|t| t.3);
                        p.drawn_buffer = p.textured() && p.tex0.is_some_and(|t| (t & 0x3FFF) as u16 == drawn_tbp);
                        let tbp = p.tex0.map(|t| (t & 0x3FFF) as u16);
                        if p.texture.is_none() && tbp == Some(UNIT_RT_TBP) {
                            p.texture = rt.map(RtContent::key);
                        } else if p.texture.is_none() && tbp.is_some_and(|t| copies.contains(&t)) {
                            p.texture = Some(TextureKey::Frame);
                        }
                        if trace_calls {
                            eprintln!("bmodule call trace:   prim flags {:#x} vertices {} tex0 {:x?} texture {:?} alpha {:x?} ot {:?} layer {:?}", p.flags, p.vertices.len(), p.tex0, p.texture, p.alpha, p.ot_key, p.layer);
                        }
                        out.prims.push(p);
                    }
                    let v = if buffered { c.reg(4) } else { 0 };
                    return Hook::Return(Some(v));
                }
                // Packet layers: begin hands out a fresh pointer, end
                // (layer, ot, key) closes it.
                0x0020_78A0 => {
                    next_layer += 1;
                    open_layers.push(next_layer);
                    return Hook::Return(Some(u64::from(LAYER_SCRATCH + 0x40 * (next_layer & 0xFF))));
                }
                0x0020_7910 => {
                    let (ot, key) = (c.reg(5) as u32, c.reg(6) as u32 as i32);
                    if let Some(id) = open_layers.pop() {
                        // 0x207970 clamps the key like 0x206F90 [F].
                        out.layers.push((id, (ot != 0).then_some((key as u32).min(OT_ENTRIES - 2) as i32), out.prims.len()));
                    }
                    return Hook::Return(Some(0));
                }
                // The unit model draws of 0x1F0FD0 (into the offscreen
                // target): recorded, not run (the model renderer is VU1).
                0x001E_F6F0 | 0x001E_F8F0 => {
                    let obj = c.reg(5) as u32;
                    if let Some(role) = OBJECT_INDEX.iter().position(|i| u32::from(*i) == obj) {
                        rt = Some(RtContent::Unit(role as u8));
                        let (pre, extra) = if pc == 0x001E_F8F0 { (c.reg(11) as u32, c.read_u64(c.reg(29) as u32)) } else { (0, 0) };
                        out.unit_renders.push(UnitRender { role: role as u8, before: out.prims.len(), pre, pre_vp: None, extra });
                        // Both arguments are module callbacks(packet) -> packet
                        // [F]: t3 (0xEC(sp)) runs at 0x1EFE70 with the target
                        // and camera set, before the unit's model; the stack
                        // argument at 0x1F0818 after the unit and its colour
                        // pass, before the composite. 0x1EF8F0 returns the
                        // packet pointer they leave.
                        let code = |a: u32| a != 0 && (LINK_ADDRESS..LINK_ADDRESS + RECORD_BYTES as u32).contains(&a);
                        let post = extra as u32;
                        let (first, then) = match (code(pre), code(post)) {
                            (true, true) => (pre, post),
                            (true, false) => (pre, 0),
                            (false, true) => (post, 0),
                            (false, false) => (0, 0),
                        };
                        if first != 0 && in_target.is_none() {
                            let out_record = c.reg(9) as u32;
                            if out_record != 0 {
                                c.write_u64(out_record + 0x40, UNIT_RT_TEX0);
                                c.write_u64(out_record + 0x48, 0);
                            }
                            in_target = Some((role as u8, c.reg(31), then, code(pre)));
                            c.gpr[31][0] = u64::from(TARGET_CALLBACK_RETURN);
                            return Hook::Jump(first);
                        }
                    }
                    // Its output record (t1, 0x1F0FD0's sp + 0xA8): the target's
                    // TEX0 at +0x40 and TEX1 at +0x48, which 0x1F0FD0 hands to
                    // the module [emu card 21: TBP 0x2000, TBW 8, C_16, 512 x
                    // 512, TCC 1, CLD 1; TEX1 nearest].
                    let out_record = c.reg(9) as u32;
                    if out_record != 0 {
                        c.write_u64(out_record + 0x40, UNIT_RT_TEX0);
                        c.write_u64(out_record + 0x48, 0);
                    }
                    let a0 = c.reg(4);
                    return Hook::Return(Some(a0));
                }
                0x001E_25E0 => {
                    let h = ((c.reg(4) as u16 & 0xFF) << 8) | (c.reg(5) as u16 & 0xFF);
                    let v = match handles.iter().position(|x| *x == h) {
                        Some(role) => u64::from(OBJECT_INDEX[role]),
                        None => u64::MAX,
                    };
                    return Hook::Return(Some(v));
                }
                0x0020_2530 => {
                    let key = (c.reg(4) as i32, c.reg(5) as i32);
                    let v = textures.get(&key).copied().unwrap_or_else(|| {
                        out.missing_textures.push(key);
                        0
                    });
                    return Hook::Return(Some(u64::from(v)));
                }
                _ => {}
            }
            // 0x207FC0(packet, dest, .., tex0, clamp): a destination TBP > 0
            // queues a GS local copy of the frame being drawn (BITBLTBUF from
            // FRAME of the other parity, 512 x 512, TRXDIR 2) and points the
            // TEX0 at the copy [F 0x2082E0; run b212 / b352 / b451: dest
            // 0x2000 = 0x2095A0's 0x2E9638; emu card 100 c150: a 921-vertex
            // strip samples TBP 0x2000].
            if pc == 0x0020_7FC0 {
                let dest = c.reg(5) as u32 as i32;
                if dest > 0 {
                    let tbp = (dest & 0x3FFF) as u16;
                    out.frame_copies.push(FrameCopy { tbp, before: out.prims.len(), layer: open_layers.last().copied() });
                    if tbp == UNIT_RT_TBP {
                        rt = Some(RtContent::FrameCopy);
                    } else if !copies.contains(&tbp) {
                        copies.push(tbp);
                    }
                }
            }
            if let Some((_, name, answer)) = BOUNDARY.iter().find(|b| b.0 == pc) {
                out.boundary.push((pc, name, [c.reg(4) as u32, c.reg(5) as u32, c.reg(6) as u32, c.reg(7) as u32]));
                if let Some(v) = answer {
                    return Hook::Return(Some(*v));
                }
            }
            Hook::Run
        };
        // The executor sets both handles before every call [F 0x1F52E0].
        let arg: i64 = if self.calls == 0 { 0 } else { -1 };
        let (entry, data_area) = (self.entry, self.data_area);
        let r = (|| {
            self.cpu.call(SET_TARGET, &[u64::from(handles[1])], STACK, 10_000, &mut hooks)?;
            self.cpu.call(SET_SELF, &[u64::from(handles[0])], STACK, 10_000, &mut hooks)?;
            self.cpu.call(entry, &[u64::from(data_area), arg as u64], STACK, STEPS_PER_CALL, &mut hooks)
        })();
        drop(hooks);
        self.probe = probe;
        let r = r? as u32 as i32;
        out.ret = r;
        out.steps = self.cpu.steps;
        self.calls += 1;
        if r == 8 {
            self.finished = true;
        }
        self.rt = rt;
        self.copies = copies;
        for role in 0..2 {
            let o = OBJECT_RECORD[role];
            out.flags_20d[role] = self.cpu.read_u8(o + 0x20D);
            out.stage_light = self.cpu.read_u8(STAGE_LIGHT);
            out.unit_writes[role] = UnitWrites {
                tint: std::array::from_fn(|k| self.cpu.read_u8(o + 0x12C + k as u32)),
                add: std::array::from_fn(|k| self.cpu.read_u8(o + 0x130 + k as u32)),
                scale: std::array::from_fn(|k| self.cpu.read_f32(o + 0x30 + 4 * k as u32)),
                mode: self.cpu.read_u8(o + 0x1F8),
                fade: self.cpu.read_f32(o + 0x128),
            };
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn draw_order_follows_the_ordering_table() {
        use super::{draw_order, GsPrim};
        let p = |ot_key: Option<i32>, layer: Option<u32>| GsPrim { ot_key, layer, ..Default::default() };
        // 0 direct; 1 key 5; 2, 3 in layer 1 (linked at key 5 after 4);
        // 4 key 9; 5 key 5 (after the layer); 6 key 2.
        let prims = vec![p(None, None), p(Some(5), None), p(None, Some(1)), p(None, Some(1)), p(Some(9), None), p(Some(5), None), p(Some(2), None)];
        let order = draw_order(&prims, &[(1, Some(5), 5)]);
        assert_eq!(order, vec![0, 4, 5, 2, 3, 1, 6]);
        // A copy queued inside layer 1 between its two primitives lands
        // between them in the GS order; one sent straight before 0, first.
        let layers = [(1, Some(5), 5)];
        assert_eq!(super::copy_position(&prims, &layers, &super::FrameCopy { tbp: 0x2000, before: 3, layer: Some(1) }), 4);
        assert_eq!(super::copy_position(&prims, &layers, &super::FrameCopy { tbp: 0x2000, before: 0, layer: None }), 0);
        assert_eq!(super::copy_position(&prims, &layers, &super::FrameCopy { tbp: 0x2000, before: 2, layer: Some(1) }), 3);
    }

    use super::*;

    #[test]
    fn descriptor_lines_decode_to_window_coordinates() {
        let mut cpu = Cpu::new();
        let d = 0x10_0000;
        // Two vertices of an untextured line (flags 0x601): RGBAQ, XYZ2 twice; ALPHA follows.
        cpu.write_u16(d, 4);
        cpu.write_u16(d + 2, 0x0601);
        cpu.write_u64(d + 8, 0x5151);
        cpu.write_u64(d + 0x10, 0x3F80_0000_2840_80C0);
        cpu.write_u64(d + 0x18, (100u64 << 32) | (((1792 + 20) * 16) << 16) | ((1792 + 10) * 16));
        cpu.write_u64(d + 0x20, 0x3F80_0000_0000_00FF);
        cpu.write_u64(d + 0x28, (7u64 << 32) | (((1792 + 30) * 16 + 8) << 16) | ((1792 + 40) * 16));
        cpu.write_u64(d + 0x30, 0x80_0000_0048);
        let p = decode_descriptor(&mut cpu, d).unwrap();
        assert_eq!(p.kind, 1);
        assert_eq!(p.alpha, Some(0x80_0000_0048));
        assert_eq!(p.vertices.len(), 2);
        assert_eq!((p.vertices[0].x, p.vertices[0].y, p.vertices[0].z), (10.0, 20.0, 100));
        assert_eq!(p.vertices[0].rgba, [0xC0, 0x80, 0x40, 0x28]);
        assert_eq!((p.vertices[1].x, p.vertices[1].y), (40.0, 30.5));
        assert_eq!(p.vertices[1].rgba, [0xFF, 0, 0, 0]);
    }

    #[test]
    fn shim_texture_bases_map_back() {
        for k in [TextureKey::TexEff(0), TextureKey::TexEff(15), TextureKey::Own(0), TextureKey::Own(7), TextureKey::Frame] {
            assert_eq!(TextureKey::from_tbp(k.tbp()), Some(k));
        }
        assert_eq!(TextureKey::from_tbp(0x3C00), None);
        assert_eq!(TextureKey::from_tbp(0x800), Some(TextureKey::Frame));
        assert_eq!(TextureKey::from_tbp(UNIT_RT_TBP), None);
    }

    /// The arena's view-projection from footage 044's camera (eye +0x100,
    /// dir +0x110, d 675.0015) equals the record at 0x2ED720 [emu].
    #[test]
    fn view_projection_matches_the_footage_record() {
        let eye = [22.854_09, 51.182_39, -37.686_54];
        let dir = [-0.475_082_9, -0.024_579_08, -0.879_597_6];
        let cam = CameraMirror { eye, target: [eye[0] + dir[0] * 48.1, eye[1] + dir[1] * 48.1, eye[2] + dir[2] * 48.1], distance: 675.001_46, roll: 0.0 };
        let vp = mat_mul(&cam.view(), &cam.projection());
        let want = [
            [-379.0607, -962.3822, 973.0887, -0.4751],
            [-50.3380, -956.4944, 50.3441, -0.0246],
            [-2122.1946, -1781.8132, 1801.6360, -0.8796],
            [-68738.68, 3799.6484, 16_822_346.0, -21.0334],
        ];
        for i in 0..4 {
            for j in 0..4 {
                let tol = (want[i][j] as f32).abs() * 2e-4 + 2e-3;
                assert!((vp[i][j] - want[i][j] as f32).abs() <= tol, "{i},{j}: {} vs {}", vp[i][j], want[i][j]);
            }
        }
    }
}
