//! Tournaments end to end over loopback TCP (a port addition): scripted
//! clients sign up, check in, say ready and play every match with the
//! computer-driven client until there is a champion; a quick tournament
//! fills, has a no-show and opens the next one; an older client sees none
//! of it; and a version-1 account database upgrades with a backup.

use roses_net::friends::{connect, LobbyClient, OnlineConfig};
use roses_net::lobby::{
    BracketSide, EntryStatus, LobbyEvent, LobbyProblem, LobbyRequest, TournamentInfo, TournamentState, WinHow, CHECK_IN_OPENS_SECS,
    DRAW_SECS, QUICK_CHECK_IN_SECS, READY_SECS,
};
use roses_net::transport::LoopbackListener;
use roses_net::{synthetic, ClientMessage, Compat, Join, ServerMessage};
use roses_server::accounts::{Accounts, PasswordCost};
use roses_server::hub::{Hub, HubAcceptor, HubData, HubSettings};
use roses_server::play::{play_bot, PlayOptions};
use roses_server::tournament::{bracket_size, Tournament};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Friday 4 Dec 2026 19:00 UTC (the launch tournament's time).
const START: i64 = 1_796_410_800;

fn compat() -> Compat {
    Compat::current(roses_net::data_fingerprint(&synthetic::catalogue(), &roses_net::RulesTables::empty()))
}

fn temp_dir(what: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("roses-tourn-{what}-{:016x}", roses_server::random_u64()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Server {
    addr: String,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Hub>>,
}

impl Server {
    fn start(db: &Path, settings: HubSettings) -> Self {
        let listener = LoopbackListener::bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let accounts = Accounts::open(db).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut hub = Hub::new(settings, HubData::synthetic(false), accounts, |l| eprintln!("hub: {l}"));
            let mut acceptors: Vec<Box<dyn HubAcceptor + Send>> = vec![Box::new(listener)];
            hub.run(&mut acceptors, &flag).unwrap();
            hub
        });
        Self { addr, stop, thread: Some(thread) }
    }

    fn config(&self) -> OnlineConfig {
        OnlineConfig { server: self.addr.clone(), name: None, tls: false, pin: None }
    }

    fn client(&self) -> LobbyClient {
        LobbyClient::start(connect(&self.config()).unwrap(), compat(), "tournament test").unwrap()
    }

    fn stop(mut self) -> Hub {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap()
    }
}

fn settings(clock: fn() -> i64, file: &Path, quick: bool) -> HubSettings {
    HubSettings {
        name: "Tournament test".into(),
        password_cost: PasswordCost::TEST,
        limits: roses_server::serve::Limits::UNLIMITED,
        wall_clock: clock,
        weekly: None,
        quick_tournaments: quick,
        tournaments_file: Some(file.to_path_buf()),
        ..HubSettings::default()
    }
}

fn pump_until(clients: &mut [&mut LobbyClient], what: &str, secs: u64, mut done: impl FnMut(&[&mut LobbyClient]) -> bool) {
    let start = Instant::now();
    loop {
        for c in clients.iter_mut() {
            c.pump();
        }
        if done(clients) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(secs), "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(3));
    }
}

fn info(c: &LobbyClient, id: u64) -> Option<TournamentInfo> {
    c.tournament_list.as_ref()?.iter().find(|t| t.id == id).cloned()
}

/// Sign up `names` with the invite; each asks for the list.
fn players(server: &Server, invite: &str, names: &[String]) -> Vec<LobbyClient> {
    let mut out: Vec<LobbyClient> = names
        .iter()
        .map(|n| {
            let mut c = server.client();
            c.send(LobbyRequest::SignUp { username: n.clone(), password: "tournament pw".into(), invite: invite.into() });
            c
        })
        .collect();
    let mut refs: Vec<&mut LobbyClient> = out.iter_mut().collect();
    pump_until(&mut refs, "sign-ups", 60, |cs| cs.iter().all(|c| c.user.is_some()));
    for c in out.iter_mut() {
        assert!(c.tournaments, "the server offers tournaments");
        c.send(LobbyRequest::ListTournaments);
    }
    out
}

/// Play the running tournament `id` out: every prompt is answered (except
/// the `silent` player's), every match is played by the computer-driven
/// client; when the silent player's opponent is ready and waiting, the
/// clock moves past the ready time. Returns the champion.
fn play_out(server: &Server, clients: &mut [LobbyClient], id: u64, silent: Option<&str>, clock: &AtomicI64) -> String {
    let decks = synthetic::submissions();
    let mut started: HashSet<(String, u8)> = HashSet::new();
    let mut threads = Vec::new();
    let mut jumped = false;
    let begin = Instant::now();
    loop {
        for c in clients.iter_mut() {
            c.pump();
        }
        if let Some(t) = info(&clients[0], id).filter(|t| t.state == TournamentState::Finished) {
            for th in threads {
                let _ = std::thread::JoinHandle::join(th);
            }
            return t.champion.expect("a champion");
        }
        assert!(begin.elapsed() < Duration::from_secs(600), "the tournament did not finish");
        for c in clients.iter_mut() {
            let me = c.user.clone().unwrap();
            if let Some(p) = c.prompt.clone() {
                if p.tournament == id && !p.you_ready && Some(me.as_str()) != silent {
                    c.send(LobbyRequest::TournamentReady { id });
                    // Answered: wait for the server's next prompt.
                    if let Some(p) = c.prompt.as_mut() {
                        p.you_ready = true;
                    }
                }
            }
            if let Some(r) = c.ready.clone() {
                if started.insert((r.match_id.clone(), r.seat)) {
                    let config = server.config();
                    let deck = decks[usize::from(r.seat)].clone();
                    threads.push(std::thread::spawn(move || {
                        play_bot(
                            || connect::<ClientMessage, ServerMessage>(&config),
                            &compat(),
                            &synthetic::catalogue(),
                            PlayOptions { join: Join::Ticket { ticket: r.ticket }, deck: Some(deck), ..PlayOptions::default() },
                            |_| {},
                        )
                    }));
                }
            }
        }
        // The silent player's opponent said ready: the ready time runs out.
        if let (Some(silent), false) = (silent, jumped) {
            let waiting = clients.iter().find(|c| c.user.as_deref() == Some(silent)).and_then(|c| c.prompt.clone());
            if let Some(p) = waiting.filter(|p| p.tournament == id) {
                let opp_ready = clients
                    .iter()
                    .find(|c| c.user.as_deref() == Some(p.opponent.as_str()))
                    .and_then(|c| c.prompt.as_ref())
                    .is_some_and(|q| q.you_ready && q.opponent == silent);
                if opp_ready {
                    clock.fetch_add(READY_SECS + 1, Ordering::SeqCst);
                    jumped = true;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(3));
    }
}

/// A scheduled tournament of `n` checked-in players (and one who signed
/// up but did not check in), with an optional no-show, to its champion.
fn scheduled(n: usize, silent: bool, clock: &'static AtomicI64, clock_fn: fn() -> i64) {
    let dir = temp_dir(&format!("sched{n}"));
    let db = dir.join("accounts.db");
    let file = dir.join("tournaments.json");
    let operator = Accounts::open(&db).unwrap();
    let invite = operator.create_invite(200, Some("test"), None).unwrap();
    clock.store(START - 2 * 86_400, Ordering::SeqCst);
    let server = Server::start(&db, settings(clock_fn, &file, false));
    // The operator's command (another process in real life).
    let id = operator.insert_tournament(&Tournament::new("Friday Cup", None, START, 128, true, START - 3 * 86_400)).unwrap();
    let mut names: Vec<String> = (1..=n).map(|i| format!("Duelist{i}")).collect();
    if silent {
        names[n - 1] = "Silent".into();
    }
    names.push("Absent".into());
    let mut clients = players(&server, &invite, &names);
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "the tournament listed", 30, |cs| cs.iter().all(|c| info(c, id).is_some()));
    }
    // A new account may not create one (no finished match).
    clients[0].send(LobbyRequest::CreateTournament { name: "My Cup".into(), starts: START + 86_400, cap: 8 });
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().take(1).collect();
        pump_until(&mut refs, "not eligible", 30, |cs| cs[0].note.as_deref() == Some(&*LobbyProblem::NotEligible.to_string()));
    }
    // Sign-up is open at once; the preview grows as players join.
    for (i, c) in clients.iter_mut().enumerate() {
        c.send(LobbyRequest::JoinTournament { id });
        if i == 0 {
            c.send(LobbyRequest::TournamentBracket { id });
        }
    }
    let total = names.len();
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "everyone signed up", 30, |cs| {
            cs[0].bracket.as_ref().is_some_and(|b| b.tournament.entrants as usize == total)
                && cs.iter().all(|c| info(c, id).is_some_and(|t| t.you == Some(EntryStatus::SignedUp)))
        });
    }
    let preview = clients[0].bracket.clone().unwrap();
    assert!(preview.preview);
    assert_eq!(usize::from(preview.size), bracket_size(total), "the preview counts everyone signed up");
    let first: Vec<String> = preview.rounds[0]
        .iter()
        .flat_map(|g| g.sides.iter())
        .filter_map(|s| match s {
            BracketSide::Player { name, seed: 1 } => Some(name.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(first, vec![names[0].clone()], "the preview is in sign-up order");
    // Check-in is not open until 60 minutes before.
    clients[0].send(LobbyRequest::CheckIn { id });
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().take(1).collect();
        pump_until(&mut refs, "too early", 30, |cs| cs[0].note.as_deref() == Some(&*LobbyProblem::NotCheckInTime.to_string()));
    }
    clock.store(START - CHECK_IN_OPENS_SECS, Ordering::SeqCst);
    for c in clients.iter_mut().filter(|c| c.user.as_deref() != Some("Absent")) {
        c.send(LobbyRequest::CheckIn { id });
    }
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "check-in", 30, |cs| info(cs[0], id).is_some_and(|t| t.checked_in as usize == n));
    }
    // The draw at 30 minutes before, from the checked-in players only.
    clock.store(START - DRAW_SECS, Ordering::SeqCst);
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "the draw", 30, |cs| cs[0].bracket.as_ref().is_some_and(|b| !b.preview));
    }
    let drawn = clients[0].bracket.clone().unwrap();
    assert_eq!(drawn.tournament.state, TournamentState::Drawn);
    assert_eq!(usize::from(drawn.size), bracket_size(n));
    let in_bracket: Vec<String> = drawn.rounds[0]
        .iter()
        .flat_map(|g| g.sides.iter())
        .filter_map(|s| match s {
            BracketSide::Player { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(in_bracket.len(), n);
    assert!(!in_bracket.contains(&"Absent".to_string()), "not checked in: dropped at the draw");
    let absent = clients.iter().find(|c| c.user.as_deref() == Some("Absent")).unwrap();
    assert_eq!(info(absent, id).unwrap().you, Some(EntryStatus::Out));
    let byes = drawn.rounds[0].iter().filter(|g| g.how == Some(WinHow::Bye)).count();
    assert_eq!(byes, bracket_size(n) - n);
    assert!(clients.iter().all(|c| c.prompt.is_none()), "no match before the start");
    // The start: matches begin.
    clock.store(START, Ordering::SeqCst);
    let champion = play_out(&server, &mut clients, id, silent.then_some("Silent"), clock);
    assert!(names.contains(&champion), "{champion}");
    assert_ne!(champion, "Absent");
    // The champion's badge shows in everyone's player list.
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "the badge", 30, |cs| cs[1].players.iter().any(|p| p.name == champion && p.titles == 1));
    }
    // The web page's file (written every few seconds).
    let begin = Instant::now();
    loop {
        let json: serde_json::Value = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let shown = json["tournaments"].as_array().is_some_and(|l| {
            l.iter().any(|t| t["tournament"]["champion"] == champion.as_str() && t["preview"] == false && t["rounds"].as_array().is_some_and(|r| !r.is_empty()))
        });
        if shown {
            assert_eq!(json["preview_banner"], roses_net::lobby::PREVIEW_BANNER);
            break;
        }
        assert!(begin.elapsed() < Duration::from_secs(20), "the tournaments file does not show the champion");
        std::thread::sleep(Duration::from_millis(100));
    }
    let hub = server.stop();
    let t = hub.tournaments().into_iter().find(|t| t.id == id).unwrap().clone();
    let b = t.bracket.as_ref().unwrap();
    assert_eq!(t.champion.as_deref(), Some(champion.as_str()));
    if silent {
        let lost: Vec<_> = b.rounds.iter().flatten().filter(|g| g.how == Some(WinHow::Walkover)).collect();
        assert_eq!(lost.len(), 1, "the silent player lost by walkover");
        assert!(!b.alive(b.index_of("Silent").unwrap()));
    }
    // The history: a row per decided match, played ones linked to their duel.
    let rows = operator.tournament_results(id).unwrap();
    assert_eq!(rows.len(), bracket_size(n) - 1);
    let played = rows.iter().filter(|r| r.5 == "played").count();
    assert!(played >= 1 && rows.iter().filter(|r| r.5 == "played").all(|r| r.6.is_some()));
    assert_eq!(operator.badges_of(&champion).unwrap(), (1, 0));
    drop(hub);
    let _ = std::fs::remove_dir_all(&dir);
}

