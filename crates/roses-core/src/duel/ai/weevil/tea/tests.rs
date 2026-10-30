//! Private oracle comparison of Tea's module (cases `cases-opp11`, see
//! [`super::super::opponent_cases`]).

use super::Tea;

#[test]
fn private_tea_oracle_cases_match_the_original() {
    super::super::opponent_cases::check::<Tea>("cases-opp11", "tea");
}
