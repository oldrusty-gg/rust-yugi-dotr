//! The five required cases of ONLINE_PLAN.md's first online milestone,
//! driven through the tick-based match state machine (no transport, no
//! frame rate). `tests/tcp.rs` repeats a match over loopback TCP.

use crate::online::{OnlineMatch, Outbox};
use crate::replay::{replay, state_hash, MatchSetup, ReplayAction};
use roses_core::duel::{Catalogue, DuelCommand, DuelEvent, EndReason, Pick};
use roses_core::{GridPos, UnitId};
use roses_net::client::{Bot, BotAction, ClientState};
use roses_net::filter::scramble_hidden;
use roses_net::{RulesTables, 
    data_fingerprint, player_view, synthetic, Cause, ClientMessage, Compat, Decision, Incompatible,
    Join, MatchStatus, OnlinePolicy, Refusal, Rejection, SeenEvent, ServerMessage,
};

const TOKENS: [&str; 2] = ["seat-zero-secret-token", "seat-one-secret-token"];

fn setup(seed: u64) -> MatchSetup {
    MatchSetup {
        seed,
        terrain: synthetic::board(),
        leaders: synthetic::leaders(),
        deck_sources: synthetic::decks(),
        first: 0,
        leader_abilities: [vec![], vec![]],
        custom_rules: None,
        leader_ranks: [0; 2],
        destiny_rows: None,
    }
}

fn compat() -> Compat {
    Compat::current(data_fingerprint(&synthetic::catalogue(), &RulesTables::empty()))
}

/// A match driven by explicit ticks, with each seat's inbox and client
/// state, checking every delivered message for leaks.
struct Harness {
    m: OnlineMatch,
    inbox: [Vec<ServerMessage>; 2],
    clients: [ClientState; 2],
    now: u64,
    leak_checks: usize,
}

impl Harness {
    fn new(policy: OnlinePolicy, seed: u64) -> Self {
        Self {
            m: OnlineMatch::new(
                "test-match",
                setup(seed),
                synthetic::catalogue(),
                policy,
                TOKENS.map(String::from),
            )
            .unwrap(),
            inbox: [Vec::new(), Vec::new()],
            clients: [ClientState::new(), ClientState::new()],
            now: 0,
            leak_checks: 0,
        }
    }

    /// A match whose seats submit decks; both join with the synthetic
    /// legal decks.
    fn submitted(policy: OnlinePolicy, seed: u64) -> Self {
        let mut h = Self::new(policy.clone(), seed);
        h.m = OnlineMatch::with_submitted_decks(
            "test-submitted",
            setup(seed),
            synthetic::catalogue(),
            policy,
            TOKENS.map(String::from),
        )
        .unwrap();
        let decks = synthetic::submissions();
        for seat in 0..2u8 {
            let (s, out) = h
                .m
                .hello_with_deck(&compat(), &Join::Seat { seat: Some(seat) }, Some(&decks[usize::from(seat)]), 0)
                .unwrap();
            assert_eq!(s, seat);
            h.take(out);
        }
        assert_eq!(h.m.status(), MatchStatus::Running);
        h
    }

    fn started(policy: OnlinePolicy, seed: u64) -> Self {
        let mut h = Self::new(policy, seed);
        for seat in 0..2u8 {
            let (s, out) = h
                .m
                .hello(&compat(), &Join::Seat { seat: Some(seat) }, 0)
                .unwrap();
            assert_eq!(s, seat);
            h.take(out);
        }
        assert_eq!(h.m.status(), MatchStatus::Running);
        h
    }

    /// Deliver an outbox: leak-check each message, feed the clients.
    fn take(&mut self, out: Outbox) {
        for (seat, msg) in out {
            self.check_permitted(seat, &msg);
            self.clients[usize::from(seat)].apply(&msg);
            self.inbox[usize::from(seat)].push(msg);
        }
    }

    fn send(&mut self, seat: u8, msg: ClientMessage) -> Vec<ServerMessage> {
        let before = self.inbox[usize::from(seat)].len();
        let out = self.m.handle(seat, msg, self.now);
        self.take(out);
        self.inbox[usize::from(seat)][before..].to_vec()
    }

    fn command(&mut self, seat: u8, command: DuelCommand) -> Vec<ServerMessage> {
        let msg = self.clients[usize::from(seat)].command(command);
        self.send(seat, msg)
    }

    fn tick(&mut self, now: u64) {
        self.now = now;
        let out = self.m.tick(now);
        self.take(out);
    }

