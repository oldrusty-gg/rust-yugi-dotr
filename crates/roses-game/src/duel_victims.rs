//! A destroyed trap victim's modules, as traced in
//! the project notes 3-4 [F code; emu: c603 88,320
//! vertices to 0.16 px, c545 400 vertices to 0.062 px against the
//! interpreter]:
//! - c603 (0x25B, 60 updates, returns 2 at u 52): the victim's card cut into
//!   23 x 16 pieces on the card's plane that spin (rot y + 0x100 u (u - 1))
//!   and random-walk apart, alpha 130 - 2u, opaque (Cs * As) for u 1-16 then
//!   additive;
//! - c545 (0x221, 15 updates): two additive puff emitters (TEXEFF 15
//!   (240, 16 f)) on a billboard at the victim.
//!
//! [H / not reproduced]: the port's random values; the pieces sample the
//! card's face (or back) image where the original renders the card copy to
//! texture 0x2000 (its orientation for sideways cards was not traced); the
//! victim's model fade (alpha - 4 per update) is shown by hiding the unit.
use crate::duel_c600::rotate;
use crate::duel_c602::{billboard_axes, projector};
use crate::duel_c621::FxTri;

pub const C603_UPDATES: u32 = 60;
pub const C545_UPDATES: u32 = 15;

fn lcg(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
    ((*seed >> 32) & 0x7FFF_FFFF) as u32
}

/// c603's pieces.
#[derive(Clone, Debug, PartialEq)]
pub struct C603 {
    seed: u64,
    pub pos: [f64; 3],
    /// The card object's rotation (0x4000, Y, Z).
    pub rot: [i32; 3],
    /// Piece offsets o(i, j), row-major (23 rows of 16).
    offsets: Vec<[f32; 3]>,
    /// The offsets as drawn in the last update.
    drawn: Vec<[f32; 3]>,
    /// The update last drawn.
    pub u: u32,
    /// The param's grid: half size, columns, UV cell (w, h).
    half: f32,
    columns: usize,
    cell: [f32; 2],
}

impl C603 {
    pub fn new(pos: [f64; 3], rot: [i32; 3], seed: u32) -> Self {
        Self::with_param(pos, rot, seed, 0)
    }

    /// The per-param tables D+0x1B44..: param 0 half 0.8, 23 x 16, y step
    /// factor -2.0 (cells 16 x 16); param 1 (the Force Field's trap
    /// shatter) half 1.6, 8 x 8, factor -2.875 (cells 32 x 46) [F FV 3,
    /// PEGASUS-MAGIC.md 3.5; H: the param-1 grid origin follows param 0's
    /// rule (half a step in from the card's edge)].
    pub fn with_param(pos: [f64; 3], rot: [i32; 3], seed: u32, param: u8) -> Self {
        let (half, rows, columns, factor) = if param == 1 { (1.6f32, 8, 8, 2.875f32) } else { (0.8, 23, 16, 2.0) };
        let (dx, dy) = (2.0 * half, half * factor);
        let offsets: Vec<[f32; 3]> = (0..rows)
            .flat_map(|i| (0..columns).map(move |j| [-12.8 + half + dx * j as f32, 18.4 - dy / 2.0 - dy * i as f32, 0.0]))
            .collect();
        Self {
            seed: u64::from(seed) | 1,
            pos,
            rot,
            drawn: offsets.clone(),
            offsets,
            u: 0,
            half,
            columns,
            cell: [256.0 / columns as f32, 368.0 / rows as f32],
        }
    }

    /// Update `u`: the pieces as drawn this update (`pieces`), then the walk.
    pub fn update(&mut self, u: u32) {
        self.u = u;
        self.drawn = self.offsets.clone();
        let m = 2 * u.max(1);
        for o in &mut self.offsets {
            for k in [2usize, 0, 1] {
                o[k] += 0.2 * ((lcg(&mut self.seed) % m) as f32 - u as f32);
            }
        }
    }

    /// Alpha (128 = 1.0) and whether the pieces add (else Cs * As).
    pub fn alpha(&self) -> (u8, bool) {
        ((130u32.saturating_sub(2 * self.u)) as u8, self.u > 16)
    }

    /// World corners and UVs (of a 256 x 368 card image) of every piece:
    /// pos + q . R(X, Y + 0x100 u (u - 1), Z).
    pub fn pieces(&self) -> Vec<([[f64; 3]; 4], [[f32; 2]; 4])> {
        let spin = (0x100 * self.u * self.u.saturating_sub(1)) as i32;
        let r = [self.rot[0], self.rot[1] + spin, self.rot[2]];
        self.drawn
            .iter()
            .enumerate()
            .map(|(n, o)| {
                let (i, j) = ((n / self.columns) as f32, (n % self.columns) as f32);
                let (h, [cw, ch]) = (self.half, self.cell);
                let corner = |dx: f32, dy: f32| -> [f64; 3] {
                    let w = rotate([o[0] + dx, o[1] + dy, o[2]], r);
                    std::array::from_fn(|k| self.pos[k] + f64::from(w[k]))
                };
                (
                    [corner(-h, h), corner(h, h), corner(-h, -h), corner(h, -h)],
                    [[cw * j, ch * i], [cw * j + cw - 1.0, ch * i], [cw * j, ch * i + ch - 1.0], [cw * j + cw - 1.0, ch * i + ch - 1.0]],
                )
            })
            .collect()
    }
}

/// c545's two emitters.
#[derive(Clone, Debug, PartialEq)]
pub struct C545 {
    pub pos: [f64; 3],
    /// Per emitter: 16 positions, alpha, step, scale, half size, colour.
    emitters: Vec<(Vec<[f32; 3]>, u8, u8, f32, f32, [u8; 3])>,
    u: u32,
    tris_alpha: [u8; 2],
}

