//! Duel board cell and unit layers (no GPU code): the per-frame input a game
//! loop fills from the rules engine, the overlay animation state the original
//! keeps per cell (pulses, easing, mode latch) and the geometry and colours to
//! draw. Sources are PAL EE addresses from the private board research
//! (`board-spec.json` "overlays" and "units", GS dump draws 6865-6871 and
//! 6990-6996); [H] marks inferred rules.
//!
//! - Fill: quad +-24 at y = -0.2, colour = current fill RGB * brightness/128,
//!   additive Cd + Cs*As/128, Z GEQUAL, no Z write (0x1DD7D0 -> 0x1DCFC0).
//! - Outline: 3 antialiased LINE_STRIP loops (24, 23.8, 23.6) at y = -0.1,
//!   additive, Z GEQUAL, no Z write; unit side colour, bit1 pulses with P2.
//! - Easing (0x1E44B0): every update each channel moves <= 8 toward its target;
//!   a cell whose overlay mode changes first fades its fill alpha to 0.
//! - P1 (u16 0x2DB826) +-0x200 per update, P2 (0x2DB828) +-0x800, reflecting
//!   at 0 and 0x8000; cursor alpha = 0x40 + (P1 >> 8).
//! - Colour sets (0x1AEF30) and states (u8 0x2DB8F7): 0x16/0x18 cursor
//!   row/column cross; 0x1D grey board, reachable yellow, path magenta/teal;
//!   0x3E grey board, reachable blue; 0x21 cursor only.
//! - Cursor tint pass (6867-6871): the cursor cell is redrawn into 0x2000,
//!   a full-screen sprite adds colour * (alpha * brightness >> 7) / 128 and
//!   the result replaces the cell in the frame.
//! - Face-down card: TEXSYS #2 card back (U 256-512, V 0-368), 25.6 x 36.8 at
//!   y = 0, side 0 rotated 180 degrees, defence 90 degrees [H: direction],
//!   RGB (acted ? 64 : 128) * g, normal alpha blend, Z write.
//! - Leader emblem: TEXSYS #5 half by side, 33.33 square at y = 0, same
//!   brightness rule, side-colour outline (antialiased line strip, Z write).
//!
//! Face-up cards keep the same quad with their own card image when the input
//! carries its values (`UnitInput::face`, duel play); otherwise the quad is
//! reported through `UnitLayers::unsupported`. 3D unit models are drawn by
//! the renderer's own layer when a models import is given and are reported
//! here (never replaced by other art).
use crate::camera::Mat4;
use crate::duel_board::{GS_PROJECTION, cell_center};
use serde::Deserialize;

pub const CELL_COUNT: usize = 49;
pub const FILL_Y: f64 = -0.2;
pub const OUTLINE_Y: f64 = -0.1;
pub const UNIT_Y: f64 = 0.0;
pub const FILL_HALF: f64 = 24.0;
pub const OUTLINE_HALVES: [f64; 3] = [24.0, 23.8, 23.6];
pub const EMBLEM_HALF: f64 = 50.0 / 3.0;
pub const CARD_HALF: [f64; 2] = [12.8, 18.4];
/// Card back inside TEXSYS #2: texels x 256..512, y 0..368.
pub const CARD_BACK_ORIGIN: [u32; 2] = [256, 0];
pub const CARD_IMAGE: [u32; 2] = [256, 368];
pub const P1_STEP: u16 = 0x200;
pub const P2_STEP: u16 = 0x800;
pub const PULSE_TOP: u16 = 0x8000;
pub const EASE_STEP: u8 = 8;

// ---------------------------------------------------------------- input
/// One unit as the rules engine sees it (unit record 0x2DB990 + side*0x3F4 +
/// slot*0x5C: card +0x3C, cell +0x40/+0x41, flags +0x57).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UnitInput {
    pub side: u8,
    /// 0 = leader, 1..=10 other units.
    pub slot: u8,
    pub cell: [u8; 2],
    pub card: u16,
    #[serde(default)]
    pub face_up: bool,
    /// Flags +0x57 bit3.
    #[serde(default)]
    pub defence: bool,
    /// Flags +0x57 bit0: done this turn, drawn at brightness 64.
    #[serde(default)]
    pub acted: bool,
    /// Flags +0x57 bit1: outline pulses red/white with P2.
    #[serde(default)]
    pub outline_pulse: bool,
    /// What the face-up card image shows (duel play; `None` elsewhere):
    /// without it a face-up card's quad is reported, not drawn.
    #[serde(default)]
    pub face: Option<BoardFace>,
    /// Drawn offset (+0x34/+0x36, world x/z) of a unit walking to `cell`.
    #[serde(default)]
    pub offset: [f32; 2],
    /// Yaw offset of the model (record +0x30, 65536 per turn): a unit turned
    /// to face its battle opponent.
    #[serde(default)]
    pub turn: i32,
    /// A reveal flip in progress: +0x54 (0x20 per update to 0xFF; the card
    /// turns about its long axis, Z = trunc(p54 * 0x8000 / 255)) [F/emu
    /// FLIP-AND-VICTIMS.md 1].
    #[serde(default)]
    pub flip: Option<u8>,
    /// The card object's alpha +0x43 when not 128 (c602 param 1 fades the
    /// activated card by 4 per update).
    #[serde(default)]
    pub alpha: Option<u8>,
    /// +0x55 while the card turns between upright and sideways (defence):
    /// rot y = base - trunc(p55 * 0x4000 / 255), 0x20 per update [F/emu
    /// FLIP-AND-VICTIMS.md 1.1]; `None` at rest.
    #[serde(default)]
    pub sideways: Option<u8>,
    /// A module turning and shrinking the card (c560): yaw added to its rot
    /// y (0x10000 per turn) and the factor of its (x, y) scale.
    #[serde(default)]
    pub spin: Option<(i32, f32)>,
    /// The model's scale set by a module (c560), when not 1.
    #[serde(default)]
    pub model_scale: Option<f32>,
    /// A leader's emblem scale d34 (0..=128) while the duel's opening grows
    /// it (0x1DDA00: half size 16.67·d34/128); `None` = full.
    #[serde(default)]
    pub emblem: Option<u8>,
}