static CLOCK_16: AtomicI64 = AtomicI64::new(0);
fn clock_16() -> i64 {
    CLOCK_16.load(Ordering::SeqCst)
}
static CLOCK_5: AtomicI64 = AtomicI64::new(0);
fn clock_5() -> i64 {
    CLOCK_5.load(Ordering::SeqCst)
}
static CLOCK_Q: AtomicI64 = AtomicI64::new(0);
fn clock_q() -> i64 {
    CLOCK_Q.load(Ordering::SeqCst)
}

#[test]
fn sixteen_players_play_a_weekly_tournament_to_a_champion() {
    scheduled(16, false, &CLOCK_16, clock_16);
}

#[test]
fn five_players_with_a_no_show_play_to_a_champion() {
    scheduled(5, true, &CLOCK_5, clock_5);
}

#[test]
fn a_quick_tournament_fills_has_a_no_show_plays_out_and_opens_the_next() {
    let dir = temp_dir("quick");
    let db = dir.join("accounts.db");
    let file = dir.join("tournaments.json");
    let operator = Accounts::open(&db).unwrap();
    let invite = operator.create_invite(50, None, None).unwrap();
    CLOCK_Q.store(START, Ordering::SeqCst);
    let server = Server::start(&db, settings(clock_q, &file, true));
    // An older client (the released game: no `tournaments` in its hello).
    let mut old = LobbyClient::start_before_tournaments(connect(&server.config()).unwrap(), compat(), "old game").unwrap();
    old.send(LobbyRequest::SignUp { username: "OldGame".into(), password: "tournament pw".into(), invite: invite.clone() });
    let mut old_events = Vec::new();
    let names: Vec<String> = ["Yugi", "Joey", "Mai", "NoShow", "Kaiba"].iter().map(|s| s.to_string()).collect();
    let mut clients = players(&server, &invite, &names);
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "the quick tournament", 30, |cs| cs[0].tournament_list.as_ref().is_some_and(|l| l.iter().any(|t| t.quick)));
    }
    let quick = clients[0].tournament_list.as_ref().unwrap().iter().find(|t| t.quick).unwrap().clone();
    assert_eq!((quick.state, quick.entrants, quick.cap, quick.starts), (TournamentState::Open, 0, 4, 0));
    let id = quick.id;
    // Kaiba joins and leaves before it fills; then four join.
    clients[4].send(LobbyRequest::JoinTournament { id });
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "Kaiba in", 30, |cs| info(cs[0], id).is_some_and(|t| t.entrants == 1));
    }
    clients[4].send(LobbyRequest::LeaveTournament { id });
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "Kaiba out", 30, |cs| info(cs[0], id).is_some_and(|t| t.entrants == 0));
    }
    for c in clients.iter_mut().take(4) {
        c.send(LobbyRequest::JoinTournament { id });
    }
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "the check-in prompts", 30, |cs| cs.iter().take(4).all(|c| c.check_in.as_ref().is_some_and(|(t, _)| t.id == id)));
    }
    let (_, closes) = clients[0].check_in.clone().unwrap();
    assert_eq!(closes, START + QUICK_CHECK_IN_SECS);
    // A fresh quick tournament opened for the next players.
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "the next quick one", 30, |cs| {
            cs[4].tournament_list.as_ref().is_some_and(|l| l.iter().any(|t| t.quick && t.id != id && t.state == TournamentState::Open && t.entrants == 0))
        });
    }
    // Three check in; NoShow does not.
    for c in clients.iter_mut().take(3) {
        c.send(LobbyRequest::CheckIn { id });
    }
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "three checked in", 30, |cs| info(cs[0], id).is_some_and(|t| t.checked_in == 3));
    }
    CLOCK_Q.store(closes, Ordering::SeqCst);
    let champion = play_out(&server, &mut clients, id, None, &CLOCK_Q);
    assert!(["Yugi", "Joey", "Mai"].contains(&champion.as_str()), "{champion}");
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "the quick badge", 30, |cs| cs[4].players.iter().any(|p| p.name == champion && p.quick_titles == 1 && p.titles == 0));
    }
    let noshow = clients.iter().find(|c| c.user.as_deref() == Some("NoShow")).unwrap();
    assert_eq!(info(noshow, id).unwrap().you, Some(EntryStatus::Out));
    // The older client was never sent a tournament message.
    for _ in 0..50 {
        old_events.extend(old.pump());
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(old.user.is_some(), "the older client signed in and stayed");
    assert!(!old.tournaments || old.tournament_list.is_none());
    let tournament_events = old_events
        .iter()
        .filter(|e| {
            matches!(
                e,
                LobbyEvent::Tournaments { .. }
                    | LobbyEvent::TournamentBracket { .. }
                    | LobbyEvent::TournamentMatch { .. }
                    | LobbyEvent::TournamentCheckIn { .. }
                    | LobbyEvent::TournamentCreated { .. }
            )
        })
        .count();
    assert_eq!(tournament_events, 0);
    assert!(old.tournament_list.is_none() && old.prompt.is_none() && old.check_in.is_none());
    let hub = server.stop();
    let t = hub.tournaments().into_iter().find(|t| t.id == id).unwrap().clone();
    assert_eq!(t.bracket.as_ref().unwrap().size(), 4);
    assert_eq!(t.bracket.as_ref().unwrap().rounds[0].iter().filter(|g| g.how == Some(WinHow::Bye)).count(), 1, "the no-show's slot is a bye");
    assert_eq!(operator.badges_of(&champion).unwrap(), (0, 1));
    drop(hub);
    let _ = std::fs::remove_dir_all(&dir);
}

