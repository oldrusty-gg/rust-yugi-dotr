//! The deck editor's list screen (mode 6, states 3 and 4: trunk list and
//! deck list side by side) as HUD primitives, drawn by `roses_client::
//! duel_hud::render_primitives` from the user's TEXSYS.MRG atlases #6/#7,
//! PICMINI.MRG portrait and the message font — the same atlases and the
//! same sprite-descriptor table (EE 0x28F300) the duel HUD uses.
//!
//! Provenance. The draw function 0x1BBB60 was not traced instruction by
//! instruction; the layout below is read from one software-GS dump of the
//! original editor frame (PAL, map state 622 → SYSTEM icon 0, state 3 at
//! entry: the project notes),
//! tagged **[D]**: every sprite of that frame was matched to a descriptor
//! of the executable's tables (sprites 0x28F300, attribute icons 0x28F2A0,
//! gold glyphs 0x28FDF0) and its position recorded. What the dump could
//! not show is **[H]** (hypothesis, named in the code) or left out and
//! reported through the notes: the detail panes, the catalogue, the message
//! states, the sort labels of keys other than 0, empty deck slots, the
//! pane slide, the background and its scroll, and the animation slots.
//! Nothing here is a game byte: descriptor indices, pixel positions and
//! the on-screen words are what the dump shows.
use roses_client::duel_hud::{
    Descriptor, Element, HUD_Z, HudTables, PrimKind, Primitive, Target, TextureSource, Vertex,
};
use roses_core::card_block::{CardBlock, EMPTY_CARD};
use roses_core::deck_editor::{
    self, DECK_LIST_LEN, EditorSnapshot, StaticCards, TRUNK_CATEGORY, VISIBLE_ROWS, state,
};

/// Card names start at message 323 (0x1DF480).
pub const CARD_NAME_MESSAGE: u16 = 323;
/// The name box samples 12 canvas texels per composed code, squeezed into
/// 104 pixels [D: 12-code names sample 144 texels; "Maiden of the
/// Moonlight" and "Moon Envoy" fill the same 104 px]. Names shorter than
/// the box are drawn 1:1 [H: not observed].
pub const NAME_BOX_WIDTH: i32 = 104;
pub const TEXELS_PER_CODE: usize = 12;

/// Row pitch and the two list origins [D].
pub const ROW_PITCH: i32 = 54;
pub const TRUNK_X: i32 = 8;
pub const DECK_X: i32 = 264;
pub const LIST_Y: i32 = 112;
pub const ROW_W: i32 = 240;
pub const ROW_H: i32 = 54;

/// Sprite descriptor indices of the 3×3 frame sets and icons [D].
mod desc {
    /// 12-px corners, used for the title plate and the portrait frame.
    pub const FRAME_TITLE: usize = 95;
    /// 5-px corners: the button row boxes.
    pub const FRAME_SMALL: usize = 104;
    /// The cursor frame.
    pub const FRAME_CURSOR: usize = 113;
    /// The hint bar.
    pub const FRAME_HINT: usize = 122;
    /// Tinted box (vertex colour): rows, name boxes, footers.
    pub const FRAME_BOX: usize = 140;
    /// 5-px panel: CHEST / DECK / portrait panels.
    pub const FRAME_PANEL: usize = 158;
    /// Title ribbon and tile.
    pub const RIBBON: usize = 54;
    pub const TITLE_TILE: usize = 189;
    /// Icons.
    pub const ARROW_UP: usize = 196;
    pub const ARROW_DOWN: usize = 197;
    pub const LEADER_EMBLEM: usize = 198;
    pub const LEADER_STAR: usize = 202;
    pub const COUNT_ICON: usize = 203;
    pub const CARDS_ICON: usize = 205;
    pub const SORT_ICON_FIRST: usize = 209;
    pub const SORT_ICONS: usize = 10;
    pub const ATK_ICON: usize = 211;
    pub const DEF_ICON: usize = 212;
    pub const KIND_ICON: usize = 215;
    pub const DC_ICON: usize = 216;
    pub const DECK_BADGE_FIRST: usize = 227;
    pub const DECK_ICON_FIRST: usize = 230;
    pub const DECK_LABEL: usize = 233;
    pub const TRIANGLE_BUTTON: usize = 251;
    pub const CIRCLE_BUTTON: usize = 252;
    pub const CROSS_BUTTON: usize = 253;
    pub const CIRCLE_BUTTON_SMALL: usize = 254;
    pub const RIGHT_BUTTON: usize = 255;
    pub const L1_LABEL: usize = 260;
    pub const R1_LABEL: usize = 261;
    pub const L2_LABEL: usize = 262;
    pub const R2_LABEL: usize = 263;
}

const WHITE: [u8; 4] = [128, 128, 128, 128];
const DIM: [u8; 4] = [48, 48, 48, 128];
const GOLD: [u8; 4] = [160, 96, 0, 128];
const NAME_BOX: [u8; 4] = [160, 128, 0, 128];
const LEADER_NAME_BOX: [u8; 4] = [64, 32, 96, 128];
/// Gold font sizes: 0 = large (DECK LEADER), 1 = small (numbers, labels),
/// 2 = title (BUILD DECK, CHEST) [D].
const FONT_LARGE: usize = 0;
const FONT_SMALL: usize = 1;
const FONT_TITLE: usize = 2;
const PROPORTIONAL: u32 = 1;
/// Flags 0: monospace cells, each glyph centred (the footers and the hint
/// bar: EXIT at x 44.5/53.5/63.5/71.5, SELECT DECK from 332.5, MOV from
/// 52.0, DTL from 109.5, LEADER from 167.5 in the entry dump [D draws
/// 5728, 5860, 5866-5870]).
const MONOSPACE: u32 = 0;
const GLYPHS_PER_SIZE: usize = 37;

/// What the builder could not draw, reported once per frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note(pub String);

/// The per-frame inputs.
pub struct ScreenInputs<'a> {
    pub snapshot: &'a EditorSnapshot,
    pub block: &'a CardBlock,
    pub cards: &'a StaticCards,
    pub tables: &'a HudTables,
}

/// What a card row reads.
struct RowInputs<'a> {
    block: &'a CardBlock,
    cards: &'a StaticCards,
    tables: &'a HudTables,
}

fn vtx(x: f32, y: f32, u: f32, v: f32, rgba: [u8; 4]) -> Vertex {
    Vertex { x, y, u, v, rgba }
}

struct Builder<'a> {
    t: &'a HudTables,
    prims: Vec<Primitive>,
    notes: Vec<Note>,
}

