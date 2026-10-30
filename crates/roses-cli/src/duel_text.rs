//! A terminal duel against Weevil for testing the rules engine end to end.
//!
//! This is a development harness, not the game's presentation: the board is
//! printed as text and commands are typed. The duel itself is set up from the
//! user's disc as in the original's first campaign duel (player deck record
//! 0x09, Weevil's 0x1C, board 2, the campaign deck shuffle, the player
//! first). The original RNG state cannot be reproduced, so the shuffle uses a
//! seed. Leader abilities are left out: the ones these leaders have (#0, #1,
//! #3) have no modelled effect.

use roses_core::duel::ai::{self, pad, weevil};
use roses_core::duel::ui::{self, DuelUi};
use roses_core::duel::{
    build_campaign_decks, CardData, Catalogue, Duel, DuelCommand, DuelEvent, DuelSetup,
    PAD_CONTROLLER,
};
use roses_core::{GridPos, UnitId};
use std::io::{self, BufRead, Write};

pub const PLAYER_DECK: usize = 0x09;
pub const WEEVIL_DECK: usize = 0x1C;
pub const WEEVIL_BOARD: usize = 2;

/// Everything read from the disc for this duel.
pub struct Setup {
    pub cards: Catalogue,
    pub names: Vec<String>,
    pub setup: DuelSetup,
    pub opponent: Opponent,
    /// The Destiny Draw's card rows (0x2B7AF0) for `Duel::set_destiny_rows`.
    pub destiny_rows: roses_core::duel::script::DestinyRows,
}

/// Who plays side 1.
pub enum Opponent {
    /// The ported controller 0x1117D0 with its planner 0x10D210.
    Weevil(Box<dyn weevil::FusionData>),
    /// No AI: side 1 ends every turn (explicitly a stand-in).
    Pass,
}

pub fn prepare(executable: &[u8], names: Vec<String>, seed: u64) -> crate::Result<Setup> {
    let cards = crate::duel_catalogue::build(executable)?;
    let decks = roses_assets::parse_pal_decks(executable)?.decks;
    let rules = roses_assets::parse_pal_rules(executable)?;
    let mut rng = seed;
    let shuffled = build_campaign_decks(
        &mut rng,
        [&decks[PLAYER_DECK].cards, &decks[WEEVIL_DECK].cards],
    );
    let setup = DuelSetup {
        terrain: rules.maps[WEEVIL_BOARD].cells.clone(),
        leaders: [
            decks[PLAYER_DECK].leader_card,
            decks[WEEVIL_DECK].leader_card,
        ],
        decks: shuffled,
        first: 0,
        leader_abilities: [vec![], vec![]],
        // The pad side and Weevil's module (side +0x33C = 100 / 2).
        controllers: [PAD_CONTROLLER, 2],
    };
    let opponent = match crate::duel_catalogue::fusion_data(executable) {
        Ok(data) => Opponent::Weevil(data),
        Err(_) => Opponent::Pass,
    };
    let destiny_rows = roses_assets::parse_pal_destiny_rows(executable)?;
    Ok(Setup {
        cards,
        names,
        setup,
        opponent,
        destiny_rows,
    })
}

const TERRAIN: [char; 10] = ['F', 'W', 'M', 'm', 'S', 'D', 'T', '.', '#', 'X'];

fn name(names: &[String], id: u16) -> &str {
    names.get(usize::from(id)).map_or("?", String::as_str)
}

fn short(names: &[String], id: u16) -> String {
    name(names, id)
        .chars()
        .filter(|c| !c.is_whitespace())
        .take(6)
        .collect()
}

pub fn render(duel: &Duel, cards: &Catalogue, names: &[String], viewer: u8) -> String {
    let mut out = String::new();
    let side = |s: usize| &duel.sides[s];
    out.push_str(&format!(
        "Round {}  {} to act   Weevil LP {} stars {}   You LP {} stars {}\n",
        duel.round,
        if duel.active == 0 { "you" } else { "Weevil" },
        side(1).lp,
        side(1).stars,
        side(0).lp,
        side(0).stars,
    ));
    out.push_str("      0        1        2        3        4        5        6\n");
    for row in 0..7u8 {
        out.push_str(&format!("{row} "));
        for column in 0..7u8 {
            let at = GridPos { column, row };
            let t = TERRAIN[usize::from(duel.terrain[at.index().unwrap()]).min(9)];
            let cell = match duel.unit_at(at) {
                None => format!("{t}       "),
                Some(u) => {
                    let visible = u.side == viewer || u.committed_face_up;
                    let tag = if u.side == 0 { '+' } else { '-' };
                    let label = if u.slot == 0 {
                        "LEADER".to_string()
                    } else if visible {
                        short(names, u.card)
                    } else {
                        "??????".to_string()
                    };
                    let mark = if u.defence { 'd' } else { ' ' };
                    format!("{tag}{label:<6}{mark}")
                }
            };
            out.push_str(&format!("{cell} "));
        }
        out.push('\n');
    }
    out.push_str(
        "Terrain: F forest W wasteland M mountain m meadow S sea D dark T toon . normal # labyrinth X crush;\n\
         + yours - Weevil's, d = defence\n",
    );
    out.push_str("Your hand: ");
    for (i, &id) in duel.sides[usize::from(viewer)].hand.iter().enumerate() {
        let c = cards.card(id).expect("card");
        if c.is_monster() {
            out.push_str(&format!(
                "[{i}] {} L{} {}/{}  ",
                name(names, id),
                c.level,
                c.attack,
                c.defense
            ));
        } else {
            out.push_str(&format!("[{i}] {}  ", name(names, id)));
        }
    }
    out.push('\n');
    out
}

fn describe(e: &DuelEvent, duel: &Duel, names: &[String]) -> Option<String> {
    let unit = |id: &UnitId| {
        duel.unit(*id)
            .map_or("a unit".to_string(), |u| name(names, u.card).to_string())
    };
    Some(match e {
        DuelEvent::Summoned { card, at, .. } => {
            format!(
                "summoned {} at {},{}",
                name(names, *card),
                at.column,
                at.row
            )
        }
        DuelEvent::Moved { unit: u, to, .. } => {
            format!("{} moved to {},{}", unit(u), to.column, to.row)
        }
        DuelEvent::Battle { attack, defend, .. } => format!("battle {attack} vs {defend}"),
        DuelEvent::Damage { side, amount } => {
            format!(
                "{} lose {amount} LP",
                if *side == 0 { "you" } else { "Weevil" }
            )
        }
        DuelEvent::Destroyed { card, .. } => format!("{} destroyed", name(names, *card)),
        DuelEvent::Revealed { card, .. } => format!("{} revealed", name(names, *card)),
        DuelEvent::Activated { card, .. } => format!("{} activated", name(names, *card)),
        DuelEvent::Fused { result, .. } => format!("fusion: {}", name(names, *result)),
        DuelEvent::Equipped { monster, power_up, value, .. } => {
            format!("{} raises {} by {value}", name(names, *power_up), name(names, *monster))
        }
        DuelEvent::Dropped { card, .. } => format!("{} to the graveyard", name(names, *card)),
        DuelEvent::Spellbound { unit: u } => format!("{} spellbound", unit(u)),
        DuelEvent::Cancelled { unit: u } => format!("{}'s action was stopped", unit(u)),
        DuelEvent::Ended { outcome } => format!("duel over: {outcome:?}"),
        _ => return None,
    })
}

