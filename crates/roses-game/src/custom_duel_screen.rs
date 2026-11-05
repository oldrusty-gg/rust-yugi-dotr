//! Custom Duel's set-up screen (mode 9 draw 0x1C8280 → 0x1C82B0) as the
//! original's HUD-leaf calls (`duel_hud::MenuCall`), then primitives drawn
//! from the user's TEXSYS.MRG atlases, PICMINI portraits and message font.
//! Bevy-free; the scene is `custom_duel_render.rs` in the binary.
//!
//! Provenance (the project notes §5): the
//! draw functions 0x1C82B0, 0x1C85E0, 0x1C8A40, 0x1C8B60, 0x1C93D0,
//! 0x1C94D0, 0x1C9C80, 0x1CA1B0, 0x1CABE0, 0x1CB890 and 0x1CD620 were read
//! [F] and run in the EE interpreter; their leaf calls (positions, sizes,
//! descriptor indices, values) were logged per state [emu] and the private
//! test below compares this builder with those logs. The text canvas line of
//! a message is the port's (each sprite recomposes its line; the original
//! loads up to eight lines first). Two players (selector 0; `work/
//! research/modes/two-player/NOTES.md`): the READY box of 0x1C82B0 and
//! the Leader Ability / Card Display sub-screens 0x1CB090 / 0x1CB490 [F,
//! emu d7-two-players]. Own maps (map editor, the project notes): Edit Map's list 0x1CC870, the map lists' own-map
//! previews read from each side's block, the chosen row's band and the
//! terrain gauges (0x206F90) [F, emu d5-ownlist / d6-ownmaps-both].
//! Nothing here is a game byte: strings and colours are read from the
//! user's executable; positions are what the code computes.
use roses_client::custom_duel::{self as cd, mode, row, CustomDuel};
use roses_client::duel_hud::{self, HudTables, MenuCall, YesNoTables};

/// The executable's strings and colour tables the screen uses.
#[derive(Clone, Debug)]
pub struct ScreenData {
    pub custom: Vec<u8>,
    pub vs: Vec<u8>,
    pub change: Vec<u8>,
    pub exit: Vec<u8>,
    pub ready: Vec<u8>,
    pub back: Vec<u8>,
    pub start: Vec<u8>,
    pub open: Vec<u8>,
    pub hide: Vec<u8>,
    /// Leader Ability's choices (0x2BE3DE "NOT USE" / 0x2BE3DA "USE").
    pub not_use: Vec<u8>,
    pub use_: Vec<u8>,
    /// YES / NO (0x2BE2EF / 0x2BE2F3).
    pub yes: Vec<u8>,
    pub no: Vec<u8>,
    /// 0x2BDBE0: the background's edge and centre colours (B, G, R, A ×2).
    pub background: [u8; 8],
    /// 0x290720: the LP gauge colours.
    pub gauge: [u8; 8],
    /// 0x2BE3EC "EDIT" (the own-map list's Cross hint).
    pub edit: Vec<u8>,
    /// 0x2BDC40: the terrain gauges' colours (the map editor's tables).
    pub terrain: roses_client::map_editor::EditorTables,
}

impl ScreenData {
    pub fn from_executable(executable: &[u8]) -> Result<Self, String> {
        let s = |a: u32| duel_hud::elf_string(executable, a).map_err(|e| e.to_string());
        let t = |a: u32| -> Result<[u8; 8], String> {
            duel_hud::elf_slice(executable, a, 8)
                .map_err(|e| e.to_string())
                .map(|b| b.try_into().unwrap())
        };
        let yn = YesNoTables::from_executable(executable).map_err(|e| e.to_string())?;
        Ok(Self {
            yes: yn.yes,
            no: yn.no,
            custom: s(0x2B_E3BF)?,
            vs: s(0x2B_E3C6)?,
            change: s(0x2B_E3D3)?,
            exit: s(0x2B_E318)?,
            ready: s(0x2B_E3B2)?,
            back: s(0x2B_E313)?,
            start: s(0x2B_E3E6)?,
            open: s(0x2B_E3C9)?,
            hide: s(0x2B_E3CE)?,
            not_use: s(0x2B_E3DE)?,
            use_: s(0x2B_E3DA)?,
            background: t(0x2B_DBE0)?,
            gauge: t(0x29_0720)?,
            edit: s(0x2B_E3EC)?,
            terrain: roses_client::map_editor::EditorTables::from_executable(executable)?,
        })
    }
}

/// A side's leader as the panel shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leader {
    pub card: u16,
    /// Card record byte 0 & 0x1F (the type picture and MSG 38 + type).
    pub kind: u8,
    /// 0x2154C0: the stored rank (MSG 83 + rank).
    pub rank: u8,
}

/// One side's panel inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideView {
    /// The name plate's message (4 = the player's name, 141 + selector
    /// for the computer).
    pub name: u16,
    pub leader: Option<Leader>,
    /// 0x215040 and 0x215690 (the current deck's cost).
    pub deck: u8,
    pub cost: u16,
    /// Each deck's leader card when it has one (Select Deck's portraits).
    pub deck_leaders: [Option<u16>; 3],
}

pub struct ScreenInputs<'a> {
    pub menu: &'a CustomDuel,
    pub sides: [SideView; 2],
    pub has_maps: [bool; 2],
    /// The 49 terrain cells of each default map 0x16.. (the map list's
    /// preview), indexed by map − 0x16.
    pub default_maps: &'a [Vec<u8>],
    /// Each side's card block's five own maps (map editor, +0xFEA3).
    pub own_maps: [Vec<[u8; 49]>; 2],
}

/// The portraits the calls sample: slots 0/1 are the panels' leaders,
/// slot 2.. the Select Deck screen's (`Portrait { slot }`).
pub struct Screen {
    pub calls: Vec<MenuCall>,
    pub portraits: Vec<Option<u16>>,
    pub notes: Vec<String>,
}

struct B<'a> {
    d: &'a ScreenData,
    t: &'a HudTables,
    calls: Vec<MenuCall>,
    portraits: Vec<Option<u16>>,
    notes: Vec<String>,
}

const MSG_TYPE_BASE: u16 = 38;
const MSG_RANK_BASE: u16 = 83;
const CARD_NAME_BASE: u16 = 323;
const MSG_EXIT_QUESTION: u16 = 146;

