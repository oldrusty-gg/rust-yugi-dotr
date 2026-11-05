//! The map's two system dialogs (Claude, 5 Oct 2026; notes and oracles in
//! the project notes):
//!
//! - Select → "Are you sure you wish to return to Title Screen?" (MSG 146),
//!   W+5440 / W+5441 at 0x2378EC..0x237A98 [F]: overlay slot 3 kind 7
//!   (0x1E5540(7, 0, y, 146), D35 = 0: NO preselected) slides up from
//!   y 0x200 to 0 in 8 updates, 0x1E56B0 asks YES / NO, YES pops the mode
//!   (0x1A9290, the title) every update after, NO / Circle slides it down
//!   again [emu captures/select-*.json].
//! - SYSTEM icon 2 → the OPTION panel, W+543E / W+543F at 0x237570..0x2376E0
//!   [F]: overlay slot 0 kind 7 (0x1E5270(SAVE[0], 0, y): D54 = side, D36
//!   bits 0..2 = the block's options, D35 = row 0) slides up the same way;
//!   0x1E5320 takes Up/Down (repeat: the row, two rows), Left/Right (bit
//!   row + 1 set / cleared), Cross/Square (write both blocks, SE 4) and
//!   Circle (SE 5, nothing written); then it slides down in 7 updates and
//!   goes off on the 8th [emu captures/options-toggle.json].
//!
//! While either is up the map takes no other input (the cursor stays) [emu].
use super::*;

/// MSG 146 "Are you sure you wish to return to Title Screen?".
pub const RETURN_TO_TITLE_MESSAGE: u16 = 146;
/// Updates of the slide (W+5441 / W+543F count to 8).
pub const DIALOG_SLIDE_UPDATES: u8 = 8;
/// y per slide update: 0x200 − 0x40·n.
pub const DIALOG_SLIDE_STEP: i16 = 0x40;
pub const DIALOG_HIDDEN_Y: i16 = 0x200;
/// SE 3 (cursor), 4 (confirm), 5 (cancel) of 0x1E56B0 / 0x1E5320.
pub const SFX_CURSOR: u16 = 3;
pub const SFX_CONFIRM: u16 = 4;
pub const SFX_CANCEL: u16 = 5;
/// SE 2: L1 / R1 at Stonehenge (0x2389C8 / 0x238A38).
pub const SFX_FLIP: u16 = 2;
/// The panel's two rows: row r toggles D36 bit r + 1 (Monster Battle,
/// Sound); bit 0 (card-name notation) has no row in PAL [F 0x1E5360..].
pub const OPTION_ROWS: u8 = 2;

/// New presses this update for the system dialogs, plus Up/Down as
/// 0x203380 mode 2 (the repeat) and L1/R1 (Stonehenge's opponent).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuPad {
    pub select: bool,
    pub cross: bool,
    pub square: bool,
    pub circle: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub l1: bool,
    pub r1: bool,
}

impl MenuPad {
    pub const NONE: MenuPad = MenuPad {
        select: false,
        cross: false,
        square: false,
        circle: false,
        left: false,
        right: false,
        up: false,
        down: false,
        l1: false,
        r1: false,
    };
}

/// W+5440 1 opening, 2 asking, 3 YES (pop), 4 closing; W+5441 the slide.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TitleDialog {
    pub state: u8,
    pub slide: u8,
    /// D35: YES selected.
    pub yes: bool,
}

impl TitleDialog {
    /// The box's y (overlay slot 3, added to its centred top).
    pub fn y(&self) -> i16 {
        DIALOG_HIDDEN_Y - DIALOG_SLIDE_STEP * i16::from(self.slide)
    }
}

/// W+543E 1 opening, 2 open, 3 closing; W+543F the slide; D35 the row,
/// D36 the bits, D54 the side (the panel's colour).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OptionsPanel {
    pub state: u8,
    pub slide: u8,
    pub row: u8,
    pub bits: u8,
    pub side: u8,
}

impl OptionsPanel {
    /// The panel's y offset (slot 0's 0xC9C, used raw by its draw).
    pub fn y(&self) -> i16 {
        DIALOG_HIDDEN_Y - DIALOG_SLIDE_STEP * i16::from(self.slide)
    }
}

impl MapModel {
    /// One update of whichever system dialog is up; true while one is (the
    /// rest of the free cursor's update does not run).
    pub(super) fn system_dialogs(&mut self, menu: MenuPad, events: &mut Vec<MapEvent>) -> bool {
        if let Some(mut panel) = self.options.take() {
            self.options_update(&mut panel, menu, events);
            if panel.state != 0 {
                self.options = Some(panel);
            }
            return true;
        }
        if let Some(mut dialog) = self.title_dialog.take() {
            self.title_dialog_update(&mut dialog, menu, events);
            if dialog.state != 0 {
                self.title_dialog = Some(dialog);
            }
            return true;
        }
        false
    }

    /// Select with no icon panel up (0x23793C): SE 4, the box at y 0x200.
    pub(super) fn open_title_dialog(&mut self, events: &mut Vec<MapEvent>) {
        events.push(MapEvent::Sfx(SFX_CONFIRM));
        self.title_dialog = Some(TitleDialog { state: 1, slide: 0, yes: false });
    }

