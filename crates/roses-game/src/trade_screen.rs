//! Trade's screen (mode 8 draw 0x1C44B0 → 0x1C44E0) as the original's
//! HUD-leaf calls (`duel_hud::MenuCall`), Bevy-free; the scene is
//! `trade_render.rs` in the binary (Claude, 6 Oct 2026).
//!
//! Provenance (`work/research/modes/two-player/NOTES.md` §3,
//! `d_trade_1c3050.txt`): 0x1C47F0 background, 0x1C4C50 title, 0x1C4D20
//! side panel, 0x1C5950 card row, 0x1C63A0 cursor and hint bar, 0x1C66C0
//! READY, 0x1C67D0 / 0x1C6980 / 0x1C6B30 the question and refusal boxes
//! were read [F] and run in the EE interpreter (`tools/run_trade.py`);
//! the private test compares this builder with the logged leaf calls
//! [emu]. The card detail pane (DTL, 0x1BE6E0) is `card_detail`'s, with its
//! own oracle. The current sort key's icon carries 0x1B48A0's overlay (mode
//! 1 grey, mode 2 in 0x2BDC24's colour while SELECT is held; alpha =
//! 0x2DB828 >> 8) and the refusal box (0x1B1830 with the press flag) the
//! press icon; the boxes of 0x1B1830 are compared with the interpreter's
//! calls too [emu]. Nothing here is a game byte: strings and colours are
//! read from the user's executable.
use roses_client::duel_hud::{self, HudTables, MenuCall, YesNoTables};
use roses_client::trade::{self, Trade};
use roses_core::card_block::{CardBlock, EMPTY_CARD};
use roses_core::deck_editor::StaticCards;

/// The executable's strings and colours the screen uses.
#[derive(Clone, Debug)]
pub struct TradeData {
    pub trade: Vec<u8>,
    pub players: [Vec<u8>; 2],
    pub total: Vec<u8>,
    /// 0x290710: CHEST, EXPORTS, IMPORTS.
    pub views: [Vec<u8>; 3],
    /// 0x290250: the ten sort keys' labels.
    pub sort_labels: Vec<Vec<u8>>,
    /// IM / EX (the row's count tag in the import / other views).
    pub import_tag: Vec<u8>,
    pub export_tag: Vec<u8>,
    pub ex_set: Vec<u8>,
    pub detail: Vec<u8>,
    pub exit: Vec<u8>,
    pub ready: Vec<u8>,
    /// The refusal box's title (0x2BE3B8).
    pub error: Vec<u8>,
    pub yes: Vec<u8>,
    pub no: Vec<u8>,
    /// 0x2BDBD8: background edge and centre colours (B, G, R, A ×2).
    pub background: [u8; 8],
    /// 0x2BDCC0 / C4 / C8: the name's three layers (B, G, R, A).
    pub tints: [[u8; 4]; 3],
    /// 0x2BDC24: the sort label's colour while SELECT is held.
    pub select_tint: [u8; 4],
    /// The detail pane's strings and tables.
    pub pane: crate::card_detail::DetailData,
}

const EE_TO_FILE: u32 = 0xF_9680;

fn word(exe: &[u8], ee: u32) -> Result<u32, String> {
    let at = (ee - EE_TO_FILE) as usize;
    exe.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or_else(|| format!("{ee:#x} is outside the executable"))
}

impl TradeData {
    pub fn from_executable(exe: &[u8]) -> Result<Self, String> {
        let s = |a: u32| duel_hud::elf_string(exe, a).map_err(|e| e.to_string());
        let ptr = |table: u32, k: u32| -> Result<Vec<u8>, String> { s(word(exe, table + 4 * k)?) };
        let bytes = |a: u32, n: usize| duel_hud::elf_slice(exe, a, n).map_err(|e| e.to_string()).map(|b| b.to_vec());
        let tint = |a: u32| -> Result<[u8; 4], String> {
            let c = bytes(a, 4)?;
            Ok([c[2], c[1], c[0], c[3]])
        };
        let yn = YesNoTables::from_executable(exe).map_err(|e| e.to_string())?;
        Ok(Self {
            trade: s(0x2B_E383)?,
            players: [s(0x2B_E39C)?, s(0x2B_E399)?],
            total: s(0x2B_E39F)?,
            views: [ptr(0x29_0710, 0)?, ptr(0x29_0710, 1)?, ptr(0x29_0710, 2)?],
            sort_labels: (0..10).map(|k| ptr(0x29_0250, k)).collect::<Result<_, _>>()?,
            import_tag: s(0x2B_E3A5)?,
            export_tag: s(0x2B_E3A8)?,
            ex_set: s(0x2B_E3AB)?,
            detail: s(0x2B_E36B)?,
            exit: s(0x2B_E318)?,
            ready: s(0x2B_E3B2)?,
            error: s(0x2B_E3B8)?,
            yes: yn.yes,
            no: yn.no,
            background: bytes(0x2B_DBD8, 8)?.try_into().unwrap(),
            tints: [tint(0x2B_DCC0)?, tint(0x2B_DCC4)?, tint(0x2B_DCC8)?],
            select_tint: tint(0x2B_DC24)?,
            pane: crate::card_detail::DetailData::from_executable(exe)?,
        })
    }
}

