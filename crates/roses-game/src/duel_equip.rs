//! The equip / power-up pair modules of a summon chain, as traced in
//! the project notes [F + emu: the original module code run
//! headless in the EE interpreter; no footage of an equip pair exists].
//!
//! Every module keeps C538's card choreography without the spin (both
//! copies hold 9 updates, B slides onto A at view (-101, 8) in 6 with its
//! trail, B fades by 16 per update from 3 updates after the module's
//! return 2) and draws its effect layers from update 16, additive, Z max,
//! OT key 0 (over the HUD), on one billboard node at view (-101, 8).
//!
//! Ported: 0x218 (the stat power-ups) and 0x219 Riryoku (`duel_c537`: its
//! "+N" number and the green library, `EquipFx::riryoku`; NEWLY-ALLOWED.md
//! 14). Not ported (their cards are refused by the rules): 0x21C Crush
//! Card's smoke, 0x21D Paralyzing Potion's sparks and strands, 0x21E
//! Cursebreaker's key.
use crate::duel_c600::{decay, project_at, rotate};
use crate::duel_c621::FxTri;

/// A module's length in updates and the update that returns 2 (the
/// handler applies the equip on it) [F, emu].
pub fn timing(module: u16) -> Option<(u32, u32)> {
    match module {
        0x218 | 0x219 | 0x21D => Some((43, 22)),
        0x21C => Some((46, 26)),
        0x21E => Some((52, 32)),
        _ => None,
    }
}

/// The handler's class sound at its counter 15 (bank 1) [F 0x271B90 /
/// 0x272560].
pub fn class_sound(module: u16) -> Option<u8> {
    match module {
        0x218 => Some(0x3B),
        0x219 => Some(0x4D),
        0x21C => Some(0x4B),
        0x21D => Some(0x4E),
        0x21E => Some(0x4F),
        _ => None,
    }
}

/// B's alpha at module update `u`: 128, then -16 per update from
/// ret 2 + 3 [emu].
pub fn b_alpha(module: u16, u: u32) -> u8 {
    let Some((_, ret2)) = timing(module) else {
        return 128;
    };
    let k = u.saturating_sub(ret2 + 2);
    128u32.saturating_sub(16 * k) as u8
}

/// The effect library's ring / glow / ray set (the C600 / C604 code) with a
/// module's colours: node scale 0.1 at the first draw, +0.2 while <= 1,
/// then +0.005 with the alpha decays (rings 2, glow 3, rays 2) [F, emu].
#[derive(Clone, Debug, PartialEq)]
pub struct EquipFx {
    seed: u64,
    start: u32,
    s: f32,
    ring_a: u8,
    ring_b: u8,
    glow: u8,
    rays_alpha: u8,
    /// RGB of ring A, ring B, glow, rays.
    colours: [[u8; 3]; 4],
    rays: Vec<[i32; 3]>,
    /// The node's view position on the depth-800 plane.
    anchor: [f32; 2],
    /// SPARKS (c601): points (x 1.3 per update), alpha, colour.
    sparks: Vec<[f32; 3]>,
    spark_alpha: u8,
    spark_rgb: [u8; 3],
    /// The draws of the last update.
    pub tris: Vec<FxTri>,
    /// Riryoku's module (0x219), which draws its number and its library
    /// itself.
    c537: Option<crate::duel_c537::C537>,
}

impl EquipFx {
    /// The module's effect layers, `None` for a module not ported.
    pub fn new(module: u16, seed: u32) -> Option<Self> {
        let colours = match module {
            // 0x218 "Power increase!" [F descriptors, emu q536].
            0x218 => [[224, 224, 128], [128, 128, 224], [128, 128, 224], [128, 128, 224]],
            _ => return None,
        };
        let mut fx = Self {
            seed: u64::from(seed) | 1,
            start: 16,
            s: 0.1,
            ring_a: 32,
            ring_b: 48,
            glow: 96,
            rays_alpha: 32,
            colours,
            rays: Vec::new(),
            anchor: [-101.0, 8.0],
            sparks: Vec::new(),
            spark_alpha: 0,
            spark_rgb: [0; 3],
            tris: Vec::new(),
            c537: None,
        };
        fx.rays = (0..64).map(|_| std::array::from_fn(|_| (fx.rand() & 0xFFFF) as i32)).collect();
        Some(fx)
    }

