//! MODES (a port addition): N-player matches ([`MultiMatch`]) and the
//! hub's N-player rooms, over in-memory connections.

use crate::friends_tests::{until, yugi_compat, Running};
use crate::hub::HubSettings;
use crate::multi::{replay_multi, MultiMatch, MultiSeatPlan};
use crate::online::Outbox;
use crate::play::{play_bot, PlayOptions};
use roses_core::duel::{EndReason, MultiMode};
use roses_net::client::ClientState;
use roses_net::lobby::{
    LobbyProblem, LobbyRequest, MatchResult, RoomMode, RoomSettings, ROOM_TURN_SECONDS,
};
use roses_net::{
    synthetic, Cause, ClientMessage, DeckSubmission, Join, MatchStatus, OnlinePolicy, RulesTables, SeatInfo, ServerMessage,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn deck(i: usize) -> DeckSubmission {
    synthetic::submissions()[i % 2].clone()
}

fn settings(mode: RoomMode) -> RoomSettings {
    RoomSettings { mode, turn_seconds: 0, ..RoomSettings::default() }
}

/// A match of `humans` players (seats 0..humans) and computers after them.
fn game(humans: usize, settings: RoomSettings, seed: u64) -> MultiMatch {
    let team = |s: usize| if settings.mode == RoomMode::Teams { (s / 2) as u8 } else { s as u8 };
    let seats = (0..4)
        .map(|s| MultiSeatPlan {
            info: SeatInfo { name: format!("P{s}"), computer: s >= humans, team: team(s) },
            deck: (s >= humans).then(|| deck(s)),
            token: format!("token{s}"),
        })
        .collect();
    MultiMatch::new(
        "m",
        seats,
        settings,
        synthetic::board(),
        seed,
        synthetic::catalogue(),
        RulesTables::empty(),
        None,
        OnlinePolicy { reconnect_grace: 1_000, ..OnlinePolicy::default() },
    )
    .unwrap()
    .with_computer_delay(0)
}

fn take(clients: &mut [ClientState], out: Outbox) {
    for (seat, m) in out {
        clients[usize::from(seat)].apply(&m);
    }
}

#[test]
fn four_computers_and_no_player_play_a_match_to_its_end_and_the_record_replays() {
    for (mode, seed) in [(RoomMode::FreeForAll, 1), (RoomMode::FreeForAll, 2), (RoomMode::Teams, 3), (RoomMode::Teams, 4)] {
        let mut g = game(1, settings(mode), seed);
        let mut clients = vec![ClientState::new(), ClientState::new(), ClientState::new(), ClientState::new()];
        let (seat, out) = g.hello(&yugi_compat(), &Join::Seat { seat: Some(0) }, Some(&deck(0)), false, false, 0).unwrap();
        assert_eq!(seat, 0);
        take(&mut clients, out);
        assert!(g.started(), "the computer seats were ready");
        // The player concedes at once; the three computers play it out.
        let out = g.handle(0, clients[0].forfeit(), 1);
        take(&mut clients, out);
        let mut now = 1;
        while g.ended().is_none() && now < 200_000 {
            now += 10;
            let out = g.tick(now);
            take(&mut clients, out);
        }
        let (outcome, _) = g.ended().unwrap_or_else(|| panic!("{mode:?} seed {seed}: unfinished"));
        let duel = g.duel().unwrap();
        assert!(duel.is_out(0));
        eprintln!("{mode:?} seed {seed}: {outcome:?} after {} revisions, round {}", g.revision(), duel.round);
        if let Some(w) = outcome.winner {
            assert!(duel.alive(w) || outcome.reason == EndReason::TurnLimit);
        }
        // The server's record reproduces the match.
        let replayed = replay_multi(g.replay(), &synthetic::catalogue(), &RulesTables::empty()).unwrap();
        assert_eq!(&replayed, duel);
        // The player kept receiving the match after being knocked out.
        assert_eq!(clients[0].revision, g.revision());
        assert!(matches!(clients[0].status, Some(MatchStatus::Ended { .. })));
    }
}

#[test]
fn the_welcome_names_every_seat_and_the_room_settings() {
    let room = RoomSettings { mode: RoomMode::Teams, cards_leave: false, start_lp: 6000, shared_lp: true, turn_seconds: 120, spectators: false, map: None };
    let mut g = game(2, room.clone(), 7);
    let (_, out) = g.hello(&yugi_compat(), &Join::Seat { seat: Some(1) }, Some(&deck(1)), false, false, 0).unwrap();
    let welcome = out.iter().find_map(|(_, m)| match m {
        ServerMessage::Welcome { multi, policy, .. } => Some((multi.clone().unwrap(), policy.clone())),
        _ => None,
    });
    let (multi, policy) = welcome.unwrap();
    assert_eq!(multi.settings, room);
    assert_eq!(multi.seats.len(), 4);
    assert_eq!(multi.seats.iter().map(|s| s.team).collect::<Vec<_>>(), vec![0, 0, 1, 1]);
    assert!(multi.seats[2].computer && !multi.seats[0].computer);
    assert_eq!(policy.turn_limit, Some(120_000));
    assert!(!g.started(), "seat 0 has not joined");
    // A seat is taken once; the computer's seat cannot be taken.
    assert!(g.hello(&yugi_compat(), &Join::Seat { seat: Some(1) }, Some(&deck(1)), false, false, 0).is_err());
    assert!(g.hello(&yugi_compat(), &Join::Seat { seat: Some(3) }, Some(&deck(1)), false, false, 0).is_err());
    let (_, out) = g.hello(&yugi_compat(), &Join::Seat { seat: Some(0) }, Some(&deck(0)), false, false, 0).unwrap();
    assert!(g.started());
    // Both players get the start; the rules are the room's.
    let snaps = out.iter().filter(|(_, m)| matches!(m, ServerMessage::Snapshot { .. })).count();
    assert_eq!(snaps, 2);
    let d = g.duel().unwrap();
    let m = d.multi().unwrap();
    assert_eq!(m.rules.mode, MultiMode::Teams);
    assert!(!m.rules.cards_leave && m.rules.shared_lp);
    assert!(d.sides.iter().all(|s| s.lp == 6000));
    // The team-mate's hand is in the seat's view, an enemy's is not.
    let ServerMessage::Snapshot { snapshot } = &out.iter().find(|(s, m)| *s == 0 && matches!(m, ServerMessage::Snapshot { .. })).unwrap().1 else {
        unreachable!()
    };
    assert_eq!(snapshot.view.team_hands.len(), 1);
    assert_eq!(snapshot.view.team_hands[0].side, 1);
    assert_eq!(snapshot.view.team_hands[0].hand, d.sides[1].hand);
    assert!(snapshot.view.multi.is_some());
}

#[test]
fn a_disconnected_players_turns_are_skipped_and_the_grace_knocks_them_out() {
    let mut g = game(2, settings(RoomMode::FreeForAll), 5);
    let mut clients = vec![ClientState::new(), ClientState::new(), ClientState::new(), ClientState::new()];
    for s in 0..2u8 {
        let (_, out) = g.hello(&yugi_compat(), &Join::Seat { seat: Some(s) }, Some(&deck(usize::from(s))), false, false, 0).unwrap();
        take(&mut clients, out);
    }
    assert!(g.started());
    // Seat 1 leaves: the match goes on (never paused), and when its turn
    // comes the server ends it.
    let out = g.disconnect(1, 10);
    take(&mut clients, out);
    let mut now = 10;
    let mut skipped = false;
    while now < 900 {
        now += 10;
        let before = g.revision();
        // Seat 0 ends its turns at once.
        if g.duel().unwrap().active == 0 && clients[0].my_turn() {
            let m = clients[0].command(roses_core::duel::DuelCommand::EndTurn);
            let out = g.handle(0, m, now);
            take(&mut clients, out);
        }
        let out = g.tick(now);
        take(&mut clients, out);
        let entries = &g.replay().entries;
        skipped |= entries[before as usize..].iter().any(|e| e.cause == Cause::TurnTimeout);
        assert!(!matches!(g.status(), MatchStatus::Paused { .. }));
    }
    assert!(skipped, "the absent player's turn was ended by the server");
    assert!(!g.replay().entries.iter().any(|e| e.cause == Cause::Abandoned), "still within the grace");
    // Past the grace (1 s here) the absent player is out (unless the
    // others knocked them out first or the match ended).
    let alive = g.ended().is_none() && g.duel().unwrap().alive(1);
    let out = g.tick(1_100);
    take(&mut clients, out);
    if alive {
        assert!(g.duel().unwrap().is_out(1));
        assert!(g.replay().entries.iter().any(|e| e.cause == Cause::Abandoned));
    }
    // A player who resumes in time keeps the seat (fresh match).
    let mut h = game(2, settings(RoomMode::FreeForAll), 6);
    for s in 0..2u8 {
        h.hello(&yugi_compat(), &Join::Seat { seat: Some(s) }, Some(&deck(usize::from(s))), false, false, 0).unwrap();
    }
    h.disconnect(1, 10);
    let (seat, _) = h.hello(&yugi_compat(), &Join::Resume { token: "token1".into() }, None, false, false, 500).unwrap();
    assert_eq!(seat, 1);
    h.tick(5_000);
    assert!(!h.duel().unwrap().is_out(1));
}

#[test]
fn three_expired_turns_knock_a_player_out() {
    let room = RoomSettings { turn_seconds: 1, ..settings(RoomMode::FreeForAll) };
    let mut g = game(1, room, 9);
    g.hello(&yugi_compat(), &Join::Seat { seat: Some(0) }, Some(&deck(0)), false, false, 0).unwrap();
    let mut now = 0;
    while !g.duel().unwrap().is_out(0) && g.ended().is_none() && now < 2_000_000 {
        now += 100;
        g.tick(now);
    }
    let timeouts = g.replay().entries.iter().filter(|e| e.cause == Cause::TurnTimeout && e.actor == roses_net::Actor::Server).count();
    assert!(g.duel().unwrap().is_out(0) || g.ended().is_some());
    assert!(timeouts >= 2, "two turns ended by the clock first ({timeouts})");
    assert!(g.replay().entries.iter().any(|e| e.cause == Cause::RepeatedTimeouts));
}

#[test]
fn the_start_deadline_gives_missing_players_the_computer() {
    let mut g = game(3, settings(RoomMode::FreeForAll), 11);
    g.hello(&yugi_compat(), &Join::Seat { seat: Some(0) }, Some(&deck(0)), false, false, 0).unwrap();
    assert!(!g.started());
    let (filled, out) = g.fill_with_computers(|| ("Synthetic Pegasus".into(), deck(1))).unwrap();
    assert_eq!(filled, vec![1, 2]);
    assert!(g.started());
    assert!(out.iter().any(|(s, m)| *s == 0 && matches!(m, ServerMessage::Snapshot { .. })));
    let infos = g.seat_infos();
    assert!(infos[1].computer && infos[2].computer && infos[1].name == "Synthetic Pegasus");
    // A late player cannot take a seat the computer now plays.
    assert!(g.hello(&yugi_compat(), &Join::Seat { seat: Some(1) }, Some(&deck(1)), false, false, 0).is_err());
}

/// Four players in `mode`, all joined; returns the match.
fn all_players(mode: RoomMode, seed: u64) -> MultiMatch {
    let mut g = game(4, settings(mode), seed);
    for s in 0..4u8 {
        g.hello(&yugi_compat(), &Join::Seat { seat: Some(s) }, Some(&deck(usize::from(s))), false, false, 0).unwrap();
    }
    assert!(g.started());
    g
}

#[test]
fn everyone_cut_off_at_once_does_not_race_the_match_to_the_round_limit() {
    let mut g = all_players(RoomMode::FreeForAll, 21);
    // A network cut: all four drop within the same moment.
    for s in 0..4u8 {
        g.disconnect(s, 100);
    }
    let (round, revision) = (g.round(), g.revision());
    let mut now = 100;
    while now < 900 {
        now += 10;
        g.tick(now);
    }
    assert_eq!(g.ended(), None, "the match waits inside the grace");
    assert_eq!(g.round(), round, "no turn was skipped with nobody there");
    assert_eq!(g.revision(), revision);
    // One comes back in time: the match goes on, with a full turn clock.
    let (seat, _) = g.hello(&yugi_compat(), &Join::Resume { token: "token2".into() }, None, false, false, 950).unwrap();
    assert_eq!(seat, 2);
    g.tick(960);
    assert_eq!(g.duel().unwrap().active, 2, "the others are skipped up to the one who returned");
    assert!(g.duel().unwrap().alive(2));
}

#[test]
fn everyone_gone_past_the_grace_ends_the_match_for_the_last_to_leave() {
    let mut g = all_players(RoomMode::FreeForAll, 22);
    // Seat 3 leaves first and seat 1 last, whatever the seat numbers.
    for (s, at) in [(3u8, 100u64), (0, 110), (2, 120), (1, 130)] {
        g.disconnect(s, at);
    }
    g.tick(1_200);
    let (outcome, cause) = g.ended().expect("the grace is over for all");
    assert_eq!(cause, Cause::Abandoned);
    assert_eq!(outcome.winner, Some(1), "the last to leave outlasted the others");
    replay_multi(g.replay(), &synthetic::catalogue(), &RulesTables::empty()).unwrap();
}

#[test]
fn a_room_of_computers_stops_once_the_last_player_has_left() {
    for (mode, seed, won) in [(RoomMode::FreeForAll, 23, None), (RoomMode::Teams, 24, Some(2u8))] {
        let mut g = game(1, settings(mode), seed).with_computer_delay(350);
        g.hello(&yugi_compat(), &Join::Seat { seat: Some(0) }, Some(&deck(0)), false, false, 0).unwrap();
        g.disconnect(0, 10);
        // Inside the grace the player may come back: the computers play on.
        g.tick(500);
        assert_eq!(g.ended(), None);
        // Past it nobody is left to play for.
        g.tick(1_100);
        let (outcome, cause) = g.ended().unwrap_or_else(|| panic!("{mode:?}: computers played on"));
        assert_eq!(cause, Cause::Abandoned);
        let winner = outcome.winner.expect("one side is left");
        assert_ne!(winner, 0, "a player who left does not win");
        if let Some(w) = won {
            assert_eq!(winner / 2, w / 2, "2v2: the team without the player who left");
        }
        replay_multi(g.replay(), &synthetic::catalogue(), &RulesTables::empty()).unwrap();
    }
    // A player knocked out who stays connected keeps watching the rest.
    let mut g = game(1, settings(RoomMode::FreeForAll), 25).with_computer_delay(350);
    let mut clients = vec![ClientState::new(), ClientState::new(), ClientState::new(), ClientState::new()];
    let (_, out) = g.hello(&yugi_compat(), &Join::Seat { seat: Some(0) }, Some(&deck(0)), false, false, 0).unwrap();
    take(&mut clients, out);
    g.handle(0, clients[0].forfeit(), 1);
    g.tick(2_000);
    assert_eq!(g.ended(), None, "still on the screen: the match goes on");
    // Closing the game then ends it at once.
    g.disconnect(0, 2_100);
    assert_eq!(g.ended().map(|e| e.1), Some(Cause::Abandoned));
}

#[test]
fn a_refused_handshake_does_not_lose_the_revisions_it_ran() {
    // Seat 1 never came, so the computer took it; seat 0 plays on.
    let mut g = game(2, settings(RoomMode::FreeForAll), 26).with_computer_delay(100);
    let mut clients = vec![ClientState::new(), ClientState::new(), ClientState::new(), ClientState::new()];
    let (_, out) = g.hello(&yugi_compat(), &Join::Seat { seat: Some(0) }, Some(&deck(0)), false, false, 0).unwrap();
    take(&mut clients, out);
    let (_, out) = g.fill_with_computers(|| ("Synthetic Pegasus".into(), deck(1))).unwrap();
    take(&mut clients, out);
    // Hand the turn to the computers so that the clock moves the match.
    let mut now = 0;
    while g.duel().unwrap().active == 0 {
        now += 10;
        let m = clients[0].command(roses_core::duel::DuelCommand::EndTurn);
        let out = g.handle(0, m, now);
        take(&mut clients, out);
    }
    assert_eq!(clients[0].revision, g.revision());
    let mut refused = 0;
    for _ in 0..50 {
        now += 150;
        let before = g.revision();
        // The late player's stale resume is refused while the clock is due.
        match g.hello(&yugi_compat(), &Join::Resume { token: "token1".into() }, None, false, false, now) {
            Err(_) => refused += u32::from(g.revision() > before),
            Ok(_) => panic!("a computer's seat cannot be resumed"),
        }
        let out = g.tick(now);
        take(&mut clients, out);
        assert_eq!(clients[0].revision, g.revision(), "seat 0 got every revision");
        if g.duel().unwrap().active == 0 {
            break;
        }
    }
    assert!(refused > 0, "a revision was run by a refused handshake");
}

#[test]
fn nobody_watching_costs_no_spectator_frames_and_a_late_spectator_starts_fresh() {
    let mut g = game(1, settings(RoomMode::FreeForAll), 27).with_computer_delay(0);
    g.hello(&yugi_compat(), &Join::Seat { seat: Some(0) }, Some(&deck(0)), false, false, 0).unwrap();
    g.handle(0, ClientMessage::Command { seq: 1, base_revision: 0, command: roses_core::duel::DuelCommand::EndTurn }, 1);
    assert!(g.revision() > 0);
    assert!(g.take_spectator_messages().is_empty(), "no frame is built for nobody");
    // The first spectator's snapshot is the current state; frames follow.
    let shot = g.spectator_snapshot().unwrap();
    assert_eq!(shot.revision, g.revision());
    g.set_watchers(1);
    g.take_spectator_messages();
    let mut now = 10;
    while g.ended().is_none() && g.take_spectator_messages().is_empty() && now < 5_000 {
        now += 10;
        let m = ClientMessage::Command { seq: now, base_revision: g.revision(), command: roses_core::duel::DuelCommand::EndTurn };
        g.handle(0, m, now);
    }
    assert!(now < 5_000, "a spectator gets the next revision");
}

#[test]
fn signing_in_from_a_second_place_closes_the_first_places_rooms() {
    use crate::lobby::{Action, Lobby};
    use roses_net::lobby::LobbyEvent;
    let mut lobby = Lobby::new(false);
    lobby.enter("Yugi", 1, false);
    lobby.enter("Kaiba", 2, false);
    let actions = lobby.request("Yugi", &LobbyRequest::CreateModeRoom { settings: RoomSettings::default() }, 0);
    let code = actions
        .iter()
        .find_map(|a| match a {
            Action::Send(_, LobbyEvent::ModeRoom { room }) => Some(room.code.clone()),
            _ => None,
        })
        .unwrap();
    lobby.request("Kaiba", &LobbyRequest::JoinModeRoom { code: code.clone() }, 0);
    // Yugi's second game: the first connection is signed out, and his room
    // (where Kaiba sits) must not stay open with nobody hosting it.
    let actions = lobby.enter("Yugi", 3, false);
    assert!(actions.iter().any(|a| matches!(a, Action::Send(2, LobbyEvent::ModeRoomClosed))), "{actions:?}");
    let again = lobby.request("Joey", &LobbyRequest::JoinModeRoom { code: code.clone() }, 0);
    assert!(again.is_empty() || !again.iter().any(|a| matches!(a, Action::Send(_, LobbyEvent::ModeRoom { .. }))));
    // Kaiba may open his own room; Yugi's old 2-player room is gone as well.
    let actions = lobby.request("Kaiba", &LobbyRequest::CreateModeRoom { settings: RoomSettings::default() }, 0);
    assert!(actions.iter().any(|a| matches!(a, Action::Send(2, LobbyEvent::ModeRoom { .. }))), "{actions:?}");
    let actions = lobby.request("Yugi", &LobbyRequest::CreateRoom { map: None }, 0);
    let room = actions
        .iter()
        .find_map(|a| match a {
            Action::Send(3, LobbyEvent::RoomOpened { code }) => Some(code.clone()),
            _ => None,
        })
        .unwrap();
    lobby.enter("Yugi", 4, false);
    lobby.enter("Joey", 5, false);
    let actions = lobby.request("Joey", &LobbyRequest::JoinRoom { code: room }, 0);
    assert!(!actions.iter().any(|a| matches!(a, Action::PairPrivate(..))), "a room of the old connection cannot pair: {actions:?}");
}

// ---- the hub: rooms, the start, old clients ---------------------------------

fn hub_settings() -> HubSettings {
    HubSettings { multi_computer_delay_ms: 0, ..HubSettings::default() }
}

fn sign_up(hub: &Running, name: &str, invite: &str) -> roses_net::friends::LobbyClient {
    let mut c = hub.lobby();
    c.send(LobbyRequest::SignUp { username: name.into(), password: format!("{name} password"), invite: invite.into() });
    until(&mut c, "sign-up", |c| c.user.is_some());
    assert!(c.modes, "the server offers MODES");
    c
}

/// Rooms: create, join, move seat, computer seats, settings, start; both
/// players play their seats with the simple computer client to the end.
fn play_room(mode: RoomMode, cards_leave: bool, shared_lp: bool, kaiba_seat: u8) -> (Vec<ServerMessage>, Vec<ServerMessage>, MatchResult, MatchResult) {
    let (hub, invite) = Running::with_settings(false, 2, hub_settings(), None);
    let mut yugi = sign_up(&hub, "Yugi", &invite);
    let mut kaiba = sign_up(&hub, "Kaiba", &invite);
    // Settings the server cannot play are refused.
    let bad = RoomSettings { start_lp: 0, ..RoomSettings::default() };
    yugi.send(LobbyRequest::CreateModeRoom { settings: bad });
    until(&mut yugi, "bad settings", |c| c.note.as_deref() == Some(&LobbyProblem::BadSettings.to_string()));
    let room = RoomSettings { mode, cards_leave, shared_lp, turn_seconds: 0, start_lp: 3000, ..RoomSettings::default() };
    yugi.send(LobbyRequest::CreateModeRoom { settings: room.clone() });
    until(&mut yugi, "room", |c| c.mode_room.is_some());
    let code = yugi.mode_room.as_ref().unwrap().code.clone();
    kaiba.send(LobbyRequest::JoinModeRoom { code: code.to_lowercase() });
    until(&mut kaiba, "joined", |c| c.mode_room.as_ref().is_some_and(|r| r.seats[1].player.as_deref() == Some("Kaiba")));
    // Only the host changes settings or computer seats.
    kaiba.send(LobbyRequest::StartModeRoom);
    until(&mut kaiba, "not host", |c| c.note.as_deref() == Some(&LobbyProblem::NotHost.to_string()));
    if kaiba_seat != 1 {
        kaiba.send(LobbyRequest::ModeRoomSeat { seat: kaiba_seat });
        until(&mut kaiba, "moved", |c| {
            c.mode_room.as_ref().is_some_and(|r| r.seats[usize::from(kaiba_seat)].player.as_deref() == Some("Kaiba"))
        });
    }
    yugi.send(LobbyRequest::ModeRoomComputer { seat: 3, computer: true });
    until(&mut yugi, "computer seat", |c| c.mode_room.as_ref().is_some_and(|r| r.seats[3].computer));
    if mode == RoomMode::Teams {
        let r = yugi.mode_room.as_ref().unwrap();
        assert_eq!(r.seats.iter().map(|s| s.team).collect::<Vec<_>>(), vec![0, 0, 1, 1]);
    }
    yugi.send(LobbyRequest::StartModeRoom);
    until(&mut yugi, "ready", |c| c.multi_ready.is_some() && c.mode_room.is_none());
    until(&mut kaiba, "ready", |c| c.multi_ready.is_some() && c.mode_room.is_none());
    let (ry, rk) = (yugi.multi_ready.clone().unwrap(), kaiba.multi_ready.clone().unwrap());
    assert_eq!((ry.seat, rk.seat), (0, kaiba_seat));
    assert_eq!(ry.settings, room);
    assert_eq!(ry.seats.iter().filter(|s| s.computer).count(), 2, "the open seat and seat 3 got the computer");
    let collected: Vec<Arc<Mutex<Vec<ServerMessage>>>> = vec![Arc::default(), Arc::default()];
    let threads: Vec<_> = [(ry.ticket.clone(), 0usize), (rk.ticket.clone(), 1usize)]
        .into_iter()
        .map(|(ticket, i)| {
            let connect = hub.match_connector();
            let sink = collected[i].clone();
            std::thread::spawn(move || {
                let options = PlayOptions {
                    join: Join::Ticket { ticket },
                    deck: Some(deck(i)),
                    timeout: Duration::from_secs(240),
                    ..PlayOptions::default()
                };
                play_bot(connect, &yugi_compat(), &synthetic::catalogue(), options, |m| sink.lock().unwrap().push(m.clone()))
            })
        })
        .collect();
    for t in threads {
        let report = t.join().unwrap().unwrap();
        assert!(report.state.ended.is_some());
    }
    until(&mut yugi, "result", |c| c.over.is_some());
    until(&mut kaiba, "result", |c| c.over.is_some());
    let (y, k) = (yugi.over.clone().unwrap().1, kaiba.over.clone().unwrap().1);
    let hub = hub.stop();
    let log = hub.accounts().matches(10).unwrap();
    assert_eq!(log.len(), 1);
    let take = |i: usize| collected[i].lock().unwrap().clone();
    (take(0), take(1), y, k)
}

fn last_view(messages: &[ServerMessage]) -> roses_net::PlayerView {
    messages
        .iter()
        .rev()
        .find_map(|m| match m {
            ServerMessage::Update { view, .. } => Some(view.clone()),
            ServerMessage::Snapshot { snapshot } => Some(snapshot.view.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn a_free_for_all_room_plays_to_the_end_with_two_players_and_two_computers() {
    let (y, k, ry, rk) = play_room(RoomMode::FreeForAll, false, false, 2);
    let v = last_view(&y);
    let m = v.multi.clone().unwrap();
    assert_eq!(m.rules.mode, MultiMode::FreeForAll);
    assert!(!m.rules.cards_leave);
    assert_eq!(m.rules.start_lp, 3000);
    // Knocked-out players' cards stayed (every leader is still there).
    for s in 0..4u8 {
        assert!(v.board.units.iter().any(|u| u.side == s && u.slot == 0), "leader {s}");
    }
    assert!(m.out.iter().filter(|o| o.is_some()).count() >= 1);
    // Results: at most one of them won.
    assert!(!(ry == MatchResult::Won && rk == MatchResult::Won));
    let welcome = y.iter().find_map(|m| match m {
        ServerMessage::Welcome { multi, .. } => multi.clone(),
        _ => None,
    });
    assert_eq!(welcome.unwrap().seats.len(), 4);
    let _ = k;
}

#[test]
fn a_2v2_room_with_shared_lp_plays_to_the_end_and_team_mates_win_together() {
    // Kaiba moves to seat 1: Yugi and Kaiba are team-mates (S + N).
    let (y, k, ry, rk) = play_room(RoomMode::Teams, true, true, 1);
    assert_eq!(ry, rk, "team-mates share the result");
    let v = last_view(&y);
    let m = v.multi.clone().unwrap();
    assert!(m.rules.shared_lp && m.rules.cards_leave);
    // Shared LP: in every view the living members of a team have one LP.
    for msg in y.iter().chain(&k) {
        let ServerMessage::Update { view, .. } = msg else { continue };
        let m = view.multi.as_ref().unwrap();
        for team in [0u8, 1] {
            let lps: Vec<u16> = (0..4)
                .filter(|&s| m.teams[s] == team && m.out[s].is_none())
                .map(|s| view.board.lp[s])
                .collect();
            assert!(lps.windows(2).all(|w| w[0] == w[1]), "team {team}: {lps:?}");
        }
        // Each sees the team-mate's hand, never an enemy's.
        if m.out[usize::from(view.seat)].is_none() {
            assert!(view.team_hands.iter().all(|t| m.teams[usize::from(t.side)] == m.teams[usize::from(view.seat)]));
        }
    }
    // Cards leave: the knocked-out sides have no units.
    for (s, out) in m.out.iter().enumerate() {
        if out.is_some() {
            assert!(v.board.units.iter().all(|u| usize::from(u.side) != s));
        }
    }
}

/// A running N-player room of Yugi and three computers that hardly move,
/// and the match's id and Yugi's match connection.
fn yugi_with_computers(
    hub: &Running,
    invite: &str,
) -> (roses_net::friends::LobbyClient, String, impl roses_net::transport::Connection<ClientMessage, ServerMessage>) {
    use roses_net::transport::{Connection, Received};
    let mut yugi = sign_up(hub, "Yugi", invite);
    yugi.send(LobbyRequest::CreateModeRoom { settings: RoomSettings::default() });
    until(&mut yugi, "room", |c| c.mode_room.is_some());
    yugi.send(LobbyRequest::StartModeRoom);
    until(&mut yugi, "ready", |c| c.multi_ready.is_some());
    let ready = yugi.multi_ready.clone().unwrap();
    let mut conn = hub.match_connector()().unwrap();
    conn.send(&ClientMessage::hello(yugi_compat(), Join::Ticket { ticket: ready.ticket }, Some(deck(0)), false)).unwrap();
    let start = std::time::Instant::now();
    loop {
        if let Received::Message(ServerMessage::Snapshot { .. }) = conn.poll() {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "the match did not start");
        std::thread::sleep(Duration::from_millis(2));
    }
    (yugi, ready.match_id, conn)
}

#[test]
fn turning_spectators_off_sends_away_the_watchers_of_an_n_player_match() {
    use roses_net::transport::{Connection, Received};
    let settings = HubSettings { multi_computer_delay_ms: 600_000, ..HubSettings::default() };
    let (hub, invite) = Running::with_settings(false, 2, settings, None);
    let (mut yugi, match_id, _seat) = yugi_with_computers(&hub, &invite);
    let mut kaiba = sign_up(&hub, "Kaiba", &invite);
    kaiba.send(LobbyRequest::Watch { match_id });
    until(&mut kaiba, "watch ticket", |c| c.watch.is_some());
    let ticket = kaiba.watch.clone().unwrap().ticket;
    let mut watcher = hub.match_connector()().unwrap();
    watcher.send(&ClientMessage::hello(yugi_compat(), Join::Watch { ticket }, None, false)).unwrap();
    let wait_for = |watcher: &mut dyn Connection<ClientMessage, ServerMessage>, what: &str, done: &dyn Fn(&ServerMessage) -> bool| {
        let start = std::time::Instant::now();
        loop {
            if let Received::Message(m) = watcher.poll() {
                if done(&m) {
                    return;
                }
            }
            assert!(start.elapsed() < Duration::from_secs(20), "{what}");
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    wait_for(&mut watcher, "the spectator's snapshot", &|m| matches!(m, ServerMessage::Snapshot { .. }));
    yugi.send(LobbyRequest::AllowSpectators { allow: false });
    wait_for(&mut watcher, "sent away", &|m| matches!(m, ServerMessage::Refused { reason: roses_net::Refusal::NotWatchable }));
}

#[test]
fn the_match_limit_counts_n_player_matches_for_a_2_player_match_too() {
    let settings = HubSettings { multi_computer_delay_ms: 600_000, max_matches: 1, ..HubSettings::default() };
    let (hub, invite) = Running::with_settings(false, 3, settings, None);
    let (_yugi, _, _seat) = yugi_with_computers(&hub, &invite);
    let mut kaiba = sign_up(&hub, "Kaiba", &invite);
    let mut joey = sign_up(&hub, "Joey", &invite);
    kaiba.send(LobbyRequest::Challenge { to: "Joey".into(), map: None });
    until(&mut joey, "challenge", |c| !c.incoming.is_empty());
    let id = joey.incoming[0].0;
    joey.send(LobbyRequest::Answer { id, accept: true });
    until(&mut joey, "busy", |c| c.note.as_deref() == Some(&LobbyProblem::ServerBusy.to_string()));
    assert!(joey.ready.is_none());
    assert_eq!(hub.stop().match_count(), 1);
}

#[test]
fn log_ins_no_account_can_match_and_probes_for_taken_names_are_limited() {
    use crate::accounts::PasswordCost;
    use crate::hub::ATTEMPTS_PER_ADDRESS;
    use roses_net::lobby::AuthProblem;
    let settings = HubSettings { open_signup: true, password_cost: PasswordCost::TEST, ..HubSettings::default() };
    let peer = Some("203.0.113.9".parse().unwrap());
    let (hub, _) = Running::with_settings(false, 1, settings, peer);
    let mut owner = hub.lobby();
    owner.send(LobbyRequest::SignUp { username: "Yugi".into(), password: "heart of cards".into(), invite: String::new() });
    until(&mut owner, "sign-up", |c| c.user.is_some());
    // Shapes no account has: refused as a wrong password, and counted.
    let mut c = hub.lobby();
    let answer = |c: &mut roses_net::friends::LobbyClient, request: LobbyRequest| {
        c.send(request);
        until(c, "an answer", |c| c.auth_problem.is_some());
        c.auth_problem.unwrap()
    };
    let long_name = LobbyRequest::LogIn { username: "x".repeat(4096), password: "heart of cards".into() };
    assert_eq!(answer(&mut c, long_name), AuthProblem::BadCredentials);
    let long_password = LobbyRequest::LogIn { username: "Yugi".into(), password: "y".repeat(65) };
    assert_eq!(answer(&mut c, long_password), AuthProblem::BadCredentials);
    // The real account still signs in with its real password.
    let good = LobbyRequest::LogIn { username: "Yugi".into(), password: "heart of cards".into() };
    c.send(good);
    until(&mut c, "signed in", |c| c.user.is_some());
    // Asking for taken names runs out like guessing does.
    let mut probe = hub.lobby();
    let taken = || LobbyRequest::SignUp { username: "Yugi".into(), password: "any password".into(), invite: String::new() };
    for _ in 2..ATTEMPTS_PER_ADDRESS {
        assert_eq!(answer(&mut probe, taken()), AuthProblem::UsernameTaken);
    }
    assert_eq!(answer(&mut probe, taken()), AuthProblem::TooManyAttempts);
}

#[test]
fn an_old_client_sees_no_rooms_and_no_n_player_match() {
    let (hub, invite) = Running::with_settings(false, 3, hub_settings(), None);
    let mut yugi = sign_up(&hub, "Yugi", &invite);
    let mut kaiba = sign_up(&hub, "Kaiba", &invite);
    // An old client: its hello has no `modes`.
    let mut old = hub.old_lobby();
    old.send(LobbyRequest::SignUp { username: "Joey".into(), password: "red eyes black".into(), invite });
    until(&mut old, "sign-up", |c| c.user.is_some());
    // A running N-player match with spectators allowed...
    yugi.send(LobbyRequest::CreateModeRoom { settings: RoomSettings { turn_seconds: ROOM_TURN_SECONDS, ..RoomSettings::default() } });
    until(&mut yugi, "room", |c| c.mode_room.is_some());
    let code = yugi.mode_room.as_ref().unwrap().code.clone();
    kaiba.send(LobbyRequest::JoinModeRoom { code });
    until(&mut kaiba, "joined", |c| c.mode_room.is_some());
    yugi.send(LobbyRequest::StartModeRoom);
    until(&mut yugi, "ready", |c| c.multi_ready.is_some());
    let ry = yugi.multi_ready.clone().unwrap();
    // Yugi joins (so the match can start once Kaiba does).
    let mut conn = hub.match_connector()().unwrap();
    use roses_net::transport::{Connection, Received};
    conn.send(&ClientMessage::hello(yugi_compat(), Join::Ticket { ticket: ry.ticket.clone() }, Some(deck(0)), false)).unwrap();
    let rk = { until(&mut kaiba, "ready", |c| c.multi_ready.is_some()); kaiba.multi_ready.clone().unwrap() };
    let mut conn2 = hub.match_connector()().unwrap();
    conn2.send(&ClientMessage::hello(yugi_compat(), Join::Ticket { ticket: rk.ticket.clone() }, Some(deck(1)), false)).unwrap();
    // ...is listed for a client that plays MODES and not for the old one.
    let start = std::time::Instant::now();
    loop {
        yugi.pump();
        let _ = (conn.poll(), conn2.poll());
        kaiba.send(LobbyRequest::ListMatches);
        old.send(LobbyRequest::ListMatches);
        std::thread::sleep(Duration::from_millis(20));
        kaiba.pump();
        old.pump();
        if kaiba.live.as_ref().is_some_and(|l| l.iter().any(|m| m.seats.len() == 4)) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "the N-player match was not listed");
    }
    // Kaiba plays in it, so may not watch it; the list is what counts.
    assert!(old.live.as_ref().is_some_and(|l| l.iter().all(|m| m.seats.is_empty() && m.mode.is_none())));
    assert!(old.mode_room.is_none() && old.multi_ready.is_none());
    while let Received::Message(_) = conn.poll() {}
    drop((conn, conn2));
    hub.stop();
}

// ---- leaving, waiting and missing seats (issues 7, 9, 10) -------------------

#[test]
fn a_room_with_people_waiting_stays_open_and_a_host_left_alone_gets_the_full_time() {
    use crate::lobby::{Action, Lobby, ROOM_MS};
    use roses_net::lobby::LobbyEvent;
    let closed = |a: &[Action]| a.iter().any(|a| matches!(a, Action::Send(_, LobbyEvent::ModeRoomClosed)));
    let mut lobby = Lobby::new(false);
    lobby.enter("Yugi", 1, false);
    lobby.enter("Kaiba", 2, false);
    let actions = lobby.request("Yugi", &LobbyRequest::CreateModeRoom { settings: RoomSettings::default() }, 0);
    let code = actions
        .iter()
        .find_map(|a| match a {
            Action::Send(_, LobbyEvent::ModeRoom { room }) => Some(room.code.clone()),
            _ => None,
        })
        .unwrap();
    lobby.request("Kaiba", &LobbyRequest::JoinModeRoom { code }, 0);
    for now in [ROOM_MS - 1, ROOM_MS, 2 * ROOM_MS] {
        assert!(!closed(&lobby.tick(now)), "two people wait at {now}");
    }
    // Alone again: a full time from then, and a host still changing the
    // room is still there.
    lobby.request("Kaiba", &LobbyRequest::LeaveRoom, 2 * ROOM_MS);
    assert!(!closed(&lobby.tick(3 * ROOM_MS - 1)));
    lobby.request("Yugi", &LobbyRequest::ModeRoomComputer { seat: 3, computer: true }, 3 * ROOM_MS - 1);
    assert!(!closed(&lobby.tick(3 * ROOM_MS)));
    // Nobody came and the host did nothing: the room closes as before.
    assert!(closed(&lobby.tick(4 * ROOM_MS)));
}

type MatchConn = roses_net::transport::Memory<ClientMessage, ServerMessage>;

/// Yugi's room with Kaiba and two computer seats, started. `settings` keeps
/// the computers slow, so the match stands still for the test.
fn started_room(settings: HubSettings) -> (Running, roses_net::friends::LobbyClient, roses_net::friends::LobbyClient) {
    let (hub, invite) = Running::with_settings(false, 4, settings, None);
    let mut yugi = sign_up(&hub, "Yugi", &invite);
    let mut kaiba = sign_up(&hub, "Kaiba", &invite);
    yugi.send(LobbyRequest::CreateModeRoom { settings: RoomSettings { turn_seconds: 0, ..RoomSettings::default() } });
    until(&mut yugi, "room", |c| c.mode_room.is_some());
    let code = yugi.mode_room.as_ref().unwrap().code.clone();
    kaiba.send(LobbyRequest::JoinModeRoom { code });
    until(&mut yugi, "joined", |c| c.mode_room.as_ref().is_some_and(|r| r.seats[1].player.is_some()));
    yugi.send(LobbyRequest::StartModeRoom);
    until(&mut yugi, "ready", |c| c.multi_ready.is_some());
    until(&mut kaiba, "ready", |c| c.multi_ready.is_some());
    (hub, yugi, kaiba)
}

fn take_seat(hub: &Running, ticket: &str, seat: usize) -> MatchConn {
    use roses_net::transport::Connection;
    let mut conn = hub.match_connector()().unwrap();
    conn.send(&ClientMessage::hello(yugi_compat(), Join::Ticket { ticket: ticket.into() }, Some(deck(seat)), false)).unwrap();
    conn
}

fn wait_for(conn: &mut MatchConn, what: &str, mut done: impl FnMut(&ServerMessage) -> bool) {
    use roses_net::transport::{recv_timeout, Received};
    loop {
        match recv_timeout(conn, Duration::from_secs(10)) {
            Received::Message(m) if done(&m) => return,
            Received::Message(_) => {}
            _ => panic!("no {what}"),
        }
    }
}

#[test]
fn a_knocked_out_player_who_leaves_is_free_and_a_later_end_leaves_their_next_match_alone() {
    use crate::friends_tests::sees;
    use roses_net::lobby::PlayerState;
    use roses_net::transport::Connection;
    let (hub, mut yugi, mut kaiba) = started_room(HubSettings { multi_computer_delay_ms: 60_000, ..HubSettings::default() });
    let mut seat0 = take_seat(&hub, &yugi.multi_ready.clone().unwrap().ticket, 0);
    let mut seat1 = take_seat(&hub, &kaiba.multi_ready.clone().unwrap().ticket, 1);
    wait_for(&mut seat0, "start", |m| matches!(m, ServerMessage::Snapshot { .. }));
    wait_for(&mut seat1, "start", |m| matches!(m, ServerMessage::Snapshot { .. }));
    // Kaiba concedes and closes the duel; Yugi and the computers play on.
    seat1.send(&ClientMessage::Forfeit { seq: 1 }).unwrap();
    wait_for(&mut seat1, "the knock-out", |m| matches!(m, ServerMessage::Accepted { .. }));
    seat1.close();
    until(&mut yugi, "Kaiba free", |c| sees(c, "Kaiba", PlayerState::Idle));
    until(&mut kaiba, "Kaiba's result", |c| c.over.is_some());
    assert_eq!(kaiba.over.clone().unwrap().1, MatchResult::Lost);
    // Signing in again does not pull him back, and he may play something else.
    let mut again = hub.lobby();
    again.send(LobbyRequest::LogIn { username: "Kaiba".into(), password: "Kaiba password".into() });
    until(&mut again, "signed in", |c| c.user.is_some());
    again.send(LobbyRequest::CreateModeRoom { settings: RoomSettings { turn_seconds: 0, ..RoomSettings::default() } });
    until(&mut again, "his own room", |c| c.mode_room.is_some());
    assert!(again.multi_ready.is_none(), "pulled back into the old match");
    for seat in 1..4 {
        again.send(LobbyRequest::ModeRoomComputer { seat, computer: true });
    }
    until(&mut again, "computers", |c| c.mode_room.as_ref().is_some_and(|r| r.seats[3].computer));
    again.send(LobbyRequest::StartModeRoom);
    until(&mut again, "his next match", |c| c.multi_ready.is_some());
    // The old match's end frees Yugi and must not touch Kaiba's new match.
    until(&mut yugi, "Kaiba busy", |c| sees(c, "Kaiba", PlayerState::InMatch));
    seat0.send(&ClientMessage::Forfeit { seq: 1 }).unwrap();
    wait_for(&mut seat0, "the knock-out", |m| matches!(m, ServerMessage::Accepted { .. }));
    seat0.close();
    until(&mut yugi, "Yugi free", |c| c.over.is_some() && sees(c, "Yugi", PlayerState::Idle));
    assert!(sees(&yugi, "Kaiba", PlayerState::InMatch), "{:?}", yugi.players);
    drop(hub.stop());
}

#[test]
fn a_player_whose_seat_went_to_the_computer_is_told_and_freed() {
    use roses_net::lobby::PlayerState;
    let (hub, mut yugi, mut kaiba) =
        started_room(HubSettings { multi_computer_delay_ms: 60_000, start_ms: 300, ..HubSettings::default() });
    // Kaiba's game could not join (match_failed): only Yugi comes.
    let mut seat0 = take_seat(&hub, &yugi.multi_ready.clone().unwrap().ticket, 0);
    wait_for(&mut seat0, "start", |m| matches!(m, ServerMessage::Snapshot { .. }));
    until(&mut kaiba, "told", |c| c.over.is_some());
    assert_eq!(kaiba.over.clone().unwrap().1, MatchResult::Cancelled);
    until(&mut yugi, "Kaiba free", |c| crate::friends_tests::sees(c, "Kaiba", PlayerState::Idle));
    drop(hub.stop());
}

// ---- issue 8: seats of an N-player match learn who is away ------------------

/// Seats 0..3 are players, seat 3 a computer; `away_seat` drops at once,
/// `slow_seat` sits out its clock, the others end their turns at once.
/// Returns what seat 0's [`AwaySeats`] said after every update.
fn play_watching_away(turn_seconds: u16, away_seat: Option<u8>, slow_seat: Option<u8>) -> Vec<bool> {
    use roses_core::duel::DuelCommand;
    use roses_net::client::AwaySeats;
    let room = RoomSettings { turn_seconds, ..settings(RoomMode::FreeForAll) };
    let mut g = game(3, room, 40);
    let mut clients = vec![ClientState::new(), ClientState::new(), ClientState::new(), ClientState::new()];
    for s in 0..3u8 {
        let (_, out) = g.hello(&yugi_compat(), &Join::Seat { seat: Some(s) }, Some(&deck(usize::from(s))), false, false, 0).unwrap();
        take(&mut clients, out);
    }
    let mut away = AwaySeats::default();
    let mut said = Vec::new();
    let mut feed = |clients: &mut Vec<ClientState>, out: Outbox, now: u64| {
        for (seat, m) in out {
            let i = usize::from(seat);
            let was = clients[i].view.as_ref().map(|v| v.board.active);
            clients[i].apply(&m);
            if seat == 0 {
                if let ServerMessage::Update { actor, cause, view, .. } = &m {
                    away.update(Some(0), was, *actor, *cause, view, clients[i].turn_limit, now);
                    said.push(away.any());
                }
            }
        }
    };
    let mut now = 1;
    if let Some(s) = away_seat {
        let out = g.disconnect(s, now);
        feed(&mut clients, out, now);
    }
    while now < 200_000 && g.ended().is_none() {
        now += 100;
        let out = g.tick(now);
        feed(&mut clients, out, now);
        let active = g.duel().unwrap().active;
        let held = Some(active) == away_seat || Some(active) == slow_seat || active == 3;
        if !held && clients[usize::from(active)].my_turn() {
            let m = clients[usize::from(active)].command(DuelCommand::EndTurn);
            let out = g.handle(active, m, now);
            feed(&mut clients, out, now);
        }
    }
    said
}

#[test]
fn a_seat_is_told_when_another_seats_turns_are_skipped_and_a_slow_seat_is_not_called_away() {
    // No clock: every server-ended turn is a skip.
    let said = play_watching_away(0, Some(1), None);
    assert!(said.iter().any(|&s| s), "seat 1 is gone and nobody said so");
    // A clock, and the same drop: the skip comes long before the limit.
    let said = play_watching_away(30, Some(1), None);
    assert!(said.iter().any(|&s| s), "seat 1 is gone and nobody said so");
    // A clock and nobody gone: seat 2 sits out its clock, which is not away.
    let said = play_watching_away(30, None, Some(2));
    assert!(said.len() > 3, "the turns went round");
    assert!(!said.iter().any(|&s| s), "seat 2 is only slow");
}

#[test]
fn the_away_note_ends_when_the_seat_acts_or_is_knocked_out() {
    use roses_net::client::AwaySeats;
    use roses_net::Actor;
    let mut g = game(2, settings(RoomMode::FreeForAll), 41);
    let mut c = ClientState::new();
    for s in 0..2u8 {
        let (_, out) = g.hello(&yugi_compat(), &Join::Seat { seat: Some(s) }, Some(&deck(usize::from(s))), false, false, 0).unwrap();
        for (seat, m) in out {
            if seat == 0 {
                c.apply(&m);
            }
        }
    }
    let mut view = c.view.clone().unwrap();
    let mut away = AwaySeats::default();
    away.update(Some(0), Some(1), Actor::Server, Cause::TurnTimeout, &view, None, 10);
    assert!(away.any());
    // Its own timeout never calls this seat away.
    let mut mine = AwaySeats::default();
    mine.update(Some(0), Some(0), Actor::Server, Cause::TurnTimeout, &view, None, 10);
    assert!(!mine.any());
    // It came back.
    away.update(Some(0), Some(1), Actor::Seat { seat: 1 }, Cause::Command, &view, None, 20);
    assert!(!away.any());
    // Knocked out: nothing to wait for.
    away.update(Some(0), Some(1), Actor::Server, Cause::TurnTimeout, &view, None, 30);
    assert!(away.any());
    let knocked_out = g.duel().unwrap().forfeit_with(1, &synthetic::catalogue()).unwrap().0;
    view.multi = knocked_out.multi().cloned();
    away.update(Some(0), Some(2), Actor::Server, Cause::Abandoned, &view, None, 40);
    assert!(!away.any());
    // A 2-seat match pauses instead and is left alone.
    view.multi = None;
    away.update(Some(0), Some(1), Actor::Server, Cause::TurnTimeout, &view, None, 50);
    assert!(!away.any());
}
