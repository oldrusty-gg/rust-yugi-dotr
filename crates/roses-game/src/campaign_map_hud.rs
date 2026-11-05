//! The campaign map's 2D HUD (mode 0x0B draw pass 0x21D4E0) as calls of
//! the original's emitters, then as `duel_hud` primitives drawn from the
//! user's TEXSYS.MRG atlases #6/#7, gold font and message font.
//!
//! Provenance [F, emu]: the draw functions 0x233FC0 (bottom panel),
//! 0x2349B0 (SYSTEM box), 0x229C40 (cursor), 0x234CC0 (hover panel) and
//! 0x2360C0 (place tooltip) were read instruction by instruction, and the
//! calls they make were logged from the EE interpreter at state 622 + 47
//! frames (`story/the project notes, Chester
//! hovered); the private test below compares this builder's calls with
//! that log. The emitters: 0x217360 (sprite descriptor: rect + offset, the
//! sprite record 0x294480 + 20·id gives the atlas rect and slot 0 = TEXSYS
//! #7, 1 = #6), 0x217440 (sprite at a point with the record's size),
//! 0x218E70 (window record x0, y0, x1, y1, style: fill and frame pieces),
//! 0x218460 (gouraud fill), 0x1B7B50 / 0x1B7A80 (gold font, numbers) and
//! 0x246B90 (a composed message line, squeezed to a width limit). The
//! sprite emitter 0x217760 writes integer pixel corners and texel centres
//! (+0.5) [F]. Nothing here is a game byte: descriptor and record addresses
//! are read from the user's executable at run time.
//!
//! Not drawn (said by `notes`): the hover panel's opponent figure (the
//! W+540C model pass 0x247310 and the three 0x2177F0 sprites), the white
//! route's and free-duel mode's variants (W+18C, window colours 10/11).
//! [H] The window alpha argument changes only the fill (the frame pieces are
//! drawn at 128); the text canvas uses the duel HUD's CLUT.
use roses_client::campaign_map::{self as map, MapModel};
use roses_client::duel_hud::{
    Element, HUD_Z, HudTables, PrimKind, Primitive, Target, TextureSource, Vertex,
};

/// Card names start at message 323 (0x1DF480).
pub const CARD_NAME_MESSAGE: u16 = 323;

const SPRITES: u32 = 0x29_4480;
const SPRITE_STRIDE: u32 = 20;
const DESC_STRIDE: u32 = 38;
const WINDOW_STRIDE: u32 = 10;

// Descriptor tables of the bottom panel (0x233FC0).
const DESC_SIDE_EMBLEM: u32 = 0x29_74B0;
const WINDOWS_BOTTOM: u32 = 0x29_6F00;
const WINDOW_RANK_BAR: u32 = 0x29_6FA0;
const DESC_DECK_ICON: u32 = 0x29_7200;
const DESC_RANK: u32 = 0x29_6FC0;
const DESC_STAR_A: u32 = 0x29_7060;
const DESC_STAR_B: u32 = 0x29_7086;
const DESC_STAR_C: u32 = 0x29_70AC;
const DESC_ROSES: u32 = 0x29_75F0;
const DESC_LABELS: u32 = 0x29_7550;
const STRING_ROSE: u32 = 0x2D_2727;
const STRING_CARDS: u32 = 0x2D_272C;
const SPACE_MESSAGE: u16 = 4;
// SYSTEM box (0x2349B0).
const DESC_SYSTEM_FILL: u32 = 0x29_76D8;
const DESC_SYSTEM_ICONS: u32 = 0x29_7640;
const STRING_ICON_LABELS: [(u32, usize); 4] =
    [(0x2D_2732, 4), (0x2D_2737, 4), (0x2D_273C, 6), (0x2D_2743, 4)];
const DESC_ENTER: u32 = 0x29_7F30;
const DESC_ENTER_HOVERED: u32 = 0x29_7F56;
const STRING_SYSTEM: u32 = 0x2D_2748;
// Cursor (0x229C40).
const DESC_CURSOR: [u32; 3] = [0x29_5880, 0x29_58A6, 0x29_58CC];
// Hover panel (0x234CC0).
const DESC_PANEL_EMBLEM: u32 = 0x29_7500;
const WINDOW_PANEL_NAME: u32 = 0x29_6F20;
const WINDOW_PANEL_DC: u32 = 0x29_6F34;
const WINDOW_PANEL_FIGURE: u32 = 0x29_6F3E;
const DESC_PANEL_DC: u32 = 0x29_7700;
const DESC_ORNAMENT_LEFT: u32 = 0x29_77CC;
const DESC_ORNAMENT_RIGHT: u32 = 0x29_77F2;
const DESC_ORNAMENT_EMBLEM: u32 = 0x29_7780;
// The record panel (0x2359C4).
const WINDOW_RECORD_A: u32 = 0x29_6F2A;
const WINDOW_RECORD_B: u32 = 0x29_6F48;
const WINDOW_RECORD_C: u32 = 0x29_6FAA;
const STRING_DECK: u32 = 0x2D_2732;
const STRING_LEADER: u32 = 0x2D_2818;
const DESC_RECORD_TYPE: u32 = 0x29_7480;
const DESC_RECORD_RANK: u32 = 0x29_70E0;
const DESC_RECORD_STAR_A: u32 = 0x29_7180;
const DESC_RECORD_STAR_B: u32 = 0x29_71A6;
const DESC_RECORD_STAR_C: u32 = 0x29_71CC;
const DESC_RECORD_WIN: u32 = 0x29_7726;
const DESC_RECORD_LOSS: u32 = 0x29_774C;
/// The static card records (20 bytes; byte 0 & 0x1F = type) [F 0x1DE350].
const CARD_TABLE: u32 = 0x2B_E400;
/// Switch 0x2D25D0 on type + 1 (0x235B28..0x235C68): type -> sprite id.
const TYPE_ICON_SPRITES: [u16; 21] = [
    78, 88, 93, 79, 83, 85, 86, 87, 81, 82, 89, 84, 90, 91, 92, 80, 94, 95, 96, 97, 101,
];
const NODE_TABLE: u32 = 0x29_7F80;
/// The page box quad (0x230840's copy at sp+0x140).
const TEXT_BOX: u32 = 0x29_7ED6;
/// The advance mark (0x22E888, slot 1 (0, 112) 16×16).
const ADVANCE_SPRITE: u16 = 152;
const NODE_STRIDE: u32 = 0x34;
// The story's bottom speaker panel (0x232EF0, W+53BA), drawn over the map
// for 12040/12041: the hover panel's emblem and name window (x1 = 384,
// 0x232F20), the flipped wings and the ornament.
const DIALOGUE_NAME_X1: i32 = 0x180;
const DIALOGUE_NAME_LIMIT: i32 = 0x144;
const DESC_DIALOGUE_WING_LEFT: u32 = 0x29_7818;
const DESC_DIALOGUE_WING_RIGHT: u32 = 0x29_783E;

/// Draw-order keys: the original's ordering-table keys of this frame, in
/// ascending order (the log's 3312, 5056, 7600, 9872, 10960).
const KEY_HUD: u32 = 0;
const KEY_HUD_TEXT: u32 = 1;
const KEY_PANEL: u32 = 2;
const KEY_PANEL_TEXT: u32 = 3;
const KEY_TOOLTIP: u32 = 4;
/// The page box (the log's 22208).
const KEY_TEXT: u32 = 5;
/// The story draw after the HUD: letterbox 0x233750, then the centred
/// window 0x232C80 and the advance mark [emu trace-trigger f245].
const KEY_LETTERBOX: u32 = 6;
const KEY_STORY_WINDOW: u32 = 7;
/// The leader box 0x21D5DC: appended to the packet after the map's draw
/// pass 0x229570 returns, so after everything above [F; emu trace-win3
/// f15: the last 2D call of the update].
const KEY_LEADER_BOX: u32 = 8;
/// The leader box's gouraud quad (TL, TR, BL, BR halfwords, then four
/// RGBA corners): (16, 120)-(106, 284), black alpha 64 [F, emu].
const DESC_LEADER_BOX: u32 = 0x29_7EB0;

const WHITE: [u8; 4] = [128, 128, 128, 128];

/// One emitter call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    /// 0x217500: a textured sprite (x0, y0)-(x1, y1) sampling the atlas
    /// rect at (u, v) of `size` texels (texel centres).
    Quad {
        key: u32,
        xy: [i32; 4],
        uv: [i32; 2],
        size: [i32; 2],
        rgba: [u8; 4],
        slot: u8,
        filter: bool,
    },
    /// 0x217B10: a textured quad of four corners (TL, TR, BL, BR) and
    /// four texel corners (a quad sprite record: flips and stretches).
    Quad4 {
        key: u32,
        xy: [[i32; 2]; 4],
        uv: [[i32; 2]; 4],
        rgba: [u8; 4],
        slot: u8,
        filter: bool,
    },
    /// 0x2467C0: row `row` of a composed message (canvas line 1 + row)
    /// with its four corner alphas (TL, TR, BL, BR; 0x80 opaque).
    TextRow {
        key: u32,
        message: u16,
        row: u8,
        x: i32,
        y: i32,
        alpha: [u8; 4],
    },
    /// 0x218460: a gouraud quad (TL, TR, BL, BR).
    Fill {
        key: u32,
        xy: [[i32; 2]; 4],
        rgba: [[u8; 4]; 4],
    },
    /// 0x1B7B50: a gold-font string.
    Font {
        key: u32,
        x: i32,
        y: i32,
        size: usize,
        flags: u32,
        text: Vec<u8>,
    },
    /// 0x246B90: a composed message line at (x, y), squeezed to `limit`.
    Text {
        key: u32,
        message: u16,
        x: i32,
        y: i32,
        limit: i32,
    },
    /// 0x246B90 with a colour (the tutorial menu's rows: the chosen row
    /// 0xFF, the others 0x40, alpha 0x80; 0x23BA00).
    TintedText {
        key: u32,
        message: u16,
        x: i32,
        y: i32,
        rgba: [u8; 4],
    },
}

impl Call {
    fn key(&self) -> u32 {
        match self {
            Call::Quad { key, .. }
            | Call::Quad4 { key, .. }
            | Call::TextRow { key, .. }
            | Call::Fill { key, .. }
            | Call::Font { key, .. }
            | Call::Text { key, .. }
            | Call::TintedText { key, .. } => *key,
        }
    }
}

/// The player's deck as the bottom panel reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeckHud {
    /// 0x215040(0): the current deck.
    pub index: u8,
    /// W+5444 = 0x215690(0, deck).
    pub cost: u16,
    /// 0x215160(0, deck): the deck leader.
    pub leader: u16,
    /// 0x2154C0(0, leader): the leader's stored rank (0 = none).
    pub leader_rank: u16,
}

/// An opponent's static deck as the record panel reads it (0x214120 loads
/// record 0x1A + C4 into side 1): its leader and the leader's rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpponentHud {
    pub leader: u16,
    pub leader_rank: u16,
}

/// A 0x26-byte sprite descriptor.
#[derive(Clone, Copy, Debug)]
struct Desc {
    rect: [i32; 4],
    rgba: [u8; 4],
    filter: bool,
    sprite: u16,
}

/// What the builder could not draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note(pub String);

struct Builder<'a> {
    exe: &'a [u8],
    key: u32,
    calls: Vec<Call>,
    notes: Vec<Note>,
}

fn read(exe: &[u8], address: u32, len: usize) -> Option<&[u8]> {
    roses_client::duel_hud::elf_slice(exe, address, len).ok()
}