impl Builder<'_> {
    fn note(&mut self, text: impl Into<String>) {
        let n = Note(text.into());
        if !self.notes.contains(&n) {
            self.notes.push(n);
        }
    }

    fn push(&mut self, kind: PrimKind, texture: TextureSource, verts: Vec<Vertex>) {
        let bilinear = matches!(texture, TextureSource::TextCanvas { .. });
        self.prims.push(Primitive {
            element: Element::Banner,
            kind,
            texture,
            target: Target::Frame,
            blend: Some(0),
            alpha_test: true,
            bilinear,
            gouraud: false,
            z: HUD_Z,
            verts,
        });
    }

    fn descriptor(&mut self, index: usize) -> Option<Descriptor> {
        let d = self.t.sprites.get(index).copied();
        if d.is_none() {
            self.note(format!("sprite descriptor {index} is past the table"));
        }
        d
    }

    /// An icon sprite at integer pixels, the descriptor's texels + 0.5 [D].
    fn icon(&mut self, index: usize, x: i32, y: i32, rgba: [u8; 4]) {
        if let Some(d) = self.descriptor(index) {
            let (w, h) = (d.width(), d.height());
            self.icon_scaled(&d, x, y, w, h, rgba);
        }
    }

    fn icon_scaled(&mut self, d: &Descriptor, x: i32, y: i32, w: i32, h: i32, rgba: [u8; 4]) {
        let (u0, v0) = (f32::from(d.u0) + 0.5, f32::from(d.v0) + 0.5);
        let (u1, v1) = (f32::from(d.u1) + 0.5, f32::from(d.v1) + 0.5);
        self.push(
            PrimKind::Sprite,
            TextureSource::Atlas(d.slot),
            vec![
                vtx(x as f32, y as f32, u0, v0, rgba),
                vtx((x + w) as f32, (y + h) as f32, u1, v1, rgba),
            ],
        );
    }

    /// A 3×3 frame of the descriptor set `first..first+9` (centre piece 4
    /// skipped) at (x, y) with outer size (w, h): pieces at integer pixel
    /// edges − 0.5 and integer texels, as the HUD's frame routine [D].
    fn frame(&mut self, first: usize, x: i32, y: i32, w: i32, h: i32, rgba: [u8; 4]) {
        let Some(p) = (0..9)
            .map(|k| self.t.sprites.get(first + k).copied())
            .collect::<Option<Vec<Descriptor>>>()
        else {
            self.note(format!("frame set {first} is past the descriptor table"));
            return;
        };
        let (lw, rw) = (p[0].width(), p[2].width());
        let (th, bh) = (p[0].height(), p[6].height());
        let widths = [lw, w - lw - rw, rw];
        let heights = [th, h - th - bh, bh];
        let mut py = 0;
        for r in 0..3 {
            let mut px = 0;
            for c in 0..3 {
                let (pw, ph) = (widths[c], heights[r]);
                if r * 3 + c != 4 && pw > 0 && ph > 0 {
                    let d = p[r * 3 + c];
                    let (x0, y0) = ((x + px) as f32 - 0.5, (y + py) as f32 - 0.5);
                    self.push(
                        PrimKind::Sprite,
                        TextureSource::Atlas(d.slot),
                        vec![
                            vtx(x0, y0, f32::from(d.u0), f32::from(d.v0), rgba),
                            vtx(
                                x0 + pw as f32,
                                y0 + ph as f32,
                                f32::from(d.u1),
                                f32::from(d.v1),
                                rgba,
                            ),
                        ],
                    );
                }
                px += pw;
            }
            py += heights[r];
        }
    }

    /// The gold font (0x1B7B50 as the HUD ports it): monospace cells of the
    /// size's width unless `proportional`; glyphs centred in their cell.
    fn gold(&mut self, x: i32, y: i32, size: usize, flags: u32, text: &[u8]) {
        let glyph = |ch: u8| -> Option<usize> {
            match ch {
                b'A'..=b'Z' => Some(usize::from(ch - b'A')),
                b'a'..=b'z' => Some(usize::from(ch - b'a')),
                b'0'..=b'9' => Some(usize::from(ch - b'0') + 26),
                b'?' => Some(36),
                _ => None,
            }
        };
        let Some(&(cw, _)) = self.t.font_sizes.get(size) else {
            self.note(format!("gold font size {size} is not in the table"));
            return;
        };
        let cw = i32::from(cw);
        let proportional = flags & PROPORTIONAL != 0;
        let mut pen = x as f32;
        for &ch in text {
            let Some(g) = glyph(ch) else {
                pen += (if proportional { cw >> 1 } else { cw }) as f32;
                continue;
            };
            let Some(d) = self.t.glyphs.get(g + GLYPHS_PER_SIZE * size).copied() else {
                self.note(format!("gold glyph {g} of size {size} is past the table"));
                return;
            };
            let (gw, gh) = (d.width(), d.height());
            let ox = if proportional {
                0.0
            } else {
                (cw - gw) as f32 / 2.0
            };
            let gx = pen + ox;
            self.push(
                PrimKind::TriStrip,
                TextureSource::Atlas(d.slot),
                vec![
                    vtx(
                        gx,
                        y as f32,
                        f32::from(d.u0) + 0.5,
                        f32::from(d.v0) + 0.5,
                        WHITE,
                    ),
                    vtx(
                        gx + gw as f32,
                        y as f32,
                        f32::from(d.u1) + 0.5,
                        f32::from(d.v0) + 0.5,
                        WHITE,
                    ),
                    vtx(
                        gx,
                        (y + gh) as f32,
                        f32::from(d.u0) + 0.5,
                        f32::from(d.v1) + 0.5,
                        WHITE,
                    ),
                    vtx(
                        gx + gw as f32,
                        (y + gh) as f32,
                        f32::from(d.u1) + 0.5,
                        f32::from(d.v1) + 0.5,
                        WHITE,
                    ),
                ],
            );
            pen += (if proportional { gw } else { cw }) as f32;
        }
    }

    /// A right-aligned decimal in `width` monospace cells (0x1B7A80 without
    /// zero padding): leading cells blank.
    fn number(&mut self, x: i32, y: i32, width: usize, value: u32) {
        let digits = value.to_string();
        let mut text = vec![b' '; width.saturating_sub(digits.len())];
        text.extend_from_slice(digits.as_bytes());
        self.gold(x, y, FONT_SMALL, 0, &text);
    }

    /// A card name: three layered sprites of canvas line 1 (shadow +2, +1,
    /// then the lit text), 12 texels per code squeezed into the box [D].
    fn name(&mut self, card: u16, x: i32, y: i32) {
        let message = CARD_NAME_MESSAGE + card;
        let Some(codes) = self.t.message(message).map(<[u16]>::len) else {
            self.note(format!("card {card} has no name message"));
            return;
        };
        let texels = (codes * TEXELS_PER_CODE) as f32;
        let width = texels.min(NAME_BOX_WIDTH as f32);
        for (dx, rgba) in [
            (2, [0, 0, 0, 96]),
            (1, [64, 64, 64, 128]),
            (0, [160, 160, 160, 128]),
        ] {
            let (x0, y0) = ((x + dx) as f32, (y + dx) as f32);
            self.push(
                PrimKind::Sprite,
                TextureSource::TextCanvas {
                    line: 1,
                    message: Some(message),
                },
                vec![
                    vtx(x0, y0, 0.5, 22.5, rgba),
                    vtx(x0 + width, y0 + 22.0, texels + 0.5, 44.5, rgba),
                ],
            );
        }
    }

    /// A 12×50 ornament rotated 90° (the title plate's and the cursor's
    /// side pieces, 0x1BBB90's two triangles each) [D].
    fn side_piece(&mut self, x: i32, y: i32, flip: bool, uv: [f32; 4]) {
        let (x0, y0) = (x as f32 + 0.5, y as f32 + 0.5);
        let (x1, y1) = (x0 + 12.0, y0 + 50.0);
        let (u0, v0, u1, v1) = (uv[0], uv[1], uv[2], uv[3]);
        let (xa, xb) = if flip { (x1, x0) } else { (x0, x1) };
        self.push(
            PrimKind::TriStrip,
            TextureSource::Atlas(7),
            vec![
                vtx(xa, y1, u0, v0, WHITE),
                vtx(xa, y0, u1, v0, WHITE),
                vtx(xb, y1, u0, v1, WHITE),
                vtx(xb, y0, u1, v1, WHITE),
            ],
        );
    }

    /// One card row at (x, y): frame, id, type picture, attribute, kind
    /// icon, level, ATK/DEF, DC, trunk copies, copies per deck, name [D].
    /// With `copies` (the deck view, a port addition) the trunk and deck
    /// columns give way to the copies in the deck viewed, and a spell, trap
    /// or ritual shows its kind's picture as the detail pane does
    /// (descriptor 0x14 + kind, 0x1BF1D0 [F]).
    fn card_row(&mut self, inp: &RowInputs<'_>, card: u16, x: i32, y: i32, bright: bool, copies: Option<u8>) {
        self.frame(
            desc::FRAME_BOX,
            x,
            y,
            ROW_W,
            ROW_H,
            if bright { WHITE } else { DIM },
        );
        let Some(rec) = inp.cards.record(card) else {
            self.note(format!("card {card} has no static record"));
            return;
        };
        self.number(x + 8, y + 7, 3, u32::from(card));
        let kind = rec[0] >> 5;
        let sub = usize::from(rec[0] & 0x1F);
        let attribute = usize::from(rec[2] & 0xF);
        let level = u32::from(rec[2] >> 4);
        let atk = u32::from(u16::from_le_bytes([rec[8], rec[9]]) & 0x1FFF);
        let def = u32::from(u16::from_le_bytes([rec[10], rec[11]]) & 0x1FFF);
        if kind == 0 {
            // Monster: type picture (descriptor = sub-type), attribute icon.
            self.icon(sub, x + 4, y + 26, WHITE);
            match inp.tables.attribute_icons.get(attribute).copied() {
                Some(d) => self.icon_scaled(&d, x + 30, y + 26, 24, 24, WHITE),
                None => self.note(format!("attribute {attribute} has no icon")),
            }
            self.icon(desc::KIND_ICON, x + 56, y + 35, WHITE);
            self.number(x + 79, y + 37, 1, level);
            self.icon(desc::ATK_ICON, x + 184, y + 3, WHITE);
            self.number(x + 200, y + 5, 4, atk);
            self.icon(desc::DEF_ICON, x + 184, y + 19, WHITE);
            self.number(x + 200, y + 21, 4, def);
        } else if copies.is_some() {
            self.icon(0x14 + usize::from(kind), x + 4, y + 26, WHITE);
        } else {
            self.note(format!(
                "card {card} is kind {kind} (spell/trap/ritual): its row icons were not observed"
            ));
        }
        self.icon(desc::DC_ICON, x + 91, y + 35, WHITE);
        self.number(x + 107, y + 37, 2, u32::from(rec[3]));
        self.icon(desc::COUNT_ICON, x + 125, y + 35, WHITE);
        if let Some(n) = copies {
            self.number(x + 142, y + 37, 1, u32::from(n));
            self.frame(desc::FRAME_BOX, x + 72, y + 3, 112, 32, NAME_BOX);
            self.name(card, x + 76, y + 7);
            return;
        }
        let trunk = deck_editor::count_copies(inp.block, card, Some(TRUNK_CATEGORY));
        self.number(x + 142, y + 37, 1, trunk as u32);
        for d in 0..3u8 {
            let icon_x = x + 153 + 28 * i32::from(d);
            self.icon(
                desc::DECK_ICON_FIRST + usize::from(d),
                icon_x,
                y + 35,
                WHITE,
            );
            let copies = deck_editor::count_copies(inp.block, card, Some(d));
            self.number(icon_x + 17, y + 37, 1, copies as u32);
        }
        self.frame(desc::FRAME_BOX, x + 72, y + 3, 112, 32, NAME_BOX);
        self.name(card, x + 76, y + 7);
    }

    /// The deck's leader row (list index 0) at (x, y) [D].
    fn leader_row(&mut self, inp: &ScreenInputs<'_>, leader: u16, x: i32, y: i32, deck: u8) {
        self.frame(desc::FRAME_BOX, x, y, ROW_W, ROW_H, WHITE);
        if leader == EMPTY_CARD {
            self.note("a deck without a leader: the empty leader row was not observed");
            return;
        }
        self.number(x + 8, y + 7, 3, u32::from(leader));
        self.icon(desc::LEADER_EMBLEM, x + 40, y + 3, WHITE);
        self.icon(desc::LEADER_STAR, x + 56, y + 3, WHITE);
        if let Some(rec) = inp.cards.record(leader) {
            self.icon(usize::from(rec[0] & 0x1F), x + 4, y + 26, WHITE);
            let attribute = usize::from(rec[2] & 0xF);
            if let Some(d) = inp.tables.attribute_icons.get(attribute).copied() {
                self.icon_scaled(&d, x + 30, y + 26, 24, 24, WHITE);
            }
        }
        self.gold(x + 68, y + 37, FONT_LARGE, PROPORTIONAL, b"DECK LEADER");
        self.icon(
            desc::DECK_BADGE_FIRST + usize::from(deck.min(2)),
            x + 199,
            y + 16,
            WHITE,
        );
        self.frame(desc::FRAME_BOX, x + 72, y + 3, 112, 32, LEADER_NAME_BOX);
        self.name(leader, x + 76, y + 7);
    }

    /// The sort-key icon row and label of a list header at x [D].
    fn sort_header(&mut self, x: i32, keys: [u8; 3], label_x: i32) {
        self.frame(desc::FRAME_BOX, x, 64, 176, 48, GOLD);
        self.icon(desc::SORT_ICON_FIRST, x + 8, 73, WHITE);
        for k in 0..desc::SORT_ICONS - 1 {
            self.icon(
                desc::SORT_ICON_FIRST + 1 + k,
                x + 24 + 16 * k as i32,
                73,
                WHITE,
            );
        }
        if keys[0] == 0 {
            self.gold(label_x, 94, FONT_SMALL, PROPORTIONAL, b"NEWCOMER SORT");
        } else {
            self.note(format!(
                "sort key {} label: only key 0 (NEWCOMER SORT) was observed",
                keys[0]
            ));
        }
    }
}