impl B<'_> {
    fn c(&mut self, call: MenuCall) {
        self.calls.push(call);
    }
    fn panel(&mut self, grad: i32, deco: i32, ptype: usize, x: i32, y: i32, wu: i32, hu: i32) {
        self.c(MenuCall::Panel { grad, deco, ptype, x, y, wu, hu, bright: true });
    }
    fn sprite(&mut self, desc: usize, x: i32, y: i32) {
        self.c(MenuCall::Sprite { desc, x, y });
    }
    fn gold(&mut self, x: i32, y: i32, size: usize, flags: u32, text: &[u8]) {
        self.c(MenuCall::Gold { x, y, size, flags, text: text.to_vec(), dim: false });
    }
    fn number(&mut self, x: i32, y: i32, size: usize, flags: u32, digits: i32, value: i32) {
        self.c(MenuCall::Number { x, y, size, flags, digits, value, dim: false });
    }
    /// (width, lines) of a composed message.
    fn measure(&mut self, message: u16) -> Option<(i32, i32)> {
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
    /// 0x246B90 with the callers' squeeze: sx = limit / w when w > limit;
    /// sy = hlimit / h when h > hlimit (22-pixel lines).
    fn text(&mut self, message: u16, x: i32, y: i32, sx: f32, sy: f32) {
        self.c(MenuCall::Text { line: 1, message, x, y, sx, sy, lines: 1, dim: false });
    }
    fn fit(w: i32, limit: i32) -> f32 {
        if w > limit { limit as f32 / w as f32 } else { 1.0 }
    }
    /// A line centred on `cx`: x = cx − trunc((w >> 1) · sx) (0x1A2B80).
    fn centred(&mut self, message: u16, cx: i32, y: i32, limit: i32, hlimit: i32) {
        let Some((w, lines)) = self.measure(message) else { return };
        let sx = Self::fit(w, limit);
        let h = 22 * lines;
        let sy = if h > hlimit { hlimit as f32 / h as f32 } else { 1.0 };
        let x = cx - (((w >> 1) as f32) * sx) as i32;
        self.text(message, x, y, sx, sy);
    }
}

/// The calls of one frame.
pub fn build(data: &ScreenData, tables: &HudTables, inp: &ScreenInputs<'_>) -> Screen {
    let mut b = B { d: data, t: tables, calls: Vec::new(), portraits: vec![None; 2], notes: Vec::new() };
    let m = inp.menu;
    background(&mut b);
    title(&mut b, 0, 0);
    side_panel(&mut b, inp, 0, 0x10, 0x40);
    side_panel(&mut b, inp, 1, 0x100, 0x40);
    let states = [m.players[0].state, m.players[1].state];
    if states.iter().any(|s| s & 0xF0 == mode::MAP) {
        map_list(&mut b, inp, 0x1E, 0xF0);
    } else {
        for side in 0..2u8 {
            if side == 1 && m.selector != 0 {
                break;
            }
            if states[usize::from(side)] & 0xF0 == mode::READY {
                ready_box(&mut b, 0x80 + 0x100 * i32::from(side));
            } else {
                rows_or_edit(&mut b, inp, side, 0x10 + 0xF0 * i32::from(side), 0x106);
            }
        }
    }
    if states.iter().any(|s| s & 0xF0 == mode::EXIT) {
        b.c(MenuCall::Darken { alpha: 0x40 });
        exit_box(&mut b, m.exit_yes);
    }
    Screen { calls: b.calls, portraits: b.portraits, notes: b.notes }
}

/// 0x1C85E0: two horizontal gradients, edges → centre → edges.
fn background(b: &mut B<'_>) {
    let c = b.d.background;
    let edge = [c[2], c[1], c[0], c[3]];
    let mid = [c[6], c[5], c[4], c[7]];
    for (x0, x1, a, z) in [(0.0, 256.0, edge, mid), (256.0, 512.0, mid, edge)] {
        b.c(MenuCall::Quad { corners: [(x0, 0.0, a), (x1, 0.0, z), (x0, 512.0, a), (x1, 512.0, z)] });
    }
}

/// 0x1C8A40.
fn title(b: &mut B<'_>, x: i32, y: i32) {
    b.c(MenuCall::Tiles { desc: 189, x: x + 0xC0, y: y + 0x20, nx: 4, ny: 1 });
    b.panel(-1, 0x73, 4, x + 0xC0, y + 0x20, 13, 2);
    let (custom, vs) = (b.d.custom.clone(), b.d.vs.clone());
    b.gold(x + 0x100, y + 0x27, 2, 3, &custom);
    b.gold(x + 0x100, y + 0x46, 2, 3, &vs);
}