impl Builder<'_> {
    fn note(&mut self, text: impl Into<String>) {
        let n = Note(text.into());
        if !self.notes.contains(&n) {
            self.notes.push(n);
        }
    }

    fn desc(&mut self, address: u32) -> Option<Desc> {
        let Some(b) = read(self.exe, address, DESC_STRIDE as usize) else {
            self.note(format!("descriptor 0x{address:X} is outside the executable"));
            return None;
        };
        let h = |o: usize| i32::from(i16::from_le_bytes([b[o], b[o + 1]]));
        Some(Desc {
            rect: [h(0), h(2), h(4), h(6)],
            rgba: [b[16], b[17], b[18], b[19]],
            filter: b[33] != 0,
            sprite: u16::from_le_bytes([b[36], b[37]]),
        })
    }

    /// The sprite record: atlas (u, v, w, h) and slot.
    fn sprite(&mut self, id: u16) -> Option<([i32; 4], u8)> {
        let address = SPRITES + u32::from(id) * SPRITE_STRIDE;
        let Some(b) = read(self.exe, address, SPRITE_STRIDE as usize) else {
            self.note(format!("sprite {id} is outside the executable"));
            return None;
        };
        let h = |o: usize| i32::from(i16::from_le_bytes([b[o], b[o + 1]]));
        Some(([h(0), h(2), h(4), h(6)], b[16]))
    }

    fn string(&mut self, address: u32, len: usize) -> Vec<u8> {
        match read(self.exe, address, len) {
            Some(b) => b.to_vec(),
            None => {
                self.note(format!("string 0x{address:X} is outside the executable"));
                Vec::new()
            }
        }
    }

    fn quad(&mut self, xy: [i32; 4], sprite: u16, rgba: [u8; 4], filter: bool) {
        if let Some(([u, v, w, h], slot)) = self.sprite(sprite) {
            self.calls.push(Call::Quad {
                key: self.key,
                xy,
                uv: [u, v],
                size: [w, h],
                rgba,
                slot,
                filter,
            });
        }
    }

    /// 0x217360: the descriptor's rect moved by (dx, dy).
    fn sprite_desc(&mut self, address: u32, dx: i32, dy: i32) {
        if let Some(d) = self.desc(address) {
            let [x0, y0, x1, y1] = d.rect;
            self.quad([x0 + dx, y0 + dy, x1 + dx, y1 + dy], d.sprite, d.rgba, d.filter);
        }
    }

    /// 0x217360 with a rect already adjusted (the SYSTEM icon growth).
    fn sprite_rect(&mut self, address: u32, rect: [i32; 4]) {
        if let Some(d) = self.desc(address) {
            self.quad(rect, d.sprite, d.rgba, d.filter);
        }
    }

    /// 0x2177F0 mode 0: a four-corner descriptor moved by (dx, dy) with
    /// the quad sprite record's four texel corners.
    fn quad_desc(&mut self, address: u32, dx: i32, dy: i32) {
        self.quad_desc_mode(address, dx, dy, 0);
    }

    /// 0x2177F0 with its mode argument: modes 1/2/3 take one texel off the
    /// sprite record's u / v / both corners (0x217944..0x21797C) [F].
    fn quad_desc_mode(&mut self, address: u32, dx: i32, dy: i32, mode: u8) {
        let Some(b) = read(self.exe, address, DESC_STRIDE as usize) else {
            self.note(format!("descriptor 0x{address:X} is outside the executable"));
            return;
        };
        let h = |o: usize| i32::from(i16::from_le_bytes([b[o], b[o + 1]]));
        let xy = [0, 1, 2, 3].map(|k| [h(4 * k) + dx, h(4 * k + 2) + dy]);
        let rgba = [b[16], b[17], b[18], b[19]];
        let filter = b[33] != 0;
        let sprite = u16::from_le_bytes([b[36], b[37]]);
        let address = SPRITES + u32::from(sprite) * SPRITE_STRIDE;
        let Some(r) = read(self.exe, address, SPRITE_STRIDE as usize) else {
            self.note(format!("sprite {sprite} is outside the executable"));
            return;
        };
        let hr = |o: usize| i32::from(i16::from_le_bytes([r[o], r[o + 1]]));
        let (du, dv) = (i32::from(mode & 1), i32::from(mode >> 1 & 1));
        let uv = [0, 1, 2, 3].map(|k| [hr(4 * k) - du, hr(4 * k + 2) - dv]);
        self.calls.push(Call::Quad4 {
            key: self.key,
            xy,
            uv,
            rgba,
            slot: r[16],
            filter,
        });
    }

    /// A sprite record at (x, y) with its own size, full colour.
    fn sprite_desc_at_sprite(&mut self, sprite: u16, x: i32, y: i32) {
        if let Some(([_, _, w, h], _)) = self.sprite(sprite) {
            self.quad([x, y, x + w, y + h], sprite, WHITE, false);
        }
    }

    /// 0x217440: the sprite's own size at (x, y).
    fn sprite_at(&mut self, address: u32, x: i32, y: i32) {
        if let Some(d) = self.desc(address) {
            if let Some(([_, _, w, h], _)) = self.sprite(d.sprite) {
                self.quad([x, y, x + w, y + h], d.sprite, d.rgba, d.filter);
            }
        }
    }

    fn fill(&mut self, [x0, y0, x1, y1]: [i32; 4], top: [u8; 4], bottom: [u8; 4]) {
        self.calls.push(Call::Fill {
            key: self.key,
            xy: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
            rgba: [top, top, bottom, bottom],
        });
    }

    /// 0x218E70: window record `address` moved by (dx, dy), colour `sel`,
    /// fill alpha `alpha`.
    fn window(&mut self, address: u32, sel: i32, dx: i32, dy: i32, alpha: i32) {
        self.window_rect(address, None, sel, dx, dy, alpha);
    }

    /// A window on a record whose width and height the caller wrote (the
    /// caption's copy of 0x296F52).
    #[allow(clippy::too_many_arguments)]
    fn window_rect_size(
        &mut self,
        address: u32,
        width: i32,
        height: i32,
        sel: i32,
        dx: i32,
        dy: i32,
        alpha: i32,
    ) {
        self.window_rect_full(address, Some(width), Some(height), sel, dx, dy, alpha);
    }

    fn window_rect(
        &mut self,
        address: u32,
        width: Option<i32>,
        sel: i32,
        dx: i32,
        dy: i32,
        alpha: i32,
    ) {
        self.window_rect_full(address, width, None, sel, dx, dy, alpha);
    }

    #[allow(clippy::too_many_arguments)]
    fn window_rect_full(
        &mut self,
        address: u32,
        width: Option<i32>,
        height: Option<i32>,
        sel: i32,
        dx: i32,
        dy: i32,
        alpha: i32,
    ) {
        let Some(b) = read(self.exe, address, WINDOW_STRIDE as usize) else {
            self.note(format!("window 0x{address:X} is outside the executable"));
            return;
        };
        let h = |o: usize| i32::from(i16::from_le_bytes([b[o], b[o + 1]]));
        let (mut x1, style) = (h(4), h(8));
        if let Some(w) = width {
            // 0x2361C4 writes the tooltip's width into the record.
            x1 = h(0) + w;
        }
        let y1 = height.map_or(h(6), |hh| h(2) + hh);
        let (x0, y0, y1) = (h(0) + dx, h(2) + dy, y1 + dy);
        let x1 = x1 + dx;
        self.window_box([x0, y0, x1, y1], style, sel, alpha);
    }

    /// 0x218E70's drawing for a record (x0, y0, x1, y1, style) already in
    /// place: colour `sel`, fill alpha `alpha`.
    fn window_box(&mut self, [x0, y0, x1, y1]: [i32; 4], style: i32, sel: i32, alpha: i32) {
        let inner: [u8; 3] = match sel {
            // 10/11 (the tutorial menu's title plate, 10 + side): the same
            // gradient as 0/1 [emu modes/tutorial45/menu-hud-calls.json].
            0 | 10 => [96, 0, 0],
            1 | 11 => [0, 96, 96],
            2 => [0, 0, 0],
            3 => [128, 128, 128],
            4 => [255, 255, 255],
            5 => [0, 0, 96],
            _ => {
                self.note(format!("window colour {sel} (W+6C-driven) is not drawn"));
                return;
            }
        };
        let outer = inner.map(|c| c >> 1);
        let a = alpha.clamp(0, 255) as u8;
        let with = |c: [u8; 3], a: u8| [c[0], c[1], c[2], a];
        match style {
            0 | 2 => {
                let fill = [x0 + 2, y0 + 2, x1 - 2, y1 - 2];
                match sel {
                    2 => self.fill(fill, with(inner, a), with(inner, a)),
                    3 => {}
                    _ => {
                        let mid = fill[1] + (fill[3] - fill[1]) / 2;
                        self.fill([fill[0], fill[1], fill[2], mid], with(outer, a), with(inner, a));
                        self.fill([fill[0], mid, fill[2], fill[3]], with(inner, a), with(outer, a));
                    }
                }
                let atlas = if style == 0 { [64, 0, 16, 14] } else { [48, 64, 8, 8] };
                self.frame([x0, y0, x1, y1], atlas, WHITE);
            }
            // The tutorial menu's styles [emu modes/tutorial45/menu-hud-calls
            // .json]: 5 / 7 the top / bottom half of a large window (fill
            // inset 6, 16-px pieces of TEXSYS #7 at (160, 208)), 6 the
            // chosen row's bar (fill inset 2, 8-px frame at (48, 240)).
            5 => {
                self.fill([x0 + 6, y0 + 6, x1 - 6, y1], with(inner, a), with(inner, a));
                self.atlas_quad([x0, y0, x0 + 16, y0 + 16], [160, 208], [16, 16], WHITE);
                self.atlas_quad([x1 - 16, y0, x1, y0 + 16], [176, 208], [16, 16], WHITE);
                self.atlas_quad([x0 + 16, y0, x1 - 16, y0 + 16], [175, 208], [1, 16], WHITE);
                self.atlas_quad([x0, y0 + 16, x0 + 16, y1], [160, 223], [16, 1], WHITE);
                self.atlas_quad([x1 - 16, y0 + 16, x1, y1], [176, 223], [16, 1], WHITE);
            }
            7 => {
                self.fill([x0 + 6, y0, x1 - 6, y1 - 6], with(inner, a), with(inner, a));
                self.atlas_quad([x0, y1 - 16, x0 + 16, y1], [160, 224], [16, 16], WHITE);
                self.atlas_quad([x1 - 16, y1 - 16, x1, y1], [176, 224], [16, 16], WHITE);
                self.atlas_quad([x0 + 16, y1 - 16, x1 - 16, y1], [175, 224], [1, 16], WHITE);
                self.atlas_quad([x0, y0, x0 + 16, y1 - 16], [160, 223], [16, 1], WHITE);
                self.atlas_quad([x1 - 16, y0, x1, y1 - 16], [176, 223], [16, 1], WHITE);
            }
            6 => {
                self.fill([x0 + 2, y0 + 2, x1 - 2, y1 - 2], with(inner, a), with(inner, a));
                self.frame([x0, y0, x1, y1], [48, 240, 8, 8], WHITE);
            }
            3 => {
                let piece = with(inner, 128);
                self.bar([x0, y0, x1, y1], [64, 56, 16, 28], piece);
            }
            4 => {
                self.fill([x0 + 2, y0 + 2, x1 - 2, y1], with(outer, a), with(outer, a));
                self.bar_pieces([x0, y0, x1, y1], [50, 116, 9, 12], [60, 116], WHITE);
            }
            _ => self.note(format!("window style {style} is not drawn")),
        }
    }

    fn atlas_quad(&mut self, xy: [i32; 4], uv: [i32; 2], size: [i32; 2], rgba: [u8; 4]) {
        self.calls.push(Call::Quad {
            key: self.key,
            xy,
            uv,
            size,
            rgba,
            slot: 0,
            filter: false,
        });
    }

    /// The eight frame pieces of styles 0 and 2 (corners, then the
    /// stretched edges), TEXSYS #7.
    fn frame(&mut self, [x0, y0, x1, y1]: [i32; 4], [sx, sy, cw, ch]: [i32; 4], rgba: [u8; 4]) {
        let pieces = [
            ([x0, y0, x0 + cw, y0 + ch], [sx, sy], [cw, ch]),
            ([x1 - cw, y0, x1, y0 + ch], [sx + cw, sy], [cw, ch]),
            ([x0, y1 - ch, x0 + cw, y1], [sx, sy + ch], [cw, ch]),
            ([x1 - cw, y1 - ch, x1, y1], [sx + cw, sy + ch], [cw, ch]),
            ([x0 + cw, y0, x1 - cw, y0 + ch], [sx + cw, sy], [1, ch]),
            ([x0 + cw, y1 - ch, x1 - cw, y1], [sx + cw, sy + ch], [1, ch]),
            ([x0, y0 + ch, x0 + cw, y1 - ch], [sx, sy + ch], [cw, 1]),
            ([x1 - cw, y0 + ch, x1, y1 - ch], [sx + cw, sy + ch], [cw, 1]),
        ];
        for (xy, uv, size) in pieces {
            self.atlas_quad(xy, uv, size, rgba);
        }
    }

    /// Style 3: a one-row bar (left cap, right cap, stretched middle).
    fn bar(&mut self, rect: [i32; 4], [sx, sy, cw, ch]: [i32; 4], rgba: [u8; 4]) {
        self.bar_pieces(rect, [sx, sy, cw, ch], [sx + cw, sy], rgba);
    }

    fn bar_pieces(
        &mut self,
        [x0, y0, x1, y1]: [i32; 4],
        [sx, sy, cw, ch]: [i32; 4],
        right: [i32; 2],
        rgba: [u8; 4],
    ) {
        let top = y1 - ch;
        let _ = y0;
        self.atlas_quad([x0, top, x0 + cw, y1], [sx, sy], [cw, ch], rgba);
        self.atlas_quad([x1 - cw, top, x1, y1], right, [cw, ch], rgba);
        self.atlas_quad([x0 + cw, top, x1 - cw, y1], [sx + cw, sy], [1, ch], rgba);
    }

    fn font(&mut self, x: i32, y: i32, size: usize, flags: u32, text: Vec<u8>) {
        self.calls.push(Call::Font {
            key: self.key,
            x,
            y,
            size,
            flags,
            text,
        });
    }

    /// 0x1B7A80 without zero padding: right-aligned in `width` cells.
    fn number(&mut self, x: i32, y: i32, size: usize, width: usize, value: i32) {
        let digits = value.max(0).to_string();
        let mut text = vec![b' '; width.saturating_sub(digits.len())];
        text.extend_from_slice(digits.as_bytes());
        self.font(x, y, size, 0, text);
    }

    fn text(&mut self, message: u16, x: i32, y: i32, limit: i32) {
        self.calls.push(Call::Text {
            key: self.key,
            message,
            x,
            y,
            limit,
        });
    }
}