/// The values a face-up board card's image shows (0x1CEF60 -> 0x1DCEB0
/// with a field record, large mode): stars = cost 0x1E1BD0, ATK/DEF =
/// 0x1E1570 / 0x1E1650 plus the terrain bonus 0x267A00 at the unit's cell
/// (the cursor cell for the unit a move selection holds), effect frame when
/// 0x1E1C60 is non-zero [emu: 44 compositions in 27 savestates].
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoardFace {
    pub level: u8,
    pub attack: i32,
    pub defence: i32,
    pub effect: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PulseStart {
    pub p1: u16,
    pub p1_rising: bool,
    pub p2: u16,
    pub p2_rising: bool,
}

fn default_state() -> u8 {
    0x18
}
fn default_brightness() -> u8 {
    128
}

/// Per-frame overlay input. The cursor itself comes from the board camera.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OverlayInput {
    /// Duel state u8 0x2DB8F7.
    #[serde(default = "default_state")]
    pub state: u8,
    /// Active side u8 0x2DB8F4.
    #[serde(default)]
    pub active_side: u8,
    /// 49 reach distances (0x1E3760 map, row-major); 0xFF = unreachable.
    /// Empty = nothing reachable.
    #[serde(default)]
    pub marks: Vec<u8>,
    /// Move allowance u8 0x2DB8FB (yellow alpha 0xD0 within it, else 0x40).
    #[serde(default)]
    pub move_allowance: u8,
    /// Path cells (0x2DB8FA entries), drawn in colour set C in state 0x1D.
    #[serde(default)]
    pub path: Vec<[u8; 2]>,
    /// Cell +0x1A and card/emblem brightness g (128 = 1.0).
    #[serde(default = "default_brightness")]
    pub brightness: u8,
    #[serde(default)]
    pub units: Vec<UnitInput>,
    /// Pulse phase at start (default: both 0, rising).
    #[serde(default)]
    pub pulse: Option<PulseStart>,
    /// Start with every current colour at its target (a settled board).
    #[serde(default)]
    pub settled: bool,
    /// No cell fills or outlines (the duel's opening states draw none
    /// [PCSX2 modes/older/intro]).
    #[serde(default)]
    pub hidden_cells: bool,
}

impl OverlayInput {
    pub fn validate(&self) -> Result<(), String> {
        if !(self.marks.is_empty() || self.marks.len() == CELL_COUNT)
            || self.active_side > 1
            || self.brightness > 128
            || self.path.iter().any(|c| c[0] > 6 || c[1] > 6)
        {
            return Err("Overlay input: marks must be 0 or 49 cells, side 0/1, brightness <= 128, path cells 0..6".into());
        }
        if self.units.len() > 22 {
            return Err("Overlay input: at most 22 units".into());
        }
        for (i, u) in self.units.iter().enumerate() {
            if u.side > 1 || u.slot > 10 || u.cell[0] > 6 || u.cell[1] > 6 {
                return Err(format!(
                    "Overlay input: unit {i} has an invalid side, slot or cell"
                ));
            }
            if self.units[..i]
                .iter()
                .any(|v| v.side == u.side && v.slot == u.slot)
            {
                return Err(format!(
                    "Overlay input: unit {i} repeats side {} slot {}",
                    u.side, u.slot
                ));
            }
        }
        if self
            .pulse
            .is_some_and(|p| p.p1 > PULSE_TOP || p.p2 > PULSE_TOP)
        {
            return Err("Overlay input: pulses are 0..=0x8000".into());
        }
        Ok(())
    }
    fn unit_at(&self, cell: (u8, u8)) -> Option<&UnitInput> {
        self.units.iter().find(|u| (u.cell[0], u.cell[1]) == cell)
    }
}

