//! The card detail pane 0x1BE6E0 (the deck editor's and Trade's DTL view)
//! as the original's HUD-leaf calls (`duel_hud::MenuCall`), Bevy-free
//! (Claude, 6 Oct 2026).
//!
//! Provenance (`work/research/modes/panels/NOTES.md` §2,
//! `two-player/d_detail_1be6e0.txt`): 0x1BE6E0 (name plate, the four page
//! tabs, the editor's extra boxes) and its pages 0x1BF1D0 (card data),
//! 0x1BFED0 (description), 0x1BFFF0 (the power-ups 0x24AD30 lists) and
//! 0x1C01F0 (leader abilities, 0x1E1F00 at the block's rank) were read [F]
//! and run in the EE interpreter (`panels/tools/run_detail.py`); the private
//! test compares this builder with the logged leaf calls of every page
//! [emu]. The card picture is drawn when the caller has it (`picture`), as
//! 0x1B8940 does once 0x200270(1, picture) finds it resident; otherwise the
//! pane shows the black box with "NOW LOADING" blinking with the scroll
//! phase 0x2DB828, as the original does until its loader finishes.
//! Nothing here is a game byte: strings, tables and colours are read from
//! the user's executable.
use roses_client::duel_hud::{self, HudTables, MenuCall};
use roses_core::card_block::CardBlock;
use roses_core::deck_editor::StaticCards;

const EE_TO_FILE: u32 = 0xF_9680;
/// 0x1DE350 maps this card to record 0x3E7 (none); the pane still draws it.
pub const SPECIAL_CARD: u16 = 0x29F;
/// Pages 2 and 3 exist for the monster ids below this (0x1BFFF0, 0x1C01F0).
const MONSTER_LIMIT: u16 = 0x2AB;
/// Card names start at message 323 (0x1DF480), descriptions at 0x499 + card.
const NAME_MESSAGE: u16 = 323;
const TEXT_MESSAGE: u16 = 0x499;
const CARDS: usize = 0x356;

/// The executable's strings, colours and tables the pane reads.
#[derive(Clone, Debug)]
pub struct DetailData {
    /// 0x2BDCC0 / C4 / C8 / CC (B, G, R, A stored; kept as R, G, B, A).
    pub tints: [[u8; 4]; 4],
    /// 0x2BE343: the editor's library label.
    pub library: Vec<u8>,
    /// 0x2BE313: the catalogue's footer word (0x1C04E0 state 0).
    pub back: Vec<u8>,
    /// 0x2BE34B: the picture box's text while the picture loads.
    pub loading: Vec<u8>,
    /// 0x2BE35B: the number line's format (one `%0Nd`).
    pub number: Vec<u8>,
    /// 0x2905E8: the loading text's colour (its alpha byte is replaced).
    pub loading_rgba: [u8; 4],
    /// 0x2BE31D: the editor's trunk-list footer word (0x1BC620, blinking).
    pub reincarnation: Vec<u8>,
    /// 0x2905E0: its colour (B, G, R stored; kept R, G, B; the alpha byte is
    /// replaced by 0x2DB828 >> 8) [F 0x1BCDB0..0x1BCDE8].
    pub reincarnation_rgba: [u8; 4],
    /// 0x2BDBD0: the editor background's edge and centre colours (B, G, R, A
    /// x2) [F 0x1BBF50].
    pub editor_background: [u8; 8],
    /// 0x2C26B8 + cardÂ·40: leader ability rows (monsters).
    pub abilities: Vec<[u16; 20]>,
    /// 0x29C970 + cardÂ·7: power-up compatibility bits (0x2F0 + bit).
    pub equips: Vec<[u8; 7]>,
}

fn word(exe: &[u8], ee: u32) -> Result<u32, String> {
    let at = (ee - EE_TO_FILE) as usize;
    exe.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or_else(|| format!("{ee:#x} is outside the executable"))
}

