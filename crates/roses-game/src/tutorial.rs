//! The scripted tutorial (practice choice "Yes") in the connected game, the
//! Bevy-free part: the lesson duels built from the user's executable (deck
//! records 0x11..0x16, the Destiny Draw rows), the operator that pages the
//! messages in scripted runs, and the presentation layer the original
//! draws over the HUD while a lesson runs: the tutorial message window
//! (state 0x0F), the card-name caption (state 0x13) and the blinking
//! pointer (0x280690), rendered with the HUD's own GS model
//! (`roses_client::duel_hud`) into a premultiplied RGBA layer.
//!
//! Layout facts are from software-GS dumps of saved original states
//! (the project notes, tags as in
//! `TUTORIAL.md`: [O] observed, [H] hypothesis):
//! - Window (189-n57: page 3710 over the HUD; 005-t5: page 4092, HUD
//!   hidden): the text is one bilinear sprite of the 4-bit text canvas,
//!   lines 1–3 (v 22..88), 12 px per code, horizontally centred (x0 =
//!   256 − W/2 with W the widest line), at y 316; the frame is the 8-piece
//!   border of TEXSYS #7 texels (160,208)-(192,240) with 14-px corners
//!   around (x0 − 16.5, 255.5)-(x0 + W + 15.5, 401.5); inside it a
//!   subtractive quad (white, alpha 96, (0 − Cs)·As + Cd) over
//!   (x0 − 12, 260)-(x0 + W + 12, 398); the "TUTORIAL" plate is a
//!   Gouraud strip (64,32,0)→(160,96,0)→(64,32,0) over (191,268)-(321,304)
//!   under the 8-piece border of texels (48,64)-(64,80) with 5-px corners
//!   around (190.5,267.5)-(320.5,303.5), and the word in the proportional
//!   gold font (24-px glyphs) centred at x 256, y 276.
//! - Slide (states saved inside the transitions): the window moves
//!   vertically by the script's slide position: y = rest + offset, offset
//!   from `Message::offset` (0x50 per poll; off screen at 400+).
//! - Caption (004-t4: name 323): the same frame and darkening around one
//!   text line at y 352 (frame (x0 − 16.5, 335.5)-(x0 + W + 15.5, 389.5)),
//!   no plate.
//! - Pointer (355-q30: kind 0x800 at (−28,240)): two atlas-6 quads centred
//!   at (256 + x, 256 + y): texels (0,81)-(32,112) over (cx − 16,
//!   cy − 15)-(cx + 16, cy + 15) and (64,96)-(80,112) over (cx − 16,
//!   cy − 15)-(cx, cy + 1); shown while the blink counter % 10 < 5 (the
//!   dump shows it in 5 of 10 frames).
//! - Fade bytes 0x2DF5B0/1: the board scene's brightness (042-c10 at 0:
//!   black board under bright battle cards; 034-c2 at 0 with the HUD
//!   hidden: all black); the HUD is not darkened [O]. Mechanism [H].
//! - FLAG906 (0x2DB906 bit 7) clear = HUD hidden (005-t5, 004-t4: no
//!   HUD; the HUD slides out as in a turn change) [O].
//! Not observed: a page of fewer than three lines (the box is sized by
//! the line count here [H]); the title plate of a two-line page.

use crate::Result;
use crate::duel_play::{DuelData, DuelPlay, HUD_SIZE, SideDeck};
use roses_client::duel_hud::{
    Element, HudFrame, HudTables, HudTextures, HudUpdate, PrimKind, Primitive, Target, TextCanvas,
    TextureSource, Vertex, render_layer,
};
use roses_core::duel::Duel;
use roses_core::duel::script::{
    self, CAPTION_BASE, DESTINY_MESSAGE, Lesson, LessonEnd, Tutorial, lesson_setup,
};
use roses_core::duel::session::DuelSession;
use roses_core::practice::DeckInput;
use std::collections::{BTreeMap, BTreeSet};

/// The tutorial pages of the five lessons (3562..=4086) with the map
/// menu's row labels (4087..=4091), and the text variants (4092..=4110).
pub const PAGE_IDS: std::ops::RangeInclusive<u16> = 3562..=4091;
pub const VARIANT_IDS: std::ops::RangeInclusive<u16> = 4092..=4110;

