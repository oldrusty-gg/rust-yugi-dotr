//! Private oracle comparison of Bakura's module (cases `cases-opp18`, see
//! [`super::super::opponent_cases`]).

use super::Bakura;

#[test]
fn private_bakura_oracle_cases_match_the_original() {
    super::super::opponent_cases::check::<Bakura>("cases-opp18", "bakura");
}
