//! `roses-server multibench`: what N-player rooms full of computer players
//! cost the hub's one thread (issue 12).
//!
//!   roses-server multibench (--duel-import <dir> | --synthetic) [--rooms N]
//!       [--seconds S] [--mode ffa|teams] [--delay-ms MS (default 350, the hub's)]
//!
//! Runs `--rooms` N-player matches in this one thread, as the hub does (it
//! ticks every match each pass): seat 0 joins and concedes at once, the
//! other three are the server's computer players with the opponents' decks
//! and play the match out; a match that ends is replaced by a new one.
//! Time is real (the computers wait `--delay-ms` between commands, as on
//! the live hub). Reported: passes, commands, matches finished, the share
//! of the run spent ticking (the hub thread's load from computers alone)
//! and pass-time percentiles. A pass that takes long holds every other
//! connection on the hub for that time.

use crate::multi::{MultiMatch, MultiSeatPlan};
use roses_core::duel::Catalogue;
use roses_net::client::ClientState;
use roses_net::lobby::{RoomMode, RoomSettings};
use roses_net::{data_fingerprint, synthetic, Compat, DeckSubmission, Join, OnlinePolicy, RulesTables, SeatInfo};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What the bench plays with.
pub struct BenchData {
    pub cards: Catalogue,
    pub tables: RulesTables,
    pub terrain: Vec<u8>,
    pub player_deck: DeckSubmission,
    pub computer_decks: Vec<(String, DeckSubmission)>,
    pub computer_tables: Option<Arc<roses_core::duel::ai::weevil::Tables>>,
}

impl BenchData {
    pub fn synthetic() -> Self {
        let decks = synthetic::submissions();
        Self {
            cards: synthetic::catalogue(),
            tables: RulesTables::empty(),
            terrain: synthetic::board(),
            player_deck: decks[0].clone(),
            computer_decks: vec![("CPU".into(), decks[1].clone())],
            computer_tables: None,
        }
    }

    pub fn from_import(dir: &std::path::Path) -> Result<Self, String> {
        let d = roses_net::data::load_duel_import(dir)?;
        let player_deck = roses_net::deck::from_import_side(&d.first_duel.sides[0]);
        let mut computer_decks: Vec<(String, DeckSubmission)> = d.opponents.into_iter().map(|o| (o.name, o.deck)).collect();
        if computer_decks.is_empty() {
            computer_decks.push(("CPU".into(), roses_net::deck::from_import_side(&d.first_duel.sides[1])));
        }
        Ok(Self {
            cards: d.catalogue,
            tables: d.tables,
            terrain: d.first_duel.terrain,
            player_deck,
            computer_decks,
            computer_tables: d.computer_tables.map(Arc::new),
        })
    }
}

/// The result of a run.
#[derive(Clone, Debug, Default)]
pub struct BenchReport {
    pub rooms: usize,
    pub wall: Duration,
    /// Time spent in the matches' ticks.
    pub busy: Duration,
    pub passes: u64,
    /// Revisions committed (computer commands and turn ends).
    pub revisions: u64,
    pub finished: u64,
    /// Pass times, sorted (µs).
    pub pass_us: Vec<u32>,
}

impl BenchReport {
    pub fn percentile(&self, p: f64) -> u32 {
        if self.pass_us.is_empty() {
            return 0;
        }
        let i = ((self.pass_us.len() - 1) as f64 * p).round() as usize;
        self.pass_us[i]
    }

    pub fn load(&self) -> f64 {
        self.busy.as_secs_f64() / self.wall.as_secs_f64().max(1e-9)
    }

    pub fn line(&self) -> String {
        format!(
            "{} rooms, {:.1} s: hub thread {:.1} % busy with computer rooms; {} passes, {} revisions ({:.0}/s), {} matches finished; pass p50 {} us, p99 {} us, max {} us",
            self.rooms,
            self.wall.as_secs_f64(),
            100.0 * self.load(),
            self.passes,
            self.revisions,
            self.revisions as f64 / self.wall.as_secs_f64().max(1e-9),
            self.finished,
            self.percentile(0.5),
            self.percentile(0.99),
            self.pass_us.last().copied().unwrap_or(0)
        )
    }
}

struct Room {
    game: MultiMatch,
    revision: u64,
}