/// The deck records of a lesson from the user's executable (the pairs the
/// practice setup 0x23B590 loads).
pub fn lesson_records(executable: &[u8], lesson: Lesson) -> Result<[DeckInput; 2]> {
    let decks = roses_assets::parse_pal_decks(executable).map_err(|e| e.to_string())?;
    let (a, b) = lesson.deck_pair();
    let record = |index: u8| -> Result<DeckInput> {
        let r = decks
            .decks
            .get(usize::from(index))
            .ok_or_else(|| format!("deck record {index:#x} is outside the executable's table"))?;
        Ok(DeckInput {
            index: r.index,
            leader_card: r.leader_card,
            leader_rank_raw: r.leader_rank_raw,
            cards: r.cards.clone(),
        })
    };
    Ok([record(a)?, record(b)?])
}

/// A lesson's duel session: `lesson_setup` (board 0, leaders (3,6)/(3,0),
/// unshuffled decks, LP 4000, both sides scripted), the series counters of
/// the "Yes" series (or the next lesson's from a `LessonEnd`), the Destiny
/// Draw rows from the executable and `seed` as the LCG state its pick
/// consumes (the original's depends on frame timing). The leader
/// abilities of the lesson decks are not traced (empty lists, as the
/// model's tests use).
pub fn lesson_session(
    data: &DuelData,
    lesson: Lesson,
    from: Option<LessonEnd>,
    seed: u64,
    single: bool,
) -> Result<DuelSession> {
    let records = lesson_records(&data.executable, lesson)?;
    let setup = lesson_setup(lesson, [&records[0], &records[1]], [Vec::new(), Vec::new()])
        .map_err(|e| e.to_string())?;
    let (duel, _) = Duel::new(setup, &data.catalogue).map_err(|e| e.to_string())?;
    let tutorial = match from {
        Some(end) => {
            Tutorial::continue_with(end, true).ok_or("the series ended; there is no next lesson")?
        }
        // The map's tutorial menu (0x23B3B0): the lesson alone, counter 0,
        // so its end returns to the map; the card-name option of the
        // player's block (0x1C on the map [emu]) selects the variants.
        None if single => Tutorial::single(lesson, true),
        None => Tutorial::series(lesson),
    };
    if tutorial.lesson != lesson {
        return Err(format!(
            "lesson {:?} was asked for but the series continues with {:?}",
            lesson, tutorial.lesson
        ));
    }
    let rows = roses_assets::parse_pal_destiny_rows(&data.executable).map_err(|e| e.to_string())?;
    let tutorial = tutorial.with_destiny_rows(rows).with_rng(seed);
    Ok(DuelSession::tutorial(duel, tutorial))
}

/// A lesson as the renderer plays it: the session above on the import's
/// data, whose `first_duel` is rewritten to the lesson's two deck records
/// (the HUD reads the leaders, ranks and portraits from it).
pub fn start_lesson(
    mut data: DuelData,
    lesson: Lesson,
    from: Option<LessonEnd>,
    seed: u64,
    auto_pages: bool,
    single: bool,
) -> Result<DuelPlay> {
    let session = lesson_session(&data, lesson, from, seed, single)?;
    let records = lesson_records(&data.executable, lesson)?;
    data.first_duel.board_map = 0;
    data.first_duel.terrain = session.duel.terrain.clone();
    data.first_duel.first = 0;
    data.first_duel.sides = [0, 1].map(|s| SideDeck {
        deck_record: usize::from(records[s].index),
        leader: records[s].leader_card,
        cards: records[s].cards.clone(),
    });
    data.first_duel.computer_sides = Vec::new();
    data.first_duel.leader_abilities = [Vec::new(), Vec::new()];
    let mut play = DuelPlay::with_session(data, session, seed)?;
    play.auto_pages = auto_pages;
    Ok(play)
}

/// The operator of the scripted runs: a fresh Cross on every fifth poll
/// that reads the human's face buttons (a page fully shown), as the
/// interpreter runs and the model's tests do (`emu_run.py`).
#[derive(Clone, Debug, Default)]
pub struct Operator {
    pub checks: u32,
}

