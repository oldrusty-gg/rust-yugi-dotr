//! SYSTEM icon 3: the tutorial's lesson menu ("HELP"), W+543E / W+543F /
//! W+546D at 0x2376E4..0x2378B4, drawn by 0x23B770; private notes in
//! the project notes [F, emu].
//!
//! × on the icon (state 0): SE 4, W+543E = 1, W+543F = 0, W+546D = 0. State
//! 1 slides the window in over 8 updates (W+543F to 8; the window's y is
//! 0x200 − W+543F·0x40), then state 2: Circle (new press) → SE 5, state 10;
//! else Cross → SE 1, state 3, and 0x23B710 starts the lesson (0x312B78 = 1,
//! the player's card block backed up to 0x1E301E0, mode 1 pushed; the duel
//! mode's 0x23B3B0 picks controls 32..35 for rows 0..3, 36 otherwise, and
//! the map's mode switch restores the block at 0x21D14C on the way back);
//! while in states 1..2, Up / Down (0x203380 mode 2, the repeat) move
//! W+546D over 0..4 with wrap-around, SE 3 each. State 10 slides it out
//! over 8 updates and clears W+543E. Rows: MSG 4087..4091 ("Duel Basics" …
//! "Duel-related Functions", 0x245BB0 with 0xFF7 + row) [F].
use super::*;
use super::system::{SFX_CANCEL, SFX_CONFIRM, SFX_CURSOR};

/// The menu's rows (W+546D 0..=4).
pub const TUTORIAL_ROWS: u8 = 5;
/// MSG id of row 0's label; row r shows `TUTORIAL_LABEL_BASE + r`.
pub const TUTORIAL_LABEL_BASE: u16 = 4087;
/// SE 1: the lesson is chosen (0x2377D4).
pub const SFX_LESSON: u16 = 1;

/// W+543E (1 opening, 2 open, 3 chosen, 10 closing), W+543F the slide,
/// W+546D the row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TutorialMenu {
    pub state: u8,
    pub slide: u8,
    pub row: u8,
}

impl TutorialMenu {
    /// The window's y offset (0x23B7F4: 0x200 − W+543F·0x40).
    pub fn y(&self) -> i16 {
        DIALOG_HIDDEN_Y - DIALOG_SLIDE_STEP * i16::from(self.slide)
    }

    /// The label of `row`.
    pub fn label(row: u8) -> u16 {
        TUTORIAL_LABEL_BASE + u16::from(row)
    }
}

impl MapModel {
    /// × on SYSTEM icon 3 (0x23771C): SE 4, the menu at row 0.
    pub(super) fn open_tutorial_menu(&mut self, events: &mut Vec<MapEvent>) {
        events.push(MapEvent::Sfx(SFX_CONFIRM));
        self.tutorial_menu = Some(TutorialMenu {
            state: 1,
            slide: 0,
            row: 0,
        });
    }

    /// One update of the menu; true while it is up (the free cursor's
    /// update does not run).
    pub(super) fn tutorial_menu_update(&mut self, menu: MenuPad, events: &mut Vec<MapEvent>) -> bool {
        let Some(mut m) = self.tutorial_menu.take() else {
            return false;
        };
        match m.state {
            1 => {
                // 0x237758: W+543F + 1; at 8 the menu takes input.
                m.slide += 1;
                if m.slide >= DIALOG_SLIDE_UPDATES {
                    m.slide = DIALOG_SLIDE_UPDATES;
                    m.state = 2;
                }
            }
            2 => {
                if menu.circle {
                    events.push(MapEvent::Sfx(SFX_CANCEL));
                    m.state = 10;
                } else if menu.cross {
                    events.push(MapEvent::Sfx(SFX_LESSON));
                    m.state = 3;
                    // 0x23B710: the lesson's duel mode is pushed.
                    self.tutorial_request = Some(m.row);
                    events.push(MapEvent::TutorialLesson { row: m.row });
                }
            }
            3 => {
                // The mode switch is pending; the request stays up.
                self.tutorial_request = Some(m.row);
            }
            _ => {
                // 0x2378B8: W+543F − 1; at 0 the menu is gone.
                m.slide = m.slide.saturating_sub(1);
                if m.slide == 0 {
                    m.state = 0;
                }
            }
        }
        // 0x23780C: the rows move in states 1..2 (not on the update a
        // choice was made).
        if m.state < 3 && (menu.up || menu.down) {
            events.push(MapEvent::Sfx(SFX_CURSOR));
            m.row = if menu.up {
                (m.row + TUTORIAL_ROWS - 1) % TUTORIAL_ROWS
            } else {
                (m.row + 1) % TUTORIAL_ROWS
            };
        }
        if m.state != 0 {
            self.tutorial_menu = Some(m);
        }
        true
    }