// ---------------------------------------------------------------- colours
/// Colour set entry (0x1AEF30): RGB and, for D and F, a fixed alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourSet {
    A,
    B,
    C,
    D,
    E,
    F,
}
pub fn colour(set: ColourSet, side: u8) -> [u8; 3] {
    let own = side == 0;
    match set {
        ColourSet::A if own => [128, 0, 0],
        ColourSet::A => [96, 96, 96],
        ColourSet::B if own => [96, 96, 96],
        ColourSet::B => [128, 0, 0],
        ColourSet::C if own => [128, 0, 96],
        ColourSet::C => [0, 128, 96],
        ColourSet::D => [0, 0, 128],
        ColourSet::E => [128, 128, 0],
        ColourSet::F => [128, 128, 128],
    }
}
pub fn side_outline(side: u8) -> [u8; 3] {
    if side == 0 {
        [255, 0, 0]
    } else {
        [255, 255, 255]
    }
}

/// Overlay family of a state; the per-cell mode is the family of its fill
/// (0 = no fill). Numbering is ours [H]; the round-2 cells show 1 for the cross.
pub fn family(state: u8) -> u8 {
    match state {
        0x16 | 0x18 => 1,
        0x1D => 2,
        0x3E => 3,
        _ => 4,
    }
}