/// What a frame draws.
pub struct Screen {
    pub calls: Vec<MenuCall>,
    /// Call ranges that stand for one 0x1B1830 box (the test skips them).
    pub boxes: Vec<std::ops::Range<usize>>,
    /// Call ranges of the detail panes (`card_detail`, checked by its own
    /// oracle), with the side whose card picture a `Picture` call binds.
    pub panes: Vec<(std::ops::Range<usize>, usize)>,
    pub notes: Vec<String>,
}

pub struct Inputs<'a> {
    pub trade: &'a Trade,
    pub blocks: &'a [CardBlock; 2],
    pub cards: &'a StaticCards,
    /// SELECT held on each pad (0x1C4D20 highlights the second sort key).
    pub select_held: [bool; 2],
    /// Each side's detail card picture is resident (0x200270(1, picture)).
    pub pictures: [bool; 2],
    /// 0x2DA954: the update counter (the press icon's cel).
    pub counter: u32,
}

struct B<'a> {
    d: &'a TradeData,
    t: &'a HudTables,
    calls: Vec<MenuCall>,
    boxes: Vec<std::ops::Range<usize>>,
    panes: Vec<(std::ops::Range<usize>, usize)>,
    notes: Vec<String>,
}

impl B<'_> {
    fn c(&mut self, call: MenuCall) {
        self.calls.push(call);
    }
    fn panel(&mut self, grad: i32, deco: i32, ptype: usize, x: i32, y: i32, wu: i32, hu: i32, bright: bool) {
        self.c(MenuCall::Panel { grad, deco, ptype, x, y, wu, hu, bright });
    }
    fn sprite(&mut self, desc: usize, x: i32, y: i32) {
        self.c(MenuCall::Sprite { desc, x, y });
    }
    fn gold(&mut self, x: i32, y: i32, size: usize, flags: u32, text: &[u8], dim: bool) {
        self.c(MenuCall::Gold { x, y, size, flags, text: text.to_vec(), dim });
    }
    fn number(&mut self, x: i32, y: i32, size: usize, flags: u32, digits: i32, value: i32) {
        self.c(MenuCall::Number { x, y, size, flags, digits, value, dim: false });
    }
    fn boxed(&mut self, colour: i32, x: i32, y: i32, wu: i32, hu: i32) {
        self.c(MenuCall::Box { colour, x, y, wu, hu });
    }
    fn width(&mut self, message: u16) -> Option<(i32, i32)> {
        match self.t.message(message) {
            Some(codes) => {
                let (lines, w) = duel_hud::TextCanvas::measure(codes);
                Some((w as i32, lines as i32))
            }
            None => {
                self.notes.push(format!("message {message} was not imported"));
                None
            }
        }
    }
}