/// The HUD's emitter calls this frame, in draw order, and what was left out.
pub fn calls(
    model: &MapModel,
    exe: &[u8],
    deck: &DeckHud,
    opponents: &std::collections::BTreeMap<u8, OpponentHud>,
) -> (Vec<Call>, Vec<Note>) {
    let mut b = Builder {
        exe,
        key: KEY_HUD,
        calls: Vec::new(),
        notes: Vec::new(),
    };
    if model.save.side != 0 {
        b.note("the White Rose side's HUD uses the red layout with side 1's emblem and windows (its own branches are not traced)");
    }
    bottom_panel(&mut b, model, deck);
    system_box(&mut b, model);
    cursor(&mut b, model);
    hover_panel(&mut b, model, opponents);
    tooltip(&mut b, model);
    text_box(&mut b, model);
    letterbox(&mut b, model);
    story_window(&mut b, model);
    story_dialogue(&mut b, model);
    tutorial_menu(&mut b, model);
    let mut calls = b.calls;
    calls.sort_by_key(Call::key);
    (calls, b.notes)
}

/// SYSTEM icon 3's lesson menu (0x23B770) [F, emu modes/tutorial45/
/// menu-hud-calls.json]: the window records 0x29A3C0 (two halves, the row
/// bar, the title plate) at y offset s0 = 0x200 − W+543F·0x40; the two
/// halves at (32, 80 + s0), colour 2, fill alpha 0x65; the plate at (215,
/// 94 + s0), colour 10 + side; "HELP" (0x2D2743) in gold size 2 at (228,
/// 102 + s0); the rows MSG 4087 + r at (64, 144 + 36 r + s0) (each 22 px
/// high + 14) and the bar at (48, row y − 3), colour 2, alpha 0.
fn tutorial_menu(b: &mut Builder, model: &MapModel) {
    const WINDOWS: u32 = 0x29_A3C0;
    const STRING_HELP: u32 = 0x2D_2743;
    let Some(menu) = model.tutorial_menu.as_ref() else {
        return;
    };
    let key = b.key;
    b.key = KEY_TOOLTIP;
    let s0 = i32::from(menu.y());
    let side = i32::from(model.save.side);
    b.window(WINDOWS, 2, 32, 80 + s0, 0x65);
    b.window(WINDOWS + WINDOW_STRIDE, 2, 32, 80 + s0, 0x65);
    b.window(WINDOWS + 3 * WINDOW_STRIDE, 10 + side, 215, 94 + s0, 128);
    let help = b.string(STRING_HELP, 4);
    b.font(228, 102 + s0, 2, 1, help);
    for r in 0..map::TUTORIAL_ROWS {
        let y = 144 + 36 * i32::from(r) + s0;
        let c = if r == menu.row { 0xFF } else { 0x40 };
        b.calls.push(Call::TintedText {
            key: b.key,
            message: map::TutorialMenu::label(r),
            x: 64,
            y,
            rgba: [c, c, c, 0x80],
        });
    }
    let y = 144 + 36 * i32::from(menu.row) + s0 - 3;
    // 0x23BB00: the bar record's y1 := the row's height + 6 (22 + 6).
    b.window_rect_size(WINDOWS + 2 * WINDOW_STRIDE, 416, 22 + 6, 2, 48, y, 0);
    b.key = key;
}

/// 0x233FC0 (W+18C = 0, red side).
fn bottom_panel(b: &mut Builder, model: &MapModel, deck: &DeckHud) {
    let s3 = 320 - model.hud.slide;
    let side = i32::from(model.save.side);
    let alpha = model.hud.alpha_bottom;
    b.sprite_desc(DESC_SIDE_EMBLEM + side as u32 * DESC_STRIDE, 0, s3);
    for k in 0..3 {
        b.window(WINDOWS_BOTTOM + k * WINDOW_STRIDE, side, 0, s3, alpha);
    }
    b.window(WINDOW_RANK_BAR, side, 0, s3, alpha);
    b.sprite_desc(DESC_DECK_ICON + u32::from(deck.index) * DESC_STRIDE, 0, s3);
    if deck.leader_rank != 0 {
        let s0 = u32::from(deck.leader_rank - 1);
        b.sprite_desc(DESC_RANK + (s0 / 3) * DESC_STRIDE, 0, s3);
        let stars: &[u32] = match s0 % 3 {
            0 => &[DESC_STAR_C],
            1 => &[DESC_STAR_A, DESC_STAR_B],
            _ => &[DESC_STAR_A, DESC_STAR_B, DESC_STAR_C],
        };
        for &d in stars {
            b.sprite_desc(d, 0, s3);
        }
    }
    // The rose-card nodes (jump table 0x2D25A0; the white side's
    // 0x2345EC..) at state ≥ 3.
    let roses = model.save.rose_cards(&model.data) as i32;
    for i in 0..roses {
        b.sprite_desc(DESC_ROSES, 14 * i, s3);
    }
    for i in 0..8 {
        b.sprite_desc(DESC_ROSES + DESC_STRIDE, 14 * i, s3);
    }
    for k in 0..4 {
        b.sprite_desc(DESC_LABELS + k * DESC_STRIDE, 0, s3);
    }
    b.number(141, s3 + 384, 1, 4, i32::from(deck.cost));
    b.number(231, s3 + 384, 1, 4, i32::from(model.save.wins));
    b.number(317, s3 + 384, 1, 4, i32::from(model.save.losses));
    let rose = b.string(STRING_ROSE, 4);
    b.font(379, s3 + 375, 1, 1, rose);
    let cards = b.string(STRING_CARDS, 5);
    b.font(418, s3 + 375, 1, 1, cards);
    b.key = KEY_HUD_TEXT;
    b.text(SPACE_MESSAGE, 54, s3 + 348, i32::MAX);
    b.key = KEY_PANEL;
    // The leader's name on the rank bar record 0x296F14: (x0 + 4, y0 + 2).
    let bar = read(b.exe, WINDOWS_BOTTOM + 2 * WINDOW_STRIDE, 4)
        .map(|r| (i32::from(i16::from_le_bytes([r[0], r[1]])), i32::from(i16::from_le_bytes([r[2], r[3]]))));
    if let Some((x, y)) = bar {
        b.text(CARD_NAME_MESSAGE + deck.leader, x + 4, y + 2 + s3, 248);
    }
    b.key = KEY_HUD;
}

/// 0x2349B0.
fn system_box(b: &mut Builder, model: &MapModel) {
    let s1 = 256 - model.hud.slide;
    if let Some(d) = read(b.exe, DESC_SYSTEM_FILL, 16) {
        let h = |o: usize| i32::from(i16::from_le_bytes([d[o], d[o + 1]]));
        b.fill(
            [h(0), h(2) + s1, h(4), h(10) + s1],
            [0, 0, 0, 64],
            [0, 0, 0, 64],
        );
    }
    let s6 = s1 + 324;
    for i in 0..4u8 {
        let address = DESC_SYSTEM_ICONS + u32::from(i) * DESC_STRIDE;
        let Some(d) = b.desc(address) else { continue };
        let hovered = model.hovered_icon == Some(i);
        let mut rect = d.rect;
        if hovered {
            let g = (model.hud.icon_grow[usize::from(i)] * 6) >> 4;
            rect = [rect[0] - g, rect[1] - g, rect[2] + g, rect[3] + g];
        }
        b.sprite_rect(address, [rect[0], rect[1] + s1, rect[2], rect[3] + s1]);
        if hovered {
            let (s, len) = STRING_ICON_LABELS[usize::from(i)];
            let label = b.string(s, len);
            b.font(438, s6, 1, 1, label);
        }
    }
    if model.hovered_icon.is_none() {
        b.sprite_desc(DESC_ENTER, -124, s1 - 11);
        let system = b.string(STRING_SYSTEM, 6);
        b.font(438, s6, 1, 1, system);
    } else {
        b.sprite_desc(DESC_ENTER_HOVERED, -124, s1 - 11);
    }
}

/// 0x229BE4..0x229CE8: the hand cursor, drawn while W+5374 is clear and
/// the paths are idle (also over an open text [emu hud-oracle-653]).
fn cursor(b: &mut Builder, model: &MapModel) {
    if model.hud.hidden || !paths_idle(model) {
        return;
    }
    let (x, y) = (i32::from(model.cursor.0), i32::from(model.cursor.1));
    match model.cursor_mode {
        0 => b.sprite_at(DESC_CURSOR[0], x, y),
        1 => b.sprite_at(DESC_CURSOR[1], x, y),
        _ => {
            b.sprite_at(DESC_CURSOR[1], x, y);
            b.sprite_at(DESC_CURSOR[2], x, y);
        }
    }
}

fn paths_idle(model: &MapModel) -> bool {
    model.paths.iter().all(|p| p.state == 0 || p.state == 3)
}

