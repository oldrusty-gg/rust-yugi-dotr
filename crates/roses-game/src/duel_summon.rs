//! The summon flight (states 0x4C / 0x4D, then 0x4F commit, 0x50 cleanup and
//! 0x16): the picked card held front-up at view (0, 8) in 0x46 flies to its
//! cell in 29 updates and lands as the face-down board card [F/emu
//! `claude-summon/SUMMON-PRESENTATION.md` 2]:
//! - every field moves linearly with c = floor(128 k / 30), k = 2..30 (one
//!   per update), from the 0x46 pose: position view (0, 8) at depth 800,
//!   spin (0, 0x10000, 0x10000), tilt = the camera's angles, scale (0.375,
//!   0.3913); to the cell centre (50 (c - 3), 0, 50 (r - 3)), spin (0x4000,
//!   0x8000 for side 0 / 0 for side 1, 0), tilt 0, scale 0.1; ints
//!   truncated and wrapped to 16 bits (0x1CF140);
//! - three more updates (commit, cleanup, 0x16) before the cursor.
//! Checked against the capture's k = 2 / 15 / 30 samples (test).
use crate::duel_versus::{CardPose, VersusCard};

/// Flight updates (k = 2..30) and the updates after it until the cursor.
pub const FLIGHT_UPDATES: u32 = 29;
pub const AFTER_UPDATES: u32 = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct SummonFlight {
    /// The flying card (its pose is set from `from`, `to` and the step).
    pub card: VersusCard,
    pub from: CardPose,
    pub to: CardPose,
    /// Updates played (1 = k 2).
    pub step: u32,
    /// The summoned unit (side, slot), hidden on the board until it lands.
    pub unit: (u8, u8),
    /// A fusion chain's modules and the survivor's turn, played first.
    pub fusion: Option<FusionShow>,
}

/// The 0x46 pose: view (0, 8) at depth 800 through `view`, facing the camera.
pub fn held_pose(view: &crate::camera::Mat4, polar: [f64; 3]) -> Option<CardPose> {
    let m: roses_assets::opening_scene::Matrix = view.map(|r| r.map(|v| v as f32));
    let p = roses_assets::opening_scene::ee_view::view_to_world(&m, 0.0, 8.0, 800.0).ok()??;
    Some(CardPose {
        position: [p[0], p[1], p[2]],
        spin: [0, 0x10000, 0x10000],
        tilt: crate::duel_versus::camera_angles(polar),
        scale: [0.375, 0.3913],
    })
}

/// The board pose a summoned card lands in: face down and upright for a
/// fresh record; a surviving field unit (slot 0 of a chain) carries its
/// committed flags +0x57 bit4 (face up: z 0x8000) and bit5 (defence: y
/// - 0x4000) [F SUMMON-PRESENTATION.md 2.1-2.2; H: the signed y end].
pub fn landing_pose(side: u8, cell: (u8, u8), face_up: bool, defence: bool) -> CardPose {
    let [x, z] = crate::duel_board::cell_center(cell.0, cell.1);
    let y = crate::duel_play::side_model_base(side) - if defence { 0x4000 } else { 0 };
    CardPose {
        position: [x as f32, 0.0, z as f32],
        spin: [0x4000, y, if face_up { 0x8000 } else { 0 }],
        tilt: [0, 0, 0],
        scale: [0.1, 0.1],
    }
}

/// 0x1CF140 at counter k (c = floor(128 k / 30), t = c / 128).
pub fn pose_at(from: &CardPose, to: &CardPose, k: u32) -> CardPose {
    let c = (128 * k.min(30) / 30) as f32;
    let t = c / 128.0;
    let f = |a: f32, b: f32| a * (1.0 - t) + b * t;
    let i = |a: i32, b: i32| (f(a as f32, b as f32) as i32) & 0xFFFF;
    CardPose {
        position: std::array::from_fn(|n| f(from.position[n], to.position[n])),
        spin: std::array::from_fn(|n| i(from.spin[n], to.spin[n])),
        tilt: std::array::from_fn(|n| i(from.tilt[n], to.tilt[n])),
        scale: std::array::from_fn(|n| f(from.scale[n], to.scale[n])),
    }
}

