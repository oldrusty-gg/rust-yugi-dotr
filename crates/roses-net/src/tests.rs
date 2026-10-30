use crate::client::{Bot, BotAction, ClientState};
use crate::filter::{player_view, scramble_hidden, seen_events, PlayerView, SeenEvent};
use crate::protocol::*;
use crate::synthetic;
use crate::transport::{memory_pair, recv_timeout, Connection, FramedTcp, LoopbackListener, Received};
use roses_core::duel::{build_campaign_decks, Duel, DuelCommand, DuelEvent, DuelSetup, PAD_CONTROLLER};
use roses_core::GridPos;
use std::time::Duration;

fn duel() -> Duel {
    let cards = synthetic::catalogue();
    let decks = synthetic::decks();
    let mut rng = 7;
    let decks = build_campaign_decks(&mut rng, [&decks[0], &decks[1]]);
    Duel::new(
        DuelSetup {
            terrain: synthetic::board(),
            leaders: synthetic::leaders(),
            decks,
            first: 0,
            leader_abilities: [vec![], vec![]],
            controllers: [PAD_CONTROLLER; 2],
        },
        &cards,
    )
    .unwrap()
    .0
}

#[test]
fn compat_reports_each_mismatch() {
    let server = Compat::current("sha256:aa");
    assert_eq!(Compat::current("sha256:aa").check(&server), Ok(()));
    let mut c = Compat::current("sha256:aa");
    c.protocol += 1;
    assert!(matches!(c.check(&server), Err(Incompatible::Protocol { .. })));
    let mut c = Compat::current("sha256:aa");
    c.rules_profile = "dotr-mod-x".into();
    assert!(matches!(c.check(&server), Err(Incompatible::RulesProfile { .. })));
    let mut c = Compat::current("sha256:aa");
    c.duel_schema = "roses-duel-0".into();
    assert!(matches!(c.check(&server), Err(Incompatible::DuelSchema { .. })));
    assert!(matches!(
        Compat::current("sha256:bb").check(&server),
        Err(Incompatible::GameData { .. })
    ));
    let mut c = Compat::current("sha256:aa");
    c.mods = vec!["hybrid".into()];
    assert!(matches!(c.check(&server), Err(Incompatible::Mods { .. })));
}

#[test]
fn fingerprint_changes_with_any_rules_data() {
    let a = synthetic::catalogue();
    let mut b = a.clone();
    b.cards[5].attack += 1;
    let none = crate::RulesTables::empty();
    assert_eq!(data_fingerprint(&a, &none), data_fingerprint(&a.clone(), &none));
    assert_ne!(data_fingerprint(&a, &none), data_fingerprint(&b, &none));
    // The leader ability table and the Destiny Draw rows are rules data too.
    let mut abilities = crate::RulesTables { abilities: vec![[0; 20]; 854], destiny: None };
    let with_table = data_fingerprint(&a, &abilities);
    assert_ne!(with_table, data_fingerprint(&a, &none));
    abilities.abilities[3][1] = 7;
    assert_ne!(data_fingerprint(&a, &abilities), with_table);
    abilities.abilities[3][1] = 0;
    abilities.destiny = Some([[0; 3]; 8]);
    assert_ne!(data_fingerprint(&a, &abilities), with_table);
}

#[test]
fn match_options_give_the_custom_duel_words() {
    let o = MatchOptions { leader_ability: [true, false], card_display_open: [false, true] };
    assert_eq!(o.words(), [1, 2]);
    assert_eq!(MatchOptions::default().words(), [3, 0]);
}