/// 0x234CC0..0x235934: the hovered node's panel (speaker emblem, name,
/// opponent DC), sliding in with the fade W+6C. The opponent figure and
/// its frame decorations are not drawn.
fn hover_panel(
    b: &mut Builder,
    model: &MapModel,
    opponents: &std::collections::BTreeMap<u8, OpponentHud>,
) {
    if model.hud.panel <= 0 {
        caption(b, model);
        return;
    }
    // C6: the last hovered node (the panel slides out on it).
    let node = model.save.hovered;
    let state = model.save.state(node);
    // Visibility (0x2352B4..0x2353BC, red side): not on nodes 0/1 or an
    // unrevealed node; W+53AD is assumed clear [H]. The White Rose side
    // shows the nodes with a speaker (Brest is Yugi's duel there) [H: its
    // branch of the visibility code is not traced].
    let Some(variant) = model.hover_text(node) else {
        return;
    };
    let visible = match (model.save.side, node) {
        (0, 0 | 1) => false,
        (0, _) => (1..=5).contains(&state),
        _ => (1..=5).contains(&state) && variant.speaker_message.is_some(),
    };
    if !visible {
        return;
    }
    b.key = KEY_PANEL;
    let s2 = ((model.hud.panel * 5 * 64) >> 8) - 320;
    // s7: Stonehenge shows the other side's emblem; elsewhere the node's
    // base model 8 picks emblem 0, any other emblem 1 (0x234E28..).
    let s7 = if node == map::STONEHENGE_NODE {
        i32::from(model.save.side ^ 1)
    } else {
        let base = read(b.exe, NODE_TABLE + u32::from(node) * NODE_STRIDE + 0x10, 2)
            .map(|r| i16::from_le_bytes([r[0], r[1]]));
        i32::from(base != Some(8))
    };
    b.sprite_desc(DESC_PANEL_EMBLEM + s7 as u32 * DESC_STRIDE, 0, s2);
    let alpha = model.hud.alpha_top;
    b.window(WINDOW_PANEL_NAME, s7, 0, s2, alpha);
    b.window(WINDOW_PANEL_DC, s7, 0, s2, alpha);
    // The figure window (t2 = 1, fill alpha 0) at fp = s2 - 16 * W+68
    // (W+68 = 0 [H]); its ornaments (0x2177F0) below it.
    b.window(WINDOW_PANEL_FIGURE, s7, 0, s2, 0);
    b.note("the hover panel's talking face (0x247310 slot) is not drawn");
    b.quad_desc(DESC_ORNAMENT_LEFT, 0, s2);
    b.quad_desc(DESC_ORNAMENT_RIGHT, 0, s2);
    b.quad_desc(DESC_ORNAMENT_EMBLEM + s7 as u32 * DESC_STRIDE, 0, s2);
    b.sprite_desc(DESC_PANEL_DC, -100, s2);
    if let Some(dc) = model.opponent_dc {
        b.number(310, s2 + 26, 0, 4, i32::from(dc));
    } else {
        b.note("the opponent's deck cost is unknown; its number is not drawn");
    }
    if let Some(speaker) = variant.speaker_message {
        b.key = KEY_PANEL_TEXT;
        b.text(speaker, 53, s2 + 21, 224);
    }
    b.key = KEY_PANEL;
    // The record panel (s3): nodes at states 2..=5, or the post-duel text
    // (W+537B with W+5442 = 100, or W+537A with W+5442 < 100) [H: the
    // text flags are taken as "a text window is open"].
    // W+537A is the post-duel text's flag: 0 during the encounter taunt
    // [emu 653-mgo: 537A 0, 537B 4, 5442 0].
    // The gate 0x235398: Stonehenge's second visit (W+53C4 = 1) shows no
    // record before state 4.
    let gated = node == map::STONEHENGE_NODE && model.w53c4 == 1 && state < 4;
    // 0x23548C..0x2354F8: W+537B (the text box's display state, 12 once
    // the page closed) with W+5442 = 100, or W+537A and W+537B with W+5442
    // < 100: the record stays up through the countdown after the page
    // closed [emu trace-win3 f15..f48: disp 2..12, ph537a 3].
    let returning = matches!(model.handler, map::Handler::Return { phase: 2.., .. });
    let record = ((2..=5).contains(&state) && !gated)
        || (returning && model.countdown <= 100);
    if record {
        // 0x2359C4 raises W+548C before anything else of the record panel.
        leader_box(b, model);
        let found = variant
            .opponent
            .and_then(|o| opponents.get(&o).map(|h| (o, *h)));
        match found {
            Some(opponent) => record_panel(b, model, s7, s2, opponent),
            None => b.note("the record panel's opponent deck is unknown; it is not drawn"),
        }
    }
    b.key = KEY_HUD;
}

/// 0x21D548..0x21D5E4: the black box behind the opponent leader's model
/// (object 31, drawn into the box by 0x21DA40 once loaded). It is drawn
/// on the update the record panel raised W+548C (cleared after the draw)
/// while W+6C ≠ 0, moved by x = −(640·W+5442)/100 (sliding in with the
/// post-duel countdown, −64 per update) and y = (W+6C·320 >> 8) − 320
/// (−25 per update as the panel slides out under a story entry) [F; emu
/// trace-win3 f15..f61, trace-seto-cam f204..f215]. [H] The flag survives
/// an update with W+6C = 0 (the draw is skipped without clearing it); the
/// port draws the box only on updates that draw the record panel. The
/// leader's 3D model inside the box is not drawn.
fn leader_box(b: &mut Builder, model: &MapModel) {
    if model.hud.panel == 0 {
        return;
    }
    let Some(d) = read(b.exe, DESC_LEADER_BOX, 32) else {
        b.note("the leader box descriptor is outside the executable");
        return;
    };
    let h = |o: usize| i32::from(i16::from_le_bytes([d[o], d[o + 1]]));
    let corner = |k: usize| [d[16 + 4 * k], d[17 + 4 * k], d[18 + 4 * k], d[19 + 4 * k]];
    let dx = -(640 * model.countdown) / 100;
    let dy = ((model.hud.panel * 5 * 64) >> 8) - 320;
    let key = b.key;
    b.key = KEY_LEADER_BOX;
    b.calls.push(Call::Fill {
        key: b.key,
        xy: [
            [h(0) + dx, h(2) + dy],
            [h(4) + dx, h(6) + dy],
            [h(8) + dx, h(10) + dy],
            [h(12) + dx, h(14) + dy],
        ],
        rgba: [corner(0), corner(1), corner(2), corner(3)],
    });
    b.note("the opponent leader's model in the leader box (object 31, 0x21DA40) is not drawn");
    b.key = key;
}

/// The hover panel's talking-face request (0x2355FC..0x2357A0): the
/// speaker index (C4 + 1; C4 when the speaker message is 218), the kind
/// (1 = blink; 3 = blink and talk, taken for the post-duel text [H: W+540C
/// = 10's setter is untraced]) and the slot position (the figure window's x0 = 400, its y0 +
/// the panel slide; W+68 = 0 [H]). `None` while the panel is not drawn.
pub fn panel_face(model: &MapModel) -> Option<(u8, u8, i16, i16)> {
    if model.hud.panel <= 0 {
        return None;
    }
    let node = model.save.hovered;
    let state = model.save.state(node);
    let variant = model.hover_text(node)?;
    let visible = match (model.save.side, node) {
        (0, 0 | 1) => false,
        (0, _) => (1..=5).contains(&state),
        _ => (1..=5).contains(&state) && variant.speaker_message.is_some(),
    };
    if !visible {
        return None;
    }
    let c4 = variant.opponent?;
    let speaker = if variant.speaker_message == Some(218) { c4 } else { c4 + 1 };
    // W+540C stays 2 (kind 1, blinking) through the encounter taunt [emu
    // 653-mgo]; kind 3 (talking) is taken for the post-duel text [H].
    let post_duel_text = model.text.is_some() && matches!(model.handler, map::Handler::Return { .. });
    let kind = if post_duel_text { 3 } else { 1 };
    let s2 = ((model.hud.panel * 5 * 64) >> 8) - 320;
    Some((speaker, kind, 400, s2 as i16))
}

/// 0x2359C4..0x235F88: the record against the hovered opponent, sliding in
/// from the left with the post-duel countdown: x offset
/// -(640 * W+5442) / 100 - 112.
fn record_panel(
    b: &mut Builder,
    model: &MapModel,
    sel: i32,
    s2: i32,
    (opponent, hud): (u8, OpponentHud),
) {
    let s1 = -(640 * model.countdown) / 100 - 112;
    let alpha = model.hud.alpha_top;
    for w in [WINDOW_RECORD_A, WINDOW_RECORD_B, WINDOW_RECORD_C] {
        b.window(w, sel, s1, s2, alpha);
    }
    let deck = b.string(STRING_DECK, 4);
    b.font(s1 + 152, s2 + 63, 1, 1, deck);
    let leader = b.string(STRING_LEADER, 6);
    b.font(s1 + 192, s2 + 63, 1, 1, leader);
    // The leader's type icon: card byte 0 & 0x1F through switch 0x2D25D0,
    // written into descriptor 0x297480's sprite id.
    let kind = read(b.exe, CARD_TABLE + u32::from(hud.leader) * 20, 1).map(|r| r[0] & 0x1F);
    let sprite = kind.and_then(|k| TYPE_ICON_SPRITES.get(usize::from(k)).copied());
    match (b.desc(DESC_RECORD_TYPE), sprite) {
        (Some(d), Some(sprite)) => {
            let [x0, y0, x1, y1] = d.rect;
            b.quad([x0 + s1, y0 + s2, x1 + s1, y1 + s2], sprite, d.rgba, d.filter);
        }
        _ => b.note("the record panel's type icon is not drawn"),
    }
    if hud.leader_rank != 0 {
        let s0 = u32::from(hud.leader_rank - 1);
        b.sprite_desc(DESC_RECORD_RANK + (s0 / 3) * DESC_STRIDE, s1, s2);
        let stars: &[u32] = match s0 % 3 {
            0 => &[DESC_RECORD_STAR_C],
            1 => &[DESC_RECORD_STAR_A, DESC_RECORD_STAR_B],
            _ => &[DESC_RECORD_STAR_A, DESC_RECORD_STAR_B, DESC_RECORD_STAR_C],
        };
        for &d in stars {
            b.sprite_desc(d, s1, s2);
        }
    }
    b.sprite_desc(DESC_RECORD_WIN, s1, s2);
    b.sprite_desc(DESC_RECORD_LOSS, s1, s2);
    let o = usize::from(opponent);
    let wins = model.save.wins_by_opponent.get(o).copied().unwrap_or(0);
    let losses = model.save.losses_by_opponent.get(o).copied().unwrap_or(0);
    b.number(s1 + 324, s2 + 73, 1, 4, i32::from(wins));
    b.number(s1 + 421, s2 + 73, 1, 4, i32::from(losses));
    let bar = read(b.exe, WINDOW_RECORD_B, 4).map(|r| {
        (
            i32::from(i16::from_le_bytes([r[0], r[1]])),
            i32::from(i16::from_le_bytes([r[2], r[3]])),
        )
    });
    if let Some((x, y)) = bar {
        b.key = KEY_PANEL_TEXT;
        b.text(CARD_NAME_MESSAGE + hud.leader, x + 4 + s1, y + 2 + s2, 246);
        b.key = KEY_PANEL;
    }
}

/// 0x2329F8..0x232C28: caption 2520 ("Choose a destination") in a window
/// centred on x 256 at y 64 while the HUD is shown, the hover panel is
/// out, no text is open and nothing is hovered.
fn caption(b: &mut Builder, model: &MapModel) {
    if !model.hud.shown
        || model.hud.hidden
        || model.text.is_some()
        || model.cursor_mode != 0
        || model.hovered_icon.is_some()
        || !paths_idle(model)
        || !matches!(model.handler, map::Handler::Free)
    {
        return;
    }
    let message = map::CAPTION_MESSAGE;
    let window = (text_width(model, message) + 12).min(480);
    let x = 256 - (window >> 1);
    b.key = KEY_TOOLTIP;
    b.window_rect_size(0x29_6F52, window, 22 + 12, 2, x, 64, 64);
    b.text(message, x + 6, 70, i32::MAX);
    b.key = KEY_HUD;
}

/// 0x2360C0..0x2363E4: the place tooltip, or MSG 250 while over DC: the
/// same window and text calls, MSG 250 centred on x 256 at y 288 (W+5448,
/// cleared after the draw at 0x2363E0) [F].
fn tooltip(b: &mut Builder, model: &MapModel) {
    b.key = KEY_TOOLTIP;
    let tooltip = if model.over_dc {
        Some((map::OVER_DC_MESSAGE, None))
    } else {
        model.place_tooltip().map(|m| (m, Some(model.cursor)))
    };
    if let Some((message, cursor)) = tooltip {
        let width = text_width(model, message);
        let mut window = width + 16;
        let mut limit = i32::MAX;
        if cursor.is_none() && window > 480 {
            // 0x2362B0..0x2362F4: MSG 250's window at most 480 wide at x
            // 16, the text squeezed by (480 − 16) / its width (0x2363B8).
            window = 480;
            limit = window - 16;
        }
        let (x, y) = match cursor {
            Some((cx, cy)) => {
                let (cx, cy) = (i32::from(cx), i32::from(cy));
                let half = window >> 1;
                let mut x = cx - half;
                if x < 121 {
                    x = 120;
                }
                if cx + half >= 370 {
                    x = 370 - window;
                }
                (x, (cy + 32).clamp(128, 354))
            }
            None => (256 - (window >> 1), 288),
        };
        // Both through 0x218E70 on record 0x296F52 (its width written at
        // 0x2361C4 / 0x2362C8), colour 2, fill alpha 0x40 [F 0x23627C,
        // 0x2362F8]; the text at (x + 8, y + 4) (0x23633C / 0x2363A0, y
        // 0x124 for MSG 250).
        b.window_rect(0x29_6F52, Some(window), 2, x, y, 64);
        b.text(message, x + 8, y + 4, limit);
    }
    b.key = KEY_HUD;
}

