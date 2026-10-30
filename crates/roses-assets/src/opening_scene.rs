//! Native derivation of the opening's Simon scene (event 10040, Stonehenge with
//! Simon McMooran) from the user's disc: the eight world models of MODEL.MRG
//! pack 37, their placement, the camera, projection, fog and lighting exactly as
//! the PAL executable computes them, so that the scene no longer depends on a
//! private emulator capture.
//!
//! Sources (PAL SLES_515.79 EE addresses, all traced from the executable and
//! replayed bit-exactly against a saved memory image, see
//! `work/research/story/claude-opening-disc/SIMON-SCENE.md`):
//!
//! * World models: the story setup calls the pack loader `0x1E91B0(kind 3,
//!   sub 4)` (call site 0x21C0BC..0x21C0C4, the new-game path that then selects
//!   event 10020 at 0x21C15C). Pack index = kind base 33 (0x291E24) + 4 = 37;
//!   the pack offset table 0x291CD0 gives the MODEL.MRG range, the per-pack
//!   entry list (0x292100[37]) the RH0 offsets, 0x291F20/0x292060 the first
//!   object slot and entry count, and the id list at 0x2CF43C + 80·sub the
//!   object ids (400..409). The pack walker 0x1E9710 assigns entries to slots
//!   34.. in order; 0x1F1710 hooks a trailing `MOT` block (RH0 header word 1
//!   bit 0) into the object as its animation.
//! * Camera: controller defaults 0x1F8760; Simon updater 0x23A5C4 (eye
//!   (0,0,512), target (−64·pan/100, 20.2 + 768·level/4096, 0)); look-at
//!   0x1FB1F0 with the software `sqrtf`; view 0x20CD50 (basis, roll, general
//!   4×4 inverse 0x20B550); projection 0x20D050 with the camera apply
//!   0x1F9C40 (800, viewport 512, offsets 256) and display mode 9 / PSMZ24
//!   (boot call 0x1001E0); view·projection 0x20E160.
//! * Lights: default light 9 (colour 0.5, angles (−0x3800, 0x800, 0)) through
//!   0x1FA7F0 (rotation 0x20BDA0 applied to (0,0,1)), packet 0x1F9F40 →
//!   0x20C540 (view transform) → 0x208650 (colour × direction rows).
//! * Objects: draw 0x1EF8F0 (identity placement, colour multipliers 0.8/0.8/1.0
//!   from 0x1E8D60), emitter 0x20EC70 with the frustum test 0x20E550 (near/far
//!   crossing selects the clipping VU program 0x1A4, otherwise the
//!   pre-projected program 0x30), normals 0x20D1E0, node walk 0x20F7A0 and
//!   the animated walk 0x210050/0x210480 (MOT keyframes, angle interpolation
//!   0x20C940), materials 0x2119E0, fog packets 0x208BE0/0x208B30.
//!
//! Floating point follows PCSX2's execution of this game (`ee_float`).
use crate::opening_textures::{self, OpeningTextureRecord};
use crate::story_geometry::{self, StoryModel};
use crate::{Error, Result, RgbaImage, PAL_EXECUTABLE_SHA256};
use ee_float::{ee, sqrtf, vu};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub mod ee_float;
pub mod ee_view;

pub type Qword = [f32; 4];
/// Four qwords; qword `k` is column `k` of the matrix (`c = Σ_k q_k · p_k`).
pub type Matrix = [Qword; 4];

pub const SCENE_PROFILE: &str = "roses-private-captured-rh0-scene-1";
pub const PROVENANCE_KIND: &str = "iso_native_replay";
pub const MODEL_ARCHIVE_ISO_PATH: &str = "DATA/MODEL.MRG";
pub const PAL_MODEL_ARCHIVE_SHA256: &str = story_geometry::PAL_MODEL_ARCHIVE_SHA256;
/// Object slots of the world models (`0x291EF0[3]`).
pub const FIRST_WORLD_SLOT: u16 = 34;
const MAX_PACK_ENTRIES: usize = 16;
const MAX_MOT_BYTES: usize = 1024 * 1024;
const MAX_TRIANGLES: usize = 200_000;

/// Runtime state of the settled Simon view. `pan` is byte 0x1ED5439 (100 after
/// the dialogue's camera pan), `level` the 0..0x1000 word 0x1ED0064 (0 once the
/// entry tilt is over) and `sky_frame` the sky object's animation counter
/// (object +0x19C; 3.0 in the captured state, see the notes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimonPose {
    pub pan: i8,
    pub level: i16,
    pub sky_frame: f32,
}

impl Default for SimonPose {
    fn default() -> Self {
        Self {
            pan: 100,
            level: 0,
            sky_frame: 3.0,
        }
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(format!("Simon scene: {}", message.into()))
}
fn unsupported(message: impl Into<String>) -> Error {
    Error::Unsupported(format!("Simon scene: {}", message.into()))
}

// ---------------------------------------------------------------------------
// Executable access
// ---------------------------------------------------------------------------

/// The fingerprinted PAL executable, mapped through its ELF program headers.
pub struct Executable<'a> {
    bytes: &'a [u8],
    segments: Vec<(u32, u32, usize)>,
}

impl<'a> Executable<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self> {
        let hash = format!("{:x}", Sha256::digest(bytes));
        if hash != PAL_EXECUTABLE_SHA256 {
            return Err(Error::UnsupportedExecutable {
                actual_sha256: hash,
            });
        }
        let u16_at = |at: usize| -> Result<u16> {
            bytes
                .get(at..at + 2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .ok_or_else(|| invalid("executable header truncated"))
        };
        let u32_at = |at: usize| -> Result<u32> {
            bytes
                .get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(|| invalid("executable header truncated"))
        };
        if bytes.get(0..7) != Some(&[0x7f, b'E', b'L', b'F', 1, 1, 1][..]) {
            return Err(invalid("not a little-endian ELF32 image"));
        }
        let (phoff, phentsize, phnum) = (
            u32_at(0x1c)? as usize,
            u16_at(0x2a)? as usize,
            u16_at(0x2c)? as usize,
        );
        if phentsize < 32 || phnum > 4096 {
            return Err(invalid("program header table unsupported"));
        }
        let mut segments = Vec::new();
        for index in 0..phnum {
            let base = phoff + index * phentsize;
            if u32_at(base)? != 1 {
                continue;
            }
            let (offset, vaddr, filesz) =
                (u32_at(base + 4)?, u32_at(base + 8)?, u32_at(base + 16)?);
            if vaddr.checked_add(filesz).is_some()
                && (offset as usize)
                    .checked_add(filesz as usize)
                    .is_some_and(|end| end <= bytes.len())
            {
                segments.push((vaddr, filesz, offset as usize));
            }
        }
        Ok(Self { bytes, segments })
    }

    pub fn read(&self, address: u32, length: usize) -> Result<&'a [u8]> {
        for &(vaddr, filesz, offset) in &self.segments {
            if address >= vaddr
                && u64::from(address) + length as u64 <= u64::from(vaddr) + u64::from(filesz)
            {
                let start = offset + (address - vaddr) as usize;
                return self
                    .bytes
                    .get(start..start + length)
                    .ok_or_else(|| invalid("executable segment exceeds the file"));
            }
        }
        Err(invalid(format!(
            "EE address {address:#x} is not in a loaded segment"
        )))
    }
    pub fn u32(&self, address: u32) -> Result<u32> {
        Ok(u32::from_le_bytes(
            self.read(address, 4)?.try_into().unwrap(),
        ))
    }
    pub fn u16(&self, address: u32) -> Result<u16> {
        Ok(u16::from_le_bytes(
            self.read(address, 2)?.try_into().unwrap(),
        ))
    }
    pub fn f32(&self, address: u32) -> Result<f32> {
        Ok(f32::from_bits(self.u32(address)?))
    }
    fn qword(&self, address: u32) -> Result<Qword> {
        Ok([
            self.f32(address)?,
            self.f32(address + 4)?,
            self.f32(address + 8)?,
            self.f32(address + 12)?,
        ])
    }
    /// The 16-bit immediate of a `lui` at `address`.
    fn lui(&self, address: u32) -> Result<u32> {
        let word = self.u32(address)?;
        if word >> 26 != 0x0f {
            return Err(unsupported(format!("expected lui at {address:#x}")));
        }
        Ok(word & 0xffff)
    }
    /// The 16-bit immediate of an `ori` at `address`.
    fn ori(&self, address: u32) -> Result<u32> {
        let word = self.u32(address)?;
        if word >> 26 != 0x0d {
            return Err(unsupported(format!("expected ori at {address:#x}")));
        }
        Ok(word & 0xffff)
    }
    /// The signed immediate of an `addiu` at `address`.
    fn addiu(&self, address: u32) -> Result<i32> {
        let word = self.u32(address)?;
        if word >> 26 != 0x09 {
            return Err(unsupported(format!("expected addiu at {address:#x}")));
        }
        Ok(i32::from((word & 0xffff) as u16 as i16))
    }
    /// A float built by `lui` (upper half) at `lui_at`, lower half zero.
    fn float_lui(&self, lui_at: u32) -> Result<f32> {
        Ok(f32::from_bits(self.lui(lui_at)? << 16))
    }
    /// A float built by `lui`/`ori` at the two addresses.
    fn float_lui_ori(&self, lui_at: u32, ori_at: u32) -> Result<f32> {
        Ok(f32::from_bits(
            (self.lui(lui_at)? << 16) | self.ori(ori_at)?,
        ))
    }
}

