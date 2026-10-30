//! MODES (a port addition): headless Bevy clients (`DuelPlay` online, the
//! pad-level computer player on their pads) play N-player matches against
//! the authoritative `roses_server::multi::MultiMatch` served in-process
//! over memory connections, with the other seats the server's computers.

use crate::duel_online::{ClientConn, Connector, OnlineLink, PadBot};
use crate::duel_play::DuelPlay;
use roses_core::duel::Outcome;
use roses_net::lobby::{RoomMode, RoomSettings};
use roses_net::transport::{memory_pair, Received};
use roses_net::{ClientMessage, Join, RulesTables, SeatInfo, ServerMessage};
use roses_server::multi::{MultiMatch, MultiSeatPlan};
use roses_server::serve::ServerConn;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct Run {
    seat: u8,
    outcome: Option<Outcome>,
    halted: Option<String>,
    presses: u64,
    turns: u32,
    results: u32,
}

/// Serve `game` until `stop`: connections say hello with their ticket's
/// seat (`Join::Seat`), then their messages are handled; the clock is real
/// milliseconds.
fn serve(game: &mut MultiMatch, rx: mpsc::Receiver<ServerConn>, stop: &AtomicBool) {
    let start = Instant::now();
    let mut waiting: Vec<ServerConn> = Vec::new();
    let mut seats: Vec<Option<ServerConn>> = (0..game.seat_count()).map(|_| None).collect();
    let deliver = |seats: &mut Vec<Option<ServerConn>>, out: roses_server::online::Outbox| {
        for (seat, m) in out {
            if let Some(Some(c)) = seats.get_mut(usize::from(seat)) {
                let _ = c.send(&m);
            }
        }
    };
    while !stop.load(Ordering::Relaxed) {
        let now = start.elapsed().as_millis() as u64;
        while let Ok(c) = rx.try_recv() {
            waiting.push(c);
        }
        let mut still = Vec::new();
        for mut c in waiting.drain(..) {
            match c.poll() {
                Received::Message(ClientMessage::Hello { compat, join, deck, presence, spectate }) => {
                    match game.hello(&compat, &join, deck.as_ref(), presence, spectate, now) {
                        Ok((seat, out)) => {
                            seats[usize::from(seat)] = Some(c);
                            deliver(&mut seats, out);
                        }
                        Err(reason) => {
                            let _ = c.send(&ServerMessage::Refused { reason });
                        }
                    }
                }
                Received::Empty => still.push(c),
                _ => {}
            }
        }
        waiting = still;
        for s in 0..seats.len() {
            loop {
                let Some(c) = seats[s].as_mut() else { break };
                match c.poll() {
                    Received::Message(m) => {
                        let out = game.handle(s as u8, m, now);
                        deliver(&mut seats, out);
                    }
                    _ => break,
                }
            }
        }
        let out = game.tick(now);
        deliver(&mut seats, out);
        std::thread::sleep(Duration::from_micros(300));
    }
}

/// One headless client: join `seat`, play through the pad bot until the
/// match is over and the placing list has shown.
fn client(to_server: mpsc::Sender<ServerConn>, seat: u8, names: Vec<String>, hud: bool) -> Run {
    let cards = roses_net::synthetic::catalogue();
    let data = crate::duel_online::tests::bare_data(cards.clone(), roses_net::synthetic::board());
    let connector: Connector = Box::new(move || {
        let (client, server) = memory_pair::<ClientMessage, ServerMessage>();
        to_server.send(Box::new(server)).map_err(|_| std::io::Error::other("server gone"))?;
        Ok(Box::new(client) as ClientConn)
    });
    let deck = roses_net::synthetic::submissions()[usize::from(seat) % 2].clone();
    let link = OnlineLink::connect(connector, &cards, &RulesTables::empty(), Join::Seat { seat: Some(seat) }, Some(deck), Duration::from_secs(60), |_| {})
        .expect("joined");
    assert_eq!(link.seat, seat);
    let mut play = DuelPlay::online_multi(data, link, names).expect("online play");
    assert!(play.multi.is_some(), "an N-player duel");
    let mut draw = hud.then(|| crate::duel_play::DuelHud::new(&play).expect("HUD"));
    let mut bot = PadBot::new();
    let mut camera = crate::duel_board::BoardCamera::snapped((3, 3));
    let (mut presses, mut turns, mut last_active) = (0, 0, None);
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(240) {
        let link = play.online.as_ref().expect("online");
        let can = !play.animating() && link.may_act();
        let view = link.state.view.clone().expect("a view");
        let revision = link.state.revision;
        let key = bot.key(&play.session.duel, &play.session.input, &view, revision, can, &play.data.catalogue);
        presses += u64::from(key != 0);
        play.press(key);
        if last_active != Some(play.session.duel.active) {
            last_active = Some(play.session.duel.active);
            turns += 1;
        }
        let cursor = play.cursor();
        crate::duel_play::drive_camera(&mut camera, &mut play, cursor, &|_| None);
        camera.tick();
        if let Some(h) = draw.as_mut() {
            h.update(&play);
        }
        if play.halted.is_some() {
            break;
        }
        if !play.animating() && play.online.as_ref().is_some_and(|l| !l.may_act()) {
            std::thread::sleep(Duration::from_micros(300));
        }
    }
    Run {
        seat,
        outcome: play.session.duel.outcome,
        halted: play.halted.clone(),
        presses,
        turns,
        results: play.multi.as_ref().map_or(0, |m| m.results),
    }
}

