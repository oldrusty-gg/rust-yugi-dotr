//! The original duel HUD of the PAL executable (SLES_515.79): the primitive
//! list the game's routine 0x1B1D50 builds for one frame (LP panels, card
//! boxes, legend, terrain box and picture, round counter, hand boxes and the
//! hand overlay), reproduced from the game's own layout code.
//!
//! Bevy-independent. [`HudTables`] holds the executable's constant tables
//! (read from the user's executable at run time, never embedded) and the
//! message lengths; [`HudState`] is a plain snapshot of the state fields the
//! HUD reads; [`frame`] returns the primitives in GS draw order plus the HUD
//! bookkeeping the routine writes back. Everything the original research
//! could not establish is reported as [`Unsupported`] instead of guessed.
//!
//! Coordinates are GS window coordinates of the 512×512 frame with the
//! 1792/1792 XYOFFSET removed; all values are exact multiples of 1/16 as the
//! GS registers hold them.
//!
//! Text is drawn from a 4-bit canvas (GS 0x2040): line 0 is a digit strip,
//! lines 1–3 hold composed messages. [`TextCanvas`] reproduces its contents
//! from the message file's font cells.
//!
//! [`HudTextures`] decodes the atlases, portraits and font from the user's
//! files; [`render`] draws a frame's primitives into a 16-bit frame with the
//! GS model of [`crate::gs`] exactly as the original frame buffer receives
//! them; [`render_layer`] gives the HUD alone as a premultiplied RGBA8 layer
//! for compositing over another renderer's 3D frame.
//!
//! [`from_duel`] builds the [`HudState`] from a `roses_core` duel, the rule
//! getters of `roses_core::duel::queries` and the input-state bytes
//! ([`HudUi`], which [`ui_from_input`] derives from the port's input
//! states).

use crate::gs::{self, Blend, Sprite, Target16, Texture, Triangle};
use std::collections::BTreeMap;

mod engine;
pub use engine::{
    after_skipped_states, from_duel, initial_update, settled_hand_positions, ui_from_input,
    FlashWave, GraveyardUi, HudExtra, HudRecord, HudUi, Selected,
};

/// Frame depth of every HUD primitive (0x1DB160(0xFFFFF8)).
pub const HUD_Z: u32 = 0xFF_FFF8;
/// Depth of the hand overlay (0x1DB160(0xFFFFFC) before 0x1AFF00).
pub const OVERLAY_Z: u32 = 0xFF_FFFC;
/// Depth of the overlay windows (0x1B0120 sets 0x1DB160(0xFFFFFE)) [F;
/// emu: every GRAVEYARD-panel vertex has Z 0xFFFFFE].
pub const WINDOW_Z: u32 = 0xFF_FFFE;

const DESCRIPTORS: u32 = 0x28_F300;
const DESCRIPTOR_COUNT: usize = 280;
const ATTRIBUTE_ICONS: u32 = 0x28_F2A0;
const GLYPHS: u32 = 0x28_FDF0;
const GLYPH_COUNT: usize = 111;
const LEGEND_SETS: u32 = 0x29_0400;
const LEGEND_SET_COUNT: usize = 21;
const COLOURS: u32 = 0x2B_DBC0;
const COLOUR_COUNT: usize = 20;
const FONT_SIZES: u32 = 0x2B_DCD0;
const MARGINS: u32 = 0x2B_DD40;
const MARGIN_COUNT: usize = 14;
const DECO_OFFSETS: u32 = 0x2B_DD78;
const ALPHA_MODES: u32 = 0x2D_0FA0;
const TEXT_ALPHA: u32 = 0x2D_2F80;
const TEXT_GREY: u32 = 0x2D_2FB0;
const DIMX: u32 = 0x29_3200;
/// Overlay window titles: pointers by slot code (0x1B0120 reads
/// [0x2903B0 + 4·code]).
const WINDOW_TITLES: u32 = 0x29_03B0;
/// Slot code of the GRAVEYARD panel (0x1D52F8 writes 0x0E to slot 0).
const GRAVEYARD_CODE: u32 = 0x0E;
/// The OPTION panel's title (slot 0 code 7, "OPTION").
const OPTION_CODE: u32 = 7;

/// Names start at message 323 (0x1DF480); 854 cards have names.
const CARD_NAME_MESSAGE: u16 = 323;
const CARD_NAMES: u16 = 854;

/// One 10-byte sprite descriptor: texture slot and texel rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Descriptor {
    pub slot: u8,
    pub u0: u16,
    pub v0: u16,
    pub u1: u16,
    pub v1: u16,
}

impl Descriptor {
    fn parse(b: &[u8]) -> Self {
        let h = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
        Self {
            slot: b[0],
            u0: h(2),
            v0: h(4),
            u1: h(6),
            v1: h(8),
        }
    }
    pub fn width(&self) -> i32 {
        (i32::from(self.u1) - i32::from(self.u0)).abs()
    }
    pub fn height(&self) -> i32 {
        (i32::from(self.v1) - i32::from(self.v0)).abs()
    }
}

/// The executable's HUD tables plus the composed-text widths.
#[derive(Clone, Debug)]
pub struct HudTables {
    /// EE 0x28F300: atlas sprite descriptors.
    pub sprites: Vec<Descriptor>,
    /// EE 0x28F2A0: attribute icons (0–5 monster attributes, 6–8 classes).
    pub attribute_icons: Vec<Descriptor>,
    /// EE 0x28FDF0: gold-font glyphs, 37 per font size.
    pub glyphs: Vec<Descriptor>,
    /// EE 0x2BDBC0: outer then inner colour, each stored B, G, R, A.
    pub colours: Vec<[u8; 8]>,
    /// EE 0x2BDD40: panel margins (left, right, top, bottom) per panel type.
    pub margins: Vec<[u8; 4]>,
    /// EE 0x2BDD78: decoration offsets per decoration number.
    pub deco_offsets: [u8; 16],
    /// EE 0x2BDCD0: gold font (cell width, line height) per size.
    pub font_sizes: [(u8, u8); 3],
    /// EE 0x290400: legend rows (icon word, message id) per set.
    pub legend_sets: Vec<[(u16, u16); 4]>,
    /// EE 0x2D0FA0: the four GS ALPHA register values of 0x208450.
    pub alpha_modes: [u64; 4],
    /// Text canvas CLUT (C_32, 16 entries) as 0x2459F0 builds it for colour
    /// set 0: grey level from EE 0x2D2FB0, alpha from EE 0x2D2F80.
    pub text_clut: [[u8; 4]; 16],
    /// GS DIMX (dither matrix, DTHE on) in force for every HUD draw of the
    /// dumped frame: the second of three matrices at EE 0x2931F8..0x293210
    /// (which one the game selects when is not established).
    pub dimx: u64,
    /// The OPTION panel's title ([0x2903B0 + 4·7], gold font).
    pub option_title: Vec<u8>,
    /// The GRAVEYARD panel's title (NUL-terminated string at
    /// [0x2903B0 + 4·0x0E], drawn in the gold font).
    pub graveyard_title: Vec<u8>,
    /// Expanded message codes by message id (MSG_E.DAT).
    messages: BTreeMap<u16, Vec<u16>>,
}

/// A NUL-terminated string at an executable address (at most 64 bytes).
pub fn elf_string(elf: &[u8], address: u32) -> crate::Result<Vec<u8>> {
    let mut out = Vec::new();
    for i in 0..64 {
        match elf_slice(elf, address + i, 1)?[0] {
            0 => return Ok(out),
            b => out.push(b),
        }
    }
    Err(format!("no string end at EE 0x{address:X}"))
}

/// Map an EE address inside the executable's first loadable segment.
pub fn elf_slice(elf: &[u8], address: u32, len: usize) -> crate::Result<&[u8]> {
    let rd16 = |o: usize| -> crate::Result<u16> {
        elf.get(o..o + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .ok_or_else(|| "executable header truncated".to_string())
    };
    let rd32 = |o: usize| -> crate::Result<u32> {
        elf.get(o..o + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| "executable header truncated".to_string())
    };
    if elf.get(..4) != Some(b"\x7fELF".as_slice()) {
        return Err("not an ELF executable".into());
    }
    let phoff = rd32(0x1C)? as usize;
    let (phentsize, phnum) = (rd16(0x2A)? as usize, rd16(0x2C)? as usize);
    for i in 0..phnum {
        let h = phoff + i * phentsize;
        let (kind, offset, vaddr, filesz) = (rd32(h)?, rd32(h + 4)?, rd32(h + 8)?, rd32(h + 16)?);
        if kind == 1
            && address >= vaddr
            && u64::from(address) + len as u64 <= u64::from(vaddr) + u64::from(filesz)
        {
            let start = offset as usize + (address - vaddr) as usize;
            return elf
                .get(start..start + len)
                .ok_or_else(|| format!("executable too short for EE 0x{address:X}"));
        }
    }
    Err(format!("EE 0x{address:X} is not in a loaded segment"))
}

impl HudTables {
    /// Read the tables from the user's executable. `messages` maps message
    /// ids to their expanded MSG_E.DAT codes; [`HudTables::required_messages`]
    /// lists the ids the HUD can compose.
    pub fn from_executable(
        executable: &[u8],
        messages: BTreeMap<u16, Vec<u16>>,
    ) -> crate::Result<Self> {
        let table = |address: u32, len: usize| elf_slice(executable, address, len);
        let descriptors = |address: u32, count: usize| -> crate::Result<Vec<Descriptor>> {
            Ok(table(address, count * 10)?
                .chunks(10)
                .map(Descriptor::parse)
                .collect())
        };
        let colours = table(COLOURS, COLOUR_COUNT * 8)?
            .chunks(8)
            .map(|c| c.try_into().unwrap())
            .collect();
        let margins = table(MARGINS, MARGIN_COUNT * 4)?
            .chunks(4)
            .map(|c| c.try_into().unwrap())
            .collect();
        let sizes = table(FONT_SIZES, 6)?;
        let legend_sets = table(LEGEND_SETS, LEGEND_SET_COUNT * 16)?
            .chunks(16)
            .map(|s| {
                let w = |o: usize| u16::from_le_bytes([s[o], s[o + 1]]);
                [0, 1, 2, 3].map(|k| (w(4 * k), w(4 * k + 2)))
            })
            .collect();
        let modes = table(ALPHA_MODES, 32)?;
        let alpha = table(TEXT_ALPHA, 16)?;
        let grey = table(TEXT_GREY, 16)?;
        Ok(Self {
            sprites: descriptors(DESCRIPTORS, DESCRIPTOR_COUNT)?,
            attribute_icons: descriptors(ATTRIBUTE_ICONS, 9)?,
            glyphs: descriptors(GLYPHS, GLYPH_COUNT)?,
            colours,
            margins,
            deco_offsets: table(DECO_OFFSETS, 16)?.try_into().unwrap(),
            font_sizes: [0, 1, 2].map(|i| (sizes[2 * i], sizes[2 * i + 1])),
            legend_sets,
            alpha_modes: [0, 1, 2, 3]
                .map(|i| u64::from_le_bytes(modes[8 * i..8 * i + 8].try_into().unwrap())),
            text_clut: std::array::from_fn(|i| [grey[i], grey[i], grey[i], alpha[i]]),
            dimx: u64::from_le_bytes(table(DIMX, 8)?.try_into().unwrap()),
            graveyard_title: {
                let p = table(WINDOW_TITLES + 4 * GRAVEYARD_CODE, 4)?;
                elf_string(executable, u32::from_le_bytes(p.try_into().unwrap()))?
            },
            option_title: {
                let p = table(WINDOW_TITLES + 4 * OPTION_CODE, 4)?;
                elf_string(executable, u32::from_le_bytes(p.try_into().unwrap()))?
            },
            messages,
        })
    }

    /// Read the tables from the user's executable and MSG_E.DAT (the
    /// supported English message file; its hash is verified).
    pub fn from_files(executable: &[u8], message_file: &[u8]) -> crate::Result<Self> {
        let mut messages = BTreeMap::new();
        for chunk in Self::required_messages().chunks(256) {
            let parsed =
                roses_assets::parse_pal_messages(message_file, chunk).map_err(|e| e.to_string())?;
            for m in parsed.messages {
                messages.insert(m.message_id, m.expanded_codes);
            }
        }
        Self::from_executable(executable, messages)
    }

    /// Message ids the HUD may compose: hidden-card texts, classes, races,
    /// terrains, ranks, legend labels and card names.
    pub fn required_messages() -> Vec<u16> {
        let mut ids: Vec<u16> = (29..=36).collect();
        ids.extend(38..=69); // races 0..=31
        ids.extend(71..=95); // terrain -1..=9, "????", ranks 0..=12
        ids.extend(116..=123); // the OPTION panel's rows (L1 in the duel)
        ids.extend(175..=195); // legend labels
        ids.extend(CARD_NAME_MESSAGE..CARD_NAME_MESSAGE + CARD_NAMES);
        ids.extend(260..=280); // message box texts (0x104 "Trap activated!" ..)
        ids.push(814);
        ids
    }

    /// Supply or replace a message (screens that compose their own lines,
    /// e.g. the port's Online Play menus, under ids no MSG_E.DAT message uses).
    pub fn set_message(&mut self, id: u16, codes: Vec<u16>) {
        self.messages.insert(id, codes);
    }

    /// Expanded codes of a message, if supplied.
    pub fn message(&self, id: u16) -> Option<&[u16]> {
        self.messages.get(&id).map(Vec::as_slice)
    }

    fn inner(&self, colour: usize) -> [u8; 4] {
        let c = self.colours[colour];
        [c[6], c[5], c[4], c[7]]
    }

    fn outer(&self, colour: usize) -> [u8; 4] {
        let c = self.colours[colour];
        [c[2], c[1], c[0], c[3]]
    }
}

/// One side's HUD fields (S = EE 0x2DABA0 + side·0x350).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HudSide {
    /// S+0x33A.
    pub lp: u16,
    /// S+0x33C: ≥ 100 human player, < 100 CPU.
    pub controller: u8,
    /// S+0x33D / S+0x33E: cursor column and row.
    pub cursor: [u8; 2],
    /// S+0x341: star chips.
    pub stars: u8,
    /// S+0x230: deck count (0x1DFC40).
    pub deck: u8,
    /// S+0x32E: hand count (0x1E0390).
    pub hand: u8,
    /// S+0x346 (bit 0: summon still allowed).
    pub summon_flags: u8,
    /// S+0x344 (1..=0x7F with deck > 0 flashes the Sm icon).
    pub summon_counter: u8,
    /// 0x1E13D0(side): units present in slots 1–5 of the side's group.
    pub monsters: u8,
    /// 0x1E1470(side): units present in slots 6–10.
    pub magic_traps: u8,
}

/// Values of a unit evaluated at one board cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnitStats {
    /// 0x267A00(unit, col, row, 1) + 0x1E1570(unit, 0) (clamped by the HUD).
    pub attack: i32,
    /// 0x267A00(unit, col, row, 0) + 0x1E1650(unit, 0).
    pub defence: i32,
    /// 0x24F7D0(terrain at the cell, unit): 0 none, 1 bonus, 2 penalty.
    pub terrain_effect: i32,
    /// 0x247E10(unit, col, row) (the HUD tests the low nibble ≥ 2).
    pub allowance: i32,
}

/// A unit record (EE 0x2DB990 + group·0x3F4 + slot·0x5C) as the card boxes
/// and the legend read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HudUnit {
    /// +0 is non-zero (the record has a card).
    pub has_card: bool,
    /// +0x3C.
    pub card: u16,
    /// +0x3E: class << 5 | race.
    pub kind: u8,
    /// +0x3F.
    pub attribute: u8,
    /// +0x40 / +0x41.
    pub cell: [u8; 2],
    /// +0x44.
    pub side: u8,
    /// +0x45: 0 leader, 1–5 monsters, 6–10 magic/trap, ≥ 11 none.
    pub slot: u8,
    /// +0x49 / +0x4A (0xFF = infinite).
    pub turns: [u8; 2],
    /// +0x57.
    pub flags: u8,
    /// +0x58.
    pub flags2: u8,
    /// 0x251C80(side, slot).
    pub controller: u8,
    /// 0x2154C0(side, card).
    pub rank: i32,
    /// 0x1DA6A0(unit) != 0 (face-down card the viewer may not see).
    pub face_down_hidden: bool,
    /// 0x1E1C60(unit, 0) != 0.
    pub effect: bool,
    /// Bit a set when 0x1E1EF0(unit, a, 0) != 0, a in 4..=16.
    pub abilities: u32,
    /// 0x268CA0(unit) != 0 (legend of mode 0x18).
    pub legend_query: bool,
    /// Evaluated at the cursor cell of S(d3c).
    pub at_cursor: UnitStats,
    /// Evaluated at the board cell that holds the unit.
    pub at_own_cell: UnitStats,
}

/// One hand card of side [0x2DB8F4] for the hand overlay (0x1AFF00).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HandOverlayCard {
    /// Screen position floats of the card object (record +4 → object +0/+4);
    /// `None` when the record or its object is absent.
    pub position: Option<[f32; 2]>,
    /// 0x1E0340(side, i): card id in bits 0–11, fusion order in bits 12–15.
    pub word: i32,
    /// What 0x1DCEB0 composes on the hand record's small card face
    /// (0x1AFAC0); `None` when the record is absent.
    pub face: Option<HandFace>,
}

/// The values the small card face of a hand record shows (0x1DBA80 with
/// s0 = 0): stars = cost 0x1E1BD0, values = 0x1E1570 / 0x1E1650 plus the
/// terrain bonus 0x267A00 at the record's cell (+0x40/+0x41, the summon
/// cell the refill wrote) [F/emu 683-p2-x1].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HandFace {
    pub card: u16,
    pub level: u8,
    pub attack: i32,
    pub defence: i32,
}

/// The state fields the HUD reads. `dXX` are the bytes at EE 0x2DABA0 + 0xDXX.
#[derive(Clone, Debug, PartialEq)]
pub struct HudState {
    pub sides: [HudSide; 2],
    /// 0x2DB8D5.
    pub d35: u8,
    /// 0x2DB8DC: side whose turn the HUD shows.
    pub d3c: u8,
    /// 0x2DB8DD: turn number (top box shows 100 − d3d).
    pub d3d: u8,
    /// 0x2DB8DE: slide offset (0 shown .. 128 hidden), before this frame.
    pub d3e: u8,
    /// 0x2DB8DF: upper card box slide, before this frame.
    pub d3f: u8,
    /// 0x2DB8E0..E3: main (d40, d41) and upper (d42, d43) card-box cells.
    pub d40: u8,
    pub d41: u8,
    pub d42: u8,
    pub d43: u8,
    /// 0x2DB8E4: banner alpha.
    pub d44: u8,
    /// 0x2DB8E5 / 0x2DB8E7 / 0x2DB8E8: hand-selection UI.
    pub d45: u8,
    pub d47: u8,
    pub d48: u8,
    /// 0x2DB8EF: selected hand card.
    pub d4f: u8,
    /// 0x2DB8F3 / 0x2DB8F4: live turn number and side.
    pub d53: u8,
    pub d54: u8,
    /// 0x2DB8F7: UI mode.
    pub d57: u8,
    /// 0x2DB905 / 0x2DB906 / 0x2DB907: HUD control bits.
    pub d65: u8,
    pub d66: u8,
    pub d67: u8,
    /// u16 EE 0x2DB828 (overlay alpha = value >> 8).
    pub flash: u16,
    /// 0x1A8C50(30) != 0 (enables hidden face-down cards in the card box).
    pub option_30: bool,
    /// Online (a port addition): the side this screen belongs to
    /// (`engine::HudExtra::own_side`). `None` offline.
    pub own_side: Option<u8>,
    /// Unit records referenced by the fields below.
    pub units: Vec<HudUnit>,
    /// 0x1E4960 per cell (row·7 + col): index into `units`.
    pub board: [Option<usize>; 49],
    /// Cell +0x16 per cell (row·7 + col).
    pub terrain: [u8; 49],
    /// [0x2DB268] / [0x2DB26C].
    pub unit_a: Option<usize>,
    pub unit_b: Option<usize>,
    /// 0x1E07C0(2, d4f + 1): the selected hand card's record.
    pub hand_card: Option<usize>,
    /// 0x2692E0(A.side, A.slot, B.side, B.slot); read when A and B are
    /// valid and their controllers differ.
    pub advantage: Option<i32>,
    /// 0x1FB900(−1, −1) != 0 (legend of mode 0x30).
    pub legend_1fb900: bool,
    /// 0x1E3760(cursor of S(d3c)) != 0 (legend of mode 0x3E).
    pub legend_1e3760: bool,
    /// 0x1F2580(T + 74): the terrain tile model is loaded.
    pub terrain_model_ready: bool,
    /// Online duels (a port addition): each side's player name (the gold
    /// font's letters and digits), drawn in place of the PLAYER / ENEMY
    /// plates under the portraits and on the graveyard tabs; none: the
    /// original's plates. Compiled out without the `online` feature.
    #[cfg(feature = "online")]
    pub names: [Option<Vec<u8>>; 2],
    /// Online duels (a port addition): small gold lines under the round
    /// counter, between the two LP plates ("2 WATCHING" for a player while
    /// spectators watch; SPECTATING for a spectator); none offline.
    #[cfg(feature = "online")]
    pub notes: Vec<Vec<u8>>,
    /// Left and right hand-box values: 0x1E1B70(2, d4f + 1, 0), and for the
    /// right box 0x270280() instead when [0x2DB8EE] != 0.
    pub hand_box_values: [i32; 2],
    /// 0x1AB8B0(1) (the overlay needs 1).
    pub overlay_mode: i8,
    /// Hand cards of side d54 (count = its hand count).
    pub hand_overlay: Vec<HandOverlayCard>,
    /// Overlay window slot 3 opened by 0x1E5540 kind 0 (state 0x13), drawn
    /// by 0x1B0120 after the HUD; `None` = closed (0x18).
    pub message_box: Option<MessageBox>,
    /// Overlay window slot 0 with code 0x0E: the GRAVEYARD selection panel
    /// of Mimicat / Monster Reborn (states 0x2B..0x2E), drawn by 0x1B0120
    /// before slot 3; `None` = slot 0 not 0x0E.
    pub graveyard: Option<GraveyardPanel>,
}

