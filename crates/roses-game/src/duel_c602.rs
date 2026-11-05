//! CMODULE c602 (0x25A), the trap / effect burst on the activated card, as
//! traced in the project notes [F code; emu: an independent
//! model matched every vertex of the interpreter's 30 updates to 0.15 px
//! with the port's board camera]:
//! - RAYS: 64 fixed vertical blades every 5.625 deg round the cell centre
//!   (TEXEFF 15 (208, 32)), (176, 176, 96), alpha 32;
//! - POINTS: 60 stars in a camera-facing cloud 3 units above it, x 1.04 per
//!   update (TEXEFF 8 (0, 90)), grey, alpha 128;
//! - RINGS A/B/C: horizontal jagged crowns, radius 0.1 -> 34 / 37 / 40 at
//!   heights 28 / 21 / 14 (random rim), TEXEFF 15 strip (142..174, 0..64),
//!   (176, 176, 96), alpha 96.
//! All additive. Rays / rings scale 0.1, 0.4, 0.7, 1.0 then +0.05; points
//! stop at 1.0; decays from u 8 (rays 2, points 10, rings 6).
//!
//! [H / not reproduced]: the port's random values (distributions only);
//! the original sorts the primitives with the scene by depth, the port
//! draws them over the board without a depth test; params 1 / 2 (card fade,
//! shatter) are not ported.
use crate::duel_c600::{decay, rotate};
use crate::duel_c621::FxTri;

/// c602's length (returns 8 at u 30).
pub const UPDATES: u32 = 30;

#[derive(Clone, Debug, PartialEq)]
pub struct C602 {
    seed: u64,
    /// The node position: the target card's cell centre.
    pub pos: [f64; 3],
    s_rays: f32,
    s_points: f32,
    a_rays: u8,
    a_points: u8,
    a_rings: u8,
    points: Vec<[f32; 3]>,
    /// Per ring: 30 (inner, outer) node-local points.
    rings: Vec<Vec<([f32; 3], [f32; 3])>>,
    /// The update last stepped (`points` as drawn in it).
    u: u32,
    /// (rays / rings scale, points scale, alphas rays / points / rings) as
    /// drawn in that update.
    drawn: (f32, f32, u8, u8, u8),
}

impl C602 {
    pub fn new(pos: [f64; 3], seed: u32) -> Self {
        let mut c = Self {
            seed: u64::from(seed) | 1,
            pos,
            s_rays: 0.1,
            s_points: 0.1,
            a_rays: 32,
            a_points: 128,
            a_rings: 96,
            points: Vec::new(),
            rings: Vec::new(),
            u: 0,
            drawn: (0.1, 0.1, 32, 128, 96),
        };
        // 0x1E00700(desc, tex, 0.1, r_out, 0, h), the XZ variant: point 0
        // without rand, points 1..30 at 12-degree steps (point 30 unused).
        for (r_out, h) in [(34.0f32, 28i32), (37.0, 21), (40.0, 14)] {
            let r_in = 0.1f32;
            let mut ring = vec![([r_in, 0.0, 0.0], [r_out, h as f32, 0.0])];
            for k in 1..=30 {
                let a = 0.209_439_5f32 * k as f32;
                let y = h + (((c.rand() % (2 * h as u32)) as i32 - h) >> 1);
                ring.push(([r_in * a.cos(), 0.0, r_in * a.sin()], [r_out * a.cos(), y as f32, r_out * a.sin()]));
            }
            ring.truncate(30);
            c.rings.push(ring);
        }
        // The unused fourth descriptor's 30 draws (kept for the order).
        for _ in 0..30 {
            c.rand();
        }
        c.points = (0..60)
            .map(|_| [(c.rand() % 50) as f32 - 25.0, (c.rand() % 25) as f32, (c.rand() % 50) as f32 - 25.0])
            .collect();
        c
    }

    /// newlib's rand (the port's own sequence).
    fn rand(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
        ((self.seed >> 32) & 0x7FFF_FFFF) as u32
    }

    /// Module update `u` (1..=30): the points move (drawn this update), then
    /// the scales and alphas step for the next one (0x1E024A8's order).
    pub fn update(&mut self, u: u32) {
        self.u = u;
        for p in &mut self.points {
            *p = p.map(|v| v * 1.04);
        }
        self.drawn = (self.s_rays, self.s_points, self.a_rays, self.a_points, self.a_rings);
        if self.s_points < 1.0 {
            self.s_points += 0.3;
        }
        if u >= 7 {
            self.a_points = decay(self.a_points, 10);
            self.a_rays = decay(self.a_rays, 2);
            self.a_rings = decay(self.a_rings, 6);
        }
        self.s_rays += if self.s_rays < 1.0 { 0.3 } else { 0.05 };
    }
}