/// 0x1C8B60(side, x, y).
fn side_panel(b: &mut B<'_>, inp: &ScreenInputs<'_>, side: u8, x: i32, y: i32) {
    let s = i32::from(side);
    let v = &inp.sides[usize::from(side)];
    b.sprite(243 + usize::from(side), x + 0xD0 * s, y);
    b.panel(s, s + 5, 11, x + 0x20, y, -0xB0, -0x20);
    if let Some((w, _)) = b.measure(v.name) {
        b.text(v.name, x + 0x26, y + 5, B::fit(w, 0xA4), 1.0);
    }
    b.panel(s, -1, 4, x, y + 0x20, -0xF0, -0x80);
    b.c(MenuCall::Box { colour: s, x: x + 0xD, y: y + 0x25, wu: -0xD6, hu: -0x5E });
    if let Some(l) = v.leader {
        b.panel(12, -1, 11, x + 0x11, y + 0x29, -0x6E, -0x56);
        b.sprite(233, x + 0x19, y + 0x31);
        b.sprite(227 + usize::from(v.deck.min(2)), x + 0x24, y + 0x44);
        b.sprite(216, x + 0x20, y + 0x64);
        b.number(x + 0x35, y + 0x65, 0, 0, 4, i32::from(v.cost));
        b.portraits[usize::from(side)] = Some(l.card);
        b.c(MenuCall::Portrait { slot: side, x: x + 0x4A, y: y + 0x2E, w: 0x30, h: 0x30, bright: true });
        b.panel(-1, -1, 4, x + 0x4A, y + 0x2E, 3, 6);
        let name = CARD_NAME_BASE + l.card;
        if let Some((w, _)) = b.measure(name) {
            b.text(name, x + 0x81, y + 0x2B, B::fit(w, 0x5C), 1.0);
        }
        b.sprite(usize::from(l.kind), x + 0x81, y + 0x47);
        let kind = MSG_TYPE_BASE + u16::from(l.kind);
        if let Some((w, _)) = b.measure(kind) {
            b.text(kind, x + 0x9B, y + 0x47, B::fit(w, 0x44), 1.0);
        }
        if inp.menu.selector != 0 || inp.menu.players[usize::from(side)].leader_set != 0 {
            b.c(MenuCall::Insignia { x: x + 0x81, y: y + 0x65, rank: i32::from(l.rank) });
            let rank = MSG_RANK_BASE + u16::from(l.rank);
            if let Some((w, _)) = b.measure(rank) {
                b.text(rank, x + 0x9B, y + 0x65, B::fit(w, 0x44), 1.0);
            }
        }
    }
    let duel = inp.menu.sides[usize::from(side)];
    b.panel(12, -1, 5, x + 0x14, y + 0x86, -0x54, -0x14);
    b.sprite(246, x + 0x16, y + 0x88);
    b.number(x + 0x3E, y + 0x8A, 1, 0, 4, i32::from(duel.lp));
    b.panel(12, -1, 5, x + 0x78, y + 0x86, -0x2E, -0x14);
    b.sprite(215, x + 0x7A, y + 0x88);
    b.number(x + 0x90, y + 0x8A, 1, 0, 2, i32::from(duel.sp));
    if inp.menu.selector == 0 {
        let text = if inp.menu.players[usize::from(side)].display_set != 0 { b.d.open.clone() } else { b.d.hide.clone() };
        b.gold(x + 0xC8, y + 0x8A, 1, 2, &text);
    }
}

/// 0x1C93D0: the rows, or the edited row's sub-screen.
fn rows_or_edit(b: &mut B<'_>, inp: &ScreenInputs<'_>, side: u8, x: i32, y: i32) {
    let st = inp.menu.players[usize::from(side)].state;
    if st & 0xF0 != mode::EDIT {
        return rows(b, inp, side, x, y);
    }
    match st & 0xF {
        row::SELECT_DECK => edit_deck(b, inp, side, x + 0xE, y + 0xB),
        row::LP_SETTING => edit_lp(b, inp, side, x + 0xE, y + 0xB),
        row::SP_RECHARGE => edit_sp(b, inp, side, x + 0xE, y + 0xB),
        row::BUILD_DECK | row::SAVE => rows(b, inp, side, x, y),
        row::LEADER_ABILITY | row::CARD_DISPLAY => edit_switch(b, inp, side, x + 0xE, y + 0xB, st & 0xF),
        row::EDIT_MAP => own_map_list(b, inp, side, x + 0xE, y - 0x16),
        r => {
            b.notes.push(format!("the sub-screen of row {r} (MSG {}) is not drawn", cd::ROW_LABEL_BASE + u16::from(r)));
            rows(b, inp, side, x, y);
        }
    }
}

/// The hint bar of the rows and the sub-screens (`exit` = EXIT / BACK;
/// the rows add READY).
fn hints(b: &mut B<'_>, side: u8, x: i32, y: i32, w: i32, exit: &[u8], ready: bool) {
    b.c(MenuCall::Box { colour: i32::from(side), x: x + 0x20, y: y + 0x85, wu: -(w - 0x3A), hu: -0x34 });
    let q = |k: i32| ((w - 0x3A) * k) >> 2;
    b.sprite(252, x + 7 + q(1), y + 0x8C);
    b.sprite(253, x + 0x17 + q(1), y + 0x8C);
    let change = b.d.change.clone();
    b.gold(x + 0x17 + q(1), y + 0xA7, 1, 2, &change);
    b.sprite(254, x + 0x18 + q(2), y + 0x8C);
    b.gold(x + 0x20 + q(2), y + 0xA7, 1, 2, exit);
    if ready {
        b.sprite(255, x + 0x21 + q(3), y + 0x8C);
        let r = b.d.ready.clone();
        b.gold(x + 0x29 + q(3), y + 0xA7, 1, 2, &r);
    }
}

/// 0x1C94D0.
fn rows(b: &mut B<'_>, inp: &ScreenInputs<'_>, side: u8, x: i32, y: i32) {
    let m = inp.menu;
    let two = m.selector == 0;
    let w = if two { 0xEA } else { 0x1DA };
    let grid = if two { &cd::GRID_TWO_PLAYERS } else { &cd::GRID_COMPUTER };
    let s = i32::from(side);
    b.panel(s, s, 0, x, y, -(w - 0x14), -0xAE);
    let current = m.players[usize::from(side)].state & 0xF;
    for c in 0..2i32 {
        for r in 0..5i32 {
            let entry = grid[(c * 5 + r) as usize];
            if entry == 0xFF || (entry == row::EDIT_MAP && !inp.has_maps[usize::from(side)]) {
                continue;
            }
            let message = cd::ROW_LABEL_BASE + u16::from(entry);
            let Some((tw, lines)) = b.measure(message) else { continue };
            let h = 22 * lines;
            let sx = B::fit(tw, 0xAC);
            if two {
                let sy = if h > 0x14 { 20.0 / h as f32 } else { 0.909_090_9 };
                let cx = x + 2 + (w >> 1);
                b.text(message, cx - (((tw >> 1) as f32) * sx) as i32, y + 0x1A + 20 * r, sx, sy);
            } else {
                let sy = if h > 0x16 { 22.0 / h as f32 } else { 1.0 };
                let cx = x + 2 + (w >> 2) * (2 * c + 1);
                b.text(message, cx - (((tw >> 1) as f32) * sx) as i32, y + 0x20 + 32 * r, sx, sy);
            }
            if entry == current {
                if two {
                    b.panel(-1, -1, 6, x - 0x5D + (w >> 1), y + 0x16 + 20 * r, -0xBE, -0x18);
                } else {
                    b.panel(-1, -1, 6, x - 0x5D + (w >> 2) * (2 * c + 1), y + 0x1C + 32 * r, -0xBE, -0x1A);
                }
            }
        }
    }
    let exit = b.d.exit.clone();
    hints(b, side, x, y, w, &exit, true);
}