impl HudState {
    /// The online player name drawn on side `side`'s plates (a port
    /// addition; always none without the `online` feature).
    pub fn plate_name(&self, side: usize) -> Option<Vec<u8>> {
        #[cfg(feature = "online")]
        {
            self.names.get(side).cloned().flatten()
        }
        #[cfg(not(feature = "online"))]
        {
            let _ = side;
            None
        }
    }
}

/// The GRAVEYARD panel (overlay slot 0, code 0x0E, drawer 0x1B0120 case
/// 0x1B0CFC; `GRAVEYARD-PANEL.md`). Every field is a byte or list the
/// selection states 0x2B..0x2E keep in the duel block B = EE 0x2DABA0.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraveyardPanel {
    /// The two candidate lists, oldest first: list 0 = side 0's graveyard
    /// (EE 0x1ED0000, pointer B+0x6D8, count B+0xC8C), list 1 = side 1's
    /// (0x1ED00F0, B+0x6DC, B+0xC90), filtered by the card's class (0x2B
    /// scan through 0x251920). A list the card does not offer is empty.
    pub lists: [Vec<GraveyardEntry>; 2],
    /// d36 & 0x7F: the list shown (0 or 1).
    pub shown: u8,
    /// d36 bit 7: one list only (no switching, one tab, no arrows).
    pub locked: bool,
    /// B+0xC8A / B+0xC8E: first list entry in the five-row window.
    pub offsets: [u16; 2],
    /// d5e / d5f: cursor row 0..=4 inside the window.
    pub rows: [u8; 2],
    /// s16 B+0xC9C (slot 0's slide): 0 closed .. 64 at rest; 0x2C adds 8
    /// per update up to 64, 0x2E takes 8 per update down to 0. Every
    /// element is drawn at its rest y + 512 − 8·slide.
    pub slide: i16,
}

/// The slide value of the GRAVEYARD panel at rest (0x2C stops at 0x40).
pub const GRAVEYARD_REST: i16 = 0x40;

/// One list entry: the card id and what 0x1DE350(card) gives for its icon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GraveyardEntry {
    pub card: u16,
    /// Byte 0 of the card record 0x1DE350(card): class << 5 | race (the
    /// same value as [`HudUnit::kind`]); `None` when 0x1DE350 returns no
    /// record (card 671, ids < 0 or ≥ 854): the row is left empty.
    pub kind: Option<u8>,
}

/// 0x1E5540(0, x, y, text): a plain box anchored at centre x 256 + x, top
/// 256 + y, sized to MSG_E message `text` (DUEL-MESSAGE-BOX.md) [F, emu].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageBox {
    pub x: i16,
    pub y: i16,
    pub text: u16,
}

impl Default for HudState {
    fn default() -> Self {
        Self {
            sides: [HudSide::default(); 2],
            d35: 0,
            d3c: 0,
            d3d: 0,
            d3e: 0,
            d3f: 0,
            d40: 0,
            d41: 0,
            d42: 0,
            d43: 0,
            d44: 0,
            d45: 0,
            d47: 0,
            d48: 0,
            d4f: 0,
            d53: 0,
            d54: 0,
            d57: 0,
            d65: 0,
            d66: 0,
            d67: 0,
            flash: 0,
            option_30: false,
            own_side: None,
            units: Vec::new(),
            board: [None; 49],
            terrain: [0; 49],
            unit_a: None,
            unit_b: None,
            hand_card: None,
            advantage: None,
            legend_1fb900: false,
            legend_1e3760: false,
            terrain_model_ready: false,
            #[cfg(feature = "online")]
            names: [None, None],
            #[cfg(feature = "online")]
            notes: Vec::new(),
            hand_box_values: [0; 2],
            overlay_mode: 0,
            hand_overlay: Vec::new(),
            message_box: None,
            graveyard: None,
        }
    }
}

/// HUD element that emitted a primitive (top-level call of 0x1B1D50).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    TerrainPicture,
    LpPanel(u8),
    CardBox,
    LegendBox,
    TerrainBox,
    RoundBox,
    RoundNumber,
    Banner,
    HandBoxLeft,
    HandBoxRight,
    HandOverlay,
    MessageBox,
    /// Overlay slot 0 code 0x0E (0x1B0CFC).
    GraveyardPanel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimKind {
    Sprite,
    TriStrip,
}

/// Texture a primitive samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureSource {
    None,
    /// Descriptor slot 6 (TEXSYS.MRG #6) or 7 (TEXSYS.MRG #7), bound every
    /// frame by 0x1AF910; 8-bit CLUT, alpha 0 or 0x80 (TEXA 0/0x80).
    Atlas(u8),
    /// The 4-bit text canvas: `line` 0 is the digit strip; lines 1–3 hold
    /// `message` composed just before the sprite is drawn.
    TextCanvas {
        line: u8,
        message: Option<u16>,
    },
    /// The side's PICMINI portrait (40×32 in a 64×32 slot).
    Portrait(u8),
    /// Render target 0x2000 (the VU1 terrain tile picture).
    RenderTarget,
    /// The card picture 0x1B8940 binds (PICTURE.MRG record, 128×128,
    /// [`HudTextures::picture`]).
    Picture,
}

/// Frame buffer a primitive draws into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Frame,
    RenderTarget,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
    pub rgba: [u8; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Primitive {
    pub element: Element,
    pub kind: PrimKind,
    pub texture: TextureSource,
    pub target: Target,
    /// ABE with this ALPHA mode (index into [`HudTables::alpha_modes`]).
    pub blend: Option<u8>,
    /// ATE with ATST GREATER, AREF 0.
    pub alpha_test: bool,
    /// Bilinear filtering (text canvas and render target).
    pub bilinear: bool,
    /// IIP (Gouraud) for the gradients.
    pub gouraud: bool,
    pub z: u32,
    /// Untextured primitives carry the UV the GS register still holds from
    /// the previous textured vertex (the packet does not write UV for them);
    /// it has no visual effect and is kept so lists compare exactly.
    pub verts: Vec<Vertex>,
}

/// Parts of the frame the research did not establish; reported, not guessed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnsupportedKind {
    /// The VU1 render of the terrain tile model into RT 0x2000 (between the
    /// RT clear and the RT sprite); only its framing is reproduced.
    TerrainTileRender { terrain: i32 },
    /// Banner 0x1B8400 (d44 != 0): never exercised.
    Banner { alpha: u8 },
    /// Panel decorations 5 and 6: never exercised.
    PanelDecoration(u8),
    /// A message the tables were not given.
    MissingMessage(u16),
    /// Card id without a name message (0x1DF480 returns −1).
    CardName(u16),
    /// Card class 6/7 (0x1DF320 would index past the attribute icons).
    CardClass(u8),
    /// Descriptor slot other than 6/7 (not bound by 0x1AF910).
    TextureSlot(u8),
    /// An input the state left out (e.g. `advantage`).
    MissingInput(&'static str),
    /// Legend of mode 0x1D read without unit A (the game reads address 0x45).
    LegendWithoutUnit,
    /// A sprite descriptor index past the table (the game would read on).
    Descriptor(usize),
    /// An attribute byte past the attribute-icon table.
    AttributeIcon(u8),
    /// A turn side other than 0/1 (d3c indexes per-side tables).
    TurnSide(u8),
    /// A GRAVEYARD-panel list number (d36 & 0x7F) other than 0/1.
    GraveyardList(u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsupported {
    pub element: Element,
    /// Index in [`HudFrame::prims`] where the missing part belongs.
    pub at: usize,
    pub kind: UnsupportedKind,
}

/// HUD bytes the routine writes back this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HudUpdate {
    pub d3c: u8,
    pub d3d: u8,
    pub d3e: u8,
    pub d3f: u8,
    pub d40: u8,
    pub d41: u8,
    pub d42: u8,
    pub d43: u8,
    pub d67: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HudFrame {
    pub prims: Vec<Primitive>,
    pub unsupported: Vec<Unsupported>,
    pub next: HudUpdate,
}

/// The HUD primitives of one frame in GS draw order.
pub fn build(state: &HudState, tables: &HudTables) -> Vec<Primitive> {
    frame(state, tables).prims
}

/// The HUD of one frame: primitives, unsupported parts and write-backs.
pub fn frame(state: &HudState, tables: &HudTables) -> HudFrame {
    let mut b = Builder {
        t: tables,
        st: state,
        prims: Vec::new(),
        unsupported: Vec::new(),
        element: Element::TerrainPicture,
        z: HUD_Z,
        gradient_blend: None,
        gold_rgba: GREY,
    };
    let next = b.run();
    // 0x1B0120: overlay slots 0..3 in order, after the HUD.
    b.graveyard_panel();
    b.message_box();
    let mut prims = b.prims;
    // The GS UV register keeps the last textured vertex's value.
    let mut uv = (0.0, 0.0);
    for p in &mut prims {
        if p.texture == TextureSource::None {
            for v in &mut p.verts {
                (v.u, v.v) = uv;
            }
        } else if let Some(v) = p.verts.last() {
            uv = (v.u, v.v);
        }
    }
    HudFrame {
        prims,
        unsupported: b.unsupported,
        next,
    }
}

/// Overlay slot 3 opened by 0x1E5540 kind 5, 6 or 7 (states 0x14, 0x15,
/// 0x16): the memory-card dialogs' boxes (`memory_card.rs`). Drawn by
/// 0x1B0120 cases 0x1B1570 / 0x1B15FC / 0x1B168C through 0x1B1830 [F;
/// DUEL-MESSAGE-BOX.md §2.3; the project notes §4].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayKind {
    /// Kind 5: the box centred vertically on 256 + y.
    Centred,
    /// Kind 6: as 5, taller by 0x1C, with the press icon.
    Press,
    /// Kind 7: YES / NO under the text; `yes` = D35.
    YesNo { yes: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayBox {
    pub kind: OverlayKind,
    pub x: i16,
    pub y: i16,
    /// MSG_E message id.
    pub text: u16,
}

/// The YES / NO strings (EE 0x2BE2EF / 0x2BE2F3) and 0x1B7B50's colour for
/// the unselected one (EE 0x2BDCC4, stored B, G, R, A).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YesNoTables {
    pub yes: Vec<u8>,
    pub no: Vec<u8>,
    pub dim: [u8; 4],
}

const YES_STRING: u32 = 0x2B_E2EF;
const NO_STRING: u32 = 0x2B_E2F3;
const GOLD_DIM: u32 = 0x2B_DCC4;
/// The press icon's first descriptor (three cels, 5 updates each).
const PRESS_ICON: usize = 190;

impl YesNoTables {
    pub fn from_executable(executable: &[u8]) -> crate::Result<Self> {
        let c = elf_slice(executable, GOLD_DIM, 4)?;
        Ok(Self {
            yes: elf_string(executable, YES_STRING)?,
            no: elf_string(executable, NO_STRING)?,
            dim: [c[2], c[1], c[0], c[3]],
        })
    }
}

/// One overlay box of kind 5/6/7 as primitives (z 0xFFFFFE, after whatever
/// HUD the mode draws). `counter` = the update counter 0x2DA954 that picks
/// the press icon's cel.
pub fn overlay_box(tables: &HudTables, yes_no: &YesNoTables, b: &OverlayBox, counter: u32) -> HudFrame {
    let state = HudState::default();
    let mut builder = Builder {
        t: tables,
        st: &state,
        prims: Vec::new(),
        unsupported: Vec::new(),
        element: Element::MessageBox,
        z: WINDOW_Z,
        gradient_blend: None,
        gold_rgba: GREY,
    };
    builder.overlay_box(b, yes_no, counter);
    HudFrame {
        prims: builder.prims,
        unsupported: builder.unsupported,
        next: HudUpdate::default(),
    }
}

/// Overlay slot 0 kind 7: the OPTION panel (0x1E5270 / 0x1E5320; the map's
/// SYSTEM icon 2 and the duel's Opt) [F 0x1B02CC..0x1B0768].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionPanel {
    /// D54: the title plate's colour (the map passes SAVE[0]).
    pub side: u8,
    /// 0xC9C: the slide, used raw (0x200 hidden .. 0 in place).
    pub y: i16,
    /// D35: the row cursor (0 Monster Battle, 1 Sound).
    pub row: u8,
    /// D36: bit 1 Display (else Abbreviate), bit 2 Stereo (else Mono).
    pub bits: u8,
}

/// The rows' messages: (heading, first, second) and the D36 bit whose set
/// state selects the first (MSG 116/120/121, 117/123/122) [F].
pub const OPTION_ROWS: [(u16, u16, u16, u8); 2] = [(116, 120, 121, 1 << 1), (117, 123, 122, 1 << 2)];

/// The OPTION panel as primitives (drawn after the mode's HUD).
pub fn option_panel(tables: &HudTables, yes_no: &YesNoTables, b: &OptionPanel) -> HudFrame {
    let state = HudState::default();
    let mut builder = Builder {
        t: tables,
        st: &state,
        prims: Vec::new(),
        unsupported: Vec::new(),
        element: Element::MessageBox,
        z: WINDOW_Z,
        gradient_blend: None,
        gold_rgba: GREY,
    };
    builder.option_panel(b, yes_no.dim);
    HudFrame {
        prims: builder.prims,
        unsupported: builder.unsupported,
        next: HudUpdate::default(),
    }
}

// ------------------------------------------------------------- NOW LOADING
/// The NOW LOADING overlay 0x1AA220, drawn by the main loop after the mode
/// fade (0x100ACC: 0x1FDFB0, 0x207CF0(3), 0x1AA220), so always over the
/// black hold of a mode change [F 0x1AA220..0x1AA894; emu modes/ui3
/// loading-cases.json, PCSX2 loading-editor].
///
/// The mode init 0x1A9330 starts it (0x1A94AC: load state 0x2DAB88 bits
/// 4–5 := 3, counter +0xC := 0, alpha 0x2DAB84 := 0, pulse 0x2DAB85 := 0,
/// pulse direction +1) with 0x2DAB8A bit 4 = whether the overlay is shown
/// for that change (see `shows_now_loading`). While the load state is 3 the
/// overlay's alpha is min(counter, 0x80); when the loads are done 0x1AABB0
/// sets the alpha to 0x80 (state 2), and state 1 waits until it has fallen
/// to 0 (−4 per drawn frame) before the fade-in channel runs: 32 frames of
/// NOW LOADING fading out after even an instant load. It draws while the
/// load state is ≥ 3 or the alpha is not 0: `counter mod 532` tiles of
/// 18×24 (28 per row from (4, 4), the rows from the 10th 48 px lower) with
/// the animation frame (counter / 532) mod 5, the rest of the 532 with the
/// previous frame once the counter has passed 532, all at the alpha; then
/// the string at (0x100, 0xEA), gold size 2, flags 3, in 0x28F1A0's colour
/// with alpha·min(pulse + 0x40, 0x80)/128. Each drawn frame the pulse moves
/// ±4 (turning at 0 and ≥ 0x80), the alpha falls by 4 when not 0 and the
/// counter grows by 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NowLoading {
    /// +0xC (0x2DA95C).
    pub counter: u32,
    /// 0x2DAB84.
    pub alpha: u8,
    /// 0x2DAB85.
    pub pulse: u8,
    /// 0x2DAB88 bits 2–3 = −1.
    pub falling: bool,
}

/// Tiles of one full pass (28 × 19).
pub const NOW_LOADING_TILES: u32 = 532;
/// Frames the overlay fades out after the loads (0x80 / 4).
pub const NOW_LOADING_FADE: u32 = 32;
const NOW_LOADING_STRING: u32 = 0x2B_DB95;
const NOW_LOADING_COLOUR: u32 = 0x28_F1A0;

/// The overlay's string and colour (EE 0x2BDB95, 0x28F1A0 stored B, G, R).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NowLoadingTables {
    pub text: Vec<u8>,
    pub rgb: [u8; 3],
}

impl NowLoadingTables {
    pub fn from_executable(executable: &[u8]) -> crate::Result<Self> {
        let c = elf_slice(executable, NOW_LOADING_COLOUR, 4)?;
        Ok(Self { text: elf_string(executable, NOW_LOADING_STRING)?, rgb: [c[2], c[1], c[0]] })
    }
}

impl NowLoading {
    /// 0x1A94E0..0x1A94F8 at the mode init.
    pub fn start() -> Self {
        Self { counter: 0, alpha: 0, pulse: 0, falling: false }
    }

    /// The loads are done (0x1AAC4C, load state 3 → 2).
    pub fn loaded(&mut self) {
        self.alpha = 0x80;
    }

    /// The tiles' alpha this frame, `None` when 0x1AA220 draws nothing
    /// (`loading` = load state ≥ 3).
    pub fn shown(&self, loading: bool) -> Option<u8> {
        if !loading && self.alpha == 0 {
            return None;
        }
        Some(if self.alpha != 0 { self.alpha } else { self.counter.min(0x80) as u8 })
    }

    /// The steps after a drawn frame (0x1AA7CC..0x1AA850).
    pub fn step(&mut self) {
        let p = i32::from(self.pulse) + if self.falling { -4 } else { 4 };
        self.pulse = p as u8;
        if self.pulse == 0 || self.pulse >= 0x80 {
            self.falling = !self.falling;
        }
        if self.alpha != 0 {
            self.alpha -= 4;
        }
        self.counter += 1;
    }
}

/// The overlay's primitives for this frame (none when it is not drawn).
pub fn now_loading(tables: &HudTables, strings: &NowLoadingTables, s: &NowLoading, loading: bool) -> HudFrame {
    let state = HudState::default();
    let mut b = Builder {
        t: tables,
        st: &state,
        prims: Vec::new(),
        unsupported: Vec::new(),
        element: Element::Banner,
        z: HUD_Z,
        gradient_blend: None,
        gold_rgba: GREY,
    };
    if let Some(alpha) = s.shown(loading) {
        let pass = (s.counter / NOW_LOADING_TILES) % 5;
        let filled = s.counter % NOW_LOADING_TILES;
        let tile = |b: &mut Builder<'_>, i: u32, frame: u32| {
            let (u, v) = if frame < 3 { (200 + 18 * (frame % 3), 136) } else { (80 + 18 * (frame % 3), 128) };
            let (col, row) = ((i % 28) as Fx, (i / 28) as Fx);
            let x = 4 + 18 * col;
            let y = 4 + 24 * row + if row >= 9 { 0x30 } else { 0 };
            let (u, v) = (u as Fx, v as Fx);
            b.sprite(
                [x * PX, y * PX, (x + 18) * PX, (y + 24) * PX],
                [u * PX + HALF, v * PX + HALF, (u + 18) * PX + HALF, (v + 24) * PX + HALF],
                [128, 128, 128, alpha],
                TextureSource::Atlas(6),
                Some(0),
                true,
            );
        };
        for i in 0..filled {
            tile(&mut b, i, pass);
        }
        if filled < s.counter {
            for i in filled..NOW_LOADING_TILES {
                tile(&mut b, i, (pass + 4) % 5);
            }
        }
        let text_alpha = ((u32::from(alpha) * (u32::from(s.pulse) + 0x40).min(0x80)) >> 7) as u8;
        let [r, g, bl] = strings.rgb;
        b.gold_rgba = [r, g, bl, text_alpha];
        b.gold_string(0x100, 0xEA, 2, 3, &strings.text);
        b.gold_rgba = GREY;
    }
    HudFrame { prims: b.prims, unsupported: b.unsupported, next: HudUpdate::default() }
}

