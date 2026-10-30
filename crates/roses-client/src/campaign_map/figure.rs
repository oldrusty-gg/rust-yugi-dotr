//! The leader figure on the campaign map: unit object 30, placed every
//! frame by 0x22AE20 (called last in the node pass 0x22A4D0). Source facts
//! (PAL SLES_515.79, private notes `story/claude-campaign-map/FIGURE.md`):
//!
//! - Model [F]: map init 0x21C1A4 loads the player's current-deck leader
//!   (0x215160(0, 0x215040(0)), also W+5406) into object 30 via 0x1DF410 /
//!   0x1E99C0; MONSTER.MRG slot = that card. The side does not matter.
//! - Per frame [F 0x22C0AC..0x22C124]: flag 0x1F3BF0(30, 1), recentre
//!   obj+0x1FF = 32, scale (0.5, 0.5, 0.5) (sp+224, 0x22AEAC), position =
//!   W+1F0, second rotation obj+0x20 (0, W+200 + 0x8000, 0) (0x1F36D0),
//!   alpha obj+0x12F = W+5480 (the
//!   HUD alpha; 0 skips the draw, 0x1EFA88), visible 0x1F42C0(30, 1); W+200
//!   is zeroed after the draw.
//! - First rotation [F 0x21C220, emu]: map init sets obj+0x10 once, by
//!   0x1F3640(30, 0x299B80 = (0, 0x8000, 0)); nothing in the map rewrites
//!   it ((0, 0x8000, 0) in 622/652/653). The unit draw 0x1EF8F0 applies
//!   both: W = T(pos) R(obj+0x10) T(c') R(obj+0x20) S' T(−c) [emu: 653-mgo's
//!   VU1 list matrices for object 30 equal that W to 5e-5]. So the figure
//!   turns by obj+0x20 and another half turn.
//! - Height [F]: y = node y + the signed byte of table 0x298E10 indexed by
//!   the leader card (683 monster cards; 24 for card 392).
//! - Branches [F 0x22AE20..0x22C0AC]; C7/C8 = save location/destination, the
//!   path from switch 0x2D2420 (`MapData::walk_path`), x/z the runtime path
//!   arrays (count dots plus one more sample: the last waypoint on land, the
//!   sea tail on sea routes [emu]):
//!   - forward path (W+1D8 ≠ 0, travel > 0, W+1DA ≤ 1024), i = W+1DE,
//!     s = W+1DA: heading of segment i blended with segment i+1 by s/1024
//!     while i < count − 1, segment i alone at i = count − 1, segment
//!     (i − 1, i) at i ≥ count; position x[i] + (x[i+1] − x[i])·s/1024 at
//!     node 0's y, or C8 once W+1D8 = 3;
//!   - reverse path: W+1DE runs from count − 1 down; position x[i+1] +
//!     (x[i] − x[i+1])·s/1024, heading of (i+1 → i) blended from the
//!     previous one (i+2 → i+1) by s/1024; at W+1D8 = 3 W+1DE = 0;
//!   - no path: heading C7 → C8, position C7 + (C8 − C7)·s/1024 (C7's y),
//!     C8 on the frame W+1DA reaches 1024 and after;
//!   - otherwise (W+1D8 = 0, travel 0, ...): position C7, heading towards
//!     the cursor's point on the plane W+190 = (6.1e-5, −10000, 0, 86500)
//!     (y = 8.65): 0x2055F0 unprojects (cursor − 256), 0x205D70 intersects
//!     the line from the camera eye [F]; the cursor ray scale is 800 per
//!     unit in x and y [emu, 137 frames of figure-idle-622.json].
//!   Heading = trunc(65536·atan2(dx, dz)/2π) & 0xFFFF (`heading`).
//! - Motion [F]: the map never sets a motion on object 30 (no 0x1F2A40 /
//!   0x1F2760 call in the map's code); the mode reset 0x1A9648 leaves motion
//!   0 forward at speed 1, and the main loop's 0x1EE6E0 advances every
//!   object's clock once per frame (0x1AADBC). So the figure loops its idle
//!   motion 0 whether it stands or walks [emu: motion 0 in 622/652/653].
//! - Hidden [F 0x21D7F0]: the 3D pass runs 0x229DD0 instead of the node pass
//!   (and hides objects 30/31) for C2 ≥ 10000 in the cases of
//!   `node_pass_runs`.
use super::*;

