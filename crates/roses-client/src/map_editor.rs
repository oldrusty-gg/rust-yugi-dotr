//! The map editor (mode 0xD, Custom Duel's Edit Map) as a Bevy-free model
//! (Claude, 7 Oct 2026). Notes, disassembly and interpreter / PCSX2 runs:
//! `work/research/modes/map-editor/NOTES.md`.
//!
//! Entry [F 0x1C7974..0x1C7A30, emu]: Custom Duel's Edit Map row on Cross
//! copies own map (0xA9C08C + 2) % 0xA9C08D of the pad side's card block
//! (0x213D70(0) + 0xFEA3 + 49·map) into the board cells (0x1E34A0), plays
//! SE 4, sets param[0xD] = 0 and pushes mode 0xD. Init 0x1CD750(param[0xD]):
//! cursor (3, 3), no move delay, the diagonal bit cleared, d57 = 0x63, d63 =
//! 0; a non-zero param would reset the own-map list to (top 3, count 5)
//! (never with Custom Duel's 0). The camera looks straight down on the board
//! (target 0, distance 800; 0x290730 / 0x290740).
//!
//! Update 0x1CD8C0 [F, emu]: the two pulses 0x2DB826 (±0x200) and 0x2DB828
//! (±0x800) step and bounce at 0 / 0x8000 (d63 ≠ 0 forces the first one
//! rising: once a direction was held it stays at 0x8000); the move delay
//! counts down; Start / Square / Cross save the board into that own map (SE
//! 4) and pop; Circle pops without saving (SE 5); L1 / R1 cycle the cursor
//! cell's terrain backwards / forwards through 0..9 (SE 3; L1 is read with
//! auto-repeat). With no delay running, a held direction moves the cursor
//! (edges block; Left before Right, Up before Down; a held diagonal moves
//! one axis per step, alternating through 0xA9C092 bit 0), SE 3 and an
//! 8-update delay; any held direction sets d63.
//!
//! Draw 0x1CDE40 [F, emu]: every cell's overlay target cleared (0x1E4240),
//! the cursor's row and column (and the cursor again) set to the colour at
//! 0x290750 with alpha 0x40 + (P1 >> 8) (0x1E42F0 mode 1), every cell eased
//! (0x1E44B0, ±8 per channel); then the duel HUD's terrain picture and
//! terrain box at (6, 0x190), the legend set 20 at (0x19A, 0x190) and the
//! terrain counts panel 0x1CE130 at (6, 0xB0). The board itself is drawn by
//! the main loop's field pass (0x1AEEE0 → 0x1AEF30), which leaves the
//! overlay targets alone outside the duel and has no cursor tint pass
//! there.
use crate::duel_hud::MenuCall;
use roses_core::card_block::{OWN_MAP_CELLS, OWN_MAP_COUNT};
use serde::Serialize;

/// Cells per side and terrain codes (L1 / R1 cycle modulo 10).
pub const SIDE: u8 = 7;
pub const TERRAINS: u8 = 10;
/// Updates between two cursor steps while a direction is held.
pub const MOVE_DELAY: u8 = 8;
/// d63 once a direction was held.
pub const HELD_MARK: u8 = 0x3C;
/// d57 (0x2DB8F7) in the editor: the HUD legend shows set 20.
pub const EDITOR_STATE: u8 = 0x63;
pub const LEGEND_SET: usize = 20;
/// Pulse steps per update and their top.
pub const P1_STEP: i32 = 0x200;
pub const P2_STEP: i32 = 0x800;
pub const PULSE_TOP: i32 = 0x8000;
/// Overlay easing per channel and update (0x1E44B0).
pub const EASE: u8 = 8;
/// Executable tables: the cross colour (B, G, R, –) and the ten terrains'
/// gauge colours (three B, G, R, A each).
pub const CROSS_COLOUR: u32 = 0x29_0750;
pub const GAUGE_COLOURS: u32 = 0x2B_DC40;
/// Terrain icons 270 + t, terrain names MSG 72 + t.
pub const TERRAIN_SPRITE: usize = 270;
/// The camera init writes [F 0x1CD764..0x1CD80C, emu]: target (0, 0, 0),
/// polar (distance 800, yaw π/2, pitch 1.5706217).
pub const CAMERA_TARGET: [f32; 3] = [0.0, 0.0, 0.0];
pub const CAMERA_POLAR: [f32; 3] = [800.0, std::f32::consts::FRAC_PI_2, 1.570_621_7];