    /// Case 1's information check, applied to every delivered message.
    fn check_permitted(&mut self, viewer: u8, msg: &ServerMessage) {
        let json = serde_json::to_string(msg).unwrap();
        for forbidden in ["\"rng\"", "\"deck\"", "temp_units", "\"seed\"", "deck_sources", "experience", "final_state"] {
            assert!(!json.contains(forbidden), "{forbidden} sent to seat {viewer}: {json}");
        }
        assert!(!json.contains(TOKENS[usize::from(viewer ^ 1)]), "opponent token leaked");
        let current = self.m.revision();
        let view = match msg {
            ServerMessage::Update { view, revision, .. } if *revision == current => Some(view),
            ServerMessage::Snapshot { snapshot } if snapshot.revision == current => Some(&snapshot.view),
            _ => None,
        };
        if let Some(view) = view {
            self.leak_checks += 1;
            assert_eq!(view.seat, viewer);
            assert_eq!(*view, player_view(self.m.duel(), viewer));
            // Non-interference: change everything hidden from the viewer;
            // what it receives must not change.
            assert_eq!(*view, player_view(&scramble_hidden(self.m.duel(), viewer, 29), viewer));
            // The client's copy rebuilt from the view gives the view back.
            let mirror = roses_net::mirror_duel(view, self.m.cards()).unwrap();
            assert_eq!(player_view(&mirror, viewer), *view);
        }
        if let ServerMessage::Update { events, view, .. } = msg {
            for e in events {
                if let SeenEvent::Public { event } = e {
                    let owned_or_visible = |unit: &UnitId| {
                        view.board
                            .units
                            .iter()
                            .find(|u| u.id == *unit)
                            .is_none_or(|u| u.side == viewer || u.card.is_some())
                    };
                    match event {
                        DuelEvent::Drew { side, .. } | DuelEvent::DestinyDraw { side, .. } | DuelEvent::Equipped { side, .. } => {
                            assert_eq!(*side, viewer, "{event:?}")
                        }
                        DuelEvent::Summoned { unit, .. } | DuelEvent::Created { unit, .. } => {
                            assert!(owned_or_visible(unit) || events.iter().any(|e| matches!(e, SeenEvent::Public { event: DuelEvent::Destroyed { unit: u, .. } } if u == unit)), "{event:?}")
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Both seats play computer-driven intentions until the match ends.
    fn play_out(&mut self, bots: &mut [Bot; 2], max_steps: usize) {
        let cards = synthetic::catalogue();
        for _ in 0..max_steps {
            if self.m.ended().is_some() {
                return;
            }
            let seat = self.m.duel().active;
            let c = &self.clients[usize::from(seat)];
            let view = c.view.clone().unwrap();
            match bots[usize::from(seat)].next(&view, c.revision, &cards) {
                Some(BotAction::Command(cmd)) => {
                    self.command(seat, cmd);
                }
                Some(BotAction::Forfeit) => {
                    let m = self.clients[usize::from(seat)].forfeit();
                    self.send(seat, m);
                }
                None => panic!("bot for the active seat {seat} did nothing"),
            }
        }
        panic!("match did not end in {max_steps} steps");
    }
}

fn no_clock() -> OnlinePolicy {
    OnlinePolicy {
        turn_limit: None,
        ..OnlinePolicy::default()
    }
}

// ---- Case 1 (and 5: replay) ----------------------------------------------

#[test]
fn case1_both_seats_finish_a_match_seeing_only_permitted_information() {
    for seed in [1u64, 2, 3] {
        let mut h = Harness::started(no_clock(), seed);
        h.play_out(&mut [Bot::new(), Bot::new()], 20_000);
        let (outcome, cause) = h.m.ended().unwrap();
        assert_eq!(cause, Cause::Command);
        assert_ne!(outcome.reason, EndReason::Surrender, "the bots should not give up");
        for seat in 0..2 {
            assert_eq!(h.clients[seat].ended, Some((outcome, cause)));
            assert_eq!(h.clients[seat].revision, h.m.revision());
            assert_eq!(h.clients[seat].view.as_ref().unwrap(), &player_view(h.m.duel(), seat as u8));
        }
        assert!(h.leak_checks > 50, "{} checks", h.leak_checks);
        // Real play happened: summons, moves and battles on both sides.
        let commands: Vec<_> = h
            .m
            .replay()
            .entries
            .iter()
            .filter_map(|e| match &e.action {
                ReplayAction::Command { side, command } => Some((*side, command)),
                _ => None,
            })
            .collect();
        for side in 0..2u8 {
            assert!(commands.iter().any(|(s, c)| *s == side && matches!(c, DuelCommand::ChooseSummon { .. })));
            assert!(commands.iter().any(|(s, c)| *s == side && matches!(c, DuelCommand::Move { .. })));
        }
        // Case 5: the server's record replays to the identical final state.
        let replayed = replay(h.m.replay(), &synthetic::catalogue(), &RulesTables::empty()).unwrap();
        assert_eq!(&replayed, h.m.duel());
        assert_eq!(state_hash(&replayed), h.m.replay().final_state);
        eprintln!("seed {seed}: {outcome:?} after {} revisions", h.m.revision());
    }
}

#[test]
fn case1_the_opponent_learns_counts_not_cards() {
    let mut h = Harness::started(no_clock(), 5);
    h.command(0, DuelCommand::OpenSummon { cell: GridPos { column: 3, row: 5 } });
    h.command(0, DuelCommand::ChooseSummon { hand: vec![0] });
    let last = h.inbox[1].last().unwrap().clone();
    let ServerMessage::Update { events, view, .. } = last else {
        panic!("{last:?}")
    };
    assert!(events.iter().any(|e| matches!(e, SeenEvent::Summoned { .. })));
    assert!(view.board.hand.is_empty());
    assert_eq!(view.board.hand_counts[0], 4);
    let drew = h.inbox[1].iter().find_map(|m| match m {
        ServerMessage::Update { events, .. } => events.iter().find_map(|e| match e {
            SeenEvent::Drew { side: 0, count } => Some(*count),
            _ => None,
        }),
        _ => None,
    });
    assert_eq!(drew, Some(5));
}

// ---- Submitted decks -------------------------------------------------------

#[test]
fn submitted_decks_are_checked_before_the_match_and_kept_from_the_other_seat() {
    use roses_net::{DeckProblem, DeckSubmission};
    let cards = synthetic::catalogue();
    let decks = synthetic::submissions();
    let mut m = OnlineMatch::with_submitted_decks("decks", setup(3), cards.clone(), no_clock(), TOKENS.map(String::from)).unwrap();
    let seat0 = Join::Seat { seat: Some(0) };
    assert_eq!(m.hello(&compat(), &seat0, 0).unwrap_err(), Refusal::DeckRequired);
    let mut short = decks[0].clone();
    short.cards.truncate(39);
    assert_eq!(
        m.hello_with_deck(&compat(), &seat0, Some(&short), 0).unwrap_err(),
        Refusal::IllegalDeck { problem: DeckProblem::WrongSize { count: 39 } }
    );
    let mut four = decks[0].clone();
    four.cards[..4].copy_from_slice(&[9, 9, 9, 9]);
    assert!(matches!(
        m.hello_with_deck(&compat(), &seat0, Some(&four), 0).unwrap_err(),
        Refusal::IllegalDeck { problem: DeckProblem::TooManyCopies { card: 9, .. } }
    ));
    let unranked = DeckSubmission { leader_rank: 0, ..decks[0].clone() };
    assert!(matches!(
        m.hello_with_deck(&compat(), &seat0, Some(&unranked), 0).unwrap_err(),
        Refusal::IllegalDeck { problem: DeckProblem::BadLeaderRank { rank: 0 } }
    ));
    // Refusals take no seat; a legal deck joins, and nothing about the duel
    // is sent before both seats are in.
    assert_eq!(m.status(), MatchStatus::Waiting);
    let (s, out) = m.hello_with_deck(&compat(), &seat0, Some(&decks[0]), 0).unwrap();
    assert_eq!(s, 0);
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(matches!(out[0], (0, ServerMessage::Welcome { .. })));
    let (s, out) = m.hello_with_deck(&compat(), &Join::Seat { seat: None }, Some(&decks[1]), 0).unwrap();
    assert_eq!(s, 1);
    assert!(m.started());
    // The duel uses each seat's own deck, leader and rank.
    let duel = m.duel();
    for seat in 0..2usize {
        let side = &duel.sides[seat];
        let mut held: Vec<u16> = side.deck.iter().chain(&side.hand).copied().collect();
        let mut submitted = decks[seat].cards.clone();
        held.sort_unstable();
        submitted.sort_unstable();
        assert_eq!(held, submitted, "seat {seat}");
        assert_eq!(duel.units.iter().find(|u| u.side == seat as u8 && u.slot == 0).unwrap().card, decks[seat].leader);
        assert_eq!(duel.leader_ranks[seat], decks[seat].leader_rank);
    }
    assert_eq!(m.replay().setup.deck_sources, [decks[0].cards.clone(), decks[1].cards.clone()]);
    // Each seat's start snapshot shows its own hand and only counts of the
    // other side; neither carries a deck list.
    for (seat, msg) in &out {
        let json = serde_json::to_string(msg).unwrap();
        assert!(!json.contains("deck_sources") && !json.contains("\"cards\""), "{json}");
        if let ServerMessage::Snapshot { snapshot } = msg {
            let own = &duel.sides[usize::from(*seat)].hand;
            assert_eq!(&snapshot.view.board.hand, own);
        }
    }
    // A late seat request is refused, a resume with any deck is not checked.
    assert_eq!(m.hello_with_deck(&compat(), &Join::Seat { seat: None }, Some(&decks[0]), 0).unwrap_err(), Refusal::MatchFull);
}

#[test]
fn a_submitted_deck_match_plays_to_the_end_and_replays() {
    for seed in [11u64, 12] {
        let mut h = Harness::submitted(no_clock(), seed);
        h.play_out(&mut [Bot::new(), Bot::new()], 20_000);
        assert!(h.m.ended().is_some());
        assert!(h.leak_checks > 50);
        let record: crate::Replay = serde_json::from_slice(&serde_json::to_vec(h.m.replay()).unwrap()).unwrap();
        assert_eq!(record.setup.leader_ranks, [1, 3]);
        assert_eq!(replay(&record, &synthetic::catalogue(), &RulesTables::empty()).unwrap(), *h.m.duel());
    }
}

// ---- Case 2 ----------------------------------------------------------------

fn assert_unchanged(h: &Harness, revision: u64, hash: &str, entries: usize) {
    assert_eq!(h.m.revision(), revision);
    assert_eq!(state_hash(h.m.duel()), hash);
    assert_eq!(h.m.replay().entries.len(), entries);
}

#[test]
fn case2_invalid_and_out_of_turn_commands_are_rejected_without_change() {
    let mut h = Harness::started(no_clock(), 9);
    let (rev, hash, n) = (h.m.revision(), state_hash(h.m.duel()), h.m.replay().entries.len());
    let reject = |h: &mut Harness, seat: u8, cmd: DuelCommand| {
        let replies = h.command(seat, cmd);
        match replies.as_slice() {
            [ServerMessage::Rejected { reason, revision, .. }] => {
                assert_eq!(*revision, h.m.revision());
                reason.clone()
            }
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(reject(&mut h, 1, DuelCommand::EndTurn), Rejection::NotYourTurn);
    assert_eq!(reject(&mut h, 1, DuelCommand::Surrender), Rejection::NotYourTurn);
    assert!(matches!(reject(&mut h, 0, DuelCommand::ChooseSummon { hand: vec![0] }), Rejection::InvalidCommand { .. }));
    assert!(matches!(reject(&mut h, 0, DuelCommand::CancelSummon), Rejection::InvalidCommand { .. }));
    assert!(matches!(reject(&mut h, 0, DuelCommand::Pick { pick: Pick::Cancel }), Rejection::InvalidCommand { .. }));
    // Moving the opponent's leader, a missing unit, off-board, a leader attack.
    let mv = |unit: u16, column: u8, row: u8| DuelCommand::Move {
        unit: UnitId(unit),
        path: vec![GridPos { column, row }],
        face_up: false,
        defence: None,
    };
    assert!(matches!(reject(&mut h, 0, mv(1, 3, 1)), Rejection::Illegal { .. }));
    assert!(matches!(reject(&mut h, 0, mv(77, 3, 1)), Rejection::InvalidCommand { .. }));
    assert!(matches!(reject(&mut h, 0, mv(0, 3, 7)), Rejection::Illegal { .. }));
    assert!(matches!(reject(&mut h, 0, DuelCommand::OpenSummon { cell: GridPos { column: 0, row: 0 } }), Rejection::Illegal { .. }));
    assert_unchanged(&h, rev, &hash, n);
    // Nothing reached seat 1 while seat 0 was refused.
    assert!(h.inbox[1].iter().all(|m| !matches!(m, ServerMessage::Update { .. })));
}

/// Deterministic generator for the fuzz test.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn random_command(r: &mut Lcg) -> DuelCommand {
    let cell = |r: &mut Lcg| GridPos {
        column: r.below(9) as u8,
        row: r.below(9) as u8,
    };
    match r.below(8) {
        0 => DuelCommand::OpenSummon { cell: cell(r) },
        1 => DuelCommand::ChooseSummon {
            hand: (0..r.below(4)).map(|_| r.below(7) as usize).collect(),
        },
        2 => DuelCommand::CancelSummon,
        3 => DuelCommand::Move {
            unit: UnitId(r.below(12) as u16),
            path: (0..r.below(4)).map(|_| cell(r)).collect(),
            face_up: r.below(2) == 0,
            defence: [None, Some(true), Some(false)][r.below(3) as usize],
        },
        4 => DuelCommand::EndTurn,
        5 => DuelCommand::Pick {
            pick: match r.below(3) {
                0 => Pick::Cancel,
                1 => Pick::Unit { unit: UnitId(r.below(12) as u16) },
                _ => Pick::Graveyard { side: r.below(3) as u8, index: r.below(5) as usize },
            },
        },
        6 => DuelCommand::Surrender,
        _ => DuelCommand::OpenSummon { cell: cell(r) },
    }
}

#[test]
fn case2_fuzzed_intentions_either_match_the_core_or_change_nothing() {
    let cards: Catalogue = synthetic::catalogue();
    let mut r = Lcg(0x5EED);
    let (mut accepted, mut rejected, mut matches) = (0, 0, 0);
    let mut h = Harness::started(no_clock(), 11);
    let mut seqs = [0u64; 2];
    for _ in 0..6000 {
        if h.m.ended().is_some() {
            let replayed = replay(h.m.replay(), &cards, &RulesTables::empty()).unwrap();
            assert_eq!(&replayed, h.m.duel());
            matches += 1;
            h = Harness::started(no_clock(), 11 + matches);
            seqs = [0; 2];
        }
        let seat = r.below(2) as u8;
        // Surrender only rarely, so the fuzz explores the duel.
        let mut command = random_command(&mut r);
        if command == DuelCommand::Surrender && r.below(50) != 0 {
            command = DuelCommand::EndTurn;
        }
        let base = match r.below(10) {
            0 => h.m.revision() + 1 + r.below(3),
            1 => h.m.revision().saturating_sub(1 + r.below(3)),
            _ => h.m.revision(),
        };
        seqs[usize::from(seat)] += 1 + r.below(2);
        let before = h.m.duel().clone();
        let (rev, entries) = (h.m.revision(), h.m.replay().entries.len());
        let replies = h.send(
            seat,
            ClientMessage::Command { seq: seqs[usize::from(seat)], base_revision: base, command: command.clone() },
        );
        match replies.first() {
            Some(ServerMessage::Accepted { revision, .. }) => {
                accepted += 1;
                assert_eq!(*revision, rev + 1);
                assert_eq!(base, rev);
                // The server's result is exactly the core's.
                let (expected, _) = before.apply(seat, &command, &cards).unwrap();
                assert_eq!(h.m.duel(), &expected);
                assert_eq!(h.m.replay().entries.len(), entries + 1);
            }
            Some(ServerMessage::Rejected { revision, reason, .. }) => {
                rejected += 1;
                assert_eq!(*revision, rev);
                assert_eq!(h.m.duel(), &before);
                assert_eq!(h.m.revision(), rev);
                assert_eq!(h.m.replay().entries.len(), entries);
                if base != rev && seat == before.active {
                    assert!(matches!(reason, Rejection::Stale { .. }), "{reason:?}");
                }
            }
            other => panic!("{other:?}"),
        }
    }
    eprintln!("fuzz: {accepted} accepted, {rejected} rejected, {matches} matches ended");
    assert!(accepted > 100 && rejected > 3000, "{accepted} / {rejected}");
    let replayed = replay(h.m.replay(), &cards, &RulesTables::empty()).unwrap();
    assert_eq!(&replayed, h.m.duel());
}

// ---- Case 3 ----------------------------------------------------------------

#[test]
fn case3_duplicates_and_stale_commands_never_execute_twice() {
    let mut h = Harness::started(no_clock(), 13);
    let open = ClientMessage::Command {
        seq: 1,
        base_revision: 0,
        command: DuelCommand::OpenSummon { cell: GridPos { column: 3, row: 5 } },
    };
    let replies = h.send(0, open.clone());
    assert_eq!(replies[0], ServerMessage::Accepted { seq: 1, revision: 1 });
    assert!(matches!(&replies[1], ServerMessage::Update { revision: 1, .. }));
    // Both seats received revision 1 exactly once.
    let updates = |h: &Harness, seat: usize| {
        h.inbox[seat]
            .iter()
            .filter_map(|m| match m {
                ServerMessage::Update { revision, .. } => Some(*revision),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(updates(&h, 0), vec![1]);
    assert_eq!(updates(&h, 1), vec![1]);
    let hash = state_hash(h.m.duel());
    // Retransmission: answered with the original decision, not executed.
    let replies = h.send(0, open.clone());
    assert_eq!(
        replies,
        vec![ServerMessage::Duplicate { seq: 1, decision: Decision::Accepted { revision: 1 } }]
    );
    assert_unchanged(&h, 1, &hash, 1);
    // A retransmitted rejection stays a rejection (same seq, now valid
    // content would not matter).
    let bad = ClientMessage::Command { seq: 2, base_revision: 1, command: DuelCommand::CancelSummon.clone() };
    let bad_choose = ClientMessage::Command { seq: 2, base_revision: 1, command: DuelCommand::ChooseSummon { hand: vec![9] } };
    assert!(matches!(h.send(0, bad_choose)[0], ServerMessage::Rejected { seq: 2, .. }));
    assert!(matches!(
        &h.send(0, bad)[0],
        ServerMessage::Duplicate { seq: 2, decision: Decision::Rejected { .. } }
    ));
    assert_unchanged(&h, 1, &hash, 1);
    // A lower seq is a duplicate too.
    assert!(matches!(h.send(0, open)[0], ServerMessage::Duplicate { seq: 1, .. }));
    // Stale: decided on revision 0, the state is at 1.
    let replies = h.send(
        0,
        ClientMessage::Command { seq: 3, base_revision: 0, command: DuelCommand::ChooseSummon { hand: vec![0] } },
    );
    assert_eq!(
        replies,
        vec![ServerMessage::Rejected {
            seq: 3,
            revision: 1,
            reason: Rejection::Stale { base_revision: 0, revision: 1 }
        }]
    );
    assert_unchanged(&h, 1, &hash, 1);
    // Accepted commands are numbered in order, each acknowledged once.
    let r = h.send(0, ClientMessage::Command { seq: 4, base_revision: 1, command: DuelCommand::ChooseSummon { hand: vec![0] } });
    assert_eq!(r[0], ServerMessage::Accepted { seq: 4, revision: 2 });
    let r = h.send(0, ClientMessage::Command { seq: 5, base_revision: 2, command: DuelCommand::EndTurn });
    assert_eq!(r[0], ServerMessage::Accepted { seq: 5, revision: 3 });
    assert_eq!(updates(&h, 1), vec![1, 2, 3]);
    let revisions: Vec<u64> = h.m.replay().entries.iter().map(|e| e.revision).collect();
    assert_eq!(revisions, vec![1, 2, 3]);
    // Seat 1's sequence numbers are independent of seat 0's.
    let r = h.send(1, ClientMessage::Command { seq: 1, base_revision: 3, command: DuelCommand::EndTurn });
    assert_eq!(r[0], ServerMessage::Accepted { seq: 1, revision: 4 });
}

// ---- Case 4 ----------------------------------------------------------------

#[test]
fn case4_disconnect_pauses_and_reconnect_restores_the_seat_view() {
    let mut h = Harness::started(OnlinePolicy { reconnect_grace: 1_000, turn_limit: Some(500), timeouts_before_forfeit: 3 }, 17);
    h.command(0, DuelCommand::OpenSummon { cell: GridPos { column: 3, row: 5 } });
    h.command(0, DuelCommand::ChooseSummon { hand: vec![0] });
    h.command(0, DuelCommand::EndTurn);
    let view_before = h.clients[1].view.clone().unwrap();
    // Seat 1 drops: seat 0 is told, the match is paused.
    h.now = 100;
    let out = h.m.disconnect(1, 100);
    h.take(out);
    assert!(matches!(h.inbox[0].last(), Some(ServerMessage::Paused { absent: [false, true], deadline: 1_100 })));
    assert_eq!(h.m.status(), MatchStatus::Paused { absent: [false, true] });
    // Nothing is accepted while paused (seat 0 off-turn anyway; seat 1's
    // own connection is gone). The turn clock is frozen: far beyond the
    // 500-tick turn limit, but within the grace, nothing happens.
    h.tick(1_000);
    assert_eq!(h.m.revision(), 3);
    // A wrong token is refused and takes no seat.
    assert_eq!(
        h.m.hello(&compat(), &Join::Resume { token: "guess".into() }, 1_000).unwrap_err(),
        Refusal::BadToken
    );
    assert_eq!(
        h.m.hello(&compat(), &Join::Seat { seat: None }, 1_000).unwrap_err(),
        Refusal::MatchFull
    );
    // The right token restores seat 1 with exactly its own view.
    h.inbox[1].clear();
    let (seat, out) = h.m.hello(&compat(), &Join::Resume { token: TOKENS[1].into() }, 1_050).unwrap();
    h.take(out);
    assert_eq!(seat, 1);
    let ServerMessage::Snapshot { snapshot } = &h.inbox[1][1] else { panic!("{:?}", h.inbox[1]) };
    assert_eq!(snapshot.view, view_before);
    assert_eq!(snapshot.view, player_view(h.m.duel(), 1));
    assert_eq!(snapshot.revision, 3);
    assert!(h.inbox[0].contains(&ServerMessage::Resumed));
    assert_eq!(h.m.status(), MatchStatus::Running);
    // The turn clock restarted from where the pause froze it (100 ticks
    // used before the pause): 399 more ticks are fine, the 400th expires.
    h.tick(1_449);
    assert_eq!(h.m.duel().active, 1);
    h.tick(1_450);
    assert_eq!(h.m.duel().active, 0, "the server ended seat 1's turn");
    let auto = h.m.replay().entries.last().unwrap();
    assert_eq!(auto.cause, Cause::TurnTimeout);
    assert_eq!(auto.action, ReplayAction::Command { side: 1, command: DuelCommand::EndTurn });
}

#[test]
fn case4_an_absent_seat_forfeits_after_the_grace_period() {
    let mut h = Harness::started(OnlinePolicy { reconnect_grace: 1_000, turn_limit: None, timeouts_before_forfeit: 3 }, 19);
    let out = h.m.disconnect(0, 10);
    h.take(out);
    h.tick(1_009);
    assert!(h.m.ended().is_none());
    h.tick(1_010);
    let (outcome, cause) = h.m.ended().unwrap();
    assert_eq!((outcome.winner, outcome.reason, cause), (Some(1), EndReason::Surrender, Cause::Abandoned));
    assert!(matches!(h.inbox[1].last(), Some(ServerMessage::Ended { cause: Cause::Abandoned, .. })));
    // Coming back afterwards shows the result; no new seat can join.
    let (_, out) = h.m.hello(&compat(), &Join::Resume { token: TOKENS[0].into() }, 2_000).unwrap();
    h.take(out);
    assert_eq!(h.clients[0].ended, Some((outcome, cause)));
    assert_eq!(h.m.hello(&compat(), &Join::Seat { seat: None }, 2_000).unwrap_err(), Refusal::MatchOver);
    assert_eq!(replay(h.m.replay(), &synthetic::catalogue(), &RulesTables::empty()).unwrap(), *h.m.duel());
}

#[test]
fn case4_repeated_turn_timeouts_forfeit_and_a_seat_may_concede_off_turn() {
    let mut h = Harness::started(OnlinePolicy { reconnect_grace: 1_000, turn_limit: Some(100), timeouts_before_forfeit: 2 }, 23);
    // Seat 0 opens a summon and idles: the server cancels it and ends the turn.
    h.command(0, DuelCommand::OpenSummon { cell: GridPos { column: 3, row: 5 } });
    h.tick(100);
    assert_eq!(h.m.duel().active, 1);
    let causes: Vec<_> = h.m.replay().entries.iter().map(|e| (e.cause, e.action.clone())).collect();
    assert_eq!(causes[1], (Cause::TurnTimeout, ReplayAction::Command { side: 0, command: DuelCommand::CancelSummon }));
    assert_eq!(causes[2], (Cause::TurnTimeout, ReplayAction::Command { side: 0, command: DuelCommand::EndTurn }));
    // Seat 1 acts in time (resetting nothing for seat 0), seat 0 idles again:
    // its second consecutive timeout forfeits.
    h.tick(150);
    h.command(1, DuelCommand::EndTurn);
    h.tick(249);
    assert!(h.m.ended().is_none());
    h.tick(250);
    let (outcome, cause) = h.m.ended().unwrap();
    assert_eq!((outcome.winner, cause), (Some(1), Cause::RepeatedTimeouts));
    assert_eq!(replay(h.m.replay(), &synthetic::catalogue(), &RulesTables::empty()).unwrap(), *h.m.duel());

    // Off-turn concession.
    let mut h = Harness::started(no_clock(), 29);
    let m = h.clients[1].forfeit();
    let r = h.send(1, m);
    assert!(matches!(r[0], ServerMessage::Accepted { .. }));
    assert_eq!(h.m.ended().map(|(o, c)| (o.winner, c)), Some((Some(0), Cause::Forfeit)));
    // Anything after the end is refused as finished.
    let m = h.clients[0].command(DuelCommand::EndTurn);
    assert!(matches!(h.send(0, m)[0], ServerMessage::Rejected { reason: Rejection::Finished, .. }));
}

// ---- Case 5 ----------------------------------------------------------------

#[test]
fn case5_incompatible_versions_fail_before_the_match_begins() {
    let mut h = Harness::new(no_clock(), 31);
    let mut wrong_data = synthetic::catalogue();
    wrong_data.cards[4].defense += 100;
    let mut old = compat();
    old.protocol -= 1;
    let mut modded = compat();
    modded.mods = vec!["hybrid-test".into()];
    let mut profile = compat();
    profile.rules_profile = "dotr-pal-variant".into();
    for (c, expect) in [
        (Compat::current(data_fingerprint(&wrong_data, &RulesTables::empty())), "GameData"),
        (old, "Protocol"),
        (modded, "Mods"),
        (profile, "RulesProfile"),
    ] {
        let err = h.m.hello(&c, &Join::Seat { seat: None }, 0).unwrap_err();
        let Refusal::Incompatible { detail } = err else { panic!("{err:?}") };
        assert!(format!("{detail:?}").starts_with(expect), "{detail:?}");
        if let Incompatible::GameData { client, server } = detail {
            assert_ne!(client, server);
        }
    }
    // No seat was taken and the match has not begun.
    assert!(!h.m.started());
    assert_eq!(h.m.status(), MatchStatus::Waiting);
    let (_, out) = h.m.hello(&compat(), &Join::Seat { seat: None }, 0).unwrap();
    h.take(out);
    let m = h.clients[0].command(DuelCommand::EndTurn);
    assert!(matches!(h.send(0, m)[0], ServerMessage::Rejected { reason: Rejection::NotStarted, .. }));
    let (seat, out) = h.m.hello(&compat(), &Join::Seat { seat: None }, 0).unwrap();
    h.take(out);
    assert_eq!(seat, 1);
    assert!(h.m.started());
}

#[test]
fn case5_replays_refuse_other_data_and_tampered_records() {
    let mut h = Harness::started(no_clock(), 37);
    h.play_out(&mut [Bot::new(), Bot::new()], 20_000);
    let record = h.m.replay().clone();
    // Through JSON (the stored form) it still reproduces the final state.
    let stored: crate::Replay = serde_json::from_slice(&serde_json::to_vec(&record).unwrap()).unwrap();
    assert_eq!(replay(&stored, &synthetic::catalogue(), &RulesTables::empty()).unwrap(), *h.m.duel());
    let mut other = synthetic::catalogue();
    other.cards[3].attack += 1;
    assert!(replay(&record, &other, &RulesTables::empty()).unwrap_err().contains("incompatible"));
    let mut tampered = record.clone();
    tampered.setup.seed ^= 1;
    assert!(replay(&tampered, &synthetic::catalogue(), &RulesTables::empty()).is_err());
    let mut truncated = record;
    truncated.entries.pop();
    assert!(replay(&truncated, &synthetic::catalogue(), &RulesTables::empty()).is_err());
}

#[test]
fn case5_results_do_not_depend_on_tick_granularity() {
    // The same intentions with the clock advanced in 1-tick steps or in one
    // jump give the same states (no frame-rate dependency).
    let run = |step: u64| {
        let mut h = Harness::started(OnlinePolicy { reconnect_grace: 10_000, turn_limit: Some(1_000), timeouts_before_forfeit: 9 }, 41);
        let mut t = 0;
        for _ in 0..3 {
            let target = t + 1_000;
            while t < target {
                t = (t + step).min(target);
                h.tick(t);
            }
        }
        (h.m.revision(), state_hash(h.m.duel()))
    };
    assert_eq!(run(1), run(1_000));
    assert_eq!(run(7), run(1_000));
    assert_eq!(run(1_000).0, 3, "three expired turns, each ended by the server");
}

// ---- Real data (the user's own import) -------------------------------------

#[test]
#[ignore = "Requires the user's duel import (ROSES_TEST_DUEL_IMPORT)"]
fn private_a_match_with_the_imported_first_duel_decks_completes() {
    let dir = std::path::PathBuf::from(std::env::var_os("ROSES_TEST_DUEL_IMPORT").unwrap());
    let import = roses_net::data::load_duel_import(&dir).unwrap();
    let cards = import.catalogue.clone();
    // The import's rules tables and the default switches (Leader Ability
    // USE): each submitted leader plays with its abilities.
    let tables = import.tables.clone();
    let compat = Compat::current(data_fingerprint(&cards, &tables));
    let decks = [0, 1].map(|s| roses_net::deck::from_import_side(&import.first_duel.sides[s]));
    let mut finished = 0;
    for seed in [1u64, 2, 3, 4, 5] {
        let mut m = OnlineMatch::with_submitted_decks("real", MatchSetup::from_import(&import.first_duel, seed), cards.clone(), no_clock(), TOKENS.map(String::from))
            .unwrap()
            .with_rules(tables.clone(), roses_net::MatchOptions::default())
            .unwrap();
        let mut clients = [ClientState::new(), ClientState::new()];
        for seat in 0..2u8 {
            let (s, out) = m.hello_with_deck(&compat, &Join::Seat { seat: Some(seat) }, Some(&decks[usize::from(seat)]), 0).unwrap();
            let _ = s;
            for (seat, msg) in out {
                clients[usize::from(seat)].apply(&msg);
            }
        }
        let mut bots = [Bot::new(), Bot::new()];
        let mut unsupported = 0;
        for _ in 0..20_000 {
            if m.ended().is_some() {
                break;
            }
            let seat = m.duel().active;
            let c = &mut clients[usize::from(seat)];
            let view = c.view.clone().unwrap();
            let msg = match bots[usize::from(seat)].next(&view, c.revision, &cards).unwrap() {
                BotAction::Command(cmd) => c.command(cmd),
                BotAction::Forfeit => c.forfeit(),
            };
            for (to, reply) in m.handle(seat, msg, 0) {
                if matches!(reply, ServerMessage::Rejected { reason: Rejection::Unsupported, .. }) {
                    unsupported += 1;
                }
                assert_eq!(serde_json::to_string(&reply).unwrap().contains("\"rng\""), false);
                if let ServerMessage::Update { view, revision, .. } = &reply {
                    if *revision == m.revision() {
                        assert_eq!(*view, player_view(&scramble_hidden(m.duel(), to, 999), to));
                        let mirror = roses_net::mirror_duel(view, &cards).unwrap();
                        assert_eq!(player_view(&mirror, to), *view);
                    }
                }
                clients[usize::from(to)].apply(&reply);
            }
        }
        let (outcome, cause) = m.ended().expect("the match ends");
        assert_eq!(replay(m.replay(), &cards, &tables).unwrap(), *m.duel());
        eprintln!("leader abilities {:?}", m.replay().setup.leader_abilities);
        eprintln!("seed {seed}: {outcome:?} {cause:?}, {} revisions, {unsupported} unsupported refusals", m.revision());
        finished += 1;
    }
    assert_eq!(finished, 5);
}

/// Every computer opponent's static deck (Weevil, Pegasus, Seto, every
/// campaign and Custom Duel opponent, with the leader rank the game gives
/// it) is accepted at join and plays a whole match against the server,
/// with Leader Ability USE (the hub's default): each deck against the next.
#[test]
#[ignore = "Requires the user's duel import (ROSES_TEST_DUEL_IMPORT)"]
fn private_every_opponent_deck_plays_a_whole_match() {
    let dir = std::path::PathBuf::from(std::env::var_os("ROSES_TEST_DUEL_IMPORT").unwrap());
    let import = roses_net::data::load_duel_import(&dir).unwrap();
    let duel_dir = if dir.join("SLES_515.79").is_file() { dir.clone() } else { dir.join("duel") };
    let opponents = roses_net::deck::load_opponent_decks(&duel_dir).unwrap();
    let cards = import.catalogue.clone();
    let tables = import.tables.clone();
    let compat = Compat::current(data_fingerprint(&cards, &tables));
    let n = opponents.len();
    for i in 0..n {
        let pair = [&opponents[i], &opponents[(i + 1) % n]];
        let decks = pair.map(|o| o.deck.clone());
        let mut m = OnlineMatch::with_submitted_decks("opp", MatchSetup::from_import(&import.first_duel, 11 + i as u64), cards.clone(), no_clock(), TOKENS.map(String::from))
            .unwrap()
            .with_rules(tables.clone(), roses_net::MatchOptions::default())
            .unwrap();
        let mut clients = [ClientState::new(), ClientState::new()];
        for seat in 0..2u8 {
            let (_, out) = m
                .hello_with_deck(&compat, &Join::Seat { seat: Some(seat) }, Some(&decks[usize::from(seat)]), 0)
                .unwrap_or_else(|e| panic!("{} refused: {e:?}", pair[usize::from(seat)].name));
            for (seat, msg) in out {
                clients[usize::from(seat)].apply(&msg);
            }
        }
        assert!(m.started(), "{} vs {}", pair[0].name, pair[1].name);
        let mut bots = [Bot::new(), Bot::new()];
        for _ in 0..40_000 {
            if m.ended().is_some() {
                break;
            }
            let seat = m.duel().active;
            let c = &mut clients[usize::from(seat)];
            let view = c.view.clone().unwrap();
            let msg = match bots[usize::from(seat)].next(&view, c.revision, &cards).unwrap() {
                BotAction::Command(cmd) => c.command(cmd),
                BotAction::Forfeit => c.forfeit(),
            };
            for (to, reply) in m.handle(seat, msg, 0) {
                if let ServerMessage::Update { view, revision, .. } = &reply {
                    if *revision == m.revision() {
                        // Hidden cards stay hidden (the other seat's deck too).
                        assert_eq!(*view, player_view(&scramble_hidden(m.duel(), to, 999), to));
                    }
                }
                clients[usize::from(to)].apply(&reply);
            }
        }
        let (outcome, cause) = m.ended().unwrap_or_else(|| panic!("{} vs {} did not end", pair[0].name, pair[1].name));
        eprintln!("{} vs {}: {outcome:?} {cause:?}, {} revisions", pair[0].name, pair[1].name, m.revision());
    }
}

// ---- Streamed presses, match options, limits --------------------------------

fn press(base: u64, through: u64, response: roses_net::PressResponse, input: roses_core::duel::ui::DuelUi) -> roses_net::Press {
    roses_net::Press { base_revision: base, through_revision: through, key: 0x2000, response, input, gap: 0 }
}

/// The acting seat's presses reach the other seat only (presentation, no
/// revision), only when they fit the match (its own revisions, the current
/// revision, its turn), and without a hidden card id.
#[test]
fn presses_reach_the_other_seat_only_when_they_fit() {
    use roses_core::duel::ui::{DetailsView, DuelUi, Mode};
    use roses_net::PressResponse::{Commands, Moved};
    let mut h = Harness::started(no_clock(), 5);
    assert_eq!(h.m.duel().active, 0);
    let relayed = |h: &mut Harness, seat: u8, p: roses_net::Press| {
        let before = h.inbox[usize::from(seat ^ 1)].len();
        let own = h.send(seat, ClientMessage::Presence { press: p });
        assert!(own.is_empty(), "nothing goes back to the sender: {own:?}");
        h.inbox[usize::from(seat ^ 1)][before..].to_vec()
    };
    let input = DuelUi::new(h.m.duel());
    // A press whose intention was accepted (the summon opened, the hand
    // refilled): relayed with its revision range.
    h.command(0, DuelCommand::OpenSummon { cell: GridPos { column: 3, row: 5 } });
    assert_eq!(h.m.revision(), 1);
    let mut after = input.clone();
    after.mode = Mode::Hand(Default::default());
    assert_eq!(relayed(&mut h, 0, press(0, 1, Commands, after.clone())).len(), 1);
    // Triangle on a hand card's details: the card id must not travel.
    after.details = Some(DetailsView { unit: None, card: h.m.duel().sides[0].hand[0], page: 0 });
    let got = relayed(&mut h, 0, press(1, 1, Moved, after.clone()));
    match &got[..] {
        [ServerMessage::Presence { seat: 0, press }] => {
            assert_eq!(press.input.details.unwrap().card, 0);
            assert_eq!(press.input.cursors, after.cursors);
            assert_eq!(press.input.mode, after.mode);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(h.m.revision(), 1, "a press changes nothing");
    // Not the acting seat, a stale base, a claim on revisions that do not exist.
    assert!(relayed(&mut h, 1, press(1, 1, Moved, after.clone())).is_empty());
    assert!(relayed(&mut h, 0, press(0, 0, Moved, after.clone())).is_empty());
    assert!(relayed(&mut h, 0, press(1, 3, Commands, after.clone())).is_empty());
    // ... and never one claiming the other seat's revisions.
    h.command(0, DuelCommand::CancelSummon);
    h.command(0, DuelCommand::EndTurn);
    let r = h.m.revision();
    h.command(1, DuelCommand::EndTurn);
    assert!(relayed(&mut h, 0, press(r, r + 1, Commands, input.clone())).is_empty());
    assert!(relayed(&mut h, 0, press(r - 1, r, Commands, input.clone())).len() == 1, "its own EndTurn");
}

/// The server's Custom Duel switches are told to both seats; with Leader
/// Ability USE each submitted leader gets its abilities at its rank from
/// the ability table (part of the fingerprint), NOT USE gives none; Card
/// Display OPEN sends that seat's hand to the other, HIDE never does.
#[test]
fn match_options_decide_leader_abilities_and_card_display() {
    let cards = synthetic::catalogue();
    let decks = synthetic::submissions();
    let mut rows = vec![[0xFFFFu16; 20]; 854];
    for d in &decks {
        rows[usize::from(d.leader)][0] = 1; // ability 1 at any rank
        rows[usize::from(d.leader)][3] = 0x0200; // ability 2 from rank 2
    }
    let tables = RulesTables { abilities: rows, destiny: None };
    for (options, expect) in [
        (roses_net::MatchOptions { leader_ability: [true, true], card_display_open: [false, true] }, [vec![1u8], vec![1, 2]]),
        (roses_net::MatchOptions { leader_ability: [false, true], card_display_open: [false, false] }, [vec![], vec![1, 2]]),
    ] {
        let mut m = OnlineMatch::with_submitted_decks("opts", setup(9), cards.clone(), no_clock(), TOKENS.map(String::from))
            .unwrap()
            .with_rules(tables.clone(), options)
            .unwrap();
        // A client without the table's fingerprint is refused.
        assert!(matches!(
            m.hello_with_deck(&compat(), &Join::Seat { seat: Some(0) }, Some(&decks[0]), 0),
            Err(Refusal::Incompatible { .. })
        ));
        let with_table = Compat::current(data_fingerprint(&cards, &tables));
        let mut inbox: [Vec<ServerMessage>; 2] = [vec![], vec![]];
        for seat in 0..2u8 {
            let (_, out) = m
                .hello_streaming(&with_table, &Join::Seat { seat: Some(seat) }, Some(&decks[usize::from(seat)]), seat == 0, 0)
                .unwrap();
            for (to, msg) in out {
                inbox[usize::from(to)].push(msg);
            }
        }
        assert_eq!(m.duel().sides[0].leader_abilities, expect[0]);
        assert_eq!(m.duel().sides[1].leader_abilities, expect[1]);
        assert_eq!(m.replay().setup.leader_abilities, expect);
        for seat in 0..2 {
            assert!(inbox[seat].iter().any(|msg| matches!(msg, ServerMessage::Welcome { options: o, .. } if *o == options)));
            let snapshot = inbox[seat].iter().rev().find_map(|msg| match msg {
                ServerMessage::Snapshot { snapshot } => Some(snapshot.clone()),
                _ => None,
            });
            let snapshot = snapshot.expect("a snapshot");
            assert_eq!(snapshot.presence, [true, false]);
            let other = 1 - seat;
            let open = options.card_display_open[other];
            assert_eq!(snapshot.view.open_hand.is_some(), open, "seat {seat}");
            if open {
                assert_eq!(snapshot.view.open_hand.as_ref(), Some(&m.duel().sides[other].hand));
            }
            assert_eq!(snapshot.view.public.leader_abilities, expect);
        }
        assert_eq!(replay(m.replay(), &cards, &tables).unwrap(), *m.duel());
    }
}

/// The per-connection budget: a burst of RATE_BURST, then RATE_PER_SECOND.
#[test]
fn the_message_budget_allows_bursts_and_a_steady_rate() {
    use crate::serve::{Budget, RATE_BURST, RATE_PER_SECOND};
    let mut b = Budget::new(0);
    for _ in 0..RATE_BURST {
        assert!(b.take(0));
    }
    assert!(!b.take(0));
    // One second refills a second's worth.
    let mut n = 0;
    while b.take(1000) {
        n += 1;
    }
    assert_eq!(n, RATE_PER_SECOND);
    // Long idleness refills no more than the burst.
    let mut n = 0;
    while b.take(1_000_000) {
        n += 1;
    }
    assert_eq!(n, RATE_BURST);
}

/// A seat that floods the server is disconnected (the match pauses; it may
/// resume), and the OS CSPRNG gives distinct 256-bit tokens.
#[test]
fn a_flooding_seat_is_cut_off() {
    use roses_net::transport::{memory_pair, recv_timeout, Connection, Received};
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;
    let mut game = OnlineMatch::new("flood", setup(3), synthetic::catalogue(), no_clock(), TOKENS.map(String::from)).unwrap();
    let (tx, mut rx) = mpsc::channel::<crate::serve::ServerConn>();
    let stop = AtomicBool::new(false);
    let mut log = Vec::new();
    std::thread::scope(|scope| {
        let server = scope.spawn(|| crate::serve::serve(&mut rx, &mut game, &stop, |l| log.push(l.to_string())).unwrap());
        let mut seats: Vec<_> = (0..2u8)
            .map(|seat| {
                let (mut client, server) = memory_pair::<ClientMessage, ServerMessage>();
                tx.send(Box::new(server)).unwrap();
                client
                    .send(&ClientMessage::Hello { compat: compat(), join: Join::Seat { seat: Some(seat) }, deck: None, presence: false, spectate: false })
                    .unwrap();
                client
            })
            .collect();
        for c in &mut seats {
            assert!(matches!(recv_timeout(c, std::time::Duration::from_secs(5)), Received::Message(ServerMessage::Welcome { .. })));
        }
        for _ in 0..(crate::serve::RATE_BURST * 3) {
            if seats[1].send(&ClientMessage::Sync).is_err() {
                break;
            }
        }
        let mut cut = false;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(10) && !cut {
            match seats[1].poll() {
                Received::Message(ServerMessage::ProtocolError { message }) => cut = message.contains("rate"),
                Received::Closed => cut = true,
                _ => std::thread::sleep(std::time::Duration::from_millis(1)),
            }
        }
        assert!(cut, "the flooding seat was not cut off");
        // Seat 0 sees the pause.
        let mut paused = false;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(10) && !paused {
            match seats[0].poll() {
                Received::Message(ServerMessage::Paused { absent, .. }) => paused = absent == [false, true],
                _ => std::thread::sleep(std::time::Duration::from_millis(1)),
            }
        }
        assert!(paused);
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        server.join().unwrap();
    });
    assert!(log.iter().any(|l| l.contains("rate")), "{log:?}");
    let (a, b) = (crate::secret_token(), crate::secret_token());
    assert_eq!(a.len(), 64);
    assert_ne!(a, b);
}