    /// c601's burst (the big card show, from its update 29): sparks, rings
    /// (224, 224, 128), glow and rays (128, 128, 224) on a node at view
    /// (0, 0) [F/emu ACTIVATION-PRESENTATION.md 2.6].
    pub fn c601(seed: u32) -> Self {
        let mut fx = Self::new(0x218, seed).expect("0x218 colours");
        fx.start = 29;
        fx.anchor = [0.0, 0.0];
        fx.colours = [[224, 224, 128], [224, 224, 128], [128, 128, 224], [128, 128, 224]];
        fx.spark_rgb = [128, 128, 224];
        fx.spark_alpha = 128;
        fx.sparks = (0..60)
            .map(|_| [(fx.rand() % 50) as f32 - 25.0, (fx.rand() % 25) as f32, (fx.rand() % 50) as f32 - 25.0])
            .collect();
        fx
    }

    /// Riryoku's pair module 0x219 (`EquipFx::new` has no amount for it):
    /// `param` = (amount << 16) | (sign << 8) | mode, the number "+N" green
    /// / "-N" red; colours (128, 224, 128) [F/emu NEWLY-ALLOWED.md 14].
    pub fn riryoku(seed: u32, param: u32) -> Self {
        let mut fx = Self::new(0x218, seed).expect("0x218 colours");
        fx.colours = [[128, 224, 128]; 4];
        let mut rng = crate::duel_fxlib::Rng::new(seed);
        fx.c537 = Some(crate::duel_c537::C537::new(param, &mut || rng.next()));
        fx
    }

    /// Riryoku's number: (ALPHA 0x88 Cs * As at u 16..21, ALPHA 0x44 alpha
    /// blend from u 22) screen triangles, drawn before the library.
    pub fn number_tris(&self) -> (&[FxTri], &[FxTri]) {
        self.c537.as_ref().map_or((&[], &[]), |m| (&m.opaque, &m.alpha_blend))
    }

    /// The Riryoku module, if this is one.
    pub fn c537(&self) -> Option<&crate::duel_c537::C537> {
        self.c537.as_ref()
    }

    /// newlib's rand (the port's own sequence; only the distribution matches).
    fn rand(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
        ((self.seed >> 32) & 0x7FFF_FFFF) as u32
    }

    fn quad(&mut self, p: [[f32; 3]; 4], uv: [[f32; 2]; 4], rgb: [u8; 3], a: u8) {
        let mut sc = [[0.0; 2]; 4];
        for i in 0..4 {
            match project_at(self.anchor, self.s, p[i]) {
                Some(v) => sc[i] = v,
                None => return,
            }
        }
        let uv = uv.map(|[u, v]| [u / 256.0, v / 256.0]);
        let c = [rgb[0], rgb[1], rgb[2], a];
        self.tris.push(FxTri { screen: [sc[0], sc[1], sc[2]], uv: [uv[0], uv[1], uv[2]], colour: [c; 3], textured: true, sheet: 15 });
        self.tris.push(FxTri { screen: [sc[1], sc[3], sc[2]], uv: [uv[1], uv[3], uv[2]], colour: [c; 3], textured: true, sheet: 15 });
    }