/// Card Display OPEN: the open side's hand and face-down cards are in the
/// other seat's view (nothing is hidden between two pads in the original);
/// HIDE sends none of them, and the mirror rebuilds either exactly.
#[test]
fn an_open_side_shows_its_hand_and_face_down_cards() {
    use crate::filter::{player_view_with, seen_events_with};
    use crate::mirror::mirror_duel;
    let cards = synthetic::catalogue();
    let d = duel();
    let (d, events) = d.apply(0, &DuelCommand::OpenSummon { cell: GridPos { column: 3, row: 5 } }, &cards).unwrap();
    let hidden = player_view_with(&d, 1, [false, false]);
    assert_eq!(hidden.open_hand, None);
    let open = player_view_with(&d, 1, [true, false]);
    assert_eq!(open.open_hand.as_ref(), Some(&d.sides[0].hand));
    assert!(matches!(&seen_events_with(&d, Some(0), 1, &events, [true, false])[0], SeenEvent::Public { .. }));
    assert!(matches!(&seen_events_with(&d, Some(0), 1, &events, [false, true])[0], SeenEvent::Drew { .. }));
    let (d, _) = d.apply(0, &DuelCommand::ChooseSummon { hand: vec![0] }, &cards).unwrap();
    let unit = d.units.iter().find(|u| u.side == 0 && u.slot != 0).unwrap();
    let seen = |v: &PlayerView| v.board.units.iter().find(|u| u.id == unit.id).unwrap().card;
    assert_eq!(seen(&player_view_with(&d, 1, [false, true])), None);
    assert_eq!(seen(&player_view_with(&d, 1, [true, false])), Some(unit.card));
    for open in [[false, false], [true, false], [true, true]] {
        for seat in 0..2 {
            let v = player_view_with(&d, seat, open);
            assert_eq!(player_view_with(&mirror_duel(&v, &cards).unwrap(), seat, open), v);
        }
    }
}

/// The server strips a streamed press's details card unless the other seat
/// sees that card; a receiver adopts only input states that fit its copy.
#[test]
fn streamed_presses_carry_no_hidden_card_and_are_checked() {
    use crate::presence::{plausible, sanitize};
    use roses_core::duel::ui::{DetailsView, DuelUi, HandMenu, Mode};
    let cards = synthetic::catalogue();
    let d = duel();
    let (d, _) = d.apply(0, &DuelCommand::OpenSummon { cell: GridPos { column: 3, row: 5 } }, &cards).unwrap();
    let mut input = DuelUi::new(&d);
    input.mode = Mode::Hand(HandMenu { cursor: 1, marks: vec![0, 2] });
    input.details = Some(DetailsView { unit: None, card: d.sides[0].hand[1], page: 0 });
    let mut press = Press { base_revision: 1, through_revision: 1, key: 0x10, response: PressResponse::Moved, input, gap: 0 };
    let mut open = press.clone();
    sanitize(&mut press, &crate::player_view_with(&d, 1, [false, false]));
    assert_eq!(press.input.details.unwrap().card, 0);
    sanitize(&mut open, &crate::player_view_with(&d, 1, [true, false]));
    assert_eq!(open.input.details.unwrap().card, d.sides[0].hand[1]);
    assert!(plausible(&press.input, &d));
    let mut bad = press.input.clone();
    bad.mode = Mode::Hand(HandMenu { cursor: 9, marks: vec![] });
    assert!(!plausible(&bad, &d));
    bad.mode = Mode::Hand(HandMenu { cursor: 0, marks: vec![1, 1] });
    assert!(!plausible(&bad, &d));
    bad.mode = Mode::Field;
    bad.cursors[1] = GridPos { column: 7, row: 0 };
    assert!(!plausible(&bad, &d));
}

#[test]
fn views_do_not_depend_on_anything_hidden_from_the_seat() {
    let cards = synthetic::catalogue();
    let mut d = duel();
    // Side 0 summons face-down, ends; side 1 summons too.
    for side in [0u8, 1] {
        let cell = if side == 0 { GridPos { column: 3, row: 5 } } else { GridPos { column: 3, row: 1 } };
        d = d.apply(side, &DuelCommand::OpenSummon { cell }, &cards).unwrap().0;
        d = d.apply(side, &DuelCommand::ChooseSummon { hand: vec![0] }, &cards).unwrap().0;
        d = d.apply(side, &DuelCommand::EndTurn, &cards).unwrap().0;
    }
    for viewer in [0u8, 1] {
        let view = player_view(&d, viewer);
        assert_eq!(view, player_view(&scramble_hidden(&d, viewer, 29), viewer));
        // The opponent's face-down unit has no card id; the own one does.
        let units: Vec<_> = view.board.units.iter().filter(|u| u.slot != 0).collect();
        assert_eq!(units.len(), 2);
        for u in units {
            assert_eq!(u.card.is_some(), u.side == viewer);
        }
        let json = serde_json::to_string(&view).unwrap();
        for forbidden in ["\"rng\"", "\"deck\"", "temp_units", "\"seed\"", "experience"] {
            assert!(!json.contains(forbidden), "{forbidden} in {json}");
        }
    }
    // The scramble really changes hidden state (else the check is vacuous).
    assert_ne!(d, scramble_hidden(&d, 0, 29));
}

