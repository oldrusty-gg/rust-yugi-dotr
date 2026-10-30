//! Private oracle comparison of T. Tristan Grey's module (cases
//! `cases-opp12`, see [`super::super::opponent_cases`]).

use super::Tristan;

#[test]
fn private_tristan_oracle_cases_match_the_original() {
    super::super::opponent_cases::check::<Tristan>("cases-opp12", "tristan");
}
