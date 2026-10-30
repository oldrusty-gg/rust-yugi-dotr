//! Private oracle comparison of Mako's module (cases `cases-opp14`, see
//! [`super::super::opponent_cases`]).

use super::Mako;

#[test]
fn private_mako_oracle_cases_match_the_original() {
    super::super::opponent_cases::check::<Mako>("cases-opp14", "mako");
}