    /// The map comes back after the lesson (the original's mode pop and
    /// 0x21D14C, which restores the card block): the menu was left in
    /// state 3; the port closes it and drops the request.
    pub fn tutorial_returned(&mut self) {
        self.tutorial_menu = None;
        self.tutorial_request = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad() -> MenuPad {
        MenuPad::default()
    }

    #[test]
    fn the_menu_slides_in_walks_its_five_rows_and_starts_a_lesson() {
        let mut m = MapModel::resume(super::super::tests::fixture(), CampaignSave::before_landing()).unwrap();
        let mut events = Vec::new();
        m.open_tutorial_menu(&mut events);
        assert_eq!(events, vec![MapEvent::Sfx(SFX_CONFIRM)]);
        for i in 1..=8u8 {
            assert!(m.tutorial_menu_update(pad(), &mut events));
            assert_eq!(m.tutorial_menu.as_ref().unwrap().slide, i);
        }
        assert_eq!(m.tutorial_menu.as_ref().unwrap().state, 2);
        assert_eq!(m.tutorial_menu.as_ref().unwrap().y(), 0);
        // Up from row 0 wraps to 4; Down from 4 wraps to 0.
        m.tutorial_menu_update(MenuPad { up: true, ..pad() }, &mut events);
        assert_eq!(m.tutorial_menu.as_ref().unwrap().row, 4);
        m.tutorial_menu_update(MenuPad { down: true, ..pad() }, &mut events);
        m.tutorial_menu_update(MenuPad { down: true, ..pad() }, &mut events);
        m.tutorial_menu_update(MenuPad { down: true, ..pad() }, &mut events);
        m.tutorial_menu_update(MenuPad { down: true, ..pad() }, &mut events);
        assert_eq!(m.tutorial_menu.as_ref().unwrap().row, 3);
        events.clear();
        m.tutorial_menu_update(MenuPad { cross: true, ..pad() }, &mut events);
        assert_eq!(
            events,
            vec![MapEvent::Sfx(SFX_LESSON), MapEvent::TutorialLesson { row: 3 }]
        );
        assert_eq!(m.tutorial_request, Some(3));
        assert_eq!(
            roses_core::duel::script::Lesson::from_menu_row(3).controller(),
            35
        );
        m.tutorial_returned();
        assert!(m.tutorial_menu.is_none() && m.tutorial_request.is_none());
        assert_eq!(TutorialMenu::label(4), 4091);
    }

    #[test]
    fn circle_closes_the_menu_over_eight_updates() {
        let mut m = MapModel::resume(super::super::tests::fixture(), CampaignSave::before_landing()).unwrap();
        let mut events = Vec::new();
        m.open_tutorial_menu(&mut events);
        for _ in 0..8 {
            m.tutorial_menu_update(pad(), &mut events);
        }
        events.clear();
        m.tutorial_menu_update(MenuPad { circle: true, ..pad() }, &mut events);
        assert_eq!(events, vec![MapEvent::Sfx(SFX_CANCEL)]);
        for _ in 0..7 {
            assert!(m.tutorial_menu_update(pad(), &mut events));
            assert!(m.tutorial_menu.is_some());
        }
        assert!(m.tutorial_menu_update(pad(), &mut events));
        assert!(m.tutorial_menu.is_none());
        assert!(!m.tutorial_menu_update(pad(), &mut events));
        assert_eq!(m.tutorial_request, None);
    }
}