/// Every executable constant the derivation uses, with its EE address.
#[derive(Clone, Debug, Serialize)]
pub struct SceneConstants {
    /// Camera controller defaults (0x1F8760): focal length, x/y scale, near, far.
    pub camera_focal: f32,
    pub camera_scale_x: f32,
    pub camera_scale_y: f32,
    pub camera_near: f32,
    pub camera_far: f32,
    /// Camera apply 0x1F9C40: viewport size and GS offsets given to 0x20D6B0.
    pub viewport: f32,
    pub viewport_offset: f32,
    /// Display mode (boot 0x1001E0) and its aspect entry (0x293234 + 40·mode).
    pub display_mode: u32,
    pub display_aspect: f32,
    /// GS projection centre (0x20A598) and the maximum depth of the boot Z format.
    pub gs_center: f32,
    pub z_format: u32,
    pub z_max: f32,
    /// Fog defaults (0x1F88BC/0x1F88C4) and the fog packet's 255 (0x208B3C).
    pub fog_near: f32,
    pub fog_far: f32,
    pub fog_255: f32,
    /// Default light 9: colour lanes and its three 16-bit angles.
    pub light_colour: f32,
    pub light_angles: [i32; 3],
    pub light_base_direction: Qword,
    /// Simon camera updater 0x23A5C4.
    pub simon_eye_z: f32,
    pub simon_pan_scale: f32,
    pub simon_pan_divisor: f32,
    pub simon_target_y: f32,
    pub simon_level_scale: f32,
    pub simon_level_divisor: f32,
    /// Projection helper constant (0x20E1B4) and the frustum test constants.
    pub two: f32,
    pub frustum_y_slope: f32,
    pub frustum_x_slope: f32,
    pub frustum_x_slope_squared: f32,
    pub frustum_four: f32,
    /// Object defaults (0x1E8D60): colour multipliers.
    pub colour_multiplier: f32,
    pub colour_multiplier_blue: f32,
    /// Draw scale test factor (0x1EFC6C) applied to the bounding radius.
    pub radius_scale: f32,
    pub sine_table: Vec<f32>,
    pub normal_scale: Qword,
    pub scale_one: Qword,
    pub zero_vector: Qword,
}

impl SceneConstants {
    pub fn read(exe: &Executable<'_>) -> Result<Self> {
        let display_mode = exe.addiu(0x1001e0)? as u32;
        let z_format = exe.addiu(0x1001e8)? as u32;
        // 0x2094D0: PSMZ32 → 0x7FFFFFFF, PSMZ24 → 0xFFFFFF, PSMZ16/16S → 0xFFFF.
        let z_max_bits: u32 = match z_format {
            0x30 => 0x7fff_ffff,
            0x31 => 0x00ff_ffff,
            0x32 | 0x3a => 0xffff,
            other => return Err(unsupported(format!("boot Z format {other:#x}"))),
        };
        let sine_table = exe
            .read(0x293430, 1025 * 4)?
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect::<Vec<_>>();
        if sine_table[0] != 0.0
            || sine_table[1024] != 1.0
            || sine_table.windows(2).any(|w| w[0] > w[1])
        {
            return Err(unsupported("sine table is not monotonic 0..1"));
        }
        Ok(Self {
            camera_focal: exe.float_lui(0x1f87e0)?,
            camera_scale_x: exe.float_lui(0x1f87c4)?,
            camera_scale_y: exe.float_lui_ori(0x1f87e8, 0x1f87f0)?,
            camera_near: exe.float_lui(0x1f87c4)?,
            camera_far: exe.float_lui(0x1f87fc)?,
            viewport: exe.float_lui(0x1f9c80)?,
            viewport_offset: exe.float_lui(0x1f9c98)?,
            display_mode,
            display_aspect: exe.f32(0x293234 + 40 * display_mode)?,
            gs_center: ee::from_i32(exe.addiu(0x20a598)?)?,
            z_format,
            z_max: ee::from_i32(z_max_bits as i32)?,
            fog_near: exe.float_lui(0x1f88bc)?,
            fog_far: exe.float_lui(0x1f88c4)?,
            fog_255: exe.float_lui(0x208b3c)?,
            light_colour: exe.float_lui(0x1f8804)?,
            light_angles: [exe.addiu(0x1f8834)?, exe.addiu(0x1f8840)?, 0],
            light_base_direction: exe.qword(0x2922c0)?,
            simon_eye_z: exe.float_lui(0x23a5d0)?,
            simon_pan_scale: exe.float_lui(0x23a5ec)?,
            simon_pan_divisor: exe.float_lui(0x23a5f4)?,
            simon_target_y: exe.float_lui_ori(0x23a620, 0x23a624)?,
            simon_level_scale: exe.float_lui(0x23a600)?,
            simon_level_divisor: exe.float_lui(0x23a60c)?,
            two: exe.float_lui(0x20e1b4)?,
            frustum_y_slope: exe.float_lui_ori(0x20e634, 0x20e648)?,
            frustum_x_slope: exe.float_lui_ori(0x20e738, 0x20e73c)?,
            frustum_x_slope_squared: exe.float_lui_ori(0x20da38, 0x20da3c)?,
            frustum_four: exe.float_lui(0x20da48)?,
            colour_multiplier: exe.float_lui_ori(0x1e8ddc, 0x1e8e10)?,
            colour_multiplier_blue: exe.float_lui(0x1e8de4)?,
            radius_scale: exe.float_lui_ori(0x1efc6c, 0x1efc70)?,
            sine_table,
            normal_scale: exe.qword(0x294440)?,
            scale_one: exe.qword(0x294450)?,
            zero_vector: exe.qword(0x294460)?,
        })
    }

    /// `0x20ACC0`: sine of a 16-bit angle (65,536 per turn) from the table,
    /// with the game's interpolation arithmetic.
    pub fn sin(&self, angle: i32) -> Result<f32> {
        let mut a = i32::from(angle as u16 as i16);
        let mut sign = 1.0f32;
        if a < 0 {
            sign = -1.0;
            a = -a;
        }
        if a >= 0x4001 {
            a = 0x8000 - a;
        }
        if a & 0x3fff == 0 {
            return Ok(if a == 0x4000 { sign } else { 0.0 });
        }
        let index = (a >> 4) as usize;
        let fraction = a & 0xf;
        let t0 = self.sine_table[index];
        let t1 = self.sine_table[index + 1];
        let acc = ee::mul(t0, ee::from_i32(16 - fraction)?)?;
        let value = ee::madd(acc, t1, ee::from_i32(fraction)?)?;
        ee::div(ee::mul(sign, value)?, 16.0)
    }
    /// `0x20ABF0`: cosine as the sine of `angle + 0x4000`.
    pub fn cos(&self, angle: i32) -> Result<f32> {
        self.sin(i32::from((angle + 0x4000) as u16 as i16))
    }

