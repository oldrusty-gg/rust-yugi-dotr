//! Independent c400 cloud state machine reconstructed from bounded source facts.
//!
//! No game tables, particles or expected quads are embedded. A supplied quarter
//! sine table and explicit shared RNG determine every emitted vertex. Particle
//! spawn arithmetic follows the observed instruction order through an explicit
//! [`FloatModel`]; the PCSX2 model reproduces the reference save state
//! bit-exactly, host `f32` does not. Emission geometry still uses host `f32`
//! (verified against the captured packets, not against EE bit patterns).
//!
//! Boot-derived start: the original executable seeds the C library `rand()`
//! state with 1 and never calls `srand`. Every main-loop iteration draws two
//! values before the object dispatch (one in the frame-flip routine 0x209120 →
//! 0x20A950 that rotates the GS dither matrix, one at 0x100820 → 0x100828), so
//! the state at the cloud object's creation is `rand^(it + it2)(1)` where `it`
//! (EE 0x2DA950) counts completed iterations and `it2` (0x2E963C) counts frame
//! flips. Both depend on how long the player stayed on the intro, title and
//! menu, so [`BootSchedule`] is an explicit input, never a tuned constant.
use crate::ee_float::FloatModel;
use serde::Serialize;

pub const SOURCE_STATE_BYTES: usize = 11_600;
pub const SINE_TABLE_ENTRIES: usize = 1_025;
pub const GROUP_COUNT: usize = 6;
pub const PARTICLES_PER_GROUP: usize = 128;
pub const MAX_QUADS: usize = GROUP_COUNT * PARTICLES_PER_GROUP;
/// Rand calls the original main loop makes before each object dispatch.
pub const EXTERNAL_RNG_CALLS_PER_TICK: u32 = 2;
/// Rand calls consumed by the original nonnegative-mode (initialisation) call.
pub const INITIALIZATION_RNG_CALLS: u64 = (GROUP_COUNT * PARTICLES_PER_GROUP * 6) as u64;
const GROUP_STRIDE: usize = 1_928;
const RNG_MULTIPLIER: u64 = 0x5851_f42d_4c95_7f2d;
pub type Result<T> = std::result::Result<T, String>;

/// Source rand state, also consumed by other game systems outside this module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceRng {
    state: u64,
}
impl SourceRng {
    /// Newlib `rand()` default; the executable never calls `srand`.
    pub const BOOT_SEED: u64 = 1;
    pub fn new(state: u64) -> Self {
        Self { state }
    }
    pub fn boot() -> Self {
        Self::new(Self::BOOT_SEED)
    }
    pub fn state(&self) -> u64 {
        self.state
    }
    pub fn next_u31(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(RNG_MULTIPLIER).wrapping_add(1);
        ((self.state >> 32) & 0x7fff_ffff) as u32
    }
    /// The state after `calls` further rand calls, in O(log calls) time.
    pub fn advanced_by(self, calls: u64) -> Self {
        // Compose the affine map x -> a*x + c by squaring.
        let (mut a, mut c) = (RNG_MULTIPLIER, 1_u64);
        let (mut ra, mut rc) = (1_u64, 0_u64);
        let mut n = calls;
        while n > 0 {
            if n & 1 == 1 {
                // result := step ∘ result
                ra = a.wrapping_mul(ra);
                rc = a.wrapping_mul(rc).wrapping_add(c);
            }
            c = a.wrapping_mul(c).wrapping_add(c);
            a = a.wrapping_mul(a);
            n >>= 1;
        }
        Self {
            state: ra.wrapping_mul(self.state).wrapping_add(rc),
        }
    }
    fn byte(&mut self) -> u8 {
        (self.next_u31() >> 23) as u8
    }
}

/// Main-loop counters at the moment the history event creates the cloud
/// object (its nonnegative-mode dispatch). Both are player-timing dependent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootSchedule {
    /// Completed main-loop iterations (EE 0x2DA950, one rand each at 0x100828).
    pub loop_ticks_before_init: u64,
    /// Frame flips (EE 0x2E963C, one rand each in the dither rotation).
    pub flips_before_init: u64,
}
impl BootSchedule {
    pub fn rand_calls_before_init(&self) -> Result<u64> {
        self.loop_ticks_before_init
            .checked_add(self.flips_before_init)
            .ok_or_else(|| "Boot schedule counters overflow".to_string())
    }
    /// The shared RNG as the cloud initialisation call first sees it.
    pub fn rng_at_init(&self) -> Result<SourceRng> {
        Ok(SourceRng::boot().advanced_by(self.rand_calls_before_init()?))
    }
}