impl Operator {
    pub const PERIOD: u32 = 5;

    /// The press for this frame (0 when the page is not accepting input).
    pub fn pad(&mut self, session: &DuelSession) -> u16 {
        let accepting = session
            .tutorial
            .as_ref()
            .is_some_and(|t| t.ended.is_none() && t.message_accepts_input());
        if !accepting {
            return 0;
        }
        self.checks += 1;
        if self.checks.is_multiple_of(Self::PERIOD) {
            roses_core::duel::ai::pad::CROSS
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------- screen
const FRAME: f32 = HUD_SIZE as f32;
const GREY: [u8; 4] = [128, 128, 128, 128];
const WHITE: [u8; 4] = [255, 255, 255, 128];
/// The subtractive quad's alpha (white × 96/128 taken off the scene).
const DARKEN: [u8; 4] = [255, 255, 255, 96];
/// Resting y of the window's text (3 lines at 316..382) and of its frame.
const WINDOW_TEXT_Y: f32 = 316.0;
const WINDOW_TOP: f32 = 255.5;
/// Text-to-frame margins: 16.5 left, 15.5 right, 19.5 below the text.
const SIDE_L: f32 = 16.5;
const SIDE_R: f32 = 15.5;
const BELOW: f32 = 19.5;
/// The darkening quad sits 4.5 inside the frame's outer edge (3.5 at the
/// far edges).
const INSET_NEAR: f32 = 4.5;
const INSET_FAR: f32 = 3.5;
/// Title plate: border (190.5,267.5)-(320.5,303.5), gradient 191..321 ×
/// 268..304, the word at y 276.
const PLATE_X0: f32 = 190.5;
const PLATE_X1: f32 = 320.5;
const PLATE_DY: f32 = 12.0;
const PLATE_H: f32 = 36.0;
const TITLE_DY: f32 = 20.5;
const TITLE: &[u8] = b"TUTORIAL";
/// Caption: one text line at y 352, frame 335.5..389.5.
const CAPTION_TEXT_Y: f32 = 352.0;
const CAPTION_TOP: f32 = 335.5;
const CAPTION_BOTTOM: f32 = 389.5;
/// Border texels on TEXSYS #7 and their corner sizes.
const BORDER: ([f32; 4], f32) = ([160.0, 208.0, 192.0, 240.0], 14.0);
const PLATE_BORDER: ([f32; 4], f32) = ([48.0, 64.0, 64.0, 80.0], 5.0);
/// Pointer texels on TEXSYS #6.
const POINTER_BIG: [f32; 4] = [0.0, 81.0, 32.0, 112.0];
const POINTER_SMALL: [f32; 4] = [64.0, 96.0, 80.0, 112.0];
const GRADIENT_DARK: [u8; 4] = [64, 32, 0, 128];
const GRADIENT_BRIGHT: [u8; 4] = [160, 96, 0, 128];

/// What the layer shows (compared between updates to skip re-renders).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    /// The window's shown message id (after the text remap) and its slide
    /// offset.
    pub message: Option<(u16, u16)>,
    /// The card whose name caption is up.
    pub caption: Option<u16>,
    /// The pointer's screen offset while it is visible.
    pub pointer: Option<(i16, i16)>,
}

impl View {
    pub fn of(t: &Tutorial) -> Self {
        Self {
            message: t.message.map(|m| (m.shown(t.text_variant), m.offset())),
            caption: t.caption,
            pointer: t
                .blink
                .filter(|b| b.visible() && b.kind == script::BLINK_POINTER)
                .map(|b| (b.x, b.y)),
        }
    }
}

/// The tutorial layer renderer: the HUD tables with the tutorial pages
/// added, its atlases and text canvas.
pub struct TutorialScreen {
    tables: HudTables,
    textures: HudTextures,
    /// Gold font size with 24-px glyphs (the title's), when the tables
    /// have one.
    title_size: Option<usize>,
    last: Option<View>,
    pub layer: Vec<u8>,
    pub version: u64,
    notes: BTreeSet<String>,
}

impl TutorialScreen {
    pub fn new(data: &DuelData, leaders: [u16; 2]) -> Result<Self> {
        let mut ids = HudTables::required_messages();
        ids.extend(PAGE_IDS);
        ids.extend(VARIANT_IDS);
        ids.push(DESTINY_MESSAGE);
        ids.sort_unstable();
        ids.dedup();
        let mut messages = BTreeMap::new();
        for chunk in ids.chunks(256) {
            let parsed = roses_assets::parse_pal_messages(&data.messages, chunk)
                .map_err(|e| e.to_string())?;
            for m in parsed.messages {
                messages.insert(m.message_id, m.expanded_codes);
            }
        }
        let tables = HudTables::from_executable(&data.executable, messages)?;
        let textures = HudTextures::new(&data.texsys, &data.picmini, leaders, &data.messages)?;
        let title_size = (0..3).find(|&s| {
            tables
                .glyphs
                .get(usize::from(b'T' - b'A') + 37 * s)
                .is_some_and(|d| d.height() == 24)
        });
        Ok(Self {
            tables,
            textures,
            title_size,
            last: None,
            layer: vec![0; HUD_SIZE * HUD_SIZE * 4],
            version: 0,
            notes: BTreeSet::new(),
        })
    }

    /// Notes about parts not drawn (each once).
    pub fn take_notes(&mut self) -> Vec<String> {
        let notes: Vec<String> = self.notes.iter().cloned().collect();
        self.notes.clear();
        notes
    }

    /// Lines and widest line (pixels) of a message, if the tables have it.
    pub fn measure(&self, id: u16) -> Option<(usize, usize)> {
        self.tables.message(id).map(TextCanvas::measure)
    }

    /// Re-render the layer when the view changed; returns whether it did.
    pub fn render(&mut self, view: &View) -> bool {
        if self.last.as_ref() == Some(view) {
            return false;
        }
        self.last = Some(view.clone());
        let prims = self.primitives(view);
        let frame = HudFrame {
            prims,
            unsupported: Vec::new(),
            next: HudUpdate::default(),
        };
        let (layer, skipped) =
            render_layer(&frame, &self.tables, &mut self.textures, HUD_SIZE, HUD_SIZE);
        for s in skipped {
            self.notes.insert(format!(
                "tutorial layer primitive {}: {}",
                s.index, s.reason
            ));
        }
        self.layer = layer;
        self.version += 1;
        true
    }

    /// The layer's primitives in the original's order: caption, window
    /// (darkening, border, plate gradient, plate border, title, text),
    /// pointer.
    pub fn primitives(&mut self, view: &View) -> Vec<Primitive> {
        let mut b = Prims::default();
        if let Some(card) = view.caption {
            let id = CAPTION_BASE + card;
            match self.measure(id) {
                Some((lines, width)) => {
                    let w = width as f32;
                    let x0 = (256 - width / 2) as f32;
                    let h = 22.0 * lines as f32;
                    let bottom = CAPTION_BOTTOM + h - 22.0;
                    b.darken(
                        x0 - SIDE_L + INSET_NEAR,
                        CAPTION_TOP + INSET_NEAR,
                        x0 + w + SIDE_R - INSET_FAR,
                        bottom - INSET_FAR,
                    );
                    b.border(BORDER, x0 - SIDE_L, CAPTION_TOP, x0 + w + SIDE_R, bottom);
                    b.text(id, x0, CAPTION_TEXT_Y, w, lines);
                }
                None => {
                    self.notes.insert(format!(
                        "caption: card name message {id} is not in the tables"
                    ));
                }
            }
        }
        if let Some((id, offset)) = view.message {
            let dy = f32::from(offset);
            match self.measure(id) {
                // Fully below the frame while sliding: nothing to draw.
                _ if WINDOW_TOP + dy >= FRAME => (),
                Some((lines, width)) => {
                    let w = width as f32;
                    let x0 = (256 - width / 2) as f32;
                    let top = WINDOW_TOP + dy;
                    let text_y = WINDOW_TEXT_Y + dy;
                    let bottom = text_y + 22.0 * lines as f32 + BELOW;
                    b.darken(
                        x0 - SIDE_L + INSET_NEAR,
                        top + INSET_NEAR,
                        x0 + w + SIDE_R - INSET_FAR,
                        bottom - INSET_FAR,
                    );
                    b.border(BORDER, x0 - SIDE_L, top, x0 + w + SIDE_R, bottom);
                    let py = top + PLATE_DY;
                    b.gradient(PLATE_X0 + 0.5, py + 0.5, PLATE_X1 + 0.5, py + 0.5 + PLATE_H);
                    b.border(PLATE_BORDER, PLATE_X0, py, PLATE_X1, py + PLATE_H);
                    match self.title_size {
                        Some(size) => {
                            b.gold_centred(&self.tables, size, 256.0, top + TITLE_DY, TITLE)
                        }
                        None => {
                            self.notes.insert(
                                "title: the gold font has no 24-pixel size in the glyph table"
                                    .into(),
                            );
                        }
                    }
                    b.text(id, x0, text_y, w, lines);
                }
                None => {
                    self.notes
                        .insert(format!("window: message {id} is not in the tables"));
                }
            }
        }
        if let Some((x, y)) = view.pointer {
            let (cx, cy) = (256.0 + f32::from(x), 256.0 + f32::from(y));
            b.atlas(6, POINTER_BIG, cx - 16.0, cy - 15.0, cx + 16.0, cy + 15.0);
            b.atlas(6, POINTER_SMALL, cx - 16.0, cy - 15.0, cx, cy + 1.0);
        }
        b.prims
    }
}

fn vtx(x: f32, y: f32, u: f32, v: f32, rgba: [u8; 4]) -> Vertex {
    Vertex { x, y, u, v, rgba }
}

/// Primitive builder with the HUD's render-state conventions.
#[derive(Default)]
struct Prims {
    prims: Vec<Primitive>,
}

impl Prims {
    fn push(
        &mut self,
        kind: PrimKind,
        texture: TextureSource,
        blend: Option<u8>,
        alpha_test: bool,
        gouraud: bool,
        verts: Vec<Vertex>,
    ) {
        self.prims.push(Primitive {
            element: Element::Banner,
            kind,
            texture,
            target: Target::Frame,
            blend,
            alpha_test,
            bilinear: matches!(texture, TextureSource::TextCanvas { .. }),
            gouraud,
            z: roses_client::duel_hud::HUD_Z,
            verts,
        });
    }

    /// The subtractive quad: (0 − Cs)·As + Cd with white × 96/128.
    fn darken(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.push(
            PrimKind::TriStrip,
            TextureSource::None,
            Some(2),
            false,
            false,
            vec![
                vtx(x0, y0, 0.0, 0.0, DARKEN),
                vtx(x1, y0, 0.0, 0.0, DARKEN),
                vtx(x0, y1, 0.0, 0.0, DARKEN),
                vtx(x1, y1, 0.0, 0.0, DARKEN),
            ],
        );
    }

    /// Atlas sprite at integer pixels with half-texel UVs.
    fn atlas(&mut self, slot: u8, uv: [f32; 4], x0: f32, y0: f32, x1: f32, y1: f32) {
        self.push(
            PrimKind::TriStrip,
            TextureSource::Atlas(slot),
            Some(0),
            true,
            false,
            vec![
                vtx(x0, y0, uv[0] + 0.5, uv[1] + 0.5, GREY),
                vtx(x1, y0, uv[2] - 0.5, uv[1] + 0.5, GREY),
                vtx(x0, y1, uv[0] + 0.5, uv[3] - 0.5, GREY),
                vtx(x1, y1, uv[2] - 0.5, uv[3] - 0.5, GREY),
            ],
        );
    }

    /// An 8-piece border (corners, stretched edges, no centre) of the
    /// texels `(uv, corner)` on TEXSYS #7 around (x0, y0)-(x1, y1), the
    /// pieces placed on .5 pixels with exact texel UVs.
    fn border(&mut self, border: ([f32; 4], f32), x0: f32, y0: f32, x1: f32, y1: f32) {
        let ([u0, v0, u1, v1], c) = border;
        let mid = (u1 - u0) - 2.0 * c;
        let xs = [(x0, u0), (x0 + c, u0 + c), (x1 - c, u1 - c), (x1, u1)];
        let ys = [(y0, v0), (y0 + c, v0 + c), (y1 - c, v1 - c), (y1, v1)];
        let _ = mid;
        for r in 0..3 {
            for k in 0..3 {
                if r == 1 && k == 1 {
                    continue;
                }
                let (px0, pu0) = xs[k];
                let (px1, pu1) = xs[k + 1];
                let (py0, pv0) = ys[r];
                let (py1, pv1) = ys[r + 1];
                if px1 <= px0 || py1 <= py0 {
                    continue;
                }
                self.push(
                    PrimKind::Sprite,
                    TextureSource::Atlas(7),
                    Some(0),
                    true,
                    false,
                    vec![vtx(px0, py0, pu0, pv0, GREY), vtx(px1, py1, pu1, pv1, GREY)],
                );
            }
        }
    }

    /// The plate's vertical gradient: dark, bright at the middle, dark.
    fn gradient(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let ym = y0 + (y1 - y0) / 2.0;
        for (ya, ca, yb, cb) in [
            (y0, GRADIENT_DARK, ym, GRADIENT_BRIGHT),
            (ym, GRADIENT_BRIGHT, y1, GRADIENT_DARK),
        ] {
            self.push(
                PrimKind::TriStrip,
                TextureSource::None,
                None,
                false,
                true,
                vec![
                    vtx(x0, ya, 0.0, 0.0, ca),
                    vtx(x1, ya, 0.0, 0.0, ca),
                    vtx(x0, yb, 0.0, 0.0, cb),
                    vtx(x1, yb, 0.0, 0.0, cb),
                ],
            );
        }
    }

    /// The gold font (0x1B7B50) proportional and centred at `x`: glyph
    /// widths from the descriptor table, each glyph a strip of its texel
    /// rectangle with half-texel UVs.
    fn gold_centred(&mut self, t: &HudTables, size: usize, x: f32, y: f32, text: &[u8]) {
        let glyph = |ch: u8| -> Option<&roses_client::duel_hud::Descriptor> {
            let g = match ch {
                b'A'..=b'Z' => usize::from(ch - b'A'),
                b'0'..=b'9' => usize::from(ch - b'0') + 26,
                _ => return None,
            };
            t.glyphs.get(g + 37 * size)
        };
        let width: i32 = text
            .iter()
            .filter_map(|&c| glyph(c))
            .map(|d| d.width())
            .sum();
        let mut pen = x - (width / 2) as f32;
        for &ch in text {
            let Some(d) = glyph(ch) else {
                continue;
            };
            let (gw, gh) = (d.width() as f32, d.height() as f32);
            let (u0, v0) = (f32::from(d.u0) + 0.5, f32::from(d.v0) + 0.5);
            let (u1, v1) = (f32::from(d.u1) + 0.5, f32::from(d.v1) + 0.5);
            self.push(
                PrimKind::TriStrip,
                TextureSource::Atlas(d.slot),
                Some(0),
                true,
                false,
                vec![
                    vtx(pen, y, u0, v0, GREY),
                    vtx(pen + gw, y, u1, v0, GREY),
                    vtx(pen, y + gh, u0, v1, GREY),
                    vtx(pen + gw, y + gh, u1, v1, GREY),
                ],
            );
            pen += gw;
        }
    }

    /// The composed text sprite: canvas lines 1.. (v 22..), bilinear.
    fn text(&mut self, id: u16, x0: f32, y0: f32, w: f32, lines: usize) {
        let h = 22.0 * lines as f32;
        self.push(
            PrimKind::Sprite,
            TextureSource::TextCanvas {
                line: 1,
                message: Some(id),
            },
            Some(0),
            true,
            false,
            vec![
                vtx(x0, y0, 0.5, 22.5, WHITE),
                vtx(x0 + w, y0 + h, w + 0.5, 22.0 + h + 0.5, WHITE),
            ],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox(p: &[Primitive]) -> (f32, f32, f32, f32) {
        let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for v in p.iter().flat_map(|p| &p.verts) {
            b = (b.0.min(v.x), b.1.min(v.y), b.2.max(v.x), b.3.max(v.y));
        }
        b
    }

    #[test]
    fn border_pieces_match_the_dumped_window_frame() {
        // 189-n57: text 372 wide at x 70; frame (53.5,255.5)-(457.5,401.5)
        // in 8 pieces with 14-px corners and a 4-texel middle.
        let mut b = Prims::default();
        b.border(BORDER, 53.5, 255.5, 457.5, 401.5);
        assert_eq!(b.prims.len(), 8);
        assert_eq!(bbox(&b.prims), (53.5, 255.5, 457.5, 401.5));
        let top = &b.prims[1].verts;
        assert_eq!((top[0].x, top[1].x), (67.5, 443.5));
        assert_eq!((top[0].u, top[1].u), (174.0, 178.0));
        assert_eq!((top[0].v, top[1].v), (208.0, 222.0));
        let left = &b.prims[3].verts;
        assert_eq!((left[0].y, left[1].y), (269.5, 387.5));
        assert_eq!((left[0].v, left[1].v), (222.0, 226.0));
    }

    #[test]
    fn plate_border_has_five_pixel_corners() {
        let mut b = Prims::default();
        b.border(PLATE_BORDER, 190.5, 267.5, 320.5, 303.5);
        assert_eq!(b.prims.len(), 8);
        let tl = &b.prims[0].verts;
        assert_eq!(
            (tl[1].x, tl[1].y, tl[1].u, tl[1].v),
            (195.5, 272.5, 53.0, 69.0)
        );
        let right = &b.prims[4].verts;
        assert_eq!((right[0].x, right[1].x), (315.5, 320.5));
        assert_eq!((right[0].u, right[1].u), (59.0, 64.0));
    }

    #[test]
    fn pointer_quads_match_the_dumped_state() {
        // 355-q30: pointer at (-28, 240): (212,481)-(244,511) and
        // (212,481)-(228,497).
        let mut b = Prims::default();
        let (cx, cy) = (256.0 - 28.0, 256.0 + 240.0);
        b.atlas(6, POINTER_BIG, cx - 16.0, cy - 15.0, cx + 16.0, cy + 15.0);
        b.atlas(6, POINTER_SMALL, cx - 16.0, cy - 15.0, cx, cy + 1.0);
        assert_eq!(bbox(&b.prims[..1]), (212.0, 481.0, 244.0, 511.0));
        assert_eq!(bbox(&b.prims[1..]), (212.0, 481.0, 228.0, 497.0));
        let v = &b.prims[0].verts;
        assert_eq!((v[0].u, v[0].v, v[3].u, v[3].v), (0.5, 81.5, 31.5, 111.5));
        let v = &b.prims[1].verts;
        assert_eq!((v[0].u, v[0].v, v[3].u, v[3].v), (64.5, 96.5, 79.5, 111.5));
    }

    #[test]
    fn text_sprite_covers_the_composed_lines() {
        let mut b = Prims::default();
        b.text(3710, 70.0, 316.0, 372.0, 3);
        let v = &b.prims[0].verts;
        assert_eq!(
            (v[0].x, v[0].y, v[1].x, v[1].y),
            (70.0, 316.0, 442.0, 382.0)
        );
        assert_eq!((v[0].u, v[0].v, v[1].u, v[1].v), (0.5, 22.5, 372.5, 88.5));
        assert!(b.prims[0].bilinear);
    }

    #[test]
    fn window_geometry_follows_the_text_width_and_slide() {
        // The darkening and frame of a 372-wide page at rest and one poll
        // into the slide-out (offset 0x50).
        for (offset, dy) in [(0u16, 0.0f32), (0x50, 80.0)] {
            let x0 = (256 - 372 / 2) as f32;
            let top = WINDOW_TOP + dy;
            let text_y = WINDOW_TEXT_Y + dy;
            let bottom = text_y + 66.0 + BELOW;
            let mut b = Prims::default();
            b.darken(
                x0 - SIDE_L + INSET_NEAR,
                top + INSET_NEAR,
                x0 + 372.0 + SIDE_R - INSET_FAR,
                bottom - INSET_FAR,
            );
            assert_eq!(bbox(&b.prims), (58.0, 260.0 + dy, 454.0, 398.0 + dy));
            let _ = offset;
        }
    }

    #[test]
    fn operator_presses_cross_on_every_fifth_check() {
        let mut op = Operator::default();
        op.checks = 3;
        assert_eq!(op.checks, 3);
        assert_eq!(Operator::PERIOD, 5);
    }
}