static CLOCK_C: AtomicI64 = AtomicI64::new(0);
fn clock_c() -> i64 {
    CLOCK_C.load(Ordering::SeqCst)
}

#[test]
fn creating_needs_a_finished_match_and_one_open_tournament_per_creator() {
    let dir = temp_dir("create");
    let db = dir.join("accounts.db");
    let operator = Accounts::open(&db).unwrap();
    let invite = operator.create_invite(10, None, None).unwrap();
    let now = START - 10 * 86_400;
    CLOCK_C.store(now, Ordering::SeqCst);
    let server = Server::start(&db, settings(clock_c, &dir.join("t.json"), false));
    let mut clients = players(&server, &invite, &["Pegasus".to_string(), "Bakura".to_string()]);
    // Pegasus has a finished match on record.
    operator
        .record_match(&roses_server::accounts::MatchRow {
            id: "match-1".into(),
            players: ["Pegasus".into(), "COMPUTER".into()],
            started: 1,
            ended: 2,
            outcome: "Pegasus won (LifePoints)".into(),
            cause: "Command".into(),
            revisions: 10,
        })
        .unwrap();
    let ask = |c: &mut LobbyClient, name: &str, starts: i64, cap: u16| {
        c.note = None;
        c.created = None;
        c.send(LobbyRequest::CreateTournament { name: name.into(), starts, cap });
        let mut refs = vec![c];
        pump_until(&mut refs, "an answer", 30, |cs| cs[0].note.is_some() || cs[0].created.is_some());
    };
    let note = |p: LobbyProblem| Some(p.to_string());
    ask(&mut clients[1], "Bakura Cup", now + 86_400, 8);
    assert_eq!(clients[1].note, note(LobbyProblem::NotEligible), "Bakura has no finished match");
    ask(&mut clients[0], "Bad  Name", now + 86_400, 8);
    assert_eq!(clients[0].note, note(LobbyProblem::BadTournament));
    ask(&mut clients[0], "Pegasus Cup", now + 60, 8);
    assert_eq!(clients[0].note, note(LobbyProblem::BadTournament), "too soon");
    ask(&mut clients[0], "Pegasus Cup", now + 86_400, 200);
    assert_eq!(clients[0].note, note(LobbyProblem::BadTournament), "cap too big");
    ask(&mut clients[0], "Pegasus Cup", now + 86_400, 8);
    let created = clients[0].created.expect("created");
    ask(&mut clients[0], "Pegasus Cup Two", now + 2 * 86_400, 8);
    assert_eq!(clients[0].note, note(LobbyProblem::AlreadyHosting));
    // Its creator cancels it; then a new one may be made.
    clients[0].send(LobbyRequest::CancelTournament { id: created });
    {
        let mut refs: Vec<&mut LobbyClient> = clients.iter_mut().collect();
        pump_until(&mut refs, "cancelled", 30, |cs| info(cs[0], created).is_some_and(|t| t.state == TournamentState::Cancelled));
    }
    ask(&mut clients[0], "Pegasus Cup Two", now + 2 * 86_400, 8);
    assert!(clients[0].created.is_some());
    let _ = server.stop();
    let _ = std::fs::remove_dir_all(&dir);
}