/// 0x230840 (style 0, W+537C = 0): the page box. The fill is descriptor
/// 0x297ED6's quad with its bottom at y0 + (y2 − y0)·W+53A2/100; each row r
/// of the composed message is a text-canvas row at (x0 + 6, y0 + 6 + 22r)
/// (0x2467C0). Then the advance mark (sprite 152 at (380, 108), 0x22E888)
/// while the page waits. [H] Rows show at full alpha once revealed (the
/// original ramps four corner alphas per row, W+538A); the mark does not
/// move.
fn text_box(b: &mut Builder, model: &MapModel) {
    let Some(text) = &model.text else { return };
    if text.phase < 2 || text.openness <= 0 {
        return;
    }
    let Some(d) = read(b.exe, TEXT_BOX, 32) else {
        b.note("the text box descriptor is outside the executable");
        return;
    };
    let h = |o: usize| i32::from(i16::from_le_bytes([d[o], d[o + 1]]));
    let (x0, y0, x1, y2) = (h(0), h(2), h(4), h(10));
    let bottom = y0 + (y2 - y0) * text.openness.min(100) / 100;
    // Corner colours: four RGBA bytes each from +16 (black, alpha 64).
    let top = [d[16], d[17], d[18], d[19]];
    let low = [d[24], d[25], d[26], d[27]];
    b.key = KEY_TEXT;
    b.fill([x0, y0, x1, bottom], top, low);
    if text.phase >= 3 && text.phase < 11 {
        let rows = model
            .data
            .messages
            .get(&text.message)
            .map_or(1, |m| usize::from(m.lines.max(1)))
            .min(3);
        let alphas = match text.phase {
            3 => reveal_alphas(rows, text.revealed),
            5 => fade_alphas(reveal_alphas(rows, u32::MAX), text.timer),
            _ => reveal_alphas(rows, u32::MAX),
        };
        for (r, alpha) in alphas.iter().enumerate().take(rows) {
            if alpha.iter().all(|&a| a == 0) {
                continue;
            }
            b.calls.push(Call::TextRow {
                key: b.key,
                message: text.message,
                row: r as u8,
                x: x0 + 6,
                y: y0 + 6 + 22 * r as i32,
                alpha: *alpha,
            });
        }
    }
    if text.phase == 4 {
        b.sprite_desc_at_sprite(ADVANCE_SPRITE, 380, 108);
    }
    b.key = KEY_HUD;
}

/// 0x233750: two opaque black bands (0x2181C0, GS alpha 128) over raw rows
/// 0..S and 512 − S..512, S = W+1E + W+20 + W+22 [F, emu].
fn letterbox(b: &mut Builder, model: &MapModel) {
    let height = model.letterbox.height();
    if height <= 0 {
        return;
    }
    let black = [0, 0, 0, 128];
    b.key = KEY_LETTERBOX;
    b.fill([0, 0, 512, height], black, black);
    b.fill([0, 512 - height, 512, 512], black, black);
    b.key = KEY_HUD;
}

/// 0x232C80 (W+5487 = 1, events 11120/11124/11149): a style-0 window
/// (record (0, 0, w, h, 0) moved to (x, 256), colour 2, fill alpha 64
/// inset 2, TEXSYS #7 frame (64, 0) 16x14) and the composed message whole
/// at (x + 6, 262) on every frame of the page (0x246B90; drawn here as its
/// rows at full alpha); the advance mark at (472, 472) while the page
/// waits [emu trace-trigger f240..f260, trace-win3 f1751..f1777].
fn story_window(b: &mut Builder, model: &MapModel) {
    let Some(window) = &model.story_window else {
        return;
    };
    let Some([x, y, w, h]) = model.story_window_rect() else {
        b.note(format!("story window message {} is not in the map data", window.message));
        return;
    };
    b.key = KEY_STORY_WINDOW;
    b.window_box([x, y, x + w, y + h], 0, 2, 64);
    let lines = model
        .data
        .messages
        .get(&window.message)
        .map_or(1, |m| i32::from(m.lines.max(1)));
    for r in 0..lines.min(3) {
        b.calls.push(Call::TextRow {
            key: b.key,
            message: window.message,
            row: r as u8,
            x: x + map::STORY_WINDOW_TEXT_DX,
            y: map::STORY_WINDOW_TEXT_Y + 22 * r,
            alpha: [128; 4],
        });
    }
    if window.phase == 4 {
        let (mx, my) = map::STORY_ADVANCE_MARK;
        b.sprite_desc_at_sprite(ADVANCE_SPRITE, mx, my);
    }
    b.key = KEY_HUD;
}

/// 0x2332B0..0x233628 (W+53BA = s, 12040/12041 over the map): the bottom
/// speaker panel, x offset s3 = 512 - s/2 for the emblem, name window and
/// name, y offset s0 = 256 - s/4 for the face window, wings and ornament;
/// then the page rows at (136, 400 + 22r) (0x2329B4, revealed like the map
/// pages) and the advance mark at (472, 472) while the page waits [F, emu
/// trace-dover f40/f62]. The face itself is the face slot (`dialogue_face`).
fn story_dialogue(b: &mut Builder, model: &MapModel) {
    let Some(d) = model.story_dialogue.as_ref().filter(|d| d.drawn()) else {
        return;
    };
    b.key = KEY_STORY_WINDOW;
    let (s3, s0) = d.offsets();
    let sel = i32::from(d.selector);
    b.sprite_desc(DESC_PANEL_EMBLEM + sel as u32 * DESC_STRIDE, s3 + 120, 352);
    match read(b.exe, WINDOW_PANEL_NAME, 2) {
        Some(r) => {
            let x0 = i32::from(i16::from_le_bytes([r[0], r[1]]));
            b.window_rect(WINDOW_PANEL_NAME, Some(DIALOGUE_NAME_X1 - x0), sel, s3 + 120, 352, 128);
        }
        None => b.note("the dialogue's name window is outside the executable"),
    }
    // The face window: record (0, 0, 104, 144, style 0), fill alpha 0.
    let y = s0 + map::DIALOGUE_FACE_Y;
    let x = i32::from(map::DIALOGUE_FACE_X);
    b.window_box([x, y, x + 104, y + 144], 0, sel, 0);
    b.quad_desc_mode(DESC_DIALOGUE_WING_LEFT, -384, s0 + 216, 2);
    b.quad_desc_mode(DESC_DIALOGUE_WING_RIGHT, -384, s0 + 216, 2);
    b.quad_desc(DESC_ORNAMENT_EMBLEM + sel as u32 * DESC_STRIDE, -384, s0 + 208);
    b.text(d.name_message, s3 + 173, 373, DIALOGUE_NAME_LIMIT);
    if (3..=5).contains(&d.phase) {
        let rows = model
            .data
            .messages
            .get(&d.message)
            .map_or(1, |m| usize::from(m.lines.max(1)))
            .min(3);
        let alphas = match d.phase {
            3 => reveal_alphas(rows, d.timer),
            5 => fade_alphas(reveal_alphas(rows, u32::MAX), d.timer),
            _ => reveal_alphas(rows, u32::MAX),
        };
        for (r, alpha) in alphas.iter().enumerate().take(rows) {
            if alpha.iter().all(|&a| a == 0) {
                continue;
            }
            b.calls.push(Call::TextRow {
                key: b.key,
                message: d.message,
                row: r as u8,
                x: map::DIALOGUE_TEXT_X,
                y: map::DIALOGUE_TEXT_Y + 22 * r as i32,
                alpha: *alpha,
            });
        }
    }
    if d.phase == 4 {
        let (mx, my) = map::STORY_ADVANCE_MARK;
        b.sprite_desc_at_sprite(ADVANCE_SPRITE, mx, my);
    }
    b.key = KEY_HUD;
}

/// The dialogue panel's face request (speaker, kind, x, y) and mood.
pub fn dialogue_face(model: &MapModel) -> Option<((u8, u8, i16, i16), u8)> {
    let d = model.story_dialogue.as_ref()?;
    d.face_request().map(|f| (f, d.mood))
}

/// The row reveal of layouts 0/2 (0x230B94..0x230D0C) after `updates`
/// updates: each update walks the rows, then TL, TR, BL, BR, adding 0x30
/// (clamped to 0x80) where the gate holds, the gates seeing this pass's
/// values: TL of row 0 always, of row r when row r−1's TR ≥ 0x80; BL when
/// TL ≥ 0x41; TR when BL ≥ 0x80; BR when TR ≥ 0x21. 1/2/3-line pages
/// finish in 7/13/19 updates [F, emu the project notes].
pub fn reveal_alphas(rows: usize, updates: u32) -> [[u8; 4]; 3] {
    // Shared with the story scene's motion.
    roses_client::story_motion::reveal_alphas(rows.max(1), updates)
}

/// The page-turn fade of layout 0 (0x230D14..): every alpha −0x18 per update.
fn fade_alphas(a: [[u8; 4]; 3], updates: u32) -> [[u8; 4]; 3] {
    roses_client::story_motion::fade_alphas(a, updates)
}

/// The composed width of a message line: 12 texels per code [F 0x245BB0].
fn text_width(model: &MapModel, message: u16) -> i32 {
    model
        .data
        .messages
        .get(&message)
        .map_or(0, |m| 12 * i32::from(m.glyphs))
}

// ------------------------------------------------------------- primitives

fn vtx(x: f32, y: f32, u: f32, v: f32, rgba: [u8; 4]) -> Vertex {
    Vertex { x, y, u, v, rgba }
}

fn prim(kind: PrimKind, texture: TextureSource, alpha_test: bool, gouraud: bool, verts: Vec<Vertex>) -> Primitive {
    Primitive {
        element: Element::Banner,
        kind,
        texture,
        target: Target::Frame,
        blend: Some(0),
        alpha_test,
        bilinear: matches!(texture, TextureSource::TextCanvas { .. }),
        gouraud,
        z: HUD_Z,
        verts,
    }
}

/// The calls as `duel_hud` primitives (atlases 6/7, the gold font of
/// `tables`, the text canvas line 1).
pub fn primitives(calls: &[Call], tables: &HudTables, notes: &mut Vec<Note>) -> Vec<Primitive> {
    let mut prims = Vec::new();
    for call in calls {
        match call {
            Call::Quad {
                xy,
                uv,
                size,
                rgba,
                slot,
                filter,
                ..
            } => {
                let atlas = if *slot == 0 { 7 } else { 6 };
                let (u0, v0) = (uv[0] as f32 + 0.5, uv[1] as f32 + 0.5);
                let (u1, v1) = (u0 + size[0] as f32, v0 + size[1] as f32);
                let mut p = prim(
                    PrimKind::Sprite,
                    TextureSource::Atlas(atlas),
                    true,
                    false,
                    vec![
                        vtx(xy[0] as f32, xy[1] as f32, u0, v0, *rgba),
                        vtx(xy[2] as f32, xy[3] as f32, u1, v1, *rgba),
                    ],
                );
                p.bilinear = *filter;
                prims.push(p);
            }
            Call::Quad4 {
                xy,
                uv,
                rgba,
                slot,
                filter,
                ..
            } => {
                let atlas = if *slot == 0 { 7 } else { 6 };
                let verts = (0..4)
                    .map(|i| {
                        vtx(
                            xy[i][0] as f32,
                            xy[i][1] as f32,
                            uv[i][0] as f32 + 0.5,
                            uv[i][1] as f32 + 0.5,
                            *rgba,
                        )
                    })
                    .collect();
                let mut p = prim(PrimKind::TriStrip, TextureSource::Atlas(atlas), true, false, verts);
                p.bilinear = *filter;
                prims.push(p);
            }
            Call::TextRow {
                message,
                row,
                x,
                y,
                alpha,
                ..
            } => {
                let Some(codes) = tables.message(*message) else {
                    let n = Note(format!("message {message} is not in the HUD tables"));
                    if !notes.contains(&n) {
                        notes.push(n);
                    }
                    continue;
                };
                let line_codes = codes
                    .split(|&c| c & 0x7FFF == 0x2000)
                    .nth(usize::from(*row))
                    .map_or(0, <[u16]>::len);
                let w = (12 * line_codes) as f32;
                let v = 22.0 * (1 + u32::from(*row)) as f32;
                let (x0, y0) = (*x as f32, *y as f32);
                let c = |a: u8| [128, 128, 128, a];
                prims.push(prim(
                    PrimKind::TriStrip,
                    TextureSource::TextCanvas {
                        line: 1,
                        message: Some(*message),
                    },
                    true,
                    true,
                    vec![
                        vtx(x0, y0, 0.5, v + 0.5, c(alpha[0])),
                        vtx(x0 + w, y0, w + 0.5, v + 0.5, c(alpha[1])),
                        vtx(x0, y0 + 22.0, 0.5, v + 22.5, c(alpha[2])),
                        vtx(x0 + w, y0 + 22.0, w + 0.5, v + 22.5, c(alpha[3])),
                    ],
                ));
            }
            Call::Fill { xy, rgba, .. } => {
                let verts = (0..4)
                    .map(|i| vtx(xy[i][0] as f32, xy[i][1] as f32, 0.0, 0.0, rgba[i]))
                    .collect();
                prims.push(prim(PrimKind::TriStrip, TextureSource::None, false, true, verts));
            }
            Call::Font {
                x,
                y,
                size,
                flags,
                text,
                ..
            } => gold(&mut prims, tables, notes, *x, *y, *size, *flags, text),
            Call::TintedText {
                message, x, y, rgba, ..
            } => {
                let Some(codes) = tables.message(*message).map(<[u16]>::len) else {
                    let n = Note(format!("message {message} is not in the HUD tables"));
                    if !notes.contains(&n) {
                        notes.push(n);
                    }
                    continue;
                };
                let w = (12 * codes) as f32;
                let (x0, y0) = (*x as f32, *y as f32);
                prims.push(prim(
                    PrimKind::Sprite,
                    TextureSource::TextCanvas {
                        line: 1,
                        message: Some(*message),
                    },
                    true,
                    false,
                    vec![
                        vtx(x0, y0, 0.5, 22.5, *rgba),
                        vtx(x0 + w, y0 + 22.0, w + 0.5, 44.5, *rgba),
                    ],
                ));
            }
            Call::Text {
                message, x, y, limit, ..
            } => {
                let Some(codes) = tables.message(*message).map(<[u16]>::len) else {
                    let n = Note(format!("message {message} is not in the HUD tables"));
                    if !notes.contains(&n) {
                        notes.push(n);
                    }
                    continue;
                };
                let w = 12 * codes as i32;
                let sx = if w < *limit { 1.0 } else { *limit as f32 / w as f32 };
                let dw = (w as f32 * sx) as i32;
                let (x0, y0) = (*x as f32, *y as f32);
                prims.push(prim(
                    PrimKind::Sprite,
                    TextureSource::TextCanvas {
                        line: 1,
                        message: Some(*message),
                    },
                    true,
                    false,
                    vec![
                        vtx(x0, y0, 0.5, 22.5, WHITE),
                        vtx(x0 + dw as f32, y0 + 22.0, w as f32 + 0.5, 44.5, WHITE),
                    ],
                ));
            }
        }
    }
    prims
}