/// The calls of one frame (0x1C44E0).
pub fn build(d: &TradeData, t: &HudTables, inp: &Inputs<'_>) -> Screen {
    let mut b = B { d, t, calls: Vec::new(), boxes: Vec::new(), panes: Vec::new(), notes: Vec::new() };
    let tr = inp.trade;
    background(&mut b);
    title(&mut b, 0, 0);
    for side in 0..2usize {
        side_panel(&mut b, inp, side, 0, 0);
        let s = &tr.sides[side];
        if s.count != 0 {
            cursor(&mut b, inp, side, 0, 0);
        }
        if s.slide != 0 {
            let alpha = (((i32::from(s.slide) << 6) >> 8) & 0xFF) as u8;
            b.c(MenuCall::DarkenRect { x: 0x100 * side as i32, y: 0, w: 0x100, h: 0x200, alpha });
            // 0x1C458C / 0x1C4698: D5C = side, then 0x1BE6E0(card, ±slide,
            // 0x20, side, 1, 1, 0).
            let pane = crate::card_detail::Pane {
                card: s.card(),
                x: if side == 0 { i32::from(s.slide) } else { -i32::from(s.slide) },
                y: 0x20,
                side: side as u8,
                page: s.page,
                raised_tab: true,
                tinted: true,
                editor_state: None,
                block: &inp.blocks[side],
                cards: inp.cards,
                picture: inp.pictures[side],
                blink: (tr.scroll[1] >> 8) as u8,
                rank: None,
            };
            let (calls, notes) = crate::card_detail::build(&b.d.pane, b.t, &pane);
            let start = b.calls.len();
            b.calls.extend(calls);
            b.panes.push((start..b.calls.len(), side));
            b.notes.extend(notes);
        }
        if matches!(s.state, trade::state::READY | trade::state::TRADE_ASK | trade::state::SAVING) {
            b.c(MenuCall::DarkenRect { x: 0x100 * side as i32, y: 0, w: 0x100, h: 0x200, alpha: 0x40 });
            ready_box(&mut b, if side == 0 { -0x80 } else { 0x80 }, 0);
        }
    }
    let either = |st: u8| tr.sides.iter().any(|s| s.state == st);
    if either(trade::state::CANCEL_ASK) {
        let title = b.d.exit.clone();
        question(&mut b, tr.yes, trade::MSG_CANCEL, title, 0xA3, 0xBA, 0x139);
    }
    if either(trade::state::TRADE_ASK) {
        let title = b.d.trade.clone();
        question(&mut b, tr.yes, trade::MSG_TRADE, title, 0xAE, 0xA4, 0x12E);
    }
    if either(trade::state::REFUSED) {
        let title = b.d.error.clone();
        let start = b.calls.len();
        banner_box(&mut b, trade::MSG_CANNOT, 0x100, 0x8C, 0x100, 0xE8, &title, Some(inp.counter));
        let end = b.calls.len();
        b.boxes.push(start..end);
    }
    Screen { calls: b.calls, boxes: b.boxes, panes: b.panes, notes: b.notes }
}

/// 0x1C47F0: two horizontal gradients, edges → centre → edges.
fn background(b: &mut B<'_>) {
    let c = b.d.background;
    let edge = [c[2], c[1], c[0], c[3]];
    let mid = [c[6], c[5], c[4], c[7]];
    for (x0, x1, a, z) in [(0.0, 256.0, edge, mid), (256.0, 512.0, mid, edge)] {
        b.c(MenuCall::Quad { corners: [(x0, 0.0, a), (x1, 0.0, z), (x0, 512.0, a), (x1, 512.0, z)] });
    }
}

/// 0x1C4C50.
fn title(b: &mut B<'_>, x: i32, y: i32) {
    b.c(MenuCall::Tiles { desc: 189, x: x + 0xD0, y: y + 0x20, nx: 3, ny: 1 });
    b.panel(-1, 0x73, 4, x + 0xD0, y + 0x20, -0x60, 2, true);
    let t = b.d.trade.clone();
    b.gold(x + 0x100, y + 0x27, 2, 3, &t, false);
}

