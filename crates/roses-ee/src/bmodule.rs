//! Drawing attack effects: a stateful runner for a BMODULE program whose
//! boundary is the port's own battle state (option A of
//! `duel/EFFECTS-DRAWING-AUDIT.md`; research in
//! `duel/claude-bmodule/NOTES.md`).
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
const OBJECT_TABLE: u32 = 0x002D_E2C0;
pub const OBJECT_INDEX: [u8; 2] = [25, 20];
pub const OBJECT_RECORD: [u32; 2] = [0x01FE_42D0, 0x01FE_3830];
const OBJECT_BYTES: u32 = 0x220;
const LINKED_TYPE: u32 = 0x002D_F5A0;
const CAMERA_POINTER: u32 = 0x002D_F5C0;
const CAMERA_OBJECT: u32 = 0x01FE_04E0;
const CAMERA_STACK_INDEX: u32 = 0x002E_D6C0;
const CAMERA_RECORD: u32 = 0x002E_D720;
const PACKET_POINTER: u32 = 0x002E_9650;
const PACKET_SCRATCH: u32 = 0x01F4_0000;
const LAYER_SCRATCH: u32 = 0x01F9_0000;
/// Effect textures are copied here (RH2 records with patched TEX0).
const TEXTURE_AREA: u32 = 0x01A0_0000;
const TEXTURE_AREA_END: u32 = 0x01DF_0000;

/// The PAL frame's projection (distance d; y keeps the pixel aspect
/// 1074.2855 / 800; z rows as the arena's record) [emu 0x2ED7A0].
const ASPECT_Y: f32 = 1_074.285_5 / 800.0;

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

/// A texture an effect draws with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextureKey {
    /// TEXEFF.MRG record (type 4).
    TexEff(u8),
    /// The attacker's effect RH2 in its MONSTER.MRG slot, obj+0x144 + 4 *
    /// slot (type 5).
    Own(u8),
    /// The other display buffer (TBP 0 / 0x800, PSMCT16S 512 x 512): the
    /// previous frame [F 0x209640 / 0x207FC0; emu B125].
    Frame,
    /// The offscreen render target at TBP 0x2000 holding unit `role` (0 the
    /// attacker, 1 its target) as 0x1F0FD0 last drew it there (the
    /// afterimage draws 0x1F8270 / 0x1F8280 / 0x1F82B0) [F; emu card 21].
    UnitRt(u8),
}

/// TBP0 of the offscreen target the unit-model extra draws render into.
pub const UNIT_RT_TBP: u16 = 0x2000;
/// Its TEX0: TBP 0x2000, TBW 8, PSMCT16, TW = TH = 9, TCC 1, CLD 1.
pub const UNIT_RT_TEX0: u64 = 0x2000 | (8 << 14) | (2 << 20) | (9 << 26) | (9 << 30) | (1 << 34) | (1 << 61);

impl TextureKey {
    /// The TBP0 of the texture (GS block units): the shim's own for TEXEFF
    /// (0x3400 + 0x40 k) and the attacker's RH2s (0x3800 + 0x40 e), clear of
    /// the frame buffers and render targets the game itself computes.
    pub fn tbp(self) -> u16 {
        match self {
            Self::TexEff(k) => 0x3400 + 0x40 * u16::from(k & 0xF),
            Self::Own(k) => 0x3800 + 0x40 * u16::from(k & 7),
            Self::Frame => 0,
            Self::UnitRt(_) => UNIT_RT_TBP,
        }
    }
    /// The shim's texture or a frame buffer at `tbp` (a render target needs
    /// the role, see [`EffectUpdate`]).
    pub fn from_tbp(tbp: u16) -> Option<Self> {
        match tbp {
            0 | 0x800 => Some(Self::Frame),
            0x3400..=0x37C0 if tbp & 0x3F == 0 => Some(Self::TexEff(((tbp - 0x3400) / 0x40) as u8)),
            0x3800..=0x39C0 if tbp & 0x3F == 0 => Some(Self::Own(((tbp - 0x3800) / 0x40) as u8)),
            _ => None,
        }
    }
}