#[test]
fn opponent_draws_and_face_down_summons_lose_their_card_ids() {
    let cards = synthetic::catalogue();
    let d = duel();
    let (open, events) = d
        .apply(0, &DuelCommand::OpenSummon { cell: GridPos { column: 3, row: 5 } }, &cards)
        .unwrap();
    let drawn = match &events[0] {
        DuelEvent::Drew { cards, .. } => cards.clone(),
        e => panic!("{e:?}"),
    };
    assert_eq!(
        seen_events(&open, Some(0), 1, &events),
        vec![SeenEvent::Drew { side: 0, count: drawn.len() }]
    );
    assert_eq!(
        seen_events(&open, Some(0), 0, &events),
        vec![SeenEvent::Public { event: events[0].clone() }]
    );
    let (summoned, events) = open
        .apply(0, &DuelCommand::ChooseSummon { hand: vec![1] }, &cards)
        .unwrap();
    let theirs = seen_events(&summoned, Some(0), 1, &events);
    assert!(theirs.iter().any(|e| matches!(e, SeenEvent::Summoned { .. })));
    assert!(!serde_json::to_string(&theirs).unwrap().contains("\"card\""));
    let mine = seen_events(&summoned, Some(0), 0, &events);
    assert!(mine
        .iter()
        .any(|e| matches!(e, SeenEvent::Public { event: DuelEvent::Summoned { card, .. } } if *card == drawn[1])));
}

#[test]
fn hidden_combinations_are_redacted_for_the_opponent() {
    let d = duel();
    let events = vec![
        DuelEvent::Fused { cards: [3, 4], result: 9 },
        DuelEvent::Equipped { side: 0, monster: 3, power_up: 5, value: 500 },
        DuelEvent::DestinyDraw { side: 0, card: 7 },
    ];
    let theirs = seen_events(&d, Some(0), 1, &events);
    assert_eq!(
        theirs,
        vec![
            SeenEvent::Combined { side: Some(0) },
            SeenEvent::Combined { side: Some(0) },
            SeenEvent::DestinyDraw { side: 0 },
        ]
    );
    assert!(seen_events(&d, Some(0), 0, &events)
        .iter()
        .all(|e| matches!(e, SeenEvent::Public { .. })));
}

#[test]
fn memory_and_tcp_transports_carry_messages_and_refuse_bad_frames() {
    let (mut a, mut b) = memory_pair::<ClientMessage, ServerMessage>();
    a.send(&ClientMessage::Sync).unwrap();
    assert_eq!(b.poll(), Received::Message(ClientMessage::Sync));
    assert_eq!(b.poll(), Received::Empty);
    a.close();
    assert_eq!(b.poll(), Received::Closed);

    let listener = LoopbackListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    assert!(LoopbackListener::bind("0.0.0.0:0".parse().unwrap()).is_err());
    let addr = listener.local_addr().unwrap();
    let mut client = FramedTcp::<ClientMessage, ServerMessage>::connect(addr).unwrap();
    let mut server = loop {
        if let Some(s) = listener.accept::<ServerMessage, ClientMessage>().unwrap() {
            break s;
        }
    };
    client.send(&ClientMessage::Forfeit { seq: 9 }).unwrap();
    assert_eq!(
        recv_timeout(&mut server, Duration::from_secs(5)),
        Received::Message(ClientMessage::Forfeit { seq: 9 })
    );
    client.send_raw(b"{not json").unwrap();
    assert!(matches!(recv_timeout(&mut server, Duration::from_secs(5)), Received::Malformed(_)));
    // A length prefix over the limit closes the connection (a server's
    // listener accepts client frames of at most MAX_CLIENT_FRAME).
    client.send_raw(&vec![b' '; crate::transport::MAX_CLIENT_FRAME + 1]).unwrap();
    assert!(matches!(recv_timeout(&mut server, Duration::from_secs(5)), Received::Malformed(_)));
    assert_eq!(server.poll(), Received::Closed);
    // The memory transport applies a limit the same way.
    let (mut a, b) = memory_pair::<ClientMessage, ServerMessage>();
    let mut b = b.with_max_frame(8);
    a.send(&ClientMessage::Sync).unwrap();
    assert!(matches!(b.poll(), Received::Malformed(_)));
}