/// PS2 pad bits as 0x203380 tests them.
pub mod pad {
    pub use crate::custom_duel::pad::*;
}

/// One update's pad: `pressed` = new presses (0x203380 mode 0), `repeat` =
/// auto-repeats (mode 2 adds them to the new presses), `held` = the
/// current word (mode 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditorPad {
    pub pressed: u16,
    pub repeat: u16,
    pub held: u16,
}

impl EditorPad {
    /// A new press held this update.
    pub fn press(bits: u16) -> Self {
        Self { pressed: bits, repeat: bits, held: bits }
    }
}

/// A pulse word with its two-bit signed direction (0x2DB904 bits 0-1 for
/// 0x2DB826, bits 2-3 for 0x2DB828).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Pulse {
    pub value: u16,
    /// The field's two bits (1 = rising, 3 = falling; 0 and 2 as found).
    pub dir: u8,
}

impl Pulse {
    pub fn new(value: u16, rising: bool) -> Self {
        Self { value, dir: if rising { 1 } else { 3 } }
    }
    fn signed(&self) -> i32 {
        let d = i32::from(self.dir & 3);
        if d >= 2 { d - 4 } else { d }
    }
    /// One step [F 0x1CD90C..0x1CD98C]: clamp to 0..=0x8000, then the
    /// direction negated at either end.
    pub fn step(&mut self, step: i32) {
        let v = i32::from(self.value) + self.signed() * step;
        self.value = v.clamp(0, PULSE_TOP) as u16;
        if self.value == 0 || i32::from(self.value) >= PULSE_TOP {
            self.dir = (-self.signed() & 3) as u8;
        }
    }
    pub fn rising(&self) -> bool {
        self.signed() > 0
    }
}

/// One board cell's overlay colours as the cell record keeps them (B, G, R,
/// A in memory order): target fill +4, current fill +8, target outline
/// +0xC, current outline +0x10, mode +0x18 and the latched mode +0x19.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CellOverlay {
    pub target: [u8; 4],
    pub fill: [u8; 4],
    pub outline_target: [u8; 4],
    pub outline: [u8; 4],
    pub mode: u8,
    pub latched: u8,
}

impl CellOverlay {
    /// 0x1E42F0(x, y, mode, colour, 0, 0): the targets only.
    fn set(&mut self, mode: u8, colour: Option<[u8; 4]>) {
        self.mode = mode;
        self.target = colour.unwrap_or([0; 4]);
        self.outline_target = [0; 4];
    }

    /// 0x1E44B0 [F 0x1E4524..0x1E48B8]: a changed mode first fades the
    /// current alpha out (8 per update, nothing else that update) and is
    /// latched the update the alpha is 0; otherwise each fill and outline
    /// channel moves at most 8 toward its target.
    pub fn ease(&mut self) {
        if self.mode != self.latched {
            if self.fill[3] != 0 {
                self.fill[3] -= self.fill[3].min(EASE);
            } else {
                self.latched = self.mode;
            }
            return;
        }
        let toward = |cur: u8, target: u8| {
            if target < cur { cur - (cur - target).min(EASE) } else { cur + (target - cur).min(EASE) }
        };
        for k in [2, 1, 0, 3] {
            self.fill[k] = toward(self.fill[k], self.target[k]);
        }
        for k in [2, 1, 0, 3] {
            self.outline[k] = toward(self.outline[k], self.outline_target[k]);
        }
    }

