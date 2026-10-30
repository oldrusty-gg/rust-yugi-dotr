//! `duel_multi` in a build without the `online` feature (the original game
//! only, docs/ADDITIONS.md): there are no N-player duels, so a
//! [`MultiPlay`] cannot be made and `DuelPlay::multi` is always `None`.

use std::convert::Infallible;

/// `DuelPlay::halted` after an N-player match (never without `online`).
pub const MULTI_DONE: &str = "the match is over";

/// The N-player state of a duel: uninhabited without `online`.
pub struct MultiPlay {
    pub seat: Option<u8>,
    pub cameras: Vec<crate::duel_play::SideCamera>,
    pub local: Option<Infallible>,
    never: Infallible,
}

impl MultiPlay {
    pub fn name(&self, _: u8) -> String {
        match self.never {}
    }
}
