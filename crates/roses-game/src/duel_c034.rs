//! CMODULE c034 (0x22), Negate Attack's shield on the attacker, as traced
//! in the project notes 3 [F code; emu: the model
//! matched the interpreter to 0.062 px for both sides]: a white dome of
//! 9 x 18 untextured Gouraud quads, 50 units above the attacker's cell and
//! 32 in front of it, turned to face the opponent's half; alpha +32 to 192
//! (u 9-14), held, -16 from u 19; each quad sent once to u 17, three times
//! from u 18; nothing drawn u 1-8 and from u 30. 43 updates.
//!
//! [H / not reproduced]: drawn over the board without a depth test (the
//! original keys it by depth).
use crate::duel_c602::projector;
use crate::duel_c621::FxTri;

pub const UPDATES: u32 = 43;

#[derive(Clone, Debug, PartialEq)]
pub struct C034 {
    /// The dome's centre: cell centre + (0, 50, -32 side 0 / +32 side 1).
    pub pos: [f64; 3],
    pub side: u8,
    u: u32,
    alpha: u32,
    /// (alpha, copies) drawn in the last update, `None` when nothing.
    drawn: Option<(u32, u32)>,
}

impl C034 {
    pub fn new(cell_centre: [f64; 3], side: u8) -> Self {
        let dz = if side == 0 { -32.0 } else { 32.0 };
        Self { pos: [cell_centre[0], cell_centre[1] + 50.0, cell_centre[2] + dz], side, u: 0, alpha: 0, drawn: None }
    }

    pub fn update(&mut self, u: u32) {
        self.u = u;
        self.drawn = None;
        if !(9..30).contains(&u) {
            return;
        }
        if u < 18 {
            if self.alpha < 0xB0 {
                self.alpha += 32;
            }
        } else if u >= 19 {
            if self.alpha < 17 {
                return;
            }
            self.alpha -= 16;
        }
        self.drawn = Some((self.alpha, if u < 18 { 1 } else { 3 }));
    }

    /// A dome point (lat, lon in degrees) turned about x by +90 deg (side 0)
    /// or -90 (side 1).
    fn point(&self, lat: f32, lon: f32) -> [f64; 3] {
        let (sl, cl) = lon.to_radians().sin_cos();
        let r = 35.0 - 0.388_888_9 * lat;
        let (x, y, z) = (r * cl, 16.0 * lat.to_radians().sin(), r * sl);
        // y' = y cos + z sin, z' = z cos - y sin with +-90 deg.
        let (y2, z2) = if self.side == 0 { (z, -y) } else { (-z, y) };
        [self.pos[0] + f64::from(x), self.pos[1] + f64::from(y2), self.pos[2] + f64::from(z2)]
    }

    pub fn draw(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<FxTri> {
        let mut out = Vec::new();
        let Some((a, copies)) = self.drawn else {
            return out;
        };
        let Some(project) = projector(eye, target) else {
            return out;
        };
        let step = a / 9;
        for b in 0..9u32 {
            let (lat0, lat1) = (10.0 * b as f32, 10.0 * b as f32 + 10.0);
            let (outer, inner) = (((b + 1) * step).min(255) as u8, (b * step).min(255) as u8);
            for s in 0..18 {
                let (lon0, lon1) = (20.0 * s as f32, 20.0 * s as f32 + 20.0);
                let v = [self.point(lat1, lon1), self.point(lat1, lon0), self.point(lat0, lon1), self.point(lat0, lon0)];
                let (Some(p0), Some(p1), Some(p2), Some(p3)) = (project(v[0]), project(v[1]), project(v[2]), project(v[3])) else {
                    continue;
                };
                let (c0, c2) = ([255, 255, 255, outer], [255, 255, 255, inner]);
                for _ in 0..copies {
                    out.push(FxTri { screen: [p0, p1, p2], uv: [[0.0; 2]; 3], colour: [c0, c0, c2], textured: false, sheet: 0 });
                    out.push(FxTri { screen: [p1, p3, p2], uv: [[0.0; 2]; 3], colour: [c0, c2, c2], textured: false, sheet: 0 });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_and_copies_follow_the_note() {
        let mut c = C034::new([0.0; 3], 0);
        let mut seen = Vec::new();
        for u in 1..=43 {
            c.update(u);
            seen.push(c.drawn);
        }
        assert_eq!(seen[7], None);
        assert_eq!(seen[8], Some((32, 1)));
        assert_eq!(seen[13], Some((192, 1)));
        assert_eq!(seen[17], Some((192, 3)));
        assert_eq!(seen[18], Some((176, 3)));
        assert_eq!(seen[28], Some((16, 3)));
        assert_eq!(seen[29], None);
        // The dome's apex points toward -z for side 0 (the opponent).
        let apex = c.point(90.0, 0.0);
        assert!((apex[2] - (-32.0 - 16.0)).abs() < 1e-4 && (apex[1] - 50.0).abs() < 1e-4, "{apex:?}");
    }
}