/// The source's 0..quarter-turn table: 1,025 little-endian f32 samples.
#[derive(Clone, Debug)]
pub struct SineTable {
    values: [f32; SINE_TABLE_ENTRIES],
}
impl SineTable {
    pub fn from_le_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != SINE_TABLE_ENTRIES * 4 {
            return Err("Sine table must contain exactly 1,025 f32 samples".into());
        }
        let mut values = [0.; SINE_TABLE_ENTRIES];
        for (sample, chunk) in values.iter_mut().zip(bytes.chunks_exact(4)) {
            *sample = f32::from_le_bytes(chunk.try_into().unwrap());
        }
        if values[0] != 0.
            || values[SINE_TABLE_ENTRIES - 1] != 1.
            || values
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || values.windows(2).any(|v| v[1] < v[0])
        {
            return Err("Sine table must be finite, monotone 0..1 with exact endpoints".into());
        }
        Ok(Self { values })
    }
    /// One complete turn is 65,536 units; conversion exactly wraps to i16.
    pub fn sin(&self, angle: u16) -> f32 {
        let mut a = i32::from(angle as i16);
        let sign = if a < 0 {
            a = -a;
            -1.0_f32
        } else {
            1.0_f32
        };
        if a > 16_384 {
            a = 32_768 - a;
        }
        if a & 16_383 == 0 {
            return if a == 16_384 { sign } else { 0. };
        }
        let index = (a >> 4) as usize;
        let remainder = (a & 15) as f32;
        let lo = self.values[index] * (16. - remainder);
        let hi = self.values[index + 1] * remainder;
        let weighted = lo + hi;
        (sign * weighted) / 16.
    }
    pub fn cos(&self, angle: u16) -> f32 {
        self.sin(angle.wrapping_add(16_384))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub tile: u8,
    pub scale: f32,
    pub grayscale: u8,
    pub base_alpha: u8,
    pub position: [i32; 2],
}
#[derive(Clone, Debug, PartialEq)]
pub struct CloudGroup {
    active: bool,
    age: u8,
    particles: [Particle; PARTICLES_PER_GROUP],
}
impl CloudGroup {
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn age(&self) -> u8 {
        self.age
    }
    pub fn particles(&self) -> &[Particle; PARTICLES_PER_GROUP] {
        &self.particles
    }
}
/// Which cloud module the state runs [F, CMODULE.MRG]: c400 (the history's
/// clouds, launched by event 10020) or c402 (launched by the endings 11184 /
/// 12156 once W+64 ≥ 2001, 0x2217D0 / 0x222CF4). c402 is c400's code with
/// three changes: the groups run in reverse (position and size factors use
/// 6 − age instead of age, so the clouds close in and shrink), the alpha
/// fades in with the counter, min(counter / 96, 1) (0x101020(counter, 96))
/// instead of fading out after an exit request (it has no exit counter),
/// and it ends when the story's sub-step W+536C is 3 with the letterbox
/// closed (W+1C = 2). Spawning, the RNG use, the textures and the rotation
/// (counter · 768) are the same.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloudKind {
    #[default]
    Rising,
    Converging,
}

/// c402's fade-in length (0x1E006A4: 96.0).
pub const CONVERGING_FADE_UPDATES: f32 = 96.0;

#[derive(Clone, Debug, PartialEq)]
pub struct CloudState {
    kind: CloudKind,
    exit_counter: u32,
    counter: u32,
    next_group: usize,
    groups: [CloudGroup; GROUP_COUNT],
    /// Arithmetic used by every later particle spawn.
    arithmetic: FloatModel,
}

