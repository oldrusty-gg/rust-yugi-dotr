//! Private oracle comparison of the generic module as Shadi's (cases
//! `cases-opp16`, see [`super::super::opponent_cases`]).

use super::Shadi;

#[test]
fn private_shadi_oracle_cases_match_the_original() {
    super::super::opponent_cases::check::<Shadi>("cases-opp16", "shadi");
}

/// Custom Duel's computer: the same module with the VS control byte at
/// side +0x33C and the VS decks (`tools/gen_vs.py`: 25 deck 0x32, 22 with a
/// starter deck as the mirror of the player's block, 23 deck 0x30, 24 deck
/// 0x31). Directories that were not generated are skipped.
#[test]
fn private_vs_oracle_cases_match_the_original() {
    let Some(root) = std::env::var_os("ROSES_WEEVIL_ORACLE") else {
        return;
    };
    for dir in ["cases-vs25-32", "cases-vs22-9", "cases-vs23-30", "cases-vs24-31"] {
        if std::path::Path::new(&root).join(dir).is_dir() {
            super::super::opponent_cases::check::<Shadi>(dir, dir);
        }
    }
}
