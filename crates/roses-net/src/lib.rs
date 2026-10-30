//! Online duel protocol for the first networked milestone (ONLINE_PLAN.md):
//! versioned handshake, action-level intentions with sequence numbers,
//! explicit acknowledgements and state revisions, per-seat filtered views
//! and events, and a transport trait (framed JSON over loopback TCP, or in
//! memory). The authoritative match lives in `roses-server`.
//!
//! Command boundary: clients send `roses_core::duel::DuelCommand` (open a
//! summon cell, choose hand cards, move a unit along a path, answer a
//! selection, end the turn, surrender), which `Duel::apply` validates
//! against the authoritative state. Pad keys and cursor state stay in the
//! client's input layer (`roses_core::duel::ui::DuelUi` already turns keys
//! into exactly these commands), so the server never trusts a client's
//! cursor, eligibility or outcome claims.

pub mod client;
pub mod custom_decks;
pub mod data;
pub mod deck;
pub mod filter;
pub mod friends;
pub mod lobby;
pub mod mirror;
pub mod presence;
pub mod protocol;
pub mod rules;
#[doc(hidden)]
pub mod synthetic;
/// Moved to `platform_net` (the platform's network layer); re-exported so
/// `roses_net::tls` / `roses_net::transport` keep working.
pub use platform_net::{tls, transport};

pub use deck::{check_deck, DeckProblem, DeckSubmission};
pub use filter::{
    player_view, player_view_with, seen_events, seen_events_for, seen_events_with, spectator_events, spectator_view, view_for,
    PlayerView, PublicDetail, PublicUnit, SeenEvent, Viewer,
};
pub use mirror::{hidden_card, mirror_duel};
pub use protocol::*;
pub use rules::RulesTables;

#[cfg(test)]
mod spectate_tests;
#[cfg(test)]
mod tests;