/// RH2 records the module may ask for (from the user's import).
#[derive(Clone, Debug, Default)]
pub struct EffectTextures {
    /// TEXEFF.MRG records by id (type 4).
    pub texeff: Vec<Option<Vec<u8>>>,
    /// The attacker's effect RH2s by effect slot 0..8 (type 5).
    pub own: Vec<Option<Vec<u8>>>,
    /// Other archives' records a C module loads, as (type, record, RH2)
    /// (e.g. c401's TEXEVE records, type 8): placed as `TextureKey::Own(i)`
    /// for the i-th entry (at most 8, when `own` is empty).
    pub other: Vec<(i32, i32, Vec<u8>)>,
}

/// One vertex as the GS receives it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GsVertex {
    /// Window coordinates: XYZ2 x / 16 - 1792 (the 512-wide frame's
    /// XYOFFSET), y likewise.
    pub x: f32,
    pub y: f32,
    pub z: u32,
    /// RGBAQ bytes (alpha 0x80 = 1.0).
    pub rgba: [u8; 4],
    pub q: f32,
    /// UV in texels (FST) or ST (with q).
    pub uv: [f32; 2],
}

/// A primitive from a descriptor (0x206F30).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GsPrim {
    /// Descriptor flags (+2): kind bits 0-3, 0x10 IIP, 0x20 TME, 0x100 FGE,
    /// 0x200 ABE (+ ALPHA), 0x400 AA1, 0x800 FST, 0x1000 context 2, 0x2000
    /// FIX, 0x4000 CLAMP, 0x8000 Z write.
    pub flags: u16,
    /// GS PRIM type: 0 point, 1 line, 2 line strip, 3 triangle, 4 strip,
    /// 5 fan, 6 sprite.
    pub kind: u8,
    pub vertices: Vec<GsVertex>,
    pub alpha: Option<u64>,
    pub tex0: Option<u64>,
    /// TEX1 (filters, LOD K / L), with TEX0.
    pub tex1: Option<u64>,
    /// CLAMP when the descriptor writes it (flag 0x4000); else the GS keeps
    /// the previous draw's, REPEAT in every dump checked [emu 062-066: the
    /// frame-texture strips 0xA34; card 21's 0x226 composite].
    pub clamp: Option<u64>,
    /// The texture TEX0.TBP0 points at, when it is one this shim placed.
    pub texture: Option<TextureKey>,
    /// Written into a caller buffer (0x206F90) rather than the frame.
    pub buffered: bool,
    /// The descriptor's address (research).
    pub desc: u32,
    /// The ordering-table key it was linked at (0x206F30's a1 / a2: the
    /// OT 0x2E91B8, drawn from the highest key down, later links of a key
    /// first) [F 0x207970; emu OT links in footage 044 / 064].
    pub ot_key: Option<i32>,
    /// The packet layer (0x2078A0 ... 0x207910) it was written into.
    pub layer: Option<u32>,
    /// Drawn into this role's offscreen unit target (FRAME 0x2000): sent by
    /// the module's callback that 0x1EF8F0 calls between the unit's render
    /// there and its composite [F 0x1F080C..0x1F0818; emu card 484 c20
    /// draw 16, the "glass" strip].
    pub target: Option<u8>,
    /// Sent by the pre-model callback (0x1EF8F0's t3): drawn into the target
    /// before the unit's model [F 0x1EFE70].
    pub target_pre: bool,
    /// Drawn after the update's first frame copy (GS order, set by the
    /// caller from [`copy_position`]): not part of what the copy holds.
    pub post_copy: bool,
    /// TEX0 points at the buffer being drawn this frame (0x209640(1): TBP 0
    /// on odd frame counters 0x2E963C, else 0x800 [F]): the low-RAM TEX0 0
    /// on every other frame, reading what the frame holds so far.
    pub drawn_buffer: bool,
}

