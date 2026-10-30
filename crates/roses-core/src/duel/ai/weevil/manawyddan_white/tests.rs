//! Private oracle comparison of white Manawyddan's module (cases
//! `cases-opp21`, see [`super::super::opponent_cases`]).

use super::ManawyddanWhite;

#[test]
fn private_manawyddan_white_oracle_cases_match_the_original() {
    super::super::opponent_cases::check::<ManawyddanWhite>("cases-opp21", "manawyddan21");
}
