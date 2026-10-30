//! The card-versus screen of the battle scene (C module 604): the attacker's
//! and the defender's cards fly from their board cells to the two halves of
//! the screen, then hold there as flat 256x368 faces.
//!
//! Traced from the original [F c604 init 0x1E0386C, motion 0x1E017F0, draw
//! 0x1E01A00 -> 0x1DB180 / 0x1DB530; emu footage RAM battle-pine/009-010 and
//! the PCSX2 GS frames dumps-battle-frames 118-138]:
//! - Each card object starts as a copy of its unit's board card object:
//!   position the cell centre (x, 0, z), spin angles (0x4000, the side's base
//!   yaw, 0x8000), tilt 0 and scale 0.1.
//! - Targets: position = the world point of view (-128 / +128, 0, 800)
//!   through the current (battle camera) view; spin (0, 0x10000, 0x10000);
//!   tilt = the camera angles (0x205580: -pitch, 0x4000 - yaw) minus the
//!   start tilt; scale 1.0; 20 updates (1 when the launch parameter's bit 18
//!   is set, not modelled).
//! - Every update each field moves by (target - current) / remaining
//!   (angles truncated to integers), until the card arrives; on arrival its
//!   spin is cleared. Moving cards are drawn as 3D quads through the
//!   battle camera (whose straight-down view is the degenerate matrix of
//!   `duel_board::straight_down_view`), with the back of the card when the
//!   quad faces away; once both have arrived they are drawn as screen
//!   sprites at x 0-256 / 256-512, y 8.875-503.0625.
use crate::camera::Mat4;
use crate::duel_board;

/// The c604 card quad (local, z = 0) and its texture corners: 256 x 368.
pub const CARD_HALF: [f64; 2] = [128.0, 184.0];
pub const CARD_CORNERS: [[f64; 2]; 4] = [
    [-128.0, 184.0],
    [-128.0, -184.0],
    [128.0, -184.0],
    [128.0, 184.0],
];
pub const CARD_UV: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 368.0], [256.0, 368.0], [256.0, 0.0]];
/// Fly-in length (c604 +0x1378 / +0x13C8).
pub const FLY_UPDATES: i32 = 20;
/// View-space x of the two cards' targets (c604 +0x1370 / +0x13C0) and the
/// projection distance (camera +0x44).
pub const TARGET_X: [f32; 2] = [-128.0, 128.0];
pub const PROJECTION_DISTANCE: f32 = 800.0;
/// The resting sprites (GS dump dumps-battle-versus draws 119 / 102): x
/// from 256 + sx - 128, y the projected 368-texel height at depth 800.
pub const REST_Y: [f32; 2] = [8.875, 503.0625];

/// One c604 card object: the fields 0x1E017F0 moves (+0x00 position,
/// +0x10 spin, +0x20 tilt, +0x30 scale).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardPose {
    pub position: [f32; 3],
    pub spin: [i32; 3],
    pub tilt: [i32; 3],
    pub scale: [f32; 2],
}

/// A versus card: what it shows and how it moves.
#[derive(Clone, Debug, PartialEq)]
pub struct VersusCard {
    pub card: u16,
    pub level: u8,
    pub attack: i32,
    pub defence: i32,
    pub pose: CardPose,
    pub target: CardPose,
    /// Updates left (target +0x48); 0 = arrived.
    pub remaining: i32,
}

/// The board card object of a unit on `cell` of `side` [emu footage 009:
/// both cards' fields at k = 11 interpolate back to these]; a card in
/// defence lies sideways, rot y 0x4000 less [emu FLIP-AND-VICTIMS.md 1.3].
pub fn board_card_pose(side: u8, cell: (u8, u8), defence: bool) -> CardPose {
    let [x, z] = duel_board::cell_center(cell.0, cell.1);
    let y = crate::duel_play::side_model_base(side) - if defence { 0x4000 } else { 0 };
    CardPose {
        position: [x as f32, 0.0, z as f32],
        spin: [0x4000, y & 0xFFFF, 0x8000],
        tilt: [0, 0, 0],
        scale: [0.1, 0.1],
    }
}

/// 0x1FC1C0: the camera's angles (x = -pitch, y = 0x4000 - yaw) from its
/// float polar (radians), trunc(32768 * a / pi) in f32.
pub fn camera_angles(polar: [f64; 3]) -> [i32; 3] {
    let pi = std::f32::consts::PI;
    let a = |v: f64| (32768.0f32 * v as f32 / pi) as i32;
    [-a(polar[2]), 0x4000 - a(polar[1]), 0]
}