impl GsPrim {
    pub fn gouraud(&self) -> bool {
        self.flags & 0x10 != 0
    }
    pub fn textured(&self) -> bool {
        self.flags & 0x20 != 0
    }
    pub fn blended(&self) -> bool {
        self.flags & 0x200 != 0
    }
    pub fn uv_texels(&self) -> bool {
        self.flags & 0x800 != 0
    }
    /// TEX0 (TW, TH) as texel sizes.
    pub fn texture_size(&self) -> Option<(u32, u32)> {
        let t = self.tex0?;
        Some((1 << ((t >> 26) & 15), 1 << ((t >> 30) & 15)))
    }
}

/// Decode a primitive descriptor at `desc` (0x206F90's input): `+0 u16 n`
/// register count, `+2 u16 flags`, `+8 u64` 4-bit GS register ids, `+0x10`
/// n u64 register-format values, then ALPHA (0x200), TEX0 / TEX1 (0x20),
/// CLAMP (0x20 and 0x4000) [F 0x206F90].
pub fn decode_descriptor(cpu: &mut Cpu, desc: u32) -> Option<GsPrim> {
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

/// The arena camera as the port draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraMirror {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    /// Projection distance (camera +0x44, 256 / tan(fov / 2)).
    pub distance: f32,
    pub roll: f32,
}

impl CameraMirror {
    /// The view matrix (row vectors v' = v M): right, up (y down) and
    /// forward columns from eye and target, as the arena's look-at [emu
    /// 0x2ED760 in footage 044 against its eye +0x100 and dir +0x110].
    pub fn view(&self) -> [[f32; 4]; 4] {
        let f = normalize(sub(self.target, self.eye)).unwrap_or([0.0, 0.0, 1.0]);
        let r = normalize(cross(f, [0.0, 1.0, 0.0])).unwrap_or([1.0, 0.0, 0.0]);
        let u = cross(f, r);
        let t = [-dot(r, self.eye), -dot(u, self.eye), -dot(f, self.eye)];
        [
            [r[0], u[0], f[0], 0.0],
            [r[1], u[1], f[1], 0.0],
            [r[2], u[2], f[2], 0.0],
            [t[0], t[1], t[2], 1.0],
        ]
    }
    /// The projection record (0x2ED7A0).
    pub fn projection(&self) -> [[f32; 4]; 4] {
        let d = self.distance;
        [[d, 0.0, 0.0, 0.0], [0.0, d * ASPECT_Y, 0.0, 0.0], [2048.0, 2048.0, -2048.25, 1.0], [0.0, 0.0, 16_779_264.0, 0.0]]
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn normalize(a: [f32; 3]) -> Option<[f32; 3]> {
    let l = dot(a, a).sqrt();
    (l > 1e-12).then(|| [a[0] / l, a[1] / l, a[2] / l])
}
fn mat_mul(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..4).map(|k| a[i][k] * b[k][j]).sum()))
}

/// One arena unit object as the module sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitMirror {
    /// obj+0x00.
    pub position: [f32; 3],
    /// obj+0x10 (s32 angles, 0x10000 = a turn).
    pub rotation: [i32; 3],
    /// obj+0x80, the animated position.
    pub animated: [f32; 3],
    /// obj+0x1E0 current motion, obj+0x19C its time, obj+0x198 speed.
    pub motion: u8,
    pub time: f32,
    pub speed: f32,
    /// obj+0x1F7 top bits: 1 forward, -1 backward, 0 stopped.
    pub play_mode: i8,
    /// obj+0x20D bit 0 (0x1F84D0 -> 0x1F4500 sets it): the motion step
    /// 0x1EE6E0 and the cue player 0x1EEC80 skip the unit [F 0x1EE7B4,
    /// 0x1EED60]; B220 holds Orion at t 10 with it [emu battle-pine 036-043].
    pub frozen: bool,
    /// Frame counts by motion (obj+0x1C0 + 4 m).
    pub frames: [u32; 8],
    /// obj+0x12F (0x80 opaque).
    pub alpha: u8,
    /// obj+0x1FE, the skeleton's node count.
    pub nodes: u8,
    /// obj+0x206 (monster model index).
    pub model: u16,
    /// Node 0's bounding sphere: centre obj+0x90, radius obj+0x1E4 [emu].
    pub bounds: [f32; 4],
}

