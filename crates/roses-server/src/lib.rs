//! Headless authoritative duel server (ONLINE_PLAN.md, first online
//! milestone). [`online::OnlineMatch`] owns the `roses_core` duel and is a
//! pure tick-driven state machine; [`serve`] attaches transport connections
//! to it; [`play`] is a client runner (computer-driven or scripted) used by
//! the client binary and the tests.

pub mod accounts;
pub mod friends_cli;
pub mod hub;
pub mod loadtest;
pub mod lobby;
pub mod multi;
pub mod name_filter;
pub mod online;
pub mod play;
pub mod replay;
pub mod serve;
pub mod tls;

pub use online::OnlineMatch;
pub use replay::{replay, state_hash, MatchSetup, Replay};

/// `N` bytes from the operating system's CSPRNG (`getrandom`: BCryptGenRandom
/// / ProcessPrng on Windows, getrandom(2) on Linux). A failing OS source is
/// fatal: tokens and seeds must never fall back to something guessable.
fn os_random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::getrandom(&mut bytes).expect("the operating system's random source failed");
    bytes
}

/// 64 unpredictable bits from the OS CSPRNG (match seeds and ids).
pub fn random_u64() -> u64 {
    u64::from_le_bytes(os_random())
}

/// A secret resume token: 256 bits from the OS CSPRNG, hex.
pub fn secret_token() -> String {
    os_random::<32>().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod friends_tests;
#[cfg(test)]
mod spectate_tests;
#[cfg(test)]
mod multi_tests;
#[cfg(test)]
mod seed_tests;
