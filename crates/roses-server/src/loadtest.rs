//! `roses-server loadtest`: how many simultaneous matches a hub carries.
//!
//!   roses-server loadtest --addr <host:port> (--local | --pin sha256:.. | [WebPKI])
//!       (--duel-import <dir> | --synthetic) [--pairs N] [--computer N]
//!       [--duration S] [--think MIN-MAX (ms, default 500-2000) | --fast]
//!       [--invite CODE] [--prefix NAME] [--ramp-ms 100] [--report-every S]
//!
//! Each pair signs up two throwaway accounts (with `--invite`, a code from
//! `roses-server accounts invite --uses N`, which skips the open sign-up
//! limits; without it, open sign-up: 5 an hour per address), pairs them
//! by a private room code and plays both seats with the computer player of
//! `roses_net::client::Bot`, waiting a random think time (`--think`) before
//! each intention, match after match. `--computer N` adds players that play
//! the server's computer instead (one seat each). After `--duration` every
//! match still running is forfeited so the run ends on time. Reported:
//! matches started / completed / forfeited at the end, intention round trip
//! (sent until accepted or rejected) and lobby ping percentiles, time to be
//! paired and to join, connection failures and errors.
//!
//! Point it only at a hub you run for the test (a temporary one, e.g.
//! `hub --local 127.0.0.1:7990 --db /tmp/lt.db --max-matches 512
//! --max-connections 2048`): the accounts it makes stay in that hub's db.

use roses_core::duel::Catalogue;
use roses_net::client::{Bot, BotAction, ClientState};
use roses_net::friends::{LobbyClient, OnlineConfig};
use roses_net::lobby::{AuthProblem, LobbyEvent, LobbyRefusal, LobbyRequest};
use roses_net::transport::{Connection, Received};
use roses_net::{data_fingerprint, synthetic, ClientMessage, Compat, Decision, DeckSubmission, Join, ServerMessage};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

type MatchConn = Box<dyn Connection<ClientMessage, ServerMessage> + Send>;

#[derive(Clone)]
struct Setup {
    config: OnlineConfig,
    compat: Compat,
    cards: Arc<Catalogue>,
    decks: [DeckSubmission; 2],
    invite: String,
    password: String,
    think: (u64, u64),
    duration: Duration,
}

#[derive(Default)]
struct Stats {
    /// Intention round trips (µs).
    rtt: Vec<u32>,
    /// Lobby Ping → Pong (µs).
    ping: Vec<u32>,
    /// Sign-up sent → signed in (ms).
    signup: Vec<u32>,
    /// Room created / computer asked → MatchReady (ms).
    pair: Vec<u32>,
    /// Match Hello → first snapshot (ms).
    join: Vec<u32>,
    commands: u64,
    rejected: u64,
    started: u64,
    completed: u64,
    bot_forfeits: u64,
    end_forfeits: u64,
    connect_failures: u64,
    errors: BTreeMap<String, u64>,
}

impl Stats {
    fn error(&mut self, e: &str) {
        // Group errors by their text up to the first digit (revisions vary).
        let key: String = e.chars().take_while(|c| !c.is_ascii_digit()).take(90).collect();
        *self.errors.entry(key).or_default() += 1;
    }
}

struct Shared {
    stats: Mutex<Stats>,
    active: AtomicUsize,
    /// Players signed in (the matches start once all are).
    signed_in: AtomicUsize,
    /// When matches stop (set once everyone is signed in).
    deadline: OnceLock<Instant>,
}

impl Shared {
    /// The end of the run (far away until it has started).
    fn deadline(&self) -> Instant {
        self.deadline.get().copied().unwrap_or_else(|| Instant::now() + Duration::from_secs(3600))
    }

    fn with<R>(&self, f: impl FnOnce(&mut Stats) -> R) -> R {
        f(&mut self.stats.lock().expect("stats"))
    }
}

/// xorshift64* for think times (not security relevant).
struct Rng(u64);

impl Rng {
    fn between(&mut self, lo: u64, hi: u64) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let r = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        if hi <= lo { lo } else { lo + r % (hi - lo + 1) }
    }
}

fn pct(v: &mut [u32], p: f64) -> u32 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    let i = ((v.len() as f64 - 1.0) * p).round() as usize;
    v[i.min(v.len() - 1)]
}

