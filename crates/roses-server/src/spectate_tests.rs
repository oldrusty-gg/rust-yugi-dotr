//! Spectating (a port addition): the match's spectator log and delay, the
//! presses and the "N watching" count ([`OnlineMatch`]), and the hub's
//! WATCH list, tickets, caps and opt-outs over in-memory connections.

use crate::friends_tests::{sees, until, yugi_compat, Running};
use crate::hub::{HubSettings, COMPUTER_NAME};
use crate::online::{OnlineMatch, Outbox};
use crate::play::{play_bot, PlayOptions, PlayReport};
use crate::replay::MatchSetup;
use roses_core::duel::{DuelCommand, DuelEvent};
use roses_core::GridPos;
use roses_net::client::{Bot, BotAction, ClientState};
use roses_net::filter::scramble_hidden_from_all;
use roses_net::friends::LobbyClient;
use roses_net::lobby::{LobbyProblem, LobbyRequest, PlayerState};
use roses_net::transport::{Connection, Received};
use roses_net::{
    spectator_view, synthetic, ClientMessage, Join, MatchOptions, MatchStatus, OnlinePolicy, PlayerView, Refusal, RulesTables,
    SeenEvent, ServerMessage,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A card id no synthetic card has.
const SENTINEL: u16 = 0xBEEF;

const OPEN: MatchOptions = MatchOptions { leader_ability: [true; 2], card_display_open: [true; 2] };

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

fn no_clock() -> OnlinePolicy {
    OnlinePolicy { turn_limit: None, ..OnlinePolicy::default() }
}

/// What a spectator's view must never hold: a hand, an open hand, a
/// face-down card's identity or its hidden details, a Destiny Draw byte.
fn assert_nobodys_view(v: &PlayerView) {
    assert!(v.spectator, "not a spectator's view");
    assert!(v.board.hand.is_empty() && v.open_hand.is_none(), "a hand went to a spectator");
    assert_eq!(v.public.own_destiny, 0);
    for (u, p) in v.board.units.iter().zip(&v.public.units) {
        assert_eq!(u.card.is_some(), p.committed_face_up, "unit {:?}: a face-down card's identity", u.id);
        if u.card.is_none() {
            assert!(p.record.is_none() && p.attack_bonus == 0 && p.defense_bonus == 0);
        }
    }
}

/// Every message a spectator received: only spectator messages, none with
/// anything hidden.
fn assert_nothing_hidden(messages: &[ServerMessage]) {
    for m in messages {
        let json = serde_json::to_string(m).unwrap();
        for forbidden in ["open_hand", "\"rng\"", "\"deck\"", "temp_units", "\"seed\"", "deck_sources", "final_state", "resume_token"] {
            assert!(!json.contains(forbidden), "{forbidden} sent to a spectator: {json}");
        }
        match m {
            ServerMessage::Snapshot { snapshot } => {
                assert_eq!(snapshot.last_seq, 0);
                assert_nobodys_view(&snapshot.view);
            }
            ServerMessage::Update { view, events, .. } => {
                assert_nobodys_view(view);
                for e in events {
                    let SeenEvent::Public { event } = e else { continue };
                    match event {
                        DuelEvent::Drew { .. } | DuelEvent::DestinyDraw { .. } | DuelEvent::Fused { .. } | DuelEvent::Equipped { .. } => {
                            panic!("a hidden event went to a spectator: {event:?}")
                        }
                        DuelEvent::Summoned { unit, .. } | DuelEvent::Created { unit, .. } => {
                            let shown = view.board.units.iter().find(|u| u.id == *unit).is_none_or(|u| u.card.is_some());
                            let revealed = events.iter().any(|x| {
                                matches!(x, SeenEvent::Public { event: DuelEvent::Revealed { unit: r, .. } | DuelEvent::Destroyed { unit: r, .. } } if r == unit)
                            });
                            assert!(shown || revealed, "a face-down card went to a spectator: {event:?}");
                        }
                        _ => {}
                    }
                }
            }
            ServerMessage::Watching { .. }
            | ServerMessage::Watchers { .. }
            | ServerMessage::Presence { .. }
            | ServerMessage::Paused { .. }
            | ServerMessage::Resumed
            | ServerMessage::Ended { .. }
            | ServerMessage::Refused { .. } => {}
            other => panic!("a seat's message went to a spectator: {other:?}"),
        }
    }
}

// ---- the match: the spectator log, the delay, presses, the count -------------

/// A match whose seats play computer intentions, with the spectators'
/// messages collected (as the hub fans them out).
struct Driven {
    m: OnlineMatch,
    clients: [ClientState; 2],
    seats: [Vec<ServerMessage>; 2],
    spectators: Vec<ServerMessage>,
}

impl Driven {
    fn new(seed: u64, options: MatchOptions, delay: u8, spectate: [bool; 2]) -> Self {
        let m = OnlineMatch::new("spectated", setup(seed), synthetic::catalogue(), no_clock(), ["t0".into(), "t1".into()])
            .unwrap()
            .with_rules(RulesTables::empty(), options)
            .unwrap()
            .with_spectator_delay(delay);
        let mut d = Self { m, clients: [ClientState::new(), ClientState::new()], seats: [Vec::new(), Vec::new()], spectators: Vec::new() };
        for seat in 0..2u8 {
            let (_, out) = d
                .m
                .hello_spectating(&yugi_compat(), &Join::Seat { seat: Some(seat) }, None, true, spectate[usize::from(seat)], 0)
                .unwrap();
            d.take(out);
        }
        d
    }

    fn take(&mut self, out: Outbox) {
        for (seat, msg) in out {
            self.clients[usize::from(seat)].apply(&msg);
            self.seats[usize::from(seat)].push(msg);
        }
        let spectator = self.m.take_spectator_messages();
        // At every revision (no delay) the spectators' view is the view of
        // nobody of the authoritative duel, whatever is hidden.
        for msg in &spectator {
            if let ServerMessage::Update { revision, view, .. } = msg {
                if *revision == self.m.revision() && self.m.spectator_delay() == 0 {
                    assert_eq!(*view, spectator_view(self.m.duel()));
                    let scrambled = spectator_view(&scramble_hidden_from_all(self.m.duel(), SENTINEL));
                    assert_eq!(*view, scrambled);
                }
            }
        }
        self.spectators.extend(spectator);
    }

    fn step(&mut self, bots: &mut [Bot; 2]) -> bool {
        if self.m.ended().is_some() {
            return false;
        }
        let cards = synthetic::catalogue();
        let seat = self.m.duel().active;
        let c = &mut self.clients[usize::from(seat)];
        let view = c.view.clone().unwrap();
        let msg = match bots[usize::from(seat)].next(&view, c.revision, &cards) {
            Some(BotAction::Command(cmd)) => c.command(cmd),
            _ => c.forfeit(),
        };
        let out = self.m.handle(seat, msg, 0);
        self.take(out);
        true
    }
}

/// The spectator log without a delay: a snapshot at the start, then one
/// update per revision in order (each the view of nobody, independent of
/// every hidden card, even with both sides OPEN), then the end; nothing is
/// kept back and the replay is never part of it.
#[test]
fn the_spectator_log_follows_every_revision_with_the_view_of_nobody() {
    for seed in [3u64, 4] {
        let mut d = Driven::new(seed, OPEN, 0, [false; 2]);
        let start = d.m.spectator_snapshot().expect("started");
        assert_eq!(start.revision, 0);
        assert_nobodys_view(&start.view);
        // Seats under OPEN do see the other hand (the switch is live).
        assert!(d.clients[0].view.as_ref().unwrap().open_hand.is_some());
        let mut bots = [Bot::new(), Bot::new()];
        let mut steps = 0;
        while d.step(&mut bots) {
            steps += 1;
            assert!(steps < 20_000);
            assert_eq!(d.m.spectator_backlog(), 0);
        }
        let revisions: Vec<u64> = d
            .spectators
            .iter()
            .filter_map(|m| match m {
                ServerMessage::Update { revision, .. } => Some(*revision),
                _ => None,
            })
            .collect();
        assert_eq!(revisions, (1..=d.m.revision()).collect::<Vec<_>>());
        assert!(matches!(d.spectators.last(), Some(ServerMessage::Ended { .. })));
        assert_nothing_hidden(&d.spectators);
        // A late joiner's snapshot is the end.
        let end = d.m.spectator_snapshot().unwrap();
        assert_eq!(end.revision, d.m.revision());
        assert!(matches!(end.status, MatchStatus::Ended { .. }));
    }
}

/// With a delay of one turn a turn's revisions reach spectators only once
/// the next turn has begun (a late joiner's snapshot lags the same way), no
/// press is relayed to them, and the end flushes everything.
#[test]
fn a_spectator_delay_holds_each_turn_back_until_the_next_begins() {
    let mut d = Driven::new(6, MatchOptions::default(), 1, [false; 2]);
    let cards = synthetic::catalogue();
    assert_eq!(d.m.set_watchers(1), Vec::new(), "no seat reads the count here");
    d.spectators.clear();
    // Side 0's first turn: open a summon, summon, nothing released yet.
    let c = d.clients[0].command(DuelCommand::OpenSummon { cell: GridPos { column: 3, row: 5 } });
    let out = d.m.handle(0, c, 0);
    d.take(out);
    let c = d.clients[0].command(DuelCommand::ChooseSummon { hand: vec![0] });
    let out = d.m.handle(0, c, 0);
    d.take(out);
    assert_eq!(d.m.revision(), 2);
    assert!(d.spectators.iter().all(|m| !matches!(m, ServerMessage::Update { .. })), "held back: {:?}", d.spectators);
    assert_eq!(d.m.spectator_snapshot().unwrap().revision, 0, "a late joiner sees the start");
    // A press of side 0 is not relayed to spectators while delayed.
    let input = roses_core::duel::ui::DuelUi::new(d.m.duel());
    let press = roses_net::Press { base_revision: 2, through_revision: 2, key: 0x2000, response: roses_net::PressResponse::Moved, input, gap: 0 };
    let out = d.m.handle(0, ClientMessage::Presence { press }, 0);
    d.take(out);
    assert!(d.spectators.iter().all(|m| !matches!(m, ServerMessage::Presence { .. })));
    // The turn ends: side 1's turn begins, side 0's revisions go out.
    let c = d.clients[0].command(DuelCommand::EndTurn);
    let out = d.m.handle(0, c, 0);
    d.take(out);
    let released: Vec<u64> = d.spectators.iter().filter_map(|m| match m { ServerMessage::Update { revision, .. } => Some(*revision), _ => None }).collect();
    assert_eq!(released, [1, 2]);
    assert_eq!(d.m.spectator_backlog(), 1, "the end-turn revision opens the next turn and waits");
    assert_eq!(d.m.spectator_snapshot().unwrap().revision, 2);
    // Play on: the end releases everything, then the end message.
    let mut bots = [Bot::new(), Bot::new()];
    while d.step(&mut bots) {}
    let _ = cards;
    let revisions: Vec<u64> = d.spectators.iter().filter_map(|m| match m { ServerMessage::Update { revision, .. } => Some(*revision), _ => None }).collect();
    assert_eq!(revisions, (1..=d.m.revision()).collect::<Vec<_>>());
    assert!(matches!(d.spectators.last(), Some(ServerMessage::Ended { .. })));
    assert_eq!(d.m.spectator_backlog(), 0);
    assert_nothing_hidden(&d.spectators);
}

/// Without a delay the acting seat's presses also reach spectators, with
/// the details card removed against the view of nobody (even under OPEN,
/// where the other seat does get it); the "N watching" count goes to the
/// seats that read it only (an older client's hello never asks).
#[test]
fn presses_and_the_watching_count_reach_spectators_and_new_clients_only() {
    use roses_core::duel::ui::{DetailsView, DuelUi, Mode};
    let mut d = Driven::new(5, OPEN, 0, [false, true]);
    // No spectators: a press goes to the other seat only.
    let c = d.clients[0].command(DuelCommand::OpenSummon { cell: GridPos { column: 3, row: 5 } });
    let out = d.m.handle(0, c, 0);
    d.take(out);
    let mut input = DuelUi::new(d.m.duel());
    input.mode = Mode::Hand(Default::default());
    input.details = Some(DetailsView { unit: None, card: d.m.duel().sides[0].hand[0], page: 0 });
    let press = roses_net::Press { base_revision: 1, through_revision: 1, key: 0x2000, response: roses_net::PressResponse::Moved, input, gap: 0 };
    let out = d.m.handle(0, ClientMessage::Presence { press: press.clone() }, 0);
    d.take(out);
    assert!(d.spectators.iter().all(|m| !matches!(m, ServerMessage::Presence { .. })));
    // One spectator: seat 1 (a new client) is told, seat 0 (as an older
    // client) is not; spectators get the count too.
    let out = d.m.set_watchers(1);
    assert_eq!(out, vec![(1, ServerMessage::Watchers { count: 1 })]);
    d.take(out);
    assert!(d.spectators.contains(&ServerMessage::Watchers { count: 1 }));
    let out = d.m.handle(0, ClientMessage::Presence { press: press.clone() }, 0);
    d.take(out);
    let to_spectators: Vec<_> = d.spectators.iter().filter_map(|m| match m { ServerMessage::Presence { seat, press } => Some((*seat, press.clone())), _ => None }).collect();
    assert_eq!(to_spectators.len(), 1);
    assert_eq!(to_spectators[0].1.input.details.unwrap().card, 0, "no hand card's identity for spectators");
    let to_seat = d.seats[1].iter().rev().find_map(|m| match m { ServerMessage::Presence { press, .. } => Some(press.clone()), _ => None }).unwrap();
    assert_eq!(to_seat.input.details.unwrap().card, d.m.duel().sides[0].hand[0], "the OPEN seat still sees it");
    // A seat that resumes with a hello that reads the count is told at once.
    let (_, out) = d.m.hello_spectating(&yugi_compat(), &Join::Resume { token: "t0".into() }, None, true, true, 0).unwrap();
    assert!(out.contains(&(0, ServerMessage::Watchers { count: 1 })));
    assert!(d.seats[0].iter().all(|m| !matches!(m, ServerMessage::Watchers { .. })), "the older hello was never sent it");
    assert_eq!(d.m.set_watchers(0), vec![(0, ServerMessage::Watchers { count: 0 }), (1, ServerMessage::Watchers { count: 0 })]);
    assert_nothing_hidden(&d.spectators);
}

// ---- the hub ---------------------------------------------------------------------

/// What one spectator received.
struct Watched {
    messages: Vec<ServerMessage>,
    closed: bool,
}

/// Watch with `ticket` on a new match connection until the match ends, the
/// connection closes, or `leave_after` updates (then it leaves). `joined`
/// is set once the snapshot arrived.
fn watch(hub: &Running, ticket: String, joined: Arc<AtomicBool>, leave_after: Option<usize>) -> std::thread::JoinHandle<Watched> {
    let mut connect = hub.match_connector();
    std::thread::spawn(move || {
        let mut conn = connect().unwrap();
        conn.send(&ClientMessage::hello(yugi_compat(), Join::Watch { ticket }, None, false)).unwrap();
        let mut w = Watched { messages: Vec::new(), closed: false };
        let start = Instant::now();
        let mut updates = 0;
        loop {
            assert!(start.elapsed() < Duration::from_secs(60), "the spectator timed out: {:?}", w.messages.last());
            match conn.poll() {
                Received::Message(m) => {
                    let end = matches!(m, ServerMessage::Ended { .. } | ServerMessage::Refused { .. });
                    if matches!(m, ServerMessage::Snapshot { .. }) {
                        joined.store(true, Ordering::Relaxed);
                    }
                    if matches!(m, ServerMessage::Update { .. }) {
                        updates += 1;
                    }
                    w.messages.push(m);
                    if end {
                        return w;
                    }
                    if leave_after.is_some_and(|n| updates >= n) {
                        conn.close();
                        return w;
                    }
                }
                Received::Empty => std::thread::sleep(Duration::from_millis(1)),
                Received::Closed => {
                    w.closed = true;
                    return w;
                }
                Received::Malformed(e) => panic!("{e}"),
            }
        }
    })
}

fn wait_flag(flag: &AtomicBool, what: &str) {
    let start = Instant::now();
    while !flag.load(Ordering::Relaxed) {
        assert!(start.elapsed() < Duration::from_secs(20), "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Ask for the WATCH list until `done` holds.
fn list_until(c: &mut LobbyClient, what: &str, done: impl Fn(&[roses_net::lobby::LiveMatch]) -> bool) {
    let start = Instant::now();
    loop {
        c.live = None;
        c.send(LobbyRequest::ListMatches);
        until(c, "a list", |c| c.live.is_some());
        if done(c.live.as_deref().unwrap()) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "timed out waiting for the list: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn sign_up(hub: &Running, name: &str, invite: &str) -> LobbyClient {
    let mut c = hub.lobby();
    c.send(LobbyRequest::SignUp { username: name.into(), password: format!("{name} secret pass"), invite: invite.into() });
    until(&mut c, "sign-up", |c| c.user.is_some());
    c
}

/// Play the seat of `ready` with a pad-free computer client (waiting for
/// `gate`), as a client that reads the spectating messages or as an older
/// one.
fn play(hub: &Running, ticket: String, seat: u8, gate: Arc<AtomicBool>, spectate: bool) -> std::thread::JoinHandle<PlayReport> {
    let connect = hub.match_connector();
    std::thread::spawn(move || {
        play_bot(
            connect,
            &yugi_compat(),
            &synthetic::catalogue(),
            PlayOptions {
                join: Join::Ticket { ticket },
                deck: Some(synthetic::submissions()[usize::from(seat)].clone()),
                gate: Some(gate),
                spectate,
                timeout: Duration::from_secs(90),
                ..PlayOptions::default()
            },
            |_| {},
        )
        .unwrap()
    })
}

/// A whole match against the server's computer, OPEN on both sides, is
/// listed (names, round, spectators), watched from a mid-match snapshot to
/// the end without a hand or a face-down card, the player (a new client) is
/// told how many watch, and the ended match leaves the list.
#[test]
fn a_spectator_watches_a_whole_computer_match_and_never_sees_hidden_cards() {
    let settings = HubSettings { options: OPEN, ..HubSettings::default() };
    let (hub, invite) = Running::with_settings(true, 4, settings, None);
    let mut joey = sign_up(&hub, "Joey", &invite);
    let mut mai = sign_up(&hub, "Mai", &invite);
    assert!(mai.spectate, "the welcome offers spectating");
    list_until(&mut mai, "nothing yet", |l| l.is_empty());
    joey.send(LobbyRequest::PlayComputer { map: None, computer: None });
    until(&mut joey, "match ready", |c| c.ready.is_some());
    let ready = joey.ready.clone().unwrap();
    let gate = Arc::new(AtomicBool::new(false));
    let player = play(&hub, ready.ticket.clone(), ready.seat, gate.clone(), true);
    list_until(&mut mai, "the match", |l| l.iter().any(|m| m.match_id == ready.match_id));
    let listed = mai.live.clone().unwrap().into_iter().find(|m| m.match_id == ready.match_id).unwrap();
    let mut names = ["Joey".to_string(), COMPUTER_NAME.to_string()];
    if ready.seat == 1 {
        names.swap(0, 1);
    }
    assert_eq!((listed.players.clone(), listed.watching, listed.round), (names.clone(), 0, 1));
    // Joey cannot watch his own match; Mai can.
    joey.send(LobbyRequest::Watch { match_id: ready.match_id.clone() });
    until(&mut joey, "own match", |c| c.note.as_deref() == Some(&*LobbyProblem::NotWatchable.to_string()));
    mai.send(LobbyRequest::Watch { match_id: ready.match_id.clone() });
    until(&mut mai, "watch ticket", |c| c.watch.is_some());
    let w = mai.watch.clone().unwrap();
    assert_eq!((w.players.clone(), w.options), (names.clone(), OPEN));
    let joined = Arc::new(AtomicBool::new(false));
    let spectator = watch(&hub, w.ticket, joined.clone(), None);
    wait_flag(&joined, "the spectator's snapshot");
    list_until(&mut mai, "one watching", |l| l.iter().any(|m| m.match_id == ready.match_id && m.watching == 1));
    gate.store(true, Ordering::Relaxed);
    let report = player.join().unwrap();
    let watched = spectator.join().unwrap();
    assert!(!watched.closed);
    let m = &watched.messages;
    assert!(matches!(&m[0], ServerMessage::Watching { names: n, delay_turns: 0, .. } if *n == names), "{:?}", m[0]);
    let ServerMessage::Snapshot { snapshot } = &m[1] else { panic!("{:?}", m[1]) };
    assert_nothing_hidden(m);
    // The updates follow the snapshot one by one to the player's last.
    let revisions: Vec<u64> = m.iter().filter_map(|x| match x { ServerMessage::Update { revision, .. } => Some(*revision), _ => None }).collect();
    assert_eq!(revisions, (snapshot.revision + 1..=report.state.revision).collect::<Vec<_>>());
    assert!(revisions.len() > 20, "{} updates watched", revisions.len());
    let ServerMessage::Ended { outcome, .. } = m.last().unwrap() else { panic!("{:?}", m.last()) };
    assert_eq!(Some(*outcome), report.state.ended.map(|e| e.0));
    // What the spectator knows of the board is what the player saw.
    let last_view = m.iter().rev().find_map(|x| match x { ServerMessage::Update { view, .. } => Some(view), _ => None }).unwrap();
    let player_view = report.state.view.as_ref().unwrap();
    assert_eq!(last_view.board.lp, player_view.board.lp);
    for (s, p) in last_view.board.units.iter().zip(&player_view.board.units) {
        assert!(s.card.is_none() || s.card == p.card);
    }
    // The player saw the other side's hand (OPEN); the spectator never did.
    assert!(report.received.iter().any(|x| matches!(x, ServerMessage::Update { view, .. } if view.open_hand.is_some())));
    // The player (reading the count) was told someone watched.
    assert!(report.received.contains(&ServerMessage::Watchers { count: 1 }));
    until(&mut joey, "match over", |c| c.over.is_some());
    list_until(&mut mai, "the match gone", |l| l.iter().all(|m| m.match_id != ready.match_id));
    mai.send(LobbyRequest::Watch { match_id: ready.match_id.clone() });
    until(&mut mai, "no such match", |c| c.note.as_deref() == Some(&*LobbyProblem::NoSuchMatch.to_string()));
    let _ = hub.stop();
}

/// Caps per match and per server, a spectator leaving mid-match (the
/// players are told the new count; an older client is never told), a
/// private room's match unlisted and unwatchable, a bad ticket, and a
/// player turning "Allow spectators" off (listed no more, watchers sent
/// away, new ones refused) and on again.
#[test]
fn caps_opt_outs_private_rooms_and_leaving_spectators() {
    let settings = HubSettings { max_spectators_per_match: 2, max_spectators: 3, ..HubSettings::default() };
    let (hub, invite) = Running::with_settings(true, 12, settings, None);
    let mut yugi = sign_up(&hub, "Yugi", &invite);
    let mut kaiba = sign_up(&hub, "Kaiba", &invite);
    let mut joey = sign_up(&hub, "Joey", &invite);
    let mut watchers: Vec<LobbyClient> = ["Tea", "Bakura", "Marik", "Ishizu"].iter().map(|n| sign_up(&hub, n, &invite)).collect();
    // Yugi vs Kaiba (a challenge: listed); Joey vs the computer.
    until(&mut yugi, "Kaiba online", |c| sees(c, "Kaiba", PlayerState::Idle));
    yugi.send(LobbyRequest::Challenge { to: "Kaiba".into(), map: None });
    until(&mut kaiba, "challenge", |c| !c.incoming.is_empty());
    let id = kaiba.incoming[0].0;
    kaiba.send(LobbyRequest::Answer { id, accept: true });
    until(&mut yugi, "ready", |c| c.ready.is_some());
    until(&mut kaiba, "ready", |c| c.ready.is_some());
    joey.send(LobbyRequest::PlayComputer { map: None, computer: None });
    until(&mut joey, "ready", |c| c.ready.is_some());
    let gate = Arc::new(AtomicBool::new(false));
    let (ry, rk, rj) = (yugi.ready.clone().unwrap(), kaiba.ready.clone().unwrap(), joey.ready.clone().unwrap());
    // Yugi plays as an older client; Kaiba and Joey read the count.
    let players = [
        play(&hub, ry.ticket.clone(), ry.seat, gate.clone(), false),
        play(&hub, rk.ticket.clone(), rk.seat, gate.clone(), true),
        play(&hub, rj.ticket.clone(), rj.seat, gate.clone(), true),
    ];
    let m1 = ry.match_id.clone();
    let m2 = rj.match_id.clone();
    list_until(&mut watchers[0], "both matches", |l| l.len() == 2);
    // A bad ticket.
    let refused = watch(&hub, "guess".into(), Arc::new(AtomicBool::new(false)), None).join().unwrap();
    assert!(matches!(refused.messages[..], [ServerMessage::Refused { reason: Refusal::BadToken }]));
    // Two may watch m1; the third is refused (per match).
    let mut tickets = Vec::new();
    for w in watchers.iter_mut().take(2) {
        w.send(LobbyRequest::Watch { match_id: m1.clone() });
        until(w, "ticket", |c| c.watch.is_some());
        tickets.push(w.watch.take().unwrap().ticket);
    }
    let flags: Vec<_> = (0..2).map(|_| Arc::new(AtomicBool::new(false))).collect();
    let tea = watch(&hub, tickets[0].clone(), flags[0].clone(), Some(1));
    let bakura = watch(&hub, tickets[1].clone(), flags[1].clone(), None);
    wait_flag(&flags[0], "Tea watching");
    wait_flag(&flags[1], "Bakura watching");
    watchers[2].send(LobbyRequest::Watch { match_id: m1.clone() });
    until(&mut watchers[2], "full", |c| c.note.as_deref() == Some(&*LobbyProblem::WatchersFull.to_string()));
    // A third on m2 (three on the server), then the server is full.
    watchers[2].send(LobbyRequest::Watch { match_id: m2.clone() });
    until(&mut watchers[2], "ticket", |c| c.watch.is_some());
    let marik_flag = Arc::new(AtomicBool::new(false));
    let marik = watch(&hub, watchers[2].watch.take().unwrap().ticket, marik_flag.clone(), None);
    wait_flag(&marik_flag, "Marik watching");
    watchers[3].send(LobbyRequest::Watch { match_id: m2.clone() });
    until(&mut watchers[3], "server full", |c| c.note.as_deref() == Some(&*LobbyProblem::WatchersFull.to_string()));
    list_until(&mut watchers[3], "counts", |l| {
        l.iter().any(|m| m.match_id == m1 && m.watching == 2) && l.iter().any(|m| m.match_id == m2 && m.watching == 1)
    });
    // Play starts; Tea leaves after one update: m1 has one spectator again.
    gate.store(true, Ordering::Relaxed);
    let tea = tea.join().unwrap();
    assert!(!tea.closed);
    assert_nothing_hidden(&tea.messages);
    list_until(&mut watchers[3], "Tea gone", |l| l.iter().any(|m| m.match_id == m1 && m.watching == 1));
    // A private room's match: not listed, not watchable.
    let mut mai = sign_up(&hub, "Mai", &invite);
    let mut ishizu = sign_up(&hub, "Odion", &invite);
    mai.send(LobbyRequest::CreateRoom { map: None });
    until(&mut mai, "room", |c| c.room.is_some());
    ishizu.send(LobbyRequest::JoinRoom { code: mai.room.clone().unwrap() });
    until(&mut mai, "room match", |c| c.ready.is_some());
    let room = mai.ready.clone().unwrap();
    let room_gate = Arc::new(AtomicBool::new(false));
    let room_players = [
        play(&hub, room.ticket.clone(), room.seat, room_gate.clone(), true),
        {
            until(&mut ishizu, "room match", |c| c.ready.is_some());
            let r = ishizu.ready.clone().unwrap();
            play(&hub, r.ticket, r.seat, room_gate.clone(), true)
        },
    ];
    std::thread::sleep(Duration::from_millis(100));
    list_until(&mut watchers[3], "no room match", |l| l.iter().all(|m| m.match_id != room.match_id));
    watchers[3].send(LobbyRequest::Watch { match_id: room.match_id.clone() });
    until(&mut watchers[3], "private", |c| c.note.as_deref() == Some(&*LobbyProblem::NotWatchable.to_string()));
    room_gate.store(true, Ordering::Relaxed);
    // Joey turns spectators off: Marik is sent away, m2 is no longer listed.
    joey.send(LobbyRequest::AllowSpectators { allow: false });
    let marik = marik.join().unwrap();
    assert!(
        marik.messages.iter().any(|m| matches!(m, ServerMessage::Refused { reason: Refusal::NotWatchable })),
        "{:?}",
        marik.messages.last()
    );
    assert_nothing_hidden(&marik.messages);
    list_until(&mut watchers[3], "m2 unlisted", |l| l.iter().all(|m| m.match_id != m2));
    watchers[3].note = None;
    watchers[3].send(LobbyRequest::Watch { match_id: m2.clone() });
    until(&mut watchers[3], "opted out", |c| c.note.as_deref() == Some(&*LobbyProblem::NotWatchable.to_string()));
    joey.send(LobbyRequest::AllowSpectators { allow: true });
    // Every match ends; Bakura watched m1 to its end.
    let reports: Vec<PlayReport> = players.into_iter().map(|p| p.join().unwrap()).collect();
    for p in room_players {
        p.join().unwrap();
    }
    let bakura = bakura.join().unwrap();
    assert!(matches!(bakura.messages.last(), Some(ServerMessage::Ended { .. })));
    assert_nothing_hidden(&bakura.messages);
    // Yugi (an older client) was never sent the count; Kaiba was, and saw
    // it go 1, 2, 1 as Tea and Bakura came and Tea left.
    assert!(reports[0].received.iter().all(|m| !matches!(m, ServerMessage::Watchers { .. })));
    let counts: Vec<u16> = reports[1].received.iter().filter_map(|m| match m { ServerMessage::Watchers { count } => Some(*count), _ => None }).collect();
    assert_eq!(&counts[..3], &[1, 2, 1], "{counts:?}");
    assert!(reports[2].received.iter().any(|m| matches!(m, ServerMessage::Watchers { count: 0 })), "Joey saw Marik sent away");
    let _ = hub.stop();
}

/// A hub that does not offer spectating (no spectators allowed) says so in
/// its welcome, as an older hub's welcome does.
#[test]
fn a_hub_without_spectators_does_not_offer_them() {
    let settings = HubSettings { max_spectators: 0, ..HubSettings::default() };
    let (hub, invite) = Running::with_settings(false, 1, settings, None);
    let c = sign_up(&hub, "Rex", &invite);
    assert!(!c.spectate);
    let _ = hub.stop();
}

/// A client of this build on a hub from before spectating (loopback,
/// unencrypted, open sign-up, synthetic data, with its computer):
/// `ROSES_OLDER_HUB=127.0.0.1:<port>`. The welcome offers no spectating (so
/// the game shows no WATCH and never sends the new requests), and a whole
/// match is played with the new hello (`spectate: true`), which the older
/// hub reads as before.
#[test]
#[ignore = "Needs an older roses-server hub on loopback (ROSES_OLDER_HUB)"]
fn private_this_client_plays_on_an_older_hub() {
    let Some(addr) = std::env::var_os("ROSES_OLDER_HUB").and_then(|a| a.into_string().ok()) else { return };
    let config = roses_net::friends::OnlineConfig { server: addr, name: None, tls: false, pin: None };
    let conn = roses_net::friends::connect::<LobbyRequest, roses_net::lobby::LobbyEvent>(&config).unwrap();
    let mut c = LobbyClient::start(conn, yugi_compat(), "spectate compat test").unwrap();
    let name = format!("Compat{}", crate::random_u64() % 100_000);
    c.send(LobbyRequest::SignUp { username: name, password: "compat secret pass".into(), invite: String::new() });
    until(&mut c, "sign-up", |c| c.user.is_some() || c.auth_problem.is_some());
    assert!(c.user.is_some(), "{:?}", c.auth_problem);
    assert!(!c.spectate, "an older hub does not offer spectating");
    c.send(LobbyRequest::PlayComputer { map: None, computer: None });
    until(&mut c, "match ready", |c| c.ready.is_some());
    let ready = c.ready.clone().unwrap();
    let report = play_bot(
        || roses_net::friends::connect::<ClientMessage, ServerMessage>(&config),
        &yugi_compat(),
        &synthetic::catalogue(),
        PlayOptions {
            join: Join::Ticket { ticket: ready.ticket },
            deck: Some(synthetic::submissions()[usize::from(ready.seat)].clone()),
            spectate: true,
            ..PlayOptions::default()
        },
        |_| {},
    )
    .unwrap();
    assert!(report.state.ended.is_some());
    assert!(report.received.iter().all(|m| !matches!(m, ServerMessage::Watchers { .. } | ServerMessage::Watching { .. })));
    until(&mut c, "match over", |c| c.over.is_some());
    eprintln!("played a whole match on the older hub: {:?} after {} revisions", report.state.ended, report.state.revision);
}