fn new_room(data: &BenchData, mode: RoomMode, delay_ms: u64, seed: u64, now: u64) -> Result<Room, String> {
    let team = |s: usize| if mode == RoomMode::Teams { (s / 2) as u8 } else { s as u8 };
    let seats = (0..4)
        .map(|s| {
            let (name, deck) = if s == 0 {
                ("Bench".to_string(), None)
            } else {
                let (n, d) = &data.computer_decks[(seed as usize + s) % data.computer_decks.len()];
                (n.clone(), Some(d.clone()))
            };
            MultiSeatPlan { info: SeatInfo { name, computer: s > 0, team: team(s) }, deck, token: format!("bench{s}") }
        })
        .collect();
    let settings = RoomSettings { mode, turn_seconds: 0, ..RoomSettings::default() };
    let mut game = MultiMatch::new(
        format!("bench-{seed}"),
        seats,
        settings,
        data.terrain.clone(),
        seed,
        data.cards.clone(),
        data.tables.clone(),
        data.computer_tables.clone(),
        OnlinePolicy::default(),
    )?
    .with_computer_delay(delay_ms);
    let compat = Compat::current(data_fingerprint(&data.cards, &data.tables));
    let mut client = ClientState::new();
    let (_, out) = game
        .hello(&compat, &Join::Seat { seat: Some(0) }, Some(&data.player_deck), false, false, now)
        .map_err(|r| format!("the bench seat was refused: {r:?}"))?;
    for (_, m) in &out {
        client.apply(m);
    }
    // Seat 0 concedes; the three computers play the match out.
    let _ = game.handle(0, client.forfeit(), now);
    Ok(Room { game, revision: 0 })
}

/// Run `rooms` computer rooms in this thread for `seconds`.
pub fn run(data: &BenchData, rooms: usize, seconds: f64, mode: RoomMode, delay_ms: u64) -> Result<BenchReport, String> {
    let start = Instant::now();
    let now_ms = || start.elapsed().as_millis() as u64;
    let mut seed = 1u64;
    let mut all = Vec::with_capacity(rooms);
    for _ in 0..rooms {
        all.push(new_room(data, mode, delay_ms, seed, 0)?);
        seed += 1;
    }
    let mut report = BenchReport { rooms, ..BenchReport::default() };
    let run_for = Duration::from_secs_f64(seconds);
    let begun = Instant::now();
    while begun.elapsed() < run_for {
        let pass = Instant::now();
        let now = now_ms();
        for room in all.iter_mut() {
            let _ = room.game.tick(now);
            let r = room.game.revision();
            report.revisions += r - room.revision;
            room.revision = r;
            if room.game.ended().is_some() {
                report.finished += 1;
                *room = new_room(data, mode, delay_ms, seed, now)?;
                seed += 1;
            }
        }
        let took = pass.elapsed();
        report.busy += took;
        report.passes += 1;
        report.pass_us.push(took.as_micros().min(u128::from(u32::MAX)) as u32);
        // The hub sleeps 1 ms on a pass with nothing to do.
        if took < Duration::from_millis(1) {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    report.wall = begun.elapsed();
    report.pass_us.sort_unstable();
    Ok(report)
}

/// The `multibench` subcommand.
pub fn main(args: Vec<String>) -> Result<(), String> {
    let mut args = args.into_iter();
    let (mut import, mut synthetic_data) = (None::<PathBuf>, false);
    let (mut rooms, mut seconds, mut mode, mut delay) = (vec![1usize, 10, 50, 100], 20.0f64, RoomMode::FreeForAll, 350u64);
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--duel-import" => import = Some(value()?.into()),
            "--synthetic" => synthetic_data = true,
            "--rooms" => rooms = value()?.split(',').map(|n| n.trim().parse().map_err(|_| "bad --rooms")).collect::<Result<_, _>>()?,
            "--seconds" => seconds = value()?.parse().map_err(|_| "bad --seconds")?,
            "--delay-ms" => delay = value()?.parse().map_err(|_| "bad --delay-ms")?,
            "--mode" => {
                mode = match value()?.as_str() {
                    "ffa" => RoomMode::FreeForAll,
                    "teams" => RoomMode::Teams,
                    o => return Err(format!("--mode {o}: ffa or teams")),
                }
            }
            o => return Err(format!("unknown multibench argument {o}")),
        }
    }
    let data = match (import, synthetic_data) {
        (Some(dir), _) => BenchData::from_import(&dir)?,
        (None, true) => BenchData::synthetic(),
        (None, false) => return Err("multibench needs --duel-import <dir> or --synthetic".into()),
    };
    for n in rooms {
        println!("{}", run(&data, n, seconds, mode, delay)?.line());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computer_rooms_play_on_one_thread_and_the_report_adds_up() {
        let r = run(&BenchData::synthetic(), 3, 1.0, RoomMode::FreeForAll, 0).unwrap();
        assert_eq!(r.rooms, 3);
        assert!(r.passes > 0 && r.pass_us.len() as u64 == r.passes);
        assert!(r.revisions > 0, "the computers played: {}", r.line());
        assert!(r.load() > 0.0 && r.load() <= 1.0);
        assert!(r.percentile(0.5) <= r.percentile(0.99));
    }
}