/// The sub-screens' common frame: the plate with the row's label and the
/// big box; returns the width.
fn edit_frame(b: &mut B<'_>, inp: &ScreenInputs<'_>, side: u8, x: i32, y: i32, r: u8) -> i32 {
    let w = if inp.menu.selector == 0 { 0xD4 } else { 0x1C4 };
    b.panel(12, 3, 4, x + ((w - 0xB2) >> 1), y, -0xB2, -0x20);
    b.centred(cd::ROW_LABEL_BASE + u16::from(r), x + (w >> 1), y + 5, 0x9A, 0x16);
    b.panel(i32::from(side), -1, 11, x, y + 0x20, -w, -0x96);
    w
}

/// 0x1C9C80: the three decks.
fn edit_deck(b: &mut B<'_>, inp: &ScreenInputs<'_>, side: u8, x: i32, y: i32) {
    let w = edit_frame(b, inp, side, x, y, row::SELECT_DECK);
    let shown = inp.menu.players[usize::from(side)].deck;
    for d in 0..3u8 {
        let cx = x + (w >> 2) * (i32::from(d) + 1);
        b.sprite(227 + usize::from(d), cx - 12, y + 0x2B);
        let lit = d == shown;
        // A deck without a leader keeps the call; its graphics slot holds
        // nothing the port can name, so the renderer skips it [H].
        let slot = b.portraits.len() as u8;
        b.portraits.push(inp.sides[usize::from(side)].deck_leaders[usize::from(d)]);
        b.c(MenuCall::Portrait { slot, x: cx - 24, y: y + 0x46, w: 48, h: 48, bright: lit });
        if lit {
            b.panel(-1, -1, 4, cx - 24, y + 0x46, -48, -48);
        } else {
            b.panel(5, -1, 7, cx - 24, y + 0x46, -48, -48);
        }
    }
    let back = b.d.back.clone();
    hints(b, side, x - 0xE, y - 0xB, w + 0x16, &back, false);
}

/// 0x1CA1B0: LP with its gauge (trunc(112 · LP / 9999) wide).
fn edit_lp(b: &mut B<'_>, inp: &ScreenInputs<'_>, side: u8, x: i32, y: i32) {
    let w = edit_frame(b, inp, side, x, y, row::LP_SETTING);
    let cx = x + (w >> 1);
    let lp = inp.menu.players[usize::from(side)].lp;
    b.panel(12, -1, 5, cx - 0x38, y + 0x38, -0x70, -0x14);
    b.sprite(246, cx - 0x36, y + 0x3A);
    b.number(cx - 0xE, y + 0x3C, 0, 0, 4, i32::from(lp));
    b.sprite(207, cx - 0x52, y + 0x4C);
    b.panel(13, -1, 7, cx - 0x38, y + 0x50, -0x70, -0x10);
    let mut g = b.d.gauge;
    if side == 0 {
        for k in [0, 1, 4, 5] {
            g[k] = 0;
        }
    }
    let (c1, c2) = ([g[2], g[1], g[0], g[3]], [g[6], g[5], g[4], g[7]]);
    let gw = (112.0 * f32::from(lp) / 9999.0) as i32;
    let (x0, x1) = ((cx - 0x38) as f32, (cx - 0x38 + gw) as f32);
    let (y0, y1, y2) = ((y + 0x50) as f32, (y + 0x58) as f32, (y + 0x60) as f32);
    b.c(MenuCall::Quad { corners: [(x0, y0, c1), (x1, y0, c1), (x0, y1, c2), (x1, y1, c2)] });
    b.c(MenuCall::Quad { corners: [(x0, y1, c2), (x1, y1, c2), (x0, y2, c1), (x1, y2, c1)] });
    b.panel(-1, -1, 7, cx - 0x38, y + 0x50, -0x70, -0x10);
    b.sprite(208, cx + 0x3A, y + 0x4C);
    let back = b.d.back.clone();
    hints(b, side, x - 0xE, y - 0xB, w + 0x16, &back, false);
}

/// 0x1CABE0: SP Recharge Rate.
fn edit_sp(b: &mut B<'_>, inp: &ScreenInputs<'_>, side: u8, x: i32, y: i32) {
    let w = edit_frame(b, inp, side, x, y, row::SP_RECHARGE);
    let cx = x + (w >> 1);
    b.sprite(207, cx - 0x38, y + 0x44);
    b.panel(12, -1, 5, cx - 0x1E, y + 0x46, -0x3C, -0x14);
    b.sprite(215, cx - 0x1C, y + 0x48);
    b.number(cx - 6, y + 0x4A, 0, 0, 2, i32::from(inp.menu.players[usize::from(side)].sp));
    b.sprite(208, cx + 0x20, y + 0x44);
    let back = b.d.back.clone();
    hints(b, side, x - 0xE, y - 0xB, w + 0x16, &back, false);
}

/// 0x1C82B0's READY box (a player waiting in 2P): the plate 0x1B5360(12,
/// −1, 11) sized by 0x1B82C0(2, 3, "READY") + (0x18, 0xC), centred on
/// (`cx`, 0x164), the word at (`cx`, 0x16C − h/2) [F 0x1C8364..0x1C842C].
fn ready_box(b: &mut B<'_>, cx: i32) {
    let text = b.d.ready.clone();
    let (tw, th) = gold_measure(b.t, 2, &text);
    let (w, h) = (tw + 0x18, th + 0xC);
    b.panel(12, -1, 11, cx - (w >> 1), 0x164 - (h >> 1), -w, -h);
    b.gold(cx, 0x16C - (h >> 1), 2, 3, &text);
}