// ---------------------------------------------------------------- menu calls
/// One emitter call of a menu mode's draw pass, drawn with the duel HUD's
/// routines (Custom Duel's draw 0x1C82B0 calls these leaves; see
/// `roses_game::custom_duel_screen`). Coordinates are
/// screen pixels (the callers' + 0x700 GS offset removed).
#[derive(Clone, Debug, PartialEq)]
pub enum MenuCall {
    /// 0x1B5360(grad, deco, ptype, x, y, wu, hu, bright).
    Panel { grad: i32, deco: i32, ptype: usize, x: i32, y: i32, wu: i32, hu: i32, bright: bool },
    /// 0x1B48A0 / 0x1B4860 with descriptor `desc` of 0x28F300 at (x, y),
    /// plain (grey 128).
    Sprite { desc: usize, x: i32, y: i32 },
    /// 0x1B6ED0(x, y, nx, ny): nx × ny tiles of descriptor 189, each at its
    /// pixel edge − 0.5 with exact texels.
    Tiles { desc: usize, x: i32, y: i32, nx: i32, ny: i32 },
    /// 0x1B71C0 / 0x1B71D0 with no fill: the tinted box (frame set 0) in
    /// colour set `colour` (−1 = 2).
    Box { colour: i32, x: i32, y: i32, wu: i32, hu: i32 },
    /// 0x1B7B50: a gold-font string; `dim` = the 0x2BDCC4 colour (t3).
    Gold { x: i32, y: i32, size: usize, flags: u32, text: Vec<u8>, dim: bool },
    /// 0x1B7A80: a gold-font number of `digits` cells.
    Number { x: i32, y: i32, size: usize, flags: u32, digits: i32, value: i32, dim: bool },
    /// 0x246B90: canvas `line` holding `message` (`lines` canvas lines
    /// tall), scaled (sx, sy).
    Text { line: u8, message: u16, x: i32, y: i32, sx: f32, sy: f32, lines: u8, dim: bool },
    /// 0x1B7880: `n` digits of the canvas digit strip (pitch 16 when
    /// `narrow`, else 20), right-aligned from (x, y); `dim` = t3 0x2BDCC4.
    Digits { x: i32, y: i32, n: i32, value: i32, narrow: bool, dim: bool },
    /// 0x1B7880 with t2 ≠ 0: the same digits, all `n` of them (zero
    /// padded); `narrow` = t1 ≠ 0 (pitch 16, else 20) [F 0x1B78D0].
    DigitsPadded { x: i32, y: i32, n: i32, value: i32, narrow: bool, dim: bool },
    /// 0x1B48A0 with the mirror flag: the descriptor's U reversed.
    SpriteFlipped { desc: usize, x: i32, y: i32 },
    /// 0x1B8B70: portrait slot `slot` (the HUD textures' two PICMINI slots).
    Portrait { slot: u8, x: i32, y: i32, w: i32, h: i32, bright: bool },
    /// 0x1B5120: rank insignia.
    Insignia { x: i32, y: i32, rank: i32 },
    /// An untextured gouraud quad (TL, TR, BL, BR) as 0x206F90 emits it.
    Quad { corners: [(f32, f32, [u8; 4]); 4] },
    /// 0x1B8E40: the full-screen quad with ALPHA mode 2 and alpha `alpha`.
    Darken { alpha: u8 },
    /// 0x1B8E40 / an untextured sprite with ALPHA mode 2 over (x, y, w, h).
    DarkenRect { x: i32, y: i32, w: i32, h: i32, alpha: u8 },
    /// 0x1B48A0 with an attribute icon of 0x28F2A0 (0-5 attributes, 6-8
    /// classes).
    Attribute { icon: usize, x: i32, y: i32 },
    /// 0x1B5120(x, y, rank, small, mirror): bars and stars at 16 px (12 px
    /// small, 3/4 size), the stars' columns swapped and the bars mirrored
    /// when `mirror` [F 0x1B5120].
    InsigniaEx { x: i32, y: i32, rank: i32, small: bool, mirror: bool },
    /// An untextured line (0x206F90 packet, two vertices) drawn one pixel
    /// tall [H: the GS line rasteriser is not modelled].
    Line { x0: i32, y0: i32, x1: i32, rgba: [u8; 4] },
    /// 0x1B7B50 with a colour pointer (t3) other than the dim one.
    GoldTint { x: i32, y: i32, size: usize, flags: u32, text: Vec<u8>, rgba: [u8; 4] },
    /// 0x246B90 with a colour pointer (t0): the canvas line in `rgba`
    /// (a message of several lines covers that many 22-row bands).
    TextTint { line: u8, message: u16, x: i32, y: i32, sx: f32, sy: f32, rgba: [u8; 4] },
    /// 0x1B71D0 with a fill colour set (t2 >= 0): the fill, then the box.
    BoxFilled { colour: i32, x: i32, y: i32, wu: i32, hu: i32, fill: i32 },
    /// 0x1B48A0 with its overlay (stack +0x20 = ALPHA mode `mode`): the
    /// sprite, then an untextured sprite over it in `rgba` (the colour
    /// pointer's B, G, R or grey 128; alpha = 0x2DB828 >> 8), alpha-tested
    /// [F 0x1B5020..0x1B50DC].
    SpriteOverlay { desc: usize, x: i32, y: i32, mode: u8, rgba: [u8; 4] },
    /// 0x1B8940(picture, x, y, w, h): the bound card picture's 128×128
    /// texels (U/V 0.5..128.5) over (x, y)-(x + w, y + h), grey 128
    /// [F 0x1B8940; nearest sampling and no blend are [H]].
    Picture { x: i32, y: i32, w: i32, h: i32 },
    /// 0x1B48A0 with grey 0x30 (stack +0x28) instead of 0x80: the dimmed
    /// sprite (the editor catalogue's seen-but-not-owned cells).
    SpriteDim { desc: usize, x: i32, y: i32 },
    /// 0x1B4370(colour, terrain, x, y): the terrain picture's backing and
    /// the render-target sprite of the terrain model (the map editor).
    TerrainPicture { colour: usize, terrain: i32, x: i32, y: i32 },
    /// 0x1B4220(terrain, x, y): the terrain name box (MSG 72 + terrain).
    TerrainBox { terrain: i32, x: i32, y: i32 },
    /// 0x1B3CE0 with the button legend set `set` chosen (d3c = `side`).
    Legend { side: u8, set: usize, x: i32, y: i32 },
}

/// The menu calls as primitives, in call order.
pub fn menu_frame(tables: &HudTables, yes_no: &YesNoTables, calls: &[MenuCall]) -> HudFrame {
    // Only `MenuCall::TerrainPicture` reads the state: its model is drawn
    // by the caller into the render target.
    let state = HudState { terrain_model_ready: true, ..HudState::default() };
    let mut b = Builder {
        t: tables,
        st: &state,
        prims: Vec::new(),
        unsupported: Vec::new(),
        element: Element::Banner,
        z: HUD_Z,
        gradient_blend: None,
        gold_rgba: GREY,
    };
    let dim = [yes_no.dim[0], yes_no.dim[1], yes_no.dim[2], yes_no.dim[3]];
    for call in calls {
        match call {
            MenuCall::Panel { grad, deco, ptype, x, y, wu, hu, bright } => {
                b.panel(*grad, *deco, *ptype, *x, *y, *wu, *hu, *bright)
            }
            MenuCall::Sprite { desc, x, y } => {
                if let Some(d) = b.desc(*desc as i64) {
                    b.atlas(d, *x, *y, Atlas::bright(true));
                }
            }
            MenuCall::Tiles { desc, x, y, nx, ny } => {
                let Some(d) = b.desc(*desc as i64) else { continue };
                let (w, h) = (d.width(), d.height());
                for r in 0..*ny {
                    for c in 0..*nx {
                        let (x0, y0) = ((x + c * w) * PX - HALF, (y + r * h) * PX - HALF);
                        let uv = [i32::from(d.u0) * PX, i32::from(d.v0) * PX, i32::from(d.u1) * PX, i32::from(d.v1) * PX];
                        b.atlas_sprite(&d, [x0, y0, x0 + w * PX, y0 + h * PX], uv, GREY);
                    }
                }
            }
            MenuCall::Box { colour, x, y, wu, hu } => b.box_frame(*colour, *x, *y, *wu, *hu),
            MenuCall::Gold { x, y, size, flags, text, dim: d } => {
                b.gold_rgba = if *d { dim } else { GREY };
                b.gold_string(*x, *y, *size, *flags, text);
                b.gold_rgba = GREY;
            }
            MenuCall::Number { x, y, size, flags, digits, value, dim: d } => {
                b.gold_rgba = if *d { dim } else { GREY };
                b.gold_number(*x, *y, *size, *flags, *digits, *value);
                b.gold_rgba = GREY;
            }
            MenuCall::Text { line, message, x, y, sx, sy, lines, dim: d } => {
                let Some(codes) = b.t.message(*message) else {
                    b.unsupported(UnsupportedKind::MissingMessage(*message));
                    continue;
                };
                let (_, w) = TextCanvas::measure(codes);
                let at = b.prims.len();
                b.text_lines(*line, i32::from(*message), *x, *y, w as i32, *sx, *sy, i32::from((*lines).max(1)));
                if *d {
                    for p in &mut b.prims[at..] {
                        for v in &mut p.verts {
                            v.rgba = [dim[0], dim[1], dim[2], WHITE[3]];
                        }
                    }
                }
            }
            MenuCall::Portrait { slot, x, y, w, h, bright } => b.portrait(*slot, *x, *y, *w, *h, *bright),
            MenuCall::Digits { x, y, n, value, narrow, dim: d } | MenuCall::DigitsPadded { x, y, n, value, narrow, dim: d } => {
                let at = b.prims.len();
                let pad = matches!(call, MenuCall::DigitsPadded { .. });
                b.lp_digits_padded(*x, *y, *n, *value, *narrow, pad);
                if *d {
                    for p in &mut b.prims[at..] {
                        for v in &mut p.verts {
                            v.rgba = [dim[0], dim[1], dim[2], WHITE[3]];
                        }
                    }
                }
            }
            MenuCall::SpriteFlipped { desc, x, y } => {
                let Some(d) = b.desc(*desc as i64) else { continue };
                let (w, h) = (d.width(), d.height());
                let xy = [x * PX, y * PX, (x + w) * PX, (y + h) * PX];
                let uv = [
                    i32::from(d.u1) * PX + HALF,
                    i32::from(d.v0) * PX + HALF,
                    i32::from(d.u0) * PX + HALF,
                    i32::from(d.v1) * PX + HALF,
                ];
                b.atlas_sprite(&d, xy, uv, GREY);
            }
            MenuCall::Insignia { x, y, rank } => b.rank_insignia(*x, *y, *rank),
            MenuCall::Quad { corners } => {
                let verts = corners
                    .iter()
                    .map(|&(x, y, rgba)| Vertex { x, y, u: 0.0, v: 0.0, rgba })
                    .collect();
                b.gradient(verts);
            }
            MenuCall::DarkenRect { x, y, w, h, alpha } => {
                let c = [255, 255, 255, *alpha];
                let (x0, y0, x1, y1) = (*x as f32, *y as f32, (x + w) as f32, (y + h) as f32);
                b.prims.push(Primitive {
                    element: Element::Banner,
                    kind: PrimKind::TriStrip,
                    texture: TextureSource::None,
                    target: Target::Frame,
                    blend: Some(2),
                    alpha_test: false,
                    bilinear: false,
                    gouraud: false,
                    z: HUD_Z,
                    verts: [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
                        .iter()
                        .map(|&(x, y)| Vertex { x, y, u: 0.0, v: 0.0, rgba: c })
                        .collect(),
                });
            }
            MenuCall::GoldTint { x, y, size, flags, text, rgba } => {
                b.gold_rgba = *rgba;
                b.gold_string(*x, *y, *size, *flags, text);
                b.gold_rgba = GREY;
            }
            MenuCall::Attribute { icon, x, y } => {
                if let Some(d) = b.t.attribute_icons.get(*icon).copied() {
                    b.atlas(d, *x, *y, Atlas::bright(true));
                }
            }
            MenuCall::InsigniaEx { x, y, rank, small, mirror } => b.rank_insignia_ex(*x, *y, *rank, *small, *mirror),
            MenuCall::Line { x0, y0, x1, rgba } => {
                let (a, c, yy) = (*x0 as f32, *x1 as f32, *y0 as f32);
                b.gradient(
                    [(a, yy), (c, yy), (a, yy + 1.0), (c, yy + 1.0)]
                        .iter()
                        .map(|&(x, y)| Vertex { x, y, u: 0.0, v: 0.0, rgba: *rgba })
                        .collect(),
                );
            }
            MenuCall::TextTint { line, message, x, y, sx, sy, rgba } => {
                let Some(codes) = b.t.message(*message) else {
                    b.unsupported(UnsupportedKind::MissingMessage(*message));
                    continue;
                };
                let (lines, w) = TextCanvas::measure(codes);
                let at = b.prims.len();
                b.text_lines(*line, i32::from(*message), *x, *y, w as i32, *sx, *sy, lines as i32);
                for p in &mut b.prims[at..] {
                    for v in &mut p.verts {
                        v.rgba = *rgba;
                    }
                }
            }
            MenuCall::SpriteOverlay { desc, x, y, mode, rgba } => {
                if let Some(d) = b.desc(*desc as i64) {
                    b.atlas(d, *x, *y, Atlas::bright(true));
                    let (w, h) = (d.width(), d.height());
                    b.sprite([x * PX, y * PX, (x + w) * PX, (y + h) * PX], [0; 4], *rgba, TextureSource::None, Some(*mode), true);
                }
            }
            MenuCall::BoxFilled { colour, x, y, wu, hu, fill } => {
                b.box_frame_filled(*colour, *x, *y, *wu, *hu, *fill)
            }
            MenuCall::SpriteDim { desc, x, y } => {
                if let Some(d) = b.desc(*desc as i64) {
                    b.atlas(d, *x, *y, Atlas::bright(false));
                }
            }
            MenuCall::TerrainPicture { colour, terrain, x, y } => {
                b.element = Element::TerrainPicture;
                b.terrain_picture(*colour, *terrain, *x, *y);
                b.element = Element::Banner;
            }
            MenuCall::TerrainBox { terrain, x, y } => b.terrain_box(*terrain, *x, *y),
            MenuCall::Legend { side, set, x, y } => b.legend_rows(*side, Some(*set), *x, *y),
            MenuCall::Picture { x, y, w, h } => b.sprite(
                [x * PX, y * PX, (x + w) * PX, (y + h) * PX],
                [HALF, HALF, 128 * PX + HALF, 128 * PX + HALF],
                GREY,
                TextureSource::Picture,
                None,
                false,
            ),
            MenuCall::Darken { alpha } => {
                let c = [255, 255, 255, *alpha];
                let s = 512.0;
                b.prims.push(Primitive {
                    element: Element::Banner,
                    kind: PrimKind::TriStrip,
                    texture: TextureSource::None,
                    target: Target::Frame,
                    blend: Some(2),
                    alpha_test: false,
                    bilinear: false,
                    gouraud: false,
                    z: HUD_Z,
                    verts: [(0.0, 0.0), (s, 0.0), (0.0, s), (s, s)]
                        .iter()
                        .map(|&(x, y)| Vertex { x, y, u: 0.0, v: 0.0, rgba: c })
                        .collect(),
                });
            }
        }
    }
    HudFrame { prims: b.prims, unsupported: b.unsupported, next: HudUpdate::default() }
}

/// Coordinates in 1/16 pixel (the GS 12.4 / 10.4 fixed-point units).
type Fx = i32;
const PX: Fx = 16;
const HALF: Fx = 8;

fn vx(x: Fx, y: Fx, u: Fx, v: Fx, rgba: [u8; 4]) -> Vertex {
    Vertex {
        x: x as f32 / 16.0,
        y: y as f32 / 16.0,
        u: u as f32 / 16.0,
        v: v as f32 / 16.0,
        rgba,
    }
}

/// Where a card box evaluates its unit's values.
#[derive(Clone, Copy)]
enum Eval {
    Cursor,
    OwnCell,
}

struct Builder<'a> {
    t: &'a HudTables,
    st: &'a HudState,
    prims: Vec<Primitive>,
    unsupported: Vec<Unsupported>,
    element: Element,
    z: u32,
    /// The ALPHA mode of the panel gradient being drawn (0x1B5360).
    gradient_blend: Option<u8>,
    /// 0x1B7B50's colour argument (t3; 0 = 0x2BDD80, grey 128).
    gold_rgba: [u8; 4],
}

const GREY: [u8; 4] = [128, 128, 128, 128];
const WHITE: [u8; 4] = [255, 255, 255, 128];