    /// The fill as the port's board overlay takes it (R, G, B, A).
    pub fn fill_rgba(&self) -> [u8; 4] {
        [self.fill[2], self.fill[1], self.fill[0], self.fill[3]]
    }
}

/// What an update asks of the frontend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorEvent {
    Sfx(u16),
    /// Start / Square / Cross: write `cells` to own map `map` of the pad
    /// side's block (0x213D70(0)), then pop.
    Save { map: usize, cells: [u8; OWN_MAP_CELLS] },
    /// Circle: pop without saving (0x1A9290).
    Cancel,
}

/// SE ids (0x216EA0(0, id)).
pub const SFX_CURSOR: u16 = 3;
pub const SFX_CONFIRM: u16 = 4;
pub const SFX_CANCEL: u16 = 5;

/// The editor's state: the board cells' terrain (0x2DC570 + 0x1C·i + 0x16),
/// 0xA9C08C..0xA9C092, the pulses and d63 / their overlays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapEditor {
    pub cells: [u8; OWN_MAP_CELLS],
    /// 0xA9C08C / 0xA9C08D: Custom Duel's own-map list (the saved map is
    /// (top + 2) % count).
    pub own_top: u8,
    pub own_count: u8,
    /// 0xA9C08F / 0xA9C090.
    pub cursor: (u8, u8),
    /// 0xA9C091.
    pub delay: u8,
    /// 0xA9C092 bit 0: the last step of a held diagonal was horizontal.
    pub diagonal: bool,
    /// 0x2DB826 / 0x2DB828 with their directions (0x2DB904).
    pub p1: Pulse,
    pub p2: Pulse,
    /// 0x2DB903.
    pub d63: u8,
    pub overlay: [CellOverlay; OWN_MAP_CELLS],
}

impl MapEditor {
    /// Init 0x1CD750(param) on the board `cells` Custom Duel loaded; the
    /// pulses continue from the previous modes (`p1`, `p2`), the overlay from
    /// the board's (0x1A9520 cleared it at the mode switch).
    pub fn new(cells: [u8; OWN_MAP_CELLS], own_top: u8, own_count: u8, param: i32, p1: Pulse, p2: Pulse) -> Self {
        let (own_top, own_count) = if param != 0 { (OWN_MAP_COUNT as u8 - 2, OWN_MAP_COUNT as u8) } else { (own_top, own_count) };
        Self {
            cells,
            own_top,
            own_count,
            cursor: (3, 3),
            delay: 0,
            diagonal: false,
            p1,
            p2,
            d63: 0,
            overlay: [CellOverlay::default(); OWN_MAP_CELLS],
        }
    }

    /// The own map a save writes ((0xA9C08C + 2) % 0xA9C08D).
    pub fn map(&self) -> usize {
        usize::from((self.own_top + 2) % self.own_count.max(1))
    }

    pub fn cell(&self, x: u8, y: u8) -> u8 {
        self.cells[usize::from(y) * usize::from(SIDE) + usize::from(x)]
    }

    /// The cursor cell's terrain.
    pub fn terrain(&self) -> u8 {
        self.cell(self.cursor.0, self.cursor.1)
    }

    /// Cells of each terrain (0x1CE130's counts).
    pub fn counts(&self) -> [u8; TERRAINS as usize] {
        let mut n = [0u8; TERRAINS as usize];
        for &t in &self.cells {
            if let Some(c) = n.get_mut(usize::from(t)) {
                *c += 1;
            }
        }
        n
    }