fn cell(text: &str) -> Option<GridPos> {
    let digits: Vec<u8> = text
        .chars()
        .filter(|c| c.is_ascii_digit())
        .map(|c| c as u8 - b'0')
        .collect();
    match digits[..] {
        [column, row] if column < 7 && row < 7 => Some(GridPos { column, row }),
        _ => None,
    }
}

/// Parse one typed command for side 0.
fn parse(line: &str, duel: &Duel) -> Result<Vec<DuelCommand>, String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    match words.first().copied() {
        Some("s" | "summon") => {
            let at = words
                .get(1)
                .and_then(|w| cell(w))
                .ok_or("summon needs a cell, e.g. s 35 0")?;
            let hand = words[2..]
                .iter()
                .map(|w| {
                    w.parse::<usize>()
                        .map_err(|_| format!("bad hand index {w}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if hand.is_empty() {
                return Err("summon needs hand indices, e.g. s 35 0 or s 35 0 2 (fusion)".into());
            }
            Ok(vec![
                DuelCommand::OpenSummon { cell: at },
                DuelCommand::ChooseSummon { hand },
            ])
        }
        Some("m" | "move") => {
            let from = words
                .get(1)
                .and_then(|w| cell(w))
                .ok_or("move needs a cell")?;
            let unit = duel.unit_at(from).ok_or("no unit there")?.id;
            let (mut path, mut face_up, mut defence) = (Vec::new(), false, None);
            for w in &words[2..] {
                match *w {
                    "up" => face_up = true,
                    "def" => defence = Some(true),
                    "atk" => defence = Some(false),
                    w => path.push(cell(w).ok_or(format!("bad cell {w}"))?),
                }
            }
            Ok(vec![DuelCommand::Move {
                unit,
                path,
                face_up,
                defence,
            }])
        }
        Some("e" | "end") => Ok(vec![DuelCommand::EndTurn]),
        Some("surrender") => Ok(vec![DuelCommand::Surrender]),
        _ => {
            Err("commands: s <cell> <hand…> | m <cell> [cells…] [up] [def|atk] | e | b | q".into())
        }
    }
}

/// What a computer turn reads.
struct Data<'a> {
    cards: &'a Catalogue,
    fusions: &'a dyn weevil::FusionData,
    names: &'a [String],
}

/// Delegates to the boxed tables (the planner takes `impl FusionData`).
struct Tables<'a>(&'a dyn weevil::FusionData);
impl weevil::FusionData for Tables<'_> {
    fn fuse(&self, a: u16, b: u16) -> Option<u16> {
        self.0.fuse(a, b)
    }
    fn equip(&self, monster: u16, card: u16) -> i32 {
        self.0.equip(monster, card)
    }
    fn category(&self, card: u16) -> u8 {
        self.0.category(card)
    }
    fn value(&self, card: u16) -> u16 {
        self.0.value(card)
    }
}

/// A computer turn through the ported planner (0x10D210, the controller
/// Weevil uses; `side` 0 lets it play the player's deck as well). As in the
/// original, each plan becomes a key queue (`ai::keys`, emitter 0x277BD0)
/// fed to the same pad-driven input states a human uses (`ui::DuelUi`), so
/// a key the board refuses is dropped exactly where the original drops it.
/// The driver's watchdog (0x1117D0, 600 keys in a turn) presses Circle three
/// times and Start.
fn weevil_turn(
    duel: &mut Duel,
    side: u8,
    input: &mut DuelUi,
    memory: &mut weevil::Memory,
    data: &Data,
    out: &mut impl Write,
) -> crate::Result<()> {
    let Data {
        cards,
        fusions,
        names,
    } = *data;
    use roses_core::duel::session::WATCHDOG_CALLS;
    let who = if side == 1 {
        "Weevil"
    } else {
        "Computer (you)"
    };
    // One loop pass = one controller call of 0x1117D0: the watchdog counts
    // every call (NoKey included), calls 600-602 give Circle, later ones
    // Start, as in `roses_core::duel::session`.
    let mut queue = std::collections::VecDeque::new();
    for calls in 1..=WATCHDOG_CALLS + 8 {
        if duel.outcome.is_some() || duel.active != side {
            return Ok(());
        }
        let key = if calls > WATCHDOG_CALLS + 2 {
            pad::START
        } else if calls >= WATCHDOG_CALLS {
            pad::CIRCLE
        } else {
            if queue.is_empty() {
                weevil::sync(memory, duel, side);
                match weevil::plan(duel, side, memory, cards, &Tables(fusions), 0)? {
                    weevil::Plan::EndTurn => queue.push_back(pad::START),
                    weevil::Plan::NoKey => continue,
                    // The module marked the unit acted itself (Pegasus's
                    // 0x1E30F0); Weevil's module never does.
                    weevil::Plan::MarkActed(id) => {
                        if let Some(u) = duel.units.iter_mut().find(|u| u.id == id) {
                            u.acted = true;
                        }
                        continue;
                    }
                    weevil::Plan::Act(action) => queue.extend(ai::keys(
                        duel,
                        side,
                        input.cursors[usize::from(side)],
                        &action,
                        cards,
                    )),
                }
            }
            queue.pop_front().expect("a planned key")
        };
        let before = input.clone();
        let ui::Response::Commands(commands) = input.press(duel, cards, key)? else {
            continue;
        };
        let mut next = duel.clone();
        let mut shown = Vec::new();
        for cmd in &commands {
            match next.apply(side, cmd, cards) {
                Ok((d, events)) => {
                    shown.extend(events.iter().filter_map(|e| describe(e, &d, names)));
                    next = d;
                }
                Err(e) => {
                    *input = before;
                    return Err(format!("{who}: {cmd:?}: {e}").into());
                }
            }
        }
        for text in shown {
            writeln!(out, "  {who}: {text}")?;
        }
        *duel = next;
    }
    Err(format!("{who} did not finish the turn after the watchdog").into())
}