fn play_match(mode: RoomMode, players: &[u8], seed: u64) -> (Option<(Outcome, roses_net::Cause)>, Vec<Run>) {
    let names: Vec<String> = (0..4).map(|s| format!("P{s}")).collect();
    let team = |s: u8| if mode == RoomMode::Teams { s / 2 } else { s };
    let seats = (0..4u8)
        .map(|s| MultiSeatPlan {
            info: SeatInfo { name: names[usize::from(s)].clone(), computer: !players.contains(&s), team: team(s) },
            deck: (!players.contains(&s)).then(|| roses_net::synthetic::submissions()[usize::from(s) % 2].clone()),
            token: format!("token{s}"),
        })
        .collect();
    let settings = RoomSettings { mode, turn_seconds: 0, ..RoomSettings::default() };
    let mut game = MultiMatch::new(
        "bevy-multi",
        seats,
        settings,
        roses_net::synthetic::board(),
        seed,
        roses_net::synthetic::catalogue(),
        RulesTables::empty(),
        None,
        roses_net::OnlinePolicy::default(),
    )
    .unwrap()
    .with_computer_delay(5);
    let (tx, rx) = mpsc::channel::<ServerConn>();
    let stop = AtomicBool::new(false);
    let runs = std::thread::scope(|scope| {
        let server = scope.spawn(|| serve(&mut game, rx, &stop));
        let clients: Vec<_> = players
            .iter()
            .map(|&seat| {
                let tx = tx.clone();
                let names = names.clone();
                scope.spawn(move || client(tx, seat, names, false))
            })
            .collect();
        let runs: Vec<Run> = clients.into_iter().map(|c| c.join().unwrap()).collect();
        stop.store(true, Ordering::Relaxed);
        server.join().unwrap();
        runs
    });
    (game.ended(), runs)
}

/// FFA: one player (seat 0) and three computers; 2v2: players at S and E
/// (enemies), computers at N and W. Every client follows the match to its
/// end, presents every revision (other seats' turns and battles too) and
/// shows the placing list, then leaves (`MULTI_DONE`).
#[test]
fn bevy_clients_play_n_player_matches_against_the_server() {
    for (mode, players, seed) in [(RoomMode::FreeForAll, vec![0u8], 5u64), (RoomMode::Teams, vec![0u8, 2], 9)] {
        let (ended, runs) = play_match(mode, &players, seed);
        let (outcome, cause) = ended.expect("the match ended");
        eprintln!("{mode:?}: {outcome:?} ({cause:?})");
        for r in &runs {
            eprintln!("seat {}: {} presses, {} turns seen, placing list {} updates, halted {:?}", r.seat, r.presses, r.turns, r.results, r.halted);
            assert_eq!(r.outcome, Some(outcome), "seat {}", r.seat);
            assert_eq!(r.halted.as_deref(), Some(crate::duel_multi::MULTI_DONE), "seat {}", r.seat);
            assert!(r.turns >= 4, "seat {} saw {} turns", r.seat, r.turns);
        }
    }
}