/// 0x1C4D20(side, x, y): plate, totals, chest count, sort keys, six rows,
/// the view tabs and the position box.
fn side_panel(b: &mut B<'_>, inp: &Inputs<'_>, side: usize, x: i32, y: i32) {
    let tr = inp.trade;
    let s = &tr.sides[side];
    let right = side == 1;
    let pick = |r: i32, l: i32| if right { r } else { l };
    // SELECT held in the list state: the second sort key (0x1C4D78).
    let sel = usize::from(s.state == trade::state::LIST && inp.select_held[side]);
    let plate = x + pick(0x1C0, 0x10);
    b.panel(12, -1, 11, plate, y + 0x10, -0x30, -0x30, true);
    let who = b.d.players[side].clone();
    b.gold(plate + 0x18, y + 0x1F, 2, 3, &who, false);
    let tot = x + pick(0x140, 0x40);
    b.panel(12, -1, 6, tot, y + 0x10, -0x7C, -0x2C, true);
    let (total, ex, im) = (b.d.total.clone(), b.d.views[1].clone(), b.d.views[2].clone());
    b.gold(tot + 0x40, y + 0x15, 1, 3, &total, false);
    b.gold(tot + 0x20, y + 0x23, 1, 3, &ex, false);
    b.number(tot + 0x20, y + 0x31, 1, 2, 4, s.exported() as i32);
    b.gold(tot + 0x60, y + 0x23, 1, 3, &im, false);
    b.number(tot + 0x60, y + 0x31, 1, 2, 4, tr.sides[side ^ 1].exported() as i32);
    let chest = x + pick(0x1B0, 0x10);
    b.panel(12, -1, 5, chest, y + 0x40, -0x40, -0x14, true);
    b.sprite(203, chest + 2, y + 0x42);
    // 0x216180: every trunk copy of the block.
    let copies: usize = (0..trade::CARDS as u16).map(|c| trade::trunk_copies(&inp.blocks[side], c)).sum();
    b.number(chest + 0x17, y + 0x44, 1, 0, 4, copies as i32);
    b.boxed(3, chest, y + 0x54, -0x40, -0x1C);
    b.sprite(260, x + pick(0x1D5, 0x15), y + 0x5D);
    b.sprite(196, x + pick(0x1B4, 0x2C), y + 0x5C);
    let keys = x + pick(0x100, 0x50);
    b.boxed(3, keys, y + 0x40, -0xB0, -0x30);
    b.sprite(262, keys + 7, y + 0x5E);
    // The current key's icon: 0x1B48A0 overlay mode 1 (grey 128) or, with
    // SELECT held, mode 2 in 0x2BDC24's colour; alpha 0x2DB828 >> 8
    // [F 0x1C53A4..0x1C5414, 0x1B5020].
    let alpha = (tr.scroll[1] >> 8) as u8;
    for k in 0..10usize {
        let (ix, iy) = (keys + 8 + 16 * k as i32, y + 0x49);
        if k == usize::from(s.keys[sel]) {
            let c = b.d.select_tint;
            let (mode, rgba) = if sel == 1 { (2, [c[0], c[1], c[2], alpha]) } else { (1, [128, 128, 128, alpha]) };
            b.c(MenuCall::SpriteOverlay { desc: 209 + k, x: ix, y: iy, mode, rgba });
        } else {
            b.sprite(209 + k, ix, iy);
        }
    }
    b.sprite(263, keys + 0x93, y + 0x5E);
    let key = s.keys[sel];
    let label = b.d.sort_labels.get(usize::from(key)).cloned().unwrap_or_default();
    if sel == 1 {
        // t3 = 0x2BDC24 with flag 8 [F 0x1C54A0]; the font's use of the
        // colour's alpha is not traced [H].
        let rgba = b.d.select_tint;
        b.c(MenuCall::GoldTint { x: keys + 0x58, y: y + 0x5E, size: 1, flags: 3 | 8, text: label, rgba });
    } else {
        b.gold(keys + 0x58, y + 0x5E, 1, 3, &label, false);
    }
    let rows = x + pick(0x108, 8);
    for r in 0..6u16 {
        let card = s.list.get(usize::from(s.top + r)).copied().unwrap_or(EMPTY_CARD);
        row(b, inp, side, card, rows, y + 0x70 + 0x36 * i32::from(r));
    }
    let bottom = x + pick(0x144, 0x10);
    b.boxed(3, bottom, y + 0x1B4, -0xAC, -0x32);
    b.sprite(197, x + pick(0x1B4, 0x2C), y + 0x1B8);
    b.sprite(261, x + pick(0x1D5, 0x15), y + 0x1BD);
    b.sprite(254, x + pick(0x1D9, 0x17), y + 0x1D0);
    let exit = b.d.exit.clone();
    b.gold(x + pick(0x1B1, 0x2C), y + 0x1D3, 1, 0, &exit, false);
    let tabs = x + pick(0x14A, 0x56);
    for k in 0..3u8 {
        let ty = y + 0x1B8 + 0xE * i32::from(k);
        b.boxed(3, tabs, ty, -0x60, -0xE);
        let label = b.d.views[usize::from(k)].clone();
        b.gold(tabs + 1, ty + 1, 1, 0, &label, s.view != k);
    }
    let pos = x + pick(0x108, 0xBC);
    b.boxed(3, pos, y + 0x1B4, -0x3C, -0x32);
    if s.count != 0 {
        b.c(MenuCall::Line { x0: pos + 10, y0: y + 0x1B4 + 0x18, x1: pos + 0x32, rgba: [0xA0, 0xA0, 0xA0, 0x80] });
        b.number(pos + 0x1E, y + 0x1BC, 1, 2, 3, i32::from(s.top) + i32::from(s.cursor) + 1);
        b.number(pos + 0x1E, y + 0x1D3, 1, 2, 3, i32::from(s.count));
    }
}

/// 0x1DF290: the row colour class of a card kind.
fn kind_class(kind: u8) -> u8 {
    match kind {
        1 | 2 => 1,
        3 | 4 => 2,
        5 => 3,
        k => k,
    }
}

