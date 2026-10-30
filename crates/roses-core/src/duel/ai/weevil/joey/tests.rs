//! Private oracle comparison of Joey's module (cases `cases-opp15`, see
//! [`super::super::opponent_cases`]).

use super::Joey;

#[test]
fn private_joey_oracle_cases_match_the_original() {
    super::super::opponent_cases::check::<Joey>("cases-opp15", "joey");
}
