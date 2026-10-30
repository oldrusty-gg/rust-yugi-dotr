//! Pose, skinning and per-vertex lighting of decoded MONSTER.MRG unit models
//! (`roses_assets::MonsterModel`), independent of any renderer.
//!
//! Ports of the PAL EE routines as decoded in the private model research
//! (`work/research/duel/claude-models/NOTES.md`, reference scripts
//! `pose.py`, `verify_*.py`):
//!
//! - key sampling 0x210480: a channel with fewer than three keys is constant
//!   key 0; otherwise the binary search finds i with f_i <= t < f_i+1
//!   (clamped to the last key) and interpolates linearly in f32; angles take
//!   the 16-bit shortest path and truncate (a0 w0 + a1 w1 + 0.5) (0x20C940);
//! - node evaluation 0x210050 / 0x2100D0 in preorder: translation keys are
//!   multiplied by 1 (node 0) or the RH0 f32 at +0x64; a present track clears
//!   the node's no-T/no-R/no-S flag; M = M_parent T(trans * S_parent)
//!   Rz Ry Rx (unless both flags), S = S_parent * scale (unless no-S), and the
//!   runtime matrix block = M diag(S). Type-7 motions keep node 0's scale;
//! - sin/cos: the executable's 1025-entry quarter table with 1/16 steps
//!   interpolated linearly (0x20ACC0 / 0x20ABF0), rotation order 0x20D350;
//! - object matrix 0x1EF8F0 for units: W = T(pos) R(rot) T(c') R(rot2) S' T(-c);
//! - motion time 0x1EE6E0 per update and SetMotion 0x1F2A40 (t starts at 1);
//! - vertex = sum_i w_i W B_i [p_i, 1], normal = sum_i w_i rot(W B_i) n_i
//!   with normalised columns (not renormalised afterwards), colour per channel
//!   floor(min(max, 0.4 g ambient + 1.6 g diffuse max(0, n . L))), A = diffuse.w,
//!   with the board unit light L = 0.5 (-0.7071, 0.7071, 0) in world space.
//!
//! Matrices are f64 like the verified reference; key interpolation keeps the
//! reference's f32 steps. Not implemented: the 6-update motion cross-fade
//! (0x1F2770; its rotation blend 0x20C9B0 was not verified in savestates) and
//! the summon grow-in.
use roses_assets::{
    MonsterAngleKey, MonsterDraw, MonsterModel, MonsterMotion, MonsterVecKey,
    PAL_SINE_TABLE_ENTRIES,
};
use std::collections::BTreeMap;

/// Column-major 4x4 matrix: `m[column][row]`.
pub type Mat4 = [[f64; 4]; 4];

pub fn identity() -> Mat4 {
    std::array::from_fn(|c| std::array::from_fn(|r| f64::from(u8::from(c == r))))
}
pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    std::array::from_fn(|c| {
        std::array::from_fn(|r| {
            a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3]
        })
    })
}
pub fn translation(t: [f64; 3]) -> Mat4 {
    let mut m = identity();
    m[3] = [t[0], t[1], t[2], 1.0];
    m
}
pub fn scaling(s: [f64; 3]) -> Mat4 {
    let mut m = identity();
    for (i, v) in s.into_iter().enumerate() {
        m[i][i] = v;
    }
    m
}
pub fn apply(m: &Mat4, p: [f64; 4]) -> [f64; 4] {
    std::array::from_fn(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r] * p[3])
}

/// The executable's quarter sine table (roses_assets::parse_pal_sine_table).
#[derive(Clone, Debug)]
pub struct SineTable(Vec<f32>);