impl VersusCard {
    /// c604 init for card `index` (0 attacker, left; 1 defender, right).
    pub fn new(
        index: usize,
        start: CardPose,
        view: &roses_assets::opening_scene::Matrix,
        polar: [f64; 3],
        face: (u16, u8, i32, i32),
    ) -> Option<Self> {
        let p = roses_assets::opening_scene::ee_view::view_to_world(
            view,
            TARGET_X[index],
            0.0,
            PROJECTION_DISTANCE,
        )
        .ok()??;
        let cam = camera_angles(polar);
        Some(Self {
            card: face.0,
            level: face.1,
            attack: face.2,
            defence: face.3,
            pose: start,
            target: CardPose {
                position: [p[0], p[1], p[2]],
                spin: [0, 0x10000, 0x10000],
                tilt: std::array::from_fn(|i| cam[i].wrapping_sub(start.tilt[i])),
                scale: [1.0, 1.0],
            },
            remaining: FLY_UPDATES,
        })
    }

    /// One update of 0x1E017F0: returns true while the card still moves.
    pub fn step(&mut self) -> bool {
        if self.remaining <= 0 {
            return false;
        }
        // EE arithmetic as 0x1E017F0 runs it (PCSX2 rounding).
        use roses_assets::opening_scene::ee_float::ee;
        let Ok(f) = ee::from_i32(self.remaining).and_then(|n| ee::div(1.0, n)) else {
            return false;
        };
        let lerp = |c: f32, t: f32| -> f32 {
            ee::sub(t, c)
                .and_then(|d| ee::mul(f, d))
                .and_then(|d| ee::add(c, d))
                .unwrap_or(t)
        };
        let turn = |c: i32, t: i32| -> i32 {
            let d = ee::from_i32(t.wrapping_sub(c))
                .and_then(|d| ee::mul(f, d))
                .and_then(ee::to_i32)
                .unwrap_or(0);
            c.wrapping_add(d)
        };
        let (c, t) = (&mut self.pose, &self.target);
        for i in 0..2 {
            c.scale[i] = lerp(c.scale[i], t.scale[i]);
        }
        for i in 0..3 {
            c.spin[i] = turn(c.spin[i], t.spin[i]);
        }
        for i in 0..3 {
            c.tilt[i] = turn(c.tilt[i], t.tilt[i]);
        }
        for i in 0..3 {
            c.position[i] = lerp(c.position[i], t.position[i]);
        }
        self.remaining -= 1;
        if self.remaining == 0 {
            // Arrival (c604 0x1E043A4): the spin is cleared.
            self.pose.spin = [0, 0, 0];
            return false;
        }
        true
    }

    /// The card's model-to-world matrix (0x1DB180 with flag 0x20 set):
    /// S Rxz(spin x, z) Ry(spin y) (R(tilt) + T(position)) in the
    /// executable's row-vector order, as a column-major [`Mat4`].
    pub fn world(&self) -> Mat4 {
        let p = &self.pose;
        let mut a = rotation(p.tilt);
        a[3] = [
            f64::from(p.position[0]),
            f64::from(p.position[1]),
            f64::from(p.position[2]),
            1.0,
        ];
        let ry = rotation([0, p.spin[1], 0]);
        let rxz = rotation([p.spin[0], 0, p.spin[2]]);
        let s = duel_board::scale([f64::from(p.scale[0]), f64::from(p.scale[1]), 1.0]);
        crate::camera::mul(&crate::camera::mul(&a, &ry), &crate::camera::mul(&rxz, &s))
    }

    /// Whether the front shows (0x1DB180's test: the quad's view-space
    /// normal against the direction from its centroid to the eye).
    pub fn front_facing(&self, view: &Mat4) -> bool {
        let m = crate::camera::mul(view, &self.world());
        let apply = |p: [f64; 4]| -> [f64; 4] {
            std::array::from_fn(|r| {
                m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r] * p[3]
            })
        };
        let mut c = [0.0f64; 3];
        for [x, y] in CARD_CORNERS {
            let v = apply([x, y, 0.0, 1.0]);
            for i in 0..3 {
                c[i] -= v[i] / 4.0;
            }
        }
        let n = [m[2][0], m[2][1], m[2][2]];
        n[0] * c[0] + n[1] * c[1] + n[2] * c[2] > 0.0
    }
}