/// Build the list screen of states 3/4 for one frame.
pub fn build_screen(inp: &ScreenInputs<'_>) -> (Vec<Primitive>, Vec<Note>) {
    let mut b = Builder {
        t: inp.tables,
        prims: Vec::new(),
        notes: Vec::new(),
    };
    let snap = inp.snapshot;
    let rows_in = RowInputs { block: inp.block, cards: inp.cards, tables: inp.tables };
    let d = deck_editor::current_deck(inp.block);
    let deck = deck_editor::deck_record(inp.block, d);
    match snap.state {
        state::TRUNK_LIST
        | state::DECK_LIST
        | state::TRUNK_DETAIL
        | state::TRUNK_LIST_DETAIL
        | state::DECK_LIST_DETAIL
        | state::DECK_DETAIL
        | state::CATALOGUE => {}
        // Drawn over the lists by `message_box_calls`.
        state::MSG_PASSWORD_WRONG | state::MSG_PASSWORD_USED | state::MSG_DECK_INVALID => {}
        other => b.note(format!(
            "editor state {other:#x} (prompts and messages) is not drawn; the lists are shown as in state 3"
        )),
    }

    // Title plate: five tiles, the 12-corner frame, two rotated ornaments,
    // the ribbon and BUILD DECK.
    for i in 0..5 {
        if let Some(t) = b.descriptor(desc::TITLE_TILE) {
            b.icon_scaled(&t, 128 + 32 * i, 32, 32, 32, WHITE);
            // The tiles sit at −0.5 with integer texels [D]; icon_scaled
            // adds 0.5 to the texels, so emit them directly instead.
            b.prims.pop();
            let (x0, y0) = ((128 + 32 * i) as f32 - 0.5, 31.5);
            b.push(
                PrimKind::Sprite,
                TextureSource::Atlas(t.slot),
                vec![
                    vtx(x0, y0, f32::from(t.u0), f32::from(t.v0), WHITE),
                    vtx(
                        x0 + 32.0,
                        y0 + 32.0,
                        f32::from(t.u1),
                        f32::from(t.v1),
                        WHITE,
                    ),
                ],
            );
        }
    }
    b.frame(desc::FRAME_TITLE, 128, 32, 160, 32, WHITE);
    b.side_piece(116, 32, false, [64.0, 84.0, 96.0, 96.0]);
    b.side_piece(286, 32, true, [64.0, 84.0, 96.0, 96.0]);
    b.icon(desc::RIBBON, 160, 17, WHITE);
    b.gold(141, 39, FONT_TITLE, PROPORTIONAL, b"BUILD DECK");

    // CHEST panel and the chest count (trunk copies of the highlighted
    // trunk card [H: 0 observed with every copy in the deck]).
    b.frame(desc::FRAME_PANEL, 16, 16, 96, 48, WHITE);
    b.gold(31, 31, FONT_TITLE, PROPORTIONAL, b"CHEST");
    b.frame(desc::FRAME_SMALL, 16, 64, 64, 20, WHITE);
    b.icon(desc::COUNT_ICON, 18, 66, WHITE);
    let highlighted = snap
        .trunk
        .get(usize::from(snap.trunk_window) + usize::from(snap.trunk_row))
        .copied()
        .unwrap_or(EMPTY_CARD);
    let chest = deck_editor::count_copies(inp.block, highlighted, Some(TRUNK_CATEGORY));
    b.number(66, 68, 1, chest as u32);
    // L1 box with the up arrow.
    b.frame(desc::FRAME_BOX, 16, 84, 64, 28, GOLD);
    b.icon(desc::L1_LABEL, 21, 93, WHITE);
    b.icon(desc::ARROW_UP, 44, 92, WHITE);
    // Trunk sort header: L2 label, icons, R2 label, the sort name.
    b.sort_header(80, snap.trunk_keys, 117);
    b.icon(desc::L2_LABEL, 87, 94, WHITE);
    b.icon(desc::R2_LABEL, 227, 94, WHITE);

    // DECK panel: label, DC icon and the deck cost, cards icon and count.
    b.frame(desc::FRAME_PANEL, 304, 16, 128, 48, WHITE);
    b.icon(desc::DECK_LABEL, 346, 24, WHITE);
    b.icon(desc::DC_ICON, 316, 42, WHITE);
    b.number(345, 44, 3, u32::from(deck.cost));
    b.icon(desc::CARDS_ICON, 380, 42, WHITE);
    b.number(400, 44, 2, deck.count() as u32);
    // Deck sort header (no L2/R2 labels on the right [D]).
    b.sort_header(256, snap.deck_keys, 293);
    // Leader portrait panel: the deck leader's PICMINI picture stretched
    // to 54×58 behind the 12-corner frame, the L1 box with the up arrow.
    b.frame(desc::FRAME_PANEL, 432, 16, 64, 68, WHITE);
    if deck.leader != EMPTY_CARD {
        b.push(
            PrimKind::Sprite,
            TextureSource::Portrait(0),
            vec![
                vtx(437.0, 21.0, 0.5, 0.5, WHITE),
                vtx(491.0, 79.0, 40.5, 32.5, WHITE),
            ],
        );
    }
    b.frame(desc::FRAME_TITLE, 437, 21, 54, 58, WHITE);
    b.frame(desc::FRAME_BOX, 432, 84, 64, 28, GOLD);
    b.icon(desc::ARROW_UP, 436, 92, WHITE);

    // Trunk rows.
    for j in 0..VISIBLE_ROWS {
        let index = usize::from(snap.trunk_window) + usize::from(j);
        let y = LIST_Y + ROW_PITCH * i32::from(j);
        match snap.trunk.get(index).copied() {
            Some(card) if card != EMPTY_CARD => b.card_row(&rows_in, card, TRUNK_X, y, false, None),
            _ => b.note("a trunk list shorter than six rows was not observed; blank rows drawn"),
        }
    }
    // Deck rows: index 0 is the leader row.
    for j in 0..VISIBLE_ROWS {
        let index = usize::from(snap.deck_window) + usize::from(j);
        let y = LIST_Y + ROW_PITCH * i32::from(j);
        if index >= DECK_LIST_LEN {
            break;
        }
        if index == 0 {
            b.leader_row(inp, snap.deck_list[0], DECK_X, y, d);
        } else {
            match snap.deck_list.get(index).copied() {
                Some(card) if card != EMPTY_CARD => b.card_row(&rows_in, card, DECK_X, y, false, None),
                _ => {
                    b.frame(desc::FRAME_BOX, DECK_X, y, ROW_W, ROW_H, DIM);
                    b.note("an empty deck slot row was not observed; only its frame is drawn");
                }
            }
        }
    }

    // Footers: left (R1 box with the down arrow, ○ EXIT; index / count),
    // right (index / 41; SELECT DECK with the three deck badges).
    b.frame(desc::FRAME_BOX, 16, 436, 172, 50, GOLD);
    b.icon(desc::ARROW_DOWN, 44, 440, WHITE);
    b.icon(desc::R1_LABEL, 21, 445, WHITE);
    b.icon(desc::CIRCLE_BUTTON_SMALL, 23, 464, WHITE);
    b.gold(44, 467, FONT_SMALL, MONOSPACE, b"EXIT");
    b.frame(desc::FRAME_BOX, 188, 436, 60, 50, GOLD);
    let trunk_index = u32::from(snap.trunk_window) + u32::from(snap.trunk_row) + 1;
    b.number(214, 444, 2, trunk_index.min(u32::from(snap.trunk_count)));
    b.number(214, 467, 2, u32::from(snap.trunk_count));
    b.frame(desc::FRAME_BOX, 264, 436, 60, 50, GOLD);
    let deck_index = u32::from(snap.deck_window) + u32::from(snap.deck_row) + 1;
    b.number(285, 444, 2, deck_index);
    b.number(285, 467, 2, DECK_LIST_LEN as u32);
    b.frame(desc::FRAME_BOX, 324, 436, 172, 50, GOLD);
    b.icon(desc::ARROW_DOWN, 436, 440, WHITE);
    b.gold(332, 442, FONT_SMALL, MONOSPACE, b"SELECT DECK");
    for k in 0..3u8 {
        let rgba = if k == d { WHITE } else { DIM };
        b.icon(
            desc::DECK_BADGE_FIRST + usize::from(k),
            354 + 25 * i32::from(k),
            458,
            rgba,
        );
    }

    // The cursor frame with its two side pieces on the active list's row,
    // and the hint bar under it (× MOV, △ DTL, ▶ LEADER) [D: trunk row 0;
    // H: the same geometry on the deck list].
    let (cursor_x, row) = if snap.state == state::DECK_LIST || snap.state == state::DECK_LIST_DETAIL
    {
        (DECK_X - 2, snap.deck_row)
    } else {
        (TRUNK_X - 2, snap.trunk_row)
    };
    let cursor_y = LIST_Y - 2 + ROW_PITCH * i32::from(row);
    b.frame(desc::FRAME_CURSOR, cursor_x, cursor_y, 244, 58, WHITE);
    b.side_piece(cursor_x - 5, cursor_y + 3, false, [0.0, 116.0, 50.0, 128.0]);
    b.side_piece(
        cursor_x + 237,
        cursor_y + 3,
        true,
        [0.0, 116.0, 50.0, 128.0],
    );
    let hint_y = cursor_y + 56;
    b.frame(desc::FRAME_HINT, cursor_x + 2, hint_y, 240, 20, WHITE);
    b.icon(desc::CROSS_BUTTON, cursor_x + 7, hint_y + 2, WHITE);
    b.icon(desc::CIRCLE_BUTTON, cursor_x + 24, hint_y + 2, WHITE);
    b.gold(cursor_x + 46, hint_y + 5, FONT_SMALL, MONOSPACE, b"MOV");
    b.icon(desc::TRIANGLE_BUTTON, cursor_x + 81, hint_y + 2, WHITE);
    b.gold(cursor_x + 103, hint_y + 5, FONT_SMALL, MONOSPACE, b"DTL");
    b.icon(desc::RIGHT_BUTTON, cursor_x + 139, hint_y + 2, WHITE);
    b.gold(
        cursor_x + 161,
        hint_y + 5,
        FONT_SMALL,
        MONOSPACE,
        b"LEADER",
    );
    (b.prims, b.notes)
}