impl SineTable {
    pub fn new(values: Vec<f32>) -> Result<Self, String> {
        if values.len() != PAL_SINE_TABLE_ENTRIES
            || values[0] != 0.0
            || values[PAL_SINE_TABLE_ENTRIES - 1] != 1.0
            || values.iter().any(|v| !v.is_finite())
        {
            return Err("sine table must hold 1025 values from 0 to 1".into());
        }
        Ok(Self(values))
    }
    /// sin of a 16-bit angle (0x10000 = 360 degrees), EE 0x20ACC0.
    pub fn sin(&self, angle: i64) -> f64 {
        let mut a = i64::from((angle & 0xffff) as u16 as i16);
        let mut sign = 1.0;
        if a < 0 {
            sign = -1.0;
            a = -a;
        }
        if a > 0x4000 {
            a = 0x8000 - a;
        }
        if a & 0x3fff == 0 {
            return if a == 0x4000 { sign } else { 0.0 * sign };
        }
        let i = (a >> 4) as usize;
        let f = (a & 15) as f64;
        sign * (f64::from(self.0[i]) * (16.0 - f) + f64::from(self.0[i + 1]) * f) / 16.0
    }
    /// EE 0x20ABF0.
    pub fn cos(&self, angle: i64) -> f64 {
        self.sin(angle + 0x4000)
    }
    /// EE 0x20D350: R = Rz(z) Ry(y) Rx(x) for column vectors.
    pub fn rotation(&self, r: [i32; 3]) -> Mat4 {
        let (cx, sx) = (self.cos(r[0].into()), self.sin(r[0].into()));
        let (cy, sy) = (self.cos(r[1].into()), self.sin(r[1].into()));
        let (cz, sz) = (self.cos(r[2].into()), self.sin(r[2].into()));
        [
            [cy * cz, cy * sz, -sy, 0.0],
            [sx * sy * cz - cx * sz, sx * sy * sz + cx * cz, sx * cy, 0.0],
            [cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cx * cy, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }
}

/// Binary search of 0x210480 (as the EE does it; past the end converges to
/// the last key).
fn find_key(frames: &[u32], t: f32) -> usize {
    let n = frames.len();
    let t = f64::from(t);
    let (mut lo, mut hi) = (0usize, n);
    let mut mid = n >> 1;
    while lo < mid {
        if mid + 1 < n && t < f64::from(frames[mid + 1]) {
            if t >= f64::from(frames[mid]) {
                return mid;
            }
            hi = mid;
        } else {
            lo = mid;
        }
        mid = (lo + hi) >> 1;
    }
    mid
}

fn weights(f0: u32, f1: u32, t: f32) -> (f32, f32) {
    let dt = (i64::from(f1) - i64::from(f0)) as f32;
    ((f1 as f32 - t) / dt, (t - f0 as f32) / dt)
}

/// One translation or scale channel at time `t`.
pub fn sample_vector(keys: &[MonsterVecKey], t: f32) -> [f32; 3] {
    if keys.len() < 3 {
        return keys[0].1;
    }
    let frames: Vec<u32> = keys.iter().map(|k| k.0).collect();
    let i = find_key(&frames, t);
    if i + 1 >= keys.len() {
        return keys[i].1;
    }
    let (w0, w1) = weights(keys[i].0, keys[i + 1].0, t);
    std::array::from_fn(|k| w0 * keys[i].1[k] + w1 * keys[i + 1].1[k])
}

/// EE 0x20C940: 16-bit shortest-path angle interpolation, truncated.
pub fn lerp_angle(a0: i32, a1: i32, w0: f32, w1: f32) -> i32 {
    let a0 = a0 & 0xffff;
    let mut a1 = a1 & 0xffff;
    let d = a1 - a0;
    if d >= 0x8001 {
        a1 -= 0x10000;
    } else if d < -0x8000 {
        a1 += 0x10000;
    }
    ((a0 as f32 * w0 + a1 as f32 * w1) + 0.5f32) as i32
}

/// One rotation channel at time `t`.
pub fn sample_angles(keys: &[MonsterAngleKey], t: f32) -> [i32; 3] {
    if keys.len() < 3 {
        return keys[0].1;
    }
    let frames: Vec<u32> = keys.iter().map(|k| k.0).collect();
    let i = find_key(&frames, t);
    if i + 1 >= keys.len() {
        return keys[i].1;
    }
    let (w0, w1) = weights(keys[i].0, keys[i + 1].0, t);
    std::array::from_fn(|k| lerp_angle(keys[i].1[k], keys[i + 1].1[k], w0, w1))
}

/// Node matrices of one evaluated pose.
#[derive(Clone, Debug)]
pub struct Pose {
    /// Per table index: node matrix without scale.
    pub nodes: Vec<Mat4>,
    /// Per table index: accumulated scale.
    pub scales: Vec<[f64; 3]>,
    /// Runtime matrix blocks by DMA-1 qword index: node matrix * diag(S).
    pub blocks: BTreeMap<u16, Mat4>,
}

/// A motion change being blended (ChangeMotion 0x1F2760 -> 0x1F2770):
/// the previous motion at its own time, and the weight of the current one.
#[derive(Clone, Copy, Debug)]
pub struct PoseBlend<'a> {
    pub previous: &'a MonsterMotion,
    pub time: f32,
    /// Weight of the current motion (0 = all previous, 1 = all current).
    pub weight: f32,
}

/// 0x20C9B0: blend two rotations component-wise; B is replaced by the
/// equivalent (x + pi, pi - y, z + pi) when that is closer to A [F
/// claude-models/NOTES.md; "closer" = smaller sum of wrapped component
/// distances, H].
pub fn blend_angles(a: [i32; 3], b: [i32; 3], w: f32) -> [i32; 3] {
    let wrap = |d: i32| {
        let d = d & 0xffff;
        if d > 0x8000 { 0x10000 - d } else { d }
    };
    let distance = |b: [i32; 3]| -> i32 { (0..3).map(|k| wrap(b[k] - a[k])).sum() };
    let alt = [b[0] + 0x8000, 0x8000 - b[1], b[2] + 0x8000];
    let b = if distance(alt) < distance(b) { alt } else { b };
    std::array::from_fn(|k| lerp_angle(a[k], b[k], 1.0 - w, w))
}

/// Evaluate `motion` (None = the bind pose of the node table) at time `t`
/// (motion frames, the draw's pre-update time).
pub fn evaluate_pose(
    model: &MonsterModel,
    motion: Option<&MonsterMotion>,
    t: f32,
    table: &SineTable,
) -> Result<Pose, String> {
    evaluate_pose_blended(model, motion, t, None, table)
}

/// [`evaluate_pose`] during a motion change: each node's translation and
/// scale are lerped and its rotation blended ([`blend_angles`]) between the
/// previous motion and the current one.
pub fn evaluate_pose_blended(
    model: &MonsterModel,
    motion: Option<&MonsterMotion>,
    t: f32,
    blend: Option<PoseBlend<'_>>,
    table: &SineTable,
) -> Result<Pose, String> {
    let n = model.nodes.len();
    if n == 0 {
        return Err("model has no nodes".into());
    }
    let mut pose = Pose {
        nodes: vec![identity(); n],
        scales: vec![[1.0; 3]; n],
        blocks: BTreeMap::new(),
    };
    let mut counter = 0usize;
    let mut visited = 0usize;
    // Explicit stack of (first sibling, parent scale, parent matrix).
    let mut stack: Vec<(Option<u16>, [f64; 3], Mat4)> = vec![(Some(0), [1.0; 3], identity())];
    while let Some((mut current, parent_scale, parent)) = stack.pop() {
        while let Some(index) = current {
            let index = usize::from(index);
            visited += 1;
            if visited > n {
                return Err("node links cycle".into());
            }
            let node = &model.nodes[index];
            let mut scale = node.scale.map(f64::from);
            let mut rotation = node.rotation;
            let mut trans = node.translation.map(f64::from);
            let mut flags = node.flags;
            let ts = if counter == 0 {
                1.0f32
            } else {
                model.inv_root_scale
            };
            // One motion's values for this node (the bind values where it
            // has no channel).
            let sample = |motion: &MonsterMotion, t: f32, flags: &mut u16| {
                let mut trans = node.translation.map(f64::from);
                let mut rotation = node.rotation;
                let mut scale = node.scale.map(f64::from);
                let track = &motion.tracks[counter];
                if let Some(keys) = &track.translation {
                    *flags &= !1;
                    trans = sample_vector(keys, t).map(|v| f64::from(v * ts));
                }
                if let Some(keys) = &track.rotation {
                    *flags &= !2;
                    rotation = sample_angles(keys, t);
                }
                if let Some(keys) = &track.scale {
                    *flags &= !4;
                    scale = sample_vector(keys, t).map(f64::from);
                }
                if counter == 0 && motion.kind & 4 != 0 {
                    scale = node.scale.map(f64::from);
                }
                (trans, rotation, scale)
            };
            if let Some(motion) = motion.filter(|m| counter < usize::from(m.node_count)) {
                (trans, rotation, scale) = sample(motion, t, &mut flags);
            }
            if let Some(b) = blend.filter(|b| counter < usize::from(b.previous.node_count)) {
                let (pt, pr, ps) = sample(b.previous, b.time, &mut flags);
                let w = f64::from(b.weight);
                trans = std::array::from_fn(|k| pt[k] + (trans[k] - pt[k]) * w);
                scale = std::array::from_fn(|k| ps[k] + (scale[k] - ps[k]) * w);
                rotation = blend_angles(pr, rotation, b.weight);
            }
            counter += 1;
            let mut s = parent_scale;
            let mut m = parent;
            if flags & 3 != 3 {
                let tv: [f64; 3] = std::array::from_fn(|k| trans[k] * parent_scale[k]);
                if flags & 1 == 0 {
                    m = mul(&m, &translation(tv));
                }
                if flags & 2 == 0 {
                    m = mul(&m, &table.rotation(rotation));
                }
            }
            if flags & 4 == 0 {
                s = std::array::from_fn(|k| s[k] * scale[k]);
            }
            pose.nodes[index] = m;
            pose.scales[index] = s;
            if node.matrix_block != 0 {
                pose.blocks.insert(node.matrix_block, mul(&m, &scaling(s)));
            }
            if node.child.is_some() {
                stack.push((node.sibling, parent_scale, parent));
                // Continue with the child subtree; siblings resume later with
                // this node's parent state (pushed above).
                stack.push((node.child, s, m));
                current = None;
            } else {
                current = node.sibling;
            }
        }
    }
    Ok(pose)
}

/// Unit object state read by 0x1EF8F0 (object types 4-6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitObject {
    pub position: [f32; 3],
    pub rotation: [i32; 3],
    pub scale: [f32; 3],
    pub rotation2: [i32; 3],
    /// obj+0x1E8: clamp(40 / node-0 radius, 0.3, 1).
    pub grow: f32,
    /// obj+0x1FF (0..=32): 32 = recentred on the cell and normalised.
    pub recentre: u8,
}

