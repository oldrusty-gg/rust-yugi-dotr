//! The tournament rules as pure functions: sizing, seeding and byes,
//! walkovers and no-shows, the preview, the draw's timing, the create
//! limits, the weekly time and the quick tournament.

use super::*;
use roses_net::lobby::TournamentState;

fn names(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("Player{i}")).collect()
}

const START: i64 = 1_796_410_800; // Friday 4 Dec 2026 19:00 UTC

#[test]
fn the_bracket_is_the_next_power_of_two_at_least_four() {
    for (players, size) in [(0, 4), (1, 4), (2, 4), (3, 4), (4, 4), (5, 8), (8, 8), (9, 16), (15, 16), (16, 16), (17, 32), (33, 64), (65, 128), (128, 128)] {
        assert_eq!(bracket_size(players), size, "{players} players");
    }
}

#[test]
fn seeds_follow_the_standard_order() {
    assert_eq!(seed_slots(4), [1, 4, 2, 3]);
    assert_eq!(seed_slots(8), [1, 8, 4, 5, 2, 7, 3, 6]);
    for size in [4, 8, 16, 32, 64, 128] {
        let slots = seed_slots(size);
        let mut sorted = slots.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (1..=size).collect::<Vec<_>>());
        // Each first-round pair adds up to size + 1: 1 v size, 2 v size - 1.
        assert!(slots.chunks(2).all(|p| p[0] + p[1] == size + 1));
    }
}

/// The byes of a draw of `n`: how many, and that no bye meets a bye or one
/// bye player meets another in the first round.
fn check_byes(n: usize) -> Bracket {
    let b = Bracket::draw(&names(n), 7);
    let size = bracket_size(n);
    assert_eq!(b.size(), size, "{n}");
    assert_eq!(b.rounds.len(), size.trailing_zeros() as usize, "{n}: rounds");
    let first = &b.rounds[0];
    let byes = first.iter().filter(|g| g.sides.contains(&Side::Bye)).count();
    assert_eq!(byes, size - n, "{n}: one bye per empty slot");
    assert!(first.iter().all(|g| g.sides != [Side::Bye, Side::Bye]), "{n}: two byes never meet");
    // Every bye is a walkover for its player, carried into round 2.
    for (i, g) in first.iter().enumerate() {
        if g.sides.contains(&Side::Bye) {
            assert_eq!(g.how, Some(WinHow::Bye), "{n}");
            let through = g.sides[usize::from(g.winner.unwrap())];
            assert!(matches!(through, Side::Player { .. }));
            if b.rounds.len() > 1 {
                assert_eq!(b.rounds[1][i / 2].sides[i % 2], through, "{n}: the bye player is in round 2");
            }
        } else {
            assert!(g.open(), "{n}: a real first-round match waits to be played");
        }
    }
    b
}

#[test]
fn byes_fill_the_empty_slots_for_4_5_15_16_17_and_128_players() {
    for n in [4, 5, 15, 16, 17, 128] {
        check_byes(n);
    }
    // 15 players: a 16 bracket with one bye, to seed 1.
    let b = check_byes(15);
    let bye_game = b.rounds[0].iter().find(|g| g.sides.contains(&Side::Bye)).unwrap();
    assert_eq!(bye_game.sides[0], Side::Player { index: 0 }, "the top seed has the bye");
    assert_eq!(b.open_games().len(), 7);
    // 5 players: an 8 bracket, seeds 1-3 have byes, 4 v 5 plays; seeds 2
    // and 3 (both through on byes) already meet in round 2.
    let b = check_byes(5);
    assert_eq!(b.open_games(), vec![(0, 1), (1, 1)]);
    // 17 players: a 32 bracket, 15 byes, one first-round match.
    assert_eq!(check_byes(17).open_games().len(), 1 + 7);
}

#[test]
fn the_draw_is_random_but_repeats_from_its_logged_seed() {
    let players = names(16);
    let a = Bracket::draw(&players, 42);
    assert_eq!(a, Bracket::draw(&players, 42), "the same seed gives the same draw");
    assert_ne!(a.players, Bracket::draw(&players, 43).players, "another seed, another draw");
    let mut sorted = a.players.clone();
    sorted.sort();
    let mut expected = players.clone();
    expected.sort();
    assert_eq!(sorted, expected, "every player once");
}

#[test]
fn winners_go_through_to_a_champion() {
    let mut b = Bracket::draw(&names(4), 1);
    assert_eq!(b.open_games(), vec![(0, 0), (0, 1)]);
    b.decide(0, 0, 1, WinHow::Played);
    b.decide(0, 1, 0, WinHow::Played);
    assert_eq!(b.open_games(), vec![(1, 0)]);
    let finalist = b.rounds[1][0].player(1).unwrap();
    b.decide(1, 0, 1, WinHow::Played);
    assert!(b.done());
    assert_eq!(b.champion(), Some(finalist));
    assert!(b.alive(finalist));
    assert_eq!((0..4).filter(|&i| b.alive(i)).count(), 1);
}