impl Builder<'_> {
    fn unsupported(&mut self, kind: UnsupportedKind) {
        self.unsupported.push(Unsupported {
            element: self.element,
            at: self.prims.len(),
            kind,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        kind: PrimKind,
        texture: TextureSource,
        blend: Option<u8>,
        alpha_test: bool,
        verts: Vec<Vertex>,
    ) {
        let bilinear = matches!(
            texture,
            TextureSource::TextCanvas { .. } | TextureSource::RenderTarget
        );
        self.prims.push(Primitive {
            element: self.element,
            kind,
            texture,
            target: Target::Frame,
            blend,
            alpha_test,
            bilinear,
            gouraud: false,
            z: self.z,
            verts,
        });
    }

    /// Flat sprite (x0, y0)-(x1, y1).
    #[allow(clippy::too_many_arguments)]
    fn sprite(
        &mut self,
        xy: [Fx; 4],
        uv: [Fx; 4],
        rgba: [u8; 4],
        texture: TextureSource,
        blend: Option<u8>,
        alpha_test: bool,
    ) {
        let [x0, y0, x1, y1] = xy;
        let [u0, v0, u1, v1] = uv;
        self.push(
            PrimKind::Sprite,
            texture,
            blend,
            alpha_test,
            vec![vx(x0, y0, u0, v0, rgba), vx(x1, y1, u1, v1, rgba)],
        );
    }

    /// Descriptor `i`, or an unsupported record when it is past the table.
    fn desc(&mut self, i: i64) -> Option<Descriptor> {
        let d = usize::try_from(i)
            .ok()
            .and_then(|i| self.t.sprites.get(i).copied());
        if d.is_none() {
            self.unsupported(UnsupportedKind::Descriptor(i.max(0) as usize));
        }
        d
    }

    fn slot(&mut self, d: &Descriptor) -> TextureSource {
        if !matches!(d.slot, 6 | 7) {
            self.unsupported(UnsupportedKind::TextureSlot(d.slot));
        }
        TextureSource::Atlas(d.slot)
    }

    /// Textured atlas sprite of the standard HUD state.
    fn atlas_sprite(&mut self, d: &Descriptor, xy: [Fx; 4], uv: [Fx; 4], rgba: [u8; 4]) {
        let tex = self.slot(d);
        self.sprite(xy, uv, rgba, tex, Some(0), true);
    }

    fn run(&mut self) -> HudUpdate {
        let st = self.st;
        let mut next = HudUpdate {
            d3c: st.d3c,
            d3d: st.d3d,
            d3e: st.d3e,
            d3f: st.d3f,
            d40: st.d40,
            d41: st.d41,
            d42: st.d42,
            d43: st.d43,
            d67: st.d67,
        };
        if st.d67 & 2 != 0 {
            return next;
        }
        let (mut step, mut show) = (8, st.d67 & 1 != 0);
        if (st.d67 & 4 != 0 || st.d45 != 0 || st.d47 != 0 || st.d48 != 0) && st.d65 & 0x10 == 0 {
            show = true;
            step = 16;
        }
        if show {
            next.d3e = (i32::from(st.d3e) - step).max(0) as u8;
        } else {
            next.d3e = (i32::from(st.d3e) + step).min(128) as u8;
            if next.d3e == 128 {
                next.d3c = st.d54;
                next.d3d = st.d53;
                next.d67 = (st.d67 & !1) | (st.d66 >> 7);
                let cursor = st.sides[usize::from(st.d54 & 1)].cursor;
                (next.d40, next.d41) = (cursor[0], cursor[1]);
            }
        }
        if next.d3e == 128 {
            return next;
        }
        let s = i32::from(next.d3e);
        let d3c = st.d3c;
        if d3c > 1 {
            self.unsupported(UnsupportedKind::TurnSide(d3c));
            return next;
        }
        let cursor = st.sides[usize::from(d3c & 1)].cursor;
        let terrain = terrain_at(st, i32::from(cursor[0]), i32::from(cursor[1]));
        let (near, far) = if d3c == 0 { (410, 6) } else { (6, 410) };

        self.element = Element::TerrainPicture;
        self.terrain_picture(usize::from(d3c), terrain, near, 400 + s);
        for side in 0..2u8 {
            self.element = Element::LpPanel(side);
            self.lp_panel(side, if side == 0 { 18 } else { 312 }, 20 - s, d3c == side);
        }
        self.element = Element::CardBox;
        self.card_boxes(&mut next, s, cursor);
        self.element = Element::LegendBox;
        self.legend(d3c, far, 400 + s);
        self.element = Element::TerrainBox;
        self.terrain_box(terrain, near, 400 + s);
        self.element = Element::RoundBox;
        self.panel(5, 3, 4, 228, 16 - s, 4, 2, true);
        self.element = Element::RoundNumber;
        self.gold_number(256, 22 - s, 2, 0x22, 2, 100 - i32::from(st.d3d));
        // Online (a port addition): the spectating note under the round
        // box, between the LP plates (no original element is drawn there).
        #[cfg(feature = "online")]
        {
            let mut y = 62 - s;
            for line in &st.notes {
                let text: Vec<u8> = line.iter().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { b' ' }).collect();
                let (_, lh) = self.gold_measure(1, 1, &text);
                self.gold_string(256, y, 1, 3, &text);
                y += lh;
            }
        }
        if st.d44 != 0 {
            self.element = Element::Banner;
            self.unsupported(UnsupportedKind::Banner { alpha: st.d44 });
        }
        if st.d48 != 0 {
            let y = 106 - s + ((i32::from(st.d45) * 3) << 6 >> 7) - 192;
            self.element = Element::HandBoxLeft;
            self.hand_box(156, y, false);
            self.element = Element::HandBoxRight;
            self.hand_box(268, y, true);
        }
        self.element = Element::HandOverlay;
        self.z = OVERLAY_Z;
        self.hand_overlay();
        self.z = HUD_Z;
        next
    }

    // ---- 0x1B0120 state 0x13: the message box (slot 3) -------------------
    fn message_box(&mut self) {
        let Some(m) = self.st.message_box else {
            return;
        };
        self.element = Element::MessageBox;
        self.z = WINDOW_Z;
        let Some(codes) = self.t.message(m.text) else {
            self.unsupported(UnsupportedKind::MissingInput("message box text"));
            return;
        };
        let (lines, w) = TextCanvas::measure(codes);
        let (w, h) = (w as i32, 22 * lines as i32);
        // 0x1B1830: text wider than 456 px is squeezed into a 480 box.
        let (s, bw) = if w >= 457 { (456.0 / w as f32, 480) } else { (1.0, w + 24) };
        let (cx, top) = (256 + i32::from(m.x), 256 + i32::from(m.y));
        self.panel(13, -1, 1, cx - bw / 2 - 4, top, -bw, -(h + 24), true);
        self.text_line_h(1, i32::from(m.text), cx - (w as f32 * s / 2.0) as i32, top + 16, w, s, lines as i32);
        self.z = HUD_Z;
    }

    // ---- 0x1B0120 states 0x14 / 0x15 / 0x16: kinds 5, 6, 7 ---------------
    fn overlay_box(&mut self, b: &OverlayBox, yes_no: &YesNoTables, counter: u32) {
        let Some(codes) = self.t.message(b.text) else {
            self.unsupported(UnsupportedKind::MissingInput("overlay box text"));
            return;
        };
        let (lines, w) = TextCanvas::measure(codes);
        let (w, h) = (w as i32, 22 * lines as i32);
        // 0x1B1830 without title, card or portrait: text wider than 456 px
        // is squeezed into 480.
        let (s, text_w) = if w >= 457 { (456.0 / w as f32, 480) } else { (1.0, w + 24) };
        // Callers: the minimum height and the top centring it on 256 + y;
        // kind 7 passes x 0x100 and width 0x15A (a minimum) [F 0x1B1570..].
        let (min_h, cx, width) = match b.kind {
            OverlayKind::Centred => (h + 0x18, 256 + i32::from(b.x), text_w),
            OverlayKind::Press => (h + 0x34, 256 + i32::from(b.x), text_w),
            OverlayKind::YesNo { .. } => (h + 0x3C, 256, text_w.max(0x15A)),
        };
        let top = 256 - (min_h >> 1) + i32::from(b.y);
        let inner_h = min_h.max(h + 0x18 + if b.kind == OverlayKind::Press { 0x1C } else { 0 });
        self.panel(13, -1, 1, cx - width / 2 - 4, top, -width, -inner_h, true);
        self.text_line_h(1, i32::from(b.text), cx - (w as f32 * s / 2.0) as i32, top + 16, w, s, lines as i32);
        match b.kind {
            OverlayKind::Centred => {}
            OverlayKind::Press => {
                let cel = PRESS_ICON + ((counter / 5) % 3) as usize;
                if let Some(&d) = self.t.sprites.get(cel) {
                    self.atlas(d, cx - 8, top + inner_h - 0x1C, Atlas::bright(true));
                }
            }
            OverlayKind::YesNo { yes } => {
                // 0x1B7B50(size 2, flags 3 = proportional, centred) at
                // x 0xAA / 0x156, y = (h >> 1) + 0xDC + y; the unselected
                // word in the 0x2BDCC4 colour [F 0x1B171C..0x1B17A0].
                let y = (min_h >> 1) + 0xDC + i32::from(b.y);
                let dim = yes_no.dim;
                self.gold_rgba = if yes { GREY } else { dim };
                self.gold_string(0xAA, y, 2, 3, &yes_no.yes);
                self.gold_rgba = if yes { dim } else { GREY };
                self.gold_string(0x156, y, 2, 3, &yes_no.no);
                self.gold_rgba = GREY;
            }
        }
    }

    // ---- 0x1B0120 slot 0 code 7 = 0x1B02CC: the OPTION panel --------------
    // fp = 0xC9C (raw); 0x1B1830(D54, title "OPTION") centred on x 0x100,
    // top fp + 0xB4, width 0x1BC, height 0x98; per row (y fp + 0xF9 + 40·r)
    // the heading at x 0x34 (squeezed to 200 px) and the two choices at
    // x 0x110 / 0x174 (squeezed to 80 px), the unselected one in the
    // 0x2BDCC4 colour (0x246B90 t0); text slots 4..9; then the row cursor
    // 0x1B5360(-1, -1, 7, 0x34, y, -400, -22) [F].
    fn option_panel(&mut self, b: &OptionPanel, dim: [u8; 4]) {
        let fp = i32::from(b.y);
        let title = self.t.option_title.clone();
        self.titled_box(i32::from(b.side), &title, 0x100, fp + 0xB4, 0x1BC, 0x98);
        let top = fp + 0xF9;
        // 0x2BDCC4 is the vertex colour of the unselected word (the port's
        // canvas texels are full white, so 0x40 shows them at half).
        let dim = [dim[0], dim[1], dim[2], WHITE[3]];
        for (r, (heading, first, second, bit)) in OPTION_ROWS.iter().enumerate() {
            let y = top + 40 * r as i32;
            // The original's slots 4..9; the port's canvas composes lines
            // 1..3 per sprite, so each row reuses them.
            let line = 1;
            self.text_fit_rgba(line, i32::from(*heading), 0x34, y, 200, WHITE);
            let on = b.bits & bit != 0;
            self.text_fit_rgba(line + 1, i32::from(*first), 0x110, y, 80, if on { WHITE } else { dim });
            self.text_fit_rgba(line + 2, i32::from(*second), 0x174, y, 80, if on { dim } else { WHITE });
        }
        self.panel(-1, -1, 7, 0x34, top + 40 * i32::from(b.row), -400, -22, true);
    }

    /// `text_fit` with 0x246B90's colour (t0).
    fn text_fit_rgba(&mut self, line: u8, message: i32, x: i32, y: i32, limit: i32, rgba: [u8; 4]) {
        let Some(w) = self.width(message) else {
            return;
        };
        let sx = if w > limit { limit as f32 / w as f32 } else { 1.0 };
        let dw = (w as f32 * sx) as i32;
        let v = 22 * i32::from(line);
        self.sprite(
            [x * PX, y * PX, (x + dw) * PX, (y + 22) * PX],
            [
                HALF & 0x3FFF,
                (v * PX + HALF) & 0x3FFF,
                (w * PX + HALF) & 0x3FFF,
                ((v + 22) * PX + HALF) & 0x3FFF,
            ],
            rgba,
            TextureSource::TextCanvas {
                line,
                message: u16::try_from(message).ok(),
            },
            Some(0),
            true,
        );
    }

    // ---- 0x1B0120 slot 0 code 0x0E = 0x1B0CFC: the GRAVEYARD panel ---------
    fn graveyard_panel(&mut self) {
        let st = self.st;
        let Some(g) = st.graveyard.as_ref() else {
            return;
        };
        self.element = Element::GraveyardPanel;
        self.z = WINDOW_Z;
        let shown = g.shown;
        if shown > 1 {
            self.unsupported(UnsupportedKind::GraveyardList(shown));
            self.z = HUD_Z;
            return;
        }
        // 0x1DF760(side) = S+0x33C: ENEMY plates for a CPU (< 100).
        let cpu = |s: usize| i64::from(st.sides[s].controller < 100);
        // fp = 0x200 - (c9c << 9 >> 6) (lh, 32-bit shifts).
        let fp = 0x200 - (i32::from(g.slide).wrapping_shl(9) >> 6);
        let list = usize::from(shown);
        let (offset, count, row) = (
            i32::from(g.offsets[list]),
            g.lists[list].len() as i32,
            i32::from(g.rows[list]),
        );
        let title = self.t.graveyard_title.clone();
        self.titled_box(2, &title, 0x100, fp + 0x88, 0x15A, 0xF0);
        let on = Atlas::bright(true);
        // Tabs: list 0 = side 0's graveyard (desc 0xB9 PLAYER / 0xBA ENEMY,
        // red plates), list 1 = side 1's (0xBB / 0xBC, white plates).
        let tab = |this: &mut Self, l: usize, x: i32, a: Atlas| {
            if let Some(d) = this.desc(0xB9 + 2 * l as i64 + cpu(l)) {
                match st.plate_name(l) {
                    Some(name) => this.name_plate(d, &name, x, fp + 0xBE),
                    None => this.atlas(d, x, fp + 0xBE, a),
                }
            }
        };
        // The shown tab flashes (overlay ALPHA mode 1); the other is dim.
        let shown_tab = Atlas { overlay: 1, ..on };
        let other_tab = Atlas::bright(false);
        if g.locked {
            tab(self, list, 0xE8, shown_tab);
        } else {
            tab(self, 0, 0xB8, if shown == 0 { shown_tab } else { other_tab });
            for (desc, x) in [(0x10D, 0xF1), (0x10C, 0x104)] {
                if let Some(d) = self.desc(desc) {
                    self.atlas(d, x, fp + 0xC3, on);
                }
            }
            tab(self, 1, 0x118, if shown == 1 { shown_tab } else { other_tab });
        }
        // L1 marker (upper half of the red diamond + "L1").
        for (desc, x, y) in [(0xC4, 0x167, 0xC2), (0x104, 0x18B, 0xC7)] {
            if let Some(d) = self.desc(desc) {
                self.atlas(d, x, fp + y, on);
            }
        }
        let colour = 14 + i32::from(shown);
        self.box_frame_filled(colour, 0x74, fp + 0xD8, -0x118, -0x7E, colour);
        // R1 marker (lower half + "R1").
        for (desc, x, y) in [(0xC5, 0x167, 0x15C), (0x105, 0x18B, 0x15C)] {
            if let Some(d) = self.desc(desc) {
                self.atlas(d, x, fp + y, on);
            }
        }
        // Rows: type icon + card name squeezed to 250 px.
        let mut y = fp + 0xDB;
        for r in 0..5 {
            if offset + r >= count {
                break;
            }
            let entry = g.lists[list][(offset + r) as usize];
            if let Some(kind) = entry.kind {
                let class = kind >> 5;
                let icon = if class == 0 {
                    i64::from(kind & 0x1F)
                } else {
                    i64::from(class) + 0x14
                };
                if let Some(d) = self.desc(icon) {
                    self.atlas(d, 0x77, y, on);
                }
                if entry.card < CARD_NAMES {
                    self.text_fit(1, i32::from(entry.card + CARD_NAME_MESSAGE), 0x8F, y, 250);
                } else {
                    self.unsupported(UnsupportedKind::CardName(entry.card));
                }
            }
            y += 24;
        }
        if count != 0 {
            self.panel(-1, -1, 7, 0x77, fp + 0xDB + 24 * row, -0x112, -0x18, true);
        }
        self.z = HUD_Z;
    }

    // ---- 0x1B1830 with a title and no text, card or icon --------------------
    /// The window box: panel type 1 (colour 13) `width` wide (at least the
    /// title plate + 0x18), at least `min_height` tall, centred on `cx`;
    /// the title plate (panel type 11 in `colour`) at top + 0xC with the
    /// title in the proportional gold font (size 2) centred at top + 0x14.
    fn titled_box(&mut self, colour: i32, title: &[u8], cx: i32, top: i32, width: i32, min_height: i32) {
        let (plate_w, plate_h) = if title.is_empty() {
            (0, 0)
        } else {
            let (w, h) = self.gold_measure(2, 3, title);
            (w + 0x18, h + 0xC)
        };
        let content_h = if title.is_empty() { 0xC } else { plate_h + 0x18 };
        let width = width.abs().max(plate_w + 0x18);
        let height = min_height.max(content_h);
        self.panel(13, -1, 1, cx - (width >> 1) - 4, top, -width, -height, true);
        if !title.is_empty() {
            self.panel(colour, -1, 11, cx - (plate_w >> 1), top + 0xC, -plate_w, -plate_h, true);
            self.gold_string(cx, top + 0x14, 2, 3, title);
        }
    }

    // ---- 0x1B4370: terrain picture ---------------------------------------
    fn terrain_picture(&mut self, colour: usize, terrain: i32, x: i32, y: i32) {
        let c = self.t.colours[colour];
        self.sprite(
            [(x + 2) * PX, (y + 2) * PX, (x + 94) * PX, (y + 64) * PX],
            [0; 4],
            [c[6], c[5], c[4], 128],
            TextureSource::None,
            None,
            false,
        );
        if !self.st.terrain_model_ready {
            return;
        }
        // Clear RT 0x2000 to 0 (the whole 4096-unit window).
        self.prims.push(Primitive {
            element: self.element,
            kind: PrimKind::Sprite,
            texture: TextureSource::None,
            target: Target::RenderTarget,
            blend: None,
            alpha_test: false,
            bilinear: false,
            gouraud: false,
            z: 0,
            verts: vec![
                vx(-1792 * PX, -1792 * PX, 0, 0, [0; 4]),
                vx(2303 * PX, 2303 * PX, 0, 0, [0; 4]),
            ],
        });
        self.unsupported(UnsupportedKind::TerrainTileRender { terrain });
        self.sprite(
            [
                (x + 2) * PX + HALF,
                (y + 2) * PX + HALF,
                (x + 92) * PX + HALF,
                (y + 63) * PX + HALF,
            ],
            [0, 0, 512 * PX, 512 * PX],
            WHITE,
            TextureSource::RenderTarget,
            Some(0),
            true,
        );
    }

    // ---- 0x1B2590: LP panel ------------------------------------------------
    fn lp_panel(&mut self, side: u8, x: i32, y: i32, active: bool) {
        let st = self.st;
        let s = st.sides[usize::from(side)];
        let ctl = [st.sides[0].controller, st.sides[1].controller];
        let hide = counts_hidden(ctl, st.d3c, side, st.own_side);
        let side_i = i32::from(side);
        self.panel(side_i, side_i, 0, x - 13, y - 15, 37, 13, true);
        let px = x + 3 + if side == 0 { 127 } else { 1 };
        self.portrait(side, px, y + 4, 48, 48, active);
        self.panel(-1, -1, 4, px, y + 4, 3, 6, active);
        let bx = x + 3 + 48 * side_i;
        self.box_frame(side_i, bx + 4, y + 9, 12, 1);
        self.lp_digits(bx + 38, y + 13, 4, i32::from(s.lp), false);
        let label = 185 + 2 * usize::from(side) + usize::from(s.controller < 100);
        match st.plate_name(usize::from(side)) {
            Some(name) => self.name_plate(self.t.sprites[label], &name, px, y + 51),
            None => self.atlas(self.t.sprites[label], px, y + 51, Atlas::bright(active)),
        }
        self.atlas(self.t.sprites[246], bx + 8, y + 15, Atlas::bright(true));
        self.atlas(self.t.sprites[215], bx + 8, y + 39, Atlas::bright(true));
        if hide {
            self.gold_string(bx + 26, y + 40, 0, 0, b"??");
        } else {
            self.gold_number(bx + 26, y + 40, 0, 0, 2, i32::from(s.stars));
        }
        self.atlas(self.t.sprites[205], bx + 64, y + 39, Atlas::bright(true));
        self.gold_number(bx + 82, y + 40, 0, 0, 2, i32::from(s.deck));
        self.atlas(self.t.sprites[225], bx + 8, y + 55, Atlas::bright(true));
        if hide {
            self.gold_string(bx + 26, y + 56, 0, 0, b"??");
        } else {
            self.gold_number(bx + 26, y + 56, 0, 0, 2, i32::from(s.monsters));
        }
        self.atlas(self.t.sprites[226], bx + 64, y + 55, Atlas::bright(true));
        if hide {
            self.gold_string(bx + 82, y + 56, 0, 0, b"??");
        } else {
            self.gold_number(bx + 82, y + 56, 0, 0, 2, i32::from(s.magic_traps));
        }
    }

    // ---- card-box selection in 0x1B1D50 -------------------------------------
    fn card_boxes(&mut self, next: &mut HudUpdate, s: i32, cursor: [u8; 2]) {
        let st = self.st;
        let unit = |i: Option<usize>| i.map(|i| &st.units[i]);
        let side_of = |i: Option<usize>| unit(i).map_or(st.d3c, |u| u.side);
        let mut d3f = i32::from(st.d3f);
        if st.d45 != 0 || st.d47 != 0 {
            if st.d67 & 8 != 0 || (st.d67 & 0x10 == 0 && st.d47 != 0) {
                d3f = (d3f - 16).max(0);
            } else {
                d3f = (d3f + 16).min(128);
            }
            next.d3f = d3f as u8;
            let d57 = st.d57;
            let b = if (81..=83).contains(&d57) || (st.d35 == 83 && (32..=34).contains(&d57)) {
                st.unit_a
            } else {
                st.unit_b
            };
            let mut deco = true;
            if d3f != 0 {
                let y = 432 - ((d3f * 64) >> 7) + s;
                self.card_box(
                    st.hand_card,
                    Eval::Cursor,
                    side_of(st.hand_card),
                    104,
                    y,
                    -1,
                    6,
                    true,
                );
                deco = false;
            }
            self.card_box(b, Eval::Cursor, side_of(b), 104, 432 + s, -1, 3, deco);
            return;
        }
        let valid = |i: Option<usize>| unit(i).filter(|u| u.slot < 11);
        let (mut mode_main, mut mode_upper) = (1, 1);
        if let (Some(a), Some(b)) = (valid(st.unit_a), valid(st.unit_b)) {
            if a.controller != b.controller {
                match st.advantage {
                    Some(r) => {
                        mode_main = r;
                        mode_upper = if r < 0 { r } else { 2 - r };
                    }
                    None => self.unsupported(UnsupportedKind::MissingInput("advantage")),
                }
            }
        }
        let (mut d40, mut d41, mut d42, mut d43) =
            (st.d40, st.d41, i32::from(st.d42), i32::from(st.d43));
        let mut candidate = (-1, -1);
        match valid(st.unit_a) {
            Some(a) if a.flags & 0x40 != 0 => {
                (d40, d41) = (a.cell[0], a.cell[1]);
                if cursor != a.cell {
                    candidate = (i32::from(cursor[0]), i32::from(cursor[1]));
                    if d3f == 0 {
                        (d42, d43) = candidate;
                    }
                }
            }
            _ => {
                if matches!(st.d57, 0x18 | 0x3E | 0x30) {
                    (d40, d41) = (cursor[0], cursor[1]);
                }
            }
        }
        if (d42, d43) == candidate && unit_at(st, candidate.0, candidate.1).is_some() {
            d3f = (d3f + 16).min(128);
        } else {
            d3f = (d3f - 16).max(0);
        }
        (next.d40, next.d41, next.d42, next.d43, next.d3f) =
            (d40, d41, d42 as u8, d43 as u8, d3f as u8);
        let mut deco = true;
        if d3f != 0 {
            let u = unit_at(st, d42, d43);
            let y = 432 - ((d3f * 64) >> 7) + s;
            self.card_box(u, Eval::OwnCell, side_of(u), 104, y, mode_upper, 3, true);
            deco = false;
        }
        let u = unit_at(st, i32::from(d40), i32::from(d41));
        self.card_box(
            u,
            Eval::Cursor,
            side_of(u),
            104,
            432 + s,
            mode_main,
            3,
            deco,
        );
    }

    // ---- 0x1B2EC0: card box ---------------------------------------------
    #[allow(clippy::too_many_arguments)]
    fn card_box(
        &mut self,
        unit: Option<usize>,
        eval: Eval,
        colour: u8,
        x: i32,
        y: i32,
        mode: i32,
        ptype: usize,
        deco: bool,
    ) {
        let st = self.st;
        let c = i32::from(colour);
        self.panel(c, if deco { 7 } else { -1 }, ptype, x, y, -300, 10, true);
        let Some(u) = unit.map(|i| st.units[i]) else {
            return;
        };
        if !u.has_card {
            return;
        }
        let (class, race) = (u.kind >> 5, u.kind & 31);
        let attr = match class {
            0 => usize::from(u.attribute),
            1 | 2 => 6,
            3 | 4 => 7,
            5 => 8,
            _ => {
                self.unsupported(UnsupportedKind::CardClass(class));
                return;
            }
        };
        let icon = if class == 0 {
            usize::from(race)
        } else {
            usize::from(class) + 20
        };
        let hidden = st.option_30 && u.slot > 0 && u.face_down_hidden;
        let lines: [i32; 3] = if hidden {
            [29, 30, 82]
        } else {
            if u.card >= CARD_NAMES {
                self.unsupported(UnsupportedKind::CardName(u.card));
                return;
            }
            [
                i32::from(u.card + CARD_NAME_MESSAGE),
                if class == 0 {
                    i32::from(race) + 38
                } else {
                    i32::from(class) + 31
                },
                u.rank + 83,
            ]
        };
        self.box_frame(c, x + 5, y + 4, -152, 1);
        self.text_fit(1, lines[0], x + 10, y + 6, 140);
        self.atlas(
            self.t.sprites[icon],
            x + 6,
            y + 35,
            Atlas {
                black: hidden,
                draw: !hidden,
                ring: hidden,
                ..Atlas::bright(true)
            },
        );
        self.box_frame(c, x + 32, y + 33, -56, 1);
        self.text_fit(2, lines[1], x + 37, y + 35, 44);
        let overlay = match mode {
            2 => 1,
            0 => 2,
            _ => -1,
        };
        let Some(&attribute_icon) = self.t.attribute_icons.get(attr) else {
            self.unsupported(UnsupportedKind::AttributeIcon(u.attribute));
            return;
        };
        self.atlas(
            attribute_icon,
            x + 91,
            y + 35,
            Atlas {
                black: true,
                draw: !hidden,
                ring: true,
                overlay,
                ..Atlas::bright(true)
            },
        );
        let stats = match eval {
            Eval::Cursor => u.at_cursor,
            Eval::OwnCell => u.at_own_cell,
        };
        if class == 0 {
            if u.slot == 0 {
                self.rank_insignia(x + 159, y + 6, u.rank);
                self.box_frame(c, x + 185, y + 4, -56, 1);
                self.text_fit(3, lines[2], x + 188, y + 6, 48);
                if st.d3c == u.side {
                    let owner = st.sides[usize::from(u.side & 1)];
                    let sc = st.sides[usize::from(colour & 1)];
                    let (deck, hand) = (i32::from(owner.deck), i32::from(owner.hand));
                    if sc.summon_flags & 1 != 0 && deck + hand > 0 {
                        let flash =
                            deck > 0 && sc.summon_counter > 0 && sc.summon_counter & 0x80 == 0;
                        self.atlas(
                            self.t.sprites[236],
                            x + 243,
                            y + 6,
                            Atlas {
                                overlay: if flash { 1 } else { -1 },
                                ..Atlas::bright(true)
                            },
                        );
                    }
                }
                let mut ax = x + 120;
                for a in 4..=16 {
                    if u.abilities & (1 << a) != 0 {
                        self.atlas(self.t.sprites[30 + a], ax, y + 35, Atlas::bright(true));
                        ax += 25;
                    }
                }
            } else if !hidden {
                let attack = stats.attack.clamp(0, 9999);
                let defence = stats.defence.clamp(0, 9999);
                if u.effect {
                    self.atlas(self.t.sprites[241], x + 120, y + 32, Atlas::bright(true));
                }
                self.atlas(self.t.sprites[234], x + 159, y + 6, Atlas::bright(true));
                self.box_frame(c, x + 185, y + 4, -56, 1);
                self.gold_number(x + 194, y + 12, 1, 0, 4, attack);
                self.atlas(self.t.sprites[235], x + 159, y + 35, Atlas::bright(true));
                self.box_frame(c, x + 185, y + 33, -56, 1);
                self.gold_number(x + 194, y + 41, 1, 0, 4, defence);
                if stats.terrain_effect != 0 {
                    if let Some(d) = self.desc(237 + i64::from(stats.terrain_effect)) {
                        self.atlas(d, x + 243, y + 6, Atlas::bright(true));
                    }
                }
            }
            if !hidden && (stats.allowance & 0xF) >= 2 {
                self.atlas(self.t.sprites[240], x + 272, y + 6, Atlas::bright(true));
            }
        }
        if (class != 0 || u.slot != 0) && u.flags2 & 4 == 0 {
            let n = if u.turns[0] == 0xFF || u.turns[1] == 0xFF {
                0x7FFF_FFFF
            } else {
                i32::from(u.turns[0].max(u.turns[1]))
            };
            if n != 0 {
                self.atlas(self.t.sprites[237], x + 243, y + 35, Atlas::bright(true));
                self.box_frame(c, x + 269, y + 33, 1, 1);
                self.lp_digits(x + 273, y + 35, 1, n, false);
            }
        }
    }

    // ---- 0x1B3CE0: legend -------------------------------------------------
    fn legend(&mut self, d3c: u8, x: i32, y: i32) {
        let st = self.st;
        self.panel(i32::from(d3c), 2, 3, x, y, 9, 18, true);
        let a = st.unit_a.map(|i| st.units[i]);
        let b = st.unit_b;
        let set: i32 = match st.d57 {
            0x63 => 20,
            0x30 => {
                if st.legend_1fb900 {
                    -1
                } else {
                    19
                }
            }
            0x2D => {
                if st.d66 & 8 != 0 {
                    16
                } else {
                    17
                }
            }
            0x25 | 0x29 => 18,
            0x46 => 9,
            0x53 => 17,
            0x43 => 8,
            0x3E => {
                if st.legend_1e3760 {
                    -1
                } else {
                    7
                }
            }
            0x1D => match a {
                None => {
                    self.unsupported(UnsupportedKind::LegendWithoutUnit);
                    -1
                }
                Some(a) if a.slot == 0 => match b {
                    Some(bi) if Some(bi) != st.unit_a => {
                        if st.units[bi].controller == d3c {
                            6
                        } else {
                            5
                        }
                    }
                    _ => 5,
                },
                Some(a) => match b {
                    None => 5,
                    Some(bi) if Some(bi) == st.unit_a => {
                        if a.flags & 0x10 != 0 {
                            13
                        } else {
                            12
                        }
                    }
                    Some(bi) => {
                        if st.units[bi].controller == d3c {
                            14
                        } else {
                            15
                        }
                    }
                },
            },
            0x18 => match a {
                None => 0,
                Some(a) if a.controller != d3c => {
                    if a.slot > 0 && a.legend_query {
                        0
                    } else {
                        11
                    }
                }
                Some(a) => {
                    let acted = a.flags & 1 != 0;
                    let side = st.sides[usize::from(d3c & 1)];
                    let (deck, hand) = (i32::from(side.deck), i32::from(side.hand));
                    if a.slot != 0 {
                        if acted {
                            11
                        } else {
                            10
                        }
                    } else if side.summon_flags & 1 != 0 && deck + hand > 0 {
                        if acted {
                            3
                        } else {
                            1
                        }
                    } else if hand != 0 {
                        if acted {
                            4
                        } else {
                            2
                        }
                    } else if acted {
                        11
                    } else {
                        10
                    }
                }
            },
            _ => -1,
        };
        let Ok(set) = usize::try_from(set) else {
            return;
        };
        self.legend_set_rows(set, x, y);
    }

    /// 0x1B3CE0 with its set already chosen (the map editor's d57 0x63).
    fn legend_rows(&mut self, d3c: u8, set: Option<usize>, x: i32, y: i32) {
        self.panel(i32::from(d3c), 2, 3, x, y, 9, 18, true);
        if let Some(set) = set {
            self.legend_set_rows(set, x, y);
        }
    }

    fn legend_set_rows(&mut self, set: usize, x: i32, y: i32) {
        let Some(rows) = self.t.legend_sets.get(set).copied() else { return };
        for (i, &(w0, w1)) in rows.iter().enumerate() {
            let row = y + 4 + 22 * i as i32;
            let icon = i64::from(w0 & 0x3FF);
            if icon < 1023 {
                let dx = i32::from((w0 >> 10) & 7);
                let dy = i32::from((w0 >> 13) & 7);
                if let Some(d) = self.desc(icon) {
                    self.atlas(d, x + 14 + dx, row + dy, Atlas::bright(true));
                }
            }
            if w1 < 0xFFFF {
                self.text_fit(1, i32::from(w1), x + 40, row, 44);
            }
        }
    }

    // ---- 0x1B4220: terrain box ---------------------------------------------
    fn terrain_box(&mut self, terrain: i32, x: i32, y: i32) {
        let message = terrain + 72;
        self.panel(-1, 2, 3, x, y, 9, 18, true);
        self.panel(5, -1, 3, x, y + 64, 9, 2, true);
        let Some(w) = self.width(message) else {
            return;
        };
        let sc = if w >= 85 { 84.0 / w as f32 } else { 1.0 };
        let tx = x + 48 - ((w >> 1) as f32 * sc) as i32;
        self.text_line(1, message, tx, y + 69, w, sc, 1.0);
    }

    // ---- 0x1B2A90 / 0x1B2CA0: hand boxes -----------------------------------
    fn hand_box(&mut self, x: i32, y: i32, right: bool) {
        let st = self.st;
        let d3c = usize::from(st.d3c & 1);
        let question = st.sides[d3c].controller < 32 && st.sides[d3c ^ 1].controller >= 100;
        if st.d54 > 1 {
            self.unsupported(UnsupportedKind::TurnSide(st.d54));
            return;
        }
        let d54 = i32::from(st.d54);
        self.panel(d54, -1, 6, x, y, 8, 3, true);
        let icon = if right { 236 } else { 33 };
        self.atlas(self.t.sprites[icon], x + 7, y + 6, Atlas::bright(true));
        self.box_frame(d54, x + 33, y + 4, 3, 1);
        if question {
            // Message 29 composed, its width forced to 24, drawn ×1.8333334.
            self.text_line(1, 29, x + 36, y + 6, 24, f32::from_bits(0x3FEA_AAAB), 1.0);
        } else {
            let value = st.hand_box_values[usize::from(right)];
            self.lp_digits(x + 36, y + 6, 2, value, false);
        }
    }

    // ---- 0x1AFF00: hand overlay --------------------------------------------
    fn hand_overlay(&mut self) {
        let st = self.st;
        if st.overlay_mode != 1 || st.d45 == 0 || st.d47 != 0 {
            return;
        }
        for (i, card) in st.hand_overlay.iter().enumerate() {
            let Some([fx, fy]) = card.position else {
                continue;
            };
            // 0x1A2B80 truncates toward zero (saturating).
            let x = (fx as i32).wrapping_add(208);
            let y = (fy as i32).wrapping_add(164);
            if card.word < 0 {
                continue;
            }
            if usize::from(st.d4f) == i {
                self.panel(-1, -1, 8, x, y - 2, -96, -194, true);
            }
            let order = (card.word & 0xF000) >> 12;
            if card.word & 0xFFF != 999 && order != 0 {
                // The badge sits on a black backing (golden-play 752-f1-mark).
                let badge = Atlas {
                    black: true,
                    ..Atlas::bright(true)
                };
                self.atlas(self.t.sprites[55], x, y - 28, badge);
                self.gold_number(x + 16, y - 23, 0, 2, 1, order);
            }
        }
    }

    // ---- 0x1B48A0 via 0x1B4860: atlas descriptor ---------------------------
    fn atlas(&mut self, d: Descriptor, x: i32, y: i32, a: Atlas) {
        let (mut w, mut h) = (d.width(), d.height());
        if a.small {
            w = (w * 3) >> 2;
            h = (h * 3) >> 2;
        }
        let xy = [x * PX, y * PX, (x + w) * PX, (y + h) * PX];
        if a.black {
            self.sprite(xy, [0; 4], [0, 0, 0, 128], TextureSource::None, None, false);
        }
        if a.draw {
            let c = if a.bright { 128 } else { 48 };
            let uv = [
                i32::from(d.u0) * PX + HALF,
                i32::from(d.v0) * PX + HALF,
                i32::from(d.u1) * PX + HALF,
                i32::from(d.v1) * PX + HALF,
            ];
            self.atlas_sprite(&d, xy, uv, [c, c, c, 128]);
        }
        if a.ring {
            let ring = self.t.sprites[242];
            self.atlas(
                ring,
                x,
                y,
                Atlas {
                    small: a.small,
                    bright: a.bright,
                    ..Atlas::bright(true)
                },
            );
        }
        if a.overlay >= 0 {
            let alpha = (self.st.flash >> 8) as u8;
            self.sprite(
                xy,
                [0; 4],
                [128, 128, 128, alpha],
                TextureSource::None,
                Some(a.overlay as u8),
                true,
            );
        }
    }

    // ---- 0x1B5360: panel -----------------------------------------------
    #[allow(clippy::too_many_arguments)]
    fn panel(
        &mut self,
        grad: i32,
        deco: i32,
        ptype: usize,
        x: i32,
        y: i32,
        wu: i32,
        hu: i32,
        bright: bool,
    ) {
        let t = self.t;
        let p: [Descriptor; 9] = std::array::from_fn(|k| t.sprites[ptype * 9 + 59 + k]);
        let m = t.margins[ptype].map(i32::from);
        let lead_w = p[0].width() - m[0];
        let trail_w = p[2].width() - m[1];
        let lead_h = p[0].height() - m[2];
        let trail_h = p[6].height() - m[3];
        let inner_w = if wu < 0 {
            -wu
        } else {
            lead_w + trail_w + wu * p[1].width()
        };
        let inner_h = if hu < 0 {
            -hu
        } else {
            lead_h + trail_h + hu * p[3].height()
        };
        if grad >= 0 {
            let (gx, gy) = (x + m[0], y + m[2]);
            let hh = inner_h >> 1;
            let (o, i) = (t.outer(grad as usize), t.inner(grad as usize));
            let g = |yy: i32, xx: i32, c| vx(xx * PX, yy * PX, 0, 0, c);
            // 0x1B5360: colour sets 12 / 13 draw subtractively, 14 and up
            // with the standard blend (alpha test on) [F, emu].
            let blend = match grad {
                12 | 13 => Some(2),
                g if g >= 14 => Some(0),
                _ => None,
            };
            self.gradient_blend = blend;
            self.gradient(vec![
                g(gy, gx, o),
                g(gy, gx + inner_w, o),
                g(gy + hh, gx, i),
                g(gy + hh, gx + inner_w, i),
            ]);
            self.gradient(vec![
                g(gy + hh, gx, i),
                g(gy + hh, gx + inner_w, i),
                g(gy + 2 * hh, gx, o),
                g(gy + 2 * hh, gx + inner_w, o),
            ]);
            self.gradient_blend = None;
        }
        let c = if bright { 128 } else { 48 };
        self.frame_pieces(
            &p,
            x,
            y,
            wu,
            hu,
            [lead_w, trail_w, lead_h, trail_h],
            [c, c, c, 128],
        );
        if deco < 0 {
            return;
        }
        let mut deco = deco;
        loop {
            let n = (deco & 15) as usize;
            let d = t.sprites[n + 47];
            let off = i32::from(t.deco_offsets[n]);
            match n {
                3 | 4 => {
                    let (x0, y0) = (x + m[0] - off, y + m[2]);
                    let (len, wd) = (d.width(), d.height());
                    let (u0, v0, u1, v1) = (
                        i32::from(d.u0) * PX,
                        i32::from(d.v0) * PX,
                        i32::from(d.u1) * PX,
                        i32::from(d.v1) * PX,
                    );
                    let tex = self.slot(&d);
                    let lx = x0 * PX + HALF;
                    let (top, bottom) = (y0 * PX + HALF, (y0 + len) * PX + HALF);
                    self.push(
                        PrimKind::TriStrip,
                        tex,
                        Some(0),
                        true,
                        vec![
                            vx(lx, bottom, u0, v0, GREY),
                            vx(lx, top, u1, v0, GREY),
                            vx(lx + wd * PX, bottom, u0, v1, GREY),
                            vx(lx + wd * PX, top, u1, v1, GREY),
                        ],
                    );
                    let rx = (x + m[0] + inner_w + off) * PX - PX - HALF;
                    self.push(
                        PrimKind::TriStrip,
                        tex,
                        Some(0),
                        true,
                        vec![
                            vx(rx, bottom, u0, v0, GREY),
                            vx(rx, top, u1, v0, GREY),
                            vx(rx - wd * PX, bottom, u0, v1, GREY),
                            vx(rx - wd * PX, top, u1, v1, GREY),
                        ],
                    );
                }
                5 | 6 => self.unsupported(UnsupportedKind::PanelDecoration(n as u8)),
                _ => {
                    let dx = x + m[0] + (inner_w >> 1) - (d.width() >> 1);
                    let dy = y + m[2] - off;
                    let uv = [
                        i32::from(d.u0) * PX + HALF,
                        i32::from(d.v0) * PX + HALF,
                        i32::from(d.u1) * PX + HALF,
                        i32::from(d.v1) * PX + HALF,
                    ];
                    self.atlas_sprite(
                        &d,
                        [
                            dx * PX,
                            dy * PX,
                            (dx + d.width()) * PX,
                            (dy + d.height()) * PX,
                        ],
                        uv,
                        GREY,
                    );
                }
            }
            deco >>= 4;
            if deco == 0 {
                break;
            }
        }
    }

    fn gradient(&mut self, verts: Vec<Vertex>) {
        self.prims.push(Primitive {
            element: self.element,
            kind: PrimKind::TriStrip,
            texture: TextureSource::None,
            target: Target::Frame,
            blend: self.gradient_blend,
            alpha_test: self.gradient_blend.is_some(),
            bilinear: false,
            gouraud: true,
            z: self.z,
            verts,
        });
    }

    /// The 3×3 frame of a panel or box without its centre piece; pieces sit
    /// half a pixel up-left with exact texel UVs.
    #[allow(clippy::too_many_arguments)]
    fn frame_pieces(
        &mut self,
        p: &[Descriptor; 9],
        x: i32,
        y: i32,
        wu: i32,
        hu: i32,
        lead_trail: [i32; 4],
        rgba: [u8; 4],
    ) {
        let [lead_w, trail_w, lead_h, trail_h] = lead_trail;
        let mid = |n: i32, unit: i32, lead: i32, trail: i32| match n {
            1.. => n * unit,
            0 => 0,
            _ => -n - lead - trail,
        };
        let mut row_y = 0;
        for r in 0..3 {
            let (mut col_x, mut rh) = (0, 0);
            for k in 0..3 {
                let d = p[r * 3 + k];
                let mut w = d.width();
                let mut h = d.height();
                if k == 1 {
                    w = mid(wu, d.width(), lead_w, trail_w);
                }
                if r == 1 {
                    h = mid(hu, d.height(), lead_h, trail_h);
                }
                if r * 3 + k != 4 && w > 0 && h > 0 {
                    let (x0, y0) = ((x + col_x) * PX - HALF, (y + row_y) * PX - HALF);
                    let uv = [
                        i32::from(d.u0) * PX,
                        i32::from(d.v0) * PX,
                        i32::from(d.u1) * PX,
                        i32::from(d.v1) * PX,
                    ];
                    self.atlas_sprite(&d, [x0, y0, x0 + w * PX, y0 + h * PX], uv, rgba);
                }
                col_x += w;
                rh = h;
            }
            row_y += rh;
        }
    }

    // ---- 0x1B71C0: tinted box (frame set 0, no fill) ------------------------
    fn box_frame(&mut self, colour: i32, x: i32, y: i32, wu: i32, hu: i32) {
        let t = self.t;
        let tint = t.inner(if colour < 0 { 2 } else { colour as usize });
        let p: [Descriptor; 9] = std::array::from_fn(|k| t.sprites[140 + k]);
        let lt = [p[0].width(), p[2].width(), p[0].height(), p[6].height()];
        self.frame_pieces(&p, x, y, wu, hu, lt, tint);
    }

    /// 0x1B71C0 with a fill colour (t2 >= 0): first an untextured sprite
    /// over the whole box (x, y)-(x + W, y + H) in the fill set's inner
    /// colour, blended by the set as the panel gradients (12/13 ALPHA mode
    /// 2, >= 14 mode 0, both alpha-tested; else opaque), then the frame.
    fn box_frame_filled(&mut self, colour: i32, x: i32, y: i32, wu: i32, hu: i32, fill: i32) {
        let t = self.t;
        let p: [Descriptor; 9] = std::array::from_fn(|k| t.sprites[140 + k]);
        let full = |n: i32, lead: i32, unit: i32, trail: i32| {
            if n < 0 {
                -n
            } else {
                lead + n * unit + trail
            }
        };
        let w = full(wu, p[0].width(), p[1].width(), p[2].width());
        let h = full(hu, p[0].height(), p[3].height(), p[6].height());
        let blend = match fill {
            12 | 13 => Some(2),
            f if f >= 14 => Some(0),
            _ => None,
        };
        self.sprite(
            [x * PX, y * PX, (x + w) * PX, (y + h) * PX],
            [0; 4],
            t.inner(fill as usize),
            TextureSource::None,
            blend,
            blend.is_some(),
        );
        self.box_frame(colour, x, y, wu, hu);
    }

    // ---- 0x1B8B70: portrait -------------------------------------------------
    fn portrait(&mut self, side: u8, x: i32, y: i32, w: i32, h: i32, bright: bool) {
        let c = if bright { 128 } else { 48 };
        self.sprite(
            [x * PX, y * PX, (x + w) * PX, (y + h) * PX],
            [HALF, HALF, 40 * PX + HALF, 32 * PX + HALF],
            [c, c, c, 128],
            TextureSource::Portrait(side),
            None,
            false,
        );
    }

    // ---- 0x1B5120: rank insignia (small, not mirrored) -----------------------
    fn rank_insignia(&mut self, x: i32, y: i32, rank: i32) {
        if rank <= 0 {
            return;
        }
        let step = 12;
        let small = Atlas {
            small: true,
            ..Atlas::bright(true)
        };
        let Some(bars) = self.desc(198 + i64::from((rank - 1) / 3)) else {
            return;
        };
        self.atlas(bars, x, y, small);
        let stars = (rank - 1) % 3 + 1;
        let star = self.t.sprites[202];
        if stars & 1 != 0 {
            self.atlas(star, x + step, y, small);
        }
        if stars & 2 != 0 {
            self.atlas(star, x, y, small);
            self.atlas(star, x + step, y + step, small);
        }
    }

    /// 0x1B5120 with its size and mirror flags.
    fn rank_insignia_ex(&mut self, x: i32, y: i32, rank: i32, small: bool, mirror: bool) {
        if rank <= 0 {
            return;
        }
        let step = if small { 12 } else { 16 };
        let a = Atlas { small, ..Atlas::bright(true) };
        let Some(bars) = self.desc(198 + i64::from((rank - 1) / 3)) else {
            return;
        };
        if mirror {
            let (mut w, mut h) = (bars.width(), bars.height());
            if small {
                w = (w * 3) >> 2;
                h = (h * 3) >> 2;
            }
            let xy = [x * PX, y * PX, (x + w) * PX, (y + h) * PX];
            let uv = [
                i32::from(bars.u1) * PX + HALF,
                i32::from(bars.v0) * PX + HALF,
                i32::from(bars.u0) * PX + HALF,
                i32::from(bars.v1) * PX + HALF,
            ];
            self.atlas_sprite(&bars, xy, uv, GREY);
        } else {
            self.atlas(bars, x, y, a);
        }
        let stars = (rank - 1) % 3 + 1;
        let star = self.t.sprites[202];
        let (near, far) = if mirror { (step, 0) } else { (0, step) };
        if stars & 1 != 0 {
            self.atlas(star, x + far, y, a);
        }
        if stars & 2 != 0 {
            self.atlas(star, x + near, y, a);
            self.atlas(star, x + far, y + step, a);
        }
    }

    // ---- 0x1B7870: digits from the canvas digit strip ----------------------
    fn lp_digits(&mut self, x: i32, y: i32, n: i32, value: i32, narrow: bool) {
        self.lp_digits_padded(x, y, n, value, narrow, false);
    }

    /// 0x1B7880 with t2 ≠ 0: all `n` digits, zero-padded [F 0x1B79D0].
    fn lp_digits_padded(&mut self, x: i32, y: i32, n: i32, value: i32, narrow: bool, pad: bool) {
        let pitch = if narrow { 16 } else { 20 };
        let cell = |this: &mut Self, k: i32, u: i32| {
            let px = x + pitch * (n - 1 - k);
            // The callback writes UV as 14-bit fields.
            let u0 = (u * PX + HALF) & 0x3FFF;
            let u1 = ((u + 22) * PX + HALF) & 0x3FFF;
            this.sprite(
                [px * PX, y * PX, (px + 22) * PX, (y + 22) * PX],
                [u0, HALF, u1, 22 * PX + HALF],
                WHITE,
                TextureSource::TextCanvas {
                    line: 0,
                    message: None,
                },
                Some(0),
                true,
            );
        };
        if value == 0x7FFF_FFFF {
            for k in 0..n {
                cell(self, k, 264);
            }
            return;
        }
        let (mut k, mut p) = (0, 1i32);
        while k < n {
            cell(self, k, 22 * ((value / p) % 10));
            p = p.wrapping_mul(10);
            k += 1;
            if !pad && value < p {
                break;
            }
        }
        if k < n && value < 0 {
            cell(self, k, 242);
        }
    }

    // ---- 0x1B7A80 / 0x1B7B50: gold font -------------------------------------
    fn gold_number(&mut self, x: i32, y: i32, size: usize, flags: u32, width: i32, value: i32) {
        let zero_pad = flags & 0x20 != 0;
        let mut text = vec![b' '; width.max(0) as usize];
        let (mut k, mut p) = (0, 1i32);
        while k < width {
            let digit = (value / p) % 10;
            text[(width - 1 - k) as usize] = (48 + digit) as u8;
            p = p.wrapping_mul(10);
            k += 1;
            if !zero_pad && value < p {
                break;
            }
        }
        self.gold_string(x, y, size, flags, &text);
    }

    /// 0x1B82C0: width (widest line) and height (lines x line height) of
    /// a gold-font string.
    fn gold_measure(&self, size: usize, flags: u32, text: &[u8]) -> (i32, i32) {
        let (cw, lh) = self.t.font_sizes[size];
        let (cw, lh) = (i32::from(cw), i32::from(lh));
        let proportional = flags & 1 != 0;
        let glyph_w = |g: usize| self.t.glyphs[g + 37 * size].width();
        let (mut width, mut cur, mut height) = (0, 0, 0);
        for &ch in text {
            match gold_glyph(ch) {
                Some(g) => cur += if proportional { glyph_w(g) } else { cw },
                None if ch == b'\n' => {
                    width = width.max(cur);
                    cur = 0;
                    height += lh;
                }
                None => cur += if proportional { cw >> 1 } else { cw },
            }
        }
        (width.max(cur), height + lh)
    }

    /// A player's name where the original draws the plate `plate` at
    /// (x, y): the small gold font, proportional, centred on the plate
    /// (letters and digits, upper case; anything else is a space, as the
    /// server's names never have).
    #[cfg_attr(not(feature = "online"), allow(dead_code))]
    fn name_plate(&mut self, plate: Descriptor, name: &[u8], x: i32, y: i32) {
        let mut text: Vec<u8> = name.iter().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { b' ' }).collect();
        // No wider than the plate's room: the stats beside the portrait leave
        // about 64 pixels, and this is already the smallest gold font (about
        // 8 letters; 12 take 91 to 108). A longer name keeps its first word
        // (a character's, "PEGASUS CRAWFORD"), else as many letters as fit.
        const ROOM: i32 = 64;
        if self.gold_measure(1, 1, &text).0 > ROOM {
            if let Some(first) = text.split(|&c| c == b' ').find(|w| !w.is_empty()).map(<[u8]>::to_vec) {
                text = first;
            }
            while text.len() > 1 && self.gold_measure(1, 1, &text).0 > ROOM {
                text.pop();
            }
        }
        let (_, lh) = self.gold_measure(1, 1, &text);
        let cy = y + (plate.height() - lh).max(0) / 2;
        self.gold_string(x + plate.width() / 2, cy, 1, 3, &text);
    }

    fn gold_string(&mut self, x: i32, y: i32, size: usize, flags: u32, text: &[u8]) {
        let glyph = gold_glyph;
        let (cw, lh) = self.t.font_sizes[size];
        let (cw, lh) = (i32::from(cw), i32::from(lh));
        let proportional = flags & 1 != 0;
        let (width, _) = self.gold_measure(size, flags, text);
        let mut x0 = x * PX;
        if flags & 2 != 0 {
            x0 -= (width >> 1) * PX;
        }
        let (mut pen, mut py) = (x0, y * PX);
        for &ch in text {
            let Some(g) = glyph(ch) else {
                if ch == b'\n' {
                    pen = x0;
                    py += lh * PX;
                } else {
                    pen += (if proportional { cw >> 1 } else { cw }) * PX;
                }
                continue;
            };
            let d = self.t.glyphs[g + 37 * size];
            let (gw, gh) = (d.width(), d.height());
            // Monospace glyphs are centred in their cell (may land on .5).
            let ox = if proportional { 0 } else { (cw - gw) * HALF };
            let gx = pen + ox;
            let (u0, v0, u1, v1) = (
                i32::from(d.u0) * PX + HALF,
                i32::from(d.v0) * PX + HALF,
                i32::from(d.u1) * PX + HALF,
                i32::from(d.v1) * PX + HALF,
            );
            let tex = self.slot(&d);
            self.push(
                PrimKind::TriStrip,
                tex,
                Some(0),
                true,
                vec![
                    vx(gx, py, u0, v0, self.gold_rgba),
                    vx(gx + gw * PX, py, u1, v0, self.gold_rgba),
                    vx(gx, py + gh * PX, u0, v1, self.gold_rgba),
                    vx(gx + gw * PX, py + gh * PX, u1, v1, self.gold_rgba),
                ],
            );
            pen += (if proportional { gw } else { cw }) * PX;
        }
    }

    // ---- 0x246B90: composed text line ----------------------------------------
    fn width(&mut self, message: i32) -> Option<i32> {
        let id = u16::try_from(message).ok();
        match id.and_then(|id| self.t.message(id)) {
            Some(codes) => Some(12 * codes.len() as i32),
            None => {
                self.unsupported(UnsupportedKind::MissingMessage(id.unwrap_or(u16::MAX)));
                None
            }
        }
    }

    /// Callers squeeze a line to at most `limit` pixels: W ≥ limit + 1 →
    /// scale limit / W (f32), else 1.
    fn text_fit(&mut self, line: u8, message: i32, x: i32, y: i32, limit: i32) {
        let Some(w) = self.width(message) else {
            return;
        };
        let sx = if w > limit {
            limit as f32 / w as f32
        } else {
            1.0
        };
        self.text_line(line, message, x, y, w, sx, 1.0);
    }

    #[allow(clippy::too_many_arguments)]
    fn text_line(&mut self, line: u8, message: i32, x: i32, y: i32, w: i32, sx: f32, sy: f32) {
        self.text_lines(line, message, x, y, w, sx, sy, 1);
    }

    /// A message of `lines` canvas lines (22 px each) from line `line`.
    #[allow(clippy::too_many_arguments)]
    fn text_line_h(&mut self, line: u8, message: i32, x: i32, y: i32, w: i32, sx: f32, lines: i32) {
        self.text_lines(line, message, x, y, w, sx, 1.0, lines);
    }

    #[allow(clippy::too_many_arguments)]
    fn text_lines(&mut self, line: u8, message: i32, x: i32, y: i32, w: i32, sx: f32, sy: f32, lines: i32) {
        // 0x1A2B80 truncates the scaled size toward zero.
        let (dw, dh) = ((w as f32 * sx) as i32, (22.0 * lines as f32 * sy) as i32);
        let v = 22 * i32::from(line);
        self.sprite(
            [x * PX, y * PX, (x + dw) * PX, (y + dh) * PX],
            [
                HALF & 0x3FFF,
                (v * PX + HALF) & 0x3FFF,
                (w * PX + HALF) & 0x3FFF,
                ((v + 22 * lines) * PX + HALF) & 0x3FFF,
            ],
            WHITE,
            TextureSource::TextCanvas {
                line,
                message: u16::try_from(message).ok(),
            },
            Some(0),
            true,
        );
    }
}