/// 0x1CB090 (Leader Ability) / 0x1CB490 (Card Display): the row's frame,
/// the two choices in size 0 at (x+0x46 / x+0x8D, y+0x4D) — the left one
/// ("NOT USE" / "HIDE") dimmed while the shown value ([8] / [0xC]) is 1,
/// the right one ("USE" / "OPEN") while it is 0 — and the BACK hints
/// [F 0x1CB090..0x1CB480, 0x1CB490..0x1CB880].
fn edit_switch(b: &mut B<'_>, inp: &ScreenInputs<'_>, side: u8, x: i32, y: i32, r: u8) {
    let w = edit_frame(b, inp, side, x, y, r);
    let p = &inp.menu.players[usize::from(side)];
    let (shown, left, right) = if r == row::LEADER_ABILITY {
        (p.leader, b.d.not_use.clone(), b.d.use_.clone())
    } else {
        (p.display, b.d.hide.clone(), b.d.open.clone())
    };
    b.c(MenuCall::Gold { x: x + 0x46, y: y + 0x4D, size: 0, flags: 3, text: left, dim: shown != 0 });
    b.c(MenuCall::Gold { x: x + 0x8D, y: y + 0x4D, size: 0, flags: 3, text: right, dim: shown == 0 });
    let back = b.d.back.clone();
    hints(b, side, x - 0xE, y - 0xB, w + 0x16, &back, false);
}

/// 0x1CB890: the map list, the chosen map's terrain and its legend.
fn map_list(b: &mut B<'_>, inp: &ScreenInputs<'_>, x: i32, y: i32) {
    let m = inp.menu;
    b.panel(12, 3, 4, x + 0x89, y, -0xB2, -0x20);
    b.centred(cd::MSG_SELECT_MAP, 0x100, y + 5, 0x9A, 0x16);
    b.panel(0, -1, 11, x, y + 0x20, -0x1C4, -0xDE);
    b.sprite(196, x + 0x11, y + 0x25);
    b.sprite(260, x + 0x31, y + 0x27);
    b.panel(12, -1, 6, x + 0x11, y + 0x35, -0xAC, -0x7C);
    band(b, x, y);
    b.panel(-1, -1, 6, x + 0x11, y + 0x35, -0xAC, -0x7C);
    let n = m.maps.len().max(1);
    for k in 0..usize::from(cd::MAP_ROWS) {
        let e = m.maps[(usize::from(m.map_top) + k) % n];
        let message = match e >> 8 {
            0 => cd::MSG_USER_SETUP,
            1 => cd::MSG_USER_SETUP + 1,
            _ => cd::MSG_DEFAULT_MAP,
        };
        let ry = y + 0x3A + 24 * k as i32;
        if let Some((w, _)) = b.measure(message) {
            b.text(message, x + 0x1D, ry, B::fit(w, 0x70), 1.0);
        }
        // 0x1B7880(n 2, t1 0: pitch 20, t2 1: zero padded) [F 0x1CBD4C].
        b.c(MenuCall::DigitsPadded { x: x + 0x8D, y: ry, n: 2, value: i32::from(e & 0xFF), narrow: false, dim: k != 2 });
    }
    b.sprite(58, x + 0x13, y + 0x6D);
    b.c(MenuCall::SpriteFlipped { desc: 58, x: x + 0xB7, y: y + 0x6D });
    b.sprite(197, x + 0x11, y + 0xB5);
    b.sprite(261, x + 0x31, y + 0xB7);
    let chosen = m.chosen_entry();
    // 0x1CBF18 / 0x1CBF48: an own map's cells from its side's block.
    let cells = match chosen >> 8 {
        2.. => inp.default_maps.get(usize::from(chosen & 0xFF)).cloned(),
        side => inp.own_maps[usize::from(side & 1)].get(usize::from(chosen & 0xFF)).map(|m| m.to_vec()),
    };
    match &cells {
        Some(cells) => {
            for (i, &t) in cells.iter().enumerate().take(49) {
                b.sprite(270 + usize::from(t), x + 0xD0 + 16 * (i % 7) as i32, y + 0x3C + 16 * (i / 7) as i32);
            }
        }
        None => b.notes.push("the chosen map's terrain is missing".into()),
    }
    terrain_legend(b, x, y, cells.as_deref());
    b.c(MenuCall::Box { colour: 0, x: x + 0x12, y: y + 0xC4, wu: -0x1A0, hu: -0x34 });
    b.sprite(253, x + 0x61, y + 0xCB);
    b.sprite(252, x + 0x71, y + 0xCB);
    let (start, back) = (b.d.start.clone(), b.d.back.clone());
    b.gold(x + 0x71, y + 0xE6, 1, 2, &start);
    b.sprite(254, x + 0xDA, y + 0xCB);
    b.gold(x + 0xE2, y + 0xE6, 1, 2, &back);
}

/// The chosen row's band behind the list (0x206F90 sprite, colour
/// (0x80, 0, 0, 0x80) over (x + 0x11, y + 0x69)–(x + 0xC1, y + 0x81))
/// [F 0x1CBB64 / 0x1CCA94]; drawn as an untextured quad [H: its blend].
fn band(b: &mut B<'_>, x: i32, y: i32) {
    let c = [0x80, 0, 0, 0x80];
    let (x0, x1, y0, y1) = ((x + 0x11) as f32, (x + 0xC1) as f32, (y + 0x69) as f32, (y + 0x81) as f32);
    b.c(MenuCall::Quad { corners: [(x0, y0, c), (x1, y0, c), (x0, y1, c), (x1, y1, c)] });
}

/// The map list's terrain preview for another screen (Online Play's map
/// choice): the chosen map's 7 × 7 cells and the ten terrains' legend as
/// 0x1CB890 draws them, with the list's origin at (x, y) (the cells at
/// x + 0xD0, the legend up to x + 0x1B3); `None` (RANDOM) leaves the
/// cells empty and the counts at 0.
pub fn terrain_preview(d: &ScreenData, t: &HudTables, x: i32, y: i32, cells: Option<&[u8]>) -> (Vec<MenuCall>, Vec<String>) {
    let mut b = B { d, t, calls: Vec::new(), portraits: Vec::new(), notes: Vec::new() };
    if let Some(cells) = cells {
        for (i, &t) in cells.iter().enumerate().take(49) {
            b.sprite(270 + usize::from(t), x + 0xD0 + 16 * (i % 7) as i32, y + 0x3C + 16 * (i / 7) as i32);
        }
    }
    terrain_legend(&mut b, x, y, cells);
    (b.calls, b.notes)
}