/// Triangle-strip order TL, TR, BL, BR after rotation. XY is source logical
/// 640x448 space (not clipped); UV is source FST units, 16 per texel. RGBA is
/// unnormalized GS bytes, with alpha divided by 128 by the later blend stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CloudQuad {
    pub group: u8,
    pub particle: u8,
    pub age: u8,
    pub xy: [[i32; 2]; 4],
    pub uv: [[u16; 2]; 4],
    pub rgba: [u8; 4],
}
#[derive(Clone, Copy, Debug, Default)]
pub struct TickInput {
    pub request_exit: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct CloudFrame {
    pub counter_used: u32,
    pub exit_counter_used: u32,
    /// Original c400 return value: zero while active, eight when finished.
    pub completion_code: u8,
    pub quads: Vec<CloudQuad>,
}

/// One particle: six rand calls in source order. Each float field follows the
/// original `cvt.s.w`, `mul.s` by the constant, `div.s` by 256 and `add.s` of
/// the base, then the original truncating conversion.
fn spawn(rng: &mut SourceRng, initial: bool, m: FloatModel) -> Result<Particle> {
    let tile = rng.byte() % 8;
    let scale_random = f32::from(rng.byte());
    let scale = m.add(0.4, m.div_pow2(m.mul(0.6, scale_random)?, 256.)?)?;
    // The update still consumes this random value although color is constant.
    let color_random = f32::from(rng.byte());
    let grayscale = if initial {
        let factor = m.add(0.9, m.div_pow2(m.mul(0.1, color_random)?, 256.)?)?;
        m.mul(255., factor)?.trunc() as u8
    } else {
        255
    };
    let alpha_random = f32::from(rng.byte());
    let alpha_factor = m.add(0.3, m.div_pow2(m.mul(0.7, alpha_random)?, 256.)?)?;
    let base_alpha = m.mul(128., alpha_factor)?.trunc() as u8;
    let mut position = [0_i32; 2];
    for axis in &mut position {
        let fraction = m.div_pow2(m.mul(980., f32::from(rng.byte()))?, 256.)?;
        *axis = m.add(-490., fraction)?.trunc() as i32;
    }
    Ok(Particle {
        tile,
        scale,
        grayscale,
        base_alpha,
        position,
    })
}

impl CloudState {
    /// Initialization consumes exactly 4,608 rand calls, creates six groups and
    /// returns without emitting or incrementing the update counter.
    pub fn initialize(rng: &mut SourceRng, arithmetic: FloatModel) -> Result<Self> {
        let mut groups = Vec::with_capacity(GROUP_COUNT);
        for g in 0..GROUP_COUNT {
            let mut particles = Vec::with_capacity(PARTICLES_PER_GROUP);
            for _ in 0..PARTICLES_PER_GROUP {
                particles.push(spawn(rng, true, arithmetic)?);
            }
            groups.push(CloudGroup {
                active: true,
                age: (6 - g) as u8,
                particles: particles.try_into().unwrap(),
            });
        }
        Ok(Self {
            kind: CloudKind::Rising,
            exit_counter: 0,
            counter: 0,
            next_group: 0,
            groups: groups.try_into().unwrap(),
            arithmetic,
        })
    }
    /// c402's creation: the same 4,608-call initialization as c400.
    pub fn initialize_converging(rng: &mut SourceRng, arithmetic: FloatModel) -> Result<Self> {
        let mut state = Self::initialize(rng, arithmetic)?;
        state.kind = CloudKind::Converging;
        Ok(state)
    }
    pub fn kind(&self) -> CloudKind {
        self.kind
    }
    /// The counter-0 state exactly as the original creates it at the history
    /// event, together with the shared RNG the next main-loop iteration
    /// continues from. The caller then draws [`EXTERNAL_RNG_CALLS_PER_TICK`]
    /// values before every [`Self::step`].
    pub fn from_boot(schedule: &BootSchedule, arithmetic: FloatModel) -> Result<(Self, SourceRng)> {
        let mut rng = schedule.rng_at_init()?;
        let state = Self::initialize(&mut rng, arithmetic)?;
        Ok((state, rng))
    }
    pub fn arithmetic(&self) -> FloatModel {
        self.arithmetic
    }
    /// The original 11,600-byte layout with zero texture pointers (offsets
    /// 8..20), for byte comparison against exported EE memory.
    pub fn to_source_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0; SOURCE_STATE_BYTES];
        bytes[0..4].copy_from_slice(&self.exit_counter.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.counter.to_le_bytes());
        bytes[20..24].copy_from_slice(&(self.next_group as u32).to_le_bytes());
        for (g, group) in self.groups.iter().enumerate() {
            let base = GROUP_STRIDE * g;
            bytes[base + 24] = u8::from(group.active);
            bytes[base + 28..base + 32].copy_from_slice(&u32::from(group.age).to_le_bytes());
            for (i, p) in group.particles.iter().enumerate() {
                bytes[base + 32 + i] = p.tile;
                bytes[base + 160 + 4 * i..base + 164 + 4 * i]
                    .copy_from_slice(&p.scale.to_le_bytes());
                bytes[base + 672 + 2 * i] = p.grayscale;
                bytes[base + 673 + 2 * i] = p.base_alpha;
                bytes[base + 928 + 8 * i..base + 932 + 8 * i]
                    .copy_from_slice(&p.position[0].to_le_bytes());
                bytes[base + 932 + 8 * i..base + 936 + 8 * i]
                    .copy_from_slice(&p.position[1].to_le_bytes());
            }
        }
        bytes
    }
    pub fn counter(&self) -> u32 {
        self.counter
    }
    pub fn exit_counter(&self) -> u32 {
        self.exit_counter
    }
    pub fn next_group(&self) -> usize {
        self.next_group
    }
    pub fn groups(&self) -> &[CloudGroup; GROUP_COUNT] {
        &self.groups
    }
    pub fn complete(&self) -> bool {
        self.kind == CloudKind::Rising && self.exit_counter > 160
    }

    /// Bounded checkpoint import of the original c400 state only. Texture
    /// pointers/reserved bytes are not dereferenced. Caller verifies provenance;
    /// structural validation here is not an original-game fingerprint check.
    /// Groups regenerated after the import use `arithmetic`.
    pub fn from_source_bytes(bytes: &[u8], arithmetic: FloatModel) -> Result<Self> {
        if bytes.len() != SOURCE_STATE_BYTES {
            return Err("c400 state must be exactly 11,600 bytes".into());
        }
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let exit_counter = word(0);
        let counter = word(4);
        let next_group = word(20) as usize;
        if exit_counter > 161 || next_group >= GROUP_COUNT {
            return Err("c400 header counters out of bounds".into());
        }
        let mut groups = Vec::with_capacity(GROUP_COUNT);
        for g in 0..GROUP_COUNT {
            let base = GROUP_STRIDE * g;
            let active = bytes[base + 24];
            let age = word(base + 28);
            if active > 1 || age > 7 || (active == 1 && age > 6) {
                return Err("c400 active group/age out of bounds".into());
            }
            let mut particles = Vec::with_capacity(PARTICLES_PER_GROUP);
            for i in 0..PARTICLES_PER_GROUP {
                let particle = Particle {
                    tile: bytes[base + 32 + i],
                    scale: f32::from_bits(word(base + 160 + 4 * i)),
                    grayscale: bytes[base + 672 + 2 * i],
                    base_alpha: bytes[base + 673 + 2 * i],
                    position: [
                        word(base + 928 + 8 * i) as i32,
                        word(base + 932 + 8 * i) as i32,
                    ],
                };
                if particle.tile >= 8
                    || !particle.scale.is_finite()
                    || !(0.399_999_9..=1.000_000_1).contains(&particle.scale)
                    || particle.base_alpha > 128
                    || particle.position.iter().any(|v| !(-490..=490).contains(v))
                {
                    return Err(format!("c400 particle {g}/{i} out of bounds"));
                }
                particles.push(particle);
            }
            groups.push(CloudGroup {
                active: active == 1,
                age: age as u8,
                particles: particles.try_into().unwrap(),
            });
        }
        Ok(Self {
            kind: CloudKind::Rising,
            exit_counter,
            counter,
            next_group,
            groups: groups.try_into().unwrap(),
            arithmetic,
        })
    }

    /// Inspect the current state's geometry without spawning, aging or RNG
    /// consumption. A post-update checkpoint's emit is not its prior draw buffer;
    /// use the separately reconstructed pre-emission checkpoint for that oracle.
    pub fn emit(&self, sine: &SineTable) -> Vec<CloudQuad> {
        let angle = self.counter.wrapping_mul(768) as u16;
        let sn = sine.sin(angle);
        let cs = sine.cos(angle);
        let fade = match self.kind {
            // 0x100FF0(exit, 160, 64).
            CloudKind::Rising if self.exit_counter <= 64 => 1.0_f32,
            CloudKind::Rising => ((self.exit_counter as f32 - 160.) / (64.0_f32 - 160.)).max(0.),
            // 0x101020(counter, 96).
            CloudKind::Converging if (self.counter as f32) < CONVERGING_FADE_UPDATES => {
                self.counter as f32 / CONVERGING_FADE_UPDATES
            }
            CloudKind::Converging => 1.0,
        };
        let mut output = Vec::with_capacity(MAX_QUADS);
        for (g, group) in self.groups.iter().enumerate().rev() {
            if !group.active {
                continue;
            }
            // c402 scales by 6 − age (0x1E00768, 0x1E0083C).
            let age = match self.kind {
                CloudKind::Rising => f32::from(group.age),
                CloudKind::Converging => (6 - i32::from(group.age)) as f32,
            };
            let position_product = 0.7_f32 * age;
            let position_scale = 0.4_f32 + position_product / 6.;
            let size_product = 0.6_f32 * age;
            let size_factor = 0.4_f32 + size_product / 6.;
            let alpha_factor = fade * sine.sin((u32::from(group.age) * 32_768 / 6) as u16);
            for (i, particle) in group.particles.iter().enumerate().rev() {
                let extent = 170.0_f32 * size_factor;
                let half = (extent * particle.scale).trunc() as i32;
                let tx = particle.position[0] as f32 * position_scale;
                let ty = particle.position[1] as f32 * position_scale;
                let xy =
                    [(-half, -half), (half, -half), (-half, half), (half, half)].map(|(x, y)| {
                        let x = x as f32 + tx;
                        let y = y as f32 + ty;
                        let rx0 = x * cs;
                        let rx1 = y * -sn;
                        let ry0 = x * sn;
                        let ry1 = y * cs;
                        [
                            (rx0 + rx1).trunc() as i32 + 320,
                            (ry0 + ry1).trunc() as i32 + 224,
                        ]
                    });
                let u = u16::from(particle.tile % 4) * 32 * 16;
                let v = u16::from(particle.tile / 4) * 32 * 16;
                let alpha = (f32::from(particle.base_alpha) * alpha_factor).trunc() as u8;
                output.push(CloudQuad {
                    group: g as u8,
                    particle: i as u8,
                    age: group.age,
                    xy,
                    uv: [[u, v], [u + 512, v], [u, v + 512], [u + 512, v + 512]],
                    rgba: [
                        particle.grayscale,
                        particle.grayscale,
                        particle.grayscale,
                        alpha,
                    ],
                });
            }
        }
        output
    }

    /// One original negative-mode call: begin exit if requested, regenerate one
    /// group (768 rand calls), emit at current counter, age groups, then advance
    /// counters and return completion. Caller schedules all other global RNG
    /// calls explicitly; this method never assumes the two observed frame calls.
    pub fn step(
        &mut self,
        rng: &mut SourceRng,
        sine: &SineTable,
        input: TickInput,
    ) -> Result<CloudFrame> {
        if self.complete() {
            return Err("c400 simulation has completed".into());
        }
        if input.request_exit && self.exit_counter == 0 && self.kind == CloudKind::Rising {
            self.exit_counter = 1;
        }
        let slot = self.next_group;
        self.next_group = (slot + 1) % GROUP_COUNT;
        let mut particles = Vec::with_capacity(PARTICLES_PER_GROUP);
        for _ in 0..PARTICLES_PER_GROUP {
            particles.push(spawn(rng, false, self.arithmetic)?);
        }
        self.groups[slot] = CloudGroup {
            active: true,
            age: 0,
            particles: particles.try_into().unwrap(),
        };
        let counter_used = self.counter;
        let exit_counter_used = self.exit_counter;
        let quads = self.emit(sine);
        for group in &mut self.groups {
            if group.active {
                group.age += 1;
                if group.age >= 7 {
                    group.active = false;
                }
            }
        }
        self.counter = self.counter.wrapping_add(1);
        if self.exit_counter > 0 {
            self.exit_counter += 1;
        }
        Ok(CloudFrame {
            counter_used,
            exit_counter_used,
            completion_code: if self.complete() { 8 } else { 0 },
            quads,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_sine() -> SineTable {
        let bytes: Vec<_> = (0..SINE_TABLE_ENTRIES)
            .flat_map(|i| (i as f32 / 1024.).to_le_bytes())
            .collect();
        SineTable::from_le_bytes(&bytes).unwrap()
    }
    const PCSX2: FloatModel = FloatModel::Pcsx2PositiveRounding;
    fn synthetic_state() -> CloudState {
        CloudState {
            kind: CloudKind::Rising,
            exit_counter: 0,
            counter: 0,
            next_group: 0,
            groups: std::array::from_fn(|_| CloudGroup {
                active: true,
                age: 3,
                particles: [Particle {
                    tile: 5,
                    scale: 1.,
                    grayscale: 77,
                    base_alpha: 100,
                    position: [0, 0],
                }; PARTICLES_PER_GROUP],
            }),
            arithmetic: FloatModel::HostF32,
        }
    }
    fn source_bytes(state: &CloudState) -> Vec<u8> {
        state.to_source_bytes()
    }

    #[test]
    fn converging_clouds_mirror_the_ages_and_fade_in_over_96_updates() {
        let sine = synthetic_sine();
        let mut rising = synthetic_state();
        for (g, group) in rising.groups.iter_mut().enumerate() {
            group.age = g as u8;
            for (i, p) in group.particles.iter_mut().enumerate() {
                p.position = [i as i32 * 3 - 190, 40 - i as i32];
                p.scale = 0.5 + i as f32 / 512.;
            }
        }
        let mut converging = rising.clone();
        converging.kind = CloudKind::Converging;
        for (g, group) in converging.groups.iter_mut().enumerate() {
            group.age = 6 - g as u8;
        }
        // Positions and sizes: converging age a = rising age 6 - a.
        let xy = |q: &[CloudQuad]| {
            let mut v: Vec<_> = q.iter().map(|q| (q.particle, q.xy)).collect();
            v.sort();
            v
        };
        converging.counter = 96;
        rising.counter = 96;
        assert_eq!(xy(&converging.emit(&sine)), xy(&rising.emit(&sine)));
        // Alpha: sin(age·π/6) times min(counter / 96, 1); no exit counter.
        converging.counter = 48;
        let half = converging.emit(&sine);
        converging.counter = 200;
        let full = converging.emit(&sine);
        let alpha = |q: &[CloudQuad], g: u8| q.iter().find(|q| q.group == g).unwrap().rgba[3];
        let g = 3u8; // age 3: sin(π/2) = 1
        assert_eq!(alpha(&full, g), 100);
        assert_eq!(alpha(&half, g), 50);
        assert!(!converging.complete());
        let mut rng = SourceRng::new(5);
        let mut state = CloudState::initialize_converging(&mut rng, PCSX2).unwrap();
        assert_eq!(state.kind(), CloudKind::Converging);
        for _ in 0..400 {
            state.step(&mut rng, &sine, TickInput { request_exit: true }).unwrap();
        }
        assert!(!state.complete() && state.exit_counter() == 0);
    }

    #[test]
    fn source_rng_wraps_64_bits_and_masks_the_return_sign() {
        let mut rng = SourceRng::new(0);
        assert_eq!(rng.next_u31(), 0);
        assert_eq!(rng.state(), 1);
        assert_eq!(rng.next_u31(), 0x5851_f42d);
        assert_eq!(rng.state(), 0x5851_f42d_4c95_7f2e);
        let mut rng = SourceRng::new(u64::MAX);
        assert_eq!(rng.next_u31(), 0x27ae_0bd2);
        assert_eq!(rng.state(), 0xa7ae_0bd2_b36a_80d4);
    }

    #[test]
    fn rng_jump_matches_iteration_and_boot_schedule_sums_counters() {
        for (seed, calls) in [(1_u64, 0_u64), (1, 1), (1, 2), (7, 1819), (u64::MAX, 4609)] {
            let mut iterated = SourceRng::new(seed);
            for _ in 0..calls {
                iterated.next_u31();
            }
            assert_eq!(SourceRng::new(seed).advanced_by(calls), iterated, "{calls}");
        }
        assert_eq!(SourceRng::boot().state(), 1);
        let schedule = BootSchedule {
            loop_ticks_before_init: 1222,
            flips_before_init: 1223,
        };
        assert_eq!(schedule.rand_calls_before_init().unwrap(), 2445);
        assert_eq!(
            schedule.rng_at_init().unwrap(),
            SourceRng::boot().advanced_by(2445)
        );
        assert!(
            BootSchedule {
                loop_ticks_before_init: u64::MAX,
                flips_before_init: 1,
            }
            .rand_calls_before_init()
            .is_err()
        );
    }

    #[test]
    fn boot_state_is_counter_zero_and_consumes_only_initialization_calls() {
        let schedule = BootSchedule {
            loop_ticks_before_init: 3,
            flips_before_init: 4,
        };
        let (state, rng) = CloudState::from_boot(&schedule, PCSX2).unwrap();
        assert_eq!(
            rng,
            SourceRng::boot().advanced_by(7 + INITIALIZATION_RNG_CALLS)
        );
        assert_eq!(
            (state.counter(), state.exit_counter(), state.next_group()),
            (0, 0, 0)
        );
        assert_eq!(state.arithmetic(), PCSX2);
        assert_eq!(
            state
                .groups
                .iter()
                .map(|g| (g.active, g.age))
                .collect::<Vec<_>>(),
            [
                (true, 6),
                (true, 5),
                (true, 4),
                (true, 3),
                (true, 2),
                (true, 1)
            ]
        );
        // Spawn arithmetic under the verified model: r=255 scale is 0x3f7f6668,
        // one ULP above host round-to-nearest (0x3f7f6667).
        let mut fixed = SourceRng::new(0);
        let bytes: Vec<u8> = (0..6).map(|_| fixed.byte()).collect();
        let mut model = SourceRng::new(0);
        let mut host = SourceRng::new(0);
        let a = spawn(&mut model, true, PCSX2).unwrap();
        let b = spawn(&mut host, true, FloatModel::HostF32).unwrap();
        assert_eq!(
            (a.tile, a.grayscale, a.base_alpha, a.position),
            (b.tile, b.grayscale, b.base_alpha, b.position)
        );
        let expected = PCSX2
            .add(
                0.4,
                PCSX2
                    .div_pow2(PCSX2.mul(0.6, f32::from(bytes[1])).unwrap(), 256.)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(a.scale, expected);
        let round_trip = CloudState::from_source_bytes(&state.to_source_bytes(), PCSX2).unwrap();
        assert_eq!(round_trip, state);
    }

    #[test]
    fn sine_interpolation_wraps_quadrants_and_rejects_bad_tables() {
        let sine = synthetic_sine();
        assert_eq!(sine.sin(0), 0.);
        assert_eq!(sine.sin(8), 1. / 2048.);
        assert_eq!(sine.sin(16_384), 1.);
        assert_eq!(sine.sin(32_768), 0.);
        assert_eq!(sine.sin(49_152), -1.);
        assert_eq!(sine.sin(u16::MAX), -1. / 16_384.);
        assert_eq!(sine.cos(0), 1.);
        assert!(SineTable::from_le_bytes(&[0; 4]).is_err());
        let mut bytes: Vec<_> = sine.values.iter().flat_map(|v| v.to_le_bytes()).collect();
        bytes[4..8].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(SineTable::from_le_bytes(&bytes).is_err());
        bytes[4..8].copy_from_slice(&0.5_f32.to_le_bytes());
        assert!(SineTable::from_le_bytes(&bytes).is_err());
    }

    #[test]
    fn init_and_update_consume_exact_rng_calls_with_caller_owned_cadence() {
        let mut rng = SourceRng::new(123);
        let mut expected_rng = rng;
        let mut state = CloudState::initialize(&mut rng, PCSX2).unwrap();
        for _ in 0..4608 {
            expected_rng.next_u31();
        }
        assert_eq!(rng, expected_rng);
        assert_eq!(
            state.groups.iter().map(|g| g.age).collect::<Vec<_>>(),
            [6, 5, 4, 3, 2, 1]
        );
        assert_eq!((state.counter(), state.next_group()), (0, 0));
        assert!(state.groups.iter().flat_map(|g| g.particles()).all(
            |p| (0.4..1.).contains(&p.scale)
                && (229..=254).contains(&p.grayscale)
                && (38..=127).contains(&p.base_alpha)
                && p.position.iter().all(|v| (-490..=486).contains(v))
        ));
        // External callers may account for the measured global rand calls. The
        // cloud step must not silently add another pair of its own.
        for _ in 0..2 {
            rng.next_u31();
            expected_rng.next_u31();
        }
        let frame = state
            .step(&mut rng, &synthetic_sine(), TickInput::default())
            .unwrap();
        for _ in 0..768 {
            expected_rng.next_u31();
        }
        assert_eq!(rng, expected_rng);
        assert_eq!(
            (frame.counter_used, state.counter(), state.next_group()),
            (0, 1, 1)
        );
        assert_eq!(state.groups[0].age, 1);
        assert!(state.groups[0].particles.iter().all(|p| p.grayscale == 255));
        assert_eq!(frame.quads.len(), 768);
        assert!(
            frame
                .quads
                .iter()
                .filter(|q| q.group == 0)
                .all(|q| q.age == 0 && q.rgba[3] == 0)
        );
    }

    #[test]
    fn emitted_strip_rotation_order_uv_and_alpha_are_calculated() {
        let sine = synthetic_sine();
        let mut state = synthetic_state();
        let frame = state.emit(&sine);
        assert_eq!(frame.len(), MAX_QUADS);
        assert_eq!((frame[0].group, frame[0].particle), (5, 127));
        assert_eq!((frame[767].group, frame[767].particle), (0, 0));
        assert_eq!(
            frame[0].xy,
            [[201, 105], [439, 105], [201, 343], [439, 343]]
        );
        assert_eq!(
            frame[0].uv,
            [[512, 512], [1024, 512], [512, 1024], [1024, 1024]]
        );
        assert_eq!(frame[0].rgba, [77, 77, 77, 100]);
        state.counter = 64; // 64*768 = three quarter-turns.
        assert_eq!(
            state.emit(&sine)[0].xy,
            [[201, 343], [201, 105], [439, 343], [439, 105]]
        );
        state.exit_counter = 112;
        assert_eq!(state.emit(&sine)[0].rgba[3], 50);
        state.exit_counter = 160;
        assert_eq!(state.emit(&sine)[0].rgba[3], 0);
    }

    #[test]
    fn checkpoint_loader_rejects_truncation_nan_and_out_of_range_fields() {
        let state = synthetic_state();
        let bytes = source_bytes(&state);
        let load = |b: &[u8]| CloudState::from_source_bytes(b, FloatModel::HostF32);
        assert_eq!(load(&bytes).unwrap(), state);
        assert!(load(&bytes[..bytes.len() - 1]).is_err());
        for (offset, value) in [(20, 6_u32), (28, 8), (160, f32::NAN.to_bits()), (928, 491)] {
            let mut bad = bytes.clone();
            bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(load(&bad).is_err(), "offset {offset}");
        }
        for (offset, value) in [(24, 2), (32, 8), (673, 129)] {
            let mut bad = bytes.clone();
            bad[offset] = value;
            assert!(load(&bad).is_err(), "offset {offset}");
        }
        let mut pointers = bytes;
        pointers[8..20].fill(0xff); // Never followed or validated as host addresses.
        assert_eq!(load(&pointers).unwrap(), state);
    }

    #[test]
    fn exit_completion_and_counter_wrap_follow_emit_then_advance_order() {
        let sine = synthetic_sine();
        let mut rng = SourceRng::new(42);
        let mut state = synthetic_state();
        state.counter = u32::MAX;
        state.groups[5].age = 6;
        let first = state
            .step(&mut rng, &sine, TickInput { request_exit: true })
            .unwrap();
        assert_eq!(
            (
                first.counter_used,
                state.counter(),
                first.exit_counter_used,
                state.exit_counter()
            ),
            (u32::MAX, 0, 1, 2)
        );
        assert!(!state.groups[5].active());
        state.exit_counter = 159;
        assert_eq!(
            state
                .step(&mut rng, &sine, TickInput::default())
                .unwrap()
                .completion_code,
            0
        );
        let last = state.step(&mut rng, &sine, TickInput::default()).unwrap();
        assert_eq!(
            (
                last.exit_counter_used,
                state.exit_counter(),
                last.completion_code
            ),
            (160, 161, 8)
        );
        let unchanged = rng;
        assert!(state.step(&mut rng, &sine, TickInput::default()).is_err());
        assert_eq!(rng, unchanged);
    }

    #[test]
    #[ignore = "Requires independently exported private c400 state/table and GIF oracle"]
    fn private_all_768_source_emissions_match_original_packets() {
        use std::{
            fs::File,
            io::Read,
            path::{Path, PathBuf},
        };
        #[derive(serde::Deserialize)]
        struct OracleQuad {
            group: u8,
            index: u8,
            xy: [[f64; 2]; 4],
            uv_fixed_16: [[u16; 2]; 4],
            rgba: [u8; 4],
        }
        #[derive(serde::Deserialize)]
        struct Oracle {
            counter: u32,
            quads: Vec<OracleQuad>,
        }
        #[derive(serde::Deserialize)]
        struct Metadata {
            rng_state_hex: String,
        }
        fn bounded(path: &Path, maximum: usize) -> Vec<u8> {
            let file = File::open(path).unwrap();
            assert!(file.metadata().unwrap().is_file());
            let mut bytes = Vec::new();
            file.take(maximum as u64 + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            assert!(bytes.len() <= maximum);
            bytes
        }
        let directory = PathBuf::from(
            std::env::var_os("ROSES_CLOUD_ORACLE_DIR").expect("ROSES_CLOUD_ORACLE_DIR"),
        );
        let state = CloudState::from_source_bytes(
            &bounded(
                &directory.join("pre-emission-counter436-state.bin"),
                SOURCE_STATE_BYTES,
            ),
            PCSX2,
        )
        .unwrap();
        let sine = SineTable::from_le_bytes(&bounded(
            &directory.join("sine-quarter-1025-f32le.bin"),
            SINE_TABLE_ENTRIES * 4,
        ))
        .unwrap();
        // Expected GIF packets are read only here, never by a public state,
        // initialization, update, emission or renderer function.
        let expected: Oracle = serde_json::from_slice(&bounded(
            &directory.join("expected-history-next-quads.json"),
            2 * 1024 * 1024,
        ))
        .unwrap();
        assert_eq!(state.counter(), expected.counter);
        let actual = state.emit(&sine);
        assert_eq!((actual.len(), expected.quads.len()), (768, 768));
        for (index, (actual, expected)) in actual.iter().zip(&expected.quads).enumerate() {
            assert_eq!(
                (actual.group, actual.particle),
                (expected.group, expected.index),
                "order {index}"
            );
            assert_eq!(
                actual.xy.map(|p| p.map(f64::from)),
                expected.xy,
                "XY {index}"
            );
            assert_eq!(actual.uv, expected.uv_fixed_16, "UV {index}");
            assert_eq!(actual.rgba, expected.rgba, "RGBA {index}");
        }
        let mut resume = CloudState::from_source_bytes(
            &bounded(
                &directory.join("saved-counter437-state.bin"),
                SOURCE_STATE_BYTES,
            ),
            PCSX2,
        )
        .unwrap();
        assert_eq!((resume.counter(), resume.next_group()), (437, 5));
        let metadata: Metadata = serde_json::from_slice(&bounded(
            &directory.join("source-metadata-20260929.json"),
            32 * 1024,
        ))
        .unwrap();
        let mut rng = SourceRng::new(u64::from_str_radix(&metadata.rng_state_hex, 16).unwrap());
        let frame = resume.step(&mut rng, &sine, TickInput::default()).unwrap();
        assert_eq!(
            (frame.counter_used, resume.counter(), resume.next_group()),
            (437, 438, 0)
        );
        assert_eq!((frame.quads[0].group, frame.quads[0].age), (5, 0));
    }

    /// Boot-derived reproduction of the reference save state: seed 1, the
    /// recording's main-loop counters at cloud creation (it=1222, it2=1223),
    /// initialisation, then 437 iterations of two external rand calls and one
    /// update. Every state byte except the three texture pointers must match
    /// the exported EE memory, and the RNG must equal the saved global state.
    #[test]
    #[ignore = "Requires independently exported private c400 state and metadata"]
    fn private_boot_schedule_reproduces_saved_counter437_state_and_rng() {
        use std::path::PathBuf;
        #[derive(serde::Deserialize)]
        struct Metadata {
            rng_state_hex: String,
            current_counter: u32,
            current_next_group: usize,
        }
        let directory = PathBuf::from(
            std::env::var_os("ROSES_CLOUD_ORACLE_DIR").expect("ROSES_CLOUD_ORACLE_DIR"),
        );
        let saved = std::fs::read(directory.join("saved-counter437-state.bin")).unwrap();
        assert_eq!(saved.len(), SOURCE_STATE_BYTES);
        let metadata: Metadata = serde_json::from_slice(
            &std::fs::read(directory.join("source-metadata-20260929.json")).unwrap(),
        )
        .unwrap();
        let sine = SineTable::from_le_bytes(
            &std::fs::read(directory.join("sine-quarter-1025-f32le.bin")).unwrap(),
        )
        .unwrap();
        let schedule = BootSchedule {
            loop_ticks_before_init: 1222,
            flips_before_init: 1223,
        };
        let (mut state, mut rng) = CloudState::from_boot(&schedule, PCSX2).unwrap();
        for _ in 0..metadata.current_counter {
            for _ in 0..EXTERNAL_RNG_CALLS_PER_TICK {
                rng.next_u31();
            }
            state.step(&mut rng, &sine, TickInput::default()).unwrap();
        }
        assert_eq!(
            rng.state(),
            u64::from_str_radix(&metadata.rng_state_hex, 16).unwrap()
        );
        assert_eq!(
            (state.counter(), state.next_group()),
            (metadata.current_counter, metadata.current_next_group)
        );
        let mine = state.to_source_bytes();
        let differing: Vec<usize> = (0..SOURCE_STATE_BYTES)
            .filter(|&i| !(8..20).contains(&i) && saved[i] != mine[i])
            .collect();
        assert!(
            differing.is_empty(),
            "differing offsets: {:?}",
            &differing[..differing.len().min(16)]
        );
        // Host arithmetic is not capture-exact: the same schedule must differ.
        let (mut host, mut host_rng) =
            CloudState::from_boot(&schedule, FloatModel::HostF32).unwrap();
        for _ in 0..metadata.current_counter {
            for _ in 0..EXTERNAL_RNG_CALLS_PER_TICK {
                host_rng.next_u31();
            }
            host.step(&mut host_rng, &sine, TickInput::default())
                .unwrap();
        }
        assert_ne!(host.to_source_bytes(), mine);
    }
}