/// 0x1B8240: gold-font glyph of a character.
fn gold_glyph(ch: u8) -> Option<usize> {
    match ch {
        b'A'..=b'Z' => Some(usize::from(ch - b'A')),
        b'a'..=b'z' => Some(usize::from(ch - b'a')),
        b'0'..=b'9' => Some(usize::from(ch - b'0') + 26),
        b'?' => Some(36),
        _ => None,
    }
}

/// Arguments of 0x1B4860 beyond the descriptor and position.
#[derive(Clone, Copy)]
struct Atlas {
    black: bool,
    draw: bool,
    ring: bool,
    small: bool,
    bright: bool,
    overlay: i32,
}

impl Atlas {
    fn bright(bright: bool) -> Self {
        Self {
            black: false,
            draw: true,
            ring: false,
            small: false,
            bright,
            overlay: -1,
        }
    }
}

/// 0x1E3530: cell terrain, −1 off the board.
fn terrain_at(st: &HudState, col: i32, row: i32) -> i32 {
    if (0..7).contains(&col) && (0..7).contains(&row) {
        i32::from(st.terrain[(row * 7 + col) as usize])
    } else {
        -1
    }
}

/// 0x1E4960: the unit a cell points at, if its slot is < 11.
fn unit_at(st: &HudState, col: i32, row: i32) -> Option<usize> {
    if !((0..7).contains(&col) && (0..7).contains(&row)) {
        return None;
    }
    st.board[(row * 7 + col) as usize].filter(|&i| st.units[i].slot < 11)
}