    /// Update 0x1CD8C0.
    pub fn update(&mut self, p: EditorPad) -> Vec<EditorEvent> {
        let mut ev = Vec::new();
        if self.d63 != 0 {
            self.p1.dir = 1;
        }
        self.p1.step(P1_STEP);
        self.p2.step(P2_STEP);
        if self.delay != 0 {
            self.delay -= 1;
        }
        if p.pressed & (pad::START | pad::SQUARE | pad::CROSS) != 0 {
            ev.push(EditorEvent::Save { map: self.map(), cells: self.cells });
            ev.push(EditorEvent::Sfx(SFX_CONFIRM));
            return ev;
        }
        if p.pressed & pad::CIRCLE != 0 {
            ev.push(EditorEvent::Sfx(SFX_CANCEL));
            ev.push(EditorEvent::Cancel);
            return ev;
        }
        if p.pressed & (pad::L1 | pad::R1) != 0 {
            let t = self.terrain();
            let back = (p.repeat | p.pressed) & pad::L1 != 0;
            let t = if back { (t + TERRAINS - 1) % TERRAINS } else { (t + 1) % TERRAINS };
            ev.push(EditorEvent::Sfx(SFX_CURSOR));
            let (x, y) = self.cursor;
            self.cells[usize::from(y) * usize::from(SIDE) + usize::from(x)] = t;
        }
        if self.delay != 0 {
            return ev;
        }
        let mut held = p.held;
        if held & 0xF000 == 0 {
            return ev;
        }
        let (x, y) = self.cursor;
        // Edges [F 0x1CDC04..0x1CDC8C]: a blocked direction drops both of
        // its axis' bits.
        if (held & pad::LEFT != 0 && x == 0) || (held & pad::RIGHT != 0 && x >= SIDE - 1) {
            held &= !(pad::LEFT | pad::RIGHT);
        }
        if (held & pad::UP != 0 && y == 0) || (held & pad::DOWN != 0 && y >= SIDE - 1) {
            held &= !(pad::UP | pad::DOWN);
        }
        self.d63 = HELD_MARK;
        let vertical = pad::UP | pad::DOWN;
        let horizontal = pad::LEFT | pad::RIGHT;
        if self.diagonal {
            if held & vertical != 0 {
                held &= vertical;
            } else {
                self.diagonal = false;
            }
        }
        if held & horizontal != 0 {
            self.cursor.0 = if held & pad::LEFT != 0 { x - 1 } else { x + 1 };
            if held & vertical != 0 {
                self.diagonal = true;
            }
        } else if held & vertical != 0 {
            self.cursor.1 = if held & pad::UP != 0 { y - 1 } else { y + 1 };
            self.diagonal = false;
        } else {
            self.diagonal = false;
            return ev;
        }
        ev.push(EditorEvent::Sfx(SFX_CURSOR));
        self.delay = MOVE_DELAY;
        ev
    }

    /// The overlay part of draw 0x1CDE40: targets, then one easing step.
    /// `colour` = the executable's 0x290750 bytes (B, G, R, –).
    pub fn draw_overlay(&mut self, colour: [u8; 4]) {
        for c in &mut self.overlay {
            c.set(0, None);
        }
        let alpha = ((self.p1.value >> 8) as u8).wrapping_add(0x40);
        let lit = [colour[0], colour[1], colour[2], alpha];
        let (cx, cy) = self.cursor;
        for (i, c) in self.overlay.iter_mut().enumerate() {
            let (x, y) = ((i % 7) as u8, (i / 7) as u8);
            if x == cx || y == cy {
                c.set(1, Some(lit));
            }
        }
        for c in &mut self.overlay {
            c.ease();
        }
    }
}

/// The executable bytes the editor's drawing reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorTables {
    /// 0x290750 (B, G, R, –).
    pub cross: [u8; 4],
    /// 0x2BDC40: per terrain three colours (B, G, R, A).
    pub gauges: [[u8; 12]; TERRAINS as usize],
}

impl EditorTables {
    pub fn from_executable(executable: &[u8]) -> Result<Self, String> {
        let s = |a: u32, n: usize| crate::duel_hud::elf_slice(executable, a, n).map_err(|e| e.to_string());
        let cross: [u8; 4] = s(CROSS_COLOUR, 4)?.try_into().map_err(|_| "cross colour")?;
        let g = s(GAUGE_COLOURS, 12 * usize::from(TERRAINS))?;
        let gauges = std::array::from_fn(|k| g[12 * k..12 * k + 12].try_into().unwrap());
        Ok(Self { cross, gauges })
    }
}