/// The version-1 schema exactly as servers before tournaments made it.
const V1_SCHEMA: &str = "CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT NOT NULL, name_key TEXT NOT NULL UNIQUE, phc TEXT NOT NULL,
        created INTEGER NOT NULL, disabled INTEGER NOT NULL DEFAULT 0, invite TEXT);
     CREATE TABLE invites(code TEXT PRIMARY KEY, uses_left INTEGER NOT NULL, for_user TEXT, note TEXT, created INTEGER NOT NULL, expires INTEGER);
     CREATE TABLE sessions(token_hash TEXT PRIMARY KEY, user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        created INTEGER NOT NULL, expires INTEGER NOT NULL);
     CREATE TABLE matches(id TEXT PRIMARY KEY, player0 TEXT NOT NULL, player1 TEXT NOT NULL, started INTEGER NOT NULL,
        ended INTEGER NOT NULL, outcome TEXT NOT NULL, cause TEXT NOT NULL, revisions INTEGER NOT NULL);
     INSERT INTO users(name, name_key, phc, created) VALUES ('Yugi', 'yugi', 'x', 1), ('Kaiba', 'kaiba', 'y', 2);
     INSERT INTO invites(code, uses_left, created) VALUES ('ABCDEFGH', 3, 1);
     INSERT INTO matches VALUES ('match-a', 'Yugi', 'Kaiba', 1, 2, 'Yugi won (LifePoints)', 'Command', 40);
     PRAGMA user_version = 1;";