pub const FIGURE_OBJECT: u8 = 30;
pub const FIGURE_HEIGHT_TABLE_EE: u32 = 0x0029_8E10;
pub const FIGURE_HEIGHT_CARDS: usize = 683;
pub const FIGURE_SCALE: f32 = 0.5;
pub const FIGURE_RECENTRE: u8 = 32;
/// The idle motion the figure loops (never changed on the map).
pub const FIGURE_MOTION: u8 = 0;
/// obj+0x10, set at map init from 0x299B80 (0x21C220 -> 0x1F3640).
pub const FIGURE_BASE_ROTATION: [i32; 3] = [0, 0x8000, 0];
/// W+190: the plane (a, b, c, d), a x + b y + c z + d = 0, of the cursor's
/// ground point [emu: constant in every map savestate].
pub const GROUND_PLANE: [f64; 4] = [6.103_515_625e-5, -10_000.0, 0.0, 86_500.0];
/// Cursor units per unit of the camera ray in x and y (0x2055F0) [emu].
pub const CURSOR_RAY_SCALE: f64 = 800.0;
const FACING: i32 = 0x8000;
const WALK_FULL: i32 = 1024;

/// The per-card height table (signed bytes, card id < 683) read from the
/// user's executable.
pub fn figure_heights(executable: &[u8]) -> Result<Vec<i8>> {
    let start = (FIGURE_HEIGHT_TABLE_EE - EE_TO_FILE) as usize;
    let table = executable
        .get(start..start + FIGURE_HEIGHT_CARDS)
        .ok_or("the leader height table lies outside the executable")?;
    let heights: Vec<i8> = table.iter().map(|&b| b as i8).collect();
    if heights.iter().any(|&h| h < 0) {
        return Err("the leader height table has negative entries".into());
    }
    Ok(heights)
}

/// The height of `card` above the node (None for a card without a model).
pub fn figure_height(heights: &[i8], card: u16) -> Option<i8> {
    heights.get(usize::from(card)).copied()
}

/// Whether the map's 3D pass runs the node pass (which places and shows the
/// figure) rather than 0x229DD0 (which hides objects 30 and 31), for C2 =
/// `event` and the event's sub-step W+536C [F 0x21D7F0..0x21DA08].
pub fn node_pass_runs(event: i16, substep: i16) -> bool {
    match event {
        e if e < 10_000 => true,
        11_000 | 11_182 | 11_184 | 12_000 | 12_130 | 12_152 | 12_154 | 12_156 => false,
        12_150 | 11_180 => substep < 2,
        12_132 => substep >= 10,
        11_162 => substep >= 9,
        11_160 => substep < 2,
        e => e >= 11_000,
    }
}

/// Object 30's parameters for one frame (the unit-object fields the draw
/// reads: `monster_pose::UnitObject` position, rotation, scale and
/// recentre; obj+0x12F alpha).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct FigureObject {
    pub position: [f32; 3],
    /// obj+0x10 (`UnitObject::rotation`): `FIGURE_BASE_ROTATION`, kept
    /// from the map's init.
    pub base_rotation: [i32; 3],
    /// obj+0x20 (`UnitObject::rotation2`): (0, W+200 + 0x8000, 0); y may
    /// exceed 0xFFFF (0x10000 = one turn).
    pub rotation: [i32; 3],
    pub scale: [f32; 3],
    /// obj+0x1FF.
    pub recentre: u8,
    /// obj+0x12F = W+5480; 0 draws nothing.
    pub alpha: u8,
    /// The node pass ran (see `node_pass_runs`; sub-step taken as 0 for
    /// story events, whose map frames are not modelled).
    pub visible: bool,
    /// W+200 this frame.
    pub heading: u16,
    /// Motion slot (loops; one clock step per frame at speed 1).
    pub motion: u8,
}