/// Entries of the ordering table 0x2E91B8 (log2 12 at +0xC) [emu].
pub const OT_ENTRIES: u32 = 4096;

/// The stage light byte (0x1F7F10 sets it, 0x1F7F40 reads it).
pub const STAGE_LIGHT: u32 = 0x002D_F5B2;

/// The GS order of an update's primitives (indices into `prims`): the ones
/// sent straight (no ordering table, outside a layer) first in submission
/// order [H]; then the ordering table 0x2E91B8 from its highest key down,
/// the later links of a key first (entry k links to k - 1 and a link is
/// pushed at the head) [F 0x206F30 / 0x207910]; a packet layer is one link
/// whose primitives keep their submission order.
pub fn draw_order(prims: &[GsPrim], layers: &[(u32, Option<i32>, usize)]) -> Vec<usize> {
    // (key, link sequence, primitives)
    let mut links: Vec<(i32, usize, Vec<usize>)> = Vec::new();
    let mut direct = Vec::new();
    let linked_layer = |id: u32| layers.iter().find(|l| l.0 == id && l.1.is_some());
    for (i, p) in prims.iter().enumerate() {
        match (p.layer.and_then(linked_layer), p.ot_key) {
            (Some(_), _) => {}
            (None, Some(k)) => links.push((k, 2 * i + 1, vec![i])),
            (None, None) => direct.push(i),
        }
    }
    for &(id, key, at) in layers {
        let Some(k) = key else { continue };
        let members: Vec<usize> = prims.iter().enumerate().filter(|(_, p)| p.layer == Some(id)).map(|(i, _)| i).collect();
        // Linked after primitive at - 1 (sequence 2 at - 1), before
        // primitive at (2 at + 1).
        links.push((k, 2 * at, members));
    }
    // Later links of a key first.
    links.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    direct.into_iter().chain(links.into_iter().flat_map(|l| l.2)).collect()
}

/// A GS local copy of the frame queued by 0x207FC0 into the packet being
/// written: its destination, the primitives submitted before it and the
/// packet layer it went into (none: sent straight).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameCopy {
    pub tbp: u16,
    pub before: usize,
    pub layer: Option<u32>,
}

/// Where a frame copy lands in the GS order of [`draw_order`]: the number of
/// primitives drawn before it. In a linked layer it keeps its submission
/// place among the layer's primitives; else it is sent straight, among the
/// straight primitives in submission order [F 0x207FC0 writes at the packet
/// pointer it is given, as 0x206F90 does].
pub fn copy_position(prims: &[GsPrim], layers: &[(u32, Option<i32>, usize)], copy: &FrameCopy) -> usize {
    // The copy as a marker primitive at its submission place.
    let at = copy.before.min(prims.len());
    let mut all: Vec<GsPrim> = Vec::with_capacity(prims.len() + 1);
    all.extend_from_slice(&prims[..at]);
    all.push(GsPrim { kind: 7, layer: copy.layer, ..Default::default() });
    all.extend_from_slice(&prims[at..]);
    // Links made after the copy move by one; its own layer closed after it.
    let shifted: Vec<(u32, Option<i32>, usize)> =
        layers.iter().map(|&(id, key, n)| (id, key, if n > at || (n == at && Some(id) == copy.layer) { n + 1 } else { n })).collect();
    draw_order(&all, &shifted).iter().position(|&i| i == at).unwrap_or(prims.len())
}

