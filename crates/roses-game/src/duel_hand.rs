//! The duel hand row's motion (states 0x3F-0x42 open and draw-in, 0x40
//! close, 0x44-0x45 pick and its cancel) as presentation over the input
//! machine, which skips those states [F/emu
//! the project notes part 1]:
//! - six slot animators lerp linearly with t = counter / 128 (no easing);
//! - open: 8 updates, d45 += 16, fy = 512 - 504 d45 / 128; cards drawn since
//!   the row last showed wait at fx = 606 and fly in one at a time over 3
//!   updates (counter 68, 102, 128);
//! - close: the open reversed (8 updates), the last one draws nothing;
//! - pick: 15 updates (d47 += 2, counter = d47 * 128 / 30, d45 -= 16): the
//!   others fall to fy 512, the picked card moves to (0, 8) (a fusion's
//!   first pick to (-101, 8), the next to (101 + 16 (k - 2), 8)) turning once
//!   about the screen-vertical axis, showing the large face from its first
//!   back-facing update; cancel plays it backwards.
//! Card x at rest: -202 + 101 i (five cards; fewer are centred as
//! `settled_hand_positions` does [H]).
use std::f32::consts::TAU;

/// One hand card's pose: hand index, position floats (fx, fy), turn angle
/// (radians; 0 = facing front) and whether the large face is shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandPose {
    pub index: usize,
    pub position: [f32; 2],
    pub theta: f32,
    pub large: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandKind {
    Open,
    Close,
    Pick,
    Unpick,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HandAnim {
    pub kind: HandKind,
    /// Updates played (1 = the first animated update).
    pub step: u32,
    pub count: usize,
    /// Cards already in the row when it opened (the rest fly in).
    pub old: usize,
    /// The picked hand indices in pick order (one for a single summon).
    pub picks: Vec<usize>,
}

/// Rest x of hand card `i` of `count`.
pub fn rest_x(i: usize, count: usize) -> f32 {
    -50.5 * count.saturating_sub(1) as f32 + 101.0 * i as f32
}

/// Where pick number `k` (0-based) of `n` picks ends.
fn pick_target(k: usize, n: usize) -> [f32; 2] {
    if n <= 1 {
        [0.0, 8.0]
    } else if k == 0 {
        [-101.0, 8.0]
    } else {
        [101.0 + 16.0 * (k as f32 - 1.0), 8.0]
    }
}

fn lerp(a: [f32; 2], b: [f32; 2], c: u32) -> [f32; 2] {
    let t = c as f32 / 128.0;
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

impl HandAnim {
    pub fn total(&self) -> u32 {
        match self.kind {
            HandKind::Open => 8 + 3 * self.count.saturating_sub(self.old) as u32,
            HandKind::Close => 8,
            HandKind::Pick | HandKind::Unpick => 15,
        }
    }

    pub fn done(&self) -> bool {
        self.step >= self.total()
    }

    /// (d45, d47) shown at this step.
    pub fn bytes(&self) -> (u8, u8) {
        let s = self.step.min(self.total());
        match self.kind {
            HandKind::Open => ((16 * s).min(128) as u8, 0),
            HandKind::Close => (128u32.saturating_sub(16 * s) as u8, 0),
            HandKind::Pick => (128u32.saturating_sub(16 * s) as u8, (2 * s).min(30) as u8),
            HandKind::Unpick => ((16 * s).min(128) as u8, 30u32.saturating_sub(2 * s) as u8),
        }
    }

    /// The cards drawn at this step, top first.
    pub fn poses(&self) -> Vec<HandPose> {
        let (d45, d47) = self.bytes();
        let n = self.count;
        let rest = |i: usize| [rest_x(i, n), 8.0];
        match self.kind {
            HandKind::Open | HandKind::Close => {
                if d45 == 0 {
                    return Vec::new();
                }
                let fy = 512.0 - 504.0 * f32::from(d45) / 128.0;
                let s = self.step;
                (0..n)
                    .map(|i| {
                        let position = if self.kind == HandKind::Close || i < self.old {
                            [rest_x(i, n), fy]
                        } else if s <= 8 {
                            [606.0, fy]
                        } else {
                            // The draw-in of new card old + j over 3 updates.
                            let j = ((s - 9) / 3) as usize;
                            let c = [68, 102, 128][((s - 9) % 3) as usize];
                            if i < self.old + j {
                                rest(i)
                            } else if i == self.old + j {
                                lerp([606.0, 8.0], rest(i), c)
                            } else {
                                [606.0, 8.0]
                            }
                        };
                        HandPose { index: i, position, theta: 0.0, large: false }
                    })
                    .collect()
            }
            HandKind::Pick | HandKind::Unpick => pick_poses(n, &self.picks, d47, self.kind == HandKind::Pick),
        }
    }
}

/// The pick layout at d47 (0..=30): picked cards first (pick 1 on top).
pub fn pick_poses(count: usize, picks: &[usize], d47: u8, forward: bool) -> Vec<HandPose> {
    let c = u32::from(d47) * 128 / 30;
    let theta = TAU * c as f32 / 128.0;
    // The large face from the first back-facing update of a forward pick;
    // a cancel switches back at its first back-facing update.
    let large = if forward { c > 32 } else { c >= 96 };
    let mut poses: Vec<HandPose> = picks
        .iter()
        .enumerate()
        .map(|(k, &i)| HandPose {
            index: i,
            position: lerp([rest_x(i, count), 8.0], pick_target(k, picks.len()), c),
            theta,
            large,
        })
        .collect();
    for i in (0..count).filter(|i| !picks.contains(i)) {
        if c < 128 {
            poses.push(HandPose {
                index: i,
                position: lerp([rest_x(i, count), 8.0], [rest_x(i, count), 512.0], c),
                theta: 0.0,
                large: false,
            });
        }
    }
    poses
}

/// The corners of a hand card quad at `pose` in HUD pixels, with the
/// card's local x of each screen column: the picked card's 3D turn about
/// its vertical axis at camera depth 800 [emu fit, 0.1 px].
pub fn project(pose: &HandPose, lx: f32, ly: f32) -> [f32; 2] {
    let [cx, cy] = pose.position;
    let (s, c) = pose.theta.sin_cos();
    let x = cx + lx * c;
    let z = 800.0 + lx * s;
    [256.0 + x * 800.0 / z, 256.0 + (1.342_857 * cy + ly) * 800.0 / z]
}

/// The card's local x under screen column `sx` (inverse of [`project`]).
pub fn local_x(pose: &HandPose, sx: f32) -> f32 {
    let [cx, _] = pose.position;
    let (s, c) = pose.theta.sin_cos();
    let xs = sx - 256.0;
    800.0 * (xs - cx) / (800.0 * c - xs * s)
}

/// Half sizes of a hand card in HUD pixels at rest.
pub const HALF: [f32; 2] = [48.0, 96.686];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_rises_in_eight_updates_and_new_cards_fly_in() {
        let mut a = HandAnim { kind: HandKind::Open, step: 0, count: 5, old: 3, picks: Vec::new() };
        let fys: Vec<f32> = (1..=8)
            .map(|s| {
                a.step = s;
                a.poses()[0].position[1]
            })
            .collect();
        assert_eq!(fys, [449.0, 386.0, 323.0, 260.0, 197.0, 134.0, 71.0, 8.0]);
        a.step = 8;
        assert_eq!(a.poses()[3].position, [606.0, 8.0]);
        a.step = 9;
        let x = a.poses()[3].position[0];
        assert!((x - (606.0 + (rest_x(3, 5) - 606.0) * 68.0 / 128.0)).abs() < 1e-3);
        assert_eq!(a.total(), 14);
    }

    #[test]
    fn pick_quad_matches_the_traced_turn() {
        // Frame f1 of the pick (c = 8, theta 22.5 deg) of hand card 0:
        // emu x 16.8 / 114.2 [HAND-SLIDE-AND-FADE.md 1.5].
        let p = pick_poses(5, &[0], 2, true)[0];
        let left = project(&p, -48.0, 0.0)[0];
        let right = project(&p, 48.0, 0.0)[0];
        assert!((left - 16.8).abs() < 0.2 && (right - 114.2).abs() < 0.2, "{left} {right}");
        let lx = local_x(&p, 60.0);
        assert!((project(&p, lx, 0.0)[0] - 60.0).abs() < 1e-3);
    }
}