/// The gold font (0x1B7B50): monospace cells of the size's width unless
/// proportional (flags bit 0); glyphs centred in their cell.
#[allow(clippy::too_many_arguments)]
fn gold(
    prims: &mut Vec<Primitive>,
    tables: &HudTables,
    notes: &mut Vec<Note>,
    x: i32,
    y: i32,
    size: usize,
    flags: u32,
    text: &[u8],
) {
    let glyph = |ch: u8| -> Option<usize> {
        match ch {
            b'A'..=b'Z' => Some(usize::from(ch - b'A')),
            b'a'..=b'z' => Some(usize::from(ch - b'a')),
            b'0'..=b'9' => Some(usize::from(ch - b'0') + 26),
            b'?' => Some(36),
            _ => None,
        }
    };
    let Some(&(cw, _)) = tables.font_sizes.get(size) else {
        notes.push(Note(format!("gold font size {size} is not in the table")));
        return;
    };
    let cw = i32::from(cw);
    let proportional = flags & 1 != 0;
    let mut pen = x as f32;
    for &ch in text {
        let Some(g) = glyph(ch) else {
            pen += (if proportional { cw >> 1 } else { cw }) as f32;
            continue;
        };
        let Some(d) = tables.glyphs.get(g + 37 * size).copied() else {
            return;
        };
        let (gw, gh) = (d.width(), d.height());
        let ox = if proportional { 0.0 } else { (cw - gw) as f32 / 2.0 };
        let gx = pen + ox;
        let (u0, v0) = (f32::from(d.u0) + 0.5, f32::from(d.v0) + 0.5);
        let (u1, v1) = (f32::from(d.u1) + 0.5, f32::from(d.v1) + 0.5);
        prims.push(prim(
            PrimKind::TriStrip,
            TextureSource::Atlas(d.slot),
            true,
            false,
            vec![
                vtx(gx, y as f32, u0, v0, WHITE),
                vtx(gx + gw as f32, y as f32, u1, v0, WHITE),
                vtx(gx, (y + gh) as f32, u0, v1, WHITE),
                vtx(gx + gw as f32, (y + gh) as f32, u1, v1, WHITE),
            ],
        ));
        pen += (if proportional { gw } else { cw }) as f32;
    }
}