#[test]
fn a_no_show_loses_by_walkover_and_two_no_shows_send_the_higher_seed_through() {
    let b = Bracket::draw(&names(4), 3);
    let mut g = b.rounds[0][0].clone();
    g.ready = [true, false];
    assert_eq!(timeout_winner(&g), Some((0, WinHow::Walkover)));
    g.ready = [false, true];
    assert_eq!(timeout_winner(&g), Some((1, WinHow::Walkover)));
    g.ready = [true, true];
    assert_eq!(timeout_winner(&g), None, "both ready: they wait for each other");
    g.ready = [false, false];
    let higher = if g.player(0) < g.player(1) { 0 } else { 1 };
    assert_eq!(timeout_winner(&g), Some((higher, WinHow::NoShow)));
    // Seed 1 sits in slot 0 of the first match.
    assert_eq!(g.player(0), Some(0));
    assert_eq!(timeout_winner(&g), Some((0, WinHow::NoShow)));
}

#[test]
fn the_preview_grows_and_shrinks_as_players_sign_up_and_leave() {
    let mut t = Tournament::new("Friday Cup", None, START, 128, false, START - 10 * 86_400);
    let now = START - 3 * 86_400;
    let preview = |t: &Tournament| {
        let v = t.view(None);
        assert!(v.preview);
        v
    };
    assert_eq!(preview(&t).size, 4, "nobody yet: the smallest bracket, all byes");
    assert!(preview(&t).rounds[0].iter().all(|g| g.sides.iter().all(|s| *s == BracketSide::Bye)));
    for (i, n) in names(9).iter().enumerate() {
        t.sign_up(n, now + i as i64).unwrap();
        assert_eq!(usize::from(preview(&t).size), bracket_size(i + 1), "{} signed up", i + 1);
    }
    // Names in sign-up order as seeds 1..N, the rest byes.
    let v = preview(&t);
    assert_eq!(v.size, 16);
    let shown: Vec<(String, u16)> = v.rounds[0]
        .iter()
        .flat_map(|g| g.sides.iter())
        .filter_map(|s| match s {
            BracketSide::Player { name, seed } => Some((name.clone(), *seed)),
            _ => None,
        })
        .collect();
    assert_eq!(shown.len(), 9);
    assert!(shown.iter().all(|(n, seed)| *n == format!("Player{seed}")), "{shown:?}");
    let byes = v.rounds[0].iter().flat_map(|g| g.sides.iter()).filter(|s| **s == BracketSide::Bye).count();
    assert_eq!(byes, 7);
    assert!(v.rounds[0].iter().all(|g| g.winner.is_none()), "nothing is decided in a preview");
    // Leaving before the draw takes the slot away.
    t.withdraw("Player9", now).unwrap();
    assert_eq!(preview(&t).size, 8);
    t.withdraw("player8", now).unwrap();
    assert_eq!(preview(&t).size, 8);
    for n in ["Player7", "Player6", "Player5"] {
        t.withdraw(n, now).unwrap();
    }
    assert_eq!(preview(&t).size, 4);
    assert_eq!(t.withdraw("Nobody", now), Err(LobbyProblem::NotEntered));
}

#[test]
fn check_in_runs_from_60_to_30_minutes_before_and_the_draw_uses_only_the_checked_in() {
    assert_eq!((CHECK_IN_OPENS_SECS, DRAW_SECS), (3600, 1800));
    let mut t = Tournament::new("Friday Cup", None, START, 128, false, START - 10 * 86_400);
    // Sign-up is open from creation.
    for n in names(6) {
        t.sign_up(&n, START - 9 * 86_400).unwrap();
    }
    assert_eq!(t.check_in("Player1", START - 3601), Err(LobbyProblem::NotCheckInTime));
    for n in ["Player1", "Player2", "Player4", "Player6", "Player5"] {
        t.check_in(n, START - 3600).unwrap();
    }
    assert_eq!(t.info(Some("Player1")).you, Some(EntryStatus::CheckedIn));
    assert_eq!(t.info(Some("Player3")).you, Some(EntryStatus::SignedUp));
    // Late sign-up during check-in is fine; then the draw closes both.
    t.sign_up("Late", START - 1801).unwrap();
    t.check_in("Late", START - 1801).unwrap();
    assert_eq!(t.draw_at(), Some(START - 1800));
    assert!(!t.sign_up_open(START - 1800));
    assert_eq!(t.sign_up("Later", START - 1800), Err(LobbyProblem::TournamentClosed));
    assert_eq!(t.check_in("Player3", START - 1800), Err(LobbyProblem::NotCheckInTime));
    assert_eq!(t.make_draw(99), 6);
    assert_eq!(t.state, TournamentState::Drawn);
    let b = t.bracket.as_ref().unwrap();
    assert_eq!(b.seed, Some(99));
    assert_eq!(b.size(), 8);
    assert!(b.index_of("Player3").is_none(), "not checked in: dropped");
    assert!(b.index_of("Late").is_some());
    assert_eq!(t.info(Some("Player3")).you, Some(EntryStatus::Out));
    assert_eq!(t.info(Some("Player1")).you, Some(EntryStatus::InBracket));
    let v = t.view(None);
    assert!(!v.preview, "the drawn bracket is final");
    // Fewer than two checked in: cancelled.
    let mut t = Tournament::new("Quiet Cup", None, START, 8, false, START - 86_400);
    t.sign_up("Solo", START - 7200).unwrap();
    t.check_in("Solo", START - 3000).unwrap();
    assert_eq!(t.make_draw(1), 1);
    assert_eq!(t.state, TournamentState::Cancelled);
}