impl DetailData {
    pub fn from_executable(exe: &[u8]) -> Result<Self, String> {
        let s = |a: u32| duel_hud::elf_string(exe, a).map_err(|e| e.to_string());
        let bytes = |a: u32, n: usize| duel_hud::elf_slice(exe, a, n).map_err(|e| e.to_string()).map(|b| b.to_vec());
        let tint = |a: u32| -> Result<[u8; 4], String> {
            let c = bytes(a, 4)?;
            Ok([c[2], c[1], c[0], c[3]])
        };
        let rows = bytes(0x2C_26B8, usize::from(MONSTER_LIMIT) * 40)?;
        let equips = bytes(0x29_C970, CARDS * 7)?;
        let c = word(exe, 0x29_05E8)?.to_le_bytes();
        let r = word(exe, 0x29_05E0)?.to_le_bytes();
        Ok(Self {
            tints: [tint(0x2B_DCC0)?, tint(0x2B_DCC4)?, tint(0x2B_DCC8)?, tint(0x2B_DCCC)?],
            library: s(0x2B_E343)?,
            back: s(0x2B_E313)?,
            loading: s(0x2B_E34B)?,
            number: s(0x2B_E35B)?,
            loading_rgba: [c[2], c[1], c[0], c[3]],
            reincarnation: s(0x2B_E31D)?,
            reincarnation_rgba: [r[2], r[1], r[0], r[3]],
            editor_background: bytes(0x2B_DBD0, 8)?.try_into().unwrap(),
            abilities: rows.chunks_exact(40).map(|r| std::array::from_fn(|k| u16::from_le_bytes([r[2 * k], r[2 * k + 1]]))).collect(),
            equips: equips.chunks_exact(7).map(|r| r.try_into().unwrap()).collect(),
        })
    }

    /// 0x24AD30(card): the power-ups (0x2F0 + bit of the card's row) other
    /// than 0x30C, 0x315 and 0x31C, at most 12.
    pub fn power_ups(&self, card: u16) -> Vec<u16> {
        let Some(row) = self.equips.get(usize::from(card)) else { return Vec::new() };
        let mut out = Vec::new();
        for i in 0..49u16 {
            if out.len() >= 12 {
                break;
            }
            let id = 0x2F0 + i;
            if (row[usize::from(i >> 3)] >> (i & 7)) & 1 != 0 && !matches!(id, 0x30C | 0x315 | 0x31C) {
                out.push(id);
            }
        }
        out
    }

    /// The number line: the format with its one `%0Nd` filled.
    fn number_line(&self, card: u16) -> Vec<u8> {
        let f = &self.number;
        let Some(p) = f.iter().position(|&c| c == b'%') else { return f.clone() };
        let Some(d) = f[p..].iter().position(|&c| c == b'd') else { return f.clone() };
        let spec = &f[p + 1..p + d];
        let zero = spec.first() == Some(&b'0');
        let width: usize = std::str::from_utf8(spec).ok().and_then(|w| w.trim_start_matches('0').parse().ok()).unwrap_or(0);
        let digits = card.to_string();
        let pad = width.saturating_sub(digits.len());
        let mut out = f[..p].to_vec();
        out.extend(std::iter::repeat_n(if zero { b'0' } else { b' ' }, pad));
        out.extend_from_slice(digits.as_bytes());
        out.extend_from_slice(&f[p + d + 1..]);
        out
    }
}

/// PICTURE.MRG and the alternate-art table: the textures 0x1B8940 binds
/// (record = 0x215E60(side, card): the block's art counter's choice; 0x29F
/// asks for record 0x3E7, which the archive does not have).
pub struct Pictures {
    archive: Vec<u8>,
    art: Vec<roses_core::card_block::AlternateArt>,
    cache: std::collections::BTreeMap<u16, Option<roses_client::gs::Texture>>,
}

impl Pictures {
    pub fn new(archive: Vec<u8>, executable: &[u8]) -> Result<Self, String> {
        let art = roses_assets::parse_pal_alternate_art(executable)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|r| roses_core::card_block::AlternateArt { card: r.card, variants: r.variants, pictures: r.pictures })
            .collect();
        Ok(Self { archive, art, cache: Default::default() })
    }

    /// The card's picture as the GS samples it (5-bit channels shifted
    /// left, alpha 0x80), or `None` when the archive has no such record.
    pub fn texture(&mut self, card: u16, block: &CardBlock) -> Option<roses_client::gs::Texture> {
        let record = if card == SPECIAL_CARD { 0x3E7 } else { block.art_picture(card, &self.art) };
        let archive = &self.archive;
        self.cache
            .entry(record)
            .or_insert_with(|| {
                let image = roses_assets::decode_picture(archive, usize::from(record)).ok()?;
                let raw = |v: u8| ((u32::from(v) * 31 + 127) / 255) as u8;
                let texels = image.pixels.chunks_exact(4).map(|p| [raw(p[0]) << 3, raw(p[1]) << 3, raw(p[2]) << 3, 0x80]).collect();
                Some(roses_client::gs::Texture { width: image.width as usize, height: image.height as usize, texels })
            })
            .clone()
    }
}

/// Message ids the pane composes (labels, kinds, types, attributes, rank
/// names, abilities, tabs, card names and descriptions).
pub fn message_ids() -> Vec<u16> {
    let mut ids: Vec<u16> = (31..=170).collect();
    ids.extend(NAME_MESSAGE..NAME_MESSAGE + CARDS as u16);
    ids.extend(TEXT_MESSAGE..TEXT_MESSAGE + CARDS as u16);
    ids
}

