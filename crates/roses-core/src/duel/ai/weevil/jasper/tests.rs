//! Private oracle comparison of Jasper Dice's module (cases `cases-opp17`,
//! see [`super::super::opponent_cases`]).

use super::Jasper;

#[test]
fn private_jasper_oracle_cases_match_the_original() {
    super::super::opponent_cases::check::<Jasper>("cases-opp17", "jasper");
}
