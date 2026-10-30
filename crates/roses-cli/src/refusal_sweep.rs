//! Rules-engine refusal sweep (private, needs the user's executable).
//!
//! Plays many computer-vs-computer sessions through `DuelSession` and tallies
//! every `DuelError::Unsupported` outcome by message, so the refusals real
//! players can reach are found and ported in order of frequency. Notes:
//! `work/research/duel/claude-effects/REFUSALS-SWEEP.md`.
//!
//! Player decks are the ones a real player can field:
//! - every starter deck the name-derived deck select can offer (records
//!   referenced by the candidate table 0x2990C0, leader from the info record
//!   0x2990E0), played with its leader at rank 0;
//! - "progressed" decks: random deck-editor-legal 40-card decks (at most three
//!   copies of a card, 0x214C70) drawn from the starter decks plus the cards
//!   the reward reels can award from every campaign opponent met before the
//!   duel (reel-eligible cards, byte 0xB bit 5, of decks 0x1A + control for
//!   the controls below it [H: route order approximated by control number])
//!   and the special-prize pool (bit 6), a random monster of that pool as the
//!   leader at a random rank 0..=12. The deck cost (DC) limit is not applied.
#![cfg(test)]

use roses_core::duel::session::{ComputerSeat, DuelSession};
use roses_core::duel::{Duel, DuelError, DuelSetup, PAD_CONTROLLER};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const EE_TO_FILE: usize = 0xF9680;
const CANDIDATE_TABLE_EE: usize = 0x2990C0;
const DECK_INFO_EE: usize = 0x2990E0;
const ABILITY_TABLE_EE: usize = 0x2C26B8;

#[derive(Clone, Debug)]
struct Side {
    label: String,
    cards: Vec<u16>,
    leader: u16,
    rank: u8,
}

#[derive(Clone, Debug)]
struct Job {
    control: u8,
    /// Side the computer opponent sits on.
    cpu: u8,
    map: usize,
    seed: u64,
    player: Side,
    opponent: Side,
    custom: bool,
}

fn word(elf: &[u8], ee: usize) -> u16 {
    let at = ee - EE_TO_FILE;
    u16::from_le_bytes([elf[at], elf[at + 1]])
}

