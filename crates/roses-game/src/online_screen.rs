//! Draws the Online Play screens ([`crate::online_menu::View`]) with the
//! pieces the original's own menus are made of (`duel_hud::MenuCall`):
//! Custom Duel's background gradient and title plate, its panels and the
//! selected-row frame, the gold font for headings and the message font
//! (MSG_E.DAT's glyphs) for every line of text, into a 512×512 frame as
//! `custom_duel_screen` does. The layout is the port's (these screens are
//! a port addition).
//!
//! The deck view ([`crate::online_menu::DeckView`]) is drawn with the deck
//! editor's own pieces instead (`deck_editor_screen::deck_view_screen`: its
//! background, title plate, sort header, DECK panel, leader portrait, card
//! rows and cursor), the card detail pane of the editor and Trade
//! (`card_detail`, 0x1BE6E0) on the right half while it is open, else a
//! panel with the leader, its rank and the buttons.

use crate::card_detail::{self, DetailData, Pictures};
use crate::custom_duel_screen::ScreenData;
use crate::deck_editor_screen::{self, DeckViewInputs};
use crate::online_menu::{DeckView, Row, View};
use crate::online_title::font_cell;
use roses_client::duel_hud::{self, HudTables, HudTextures, MenuCall, YesNoTables};
use roses_client::gs::Target16;
use roses_core::card_block::{CardBlock, EMPTY_CARD};
use roses_core::deck_editor::StaticCards;
use roses_net::lobby::PlayerState;
use std::collections::BTreeMap;
use std::path::Path;

pub const FRAME: usize = 512;
/// Message ids for the screen's own lines (no MSG_E.DAT message uses them).
const FIRST_LINE_MESSAGE: u16 = 0xF000;
/// The button sprites of the descriptor table the hints use (read from a
/// drawn frame: 251 the green triangle, 252 the pink square, 253 the
/// cross, 254 the small circle Custom Duel and the editor put before BACK
/// and EXIT).
const TRIANGLE_BUTTON: usize = 251;
const SQUARE_BUTTON: usize = 252;
const CROSS_BUTTON: usize = 253;
const CIRCLE_BUTTON: usize = 254;
/// Rows shown at once (the list scrolls with the cursor).
const ROWS_SHOWN: usize = 8;

/// The text-canvas codes of `text` (unknown characters become '?').
pub fn text_codes(text: &str) -> Vec<u16> {
    text.chars()
        .map(|c| match c {
            ' ' => 0x2001,
            c => 0x2000 + font_cell(c).unwrap_or(0x76) as u16,
        })
        .collect()
}

/// Upper-case letters and digits for the gold font (it has no other glyphs;
/// anything else becomes a space).
fn gold(text: &str) -> Vec<u8> {
    text.bytes()
        .map(|b| match b {
            b'a'..=b'z' => b.to_ascii_uppercase(),
            b'A'..=b'Z' | b'0'..=b'9' | b'?' => b,
            _ => b' ',
        })
        .collect()
}

pub struct OnlineScreen {
    tables: HudTables,
    yes_no: YesNoTables,
    textures: HudTextures,
    data: ScreenData,
    pub target: Target16,
    next_message: u16,
    /// The deck view's pieces: the detail pane's data, the card records,
    /// PICMINI (the leader's portrait), PICTURE.MRG when the duel-end
    /// import has it (else the pane shows NOW LOADING), the leader shown.
    detail: DetailData,
    cards: StaticCards,
    picmini: Vec<u8>,
    pictures: Option<Pictures>,
    leader_shown: u16,
    block: CardBlock,
}

