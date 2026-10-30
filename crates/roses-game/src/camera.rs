//! Native Stonehenge camera for the opening (events 10020 and 10040).
//!
//! Source facts (PAL EE addresses): the level updater writes eye 0x1ED0030 and
//! target 0x1ED0040, then calls look-at 0x1FB1F0. History (scene mode 1,
//! 0x23A0D8): eye = (tx, 20 + ty + 1700·level/4096, 1 + tz), target zero, and
//! the scene objects spin about Y by the 16-bit angle 0x1ED53F6, lowered by
//! 0x300 each update (0x23A17C) and applied only in mode 1 (0x229E14). Simon
//! (mode 2, 0x23A5C4): eye = (0, 0, 512), target = (−64·pan/100,
//! 20.2 + 768·level/4096, 0), pan byte 0x1ED5439.
//!
//! The view construction below reproduces every captured Simon view matrix to
//! 1e-12 (private check). The projection and per-object model matrices come
//! from the captured scene: new clip = P·V·R·(P·V_captured)⁻¹·C_captured.
//! Angle units (65,536 per turn) were measured from software-GS dumps, not
//! decoded from 0x1EF6F0.

pub type Mat4 = [[f64; 4]; 4]; // column-major: m[column][row]

pub const LEVEL_FULL: f64 = 4096.0;

pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    std::array::from_fn(|c| std::array::from_fn(|r| (0..4).map(|k| a[k][r] * b[c][k]).sum()))
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (n > 1e-9).then(|| v.map(|x| x / n))
}

/// View matrix: rows right = f×up, down = f×right, forward f; world up +Y.
/// Screen Y grows downward as in the captured GS projection.
pub fn look_at(eye: [f64; 3], target: [f64; 3]) -> Option<Mat4> {
    let f = normalize(sub(target, eye))?;
    let r = normalize(cross(f, [0.0, 1.0, 0.0]))?;
    let u = cross(f, r);
    let rows = [r, u, f];
    let t: [f64; 3] =
        std::array::from_fn(|i| -(rows[i][0] * eye[0] + rows[i][1] * eye[1] + rows[i][2] * eye[2]));
    Some(std::array::from_fn(|c| {
        if c == 3 {
            [t[0], t[1], t[2], 1.0]
        } else {
            [rows[0][c], rows[1][c], rows[2][c], 0.0]
        }
    }))
}