impl SummonFlight {
    pub fn new(face: (u16, u8, i32, i32), from: CardPose, side: u8, cell: (u8, u8), unit: (u8, u8), committed: (bool, bool)) -> Self {
        let to = landing_pose(side, cell, committed.0, committed.1);
        Self {
            card: VersusCard {
                card: face.0,
                level: face.1,
                attack: face.2,
                defence: face.3,
                pose: from,
                target: to,
                remaining: 0,
            },
            from,
            to,
            step: 0,
            unit,
            fusion: None,
        }
    }

    /// One update; true while the presentation lasts.
    pub fn tick(&mut self) -> bool {
        if let Some(f) = &mut self.fusion {
            if !f.done() {
                f.tick();
                return true;
            }
        }
        self.step += 1;
        if self.step <= FLIGHT_UPDATES {
            self.card.pose = pose_at(&self.from, &self.to, self.step + 1);
        }
        self.step < FLIGHT_UPDATES + AFTER_UPDATES
    }

    /// The card still flies (drawn by the presentation, the unit hidden).
    pub fn flying(&self) -> bool {
        self.step < FLIGHT_UPDATES
    }

    /// The fusion presentation while it plays.
    pub fn fusion_playing(&self) -> Option<&FusionShow> {
        self.fusion.as_ref().filter(|f| !f.done())
    }
}

// ---------------------------------------------------------------- fusion

/// One pair of a summon chain as the modules show it (SUMMON-PRESENTATION.md
/// 4): C600 for a fusion (103 updates; the result appears from update 81)
/// and C538 for a failed pair (28 updates; the first card spins away, the
/// second survives).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pair {
    Fuse { result: (u16, u8, i32, i32) },
    Fail,
    /// A power-up onto a monster (`duel_equip`): `module` 0x218..0x21E,
    /// `monster` the face that survives.
    Equip { module: u16, monster: (u16, u8, i32, i32) },
}

/// A card drawn by the fusion presentation: its face, view-plane position
/// (x, y at depth 800), spin and vertex alpha (128 = opaque).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShownCard {
    pub face: (u16, u8, i32, i32),
    pub view: [f32; 2],
    pub spin: [i32; 3],
    pub alpha: u8,
}

/// C600 / C538 per pair, then the survivor's turn to the centre (0x4B,
/// 9 updates, c = 25, 38 .. 128 with one full turn) [F/emu].
#[derive(Clone, Debug, PartialEq)]
pub struct FusionShow {
    pairs: Vec<((u16, u8, i32, i32), Pair)>,
    /// The survivor (card A) of the running pair.
    survivor: (u16, u8, i32, i32),
    pair: usize,
    /// Module update (u = 1 at the first).
    u: u32,
    a: [f32; 2],
    b: [f32; 2],
    a_spin: [i32; 3],
    history: Vec<[f32; 2]>,
    history_a: Vec<[f32; 2]>,
    radius: f32,
    angle: i32,
    speed: i32,
    /// Updates of the final turn to the centre played (0 = not started).
    turn: u32,
    /// The pair bookkeeping updates between modules / before 0x4A.
    gap: u32,
    /// C600's effect layers of a running fusion pair.
    fx: Option<crate::duel_c600::C600>,
    /// An equip module's effect layers.
    equip_fx: Option<crate::duel_equip::EquipFx>,
}

const SPIN0: [i32; 3] = [0, 0x10000, 0x10000];

impl FusionShow {
    /// `first` = pick 1 (card A of the first pair); each pair's card B is
    /// the next pick.
    pub fn new(first: (u16, u8, i32, i32), pairs: Vec<((u16, u8, i32, i32), Pair)>) -> Self {
        Self {
            pairs,
            survivor: first,
            pair: 0,
            u: 0,
            a: [-101.0, 8.0],
            b: [101.0, 8.0],
            a_spin: SPIN0,
            history: Vec::new(),
            history_a: Vec::new(),
            radius: 100.0,
            angle: 0,
            speed: 0x100,
            turn: 0,
            gap: 0,
            fx: None,
            equip_fx: None,
        }
    }

    /// C600's effect triangles of this update (empty outside a fusion).
    pub fn effects(&self) -> &[crate::duel_c621::FxTri] {
        match (&self.fx, &self.equip_fx) {
            (Some(f), _) => &f.tris,
            (None, Some(e)) => &e.tris,
            _ => &[],
        }
    }