/// The 4-bit text canvas (GS 0x2040, 1024 texels wide, TBW 10) as the HUD
/// fills it: row band 0 holds the digit strip, bands 1–3 (v = 22·line) the
/// composed lines. Composition writes exactly 12 columns per code (the left
/// 12 of each 22×22 first-bank glyph cell) and leaves the rest of the line as
/// it was; checked against the round-2 GS dump.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextCanvas {
    pub width: usize,
    pub height: usize,
    /// One 4-bit index per texel (0..=15).
    pub indices: Vec<u8>,
}

/// Font cells of the message file: 22×22 4-bit cells, 242 bytes each, low
/// nibble first, starting at the message-font pointer (MSG_E.DAT 0x3239C).
#[derive(Clone, Copy, Debug)]
pub struct TextFont<'a> {
    pub cells: &'a [u8],
}

const CELL: usize = 22;
const CELL_BYTES: usize = CELL * CELL / 2;
/// Digit strip cells: '0'..'9', blank, '-', infinity.
const DIGIT_STRIP: [Option<usize>; 13] = [
    Some(225),
    Some(216),
    Some(217),
    Some(218),
    Some(219),
    Some(220),
    Some(221),
    Some(222),
    Some(223),
    Some(224),
    None,
    Some(226),
    Some(317),
];

impl TextFont<'_> {
    fn texel(&self, cell: usize, x: usize, y: usize) -> Option<u8> {
        let byte = *self.cells.get(cell * CELL_BYTES + (y * CELL + x) / 2)?;
        Some((byte >> ((x % 2) * 4)) & 0xF)
    }
}

impl Default for TextCanvas {
    fn default() -> Self {
        Self::new()
    }
}

impl TextCanvas {
    /// A cleared canvas (the original keeps whatever earlier screens left).
    pub fn new() -> Self {
        Self {
            width: 1024,
            height: 512,
            indices: vec![0; 1024 * 512],
        }
    }

    /// Row band 0: the digit strip '0123456789 -∞' in full 22-pixel cells
    /// (message-font cells 225, 216–224, blank, 226, 317).
    pub fn upload_digit_strip(&mut self, font: TextFont) -> crate::Result<()> {
        for (i, cell) in DIGIT_STRIP.iter().enumerate() {
            for y in 0..CELL {
                for x in 0..CELL {
                    let value = match cell {
                        Some(c) => font
                            .texel(*c, x, y)
                            .ok_or("font cells end before the digit strip")?,
                        None => 0,
                    };
                    self.indices[y * self.width + i * CELL + x] = value;
                }
            }
        }
        Ok(())
    }

    /// Compose `codes` into band `line` (v = 22·line; 0 is the digit strip),
    /// 12 pixels per code. Only first-bank glyph codes 0x2001..=0x20A3 are
    /// supported.
    pub fn compose(&mut self, line: u8, codes: &[u16], font: TextFont) -> crate::Result<()> {
        if line == 0 {
            return Err(format!("text line {line} is not a composed line"));
        }
        if let Some(code) = codes
            .iter()
            .find(|c| **c != 0x2000 && !(0x2001..=0x20A3).contains(*c))
        {
            return Err(format!("unsupported message code 0x{code:04X}"));
        }
        // A line separator (0x2000) continues on the next canvas line: the
        // tutorial's three-line pages fill lines 1-3 [O software-GS dumps of
        // saved states 189-n57 and 005-t5: one sprite over v 22..88]. The
        // rest of each written line is cleared so a shorter page leaves no
        // stale glyphs.
        let lines: Vec<&[u16]> = codes.split(|&c| c == 0x2000).collect();
        // The tutorial's pages run to six lines (saved state 367-r2); the
        // canvas holds 23 bands of 22 rows.
        if CELL * (usize::from(line) + lines.len()) > self.height {
            return Err(format!(
                "message of {} lines does not fit from canvas line {line}",
                lines.len()
            ));
        }
        for (k, segment) in lines.iter().enumerate() {
            if segment.len() * 12 > self.width {
                return Err("message wider than the canvas".into());
            }
            let row = 22 * (usize::from(line) + k);
            for (i, &code) in segment.iter().enumerate() {
                let cell = usize::from(code - 0x2000);
                for y in 0..CELL {
                    for x in 0..12 {
                        let value = font.texel(cell, x, y).ok_or("font cell missing")?;
                        self.indices[(row + y) * self.width + 12 * i + x] = value;
                    }
                }
            }
            if lines.len() > 1 {
                for y in 0..CELL {
                    for x in segment.len() * 12..self.width {
                        self.indices[(row + y) * self.width + x] = 0;
                    }
                }
            }
        }
        Ok(())
    }

    /// Lines a message occupies (its 0x2000 separators plus one) and the
    /// widest line in pixels (12 per code).
    pub fn measure(codes: &[u16]) -> (usize, usize) {
        let mut count = 0;
        let mut width = 0;
        for l in codes.split(|&c| c == 0x2000) {
            count += 1;
            width = width.max(l.len() * 12);
        }
        (count, width)
    }
}

// ---- Rendering --------------------------------------------------------------

/// The HUD's textures, decoded from the user's files, and the live text
/// canvas (composed as the primitives are drawn).
#[derive(Clone, Debug)]
pub struct HudTextures {
    /// TEXSYS.MRG #6 (descriptor slot 6): icons, digits, gold font.
    pub atlas6: Texture,
    /// TEXSYS.MRG #7 (descriptor slot 7): frames, ornaments, labels.
    pub atlas7: Texture,
    /// PICMINI pictures of the two sides' leader cards ([H] the portrait is
    /// the side's deck-leader card; the round-2 dump shows cards 392/397).
    pub portraits: [Texture; 2],
    /// Further PICMINI pictures (a port addition, the N-player modes'
    /// compact plates): portrait slot 2 + i draws entry i.
    pub extra_portraits: Vec<Texture>,
    /// The terrain tile picture (render target 0x2000, 512×512). Its VU1
    /// render is not supported; without a picture the RT sprite is skipped.
    pub render_target: Option<Texture>,
    /// The card picture of [`TextureSource::Picture`] (the detail pane's).
    pub picture: Option<Texture>,
    font: Vec<u8>,
    text: HudText,
}

/// The live text canvas and its texture: the only part of [`HudTextures`]
/// that drawing changes.
#[derive(Clone, Debug)]
struct HudText {
    canvas: TextCanvas,
    /// The canvas through the text CLUT, and the CLUT it was built with.
    canvas_texture: Option<(Texture, [[u8; 4]; 16])>,
}

impl Default for HudText {
    fn default() -> Self {
        Self { canvas: TextCanvas { width: 0, height: 0, indices: Vec::new() }, canvas_texture: None }
    }
}

/// A primitive [`render`] did not draw, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    /// Index into the primitives passed in.
    pub index: usize,
    pub reason: String,
}

/// A 16-bit CLUT texture of the user's files as the GS expands it (TEXA
/// TA0 = 0, TA1 = 0x80: 1-bit alpha 0 or 0x80).
fn clut_texture(t: &roses_assets::OpeningIndexedTexture) -> Texture {
    Texture::from_c16_clut(
        t.width as usize,
        t.height as usize,
        &t.palette_indices,
        &t.palette_words_raw,
        0,
        0x80,
    )
}

/// A card's PICMINI picture (40×32 in its 64×32 slot).
fn portrait(picmini: &[u8], card: u16) -> crate::Result<Texture> {
    roses_assets::decode_pal_mini_picture(picmini, card)
        .map(|t| clut_texture(&t))
        .map_err(|e| format!("PICMINI card {card}: {e}"))
}

impl HudTextures {
    /// Decode TEXSYS #6/#7, the leader portraits and the message font from the
    /// user's TEXSYS.MRG, PICMINI.MRG and MSG_E.DAT bytes, and upload the
    /// canvas digit strip.
    pub fn new(
        texsys: &[u8],
        picmini: &[u8],
        leaders: [u16; 2],
        message_file: &[u8],
    ) -> crate::Result<Self> {
        let atlas = |index: usize| -> crate::Result<Texture> {
            roses_assets::decode_pal_opening_indexed_texture(
                texsys,
                roses_assets::OpeningArchiveKind::TexSys,
                index,
            )
            .map(|t| clut_texture(&t))
            .map_err(|e| format!("TEXSYS #{index}: {e}"))
        };
        let offset = roses_assets::decode_pal_name_font(message_file)
            .map_err(|e| e.to_string())?
            .metadata
            .file_offset;
        let font = message_file
            .get(offset..)
            .ok_or("message font missing")?
            .to_vec();
        let mut canvas = TextCanvas::new();
        canvas.upload_digit_strip(TextFont { cells: &font })?;
        Ok(Self {
            atlas6: atlas(6)?,
            atlas7: atlas(7)?,
            portraits: [
                portrait(picmini, leaders[0])?,
                portrait(picmini, leaders[1])?,
            ],
            extra_portraits: Vec::new(),
            render_target: None,
            picture: None,
            font,
            text: HudText { canvas, canvas_texture: None },
        })
    }

    /// Replace the two portraits with the given leader cards' pictures.
    pub fn set_leaders(&mut self, picmini: &[u8], leaders: [u16; 2]) -> crate::Result<()> {
        self.portraits = [
            portrait(picmini, leaders[0])?,
            portrait(picmini, leaders[1])?,
        ];
        Ok(())
    }

    /// Replace the further portraits (slots 2 ..) with the given cards'
    /// pictures (a port addition: the N-player compact plates).
    pub fn set_extra_portraits(&mut self, picmini: &[u8], cards: &[u16]) -> crate::Result<()> {
        self.extra_portraits = cards.iter().map(|&c| portrait(picmini, c)).collect::<crate::Result<_>>()?;
        Ok(())
    }