/// The caller's arguments and the state the pane reads.
pub struct Pane<'a> {
    pub card: u16,
    /// a2 / a3: the slide position (Trade: side 0 = slide, side 1 = âˆ’slide).
    pub x: i32,
    pub y: i32,
    /// t0: 0 = left pane (x âˆ’ 0x100), 1 = right (x + 0x200); also selects
    /// the page byte 0x2DB85C + side.
    pub side: u8,
    /// 0x2DB85C + side.
    pub page: u8,
    /// t1: the current page's tab as a raised panel (else a plain box).
    pub raised_tab: bool,
    /// t2: the tab block's box in the card's colour (else colour 2).
    pub tinted: bool,
    /// t3 with 0x2DB8FD: the deck editor's library boxes and its state.
    pub editor_state: Option<u8>,
    /// The block 0x2DB8FC selects (rank, alternate art).
    pub block: &'a CardBlock,
    pub cards: &'a StaticCards,
    /// 0x200270(1, picture) found the picture resident.
    pub picture: bool,
    /// 0x2DB828 >> 8: the loading text's alpha.
    pub blink: u8,
    /// 0x2154C0(D5C, card) when the caller keeps no card block (the duel:
    /// the HUD's rank getter); None reads the block's stored rank.
    pub rank: Option<i32>,
}

impl Pane<'_> {
    /// 0x2154C0(D5C, card): the stored rank, −1 for none.
    fn rank_of(&self, card: u16) -> i32 {
        self.rank.unwrap_or_else(|| self.block.stored_rank(card).map_or(-1, i32::from))
    }
}

/// 0x1DF290 + the +4 override: the row colour class (7 + class is the
/// colour set).
pub(crate) fn class(rec: &[u8; 20]) -> (u8, u8) {
    let kind = rec[0] >> 5;
    let raw = match kind {
        1 | 2 => 1,
        3 | 4 => 2,
        5 => 3,
        k => k,
    };
    let mut c = raw;
    if c == 0 && u16::from_le_bytes([rec[4], rec[5]]) != 0xFFFF {
        c = 4;
    }
    (raw, c)
}

struct B<'a> {
    d: &'a DetailData,
    t: &'a HudTables,
    calls: Vec<MenuCall>,
    notes: Vec<String>,
}

impl B<'_> {
    fn c(&mut self, call: MenuCall) {
        self.calls.push(call);
    }
    /// 0x246FE0(line)+0xC / +0x10: the composed width and height.
    fn size(&mut self, message: u16) -> Option<(i32, i32)> {
        match self.t.message(message) {
            Some(codes) => {
                let (lines, w) = duel_hud::TextCanvas::measure(codes);
                Some((w as i32, 22 * lines as i32))
            }
            None => {
                self.notes.push(format!("message {message} was not imported"));
                None
            }
        }
    }
    /// 0x246B90 with tint `k` of 0x2BDCC0: `fit_w`/`fit_h` are the callers'
    /// limits (scale = limit / size once size > limit; 0 = none).
    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, band: u8, message: u16, x: i32, y: i32, fit_w: i32, fit_h: i32, tint: usize) {
        let Some((w, h)) = self.size(message) else { return };
        let sx = if fit_w > 0 && w > fit_w { fit_w as f32 / w as f32 } else { 1.0 };
        let sy = if fit_h > 0 && h > fit_h { fit_h as f32 / h as f32 } else { 1.0 };
        let rgba = self.d.tints[tint];
        self.c(MenuCall::TextTint { line: band, message, x, y, sx, sy, rgba });
    }
    fn boxed(&mut self, colour: i32, x: i32, y: i32, wu: i32, hu: i32) {
        self.c(MenuCall::Box { colour, x, y, wu, hu });
    }
}