    /// `0x20BDA0` / `0x20C0B0`: rotation matrix from three 16-bit angles.
    pub fn rotation(&self, angles: [i32; 3]) -> Result<Matrix> {
        let (cx, sx) = (self.cos(angles[0])?, self.sin(angles[0])?);
        let (cy, sy) = (self.cos(angles[1])?, self.sin(angles[1])?);
        let (cz, sz) = (self.cos(angles[2])?, self.sin(angles[2])?);
        let m00 = ee::mul(cy, cz)?;
        let sxsy = ee::mul(sx, sy)?;
        let m10 = ee::msub(ee::mul(cz, sxsy)?, cx, sz)?;
        let cxsy = ee::mul(cx, sy)?;
        let m20 = ee::madd(ee::mul(cz, cxsy)?, sx, sz)?;
        let m01 = ee::mul(cy, sz)?;
        let m11 = ee::madd(ee::mul(sz, sxsy)?, cx, cz)?;
        let m21 = ee::msub(ee::mul(sz, cxsy)?, sx, cz)?;
        let m02 = ee::neg(sy);
        let m12 = ee::mul(sx, cy)?;
        let m22 = ee::mul(cx, cy)?;
        Ok([
            [m00, m01, m02, 0.0],
            [m10, m11, m12, 0.0],
            [m20, m21, m22, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }
}

// ---------------------------------------------------------------------------
// VU0 macro matrix helpers
// ---------------------------------------------------------------------------

fn vu_lane_mul(a: &Qword, s: f32) -> Result<Qword> {
    Ok([
        vu::mul(a[0], s)?,
        vu::mul(a[1], s)?,
        vu::mul(a[2], s)?,
        vu::mul(a[3], s)?,
    ])
}
fn vu_lane_madd(acc: &Qword, a: &Qword, s: f32) -> Result<Qword> {
    Ok([
        vu::madd(acc[0], a[0], s)?,
        vu::madd(acc[1], a[1], s)?,
        vu::madd(acc[2], a[2], s)?,
        vu::madd(acc[3], a[3], s)?,
    ])
}
/// `MULAx/MADDAy/MADDAz/MADDw`: `Σ_k a[k] · v[k]` in lane order x, y, z, w.
fn vu_mat_vec(a: &Matrix, v: &Qword) -> Result<Qword> {
    let acc = vu_lane_mul(&a[0], v[0])?;
    let acc = vu_lane_madd(&acc, &a[1], v[1])?;
    let acc = vu_lane_madd(&acc, &a[2], v[2])?;
    vu_lane_madd(&acc, &a[3], v[3])
}
/// `0x20B370`: `a × b` column by column.
fn vu_mat_mul(a: &Matrix, b: &Matrix) -> Result<Matrix> {
    Ok([
        vu_mat_vec(a, &b[0])?,
        vu_mat_vec(a, &b[1])?,
        vu_mat_vec(a, &b[2])?,
        vu_mat_vec(a, &b[3])?,
    ])
}
fn identity() -> Matrix {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

// ---------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------

/// `0x20C7E0`: divide a vector by its length (software sqrt, EE division).
fn normalize(v: [f32; 3]) -> Result<[f32; 3]> {
    let acc = ee::add(ee::mul(v[0], v[0])?, ee::mul(v[1], v[1])?)?;
    let length = sqrtf(ee::madd(acc, v[2], v[2])?)?;
    Ok([
        ee::div(v[0], length)?,
        ee::div(v[1], length)?,
        ee::div(v[2], length)?,
    ])
}

/// `0x20B550`: general 4×4 inverse by cofactors in the executable's exact
/// operation order (each entry is `±(cofactor / det)` with a nearest-rounded
/// division; the negations pass through the soft-float double path unchanged).
pub fn invert(m: &Matrix) -> Result<Matrix> {
    let (mul, add, sub, madd, msub, div) = (ee::mul, ee::add, ee::sub, ee::madd, ee::msub, ee::div);
    let (m0x, m0y, m0z, m0w) = (m[0][0], m[0][1], m[0][2], m[0][3]);
    let (m1x, m1y, m1z, m1w) = (m[1][0], m[1][1], m[1][2], m[1][3]);
    let (m2x, m2y, m2z, m2w) = (m[2][0], m[2][1], m[2][2], m[2][3]);
    let (m3x, m3y, m3z, m3w) = (m[3][0], m[3][1], m[3][2], m[3][3]);
    // Determinant (0x20B5CC..0x20B704).
    let f12 = mul(m1z, m2w)?;
    let f4 = mul(m1x, m2w)?;
    let f6 = mul(m1z, m2x)?;
    let f1 = mul(m1y, m2z)?;
    let f11 = madd(mul(m3w, f1)?, m3y, f12)?;
    let f3 = mul(m1w, m2y)?;
    let f0a = mul(m3x, f1)?;
    let f14 = mul(m1w, m2z)?;
    let f5 = mul(m1x, m2y)?;
    let f10 = mul(m3z, f3)?;
    let f7 = mul(m1z, m2y)?;
    let acc = add(f10, f11)?;
    let f10 = mul(m3w, f7)?;
    let f8 = mul(m1y, m2x)?;
    let f13 = msub(acc, m3y, f14)?;
    let acc = sub(f13, f10)?;
    let f12 = mul(m3x, f12)?;
    let f9 = mul(m1x, m2z)?;
    let f13 = mul(m3w, f9)?;
    let f2 = mul(m1y, m2w)?;
    let f10 = mul(m3x, f7)?;
    let f1 = mul(m1w, m2x)?;
    let f13 = add(f13, f12)?;
    let f12 = mul(m3z, f1)?;
    let f11 = mul(m3x, f14)?;
    let f12 = add(f12, f13)?;
    let f12 = sub(f12, f11)?;
    let f11 = mul(m3y, f1)?;
    let f1 = mul(m3w, f6)?;
    let f12 = sub(f12, f1)?;
    let f1 = mul(m3z, f4)?;
    let cof_a = sub(f12, f1)?; // sp+0x68
    let cof_b = msub(acc, m3z, f2)?; // f7
    let f1 = msub(mul(m0x, cof_b)?, m0y, cof_a)?;
    let f2 = madd(mul(m3w, f5)?, m3x, f2)?;
    let acc = add(f11, f2)?;
    let f2 = mul(m3z, f5)?;
    let f6 = mul(m3y, f6)?;
    let f0 = add(f2, f0a)?;
    let f0 = add(f6, f0)?;
    let f2 = sub(f0, f10)?;
    let f0 = mul(m3w, f8)?;
    let f3 = msub(acc, m3x, f3)?;
    let acc = sub(f3, f0)?;
    let cof_d = msub(acc, m3y, f4)?; // sp+0x6c
    let f0 = mul(m3z, f8)?;
    let f0 = sub(f2, f0)?;
    let f9 = mul(m3y, f9)?;
    let cof_e = sub(f0, f9)?; // sp+0x70
    let f0 = mul(m0z, cof_d)?;
    let acc = add(f0, f1)?;
    let det = msub(acc, m0w, cof_e)?;
    if det == 0.0 {
        return Err(invalid("singular matrix"));
    }
    // `(float)(-(double)x)` is an exact sign flip.
    let entry = |numerator: f32, negate: bool| -> Result<f32> {
        let q = div(numerator, det)?;
        Ok(if negate { -q } else { q })
    };
    let mut out = [[0.0f32; 4]; 4];
    out[0][0] = entry(cof_b, false)?;
    out[1][0] = entry(cof_a, true)?;
    out[2][0] = entry(cof_d, false)?;
    out[3][0] = entry(cof_e, true)?;
    // sp+0xd4
    let sp90 = mul(m0y, m2z)?;
    let acc = mul(m3w, sp90)?;
    let sp78 = mul(m0z, m2w)?;
    let f1 = madd(acc, m3y, sp78)?;
    let sp88 = mul(m0w, m2y)?;
    let f0 = mul(m3z, sp88)?;
    let acc = add(f0, f1)?;
    let sp7c = mul(m0w, m2z)?;
    let f1 = msub(acc, m3y, sp7c)?;
    let sp9c = mul(m0z, m2y)?;
    let f0 = mul(m3w, sp9c)?;
    let acc = sub(f1, f0)?;
    let sp80 = mul(m0y, m2w)?;
    let f0 = msub(acc, m3z, sp80)?;
    out[0][1] = entry(f0, true)?;
    // sp+0xe4
    let spa4 = mul(m0x, m2z)?;
    let acc = mul(m3w, spa4)?;
    let f1 = madd(acc, m3x, sp78)?;
    let sp84 = mul(m0w, m2x)?;
    let f0 = mul(m3z, sp84)?;
    let acc = add(f0, f1)?;
    let f1 = msub(acc, m3x, sp7c)?;
    let sp98 = mul(m0z, m2x)?;
    let f0 = mul(m3w, sp98)?;
    let acc = sub(f1, f0)?;
    let sp8c = mul(m0x, m2w)?;
    let f0 = msub(acc, m3z, sp8c)?;
    out[1][1] = entry(f0, false)?;
    // sp+0xf4
    let f1 = mul(m3y, sp84)?;
    let sp94 = mul(m0x, m2y)?;
    let acc = mul(m3w, sp94)?;
    let f0 = madd(acc, m3x, sp80)?;
    let acc = add(f1, f0)?;
    let f1 = msub(acc, m3x, sp88)?;
    let spa0 = mul(m0y, m2x)?;
    let f0 = mul(m3w, spa0)?;
    let acc = sub(f1, f0)?;
    let f0 = msub(acc, m3y, sp8c)?;
    out[2][1] = entry(f0, true)?;
    // sp+0x104
    let acc = mul(m3z, sp94)?;
    let f1 = madd(acc, m3x, sp90)?;
    let f0 = mul(m3y, sp98)?;
    let acc = add(f0, f1)?;
    let f1 = msub(acc, m3x, sp9c)?;
    let f0 = mul(m3z, spa0)?;
    let acc = sub(f1, f0)?;
    let f0 = msub(acc, m3y, spa4)?;
    out[3][1] = entry(f0, false)?;
    // sp+0xd8
    let spc0 = mul(m0y, m1z)?;
    let acc = mul(m3w, spc0)?;
    let spa8 = mul(m0z, m1w)?;
    let f1 = madd(acc, m3y, spa8)?;
    let spb8 = mul(m0w, m1y)?;
    let f0 = mul(m3z, spb8)?;
    let acc = add(f0, f1)?;
    let spac = mul(m0w, m1z)?;
    let f2 = msub(acc, m3y, spac)?;
    let spc8 = mul(m0z, m1y)?;
    let f0 = mul(m3w, spc8)?;
    let acc = sub(f2, f0)?;
    let spb0 = mul(m0y, m1w)?;
    let f0 = msub(acc, m3z, spb0)?;
    out[0][2] = entry(f0, false)?;
    // sp+0xe8
    let spcc = mul(m0x, m1z)?;
    let acc = mul(m3w, spcc)?;
    let spb4 = mul(m0w, m1x)?;
    let spc4 = mul(m0z, m1x)?;
    let spbc = mul(m0x, m1w)?;
    let f1 = madd(acc, m3x, spa8)?;
    let f0 = mul(m3z, spb4)?;
    let acc = add(f0, f1)?;
    let f1 = msub(acc, m3x, spac)?;
    let f0 = mul(m3w, spc4)?;
    let acc = sub(f1, f0)?;
    let f0 = msub(acc, m3z, spbc)?;
    out[1][2] = entry(f0, true)?;
    // sp+0xf8
    let f29 = mul(m0x, m1y)?;
    let acc = mul(m3w, f29)?;
    let f28 = mul(m0y, m1x)?;
    let f1 = mul(m3w, f28)?;
    let f2 = madd(acc, m3x, spb0)?;
    let f0 = mul(m3y, spb4)?;
    let acc = add(f0, f2)?;
    let f0 = msub(acc, m3x, spb8)?;
    let acc = sub(f0, f1)?;
    let f0 = msub(acc, m3y, spbc)?;
    out[2][2] = entry(f0, false)?;
    // sp+0x108
    let acc = mul(m3z, f29)?;
    let f1 = mul(m3z, f28)?;
    let f2 = madd(acc, m3x, spc0)?;
    let f0 = mul(m3y, spc4)?;
    let acc = add(f0, f2)?;
    let f0 = msub(acc, m3x, spc8)?;
    let acc = sub(f0, f1)?;
    let f0 = msub(acc, m3y, spcc)?;
    out[3][2] = entry(f0, true)?;
    // sp+0xdc
    let acc = mul(m2w, spc0)?;
    let f1 = madd(acc, m2y, spa8)?;
    let f0 = mul(m2z, spb8)?;
    let acc = add(f0, f1)?;
    let f1 = msub(acc, m2y, spac)?;
    let f0 = mul(m2w, spc8)?;
    let acc = sub(f1, f0)?;
    let f0 = msub(acc, m2z, spb0)?;
    out[0][3] = entry(f0, true)?;
    // sp+0xec
    let acc = mul(m2w, spcc)?;
    let f1 = madd(acc, m2x, spa8)?;
    let f0 = mul(m2z, spb4)?;
    let acc = add(f0, f1)?;
    let f1 = msub(acc, m2x, spac)?;
    let f0 = mul(m2w, spc4)?;
    let acc = sub(f1, f0)?;
    let f0 = msub(acc, m2z, spbc)?;
    out[1][3] = entry(f0, false)?;
    // sp+0xfc
    let acc = mul(m2w, f29)?;
    let f1 = madd(acc, m2x, spb0)?;
    let f0 = mul(m2y, spb4)?;
    let acc = add(f0, f1)?;
    let f1 = msub(acc, m2x, spb8)?;
    let f0 = mul(m2w, f28)?;
    let acc = sub(f1, f0)?;
    let f0 = msub(acc, m2y, spbc)?;
    out[2][3] = entry(f0, true)?;
    // sp+0x10c
    let acc = mul(m2z, f29)?;
    let f1 = mul(m2z, f28)?;
    let f2 = madd(acc, m2x, spc0)?;
    let f0 = mul(m2y, spc4)?;
    let acc = add(f0, f2)?;
    let f0 = msub(acc, m2x, spc8)?;
    let acc = sub(f0, f1)?;
    let f0 = msub(acc, m2y, spcc)?;
    out[3][3] = entry(f0, false)?;
    Ok(out)
}

/// The camera slot's matrices (0x2ED720): view·projection, view, projection.
#[derive(Clone, Debug, Serialize)]
pub struct CameraMatrices {
    pub eye: Qword,
    pub target: Qword,
    pub forward: Qword,
    pub view: Matrix,
    pub projection: Matrix,
    pub view_projection: Matrix,
    /// Frustum normalisers of the slot (+0x188, +0x18C, +0x190).
    pub frustum_norm: [f32; 3],
}

impl CameraMatrices {
    pub fn simon(c: &SceneConstants, pose: SimonPose) -> Result<Self> {
        if !(0..=100).contains(&pose.pan) || !(0..=0x1000).contains(&pose.level) {
            return Err(unsupported("pan must be 0..=100 and level 0..=4096"));
        }
        // 0x23A5C4: eye and target.
        let eye = [0.0, 0.0, c.simon_eye_z, 0.0];
        let tx = ee::div(
            ee::mul(c.simon_pan_scale, ee::from_i32(i32::from(pose.pan))?)?,
            c.simon_pan_divisor,
        )?;
        let ty = ee::add(
            c.simon_target_y,
            ee::div(
                ee::mul(c.simon_level_scale, ee::from_i32(i32::from(pose.level))?)?,
                c.simon_level_divisor,
            )?,
        )?;
        let target = [tx, ty, 0.0, 0.0];
        // 0x1FB1F0: forward = normalize(target - eye).
        let d = [
            ee::sub(target[0], eye[0])?,
            ee::sub(target[1], eye[1])?,
            ee::sub(target[2], eye[2])?,
        ];
        let f = normalize(d)?;
        let forward = [f[0], f[1], f[2], 0.0];
        // 0x20CD50: camera basis from the forward vector (world up +Y).
        let (fx, fy, fz) = (f[0], f[1], f[2]);
        let r = [fz, 0.0, ee::neg(fx)];
        let u = [
            ee::mul(ee::neg(fy), fx)?,
            ee::madd(ee::mul(fx, fx)?, fz, fz)?,
            ee::mul(ee::neg(fy), fz)?,
        ];
        let rl = sqrtf(ee::madd(
            ee::add(ee::mul(r[0], r[0])?, ee::mul(r[1], r[1])?)?,
            r[2],
            r[2],
        )?)?;
        let r = [ee::div(r[0], rl)?, ee::div(r[1], rl)?, ee::div(r[2], rl)?];
        let ul = sqrtf(ee::madd(
            ee::add(ee::mul(u[0], u[0])?, ee::mul(u[1], u[1])?)?,
            u[2],
            u[2],
        )?)?;
        let u = [ee::div(u[0], ul)?, ee::div(u[1], ul)?, ee::div(u[2], ul)?];
        let fl = sqrtf(ee::madd(
            ee::add(ee::mul(fx, fx)?, ee::mul(fy, fy)?)?,
            fz,
            fz,
        )?)?;
        let fw = [ee::div(fx, fl)?, ee::div(fy, fl)?, ee::div(fz, fl)?];
        let camera: Matrix = [
            [ee::neg(r[0]), ee::neg(r[1]), ee::neg(r[2]), 0.0],
            [ee::neg(u[0]), ee::neg(u[1]), ee::neg(u[2]), 0.0],
            [fw[0], fw[1], fw[2], 0.0],
            [eye[0], eye[1], eye[2], 1.0],
        ];
        let roll = c.rotation([0, 0, 0])?; // roll angle 0
        let view = invert(&vu_mat_mul(&camera, &roll)?)?;
        // 0x20D050 through 0x20E160.
        let projection = projection(c)?;
        let view_projection = vu_mat_mul(&projection, &view)?;
        // 0x20D9D0: frustum normalisers.
        let focal2 = ee::mul(c.camera_focal, c.camera_focal)?;
        let n0 = ee::add(c.frustum_x_slope_squared, focal2)?;
        let side = ee::add(
            ee::div(ee::mul(c.viewport, c.viewport)?, c.frustum_four)?,
            focal2,
        )?;
        Ok(Self {
            eye,
            target,
            forward,
            view,
            projection,
            view_projection,
            frustum_norm: [n0, side, side],
        })
    }
}

/// `0x20E160` + `0x20D050`: the GS projection for the display mode.
fn projection(c: &SceneConstants) -> Result<Matrix> {
    let centre_x = ee::sub(
        ee::add(c.gs_center, c.viewport_offset)?,
        ee::div(c.viewport, c.two)?,
    )?;
    let centre_y = ee::sub(
        ee::add(c.gs_center, c.viewport_offset)?,
        ee::div(c.viewport, c.two)?,
    )?;
    let scale_y = ee::mul(c.camera_scale_y, c.display_aspect)?;
    let far = ee::mul(c.two, c.camera_far)?;
    let z_near_value = 0.0f32; // 0x2094D0 stores 0 for the minimum depth
    let mut m = identity();
    m[0][0] = c.camera_focal;
    m[1][1] = c.camera_focal;
    m[2][2] = 0.0;
    m[3][3] = 0.0;
    m[3][2] = 1.0;
    m[2][3] = 1.0;
    let mut p = identity();
    p[0][0] = c.camera_scale_x;
    p[1][1] = scale_y;
    let f0 = ee::add(ee::neg(z_near_value), c.z_max)?;
    let f1 = ee::mul(ee::mul(far, c.camera_near)?, f0)?;
    let acc = ee::mul(ee::neg(c.z_max), c.camera_near)?;
    let f2 = ee::add(ee::neg(c.camera_near), far)?;
    p[2][2] = ee::div(f1, f2)?;
    p[3][0] = centre_x;
    let f0 = ee::madd(acc, z_near_value, far)?;
    p[3][1] = centre_y;
    p[3][2] = ee::div(f0, f2)?;
    vu_mat_mul(&p, &m)
}

// ---------------------------------------------------------------------------
// Lights and fog
// ---------------------------------------------------------------------------

/// VU qwords 63..65 (light rows) and 83 (global fog) plus the per-draw fog
/// override.
#[derive(Clone, Debug, Serialize)]
pub struct Lighting {
    pub light_direction_world: Qword,
    pub light_rows: [Qword; 3],
    pub fog_near: f32,
    pub fog_far: f32,
    pub fog_colour: [u8; 3],
    pub fog_qword83: Qword,
    pub draw_fog_vif83: Qword,
}

impl Lighting {
    pub fn derive(c: &SceneConstants, camera: &CameraMatrices) -> Result<Self> {
        // 0x1FA7F0: direction = R(angles) · (0,0,1,0) through the EE mat×vec
        // path, which reduces exactly to the third column.
        let rot = c.rotation(c.light_angles)?;
        let b = c.light_base_direction;
        let dir = [
            ee_mat_vec_lane(&rot, &b, 0)?,
            ee_mat_vec_lane(&rot, &b, 1)?,
            ee_mat_vec_lane(&rot, &b, 2)?,
            ee_mat_vec_lane(&rot, &b, 3)?,
        ];
        // 0x1F9F40: view-transformed directions of the three sub-lights, then
        // 0x208650 multiplies the colour block by the direction rows.
        let d0 = vu_mat_vec(&camera.view, &dir)?;
        let d1 = vu_mat_vec(&camera.view, &[0.0, 0.0, 1.0, 0.0])?;
        let d2 = d1;
        let colours: Matrix = [
            [c.light_colour, c.light_colour, c.light_colour, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
        ];
        let directions: Matrix = [
            [d0[0], d1[0], d2[0], 0.0],
            [d0[1], d1[1], d2[1], 0.0],
            [d0[2], d1[2], d2[2], 0.0],
            [0.0, 0.0, 0.0, 0.0],
        ];
        let rows = vu_mat_mul(&colours, &directions)?;
        // 0x208B30 with (near, far, near, far).
        let scale = ee::div(c.fog_255, ee::sub(c.fog_far, c.fog_near)?)?;
        let fog_qword83 = [
            scale,
            ee::mul(c.fog_near, scale)?,
            scale,
            ee::mul(c.fog_far, scale)?,
        ];
        Ok(Self {
            light_direction_world: dir,
            light_rows: [rows[0], rows[1], rows[2]],
            fog_near: c.fog_near,
            fog_far: c.fog_far,
            fog_colour: [255, 255, 255],
            fog_qword83,
            // 0x208BE0 with fog disabled for the light set: (0, 0, 0, 255).
            draw_fog_vif83: [0.0, 0.0, 0.0, c.fog_255],
        })
    }
}

/// `0x20C540` with a matrix argument (EE FPU path): lane `i` of `m · v`.
fn ee_mat_vec_lane(m: &Matrix, v: &Qword, lane: usize) -> Result<f32> {
    let acc = ee::mul(v[1], m[1][lane])?;
    let t = ee::madd(acc, v[0], m[0][lane])?;
    let acc = ee::add(ee::mul(v[2], m[2][lane])?, t)?;
    ee::madd(acc, v[3], m[3][lane])
}

// ---------------------------------------------------------------------------
// World models
// ---------------------------------------------------------------------------

/// A MOT animation block trailing an RH0 record.
#[derive(Clone, Debug, Serialize)]
pub struct Motion {
    pub archive_offset: u64,
    pub byte_len: u32,
    pub sha256: String,
    pub node_count: u16,
    pub frame_count: u32,
    pub flags: u16,
    pub tracks: Vec<MotionNodeTracks>,
}
#[derive(Clone, Debug, Serialize)]
pub struct MotionNodeTracks {
    pub translation: Vec<[i32; 4]>,
    pub rotation: Vec<[i32; 4]>,
    pub scale: Vec<[i32; 4]>,
}

impl Motion {
    pub fn parse(bytes: &[u8], archive_offset: u64) -> Result<Self> {
        let word = |at: usize| -> Result<u32> {
            bytes
                .get(at..at + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .ok_or_else(|| invalid("MOT block truncated"))
        };
        if bytes.get(..4) != Some(b"MOT\x1a") {
            return Err(invalid("missing MOT magic"));
        }
        let len = word(4)? as usize;
        if len != bytes.len() || len > MAX_MOT_BYTES || len < 0x50 {
            return Err(invalid("MOT length"));
        }
        let node_count = word(0x10)? as u16;
        if word(0x14)? != 0x20 || word(0x20)? != 0x30 {
            return Err(unsupported("MOT header layout"));
        }
        let frame_count = word(0x24)?;
        let flags = word(0x28)? as u16;
        let entry_len = if flags & 4 != 0 { 24 } else { 16 };
        let mut tracks = Vec::new();
        for node in 0..node_count as usize {
            let entry = 0x30 + node * entry_len;
            let track = |ptr_at: usize, count_at: usize| -> Result<Vec<[i32; 4]>> {
                let ptr = word(ptr_at)? as usize;
                let count = word(count_at)? as usize;
                if ptr == 0 {
                    return Ok(Vec::new());
                }
                if count > 4096 || ptr + count * 16 > len {
                    return Err(invalid("MOT track exceeds the block"));
                }
                (0..count)
                    .map(|k| {
                        Ok([
                            word(ptr + k * 16)? as i32,
                            word(ptr + k * 16 + 4)? as i32,
                            word(ptr + k * 16 + 8)? as i32,
                            word(ptr + k * 16 + 12)? as i32,
                        ])
                    })
                    .collect()
            };
            tracks.push(MotionNodeTracks {
                translation: track(entry, entry + 8)?,
                rotation: track(entry + 4, entry + 12)?,
                scale: if flags & 4 != 0 {
                    track(entry + 8, entry + 16)?
                } else {
                    Vec::new()
                },
            });
        }
        Ok(Self {
            archive_offset,
            byte_len: len as u32,
            sha256: format!("{:x}", Sha256::digest(bytes)),
            node_count,
            frame_count,
            flags,
            tracks,
        })
    }
}

/// Play-mode of an object's primary animation layer: bits 6..7 of byte
/// +0x1F7, `1` (forward) from the object init `0x1E8DF0`.
pub const ANIMATION_FORWARD_STEP_DEFAULT: u8 = 1;

/// `0x1EE6E0` (story update loop `0x21DC1C`, once per object and update,
/// right after its draw) for the forward play mode: with `end = frame_count −
/// 1`, a frame below `end` advances by `speed · step` but never past `end`,
/// and a frame at (or beyond) `end` restarts at 1.0. `step` is the global
/// byte 0x2DAB86 (1 after boot, `0x1A7580`; setter `0x1A90C0`), `speed` the
/// layer's +0x198 (1.0 from the object init). Replayed bit-exactly against the
/// saved PAL image (3 → 4, 499 → 500, 500 → 1, step 2: 499 → 500).
pub fn advance_animation_frame(frame: f32, speed: f32, step: u8, frame_count: u32) -> Result<f32> {
    if frame_count == 0 {
        return Err(unsupported("animation without frames"));
    }
    let end = ee::sub(key_time(frame_count as i32)?, 1.0)?;
    let delta = ee::mul(speed, ee::from_i32(i32::from(step))?)?;
    if frame < end {
        let next = ee::add(frame, delta)?;
        Ok(if next <= end { next } else { end })
    } else {
        Ok(1.0)
    }
}

impl Motion {
    /// The frame counter after `updates` story updates from `start` (1.0 at
    /// object creation) under the forward play mode.
    pub fn frame_after(&self, start: f32, updates: u64, speed: f32, step: u8) -> Result<f32> {
        let mut frame = start;
        for _ in 0..updates {
            frame = advance_animation_frame(frame, speed, step, self.frame_count)?;
        }
        Ok(frame)
    }
}

/// Key time as the game converts it: unsigned int → float.
fn key_time(t: i32) -> Result<f32> {
    if t >= 0 {
        ee::from_i32(t)
    } else {
        let half = ((t as u32) >> 1) | (t as u32 & 1);
        ee::add(ee::from_i32(half as i32)?, ee::from_i32(half as i32)?)
    }
}

/// `0x210480` track lookup: the game's bisection over key times, returning
/// the key index and, when one follows, the next key to interpolate towards.
/// Fewer than three keys use the first key unchanged.
fn track_segment(keys: &[[i32; 4]], frame: f32) -> Result<(usize, Option<usize>)> {
    let count = keys.len();
    if count < 3 {
        return Ok((0, None));
    }
    let (mut lo, mut hi, mut t0) = (0usize, count, count >> 1);
    if t0 > 0 {
        loop {
            // The game reads key[t0 + 1] even past the array (the sky's
            // track ends with its MOT block, at frame 500). At the last key
            // with the frame at or after its time both branches end on t0
            // (frame < next: break; else lo = t0 = (lo + hi) >> 1: break),
            // whatever the word read; any other case is refused.
            let Some(next) = keys.get(t0 + 1) else {
                if frame >= key_time(keys[t0][0])? {
                    break;
                }
                return Err(unsupported("animation frame beyond the last keyframe"));
            };
            let next_time = key_time(next[0])?;
            if frame < next_time {
                if frame < key_time(keys[t0][0])? {
                    hi = t0;
                } else {
                    break;
                }
            } else {
                lo = t0;
            }
            t0 = (lo + hi) >> 1;
            if lo >= t0 {
                break;
            }
        }
    }
    Ok((t0, if t0 + 1 < count { Some(t0 + 1) } else { None }))
}

/// `0x20C940`: interpolate two 16-bit angles with the given weights.
fn lerp_angle(a: i32, b: i32, wa: f32, wb: f32) -> Result<i32> {
    let (a16, mut b16) = ((a & 0xffff) as i32, (b & 0xffff) as i32);
    let diff = b16 - a16;
    if diff >= 0x8001 {
        b16 -= 0x10000;
    } else if diff < -0x8000 {
        b16 += 0x10000;
    }
    let acc = ee::mul(ee::from_i32(a16)?, wa)?;
    let v = ee::madd(acc, ee::from_i32(b16)?, wb)?;
    ee::to_i32(ee::add(0.5, v)?)
}

/// Placement of one world model in the scene.
#[derive(Clone, Debug, Serialize)]
pub struct WorldModel {
    pub slot: u16,
    pub object_id: u16,
    pub pack_entry: usize,
    pub archive_offset: u64,
    pub record_bytes: u32,
    pub record_sha256: String,
    pub geometry_sha256: String,
    #[serde(skip)]
    pub model: StoryModel,
    pub motion: Option<Motion>,
    /// Frustum classification (0x20E550) of the bounding sphere.
    pub visibility_flags: u32,
    pub view_space: bool,
    pub vu_program_entry: u32,
    /// The 7-qword matrix block the VU receives: position matrix then the
    /// three normal rows.
    pub emitted_matrix: Matrix,
    pub normal_rows: [Qword; 3],
    pub emitted_matrix_sha256: String,
    pub material_multipliers: [f32; 3],
    pub bounding_radius: f32,
    pub sky_rotation_angles: Option<[i32; 3]>,
}

struct Walk<'a> {
    c: &'a SceneConstants,
    current: Matrix,
    normals: [Qword; 3],
    stack: Vec<(Matrix, [Qword; 3])>,
    table: BTreeMap<u16, (Matrix, [Qword; 3])>,
    motion: Option<&'a Motion>,
    frame: f32,
    translation_unit: f32,
    angles_used: Option<[i32; 3]>,
}

impl<'a> Walk<'a> {
    /// `0x20D1E0`: normal rows from the current matrix's first three columns.
    fn compute_normals(&mut self) -> Result<()> {
        let scale = self.c.normal_scale[0];
        for i in 0..3 {
            let col = self.current[i];
            let sq = [
                vu::mul(col[0], col[0])?,
                vu::mul(col[1], col[1])?,
                vu::mul(col[2], col[2])?,
            ];
            let len2 = vu::add(vu::add(sq[0], sq[1])?, sq[2])?;
            let q = vu::rsqrt(scale, len2)?;
            self.normals[i] = [
                vu::mul(col[0], q)?,
                vu::mul(col[1], q)?,
                vu::mul(col[2], q)?,
                0.0,
            ];
        }
        Ok(())
    }

    fn node(&self, model: &StoryModel, index: usize) -> Result<[u32; 16]> {
        model
            .raw_node_words
            .get(index)
            .copied()
            .ok_or_else(|| invalid("node index out of range"))
    }

    /// `0x20F7A0` / `0x2100D0`: recursive node walk with the accumulated scale.
    fn walk(
        &mut self,
        model: &StoryModel,
        index: usize,
        scale_in: Qword,
        depth: usize,
    ) -> Result<()> {
        if depth > 8 {
            return Err(unsupported("node hierarchy deeper than 8"));
        }
        let node = self.node(model, index)?;
        let mut scale = scale_in;
        let mut flags = (node[3] & 0xffff) as u16;
        let mut node_scale = [
            f32::from_bits(node[0]),
            f32::from_bits(node[1]),
            f32::from_bits(node[2]),
        ];
        let mut rotation = [node[4] as i32, node[5] as i32, node[6] as i32];
        let mut translation = [
            f32::from_bits(node[8]),
            f32::from_bits(node[9]),
            f32::from_bits(node[10]),
        ];
        if let Some(motion) = self.motion {
            if index < motion.node_count as usize {
                let tracks = &motion.tracks[index];
                let unit = if index == 0 {
                    1.0
                } else {
                    self.translation_unit
                };
                if !tracks.translation.is_empty() {
                    flags &= !1;
                    translation = eval_float_track(&tracks.translation, self.frame, unit)?;
                }
                if !tracks.rotation.is_empty() {
                    flags &= !2;
                    rotation = eval_angle_track(&tracks.rotation, self.frame)?;
                    if index == 0 {
                        self.angles_used = Some(rotation);
                    }
                }
                if !tracks.scale.is_empty() {
                    flags &= !4;
                    node_scale = eval_float_track(&tracks.scale, self.frame, unit)?;
                }
            }
        }
        let pushed = flags & 3 != 3;
        if pushed {
            let t = [
                ee::mul(translation[0], scale_in[0])?,
                ee::mul(translation[1], scale_in[1])?,
                ee::mul(translation[2], scale_in[2])?,
                self.c.zero_vector[3],
            ];
            self.stack.push((self.current, self.normals));
            // 0x20D350: translation (if flag bit 0 clear) then rotation (bit 1).
            if flags & 1 == 0 {
                let acc = vu_lane_mul(&self.current[0], t[0])?;
                let acc = vu_lane_madd(&acc, &self.current[1], t[1])?;
                let acc = vu_lane_madd(&acc, &self.current[2], t[2])?;
                self.current[3] = vu_lane_madd(&acc, &self.current[3], 1.0)?;
            }
            if flags & 2 == 0 {
                let r = self.c.rotation(rotation)?;
                let m = vu_mat_mul(&self.current, &r)?;
                self.current[0] = m[0];
                self.current[1] = m[1];
                self.current[2] = m[2];
                // The normal rows rotate the same way (vf24..26; the fourth
                // row vf27 only contributes a zero product because R.w = 0).
                let n: Matrix = [self.normals[0], self.normals[1], self.normals[2], [0.0; 4]];
                let rotated = vu_mat_mul(&n, &r)?;
                self.normals = [rotated[0], rotated[1], rotated[2]];
            }
        }
        if flags & 4 == 0 {
            scale = [
                ee::mul(scale[0], node_scale[0])?,
                ee::mul(scale[1], node_scale[1])?,
                ee::mul(scale[2], node_scale[2])?,
                scale[3],
            ];
        }
        let matrix_index = (node[3] >> 16) as u16;
        if matrix_index != 0 {
            // 0x20D270 then 0x20C290: the stored position matrix is scaled.
            let mut s = identity();
            s[0][0] = scale[0];
            s[1][1] = scale[1];
            s[2][2] = scale[2];
            let stored = vu_mat_mul(&self.current, &s)?;
            self.table.insert(matrix_index, (stored, self.normals));
        }
        let child = node[7];
        if child != 0 {
            if child % 64 != 0 {
                return Err(invalid("child node offset"));
            }
            self.walk(model, (child / 64) as usize, scale, depth + 1)?;
        }
        if pushed {
            let (m, n) = self.stack.pop().ok_or_else(|| invalid("matrix stack"))?;
            self.current = m;
            self.normals = n;
        }
        let sibling = node[11];
        if sibling != 0 {
            if sibling % 64 != 0 {
                return Err(invalid("sibling node offset"));
            }
            self.walk(model, (sibling / 64) as usize, scale_in, depth + 1)?;
        }
        Ok(())
    }
}

fn track_weights(keys: &[[i32; 4]], t0: usize, t1: usize, frame: f32) -> Result<(f32, f32)> {
    let dt = key_time(keys[t1][0] - keys[t0][0])?;
    let w0 = ee::div(ee::sub(key_time(keys[t1][0])?, frame)?, dt)?;
    let w1 = ee::div(ee::sub(frame, key_time(keys[t0][0])?)?, dt)?;
    Ok((w0, w1))
}

fn eval_float_track(keys: &[[i32; 4]], frame: f32, unit: f32) -> Result<[f32; 3]> {
    let (t0, t1) = track_segment(keys, frame)?;
    let value = |k: usize, lane: usize| f32::from_bits(keys[k][lane] as u32);
    let mut out = [0.0f32; 3];
    match t1 {
        None => {
            for lane in 0..3 {
                out[lane] = ee::mul(value(t0, lane + 1), unit)?;
            }
        }
        Some(t1) => {
            let (w0, w1) = track_weights(keys, t0, t1, frame)?;
            for lane in 0..3 {
                let v = ee::madd(ee::mul(w0, value(t0, lane + 1))?, w1, value(t1, lane + 1))?;
                out[lane] = ee::mul(v, unit)?;
            }
        }
    }
    Ok(out)
}

fn eval_angle_track(keys: &[[i32; 4]], frame: f32) -> Result<[i32; 3]> {
    let (t0, t1) = track_segment(keys, frame)?;
    match t1 {
        None => Ok([keys[t0][1], keys[t0][2], keys[t0][3]]),
        Some(t1) => {
            let (w0, w1) = track_weights(keys, t0, t1, frame)?;
            Ok([
                lerp_angle(keys[t0][1], keys[t1][1], w0, w1)?,
                lerp_angle(keys[t0][2], keys[t1][2], w0, w1)?,
                lerp_angle(keys[t0][3], keys[t1][3], w0, w1)?,
            ])
        }
    }
}

/// `0x20E550`: classify the view-space bounding sphere. Bit 0 = visible,
/// bit 1 = crosses a half-viewport plane, bit 2 = crosses the near/far plane
/// or one of the four steep planes (which also set bit 1). The emitter uses
/// bit 2 to select the clipping VU program.
fn frustum_flags(
    c: &SceneConstants,
    cam: &CameraMatrices,
    centre: &Qword,
    radius: f32,
) -> Result<u32> {
    let mut s0 = 0u32;
    let (x, y, z) = (centre[0], centre[1], centre[2]);
    let (near, far) = (c.camera_near, c.camera_far);
    let z_plus = ee::add(z, radius)?;
    if !(near <= z_plus) {
        return Ok(4);
    }
    let z_minus = ee::sub(z, radius)?;
    if !(near <= z_minus) {
        s0 = 4;
    }
    if far < z_minus {
        return Ok(4);
    }
    if far < z_plus {
        s0 = 4;
    }
    let span_y = ee::mul(c.viewport, ee::div(1.0, c.camera_scale_y)?)?;
    let span_x = ee::mul(c.viewport, ee::div(1.0, c.camera_scale_x)?)?;
    let focal_y = ee::mul(c.camera_focal, y)?;
    let steep_y = ee::mul(c.frustum_y_slope, z)?;
    let r2 = ee::mul(radius, radius)?;
    // Steep planes: a negative signed distance with the sphere outside the
    // plane rejects the object; otherwise crossing marks bits 1 and 2.
    let mut steep = |signed: f32| -> Result<bool> {
        let d2 = ee::div(ee::mul(signed, signed)?, cam.frustum_norm[0])?;
        if signed < 0.0 {
            if d2 < r2 {
                s0 = 6;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            if d2 <= r2 {
                s0 = 6;
            }
            Ok(true)
        }
    };
    if !steep(ee::sub(steep_y, focal_y)?)? || !steep(ee::add(steep_y, focal_y)?)? {
        return Ok(4);
    }
    let steep_x = ee::mul(c.frustum_x_slope, z)?;
    let focal_x = ee::mul(c.camera_focal, x)?;
    if !steep(ee::sub(steep_x, focal_x)?)? || !steep(ee::add(steep_x, focal_x)?)? {
        return Ok(4);
    }
    // Half-viewport planes: crossing marks bit 1; outside returns invisible.
    let mut side = |signed: f32, norm: f32| -> Result<bool> {
        let d2 = ee::div(ee::mul(signed, signed)?, norm)?;
        if signed < 0.0 {
            if d2 < r2 {
                s0 |= 2;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            if d2 <= r2 {
                s0 |= 2;
            }
            Ok(true)
        }
    };
    let half_y = ee::div(ee::mul(z, span_y)?, c.two)?;
    if !side(ee::sub(half_y, focal_y)?, cam.frustum_norm[2])?
        || !side(ee::add(half_y, focal_y)?, cam.frustum_norm[2])?
    {
        return Ok(s0 | 2);
    }
    let half_x = ee::div(ee::mul(z, span_x)?, c.two)?;
    if !side(ee::sub(half_x, focal_x)?, cam.frustum_norm[1])?
        || !side(ee::add(half_x, focal_x)?, cam.frustum_norm[1])?
    {
        return Ok(s0 | 2);
    }
    Ok(s0 | 1)
}

/// `0x208490`: VU program entry from the emitter flags (no fog variants here).
fn vu_entry(clipping: bool) -> u32 {
    if clipping {
        0x1a4
    } else {
        0x30
    }
}

fn matrix_sha256(m: &Matrix, n: &[Qword; 3]) -> String {
    let mut hasher = Sha256::new();
    for q in m.iter().chain(n.iter()) {
        for v in q {
            hasher.update(v.to_le_bytes());
        }
    }
    format!("{:x}", hasher.finalize())
}

/// The world pack of the opening story world, read from the executable.
#[derive(Clone, Debug, Serialize)]
pub struct WorldPack {
    pub kind: u32,
    pub sub_index: u32,
    pub pack_index: u32,
    pub archive_offset: u64,
    pub byte_len: u32,
    pub first_slot: u16,
    pub entry_offsets: Vec<u32>,
    pub object_ids: Vec<u16>,
}

impl WorldPack {
    pub fn story(exe: &Executable<'_>) -> Result<Self> {
        Self::at_call(exe, 0x21c0bc, 0x21c0c0)
    }

    /// The pack a `0x1E91B0(kind, sub, ..)` call loads, its two argument
    /// instructions at `kind_at` / `sub_at` (`addiu a0/a1, zero, n` or
    /// `por a1, zero, zero` for 0).
    pub fn at_call(exe: &Executable<'_>, kind_at: u32, sub_at: u32) -> Result<Self> {
        let kind = exe.addiu(kind_at)? as u32;
        let sub_index = if exe.u32(sub_at)? == 0x7000_2e28 {
            0
        } else {
            exe.addiu(sub_at)? as u32
        };
        if kind > 6 || sub_index > 5 {
            return Err(unsupported("story world pack arguments"));
        }
        let base = exe.u32(0x291e24 + kind * 8)? & 0xffff;
        let pack_index = base + sub_index;
        if pack_index >= 81 {
            return Err(unsupported("pack index"));
        }
        let start = exe.u32(0x291cd0 + pack_index * 4)?;
        let end = exe.u32(0x291cd0 + (pack_index + 1) * 4)?;
        if end <= start {
            return Err(unsupported("pack extent"));
        }
        let first_slot = exe.u32(0x291f20 + pack_index * 4)? as u16;
        let count = exe.u32(0x292060 + pack_index * 4)? as usize;
        let list = exe.u32(0x292100 + pack_index * 4)?;
        if count == 0 || count > MAX_PACK_ENTRIES || list == 0 {
            return Err(unsupported("pack entry list"));
        }
        let entry_offsets = (0..count)
            .map(|k| exe.u32(list + 4 * k as u32))
            .collect::<Result<Vec<_>>>()?;
        let id_count = exe.u32(0x291ec0 + kind * 4)? as usize;
        let ids_at = 0x2cf43c + 80 * sub_index;
        let object_ids = (0..id_count.min(40))
            .map(|k| exe.u16(ids_at + 2 * k as u32))
            .collect::<Result<Vec<_>>>()?;
        let id_limit = exe.u16(0x2d0090 + 7 * 2)?;
        if object_ids
            .iter()
            .take(count)
            .any(|&id| id != 999 && id >= id_limit)
        {
            return Err(unsupported("object id limit"));
        }
        Ok(Self {
            kind,
            sub_index,
            pack_index,
            archive_offset: u64::from(start),
            byte_len: end - start,
            first_slot,
            entry_offsets,
            object_ids,
        })
    }
}

/// One draw of a world model, ready for a renderer: raw source lanes plus the
/// derived colours and the copied material vectors.
#[derive(Clone, Debug, Serialize)]
pub struct DrawMesh {
    pub object_slot: u16,
    pub draw_index: u32,
    /// A second-chain draw (alpha-blended strips, PRIM ABE) [F: RH0 DMA 2].
    pub blend: bool,
    pub texture_index: usize,
    pub gs_registers_raw: BTreeMap<String, u64>,
    pub sampler_u_clamp: bool,
    pub sampler_v_clamp: bool,
    pub magnification_linear: bool,
    pub minification_code: u8,
    pub material_vectors_raw: [f32; 12],
    pub material_vectors_emitted: [f32; 12],
    pub positions_raw: Vec<[i16; 3]>,
    pub normals_raw: Vec<[i16; 3]>,
    pub vertex_rgba_gs: Vec<[u8; 4]>,
    pub uv: Vec<[f64; 2]>,
    pub indices: Vec<u32>,
}

/// `0x2119E0` with the object's colour multipliers (bit 7 of the draw flags).
fn emitted_material(raw: &[f32; 12], multipliers: [f32; 3]) -> Result<[f32; 12]> {
    let mut m = *raw;
    for lane in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
        if m[lane] < 0.0 {
            m[lane] = 0.0;
        }
    }
    for lane in [0, 1, 2, 4, 5, 6, 8, 9, 10] {
        if !(m[lane] <= 255.0) {
            m[lane] = 255.0;
        }
    }
    if m[4] != 0.0 {
        for lane in 0..3 {
            m[lane] = ee::mul(m[lane], multipliers[1])?;
        }
        for lane in 4..7 {
            m[lane] = ee::mul(m[lane], multipliers[0])?;
        }
        for lane in 8..11 {
            m[lane] = ee::mul(m[lane], multipliers[2])?;
        }
    }
    Ok(m)
}

/// The exporter's model of the VU1 main-path lighting (verified on 140 audited
/// vertices of the capture): integer 0..255 from ambient plus non-negative
/// diffuse, limited by the material maximum; alpha is the diffuse alpha lane.
fn vertex_colour(
    normal: [i16; 3],
    rows: &[Qword; 3],
    light: &[Qword; 3],
    material: &[f32; 12],
) -> [u8; 4] {
    let n: [f64; 3] = std::array::from_fn(|j| {
        (0..3)
            .map(|i| f64::from(normal[i]) * f64::from(rows[i][j]))
            .sum()
    });
    let intensity: [f64; 3] =
        std::array::from_fn(|j| (0..3).map(|i| n[i] * f64::from(light[i][j])).sum());
    let mut out = [0u8; 4];
    for j in 0..3 {
        let lit = f64::from(material[j]) + (intensity[j] * f64::from(material[4 + j])).max(0.0);
        let value = lit.min(f64::from(material[8 + j])) as i64;
        out[j] = value.clamp(0, 255) as u8;
    }
    out[3] = (f64::from(material[7]) as i64).clamp(0, 255) as u8;
    out
}

fn build_meshes(
    slot: u16,
    model: &StoryModel,
    placement: &WorldModel,
    light_rows: &[Qword; 3],
) -> Result<Vec<DrawMesh>> {
    let mut meshes = Vec::new();
    let mut triangles = 0usize;
    for (draw_index, draw) in model.draws.iter().enumerate() {
        let raw: [f32; 12] = std::array::from_fn(|i| f32::from_bits(draw.material_words[i]));
        let emitted = emitted_material(&raw, placement.material_multipliers)?;
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut colours = Vec::new();
        let mut uv = Vec::new();
        let mut indices = Vec::new();
        for strip in model
            .strips
            .iter()
            .filter(|s| usize::from(s.draw_index) == draw_index)
        {
            let base = positions.len() as u32;
            for v in &strip.vertices {
                positions.push(v.position_raw);
                normals.push(v.normal_raw);
                uv.push([
                    f64::from(v.texture_raw[0]) / 4096.0,
                    f64::from(v.texture_raw[1]) / 4096.0,
                ]);
                colours.push(vertex_colour(
                    v.normal_raw,
                    &placement.normal_rows,
                    light_rows,
                    &emitted,
                ));
            }
            for i in 2..strip.vertices.len() {
                let mut ids = [base + i as u32 - 2, base + i as u32 - 1, base + i as u32];
                if i % 2 == 1 {
                    ids.swap(0, 1);
                }
                let p: Vec<[f64; 3]> = ids
                    .iter()
                    .map(|&k| {
                        let q = positions[k as usize];
                        [f64::from(q[0]), f64::from(q[1]), f64::from(q[2])]
                    })
                    .collect();
                let u = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
                let w = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
                let cross = [
                    u[1] * w[2] - u[2] * w[1],
                    u[2] * w[0] - u[0] * w[2],
                    u[0] * w[1] - u[1] * w[0],
                ];
                if cross.iter().any(|c| *c != 0.0) {
                    indices.extend(ids);
                    triangles += 1;
                }
            }
        }
        if triangles > MAX_TRIANGLES {
            return Err(unsupported("triangle budget"));
        }
        let register = |id: u64| -> Result<u64> {
            draw.gs_registers
                .iter()
                .find(|r| r[1] == id)
                .map(|r| r[0])
                .ok_or_else(|| invalid("draw lacks a GS register"))
        };
        let clamp = register(8)?;
        let tex1 = register(0x14)?;
        meshes.push(DrawMesh {
            object_slot: slot,
            draw_index: draw_index as u32,
            blend: draw.pass == 1,
            texture_index: usize::from(draw.texture_index),
            gs_registers_raw: draw
                .gs_registers
                .iter()
                .map(|r| (r[1].to_string(), r[0]))
                .collect(),
            sampler_u_clamp: clamp & 3 != 0,
            sampler_v_clamp: (clamp >> 2) & 3 != 0,
            magnification_linear: tex1 & 32 != 0,
            minification_code: ((tex1 >> 6) & 7) as u8,
            material_vectors_raw: raw,
            material_vectors_emitted: emitted,
            positions_raw: positions,
            normals_raw: normals,
            vertex_rgba_gs: colours,
            uv,
            indices,
        });
    }
    Ok(meshes)
}

/// The whole derived scene.
#[derive(Clone, Debug, Serialize)]
pub struct SimonScene {
    pub pose: SimonPoseRecord,
    pub constants: SceneConstants,
    pub camera: CameraMatrices,
    pub lighting: Lighting,
    pub pack: WorldPack,
    pub models: Vec<WorldModel>,
    #[serde(skip)]
    pub meshes: Vec<DrawMesh>,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct SimonPoseRecord {
    pub pan: i8,
    pub level: i16,
    pub sky_frame: f32,
}

impl SimonScene {
    pub fn derive(executable: &[u8], archive: &[u8], pose: SimonPose) -> Result<Self> {
        Self::derive_world(executable, archive, pose, (0x21c0bc, 0x21c0c0))
    }

    /// The same derivation for the world pack loaded by the `0x1E91B0` call
    /// whose argument instructions are at `call` (kind, sub), e.g. the
    /// Stonehenge story events' "scene 0" (0x220888/0x22088C, event 11160).
    pub fn derive_world(
        executable: &[u8],
        archive: &[u8],
        pose: SimonPose,
        call: (u32, u32),
    ) -> Result<Self> {
        let exe = Executable::new(executable)?;
        let archive_sha256 = format!("{:x}", Sha256::digest(archive));
        if archive_sha256 != PAL_MODEL_ARCHIVE_SHA256 {
            return Err(unsupported(format!("MODEL.MRG SHA256 {archive_sha256}")));
        }
        let c = SceneConstants::read(&exe)?;
        let camera = CameraMatrices::simon(&c, pose)?;
        let lighting = Lighting::derive(&c, &camera)?;
        let pack = WorldPack::at_call(&exe, call.0, call.1)?;
        let pack_start = pack.archive_offset as usize;
        let pack_bytes = archive
            .get(pack_start..pack_start + pack.byte_len as usize)
            .ok_or_else(|| invalid("pack exceeds MODEL.MRG"))?;
        let multipliers = [
            ee::mul(c.colour_multiplier, c.colour_multiplier_blue)?,
            ee::mul(c.colour_multiplier, c.colour_multiplier_blue)?,
            c.colour_multiplier_blue,
        ];
        let mut models = Vec::new();
        let mut meshes = Vec::new();
        for (entry, &offset) in pack.entry_offsets.iter().enumerate() {
            let Some(&object_id) = pack.object_ids.get(entry) else {
                break;
            };
            if object_id == 999 {
                continue; // no object takes this entry (0x1E9710 skips ids at the limit)
            }
            let slot = pack.first_slot + entry as u16;
            let at = offset as usize;
            let size = pack_bytes
                .get(at + 12..at + 16)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
                .ok_or_else(|| invalid("pack entry header"))?;
            let record = pack_bytes
                .get(at..at + size)
                .ok_or_else(|| invalid("pack entry exceeds the pack"))?;
            let archive_offset = (pack_start + at) as u64;
            let model = story_geometry::parse_model(record, archive_offset as usize, slot)?;
            let geometry_sha256 = {
                let geo = u32::from_le_bytes(record[0x20..0x24].try_into().unwrap()) as usize;
                let len = u32::from_le_bytes(record[0x24..0x28].try_into().unwrap()) as usize;
                format!(
                    "{:x}",
                    Sha256::digest(
                        record
                            .get(geo..geo + len)
                            .ok_or_else(|| invalid("geometry section"))?
                    )
                )
            };
            // 0x1F1710: header word 1 bit 0 hooks the following MOT block.
            let motion = if model.raw_header_words[1] & 1 != 0 {
                let mot_len = pack_bytes
                    .get(at + size + 4..at + size + 8)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
                    .ok_or_else(|| invalid("MOT header"))?;
                let mot = pack_bytes
                    .get(at + size..at + size + mot_len)
                    .ok_or_else(|| invalid("MOT exceeds the pack"))?;
                Some(Motion::parse(mot, (pack_start + at + size) as u64)?)
            } else {
                None
            };
            if model.raw_header_words[1] & !1 != 0x8000_0000 {
                return Err(unsupported(format!(
                    "RH0 header flags {:#x}",
                    model.raw_header_words[1]
                )));
            }
            let placement = place_model(
                &c,
                &camera,
                &model,
                motion.as_ref(),
                pose.sky_frame,
                multipliers,
            )?;
            let world = WorldModel {
                slot,
                object_id,
                pack_entry: entry,
                archive_offset,
                record_bytes: size as u32,
                record_sha256: model.source_sha256.clone(),
                geometry_sha256,
                model,
                motion,
                visibility_flags: placement.visibility_flags,
                view_space: placement.view_space,
                vu_program_entry: placement.vu_program_entry,
                emitted_matrix: placement.emitted_matrix,
                normal_rows: placement.normal_rows,
                emitted_matrix_sha256: placement.emitted_matrix_sha256,
                material_multipliers: multipliers,
                bounding_radius: placement.bounding_radius,
                sky_rotation_angles: placement.sky_rotation_angles,
            };
            meshes.extend(build_meshes(
                slot,
                &world.model,
                &world,
                &lighting.light_rows,
            )?);
            models.push(world);
        }
        if models.is_empty() {
            return Err(unsupported("no world models"));
        }
        Ok(Self {
            pose: SimonPoseRecord {
                pan: pose.pan,
                level: pose.level,
                sky_frame: pose.sky_frame,
            },
            constants: c,
            camera,
            lighting,
            pack,
            models,
            meshes,
        })
    }

    /// Every animated model's matrices over its animation (`0x1EE6E0`'s
    /// forward mode from frame 1.0 with speed 1 and step 1, so whole frames
    /// 1 ..= frame_count − 1, restarting at 1.0): the matrix `place_model`
    /// emits at each frame with the scene's camera, and whether it is a
    /// view-space draw. In the opening only the sky (slot 37) animates.
    pub fn animation_matrices(&self) -> Result<Vec<AnimatedModel>> {
        let mut out = Vec::new();
        for world in &self.models {
            let Some(motion) = world.motion.as_ref() else {
                continue;
            };
            let last = motion.frame_count.saturating_sub(1);
            if last == 0 || last > 4096 {
                return Err(unsupported("animation length"));
            }
            let mut frames = Vec::with_capacity(last as usize);
            for frame in 1..=last {
                let placement = place_model(
                    &self.constants,
                    &self.camera,
                    &world.model,
                    Some(motion),
                    ee::from_i32(frame as i32)?,
                    world.material_multipliers,
                )?;
                frames.push((placement.emitted_matrix, placement.view_space));
            }
            out.push(AnimatedModel {
                slot: world.slot,
                frame_count: motion.frame_count,
                frames,
            });
        }
        Ok(out)
    }

    pub fn texture(
        &self,
        archive: &[u8],
        slot: u16,
        texture_index: usize,
    ) -> Result<(&OpeningTextureRecord, RgbaImage)> {
        let model = self
            .models
            .iter()
            .find(|m| m.slot == slot)
            .ok_or_else(|| invalid("unknown slot"))?;
        let record = model
            .model
            .textures
            .get(texture_index)
            .ok_or_else(|| invalid("texture index"))?;
        Ok((record, opening_textures::decode_record(archive, record)?))
    }
}

/// One animated world model: the emitted matrix (and view-space flag) for
/// frames 1 ..= frame_count − 1 (`SimonScene::animation_matrices`).
#[derive(Clone, Debug)]
pub struct AnimatedModel {
    pub slot: u16,
    pub frame_count: u32,
    pub frames: Vec<(Matrix, bool)>,
}

struct Placement {
    visibility_flags: u32,
    view_space: bool,
    vu_program_entry: u32,
    emitted_matrix: Matrix,
    normal_rows: [Qword; 3],
    emitted_matrix_sha256: String,
    bounding_radius: f32,
    sky_rotation_angles: Option<[i32; 3]>,
}

/// `0x1EF8F0` + `0x20EC70`: the identity object transform, the frustum
/// classification, program selection, normals, optional projection and the
/// node walk that produces the VU matrix block.
fn place_model(
    c: &SceneConstants,
    cam: &CameraMatrices,
    model: &StoryModel,
    motion: Option<&Motion>,
    frame: f32,
    multipliers: [f32; 3],
) -> Result<Placement> {
    let _ = multipliers;
    let node0 = model.raw_node_words[0];
    // 0x1F0248: the loaded node radius is the disc radius times the draw's
    // largest scale factor (1.0 · 1/0.94 with unit object scale).
    let disc_radius = f32::from_bits(node0[15]);
    let scale_factor = {
        let mut f20 = 1.0f32;
        let sy = ee::mul(c.radius_scale, 1.0)?;
        if f20 <= sy {
            f20 = sy;
        }
        if f20 < 1.0 {
            f20 = 1.0;
        }
        f20
    };
    let radius = ee::mul(disc_radius, scale_factor)?;
    // Object placement: position/rotation zero, scale one, pivot from node 1.
    let mut current = cam.view;
    let pivot: [f32; 3];
    let neg_pivot: [f32; 3];
    if model.raw_node_words.len() > 1 {
        let n1 = model.raw_node_words[1];
        let unit = f32::from_bits(model.raw_header_words[25]);
        let mut p = [0.0f32; 3];
        let mut q = [0.0f32; 3];
        for i in 0..3 {
            p[i] = ee::div(f32::from_bits(n1[8 + i]), unit)?;
            q[i] = ee::mul(p[i], -1.0)?;
        }
        pivot = p;
        neg_pivot = q;
    } else {
        pivot = [0.0; 3];
        neg_pivot = [0.0; 3];
    }
    // 0x20C380(0, 0, rotation (0,0,0), position (0,0,0)).
    {
        let t = [0.0f32, 0.0, 0.0];
        let acc = vu_lane_mul(&current[0], t[0])?;
        let acc = vu_lane_madd(&acc, &current[1], t[1])?;
        let acc = vu_lane_madd(&acc, &current[2], t[2])?;
        current[3] = vu_lane_madd(&acc, &current[3], 1.0)?;
        let r = c.rotation([0, 0, 0])?;
        let m = vu_mat_mul(&current, &r)?;
        current[0] = m[0];
        current[1] = m[1];
        current[2] = m[2];
    }
    // 0x20C0B0(M, scale (1,1,1), rotation (0,0,0), pivot) and 0x20B370.
    {
        let mut m = identity();
        m[3] = [pivot[0], pivot[1], pivot[2], 1.0];
        let r = c.rotation([0, 0, 0])?;
        for i in 0..3 {
            m[i][0] = r[i][0];
            m[i][1] = r[i][1];
            m[i][2] = r[i][2];
        }
        for i in 0..3 {
            m[i][0] = ee::mul(m[i][0], 1.0)?;
            m[i][1] = ee::mul(m[i][1], 1.0)?;
            m[i][2] = ee::mul(m[i][2], 1.0)?;
        }
        current = vu_mat_mul(&current, &m)?;
        let mut t = identity();
        t[3] = [neg_pivot[0], neg_pivot[1], neg_pivot[2], 1.0];
        current = vu_mat_mul(&current, &t)?;
    }
    // 0x20EC70: bounding centre through the current matrix, frustum test.
    let centre = [
        f32::from_bits(node0[12]),
        f32::from_bits(node0[13]),
        f32::from_bits(node0[14]),
        1.0,
    ];
    let centre_view = vu_mat_vec(&current, &centre)?;
    let flags = frustum_flags(c, cam, &centre_view, radius)?;
    if flags & 1 == 0 {
        return Err(unsupported("a world model is classified invisible"));
    }
    let clipping = flags & 4 != 0;
    let mut walk = Walk {
        c,
        current,
        normals: [[0.0; 4]; 3],
        stack: Vec::new(),
        table: BTreeMap::new(),
        motion,
        frame,
        translation_unit: f32::from_bits(model.raw_header_words[25]),
        angles_used: None,
    };
    walk.compute_normals()?;
    if !clipping {
        // 0x20D160: pre-multiply by the projection.
        walk.current = vu_mat_mul(&cam.projection, &walk.current)?;
    }
    walk.walk(model, 0, c.scale_one, 0)?;
    let (emitted_matrix, normal_rows) = walk
        .table
        .get(&1)
        .copied()
        .ok_or_else(|| unsupported("model stores no matrix in slot 1"))?;
    if walk.table.len() != 1 {
        return Err(unsupported("model uses more than one matrix slot"));
    }
    Ok(Placement {
        visibility_flags: flags,
        view_space: clipping,
        vu_program_entry: vu_entry(clipping),
        emitted_matrix,
        normal_rows,
        emitted_matrix_sha256: matrix_sha256(&emitted_matrix, &normal_rows),
        bounding_radius: radius,
        sky_rotation_angles: walk.angles_used,
    })
}

#[cfg(test)]
mod tests;