/// obj+0x1E8 for a model: clamp(40 / node-0 bounding radius, 0.3, 1).
pub fn grow_factor(model: &MonsterModel) -> f32 {
    let radius = model.nodes.first().map_or(0.0, |n| n.bounding_sphere[3]);
    if radius > 0.0 {
        (40.0f32 / radius).clamp(0.3, 1.0)
    } else {
        1.0
    }
}

impl UnitObject {
    /// A unit standing on board cell (column, row): position
    /// ((c - 3) 50, 50, (r - 3) 50), rotation Y 0x8000 for side 0 and 0 for
    /// side 1, scale 1, fully recentred (board notes, round-2 savestate).
    pub fn board_unit(model: &MonsterModel, column: u8, row: u8, side: u8) -> Self {
        Self {
            position: [
                (f32::from(column) - 3.0) * 50.0,
                50.0,
                (f32::from(row) - 3.0) * 50.0,
            ],
            rotation: [0, if side == 0 { 0x8000 } else { 0 }, 0],
            scale: [1.0; 3],
            rotation2: [0; 3],
            grow: grow_factor(model),
            recentre: 32,
        }
    }
}

/// EE 0x1EF8F0: W = T(pos) R(rot) T(c') R(rot2) S' T(-c) with c = node 1's
/// translation / RH0 f32 @0x64 (when node 1 is node 0's child), k =
/// recentre / 32, S' = scale (1 + (grow - 1) k), c' = (c.x (1 - k), c.y,
/// c.z (1 - k)).
pub fn object_matrix(model: &MonsterModel, object: &UnitObject, table: &SineTable) -> Mat4 {
    let c: [f64; 3] = match (model.nodes.first(), model.nodes.get(1)) {
        (Some(root), Some(n1)) if root.child == Some(1) => {
            let inv = f64::from(model.inv_root_scale);
            n1.translation.map(|v| f64::from(v) / inv)
        }
        _ => [0.0; 3],
    };
    let k = f64::from(object.recentre) / 32.0;
    let f = 1.0 + (f64::from(object.grow) - 1.0) * k;
    let s = object.scale.map(|v| f64::from(v) * f);
    let cp = [c[0] * (1.0 - k), c[1], c[2] * (1.0 - k)];
    let mut w = translation(object.position.map(f64::from));
    w = mul(&w, &table.rotation(object.rotation));
    w = mul(&w, &translation(cp));
    w = mul(&w, &table.rotation(object.rotation2));
    w = mul(&w, &scaling(s));
    mul(&w, &translation(c.map(|v| -v)))
}