fn summary_us(v: &mut [u32]) -> String {
    if v.is_empty() {
        return "none".into();
    }
    let ms = |x: u32| x as f64 / 1000.0;
    format!(
        "n={} p50 {:.1} ms, p90 {:.1}, p99 {:.1}, max {:.1}",
        v.len(),
        ms(pct(v, 0.5)),
        ms(pct(v, 0.9)),
        ms(pct(v, 0.99)),
        ms(pct(v, 1.0))
    )
}

fn summary_ms(v: &mut [u32]) -> String {
    if v.is_empty() {
        return "none".into();
    }
    format!("n={} p50 {} ms, p90 {}, p99 {}, max {}", v.len(), pct(v, 0.5), pct(v, 0.9), pct(v, 0.99), pct(v, 1.0))
}

pub fn main(args: Vec<String>) -> Result<(), String> {
    let mut args = args.into_iter();
    let mut addr: Option<String> = None;
    let (mut local, mut pin) = (false, None::<String>);
    let mut import: Option<PathBuf> = None;
    let mut synthetic_data = false;
    let (mut pairs, mut computer) = (8usize, 0usize);
    let mut duration = 60u64;
    let mut think = (500u64, 2000u64);
    let mut invite = String::new();
    let mut prefix: Option<String> = None;
    let mut ramp_ms = 100u64;
    let mut report_every = 10u64;
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--addr" => addr = Some(value()?),
            "--local" => local = true,
            "--pin" => pin = Some(value()?),
            "--duel-import" => import = Some(value()?.into()),
            "--synthetic" => synthetic_data = true,
            "--pairs" => pairs = value()?.parse().map_err(|_| "bad --pairs")?,
            "--computer" => computer = value()?.parse().map_err(|_| "bad --computer")?,
            "--duration" => duration = value()?.parse().map_err(|_| "bad --duration (seconds)")?,
            "--think" => {
                let v = value()?;
                let (lo, hi) = v.split_once('-').unwrap_or((&v, &v));
                think = (lo.parse().map_err(|_| "bad --think")?, hi.parse().map_err(|_| "bad --think")?);
            }
            "--fast" => think = (0, 0),
            "--invite" => invite = value()?,
            "--prefix" => prefix = Some(value()?),
            "--ramp-ms" => ramp_ms = value()?.parse().map_err(|_| "bad --ramp-ms")?,
            "--report-every" => report_every = value()?.parse().map_err(|_| "bad --report-every")?,
            "--help" | "-h" => {
                println!("{}", include_str!("loadtest.rs").lines().take(22).collect::<Vec<_>>().join("\n"));
                return Ok(());
            }
            other => return Err(format!("unknown loadtest argument {other}")),
        }
    }
    let addr = addr.ok_or("--addr <host:port> is required")?;
    let config = OnlineConfig { server: addr.clone(), name: None, tls: !local, pin };
    config.trust()?;
    let (cards, tables, decks) = if synthetic_data {
        (synthetic::catalogue(), roses_net::RulesTables::empty(), synthetic::submissions())
    } else {
        let dir = import.ok_or("--duel-import <dir> is required (the hub's own import), or --synthetic")?;
        let d = roses_net::data::load_duel_import(&dir)?;
        let decks = [0, 1].map(|s| roses_net::deck::from_import_side(&d.first_duel.sides[s]));
        (d.catalogue, d.tables, decks)
    };
    let compat = Compat::current(data_fingerprint(&cards, &tables));
    // Names: <prefix><index><a|b|c>, 3..=12 letters and digits.
    let prefix = prefix.unwrap_or_else(|| {
        let r = crate::random_u64();
        let alpha = b"abcdefghjkmnpqrstuvwxyz";
        let mut s = String::from("lt");
        for i in 0..4 {
            s.push(alpha[((r >> (i * 8)) % alpha.len() as u64) as usize] as char);
        }
        s
    });
    if prefix.len() + 5 > 12 || !prefix.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return Err("--prefix: a letter first, at most 7 characters".into());
    }
    let start = Instant::now();
    let setup = Setup {
        config,
        compat,
        cards: Arc::new(cards),
        decks,
        invite,
        password: "loadtest-password".into(),
        think,
        duration: Duration::from_secs(duration),
    };
    let shared = Arc::new(Shared {
        stats: Mutex::new(Stats::default()),
        active: AtomicUsize::new(0),
        signed_in: AtomicUsize::new(0),
        deadline: OnceLock::new(),
    });
    println!(
        "loadtest {addr}: {pairs} player pairs + {computer} vs the computer, {duration} s, think {}-{} ms, names {prefix}*",
        think.0, think.1
    );
    let mut threads = Vec::new();
    for i in 0..pairs + computer {
        let (setup, shared, prefix) = (setup.clone(), shared.clone(), prefix.clone());
        let vs_computer = i >= pairs;
        threads.push(std::thread::spawn(move || {
            let mut rng = Rng(crate::random_u64() | 1);
            let names: Vec<String> = if vs_computer {
                vec![format!("{prefix}{i}c")]
            } else {
                vec![format!("{prefix}{i}a"), format!("{prefix}{i}b")]
            };
            player_loop(&setup, &shared, &names, &mut rng);
        }));
        std::thread::sleep(Duration::from_millis(ramp_ms));
    }
    // Everyone signs in first (sign-up hashes are the expensive part of a
    // hub's work and are spread out in real use); then the clock starts.
    let total_players = pairs * 2 + computer;
    let sign_in_wait = Instant::now() + Duration::from_secs(300);
    while shared.signed_in.load(Ordering::Relaxed) < total_players
        && Instant::now() < sign_in_wait
        && threads.iter().any(|t| !t.is_finished())
    {
        std::thread::sleep(Duration::from_millis(100));
    }
    println!(
        "{} of {total_players} players signed in after {:.1} s; matches start now for {duration} s",
        shared.signed_in.load(Ordering::Relaxed),
        start.elapsed().as_secs_f64()
    );
    let start = Instant::now();
    let _ = shared.deadline.set(start + setup.duration);
    // Periodic report until every worker is done.
    let mut last_report = Instant::now();
    let mut last_rtt = 0usize;
    let mut last_cmds = 0u64;
    while threads.iter().any(|t| !t.is_finished()) {
        std::thread::sleep(Duration::from_millis(200));
        if last_report.elapsed() >= Duration::from_secs(report_every) {
            let dt = last_report.elapsed().as_secs_f64();
            last_report = Instant::now();
            let active = shared.active.load(Ordering::Relaxed);
            let line = shared.with(|s| {
                let mut recent: Vec<u32> = s.rtt[last_rtt..].to_vec();
                last_rtt = s.rtt.len();
                let cps = (s.commands - last_cmds) as f64 / dt;
                last_cmds = s.commands;
                let errors: u64 = s.errors.values().sum();
                format!(
                    "t={:>4}s active {active:>3}  started {} completed {}  {cps:.0} cmd/s  rtt p50 {:.1} p99 {:.1} max {:.1} ms  errors {errors} conn-fail {}",
                    start.elapsed().as_secs(),
                    s.started,
                    s.completed,
                    pct(&mut recent, 0.5) as f64 / 1000.0,
                    pct(&mut recent, 0.99) as f64 / 1000.0,
                    pct(&mut recent, 1.0) as f64 / 1000.0,
                    s.connect_failures
                )
            });
            println!("{line}");
        }
    }
    for t in threads {
        let _ = t.join();
    }
    let elapsed = start.elapsed().as_secs_f64();
    let mut s = shared.stats.lock().expect("stats");
    println!("--- loadtest summary ({elapsed:.0} s) ---");
    println!(
        "matches: started {}, completed {} ({} by the bot conceding), forfeited at the end {}",
        s.started, s.completed, s.bot_forfeits, s.end_forfeits
    );
    println!("intentions: {} sent ({:.1}/s), {} rejected", s.commands, s.commands as f64 / elapsed, s.rejected);
    println!("intention round trip: {}", summary_us(&mut s.rtt));
    println!("lobby ping: {}", summary_us(&mut s.ping));
    println!("sign-up: {}", summary_ms(&mut s.signup));
    println!("paired (room/computer → match ready): {}", summary_ms(&mut s.pair));
    println!("joined (hello → snapshot): {}", summary_ms(&mut s.join));
    println!("connection failures: {}", s.connect_failures);
    let total: u64 = s.errors.values().sum();
    println!("errors: {total}");
    for (e, n) in &s.errors {
        println!("  {n:>5}  {e}");
    }
    Ok(())
}