fn tables(db: &rusqlite::Connection) -> Vec<String> {
    let mut st = db.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name").unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
}

#[test]
fn a_version_1_database_upgrades_with_a_backup_and_keeps_everything() {
    let dir = temp_dir("migrate");
    let path = dir.join("accounts.db");
    {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.pragma_update(None, "journal_mode", "WAL").unwrap();
        db.execute_batch(V1_SCHEMA).unwrap();
    }
    let a = Accounts::open(&path).unwrap();
    assert_eq!(a.schema_version().unwrap(), 2);
    assert_eq!(a.users().unwrap().len(), 2);
    assert_eq!(a.matches(10).unwrap().len(), 1);
    assert_eq!(a.invites().unwrap().len(), 1);
    assert_eq!(a.finished_matches_of("kaiba").unwrap(), 1);
    let id = a.insert_tournament(&Tournament::new("Kept Cup", None, START, 16, false, 1)).unwrap();
    // The backup is the version-1 file as it was.
    let backups: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("accounts.db.pre-v2-"))
        .collect();
    assert_eq!(backups.len(), 1, "{backups:?}");
    let old = rusqlite::Connection::open(&backups[0]).unwrap();
    assert_eq!(old.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 1);
    assert_eq!(tables(&old), ["invites", "matches", "sessions", "users"]);
    assert_eq!(old.query_row("SELECT COUNT(*) FROM users", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
    let now_tables = tables(&rusqlite::Connection::open(&path).unwrap());
    for t in ["badges", "tournament_actions", "tournament_entries", "tournament_matches", "tournaments", "users", "matches"] {
        assert!(now_tables.iter().any(|n| n == t), "{t} missing: {now_tables:?}");
    }
    drop(a);
    // Opening again changes nothing and makes no second backup.
    let a = Accounts::open(&path).unwrap();
    assert_eq!(a.schema_version().unwrap(), 2);
    let count = std::fs::read_dir(&dir).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains("pre-v2")).count();
    assert_eq!(count, 1);
    // Rollback: marked version 1 (an older server opens it); the next newer
    // server upgrades it again and the tournaments are still there.
    a.mark_schema_v1_for_rollback().unwrap();
    drop(a);
    let raw = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(raw.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 1);
    drop(raw);
    let a = Accounts::open(&path).unwrap();
    assert_eq!(a.schema_version().unwrap(), 2);
    assert_eq!(a.tournament(id).unwrap().unwrap().name, "Kept Cup");
    drop(a);
    // A file from a newer server is refused.
    rusqlite::Connection::open(&path).unwrap().pragma_update(None, "user_version", 3).unwrap();
    assert!(Accounts::open(&path).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