impl FigureObject {
    /// The unit object 0x1EF8F0 draws (`monster_pose::object_matrix`):
    /// obj+0x10 first, obj+0x20 after the recentring; `grow` is obj+0x1E8.
    pub fn unit_object(&self, grow: f32) -> crate::monster_pose::UnitObject {
        crate::monster_pose::UnitObject {
            position: self.position,
            rotation: self.base_rotation,
            scale: self.scale,
            rotation2: self.rotation,
            grow,
            recentre: self.recentre,
        }
    }
}

impl MapModel {
    /// Node `n`'s (x, y, z) as f32.
    fn figure_node(&self, n: u8) -> [f32; 3] {
        self.data.node(n).position
    }

    /// The runtime path arrays: the dots plus the sample beyond `count`.
    fn figure_samples(&self, path: u8) -> (Vec<[f32; 2]>, usize) {
        let p = self.data.path(0, path);
        let mut x = p.dots.clone();
        let count = x.len();
        let extra = if p.sea_route {
            sea_route_tail(&p.waypoints)
        } else {
            p.waypoints.last().map(|w| [f32::from(w[0]), f32::from(w[1])])
        };
        x.extend(extra);
        (x, count)
    }

    /// The cursor's point on the plane W+190 for the current camera.
    pub fn cursor_ground(&self) -> Option<[f64; 3]> {
        let eye = camera_eye(self.camera_target);
        let a = std::f64::consts::FRAC_1_SQRT_2;
        let kx = (f64::from(self.cursor.0) - SCREEN_CENTRE) / CURSOR_RAY_SCALE;
        let ky = (f64::from(self.cursor.1) - SCREEN_CENTRE) / CURSOR_RAY_SCALE;
        // right (1, 0, 0), down (0, −1, 1)/√2, forward (0, −1, −1)/√2.
        let dir = [kx, -a * ky - a, a * ky - a];
        let [pa, pb, pc, pd] = GROUND_PLANE;
        let denom = pa * dir[0] + pb * dir[1] + pc * dir[2];
        if denom.abs() < 1e-12 {
            return None;
        }
        let t = -(pa * eye[0] + pb * eye[1] + pc * eye[2] + pd) / denom;
        Some([eye[0] + dir[0] * t, eye[1] + dir[1] * t, eye[2] + dir[2] * t])
    }

