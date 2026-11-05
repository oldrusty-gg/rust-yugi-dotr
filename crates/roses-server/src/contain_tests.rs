//! Panic containment in the hub: one match or connection that panics ends
//! itself, nothing else; and random commands from a signed-in player and a
//! seat (the structural fuzz never gets past the hello).

use crate::friends_tests::{sees, until, yugi_compat, Running};
use crate::hub::{HubSettings, TEST_PANICS};
use crate::play::{play_bot, PlayOptions};
use roses_core::duel::{DuelCommand, Pick};
use roses_core::{GridPos, UnitId};
use roses_net::friends::LobbyClient;
use roses_net::lobby::{LobbyRequest, MatchResult, PlayerState, RoomSettings};
use roses_net::transport::{Connection, Received};
use roses_net::{synthetic, ClientMessage, Join, ServerMessage};
use std::time::{Duration, Instant};

fn sign_up(hub: &Running, name: &str, invite: &str) -> LobbyClient {
    let mut c = hub.lobby();
    c.send(LobbyRequest::SignUp { username: name.into(), password: format!("{name} password"), invite: invite.into() });
    until(&mut c, "sign-up", |c| c.user.is_some());
    c
}

fn panic_for(name: &str) {
    TEST_PANICS.lock().unwrap().push(name.into());
}

fn no_panic_for(name: &str) {
    TEST_PANICS.lock().unwrap().retain(|n| n != name);
}

fn computer_match(c: &mut LobbyClient) {
    c.send(LobbyRequest::PlayComputer { map: None, computer: None });
    until(c, "match ready", |c| c.ready.is_some());
}

#[test]
fn a_match_that_panics_ends_alone_and_its_player_can_play_on() {
    let (hub, invite) = Running::start(true, 2);
    let mut broken = sign_up(&hub, "Bandit", &invite);
    let mut steady = sign_up(&hub, "Steady", &invite);
    computer_match(&mut broken);
    computer_match(&mut steady);
    panic_for("Bandit");
    until(&mut broken, "the match cancelled", |c| {
        matches!(&c.over, Some((_, MatchResult::Cancelled, None))) && sees(c, "Bandit", PlayerState::Idle)
    });
    no_panic_for("Bandit");
    // The other match never noticed: it plays to its end.
    let ready = steady.ready.clone().unwrap();
    let report = play_bot(
        hub.match_connector(),
        &yugi_compat(),
        &synthetic::catalogue(),
        PlayOptions {
            join: Join::Ticket { ticket: ready.ticket },
            deck: Some(synthetic::submissions()[usize::from(ready.seat)].clone()),
            ..PlayOptions::default()
        },
        |_| {},
    )
    .unwrap();
    assert!(report.state.ended.is_some());
    until(&mut steady, "match over", |c| c.over.is_some());
    // And the first player gets a new match.
    broken.ready = None;
    computer_match(&mut broken);
    assert_eq!(hub.stop().contained(), 1);
}

#[test]
fn a_connection_that_panics_loses_only_itself() {
    let (hub, invite) = Running::start(false, 2);
    let mut broken = sign_up(&hub, "Crasher", &invite);
    let mut steady = sign_up(&hub, "Calm", &invite);
    until(&mut steady, "both online", |c| sees(c, "Crasher", PlayerState::Idle));
    panic_for("Crasher");
    broken.send(LobbyRequest::Ping);
    until(&mut steady, "the other gone", |c| !c.players.iter().any(|p| p.name == "Crasher"));
    no_panic_for("Crasher");
    steady.send(LobbyRequest::CreateRoom { map: None });
    until(&mut steady, "the lobby still answers", |c| c.room.is_some());
    assert_eq!(hub.stop().contained(), 1);
}

#[test]
fn an_n_player_match_that_panics_ends_alone() {
    let settings = HubSettings { multi_computer_delay_ms: 0, ..HubSettings::default() };
    let (hub, invite) = Running::with_settings(true, 2, settings, None);
    let mut broken = sign_up(&hub, "Rebel", &invite);
    let mut steady = sign_up(&hub, "Quiet", &invite);
    for c in [&mut broken, &mut steady] {
        c.send(LobbyRequest::CreateModeRoom { settings: RoomSettings::default() });
        until(c, "room", |c| c.mode_room.is_some());
        c.send(LobbyRequest::StartModeRoom);
        until(c, "ready", |c| c.multi_ready.is_some());
    }
    panic_for("Rebel");
    until(&mut broken, "the match cancelled", |c| matches!(&c.over, Some((_, MatchResult::Cancelled, None))));
    no_panic_for("Rebel");
    assert!(steady.over.is_none());
    let hub = hub.stop();
    assert_eq!((hub.contained(), hub.multi_count()), (1, 1));
}

/// A small seeded generator (xorshift), so a failure repeats.
struct Rng(u64);

impl Rng {
    fn next(&mut self, below: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % below
    }