/// The calls of 0x1BE6E0 for one frame, and what could not be drawn.
pub fn build(d: &DetailData, t: &HudTables, p: &Pane<'_>) -> (Vec<MenuCall>, Vec<String>) {
    let mut b = B { d, t, calls: Vec::new(), notes: Vec::new() };
    let off = if p.side != 0 { 0x200 } else { -0x100 };
    let card = p.card;
    let rec = if card == SPECIAL_CARD { None } else { p.cards.record(card).copied() };
    // s1 = 2 until a card (or 0x29F) sets the colour class.
    let mut colour = 2;
    let named = rec.is_some() || card == SPECIAL_CARD;
    if named {
        colour = rec.as_ref().map_or(0, |r| class(r).1) as i32 + 7;
        let name = NAME_MESSAGE + card;
        let sx = (p.x) + off;
        b.c(MenuCall::Panel { grad: colour, deco: -1, ptype: 0, x: sx + 7, y: p.y + 0x1F, wu: -0xD6, hu: -0x104, bright: true });
        b.c(MenuCall::Panel { grad: colour, deco: -1, ptype: 4, x: sx + 0x27, y: p.y + 0x10, wu: -0xB0, hu: -0x1E, bright: true });
        if let Some((w, _)) = b.size(name) {
            let f = if w >= 0xA1 { 160.0 / w as f32 } else { 1.0 };
            let half = ((w >> 1) as f32 * f) as i32;
            for (k, (dx, dy)) in [(0x81, 0x15), (0x80, 0x14), (0x7F, 0x13)].into_iter().enumerate() {
                let rgba = d.tints[k];
                b.c(MenuCall::TextTint { line: 1, message: name, x: sx + dx - half, y: p.y + dy, sx: f, sy: 1.0, rgba });
            }
        }
    }
    // The editor's library boxes (t3).
    if let Some(state) = p.editor_state {
        if state == 0 {
            b.boxed(2, p.x - 0xEC, p.y + 0x142, -0x40, -0x1C);
            b.c(MenuCall::Sprite { desc: 262, x: p.x - 0xE7, y: p.y + 0x14B });
            b.c(MenuCall::Sprite { desc: 196, x: p.x - 0xD0, y: p.y + 0x14A });
            b.boxed(2, p.x - 0xEC, p.y + 0x190, -0x40, -0x1C);
            b.c(MenuCall::Sprite { desc: 263, x: p.x - 0xE7, y: p.y + 0x199 });
            b.c(MenuCall::Sprite { desc: 197, x: p.x - 0xD0, y: p.y + 0x194 });
        } else {
            b.boxed(2, p.x - 0xF0, p.y + 0x161, -0x44, -0x2C);
            if state == 1 {
                b.c(MenuCall::Sprite { desc: 255, x: p.x - 0xD6, y: p.y + 0x166 });
            }
            b.c(MenuCall::Gold { x: p.x - 0xCE, y: p.y + 0x17B, size: 1, flags: 3, text: d.library.clone(), dim: false });
        }
    }
    // The tab block: box, the current page's tab, the four labels.
    let tx = p.x + off + if p.editor_state.is_some() { 0x50 } else { 0x30 };
    if p.tinted {
        b.c(MenuCall::BoxFilled { colour, x: tx, y: p.y + 0x13E, wu: -0xA0, hu: -0x72, fill: colour });
    } else {
        b.boxed(-1, tx, p.y + 0x13E, -0xA0, -0x72);
    }
    let tab_y = p.y + 0x142 + 26 * i32::from(p.page);
    if p.raised_tab {
        b.c(MenuCall::Panel { grad: -1, deco: -1, ptype: 6, x: tx + 8, y: tab_y - 2, wu: -0x8E, hu: -0x1C, bright: true });
    } else {
        b.boxed(2, tx + 0xA, tab_y, -0x8E, -0x1C);
    }
    for k in 0..4u8 {
        b.text(2 + k, 0xA4 + u16::from(k), tx + 0xE, p.y + 0x145 + 26 * i32::from(k), 0x86, 0, 3);
    }
    if named {
        match p.page {
            0 => page_data(&mut b, p, rec.as_ref()),
            1 => page_text(&mut b, p),
            2 => page_power_ups(&mut b, p, colour),
            3 => page_abilities(&mut b, p, colour),
            _ => {}
        }
    }
    (b.calls, b.notes)
}