// ---------------------------------------------------------------- state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pulse {
    pub value: u16,
    pub rising: bool,
    pub step: u16,
}
impl Pulse {
    pub fn update(&mut self) {
        if self.rising {
            self.value = (self.value + self.step).min(PULSE_TOP);
            if self.value == PULSE_TOP {
                self.rising = false;
            }
        } else {
            self.value = self.value.saturating_sub(self.step);
            if self.value == 0 {
                self.rising = true;
            }
        }
    }
    /// High byte (0..=128).
    pub fn level(&self) -> u8 {
        (self.value >> 8) as u8
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellOverlay {
    /// RGBA, alpha 128 = 1.0.
    pub fill: [u8; 4],
    pub outline: [u8; 4],
    pub mode: u8,
    pub latched: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OverlayState {
    /// The duel's d63 was set at the update's start: P1 steps rising (the
    /// glow sits at 0x8000) [F 0x1D081C].
    pub glow_hold: bool,
    pub p1: Pulse,
    pub p2: Pulse,
    pub cells: [CellOverlay; CELL_COUNT],
}

pub struct Targets {
    pub fill: [[u8; 4]; CELL_COUNT],
    pub mode: [u8; CELL_COUNT],
    pub outline: [[u8; 4]; CELL_COUNT],
}

fn step_toward(current: u8, target: u8) -> u8 {
    if current < target {
        current.saturating_add(EASE_STEP).min(target)
    } else {
        current.saturating_sub(EASE_STEP).max(target)
    }
}

impl OverlayState {
    pub fn new(input: &OverlayInput, cursor: (u8, u8)) -> Self {
        let start = input.pulse.unwrap_or(PulseStart {
            p1: 0,
            p1_rising: true,
            p2: 0,
            p2_rising: true,
        });
        let mut state = Self {
            glow_hold: false,
            p1: Pulse {
                value: start.p1,
                rising: start.p1_rising,
                step: P1_STEP,
            },
            p2: Pulse {
                value: start.p2,
                rising: start.p2_rising,
                step: P2_STEP,
            },
            cells: [CellOverlay::default(); CELL_COUNT],
        };
        if input.settled {
            let t = state.targets(input, cursor);
            for (i, cell) in state.cells.iter_mut().enumerate() {
                cell.fill = t.fill[i];
                cell.outline = t.outline[i];
                cell.mode = t.mode[i];
                cell.latched = t.mode[i];
            }
        }
        state
    }

    pub fn targets(&self, input: &OverlayInput, cursor: (u8, u8)) -> Targets {
        if input.hidden_cells {
            return Targets {
                fill: [[0; 4]; CELL_COUNT],
                mode: [0; CELL_COUNT],
                outline: [[0; 4]; CELL_COUNT],
            };
        }
        let side = input.active_side;
        let fam = family(input.state);
        let mut fill = [[0u8; 4]; CELL_COUNT];
        let mut mode = [0u8; CELL_COUNT];
        let set = |rgb: [u8; 3], a: u8| [rgb[0], rgb[1], rgb[2], a];
        for (i, (f, m)) in fill.iter_mut().zip(mode.iter_mut()).enumerate() {
            let (c, r) = ((i % 7) as u8, (i / 7) as u8);
            let reach = input.marks.get(i).copied().unwrap_or(0xff);
            let target = match fam {
                1 if c == cursor.0 || r == cursor.1 => Some(set(colour(ColourSet::A, side), 0x40)),
                2 if input.path.contains(&[c, r]) => Some(set(colour(ColourSet::C, side), 0xd0)),
                2 if reach != 0xff => {
                    let a = if reach <= input.move_allowance {
                        0xd0
                    } else {
                        0x40
                    };
                    Some(set(colour(ColourSet::E, side), a))
                }
                2 | 3 if reach == 0xff || fam == 2 => Some(set(colour(ColourSet::F, side), 32)),
                3 => Some(set(colour(ColourSet::D, side), 128)),
                _ => None,
            };
            if let Some(t) = target {
                *f = t;
                *m = fam;
            }
        }
        // The cursor cell: A, B under an enemy unit, C from state 0x1D on.
        let ci = usize::from(cursor.1) * 7 + usize::from(cursor.0);
        let enemy = input
            .unit_at(cursor)
            .is_some_and(|u| u.side != input.active_side);
        let set_id = if input.state >= 0x1d {
            ColourSet::C
        } else if enemy {
            ColourSet::B
        } else {
            ColourSet::A
        };
        fill[ci] = set(colour(set_id, side), 0x40 + self.p1.level());
        mode[ci] = fam;
        let mut outline = [[0u8; 4]; CELL_COUNT];
        for u in &input.units {
            let i = usize::from(u.cell[1]) * 7 + usize::from(u.cell[0]);
            let rgb = if u.outline_pulse {
                let gb = ((128 - u32::from(self.p2.level().min(128))) * 255 / 128) as u8;
                [255, gb, gb]
            } else {
                side_outline(u.side)
            };
            outline[i] = set(rgb, 128);
        }
        Targets {
            fill,
            mode,
            outline,
        }
    }

    /// One original update (two PAL fields, 25 Hz): pulses, then easing toward targets.
    pub fn update(&mut self, input: &OverlayInput, cursor: (u8, u8)) {
        if self.glow_hold {
            self.p1.rising = true;
        }
        self.p1.update();
        self.p2.update();
        let t = self.targets(input, cursor);
        for (i, cell) in self.cells.iter_mut().enumerate() {
            cell.mode = t.mode[i];
            if cell.mode != cell.latched {
                if cell.fill[3] > 0 {
                    cell.fill[3] = step_toward(cell.fill[3], 0);
                } else {
                    cell.latched = cell.mode;
                }
            }
            if cell.mode == cell.latched {
                for k in 0..4 {
                    cell.fill[k] = step_toward(cell.fill[k], t.fill[i][k]);
                }
            }
            for k in 0..4 {
                cell.outline[k] = step_toward(cell.outline[k], t.outline[i][k]);
            }
        }
    }

    /// Fill colour as drawn: RGB * brightness / 128, alpha unchanged.
    pub fn fill_drawn(&self, index: usize, brightness: u8) -> [u8; 4] {
        let f = self.cells[index].fill;
        let s = |v: u8| ((u32::from(v) * u32::from(brightness)) >> 7) as u8;
        [s(f[0]), s(f[1]), s(f[2]), f[3]]
    }

    /// Cursor tint pass: the value the full-screen sprite adds to the cursor
    /// cell, per channel Cs * A' >> 7 with A' = alpha * brightness >> 7.
    pub fn cursor_tint(&self, cursor: (u8, u8), brightness: u8) -> [u8; 3] {
        let f = self.cells[usize::from(cursor.1) * 7 + usize::from(cursor.0)].fill;
        let a = (u32::from(f[3]) * u32::from(brightness)) >> 7;
        std::array::from_fn(|k| ((u32::from(f[k]) * a) >> 7).min(255) as u8)
    }
}

// ---------------------------------------------------------------- geometry
pub fn cell_index(cell: (u8, u8)) -> usize {
    usize::from(cell.1) * 7 + usize::from(cell.0)
}
/// Fill quad corners (fan order) at y = -0.2.
pub fn fill_quad(cell: (u8, u8)) -> [[f64; 3]; 4] {
    let [x, z] = cell_center(cell.0, cell.1);
    let h = FILL_HALF;
    [
        [x - h, FILL_Y, z - h],
        [x + h, FILL_Y, z - h],
        [x + h, FILL_Y, z + h],
        [x - h, FILL_Y, z + h],
    ]
}
fn square_loop(center: [f64; 2], half: f64, y: f64) -> Vec<[[f64; 3]; 2]> {
    let c = [
        [center[0] - half, y, center[1] - half],
        [center[0] + half, y, center[1] - half],
        [center[0] + half, y, center[1] + half],
        [center[0] - half, y, center[1] + half],
    ];
    (0..4).map(|i| [c[i], c[(i + 1) % 4]]).collect()
}
/// Cell outline: three closed loops at y = -0.1.
pub fn outline_segments(cell: (u8, u8)) -> Vec<[[f64; 3]; 2]> {
    let center = cell_center(cell.0, cell.1);
    OUTLINE_HALVES
        .iter()
        .flat_map(|&h| square_loop(center, h, OUTLINE_Y))
        .collect()
}

/// A textured unit quad: corners in fan order with normalized UV.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitQuad {
    pub side: u8,
    pub slot: u8,
    pub cell: (u8, u8),
    pub corners: [[f64; 3]; 4],
    pub uv: [[f32; 2]; 4],
    /// Vertex RGB (GS units); alpha is 128.
    pub rgb: u8,
    /// Emblem outline segments and side colour (emblems only).
    pub outline: Vec<[[f64; 3]; 2]>,
    pub outline_rgb: [u8; 3],
    /// A face-up card: its card id and image values (else the card back or
    /// the emblem).
    pub face: Option<(u16, BoardFace)>,
    /// A flipping card: its face normal and the corners its back shows
    /// with; the renderer draws the face (`corners`) when the normal faces
    /// the eye (0x1DB180), else the back.
    pub flip: Option<([f64; 3], [[f64; 3]; 4])>,
    /// Vertex alpha (128 = opaque).
    pub alpha: u8,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnitLayers {
    pub cards: Vec<UnitQuad>,
    pub emblems: Vec<UnitQuad>,
    /// Explicitly unsupported parts, one message each.
    pub unsupported: Vec<String>,
}

/// A leader emblem's texture half and quarter turns: side 0 U 0..0.5 and
/// no turn, side 1 U 0.5..1 turned twice; an N-player duel's E and W
/// seats (a port addition) a quarter turn between.
fn emblem_pose(side: u8) -> (f32, u8) {
    #[cfg(feature = "online")]
    if side > 1 {
        return (if side == 2 { 0.0 } else { 0.5 }, crate::duel_multi::emblem_turns(side));
    }
    if side == 0 { (0.0, 0) } else { (0.5, 2) }
}

/// A board card's quarter turns: side 0's cards 2, side 1's 0; an
/// N-player duel's E and W seats (a port addition) 1 and 3.
fn card_turns(side: u8) -> u8 {
    #[cfg(feature = "online")]
    if side > 1 {
        return crate::duel_multi::card_turns(side);
    }
    if side == 0 { 2 } else { 0 }
}

fn place(center: [f64; 2], local: [f64; 2], quarter_turns: u8) -> [f64; 3] {
    let [mut x, mut z] = local;
    for _ in 0..quarter_turns % 4 {
        (x, z) = (-z, x);
    }
    [center[0] + x, UNIT_Y, center[1] + z]
}

pub fn unit_brightness(unit: &UnitInput, brightness: u8) -> u8 {
    let base: u32 = if unit.acted { 64 } else { 128 };
    ((base * u32::from(brightness)) >> 7) as u8
}

pub fn unit_layers(input: &OverlayInput) -> UnitLayers {
    let mut out = UnitLayers::default();
    let mut models = Vec::new();
    for u in &input.units {
        let cell = (u.cell[0], u.cell[1]);
        let [cx, cz] = cell_center(cell.0, cell.1);
        let center = [cx + f64::from(u.offset[0]), cz + f64::from(u.offset[1])];
        let rgb = unit_brightness(u, input.brightness);
        if u.slot == 0 {
            // Emblem: side 0 texels U 0..128 left to right, T 0 at the far
            // edge; side 1 is turned 180 degrees and uses U 128..256.
            let h = EMBLEM_HALF * f64::from(u.emblem.unwrap_or(128)) / 128.0;
            let (u0, turns) = emblem_pose(u.side);
            let local = [[-h, -h], [h, -h], [h, h], [-h, h]];
            let uv = [[u0, 0.0], [u0 + 0.5, 0.0], [u0 + 0.5, 1.0], [u0, 1.0]];
            let corners = local.map(|l| place(center, l, turns));
            out.emblems.push(UnitQuad {
                side: u.side,
                slot: u.slot,
                cell,
                corners,
                uv,
                rgb,
                outline: (0..4).map(|i| [corners[i], corners[(i + 1) % 4]]).collect(),
                outline_rgb: side_outline(u.side),
                face: None,
                flip: None,
                alpha: 128,
            });
            models.push(format!("leader card {} (side {})", u.card, u.side));
        } else if u.face_up && u.face.is_none() {
            out.unsupported.push(format!(
                "face-up card image of card {} at ({},{}) (no values; quad omitted)",
                u.card, cell.0, cell.1
            ));
            if u.slot <= 5 {
                models.push(format!("monster card {} (side {})", u.card, u.side));
            }
        } else {
            if u.face_up && u.slot <= 5 {
                models.push(format!("monster card {} (side {})", u.card, u.side));
            }
            // Side-1 corners (S,T): (0,0) at (-12.8,+18.4), (0,368) at
            // (-12.8,-18.4), (256,368) at (+12.8,-18.4), (256,0) at (+12.8,+18.4).
            let [hx, hz] = CARD_HALF;
            let local = [[-hx, hz], [-hx, -hz], [hx, -hz], [hx, hz]];
            let uv = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
            let turns = card_turns(u.side) + u8::from(u.defence);
            let mut corners = local.map(|l| place(center, l, turns));
            if let Some(p55) = u.sideways {
                // Partway between upright (turns without defence) and
                // sideways: a quarter turn scaled by p55 / 255.
                let base = card_turns(u.side);
                let a = f64::from((u32::from(p55) * 0x4000 / 255) as i32) * std::f64::consts::TAU / 65536.0;
                let (sa, ca) = a.sin_cos();
                corners = local.map(|l| {
                    let [x, z] = l;
                    place(center, [x * ca - z * sa, x * sa + z * ca], base)
                });
            }
            if let Some((yaw, size)) = u.spin {
                // rot y += yaw turns the flat card about the vertical axis
                // (the sideways sense: rot y down is +a here) [H sign].
                let a = -f64::from(yaw) * std::f64::consts::TAU / 65536.0;
                let (sa, ca) = a.sin_cos();
                let size = f64::from(size);
                let base = local.map(|l| [l[0] * size, l[1] * size]);
                corners = base.map(|[x, z]| place(center, [x * ca - z * sa, x * sa + z * ca], turns));
            }
            let mut flip = None;
            if let Some(p54) = u.flip {
                // ex = cZ a + sZ up (a = the face-down short axis), the long
                // axis stays flat; normal = sZ a - cZ up. The face side's
                // corners use -x so that they meet the face-up rest quad.
                let z = f64::from((u32::from(p54) * 0x8000 / 255) as i32) * std::f64::consts::TAU / 65536.0;
                let (sz, cz) = z.sin_cos();
                let o = place(center, [0.0, 0.0], turns);
                let ax = place(center, [1.0, 0.0], turns);
                let a = [ax[0] - o[0], 0.0, ax[2] - o[2]];
                let ex = [cz * a[0], sz, cz * a[2]];
                let normal = [sz * a[0], -cz, sz * a[2]];
                let at = |l: [f64; 2], sign: f64| -> [f64; 3] {
                    let flat = place(center, [0.0, l[1]], turns);
                    std::array::from_fn(|k| flat[k] + sign * l[0] * ex[k])
                };
                let back = local.map(|l| at(l, 1.0));
                corners = local.map(|l| at(l, -1.0));
                flip = Some((normal, back));
            }
            out.cards.push(UnitQuad {
                side: u.side,
                slot: u.slot,
                cell,
                corners,
                uv,
                rgb,
                outline: Vec::new(),
                outline_rgb: [0; 3],
                // Face-up cards keep the quad with their own image [F].
                face: u.face.filter(|_| u.face_up).map(|f| (u.card, f)),
                flip,
                alpha: u.alpha.unwrap_or(128),
            });
        }
    }
    if !models.is_empty() {
        out.unsupported.push(format!(
            "3D unit models (MONSTER.MRG) not implemented: {}",
            models.join(", ")
        ));
    }
    out
}

// ---------------------------------------------------------------- lines
/// GS screen position (raw 512 canvas, integer pixel samples) and 1/w.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPoint {
    pub x: f64,
    pub y: f64,
    pub inv_w: f64,
}
pub fn project(view: &Mat4, p: [f64; 3]) -> Option<ScreenPoint> {
    let m = crate::camera::mul(&GS_PROJECTION, view);
    let c: [f64; 4] =
        std::array::from_fn(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r]);
    // Near plane w = 1, as for the models.
    (c[3] >= 1.0).then(|| ScreenPoint {
        x: c[0] / c[3] - 1792.0,
        y: c[1] / c[3] - 1792.0,
        inv_w: 1.0 / c[3],
    })
}
/// Antialiased (AA1) GS line as a screen-space band: the two pixels across
/// the minor axis get coverage 1 - distance, as the GS edge antialiasing does
/// [H: tent coverage, verified only against dump draws 6993/6995]. Returns four
/// vertices (NDC x, NDC y, depth 1/w) with the signed minor-axis distance,
/// in triangle-strip order; the GS integer sample maps to the GPU centre.
pub fn aa_line_band(a: ScreenPoint, b: ScreenPoint) -> [([f32; 3], f32); 4] {
    let x_major = (b.x - a.x).abs() >= (b.y - a.y).abs();
    let offset = |p: ScreenPoint, d: f64| {
        let (x, y) = if x_major {
            (p.x, p.y + d)
        } else {
            (p.x + d, p.y)
        };
        (
            [
                ((x + 0.5) / 256.0 - 1.0) as f32,
                (1.0 - (y + 0.5) / 256.0) as f32,
                p.inv_w as f32,
            ],
            d as f32,
        )
    };
    [
        offset(a, -1.0),
        offset(a, 1.0),
        offset(b, -1.0),
        offset(b, 1.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duel_board::BoardCamera;

    fn round2() -> OverlayInput {
        serde_json::from_str(
            r#"{"state":24,"active_side":0,"brightness":128,"settled":true,
            "pulse":{"p1":26112,"p1_rising":true,"p2":24576,"p2_rising":true},
            "units":[{"side":0,"slot":0,"cell":[3,6],"card":392,"face_up":true},
                     {"side":1,"slot":0,"cell":[2,0],"card":397,"face_up":true,"acted":true},
                     {"side":1,"slot":1,"cell":[3,2],"card":406,"acted":true}]}"#,
        )
        .unwrap()
    }

    #[test]
    fn round2_targets_match_the_savestate_cell_records() {
        // Savestate cell records: cross cells fill (128,0,0,64), cursor (3,6)
        // (128,0,0,pulse); outlines red at (3,6), white at (2,0) and (3,2).
        let input = round2();
        input.validate().unwrap();
        let s = OverlayState::new(&input, (3, 6));
        assert_eq!(s.cells[cell_index((0, 6))].fill, [128, 0, 0, 64]);
        assert_eq!(s.cells[cell_index((3, 0))].fill, [128, 0, 0, 64]);
        assert_eq!(s.cells[cell_index((0, 0))].fill, [0, 0, 0, 0]);
        // P1 0x6600 -> alpha 64 + 0x66 = 166 (frame-60 dump draws 6870/6996).
        assert_eq!(s.cells[cell_index((3, 6))].fill, [128, 0, 0, 166]);
        assert_eq!(s.cursor_tint((3, 6), 128), [166, 0, 0]);
        assert_eq!(s.cells[cell_index((3, 6))].outline, [255, 0, 0, 128]);
        assert_eq!(s.cells[cell_index((2, 0))].outline, [255, 255, 255, 128]);
        assert_eq!(s.cells[cell_index((3, 2))].mode, 1);
        assert_eq!(s.cells[cell_index((0, 0))].mode, 0);
    }

    /// d63 set: P1 steps rising and sits at 0x8000 (the cursor glow at full)
    /// [interpreter `claude-padfeel/d63_dpad.py`: 23040 -> 23552 -> .. 32768,
    /// then 32768 while d63 counts down].
    #[test]
    fn glow_hold_keeps_p1_rising_at_the_top() {
        let input = round2();
        let mut s = OverlayState::new(&input, (3, 3));
        s.p1 = Pulse { value: 23040, rising: false, step: P1_STEP };
        s.glow_hold = true;
        s.update(&input, (3, 3));
        assert_eq!(s.p1.value, 23552);
        for _ in 0..30 {
            s.update(&input, (3, 3));
        }
        assert_eq!(s.p1.value, 0x8000);
        s.glow_hold = false;
        s.update(&input, (3, 3));
        assert_eq!(s.p1.value, 0x8000 - 0x200);
    }

    #[test]
    fn pulses_reflect_and_easing_latches_modes() {
        let mut p = Pulse {
            value: 0x7e00,
            rising: true,
            step: P1_STEP,
        };
        p.update();
        assert_eq!((p.value, p.rising), (0x8000, false));
        p.update();
        assert_eq!((p.value, p.rising), (0x7e00, false));
        let mut input = round2();
        let mut s = OverlayState::new(&input, (3, 6));
        // Moving the cursor: the old column fades by 8 per update, new cells rise.
        s.update(&input, (4, 6));
        assert_eq!(s.cells[cell_index((3, 0))].fill[3], 56);
        // An empty cell latches at once and starts easing in the same update [H].
        assert_eq!(s.cells[cell_index((4, 0))].fill, [8, 0, 0, 8]);
        assert_eq!(s.cells[cell_index((4, 0))].latched, 1);
        s.update(&input, (4, 6));
        assert_eq!(s.cells[cell_index((4, 0))].fill, [16, 0, 0, 16]);
        // The old cursor cell stays in the cross and eases down to alpha 64.
        assert_eq!(s.cells[cell_index((3, 6))].fill[3], 166 - 16);
        // Switching to the move state: fills fade out before the new mode latches.
        input.state = 0x1d;
        input.marks = vec![0xff; 49];
        input.marks[cell_index((4, 5))] = 1;
        input.move_allowance = 1;
        let before = s.cells[cell_index((0, 6))].fill;
        s.update(&input, (4, 6));
        let after = s.cells[cell_index((0, 6))];
        assert_eq!((after.fill[3], after.fill[0]), (before[3] - 8, before[0]));
        assert_eq!((after.mode, after.latched), (2, 1));
        let t = s.targets(&input, (4, 6));
        assert_eq!(t.fill[cell_index((4, 5))], [128, 128, 0, 0xd0]);
        assert_eq!(t.fill[cell_index((0, 0))], [128, 128, 128, 32]);
        assert_eq!(t.fill[cell_index((4, 6))][..3], [128, 0, 96]);
    }

    #[test]
    fn range_state_and_enemy_cursor_colours() {
        let mut input = round2();
        input.state = 0x3e;
        input.marks = vec![0xff; 49];
        input.marks[cell_index((1, 1))] = 2;
        let s = OverlayState::new(&input, (3, 2));
        assert_eq!(s.cells[cell_index((1, 1))].fill, [0, 0, 128, 128]);
        assert_eq!(s.cells[cell_index((6, 6))].fill, [128, 128, 128, 32]);
        input.state = 0x18;
        let s = OverlayState::new(&input, (3, 2));
        // Enemy unit under the cursor: colour set B (side 0 grey).
        assert_eq!(s.cells[cell_index((3, 2))].fill[..3], [96, 96, 96]);
        let mut pulsing = input.clone();
        pulsing.units[0].outline_pulse = true;
        let s = OverlayState::new(&pulsing, (3, 2));
        assert_eq!(s.cells[cell_index((3, 6))].outline, [255, 63, 63, 128]);
    }

    #[test]
    fn unit_layers_place_cards_and_emblems_and_report_unsupported_parts() {
        let layers = unit_layers(&round2());
        assert_eq!(layers.emblems.len(), 2);
        assert_eq!(layers.cards.len(), 1);
        let e = &layers.emblems[0];
        // Side-0 emblem on (3,6): U 0 at -x, V 0 at the far edge (z - 16.67).
        assert_eq!(e.uv[0], [0.0, 0.0]);
        assert!(
            (e.corners[0][0] + EMBLEM_HALF).abs() < 1e-9
                && (e.corners[0][2] - (150.0 - EMBLEM_HALF)).abs() < 1e-9
        );
        assert_eq!(
            (e.rgb, e.outline.len(), e.outline_rgb),
            (128, 4, [255, 0, 0])
        );
        // Side-1 emblem (acted): U 128..256, turned 180 degrees, brightness 64.
        let e1 = &layers.emblems[1];
        assert_eq!((e1.uv[0][0], e1.rgb), (0.5, 64));
        assert!((e1.corners[0][0] - (-50.0 + EMBLEM_HALF)).abs() < 1e-9);
        // Side-1 face-down card on (3,2): (S,T)=(0,0) at (-12.8, +18.4).
        let c = &layers.cards[0];
        assert!(
            (c.corners[0][0] + 12.8).abs() < 1e-9
                && (c.corners[0][2] - (-50.0 + 18.4)).abs() < 1e-9
        );
        assert_eq!(c.rgb, 64);
        assert!(
            layers
                .unsupported
                .iter()
                .any(|m| m.contains("3D unit models"))
        );
        let mut input = round2();
        input.units[2].face_up = true;
        input.units[2].defence = true;
        let layers = unit_layers(&input);
        assert!(layers.cards.is_empty());
        assert!(
            layers
                .unsupported
                .iter()
                .any(|m| m.contains("face-up card image of card 406"))
        );
        // With its image values the face-up card keeps the same quad.
        let face = BoardFace {
            level: 4,
            attack: 1500,
            defence: 1200,
            effect: false,
        };
        input.units[2].face = Some(face);
        let layers = unit_layers(&input);
        assert_eq!(layers.cards.len(), 1);
        assert_eq!(layers.cards[0].face, Some((406, face)));
        assert!(
            !layers
                .unsupported
                .iter()
                .any(|m| m.contains("face-up card image"))
        );
        input.units[2].face = None;
        input.units[2].face_up = false;
        let d = &unit_layers(&input).cards[0];
        assert_eq!(d.face, None);
        // Defence: 36.8 along x.
        let xs: Vec<f64> = d.corners.iter().map(|p| p[0]).collect();
        assert!((xs.iter().cloned().fold(f64::MIN, f64::max) - 18.4).abs() < 1e-9);
    }

    #[test]
    fn card_and_emblem_quads_match_a_saved_draw_list() {
        // Tutorial savestate (cursor (3,5)): GS vertices of the side-0 card-back
        // quad on (3,5) and the side-0 emblem on (3,6) from its DMA list, in
        // fan order with (S,T) (0,0), (0,368), (256,368), (256,0).
        let view = BoardCamera::snapped((3, 5)).view().unwrap();
        let input: OverlayInput = serde_json::from_str(
            r#"{"units":[{"side":0,"slot":1,"cell":[3,5],"card":156},
                         {"side":0,"slot":0,"cell":[3,6],"card":60,"face_up":true}]}"#,
        )
        .unwrap();
        let layers = unit_layers(&input);
        let expected = [
            (
                &layers.cards[0],
                [
                    [298.4375, 324.0],
                    [303.5625, 462.0625],
                    [208.375, 462.0625],
                    [213.5, 324.0],
                ],
            ),
            // Its near edge is clipped in that list (y 613.19): far corners only.
            (
                &layers.emblems[0],
                [
                    [190.8125, 528.0625],
                    [321.125, 528.0625],
                    [190.8125, 528.0625],
                    [321.125, 528.0625],
                ],
            ),
        ];
        for (quad, gs) in expected {
            let corners: Vec<_> = quad
                .corners
                .iter()
                .map(|c| project(&view, *c).unwrap())
                .collect();
            for want in gs {
                assert!(
                    corners
                        .iter()
                        .any(|p| (p.x - want[0]).abs() < 0.07 && (p.y - want[1]).abs() < 0.07),
                    "{want:?} not among {corners:?}"
                );
            }
            let first = project(&view, quad.corners[0]).unwrap();
            assert!((first.x - gs[0][0]).abs() < 0.07 && (first.y - gs[0][1]).abs() < 0.07);
            assert_eq!(quad.uv[0], [0.0, 0.0]);
        }
    }

    #[test]
    fn outlines_and_projected_bands_follow_the_dump() {
        assert_eq!(outline_segments((3, 6)).len(), 12);
        // Dump draw 6995: outline loop 24 top-left corner at GS (177.69, 305.81).
        let view = BoardCamera::snapped((3, 6)).view().unwrap();
        let p = project(&view, [-24.0, OUTLINE_Y, 126.0]).unwrap();
        assert!((p.x - 177.6875).abs() < 0.07 && (p.y - 305.8125).abs() < 0.07);
        let q = project(&view, [24.0, OUTLINE_Y, 126.0]).unwrap();
        let band = aa_line_band(p, q);
        // x-major: the band spans +-1 pixel vertically with signed distance.
        assert_eq!((band[0].1, band[1].1), (-1.0, 1.0));
        let px = |v: f32| (f64::from(v) + 1.0) * 256.0 - 0.5;
        assert!((px(band[0].0[0]) - p.x).abs() < 1e-3);
        assert!(project(&view, [0.0, 400.0, 500.0]).is_none());
        let input = OverlayInput {
            marks: vec![0; 3],
            ..round2()
        };
        assert!(input.validate().is_err());
    }
}