/// The terrain preview's frame and the ten terrains' icons, gauges and
/// counts of the map lists (0x1CB890 / 0x1CC870).
fn terrain_legend(b: &mut B<'_>, x: i32, y: i32, cells: Option<&[u8]>) {
    b.panel(-1, 1, 1, x + 0xC9, y + 0x35, -0x76, -0x76);
    b.sprite(47, x + 0xE8, y + 0xA7);
    for k in 0..10u8 {
        let ky = y + 0x25 + 16 * i32::from(k);
        let count = cells.map_or(0, |c| c.iter().filter(|&&t| t == k).count());
        b.sprite(270 + usize::from(k), x + 0x14F, ky);
        b.panel(13, -1, 5, x + 0x15F, ky, -0x54, -0x10);
        let colours = b.d.terrain.gauges[usize::from(k)];
        roses_client::map_editor::gauge(&mut b.calls, &colours, x + 0x15F, ky, count as u8);
        b.panel(-1, -1, 5, x + 0x15F, ky, -0x54, -0x10);
        b.number(x + 0x189, ky + 2, 0, 2, 2, count as i32);
    }
}

/// 0x1CC870(side, x, y): Edit Map's own-map list ("1P User Setup" 0..4,
/// the middle row chosen: (0xA9C08C + 2) % 0xA9C08D) with the chosen map
/// of the pad side's block (0x213D70(0)) and its terrain counts
/// [F 0x1CC870..0x1CD61C].
fn own_map_list(b: &mut B<'_>, inp: &ScreenInputs<'_>, side: u8, x: i32, y: i32) {
    let m = inp.menu;
    let s = i32::from(side);
    b.panel(12, 3, 4, x + 0x7A, y, -0xD0, -0x20);
    b.centred(cd::ROW_LABEL_BASE + u16::from(row::EDIT_MAP), x + 0xE2, y + 5, 0x9A, 0x16);
    b.panel(s, -1, 11, x, y + 0x20, -0x1C4, -0xDE);
    b.panel(12, -1, 6, x + 0x11, y + 0x35, -0xAC, -0x7C);
    band(b, x, y);
    b.panel(-1, -1, 6, x + 0x11, y + 0x35, -0xAC, -0x7C);
    let n = m.own_count.max(1);
    for k in 0..cd::MAP_ROWS {
        let ry = y + 0x3A + 24 * i32::from(k);
        if let Some((w, _)) = b.measure(cd::MSG_USER_SETUP) {
            b.c(MenuCall::Text { line: 2, message: cd::MSG_USER_SETUP, x: x + 0x1D, y: ry, sx: B::fit(w, 0x70), sy: 1.0, lines: 1, dim: false });
        }
        let value = i32::from((m.own_top + k) % n);
        b.c(MenuCall::DigitsPadded { x: x + 0x8D, y: ry, n: 2, value, narrow: false, dim: k != 2 });
    }
    b.sprite(58, x + 0x13, y + 0x6D);
    b.c(MenuCall::SpriteFlipped { desc: 58, x: x + 0xB7, y: y + 0x6D });
    let chosen = usize::from((m.own_top + 2) % n);
    let cells = inp.own_maps[0].get(chosen).copied();
    match &cells {
        Some(cells) => {
            for (i, &t) in cells.iter().enumerate() {
                b.sprite(270 + usize::from(t), x + 0xD0 + 16 * (i % 7) as i32, y + 0x3C + 16 * (i / 7) as i32);
            }
        }
        None => b.notes.push("the pad side's own map is missing".into()),
    }
    terrain_legend(b, x, y, cells.as_ref().map(|c| &c[..]));
    b.c(MenuCall::Box { colour: s, x: x + 0x12, y: y + 0xC4, wu: -0x1A0, hu: -0x34 });
    b.sprite(253, x + 0x61, y + 0xCB);
    b.sprite(252, x + 0x71, y + 0xCB);
    let (edit, back) = (b.d.edit.clone(), b.d.back.clone());
    b.gold(x + 0x71, y + 0xE6, 1, 2, &edit);
    b.sprite(254, x + 0xDA, y + 0xCB);
    b.gold(x + 0xE2, y + 0xE6, 1, 2, &back);
}

/// 0x1CD620: 0x1B1830(colour 2, MSG 146, centre 0x100, top 0xB4, width
/// 0x100, min height 0x98, title "EXIT") and YES / NO at (0xC0 / 0x140,
/// 0x128), the unselected one in the 0x2BDCC4 colour.
fn exit_box(b: &mut B<'_>, yes: bool) {
    let (cx, top, width, min_h) = (0x100, 0xB4, 0x100, 0x98);
    let title = b.d.exit.clone();
    let (tw, th) = gold_measure(b.t, 2, &title);
    let (plate_w, plate_h) = (tw + 0x18, th + 0xC);
    let Some((w, lines)) = b.measure(MSG_EXIT_QUESTION) else { return };
    let h = 22 * lines;
    let min_w = w.max(plate_w) + 0x18;
    let height = plate_h + 0x18 + h + 0xC;
    let width = width.max(min_w);
    let inner_h = min_h.max(height);
    b.panel(13, -1, 1, cx - (width >> 1) - 4, top, -width, -inner_h);
    b.panel(2, -1, 11, cx - (plate_w >> 1), top + 0xC, -plate_w, -plate_h);
    b.gold(cx, top + 0x14, 2, 3, &title);
    b.c(MenuCall::Text { line: 1, message: MSG_EXIT_QUESTION, x: cx - (w >> 1), y: top + plate_h + 0x18, sx: 1.0, sy: 1.0, lines: lines as u8, dim: false });
    let yn = (b.d.yes.clone(), b.d.no.clone());
    b.c(MenuCall::Gold { x: 0xC0, y: 0x128, size: 2, flags: 3, text: yn.0, dim: !yes });
    b.c(MenuCall::Gold { x: 0x140, y: 0x128, size: 2, flags: 3, text: yn.1, dim: yes });
}

/// 0x1B82C0(size, flags 3): the proportional width and the glyph height.
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

