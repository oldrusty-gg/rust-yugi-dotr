//! CMODULE c502 (0x1F6), field magic's terrain change on one cell, as
//! traced in the project notes 1 [F code; emu: the model
//! matched the interpreter (Mountain, Dark, Toon) to 0.062 px]:
//! - a glowing frame round the 48 x 48 tile (black at radius 28, the
//!   terrain's colour at 24), alpha min(255, 16 (u - 1)), and from u 17 walls
//!   from the tile edge down to max(-40, -2 (u - 16)); additive (Dark and
//!   Labyrinth subtract; Toon and Labyrinth flicker random colours);
//! - the cell's height H = -4 min(u - 36, 32, 101 - u) for u 36..101 (the
//!   tile slides 35 units down and back up), the terrain byte written at
//!   u 68;
//! - the first cell's script also flashes the whole screen, alpha
//!   255 - 16 (u - 101), u 101-116. 116 updates.
//!
//! [H / not reproduced]: drawn without a depth test (the original keys the
//! quads by depth); the port's random values for Toon / Labyrinth.
use crate::duel_c602::projector;
use crate::duel_c621::FxTri;

pub const UPDATES: u32 = 116;
/// The update the terrain byte is written.
pub const TERRAIN_WRITE: u32 = 68;

/// The cell's height (cell +0x14) at update `u`.
pub fn height(u: u32) -> i16 {
    if (36..=101).contains(&u) {
        -4 * (u - 36).min(32).min(101 - u) as i16
    } else {
        0
    }
}

/// The frame / wall colour of a terrain and whether it subtracts.
fn colour(terrain: u8) -> ([u8; 3], bool, bool) {
    // (RGB, subtractive, random inner colours)
    match terrain {
        0 => ([64, 128, 80], false, false),
        1 => ([96, 80, 48], false, false),
        2 => ([64, 64, 64], false, false),
        3 => ([80, 96, 48], false, false),
        4 => ([64, 80, 96], false, false),
        5 => ([32, 32, 32], true, false),
        6 => ([128, 128, 128], false, true),
        7 => ([96, 80, 96], false, false),
        8 => ([32, 32, 32], true, true),
        _ => ([96, 32, 48], false, false),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct C502 {
    pub cell: (u8, u8),
    pub terrain: u8,
    /// The first cell's script (it flashes the screen).
    pub first: bool,
    seed: u64,
    u: u32,
}

impl C502 {
    pub fn new(cell: (u8, u8), terrain: u8, first: bool, seed: u32) -> Self {
        Self { cell, terrain, first, seed: u64::from(seed) | 1, u: 0 }
    }

    pub fn update(&mut self, u: u32) {
        self.u = u;
    }


    /// The frame and walls through the board camera: (triangles,
    /// subtractive).
    pub fn draw(&self, eye: [f64; 3], target: [f64; 3]) -> (Vec<FxTri>, bool) {
        let u = self.u;
        let (rgb, subtract, random) = colour(self.terrain);
        let mut out = Vec::new();
        if u == 0 || u > 101 {
            return (out, subtract);
        }
        let Some(project) = projector(eye, target) else {
            return (out, subtract);
        };
        let a = (16 * (u - 1)).min(255) as u8;
        let hw = if u >= 17 { (-2.0 * (u as f32 - 16.0)).max(-40.0) } else { 0.0 };
        let [cx, cz] = crate::duel_board::cell_center(self.cell.0, self.cell.1);
        let corners = [[-24.0f32, 24.0], [24.0, 24.0], [24.0, -24.0], [-24.0, -24.0]];
        let (inner_a, inner_b) = if random {
            // Six rands per update (the port's own sequence).
            let mut seed = self.seed ^ u64::from(u).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let mut rand = move || {
                seed = seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
                ((seed >> 32) & 0x7F) as u8
            };
            let mut c = || [rand(), rand(), rand()];
            (c(), c())
        } else {
            (rgb, rgb)
        };
        let w = |p: [f32; 3]| [cx + f64::from(p[0]), f64::from(p[1]), cz + f64::from(p[2])];
        let mut quad = |v: [[f32; 3]; 4], c: [[u8; 4]; 4]| {
            let (Some(p0), Some(p1), Some(p2), Some(p3)) = (project(w(v[0])), project(w(v[1])), project(w(v[2])), project(w(v[3]))) else {
                return;
            };
            out.push(FxTri { screen: [p0, p1, p2], uv: [[0.0; 2]; 3], colour: [c[0], c[1], c[2]], textured: false, sheet: 0 });
            out.push(FxTri { screen: [p1, p3, p2], uv: [[0.0; 2]; 3], colour: [c[1], c[3], c[2]], textured: false, sheet: 0 });
        };
        let black = [0, 0, 0, a];
        let ca = [inner_a[0], inner_a[1], inner_a[2], a];
        let cb = [inner_b[0], inner_b[1], inner_b[2], a];
        // Edges back (C3, C2), right (C2, C1), front (C1, C0), left (C0, C3).
        let edges = [(3usize, 2usize), (2, 1), (1, 0), (0, 3)];
        for (s, t) in edges {
            let (cs, ct) = (corners[s], corners[t]);
            let out7 = |c: [f32; 2]| [c[0] + c[0] / 6.0, 0.0, c[1] + c[1] / 6.0];
            quad([out7(cs), out7(ct), [cs[0], 0.0, cs[1]], [ct[0], 0.0, ct[1]]], [black, black, ca, cb]);
        }
        if a == 255 {
            for (s, t) in edges {
                let (cs, ct) = (corners[s], corners[t]);
                quad([[cs[0], hw, cs[1]], [ct[0], hw, ct[1]], [cs[0], 0.0, cs[1]], [ct[0], 0.0, ct[1]]], [black, black, ca, cb]);
            }
        }
        (out, subtract)
    }

    /// The first script's full-screen flash (u 101-116), additive grey.
    pub fn flash(&self) -> Vec<FxTri> {
        if !self.first || !(101..=UPDATES).contains(&self.u) {
            return Vec::new();
        }
        let a = 255u32.saturating_sub(16 * (self.u - 101)) as u8;
        let c = [128, 128, 128, a];
        let full = [[0.0, 0.0], [512.0, 0.0], [0.0, 512.0], [512.0, 512.0]];
        vec![
            FxTri { screen: [full[0], full[1], full[2]], uv: [[0.0; 2]; 3], colour: [c; 3], textured: false, sheet: 0 },
            FxTri { screen: [full[1], full[3], full[2]], uv: [[0.0; 2]; 3], colour: [c; 3], textured: false, sheet: 0 },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_and_flash_follow_the_note() {
        assert_eq!((height(35), height(36), height(37), height(68), height(69), height(70), height(101)), (0, 0, -4, -128, -128, -124, 0));
        let mut c = C502::new((3, 3), 2, true, 1);
        c.update(101);
        assert_eq!(c.flash()[0].colour[0][3], 255);
        c.update(116);
        assert_eq!(c.flash()[0].colour[0][3], 15);
        c.update(100);
        assert!(c.flash().is_empty());
    }
}