impl OnlineScreen {
    /// From the duel import's executable, MSG_E.DAT, TEXSYS and PICMINI.
    pub fn new(executable: &[u8], messages: &[u8], texsys: &[u8], picmini: &[u8]) -> Result<Self, String> {
        // The card names, descriptions and the pane's labels (the deck view).
        let mut codes = BTreeMap::new();
        let mut ids = HudTables::required_messages();
        ids.extend(card_detail::message_ids());
        ids.sort_unstable();
        ids.dedup();
        for chunk in ids.chunks(256) {
            for m in roses_assets::parse_pal_messages(messages, chunk).map_err(|e| e.to_string())?.messages {
                codes.insert(m.message_id, m.expanded_codes);
            }
        }
        Ok(Self {
            tables: HudTables::from_executable(executable, codes)?,
            yes_no: YesNoTables::from_executable(executable)?,
            textures: HudTextures::new(texsys, picmini, [0, 0], messages)?,
            data: ScreenData::from_executable(executable)?,
            target: Target16 { width: FRAME, height: FRAME, pixels: vec![0; FRAME * FRAME] },
            next_message: FIRST_LINE_MESSAGE,
            detail: DetailData::from_executable(executable)?,
            cards: StaticCards::from_executable(executable, 0).ok_or("the executable is too short for the card records")?,
            picmini: picmini.to_vec(),
            pictures: None,
            leader_shown: 0,
            block: CardBlock::empty(0),
        })
    }

    /// The card pictures (PICTURE.MRG of the duel-end import) for the
    /// detail pane.
    pub fn set_pictures(&mut self, archive: Vec<u8>, executable: &[u8]) -> Result<(), String> {
        self.pictures = Some(Pictures::new(archive, executable)?);
        Ok(())
    }

    /// The card records (the deck view's sort keys use the same).
    pub fn cards(&self) -> &StaticCards {
        &self.cards
    }

    pub fn from_duel_import(duel: &Path) -> Result<Self, String> {
        let d = crate::duel_play::load(duel)?;
        Self::new(&d.executable, &d.messages, &d.texsys, &d.picmini)
    }

    fn line(&mut self, calls: &mut Vec<MenuCall>, text: &str, x: i32, y: i32, limit: i32, dim: bool) {
        if text.is_empty() {
            return;
        }
        let id = self.next_message;
        self.next_message = self.next_message.wrapping_add(1).max(FIRST_LINE_MESSAGE);
        let codes = text_codes(text);
        let w = 12 * codes.len() as i32;
        self.tables.set_message(id, codes);
        let sx = if w > limit { limit as f32 / w as f32 } else { 1.0 };
        calls.push(MenuCall::Text { line: 1, message: id, x, y, sx, sy: 1.0, lines: 1, dim });
    }