/// The deck view's inputs (Online Play's look at any deck, a port
/// addition drawn with the editor's own pieces).
pub struct DeckViewInputs<'a> {
    pub tables: &'a HudTables,
    pub cards: &'a StaticCards,
    /// 999 = none.
    pub leader: u16,
    /// Each card once with its copies, in the list's order.
    pub groups: &'a [(u16, u8)],
    /// Row 0 is the leader.
    pub cursor: usize,
    pub first: usize,
    /// The sort key (`deck_editor::compare_key`'s numbering; 10 = number).
    pub sort_key: u8,
    pub sort_label: &'a [u8],
    /// The detail pane covers the right half: the deck and leader panels
    /// are not drawn.
    pub pane_open: bool,
    /// SAVE / MINE / CPU.
    pub tag: &'a [u8],
}

/// Rows of the deck view at once.
pub const DECK_VIEW_ROWS: usize = VISIBLE_ROWS as usize;

/// The deck view as the editor's list screen draws its parts: the title
/// plate (VIEW DECK), a panel with the deck's list (in place of CHEST), the
/// sort header (the chosen key's icon lit, L2 / R2), the DECK panel (cost
/// and cards), the leader's portrait, six rows on the left (the leader row,
/// then each card with its copies) and the cursor frame. The layout is the
/// editor's [D] where the piece is the editor's; its use here is the
/// port's.
pub fn deck_view_screen(v: &DeckViewInputs<'_>) -> (Vec<Primitive>, Vec<Note>) {
    let mut b = Builder { t: v.tables, prims: Vec::new(), notes: Vec::new() };
    let block = CardBlock::empty(0);
    let inp = RowInputs { block: &block, cards: v.cards, tables: v.tables };
    // Title plate.
    for i in 0..5 {
        if let Some(t) = b.descriptor(desc::TITLE_TILE) {
            let (x0, y0) = ((128 + 32 * i) as f32 - 0.5, 31.5);
            b.push(
                PrimKind::Sprite,
                TextureSource::Atlas(t.slot),
                vec![
                    vtx(x0, y0, f32::from(t.u0), f32::from(t.v0), WHITE),
                    vtx(x0 + 32.0, y0 + 32.0, f32::from(t.u1), f32::from(t.v1), WHITE),
                ],
            );
        }
    }
    b.frame(desc::FRAME_TITLE, 128, 32, 160, 32, WHITE);
    b.side_piece(116, 32, false, [64.0, 84.0, 96.0, 96.0]);
    b.side_piece(286, 32, true, [64.0, 84.0, 96.0, 96.0]);
    b.icon(desc::RIBBON, 160, 17, WHITE);
    b.gold(150, 39, FONT_TITLE, PROPORTIONAL, b"VIEW DECK");
    // The list's panel (the CHEST panel's place): SAVE, MINE or CPU.
    b.frame(desc::FRAME_PANEL, 16, 16, 96, 48, WHITE);
    b.gold(36, 31, FONT_TITLE, PROPORTIONAL, v.tag);
    // Sort header: every key's icon, the chosen one lit; L2 / R2.
    let (hx, hy) = (80, 64);
    b.frame(desc::FRAME_BOX, hx, hy, 176, 48, GOLD);
    for k in 0..desc::SORT_ICONS {
        let x = if k == 0 { hx + 8 } else { hx + 24 + 16 * (k as i32 - 1) };
        let lit = usize::from(v.sort_key) == k;
        b.icon(desc::SORT_ICON_FIRST + k, x, 73, if lit { WHITE } else { DIM });
    }
    b.icon(desc::L2_LABEL, hx + 7, 94, WHITE);
    b.icon(desc::R2_LABEL, hx + 147, 94, WHITE);
    b.gold(hx + 37, 94, FONT_SMALL, PROPORTIONAL, v.sort_label);
    if !v.pane_open {
        // DECK panel: cost and cards.
        let cost: u32 = v.groups.iter().map(|&(c, n)| v.cards.record(c).map_or(0, |r| u32::from(r[3])) * u32::from(n)).sum();
        let count: u32 = v.groups.iter().map(|g| u32::from(g.1)).sum();
        b.frame(desc::FRAME_PANEL, 304, 16, 128, 48, WHITE);
        b.icon(desc::DECK_LABEL, 346, 24, WHITE);
        b.icon(desc::DC_ICON, 316, 42, WHITE);
        b.number(345, 44, 3, cost);
        b.icon(desc::CARDS_ICON, 380, 42, WHITE);
        b.number(400, 44, 2, count);
        // The leader's portrait (PICMINI, slot 0).
        b.frame(desc::FRAME_PANEL, 432, 16, 64, 68, WHITE);
        if v.leader != EMPTY_CARD {
            b.push(
                PrimKind::Sprite,
                TextureSource::Portrait(0),
                vec![vtx(437.0, 21.0, 0.5, 0.5, WHITE), vtx(491.0, 79.0, 40.5, 32.5, WHITE)],
            );
        }
        b.frame(desc::FRAME_TITLE, 437, 21, 54, 58, WHITE);
    }
    // Rows.
    let rows = 1 + v.groups.len();
    for j in 0..DECK_VIEW_ROWS {
        let index = v.first + j;
        if index >= rows {
            break;
        }
        let y = LIST_Y + ROW_PITCH * j as i32;
        if index == 0 {
            b.frame(desc::FRAME_BOX, TRUNK_X, y, ROW_W, ROW_H, WHITE);
            if v.leader == EMPTY_CARD {
                b.gold(TRUNK_X + 68, y + 20, FONT_LARGE, PROPORTIONAL, b"NO LEADER");
                continue;
            }
            let x = TRUNK_X;
            b.number(x + 8, y + 7, 3, u32::from(v.leader));
            b.icon(desc::LEADER_EMBLEM, x + 40, y + 3, WHITE);
            b.icon(desc::LEADER_STAR, x + 56, y + 3, WHITE);
            if let Some(rec) = v.cards.record(v.leader) {
                b.icon(usize::from(rec[0] & 0x1F), x + 4, y + 26, WHITE);
                if let Some(d) = v.tables.attribute_icons.get(usize::from(rec[2] & 0xF)).copied() {
                    b.icon_scaled(&d, x + 30, y + 26, 24, 24, WHITE);
                }
            }
            b.gold(x + 68, y + 37, FONT_LARGE, PROPORTIONAL, b"DECK LEADER");
            b.frame(desc::FRAME_BOX, x + 72, y + 3, 112, 32, LEADER_NAME_BOX);
            b.name(v.leader, x + 76, y + 7);
        } else {
            let (card, n) = v.groups[index - 1];
            b.card_row(&inp, card, TRUNK_X, y, false, Some(n));
        }
    }
    // The cursor frame and its side pieces.
    if v.cursor >= v.first && v.cursor < v.first + DECK_VIEW_ROWS {
        let (cx, cy) = (TRUNK_X - 2, LIST_Y - 2 + ROW_PITCH * (v.cursor - v.first) as i32);
        b.frame(desc::FRAME_CURSOR, cx, cy, 244, 58, WHITE);
        b.side_piece(cx - 5, cy + 3, false, [0.0, 116.0, 50.0, 128.0]);
        b.side_piece(cx + 237, cy + 3, true, [0.0, 116.0, 50.0, 128.0]);
    }
    (b.prims, b.notes)
}