/// 0x1C5950(side, card, x, y): one list row.
fn row(b: &mut B<'_>, inp: &Inputs<'_>, side: usize, card: u16, x: i32, y: i32) {
    let right = side == 1;
    let pick = |r: i32, l: i32| if right { r } else { l };
    let tr = inp.trade;
    let s = &tr.sides[side];
    let Some(rec) = inp.cards.record(card).copied() else {
        b.panel(-1, -1, 9, x, y, -0xF0, -0x36, false);
        return;
    };
    let counts = if s.view == 2 { &tr.sides[side ^ 1].exports } else { &s.exports };
    let tag = if s.view == 2 { b.d.import_tag.clone() } else { b.d.export_tag.clone() };
    let kind = rec[0] >> 5;
    let type_desc = if kind == 0 { usize::from(rec[0] & 0x1F) } else { 0x14 + usize::from(kind) };
    let mut class = kind_class(kind);
    let attr = if class > 0 { class + 5 } else { rec[2] & 0xF };
    if class == 0 && u16::from_le_bytes([rec[4], rec[5]]) != 0xFFFF {
        class = 4;
    }
    let name = 323 + card;
    b.panel(i32::from(class) + 7, -1, 9, x, y, -0xF0, -0x36, false);
    if inp.blocks[side].recent.contains(&card) {
        b.sprite(245, x + pick(0xC8, 8), y + 7);
    } else {
        b.number(x + pick(0xC8, 8), y + 7, 1, 0, 3, i32::from(card));
    }
    if kind == 0 {
        let rank = inp.blocks[side].stored_rank(card).map_or(-1, i32::from);
        b.c(MenuCall::InsigniaEx { x: x + pick(0xA8, 0x28), y: y + 3, rank, small: false, mirror: right });
    }
    b.sprite(type_desc, x + pick(0xD4, 4), y + 0x1A);
    b.c(MenuCall::Attribute { icon: usize::from(attr), x: x + pick(0xBA, 0x1E), y: y + 0x1A });
    if kind == 0 {
        let lx = x + pick(0x8F, 0x38);
        b.sprite(215, lx, y + 0x23);
        b.number(lx + 0x16, y + 0x25, 1, 0, 2, i32::from(rec[2] >> 4));
        let ax = x + pick(3, 0xB8);
        b.sprite(211, ax, y + 3);
        b.number(ax + 0x10, y + 5, 1, 0, 4, i32::from(u16::from_le_bytes([rec[8], rec[9]]) & 0x1FFF));
        b.sprite(212, ax, y + 0x13);
        b.number(ax + 0x10, y + 0x15, 1, 0, 4, i32::from(u16::from_le_bytes([rec[10], rec[11]]) & 0x1FFF));
    }
    let cx = x + pick(0x56, 0x68);
    b.sprite(216, cx, y + 0x23);
    b.number(cx + 0x16, y + 0x25, 1, 0, 3, i32::from(rec[3]));
    let hx = x + pick(0x26, 0xAD);
    let all = roses_core::deck_editor::count_copies(&inp.blocks[side], card, None);
    b.sprite(if all < trade::MAX_COPIES { 203 } else { 204 }, hx, y + 0x23);
    b.number(hx + 0x13, y + 0x25, 1, 0, 1, trade::trunk_copies(&inp.blocks[side], card) as i32);
    let n = counts[usize::from(card)];
    if n != 0 {
        let px = x + pick(0, 0xA8);
        b.panel(13, -1, 7, px, y + 0x23, -0x48, -0x10, true);
        b.gold(px + 0xC, y + 0x25, 1, 0, &tag, false);
        b.number(px + 0x30, y + 0x25, 1, 0, 1, i32::from(n));
    }
    let nx = x + pick(0x38, 0x48);
    b.boxed(i32::from(class) + 7, nx, y + 3, -0x70, -0x20);
    if let Some((w, _)) = b.width(name) {
        let sx = if w >= 0x69 { 104.0 / w as f32 } else { 1.0 };
        for (k, (dx, dy)) in [(6, 9), (5, 8), (4, 7)].into_iter().enumerate() {
            let rgba = b.d.tints[k];
            b.c(MenuCall::TextTint { line: 1, message: name, x: nx + dx, y: y + dy, sx, sy: 1.0, rgba });
        }
    }
    if trade::trunk_copies(&inp.blocks[side], card) == 0 {
        b.c(MenuCall::DarkenRect { x, y, w: 0xF0, h: 0x36, alpha: 0x40 });
    }
}