    /// Object 30 for this frame (0x22AE20's placement, read after `step`):
    /// `height` is the leader card's entry of `figure_heights`.
    pub fn figure_object(&self, height: i8) -> FigureObject {
        let h = f32::from(height);
        let (state, sub, index) = self.figure.drawn;
        let (c7, c8) = (self.save.location, self.save.destination);
        let walking = state != 0 && self.travel > 0 && sub <= WALK_FULL;
        let lift = |p: [f32; 3]| [p[0], p[1] + h, p[2]];
        let (position, heading) = match (walking, self.data.walk_path(c7, c8)) {
            (true, Some((path, forward))) => {
                let (x, count) = self.figure_samples(path);
                let at = |k: isize| {
                    usize::try_from(k)
                        .ok()
                        .and_then(|k| x.get(k).copied())
                        .unwrap_or([0.0, 0.0])
                };
                let y0 = self.figure_node(0)[1] + h;
                let s = sub as f32;
                let n = count as isize;
                if forward {
                    let i = index as isize;
                    let (p0, p1, p2) = (at(i), at(i + 1), at(i + 2));
                    let d = [p1[0] - p0[0], p1[1] - p0[1]];
                    let heading = if i >= n {
                        let q = at(i - 1);
                        heading(p0[0] - q[0], p0[1] - q[1])
                    } else if i < n - 1 {
                        let e = [p2[0] - p1[0], p2[1] - p1[1]];
                        let r = (WALK_FULL - sub) as f32;
                        heading(
                            d[0] * r / 1024.0 + e[0] * s / 1024.0,
                            d[1] * r / 1024.0 + e[1] * s / 1024.0,
                        )
                    } else {
                        heading(d[0], d[1])
                    };
                    let position = if state == 3 {
                        lift(self.figure_node(c8))
                    } else {
                        [p0[0] + d[0] * s / 1024.0, y0, p0[1] + d[1] * s / 1024.0]
                    };
                    (position, heading)
                } else {
                    // W+1DE counts down from count − 1; 0 once arrived.
                    let i = if state == 3 { 0 } else { n - 1 - index as isize };
                    let (p0, p1, p2) = (at(i), at(i + 1), at(i + 2));
                    let d = [p0[0] - p1[0], p0[1] - p1[1]];
                    let heading = if i >= n {
                        let q = at(i - 1);
                        heading(p0[0] - q[0], p0[1] - q[1])
                    } else if i < n - 1 {
                        let e = [p1[0] - p2[0], p1[1] - p2[1]];
                        let r = (WALK_FULL - sub) as f32;
                        heading(
                            e[0] * r / 1024.0 + d[0] * s / 1024.0,
                            e[1] * r / 1024.0 + d[1] * s / 1024.0,
                        )
                    } else {
                        heading(d[0], d[1])
                    };
                    let position = if state == 3 {
                        lift(self.figure_node(c8))
                    } else {
                        [p1[0] + d[0] * s / 1024.0, y0, p1[1] + d[1] * s / 1024.0]
                    };
                    (position, heading)
                }
            }
            (true, None) => {
                let (a, b) = (self.figure_node(c7), self.figure_node(c8));
                let d = [b[0] - a[0], b[2] - a[2]];
                let heading = heading(d[0], d[1]);
                let s = sub as f32;
                let position = if state == 3 || sub + STRAIGHT_WALK_STEP >= WALK_FULL {
                    lift(b)
                } else {
                    [a[0] + d[0] * s / 1024.0, a[1] + h, a[2] + d[1] * s / 1024.0]
                };
                (position, heading)
            }
            (false, _) => {
                let p = lift(self.figure_node(c7));
                let heading = match self.cursor_ground() {
                    Some(g) => heading(
                        (g[0] - f64::from(p[0])) as f32,
                        (g[2] - f64::from(p[2])) as f32,
                    ),
                    None => 0,
                };
                (p, heading)
            }
        };
        FigureObject {
            position,
            base_rotation: FIGURE_BASE_ROTATION,
            rotation: [0, i32::from(heading) + FACING, 0],
            scale: [FIGURE_SCALE; 3],
            recentre: FIGURE_RECENTRE,
            alpha: self.hud_alpha.clamp(0, 255) as u8,
            visible: node_pass_runs(self.save.event, 0),
            heading,
            motion: FIGURE_MOTION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic map with invented coordinates; no retail data.
    fn fixture() -> MapData {
        let nodes: Vec<MapNode> = (0..NODE_COUNT as u8)
            .map(|index| MapNode {
                index,
                name: format!("node-{index}"),
                position: [
                    -100.0 + 12.0 * f32::from(index),
                    0.0,
                    150.0 - 9.0 * f32::from(index),
                ],
                facing_raw: 0.0,
                base_model: 9,
                castle_model: 11,
                red_paths: [0; 4],
                white_paths: [0; 4],
                path_count: 0,
                path_dirs: [0; 4],
                win_event: 301,
                loss_event: 201,
            })
            .collect();
        let mut data = MapData {
            nodes,
            paths: (0..32)
                .map(|i| MapPath {
                    side: (i / 16) as u8,
                    index: (i % 16) as u8,
                    waypoints: Vec::new(),
                    sea_route: false,
                    ends: [0, 0],
                    alternate_ends: [0, 0],
                    dots: Vec::new(),
                })
                .collect(),
            texts: Vec::new(),
            rose_card_nodes: Vec::new(),
            messages: BTreeMap::new(),
            ..MapData::default()
        };
        data.nodes[1].position = [-80.0, 0.0, 60.0];
        data.nodes[2].position = [-38.0, 0.0, 40.0];
        data.nodes[3].position = [-20.0, 0.0, 8.0];
        data.nodes[4].position = [-6.0, 2.0, -30.0];
        for (index, waypoints, ends) in [
            (1usize, vec![[-80, 60], [-38, 40]], [1, 2]),
            (3, vec![[-38, 40], [-28, 24], [-20, 8]], [2, 3]),
        ] {
            let p = &mut data.paths[index];
            p.dots = spline_dots(&waypoints, false);
            p.waypoints = waypoints;
            p.ends = ends;
            p.alternate_ends = ends;
        }
        data
    }

    fn model() -> MapModel {
        MapModel::at_event_zero(fixture()).unwrap()
    }

    fn lerp(a: [f32; 2], d: [f32; 2], s: f32, y: f32) -> [f32; 3] {
        [a[0] + d[0] * s / 1024.0, y, a[1] + d[1] * s / 1024.0]
    }

    #[test]
    fn heights_come_from_the_executable_table() {
        let start = (FIGURE_HEIGHT_TABLE_EE - EE_TO_FILE) as usize;
        let mut exe = vec![0u8; start + FIGURE_HEIGHT_CARDS];
        exe[start + 392] = 24;
        let heights = figure_heights(&exe).unwrap();
        assert_eq!(figure_height(&heights, 392), Some(24));
        assert_eq!(figure_height(&heights, 999), None);
        assert!(figure_heights(&exe[..start + 10]).is_err());
        exe[start] = 0xF0;
        assert!(figure_heights(&exe).is_err());
    }

    #[test]
    fn the_node_pass_dispatch_follows_0x21d7f0() {
        assert!(node_pass_runs(0, 0) && node_pass_runs(101, 3) && node_pass_runs(11_124, 0));
        assert!(!node_pass_runs(11_000, 0) && !node_pass_runs(12_156, 0));
        assert!(!node_pass_runs(10_500, 0));
        assert!(node_pass_runs(11_160, 1) && !node_pass_runs(11_160, 2));
        assert!(!node_pass_runs(11_162, 8) && node_pass_runs(11_162, 9));
        assert!(!node_pass_runs(12_132, 9) && node_pass_runs(12_132, 10));
    }

    #[test]
    fn the_idle_figure_stands_on_c7_and_faces_the_cursor() {
        let mut m = model();
        m.save.location = 1;
        let f = m.figure_object(24);
        assert_eq!(f.position, [-80.0, 24.0, 60.0]);
        assert_eq!(
            (f.scale, f.recentre, f.alpha, f.motion),
            ([0.5; 3], 32, 0x80, 0)
        );
        assert!(f.visible);
        assert_eq!(f.rotation, [0, i32::from(f.heading) + 0x8000, 0]);
        // The draw 0x1EF8F0 turns by obj+0x10 (the init's half turn) first,
        // then by obj+0x20 after the recentring.
        assert_eq!(f.base_rotation, [0, 0x8000, 0]);
        let unit = f.unit_object(1.0);
        assert_eq!(
            (unit.rotation, unit.rotation2, unit.scale, unit.recentre),
            (f.base_rotation, f.rotation, f.scale, f.recentre)
        );
        assert_eq!((unit.position, unit.grow), (f.position, 1.0));
        // Screen centre: the camera target moved along the view ray to y = 8.65.
        m.cursor = (256, 256);
        let g = m.cursor_ground().unwrap();
        assert!((g[1] - 8.65).abs() < 1e-6);
        assert!((g[0] - m.camera_target[0]).abs() < 1e-6);
        assert!((g[2] - (m.camera_target[1] + 8.65)).abs() < 1e-6);
        // Camera so that the figure's column is the screen centre, cursor to
        // the right: facing +x (0x4000), drawn at 0xC000.
        m.camera_target = [-80.0, 60.0 - 8.65];
        m.cursor = (400, 256);
        let f = m.figure_object(0);
        assert_eq!((f.heading, f.rotation[1]), (0x4000, 0xC000));
        // Cursor towards the camera (screen bottom): facing +z, heading 0.
        m.cursor = (256, 480);
        assert_eq!(m.figure_object(0).heading, 0);
        // Alpha follows the HUD alpha; C2 = 11000 runs 0x229DD0 (hidden).
        m.hud_alpha = 0x40;
        m.save.event = 11_000;
        let f = m.figure_object(0);
        assert_eq!((f.alpha, f.visible), (0x40, false));
    }

    #[test]
    fn a_forward_walk_places_the_figure_from_the_words_before_the_advance() {
        let mut m = model();
        m.save.location = 1;
        m.save.destination = 2;
        m.travel = 2;
        let (x, count) = m.figure_samples(1);
        assert_eq!(x[count], [-38.0, 40.0]);
        m.figure.drawn = (1, 0, 0);
        let f = m.figure_object(10);
        assert_eq!(f.position, [x[0][0], 10.0, x[0][1]]);
        assert_eq!(f.heading, heading(x[1][0] - x[0][0], x[1][1] - x[0][1]));
        m.figure.drawn = (1, 512, 1);
        let d = [x[2][0] - x[1][0], x[2][1] - x[1][1]];
        assert_eq!(m.figure_object(10).position, lerp(x[1], d, 512.0, 10.0));
        m.figure.drawn = (1, 768, count - 1);
        let last = m.figure_object(10).heading;
        let d = [x[count][0] - x[count - 1][0], x[count][1] - x[count - 1][1]];
        assert_eq!(last, heading(d[0], d[1]));
        m.figure.drawn = (3, 0, count);
        let f = m.figure_object(10);
        assert_eq!((f.position, f.heading), ([-38.0, 10.0, 40.0], last));
        // Travel 0 (back from a duel): idle on C7 whatever the words say.
        m.travel = 0;
        assert_eq!(m.figure_object(10).position, [-80.0, 10.0, 60.0]);
    }

    #[test]
    fn a_reverse_walk_runs_the_samples_backwards() {
        let mut m = model();
        m.save.location = 3;
        m.save.destination = 2;
        m.travel = 2;
        assert_eq!(m.data.walk_path(3, 2), Some((3, false)));
        let (x, count) = m.figure_samples(3);
        // First drawn frame: W+1DE = count − 1, at the sample beyond count
        // (node 3's waypoint).
        m.figure.drawn = (2, 0, 0);
        assert_eq!(m.figure_object(0).position, [-20.0, 0.0, 8.0]);
        // Last segment (W+1DE = 0): from x[1] to x[0], heading blended from
        // the previous segment (x[2] → x[1]).
        m.figure.drawn = (2, 256, count - 1);
        let f = m.figure_object(0);
        let d = [x[0][0] - x[1][0], x[0][1] - x[1][1]];
        let e = [x[1][0] - x[2][0], x[1][1] - x[2][1]];
        assert_eq!(f.position, lerp(x[1], d, 256.0, 0.0));
        assert_eq!(
            f.heading,
            heading(
                e[0] * 768.0 / 1024.0 + d[0] * 256.0 / 1024.0,
                e[1] * 768.0 / 1024.0 + d[1] * 256.0 / 1024.0
            )
        );
        // Arrived: C8, W+1DE = 0 and s = 0 → the previous segment's heading.
        m.figure.drawn = (3, 0, count);
        let f = m.figure_object(0);
        assert_eq!((f.position, f.heading), ([-38.0, 0.0, 40.0], heading(e[0], e[1])));
    }

    #[test]
    fn a_straight_walk_shows_c8_on_its_last_frame() {
        let mut m = model();
        m.save.location = 2;
        m.save.destination = 4;
        m.travel = 2;
        assert!(m.data.walk_path(2, 4).is_none());
        m.figure.drawn = (1, 512, 0);
        let f = m.figure_object(5);
        assert_eq!(f.position, [-38.0 + 32.0 * 0.5, 5.0, 40.0 - 70.0 * 0.5]);
        assert_eq!(f.heading, heading(32.0, -70.0));
        m.figure.drawn = (1, 1008, 0);
        assert_eq!(m.figure_object(5).position, [-6.0, 7.0, -30.0]);
    }

    #[test]
    fn the_model_records_the_drawn_words_of_a_walk() {
        let mut m = model();
        m.save.location = 1;
        m.start_walk(1, 2, m.data.walk_path(1, 2));
        m.walk_update();
        assert_eq!((m.figure.drawn, m.figure.sub), ((1, 0, 0), WALK_STEP));
        m.walk_update();
        assert_eq!(
            (m.figure.drawn, m.figure.sub),
            ((1, WALK_STEP, 0), 2 * WALK_STEP)
        );
        while m.figure.state != 3 {
            m.walk_update();
        }
        let count = m.data.path(0, 1).dots.len();
        assert_eq!(m.figure.drawn, (1, 768, count - 1));
        m.walk_update();
        assert_eq!(m.figure.drawn, (3, 0, count));
    }
}

#[cfg(test)]
mod private_tests {
    use super::*;
    use serde_json::Value;

    fn data() -> MapData {
        let path = std::env::var_os("ROSES_CAMPAIGN_MAP_DATA").expect("ROSES_CAMPAIGN_MAP_DATA");
        load_map_data(std::path::Path::new(&path)).unwrap()
    }

    /// `logs/figure/figure-idle-622.json` / `figure-walk-652.json`
    /// (tools/run_figure.py).
    fn log(name: &str) -> Value {
        let dir = std::env::var_os("ROSES_FIGURE_LOGS").expect("ROSES_FIGURE_LOGS");
        let bytes = std::fs::read(std::path::Path::new(&dir).join(name)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn pad(bits: i64) -> Pad {
        Pad {
            up: bits & 0x1000 != 0,
            right: bits & 0x2000 != 0,
            down: bits & 0x4000 != 0,
            left: bits & 0x8000 != 0,
            cross: bits & 0x40 != 0,
            cross_held: bits & 0x40 != 0,
            menu: MenuPad::NONE,
        }
    }

    /// Replays the log's pads and compares object 30 frame by frame;
    /// returns the largest heading difference and the number of rows whose
    /// heading was not compared: idle rows where the emulator's view matrix
    /// (0x2ED760, rebuilt per frame by the render path MapRun does not run)
    /// no longer matches the camera after an edge pan.
    fn replay(m: &mut MapModel, log: &Value) -> (i64, usize) {
        let rows = log["rows"].as_array().unwrap();
        let first = &rows[0]["obj30"];
        let c7 = m.data.node(m.save.location).position;
        let height = (first["position"][1].as_f64().unwrap() - f64::from(c7[1])) as i8;
        let mut worst = 0;
        let mut stale = 0;
        let mut pans = 0;
        for row in rows {
            m.step(pad(row["pad"].as_i64().unwrap()));
            let frame = row["frame"].as_i64().unwrap();
            let o = &row["obj30"];
            // The model's edge pan starts a frame earlier than the original's
            // (it pans when the move crosses the edge, the original once the
            // cursor is already there): follow the logged camera.
            let target = [row["target"][0].as_f64().unwrap(), row["target"][1].as_f64().unwrap()];
            if (m.camera_target[0] - target[0]).abs() + (m.camera_target[1] - target[1]).abs() > 1e-3 {
                pans += 1;
                m.camera_target = target;
            }
            let f = m.figure_object(height);
            let want: Vec<f64> = o["position"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
            for k in 0..3 {
                assert!(
                    (f64::from(f.position[k]) - want[k]).abs() < 1e-3,
                    "frame {frame}: position {:?} vs {want:?}",
                    f.position
                );
                assert_eq!(f64::from(f.scale[k]), o["scale"][k].as_f64().unwrap());
            }
            let rot = o["rotation"][1].as_i64().unwrap();
            let eye = |k: &str| row[k][2].as_f64();
            let skip = matches!((eye("eye"), eye("view_eye")), (Some(a), Some(b)) if (a - b).abs() > 1e-3);
            if skip {
                stale += 1;
            } else {
                let diff = (i64::from(f.rotation[1]) - rot).abs();
                worst = worst.max(diff);
                assert!(diff <= 1, "frame {frame}: rotation {} vs {rot}", f.rotation[1]);
            }
            assert_eq!(f.rotation[0], 0);
            assert_eq!(i64::from(f.alpha), o["alpha"].as_i64().unwrap(), "frame {frame}");
            assert_eq!(i64::from(f.recentre), o["recentre"].as_i64().unwrap());
            assert_eq!(i64::from(f.motion), o["motion"].as_i64().unwrap());
            assert!(f.visible && row["drawn"].as_bool().unwrap());
        }
        if pans > 0 {
            eprintln!("{pans} frames with the model's camera replaced by the logged one");
        }
        (worst, stale)
    }

    /// Walks with no path in the switch 0x2D2420 (4 → 2, 9 → 2): the
    /// 652-mup state with C7 patched (`modes/older/walk/run_straight.py`,
    /// `straight-<from>-2.json`): the figure position W+1F0 and heading
    /// W+200 frame by frame, the arrival (W+1D8 = 3) 64 frames after the
    /// start [emu].
    #[test]
    #[ignore = "Requires the private map import and the straight-walk logs (ROSES_CAMPAIGN_MAP_DATA, ROSES_STRAIGHT_WALK_LOGS)"]
    fn private_walks_with_no_path_go_straight_like_the_emulator() {
        let dir = std::env::var_os("ROSES_STRAIGHT_WALK_LOGS").expect("ROSES_STRAIGHT_WALK_LOGS");
        for from in [4u8, 9] {
            assert!(super::super::WALK_PATHS.iter().all(|(a, b, _)| (*a, *b) != (from, 2)));
            let bytes = std::fs::read(std::path::Path::new(&dir).join(format!("straight-{from}-2.json"))).unwrap();
            let log: Value = serde_json::from_slice(&bytes).unwrap();
            let mut m = MapModel::at_event_zero(data()).unwrap();
            for _ in 0..6 {
                m.step(Pad::UP);
            }
            m.frame = 0;
            m.hud.panel = HUD_SLIDE_FULL;
            m.save.location = from;
            m.save.node_states[usize::from(from)] = 3;
            let rows = log["rows"].as_array().unwrap();
            let c7 = m.data.node(from).position;
            let height = (rows[0]["figure"][1].as_f64().unwrap() - f64::from(c7[1])) as i8;
            let mut arrived = None;
            for row in rows {
                m.step(pad(row["pad"].as_i64().unwrap()));
                let frame = row["frame"].as_i64().unwrap();
                let f = m.figure_object(height);
                for k in 0..3 {
                    let want = row["figure"][k].as_f64().unwrap();
                    assert!(
                        (f64::from(f.position[k]) - want).abs() < 1e-3,
                        "{from}: frame {frame}: position {:?} vs {}",
                        f.position,
                        row["figure"]
                    );
                }
                if let Some(w200) = row["w200"].as_i64() {
                    let want = (w200 + 0x8000) & 0xFFFF;
                    let diff = (i64::from(f.rotation[1]) - want).abs();
                    assert!(diff <= 1 || diff >= 0xFFFF, "{from}: frame {frame}: rotation {} vs {want}", f.rotation[1]);
                }
                if arrived.is_none() && row["walk"][0].as_i64() == Some(3) {
                    arrived = Some(frame);
                    assert_eq!(m.figure.state, 3, "{from}: frame {frame}");
                }
            }
            let start = rows.iter().find(|r| r["walk"][0].as_i64() == Some(1)).unwrap()["frame"].as_i64().unwrap();
            assert_eq!(arrived.map(|a| a - start), Some(63), "{from}: W+1DA 16 → 1024 in 64 updates");
        }
    }

    #[test]
    #[ignore = "Requires the private map import and figure logs (ROSES_CAMPAIGN_MAP_DATA, ROSES_FIGURE_LOGS)"]
    fn private_idle_figure_faces_the_cursor_like_the_emulator() {
        let mut m = MapModel::at_event_zero(data()).unwrap();
        let (worst, stale) = replay(&mut m, &log("figure-idle-622.json"));
        eprintln!("idle: largest heading difference {worst}, {stale} stale rows skipped");
        assert!(stale <= 60, "{stale}");
    }

    #[test]
    #[ignore = "Requires the private map import and figure logs (ROSES_CAMPAIGN_MAP_DATA, ROSES_FIGURE_LOGS)"]
    fn private_walk_to_chester_matches_the_emulator() {
        // 652-mup = the free map after six frames of Up (cursor on Chester).
        let mut m = MapModel::at_event_zero(data()).unwrap();
        for _ in 0..6 {
            m.step(Pad::UP);
        }
        m.frame = 0;
        // 652-mup has the hover panel fully in (W+6C = 256 [emu]).
        m.hud.panel = HUD_SLIDE_FULL;
        let log = log("figure-walk-652.json");
        let (worst, stale) = replay(&mut m, &log);
        eprintln!("walk: largest heading difference {worst}");
        assert_eq!(stale, 0);
        let rows = log["rows"].as_array().unwrap();
        let last = &rows[rows.len() - 1]["walk"];
        assert_eq!((m.figure.state, m.figure.index as i64), (3, last[2].as_i64().unwrap()));
    }
}