/// 0x1BF1D0: picture, number, kind / type, attribute, ATK, DEF, level,
/// rank, DC.
fn page_data(b: &mut B<'_>, p: &Pane<'_>, rec: Option<&[u8; 20]>) {
    let off = if p.side != 0 { 0x200 } else { -0x100 };
    let card = p.card;
    let (kind, type_desc, rank, class_colour) = match rec {
        None => (0, 1usize, 0i32, 0u8),
        Some(r) => {
            let kind = r[0] >> 5;
            let (desc, rank) = if kind == 0 {
                (usize::from(r[0] & 0x1F), p.rank_of(card))
            } else {
                (0x14 + usize::from(kind), -1)
            };
            (kind, desc, rank, class(r).1)
        }
    };
    let attr = match rec {
        None => 1u8,
        Some(r) => {
            let raw = class(r).0;
            if raw > 0 { raw + 5 } else { r[2] & 0xF }
        }
    };
    let s3 = p.x + off;
    let (px, py) = (s3 + 0x87, p.y + 0x57);
    if p.picture {
        b.c(MenuCall::Picture { x: px, y: py, w: 0x56, h: 0x80 });
    } else {
        let black = [0, 0, 0, 0x80];
        let (x0, y0, x1, y1) = (px as f32, py as f32, (px + 0x56) as f32, (py + 0x80) as f32);
        b.c(MenuCall::Quad { corners: [(x0, y0, black), (x1, y0, black), (x0, y1, black), (x1, y1, black)] });
        let mut rgba = b.d.loading_rgba;
        rgba[3] = p.blink;
        b.c(MenuCall::GoldTint { x: s3 + 0xB2, y: p.y + 0x87, size: 1, flags: 2, text: b.d.loading.clone(), rgba });
    }
    b.c(MenuCall::Panel { grad: -1, deco: -1, ptype: 11, x: s3 + 0x82, y: p.y + 0x52, wu: -0x60, hu: -0x8A, bright: true });
    let number = b.d.number_line(card);
    b.c(MenuCall::Gold { x: s3 + 0xB2, y: p.y + 0xDE, size: 1, flags: 2, text: number, dim: false });
    let fp = s3 + 0x1C;
    let box_colour = i32::from(class_colour) + 7;
    // Line 2: the type (monster sub-type 0x26 + t, else kind 0x1F + k).
    let type_message = if rec.is_none() { 0x27 } else if kind == 0 { 0x26 + type_desc as u16 } else { 0x1F + u16::from(kind) };
    b.text(1, 0x9B, fp, p.y + 0x35, 0x28, 0, 0);
    b.boxed(box_colour, s3 + 0x44, p.y + 0x30, -0x9E, -0x20);
    b.c(MenuCall::Sprite { desc: type_desc, x: p.x + 0x48 + off, y: p.y + 0x34 });
    b.text(2, type_message, s3 + 0x64, p.y + 0x34, 0x78, 0, 3);
    if kind == 0 {
        b.text(3, 0x9C, fp, p.y + 0x56, 0x28, 0, 0);
        b.boxed(box_colour, s3 + 0x44, p.y + 0x51, -0x3C, -0x20);
        b.c(MenuCall::Attribute { icon: usize::from(attr), x: p.x + 0x56 + off, y: p.y + 0x55 });
        b.text(5, 0x9D, fp, p.y + 0x77, 0x28, 0, 0);
        b.boxed(box_colour, s3 + 0x44, p.y + 0x72, -0x3C, -0x20);
        if let Some(r) = rec {
            let atk = u16::from_le_bytes([r[8], r[9]]) & 0x1FFF;
            b.c(MenuCall::Number { x: s3 + 0x4F, y: p.y + 0x7D, size: 1, flags: 0, digits: 4, value: i32::from(atk), dim: false });
        }
        b.text(6, 0x9E, fp, p.y + 0x98, 0x28, 0, 0);
        b.boxed(box_colour, s3 + 0x44, p.y + 0x93, -0x3C, -0x20);
        if let Some(r) = rec {
            let def = u16::from_le_bytes([r[10], r[11]]) & 0x1FFF;
            b.c(MenuCall::Number { x: s3 + 0x4F, y: p.y + 0x9E, size: 1, flags: 0, digits: 4, value: i32::from(def), dim: false });
        }
        b.text(7, 0x9F, fp, p.y + 0xB3, 0x5E, 0, 0);
        b.boxed(box_colour, s3 + 0x1A, p.y + 0xC9, -0x66, -0x26);
        if let Some(r) = rec {
            let level = i32::from(r[2] >> 4);
            for i in 0..level {
                b.c(MenuCall::Sprite { desc: 31, x: p.x + 0x1D + off + 12 * (i % 6), y: p.y + 0xCC + 16 * (i / 6) });
            }
            b.c(MenuCall::Number { x: s3 + 0x72, y: p.y + 0xD6, size: 1, flags: 2, digits: 2, value: level, dim: false });
        }
        b.text(8, 0xA1, fp, p.y + 0xF4, 0x5E, 0, 0);
        b.c(MenuCall::InsigniaEx { x: s3 + 0x7D, y: p.y + 0xEF, rank, small: false, mirror: false });
        b.boxed(box_colour, s3 + 0x9E, p.y + 0xEF, -0x44, -0x20);
        if rec.is_some() {
            // 0x246D00: the rank's name at its own size.
            b.text(9, (0x53 + rank) as u16, s3 + 0xA7, p.y + 0xF5, 0, 0, 3);
        }
    }
    b.text(10, 0xA0, fp, p.y + 0x115, 0x5E, 0, 0);
    b.boxed(box_colour, s3 + 0x9E, p.y + 0x110, -0x44, -0x20);
    b.c(MenuCall::Sprite { desc: 216, x: p.x + 0xA5 + off, y: p.y + 0x118 });
    if let Some(r) = rec {
        b.c(MenuCall::Digits { x: s3 + 0xB9, y: p.y + 0x116, n: 2, value: i32::from(r[3]), narrow: true, dim: false });
    }
}