    /// The composed text canvas (4-bit indices).
    pub fn canvas(&self) -> &TextCanvas {
        &self.text.canvas
    }
}

impl HudText {

    /// Compose `message` into `line` and refresh that band of the canvas
    /// texture.
    fn compose(&mut self, font: &[u8], line: u8, codes: &[u16], clut: &[[u8; 4]; 16]) -> crate::Result<()> {
        self.canvas
            .compose(line, codes, TextFont { cells: font })?;
        let (lines, _) = TextCanvas::measure(codes);
        for k in 0..lines {
            // A multi-line message rewrites its whole bands.
            let width = if lines > 1 {
                self.canvas.width
            } else {
                codes.len() * 12
            };
            self.refresh_canvas(clut, Some((line + k as u8, width)));
        }
        Ok(())
    }

    /// Rebuild the canvas texture (all of it, or one line band).
    fn refresh_canvas(&mut self, clut: &[[u8; 4]; 16], band: Option<(u8, usize)>) {
        let stale = self.canvas_texture.as_ref().is_none_or(|(_, c)| c != clut);
        if stale {
            let c = &self.canvas;
            let t = Texture::from_c32_clut(c.width, c.height, &c.indices, clut);
            self.canvas_texture = Some((t, *clut));
            return;
        }
        if let (Some((line, width)), Some((t, _))) = (band, self.canvas_texture.as_mut()) {
            let w = self.canvas.width;
            for y in 22 * usize::from(line)..22 * usize::from(line) + 22 {
                for x in 0..width.min(w) {
                    t.texels[y * w + x] = clut[usize::from(self.canvas.indices[y * w + x])];
                }
            }
        }
    }
}

/// Draw a frame's HUD primitives into `target` as the GS does: the frame
/// buffer primitives in order, each text sprite after composing its message
/// into its canvas line, with the game's render state (TFX modulate, TCC,
/// the primitive's ALPHA mode, alpha test, filter) and the HUD's dither
/// matrix. Depth is not modelled (the HUD sits at the top of the depth range
/// with ZTST GEQUAL). Primitives for the terrain-tile render target are not
/// drawn; the RT sprite samples [`HudTextures::render_target`].
pub fn render(
    frame: &HudFrame,
    tables: &HudTables,
    textures: &mut HudTextures,
    target: &mut Target16,
) -> Vec<Skipped> {
    render_primitives(&frame.prims, tables, textures, target)
}

/// [`render`] for any run of primitives (e.g. one GS draw of a dump).
pub fn render_primitives(
    prims: &[Primitive],
    tables: &HudTables,
    textures: &mut HudTextures,
    target: &mut Target16,
) -> Vec<Skipped> {
    let mut text = std::mem::take(&mut textures.text);
    let skipped = draw_all(prims, tables, textures, &mut text, target, Some(tables.dimx));
    textures.text = text;
    skipped
}

fn draw_all(
    prims: &[Primitive],
    tables: &HudTables,
    textures: &HudTextures,
    text: &mut HudText,
    target: &mut Target16,
    dither: Option<u64>,
) -> Vec<Skipped> {
    let mut skipped = Vec::new();
    text.refresh_canvas(&tables.text_clut, None);
    for (index, p) in prims.iter().enumerate() {
        if p.target != Target::Frame {
            continue;
        }
        if let Err(reason) = compose_text(p, tables, &textures.font, text) {
            skipped.push(Skipped { index, reason });
            continue;
        }
        let texture = match p.texture {
            TextureSource::None => None,
            TextureSource::Atlas(6) => Some(&textures.atlas6),
            TextureSource::Atlas(7) => Some(&textures.atlas7),
            TextureSource::Atlas(slot) => {
                skipped.push(Skipped {
                    index,
                    reason: format!("texture slot {slot} is not bound"),
                });
                continue;
            }
            TextureSource::Portrait(side) => match textures
                .portraits
                .get(usize::from(side))
                .or_else(|| textures.extra_portraits.get(usize::from(side).wrapping_sub(2)))
            {
                Some(t) => Some(t),
                None => {
                    skipped.push(Skipped {
                        index,
                        reason: format!("no portrait for side {side}"),
                    });
                    continue;
                }
            },
            TextureSource::RenderTarget => match &textures.render_target {
                Some(t) => Some(t),
                None => {
                    skipped.push(Skipped {
                        index,
                        reason: "terrain tile picture (VU1 render) not supplied".into(),
                    });
                    continue;
                }
            },
            TextureSource::TextCanvas { .. } => text.canvas_texture.as_ref().map(|(t, _)| t),
            TextureSource::Picture => match &textures.picture {
                Some(t) => Some(t),
                None => {
                    skipped.push(Skipped { index, reason: "card picture not supplied".into() });
                    continue;
                }
            },
        };
        let blend = p
            .blend
            .and_then(|m| tables.alpha_modes.get(usize::from(m)))
            .map(|&a| Blend::from_register(a));
        let alpha_test_greater = p.alpha_test.then_some(0);
        let v = &p.verts;
        match p.kind {
            PrimKind::Sprite if v.len() == 2 => gs::draw(
                target,
                &Sprite {
                    xy: [v[0].x, v[0].y, v[1].x, v[1].y],
                    uv: [v[0].u, v[0].v, v[1].u, v[1].v],
                    rgba: v[1].rgba,
                    texture,
                    modulate: true,
                    tcc: true,
                    bilinear: p.bilinear,
                    blend,
                    alpha_test_greater,
                    dither,
                },
            ),
            PrimKind::TriStrip => {
                let strip: Vec<_> = v.iter().map(|v| ([v.x, v.y], [v.u, v.v], v.rgba)).collect();
                gs::draw_strip(
                    target,
                    &strip,
                    &Triangle {
                        xy: [[0.0; 2]; 3],
                        uv: [[0.0; 2]; 3],
                        rgba: [[0; 4]; 3],
                        gouraud: p.gouraud,
                        texture,
                        modulate: true,
                        tcc: true,
                        blend,
                        alpha_test_greater,
                        dither,
                    },
                );
            }
            PrimKind::Sprite => skipped.push(Skipped {
                index,
                reason: "sprite without two vertices".into(),
            }),
        }
    }
    skipped
}

/// The text a frame primitive's sprite shows, composed into its canvas line
/// before `p` is drawn.
fn compose_text(p: &Primitive, tables: &HudTables, font: &[u8], text: &mut HudText) -> Result<(), String> {
    if let TextureSource::TextCanvas {
        line,
        message: Some(m),
    } = p.texture
    {
        if line > 0 {
            return tables
                .message(m)
                .ok_or_else(|| format!("message {m} not supplied"))
                .and_then(|codes| text.compose(font, line, codes, &tables.text_clut));
        }
    }
    Ok(())
}

/// The HUD alone as a premultiplied-alpha RGBA8 layer (`width`×`height`
/// from the window origin) for compositing over another renderer's frame.
///
/// The frame is drawn twice with the same GS model, over black and over
/// white, without dithering; per pixel the colour over black is the
/// premultiplied colour and 1 − (white − black)/248 (the smallest channel
/// difference) the coverage. Opaque HUD pixels come out alpha 255, untouched
/// pixels 0, text edges and other translucent parts in between. The GS alpha
/// bit is not used: it only records whether the last write had alpha ≥ 0x80
/// and would drop the text's anti-aliased edges.
///
/// Difference from the original: the game blends each HUD draw into the same
/// 16-bit frame buffer as the 3D scene, with dithering and 5-bit rounding
/// after every draw. Compositing this layer once (out = rgb + (1 − a)·scene)
/// gives the same colours where the HUD is opaque (without the dither
/// pattern), and differs by up to about one 5-bit step where it is
/// translucent (text edges, the alpha ramp). The flashing overlays (ALPHA
/// modes 1/2, additive and subtractive) are not "over" operations and are
/// only approximated.
pub fn render_layer(
    frame: &HudFrame,
    tables: &HudTables,
    textures: &mut HudTextures,
    width: usize,
    height: usize,
) -> (Vec<u8>, Vec<Skipped>) {
    render_primitives_layer(&frame.prims, tables, textures, width, height)
}

/// [`render_layer`] for any run of primitives (e.g. another mode's HUD).
pub fn render_primitives_layer(
    prims: &[Primitive],
    tables: &HudTables,
    textures: &mut HudTextures,
    width: usize,
    height: usize,
) -> (Vec<u8>, Vec<Skipped>) {
    let blank = |fill: u16| Target16 {
        width,
        height,
        pixels: vec![fill; width * height],
    };
    let (mut black, mut white) = (blank(0), blank(0x7FFF));
    let mut text = std::mem::take(&mut textures.text);
    let skipped = draw_all(prims, tables, textures, &mut text, &mut black, None);
    draw_all(prims, tables, textures, &mut text, &mut white, None);
    textures.text = text;
    // Per pixel: the black pass's colour, and the alpha from the least
    // spread between the two passes' channels (issue #20: written into a
    // sized buffer, the same arithmetic).
    let mut rgba = vec![0u8; width * height * 4];
    for ((out, &b), &w) in rgba.chunks_exact_mut(4).zip(&black.pixels).zip(&white.pixels) {
        let channel = |p: u16, i: u32| i32::from((p >> (5 * i)) & 31) << 3;
        let (b0, b1, b2) = (channel(b, 0), channel(b, 1), channel(b, 2));
        let spread = (channel(w, 0) - b0).min(channel(w, 1) - b1).min(channel(w, 2) - b2);
        out[0] = b0 as u8;
        out[1] = b1 as u8;
        out[2] = b2 as u8;
        out[3] = (255 - (spread.clamp(0, 248) * 255 + 124) / 248) as u8;
    }
    (rgba, skipped)
}

