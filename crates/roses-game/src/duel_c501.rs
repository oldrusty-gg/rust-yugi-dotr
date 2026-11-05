//! CMODULE c501 (0x1F5), the graveyard revival (Mimicat, Monster Reborn;
//! event 1 bit 17, handler 0x2586A0), as traced in
//! the project notes [F code; emu: `tools/mf_c501_model.py`
//! equal to the interpreter on four runs]: c502's frame and walls without
//! the tile sink, colour (64, 80, 96):
//! - u 1..53: the frame round the 48 x 48 cell, alpha min(255, 16 (u - 1)),
//!   and from u 17 walls rising to min(30, 2 (u - 16)); additive;
//! - u 54..60: a full-screen additive flash, alpha 255 - 32 (u - 53);
//! - returns 2 at u 32 (the handler creates the unit the next update), 8 at
//!   u 60. No random values.
//!
//! [H / not reproduced]: drawn without a depth test (the original keys the
//! quads by depth, as c502).
use crate::duel_c602::projector;
use crate::duel_c621::FxTri;

pub const UPDATES: u32 = 60;
pub const RET2_U: u32 = 32;
const RGB: [u8; 3] = [64, 80, 96];

#[derive(Clone, Debug, PartialEq)]
pub struct C501 {
    pub cell: (u8, u8),
    u: u32,
}

impl C501 {
    pub fn new(cell: (u8, u8)) -> Self {
        Self { cell, u: 0 }
    }

    pub fn update(&mut self, u: u32) {
        self.u = u;
    }

    fn alpha(u: u32) -> u8 {
        (16 * u.saturating_sub(1)).min(255) as u8
    }

    fn wall(u: u32) -> f32 {
        if u >= 17 { (2.0 * (u as f32 - 16.0)).min(30.0) } else { 0.0 }
    }

    /// The frame and walls through the board camera (u 1..53).
    pub fn draw(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<FxTri> {
        let u = self.u;
        let mut out = Vec::new();
        if !(1..=53).contains(&u) {
            return out;
        }
        let Some(project) = projector(eye, target) else {
            return out;
        };
        let a = Self::alpha(u);
        let hw = Self::wall(u);
        let [cx, cz] = crate::duel_board::cell_center(self.cell.0, self.cell.1);
        let corners = [[-24.0f32, 24.0], [24.0, 24.0], [24.0, -24.0], [-24.0, -24.0]];
        let w = |p: [f32; 3]| [cx + f64::from(p[0]), f64::from(p[1]), cz + f64::from(p[2])];
        let mut quad = |v: [[f32; 3]; 4], c: [[u8; 4]; 4]| {
            let (Some(p0), Some(p1), Some(p2), Some(p3)) = (project(w(v[0])), project(w(v[1])), project(w(v[2])), project(w(v[3]))) else {
                return;
            };
            out.push(FxTri { screen: [p0, p1, p2], uv: [[0.0; 2]; 3], colour: [c[0], c[1], c[2]], textured: false, sheet: 0 });
            out.push(FxTri { screen: [p1, p3, p2], uv: [[0.0; 2]; 3], colour: [c[1], c[3], c[2]], textured: false, sheet: 0 });
        };
        let black = [0, 0, 0, a];
        let c = [RGB[0], RGB[1], RGB[2], a];
        let edges = [(3usize, 2usize), (2, 1), (1, 0), (0, 3)];
        for (s, t) in edges {
            let (cs, ct) = (corners[s], corners[t]);
            let out7 = |p: [f32; 2]| [p[0] + p[0] / 6.0, 0.0, p[1] + p[1] / 6.0];
            quad([out7(cs), out7(ct), [cs[0], 0.0, cs[1]], [ct[0], 0.0, ct[1]]], [black, black, c, c]);
        }
        if a == 255 {
            for (s, t) in edges {
                let (cs, ct) = (corners[s], corners[t]);
                quad([[cs[0], hw, cs[1]], [ct[0], hw, ct[1]], [cs[0], 0.0, cs[1]], [ct[0], 0.0, ct[1]]], [black, black, c, c]);
            }
        }
        out
    }

    /// u 54..60: the full-screen flash, additive grey.
    pub fn flash(&self) -> Vec<FxTri> {
        if !(54..=UPDATES).contains(&self.u) {
            return Vec::new();
        }
        let a = 255u32.saturating_sub(32 * (self.u - 53)) as u8;
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
    fn alpha_walls_and_flash_follow_the_note() {
        assert_eq!((C501::alpha(1), C501::alpha(16), C501::alpha(17)), (0, 240, 255));
        assert_eq!((C501::wall(17), C501::wall(30), C501::wall(31)), (2.0, 28.0, 30.0));
        let mut c = C501::new((3, 3));
        c.update(54);
        assert_eq!(c.flash()[0].colour[0][3], 223);
        c.update(60);
        assert_eq!(c.flash()[0].colour[0][3], 31);
        c.update(54);
        let s = std::f64::consts::FRAC_1_SQRT_2;
        assert!(c.draw([0.0, 180.0 * s, 180.0 * s], [0.0; 3]).is_empty());
    }
}