    fn cell(&mut self) -> GridPos {
        // Off the board as well as on it.
        GridPos { column: self.next(12) as u8, row: self.next(12) as u8 }
    }

    fn command(&mut self) -> DuelCommand {
        match self.next(9) {
            0 => DuelCommand::OpenSummon { cell: self.cell() },
            1 => DuelCommand::ChooseSummon { hand: (0..self.next(7)).map(|_| self.next(9) as usize).collect() },
            2 => DuelCommand::CancelSummon,
            3 => DuelCommand::Move {
                unit: UnitId(self.next(80) as u16),
                path: (0..self.next(6)).map(|_| self.cell()).collect(),
                face_up: self.next(2) == 0,
                defence: [None, Some(true), Some(false)][self.next(3) as usize],
            },
            4 => DuelCommand::Pick { pick: Pick::Unit { unit: UnitId(self.next(80) as u16) } },
            5 => DuelCommand::Pick { pick: Pick::Graveyard { side: self.next(4) as u8, index: self.next(50) as usize } },
            6 => DuelCommand::Pick { pick: Pick::Cancel },
            7 => DuelCommand::Target { side: self.next(4) as u8 },
            _ => DuelCommand::EndTurn,
        }
    }

    fn request(&mut self, room: &str) -> LobbyRequest {
        let code = if self.next(2) == 0 { room.to_string() } else { "ZZZZ".to_string() };
        match self.next(12) {
            0 => LobbyRequest::Challenge { to: ["Fuzz", "Other", "nobody", ""][self.next(4) as usize].into(), map: None },
            1 => LobbyRequest::CancelChallenge { id: self.next(5) },
            2 => LobbyRequest::Answer { id: self.next(5), accept: self.next(2) == 0 },
            3 => LobbyRequest::CreateRoom { map: None },
            4 => LobbyRequest::JoinRoom { code },
            5 => LobbyRequest::LeaveRoom,
            6 => LobbyRequest::CreateModeRoom { settings: RoomSettings::default() },
            7 => LobbyRequest::JoinModeRoom { code },
            8 => LobbyRequest::ModeRoomSeat { seat: self.next(9) as u8 },
            9 => LobbyRequest::ModeRoomComputer { seat: self.next(9) as u8, computer: self.next(2) == 0 },
            10 => LobbyRequest::Watch { match_id: code },
            _ => LobbyRequest::ListMatches,
        }
    }
}

/// Random well-formed requests from signed-in players, then random commands
/// from a seat in a running match, at the current revision so they reach the
/// rules. Nothing may panic: containment would hide it, so the count says.
#[test]
fn random_commands_from_signed_in_players_and_seats_panic_nothing() {
    let (hub, invite) = Running::start(true, 3);
    let mut fuzz = sign_up(&hub, "Fuzz", &invite);
    let mut other = sign_up(&hub, "Other", &invite);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for i in 0..3000 {
        let room = other.room.clone().unwrap_or_default();
        let (from, room) = if i % 2 == 0 { (&mut fuzz, room) } else { (&mut other, room) };
        from.send(rng.request(&room));
        from.pump();
        if i % 200 == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    // Settle: whatever those made, out of it and into a computer match.
    for c in [&mut fuzz, &mut other] {
        c.send(LobbyRequest::LeaveRoom);
    }
    let mut fresh = sign_up(&hub, "Late", &invite);
    computer_match(&mut fresh);
    let ready = fresh.ready.clone().unwrap();
    let mut conn = hub.match_connector()().unwrap();
    let deck = synthetic::submissions()[usize::from(ready.seat)].clone();
    conn.send(&ClientMessage::hello(yugi_compat(), Join::Ticket { ticket: ready.ticket }, Some(deck), false)).unwrap();
    let (mut seq, mut revision, mut sent, mut answered, mut ended) = (0, 0, 0, 0, false);
    let start = Instant::now();
    while sent < 4000 && !ended && start.elapsed() < Duration::from_secs(30) {
        loop {
            match conn.poll() {
                Received::Message(ServerMessage::Snapshot { snapshot }) => revision = snapshot.revision,
                Received::Message(ServerMessage::Update { revision: r, .. }) => revision = r,
                Received::Message(ServerMessage::Rejected { .. }) => answered += 1,
                Received::Message(_) => {}
                Received::Empty => break,
                Received::Malformed(m) => panic!("{m}"),
                Received::Closed => {
                    ended = true;
                    break;
                }
            }
        }
        seq += 1;
        sent += 1;
        let base_revision = if rng.next(8) == 0 { rng.next(1000) } else { revision };
        if conn.send(&ClientMessage::Command { seq, base_revision, command: rng.command() }).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_micros(200));
    }
    assert!(sent > 100 && answered > 0, "the match took only {sent} commands, {answered} reached the rules");
    eprintln!("{sent} commands, {answered} refused by the rules");
    drop(conn);
    assert_eq!(hub.stop().contained(), 0);
}