/// The editor bytes the catalogue pane reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Catalogue {
    /// 0x2DB8FD.
    pub state: u8,
    /// 0x2DB832: the pane slide.
    pub slide: i16,
    /// D60 (first row of ten), D61 (column), D62 (row).
    pub catalogue: [u8; 3],
    /// G+0xC8C: the owned kinds.
    pub trunk_count: u16,
}

impl Catalogue {
    pub fn of(snap: &EditorSnapshot) -> Self {
        Self { state: snap.state, slide: snap.slide, catalogue: snap.catalogue, trunk_count: snap.trunk_count }
    }
}

/// The catalogue pane 0x1C04E0(packet, slide, 0) as the original's leaf
/// calls, drawn by 0x1BBB90 while the slide 0x2DB832 is ≥ 0x101 (before
/// the detail pane and the lists) [F 0x1C04E0..0x1C0CAC, 0x1BBC34; emu
/// modes/ui3 cat-*.json]. X = slide − 0x200: the LIBRARY title plate, the
/// owned-kinds count (G+0xC8C), the L1/R1 boxes (their button labels and
/// BACK only in state 0), the 10×12 grid of card numbers (D60 = first row
/// of ten, D61/D62 the cursor) with a kind sprite (220 + 0x1DF290's class,
/// 4 for an effect monster) per card the block owns (grey 0x80) or has seen
/// (0x216350, grey 0x30; card 0x29F only when seen, sprite 220 dim), the
/// cursor panel, the column numbers above and below and the scroll bar
/// (thumb at D60·0x12E / 74).
pub fn catalogue_calls(
    snap: &Catalogue,
    block: &CardBlock,
    cards: &StaticCards,
    detail: &crate::card_detail::DetailData,
) -> Vec<roses_client::duel_hud::MenuCall> {
    use roses_client::duel_hud::MenuCall;
    let mut out = Vec::new();
    let x = i32::from(snap.slide) - 0x200;
    let y = 0;
    let first = snap.state == state::CATALOGUE;
    let [base, column, row] = snap.catalogue.map(i32::from);
    let panel = |grad: i32, ptype: usize, px: i32, py: i32, wu: i32, hu: i32| MenuCall::Panel { grad, deco: -1, ptype, x: px, y: py, wu, hu, bright: true };
    let number = |px: i32, py: i32, flags: u32, digits: i32, value: i32| MenuCall::Number { x: px, y: py, size: 1, flags, digits, value, dim: false };
    out.push(panel(12, 11, x + 0x50, y + 0x10, -0xA0, -0x30));
    out.push(MenuCall::Gold { x: x + 0xA0, y: y + 0x1F, size: 2, flags: 3, text: detail.library.clone(), dim: false });
    out.push(panel(12, 5, x + 0x10, y + 0x10, -0x40, -0x14));
    out.push(MenuCall::Sprite { desc: 203, x: x + 0x12, y: y + 0x12 });
    out.push(number(x + 0x27, y + 0x14, 0, 4, i32::from(snap.trunk_count)));
    out.push(MenuCall::Box { colour: 2, x: x + 0x10, y: y + 0x24, wu: -0x40, hu: -0x1C });
    if first {
        out.push(MenuCall::Sprite { desc: 260, x: x + 0x15, y: y + 0x2D });
    }
    out.push(MenuCall::Sprite { desc: 196, x: x + 0x2C, y: y + 0x2C });
    out.push(panel(12, 7, x + 8, y + 0x40, -0xE0, -0x17E));
    for i in 0..10 {
        out.push(number(x + 0x32 + 0x12 * i, y + 0x45, 0, 1, i));
    }
    for r in 0..12 {
        out.push(number(x + 0xF, y + 0x60 + 0x1C * r, 0x20, 3, (base + r) * 10));
    }
    for r in 0..12 {
        for c in 0..10 {
            let card = (c + (base + r) * 10) as u16;
            let (cx, cy) = (x + 0x2D + 0x12 * c, y + 0x58 + 0x1C * r);
            if card == crate::card_detail::SPECIAL_CARD {
                // 0x216350 reads this card's own record (CardBlock::record
                // hides it as the game's other readers do).
                let seen = block.records.get(usize::from(card)).is_some_and(|r| r.rank_word & roses_core::card_block::OWNED_BIT != 0);
                if seen {
                    out.push(MenuCall::SpriteDim { desc: 220, x: cx, y: cy });
                }
                continue;
            }
            let Some(rec) = cards.record(card) else { continue };
            let owned = deck_editor::count_copies(block, card, None) > 0;
            if !owned && !deck_editor::seen(block, card) {
                continue;
            }
            let desc = 220 + usize::from(crate::card_detail::class(rec).1);
            out.push(if owned { MenuCall::Sprite { desc, x: cx, y: cy } } else { MenuCall::SpriteDim { desc, x: cx, y: cy } });
        }
    }
    out.push(MenuCall::Panel { grad: -1, deco: -1, ptype: 6, x: x + 0x28 + 0x12 * column, y: y + 0x53 + 0x1C * row, wu: -0x18, hu: -0x1E, bright: true });
    for i in 0..10 {
        out.push(number(x + 0x32 + 0x12 * i, y + 0x1AD, 0, 1, i));
    }
    out.push(panel(12, 12, x + 0xE8, y + 0x40, 1, -0x17E));
    out.push(MenuCall::Panel { grad: -1, deco: -1, ptype: 13, x: x + 0xE8, y: y + 0x50 + base * 0x12E / 74, wu: 1, hu: 1, bright: true });
    out.push(MenuCall::Box { colour: 2, x: x + 0x10, y: y + 0x1BE, wu: -0xAC, hu: -0x32 });
    out.push(MenuCall::Sprite { desc: 197, x: x + 0x2C, y: y + 0x1C2 });
    if first {
        out.push(MenuCall::Sprite { desc: 261, x: x + 0x15, y: y + 0x1C7 });
        out.push(MenuCall::Sprite { desc: 254, x: x + 0x17, y: y + 0x1DA });
        out.push(MenuCall::Gold { x: x + 0x2C, y: y + 0x1DD, size: 1, flags: 0, text: detail.back.clone(), dim: false });
    }
    out
}