/// `computer_player`: side 0 is played by the same planner (a test mode).
pub fn run(
    prepared: Setup,
    input: impl BufRead,
    mut out: impl Write,
    computer_player: bool,
) -> crate::Result<i32> {
    let Setup {
        cards,
        names,
        setup,
        opponent,
        destiny_rows,
    } = prepared;
    let (mut duel, _) = Duel::new(setup, &cards)?;
    duel.set_destiny_rows(destiny_rows);
    let mut memory = weevil::Memory::new(&duel, 1);
    let mut own_memory = weevil::Memory::new(&duel, 0);
    let mut pad_input = DuelUi::new(&duel);
    if let Opponent::Pass = opponent {
        writeln!(
            out,
            "Note: Weevil's AI is not available; he will end every turn."
        )?;
    }
    write!(out, "{}", render(&duel, &cards, &names, 0))?;
    let mut lines = input.lines();
    while duel.outcome.is_none() {
        if duel.active == 0 && computer_player {
            let Opponent::Weevil(f) = &opponent else {
                return Err("the computer player needs the planner tables".into());
            };
            let data = Data {
                cards: &cards,
                fusions: f.as_ref(),
                names: &names,
            };
            if let Err(e) = weevil_turn(
                &mut duel,
                0,
                &mut pad_input,
                &mut own_memory,
                &data,
                &mut out,
            ) {
                writeln!(out, "Your computer turn stopped: {e}")?;
                return Ok(2);
            }
            write!(out, "{}", render(&duel, &cards, &names, 0))?;
            continue;
        }
        if duel.active == 1 {
            match &opponent {
                Opponent::Weevil(f) => {
                    if let Err(e) = weevil_turn(
                        &mut duel,
                        1,
                        &mut pad_input,
                        &mut memory,
                        &Data {
                            cards: &cards,
                            fusions: f.as_ref(),
                            names: &names,
                        },
                        &mut out,
                    ) {
                        writeln!(out, "Weevil's turn stopped: {e}")?;
                        return Ok(2);
                    }
                }
                Opponent::Pass => duel = duel.apply(1, &DuelCommand::EndTurn, &cards)?.0,
            }
            write!(out, "{}", render(&duel, &cards, &names, 0))?;
            continue;
        }
        write!(out, "> ")?;
        out.flush()?;
        let Some(line) = lines.next().transpose()? else {
            return Ok(0);
        };
        let line = line.trim();
        match line {
            "" => continue,
            "q" | "quit" => return Ok(0),
            "b" | "board" => {
                write!(out, "{}", render(&duel, &cards, &names, 0))?;
                continue;
            }
            _ => (),
        }
        match parse(line, &duel) {
            Err(e) => writeln!(out, "{e}")?,
            Ok(commands) => {
                let mut next = duel.clone();
                let mut shown = Vec::new();
                let mut failed = None;
                for cmd in commands {
                    match next.apply(0, &cmd, &cards) {
                        Ok((d, events)) => {
                            shown.extend(events.iter().filter_map(|e| describe(e, &d, &names)));
                            next = d;
                        }
                        Err(e) => {
                            failed = Some(e);
                            break;
                        }
                    }
                }
                match failed {
                    Some(e) => writeln!(out, "refused: {e}")?,
                    None => {
                        for s in shown {
                            writeln!(out, "  {s}")?;
                        }
                        duel = next;
                        if duel.active == 0 {
                            write!(out, "{}", render(&duel, &cards, &names, 0))?;
                        }
                    }
                }
            }
        }
    }
    writeln!(out, "Duel over: {:?}", duel.outcome)?;
    Ok(0)
}

pub fn run_stdio(prepared: Setup, computer_player: bool) -> crate::Result<i32> {
    let stdin = io::stdin();
    run(prepared, stdin.lock(), io::stdout(), computer_player)
}

#[cfg(test)]
mod tests {
    use roses_core::duel::session::{ComputerSeat, DuelSession};
    use roses_core::duel::{Duel, DuelError};