fn abilities(elf: &[u8], card: u16, rank: u8) -> Vec<u8> {
    let at = ABILITY_TABLE_EE - EE_TO_FILE + usize::from(card) * 40;
    let mut row = [0u16; 20];
    for (i, h) in row.iter_mut().enumerate() {
        *h = u16::from_le_bytes([elf[at + 2 * i], elf[at + 2 * i + 1]]);
    }
    roses_core::duel::queries::leader_abilities(&row, i32::from(rank))
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A deck-editor-legal deck (<= 3 copies) from `pool`, leader a monster.
fn progressed(pool: &[u16], monsters: &[u16], rng: &mut Rng, label: String) -> Side {
    let mut cards = Vec::with_capacity(40);
    while cards.len() < 40 {
        let c = pool[rng.below(pool.len())];
        if cards.iter().filter(|&&x| x == c).count() < 3 {
            cards.push(c);
        }
    }
    Side {
        label,
        cards,
        leader: monsters[rng.below(monsters.len())],
        rank: rng.below(13) as u8,
    }
}

fn env_list(name: &str) -> Option<Vec<u8>> {
    std::env::var(name)
        .ok()
        .map(|v| v.split(',').map(|c| c.trim().parse().unwrap()).collect())
}

fn run_job(
    job: &Job,
    elf: &[u8],
    setup: &crate::duel_text::Setup,
    maps: &[roses_assets::TerrainMap],
    tables: &roses_core::duel::ai::weevil::Tables,
) -> String {
    let head = format!(
        "control {} cpu-side {} map {} seed {} player {} vs {}",
        job.control, job.cpu, job.map, job.seed, job.player.label, job.opponent.label
    );
    let mut rng = job.seed;
    let (p, o) = (&job.player, &job.opponent);
    let pa = abilities(elf, p.leader, p.rank);
    let oa = abilities(elf, o.leader, o.rank);
    let human = job.cpu ^ 1;
    let mut sides = [p, o];
    let mut abil = [pa, oa];
    if job.cpu == 0 {
        sides.swap(0, 1);
        abil.swap(0, 1);
    }
    let mut controllers = [PAD_CONTROLLER, PAD_CONTROLLER];
    controllers[usize::from(job.cpu)] = job.control;
    let duel_setup = DuelSetup {
        terrain: maps[job.map].cells.clone(),
        leaders: [sides[0].leader, sides[1].leader],
        decks: roses_core::duel::build_campaign_decks(&mut rng, [&sides[0].cards, &sides[1].cards]),
        first: human,
        leader_abilities: abil,
        controllers,
    };
    let mut duel = match Duel::new(duel_setup, &setup.cards) {
        Ok((duel, _)) => duel,
        Err(DuelError::Unsupported(what)) => return format!("{head}: REFUSED at setup: {what}"),
        Err(e) => return format!("{head}: ERROR at setup: {e}"),
    };
    duel.set_destiny_rows(setup.destiny_rows.clone());
    if job.custom {
        let mut ranks = [p.rank, o.rank];
        if job.cpu == 0 {
            ranks.swap(0, 1);
        }
        duel.set_leader_ranks(ranks);
        duel.set_custom_rules([4000, 4000], [3, 3]);
    }
    duel.seed_rng(rng);
    let mut seats = [None, None];
    seats[usize::from(human)] = Some(ComputerSeat::new(&duel, human, 0));
    seats[usize::from(job.cpu)] = Some(ComputerSeat::with_controller(&duel, job.cpu, 0, job.control));
    let mut session = DuelSession::new(duel, seats);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        session.run(2_000_000, &setup.cards, tables)
    }));
    // ROSES_SWEEP_HASHES: the trajectory hash of each session (the events
    // of a session that ran to its end, then the final duel state), for
    // proving a rules change leaves 1v1 play bit-identical.
    if let Some(path) = std::env::var_os("ROSES_SWEEP_HASHES") {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        if let Ok(Ok(events)) = &result {
            hasher.update(serde_json::to_vec(events).unwrap());
        }
        hasher.update(serde_json::to_vec(&session.duel).unwrap());
        let line = format!("{head}	{:x}
", hasher.finalize());
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        f.write_all(line.as_bytes()).unwrap();
    }
    let state = format!(
        "round {} (side {} to act, LP {:?})",
        session.duel.round,
        session.duel.active,
        [session.duel.sides[0].lp, session.duel.sides[1].lp]
    );
    match result {
        Ok(Ok(_)) => match session.duel.outcome {
            Some(outcome) => format!("{head}: ended in {state} {outcome:?}"),
            None => format!("{head}: UNFINISHED in {state}"),
        },
        Ok(Err(DuelError::Unsupported(what))) => {
            // ROSES_SWEEP_DUMP: the refused decision point (the session is
            // restored to it: duel, seats with the memory before the sync,
            // cursors) for an interpreter run of the original module.
            if let Some(dir) = std::env::var_os("ROSES_SWEEP_DUMP") {
                let dump = serde_json::json!({
                    "job": head,
                    "what": what,
                    "control": job.control,
                    "side": job.cpu,
                    "duel": &session.duel,
                    "seats": &session.seats,
                    "cursors": &session.input.cursors,
                });
                let name = format!("c{}-s{}-{}.json", job.control, job.cpu, job.seed);
                let path = std::path::Path::new(&dir).join(name);
                std::fs::write(path, serde_json::to_string(&dump).unwrap()).unwrap();
            }
            format!("{head}: REFUSED in {state}: {what}")
        }
        Ok(Err(e)) => format!("{head}: ERROR in {state}: {e}"),
        Err(p) => {
            let what = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            format!("{head}: PANIC in {state}: {what}")
        }
    }
}

/// The refusal kind of a summary line (the message after the last ": "
/// of "REFUSED ...: what").
fn kind(line: &str) -> Option<String> {
    for tag in ["REFUSED", "ERROR", "PANIC", "UNFINISHED"] {
        if let Some(at) = line.find(&format!(": {tag}")) {
            let rest = &line[at + 2..];
            return Some(match tag {
                "REFUSED" | "ERROR" => {
                    let what = rest.split_once("): ").map_or_else(
                        || rest.split_once(": ").map_or(rest, |x| x.1),
                        |x| x.1,
                    );
                    format!("{tag}: {what}")
                }
                "PANIC" => format!("PANIC: {}", rest.split_once("): ").map_or("", |x| x.1)),
                _ => tag.to_string(),
            });
        }
    }
    None
}