/// 0x1BBF50, the first thing 0x1BBB90 draws: two untextured Gouraud strips
/// over the whole 512 × 512 frame (no blend), edge colour → centre colour
/// over x 0..256, centre → edge over 256..512, from 0x2BDBD0 (B, G, R, A
/// edge; B, G, R, A centre) [F]. Fixed coordinates: the background does not
/// scroll [F: XYZ2 0x7000/0x8000/0x9000 constants; PCSX2 pcsx2-catalogue.png
/// (64, 32, 0) at the edges, (160, 96, 0) at x 256].
pub fn background_calls(colours: [u8; 8]) -> Vec<roses_client::duel_hud::MenuCall> {
    use roses_client::duel_hud::MenuCall;
    let c = colours;
    let edge = [c[2], c[1], c[0], c[3]];
    let mid = [c[6], c[5], c[4], c[7]];
    [(0.0, 256.0, edge, mid), (256.0, 512.0, mid, edge)]
        .into_iter()
        .map(|(x0, x1, a, z)| MenuCall::Quad { corners: [(x0, 0.0, a), (x1, 0.0, z), (x0, 512.0, a), (x1, 512.0, z)] })
        .collect()
}

/// What the trunk list's REINCARNATION line reads besides the snapshot.
pub struct ReincarnationInputs<'a> {
    pub block: &'a CardBlock,
    /// SAVE+0xCA (0x23BCB0 reads it as a signed byte).
    pub duel_counter: u8,
    /// 0x1FEC40(): graphics still loading.
    pub graphics_busy: bool,
    /// 0x2DB828.
    pub blink: u16,
}

/// 0x1BC620's REINCARNATION line [F 0x1BC66C..0x1BC6A4, 0x1BCC48..0x1BCE10;
/// emu modes/ui3 reinc-*.json]: in states 2 and 3 (the trunk list, with or
/// without the pane), when the highlighted trunk card has a trunk copy
/// (0x216250(side, card, 3) > 0), nothing is loading, the duel counter is
/// at least 5 and one of the three decks is valid (40 cards and a leader):
/// 0x1B7B50 at (x + 0x84, 0x1C3), size 1, flags 3, colour 0x2905E0 with the
/// alpha 0x2DB828 >> 8 - the L3 gate of the update drawn as a blinking hint.
pub fn reincarnation_call(
    snap: &EditorSnapshot,
    inp: &ReincarnationInputs<'_>,
    data: &crate::card_detail::DetailData,
) -> Option<roses_client::duel_hud::MenuCall> {
    if snap.state != state::TRUNK_LIST && snap.state != state::TRUNK_LIST_DETAIL {
        return None;
    }
    let card = snap.trunk.get(usize::from(snap.trunk_window) + usize::from(snap.trunk_row)).copied()?;
    if deck_editor::count_copies(inp.block, card, Some(deck_editor::TRUNK_CATEGORY)) == 0
        || inp.graphics_busy
        || (inp.duel_counter as i8) < deck_editor::REINCARNATION_DUELS as i8
        || !(0..deck_editor::DECKS_PER_BLOCK).any(|d| deck_editor::deck_is_valid(inp.block, d))
    {
        return None;
    }
    let mut rgba = data.reincarnation_rgba;
    rgba[3] = (inp.blink >> 8) as u8;
    Some(roses_client::duel_hud::MenuCall::GoldTint {
        x: i32::from(snap.slide) + 0x84,
        y: 0x1C3,
        size: 1,
        flags: 3,
        text: data.reincarnation.clone(),
        rgba,
    })
}