#[test]
fn the_create_limits() {
    let now = START - 7 * 86_400;
    let ok = |_: &str| true;
    assert_eq!(check_create("Friday Cup", START, 16, now, ok), Ok(()));
    for (name, starts, cap) in [
        ("Fr", START, 16),                                      // too short
        ("A name that is far too long here", START, 16),        // too long
        ("Cup  Two", START, 16),                                // double space
        ("1234", START, 16),                                    // no letter
        ("Cup<script>", START, 16),                             // characters
        ("Friday Cup", START, 3),                               // cap too small
        ("Friday Cup", START, 129),                             // cap too big
        ("Friday Cup", now + CREATE_MIN_LEAD_SECS - 1, 16),     // too soon
        ("Friday Cup", now + CREATE_MAX_AHEAD_SECS + 1, 16),    // too far
    ] {
        assert_eq!(check_create(name, starts, cap, now, ok), Err(LobbyProblem::BadTournament), "{name} {starts} {cap}");
    }
    for cap in [4, 5, 100, 128] {
        assert_eq!(check_create("Friday Cup", START, cap, now, ok), Ok(()));
    }
    // Through the name filter, word by word and run together.
    let filter = crate::name_filter::NameFilter::parse("badword").unwrap();
    let f = |w: &str| filter.allows(w);
    assert_eq!(check_create("Bad Word Cup", START, 8, now, &f), Err(LobbyProblem::BadTournament));
    assert_eq!(check_create("My badword Cup", START, 8, now, &f), Err(LobbyProblem::BadTournament));
    assert_eq!(check_create("Good Cup", START, 8, now, &f), Ok(()));
    // The cap is kept.
    let mut t = Tournament::new("Small", None, START, 4, false, now);
    for n in names(4) {
        t.sign_up(&n, now).unwrap();
    }
    assert_eq!(t.sign_up("Fifth", now), Err(LobbyProblem::TournamentFull));
}

#[test]
fn the_weekly_time_is_friday_19_utc_and_times_parse() {
    assert_eq!(parse_utc("2026-12-04 19:00"), Some(START));
    assert_eq!(parse_utc("2026-12-04T19:00Z"), Some(START));
    assert_eq!(parse_utc("1796410800"), Some(START));
    assert_eq!(parse_utc("2026-13-04 19:00"), None);
    // From Monday 23 Nov 2026 noon: Friday 27 Nov 19:00.
    let monday = 1_795_435_200;
    assert_eq!(next_weekly(monday, WEEKLY_WEEKDAY, WEEKLY_MINUTE), START - 7 * 86_400);
    assert_eq!(next_weekly(START, WEEKLY_WEEKDAY, WEEKLY_MINUTE), START, "at the time itself");
    assert_eq!(next_weekly(START + 1, WEEKLY_WEEKDAY, WEEKLY_MINUTE), START + 7 * 86_400);
    assert_eq!(crate::friends_cli::utc_stamp(START), "2026-12-04 19:00:00");
}

#[test]
fn a_quick_tournament_fills_checks_in_for_five_minutes_and_draws() {
    let now = 1_000_000;
    let mut q = Tournament::quick(now);
    assert!(q.quick && q.draw_at().is_none() && q.sign_up_open(now));
    for (i, n) in ["Yugi", "Joey", "Mai"].iter().enumerate() {
        assert_eq!(q.sign_up(n, now + i as i64), Ok(false));
    }
    q.withdraw("Mai", now + 5).unwrap();
    assert_eq!(q.entries.len(), 2, "leaving before it fills frees the place");
    assert_eq!(q.check_in("Yugi", now + 5), Err(LobbyProblem::NotCheckInTime), "no check-in before it fills");
    q.sign_up("Mai", now + 6).unwrap();
    assert_eq!(q.sign_up("Kaiba", now + 10), Ok(true), "the fourth fills it");
    assert_eq!(q.starts, now + 10 + QUICK_CHECK_IN_SECS);
    assert_eq!(q.sign_up("Bakura", now + 11), Err(LobbyProblem::TournamentClosed));
    for n in ["Yugi", "Joey", "Mai"] {
        q.check_in(n, now + 20).unwrap();
    }
    // Kaiba does not check in: a bye takes the slot.
    assert_eq!(q.make_draw(5), 3);
    let b = q.bracket.as_ref().unwrap();
    assert_eq!(b.size(), 4);
    assert_eq!(b.rounds[0].iter().filter(|g| g.how == Some(WinHow::Bye)).count(), 1);
    assert!(b.index_of("Kaiba").is_none());
}