#[test]
fn client_state_sequences_and_forgets_decided_intentions() {
    let mut c = ClientState::new();
    let m1 = c.command(DuelCommand::EndTurn);
    let m2 = c.forfeit();
    assert!(matches!(m1, ClientMessage::Command { seq: 1, base_revision: 0, .. }));
    assert_eq!(m2, ClientMessage::Forfeit { seq: 2 });
    assert_eq!(c.resend().len(), 2);
    c.apply(&ServerMessage::Accepted { seq: 1, revision: 1 });
    assert_eq!(c.resend(), vec![m2]);
    c.apply(&ServerMessage::Duplicate { seq: 2, decision: Decision::Forgotten });
    assert!(c.resend().is_empty());
}

#[test]
fn the_bot_only_acts_on_its_turn_and_does_not_repeat_refusals() {
    let cards = synthetic::catalogue();
    let d = duel();
    let mut bot = Bot::new();
    assert_eq!(bot.next(&player_view(&d, 1), 0, &cards), None);
    let view = player_view(&d, 0);
    let first = bot.next(&view, 0, &cards).unwrap();
    assert!(matches!(first, BotAction::Command(DuelCommand::OpenSummon { .. })));
    let second = bot.next(&view, 0, &cards).unwrap();
    assert_ne!(first, second);
    // Exhausting every candidate ends in a forfeit, never a loop.
    let mut last = second;
    for _ in 0..50 {
        last = bot.next(&view, 0, &cards).unwrap();
        if last == BotAction::Forfeit {
            break;
        }
    }
    assert_eq!(last, BotAction::Forfeit);
}