/// The deck words of the bottom panel from the player's card block.
pub fn deck_hud(block: &roses_core::card_block::CardBlock) -> DeckHud {
    use roses_core::deck_editor;
    let index = deck_editor::current_deck(block);
    let deck = deck_editor::deck_record(block, index);
    let leader = deck.leader;
    let leader_rank = block.record(leader).map_or(0, |r| r.stored_rank());
    DeckHud {
        index,
        cost: deck.cost,
        leader,
        leader_rank,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The page box while the encounter text is open (state 653-mgo: the
    /// box (16,48)-(480,128) black at alpha 64 [emu hud-oracle-653]).
    #[test]
    fn rows_reveal_in_7_13_and_19_updates() {
        // The dialogue findings' timings for 1/2/3-line pages.
        for (rows, updates) in [(1, 7), (2, 13), (3, 19)] {
            let done = |n| reveal_alphas(rows, n)[rows - 1][3] >= 0x80;
            assert!(!done(updates - 1), "{rows} rows done before {updates}");
            assert!(done(updates), "{rows} rows not done at {updates}");
        }
        assert_eq!(reveal_alphas(1, 1)[0], [0x30, 0, 0, 0]);
        assert_eq!(reveal_alphas(1, 2)[0], [0x60, 0, 0x30, 0]);
    }

    /// 11149's window over the map (trace-win3 f1755, f1770 [emu]): the
    /// 144-row letterbox fills, the style-0 window (136,256) 240x34 with its
    /// fill (138,258)-(374,288) and frame pieces, the text at (142, 262),
    /// the advance mark at (472, 472) once the page waits.
    #[test]
    #[ignore = "requires ROSES_MAP_MANIFEST and ROSES_DUEL_IMPORT"]
    fn private_story_window_matches_the_traced_frame() {
        use std::path::Path;
        let manifest = std::env::var_os("ROSES_MAP_MANIFEST").expect("ROSES_MAP_MANIFEST");
        let duel = std::env::var_os("ROSES_DUEL_IMPORT").expect("ROSES_DUEL_IMPORT");
        let data = crate::duel_play::load_with_end(Path::new(&duel), None).unwrap();
        let loaded = crate::campaign_map::load_map(Path::new(&manifest)).unwrap();
        let mut map_data = loaded.data.clone();
        let parsed = roses_assets::parse_pal_messages(&data.messages, &map::WINDOW_MESSAGE_IDS).unwrap();
        for m in parsed.messages {
            map_data
                .messages
                .entry(m.message_id)
                .or_insert_with(|| map::MessageInfo::from_codes(&m.expanded_codes));
        }
        let mut save = map::CampaignSave::after_landing();
        for n in [2u8, 3, 4, 5, 6, 7, 8, 9, 10] {
            save.node_states[usize::from(n)] = map::STATE_BEATEN;
        }
        save.location = 7;
        save.event = map::EVENT_BOSWORTH_FAREWELL;
        let mut m = MapModel::resume(map_data, save).unwrap();
        m.step(map::Pad::NONE);
        let deck = DeckHud { index: 0, cost: 822, leader: 0, leader_rank: 1 };
        let (calls, notes) = calls(&m, &data.executable, &deck, &Default::default());
        assert!(notes.is_empty(), "{notes:?}");
        let story: Vec<&Call> = calls.iter().filter(|c| c.key() >= KEY_LETTERBOX).collect();
        let fills: Vec<([[i32; 2]; 4], u8)> = story
            .iter()
            .filter_map(|c| match c {
                Call::Fill { xy, rgba, .. } => Some((*xy, rgba[0][3])),
                _ => None,
            })
            .collect();
        assert_eq!(
            fills,
            [
                ([[0, 0], [512, 0], [0, 144], [512, 144]], 128),
                ([[0, 368], [512, 368], [0, 512], [512, 512]], 128),
                ([[138, 258], [374, 258], [138, 288], [374, 288]], 64),
            ]
        );
        let quads: Vec<([i32; 4], [i32; 2], [i32; 2])> = story
            .iter()
            .filter_map(|c| match c {
                Call::Quad { xy, uv, size, .. } => Some((*xy, *uv, *size)),
                _ => None,
            })
            .collect();
        assert_eq!(
            quads,
            [
                ([136, 256, 152, 270], [64, 0], [16, 14]),
                ([360, 256, 376, 270], [80, 0], [16, 14]),
                ([136, 276, 152, 290], [64, 14], [16, 14]),
                ([360, 276, 376, 290], [80, 14], [16, 14]),
                ([152, 256, 360, 270], [80, 0], [1, 14]),
                ([152, 276, 360, 290], [80, 14], [1, 14]),
                ([136, 270, 152, 276], [64, 14], [16, 1]),
                ([360, 270, 376, 276], [80, 14], [16, 1]),
            ]
        );
        assert!(story.iter().any(|c| matches!(c, Call::TextRow { message: 2828, row: 0, x: 142, y: 262, .. })));
        while m.text_phase() != 4 {
            m.step(map::Pad::NONE);
        }
        let (waiting, _) = super::calls(&m, &data.executable, &deck, &Default::default());
        assert!(waiting.iter().any(|c| matches!(
            c,
            Call::Quad { xy: [472, 472, 488, 488], uv: [0, 112], size: [16, 16], slot: 1, .. }
        )));
    }

    /// The leader box 0x21D5DC against `trace-win3.json` (Richard beaten:
    /// the record panel opens with the return text, the box slides in with
    /// the countdown −64 per update, then moves up 25 per update from
    /// 11140's C2 update until it is gone) [emu]: the sequence of its
    /// distinct (dx, dy) offsets, the corners and colour.
    #[test]
    #[ignore = "requires ROSES_MAP_MANIFEST, ROSES_DUEL_IMPORT and ROSES_STORY_RED_TRACE (trace-win3.json)"]
    fn private_leader_box_slides_as_traced() {
        use std::path::Path;
        let manifest = std::env::var_os("ROSES_MAP_MANIFEST").expect("ROSES_MAP_MANIFEST");
        let duel = std::env::var_os("ROSES_DUEL_IMPORT").expect("ROSES_DUEL_IMPORT");
        let oracle = std::env::var_os("ROSES_STORY_RED_TRACE").expect("ROSES_STORY_RED_TRACE");
        let trace: serde_json::Value =
            serde_json::from_slice(&std::fs::read(Path::new(&oracle)).unwrap()).unwrap();
        let i = |v: &serde_json::Value| v.as_i64().unwrap() as i32;
        let mut want: Vec<Option<(i32, i32)>> = Vec::new();
        let mut want_desc = None;
        for r in trace["rows"].as_array().unwrap().iter().take_while(|r| r["frame"].as_u64().unwrap() <= 70) {
            let c = r["calls"].as_array().unwrap().iter().find(|c| c["ra"] == "0x21d5dc");
            let off = c.map(|c| (i(&c["off"][0]), i(&c["off"][1])));
            if let Some(c) = c {
                want_desc = Some((c["desc"].clone(), c["rgba"].clone()));
            }
            if want.last() != Some(&off) {
                want.push(off);
            }
        }
        let exe = crate::duel_play::load_executable(Path::new(&duel)).unwrap();
        let loaded = crate::campaign_map::load_map(Path::new(&manifest)).unwrap();
        let data = loaded.data.clone();
        let mut save = map::CampaignSave::after_landing();
        for n in [2u8, 3, 4, 5, 6, 8, 9, 10] {
            save.node_states[usize::from(n)] = map::STATE_BEATEN;
        }
        save.node_states[7] = 1;
        save.hovered = 7;
        save.destination = 7;
        save.location = 7;
        let variant = data.text_variant(7, save.state(7), 0).unwrap();
        save.opponent = i16::from(variant.opponent.unwrap());
        save.apply_duel_end(&data, true);
        let mut m = MapModel::resume(data, save).unwrap();
        let deck = DeckHud { index: 0, cost: 822, leader: 0, leader_rank: 1 };
        let mut got: Vec<Option<(i32, i32)>> = Vec::new();
        let mut last_press = 0;
        let mut after_story = 0;
        while after_story < 20 && m.frame < 2000 {
            let pad = if m.text_phase() == 4 && m.frame - last_press > 3 {
                last_press = m.frame + 1;
                map::Pad::CROSS
            } else {
                map::Pad::NONE
            };
            m.step(pad);
            if matches!(m.handler, map::Handler::Story { .. }) {
                after_story += 1;
            }
            let (calls, _) = calls(&m, &exe, &deck, &Default::default());
            let boxes: Vec<&Call> = calls.iter().filter(|c| c.key() == KEY_LEADER_BOX).collect();
            assert!(boxes.len() <= 1);
            let off = boxes.first().map(|c| match c {
                Call::Fill { xy, rgba, .. } => {
                    let (desc, colour) = want_desc.as_ref().unwrap();
                    let (dx, dy) = (xy[0][0] - i(&desc[0]), xy[0][1] - i(&desc[1]));
                    let corners: Vec<i32> = xy.iter().flat_map(|p| [p[0] - dx, p[1] - dy]).collect();
                    let want_corners: Vec<i32> = desc.as_array().unwrap().iter().map(i).collect();
                    assert_eq!(corners, want_corners);
                    let want_rgba: Vec<u8> = colour.as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u8).collect();
                    assert_eq!(rgba.concat(), want_rgba);
                    (dx, dy)
                }
                _ => unreachable!(),
            });
            if got.last() != Some(&off) {
                got.push(off);
            }
        }
        assert_eq!(got, want);
    }

    /// "Exceeded DC Level!" (MSG 250, 18 glyphs) while Cross is held over
    /// Chester with a deck over the opponent's DC: the tooltip window of
    /// record 0x296F52 (style 0, colour 2) 232 wide centred on x 256 at y
    /// 288 — its fill (142, 290)-(370, 318) black at alpha 64, as the
    /// traced place tooltip's (0x2362A8: (202, 256)-(310, 284) for a 112
    /// px window at (200, 254)) — and the text at (148, 292) [F 0x2362B0..
    /// 0x2363D0].
    #[test]
    #[ignore = "requires ROSES_MAP_MANIFEST and ROSES_DUEL_IMPORT"]
    fn private_over_dc_draws_msg_250_in_the_tooltip_window() {
        use std::path::Path;
        let manifest = std::env::var_os("ROSES_MAP_MANIFEST").expect("ROSES_MAP_MANIFEST");
        let duel = std::env::var_os("ROSES_DUEL_IMPORT").expect("ROSES_DUEL_IMPORT");
        let exe = crate::duel_play::load_executable(Path::new(&duel)).unwrap();
        let loaded = crate::campaign_map::load_map(Path::new(&manifest)).unwrap();
        let mut m = MapModel::at_event_zero(loaded.data.clone()).unwrap();
        m.player_dc = Some(900);
        m.opponent_costs.insert(2, 854);
        let mut f = 0;
        while !(m.hovered == Some(2) && m.hud.panel >= 256) {
            m.step(if m.hovered == Some(2) { map::Pad::NONE } else { map::Pad::UP });
            f += 1;
            assert!(f < 80, "Chester is not hovered");
        }
        m.step(map::Pad::CROSS);
        assert!(m.over_dc);
        let deck = DeckHud { index: 0, cost: 900, leader: 0, leader_rank: 1 };
        let (calls, _) = calls(&m, &exe, &deck, &Default::default());
        let tip: Vec<&Call> = calls.iter().filter(|c| c.key() == KEY_TOOLTIP).collect();
        assert!(tip.iter().any(|c| matches!(c, Call::Text { message: 250, x: 148, y: 292, .. })), "{tip:?}");
        assert!(!tip.iter().any(|c| matches!(c, Call::Text { message, .. } if *message != 250)));
        let fills: Vec<([[i32; 2]; 4], [u8; 4])> = tip
            .iter()
            .filter_map(|c| match c {
                Call::Fill { xy, rgba, .. } => Some((*xy, rgba[0])),
                _ => None,
            })
            .collect();
        assert!(!fills.is_empty());
        assert_eq!(fills[0].0[0], [142, 290]);
        assert_eq!(fills.last().unwrap().0[3], [370, 318]);
        assert!(fills.iter().all(|(_, c)| c[3] == 64));
        // The frame pieces span the window (140, 288)-(372, 320).
        let quads: Vec<[i32; 4]> = tip
            .iter()
            .filter_map(|c| match c {
                Call::Quad { xy, .. } => Some(*xy),
                _ => None,
            })
            .collect();
        assert_eq!(quads.iter().map(|q| q[0]).min(), Some(140));
        assert_eq!(quads.iter().map(|q| q[2]).max(), Some(372));
        assert_eq!(quads.iter().map(|q| q[1]).min(), Some(288));
        assert_eq!(quads.iter().map(|q| q[3]).max(), Some(320));
        m.step(map::Pad::NONE);
        let (after, _) = super::calls(&m, &exe, &deck, &Default::default());
        assert!(!after.iter().any(|c| matches!(c, Call::Text { message: 250, .. })));
    }

    #[test]
    #[ignore = "requires ROSES_MAP_MANIFEST and ROSES_DUEL_IMPORT"]
    fn private_text_box_is_drawn_while_a_page_is_open() {
        use std::path::Path;
        let manifest = std::env::var_os("ROSES_MAP_MANIFEST").expect("ROSES_MAP_MANIFEST");
        let duel = std::env::var_os("ROSES_DUEL_IMPORT").expect("ROSES_DUEL_IMPORT");
        let exe = crate::duel_play::load_executable(Path::new(&duel)).unwrap();
        let loaded = crate::campaign_map::load_map(Path::new(&manifest)).unwrap();
        let mut m = MapModel::at_event_zero(loaded.data.clone()).unwrap();
        for f in 0..60 {
            m.step(match f {
                0..=4 => map::Pad::UP,
                30 => map::Pad::CROSS,
                _ => map::Pad::NONE,
            });
        }
        assert!(m.text.is_some(), "the taunt is open");
        let deck = DeckHud { index: 0, cost: 822, leader: 0, leader_rank: 1 };
        let (calls, notes) = calls(&m, &exe, &deck, &Default::default());
        let fills: Vec<_> = calls
            .iter()
            .filter_map(|c| match c {
                Call::Fill { xy, rgba, .. } => Some((xy[0], xy[3], rgba[0])),
                _ => None,
            })
            .collect();
        assert!(
            fills.contains(&([16, 48], [480, 128], [0, 0, 0, 64])),
            "{fills:?} {notes:?} phase {:?}",
            m.text.as_ref().map(|t| (t.phase, t.openness))
        );
        // Rendered: half-transparent black inside the box, clear outside.
        let data = crate::duel_play::load(Path::new(&duel)).unwrap();
        let mut codes = std::collections::BTreeMap::new();
        for chunk in HudTables::required_messages().chunks(256) {
            for m in roses_assets::parse_pal_messages(&data.messages, chunk).unwrap().messages {
                codes.insert(m.message_id, m.expanded_codes);
            }
        }
        let tables = HudTables::from_executable(&exe, codes).unwrap();
        let mut textures = roses_client::duel_hud::HudTextures::new(&data.texsys, &data.picmini, [0, 0], &data.messages).unwrap();
        let mut notes = Vec::new();
        let prims = primitives(&calls, &tables, &mut notes);
        let (rgba, _) = roses_client::duel_hud::render_primitives_layer(&prims, &tables, &mut textures, 512, 512);
        let alpha = |x: usize, y: usize| rgba[(y * 512 + x) * 4 + 3];
        assert!(alpha(470, 120) > 100 && alpha(470, 120) < 160, "box alpha {}", alpha(470, 120));
        assert_eq!(alpha(470, 140), 0);
    }

    /// SYSTEM icon 3's lesson menu against the emulator's log of the
    /// original with the menu open on row 3 (`modes/tutorial45/menu-hud-
    /// calls.json`, the state saved by `run_lesson.py`): the windows'
    /// rectangles, styles, colours and alphas, the "HELP" plate and the five
    /// rows; `ROSES_MENU_PNG` (optional) receives the rendered layer.
    #[test]
    #[ignore = "requires ROSES_MAP_MANIFEST, ROSES_DUEL_IMPORT and ROSES_MENU_ORACLE"]
    fn private_tutorial_menu_matches_the_emulated_frame() {
        use std::path::Path;
        let manifest = std::env::var_os("ROSES_MAP_MANIFEST").expect("ROSES_MAP_MANIFEST");
        let duel = std::env::var_os("ROSES_DUEL_IMPORT").expect("ROSES_DUEL_IMPORT");
        let oracle = std::env::var_os("ROSES_MENU_ORACLE").expect("ROSES_MENU_ORACLE");
        let exe = crate::duel_play::load_executable(Path::new(&duel)).unwrap();
        let loaded = crate::campaign_map::load_map(Path::new(&manifest)).unwrap();
        let mut m = MapModel::at_event_zero(loaded.data.clone()).unwrap();
        m.tutorial_menu = Some(map::TutorialMenu { state: 2, slide: 8, row: 3 });
        let deck = DeckHud { index: 0, cost: 822, leader: 0, leader_rank: 1 };
        let (calls, _) = calls(&m, &exe, &deck, &Default::default());
        let log: serde_json::Value =
            serde_json::from_slice(&std::fs::read(oracle).unwrap()).unwrap();
        let i = |v: &serde_json::Value| v.as_i64().unwrap() as i32;
        // The menu's emitter calls (return addresses inside 0x23B770).
        let menu: Vec<&serde_json::Value> = log["calls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| i(&c["frame"]) == 1 && c["ra"].as_str().unwrap().starts_with("0x23b"))
            .collect();
        // Windows: (x0 + dx, y0 + dy) of the record and its fills.
        let fills: Vec<[i32; 4]> = calls
            .iter()
            .filter_map(|c| match c {
                Call::Fill { xy, .. } => Some([xy[0][0], xy[0][1], xy[3][0], xy[3][1]]),
                _ => None,
            })
            .collect();
        let want_fills = [[38, 86, 474, 208], [38, 208, 474, 330], [217, 96, 295, 112], [217, 112, 295, 128], [50, 251, 462, 275]];
        for f in want_fills {
            assert!(fills.contains(&f), "fill {f:?} missing from {fills:?}");
        }
        let texts: Vec<(i32, i32)> = menu
            .iter()
            .filter(|c| c["fn"] == "text")
            .map(|c| (i(&c["args"][2]), i(&c["args"][3])))
            .collect();
        let ours: Vec<(i32, i32)> = calls
            .iter()
            .filter_map(|c| match c {
                Call::TintedText { x, y, .. } => Some((*x, *y)),
                _ => None,
            })
            .collect();
        assert_eq!(ours, texts);
        let font = menu.iter().find(|c| c["fn"] == "font").unwrap();
        assert!(calls.iter().any(|c| matches!(c, Call::Font { x, y, text, .. }
            if *x == i(&font["args"][1]) && *y == i(&font["args"][2]) && text.as_slice() == b"HELP")));
        if let Some(path) = std::env::var_os("ROSES_MENU_PNG") {
            let data = crate::duel_play::load(Path::new(&duel)).unwrap();
            let mut ids = HudTables::required_messages();
            ids.extend(map::TUTORIAL_LABEL_BASE..map::TUTORIAL_LABEL_BASE + 5);
            let mut codes = std::collections::BTreeMap::new();
            for chunk in ids.chunks(256) {
                for m in roses_assets::parse_pal_messages(&data.messages, chunk).unwrap().messages {
                    codes.insert(m.message_id, m.expanded_codes);
                }
            }
            let tables = HudTables::from_executable(&exe, codes).unwrap();
            let mut textures = roses_client::duel_hud::HudTextures::new(&data.texsys, &data.picmini, [0, 0], &data.messages).unwrap();
            let mut notes = Vec::new();
            let prims = primitives(&calls, &tables, &mut notes);
            let (rgba, _) = roses_client::duel_hud::render_primitives_layer(&prims, &tables, &mut textures, 512, 512);
            let image = bevy::image::Image::new(
                bevy::render::render_resource::Extent3d { width: 512, height: 512, depth_or_array_layers: 1 },
                bevy::render::render_resource::TextureDimension::D2,
                rgba,
                bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                bevy::asset::RenderAssetUsages::MAIN_WORLD,
            );
            image.try_into_dynamic().unwrap().save(Path::new(&path)).unwrap();
        }
    }

    /// The builder against the emulator's log of 622 + 47 frames (Chester
    /// hovered, HUD in place). Requires the private map import and log.
    #[test]
    #[ignore = "requires ROSES_MAP_MANIFEST, ROSES_DUEL_IMPORT and the private HUD log"]
    fn private_calls_match_the_emulated_frame() {
        use std::path::Path;
        let manifest = std::env::var_os("ROSES_MAP_MANIFEST").expect("ROSES_MAP_MANIFEST");
        let duel = std::env::var_os("ROSES_DUEL_IMPORT").expect("ROSES_DUEL_IMPORT");
        let exe = crate::duel_play::load_executable(Path::new(&duel)).unwrap();
        let loaded = crate::campaign_map::load_map(Path::new(&manifest)).unwrap();
        let mut m = MapModel::at_event_zero(loaded.data.clone()).unwrap();
        for f in 0..47 {
            m.step(if f < 5 { map::Pad::UP } else { map::Pad::NONE });
        }
        m.opponent_dc = Some(854);
        let deck = DeckHud {
            index: 0,
            cost: 822,
            leader: 0,
            leader_rank: 1,
        };
        let (calls, _) = calls(&m, &exe, &deck, &Default::default());
        let log: serde_json::Value = serde_json::from_slice(
            &std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(
                "../../work/research/story/claude-campaign-map/logs/hud-oracle-622-f46.json",
            ))
            .unwrap(),
        )
        .unwrap();
        // The emulated quads of frame 47 (0x217500 calls) in their order.
        let arg = |c: &serde_json::Value, i: usize| c["args"][i].as_i64().unwrap() as i32;
        let expected: Vec<([i32; 4], [i32; 2], [i32; 2])> = log["calls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["frame"] == 47 && c["fn"] == "quad")
            .map(|c| {
                (
                    [arg(c, 1), arg(c, 2), arg(c, 3), arg(c, 4)],
                    [arg(c, 5), arg(c, 6)],
                    [arg(c, 7), c["stack"][0].as_i64().unwrap() as i32],
                )
            })
            .collect();
        let mut got: Vec<_> = calls
            .iter()
            .filter_map(|c| match c {
                Call::Quad { xy, uv, size, .. } => Some((*xy, *uv, *size)),
                _ => None,
            })
            .collect();
        // The original's ordering table keeps insertion order per key; the
        // comparison is on the set of quads.
        let mut want = expected.clone();
        got.sort();
        want.sort();
        assert_eq!(want.len(), 79);
        assert_eq!(got, want);
        let frame = |f: &str| -> Vec<serde_json::Value> {
            log["calls"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["frame"] == 47 && c["fn"] == f)
                .cloned()
                .collect()
        };
        // Fills: corners and the first two vertex colours; the full-screen
        // letterbox fills (alpha 0, 0x2182F0 from 0x2181C0's caller) are
        // the map's letterbox, drawn elsewhere.
        let st = |c: &serde_json::Value, i: usize| c["stack"][i].as_i64().unwrap() as u8;
        let mut want_fills: Vec<_> = frame("fill")
            .iter()
            .filter(|c| arg(c, 3) - arg(c, 1) < 512)
            .map(|c| {
                (
                    [arg(c, 1), arg(c, 2), arg(c, 3), arg(c, 4), arg(c, 5), arg(c, 6), arg(c, 7)],
                    [st(c, 1), st(c, 2), st(c, 3), st(c, 4)],
                    [st(c, 5), st(c, 6), st(c, 7), st(c, 8)],
                )
            })
            .collect();
        let mut got_fills: Vec<_> = calls
            .iter()
            .filter_map(|c| match c {
                Call::Fill { xy, rgba, .. } => Some((
                    [xy[0][0], xy[0][1], xy[1][0], xy[1][1], xy[2][0], xy[2][1], xy[3][0]],
                    rgba[0],
                    rgba[1],
                )),
                _ => None,
            })
            .collect();
        want_fills.sort();
        got_fills.sort();
        assert_eq!(got_fills, want_fills);
        // Gold-font strings with position, size and flags.
        let mut want_fonts: Vec<_> = frame("font")
            .iter()
            .map(|c| {
                (
                    arg(c, 1),
                    arg(c, 2),
                    arg(c, 3) as usize,
                    arg(c, 4) as u32,
                    c["string"].as_str().unwrap().as_bytes().to_vec(),
                )
            })
            .collect();
        let mut got_fonts: Vec<_> = calls
            .iter()
            .filter_map(|c| match c {
                Call::Font {
                    x, y, size, flags, text, ..
                } => Some((*x, *y, *size, *flags, text.clone())),
                _ => None,
            })
            .collect();
        want_fonts.sort();
        got_fonts.sort();
        assert_eq!(got_fonts, want_fonts);
        // Text lines at their positions.
        let mut want_text: Vec<_> = frame("text").iter().map(|c| (arg(c, 2), arg(c, 3))).collect();
        let mut got_text: Vec<_> = calls
            .iter()
            .filter_map(|c| match c {
                Call::Text { x, y, .. } => Some((*x, *y)),
                _ => None,
            })
            .collect();
        want_text.sort();
        got_text.sort();
        assert_eq!(got_text, want_text);
    }

    /// Mako's bottom panel over the map (12040) against trace-dover
    /// (`story/the project notes, ROSES_MAKO_MAP_ORACLE)
    /// at f20 (sliding in), f40 (full, page box opening) and f62 (the page
    /// waits): every panel quad (emblem, name window and face window
    /// pieces, advance mark), the window fills, the wing/ornament quads
    /// (descriptor + offset), the name origin and the page rows [emu].
    #[test]
    #[ignore = "requires ROSES_MAP_MANIFEST, ROSES_DUEL_IMPORT and ROSES_MAKO_MAP_ORACLE"]
    fn private_mako_panel_matches_trace_dover() {
        use std::path::Path;
        let manifest = std::env::var_os("ROSES_MAP_MANIFEST").expect("ROSES_MAP_MANIFEST");
        let duel = std::env::var_os("ROSES_DUEL_IMPORT").expect("ROSES_DUEL_IMPORT");
        let oracle = std::env::var_os("ROSES_MAKO_MAP_ORACLE").expect("ROSES_MAKO_MAP_ORACLE");
        let trace: serde_json::Value =
            serde_json::from_slice(&std::fs::read(Path::new(&oracle)).unwrap()).unwrap();
        let rows = trace["rows"].as_array().unwrap();
        let row = |f: u64| rows.iter().find(|r| r["frame"].as_u64() == Some(f)).unwrap();
        let data = crate::duel_play::load_with_end(Path::new(&duel), None).unwrap();
        let loaded = crate::campaign_map::load_map(Path::new(&manifest)).unwrap();
        let mut map_data = loaded.data.clone();
        let parsed = roses_assets::parse_pal_messages(&data.messages, &map::map_story_message_ids()).unwrap();
        for m in parsed.messages {
            map_data
                .messages
                .entry(m.message_id)
                .or_insert_with(|| map::MessageInfo::from_codes(&m.expanded_codes));
        }
        let mut save = map::CampaignSave::white_start();
        for n in [11u8, 12, 13, 14] {
            save.node_states[usize::from(n)] = map::STATE_BEATEN;
        }
        save.node_states[15] = 1;
        save.location = 15;
        save.hovered = 15;
        save.event = map::EVENT_DOVER_STORY;
        let mut m = MapModel::resume(map_data, save).unwrap();
        let deck = DeckHud { index: 0, cost: 822, leader: 0, leader_rank: 1 };
        let i = |v: &serde_json::Value| v.as_i64().unwrap() as i32;
        // Update n = f(n - 1) (the resumed sub-step 0 is the entry frame f0).
        let mut frame = 0u64;
        for f in [20u64, 40, 62] {
            while frame < f + 1 {
                m.step(map::Pad::NONE);
                frame += 1;
            }
            let (calls, notes) = calls(&m, &data.executable, &deck, &Default::default());
            assert!(notes.iter().all(|n| !n.0.contains("dialogue")), "{notes:?}");
            let ours: Vec<&Call> = calls.iter().filter(|c| c.key() == KEY_STORY_WINDOW).collect();
            let r = row(f);
            let calls_of = |name: &str| -> Vec<serde_json::Value> {
                r["calls"].as_array().unwrap().iter().filter(|c| c["fn"] == name).cloned().collect()
            };
            // Quads drawn by the panel (0x2332B0..: emblem, window pieces)
            // and the advance mark; the HUD's own are off screen (y > 512).
            let mut want_quads: Vec<([i32; 4], [i32; 2], [i32; 2])> = calls_of("quad")
                .iter()
                .filter(|c| {
                    let d = &c["dest"];
                    let ra = c["ra"].as_str().unwrap();
                    i(&d[1]) >= 352 && i(&d[1]) < 512 && (ra == "0x21742c" || ra == "0x219980")
                })
                .map(|c| {
                    let (d, u) = (&c["dest"], &c["uv"]);
                    ([i(&d[0]), i(&d[1]), i(&d[2]), i(&d[3])], [i(&u[0]), i(&u[1])], [i(&u[2]), i(&u[3])])
                })
                .collect();
            let mut got_quads: Vec<([i32; 4], [i32; 2], [i32; 2])> = ours
                .iter()
                .filter_map(|c| match c {
                    Call::Quad { xy, uv, size, .. } if xy[1] < 512 && xy[3] > 0 => Some((*xy, *uv, *size)),
                    _ => None,
                })
                .collect();
            want_quads.sort();
            got_quads.sort();
            assert_eq!(got_quads, want_quads, "f{f} quads");
            // Fills inside the panel's windows (0x219524 in 0x218E70).
            let mut want_fills: Vec<[i32; 4]> = calls_of("fill")
                .iter()
                .filter(|c| c.get("inwin").is_some() && i(&c["args"][1]) >= 352 && i(&c["args"][1]) < 512)
                .map(|c| {
                    let a = &c["args"];
                    [i(&a[0]), i(&a[1]), i(&a[2]), i(&c["stack"][0])]
                })
                .collect();
            let mut got_fills: Vec<[i32; 4]> = ours
                .iter()
                .filter_map(|c| match c {
                    Call::Fill { xy, .. } if xy[0][1] < 512 => Some([xy[0][0], xy[0][1], xy[1][0], xy[2][1]]),
                    _ => None,
                })
                .collect();
            want_fills.sort();
            got_fills.sort();
            assert_eq!(got_fills, want_fills, "f{f} fills");
            // Wings and ornament: descriptor corners + the traced offsets.
            let want_q4: Vec<[[i32; 2]; 4]> = calls_of("quad4")
                .iter()
                .filter(|c| ["0x23352c", "0x23354c", "0x233574"].contains(&c["ra"].as_str().unwrap()))
                .map(|c| {
                    let a = &c["args"];
                    let desc = read(&data.executable, a[0].as_u64().unwrap() as u32, 16).unwrap();
                    let h = |o: usize| i32::from(i16::from_le_bytes([desc[o], desc[o + 1]]));
                    [0, 1, 2, 3].map(|k| [h(4 * k) + i(&a[1]), h(4 * k + 2) + i(&a[2])])
                })
                .collect();
            let got_q4: Vec<[[i32; 2]; 4]> = ours
                .iter()
                .filter_map(|c| match c {
                    Call::Quad4 { xy, .. } => Some(*xy),
                    _ => None,
                })
                .collect();
            assert_eq!(got_q4, want_q4, "f{f} wings/ornament");
            // The name (W+53BE) origin.
            let name = calls_of("text").into_iter().find(|c| c["ra"] == "0x2335e0").unwrap();
            assert!(
                ours.iter().any(|c| matches!(c, Call::Text { message: 211, x, y, .. }
                    if *x == i(&name["args"][1]) && *y == i(&name["args"][2]))),
                "f{f} name"
            );
            // The page rows once revealed (f62: all three at full alpha).
            if f == 62 {
                let want_rows: Vec<(i32, i32)> = calls_of("text2")
                    .iter()
                    .map(|c| (i(&c["args"][1]), i(&c["args"][2])))
                    .collect();
                let got_rows: Vec<(i32, i32)> = ours
                    .iter()
                    .filter_map(|c| match c {
                        Call::TextRow { message: 3140, x, y, alpha: [128, 128, 128, 128], .. } => Some((*x, *y)),
                        _ => None,
                    })
                    .collect();
                assert_eq!(got_rows, want_rows, "f62 rows");
            }
        }
    }
}