/// The detail pane 0x1BBB60 draws this frame [F 0x1BBC5C..0x1BBDF0]: with
/// the slide > 0 the trunk side's card (state 0: the catalogue cell, only
/// when the block has a copy or the seen bit; else the trunk list's row;
/// t1 = state 1, t3 = 1), with the slide < 0 the deck list's row (side 1,
/// t1 = state 6, t3 = 0). y = 0x20, t2 = 0. The list screen itself moves by
/// the slide (0x1BC3B0 / 0x1BC620 / 0x1BCFB0 / 0x1BD8D0 take it as their x):
/// the caller translates `build_screen`'s primitives by it [F: the
/// argument; H: every element follows it].
pub struct PaneChoice {
    pub card: u16,
    pub side: u8,
    pub raised_tab: bool,
    pub editor: bool,
}

pub fn detail_pane(snap: &EditorSnapshot, block: &CardBlock) -> Option<PaneChoice> {
    use std::cmp::Ordering;
    match snap.slide.cmp(&0) {
        Ordering::Greater => {
            let card = if snap.state == state::CATALOGUE {
                let [base, column, row] = snap.catalogue.map(u16::from);
                let card = (base + row) * 10 + column;
                let any = deck_editor::count_copies(block, card, None) > 0 || deck_editor::seen(block, card);
                if !any {
                    return None;
                }
                card
            } else {
                *snap.trunk.get(usize::from(snap.trunk_window) + usize::from(snap.trunk_row))?
            };
            Some(PaneChoice { card, side: 0, raised_tab: snap.state == state::TRUNK_DETAIL, editor: true })
        }
        Ordering::Less => {
            let card = *snap.deck_list.get(usize::from(snap.deck_window) + usize::from(snap.deck_row))?;
            Some(PaneChoice { card, side: 1, raised_tab: snap.state == state::DECK_DETAIL, editor: false })
        }
        Ordering::Equal => None,
    }
}

/// The message boxes' 0x1B1830 arguments [F 0x1C1D90, 0x1C1E20, 0x1C1EB0].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageBox {
    pub message: u16,
    pub top: i32,
    pub width: i32,
    pub min_height: i32,
    /// "CAUTION" (else "PASSWORD").
    pub caution: bool,
    pub press: bool,
}

/// The box of a message state: wrong password (MSG 151), password already
/// used (MSG 152), deck not valid (MSG 125, ○ refused). 0xA (a card shown
/// with its box, 0x1C1B90) is not drawn.
pub fn message_box(editor_state: u8) -> Option<MessageBox> {
    let password = |message| MessageBox { message, top: 0xAD, width: 0x1CC, min_height: 0xA6, caution: false, press: true };
    match editor_state {
        state::MSG_PASSWORD_WRONG => Some(password(151)),
        state::MSG_PASSWORD_USED => Some(password(152)),
        state::MSG_DECK_INVALID => Some(MessageBox { message: 125, top: 0xC3, width: 0x100, min_height: 0x7A, caution: true, press: true }),
        _ => None,
    }
}