/// Motion time of one object (obj+0x198 block, 0x1EE6E0 per update).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionClock {
    pub motion: u8,
    /// Motion length in frames (MOT header).
    pub length: u32,
    pub time: f32,
    /// obj+0x198 (1.0 in every surveyed state).
    pub speed: f32,
    /// obj+0x1F7 bits 6-7: 1 forward, -1 backward, 0 stopped.
    pub direction: i8,
    /// obj+0x1FD: updates held at frame 10 of motion 6.
    pub hold: u8,
}

impl MotionClock {
    /// SetMotion 0x1F2A40 (forward): time 1, hold counter 0.
    pub fn new(motion: u8, length: u32) -> Self {
        Self {
            motion,
            length,
            time: 1.0,
            speed: 1.0,
            direction: 1,
            hold: 0,
        }
    }
    /// One update of `vsyncs` frames. Loops restart at frame 1; motions 3
    /// and 4 stop at L - 1; motion 6 holds for 15 updates once t >= 10.
    /// Returns true when a forward loop wrapped (where 0x1EE6E0 would switch
    /// to a queued motion).
    pub fn advance(&mut self, vsyncs: u32) -> bool {
        let end = self.length as f32 - 1.0;
        let mut dt = self.speed * vsyncs as f32;
        if self.direction == 0 {
            return false;
        }
        match self.motion {
            3 | 4 if self.time >= end || self.time.is_nan() => {
                self.direction = 0;
                return false;
            }
            6 if self.time >= 10.0 && self.hold < 15 => {
                self.hold += 1;
                dt = 0.0;
            }
            _ => {}
        }
        if self.direction == 1 {
            if self.time < end {
                let next = self.time + dt;
                self.time = if next <= end { next } else { end };
                false
            } else {
                self.time = 1.0;
                true
            }
        } else {
            if self.time <= 1.0 {
                self.time = end;
                return true;
            }
            let next = self.time - dt;
            self.time = if next < 1.0 { 1.0 } else { next };
            false
        }
    }
}