/// The two gouraud quads of a terrain gauge (0x1CE130 / 0x1CB890 /
/// 0x1CC870): `count` of 49 cells as trunc(84 · count / 49) pixels from
/// (x, y), colours 0 → 1 over the top 8 rows and 1 → 2 below.
pub fn gauge(calls: &mut Vec<MenuCall>, colours: &[u8; 12], x: i32, y: i32, count: u8) {
    let w = (84.0f32 * f32::from(count) / 49.0) as i32;
    let c = |k: usize| [colours[4 * k + 2], colours[4 * k + 1], colours[4 * k], colours[4 * k + 3]];
    let (x0, x1) = (x as f32, (x + w) as f32);
    let (y0, y1, y2) = (y as f32, (y + 8) as f32, (y + 16) as f32);
    calls.push(MenuCall::Quad { corners: [(x0, y0, c(0)), (x1, y0, c(0)), (x0, y1, c(1)), (x1, y1, c(1))] });
    calls.push(MenuCall::Quad { corners: [(x0, y1, c(1)), (x1, y1, c(1)), (x0, y2, c(2)), (x1, y2, c(2))] });
}

/// The 2D calls of draw 0x1CDE40 after the overlay: the terrain picture
/// and box (as the duel HUD's 0x1B4370 / 0x1B4220 at (6, 0x190)), the
/// legend (set 20) and the counts panel 0x1CE130(6, 0xB0).
pub fn hud_calls(ed: &MapEditor, tables: &EditorTables) -> Vec<MenuCall> {
    let mut c = Vec::new();
    let t = i32::from(ed.terrain());
    c.push(MenuCall::TerrainPicture { colour: 0, terrain: t, x: 6, y: 0x190 });
    c.push(MenuCall::TerrainBox { terrain: t, x: 6, y: 0x190 });
    c.push(MenuCall::Legend { side: 0, set: LEGEND_SET, x: 0x19A, y: 0x190 });
    counts_panel(&mut c, ed, tables, 6, 0xB0);
    c
}

/// 0x1CE130(x, y): the panel and per terrain its icon (the cursor's
/// terrain with the overlay of mode 1, alpha 0x2DB828 >> 8), gauge and
/// count.
fn counts_panel(c: &mut Vec<MenuCall>, ed: &MapEditor, tables: &EditorTables, x: i32, y: i32) {
    c.push(MenuCall::Panel { grad: 0, deco: -1, ptype: 0xB, x, y, wu: -0x6E, hu: -0xAA, bright: true });
    let counts = ed.counts();
    let current = ed.terrain();
    for k in 0..TERRAINS {
        let ry = y + 5 + 16 * i32::from(k);
        let desc = TERRAIN_SPRITE + usize::from(k);
        if k == current {
            let a = (ed.p2.value >> 8) as u8;
            c.push(MenuCall::SpriteOverlay { desc, x: x + 5, y: ry, mode: 1, rgba: [128, 128, 128, a] });
        } else {
            c.push(MenuCall::Sprite { desc, x: x + 5, y: ry });
        }
        c.push(MenuCall::Panel { grad: 13, deco: -1, ptype: 5, x: x + 0x15, y: ry, wu: -0x54, hu: -0x10, bright: true });
        gauge(c, &tables.gauges[usize::from(k)], x + 0x15, ry, counts[usize::from(k)]);
        c.push(MenuCall::Panel { grad: -1, deco: -1, ptype: 5, x: x + 0x15, y: ry, wu: -0x54, hu: -0x10, bright: true });
        c.push(MenuCall::Number { x: x + 0x3F, y: y + 7 + 16 * i32::from(k), size: 0, flags: 2, digits: 2, value: i32::from(counts[usize::from(k)]), dim: false });
    }
}

#[cfg(test)]
mod tests;