#[test]
fn submitted_decks_follow_the_original_deck_rules() {
    use crate::deck::{check_deck, DeckProblem};
    let cards = synthetic::catalogue();
    let [a, b] = synthetic::submissions();
    assert_eq!(check_deck(&a, &cards), Ok(()));
    assert_eq!(check_deck(&b, &cards), Ok(()));
    let with = |f: &dyn Fn(&mut crate::deck::DeckSubmission)| {
        let mut d = a.clone();
        f(&mut d);
        check_deck(&d, &cards)
    };
    assert_eq!(with(&|d| { d.cards.pop(); }), Err(DeckProblem::WrongSize { count: 39 }));
    assert_eq!(with(&|d| d.cards.push(5)), Err(DeckProblem::WrongSize { count: 41 }));
    assert_eq!(with(&|d| d.leader = 999), Err(DeckProblem::NoLeader));
    assert_eq!(with(&|d| d.leader_rank = 0), Err(DeckProblem::BadLeaderRank { rank: 0 }));
    assert_eq!(with(&|d| d.leader_rank = 13), Err(DeckProblem::BadLeaderRank { rank: 13 }));
    // A fourth copy (the leader slot does not count).
    assert_eq!(
        with(&|d| d.cards[0..4].copy_from_slice(&[0, 0, 0, 0])),
        Err(DeckProblem::TooManyCopies { card: 0, count: 4 })
    );
    assert_eq!(with(&|d| d.cards[0..3].copy_from_slice(&[0, 0, 0])), Ok(()));
    // Cards without a record (0x29F, >= 854) or outside the catalogue.
    assert_eq!(with(&|d| d.cards[3] = 0x29F), Err(DeckProblem::UnknownCard { card: 0x29F }));
    assert_eq!(with(&|d| d.cards[3] = 900), Err(DeckProblem::UnknownCard { card: 900 }));
    assert_eq!(with(&|d| d.cards[3] = 31), Err(DeckProblem::UnknownCard { card: 31 }));
    // The JSON shape refuses unknown fields.
    assert!(serde_json::from_str::<crate::deck::DeckSubmission>(r#"{"leader":1,"leader_rank":1,"cards":[],"extra":1}"#).is_err());
}

/// The mirror rebuilt from a seat's view gives the same view back, does not
/// depend on anything hidden, and the unchanged input layer produces the
/// same intentions on it as on the authoritative duel, in every state of a
/// whole match.
#[test]
fn the_mirror_reproduces_the_view_and_the_input_layer_on_it() {
    mirror_check(&synthetic::catalogue(), duel(), 0x5eed);
}

/// The same with the user's own import (real cards, effects, the first
/// duel's decks): `ROSES_TEST_DUEL_IMPORT=<game or duel import>`.
#[test]
#[ignore = "Requires the user's duel import (ROSES_TEST_DUEL_IMPORT)"]
fn private_the_mirror_reproduces_real_duels() {
    let dir = std::path::PathBuf::from(std::env::var_os("ROSES_TEST_DUEL_IMPORT").unwrap());
    let import = crate::data::load_duel_import(&dir).unwrap();
    let f = &import.first_duel;
    for seed in [1u64, 2, 3, 4] {
        let mut rng = seed;
        let decks = build_campaign_decks(&mut rng, [&f.sides[0].cards, &f.sides[1].cards]);
        let (mut d, _) = Duel::new(
            DuelSetup {
                terrain: f.terrain.clone(),
                leaders: [f.sides[0].leader, f.sides[1].leader],
                decks,
                first: f.first,
                leader_abilities: [vec![], vec![]],
                controllers: [PAD_CONTROLLER; 2],
            },
            &import.catalogue,
        )
        .unwrap();
        d.seed_rng(rng);
        mirror_check(&import.catalogue, d, seed);
    }
}

fn mirror_check(cards: &roses_core::duel::Catalogue, start: Duel, seed: u64) {
    use crate::mirror::{duel_events, hidden_card, mirror_duel, reveal};
    use roses_core::duel::ui::{DuelUi, Response};
    let cards = cards.clone();
    let hidden = hidden_card(&cards);
    let mut d = start;
    let mut bots = [Bot::new(), Bot::new()];
    let mut rng: u64 = seed;
    let keys = [
        0x1000u16, 0x2000, 0x4000, 0x8000, 0x40, 0x20, 0x80, 0x10, 0x800, 0x100, 0x4, 0x8, 0x2, 0x1,
    ];
    let mut states = 0;
    let mut compared = 0;
    for step in 0..2000 {
        if d.outcome.is_some() {
            break;
        }
        for seat in 0..2u8 {
            let view = player_view(&d, seat);
            let m = mirror_duel(&view, &cards).unwrap();
            assert_eq!(player_view(&m, seat), view, "step {step} seat {seat}");
            let scrambled = mirror_duel(&player_view(&scramble_hidden(&d, seat, 29), seat), &cards).unwrap();
            assert_eq!(scrambled, m);
        }
        states += 1;
        let seat = d.active;
        let m = mirror_duel(&player_view(&d, seat), &cards).unwrap();
        // Random presses from the same input state on both copies.
        let mut real = DuelUi::new(&d);
        if d.pending_summon.is_none() && d.pending.is_none() {
            let mut copy = real.clone();
            for _ in 0..24 {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let k = keys[(rng >> 33) as usize % keys.len()];
                let a = real.press(&d, &cards, k);
                let b = copy.press(&m, &cards, k);
                assert_eq!(a.is_ok(), b.is_ok(), "key {k:#x}: {a:?} / {b:?}");
                let (Ok(a), Ok(b)) = (a, b) else { break };
                compared += 1;
                assert_eq!(a, b, "key {k:#x} at step {step}");
                assert_eq!(real.cursors, copy.cursors);
                assert_eq!(real.mode, copy.mode);
                if matches!(a, Response::Commands(_)) {
                    break;
                }
            }
        }
        let view = player_view(&d, seat);
        let action = bots[usize::from(seat)].next(&view, step, &cards).unwrap();
        let next = match action {
            BotAction::Command(c) => d.apply(seat, &c, &cards),
            BotAction::Forfeit => d.forfeit(seat),
        };
        if let Err(roses_core::duel::DuelError::Unsupported(_)) = &next {
            // The real-data run can reach an unported interaction: stop there.
            if seed != 0x5eed {
                break;
            }
        }
        if let Ok((next, events)) = next {
            // The other seat's presentation copy learns revealed identities.
            let other = seat ^ 1;
            let seen = seen_events(&next, Some(seat), other, &events);
            let mut before = mirror_duel(&player_view(&d, other), &cards).unwrap();
            reveal(&mut before, &seen);
            for e in &seen {
                if let SeenEvent::Public { event: DuelEvent::Revealed { unit, card } } = e {
                    assert_eq!(before.unit(*unit).map(|u| u.card), Some(*card));
                }
            }
            let shown = duel_events(&seen, hidden);
            assert!(shown.len() <= events.len());
            d = next;
        }
    }
    eprintln!("mirror check: {states} states, {compared} presses compared, outcome {:?}", d.outcome);
    assert!(states > 20 && compared > 100, "{states} states, {compared} presses");
}

#[test]
fn a_match_client_reads_the_hubs_busy_refusal() {
    // A hub turns a connection away before its first message with the
    // lobby's refusal; the match side reads it as "the server is busy".
    use crate::lobby::{LobbyEvent, LobbyRefusal, ServerFrame};
    let (mut server, client) = memory_pair::<ServerFrame, ClientMessage>();
    let mut client = client.retype::<ClientMessage, ServerMessage>();
    server
        .send(&ServerFrame::Lobby(LobbyEvent::LobbyRefused { reason: LobbyRefusal::ServerFull }))
        .unwrap();
    let Received::Message(m) = recv_timeout(&mut client, Duration::from_secs(1)) else { panic!("no message") };
    assert_eq!(m, ServerMessage::LobbyRefused { reason: LobbyRefusal::ServerFull });
    let mut state = ClientState::new();
    state.apply(&m);
    assert_eq!(state.refused, Some(Refusal::ServerBusy));
    assert!(Refusal::ServerBusy.plain().contains("busy"));
    // Pre-serialised sends arrive as the same message.
    let body = serde_json::to_vec(&ServerFrame::Match(ServerMessage::Resumed)).unwrap();
    server.send_serialized(&body).unwrap();
    assert_eq!(recv_timeout(&mut client, Duration::from_secs(1)), Received::Message(ServerMessage::Resumed));
}

/// Every computer opponent's static deck of the user's own executable is a
/// legal online deck (the server accepts each deck the game itself uses as
/// an opponent deck): `ROSES_TEST_DUEL_IMPORT=<game or duel import>`.
#[test]
#[ignore = "Requires the user's duel import (ROSES_TEST_DUEL_IMPORT)"]
fn private_every_opponent_deck_is_a_legal_online_deck() {
    let dir = std::path::PathBuf::from(std::env::var_os("ROSES_TEST_DUEL_IMPORT").unwrap());
    let import = crate::data::load_duel_import(&dir).unwrap();
    let duel = if dir.join("SLES_515.79").is_file() { dir.clone() } else { dir.join("duel") };
    let decks = crate::deck::load_opponent_decks(&duel).unwrap();
    assert_eq!(decks.len(), usize::from(crate::deck::OPPONENT_COUNT));
    for d in &decks {
        eprintln!("opponent {:#04x} record {:#04x} {:?}: leader {} rank {}", d.opponent, d.record, d.name, d.deck.leader, d.deck.leader_rank);
        assert_eq!(crate::deck::check_deck(&d.deck, &import.catalogue), Ok(()), "{}", d.name);
        // A copy as a custom deck keeps every card, the leader and its rank.
        let copy = crate::custom_decks::CustomDeck::from_submission(&d.name, &d.deck);
        assert_eq!(copy.submission().as_ref(), Ok(&d.deck));
    }
    let names: Vec<&str> = decks.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names.iter().collect::<std::collections::BTreeSet<_>>().len(), names.len(), "{names:?}");
    // Weevil (record 0x1C) is the first duel's opponent.
    assert_eq!(decks[2].deck.cards, import.first_duel.sides[1].cards);
    assert_eq!(decks[2].deck.leader, import.first_duel.sides[1].leader);
}