#[test]
#[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); release build, minutes"]
fn private_refusal_sweep() {
    // ROSES_SWEEP_SEEDS (8): seeds per control and layout;
    // ROSES_SWEEP_CONTROLS: comma list (default 1..=21, 22, 25);
    // ROSES_SWEEP_THREADS (cores - 2); ROSES_SWEEP_OUT: summary file.
    let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
    let seeds: u64 = std::env::var("ROSES_SWEEP_SEEDS").ok().map_or(8, |v| v.parse().unwrap());
    let only = env_list("ROSES_SWEEP_CONTROLS");
    // ROSES_SWEEP_SEED_LIST / ROSES_SWEEP_CPU: only these seeds / cpu sides
    // (re-running single sessions of a sweep; ROSES_SWEEP_DUMP dumps them).
    let seed_list: Option<Vec<u64>> = std::env::var("ROSES_SWEEP_SEED_LIST")
        .ok()
        .map(|v| v.split(',').map(|c| c.trim().parse().unwrap()).collect());
    let cpu_only = env_list("ROSES_SWEEP_CPU");
    let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
    let flags = roses_assets::parse_pal_reward_flags(&elf).unwrap();
    let statics = roses_core::deck_editor::StaticCards::from_executable(&elf, 0).unwrap();
    let is_monster = |c: u16| statics.kind(c) == Some(0);

    // Starter records offered by the deck select.
    let mut starters: Vec<u8> = (0..16)
        .flat_map(|i| {
            let w = word(&elf, CANDIDATE_TABLE_EE + 2 * i);
            (0..3).map(move |k| ((w >> (5 * k)) & 0x1F) as u8)
        })
        .collect();
    starters.sort_unstable();
    starters.dedup();
    let starter = |r: u8| Side {
        label: format!("starter {r:#x}"),
        cards: decks[usize::from(r)].cards.clone(),
        leader: word(&elf, DECK_INFO_EE + 12 * usize::from(r)) & 0x3FF,
        rank: 0,
    };
    let starter_cards: Vec<u16> = starters
        .iter()
        .flat_map(|&r| decks[usize::from(r)].cards.clone())
        .collect();
    let special: Vec<u16> = (0..854u16).filter(|&c| flags.special_pool(c)).collect();
    let pool_before = |control: u8| -> Vec<u16> {
        let mut pool = starter_cards.clone();
        pool.extend(&special);
        for c in 1..control.min(22) {
            pool.extend(
                decks[0x1A + usize::from(c)]
                    .cards
                    .iter()
                    .copied()
                    .filter(|&x| flags.reel_eligible(x)),
            );
        }
        pool.sort_unstable();
        pool.dedup();
        pool.retain(|&c| c < 854 && statics.record(c).is_some());
        pool
    };
    let opponent_side = |record: usize| Side {
        label: format!("deck {record:#x}"),
        cards: decks[record].cards.clone(),
        leader: decks[record].leader_card,
        rank: decks[record].leader_rank_raw.min(12),
    };

    let mut jobs = Vec::new();
    // 23 (deck 0x30) and 24 (deck 0x31) have no retail producer found [H];
    // they run only when named in ROSES_SWEEP_CONTROLS.
    let controls: Vec<u8> = (1..=21).chain([22, 25, 23, 24]).collect();
    for &control in &controls {
        let named = only.as_ref().is_some_and(|o| o.contains(&control));
        if only.as_ref().is_some_and(|o| !o.contains(&control)) || (matches!(control, 23 | 24) && !named) {
            continue;
        }
        let custom = control >= 22;
        let pool = pool_before(control);
        let monsters: Vec<u16> = pool.iter().copied().filter(|&c| is_monster(c)).collect();
        for cpu in [0u8, 1] {
            if cpu_only.as_ref().is_some_and(|o| !o.contains(&cpu)) {
                continue;
            }
            for seed in 1..=seeds {
                if seed_list.as_ref().is_some_and(|l| !l.contains(&seed)) {
                    continue;
                }
                let mut rng = Rng(u64::from(control) << 40 ^ u64::from(cpu) << 32 ^ seed);
                let player = if seed % 2 == 1 {
                    starter(starters[((seed / 2) as usize + usize::from(control)) % starters.len()])
                } else {
                    progressed(&pool, &monsters, &mut rng, format!("progressed#{seed}"))
                };
                let opponent = match control {
                    22 => Side { label: "mirror".into(), ..player.clone() },
                    25 => opponent_side(0x32),
                    24 => opponent_side(0x31),
                    23 => opponent_side(0x30),
                    c => opponent_side(0x1A + usize::from(c)),
                };
                let map = if custom {
                    0x16 + ((seed - 1) % 24) as usize
                } else {
                    usize::from(control)
                };
                jobs.push(Job { control, cpu, map, seed, player, opponent, custom });
            }
        }
    }

    let threads = std::env::var("ROSES_SWEEP_THREADS")
        .ok()
        .map_or(22, |v| v.parse().unwrap());
    let next = AtomicUsize::new(0);
    let lines = Mutex::new(vec![String::new(); jobs.len()]);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let setup = crate::duel_text::prepare(&elf, Vec::new(), 1).unwrap();
                let maps = roses_assets::parse_pal_rules(&elf).unwrap().maps;
                let tables = roses_core::duel::ai::weevil::Tables::new(
                    &crate::duel_catalogue::build(&elf).unwrap().fusions,
                    &elf,
                )
                .unwrap();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    let line = run_job(job, &elf, &setup, &maps, &tables);
                    eprintln!("{line}");
                    lines.lock().unwrap()[i] = line;
                }
            });
        }
    });
    let lines = lines.into_inner().unwrap();
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_control: BTreeMap<u8, (usize, usize)> = BTreeMap::new();
    for (job, line) in jobs.iter().zip(&lines) {
        let e = per_control.entry(job.control).or_default();
        match kind(line) {
            Some(k) => {
                *tally.entry(k).or_default() += 1;
                e.1 += 1;
            }
            None => e.0 += 1,
        }
    }
    let mut out = String::new();
    for line in &lines {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("\n== per control: ended / not ended ==\n");
    for (c, (ok, bad)) in &per_control {
        out.push_str(&format!("control {c}: {ok} ended, {bad} not\n"));
    }
    let mut kinds: Vec<_> = tally.into_iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out.push_str("\n== kinds ==\n");
    for (k, n) in &kinds {
        out.push_str(&format!("{n:5} {k}\n"));
    }
    let total_bad: usize = kinds.iter().map(|x| x.1).sum();
    out.push_str(&format!("\ntotal {} sessions, {} not ended\n", lines.len(), total_bad));
    eprintln!("{}", &out[out.find("\n== per control").unwrap()..]);
    if let Some(path) = std::env::var_os("ROSES_SWEEP_OUT") {
        std::fs::write(path, out).unwrap();
    }
}