/// One lobby player: connected, signed in.
struct Player {
    name: String,
    lobby: LobbyClient,
    ping_at: Option<Instant>,
    next_ping: Instant,
}

impl Player {
    fn pump(&mut self, shared: &Shared) -> Result<Vec<LobbyEvent>, String> {
        let events = self.lobby.pump();
        for e in &events {
            if matches!(e, LobbyEvent::Pong) {
                if let Some(at) = self.ping_at.take() {
                    let us = at.elapsed().as_micros() as u32;
                    shared.with(|s| s.ping.push(us));
                }
            }
        }
        if !self.lobby.connected() {
            return Err(format!("lobby closed: {}", self.lobby.note.clone().unwrap_or_default()));
        }
        if self.ping_at.is_none() && Instant::now() >= self.next_ping {
            self.ping_at = Some(Instant::now());
            self.next_ping = Instant::now() + Duration::from_secs(5);
            self.lobby.send(LobbyRequest::Ping);
        }
        Ok(events)
    }
}

fn connect_lobby(setup: &Setup, shared: &Shared) -> Result<LobbyClient, String> {
    for attempt in 0..20 {
        match roses_net::friends::connect::<LobbyRequest, LobbyEvent>(&setup.config)
            .and_then(|c| LobbyClient::start(c, setup.compat.clone(), "roses-loadtest"))
        {
            Ok(mut l) => {
                let until = Instant::now() + Duration::from_secs(15);
                while l.server.is_none() && l.refused.is_none() && l.connected() && Instant::now() < until {
                    l.pump();
                    std::thread::sleep(Duration::from_millis(5));
                }
                if l.server.is_some() {
                    return Ok(l);
                }
                shared.with(|s| s.connect_failures += 1);
                if let Some(LobbyRefusal::ServerFull) = l.refused {
                    std::thread::sleep(Duration::from_millis(200 + 100 * attempt));
                    continue;
                }
                return Err(format!("lobby hello failed: {:?} {:?}", l.refused, l.note));
            }
            Err(e) => {
                shared.with(|s| s.connect_failures += 1);
                if attempt == 19 {
                    return Err(format!("connect: {e}"));
                }
                std::thread::sleep(Duration::from_millis(200 + 100 * attempt));
            }
        }
    }
    Err("connect: gave up".into())
}

