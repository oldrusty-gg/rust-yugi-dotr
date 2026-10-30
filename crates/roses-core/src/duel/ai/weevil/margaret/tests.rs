//! Private oracle comparison of Margaret Mai Beaufort's module (cases
//! `cases-opp13`, see [`super::super::opponent_cases`]).

use super::Margaret;

#[test]
fn private_margaret_oracle_cases_match_the_original() {
    super::super::opponent_cases::check::<Margaret>("cases-opp13", "margaret");
}