/// 0x1BFED0: the description (message 0x499 + card) squeezed into 198Ã—256.
fn page_text(b: &mut B<'_>, p: &Pane<'_>) {
    let off = if p.side != 0 { 0x200 } else { -0x100 };
    b.text(1, TEXT_MESSAGE + p.card, p.x + off + 0x1C, p.y + 0x30, 0xC6, 0x100, 0);
}

/// 0x1BFFF0: the power-ups the monster takes, one boxed name each.
fn page_power_ups(b: &mut B<'_>, p: &Pane<'_>, colour: i32) {
    if p.card >= MONSTER_LIMIT {
        return;
    }
    let off = if p.side != 0 { 0x200 } else { -0x100 };
    let x = p.x + off + 0x1C;
    for (k, id) in b.d.power_ups(p.card).into_iter().enumerate() {
        let y = p.y + 0x30 + 0x1A * k as i32;
        b.boxed(colour, x, y, -0xC6, -0x1A);
        b.text(1 + k as u8, NAME_MESSAGE + id, x + 5, y + 2, 0xBE, 0x16, 0);
    }
}

/// 0x1C01F0: the leader abilities 4..=16 the card has at the block's rank
/// (0x1E1F00(unit, a, 0, 1)); the labels (messages 0x62 + a) are composed
/// one after another, two-line ones taking two bands.
fn page_abilities(b: &mut B<'_>, p: &Pane<'_>, colour: i32) {
    if p.card >= MONSTER_LIMIT {
        return;
    }
    let off = if p.side != 0 { 0x200 } else { -0x100 };
    let row = b.d.abilities.get(usize::from(p.card)).copied().unwrap_or([0xFFFF; 20]);
    let rank = p.rank_of(p.card);
    let mut band = 1u8;
    let (sx, bx) = (p.x + 0x1C + off, p.x + off + 0x36);
    let mut y = p.y + 0x30;
    for a in 4..=16u8 {
        let message = 0x62 + u16::from(a);
        let lines = b.t.message(message).map_or(1, |c| duel_hud::TextCanvas::measure(c).0 as u8);
        if roses_core::duel::queries::record_ability(&row, a, rank) {
            b.c(MenuCall::Sprite { desc: 0x1E + usize::from(a), x: sx, y: y + 1 });
            b.boxed(colour, bx, y, -0xAC, -0x1A);
            b.text(band, message, bx + 5, y + 2, 0xA2, 0x16, 0);
            y += 0x1A;
        }
        band += lines;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mine(c: &MenuCall) -> Option<(String, Vec<i64>)> {
        let v = |xs: &[i32]| xs.iter().map(|&x| i64::from(x)).collect();
        let f = |x: f32| (f64::from(x) * 1e4).round() as i64;
        Some(match c {
            MenuCall::Panel { grad, deco, ptype, x, y, wu, hu, .. } => ("panel".into(), v(&[*grad, *deco, *ptype as i32, *x, *y, *wu, *hu])),
            MenuCall::Sprite { desc, x, y } => ("sprite".into(), v(&[*desc as i32, *x, *y])),
            MenuCall::Attribute { icon, x, y } => ("attr".into(), v(&[*icon as i32, *x, *y])),
            MenuCall::Gold { x, y, size, flags, .. } | MenuCall::GoldTint { x, y, size, flags, .. } => ("font".into(), v(&[*x, *y, *size as i32, *flags as i32])),
            MenuCall::Number { x, y, size, flags, digits, value, .. } => ("number".into(), v(&[*x, *y, *size as i32, *flags as i32, *digits, *value])),
            MenuCall::TextTint { x, y, sx, sy, .. } => ("text".into(), vec![i64::from(*x), i64::from(*y), f(*sx), f(*sy)]),
            MenuCall::InsigniaEx { x, y, rank, small, mirror } => ("insignia".into(), v(&[*x, *y, *rank, i32::from(*small), i32::from(*mirror)])),
            MenuCall::Box { x, y, wu, hu, colour } => ("f1b71d0".into(), v(&[*colour, *x, *y, *wu, *hu, -1])),
            MenuCall::BoxFilled { x, y, wu, hu, colour, fill } => ("f1b71d0".into(), v(&[*colour, *x, *y, *wu, *hu, *fill])),
            MenuCall::Digits { x, y, n, value, narrow, .. } => ("digits".into(), v(&[*x, *y, *n, *value, i32::from(*narrow)])),
            MenuCall::Quad { corners } => ("quad".into(), vec![corners[0].0 as i64, corners[0].1 as i64, corners[3].0 as i64, corners[3].1 as i64]),
            MenuCall::Picture { x, y, w, h } => ("cardpic".into(), v(&[*x, *y, *w, *h])),
            _ => return None,
        })
    }

    fn logged(c: &serde_json::Value) -> Option<(String, Vec<i64>)> {
        let a: Vec<i64> = c["args"].as_array()?.iter().map(|x| x.as_i64().unwrap()).collect();
        let fn_ = c["fn"].as_str()?;
        let f = |k: usize| (c["fs"][k].as_f64().unwrap() * 1e4).round() as i64;
        Some(match fn_ {
            "panel" => (fn_.into(), a[1..8].to_vec()),
            "sprite" if (0x28_F2A0..0x28_F300).contains(&a[1]) => ("attr".into(), vec![(a[1] - 0x28_F2A0) / 10, a[2] - 0x700, a[3] - 0x700]),
            "sprite" => (fn_.into(), vec![(a[1] - 0x28_F300) / 10, a[2] - 0x700, a[3] - 0x700]),
            "font" => (fn_.into(), vec![a[1], a[2], a[3], a[4]]),
            "number" => (fn_.into(), a[1..7].to_vec()),
            "text" => ("text".into(), vec![a[2], a[3], f(0), f(1)]),
            "msg246d00" => ("text".into(), vec![a[2], a[3], 10000, 10000]),
            "insignia" => (fn_.into(), vec![a[1], a[2], a[3], a[4], a[5]]),
            "f1b71d0" => (fn_.into(), a[1..7].to_vec()),
            "f1b7880" => ("digits".into(), a[1..6].to_vec()),
            "cardpic" => (fn_.into(), a[2..6].to_vec()),
            "f206f90" => {
                let hex = c["packet"].as_str()?;
                let b: Vec<u8> = (0..0x60).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()).collect();
                let xy = |o: usize| {
                    let w = u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
                    (i64::from(w & 0xFFFF) / 16 - 1792, i64::from(w >> 16) / 16 - 1792)
                };
                let ((x0, y0), (x1, y1)) = (xy(0x18), xy(0x20));
                ("quad".into(), vec![x0, y0, x1, y1])
            }
            _ => return None,
        })
    }

    /// The builder against the interpreter's logs (`ROSES_DETAIL_CAPTURES` =
    /// a path list: modes/panels/captures, files `d*.json` from
    /// `tools/run_detail.py` (Trade), and modes/ui3/captures, `dd-*.json`
    /// from `tools/run_duel_detail.py` (the duel's details view, overlay
    /// slot 1 code 0x11: the block is D5C's, the picture resident);
    /// `ROSES_DUEL_IMPORT` = a duel import): for every frame that calls
    /// 0x1BE6E0, the pane is built from the logged arguments, the page byte
    /// and the run's starting block, and every logged leaf call inside the
    /// pane (0x1BE6E0..0x1C04E0) must match in order with its values.
    #[test]
    fn private_pane_matches_the_emulated_draw() {
        let (Some(dir), Some(duel)) = (std::env::var_os("ROSES_DETAIL_CAPTURES"), std::env::var_os("ROSES_DUEL_IMPORT")) else {
            return;
        };
        let data = crate::duel_play::load(std::path::Path::new(&duel)).unwrap();
        let mut ids = HudTables::required_messages();
        ids.extend(message_ids());
        ids.sort_unstable();
        ids.dedup();
        let mut codes = std::collections::BTreeMap::new();
        for chunk in ids.chunks(256) {
            for m in roses_assets::parse_pal_messages(&data.messages, chunk).unwrap().messages {
                codes.insert(m.message_id, m.expanded_codes);
            }
        }
        let tables = HudTables::from_executable(&data.executable, codes).unwrap();
        let dd = DetailData::from_executable(&data.executable).unwrap();
        let cards = StaticCards::from_executable(&data.executable, 0).unwrap();
        let mut names: Vec<std::path::PathBuf> = std::env::split_paths(&dir)
            .flat_map(|d| std::fs::read_dir(d).unwrap().filter_map(|e| e.ok()).map(|e| e.path()))
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('d')) && p.extension().is_some_and(|e| e == "json"))
            .collect();
        names.sort();
        let (mut checked, mut pages) = (0, std::collections::BTreeSet::new());
        for name in names {
            let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&name).unwrap()).unwrap();
            let name = name.display();
            let hex = |s: &str| (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect::<Vec<u8>>();
            let blocks: Vec<CardBlock> = v["start_blocks"].as_array().unwrap().iter().map(|b| CardBlock::from_bytes(&hex(b.as_str().unwrap())).unwrap()).collect();
            let calls = v["calls"].as_array().unwrap();
            let frames = v["frames"].as_array().unwrap();
            let mut i = 0;
            while i < calls.len() {
                let c = &calls[i];
                if c["fn"] != "cardimage" {
                    i += 1;
                    continue;
                }
                let a: Vec<i64> = c["args"].as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect();
                let frame = c["frame"].as_i64().unwrap();
                // The pane's own calls up to the next pane or the frame's end.
                let mut j = i + 1;
                let mut theirs = Vec::new();
                let mut picture = false;
                while j < calls.len() && calls[j]["fn"] != "cardimage" && calls[j]["frame"].as_i64() == Some(frame) {
                    let ra = i64::from_str_radix(calls[j]["ra"].as_str().unwrap().trim_start_matches("0x"), 16).unwrap();
                    if (0x1B_E6E0..0x1C_04E0).contains(&ra) {
                        picture |= calls[j]["fn"] == "cardpic";
                        if let Some(l) = logged(&calls[j]) {
                            theirs.push(l);
                        }
                    }
                    j += 1;
                }
                let side = a[4] as u8;
                // The draw of update k runs after it: the frame entry k + 1
                // holds the state (the duel runs number their entries).
                let state = frames.iter().find(|f| f["frame"].as_i64() == Some(frame + 1)).unwrap_or(&frames[frame as usize + 1]);
                let page = state["keyidx"][usize::from(side)].as_u64().unwrap() as u8;
                // D5C (0x2DB8FC): the block 0x2154C0 / 0x215E60 read (Trade
                // sets it to the side).
                let d5c = state["d5c"].as_u64().map_or(side, |b| b as u8);
                let pane = Pane {
                    card: a[1] as u16,
                    x: a[2] as i32,
                    y: a[3] as i32,
                    side,
                    page,
                    raised_tab: a[5] != 0,
                    tinted: a[6] != 0,
                    editor_state: None,
                    block: &blocks[usize::from(d5c)],
                    cards: &cards,
                    picture,
                    blink: 0,
                    rank: None,
                };
                let (built, notes) = build(&dd, &tables, &pane);
                assert!(notes.is_empty(), "{notes:?}");
                let ours: Vec<_> = built.iter().filter_map(mine).collect();
                for (k, (x, y)) in ours.iter().zip(theirs.iter()).enumerate() {
                    assert_eq!(x, y, "{name} frame {frame} card {} page {page} call {k}", a[1]);
                }
                assert_eq!(ours.len(), theirs.len(), "{name} frame {frame} card {} page {page}: call count", a[1]);
                checked += 1;
                pages.insert((a[1], page));
                i = j;
            }
        }
        eprintln!("{checked} detail panes match ({} card/page pairs)", pages.len());
        assert!(checked > 0);
    }

    /// Every card's description (message 0x499 + card) and the other
    /// labels compose on the port's text canvas (`ROSES_DUEL_IMPORT`).
    #[test]
    fn private_every_pane_message_composes() {
        let Some(duel) = std::env::var_os("ROSES_DUEL_IMPORT") else { return };
        let data = crate::duel_play::load(std::path::Path::new(&duel)).unwrap();
        let ids = message_ids();
        let offset = roses_assets::decode_pal_name_font(&data.messages).unwrap().metadata.file_offset;
        let cells = &data.messages[offset..];
        let mut bad = Vec::new();
        for chunk in ids.chunks(256) {
            for m in roses_assets::parse_pal_messages(&data.messages, chunk).unwrap().messages {
                let mut canvas = duel_hud::TextCanvas::new();
                let (lines, w) = duel_hud::TextCanvas::measure(&m.expanded_codes);
                if let Err(e) = canvas.compose(1, &m.expanded_codes, duel_hud::TextFont { cells }) {
                    bad.push((m.message_id, e));
                } else if w == 0 && (TEXT_MESSAGE..TEXT_MESSAGE + 683).contains(&m.message_id) {
                    bad.push((m.message_id, format!("empty ({lines} lines)")));
                }
            }
        }
        assert!(bad.is_empty(), "{} messages: {:?}", bad.len(), &bad[..bad.len().min(12)]);
    }

    #[test]
    fn the_number_line_fills_its_one_field() {
        let mut d = DetailData {
            tints: [[0; 4]; 4],
            library: Vec::new(),
            back: Vec::new(),
            loading: Vec::new(),
            number: b"NO.%03d!".to_vec(),
            loading_rgba: [0; 4],
            reincarnation: Vec::new(),
            reincarnation_rgba: [0; 4],
            editor_background: [0; 8],
            abilities: Vec::new(),
            equips: vec![[0; 7]; 3],
        };
        assert_eq!(d.number_line(5), b"NO.005!");
        assert_eq!(d.number_line(1234), b"NO.1234!");
        d.equips[2] = [0b0000_0101, 0, 0, 0, 0, 0, 0];
        assert_eq!(d.power_ups(2), vec![0x2F0, 0x2F2]);
    }
}