    /// The equip module running this update (the handler's message box is
    /// open from its launch until the update after its end) [F].
    pub fn equip_running(&self) -> Option<u16> {
        match self.pairs.get(self.pair) {
            Some((_, Pair::Equip { module, .. })) if self.gap == 0 && self.u >= 1 => Some(*module),
            _ => None,
        }
    }

    /// The chain's pair kinds.
    pub fn pair_kinds(&self) -> impl Iterator<Item = Pair> + '_ {
        self.pairs.iter().map(|p| p.1)
    }

    fn module_length(&self) -> u32 {
        match self.pairs.get(self.pair).map(|p| p.1) {
            Some(Pair::Fuse { .. }) => 103,
            Some(Pair::Fail) => 28,
            Some(Pair::Equip { module, .. }) => crate::duel_equip::timing(module).map_or(28, |t| t.0),
            None => 0,
        }
    }

    pub fn done(&self) -> bool {
        self.pair >= self.pairs.len() && self.turn >= 9
    }

    /// One update.
    pub fn tick(&mut self) {
        if self.pair < self.pairs.len() {
            if self.gap > 0 {
                self.gap -= 1;
                if self.gap == 0 {
                    self.u = 0;
                }
                return;
            }
            self.u += 1;
            let (b_face, kind) = self.pairs[self.pair];
            let u = self.u;
            if let Pair::Fuse { .. } = kind {
                let fx = self.fx.get_or_insert_with(|| crate::duel_c600::C600::new(0x600 + self.pair as u32));
                fx.update(u);
            }
            if let Pair::Equip { module, .. } = kind {
                if u == 1 {
                    self.equip_fx = crate::duel_equip::EquipFx::new(module, 0x218 + self.pair as u32);
                }
                if let Some(fx) = &mut self.equip_fx {
                    fx.update(u);
                }
            }
            // 4-entry pose histories for the trails (one update of lag per
            // entry [H]).
            self.history.insert(0, self.b);
            self.history.truncate(4);
            self.history_a.insert(0, self.a);
            self.history_a.truncate(4);
            match kind {
                Pair::Fuse { result } => {
                    if (7..=16).contains(&u) {
                        let n = (17 - u) as f32;
                        self.a = [self.a[0] + (-201.0 - self.a[0]) / n, self.a[1] + (8.0 - self.a[1]) / n];
                        self.b = [self.b[0] + (-1.0 - self.b[0]) / n, self.b[1] + (8.0 - self.b[1]) / n];
                    } else if (18..=84).contains(&u) {
                        // The step, then the positions [emu u 29: A =
                        // view(-101 + 27.62, 8 + 77.2), r 82, a -19968].
                        self.angle -= self.speed;
                        self.speed = (self.speed + 0x100).min(0x1000);
                        self.radius = (self.radius - 1.5).max(0.0);
                        let t = f64::from(self.angle) * std::f64::consts::TAU / 65536.0;
                        let (x, y) = ((f64::from(self.radius) * t.cos()) as f32, (f64::from(self.radius) * t.sin()) as f32);
                        self.a = [-101.0 - x, 8.0 - y];
                        self.b = [-101.0 + x, 8.0 + y];
                    }
                    if u == 80 {
                        self.survivor = result;
                    }
                }
                Pair::Fail => {
                    if (9..=14).contains(&u) {
                        let n = (15 - u) as f32;
                        self.b = [self.b[0] + (-101.0 - self.b[0]) / n, self.b[1] + (8.0 - self.b[1]) / n];
                    }
                    if (13..=22).contains(&u) {
                        let n = (23 - u) as f32;
                        self.a = [self.a[0] + (-350.0 - self.a[0]) / n, self.a[1] + (-350.0 - self.a[1]) / n];
                    }
                    if u >= 13 {
                        self.a_spin = [self.a_spin[0] + 0x1000, self.a_spin[1], self.a_spin[2] + 0x1000];
                    }
                    if u == 28 {
                        self.survivor = b_face;
                    }
                }
                Pair::Equip { monster, .. } => {
                    // C538's slide without the spin; B stays on A [F, emu].
                    if (10..=15).contains(&u) {
                        let n = (16 - u) as f32;
                        self.b = [self.b[0] + (-101.0 - self.b[0]) / n, self.b[1] + (8.0 - self.b[1]) / n];
                    }
                    if u >= self.module_length() {
                        self.survivor = monster;
                    }
                }
            }
            if u >= self.module_length() {
                // The next pair pairs the survivor (at view (-101, 8)) with
                // the next pick; two bookkeeping updates between [emu].
                self.pair += 1;
                self.a = [-101.0, 8.0];
                self.b = [101.0, 8.0];
                self.a_spin = SPIN0;
                self.history.clear();
                self.history_a.clear();
                self.fx = None;
                self.equip_fx = None;
                (self.radius, self.angle, self.speed) = (100.0, 0, 0x100);
                self.gap = 2;
            }
            return;
        }
        if self.gap > 0 {
            self.gap -= 1;
            return;
        }
        self.turn += 1;
    }

    /// The handler's sound this update (bank 1): fusion 0x4C at d0 1, 0x3A
    /// at 18, 0x3B at 80; failed pair 0x4C at 1, 0x4A at 15 [F].
    pub fn sound(&self) -> Option<u8> {
        if self.gap > 0 {
            return None;
        }
        match (self.pairs.get(self.pair).map(|p| p.1), self.u) {
            (Some(_), 1) => Some(0x4C),
            (Some(Pair::Fuse { .. }), 18) => Some(0x3A),
            (Some(Pair::Fuse { .. }), 80) => Some(0x3B),
            (Some(Pair::Fail), 15) => Some(0x4A),
            // Equip handlers: the class sound at 15, then 0x3B at 16 for
            // 0x219 / 0x21D / 0x21E [F; frame mapping as the others, H].
            (Some(Pair::Equip { module, .. }), 15) => crate::duel_equip::class_sound(module),
            (Some(Pair::Equip { module: 0x219 | 0x21D | 0x21E, .. }), 16) => Some(0x3B),
            _ => None,
        }
    }

    /// The flash alpha (C600 updates 81..96: 0xFF down by 16), 0 when none.
    pub fn flash(&self) -> u8 {
        match self.pairs.get(self.pair) {
            Some((_, Pair::Fuse { .. })) if self.gap == 0 && (81..=96).contains(&self.u) => (0xFF - 16 * (self.u - 81)) as u8,
            _ => 0,
        }
    }

    /// The cards drawn this update (first on top).
    pub fn cards(&self) -> Vec<ShownCard> {
        let card = |face, view, spin, alpha| ShownCard { face, view, spin, alpha };
        if self.pair >= self.pairs.len() {
            // 0x4B: the survivor turns once while moving to the centre.
            // d46 += 3 per pass, two passes in the first update: k = 6, 9 .. 30.
            let k = if self.turn == 0 { 0 } else { (3 * (self.turn + 1)).min(30) };
            let c = (128 * k / 30) as f32 / 128.0;
            let view = [-101.0 + 101.0 * c, 8.0];
            let spin = [0, (0x10000 as f32 * c) as i32 & 0xFFFF, 0x10000];
            return vec![card(self.survivor, view, spin, 128)];
        }
        if self.gap > 0 {
            return vec![card(self.survivor, [-101.0, 8.0], SPIN0, 128)];
        }
        let (b_face, kind) = self.pairs[self.pair];
        let u = self.u;
        let mut out = Vec::new();
        match kind {
            Pair::Fuse { result } => {
                if u >= 81 {
                    out.push(card(result, [-101.0, 8.0], SPIN0, 128));
                } else {
                    out.push(card(self.survivor, self.a, SPIN0, 128));
                    out.push(card(b_face, self.b, SPIN0, 128));
                    for (j, (pa, pb)) in self.history_a.iter().zip(&self.history).skip(1).take(3).enumerate() {
                        out.push(card(self.survivor, *pa, SPIN0, [64, 42, 32][j]));
                        out.push(card(b_face, *pb, SPIN0, [64, 42, 32][j]));
                    }
                }
            }
            Pair::Fail => {
                out.push(card(b_face, self.b, SPIN0, 128));
                for (j, p) in self.history.iter().skip(1).take(3).enumerate() {
                    out.push(card(b_face, *p, SPIN0, [64, 42, 32][j]));
                }
                out.push(card(self.survivor, self.a, self.a_spin, 128));
            }
            Pair::Equip { module, .. } => {
                // A drawn first, then B (on top); B's trail while it holds
                // and slides (u <= 15) [F, emu].
                out.push(card(b_face, self.b, SPIN0, crate::duel_equip::b_alpha(module, u)));
                if u <= 15 {
                    for (j, p) in self.history.iter().skip(1).take(3).enumerate() {
                        out.push(card(b_face, *p, SPIN0, [64, 42, 32][j]));
                    }
                }
                out.push(card(self.survivor, [-101.0, 8.0], SPIN0, 128));
            }
        }
        out
    }
}