/// Whether a plate's stars, monster and magic counts read "??". The
/// original's rule follows d3c: with two human players on one screen, only
/// the acting side's show. A seat on its own screen (online, a port
/// addition) always reads its own and never the other seat's, as against
/// the computer.
fn counts_hidden(controllers: [u8; 2], d3c: u8, side: u8, own_side: Option<u8>) -> bool {
    let d3c = usize::from(d3c & 1);
    if let Some(own) = own_side {
        return side != own;
    }
    if controllers[0] < 100 && controllers[1] < 100 {
        false
    } else if controllers[d3c] >= 100 {
        usize::from(side) != d3c
    } else {
        usize::from(side) == d3c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mode init's start, an instant load and the 32-frame fade-out
    /// the PCSX2 run of map → deck editor logged (a84 124 after the state-2
    /// frame, 120 … 0 in state 1, then the fade-in), the pulse's turns.
    #[test]
    fn now_loading_fades_out_over_32_frames_after_the_load() {
        let mut s = NowLoading::start();
        // The init frame (load state 3): drawn at alpha min(counter, 0x80) = 0.
        assert_eq!(s.shown(true), Some(0));
        s.step();
        s.loaded();
        let mut shown = Vec::new();
        while let Some(a) = s.shown(false) {
            shown.push(a);
            s.step();
        }
        assert_eq!(shown.len() as u32, NOW_LOADING_FADE);
        assert_eq!((shown[0], shown[1], shown[31]), (0x80, 0x7C, 4));
        assert_eq!(s.counter, 33);
        let mut p = NowLoading::start();
        let pulses: Vec<u8> = (0..70).map(|_| {
            p.step();
            p.pulse
        }).collect();
        assert_eq!((pulses[0], pulses[31], pulses[32], pulses[63], pulses[64]), (4, 0x80, 0x7C, 0, 4));
    }

    /// The overlay against the interpreter's GIF chains of 0x1AA220
    /// (`ROSES_LOADING_CASES` = modes/ui3/captures/loading-cases.json from
    /// tools/run_loading.py on a PCSX2 state; `ROSES_DUEL_IMPORT` for the
    /// executable and MSG_E.DAT): every primitive's kind, XY, UV and RGBA.
    #[test]
    fn private_now_loading_matches_the_emulated_draw() {
        let (Some(cases), Some(dir)) = (std::env::var_os("ROSES_LOADING_CASES"), std::env::var_os("ROSES_DUEL_IMPORT")) else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let exe = std::fs::read(dir.join("SLES_515.79")).unwrap();
        let msg = std::fs::read(dir.join("MSG_E.DAT")).unwrap();
        let tables = HudTables::from_files(&exe, &msg).unwrap();
        let strings = NowLoadingTables::from_executable(&exe).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(cases).unwrap()).unwrap();
        let mut checked = 0;
        for case in v.as_array().unwrap() {
            let before = case["before"].as_array().unwrap();
            let n = |i: usize| before[i].as_u64().unwrap();
            let s = NowLoading { counter: n(0) as u32, alpha: n(1) as u8, pulse: n(2) as u8, falling: n(3) & 0xC == 0xC };
            let loading = (n(3) >> 4) & 3 >= 3;
            let flag = case["pokes"].get("f8a").and_then(|f| f.as_u64()).is_none_or(|f| f & 0x10 != 0);
            let ours = if flag { now_loading(&tables, &strings, &s, loading).prims } else { Vec::new() };
            let theirs = case["prims"].as_array().unwrap();
            assert_eq!(ours.len(), theirs.len(), "{:?}", case["pokes"]);
            for (k, (o, t)) in ours.iter().zip(theirs).enumerate() {
                let kind = if o.kind == PrimKind::Sprite { "SPRITE" } else { "TRISTRIP" };
                assert_eq!(kind, t["prim"].as_str().unwrap(), "{:?} prim {k}", case["pokes"]);
                let tv = t["verts"].as_array().unwrap();
                assert_eq!(o.verts.len(), tv.len());
                for (ov, tv) in o.verts.iter().zip(tv) {
                    let f = |i: usize| tv[i].as_f64().unwrap() as f32;
                    let c = |i: usize| tv[i].as_u64().unwrap() as u8;
                    assert_eq!((ov.x, ov.y, ov.u, ov.v), (f(0), f(1), f(2), f(3)), "{:?} prim {k}", case["pokes"]);
                    assert_eq!(ov.rgba, [c(4), c(5), c(6), c(7)], "{:?} prim {k}", case["pokes"]);
                }
                checked += 1;
            }
            // The steps after the frame.
            if !theirs.is_empty() {
                let after = case["after"].as_array().unwrap();
                let mut s2 = s;
                s2.step();
                let a = |i: usize| after[i].as_u64().unwrap();
                assert_eq!((s2.counter, s2.alpha, s2.pulse, s2.falling), (a(0) as u32, a(1) as u8, a(2) as u8, a(3) & 0xC == 0xC));
            }
        }
        eprintln!("{checked} NOW LOADING primitives match");
        assert!(checked > 0);
    }

    /// Synthetic tables (no original data): every sprite 8×8 on slot 7,
    /// glyphs 4×6 on slot 6, arbitrary colours, margins and offsets.
    /// Kinds 5/6/7 (memory-card boxes): one 15-code line (W 180, H 22):
    /// kind 6 top 219, icon at (248, 265) [emu DUEL-MESSAGE-BOX §5]; kind 7
    /// top 256 − (22 + 0x3C) / 2, width 0x15A, YES at 0xAA / NO at 0x156 on
    /// y 41 + 0xDC with the unselected word dimmed.
    #[test]
    fn overlay_boxes_of_kinds_5_6_7_are_placed_as_0x1b1830_places_them() {
        let mut t = tables();
        t.messages.insert(3557, vec![0x2005; 15]);
        let yn = YesNoTables { yes: b"YES".to_vec(), no: b"NO".to_vec(), dim: [64, 64, 64, 128] };
        let text_top = |f: &HudFrame| {
            f.prims
                .iter()
                .find(|p| matches!(p.texture, TextureSource::TextCanvas { .. }))
                .map(|p| (p.verts[0].x, p.verts[0].y))
                .unwrap()
        };
        let b = |kind| OverlayBox { kind, x: 0, y: 0, text: 3557 };
        let f5 = overlay_box(&t, &yn, &b(OverlayKind::Centred), 0);
        assert_eq!(text_top(&f5), (166.0, 256.0 - 23.0 + 16.0));
        let f6 = overlay_box(&t, &yn, &b(OverlayKind::Press), 0);
        assert_eq!(text_top(&f6), (166.0, 219.0 + 16.0));
        let icon = f6.prims.last().unwrap();
        assert_eq!((icon.verts[0].x, icon.verts[0].y), (248.0, 265.0));
        let f7 = overlay_box(&t, &yn, &b(OverlayKind::YesNo { yes: false }), 0);
        assert_eq!(text_top(&f7), (166.0, 256.0 - 41.0 + 16.0));
        let glyphs: Vec<_> = f7.prims.iter().filter(|p| p.kind == PrimKind::TriStrip && p.texture != TextureSource::None).collect();
        assert_eq!(glyphs.len(), 5);
        assert_eq!(glyphs[0].verts[0].y, (41 + 0xDC) as f32);
        assert_eq!(glyphs[0].verts[0].rgba, [64, 64, 64, 128]);
        assert_eq!(glyphs[4].verts[0].rgba, GREY);
        assert!(f5.unsupported.is_empty() && f6.unsupported.is_empty() && f7.unsupported.is_empty());
    }

    /// The OPTION panel at rest (y 0): rows at y 0xF9 and 0x121, headings
    /// at x 0x34, choices at 0x110 / 0x174, the unselected one dimmed; the
    /// cursor frame on the row; at y 0x40 everything is 0x40 lower.
    #[test]
    fn the_option_panel_places_its_rows_as_0x1b02cc_draws_them() {
        let mut t = tables();
        for id in [116, 117, 120, 121, 122, 123] {
            t.messages.insert(id, vec![0x2005; 5]);
        }
        let yn = YesNoTables { yes: b"YES".to_vec(), no: b"NO".to_vec(), dim: [64, 64, 64, 128] };
        let texts = |f: &HudFrame| {
            f.prims
                .iter()
                .filter_map(|p| match p.texture {
                    TextureSource::TextCanvas { message: Some(m), .. } => {
                        Some((m, p.verts[0].x as i32, p.verts[0].y as i32, p.verts[0].rgba[0]))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let f = option_panel(&t, &yn, &OptionPanel { side: 0, y: 0, row: 1, bits: 0b010 });
        assert!(f.unsupported.is_empty(), "{:?}", f.unsupported);
        assert_eq!(
            texts(&f),
            [
                (116, 0x34, 0xF9, 255),
                (120, 0x110, 0xF9, 255),
                (121, 0x174, 0xF9, 64),
                (117, 0x34, 0x121, 255),
                (123, 0x110, 0x121, 64),
                (122, 0x174, 0x121, 255),
            ]
        );
        let low = option_panel(&t, &yn, &OptionPanel { side: 0, y: 0x40, row: 1, bits: 0b010 });
        assert_eq!(texts(&low)[0].2, 0xF9 + 0x40);
        assert_eq!(f.prims.len(), low.prims.len());
    }

    pub(super) fn tables() -> HudTables {
        let d = |slot, w, h| Descriptor {
            slot,
            u0: 0,
            v0: 0,
            u1: w,
            v1: h,
        };
        let messages = HudTables::required_messages()
            .into_iter()
            .map(|id| (id, vec![0x2005; 3]))
            .collect();
        HudTables {
            sprites: vec![d(7, 8, 8); DESCRIPTOR_COUNT],
            attribute_icons: vec![d(6, 8, 8); 9],
            glyphs: vec![d(6, 4, 6); GLYPH_COUNT],
            colours: (0..COLOUR_COUNT as u8)
                .map(|i| [40 + i, 60, 80, 128, 90 + i, 100, 110, 128])
                .collect(),
            margins: vec![[1, 1, 1, 1]; MARGIN_COUNT],
            deco_offsets: [3; 16],
            font_sizes: [(6, 7), (5, 7), (9, 9)],
            legend_sets: vec![[(1023, 0xFFFF); 4]; LEGEND_SET_COUNT],
            alpha_modes: [1, 2, 3, 4],
            text_clut: [[255, 255, 255, 0]; 16],
            dimx: 0,
            graveyard_title: b"TITLE".to_vec(),
            option_title: b"OPTION".to_vec(),
            messages,
        }
    }

    fn synthetic_textures() -> HudTextures {
        let solid = |w: usize, h: usize, texel| Texture {
            width: w,
            height: h,
            texels: vec![texel; w * h],
        };
        HudTextures {
            atlas6: solid(16, 16, [200, 100, 50, 0x80]),
            atlas7: solid(16, 16, [100, 200, 50, 0x80]),
            portraits: [
                solid(64, 32, [10, 20, 30, 0x80]),
                solid(64, 32, [30, 20, 10, 0x80]),
            ],
            extra_portraits: Vec::new(),
            render_target: None,
            picture: None,
            // Font cell c holds the value c & 15 everywhere.
            font: (0..320)
                .flat_map(|c: usize| std::iter::repeat_n(((c & 15) as u8) * 17, CELL_BYTES))
                .collect(),
            text: HudText { canvas: TextCanvas::new(), canvas_texture: None },
        }
    }

    #[test]
    fn render_draws_the_frame_and_reports_the_missing_tile_picture() {
        let t = tables();
        let f = frame(&shown(), &t);
        let mut textures = synthetic_textures();
        let mut target = Target16 {
            width: 512,
            height: 512,
            pixels: vec![0; 512 * 512],
        };
        let skipped = render(&f, &t, &mut textures, &mut target);
        let rt_sprite = f
            .prims
            .iter()
            .position(|p| p.texture == TextureSource::RenderTarget)
            .unwrap();
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].index, rt_sprite);
        // The composed messages (three 'E' cells, value 5) are on the canvas.
        let row = 22 * textures.canvas().width;
        assert_eq!(textures.canvas().indices[row..row + 36], [5; 36]);
        // The LP panel gradient (opaque, not blended) is in the frame.
        assert_ne!(target.pixels[50 * 512 + 100], 0);
        let (layer, _) = render_layer(&f, &t, &mut textures, 512, 512);
        let alpha = |x: usize, y: usize| layer[(y * 512 + x) * 4 + 3];
        assert_eq!(alpha(100, 50), 255);
        assert_eq!(alpha(256, 300), 0);
    }

    /// The layer is the black pass's colour with the alpha of the two
    /// passes' spread (the per-pixel formula as first written), and the
    /// canvas ends as the second pass leaves it.
    #[test]
    fn layer_is_the_two_passes_combined() {
        let t = tables();
        let f = frame(&shown(), &t);
        let mut layered = synthetic_textures();
        let (layer, skipped) = render_layer(&f, &t, &mut layered, 512, 512);
        let mut serial = synthetic_textures();
        let mut text = std::mem::take(&mut serial.text);
        let mut black = Target16 { width: 512, height: 512, pixels: vec![0; 512 * 512] };
        let mut white = Target16 { width: 512, height: 512, pixels: vec![0x7FFF; 512 * 512] };
        let serial_skipped = draw_all(&f.prims, &t, &serial, &mut text, &mut black, None);
        draw_all(&f.prims, &t, &serial, &mut text, &mut white, None);
        serial.text = text;
        let mut expected = Vec::new();
        for (&b, &w) in black.pixels.iter().zip(&white.pixels) {
            let channel = |p: u16, i: u32| i32::from((p >> (5 * i)) & 31) << 3;
            let spread = (0..3).map(|i| channel(w, i) - channel(b, i)).min().unwrap();
            expected.extend((0..3).map(|i| channel(b, i) as u8));
            expected.push((255 - (spread.clamp(0, 248) * 255 + 124) / 248) as u8);
        }
        assert_eq!(skipped, serial_skipped);
        assert!(layer == expected, "the layers differ");
        assert_eq!(layered.canvas().indices, serial.canvas().indices);
        assert_eq!(
            layered.text.canvas_texture.as_ref().map(|(t, c)| (&t.texels, c)),
            serial.text.canvas_texture.as_ref().map(|(t, c)| (&t.texels, c))
        );
    }

    fn shown() -> HudState {
        let mut st = HudState {
            d67: 1,
            d3e: 8,
            d3d: 2,
            terrain_model_ready: true,
            ..HudState::default()
        };
        st.sides[0].lp = 12345;
        st.sides[0].cursor = [3, 6];
        st.sides[1].controller = 100;
        st
    }

    #[test]
    fn no_hud_bit_draws_nothing() {
        let st = HudState {
            d67: 3,
            d3e: 40,
            ..HudState::default()
        };
        let f = frame(&st, &tables());
        assert!(f.prims.is_empty() && f.unsupported.is_empty());
        assert_eq!(f.next.d3e, 40);
    }

    #[test]
    fn sliding_out_latches_the_turn_and_draws_nothing() {
        let mut st = HudState {
            d3e: 124,
            d53: 7,
            d54: 1,
            d66: 0x80,
            ..HudState::default()
        };
        st.sides[1].cursor = [4, 2];
        let f = frame(&st, &tables());
        assert!(f.prims.is_empty());
        let n = f.next;
        assert_eq!(
            (n.d3e, n.d3c, n.d3d, n.d67, n.d40, n.d41),
            (128, 1, 7, 1, 4, 2)
        );
    }

    #[test]
    fn shown_frame_keeps_the_game_order_and_the_uv_register() {
        let mut st = shown();
        st.d44 = 5;
        let t = tables();
        let f = frame(&st, &t);
        assert_eq!(f.next.d3e, 0);
        let mut order: Vec<Element> = f.prims.iter().map(|p| p.element).collect();
        order.dedup();
        assert_eq!(
            order,
            [
                Element::TerrainPicture,
                Element::LpPanel(0),
                Element::LpPanel(1),
                Element::CardBox,
                Element::LegendBox,
                Element::TerrainBox,
                Element::RoundBox,
                Element::RoundNumber,
            ]
        );
        // Backing, RT clear, RT sprite; the tile render itself is reported.
        assert_eq!(f.prims[1].target, Target::RenderTarget);
        assert_eq!(f.prims[2].texture, TextureSource::RenderTarget);
        let kinds: Vec<_> = f.unsupported.iter().map(|u| u.kind.clone()).collect();
        assert_eq!(
            kinds,
            [
                UnsupportedKind::TerrainTileRender { terrain: 0 },
                UnsupportedKind::Banner { alpha: 5 }
            ]
        );
        assert_eq!(f.unsupported[0].at, 2);
        // Untextured primitives repeat the previous textured vertex's UV.
        let mut uv = (0.0, 0.0);
        for p in &f.prims {
            if p.texture == TextureSource::None {
                assert!(p.verts.iter().all(|v| (v.u, v.v) == uv));
            } else {
                let v = p.verts.last().unwrap();
                uv = (v.u, v.v);
            }
        }
        assert_eq!(build(&st, &t), f.prims);
    }

    #[test]
    fn a_seat_reads_its_own_plate_on_either_turn_and_offline_keeps_the_rule() {
        let pads = [100, 101];
        for d3c in [0, 1] {
            for own in [0, 1] {
                assert!(!counts_hidden(pads, d3c, own, Some(own)));
                assert!(counts_hidden(pads, d3c, own ^ 1, Some(own)));
            }
            // A spectator or an offline shared screen: the acting side's.
            assert!(!counts_hidden(pads, d3c, d3c, None));
            assert!(counts_hidden(pads, d3c, d3c ^ 1, None));
        }
        // Offline against the computer, on its turn and on the player's.
        assert!(!counts_hidden([100, 2], 0, 0, None));
        assert!(counts_hidden([100, 2], 0, 1, None));
        assert!(counts_hidden([100, 2], 1, 1, None));
        assert!(!counts_hidden([2, 3], 0, 1, None));
    }

    #[test]
    fn lp_digits_keep_four_digits_and_hide_cpu_values() {
        let f = frame(&shown(), &tables());
        let lp: Vec<f32> = f
            .prims
            .iter()
            .filter(|p| {
                p.element == Element::LpPanel(0)
                    && p.texture
                        == TextureSource::TextCanvas {
                            line: 0,
                            message: None,
                        }
            })
            .map(|p| p.verts[0].u)
            .collect();
        // 12345 shows 2345, least significant digit first.
        assert_eq!(lp, [110.5, 88.5, 66.5, 44.5]);
        // Side 1 is human, side 0 (on turn) a CPU: side 0's counts are "??"
        // (two '?' glyphs each for stars, monsters and magic/traps).
        let glyphs = |side| {
            f.prims
                .iter()
                .filter(|p| p.element == Element::LpPanel(side) && p.kind == PrimKind::TriStrip)
                .filter(|p| p.texture != TextureSource::None)
                .count()
        };
        // Deck count 0 is one glyph and is never hidden.
        assert_eq!(glyphs(0), 3 * 2 + 1);
        assert_eq!(glyphs(1), 4);
    }

    #[test]
    fn text_canvas_writes_twelve_columns_per_code() {
        // Synthetic font: cell c holds the value c & 15 in every texel.
        let cells: Vec<u8> = (0..320)
            .flat_map(|c: usize| {
                let v = (c & 15) as u8;
                std::iter::repeat_n(v | v << 4, CELL_BYTES)
            })
            .collect();
        let font = TextFont { cells: &cells };
        let mut canvas = TextCanvas::new();
        canvas.upload_digit_strip(font).unwrap();
        // '1' is cell 216 (value 8); the blank cell stays 0.
        assert_eq!(canvas.indices[22], 8);
        assert_eq!(canvas.indices[10 * 22 + 5], 0);
        canvas.compose(1, &[0x2001, 0x2002, 0x2003], font).unwrap();
        canvas.compose(1, &[0x2004], font).unwrap();
        let row = 22 * canvas.width;
        let line: Vec<u8> = canvas.indices[row..row + 37].to_vec();
        assert_eq!(&line[..12], &[4; 12]);
        assert_eq!(&line[12..24], &[2; 12]);
        assert_eq!(&line[24..36], &[3; 12]);
        assert_eq!(line[36], 0);
        assert!(canvas.compose(1, &[0x2100], font).is_err());
        // A separator continues on the next line and clears the rest of
        // every written line; three lines from line 2 do not fit.
        canvas
            .compose(1, &[0x2005, 0x2000, 0x2006, 0x2007], font)
            .unwrap();
        assert_eq!(canvas.indices[row], 5);
        assert_eq!(canvas.indices[row + 12], 0);
        let row2 = 44 * canvas.width;
        assert_eq!(canvas.indices[row2], 6);
        assert_eq!(canvas.indices[row2 + 12], 7);
        assert_eq!(canvas.indices[row2 + 24], 0);
        assert_eq!(
            TextCanvas::measure(&[0x2005, 0x2000, 0x2006, 0x2007]),
            (2, 24)
        );
        let too_many: Vec<u16> = (0..24).flat_map(|_| [0x2001, 0x2000]).collect();
        assert!(canvas.compose(2, &too_many, font).is_err());
        canvas
            .compose(
                1,
                &[0x2001, 0x2000, 0x2001, 0x2000, 0x2001, 0x2000, 0x2001],
                font,
            )
            .unwrap();
    }

    /// Synthetic tables where atlas descriptor i samples texels
    /// (i, 0)-(i + 8, 8): an atlas sprite's u0 (i + 0.5) names it.
    fn indexed_tables() -> HudTables {
        let mut t = tables();
        for (i, d) in t.sprites.iter_mut().enumerate() {
            (d.u0, d.u1) = (i as u16, i as u16 + 8);
        }
        t
    }

    fn entry(card: u16, kind: u8) -> GraveyardEntry {
        GraveyardEntry {
            card,
            kind: Some(kind),
        }
    }

    /// HUD hidden (d67 bit 1), the panel alone: list 0 = five cards of
    /// side 0 (human), list 1 = two of side 1 (CPU).
    fn graveyard_state(slide: i16) -> HudState {
        let mut st = HudState {
            d67: 2,
            flash: 0x5800,
            ..HudState::default()
        };
        st.sides[0].controller = 100;
        st.sides[1].controller = 2;
        st.graveyard = Some(GraveyardPanel {
            lists: [
                vec![
                    entry(10, 3),
                    entry(11, 0x20),
                    entry(12, 0x60),
                    entry(13, 0x80),
                    entry(14, 0x13),
                    entry(15, 0x40),
                ],
                vec![entry(20, 5), entry(21, 0xA0)],
            ],
            shown: 0,
            locked: false,
            offsets: [0, 0],
            rows: [1, 0],
            slide,
        });
        st
    }

    /// (descriptor, x, y, grey) of the panel's single atlas sprites
    /// (u0 = descriptor + 0.5; frame pieces sit on whole texels).
    fn atlas_sprites(f: &HudFrame) -> Vec<(usize, f32, f32, u8)> {
        f.prims
            .iter()
            .filter(|p| p.kind == PrimKind::Sprite && matches!(p.texture, TextureSource::Atlas(_)))
            .filter(|p| p.verts[0].u.fract() == 0.5)
            .map(|p| (p.verts[0].u as usize, p.verts[0].x, p.verts[0].y, p.verts[0].rgba[0]))
            .collect()
    }

    #[test]
    fn graveyard_panel_at_rest_draws_box_tabs_markers_rows_and_cursor() {
        let t = indexed_tables();
        let f = frame(&graveyard_state(GRAVEYARD_REST), &t);
        assert!(f.unsupported.is_empty(), "{:?}", f.unsupported);
        assert!(f
            .prims
            .iter()
            .all(|p| p.element == Element::GraveyardPanel && p.z == WINDOW_Z));
        // 0x1B1830: box gradient (colour 13, subtractive) inside the type-1
        // margins at (256 - 173 - 4 + 1, 136 + 1), 346 x 240.
        let g = &f.prims[0];
        assert_eq!((g.kind, g.gouraud, g.blend), (PrimKind::TriStrip, true, Some(2)));
        assert_eq!((g.verts[0].x, g.verts[0].y, g.verts[1].x), (80.0, 137.0, 426.0));
        assert_eq!(f.prims[1].verts[3].y, 137.0 + 240.0);
        let sprites = atlas_sprites(&f);
        assert_eq!(
            sprites,
            [
                (0xB9, 184.0, 190.0, 128), // list 0 tab, PLAYER, shown
                (0x10D, 241.0, 195.0, 128),
                (0x10C, 260.0, 195.0, 128),
                (0xBC, 280.0, 190.0, 48), // list 1 tab, ENEMY (CPU), dim
                (0xC4, 359.0, 194.0, 128),
                (0x104, 395.0, 199.0, 128),
                (0xC5, 359.0, 348.0, 128),
                (0x105, 395.0, 348.0, 128),
                (3, 119.0, 219.0, 128), // race icon
                (21, 119.0, 243.0, 128), // class 1 -> 0x14 + 1
                (23, 119.0, 267.0, 128),
                (24, 119.0, 291.0, 128),
                (0x13, 119.0, 315.0, 128),
            ]
        );
        // The shown tab's flash: untextured, ALPHA mode 1, alpha flash >> 8.
        let i = f
            .prims
            .iter()
            .position(|p| p.verts[0].u == 0xB9 as f32 + 0.5)
            .unwrap();
        let flash = &f.prims[i + 1];
        assert_eq!((flash.texture, flash.blend), (TextureSource::None, Some(1)));
        assert_eq!((flash.verts[0].x, flash.verts[1].y, flash.verts[0].rgba[3]), (184.0, 198.0, 0x58));
        // 0x1B71C0 fill of the list box: colour set 14, standard blend.
        let fill = f
            .prims
            .iter()
            .find(|p| p.kind == PrimKind::Sprite && p.texture == TextureSource::None && p.blend == Some(0))
            .unwrap();
        assert_eq!(
            [fill.verts[0].x, fill.verts[0].y, fill.verts[1].x, fill.verts[1].y],
            [116.0, 216.0, 396.0, 342.0]
        );
        assert_eq!(fill.verts[0].rgba, t.inner(14));
        // Five names (messages 323 + card) at x 143 on canvas line 1.
        let names: Vec<(Option<u16>, f32)> = f
            .prims
            .iter()
            .filter_map(|p| match p.texture {
                TextureSource::TextCanvas { line: 1, message } => Some((message, p.verts[0].y)),
                _ => None,
            })
            .collect();
        assert_eq!(
            names,
            (0..5).map(|r| (Some(333 + r as u16), 219.0 + 24.0 * r as f32)).collect::<Vec<_>>()
        );
        // Cursor: panel type 7 frame, last, at (119, 219 + 24 row) - 0.5.
        let cursor = &f.prims[f.prims.len() - 8];
        assert_eq!((cursor.verts[0].x, cursor.verts[0].y), (118.5, 242.5));
        let right = f.prims.last().unwrap().verts[1].x;
        // 274 inner + the synthetic type-7 margins (1 + 1).
        assert_eq!(right, 118.5 + 276.0);
    }

    #[test]
    fn graveyard_panel_slides_every_vertex_by_512_minus_8_slide() {
        let t = indexed_tables();
        let rest = frame(&graveyard_state(GRAVEYARD_REST), &t);
        for slide in [0i16, 8, 16, 56] {
            let f = frame(&graveyard_state(slide), &t);
            assert_eq!(f.prims.len(), rest.prims.len());
            let dy = 512.0 - 8.0 * f32::from(slide);
            for (a, b) in f.prims.iter().zip(&rest.prims) {
                for (va, vb) in a.verts.iter().zip(&b.verts) {
                    assert_eq!((va.x, va.y, va.u, va.v), (vb.x, vb.y + dy, vb.u, vb.v));
                }
            }
        }
    }

    #[test]
    fn graveyard_panel_window_follows_offset_and_row() {
        let t = indexed_tables();
        let mut st = graveyard_state(GRAVEYARD_REST);
        let g = st.graveyard.as_mut().unwrap();
        g.offsets[0] = 3;
        g.rows[0] = 2;
        // A card without a record keeps its row empty.
        g.lists[0][4].kind = None;
        let f = frame(&st, &t);
        let rows: Vec<_> = atlas_sprites(&f).into_iter().skip(8).collect();
        assert_eq!(rows, [(24, 119.0, 219.0, 128), (22, 119.0, 267.0, 128)]);
        let names = f
            .prims
            .iter()
            .filter(|p| matches!(p.texture, TextureSource::TextCanvas { .. }))
            .count();
        assert_eq!(names, 2);
        let cursor = &f.prims[f.prims.len() - 8];
        assert_eq!(cursor.verts[0].y, 219.0 + 48.0 - 0.5);
        // An empty shown list: no rows and no cursor.
        let mut st = graveyard_state(GRAVEYARD_REST);
        st.graveyard.as_mut().unwrap().lists[0].clear();
        let f = frame(&st, &t);
        assert_eq!(atlas_sprites(&f).len(), 8);
        assert!(f.prims.last().unwrap().verts[0].x >= 395.0);
    }

    #[test]
    fn graveyard_panel_tabs_follow_the_shown_list_and_the_lock() {
        let t = indexed_tables();
        let mut st = graveyard_state(GRAVEYARD_REST);
        st.graveyard.as_mut().unwrap().shown = 1;
        let f = frame(&st, &t);
        let s = atlas_sprites(&f);
        assert_eq!(&s[..4], [
            (0xB9, 184.0, 190.0, 48),
            (0x10D, 241.0, 195.0, 128),
            (0x10C, 260.0, 195.0, 128),
            (0xBC, 280.0, 190.0, 128),
        ]);
        // List 1's rows and box colour set 15.
        assert_eq!(&s[8..], [(5, 119.0, 219.0, 128), (25, 119.0, 243.0, 128)]);
        assert!(f.prims.iter().any(|p| p.texture == TextureSource::None
            && p.blend == Some(0)
            && p.verts[0].rgba == t.inner(15)));
        // Locked: the one tab at x 232, no arrows; a CPU on side 0 shows ENEMY.
        let mut st = graveyard_state(GRAVEYARD_REST);
        st.sides[0].controller = 5;
        st.graveyard.as_mut().unwrap().locked = true;
        let f = frame(&st, &t);
        let s = atlas_sprites(&f);
        assert_eq!(s[0], (0xBA, 232.0, 190.0, 128));
        assert_eq!(s[1].0, 0xC4);
        let i = f.prims.iter().position(|p| p.verts[0].u == 0xBA as f32 + 0.5).unwrap();
        assert_eq!(f.prims[i + 1].blend, Some(1));
        // A list number past 1 is reported, not drawn.
        st.graveyard.as_mut().unwrap().shown = 2;
        let f = frame(&st, &t);
        assert!(f.prims.is_empty());
        assert_eq!(f.unsupported[0].kind, UnsupportedKind::GraveyardList(2));
    }

    #[test]
    fn graveyard_panel_title_plate_is_centred_on_the_gold_title() {
        let t = indexed_tables();
        let f = frame(&graveyard_state(GRAVEYARD_REST), &t);
        // "TITLE": five proportional 4-px glyphs (20 px), plate 44 x 21
        // (glyph line 9 + 12) at (256 - 22, 148); glyphs from 246 at 156.
        let plate = &f.prims[10];
        assert!(plate.gouraud && plate.blend.is_none());
        assert_eq!((plate.verts[0].x, plate.verts[0].y), (235.0, 149.0));
        let glyphs: Vec<(f32, f32)> = f
            .prims
            .iter()
            .filter(|p| p.kind == PrimKind::TriStrip && matches!(p.texture, TextureSource::Atlas(6)))
            .map(|p| (p.verts[0].x, p.verts[0].y))
            .collect();
        assert_eq!(glyphs, [(246.0, 156.0), (250.0, 156.0), (254.0, 156.0), (258.0, 156.0), (262.0, 156.0)]);
    }

    #[test]
    fn tables_come_from_the_loadable_segment() {
        let mut elf = vec![0u8; 0x200];
        elf[..4].copy_from_slice(b"\x7fELF");
        elf[0x1C..0x20].copy_from_slice(&0x40u32.to_le_bytes());
        elf[0x2A..0x2C].copy_from_slice(&32u16.to_le_bytes());
        elf[0x2C..0x2E].copy_from_slice(&1u16.to_le_bytes());
        for (o, v) in [
            (0x40, 1u32),
            (0x44, 0x100),
            (0x48, 0x10_0000),
            (0x50, 0x100),
        ] {
            elf[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
        elf[0x110] = 0xAB;
        assert_eq!(elf_slice(&elf, 0x10_0010, 1).unwrap(), [0xAB]);
        assert!(elf_slice(&elf, 0x10_00FF, 2).is_err());
        assert!(HudTables::from_executable(&elf, BTreeMap::new()).is_err());
    }
}