/// 0x1C63A0(side, x, y, cursor): the cursor frame and, once the idle
/// timer has run out, the hint bar under it.
fn cursor(b: &mut B<'_>, inp: &Inputs<'_>, side: usize, x: i32, y: i32) {
    let s = &inp.trade.sides[side];
    let cx = x + 8 + if side == 1 { 0x100 } else { 0 };
    let cy = y + 0x70 + 0x36 * i32::from(s.cursor);
    b.panel(-1, 4, 6, cx - 2, cy - 2, -0xF0, -0x36, true);
    if s.idle != 0 {
        return;
    }
    b.panel(13, -1, 7, cx, cy + 0x36, -0xF0, -0x14, true);
    if s.view < 2 {
        b.sprite(253, cx + 5, cy + 0x38);
        b.sprite(252, cx + 0x16, cy + 0x38);
        let t = b.d.ex_set.clone();
        b.gold(cx + 0x2C, cy + 0x3B, 1, 0, &t, false);
    }
    b.sprite(251, cx + 0x6B, cy + 0x38);
    let t = b.d.detail.clone();
    b.gold(cx + 0x81, cy + 0x3B, 1, 0, &t, false);
    b.sprite(255, cx + 0xA5, cy + 0x38);
    let t = b.d.trade.clone();
    b.gold(cx + 0xBB, cy + 0x3B, 1, 0, &t, false);
}

/// 0x1C66C0(x, y): READY on a plate centred on (x + 0x100, y + 0x100).
fn ready_box(b: &mut B<'_>, x: i32, y: i32) {
    let text = b.d.ready.clone();
    let (tw, th) = gold_measure(b.t, 2, &text);
    let (w, h) = (tw + 0x18, th + 0xC);
    b.panel(12, -1, 11, x + 0x100 - (w >> 1), y + 0x100 - (h >> 1), -w, -h, true);
    b.gold(x + 0x100, y + 0x108 - (h >> 1), 2, 3, &text, false);
}

/// 0x1C67D0 / 0x1C6980: 0x1B1830(2, message, 0x100, top, 0x100, min h,
/// title), MSG 128 "Are you sure?" centred above YES / NO at (0xC0 /
/// 0x140, `line`), the unselected one dimmed.
fn question(b: &mut B<'_>, yes: bool, message: u16, title: Vec<u8>, top: i32, min_h: i32, line: i32) {
    let start = b.calls.len();
    banner_box(b, message, 0x100, top, 0x100, min_h, &title, None);
    let end = b.calls.len();
    b.boxes.push(start..end);
    if let Some((w, lines)) = b.width(trade::MSG_SURE) {
        let h = 22 * lines;
        b.c(MenuCall::Text { line: 1, message: trade::MSG_SURE, x: 0x100 - (w >> 1), y: line - (h + 0xC), sx: 1.0, sy: 1.0, lines: lines as u8, dim: false });
    }
    let (y, n) = (b.d.yes.clone(), b.d.no.clone());
    b.gold(0xC0, line, 2, 3, &y, !yes);
    b.gold(0x140, line, 2, 3, &n, yes);
}

/// 0x1B1830(colour 2, message, cx, top, width, min h, title, press): the
/// box, the title plate and its gold title, the text; with the press flag
/// 0x1C more height and the press icon (descriptor 190 + (0x2DA954 / 5) %
/// 3, as the memory-card boxes' kind 6) at (cx − 8, bottom − 0x1C)
/// [emu t5-draw-refused: panel 48,140 −408×−298, icon 248,410; layout as
/// `custom_duel_screen`'s exit box].
#[allow(clippy::too_many_arguments)]
fn banner_box(b: &mut B<'_>, message: u16, cx: i32, top: i32, width: i32, min_h: i32, title: &[u8], press: Option<u32>) {
    let (tw, th) = gold_measure(b.t, 2, title);
    let (plate_w, plate_h) = (tw + 0x18, th + 0xC);
    let Some((w, lines)) = b.width(message) else { return };
    let h = 22 * lines;
    let width = width.max(w.max(plate_w) + 0x18);
    let icon = if press.is_some() { 0x1C } else { 0 };
    let inner_h = min_h.max(plate_h + 0x18 + h + 0xC + icon);
    b.panel(13, -1, 1, cx - (width >> 1) - 4, top, -width, -inner_h, true);
    b.panel(2, -1, 11, cx - (plate_w >> 1), top + 0xC, -plate_w, -plate_h, true);
    b.gold(cx, top + 0x14, 2, 3, title, false);
    b.c(MenuCall::Text { line: 2, message, x: cx - (w >> 1), y: top + plate_h + 0x18, sx: 1.0, sy: 1.0, lines: lines as u8, dim: false });
    if let Some(counter) = press {
        b.sprite(190 + ((counter / 5) % 3) as usize, cx - 8, top + inner_h - 0x1C);
    }
}