    /// × on SYSTEM icon 2 (0x2375A0): SE 4, 0x1E5270(SAVE[0], 0, 0x200).
    pub(super) fn open_options(&mut self, events: &mut Vec<MapEvent>) {
        events.push(MapEvent::Sfx(SFX_CONFIRM));
        self.options = Some(OptionsPanel {
            state: 1,
            slide: 0,
            row: 0,
            bits: self.option_bits & crate::game_options::DEFAULT_BITS,
            side: self.save.side,
        });
    }

    fn title_dialog_update(&mut self, d: &mut TitleDialog, menu: MenuPad, events: &mut Vec<MapEvent>) {
        match d.state {
            1 => {
                d.slide += 1;
                if d.slide >= DIALOG_SLIDE_UPDATES {
                    d.slide = DIALOG_SLIDE_UPDATES;
                    d.state = 2;
                }
            }
            2 => match yes_no(&mut d.yes, menu, events) {
                Some(true) => d.state = 3,
                Some(false) => d.state = 4,
                None => {}
            },
            3 => {
                // 0x1A9290 every update until the mode switch happens.
                self.title_request = true;
                events.push(MapEvent::ReturnToTitle);
            }
            _ => {
                d.slide = d.slide.saturating_sub(1);
                if d.slide == 0 {
                    d.state = 0;
                }
            }
        }
    }

    fn options_update(&mut self, p: &mut OptionsPanel, menu: MenuPad, events: &mut Vec<MapEvent>) {
        match p.state {
            1 => {
                p.slide += 1;
                if p.slide >= DIALOG_SLIDE_UPDATES {
                    p.slide = DIALOG_SLIDE_UPDATES;
                    p.state = 2;
                }
            }
            2 => {
                // 0x1E5320: Up|Down (0x5000, repeat) first, then Left|Right
                // (0xA000), then Cross|Square (0xC0), then Circle (0x20).
                if menu.up || menu.down {
                    let before = p.row;
                    // Up: (row + 1) mod 2; Down: (row + 3) mod 2.
                    p.row = if menu.up { (p.row + 1) % OPTION_ROWS } else { (p.row + 3) % OPTION_ROWS };
                    if p.row != before {
                        events.push(MapEvent::Sfx(SFX_CURSOR));
                    }
                } else if menu.left || menu.right {
                    let before = p.bits;
                    let bit = 1 << (p.row + 1);
                    if menu.left {
                        p.bits |= bit;
                    } else {
                        p.bits &= !bit;
                    }
                    if p.bits != before {
                        events.push(MapEvent::Sfx(SFX_CURSOR));
                    }
                } else if menu.cross || menu.square {
                    // 0x214340(0, i, ·) and (1, i, ·) for i = 0..2, then SE 4.
                    self.option_bits = p.bits;
                    events.push(MapEvent::OptionsSaved { bits: p.bits });
                    events.push(MapEvent::Sfx(SFX_CONFIRM));
                    p.state = 3;
                } else if menu.circle {
                    events.push(MapEvent::Sfx(SFX_CANCEL));
                    p.state = 3;
                }
            }
            _ => {
                // 0x237664: W+543F − 1; at 0 the panel goes off (0x1E5530).
                p.slide = p.slide.saturating_sub(1);
                if p.slide == 0 {
                    p.state = 0;
                }
            }
        }
    }

    /// L1 / R1 over Stonehenge (0x23894C..0x238AA4): C9 − 1 / + 1, wrapping
    /// over 0..=2 on the free-duel map (Seto, the red and the white
    /// Manawyddan) and 0..=1 on the red side once Stonehenge is beaten;
    /// W+53C3 = 1 slides the hover panel out, and when it is in again
    /// W+53C4 = C9 picks the new opponent (`hud_update`) [F, emu
    /// captures/free-duel-oracle.json].
    pub(super) fn stonehenge_flip(&mut self, menu: MenuPad, events: &mut Vec<MapEvent>) {
        if !(menu.l1 || menu.r1)
            || self.flip
            || self.hovered_icon.is_some()
            || self.cursor_mode != 2
            || self.save.hovered != STONEHENGE_NODE
        {
            return;
        }
        let modulus: i8 = if self.free_duel {
            3
        } else if self.save.side == 0 && self.save.state(STONEHENGE_NODE) >= STATE_BEATEN {
            2
        } else {
            return;
        };
        events.push(MapEvent::Sfx(SFX_FLIP));
        let c9 = self.save.c9 as i8 + if menu.l1 { -1 } else { 1 };
        self.save.c9 = c9.rem_euclid(modulus) as u8;
        self.flip = true;
    }
}

/// 0x1E56B0: Left|Right (0xA000) set D35 (Left = YES) with SE 3 on a
/// change; Cross|Square (0xC0) answer with SE 4; Circle (0x20) is NO
/// with SE 5.
fn yes_no(yes: &mut bool, menu: MenuPad, events: &mut Vec<MapEvent>) -> Option<bool> {
    if menu.left || menu.right {
        let before = *yes;
        *yes = menu.left;
        if *yes != before {
            events.push(MapEvent::Sfx(SFX_CURSOR));
        }
        return None;
    }
    if menu.cross || menu.square {
        events.push(MapEvent::Sfx(SFX_CONFIRM));
        return Some(*yes);
    }
    if menu.circle {
        events.push(MapEvent::Sfx(SFX_CANCEL));
        return Some(false);
    }
    None
}
