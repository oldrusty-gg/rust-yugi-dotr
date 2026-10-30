//! Private oracle comparison of Yugi's module (cases `cases-opp19`, see
//! [`super::super::opponent_cases`]).

use super::Yugi;

#[test]
fn private_yugi_oracle_cases_match_the_original() {
    super::super::opponent_cases::check::<Yugi>("cases-opp19", "yugi");
}