/// One refusal dump's decision replanned by the port (REFUSALS-SWEEP.md §7):
/// ROSES_DUMP_FILE (a ROSES_SWEEP_DUMP file), ROSES_DUMP_LP1 (optional: side
/// 1's LP overridden, as the interpreter runs of the original did).
#[test]
#[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH) and a dump"]
fn private_dump_plan() {
    let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
    let dump: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(std::env::var_os("ROSES_DUMP_FILE").unwrap()).unwrap())
            .unwrap();
    let mut duel: Duel = serde_json::from_value(dump["duel"].clone()).unwrap();
    if let Some(lp) = std::env::var("ROSES_DUMP_LP1").ok().map(|v| v.parse().unwrap()) {
        duel.sides[1].lp = lp;
    }
    let side = dump["side"].as_u64().unwrap() as u8;
    let mut seat: ComputerSeat = serde_json::from_value(dump["seats"][usize::from(side)].clone()).unwrap();
    let setup = crate::duel_text::prepare(&elf, Vec::new(), 1).unwrap();
    let tables = roses_core::duel::ai::weevil::Tables::new(
        &crate::duel_catalogue::build(&elf).unwrap().fusions,
        &elf,
    )
    .unwrap();
    roses_core::duel::ai::weevil::sync(&mut seat.memory, &duel, side);
    let plan = roses_core::duel::ai::weevil::plan_for_rng(
        seat.controller,
        &duel,
        side,
        &mut seat.memory,
        &setup.cards,
        &tables,
        seat.tier,
    );
    eprintln!("PLAN {plan:?}");
}