/// Connect and sign in (sign up the first time, then resume the session).
fn sign_in(setup: &Setup, shared: &Shared, name: &str, session: &mut Option<String>) -> Result<Player, String> {
    let mut lobby = connect_lobby(setup, shared)?;
    let t0 = Instant::now();
    let until = t0 + Duration::from_secs(120);
    let request = |session: &Option<String>| match session.clone() {
        Some(s) => LobbyRequest::ResumeSession { session: s },
        None => LobbyRequest::SignUp { username: name.into(), password: setup.password.clone(), invite: setup.invite.clone() },
    };
    lobby.send(request(session));
    let mut fresh = session.is_none();
    loop {
        if Instant::now() > until {
            return Err("sign-in timed out".into());
        }
        lobby.pump();
        if !lobby.connected() {
            return Err(format!("lobby closed while signing in: {:?}", lobby.note));
        }
        if lobby.user.is_some() {
            if fresh {
                let ms = t0.elapsed().as_millis() as u32;
                shared.with(|s| s.signup.push(ms));
            }
            *session = lobby.session.clone();
            return Ok(Player { name: name.into(), lobby, ping_at: None, next_ping: Instant::now() });
        }
        if let Some(p) = lobby.auth_problem.take() {
            match p {
                AuthProblem::Busy => {
                    std::thread::sleep(Duration::from_millis(300));
                    lobby.send(request(session));
                }
                AuthProblem::SessionExpired | AuthProblem::UsernameTaken => {
                    *session = None;
                    fresh = true;
                    lobby.send(LobbyRequest::LogIn { username: name.into(), password: setup.password.clone() });
                }
                p => return Err(format!("sign-in refused: {p:?}")),
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Play match after match until the deadline; reconnect after errors.
fn player_loop(setup: &Setup, shared: &Shared, names: &[String], rng: &mut Rng) {
    let mut sessions: Vec<Option<String>> = vec![None; names.len()];
    let mut counted = false;
    while Instant::now() < shared.deadline() {
        let result = (|| -> Result<(), String> {
            let mut players = Vec::new();
            for (name, session) in names.iter().zip(sessions.iter_mut()) {
                players.push(sign_in(setup, shared, name, session)?);
            }
            if !counted {
                counted = true;
                shared.signed_in.fetch_add(players.len(), Ordering::Relaxed);
            }
            while shared.deadline.get().is_none() {
                for p in players.iter_mut() {
                    p.lobby.pump();
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            while Instant::now() < shared.deadline() {
                one_match(setup, shared, &mut players, rng)?;
            }
            for p in &mut players {
                p.lobby.close("done");
            }
            Ok(())
        })();
        if let Err(e) = result {
            shared.with(|s| s.error(&e));
            std::thread::sleep(Duration::from_millis(2000));
        }
    }
}

fn wait_lobby(
    shared: &Shared,
    players: &mut [Player],
    what: &str,
    secs: u64,
    mut done: impl FnMut(&[Player]) -> bool,
) -> Result<(), String> {
    let until = Instant::now() + Duration::from_secs(secs);
    loop {
        for p in players.iter_mut() {
            for e in p.pump(shared)? {
                if let LobbyEvent::Problem { problem } = e {
                    return Err(format!("lobby problem while waiting for {what}: {problem:?}"));
                }
            }
        }
        if done(players) {
            return Ok(());
        }
        if Instant::now() > until {
            let notes: Vec<String> = players.iter().map(|p| format!("{}: {:?}", p.name, p.lobby.note)).collect();
            return Err(format!("timed out waiting for {what} ({})", notes.join("; ")));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn one_match(setup: &Setup, shared: &Shared, players: &mut [Player], rng: &mut Rng) -> Result<(), String> {
    for p in players.iter_mut() {
        p.lobby.over = None;
        p.lobby.note = None;
    }
    let t0 = Instant::now();
    if players.iter().all(|p| p.lobby.ready.is_some()) {
        // A match from before a reconnect, offered again: play it.
    } else if players.len() == 1 {
        players[0].lobby.send(LobbyRequest::PlayComputer { map: None, computer: None });
    } else {
        players[0].lobby.send(LobbyRequest::CreateRoom { map: None });
        wait_lobby(shared, players, "a room code", 30, |p| p[0].lobby.room.is_some() || p[0].lobby.ready.is_some())?;
        // (A match still running from before a reconnect is offered again:
        // then both are told and play it.)
        if let Some(code) = players[0].lobby.room.clone() {
            players[1].lobby.send(LobbyRequest::JoinRoom { code });
        }
    }
    wait_lobby(shared, players, "the match", 30, |p| p.iter().all(|p| p.lobby.ready.is_some()))?;
    let pair_ms = t0.elapsed().as_millis() as u32;
    shared.with(|s| {
        s.pair.push(pair_ms);
        s.started += 1;
    });
    shared.active.fetch_add(1, Ordering::Relaxed);
    let result = play(setup, shared, players, rng);
    shared.active.fetch_sub(1, Ordering::Relaxed);
    result?;
    // Back in the lobby before the next match.
    wait_lobby(shared, players, "the match result", 30, |p| p.iter().all(|p| p.lobby.over.is_some()))
}

struct Seat {
    conn: MatchConn,
    state: ClientState,
    bot: Bot,
    awaiting: u64,
    pending: Option<(u64, Instant)>,
    next_at: Option<Instant>,
    hello: Instant,
    joined: bool,
    forfeited: bool,
}

fn play(setup: &Setup, shared: &Shared, players: &mut [Player], rng: &mut Rng) -> Result<(), String> {
    let mut seats = Vec::new();
    for p in players.iter() {
        let ready = p.lobby.ready.clone().expect("ready");
        let mut conn = roses_net::friends::connect::<ClientMessage, ServerMessage>(&setup.config).map_err(|e| {
            shared.with(|s| s.connect_failures += 1);
            format!("match connect: {e}")
        })?;
        conn.send(&ClientMessage::Hello {
            compat: setup.compat.clone(),
            join: Join::Ticket { ticket: ready.ticket },
            deck: Some(setup.decks[usize::from(ready.seat & 1)].clone()),
            presence: false,
            spectate: false,
        })
        .map_err(|e| format!("match hello: {e}"))?;
        seats.push(Seat {
            conn,
            state: ClientState::new(),
            bot: Bot::new(),
            awaiting: 0,
            pending: None,
            next_at: None,
            hello: Instant::now(),
            joined: false,
            forfeited: false,
        });
    }
    let give_up = shared.deadline() + Duration::from_secs(60);
    let mut end_forfeit = false;
    loop {
        let now = Instant::now();
        if now > give_up {
            return Err("match did not end after the end forfeit".into());
        }
        for p in players.iter_mut() {
            p.pump(shared)?;
        }
        let mut waiting_decision = false;
        for seat in seats.iter_mut() {
            loop {
                match seat.conn.poll() {
                    Received::Message(m) => {
                        seat.state.apply(&m);
                        match &m {
                            ServerMessage::Refused { reason } => return Err(format!("match refused: {reason:?}")),
                            ServerMessage::LobbyRefused { reason } => return Err(format!("match connection turned away: {reason:?}")),
                            ServerMessage::ProtocolError { message } => return Err(format!("protocol error: {message}")),
                            ServerMessage::Snapshot { .. } if !seat.joined => {
                                seat.joined = true;
                                let ms = seat.hello.elapsed().as_millis() as u32;
                                shared.with(|s| s.join.push(ms));
                            }
                            ServerMessage::Accepted { seq, revision }
                            | ServerMessage::Duplicate { seq, decision: Decision::Accepted { revision } } => {
                                seat.awaiting = seat.awaiting.max(*revision);
                                decided(shared, seat, *seq, false);
                            }
                            ServerMessage::Rejected { seq, .. } | ServerMessage::Duplicate { seq, .. } => {
                                decided(shared, seat, *seq, true);
                            }
                            _ => {}
                        }
                    }
                    Received::Empty => break,
                    Received::Closed => {
                        if seat.state.ended.is_some() {
                            break;
                        }
                        return Err("match connection closed".into());
                    }
                    Received::Malformed(e) => return Err(format!("malformed server message: {e}")),
                }
            }
            if seat.state.ended.is_some() {
                continue;
            }
            if seat.pending.is_some() {
                waiting_decision = true;
                continue;
            }
            if !end_forfeit && seat.state.my_turn() && seat.state.unacked.is_empty() && seat.state.revision >= seat.awaiting {
                let at = *seat.next_at.get_or_insert_with(|| now + Duration::from_millis(rng.between(setup.think.0, setup.think.1)));
                if now >= at {
                    seat.next_at = None;
                    let view = seat.state.view.clone().expect("my_turn has a view");
                    let msg = match seat.bot.next(&view, seat.state.revision, &*setup.cards) {
                        Some(BotAction::Command(c)) => Some(seat.state.command(c)),
                        Some(BotAction::Forfeit) => {
                            seat.forfeited = true;
                            Some(seat.state.forfeit())
                        }
                        None => None,
                    };
                    if let Some(msg) = msg {
                        let seq = match &msg {
                            ClientMessage::Command { seq, .. } | ClientMessage::Forfeit { seq } => *seq,
                            _ => 0,
                        };
                        seat.pending = Some((seq, Instant::now()));
                        seat.conn.send(&msg).map_err(|e| format!("send: {e}"))?;
                        waiting_decision = true;
                    }
                }
            }
        }
        if seats.iter().all(|s| s.state.ended.is_some()) {
            let bot_forfeit = seats.iter().any(|s| s.forfeited);
            shared.with(|s| {
                if end_forfeit {
                    s.end_forfeits += 1;
                } else {
                    s.completed += 1;
                    if bot_forfeit {
                        s.bot_forfeits += 1;
                    }
                }
            });
            for s in seats.iter_mut() {
                s.conn.close();
            }
            return Ok(());
        }
        // Time is up: concede (from seat 0, once it is not waiting for a
        // decision) so the run ends on time.
        if now >= shared.deadline() && !end_forfeit && seats[0].state.welcome_seen() && seats[0].pending.is_none() {
            end_forfeit = true;
            let msg = seats[0].state.forfeit();
            seats[0].conn.send(&msg).map_err(|e| format!("send: {e}"))?;
        }
        // (1 ms while an answer is due, so round trips are measured finely.)
        let pinging = players.iter().any(|p| p.ping_at.is_some());
        let nap = if waiting_decision || pinging { 1 } else { 10 };
        std::thread::sleep(Duration::from_millis(nap));
    }
}

fn decided(shared: &Shared, seat: &mut Seat, seq: u64, rejected: bool) {
    if let Some((pending, at)) = seat.pending {
        if pending == seq {
            seat.pending = None;
            let us = at.elapsed().as_micros() as u32;
            shared.with(|s| {
                s.rtt.push(us);
                s.commands += 1;
                if rejected {
                    s.rejected += 1;
                }
            });
        }
    }
}

trait Welcomed {
    fn welcome_seen(&self) -> bool;
}

impl Welcomed for ClientState {
    fn welcome_seen(&self) -> bool {
        self.seat.is_some() && self.view.is_some()
    }
}