    /// Module update `u`: draw (rings, glow, rays), then step.
    pub fn update(&mut self, u: u32) {
        self.tris.clear();
        if let Some(m) = &mut self.c537 {
            m.update(u);
            self.tris = m.additive.clone();
            return;
        }
        if u < self.start {
            return;
        }
        let [ca, cb, cg, cr] = self.colours;
        for i in 0..self.sparks.len() {
            self.sparks[i] = self.sparks[i].map(|v| v * 1.3);
            let [x, y, z] = self.sparks[i];
            let (rgb, a) = (self.spark_rgb, self.spark_alpha);
            self.quad(
                [[x - 3.0, y + 3.0, z], [x + 3.0, y + 3.0, z], [x - 3.0, y - 3.0, z], [x + 3.0, y - 3.0, z]],
                [[176.0, 64.0], [207.0, 64.0], [176.0, 95.0], [207.0, 95.0]],
                rgb,
                a,
            );
        }
        for (outer, rgb, a) in [(115.0f32, ca, self.ring_a), (130.0, cb, self.ring_b)] {
            let ring = |r: f32, j: u32| {
                let t = (12.0 * j as f32).to_radians();
                [r * t.cos(), r * t.sin(), 0.0]
            };
            for j in 0..30 {
                self.quad(
                    [ring(120.0, j), ring(outer, j), ring(120.0, j + 1), ring(outer, j + 1)],
                    [[142.0, 64.0], [142.0, 0.0], [174.0, 64.0], [174.0, 0.0]],
                    rgb,
                    a,
                );
            }
        }
        let p = |x: f32, y: f32| [100.0 * x, 100.0 * y, 0.0];
        let pts = [p(-1.0, 0.0), p(-1.0, 1.0), p(0.0, 1.0), p(1.0, 1.0), p(1.0, 0.0), p(1.0, -1.0), p(0.0, -1.0), p(-1.0, -1.0)];
        let c = [0.0, 0.0, 0.0];
        for q in [[pts[1], pts[2], pts[0], c], [pts[3], pts[2], pts[4], c], [pts[5], pts[6], pts[4], c], [pts[7], pts[6], pts[0], c]] {
            let a = self.glow;
            self.quad(q, [[176.0, 32.0], [208.0, 32.0], [176.0, 64.0], [208.0, 64.0]], cg, a);
        }
        for i in 0..64 {
            let r = self.rays[i];
            let t = [[-15.0, 300.0, 0.0], [15.0, 300.0, 0.0], [-7.5, 0.0, 0.0], [7.5, 0.0, 0.0]].map(|v| rotate(v, r));
            let a = self.rays_alpha;
            self.quad(t, [[142.0, 0.0], [175.0, 0.0], [142.0, 63.0], [175.0, 63.0]], cr, a);
        }
        if self.s <= 1.0 {
            self.s += 0.2;
        } else {
            self.s += 0.005;
            self.ring_a = decay(self.ring_a, 2);
            self.ring_b = decay(self.ring_b, 2);
            self.glow = decay(self.glow, 3);
            self.rays_alpha = decay(self.rays_alpha, 2);
            self.spark_alpha = decay(self.spark_alpha, 2);
        }
        for r in &mut self.rays {
            *r = r.map(|v| v + 0x80);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_218_alphas_and_scale_follow_the_interpreter() {
        let mut fx = EquipFx::new(0x218, 7).unwrap();
        let mut drawn = Vec::new();
        for u in 1..=43 {
            drawn.push((u, fx.s, fx.ring_a, fx.ring_b, fx.glow));
            fx.update(u);
            if u == 16 {
                // 60 ring quads + 4 glow + 64 rays, all on screen at 0.1.
                assert_eq!(fx.tris.len(), 2 * 128);
            }
        }
        let at = |u: u32| drawn[(u - 1) as usize];
        assert!((at(16).1 - 0.1).abs() < 1e-6 && (at(21).1 - 1.1).abs() < 1e-5);
        assert_eq!((at(21).2, at(22).2, at(37).2), (32, 30, 0));
        assert_eq!((at(43).3, at(43).4), (4, 30));
        assert!((at(43).1 - 1.21).abs() < 1e-4, "{}", at(43).1);
        assert!(fx.tris.len() > 0);
    }

    #[test]
    fn c601_burst_decays_as_traced() {
        // Glow 0 at u 66, rays 0 at u 50 (drawn values) [emu].
        let mut fx = EquipFx::c601(3);
        let mut glow = Vec::new();
        for u in 1..=70 {
            glow.push((fx.glow, fx.rays_alpha));
            fx.update(u);
        }
        assert_eq!(glow[28], (96, 32));
        assert_eq!((glow[49].1, glow[65].0), (0, 0));
        assert_eq!(glow[34], (93, 30));
    }

    #[test]
    fn riryoku_pair_shows_the_drained_number_and_its_library() {
        // "+500": glyphs at X 95 / 125 / 155 / 185, opaque at u 21, alpha
        // blended from u 22 (alpha 124), 128 library quads at u 16 [F/emu 14].
        let mut fx = EquipFx::riryoku(9, 500 << 16);
        assert_eq!(timing(0x219), Some((43, 22)));
        assert_eq!(class_sound(0x219), Some(0x4D));
        for u in 1..=15 {
            fx.update(u);
            assert!(fx.tris.is_empty() && fx.number_tris().0.is_empty());
        }
        fx.update(16);
        assert_eq!(fx.tris.len(), 2 * 128);
        let xs: Vec<f32> = fx.number_tris().0.iter().step_by(2).map(|t| t.screen[0][0]).collect();
        assert_eq!(xs, vec![95.0, 125.0, 155.0, 185.0]);
        for u in 17..=21 {
            fx.update(u);
        }
        assert!(!fx.number_tris().0.is_empty() && fx.number_tris().1.is_empty());
        fx.update(22);
        assert!(fx.number_tris().0.is_empty());
        assert_eq!(fx.number_tris().1[0].colour[0], [96, 176, 112, 124]);
        assert_eq!(fx.c537().map(|m| m.amount()), Some(500));
        for u in 23..=43 {
            fx.update(u);
        }
        assert_eq!(fx.number_tris().1[0].colour[0][3], 40);
        fx.update(44);
        assert!(fx.tris.is_empty() && fx.number_tris().1.is_empty());
        assert!(EquipFx::new(0x219, 1).is_none(), "0x219 needs its amount");
    }

    #[test]
    fn b_fades_from_three_updates_after_ret_2() {
        assert_eq!((b_alpha(0x218, 24), b_alpha(0x218, 25), b_alpha(0x218, 32)), (128, 112, 0));
        assert_eq!((b_alpha(0x21C, 29), b_alpha(0x21E, 35)), (112, 112));
    }
}