/// N-player matches (MODES, a port addition) with real decks: four
/// computer players (the ported modules on 2-side projections,
/// `roses_core::duel::multi::computer`) per session, free-for-all and 2v2,
/// cards leaving or staying. Tallies outcomes, knockouts by forfeit (a
/// computer whose every try was refused), unfinished sessions and panics.
/// ROSES_MULTI_SEEDS (16).
#[test]
#[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); release build"]
fn private_multi_sweep() {
    use roses_core::duel::multi::computer::{play_out, MultiComputer};
    use roses_core::duel::{EndReason, MultiMode, MultiRules, MultiSetup};
    let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
    let seeds: u64 = std::env::var("ROSES_MULTI_SEEDS").ok().map_or(16, |v| v.parse().unwrap());
    let setup = crate::duel_text::prepare(&elf, Vec::new(), 1).unwrap();
    let maps = roses_assets::parse_pal_rules(&elf).unwrap().maps;
    let tables = roses_core::duel::ai::weevil::Tables::new(&crate::duel_catalogue::build(&elf).unwrap().fusions, &elf).unwrap();
    let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    for seed in 1..=seeds {
        for (mode, leave) in [(MultiMode::FreeForAll, true), (MultiMode::FreeForAll, false), (MultiMode::Teams, true)] {
            let mut rng = Rng(seed << 8 ^ u64::from(leave) << 4 ^ (mode == MultiMode::Teams) as u64);
            let records: Vec<usize> = (0..4).map(|_| 0x1B + rng.below(21)).collect();
            let sources: Vec<Vec<u16>> = records.iter().map(|&r| decks[r].cards.clone()).collect();
            let src: Vec<&[u16]> = sources.iter().map(Vec::as_slice).collect();
            let mut lcg = seed;
            let built = roses_core::duel::build_decks(&mut lcg, &src);
            let ms = MultiSetup {
                rules: MultiRules { mode, cards_leave: leave, ..MultiRules::default() },
                terrain: maps[0x16 + rng.below(24)].cells.clone(),
                leaders: records.iter().map(|&r| decks[r].leader_card).collect(),
                decks: built,
                first: rng.below(4) as u8,
                leader_abilities: records
                    .iter()
                    .map(|&r| abilities(&elf, decks[r].leader_card, decks[r].leader_rank_raw.min(12)))
                    .collect(),
                controllers: vec![PAD_CONTROLLER; 4],
            };
            let head = format!("seed {seed} {mode:?} leave {leave} decks {records:x?}");
            let (mut duel, _) = match Duel::new_multi(ms, &setup.cards) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{head}: SETUP {e}");
                    *tally.entry("setup refused".into()).or_default() += 1;
                    continue;
                }
            };
            duel.set_destiny_rows(setup.destiny_rows.clone());
            duel.seed_rng(lcg);
            let mut computers: Vec<MultiComputer> = (0..4).map(|s| MultiComputer::new(&duel, s)).collect();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                play_out(duel, &mut computers, &setup.cards, &tables, 200_000)
            }));
            let line = match result {
                Err(_) => "PANIC".to_string(),
                Ok(d) => {
                    let m = d.multi().unwrap();
                    let forfeits = m.out.iter().flatten().filter(|k| k.reason == EndReason::Surrender).count();
                    *tally.entry(format!("forfeits {forfeits}")).or_default() += 1;
                    match d.outcome {
                        Some(o) => format!("{:?} round {}", o.reason, d.round),
                        None => format!("UNFINISHED round {}", d.round),
                    }
                }
            };
            eprintln!("{head}: {line}");
            let key = line.split(" round").next().unwrap().to_string();
            *tally.entry(key).or_default() += 1;
        }
    }
    eprintln!("== multi sweep ==");
    for (k, n) in &tally {
        eprintln!("{n:5} {k}");
    }
    assert!(!tally.contains_key("PANIC"), "a session panicked");
}