    /// Pegasus's deck record (opponent 9, Lancashire) and his control byte.
    const PEGASUS_DECK: usize = 0x23;
    const PEGASUS_CONTROL: u8 = 9;
    /// Ability table 0x2C26B8 (rows of 20 u16); ELF offset = EE − 0xF9680.
    const ABILITY_TABLE_FILE: usize = 0x2C_26B8 - 0xF_9680;

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_pegasus_to_the_end_or_refuse() {
        // The player's starter deck (Weevil's module playing it) against
        // Pegasus's deck 0x23 with his own module (0x135B00), both through
        // `DuelSession`. Each duel must end or be refused explicitly
        // (`DuelError::Unsupported`: the effects his signature cards
        // activate are not modelled by the engine); anything else fails.
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let pegasus = &decks[PEGASUS_DECK];
        let row_at = ABILITY_TABLE_FILE + usize::from(pegasus.leader_card) * 40;
        let mut row = [0u16; 20];
        for (i, h) in row.iter_mut().enumerate() {
            *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
        }
        let abilities =
            roses_core::duel::queries::leader_abilities(&row, i32::from(pegasus.leader_rank_raw));
        assert_eq!(abilities, [0, 2, 3, 9, 10]);
        let mut summary = Vec::new();
        for seed in 1..=16u64 {
            let mut rng = seed;
            let shuffled = roses_core::duel::build_campaign_decks(
                &mut rng,
                [&decks[super::PLAYER_DECK].cards, &pegasus.cards],
            );
            let duel_setup = roses_core::duel::DuelSetup {
                terrain: setup.setup.terrain.clone(),
                leaders: [decks[super::PLAYER_DECK].leader_card, pegasus.leader_card],
                decks: shuffled,
                first: 0,
                leader_abilities: [vec![], abilities.clone()],
                controllers: [roses_core::duel::PAD_CONTROLLER, PEGASUS_CONTROL],
            };
            let (mut duel, _) = Duel::new(duel_setup, &setup.cards).unwrap();
            duel.set_destiny_rows(setup.destiny_rows);
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::with_controller(&duel, 1, 0, PEGASUS_CONTROL)),
            ];
            let mut session = DuelSession::new(duel, seats);
            let result = session.run(2_000_000, &setup.cards, &tables);
            let line = match result {
                Ok(_) => format!(
                    "seed {seed}: ended in round {} LP {:?} {:?}",
                    session.duel.round,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp],
                    session.duel.outcome.expect("the duel ended")
                ),
                Err(DuelError::Unsupported(what)) => format!(
                    "seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                    session.duel.round,
                    session.duel.active,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp]
                ),
                Err(e) => panic!("seed {seed}: {e}"),
            };
            eprintln!("{line}");
            summary.push(line);
        }
        assert_eq!(summary.len(), 16);
    }

    const LABYRINTH_DECK: usize = 0x22;
    /// Labyrinth-ruler's control byte (opponent 8, Newcastle).
    const LABYRINTH_CONTROL: u8 = 8;

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_labyrinth_ruler_to_the_end_or_refuse() {
        // The player's starter deck (Weevil's module playing it) against
        // the Labyrinth-ruler's deck 0x22 with his own module (0x131930),
        // both through `DuelSession`, seeds 1..=16. Each duel must end or be
        // refused explicitly (`DuelError::Unsupported`); anything else
        // fails.
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let opponent = &decks[LABYRINTH_DECK];
        let row_at = ABILITY_TABLE_FILE + usize::from(opponent.leader_card) * 40;
        let mut row = [0u16; 20];
        for (i, h) in row.iter_mut().enumerate() {
            *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
        }
        let abilities =
            roses_core::duel::queries::leader_abilities(&row, i32::from(opponent.leader_rank_raw));
        assert_eq!(abilities, [0, 2, 3]);
        let mut summary = Vec::new();
        for seed in 1..=16u64 {
            let mut rng = seed;
            let shuffled = roses_core::duel::build_campaign_decks(
                &mut rng,
                [&decks[super::PLAYER_DECK].cards, &opponent.cards],
            );
            let duel_setup = roses_core::duel::DuelSetup {
                terrain: setup.setup.terrain.clone(),
                leaders: [decks[super::PLAYER_DECK].leader_card, opponent.leader_card],
                decks: shuffled,
                first: 0,
                leader_abilities: [vec![], abilities.clone()],
                controllers: [roses_core::duel::PAD_CONTROLLER, LABYRINTH_CONTROL],
            };
            let (mut duel, _) = Duel::new(duel_setup, &setup.cards).unwrap();
            duel.set_destiny_rows(setup.destiny_rows);
            duel.seed_rng(rng);
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::with_controller(
                    &duel,
                    1,
                    0,
                    LABYRINTH_CONTROL,
                )),
            ];
            let mut session = DuelSession::new(duel, seats);
            let result = session.run(2_000_000, &setup.cards, &tables);
            let line = match result {
                Ok(_) => format!(
                    "seed {seed}: ended in round {} LP {:?} {:?}",
                    session.duel.round,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp],
                    session.duel.outcome.expect("the duel ended")
                ),
                Err(DuelError::Unsupported(what)) => format!(
                    "seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                    session.duel.round,
                    session.duel.active,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp]
                ),
                Err(e) => panic!("seed {seed}: {e}"),
            };
            eprintln!("{line}");
            summary.push(line);
        }
        assert_eq!(summary.len(), 16);
    }

    /// Keith's control byte (opponent 4, Towton).
    const KEITH_CONTROL: u8 = 4;

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_keith_to_the_end_or_refuse() {
        // The player's starter deck (Weevil's module playing it) against
        // Keith's deck 0x1E with his own module (0x11CED0), both through
        // `DuelSession`, seeds 1..=16. Each duel must end or be refused
        // explicitly (`DuelError::Unsupported`); anything else fails.
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let opponent = &decks[KEITH_DECK];
        let row_at = ABILITY_TABLE_FILE + usize::from(opponent.leader_card) * 40;
        let mut row = [0u16; 20];
        for (i, h) in row.iter_mut().enumerate() {
            *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
        }
        let abilities =
            roses_core::duel::queries::leader_abilities(&row, i32::from(opponent.leader_rank_raw));
        assert_eq!(abilities, [0, 2, 3, 12]);
        let mut summary = Vec::new();
        for seed in 1..=16u64 {
            let mut rng = seed;
            let shuffled = roses_core::duel::build_campaign_decks(
                &mut rng,
                [&decks[super::PLAYER_DECK].cards, &opponent.cards],
            );
            let duel_setup = roses_core::duel::DuelSetup {
                terrain: setup.setup.terrain.clone(),
                leaders: [decks[super::PLAYER_DECK].leader_card, opponent.leader_card],
                decks: shuffled,
                first: 0,
                leader_abilities: [vec![], abilities.clone()],
                controllers: [roses_core::duel::PAD_CONTROLLER, KEITH_CONTROL],
            };
            let (mut duel, _) = Duel::new(duel_setup, &setup.cards).unwrap();
            duel.set_destiny_rows(setup.destiny_rows);
            duel.seed_rng(rng);
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::with_controller(&duel, 1, 0, KEITH_CONTROL)),
            ];
            let mut session = DuelSession::new(duel, seats);
            let result = session.run(2_000_000, &setup.cards, &tables);
            let line = match result {
                Ok(_) => format!(
                    "seed {seed}: ended in round {} LP {:?} {:?}",
                    session.duel.round,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp],
                    session.duel.outcome.expect("the duel ended")
                ),
                Err(DuelError::Unsupported(what)) => format!(
                    "seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                    session.duel.round,
                    session.duel.active,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp]
                ),
                Err(e) => panic!("seed {seed}: {e}"),
            };
            eprintln!("{line}");
            summary.push(line);
        }
        assert_eq!(summary.len(), 16);
    }

    /// Darkness-ruler's control byte (opponent 7, St.Albans).
    const DARKNESS_CONTROL: u8 = 7;

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_darkness_ruler_to_the_end_or_refuse() {
        // The player's starter deck (Weevil's module playing it) against
        // the Darkness-ruler's deck 0x21 with his own module (0x12C440),
        // both through `DuelSession`, seeds 1..=16. Each duel must end or be
        // refused explicitly (`DuelError::Unsupported`); anything else
        // fails.
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let opponent = &decks[DARKNESS_DECK];
        let row_at = ABILITY_TABLE_FILE + usize::from(opponent.leader_card) * 40;
        let mut row = [0u16; 20];
        for (i, h) in row.iter_mut().enumerate() {
            *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
        }
        let abilities =
            roses_core::duel::queries::leader_abilities(&row, i32::from(opponent.leader_rank_raw));
        assert_eq!(abilities, [2, 3, 10]);
        let mut summary = Vec::new();
        for seed in 1..=16u64 {
            let mut rng = seed;
            let shuffled = roses_core::duel::build_campaign_decks(
                &mut rng,
                [&decks[super::PLAYER_DECK].cards, &opponent.cards],
            );
            let duel_setup = roses_core::duel::DuelSetup {
                terrain: setup.setup.terrain.clone(),
                leaders: [decks[super::PLAYER_DECK].leader_card, opponent.leader_card],
                decks: shuffled,
                first: 0,
                leader_abilities: [vec![], abilities.clone()],
                controllers: [roses_core::duel::PAD_CONTROLLER, DARKNESS_CONTROL],
            };
            let (mut duel, _) = Duel::new(duel_setup, &setup.cards).unwrap();
            duel.set_destiny_rows(setup.destiny_rows);
            duel.seed_rng(rng);
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::with_controller(&duel, 1, 0, DARKNESS_CONTROL)),
            ];
            let mut session = DuelSession::new(duel, seats);
            let result = session.run(2_000_000, &setup.cards, &tables);
            let line = match result {
                Ok(_) => format!(
                    "seed {seed}: ended in round {} LP {:?} {:?}",
                    session.duel.round,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp],
                    session.duel.outcome.expect("the duel ended")
                ),
                Err(DuelError::Unsupported(what)) => format!(
                    "seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                    session.duel.round,
                    session.duel.active,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp]
                ),
                Err(e) => panic!("seed {seed}: {e}"),
            };
            eprintln!("{line}");
            summary.push(line);
        }
        assert_eq!(summary.len(), 16);
    }
    const KEITH_DECK: usize = 0x1E;
    const DARKNESS_DECK: usize = 0x21;

    /// Rex Raptor's control byte (opponent 3, Tewkesbury) and deck.
    const REX_CONTROL: u8 = 3;
    const REX_DECK: usize = 0x1D;

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_rex_raptor_to_the_end_or_refuse() {
        // The player's starter deck (Weevil's module playing it) against
        // Rex Raptor's deck 0x1D with his own module (0x118730), both
        // through `DuelSession`, seeds 1..=16. Each duel must end or be
        // refused explicitly (`DuelError::Unsupported`); anything else fails.
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let opponent = &decks[REX_DECK];
        let row_at = ABILITY_TABLE_FILE + usize::from(opponent.leader_card) * 40;
        let mut row = [0u16; 20];
        for (i, h) in row.iter_mut().enumerate() {
            *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
        }
        let abilities =
            roses_core::duel::queries::leader_abilities(&row, i32::from(opponent.leader_rank_raw));
        assert_eq!(abilities, [0, 1, 3, 12]);
        let mut summary = Vec::new();
        for seed in 1..=16u64 {
            let mut rng = seed;
            let shuffled = roses_core::duel::build_campaign_decks(
                &mut rng,
                [&decks[super::PLAYER_DECK].cards, &opponent.cards],
            );
            let duel_setup = roses_core::duel::DuelSetup {
                terrain: setup.setup.terrain.clone(),
                leaders: [decks[super::PLAYER_DECK].leader_card, opponent.leader_card],
                decks: shuffled,
                first: 0,
                leader_abilities: [vec![], abilities.clone()],
                controllers: [roses_core::duel::PAD_CONTROLLER, REX_CONTROL],
            };
            let (mut duel, _) = Duel::new(duel_setup, &setup.cards).unwrap();
            duel.set_destiny_rows(setup.destiny_rows);
            duel.seed_rng(rng);
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::with_controller(&duel, 1, 0, REX_CONTROL)),
            ];
            let mut session = DuelSession::new(duel, seats);
            let result = session.run(2_000_000, &setup.cards, &tables);
            let line = match result {
                Ok(_) => format!(
                    "seed {seed}: ended in round {} LP {:?} {:?}",
                    session.duel.round,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp],
                    session.duel.outcome.expect("the duel ended")
                ),
                Err(DuelError::Unsupported(what)) => format!(
                    "seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                    session.duel.round,
                    session.duel.active,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp]
                ),
                Err(e) => panic!("seed {seed}: {e}"),
            };
            eprintln!("{line}");
            summary.push(line);
        }
        assert_eq!(summary.len(), 16);
    }

    /// Necromancer's control byte (opponent 6, Exeter) and deck.
    const NECROMANCER_CONTROL: u8 = 6;
    const NECROMANCER_DECK: usize = 0x20;

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_necromancer_to_the_end_or_refuse() {
        // The player's starter deck (Weevil's module playing it) against
        // Necromancer's deck 0x20 with his own module (0x127550), both
        // through `DuelSession`, seeds 1..=16. Each duel must end or be
        // refused explicitly (`DuelError::Unsupported`); anything else fails.
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let opponent = &decks[NECROMANCER_DECK];
        let row_at = ABILITY_TABLE_FILE + usize::from(opponent.leader_card) * 40;
        let mut row = [0u16; 20];
        for (i, h) in row.iter_mut().enumerate() {
            *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
        }
        let abilities =
            roses_core::duel::queries::leader_abilities(&row, i32::from(opponent.leader_rank_raw));
        assert_eq!(abilities, [1, 2, 3, 9, 10]);
        let mut summary = Vec::new();
        for seed in 1..=16u64 {
            let mut rng = seed;
            let shuffled = roses_core::duel::build_campaign_decks(
                &mut rng,
                [&decks[super::PLAYER_DECK].cards, &opponent.cards],
            );
            let duel_setup = roses_core::duel::DuelSetup {
                terrain: setup.setup.terrain.clone(),
                leaders: [decks[super::PLAYER_DECK].leader_card, opponent.leader_card],
                decks: shuffled,
                first: 0,
                leader_abilities: [vec![], abilities.clone()],
                controllers: [roses_core::duel::PAD_CONTROLLER, NECROMANCER_CONTROL],
            };
            let (mut duel, _) = Duel::new(duel_setup, &setup.cards).unwrap();
            duel.set_destiny_rows(setup.destiny_rows);
            duel.seed_rng(rng);
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::with_controller(
                    &duel,
                    1,
                    0,
                    NECROMANCER_CONTROL,
                )),
            ];
            let mut session = DuelSession::new(duel, seats);
            let result = session.run(2_000_000, &setup.cards, &tables);
            let line = match result {
                Ok(_) => format!(
                    "seed {seed}: ended in round {} LP {:?} {:?}",
                    session.duel.round,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp],
                    session.duel.outcome.expect("the duel ended")
                ),
                Err(DuelError::Unsupported(what)) => format!(
                    "seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                    session.duel.round,
                    session.duel.active,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp]
                ),
                Err(e) => panic!("seed {seed}: {e}"),
            };
            eprintln!("{line}");
            summary.push(line);
        }
        assert_eq!(summary.len(), 16);
    }

    /// Ishtar's control byte (opponent 5, Isle of Man) and deck.
    const ISHTAR_CONTROL: u8 = 5;
    const ISHTAR_DECK: usize = 0x1F;

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_ishtar_to_the_end_or_refuse() {
        // The player's starter deck (Weevil's module playing it) against
        // Ishtar's deck 0x1F with her own module (0x1220D0), both through
        // `DuelSession`, seeds 1..=16. Each duel must end or be refused
        // explicitly (`DuelError::Unsupported`); anything else fails.
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let opponent = &decks[ISHTAR_DECK];
        let row_at = ABILITY_TABLE_FILE + usize::from(opponent.leader_card) * 40;
        let mut row = [0u16; 20];
        for (i, h) in row.iter_mut().enumerate() {
            *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
        }
        let abilities =
            roses_core::duel::queries::leader_abilities(&row, i32::from(opponent.leader_rank_raw));
        assert_eq!(abilities, [0, 2, 3, 10]);
        let mut summary = Vec::new();
        for seed in 1..=16u64 {
            let mut rng = seed;
            let shuffled = roses_core::duel::build_campaign_decks(
                &mut rng,
                [&decks[super::PLAYER_DECK].cards, &opponent.cards],
            );
            let duel_setup = roses_core::duel::DuelSetup {
                terrain: setup.setup.terrain.clone(),
                leaders: [decks[super::PLAYER_DECK].leader_card, opponent.leader_card],
                decks: shuffled,
                first: 0,
                leader_abilities: [vec![], abilities.clone()],
                controllers: [roses_core::duel::PAD_CONTROLLER, ISHTAR_CONTROL],
            };
            let (mut duel, _) = Duel::new(duel_setup, &setup.cards).unwrap();
            duel.set_destiny_rows(setup.destiny_rows);
            duel.seed_rng(rng);
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::with_controller(&duel, 1, 0, ISHTAR_CONTROL)),
            ];
            let mut session = DuelSession::new(duel, seats);
            let result = session.run(2_000_000, &setup.cards, &tables);
            let line = match result {
                Ok(_) => format!(
                    "seed {seed}: ended in round {} LP {:?} {:?}",
                    session.duel.round,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp],
                    session.duel.outcome.expect("the duel ended")
                ),
                Err(DuelError::Unsupported(what)) => format!(
                    "seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                    session.duel.round,
                    session.duel.active,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp]
                ),
                Err(e) => panic!("seed {seed}: {e}"),
            };
            eprintln!("{line}");
            summary.push(line);
        }
        assert_eq!(summary.len(), 16);
    }

    /// Manawyddan fab Llyr's control byte (opponent 20, Stonehenge, the red
    /// route's last duel) and deck.
    const MANAWYDDAN_CONTROL: u8 = 20;
    const MANAWYDDAN_DECK: usize = 0x2E;

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_manawyddan_to_the_end_or_refuse() {
        // The player's starter deck (Weevil's module playing it) against
        // Manawyddan fab Llyr's deck 0x2E with his own module (0x163A00),
        // both through `DuelSession`, seeds 1..=16. Each duel must end or
        // be refused explicitly (`DuelError::Unsupported`); anything else
        // fails. Skull Knight at rank 12 has the abilities [0, 2, 3, 9, 10,
        // 14] (row 0x2C26B8 + 75·40, the Dark Magician pattern).
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let opponent = &decks[MANAWYDDAN_DECK];
        let row_at = ABILITY_TABLE_FILE + usize::from(opponent.leader_card) * 40;
        let mut row = [0u16; 20];
        for (i, h) in row.iter_mut().enumerate() {
            *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
        }
        let abilities =
            roses_core::duel::queries::leader_abilities(&row, i32::from(opponent.leader_rank_raw));
        assert_eq!(abilities, [0, 2, 3, 9, 10, 14]);
        let mut summary = Vec::new();
        for seed in 1..=16u64 {
            let mut rng = seed;
            let shuffled = roses_core::duel::build_campaign_decks(
                &mut rng,
                [&decks[super::PLAYER_DECK].cards, &opponent.cards],
            );
            let duel_setup = roses_core::duel::DuelSetup {
                terrain: setup.setup.terrain.clone(),
                leaders: [decks[super::PLAYER_DECK].leader_card, opponent.leader_card],
                decks: shuffled,
                first: 0,
                leader_abilities: [vec![], abilities.clone()],
                controllers: [roses_core::duel::PAD_CONTROLLER, MANAWYDDAN_CONTROL],
            };
            let (mut duel, _) = Duel::new(duel_setup, &setup.cards).unwrap();
            duel.set_destiny_rows(setup.destiny_rows);
            duel.seed_rng(rng);
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::with_controller(
                    &duel,
                    1,
                    0,
                    MANAWYDDAN_CONTROL,
                )),
            ];
            let mut session = DuelSession::new(duel, seats);
            let result = session.run(2_000_000, &setup.cards, &tables);
            let line = match result {
                Ok(_) => format!(
                    "seed {seed}: ended in round {} LP {:?} {:?}",
                    session.duel.round,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp],
                    session.duel.outcome.expect("the duel ended")
                ),
                Err(DuelError::Unsupported(what)) => format!(
                    "seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                    session.duel.round,
                    session.duel.active,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp]
                ),
                Err(e) => panic!("seed {seed}: {e}"),
            };
            eprintln!("{line}");
            summary.push(line);
        }
        assert_eq!(summary.len(), 16);
    }

    /// Richard Slysheen of York's control byte (opponent 10, Bosworth) and deck.
    const YORK_CONTROL: u8 = 10;
    const YORK_DECK: usize = 0x24;

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_york_to_the_end_or_refuse() {
        // The player's starter deck (Weevil's module playing it) against
        // Richard Slysheen of York's deck 0x24 with his own module (0x13A160),
        // both through `DuelSession`, seeds 1..=16. Each duel must end or be refused
        // explicitly (`DuelError::Unsupported`); anything else fails.
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let opponent = &decks[YORK_DECK];
        let row_at = ABILITY_TABLE_FILE + usize::from(opponent.leader_card) * 40;
        let mut row = [0u16; 20];
        for (i, h) in row.iter_mut().enumerate() {
            *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
        }
        let abilities =
            roses_core::duel::queries::leader_abilities(&row, i32::from(opponent.leader_rank_raw));
        assert_eq!(abilities, [0, 1, 2, 3, 8, 9, 12]);
        let mut summary = Vec::new();
        for seed in 1..=16u64 {
            let mut rng = seed;
            let shuffled = roses_core::duel::build_campaign_decks(
                &mut rng,
                [&decks[super::PLAYER_DECK].cards, &opponent.cards],
            );
            let duel_setup = roses_core::duel::DuelSetup {
                terrain: setup.setup.terrain.clone(),
                leaders: [decks[super::PLAYER_DECK].leader_card, opponent.leader_card],
                decks: shuffled,
                first: 0,
                leader_abilities: [vec![], abilities.clone()],
                controllers: [roses_core::duel::PAD_CONTROLLER, YORK_CONTROL],
            };
            let (mut duel, _) = Duel::new(duel_setup, &setup.cards).unwrap();
            duel.set_destiny_rows(setup.destiny_rows);
            duel.seed_rng(rng);
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::with_controller(&duel, 1, 0, YORK_CONTROL)),
            ];
            let mut session = DuelSession::new(duel, seats);
            let result = session.run(2_000_000, &setup.cards, &tables);
            let line = match result {
                Ok(_) => format!(
                    "seed {seed}: ended in round {} LP {:?} {:?}",
                    session.duel.round,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp],
                    session.duel.outcome.expect("the duel ended")
                ),
                Err(DuelError::Unsupported(what)) => format!(
                    "seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                    session.duel.round,
                    session.duel.active,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp]
                ),
                Err(e) => panic!("seed {seed}: {e}"),
            };
            eprintln!("{line}");
            summary.push(line);
        }
        assert_eq!(summary.len(), 16);
    }

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_seto_to_the_end_or_refuse() {
        // Seto's own controller (PAL 0x113460) and deck record 0x1B,
        // against the starter deck driven by Weevil's module. Stonehenge
        // Seto's 0x232E40 setup uses opponent/board 1 (save+C4), so read
        // the original board 1 from the executable as the client does.
        // Every seed must end or report an explicit Unsupported boundary.
        const SETO_CONTROL: u8 = 1;
        const SETO_DECK: usize = 0x1B;
        const SETO_BOARD: usize = 1;
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let terrain = roses_assets::parse_pal_rules(&elf).unwrap().maps[SETO_BOARD]
            .cells
            .clone();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let opponent = &decks[SETO_DECK];
        let row_at = ABILITY_TABLE_FILE + usize::from(opponent.leader_card) * 40;
        let mut row = [0u16; 20];
        for (i, h) in row.iter_mut().enumerate() {
            *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
        }
        let abilities =
            roses_core::duel::queries::leader_abilities(&row, i32::from(opponent.leader_rank_raw));
        let (mut ended, mut refused) = (0, 0);
        for seed in 1..=16u64 {
            let mut rng = seed;
            let shuffled = roses_core::duel::build_campaign_decks(
                &mut rng,
                [&decks[super::PLAYER_DECK].cards, &opponent.cards],
            );
            let duel_setup = roses_core::duel::DuelSetup {
                terrain: terrain.clone(),
                leaders: [decks[super::PLAYER_DECK].leader_card, opponent.leader_card],
                decks: shuffled,
                first: 0,
                leader_abilities: [vec![], abilities.clone()],
                controllers: [roses_core::duel::PAD_CONTROLLER, SETO_CONTROL],
            };
            let (mut duel, _) = Duel::new(duel_setup, &setup.cards).unwrap();
            duel.set_destiny_rows(setup.destiny_rows);
            duel.seed_rng(rng);
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::with_controller(&duel, 1, 0, SETO_CONTROL)),
            ];
            let mut session = DuelSession::new(duel, seats);
            match session.run(2_000_000, &setup.cards, &tables) {
                Ok(_) => {
                    ended += 1;
                    eprintln!(
                        "Seto seed {seed}: ended in round {} LP {:?} {:?}",
                        session.duel.round,
                        [session.duel.sides[0].lp, session.duel.sides[1].lp],
                        session.duel.outcome.expect("the duel ended")
                    );
                }
                Err(DuelError::Unsupported(what)) => {
                    refused += 1;
                    eprintln!(
                        "Seto seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                        session.duel.round,
                        session.duel.active,
                        [session.duel.sides[0].lp, session.duel.sides[1].lp]
                    );
                    // Opt-in private evidence: preserve the exact pre-error
                    // session for reproducing the blocked key/command. These
                    // records contain user-imported game state; keep them in
                    // an ignored research directory, never source fixtures.
                    if let Some(dir) = std::env::var_os("ROSES_SETO_SESSION_DUMP") {
                        let dir = std::path::PathBuf::from(dir);
                        std::fs::create_dir_all(&dir).unwrap();
                        let path = dir.join(format!("seed-{seed:02}.json"));
                        let file = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&path)
                            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                        serde_json::to_writer_pretty(
                            file,
                            &serde_json::json!({
                                "schema": "roses-private-seto-refusal-1",
                                "seed": seed,
                                "reason": what,
                                "session": session,
                            }),
                        )
                        .unwrap();
                    }
                }
                Err(e) => panic!("Seto seed {seed}: {e}"),
            }
        }
        eprintln!("Seto sessions: {ended} ended, {refused} explicitly refused");
        assert_eq!(ended + refused, 16);
    }

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_route_3_5_decks_or_refuse() {
        // The red route after Pegasus (claude-effects/next/ROUTE-3-5.md): the
        // player's starter deck against the Labyrinth-ruler (0x22, opponent
        // 8), Keith (0x1E, 4) and Darkness-ruler (0x21, 7) decks. Their
        // controllers are not ported: Weevil's module stands in on seat 1 as
        // a test driver, so only the engine's handling of these decks' cards
        // is exercised. Each duel must end or be refused explicitly.
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let mut summary = Vec::new();
        for (name, index, control, expected) in [
            ("Labyrinth-ruler", LABYRINTH_DECK, 8u8, vec![0u8, 2, 3]),
            ("Keith", KEITH_DECK, 4, vec![0, 2, 3, 12]),
            ("Darkness-ruler", DARKNESS_DECK, 7, vec![2, 3, 10]),
        ] {
            let opponent = &decks[index];
            let row_at = ABILITY_TABLE_FILE + usize::from(opponent.leader_card) * 40;
            let mut row = [0u16; 20];
            for (i, h) in row.iter_mut().enumerate() {
                *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
            }
            let abilities = roses_core::duel::queries::leader_abilities(
                &row,
                i32::from(opponent.leader_rank_raw),
            );
            assert_eq!(abilities, expected, "{name}");
            for seed in 1..=8u64 {
                let mut rng = seed;
                let shuffled = roses_core::duel::build_campaign_decks(
                    &mut rng,
                    [&decks[super::PLAYER_DECK].cards, &opponent.cards],
                );
                let duel_setup = roses_core::duel::DuelSetup {
                    terrain: setup.setup.terrain.clone(),
                    leaders: [decks[super::PLAYER_DECK].leader_card, opponent.leader_card],
                    decks: shuffled,
                    first: 0,
                    leader_abilities: [vec![], abilities.clone()],
                    controllers: [roses_core::duel::PAD_CONTROLLER, control],
                };
                let (mut duel, _) = Duel::new(duel_setup, &setup.cards).unwrap();
            duel.set_destiny_rows(setup.destiny_rows);
                duel.seed_rng(rng);
                let seats = [
                    Some(ComputerSeat::new(&duel, 0, 0)),
                    Some(ComputerSeat::new(&duel, 1, 0)),
                ];
                let mut session = DuelSession::new(duel, seats);
                let result = session.run(2_000_000, &setup.cards, &tables);
                let line = match result {
                    Ok(_) => format!(
                        "{name} seed {seed}: ended in round {} LP {:?} {:?}",
                        session.duel.round,
                        [session.duel.sides[0].lp, session.duel.sides[1].lp],
                        session.duel.outcome.expect("the duel ended")
                    ),
                    Err(DuelError::Unsupported(what)) => format!(
                        "{name} seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                        session.duel.round,
                        session.duel.active,
                        [session.duel.sides[0].lp, session.duel.sides[1].lp]
                    ),
                    Err(e) => panic!("{name} seed {seed}: {e}"),
                };
                eprintln!("{line}");
                summary.push(line);
            }
        }
        assert_eq!(summary.len(), 24);
    }

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_the_first_duel_to_the_end_or_refuse() {
        // Weevil's module on both seats for seeds 1..=16, like the Pegasus
        // run above: each duel ends or is refused explicitly.
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let mut summary = Vec::new();
        for seed in 1..=16u64 {
            let setup = super::prepare(&elf, Vec::new(), seed).unwrap();
            let (duel, _) = Duel::new(setup.setup, &setup.cards).unwrap();
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::new(&duel, 1, 0)),
            ];
            let mut session = DuelSession::new(duel, seats);
            let line = match session.run(2_000_000, &setup.cards, &tables) {
                Ok(_) => format!(
                    "seed {seed}: ended in round {} LP {:?} {:?}",
                    session.duel.round,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp],
                    session.duel.outcome.expect("the duel ended")
                ),
                Err(DuelError::Unsupported(what)) => format!(
                    "seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                    session.duel.round,
                    session.duel.active,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp]
                ),
                Err(e) => panic!("seed {seed}: {e}"),
            };
            eprintln!("{line}");
            summary.push(line);
        }
        assert_eq!(summary.len(), 16);
    }

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_the_first_duel_to_its_end() {
        // Both sides played by the ported controller through `DuelSession`
        // (plans -> keys -> pad input -> engine), as `roses duel --player
        // computer` does. Seeds 5 and 20 end on LP in the terminal harness.
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        for (seed, winner) in [(5u64, 1u8), (20, 0)] {
            let setup = super::prepare(&elf, Vec::new(), seed).unwrap();
            let (duel, _) = Duel::new(setup.setup, &setup.cards).unwrap();
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::new(&duel, 1, 0)),
            ];
            let mut session = DuelSession::new(duel, seats);
            match session.run(2_000_000, &setup.cards, &tables) {
                Ok(_) => (),
                Err(DuelError::Unsupported(what)) => panic!("seed {seed}: {what}"),
                Err(e) => panic!("seed {seed}: {e}"),
            }
            let outcome = session.duel.outcome.expect("the duel ended");
            eprintln!(
                "seed {seed}: round {} LP {:?} {outcome:?}",
                session.duel.round,
                [session.duel.sides[0].lp, session.duel.sides[1].lp]
            );
            assert_eq!(outcome.winner, Some(winner), "seed {seed}");
        }
    }
    /// The white (Yorkist) route's duel against computer opponent `control`
    /// (deck record 0x1A + control, board `control`, 0x232E40's white
    /// branch: the opponent on side 0, the player on side 1 moving first),
    /// the player's starter deck driven by Weevil's module, seeds 1..=n;
    /// `swap` also plays the opponent on side 1 (the red layout) for as many
    /// seeds. Every duel must end or be refused explicitly. Returns the
    /// summary lines.
    fn white_route_sessions(control: u8, seeds: u64, swap: bool) -> Vec<String> {
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let terrain = roses_assets::parse_pal_rules(&elf).unwrap().maps[usize::from(control)]
            .cells
            .clone();
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let opponent = &decks[0x1A + usize::from(control)];
        let player = &decks[super::PLAYER_DECK];
        let row_at = ABILITY_TABLE_FILE + usize::from(opponent.leader_card) * 40;
        let mut row = [0u16; 20];
        for (i, h) in row.iter_mut().enumerate() {
            *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
        }
        let abilities =
            roses_core::duel::queries::leader_abilities(&row, i32::from(opponent.leader_rank_raw));
        let mut summary = Vec::new();
        let layouts: &[u8] = if swap { &[0, 1] } else { &[0] };
        for &cpu in layouts {
            for seed in 1..=seeds {
                let mut rng = seed;
                let (decks_in, leaders, abil, controllers) = if cpu == 0 {
                    (
                        roses_core::duel::build_campaign_decks(
                            &mut rng,
                            [&opponent.cards, &player.cards],
                        ),
                        [opponent.leader_card, player.leader_card],
                        [abilities.clone(), vec![]],
                        [control, roses_core::duel::PAD_CONTROLLER],
                    )
                } else {
                    (
                        roses_core::duel::build_campaign_decks(
                            &mut rng,
                            [&player.cards, &opponent.cards],
                        ),
                        [player.leader_card, opponent.leader_card],
                        [vec![], abilities.clone()],
                        [roses_core::duel::PAD_CONTROLLER, control],
                    )
                };
                let duel_setup = roses_core::duel::DuelSetup {
                    terrain: terrain.clone(),
                    leaders,
                    decks: decks_in,
                    // The player moves first (side 1 in the white layout).
                    first: u8::from(cpu == 0),
                    leader_abilities: abil,
                    controllers,
                };
                let mut duel = match Duel::new(duel_setup, &setup.cards) {
                    Ok((duel, _)) => duel,
                    Err(DuelError::Unsupported(what)) => {
                        let line = format!(
                            "control {control} on side {cpu} seed {seed}: refused at setup: {what}"
                        );
                        eprintln!("{line}");
                        summary.push(line);
                        continue;
                    }
                    Err(e) => panic!("control {control} on side {cpu} seed {seed}: {e}"),
                };
                duel.set_destiny_rows(setup.destiny_rows.clone());
                duel.seed_rng(rng);
                let human = cpu ^ 1;
                let mut seats = [None, None];
                seats[usize::from(human)] = Some(ComputerSeat::new(&duel, human, 0));
                seats[usize::from(cpu)] =
                    Some(ComputerSeat::with_controller(&duel, cpu, 0, control));
                let mut session = DuelSession::new(duel, seats);
                let line = match session.run(2_000_000, &setup.cards, &tables) {
                    Ok(_) => format!(
                        "control {control} on side {cpu} seed {seed}: ended in round {} LP {:?} {:?}",
                        session.duel.round,
                        [session.duel.sides[0].lp, session.duel.sides[1].lp],
                        session.duel.outcome.expect("the duel ended")
                    ),
                    Err(DuelError::Unsupported(what)) => format!(
                        "control {control} on side {cpu} seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                        session.duel.round,
                        session.duel.active,
                        [session.duel.sides[0].lp, session.duel.sides[1].lp]
                    ),
                    Err(e) => panic!("control {control} on side {cpu} seed {seed}: {e}"),
                };
                eprintln!("{line}");
                summary.push(line);
            }
        }
        summary
    }

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_white_route_opponents_or_refuse() {
        // Every ported white-route controller (Tea 11, ...) on its own deck
        // and board; `ROSES_WHITE_CONTROLS` (comma list) narrows the set,
        // `ROSES_WHITE_SEEDS` sets the seed count (16).
        const WHITE: [u8; 10] = [11, 12, 13, 14, 15, 16, 17, 18, 19, 21];
        let only: Option<Vec<u8>> = std::env::var("ROSES_WHITE_CONTROLS")
            .ok()
            .map(|v| v.split(',').map(|c| c.trim().parse().unwrap()).collect());
        let seeds = std::env::var("ROSES_WHITE_SEEDS")
            .ok()
            .map_or(16, |v| v.parse().unwrap());
        let ported = roses_core::duel::ai::weevil::PORTED_CONTROLLERS;
        for control in WHITE {
            if !ported.contains(&control) || only.as_ref().is_some_and(|o| !o.contains(&control)) {
                continue;
            }
            let lines = white_route_sessions(control, seeds, true);
            let ended = lines.iter().filter(|l| l.contains(": ended")).count();
            eprintln!(
                "control {control}: {ended} ended, {} refused of {}",
                lines.len() - ended,
                lines.len()
            );
            assert_eq!(lines.len() as u64, 2 * seeds);
        }
    }

    /// Custom Duel against the computer (title mode 9): control `control`
    /// with deck record `record` on side 1 (the mirror, control 22, plays a
    /// copy of the player's deck), the player's starter deck driven by
    /// Weevil's module on side 0 moving first (param[2] = 0), the 24 default
    /// VS maps 0x16.. in turn, the menu's defaults (LP 4000, SP 3, leader
    /// abilities on: param[0x1C] = 3). Seeds 1..=n; every duel must end or
    /// be refused explicitly. Returns the summary lines.
    fn custom_duel_sessions(control: u8, record: usize, seeds: u64) -> Vec<String> {
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let setup = super::prepare(&elf, Vec::new(), 1).unwrap();
        let maps = roses_assets::parse_pal_rules(&elf).unwrap().maps;
        let tables = roses_core::duel::ai::weevil::Tables::new(
            &crate::duel_catalogue::build(&elf).unwrap().fusions,
            &elf,
        )
        .unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        let player = &decks[super::PLAYER_DECK];
        let opponent = &decks[record];
        let abilities = |card: u16, rank: u8| {
            let row_at = ABILITY_TABLE_FILE + usize::from(card) * 40;
            let mut row = [0u16; 20];
            for (i, h) in row.iter_mut().enumerate() {
                *h = u16::from_le_bytes([elf[row_at + 2 * i], elf[row_at + 2 * i + 1]]);
            }
            roses_core::duel::queries::leader_abilities(&row, i32::from(rank))
        };
        let ranks = [player.leader_rank_raw.min(12), opponent.leader_rank_raw.min(12)];
        let mut summary = Vec::new();
        for seed in 1..=seeds {
            let map = 0x16 + ((seed - 1) % 24) as usize;
            let mut rng = seed;
            let duel_setup = roses_core::duel::DuelSetup {
                terrain: maps[map].cells.clone(),
                leaders: [player.leader_card, opponent.leader_card],
                decks: roses_core::duel::build_campaign_decks(&mut rng, [&player.cards, &opponent.cards]),
                first: 0,
                leader_abilities: [
                    abilities(player.leader_card, ranks[0]),
                    abilities(opponent.leader_card, ranks[1]),
                ],
                controllers: [roses_core::duel::PAD_CONTROLLER, control],
            };
            let mut duel = match Duel::new(duel_setup, &setup.cards) {
                Ok((duel, _)) => duel,
                Err(DuelError::Unsupported(what)) => {
                    let line = format!("control {control} deck {record:#x} map {map} seed {seed}: refused at setup: {what}");
                    eprintln!("{line}");
                    summary.push(line);
                    continue;
                }
                Err(e) => panic!("control {control} seed {seed}: {e}"),
            };
            duel.set_destiny_rows(setup.destiny_rows.clone());
            duel.set_leader_ranks(ranks);
            duel.set_custom_rules([4000, 4000], [3, 3]);
            duel.seed_rng(rng);
            let seats = [
                Some(ComputerSeat::new(&duel, 0, 0)),
                Some(ComputerSeat::with_controller(&duel, 1, 0, control)),
            ];
            let mut session = DuelSession::new(duel, seats);
            let line = match session.run(2_000_000, &setup.cards, &tables) {
                Ok(_) => format!(
                    "control {control} deck {record:#x} map {map} seed {seed}: ended in round {} LP {:?} {:?}",
                    session.duel.round,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp],
                    session.duel.outcome.expect("the duel ended")
                ),
                Err(DuelError::Unsupported(what)) => format!(
                    "control {control} deck {record:#x} map {map} seed {seed}: refused in round {} (side {} to act, LP {:?}): {what}",
                    session.duel.round,
                    session.duel.active,
                    [session.duel.sides[0].lp, session.duel.sides[1].lp]
                ),
                Err(e) => panic!("control {control} seed {seed}: {e}"),
            };
            eprintln!("{line}");
            summary.push(line);
        }
        summary
    }

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH); slow in debug"]
    fn private_computer_sessions_play_custom_duel_or_refuse() {
        // Custom Duel's computer: 22 (the mirror: the player's deck), 25 deck
        // 0x32 (the retail pair), 23 deck 0x30 and 24 deck 0x31 (no retail
        // producer found). `ROSES_CUSTOM_CONTROLS` narrows the set,
        // `ROSES_CUSTOM_SEEDS` sets the seed count (24).
        let only: Option<Vec<u8>> = std::env::var("ROSES_CUSTOM_CONTROLS")
            .ok()
            .map(|v| v.split(',').map(|c| c.trim().parse().unwrap()).collect());
        let seeds = std::env::var("ROSES_CUSTOM_SEEDS").ok().map_or(24, |v| v.parse().unwrap());
        for (control, record) in [(22u8, super::PLAYER_DECK), (25, 0x32), (23, 0x30), (24, 0x31)] {
            if only.as_ref().is_some_and(|o| !o.contains(&control)) {
                continue;
            }
            let lines = custom_duel_sessions(control, record, seeds);
            let ended = lines.iter().filter(|l| l.contains(": ended")).count();
            eprintln!("control {control}: {ended} ended, {} refused of {}", lines.len() - ended, lines.len());
            assert_eq!(lines.len() as u64, seeds);
        }
    }
}