/// A card shown by the fusion presentation as a pose through `view`.
pub fn shown_pose(view: &crate::camera::Mat4, polar: [f64; 3], card: &ShownCard) -> Option<CardPose> {
    let m: roses_assets::opening_scene::Matrix = view.map(|r| r.map(|v| v as f32));
    let p = roses_assets::opening_scene::ee_view::view_to_world(&m, card.view[0], card.view[1], 800.0).ok()??;
    Some(CardPose {
        position: [p[0], p[1], p[2]],
        spin: card.spin,
        tilt: crate::duel_versus::camera_angles(polar),
        scale: [0.375, 0.3913],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_surviving_field_unit_lands_with_its_committed_flags() {
        assert_eq!(landing_pose(0, (3, 5), false, false).spin, [0x4000, 0x8000, 0]);
        assert_eq!(landing_pose(0, (3, 5), true, true).spin, [0x4000, 0x4000, 0x8000]);
        assert_eq!(landing_pose(1, (3, 1), true, false).spin, [0x4000, 0, 0x8000]);
        assert_eq!(landing_pose(1, (3, 1), false, true).spin, [0x4000, -0x4000, 0]);
    }

    #[test]
    fn flight_matches_the_captured_samples() {
        // r1: side 0 to (3, 5); from = the 0x46 pose (position solved from
        // the k = 2 sample), tilt -0x2000.
        let to = landing_pose(0, (3, 5), false, false);
        assert_eq!(to.position, [0.0, 0.0, 100.0]);
        let from = CardPose {
            position: [0.0, -389.92, -318.6],
            spin: [0, 0x10000, 0x10000],
            tilt: [-0x2000, 0, 0],
            scale: [0.375, 0.3913],
        };
        let k2 = pose_at(&from, &to, 2);
        assert_eq!((k2.spin, k2.tilt[0]), ([0x400, 0xF800, 0xF000], 0xE200));
        assert!((k2.scale[0] - 0.3578).abs() < 1e-3 && (k2.scale[1] - 0.3731).abs() < 1e-3);
        let k15 = pose_at(&from, &to, 15);
        assert_eq!((k15.spin, k15.tilt[0]), ([0x2000, 0xC000, 0x8000], 0xF000));
        assert!((k15.position[1] + 194.96).abs() < 0.1 && (k15.position[2] + 109.30).abs() < 0.1);
        let k30 = pose_at(&from, &to, 30);
        assert_eq!((k30.position, k30.spin, k30.tilt[0]), ([0.0, 0.0, 100.0], [0x4000, 0x8000, 0], 0));
    }

    #[test]
    fn held_pose_is_view_0_8_at_the_board_camera() {
        let cam = crate::duel_board::BoardCamera::snapped((3, 5));
        eprintln!("eye {:?}", cam.eye());
        let p = held_pose(&cam.view().unwrap(), cam.polar).unwrap();
        eprintln!("held {:?}", p);
        assert!((p.position[1] + 389.92).abs() < 1.0 && (p.position[2] + 318.6).abs() < 1.0, "{:?}", p.position);
        assert_eq!(p.tilt, [-0x2000, 0, 0]);
    }

    #[test]
    fn fusion_spiral_follows_the_captured_orbit() {
        let face = (1, 1, 0, 0);
        let mut f = FusionShow::new(face, vec![(face, Pair::Fuse { result: (2, 1, 0, 0) })]);
        for _ in 0..29 {
            f.tick();
        }
        // emu u 29: A = view(-101 + 27.62, 8 + 77.2).
        let a = f.cards()[0].view;
        assert!((a[0] + 73.38).abs() < 0.5 && (a[1] - 85.2).abs() < 0.5, "{a:?}");
        for _ in 29..81 {
            f.tick();
        }
        assert_eq!(f.flash(), 0xFF);
        assert_eq!(f.cards()[0].face.0, 2);
        while !f.done() {
            f.tick();
        }
        assert_eq!(f.cards()[0].view, [0.0, 8.0]);
    }
}