/// Unit lighting (camera object light set type 4 + object +0x120).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitLight {
    /// World-space direction (the vertex term is max(0, n . direction)).
    pub direction: [f64; 3],
    pub colour: [f64; 3],
    pub diffuse_scale: f64,
    pub ambient_scale: f64,
}

/// Board units: light 0 = 0.5 (-0.7071, 0.7071, 0), lights 1-2 off, object
/// scales (diffuse 1.6, ambient 0.4) as stored f32 values.
pub const BOARD_UNIT_LIGHT: UnitLight = UnitLight {
    direction: [-0.707_106_769_084_930_4, 0.707_106_769_084_930_4, 0.0],
    colour: [0.5; 3],
    diffuse_scale: 1.600_000_023_841_858,
    ambient_scale: 0.400_000_005_960_464_5,
};

/// Per-vertex colour in GS units: floor(min(max, a g ambient + d g diffuse
/// max(0, n . L) colour)) per channel, alpha = diffuse lane 3.
pub fn vertex_colour(material: &[f32; 12], normal: [f64; 3], light: &UnitLight, g: f64) -> [u8; 4] {
    let dot = (normal[0] * light.direction[0]
        + normal[1] * light.direction[1]
        + normal[2] * light.direction[2])
        .max(0.0);
    let channel = |c: usize| {
        let value = light.ambient_scale * g * f64::from(material[c])
            + light.diffuse_scale * g * f64::from(material[4 + c]) * dot * light.colour[c];
        value
            .min(f64::from(material[8 + c]))
            .floor()
            .clamp(0.0, 255.0) as u8
    };
    [
        channel(0),
        channel(1),
        channel(2),
        f64::from(material[7]).floor().clamp(0.0, 255.0) as u8,
    ]
}

/// One skinned strip in world space.
#[derive(Clone, Debug, Default)]
pub struct SkinnedStrip {
    pub prim: u16,
    /// World positions (the weighted homogeneous sum divided by its w).
    pub positions: Vec<[f64; 3]>,
    pub normals: Vec<[f64; 3]>,
    pub colours: Vec<[u8; 4]>,
    /// S, T = raw / 4096.
    pub uv: Vec<[f32; 2]>,
}

#[derive(Clone, Debug)]
pub struct SkinnedDraw {
    pub pass: u8,
    pub draw: usize,
    pub strips: Vec<SkinnedStrip>,
}