impl C602 {
    /// The triangles of the update last stepped, through the board camera
    /// (`camera::look_at(eye, target)` + `GS_PROJECTION`; a quad is sent
    /// only if its 4 vertices pass 0x204D80's tests) [F, emu].
    pub fn draw(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<FxTri> {
        let mut out = Vec::new();
        if self.u == 0 {
            return out;
        }
        let Some(view) = crate::camera::look_at(eye, target) else {
            return out;
        };
        let clip = crate::camera::mul(&crate::duel_board::GS_PROJECTION, &view);
        let project = |p: [f64; 3]| -> Option<[f32; 2]> {
            let c: [f64; 4] = std::array::from_fn(|r| clip[0][r] * p[0] + clip[1][r] * p[1] + clip[2][r] * p[2] + clip[3][r]);
            if c[3] <= 0.0 {
                return None;
            }
            let (x, y, z) = (c[0] / c[3], c[1] / c[3], c[2] / c[3]);
            if !(1.0..=16_777_215.0).contains(&z) || !(0.0..4096.0).contains(&x) || !(0.0..4096.0).contains(&y) {
                return None;
            }
            Some([(x - 1792.0) as f32, (y - 1792.0) as f32])
        };
        let (s_rays, s_points, a_rays, a_points, a_rings) = self.drawn;
        let mut quad = |w: [[f64; 3]; 4], uv: [[f32; 2]; 4], rgba: [u8; 4], sheet: u8, size: f32| {
            let mut sc = [[0.0f32; 2]; 4];
            for i in 0..4 {
                match project(w[i]) {
                    Some(v) => sc[i] = v,
                    None => return,
                }
            }
            let uv = uv.map(|[u, v]| [u / size, v / size]);
            out.push(FxTri { screen: [sc[0], sc[1], sc[2]], uv: [uv[0], uv[1], uv[2]], colour: [rgba; 3], textured: true, sheet });
            out.push(FxTri { screen: [sc[1], sc[3], sc[2]], uv: [uv[1], uv[3], uv[2]], colour: [rgba; 3], textured: true, sheet });
        };
        let world = |s: f32, l: [f32; 3]| -> [f64; 3] { std::array::from_fn(|k| self.pos[k] + f64::from(s) * f64::from(l[k])) };
        // RAYS (node rotation 0; blade i turned by (0, 0x400 i, 0)).
        let blade = [[-1.5, 75.0, 0.0], [0.0, 75.0, 0.0], [-6.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
        for i in 0..64 {
            let w = blade.map(|v| world(s_rays, rotate(v, [0, 0x400 * i, 0])));
            quad(w, [[208.0, 32.0], [239.0, 32.0], [208.0, 63.0], [239.0, 63.0]], [176, 176, 96, a_rays], 15, 256.0);
        }
        // POINTS: a billboard at the card + (0, 3, 0): local x = screen
        // right, y = screen up, z = toward the eye.
        let f = norm(sub(target, eye));
        let right = norm(cross(f, [0.0, 1.0, 0.0]));
        let down = cross(f, right);
        let centre = [self.pos[0], self.pos[1] + 3.0, self.pos[2]];
        let s = f64::from(s_points);
        for p in &self.points {
            let corner = |dx: f32, dy: f32| -> [f64; 3] {
                let (qx, qy, qz) = (f64::from(p[0] + dx), f64::from(p[1] + dy), f64::from(p[2]));
                std::array::from_fn(|k| centre[k] + s * (qx * right[k] - qy * down[k] - qz * f[k]))
            };
            let w = [corner(-2.0, 2.0), corner(2.0, 2.0), corner(-2.0, -2.0), corner(2.0, -2.0)];
            quad(w, [[0.0, 90.0], [9.0, 90.0], [0.0, 99.0], [9.0, 99.0]], [128, 128, 128, a_points], 8, 128.0);
        }
        // RINGS A, B, C (node rotation 0).
        for ring in &self.rings {
            for k in 0..30 {
                let (i0, o0) = ring[k];
                let (i1, o1) = ring[(k + 1) % 30];
                let w = [world(s_rays, i0), world(s_rays, o0), world(s_rays, i1), world(s_rays, o1)];
                quad(w, [[142.0, 64.0], [142.0, 0.0], [174.0, 64.0], [174.0, 0.0]], [176, 176, 96, a_rings], 15, 256.0);
            }
        }
        out
    }
}

/// A world point -> 512-buffer pixel through the board camera
/// (`camera::look_at` + `GS_PROJECTION`), `None` when 0x204D80 rejects it.
pub fn projector(eye: [f64; 3], target: [f64; 3]) -> Option<impl Fn([f64; 3]) -> Option<[f32; 2]>> {
    let view = crate::camera::look_at(eye, target)?;
    let clip = crate::camera::mul(&crate::duel_board::GS_PROJECTION, &view);
    Some(move |p: [f64; 3]| -> Option<[f32; 2]> {
        let c: [f64; 4] = std::array::from_fn(|r| clip[0][r] * p[0] + clip[1][r] * p[1] + clip[2][r] * p[2] + clip[3][r]);
        if c[3] <= 0.0 {
            return None;
        }
        let (x, y, z) = (c[0] / c[3], c[1] / c[3], c[2] / c[3]);
        if !(1.0..=16_777_215.0).contains(&z) || !(0.0..4096.0).contains(&x) || !(0.0..4096.0).contains(&y) {
            return None;
        }
        Some([(x - 1792.0) as f32, (y - 1792.0) as f32])
    })
}

/// A billboard node's world axes (0x1039A0: rotation = the camera
/// angles): screen right, screen up, toward the eye.
pub fn billboard_axes(eye: [f64; 3], target: [f64; 3]) -> [[f64; 3]; 3] {
    let f = norm(sub(target, eye));
    let right = norm(cross(f, [0.0, 1.0, 0.0]));
    let down = cross(f, right);
    [right, down.map(|v| -v), f.map(|v| -v)]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn norm(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-12);
    a.map(|v| v / l)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trap camera on cell (2, 1), side 0: eye = T + 180 (0, sin 22.5,
    /// cos 22.5).
    fn camera() -> ([f64; 3], [f64; 3]) {
        let t = [-50.0, 0.0, -100.0];
        let p = 22.5f64.to_radians();
        (std::array::from_fn(|k| t[k] + 180.0 * [0.0, p.sin(), p.cos()][k]), t)
    }

    #[test]
    fn closed_form_projection_of_the_note() {
        // A ray vertex at s 1 (u 4): (-1.5, 75, 0) on blade 0 -> view
        // vx = dx, vy = -c22 dy + s22 dz, vz = 180 - s22 dy - c22 dz.
        let mut c = C602::new([-50.0, 0.0, -100.0], 7);
        for u in 1..=4 {
            c.update(u);
        }
        assert!((c.drawn.0 - 1.0).abs() < 1e-5, "{}", c.drawn.0);
        let (eye, target) = camera();
        let tris = c.draw(eye, target);
        let (c22, s22) = (22.5f64.to_radians().cos(), 22.5f64.to_radians().sin());
        let (dx, dy) = (-1.5, 75.0);
        let (vx, vy, vz) = (dx, -c22 * dy, 180.0 - s22 * dy);
        let want = [(256.0 + 800.0 * vx / vz) as f32, (256.0 + 1074.2855 * vy / vz) as f32];
        let got = tris[0].screen[0];
        assert!((got[0] - want[0]).abs() < 0.01 && (got[1] - want[1]).abs() < 0.01, "{got:?} {want:?}");
        // Base vertex (0, 0, 0) of every blade is the cell centre: (256, 256).
        assert!((tris[1].screen[1][0] - 256.0).abs() < 1e-3 && (tris[1].screen[1][1] - 256.0).abs() < 1e-3, "{:?}", tris[1]);
    }

    #[test]
    fn scales_and_alphas_follow_the_note() {
        let mut c = C602::new([0.0; 3], 3);
        let mut seen = Vec::new();
        for u in 1..=30 {
            c.update(u);
            seen.push(c.drawn);
        }
        let at = |u: usize| seen[u - 1];
        assert!((at(1).0 - 0.1).abs() < 1e-6 && (at(2).0 - 0.4).abs() < 1e-6 && (at(4).0 - 1.0).abs() < 1e-5);
        assert!((at(30).0 - 2.3).abs() < 1e-4 && (at(30).1 - 1.0).abs() < 1e-5);
        assert_eq!((at(7).2, at(8).2, at(23).2), (32, 30, 0));
        assert_eq!((at(8).3, at(19).3, at(20).3), (118, 8, 0));
        assert_eq!((at(8).4, at(23).4), (90, 0));
    }
}