/// 0x1B82C0(size, flags 3).
fn gold_measure(t: &HudTables, size: usize, text: &[u8]) -> (i32, i32) {
    let (cw, lh) = t.font_sizes[size];
    let mut w = 0;
    for &ch in text {
        let g = match ch {
            b'A'..=b'Z' => Some(usize::from(ch - b'A')),
            b'a'..=b'z' => Some(usize::from(ch - b'a')),
            b'0'..=b'9' => Some(usize::from(ch - b'0') + 26),
            b'?' => Some(36),
            _ => None,
        };
        w += match g {
            Some(g) => t.glyphs.get(g + 37 * size).map_or(0, |d| d.width()),
            None => i32::from(cw) >> 1,
        };
    }
    (w, i32::from(lh))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mine(c: &MenuCall) -> Option<(String, Vec<i64>)> {
        let v = |xs: &[i32]| xs.iter().map(|&x| i64::from(x)).collect();
        Some(match c {
            MenuCall::Panel { grad, deco, ptype, x, y, wu, hu, .. } => ("panel".into(), v(&[*grad, *deco, *ptype as i32, *x, *y, *wu, *hu])),
            MenuCall::Sprite { desc, x, y } => ("sprite".into(), v(&[*desc as i32, *x, *y, -1])),
            // 0x1B48A0's stack +0x20: the overlay's ALPHA mode.
            MenuCall::SpriteOverlay { desc, x, y, mode, .. } => ("sprite".into(), v(&[*desc as i32, *x, *y, i32::from(*mode)])),
            MenuCall::Attribute { icon, x, y } => ("attr".into(), v(&[*icon as i32, *x, *y])),
            MenuCall::Gold { x, y, size, flags, dim, .. } => ("font".into(), v(&[*x, *y, *size as i32, *flags as i32, i32::from(*dim)])),
            MenuCall::GoldTint { x, y, size, flags, .. } => ("font".into(), v(&[*x, *y, *size as i32, *flags as i32, 1])),
            MenuCall::Number { x, y, size, flags, digits, value, .. } => ("number".into(), v(&[*x, *y, *size as i32, *flags as i32, *digits, *value])),
            MenuCall::Text { x, y, .. } | MenuCall::TextTint { x, y, .. } => ("text".into(), v(&[*x, *y])),
            MenuCall::InsigniaEx { x, y, rank, small, mirror } => ("insignia".into(), v(&[*x, *y, *rank, i32::from(*small), i32::from(*mirror)])),
            MenuCall::Box { x, y, wu, hu, colour } => ("f1b71d0".into(), v(&[*colour, *x, *y, *wu, *hu])),
            MenuCall::Tiles { x, y, .. } => ("f1b6ed0".into(), v(&[*x, *y])),
            MenuCall::DarkenRect { w: 0xF0, .. } => return None,
            MenuCall::DarkenRect { x, y, w, h, alpha } => ("darken".into(), v(&[*x, *y, *w, *h, i32::from(*alpha)])),
            _ => return None,
        })
    }

    fn logged(c: &serde_json::Value) -> Option<(String, Vec<i64>)> {
        let ra = i64::from_str_radix(c["ra"].as_str()?.trim_start_matches("0x"), 16).ok()?;
        // The trade's own calls, 0x1B1830's (its panels, title, text) and
        // the press icon's 0x1B4860.
        if !(0x1C3050..0x1C6BC0).contains(&ra) && !(0x1B1830..0x1B1D50).contains(&ra) && ra != 0x1B4894 {
            return None;
        }
        let a: Vec<i64> = c["args"].as_array()?.iter().map(|x| x.as_i64().unwrap()).collect();
        let st: Vec<i64> = c["stack"].as_array()?.iter().map(|x| x.as_i64().unwrap()).collect();
        let fn_ = c["fn"].as_str()?;
        Some(match fn_ {
            "panel" => (fn_.into(), a[1..8].to_vec()),
            "sprite" if (0x28_F2A0..0x28_F300).contains(&a[1]) => ("attr".into(), vec![(a[1] - 0x28_F2A0) / 10, a[2] - 0x700, a[3] - 0x700]),
            "sprite" => (fn_.into(), vec![(a[1] - 0x28_F300) / 10, a[2] - 0x700, a[3] - 0x700, if ra == 0x1B4894 { -1 } else { st[8] }]),
            "font" => (fn_.into(), vec![a[1], a[2], a[3], a[4], i64::from(a[7] != 0)]),
            "number" => (fn_.into(), a[1..7].to_vec()),
            "text" | "msg246d00" => ("text".into(), a[2..4].to_vec()),
            "insignia" => (fn_.into(), vec![a[1], a[2], a[3], a[4], a[5]]),
            "f1b71d0" => (fn_.into(), a[1..6].to_vec()),
            "f1b6ed0" => (fn_.into(), a[1..3].to_vec()),
            "f1b8e40" => ("darken".into(), vec![a[1], a[2], a[3], a[4], a[5]]),
            _ => {
                let _ = st;
                return None;
            }
        })
    }

    /// The builder against the interpreter's draw logs
    /// (`ROSES_TRADE_CAPTURES` = modes/two-player/captures, files `t*-draw*.json`
    /// run with DRAWFROM 0; `ROSES_DUEL_IMPORT` = a duel import): every
    /// logged leaf call of the trade code in order, with its values; the
    /// boxes of 0x1B1830 and the detail pane are skipped (see the module
    /// notes). The model is replayed from the logged pads.
    #[test]
    fn private_screen_matches_the_emulated_draw() {
        let (Some(dir), Some(duel)) = (std::env::var_os("ROSES_TRADE_CAPTURES"), std::env::var_os("ROSES_DUEL_IMPORT")) else {
            return;
        };
        let data = crate::duel_play::load(std::path::Path::new(&duel)).unwrap();
        let mut ids = HudTables::required_messages();
        ids.extend(126..130);
        ids.extend(323..323 + 854);
        ids.extend(crate::card_detail::message_ids());
        ids.sort_unstable();
        ids.dedup();
        let mut codes = std::collections::BTreeMap::new();
        for chunk in ids.chunks(256) {
            for m in roses_assets::parse_pal_messages(&data.messages, chunk).unwrap().messages {
                codes.insert(m.message_id, m.expanded_codes);
            }
        }
        let tables = HudTables::from_executable(&data.executable, codes).unwrap();
        let td = TradeData::from_executable(&data.executable).unwrap();
        let cards = StaticCards::from_executable(&data.executable, 0).unwrap();
        let mut checked = 0;
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.starts_with('t') && n.contains("-draw") && n.ends_with(".json")).collect();
        names.sort();
        for name in names {
            let v: serde_json::Value = serde_json::from_slice(&std::fs::read(std::path::Path::new(&dir).join(&name)).unwrap()).unwrap();
            let hex = |s: &str| (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect::<Vec<u8>>();
            let starts: Vec<Vec<u8>> = v["start_blocks"].as_array().unwrap().iter().map(|b| hex(b.as_str().unwrap())).collect();
            let mut blocks = [CardBlock::from_bytes(&starts[0]).unwrap(), CardBlock::from_bytes(&starts[1]).unwrap()];
            let mut t = Trade::new(&blocks, &cards);
            let mut prev = [0u16; 2];
            for (k, f) in v["frames"].as_array().unwrap().iter().enumerate().skip(1) {
                let word = [0, 1].map(|p| f["pad"][p].as_u64().unwrap() as u16);
                let pads = [0, 1].map(|p| trade::TradePad { pressed: word[p] & !prev[p], held: word[p], repeat: word[p] & !prev[p], analog: None });
                prev = word;
                t.update(pads, &mut blocks, &cards, true);
                let frame = (k - 1) as i64;
                let inp = Inputs { trade: &t, blocks: &blocks, cards: &cards, select_held: [word[0] & 0x100 != 0, word[1] & 0x100 != 0], pictures: [false; 2], counter: 0 };
                let screen = build(&td, &tables, &inp);
                let ours: Vec<_> = screen
                    .calls
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !screen.panes.iter().any(|(r, _)| r.contains(i)))
                    .filter_map(|(_, c)| mine(c))
                    .collect();
                let theirs: Vec<_> = v["calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|c| c["frame"].as_i64() == Some(frame))
                    .filter_map(logged)
                    .collect();
                if theirs.is_empty() {
                    continue;
                }
                for (i, (a, b)) in ours.iter().zip(theirs.iter()).enumerate() {
                    assert_eq!(a, b, "{name} frame {frame} call {i}");
                }
                assert_eq!(ours.len(), theirs.len(), "{name} frame {frame} call count");
                checked += 1;
            }
        }
        eprintln!("{checked} trade frames match");
        assert!(checked > 0);
    }
}