/// What the runner reads from the battle each update.
pub trait EffectWorld {
    fn camera(&self) -> CameraMirror;
    /// Role 0 attacker, 1 defender.
    fn unit(&self, role: usize) -> UnitMirror;
    /// World position of skeleton node `node` of unit `role` (obj+0xA0 +
    /// 16 * slot as the model renderer fills it).
    fn node(&self, role: usize, node: u8) -> Option<[f32; 3]>;
    /// The display frame counter 0x2E963C the module sees (one count per
    /// update, shared by every module of the battle); `None`: the calls made.
    fn frame_counter(&self) -> Option<u32> {
        None
    }
}

/// One update's output.
#[derive(Clone, Debug, Default)]
pub struct EffectUpdate {
    /// The module's return (8 = end).
    pub ret: i32,
    /// Primitives in submission order.
    pub prims: Vec<GsPrim>,
    /// Boundary calls this shim does not implement: (address, name, a0..a3).
    pub boundary: Vec<(u32, &'static str, [u32; 4])>,
    /// 0x202530 asks for textures not available (type, id).
    pub missing_textures: Vec<(i32, i32)>,
    /// obj+0x20D of (attacker, defender) after the update.
    pub flags_20d: [u8; 2],
    /// The stage light 0x2DF5B2 after the update (0x80 = 1.0: the stage
    /// objects' light scale g; modules set it through 0x1F7F10) [F
    /// ARENA-ENVIRONMENT.md 3; run: modules 373 / 133 dim the stage].
    pub stage_light: u8,
    /// Units drawn into the offscreen target by 0x1F0FD0 (0x1EF6F0 /
    /// 0x1EF8F0), in order, with the number of primitives emitted before.
    pub unit_renders: Vec<UnitRender>,
    /// Unit object fields the module wrote, read back after the update.
    pub unit_writes: [UnitWrites; 2],
    /// Packet layers closed this update: (id, OT key when linked, number
    /// of primitives submitted before the link).
    pub layers: Vec<(u32, Option<i32>, usize)>,
    /// GS local copies of the frame queued (0x207FC0).
    pub frame_copies: Vec<FrameCopy>,
    pub steps: u64,
}

/// A unit drawn into the offscreen target (TBP 0x2000) for an afterimage.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitRender {
    pub role: u8,
    /// Primitives emitted before it in this update.
    pub before: usize,
    /// 0x1EF8F0's t3 (module t0 of 0x1F82B0): not a matrix but a module
    /// callback run before the unit's model, into the target (0 = none)
    /// [F 0x1EF96C / 0x1EFE64..0x1EFE78].
    pub pre: u32,
    /// The camera the unit is drawn into the target with when the pre
    /// callback switched it (0x20D660 / 0x20DAD0 / 0x20E3D0): the current
    /// camera record's view-projection (0x2ED720 + 0x1A0 * index 0x2ED6C0,
    /// +0, row vectors) as the callback left it [F 0x20E3D0].
    pub pre_vp: Option<[[f32; 4]; 4]>,
    /// 0x1EF8F0's stack argument (module t1), when given.
    pub extra: u64,
}

