//! `duel_online` in a build without the `online` feature (the original
//! game only, docs/ADDITIONS.md): no network code is compiled in. The types
//! the duel keeps for an online match exist so the offline code reads the
//! same, but an [`OnlineLink`] cannot be made, so `DuelPlay::online` is
//! always `None` and every online branch is dead.

use roses_core::duel::ui::DuelUi;
use roses_core::duel::{Catalogue, Duel};
use std::convert::Infallible;

/// The opponent byte of an online duel (never used without `online`).
pub const ONLINE_OPPONENT: u8 = 101;

/// What the server sent this seat (no value exists without `online`).
#[derive(Clone, Debug)]
pub enum PlayerView {}

/// The seat's view of the match.
#[derive(Clone, Debug)]
pub struct ClientState {
    pub revision: u64,
    pub view: Option<PlayerView>,
}

/// The match's Leader Ability / Card Display switches.
#[derive(Clone, Copy, Debug, Default)]
pub struct MatchOptions {
    pub leader_ability: [bool; 2],
    pub card_display_open: [bool; 2],
}

impl MatchOptions {
    pub fn words(&self) -> [u32; 2] {
        let bits = |b: [bool; 2]| u32::from(b[0]) | u32::from(b[1]) << 1;
        [bits(self.leader_ability), bits(self.card_display_open)]
    }
}

/// A server connection: uninhabited without `online`.
pub struct OnlineLink {
    pub state: ClientState,
    pub seat: u8,
    pub shown_revision: u64,
    pub hidden: u16,
    pub note: Option<String>,
    pub spectator: bool,
    pub sent_away: Option<String>,
    never: Infallible,
}

/// `DuelPlay::halted` when a spectator leaves (never without `online`).
pub const STOPPED_WATCHING: &str = "you stopped watching";

impl OnlineLink {
    pub fn take_snapshot(&mut self, _: &Catalogue) -> Option<Result<(Duel, u64), String>> {
        match self.never {}
    }
    pub fn pump(&mut self) {
        match self.never {}
    }
    pub fn may_act(&self) -> bool {
        match self.never {}
    }
    pub fn ended(&self) -> bool {
        match self.never {}
    }
    pub fn options(&self) -> MatchOptions {
        match self.never {}
    }
    pub fn tick(&mut self) {
        match self.never {}
    }
    pub fn forfeit(&mut self) {
        match self.never {}
    }
    pub fn watchers(&self) -> u16 {
        match self.never {}
    }
}

/// Leave the input state on the field after a resynchronisation (only an
/// online duel resynchronises; nothing to do without `online`).
pub fn settle_input(_: &mut DuelUi, _: &Duel, _: bool) {}

/// The pad-level computer player of an online seat (a test aid); without
/// `online` there is never a view for it to read.
#[derive(Default)]
pub struct PadBot;

impl PadBot {
    pub fn new() -> Self {
        Self
    }
    pub fn key(&mut self, _: &Duel, _: &DuelUi, view: &PlayerView, _: u64, _: bool, _: &Catalogue) -> u16 {
        match *view {}
    }
}