/// World matrices W * block of one draw's VU slots, and their rotation parts
/// with normalised columns.
fn draw_matrices(
    draw: &MonsterDraw,
    pose: &Pose,
    world: &Mat4,
) -> Result<BTreeMap<u16, (Mat4, [[f64; 3]; 3])>, String> {
    let mut out = BTreeMap::new();
    for slot in &draw.matrix_slots {
        let block = pose
            .blocks
            .get(&slot.matrix_block)
            .ok_or_else(|| format!("matrix block {} has no node", slot.matrix_block))?;
        let m = mul(world, block);
        let rotation: [[f64; 3]; 3] = std::array::from_fn(|c| {
            let norm = (m[c][0] * m[c][0] + m[c][1] * m[c][1] + m[c][2] * m[c][2]).sqrt();
            std::array::from_fn(|r| if norm > 0.0 { m[c][r] / norm } else { 0.0 })
        });
        out.insert(slot.vu_address, (m, rotation));
    }
    Ok(out)
}

/// Skin every strip of the model with `pose` placed by `world`, and light it.
pub fn skin_model(
    model: &MonsterModel,
    pose: &Pose,
    world: &Mat4,
    light: &UnitLight,
    brightness: f64,
) -> Result<Vec<SkinnedDraw>, String> {
    let mut out = Vec::new();
    for pass in &model.passes {
        for (index, draw) in pass.draws.iter().enumerate() {
            let matrices = draw_matrices(draw, pose, world)?;
            let material = draw
                .material()
                .ok_or_else(|| format!("pass {} draw {index} has no material", pass.pass))?;
            let mut strips = Vec::with_capacity(draw.strips.len());
            for strip in &draw.strips {
                let mut skinned = SkinnedStrip {
                    prim: strip.prim,
                    ..Default::default()
                };
                for vertex in &strip.vertices {
                    let mut p = [0.0f64; 4];
                    let mut n = [0.0f64; 3];
                    for influence in &vertex.influences {
                        let (m, r) = matrices
                            .get(&influence.matrix_vu)
                            .ok_or("influence names an unloaded VU slot")?;
                        let w = f64::from(influence.weight_raw) / 4096.0;
                        let q = apply(
                            m,
                            [
                                f64::from(influence.position_raw[0]),
                                f64::from(influence.position_raw[1]),
                                f64::from(influence.position_raw[2]),
                                1.0,
                            ],
                        );
                        for k in 0..4 {
                            p[k] += w * q[k];
                        }
                        let raw = influence.normal_raw.map(|v| f64::from(v) / 32768.0);
                        for k in 0..3 {
                            n[k] += w * (r[0][k] * raw[0] + r[1][k] * raw[1] + r[2][k] * raw[2]);
                        }
                    }
                    if p[3] == 0.0 {
                        return Err("vertex weights sum to zero".into());
                    }
                    skinned
                        .positions
                        .push([p[0] / p[3], p[1] / p[3], p[2] / p[3]]);
                    skinned.normals.push(n);
                    skinned
                        .colours
                        .push(vertex_colour(&material, n, light, brightness));
                    skinned.uv.push([
                        f32::from(vertex.texture_raw[0]) / 4096.0,
                        f32::from(vertex.texture_raw[1]) / 4096.0,
                    ]);
                }
                strips.push(skinned);
            }
            out.push(SkinnedDraw {
                pass: pass.pass,
                draw: index,
                strips,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use roses_assets::{MonsterNode, MonsterTrack};

    fn table() -> SineTable {
        SineTable::new(
            (0..PAL_SINE_TABLE_ENTRIES)
                .map(|i| {
                    if i == PAL_SINE_TABLE_ENTRIES - 1 {
                        1.0
                    } else {
                        ((i as f64) * std::f64::consts::FRAC_PI_2 / 1024.0).sin() as f32
                    }
                })
                .collect(),
        )
        .unwrap()
    }

    fn node(child: Option<u16>, sibling: Option<u16>, block: u16) -> MonsterNode {
        MonsterNode {
            scale: [1.0; 3],
            flags: 0,
            matrix_block: block,
            rotation: [0; 3],
            child,
            sibling,
            translation: [0.0; 3],
            bounding_sphere: [0.0, 0.0, 0.0, 20.0],
        }
    }

    fn model(nodes: Vec<MonsterNode>) -> MonsterModel {
        MonsterModel {
            schema_version: 1,
            profile_id: String::new(),
            card: 0,
            archive_offset: 0,
            slot_sha256: String::new(),
            flags: 0,
            rh0_bytes: 0,
            header_end: 0,
            raw_header_words: Vec::new(),
            sections: Vec::new(),
            inv_root_scale: 100.0,
            matrix_blocks: 4,
            nodes,
            textures: Vec::new(),
            passes: Vec::new(),
            motions: Vec::new(),
            effect_textures: Vec::new(),
            camera_records: Vec::new(),
            sound_cues: Vec::new(),
            used_slot_bytes: 0,
            unresolved: Vec::new(),
            rendering_validated: false,
        }
    }

    #[test]
    fn sine_table_follows_the_quarter_wave_rules() {
        let t = table();
        assert_eq!(t.sin(0x4000), 1.0);
        assert_eq!(t.sin(-0x4000), -1.0);
        assert_eq!(t.sin(0x8000), 0.0);
        assert_eq!(t.cos(0), 1.0);
        assert!((t.sin(0x2000) - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-7);
        // 1/16 step interpolation between table entries.
        let a = t.sin(0x10);
        let b = t.sin(0x18);
        assert!((b - (a + t.sin(0x20)) / 2.0).abs() < 1e-9);
        // Wraps by 16 bits.
        assert_eq!(t.sin(0x1_4000), 1.0);
        let r = t.rotation([0, 0x4000, 0]);
        // Ry(90): x axis maps to -z.
        assert!((r[0][2] + 1.0).abs() < 1e-12 && r[0][0].abs() < 1e-12);
    }

    #[test]
    fn keys_interpolate_like_the_ee() {
        let keys = [
            MonsterVecKey(0, [0.0, 10.0, 0.0]),
            MonsterVecKey(4, [4.0, 10.0, 8.0]),
            MonsterVecKey(8, [8.0, 0.0, 8.0]),
        ];
        assert_eq!(sample_vector(&keys, 2.0), [2.0, 10.0, 4.0]);
        assert_eq!(sample_vector(&keys, 6.0), [6.0, 5.0, 8.0]);
        assert_eq!(sample_vector(&keys, 9.0), [8.0, 0.0, 8.0]);
        // Fewer than three keys: constant key 0.
        assert_eq!(sample_vector(&keys[..2], 3.0), [0.0, 10.0, 0.0]);
        // Shortest path across the 16-bit wrap: 0xF000 -> 0x1000 via 0.
        let angles = [
            MonsterAngleKey(0, [0xF000, 0, 0]),
            MonsterAngleKey(2, [0x1000, 0, 0]),
            MonsterAngleKey(4, [0x1000, 0, 0]),
        ];
        // Midway between 0xF000 and 0x11000: 0x10000 (= 0 in 16 bits).
        assert_eq!(sample_angles(&angles, 1.0)[0], 0x10000);
        assert_eq!(lerp_angle(0, 3, 0.5, 0.5), 2);
        assert_eq!(lerp_angle(0, -3, 0.5, 0.5), -1);
    }

    #[test]
    fn preorder_evaluation_scales_children_and_fills_blocks() {
        // 0 -> 1 (block 1) -> 2 (block 8); 1's sibling 3 (block 15).
        let mut nodes = vec![
            node(Some(1), None, 0),
            node(Some(2), Some(3), 1),
            node(None, None, 8),
            node(None, None, 15),
        ];
        nodes[0].scale = [2.0; 3];
        nodes[1].translation = [1.0, 0.0, 0.0];
        nodes[2].translation = [0.0, 3.0, 0.0];
        nodes[3].translation = [0.0, 0.0, 5.0];
        let m = model(nodes);
        let pose = evaluate_pose(&m, None, 0.0, &table()).unwrap();
        // Translations are in the parent's accumulated-scale units.
        assert_eq!(pose.nodes[1][3], [2.0, 0.0, 0.0, 1.0]);
        assert_eq!(pose.nodes[2][3], [2.0, 6.0, 0.0, 1.0]);
        assert_eq!(pose.nodes[3][3], [0.0, 0.0, 10.0, 1.0]);
        assert_eq!(pose.blocks[&8][0][0], 2.0);
        assert_eq!(pose.blocks.len(), 3);
        // A motion track overrides node 1's translation (times 1/root scale).
        let motion = MonsterMotion {
            motion: 0,
            slot_offset: 0,
            byte_len: 0,
            raw_header_words: [0; 8],
            node_count: 2,
            length: 4,
            kind: 3,
            channels_raw: 0x42,
            header_word3_raw: 0,
            node_map_raw: vec![0xffff; 2],
            tracks: vec![
                MonsterTrack::default(),
                MonsterTrack {
                    translation: Some(vec![MonsterVecKey(0, [0.0, 0.0, 300.0])]),
                    rotation: None,
                    scale: None,
                },
            ],
        };
        let posed = evaluate_pose(&m, Some(&motion), 1.0, &table()).unwrap();
        assert_eq!(posed.nodes[1][3], [0.0, 0.0, 300.0 * 100.0 * 2.0, 1.0]);
    }

    #[test]
    fn object_matrix_recentres_on_node_one() {
        let mut nodes = vec![node(Some(1), None, 0), node(None, None, 1)];
        nodes[1].translation = [100.0, 200.0, 300.0];
        nodes[0].bounding_sphere[3] = 80.0;
        let m = model(nodes);
        assert_eq!(grow_factor(&m), 0.5);
        let unit = UnitObject::board_unit(&m, 3, 6, 1);
        assert_eq!(unit.position, [0.0, 50.0, 150.0]);
        let w = object_matrix(&m, &unit, &table());
        // Model centre c = (1, 2, 3) lands at (0, 2, 0) above the position
        // (x/z recentred, y kept), scaled by grow.
        let centre = apply(&w, [1.0, 2.0, 3.0, 1.0]);
        assert!((centre[0]).abs() < 1e-12 && (centre[1] - 52.0).abs() < 1e-12);
        assert!((centre[2] - 150.0).abs() < 1e-12);
        assert!((w[0][0] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn motion_clock_loops_stops_and_holds() {
        let mut idle = MotionClock::new(0, 5);
        assert_eq!(idle.time, 1.0);
        for expected in [2.0, 3.0, 4.0] {
            assert!(!idle.advance(1));
            assert_eq!(idle.time, expected);
        }
        // At L - 1 the loop restarts at frame 1.
        assert!(idle.advance(1));
        assert_eq!(idle.time, 1.0);
        // Fractional steps clamp to L - 1 first.
        let mut fast = MotionClock::new(0, 5);
        fast.speed = 2.5;
        fast.advance(1);
        assert_eq!(fast.time, 3.5);
        fast.advance(1);
        assert_eq!(fast.time, 4.0);
        let mut damage = MotionClock::new(4, 3);
        damage.advance(1);
        damage.advance(1);
        damage.advance(1);
        assert_eq!((damage.time, damage.direction), (2.0, 0));
        let mut appear = MotionClock::new(6, 21);
        for _ in 0..9 {
            appear.advance(1);
        }
        assert_eq!(appear.time, 10.0);
        for _ in 0..15 {
            appear.advance(1);
        }
        assert_eq!(appear.time, 10.0);
        appear.advance(1);
        assert_eq!(appear.time, 11.0);
    }

    #[test]
    fn lighting_uses_min_of_maximum_and_scaled_terms() {
        let material = [
            127.0, 127.0, 127.0, 0.0, 178.0, 178.0, 178.0, 128.0, 255.0, 255.0, 255.0, 0.0,
        ];
        let away = vertex_colour(&material, [1.0, 0.0, 0.0], &BOARD_UNIT_LIGHT, 1.0);
        // 0.4 * 127 = 50.8 -> 50.
        assert_eq!(away, [50, 50, 50, 128]);
        let facing = vertex_colour(
            &material,
            [
                -std::f64::consts::FRAC_1_SQRT_2,
                std::f64::consts::FRAC_1_SQRT_2,
                0.0,
            ],
            &BOARD_UNIT_LIGHT,
            1.0,
        );
        // 50.8 + 1.6 * 178 * 0.5 = 193.2 -> 193.
        assert_eq!(facing[0], 193);
        let capped = vertex_colour(
            &[
                200.0, 0.0, 0.0, 0.0, 255.0, 0.0, 0.0, 64.0, 100.0, 0.0, 0.0, 0.0,
            ],
            [-0.7, 0.7, 0.0],
            &BOARD_UNIT_LIGHT,
            1.0,
        );
        assert_eq!(capped, [100, 0, 0, 64]);
    }
}