/// 0x1B1830(colour 2, message, centre 0x100, top, width, min height, title,
/// press) as `trade_screen::banner_box` lays it out; `counter` picks the
/// press icon's cel.
pub fn message_box_calls(
    editor_state: u8,
    tables: &HudTables,
    detail: &crate::card_detail::DetailData,
    counter: u32,
) -> Vec<roses_client::duel_hud::MenuCall> {
    use roses_client::duel_hud::{MenuCall, TextCanvas};
    let Some(m) = message_box(editor_state) else { return Vec::new() };
    let Some(codes) = tables.message(m.message) else { return Vec::new() };
    let title = if m.caution { &detail.caution } else { &detail.password };
    let cx = 0x100;
    let (tw, th) = crate::trade_screen::gold_measure(tables, 2, title);
    let (plate_w, plate_h) = (tw + 0x18, th + 0xC);
    let (lines, w) = TextCanvas::measure(codes);
    let (lines, w) = (lines as i32, w as i32);
    let width = m.width.max(w.max(plate_w) + 0x18);
    let icon = if m.press { 0x1C } else { 0 };
    let inner_h = m.min_height.max(plate_h + 0x18 + 22 * lines + 0xC + icon);
    let mut out = vec![
        MenuCall::Panel { grad: 13, deco: -1, ptype: 1, x: cx - (width >> 1) - 4, y: m.top, wu: -width, hu: -inner_h, bright: true },
        MenuCall::Panel { grad: 2, deco: -1, ptype: 11, x: cx - (plate_w >> 1), y: m.top + 0xC, wu: -plate_w, hu: -plate_h, bright: true },
        MenuCall::Gold { x: cx, y: m.top + 0x14, size: 2, flags: 3, text: title.clone(), dim: false },
        MenuCall::Text { line: 2, message: m.message, x: cx - (w >> 1), y: m.top + plate_h + 0x18, sx: 1.0, sy: 1.0, lines: lines as u8, dim: false },
    ];
    if m.press {
        out.push(MenuCall::Sprite { desc: 190 + ((counter / 5) % 3) as usize, x: cx - 8, y: m.top + inner_h - 0x1C });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use roses_client::duel_hud::MenuCall;

    #[test]
    fn the_refused_exit_and_the_password_messages_have_their_boxes() {
        // The arguments of 0x1C1EB0 / 0x1C1D90 / 0x1C1E20.
        let invalid = message_box(state::MSG_DECK_INVALID).unwrap();
        assert_eq!((invalid.message, invalid.top, invalid.width, invalid.min_height, invalid.caution), (125, 0xC3, 0x100, 0x7A, true));
        assert_eq!(message_box(state::MSG_PASSWORD_WRONG).unwrap().message, 151);
        assert_eq!(message_box(state::MSG_PASSWORD_USED).unwrap().message, 152);
        // The lists have no box.
        for s in [state::TRUNK_LIST, state::DECK_LIST, state::CATALOGUE] {
            assert_eq!(message_box(s), None);
        }
    }

    fn mine(c: &MenuCall) -> Option<(String, Vec<i64>)> {
        let v = |xs: &[i32]| xs.iter().map(|&x| i64::from(x)).collect();
        Some(match c {
            MenuCall::Panel { grad, deco, ptype, x, y, wu, hu, .. } => ("panel".into(), v(&[*grad, *deco, *ptype as i32, *x, *y, *wu, *hu])),
            MenuCall::Sprite { desc, x, y } => ("sprite".into(), v(&[*desc as i32, *x, *y, 0x80])),
            MenuCall::SpriteDim { desc, x, y } => ("sprite".into(), v(&[*desc as i32, *x, *y, 0x30])),
            MenuCall::Gold { x, y, size, flags, text, .. } => ("font".into(), [v(&[*x, *y, *size as i32, *flags as i32]), text.iter().map(|&b| i64::from(b)).collect()].concat()),
            MenuCall::Number { x, y, size, flags, digits, value, .. } => ("number".into(), v(&[*x, *y, *size as i32, *flags as i32, *digits, *value])),
            MenuCall::Box { x, y, wu, hu, colour } => ("f1b71d0".into(), v(&[*colour, *x, *y, *wu, *hu, -1])),
            _ => return None,
        })
    }

    fn logged(c: &serde_json::Value) -> Option<(String, Vec<i64>)> {
        let a: Vec<i64> = c["args"].as_array()?.iter().map(|x| x.as_i64().unwrap()).collect();
        let st: Vec<i64> = c["stack"].as_array()?.iter().map(|x| x.as_i64().unwrap()).collect();
        let fn_ = c["fn"].as_str()?;
        Some(match fn_ {
            "panel" => (fn_.into(), a[1..8].to_vec()),
            "sprite" => (fn_.into(), vec![(a[1] - 0x28_F300) / 10, a[2] - 0x700, a[3] - 0x700, st[10]]),
            "font" => (fn_.into(), [vec![a[1], a[2], a[3], a[4]], c["string"].as_str().unwrap_or("").bytes().map(i64::from).collect()].concat()),
            "number" => (fn_.into(), a[1..7].to_vec()),
            "f1b71d0" => (fn_.into(), a[1..7].to_vec()),
            _ => return None,
        })
    }

    /// The catalogue builder against the interpreter's leaf calls of
    /// 0x1C04E0 (`ROSES_CATALOGUE_CAPTURES` = modes/ui3/captures, files
    /// `cat-*.json` from tools/run_catalogue.py; `ROSES_DUEL_IMPORT`): every
    /// frame that draws the pane, every call in order with its values.
    #[test]
    fn private_catalogue_matches_the_emulated_draw() {
        let (Some(dir), Some(duel)) = (std::env::var_os("ROSES_CATALOGUE_CAPTURES"), std::env::var_os("ROSES_DUEL_IMPORT")) else {
            return;
        };
        let data = crate::duel_play::load(std::path::Path::new(&duel)).unwrap();
        let detail = crate::card_detail::DetailData::from_executable(&data.executable).unwrap();
        let cards = StaticCards::from_executable(&data.executable, 0).unwrap();
        let mut checked = 0;
        for e in std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().to_string();
            if !(name.starts_with("cat-") && name.ends_with(".json")) {
                continue;
            }
            let v: serde_json::Value = serde_json::from_slice(&std::fs::read(e.path()).unwrap()).unwrap();
            let hex = |s: &str| (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect::<Vec<u8>>();
            let block = CardBlock::from_bytes(&hex(v["start_block"].as_str().unwrap())).unwrap();
            let frames = v["frames"].as_array().unwrap();
            let calls = v["calls"].as_array().unwrap();
            for f in frames.iter().skip(1) {
                let frame = f["frame"].as_i64().unwrap() - 1;
                let theirs: Vec<_> = calls
                    .iter()
                    .filter(|c| c["frame"].as_i64() == Some(frame))
                    .filter(|c| {
                        let ra = i64::from_str_radix(c["ra"].as_str().unwrap().trim_start_matches("0x"), 16).unwrap();
                        (0x1C_04E0..0x1C_0CAC).contains(&ra)
                    })
                    .filter_map(logged)
                    .collect();
                let slide = f["slide"].as_i64().unwrap() as i16;
                if slide < 0x101 {
                    assert!(theirs.is_empty());
                    continue;
                }
                let cat = f["cat"].as_array().unwrap();
                let c = Catalogue {
                    state: f["state"].as_u64().unwrap() as u8,
                    slide,
                    catalogue: [0, 1, 2].map(|i| cat[i].as_u64().unwrap() as u8),
                    trunk_count: f["trunk_count"].as_u64().unwrap() as u16,
                };
                let ours: Vec<_> = catalogue_calls(&c, &block, &cards, &detail).iter().filter_map(mine).collect();
                for (k, (x, y)) in ours.iter().zip(theirs.iter()).enumerate() {
                    assert_eq!(x, y, "{name} frame {frame} call {k}");
                }
                assert_eq!(ours.len(), theirs.len(), "{name} frame {frame}");
                checked += 1;
            }
        }
        eprintln!("{checked} catalogue frames match");
        assert!(checked > 0);
    }

    /// The REINCARNATION line against the interpreter's 0x1BC620 font calls
    /// (`ROSES_CATALOGUE_CAPTURES`, files `reinc-*.json` from
    /// tools/run_catalogue.py with the `dc` / `dup` pokes): the port's editor
    /// model replays the same pad words on the same block, and every frame's
    /// call (present or not, x, y, size, flags, text) and the stepped 0x2DB828
    /// equal the original's.
    #[test]
    fn private_reincarnation_matches_the_emulated_draw() {
        use roses_core::deck_editor::{DeckEditor, EditorEnv, PadInput};
        let (Some(dir), Some(duel)) = (std::env::var_os("ROSES_CATALOGUE_CAPTURES"), std::env::var_os("ROSES_DUEL_IMPORT")) else {
            return;
        };
        let data = crate::duel_play::load(std::path::Path::new(&duel)).unwrap();
        let detail = crate::card_detail::DetailData::from_executable(&data.executable).unwrap();
        assert_eq!(detail.reincarnation, b"REINCARNATION");
        let cards = StaticCards::from_executable(&data.executable, 0).unwrap();
        let (mut shown, mut hidden) = (0, 0);
        for e in std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().to_string();
            if !(name.starts_with("reinc-") && name.ends_with(".json")) {
                continue;
            }
            let v: serde_json::Value = serde_json::from_slice(&std::fs::read(e.path()).unwrap()).unwrap();
            let hex = |s: &str| (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect::<Vec<u8>>();
            let mut block = CardBlock::from_bytes(&hex(v["start_block"].as_str().unwrap())).unwrap();
            let mut duel_counter: u8 = v["pokes"]
                .as_str()
                .unwrap()
                .split(',')
                .find_map(|p| p.strip_prefix("dc="))
                .map_or(0, |d| d.parse().unwrap());
            let mut rng = 1u64;
            let frames = v["frames"].as_array().unwrap();
            let calls = v["calls"].as_array().unwrap();
            let mut editor = {
                let mut env = EditorEnv { block: &mut block, duel_counter: &mut duel_counter, cards: &cards, rng: &mut rng, graphics_busy: false };
                DeckEditor::new(0, &mut env)
            };
            let mut wave = roses_client::duel_hud::FlashWave {
                value: frames[0]["c88"].as_u64().unwrap() as u16,
                rising: (frames[0]["d64"].as_u64().unwrap() >> 2) & 3 == 1,
            };
            for f in frames.iter().skip(1) {
                let frame = f["frame"].as_i64().unwrap() - 1;
                let word = f["pad"].as_u64().unwrap() as u16;
                wave.step();
                {
                    let mut env = EditorEnv { block: &mut block, duel_counter: &mut duel_counter, cards: &cards, rng: &mut rng, graphics_busy: false };
                    editor.step(PadInput { word, sticks: None }, &mut env);
                }
                assert_eq!(u64::from(wave.value), f["c88"].as_u64().unwrap(), "{name} frame {frame}: 0x2DB828");
                let snap = editor.snapshot();
                assert_eq!(i64::from(snap.state), f["state"].as_i64().unwrap(), "{name} frame {frame}: state");
                assert_eq!(i64::from(snap.slide), f["slide"].as_i64().unwrap(), "{name} frame {frame}: slide");
                let theirs: Vec<Vec<i64>> = calls
                    .iter()
                    .filter(|c| c["frame"].as_i64() == Some(frame) && c["fn"] == "font" && c["string"] == "REINCARNATION")
                    .map(|c| c["args"].as_array().unwrap()[1..5].iter().map(|x| x.as_i64().unwrap()).collect())
                    .collect();
                let inp = ReincarnationInputs { block: &block, duel_counter, graphics_busy: false, blink: wave.value };
                let ours: Vec<Vec<i64>> = reincarnation_call(&snap, &inp, &detail)
                    .into_iter()
                    .map(|c| match c {
                        MenuCall::GoldTint { x, y, size, flags, text, rgba } => {
                            assert_eq!(text, b"REINCARNATION");
                            assert_eq!(rgba, [0x80, 0x80, 0x80, (wave.value >> 8) as u8]);
                            vec![i64::from(x), i64::from(y), size as i64, i64::from(flags)]
                        }
                        other => panic!("{other:?}"),
                    })
                    .collect();
                assert_eq!(ours, theirs, "{name} frame {frame}");
                if ours.is_empty() {
                    hidden += 1;
                } else {
                    shown += 1;
                }
            }
        }
        eprintln!("REINCARNATION: {shown} frames drawn, {hidden} without, all as the original");
        assert!(shown > 0 && hidden > 0);
    }
}