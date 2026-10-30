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