    /// The calls of one frame.
    pub fn calls(&mut self, view: &View, blink: bool) -> Vec<MenuCall> {
        self.next_message = FIRST_LINE_MESSAGE;
        let mut c = Vec::new();
        // 0x1C85E0's background: edges → centre → edges.
        let bg = self.data.background;
        let edge = [bg[2], bg[1], bg[0], bg[3]];
        let mid = [bg[6], bg[5], bg[4], bg[7]];
        for (x0, x1, a, z) in [(0.0, 256.0, edge, mid), (256.0, 512.0, mid, edge)] {
            c.push(MenuCall::Quad { corners: [(x0, 0.0, a), (x1, 0.0, z), (x0, 512.0, a), (x1, 512.0, z)] });
        }
        // The title plate as Custom Duel draws its own (0x1C8A40).
        c.push(MenuCall::Tiles { desc: 189, x: 0xC0, y: 0x10, nx: 4, ny: 1 });
        c.push(MenuCall::Panel { grad: -1, deco: 0x73, ptype: 4, x: 0xC0, y: 0x10, wu: 13, hu: 2, bright: true });
        c.push(MenuCall::Gold { x: 0x100, y: 0x17, size: 2, flags: 3, text: gold("ONLINE"), dim: false });
        c.push(MenuCall::Gold { x: 0x100, y: 0x36, size: 2, flags: 3, text: gold("PLAY"), dim: false });
        // The main panel and its heading plate.
        let (px, py, pw, ph) = (0x18, 0x6C, 0x1D0, 0x16C);
        c.push(MenuCall::Panel { grad: 0, deco: 0, ptype: 0, x: px, y: py, wu: -pw, hu: -ph, bright: true });
        c.push(MenuCall::Panel { grad: 12, deco: 3, ptype: 4, x: 0x100 - 0x59, y: py - 0x10, wu: -0xB2, hu: -0x20, bright: true });
        c.push(MenuCall::Gold { x: 0x100, y: py - 0x09, size: 1, flags: 3, text: gold(&view.heading), dim: false });
        let mut y = py + 0x1C;
        for line in view.info.iter().take(6) {
            self.line(&mut c, line, px + 0x14, y, pw - 0x28, false);
            y += 22;
        }
        y += 8;
        // The rows (scrolled so the cursor stays in view); a long list
        // keeps room for its "n / total" line.
        let room = usize::try_from((py + ph - 0x26 - y) / 0x1E).unwrap_or(1).max(1);
        let shown = if view.rows.len() > room { room.min(ROWS_SHOWN) } else { ROWS_SHOWN };
        let first = view.cursor.saturating_sub(shown - 1).min(view.rows.len().saturating_sub(shown));
        // With a terrain preview the rows keep to the left of it.
        let (row_x, row_w) = if view.map.is_some() { (px + 0x10, 0xC0) } else { (px + 0x30, pw - 0x60) };
        for (i, row) in view.rows.iter().enumerate().skip(first).take(shown) {
            let selected = i == view.cursor;
            let (x, w) = (row_x, row_w);
            match row {
                Row::Button(t) => self.line(&mut c, t, x + 8, y + 3, w - 16, !selected),
                Row::Field(label, value) => {
                    let caret = if selected && view.typing && blink { "-" } else { "" };
                    self.line(&mut c, &format!("{label}: {value}{caret}"), x + 8, y + 3, w - 16, !selected);
                }
                Row::Player(name, state) => {
                    self.line(&mut c, name, x + 8, y + 3, 0x90, !selected);
                    let s = match state {
                        PlayerState::Idle => "ready",
                        PlayerState::InRoom => "in a room",
                        PlayerState::InMatch => "dueling",
                    };
                    self.line(&mut c, s, x + w - 0x78, y + 3, 0x70, true);
                }
                Row::Deck { name, tag, detail, ready } => {
                    self.line(&mut c, name, x + 8, y + 3, 0xD0, !selected);
                    self.line(&mut c, detail, x + w - 0x90, y + 3, 0x48, !*ready || !selected);
                    self.line(&mut c, tag, x + w - 0x44, y + 3, 0x3C, true);
                }
                Row::Choice(label, value) => {
                    self.line(&mut c, label, x + 8, y + 3, 0xB0, !selected);
                    self.line(&mut c, value, x + w - 0xA0, y + 3, 0x98, !selected);
                }
                Row::Seat { seat, name, team, you } => {
                    // MODES (a port addition): S / N / E / W, who sits
                    // there, the 2v2 team; this player's seat bright.
                    self.line(&mut c, seat, x + 8, y + 3, 0x18, !selected);
                    let shown = if *you { format!("{name} (you)") } else { name.clone() };
                    self.line(&mut c, &shown, x + 0x2C, y + 3, w - 0xA0, !selected && !*you);
                    if let Some(t) = team {
                        self.line(&mut c, t, x + w - 0x70, y + 3, 0x68, true);
                    }
                }
                Row::Live { title, round, watching } => {
                    self.line(&mut c, title, x + 8, y + 3, 0xB0, !selected);
                    self.line(&mut c, &format!("Round {round}"), x + w - 0xB0, y + 3, 0x58, true);
                    if *watching > 0 {
                        self.line(&mut c, &format!("{watching} watching"), x + w - 0x54, y + 3, 0x50, true);
                    }
                }
            }
            if selected {
                c.push(MenuCall::Panel { grad: -1, deco: -1, ptype: 6, x, y, wu: -w, hu: -0x1C, bright: true });
            }
            y += 0x1E;
        }
        if view.rows.len() > shown {
            let more = format!("{} / {}", view.cursor + 1, view.rows.len());
            self.line(&mut c, &more, px + pw - 0x60, py + ph - 0x24, 0x50, true);
        }
        // Custom Duel's terrain preview of the board (its legend's right
        // edge at the panel's).
        if let Some(m) = &view.map {
            let (calls, notes) = crate::custom_duel_screen::terrain_preview(&self.data, &self.tables, px + pw - 0x1BB, py + 0x38, m.cells.as_deref());
            c.extend(calls);
            for n in notes {
                eprintln!("Online Play: map preview: {n}");
            }
        }
        // The note box at the bottom.
        if let Some(note) = &view.note {
            c.push(MenuCall::Box { colour: 1, x: 0x18, y: 0x1DC, wu: -0x1D0, hu: -0x1C });
            self.line(&mut c, note, 0x24, 0x1DF, 0x1B8, false);
        }
        c
    }