/// 0x20C0B0's rotation from three 16-bit angles (rows as stored; the
/// executable interpolates a 1025-entry sine table, this uses f64 sin/cos,
/// within 1e-5 of it).
pub fn rotation(r: [i32; 3]) -> Mat4 {
    let a = |v: i32| f64::from(v) * std::f64::consts::TAU / 65536.0;
    let (sx, cx) = a(r[0]).sin_cos();
    let (sy, cy) = a(r[1]).sin_cos();
    let (sz, cz) = a(r[2]).sin_cos();
    [
        [cy * cz, cy * sz, -sy, 0.0],
        [sx * sy * cz - cx * sz, sx * sy * sz + cx * cz, sx * cy, 0.0],
        [cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cx * cy, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// The damage number c604 shows on a card in the battle's return
/// [F 0x1E01B10 / 0x1E01D10; emu dumps-battle-damage frames 104-121]: a
/// sign glyph (minus for LP lost) and 1-4 digits from TEXEFF record 15 (10 x
/// 16 texels at V = 8 + 16 * glyph, minus = glyph 10, plus = 11), each
/// drawn 30 x 48 and centred on the card (x0 = cx - n * 15, y 232-280);
/// vertex colour (176, 80, 96); alpha 128 for 6 updates, then 8 less per
/// update down to 32, gone after 18 updates. The GS blend writes Cs * As
/// (the glyphs darken as they fade).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageNumber {
    /// The versus card it shows on (0 left, 1 right).
    pub card: usize,
    /// The LP change (negative = damage).
    pub value: i32,
    /// Updates since it appeared.
    pub frame: u32,
}

/// Updates the damage number shows.
pub const DAMAGE_NUMBER_UPDATES: u32 = 18;
pub const DAMAGE_COLOUR: [u8; 3] = [176, 80, 96];
pub const GLYPH_TEXELS: [f32; 2] = [10.0, 16.0];
pub const GLYPH_SCREEN: [f32; 2] = [30.0, 48.0];

impl DamageNumber {
    /// Vertex alpha this update (`None` once gone).
    pub fn alpha(&self) -> Option<u8> {
        match self.frame {
            0..=5 => Some(128),
            f if f < DAMAGE_NUMBER_UPDATES => Some((128 - 8 * (f - 5)) as u8),
            _ => None,
        }
    }

    /// Glyph indices, sign first (0x1E01B10: plus 11 or minus 10, then the
    /// digits of |value| without leading zeros, at most four).
    pub fn glyphs(&self) -> Vec<u8> {
        let v = self.value.unsigned_abs().min(9999);
        let mut out = vec![if self.value < 0 { 10 } else { 11 }];
        out.extend(v.to_string().bytes().map(|b| b - b'0'));
        out
    }

    /// Screen quads (x0, y0, x1, y1) and the texel row of each glyph.
    pub fn quads(&self) -> Vec<([f32; 4], f32)> {
        let glyphs = self.glyphs();
        let cx = 256 + TARGET_X[self.card] as i32;
        let n = glyphs.len() as i32;
        let w = GLYPH_SCREEN[0] as i32;
        // 0x1E01D10: x0 = cx - n * w / 2 (integer halving).
        let x0 = cx - (n * w) / 2;
        let y0 = 256.0 - GLYPH_SCREEN[1] / 2.0;
        glyphs
            .iter()
            .enumerate()
            .map(|(i, &g)| {
                let x = (x0 + i as i32 * w) as f32;
                (
                    [x, y0, x + GLYPH_SCREEN[0], y0 + GLYPH_SCREEN[1]],
                    8.0 + 16.0 * f32::from(g),
                )
            })
            .collect()
    }
}

/// The arena as far as the port plays it: the crossfade from the versus
/// cards (phase 4: d3a and d3b from 128 down by 4 per update over 32
/// updates) and the terrain's StageCam orbit (frames - 1 updates) after it
/// [F 0x1D00F0 phases 3-5; emu dumps-battle-frames 209-360].
#[derive(Clone, Debug, PartialEq)]
pub struct ArenaShot {
    pub terrain: u8,
    /// Updates since the crossfade started.
    pub frame: u32,
    /// Phases 5-6 (the attack and the fade out) once the orbit has ended.
    pub battle: Option<crate::duel_arena_battle::ArenaBattle>,
    /// The attack effect's primitives of this update (BMODULE run live).
    pub effect: Option<crate::duel_bmodule::EffectDraw>,
}

/// Crossfade length (32 updates of -4).
pub const ARENA_FADE_UPDATES: u32 = 32;
/// The orbit's track length (Forest: 120 frames -> 119 updates).
pub const ARENA_ORBIT_UPDATES: u32 = 119;

impl ArenaShot {
    /// d3a: the subtractive black over the arena (128 = black); phase 6
    /// raises it again.
    pub fn black(&self) -> u8 {
        if let Some(b) = &self.battle {
            // Phase 8 shows the board under the cards; the port keeps the
            // canvas black there (the cards cover all but 8-pixel bands) [H].
            return if b.returning() { 128 } else { b.fade };
        }
        128u32.saturating_sub(4 * (self.frame + 1).min(ARENA_FADE_UPDATES)) as u8
    }
    /// The arena camera: the attack's, else the stage path's at the orbit.
    pub fn camera(&self, stage: &crate::duel_arena::Cmot) -> crate::duel_arena::CmotCamera {
        match &self.battle {
            Some(b) => b.camera.view(),
            None => stage.at(self.orbit(), stage.frames.saturating_sub(1)),
        }
    }
    /// Whether the orbit (phase 5 sub 0) has ended.
    pub fn orbit_done(&self) -> bool {
        self.frame >= ARENA_FADE_UPDATES + ARENA_ORBIT_UPDATES
    }
    /// d3b: the versus cards' alpha over the arena.
    pub fn card_alpha(&self) -> u8 {
        self.black()
    }
    /// The orbit's elapsed updates (0 during the crossfade).
    pub fn orbit(&self) -> u32 {
        self.frame.saturating_sub(ARENA_FADE_UPDATES).min(ARENA_ORBIT_UPDATES)
    }
}

/// The versus screen: two cards that fly in, then rest.
#[derive(Clone, Debug, PartialEq)]
pub struct Versus {
    pub cards: [VersusCard; 2],
    /// Updates since the fly-in started.
    pub frame: u32,
    /// The battle's return (flash passes and the damage number).
    pub ret: Option<crate::duel_return::Return>,
    /// The scene's end (c605: burn and return to the board).
    pub end: Option<crate::duel_burn::Return605>,
    /// The arena (battle state 0x39 phases 4-5) while it shows.
    pub arena: Option<ArenaShot>,
    /// A leader hit (event 0x5D): only the attacker's card (card 0) shows;
    /// card 1 is a placeholder for the leader, which has no card face
    /// [F/emu LEADER-BATTLE.md].
    pub single: bool,
}

impl Versus {
    /// Both cards arrived: drawn as screen sprites.
    pub fn resting(&self) -> bool {
        self.cards.iter().all(|c| c.remaining <= 0) && self.frame > FLY_UPDATES as u32
    }

    /// The cards drawn (the leader placeholder is not).
    pub fn shown(&self) -> usize {
        if self.single { 1 } else { 2 }
    }

    /// One update: move the cards that still fly.
    pub fn tick(&mut self) {
        self.tick_with(None);
    }

    /// [`tick`](Self::tick) with the arena battle's attack effects run live
    /// on `effects` (their returns drive the battle; their draws are kept).
    pub fn tick_with(&mut self, effects: Option<&mut crate::duel_bmodule::EffectWorker>) {
        self.frame += 1;
        for card in &mut self.cards {
            card.step();
        }
        if let Some(r) = &mut self.ret {
            r.tick();
        }
        if let Some(e) = &mut self.end {
            e.tick();
        }
        if let Some(a) = &mut self.arena {
            a.frame += 1;
            if let Some(b) = &mut a.battle {
                a.effect = match effects {
                    Some(w) => w.tick(b, crate::duel_bmodule::wait()),
                    None => {
                        b.tick();
                        None
                    }
                };
            }
        }
    }

    /// Screen rectangle (x0, y0, x1, y1) of a resting card.
    pub fn rest_rect(index: usize) -> [f32; 4] {
        let x0 = 256.0 + TARGET_X[index] - 128.0;
        [x0, REST_Y[0], x0 + 256.0, REST_Y[1]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn footage_cards() -> [VersusCard; 2] {
        // battle-pine 010: battle camera target (0, 40, 25), yaw pi; the
        // attacker on (0, 0) world, the defender on (0, 50) world.
        let view = roses_assets::opening_scene::ee_view::view_from_direction(
            [0.0, 190.0, 25.0],
            [0.0, -1.0, 0.0],
        )
        .unwrap()
        .unwrap();
        let polar = [150.0, std::f64::consts::PI, std::f64::consts::FRAC_PI_2];
        let start = |side: u8, z: f32| CardPose {
            position: [0.0, 0.0, z],
            spin: [0x4000, if side == 0 { 0x8000 } else { 0 }, 0x8000],
            tilt: [0; 3],
            scale: [0.1, 0.1],
        };
        [
            VersusCard::new(0, start(1, 0.0), &view, polar, (0, 0, 0, 0)).unwrap(),
            VersusCard::new(1, start(0, 50.0), &view, polar, (0, 0, 0, 0)).unwrap(),
        ]
    }

    /// Prints the footage battle's projected card corners per update (GS
    /// window coordinates) and the face shown, for overlaying on
    /// dumps-battle-frames 118-137: `cargo test --lib footage_corners --
    /// --ignored --nocapture`.
    #[test]
    #[ignore = "prints research data"]
    fn footage_corners() {
        let view = roses_assets::opening_scene::ee_view::view_from_direction(
            [0.0, 190.0, 25.0],
            [0.0, -1.0, 0.0],
        )
        .unwrap()
        .unwrap()
        .map(|r| r.map(f64::from));
        let mut cards = footage_cards();
        for k in 1..=20 {
            let mut line = Vec::new();
            for card in &mut cards {
                card.step();
                let w = card.world();
                let corners: Vec<[f64; 2]> = CARD_CORNERS
                    .iter()
                    .map(|[x, y]| {
                        let p: [f64; 3] =
                            std::array::from_fn(|r| w[0][r] * x + w[1][r] * y + w[3][r]);
                        let g = crate::duel_board::gs_screen(&view, p);
                        [g[0], g[1]]
                    })
                    .collect();
                line.push(format!(
                    "{{\"front\": {}, \"corners\": {:?}}}",
                    card.front_facing(&view),
                    corners
                ));
            }
            println!("{{\"k\": {k}, \"cards\": [{}]}}", line.join(", "));
        }
    }

    #[test]
    fn damage_number_matches_the_footage_draws() {
        let mut d = DamageNumber {
            card: 0,
            value: -600,
            frame: 0,
        };
        assert_eq!(d.glyphs(), vec![10, 6, 0, 0]);
        let q = d.quads();
        // dumps-battle-damage draw 2192: minus 68-98, 6 98-128, 0 128-158
        // and 158-188, y 232-280; V 168, 104, 8, 8.
        assert_eq!(q[0], ([68.0, 232.0, 98.0, 280.0], 168.0));
        assert_eq!(q[1], ([98.0, 232.0, 128.0, 280.0], 104.0));
        assert_eq!(q[3], ([158.0, 232.0, 188.0, 280.0], 8.0));
        let alphas: Vec<Option<u8>> = (0..19)
            .map(|f| {
                d.frame = f;
                d.alpha()
            })
            .collect();
        assert_eq!(alphas[5], Some(128));
        assert_eq!(alphas[6], Some(120));
        assert_eq!(alphas[17], Some(32));
        assert_eq!(alphas[18], None);
    }

    #[test]
    fn fly_in_matches_the_footage_fields_at_eleven_updates() {
        let [mut a, mut b] = footage_cards();
        for _ in 0..11 {
            a.step();
            b.step();
        }
        // battle-pine/009 (remaining 9).
        assert_eq!(a.remaining, 9);
        assert_eq!(a.pose.spin, [7375, 36043, 50786]);
        assert_eq!(b.pose.spin, [7375, 50786, 50786]);
        assert_eq!(a.pose.tilt, [-9009, -9009, 0]);
        let close = |x: [f32; 3], y: [f32; 3]| (0..3).all(|i| (x[i] - y[i]).abs() < 0.01);
        assert!(
            close(a.pose.position, [35.2, -404.25, -34.65]),
            "{:?}",
            a.pose.position
        );
        assert!(
            close(b.pose.position, [-35.2, -474.65, 58.25]),
            "{:?}",
            b.pose.position
        );
        assert!((a.pose.scale[0] - 0.595).abs() < 1e-3);
        for _ in 0..9 {
            a.step();
            b.step();
        }
        // battle-pine/010: at rest.
        assert_eq!(a.pose.position, [64.0, -735.0, -63.0]);
        assert_eq!(b.pose.position, [-64.0, -863.0, 65.0]);
        assert_eq!(a.pose.spin, [0, 0, 0]);
        assert_eq!(a.pose.tilt, [-16384, -16384, 0]);
        assert_eq!(a.pose.scale, [1.0, 1.0]);
        assert_eq!(Versus::rest_rect(0), [0.0, 8.875, 256.0, 503.0625]);
        assert_eq!(Versus::rest_rect(1), [256.0, 8.875, 512.0, 503.0625]);
    }
}