impl C545 {
    pub fn new(pos: [f64; 3], seed: u32) -> Self {
        let mut seed = u64::from(seed) | 1;
        let mut emitters = Vec::new();
        for (base, step, scale, rgb) in [(5.0f32, 8u8, 1.02f32, [64u8, 32, 96]), (10.0, 6, 1.03, [128, 64, 128])] {
            let points = (0..16)
                .map(|_| {
                    let x = (lcg(&mut seed) % 20) as f32 - 10.0;
                    let y = base + (lcg(&mut seed) & 15) as f32 - 4.0;
                    [x, y, 0.0]
                })
                .collect();
            emitters.push((points, 96, step, scale, 12.0, rgb));
        }
        Self { pos, emitters, u: 0, tris_alpha: [96, 96] }
    }

    pub fn update(&mut self, u: u32) {
        self.u = u;
        for (n, e) in self.emitters.iter_mut().enumerate() {
            for p in &mut e.0 {
                *p = p.map(|v| v * e.3);
            }
            self.tris_alpha[n] = e.1;
            e.1 = e.1.saturating_sub(e.2);
        }
    }

    pub fn draw(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<FxTri> {
        let mut out = Vec::new();
        let Some(project) = projector(eye, target) else {
            return out;
        };
        let [right, up, toward] = billboard_axes(eye, target);
        let u = self.u;
        for (n, e) in self.emitters.iter().enumerate() {
            let a = self.tris_alpha[n];
            for (k, p) in e.0.iter().enumerate() {
                let k = k as u32;
                if !(k == 0 || u >= 3 * k) {
                    continue;
                }
                let f = (u.saturating_sub(1 + 3 * k) / 2).min(7) as f32;
                let h = e.4;
                let corner = |dx: f32, dy: f32| -> Option<[f32; 2]> {
                    let l = [p[0] + dx, p[1] + dy, p[2]];
                    project(std::array::from_fn(|i| {
                        self.pos[i] + f64::from(l[0]) * right[i] + f64::from(l[1]) * up[i] + f64::from(l[2]) * toward[i]
                    }))
                };
                let (Some(a0), Some(a1), Some(a2), Some(a3)) = (corner(-h, h), corner(h, h), corner(-h, -h), corner(h, -h)) else {
                    continue;
                };
                let uv = [[240.0, 16.0 * f], [255.0, 16.0 * f], [240.0, 16.0 * f + 15.0], [255.0, 16.0 * f + 15.0]].map(|[x, y]: [f32; 2]| [x / 256.0, y / 256.0]);
                let c = [e.5[0], e.5[1], e.5[2], a];
                out.push(FxTri { screen: [a0, a1, a2], uv: [uv[0], uv[1], uv[2]], colour: [c; 3], textured: true, sheet: 15 });
                out.push(FxTri { screen: [a1, a3, a2], uv: [uv[1], uv[3], uv[2]], colour: [c; 3], textured: true, sheet: 15 });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c603_alpha_blend_and_spin() {
        let mut c = C603::new([0.0; 3], [0x4000, 0, 0x8000], 4);
        c.update(1);
        assert_eq!(c.alpha(), (128, false));
        assert_eq!(c.pieces().len(), 368);
        c.update(16);
        assert_eq!(c.alpha(), (98, false));
        c.update(17);
        assert_eq!(c.alpha(), (96, true));
        c.update(60);
        assert_eq!(c.alpha().0, 10);
    }

    #[test]
    fn c603_first_update_is_the_whole_card() {
        // u 1: no spin, no walk yet; side-1 upright face up card on the
        // board: local x -> -X... the 368 pieces tile 25.6 x 36.8 units.
        let mut c = C603::new([0.0; 3], [0x4000, 0, 0x8000], 4);
        c.update(1);
        let p = c.pieces();
        let xs: Vec<f64> = p.iter().flat_map(|q| q.0.iter().map(|v| v[0])).collect();
        let zs: Vec<f64> = p.iter().flat_map(|q| q.0.iter().map(|v| v[2])).collect();
        let span = |v: &[f64]| v.iter().cloned().fold(f64::MIN, f64::max) - v.iter().cloned().fold(f64::MAX, f64::min);
        assert!((span(&xs) - 25.6).abs() < 1e-3 && (span(&zs) - 36.8).abs() < 1e-3, "{} {}", span(&xs), span(&zs));
        assert!(p.iter().flat_map(|q| q.0.iter()).all(|v| v[1].abs() < 1e-4));
    }

    #[test]
    fn c545_alphas_and_particles() {
        let mut c = C545::new([0.0; 3], 3);
        c.update(1);
        assert_eq!(c.tris_alpha, [96, 96]);
        c.update(2);
        assert_eq!(c.tris_alpha, [88, 90]);
        for u in 3..=13 {
            c.update(u);
        }
        assert_eq!(c.tris_alpha[0], 0);
    }
}

#[cfg(test)]
mod param_tests {
    use super::*;

    #[test]
    fn param_grids() {
        let c = C603::new([0.0; 3], [0x4000, 0, 0x8000], 1);
        assert_eq!(c.offsets.len(), 368);
        assert_eq!((c.offsets[0], c.offsets[17]), ([-12.0, 17.6, 0.0], [-10.4, 16.0, 0.0]));
        let p = C603::with_param([0.0; 3], [0x4000, 0, 0x8000], 1, 1);
        assert_eq!(p.offsets.len(), 64);
        assert_eq!(p.offsets[9], [-11.2 + 3.2, 16.1 - 4.6, 0.0]);
        assert_eq!(p.pieces()[0].1[3], [31.0, 45.0]);
    }
}