    fn render(&mut self, calls: &[MenuCall], notes: &mut Vec<String>) {
        let frame = duel_hud::menu_frame(&self.tables, &self.yes_no, calls);
        notes.extend(frame.unsupported.iter().map(|u| format!("unsupported {:?}", u.kind)));
        for s in duel_hud::render_primitives(&frame.prims, &self.tables, &mut self.textures, &mut self.target) {
            notes.push(format!("primitive {} skipped: {}", s.index, s.reason));
        }
    }

    /// One labelled button hint: the button's sprite and its word.
    fn hint(&mut self, c: &mut Vec<MenuCall>, sprite: Option<usize>, label: &str, x: i32, y: i32) {
        if let Some(desc) = sprite {
            c.push(MenuCall::Sprite { desc, x, y: y + 1 });
        }
        self.line(c, label, x + 24, y, 0xC0, false);
    }

    /// The deck view: the editor's pieces, the detail pane or the leader
    /// panel, the row count and the note.
    fn draw_deck(&mut self, view: &View, d: &DeckView, blink: bool) -> Vec<String> {
        self.next_message = FIRST_LINE_MESSAGE;
        let mut notes = Vec::new();
        if d.leader != EMPTY_CARD && d.leader != self.leader_shown {
            match self.textures.set_leaders(&self.picmini, [d.leader, d.leader]) {
                Ok(()) => self.leader_shown = d.leader,
                Err(e) => notes.push(format!("leader portrait {}: {e}", d.leader)),
            }
        }
        let background = deck_editor_screen::background_calls(self.detail.editor_background);
        self.render(&background, &mut notes);
        let (prims, list_notes) = deck_editor_screen::deck_view_screen(&DeckViewInputs {
            tables: &self.tables,
            cards: &self.cards,
            leader: d.leader,
            groups: &d.groups,
            cursor: d.cursor,
            first: d.first,
            sort_key: d.sort_key,
            sort_label: &gold(&d.sort_label),
            pane_open: d.detail.is_some(),
            tag: &gold(&d.tag),
        });
        notes.extend(list_notes.into_iter().map(|n| n.0));
        for s in duel_hud::render_primitives(&prims, &self.tables, &mut self.textures, &mut self.target) {
            notes.push(format!("primitive {} skipped: {}", s.index, s.reason));
        }
        let mut c = Vec::new();
        let card = if d.cursor == 0 { Some(d.leader) } else { d.groups.get(d.cursor - 1).map(|g| g.0) };
        match (d.detail, card.filter(|&c| c != EMPTY_CARD)) {
            (Some(page), Some(card)) => {
                let texture = self.pictures.as_mut().and_then(|p| p.texture(card, &self.block));
                let rank = if card < roses_core::duel::RANKED_CARDS { i32::from(d.rank) } else { -1 };
                let pane = card_detail::Pane {
                    card,
                    x: -0x100,
                    y: 0x20,
                    side: 1,
                    page,
                    raised_tab: true,
                    tinted: false,
                    editor_state: None,
                    block: &self.block,
                    cards: &self.cards,
                    picture: texture.is_some(),
                    blink: if blink { 0x80 } else { 0x20 },
                    rank: Some(rank),
                };
                let (calls, pane_notes) = card_detail::build(&self.detail, &self.tables, &pane);
                notes.extend(pane_notes);
                self.textures.picture = texture;
                c.extend(calls);
            }
            _ => {
                // The leader panel and the buttons.
                let (px, py, pw, ph) = (264, 112, 232, 322);
                c.push(MenuCall::Panel { grad: 0, deco: 0, ptype: 0, x: px, y: py, wu: -pw, hu: -ph, bright: true });
                let (x, w) = (px + 12, pw - 24);
                let mut y = py + 12;
                self.line(&mut c, &d.name, x, y, w, false);
                y += 24;
                self.line(&mut c, &format!("Leader: {}", d.leader_name), x, y, w, false);
                y += 22;
                self.line(&mut c, &format!("Rank: {}", d.rank_name), x, y, w - 40, false);
                if d.rank > 0 {
                    c.push(MenuCall::InsigniaEx { x: px + pw - 44, y: y - 1, rank: i32::from(d.rank), small: false, mirror: false });
                }
                y += 22;
                let state = match &d.problem {
                    None => format!("{} cards, ready to play", d.count),
                    Some(p) => format!("Not ready: {p}"),
                };
                self.line(&mut c, &state, x, y, w, d.problem.is_some());
                y += 34;
                self.hint(&mut c, Some(TRIANGLE_BUTTON), "CARD DETAIL", x, y);
                y += 24;
                c.push(MenuCall::Sprite { desc: 262, x, y: y + 4 });
                self.line(&mut c, "SORT", x + 48, y, 0x60, false);
                c.push(MenuCall::Sprite { desc: 263, x: x + 22, y: y + 4 });
                y += 24;
                if d.can_copy {
                    self.hint(&mut c, Some(SQUARE_BUTTON), "COPY TO MY DECKS", x, y);
                    y += 24;
                }
                if let Some(cross) = &d.cross {
                    let label = match cross.as_str() {
                        "CHOOSE" => "BRING THIS DECK",
                        "EDIT" => "EDIT ITS CARDS",
                        _ => "COPY TO MY DECKS",
                    };
                    if !(cross == "COPY" && d.can_copy) {
                        self.hint(&mut c, Some(CROSS_BUTTON), label, x, y);
                        y += 24;
                    }
                }
                self.hint(&mut c, Some(CIRCLE_BUTTON), "BACK", x, y);
            }
        }
        // Which row of how many.
        let rows = 1 + d.groups.len();
        self.line(&mut c, &format!("{} / {rows}", d.cursor + 1), 0x18, 0x1BC, 0x50, true);
        if d.detail.is_some() {
            self.line(&mut c, "Left / Right: page", 0x70, 0x1BC, 0x88, true);
        }
        if let Some(note) = &view.note {
            c.push(MenuCall::Box { colour: 1, x: 0x18, y: 0x1DC, wu: -0x1D0, hu: -0x1C });
            self.line(&mut c, note, 0x24, 0x1DF, 0x1B8, false);
        }
        self.render(&c, &mut notes);
        notes
    }

    /// Draw a frame; returns what could not be drawn.
    pub fn draw(&mut self, view: &View, blink: bool) -> Vec<String> {
        if let Some(d) = &view.deck {
            return self.draw_deck(view, d, blink);
        }
        let calls = self.calls(view, blink);
        let frame = duel_hud::menu_frame(&self.tables, &self.yes_no, &calls);
        let mut notes: Vec<String> = frame.unsupported.iter().map(|u| format!("unsupported {:?}", u.kind)).collect();
        for s in duel_hud::render_primitives(&frame.prims, &self.tables, &mut self.textures, &mut self.target) {
            notes.push(format!("primitive {} skipped: {}", s.index, s.reason));
        }
        notes
    }

    pub fn rgba(&self) -> Vec<u8> {
        self.target.to_rgba8(0, 0, FRAME, FRAME)
    }
}