/// Draw a screen into `target` (the 512×512 GS frame): the panels'
/// portraits are slots 0/1; each Select Deck portrait is drawn with its own
/// card loaded into slot 0. Returns what could not be drawn.
pub fn render(
    screen: &Screen,
    tables: &HudTables,
    yes_no: &YesNoTables,
    textures: &mut duel_hud::HudTextures,
    picmini: &[u8],
    target: &mut roses_client::gs::Target16,
) -> Vec<String> {
    let mut out = Vec::new();
    let panels = [screen.portraits[0].unwrap_or(0), screen.portraits.get(1).copied().flatten().unwrap_or(0)];
    let mut flush = |calls: &[MenuCall], leaders: [u16; 2], out: &mut Vec<String>| {
        if calls.is_empty() {
            return;
        }
        if let Err(e) = textures.set_leaders(picmini, leaders) {
            out.push(format!("portrait {leaders:?}: {e}"));
        }
        let frame = duel_hud::menu_frame(tables, yes_no, calls);
        for u in &frame.unsupported {
            out.push(format!("unsupported {:?}", u.kind));
        }
        for s in duel_hud::render_primitives(&frame.prims, tables, textures, target) {
            out.push(format!("primitive {} skipped: {}", s.index, s.reason));
        }
    };
    let mut start = 0;
    for (i, call) in screen.calls.iter().enumerate() {
        if let MenuCall::Portrait { slot, x, y, w, h, bright } = call {
            if *slot >= 2 {
                flush(&screen.calls[start..i], panels, &mut out);
                if let Some(card) = screen.portraits[usize::from(*slot)] {
                    let one = [MenuCall::Portrait { slot: 0, x: *x, y: *y, w: *w, h: *h, bright: *bright }];
                    flush(&one, [card, card], &mut out);
                }
                start = i + 1;
            }
        }
    }
    flush(&screen.calls[start..], panels, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use roses_client::custom_duel::{MenuPad, SetupBlocks};

    struct Env {
        current: [u8; 2],
        playable: [[bool; 3]; 2],
        maps: [bool; 2],
    }
    impl SetupBlocks for Env {
        fn current_deck(&self, side: u8) -> u8 {
            self.current[usize::from(side)]
        }
        fn set_current_deck(&mut self, side: u8, deck: u8) {
            self.current[usize::from(side)] = deck;
        }
        fn deck_playable(&self, side: u8, deck: u8) -> bool {
            self.playable[usize::from(side)][usize::from(deck)]
        }
        fn has_own_maps(&self, side: u8) -> bool {
            self.maps[usize::from(side)]
        }
    }

    /// One call as both sides log it: kind and integer fields.
    fn mine(c: &MenuCall) -> Option<(String, Vec<i64>)> {
        let v = |xs: &[i32]| xs.iter().map(|&x| i64::from(x)).collect();
        Some(match c {
            MenuCall::Panel { grad, deco, ptype, x, y, wu, hu, .. } => ("panel".into(), v(&[*grad, *deco, *ptype as i32, *x, *y, *wu, *hu])),
            MenuCall::Sprite { desc, x, y } | MenuCall::SpriteFlipped { desc, x, y } => ("sprite".into(), v(&[*desc as i32, *x, *y])),
            MenuCall::Gold { x, y, size, flags, .. } => ("font".into(), v(&[*x, *y, *size as i32, *flags as i32])),
            MenuCall::Number { x, y, size, flags, digits, value, .. } => ("number".into(), v(&[*x, *y, *size as i32, *flags as i32, *digits, *value])),
            MenuCall::Text { x, y, .. } => ("text".into(), v(&[*x, *y])),
            MenuCall::Portrait { x, y, w, h, .. } => ("portrait".into(), v(&[*x, *y, *w, *h])),
            MenuCall::Insignia { x, y, .. } => ("insignia".into(), v(&[*x, *y])),
            MenuCall::Box { x, y, wu, hu, .. } => ("f1b71d0".into(), v(&[*x, *y, *wu, *hu])),
            MenuCall::Digits { x, y, value, narrow, .. } => ("f1b7880".into(), v(&[*x, *y, *value, i32::from(*narrow), 0])),
            MenuCall::DigitsPadded { x, y, value, narrow, .. } => ("f1b7880".into(), v(&[*x, *y, *value, i32::from(*narrow), 1])),
            MenuCall::Tiles { x, y, .. } => ("f1b6ed0".into(), v(&[*x, *y])),
            MenuCall::Darken { .. } => ("f1b8e40".into(), vec![]),
            MenuCall::Quad { corners } => ("f206f90".into(), vec![corners[0].0 as i64, corners[0].1 as i64, corners[3].0 as i64, corners[3].1 as i64]),
            _ => return None,
        })
    }

    fn logged(c: &serde_json::Value) -> Option<(String, Vec<i64>)> {
        let ra = i64::from_str_radix(c["ra"].as_str()?.trim_start_matches("0x"), 16).ok()?;
        if !(0x1C6BC0..0x1CD750).contains(&ra) {
            return None;
        }
        let a: Vec<i64> = c["args"].as_array()?.iter().map(|x| x.as_i64().unwrap()).collect();
        let fn_ = c["fn"].as_str()?;
        Some(match fn_ {
            "panel" => (fn_.into(), a[1..8].to_vec()),
            "sprite" => (fn_.into(), vec![(a[1] - 0x28_F300) / 10, a[2] - 0x700, a[3] - 0x700]),
            "font" => (fn_.into(), a[1..5].to_vec()),
            "number" => (fn_.into(), a[1..7].to_vec()),
            "text" => (fn_.into(), a[2..4].to_vec()),
            "portrait" => (fn_.into(), a[2..6].to_vec()),
            "insignia" => (fn_.into(), a[1..3].to_vec()),
            "f1b71d0" => (fn_.into(), a[2..6].to_vec()),
            "f1b7880" => (fn_.into(), vec![a[1], a[2], a[4], i64::from(a[5] != 0), i64::from(a[6] != 0)]),
            "f1b6ed0" => (fn_.into(), a[1..3].to_vec()),
            "f1b8e40" => (fn_.into(), vec![]),
            // 0x206F90's packet: a sprite (+2 = 6) has its two corners at
            // +0x18 / +0x20, a quad its first and last at +0x18 / +0x48
            // (x, y in 1/16 pixel + 0x700).
            "f206f90" => {
                let hex = c["packet"].as_str()?;
                let b: Vec<u8> = (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()).collect();
                let h = |o: usize| i64::from(u16::from_le_bytes([b[o], b[o + 1]])) / 16 - 0x700;
                let last = if b[2] == 6 { 0x20 } else { 0x48 };
                (fn_.into(), vec![h(0x18), h(0x1A), h(last), h(last + 2)])
            }
            _ => return None,
        })
    }

    /// The builder against the interpreter's draw-call logs
    /// (`ROSES_CUSTOM_DUEL_CAPTURES` = the custom-duel captures folder,
    /// `ROSES_DUEL_IMPORT` = a duel import folder): every logged leaf call
    /// of the grid, the three vs-CPU sub-screens, the map list and the
    /// return-to-title box, in order, with its positions and values.
    #[test]
    fn private_screen_matches_the_emulated_draw() {
        let (Some(dir), Some(duel)) = (std::env::var_os("ROSES_CUSTOM_DUEL_CAPTURES"), std::env::var_os("ROSES_DUEL_IMPORT")) else {
            return;
        };
        let data = crate::duel_play::load(std::path::Path::new(&duel)).unwrap();
        let mut codes = std::collections::BTreeMap::new();
        for chunk in HudTables::required_messages().chunks(256) {
            for m in roses_assets::parse_pal_messages(&data.messages, chunk).unwrap().messages {
                codes.insert(m.message_id, m.expanded_codes);
            }
        }
        let extra: Vec<u16> = (4..6).chain(38..60).chain(83..96).chain(127..147).collect();
        for m in roses_assets::parse_pal_messages(&data.messages, &extra).unwrap().messages {
            codes.insert(m.message_id, m.expanded_codes);
        }
        let tables = HudTables::from_executable(&data.executable, codes).unwrap();
        let screen_data = ScreenData::from_executable(&data.executable).unwrap();
        let maps = roses_assets::parse_pal_rules(&data.executable).unwrap().maps;
        let default_maps: Vec<Vec<u8>> = (0..24).map(|i| maps[0x16 + i].cells.clone()).collect();
        let mut checked = 0;
        // Two players (selector 0: the READY box, Leader Ability and Card
        // Display): `ROSES_TWO_PLAYER_CAPTURES` = modes/two-player/captures.
        let two = std::env::var_os("ROSES_TWO_PLAYER_CAPTURES");
        let mut runs: Vec<(std::path::PathBuf, &str)> =
            ["d1-states.json", "d3.json", "d4.json"].iter().map(|n| (std::path::Path::new(&dir).join(n), *n)).collect();
        if let Some(two) = &two {
            runs.push((std::path::Path::new(two).join("d7-two-players.json"), "d7-two-players.json"));
        }
        // Own maps (Edit Map's list, own maps in the map list, both sides'):
        // `ROSES_MAP_EDITOR_CAPTURES` = modes/map-editor/captures.
        if let Some(me) = std::env::var_os("ROSES_MAP_EDITOR_CAPTURES") {
            for n in ["d5-ownlist.json", "d6-ownmaps-both.json"] {
                runs.push((std::path::Path::new(&me).join(n), n));
            }
        }
        for (path, name) in runs {
            let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let sel = v["selector"].as_u64().unwrap() as u8;
            let env = &v["env"];
            let mut blocks = Env {
                current: [0, 1].map(|s| env["current"][s].as_u64().unwrap() as u8),
                playable: [0, 1].map(|s| [0, 1, 2].map(|d| env["playable"][s][d].as_u64().unwrap() != 0)),
                maps: [0, 1].map(|s| env["maps"][s].as_u64().unwrap() != 0),
            };
            let own: [Vec<[u8; 49]>; 2] = [0, 1].map(|s| match env["own"][s].as_array() {
                Some(bytes) => {
                    let b: Vec<u8> = bytes.iter().map(|x| x.as_u64().unwrap() as u8).collect();
                    b.chunks(49).map(|m| m.try_into().unwrap()).collect()
                }
                None => Vec::new(),
            });
            let packets = v["calls"].as_array().unwrap().iter().any(|c| c.get("packet").is_some());
            let mut menu = CustomDuel::new(sel, &blocks);
            let frames = v["frames"].as_array().unwrap();
            let mut prev = [0u16; 2];
            for (k, f) in frames.iter().enumerate().skip(1) {
                let pad = [0, 1].map(|p| f["pad"][p].as_u64().unwrap() as u16);
                let pads = [0, 1].map(|p| MenuPad::press(pad[p] & !prev[p]));
                prev = pad;
                menu.update(pads, &mut blocks, true);
                let frame = (k - 1) as i64;
                let side = |s: u8| SideView {
                    name: match (s, sel) {
                        (0, _) => 4,
                        (_, 0) => 5,
                        _ => 141 + u16::from(sel),
                    },
                    leader: Some(if s == 1 && sel == 0 { Leader { card: 21, kind: 0, rank: 12 } } else { Leader { card: 392, kind: 8, rank: 1 } }),
                    deck: 0,
                    // 622-z0's second block (the two-player run's side 1):
                    // leader 21 at rank 12, deck cost 1748 [emu d7].
                    cost: if s == 1 && sel == 0 { 1748 } else { 822 },
                    deck_leaders: [Some(if s == 1 && sel == 0 { 21 } else { 392 }), None, None],
                };
                let inp = ScreenInputs { menu: &menu, sides: [side(0), side(1)], has_maps: blocks.maps, default_maps: &default_maps, own_maps: own.clone() };
                let screen = build(&screen_data, &tables, &inp);
                // 0x1B1830's own calls (the exit box) are not in the log.
                let mut in_box = false;
                let ours: Vec<_> = screen
                    .calls
                    .iter()
                    .filter(|c| {
                        match c {
                            MenuCall::Darken { .. } => {
                                in_box = true;
                                return true;
                            }
                            MenuCall::Gold { y: 0x128, .. } => in_box = false,
                            _ => {}
                        }
                        !in_box
                    })
                    .filter_map(mine)
                    // Older captures logged no 0x206F90 packets.
                    .filter(|(f, _)| packets || f != "f206f90")
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
                    assert_eq!(a, b, "{name} frame {frame} call {i} (state {:#x})", menu.players[0].state);
                }
                assert_eq!(ours.len(), theirs.len(), "{name} frame {frame} call count");
                checked += 1;
            }
        }
        eprintln!("{checked} frames match");
        assert!(checked > 20);
    }
}