/// Unit object fields modules write that change how the unit looks
/// [run: the sweep's object writes].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UnitWrites {
    /// obj+0x12C..0x12F: RGBA multiplier (0x80 = 1.0; alpha +0x12F).
    pub tint: [u8; 4],
    /// obj+0x130..0x133 (written with the tint by 24 modules) [H: an added colour].
    pub add: [u8; 4],
    /// obj+0x30: scale (x, y, z, w).
    pub scale: [f32; 4],
    /// obj+0x1F8: bits 0-1 pick the offscreen composite's ALPHA, bits 2-3 the
    /// full-window colour pass's (0x208450: table 0x2D0FA0 = 0x44, 0x48, 0x42,
    /// 0x84, FIX 0x80) [F 0x1EFD08 / 0x1F06EC / 0x1F0848].
    pub mode: u8,
    /// obj+0x128: the float the colour pass's alpha is scaled by (+0x133 *
    /// +0x128, 0x1A2C70) [F 0x1F0720..0x1F0760].
    pub fade: f32,
}

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
}

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
        let mut effect = Self { cpu, entry, data_area: DATA_AREA, card, model, calls: 0, finished: false, handles, textures: BTreeMap::new(), hle: true, rt: None, copies: Vec::new() };
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
        let mut effect = Self { cpu, entry, data_area: CMODULE_DATA_AREA, card: id, model: id, calls: 0, finished: false, handles, textures: BTreeMap::new(), hle: true, rt: None, copies: Vec::new() };
        effect.place_textures(textures);
        effect.static_state();
        effect.cpu.write_u32(LINKED_TYPE, 0);
        Ok(Some(effect))
    }

    /// Copy the RH2 records into the texture area with this shim's TBP0 /
    /// CBP in TEX0 (+0x30) and the record's own address at +0x0C.
    fn place_textures(&mut self, textures: &EffectTextures) {
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
            self.cpu.load(at, bytes);
            self.cpu.write_u32(at + 0x0C, at);
            let tex0 = self.cpu.read_u64(at + 0x30);
            let tbp = u64::from(key.tbp());
            let tex0 = (tex0 & !0x3FFF & !(0x3FFF << 37)) | tbp | ((tbp + 0x20) << 37);
            self.cpu.write_u64(at + 0x30, tex0);
            self.textures.insert((kind, id), at);
            at = (at + bytes.len() as u32 + 0x3F) & !0x3F;
        }
    }

    /// State that does not change during the effect.
    fn static_state(&mut self) {
        let c = &mut self.cpu;
        c.write_u32(LINKED_TYPE, 1);
        c.write_u32(LINKED_TYPE + 4, u32::from(self.model));
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
    pub fn place_role(&mut self, role: usize, index: u8, motions: &[(u8, u32)]) {
        let o = OBJECT_RECORD[role];
        let c = &mut self.cpu;
        c.write_u32(OBJECT_TABLE + 4 * u32::from(index), o);
        c.write_u8(o + 0x1F2, index);
        c.write_u8(o + 0x1F9, 8);
        for &(m, frames) in motions.iter().filter(|m| m.0 < 8) {
            c.write_u32(o + 0x178 + 4 * u32::from(m), 1);
            c.write_u32(o + 0x1C0 + 4 * u32::from(m), frames);
        }
    }

    /// `role`'s motion after the last call: (motion obj+0x1E0, time
    /// obj+0x19C, blend updates obj+0x1FB).
    pub fn object_motion(&mut self, role: usize) -> (u8, f32, u8) {
        let o = OBJECT_RECORD[role];
        (self.cpu.read_u8(o + 0x1E0), self.cpu.read_f32(o + 0x19C), self.cpu.read_u8(o + 0x1FB))
    }

    /// Write the battle's state into the image before a call.
    pub fn mirror(&mut self, world: &dyn EffectWorld) {
        let c = &mut self.cpu;
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
        c.write_u32(PACKET_POINTER, PACKET_SCRATCH);
        // The display frame counter (its parity picks the draw buffer).
        c.write_u32(0x002E_963C, world.frame_counter().unwrap_or(self.calls as u32));
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
        let mut hooks = |c: &mut Cpu, pc: u32| -> Hook {
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
        self.cpu.call(SET_TARGET, &[u64::from(handles[1])], STACK, 10_000, &mut hooks)?;
        self.cpu.call(SET_SELF, &[u64::from(handles[0])], STACK, 10_000, &mut hooks)?;
        let arg: i64 = if self.calls == 0 { 0 } else { -1 };
        let r = self.cpu.call(self.entry, &[u64::from(self.data_area), arg as u64], STACK, STEPS_PER_CALL, &mut hooks)? as u32 as i32;
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