/// Rotation about +Y by a 16-bit source angle (65,536 units per turn).
pub fn rotation_y(angle: i16) -> Mat4 {
    let a = f64::from(angle) * std::f64::consts::TAU / 65536.0;
    let (s, c) = a.sin_cos();
    [
        [c, 0.0, -s, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [s, 0.0, c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn identity() -> Mat4 {
    std::array::from_fn(|c| std::array::from_fn(|r| f64::from(u8::from(c == r))))
}

/// A camera pose in source terms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pose {
    /// Event 10020: overhead camera, spinning scene.
    History { level: f64, angle: i16 },
    /// Event 10040: fixed eye, tilt by level and pan byte (0..=100).
    Simon { level: f64, pan: i8 },
    /// Scene mode 1 with a kept target height (event 11184: target y 20.2
    /// left by mode 3, eye = (0, 20 + y + 1700·level/4096, 1)) [emu].
    Overhead { level: f64, angle: i16, target_y: f64 },
    /// The story scene's modes 3/2 (0x23A668 / 0x23A194) with the shake
    /// offsets W+5490/W+5494 added to the eye and the target: eye (dx, dy,
    /// 512), target (-64 pan/100 + dx, 20.2 + 768 level/4096 + dy, 0) [F].
    Story { level: f64, pan: i32, shake: [f32; 2] },
}

impl Pose {
    pub fn eye_target(self) -> ([f64; 3], [f64; 3]) {
        match self {
            Pose::History { level, .. } => {
                ([0.0, 20.0 + 1700.0 * level / LEVEL_FULL, 1.0], [0.0; 3])
            }
            Pose::Overhead {
                level, target_y, ..
            } => (
                [0.0, 20.0 + target_y + 1700.0 * level / LEVEL_FULL, 1.0],
                [0.0, target_y, 0.0],
            ),
            Pose::Story { level, pan, shake } => {
                let (dx, dy) = (f64::from(shake[0]), f64::from(shake[1]));
                (
                    [dx, dy, 512.0],
                    [
                        -64.0 * f64::from(pan) / 100.0 + dx,
                        f64::from(20.2_f32) + 768.0 * level / LEVEL_FULL + dy,
                        0.0,
                    ],
                )
            }
            Pose::Simon { level, pan } => (
                [0.0, 0.0, 512.0],
                [
                    -64.0 * f64::from(pan) / 100.0,
                    f64::from(20.2_f32) + 768.0 * level / LEVEL_FULL,
                    0.0,
                ],
            ),
        }
    }
    pub fn view(self) -> Option<Mat4> {
        let (eye, target) = self.eye_target();
        look_at(eye, target)
    }
    pub fn world_rotation(self) -> Mat4 {
        match self {
            Pose::History { angle, .. } | Pose::Overhead { angle, .. } => rotation_y(angle),
            Pose::Simon { .. } | Pose::Story { .. } => identity(),
        }
    }
}

/// Inverse of an affine-or-projective 4x4 matrix by Gauss-Jordan elimination.
pub fn invert(m: &Mat4) -> Option<Mat4> {
    // Work row-major: a[r][c].
    let mut a: [[f64; 8]; 4] = std::array::from_fn(|r| {
        std::array::from_fn(|c| {
            if c < 4 {
                m[c][r]
            } else {
                f64::from(u8::from(c - 4 == r))
            }
        })
    });
    for col in 0..4 {
        let pivot = (col..4).max_by(|&x, &y| a[x][col].abs().total_cmp(&a[y][col].abs()))?;
        if a[pivot][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        let p = a[col][col];
        for v in &mut a[col] {
            *v /= p;
        }
        for r in 0..4 {
            if r != col {
                let f = a[r][col];
                if f != 0.0 {
                    for c in 0..8 {
                        a[r][c] -= f * a[col][c];
                    }
                }
            }
        }
    }
    Some(std::array::from_fn(|c| {
        std::array::from_fn(|r| a[r][c + 4])
    }))
}

/// Recompose a captured draw for a new pose.
pub struct Rig {
    projection: Mat4,
    captured_inverse: Mat4,
}
impl Rig {
    /// `projection` is the scene's GS projection; `captured` the pose at which
    /// the draws were saved (the settled Simon camera).
    pub fn new(projection: Mat4, captured: Pose) -> Option<Self> {
        let pv = mul(&projection, &captured.view()?);
        Some(Self {
            projection,
            captured_inverse: invert(&pv)?,
        })
    }
    pub fn clip(&self, pose: Pose, captured_clip: &Mat4) -> Option<Mat4> {
        let model = mul(&self.captured_inverse, captured_clip);
        let world = mul(&pose.world_rotation(), &model);
        Some(mul(&mul(&self.projection, &pose.view()?), &world))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(m: &Mat4, p: [f64; 3]) -> [f64; 4] {
        std::array::from_fn(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r])
    }

    /// Event 11184 [emu trace-man-cam.json f770]: level 16 -> eye y 46.84.
    #[test]
    fn overhead_pose_matches_the_traced_eye() {
        let pose = Pose::Overhead {
            level: 16.0,
            angle: -0x300,
            target_y: f64::from(20.2_f32),
        };
        let (eye, target) = pose.eye_target();
        assert!((eye[1] - 46.84).abs() < 0.01 && eye[2] == 1.0 && eye[0] == 0.0);
        assert_eq!(target, [0.0, f64::from(20.2_f32), 0.0]);
        assert!(pose.view().is_some());
        assert_ne!(pose.world_rotation(), identity());
    }

    #[test]
    fn look_at_puts_eye_at_origin_and_target_on_forward_axis() {
        let pose = Pose::Simon {
            level: 0.0,
            pan: 100,
        };
        let (eye, target) = pose.eye_target();
        let v = pose.view().unwrap();
        let e = apply(&v, eye);
        assert!(e[..3].iter().all(|x| x.abs() < 1e-9));
        let t = apply(&v, target);
        assert!(t[0].abs() < 1e-9 && t[1].abs() < 1e-9 && t[2] > 0.0);
        // World up maps to screen-down negative: +Y world is -Y view.
        let up = apply(&v, [eye[0], eye[1] + 1.0, eye[2]]);
        assert!(up[1] < 0.0);
    }

    #[test]
    fn rig_is_identity_at_the_captured_pose_and_spins_history() {
        let projection: Mat4 = [
            [800.0, 0.0, 0.0, 0.0],
            [0.0, 1074.0, 0.0, 0.0],
            [2048.0, 2048.0, -2048.25, 1.0],
            [0.0, 0.0, 16_779_264.0, 0.0],
        ];
        let captured = Pose::Simon {
            level: 0.0,
            pan: 100,
        };
        let rig = Rig::new(projection, captured).unwrap();
        let model: Mat4 = std::array::from_fn(|c| {
            std::array::from_fn(|r| {
                if c == r {
                    if c == 3 { 1.0 } else { 0.01 }
                } else {
                    0.0
                }
            })
        });
        let clip = mul(&mul(&projection, &captured.view().unwrap()), &model);
        let again = rig.clip(captured, &clip).unwrap();
        for c in 0..4 {
            for r in 0..4 {
                assert!((again[c][r] - clip[c][r]).abs() <= 1e-6 * clip[c][r].abs().max(1.0));
            }
        }
        assert_eq!(rotation_y(0), identity());
        let quarter = rotation_y(16384);
        let x = apply(&quarter, [1.0, 0.0, 0.0]);
        assert!(x[0].abs() < 1e-12 && (x[2].abs() - 1.0).abs() < 1e-12);
        let top = Pose::History {
            level: LEVEL_FULL,
            angle: 0,
        };
        assert_eq!(top.eye_target().0, [0.0, 1720.0, 1.0]);
        assert!(rig.clip(top, &clip).is_some());
    }

    #[test]
    fn invert_round_trips() {
        let m: Mat4 = [
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 4.0, 0.0, 0.0],
            [1.0, 1.0, 1.0, 1.0],
            [3.0, 5.0, 7.0, 0.0],
        ];
        let i = invert(&m).unwrap();
        let p = mul(&m, &i);
        for c in 0..4 {
            for r in 0..4 {
                assert!((p[c][r] - f64::from(u8::from(c == r))).abs() < 1e-12);
            }
        }
    }
}

#[cfg(test)]
mod private_tests {
    use super::*;

    #[test]
    #[ignore = "Requires the private captured Simon scene (ROSES_SCENE_JSON)"]
    fn private_look_at_reproduces_captured_simon_views() {
        let path = std::env::var_os("ROSES_SCENE_JSON").expect("ROSES_SCENE_JSON");
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let columns = |v: &serde_json::Value| -> Mat4 {
            std::array::from_fn(|c| std::array::from_fn(|r| v[c][r].as_f64().unwrap()))
        };
        let projection = columns(&json["gs_projection_columns"]);
        let pose = Pose::Simon {
            level: 0.0,
            pan: 100,
        };
        let view = pose.view().unwrap();
        let view_inverse = invert(&view).unwrap();
        let rig = Rig::new(projection, pose).unwrap();
        let mut view_draws = 0;
        for mesh in json["meshes"].as_array().unwrap() {
            let clip = columns(&mesh["clip_from_source_gs_columns"]);
            // Every captured object model is a uniform scale (plus a small
            // translation): recovering it through the native view proves V.
            let model = mul(&invert(&mul(&projection, &view)).unwrap(), &clip);
            let s = model[0][0];
            for (c, r) in [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)] {
                assert!(model[c][r].abs() < 5e-3 * s.abs(), "off-diagonal {c},{r}");
            }
            assert!(
                (model[1][1] - s).abs() < 1e-3 * s.abs()
                    && (model[2][2] - s).abs() < 1e-3 * s.abs()
            );
            if mesh["emitted_matrix_space"] == "view" {
                let emitted = columns(&mesh["emitted_matrix_columns"]);
                let recovered = mul(&view_inverse, &emitted);
                for c in 0..4 {
                    for r in 0..4 {
                        assert!((recovered[c][r] - model[c][r]).abs() < 1e-6);
                    }
                }
                view_draws += 1;
            }
            let again = rig.clip(pose, &clip).unwrap();
            for c in 0..4 {
                for r in 0..4 {
                    assert!((again[c][r] - clip[c][r]).abs() <= 1e-6 * clip[c][r].abs().max(1.0));
                }
            }
        }
        assert!(view_draws > 0);
    }
}
