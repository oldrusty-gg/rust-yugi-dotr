use super::*;

const HOURGLASS: u16 = 1;
const MODIFIER: u16 = 2;

fn p(column: u8, row: u8) -> GridPos {
    GridPos { column, row }
}

fn cards() -> Vec<CardInfo> {
    let mut cards: Vec<_> = (0..5)
        .map(|id| CardInfo {
            id,
            kind: kind::MACHINE,
            kind_index: 0,
            level: 1,
            attribute: 0,
            attack: 1200,
            defense: 100,
            effects: [false; 5],
            toon: false,
            entries: [None; 5],
            equip_bits: [0; 7],
            power_up: None,
        })
        .collect();
    cards[HOURGLASS as usize].entries[4] = Some([0x415D, (600 << 16) | 250]);
    cards[HOURGLASS as usize].effects[4] = true;
    cards[MODIFIER as usize].entries[2] = Some([0x3B2C, 500]);
    cards[MODIFIER as usize].effects[2] = true;
    cards
}

fn board(cards: &Vec<CardInfo>) -> Duel {
    Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [0; 2],
            decks: [vec![], vec![]],
            first: 0,
            leader_abilities: [vec![], vec![]],
            controllers: [PAD_CONTROLLER; 2],
        },
        cards,
    )
    .unwrap()
    .0
}

fn put(d: &mut Duel, cards: &Vec<CardInfo>, side: u8, card: u16, at: GridPos) -> UnitId {
    d.create(side, card, at, cards, &mut vec![]).unwrap()
}

fn attack(d: &Duel, cards: &Vec<CardInfo>, attacker: UnitId) -> (Duel, Vec<DuelEvent>) {
    d.apply(
        0,
        &DuelCommand::Move {
            unit: attacker,
            path: vec![p(3, 3)],
            face_up: true,
            defence: None,
        },
        cards,
    )
    .unwrap()
}

fn setup(cards: &Vec<CardInfo>, lp: u16) -> (Duel, UnitId, UnitId, UnitId) {
    let mut d = board(cards);
    let attacker = put(&mut d, cards, 0, 0, p(3, 4));
    let dying = put(&mut d, cards, 1, HOURGLASS, p(3, 3));
    let ally = put(&mut d, cards, 1, 0, p(5, 2));
    let u = d.units.iter_mut().find(|u| u.id == dying).unwrap();
    u.defence = true;
    u.committed_defence = true;
    d.sides[1].lp = lp;
    (d, attacker, dying, ally)
}

#[test]
fn battle_death_pays_cost_then_boosts_allied_set_monster_and_advances() {
    let cards = cards();
    let (d, attacker, dying, ally) = setup(&cards, 900);
    let old_ally = d.unit(ally).unwrap().clone();
    let (after, events) = attack(&d, &cards, attacker);
    assert!(after.unit(dying).is_none());
    assert_eq!(after.sides[1].graveyard, vec![HOURGLASS]);
    assert_eq!(after.sides[1].lp, 650);
    assert_eq!(after.unit(attacker).unwrap().position, p(3, 3));
    let mut expected = old_ally;
    expected.attack_bonus += 600;
    expected.defense_bonus += 600;
    assert_eq!(after.unit(ally).unwrap(), &expected);
    let cost = events
        .iter()
        .position(|e| {
            matches!(
                e,
                DuelEvent::Damage {
                    side: 1,
                    amount: 250
                }
            )
        })
        .unwrap();
    let bonus = events
        .iter()
        .position(|e| matches!(e, DuelEvent::Modified { unit, .. } if *unit == ally))
        .unwrap();
    let death = events
        .iter()
        .position(|e| matches!(e, DuelEvent::Destroyed { unit, .. } if *unit == dying))
        .unwrap();
    assert!(cost < bonus && bonus < death);
}

#[test]
fn threshold_is_strict_and_no_other_monster_means_no_payment() {
    let cards = cards();
    for lp in [599, 600] {
        let (d, attacker, _, ally) = setup(&cards, lp);
        let (after, _) = attack(&d, &cards, attacker);
        assert_eq!(after.sides[1].lp, lp);
        assert_eq!(after.unit(ally).unwrap().attack_bonus, 0);
    }
    let (mut d, attacker, _, ally) = setup(&cards, 900);
    d.units.retain(|u| u.id != ally);
    let (after, _) = attack(&d, &cards, attacker);
    assert_eq!(after.sides[1].lp, 900);
}

#[test]
fn modified_cost_can_end_the_duel_but_the_bonus_and_battle_removal_still_happen() {
    let cards = cards();
    let (mut d, attacker, dying, ally) = setup(&cards, 700);
    let modifier = put(&mut d, &cards, 0, MODIFIER, p(1, 3));
    let u = d.units.iter_mut().find(|u| u.id == modifier).unwrap();
    u.face_up = true;
    u.committed_face_up = true;
    u.defence = true;
    u.committed_defence = true;
    let (after, _) = attack(&d, &cards, attacker);
    assert_eq!(after.sides[1].lp, 0);
    assert!(after.unit(dying).is_none());
    assert_eq!(after.unit(ally).unwrap().attack_bonus, 600);
    assert!(after.outcome.is_some());
}

#[test]
fn effect_removal_does_not_fire_battle_destruction_entry() {
    let cards = cards();
    let (mut d, _, dying, ally) = setup(&cards, 900);
    let mut events = vec![];
    d.remove_by_effect(dying, &cards, &mut events).unwrap();
    assert_eq!(d.sides[1].lp, 900);
    assert_eq!(d.unit(ally).unwrap().attack_bonus, 0);
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Activated { .. })));
}

#[test]
fn mutual_destruction_runs_both_effects_before_removing_either_card() {
    let cards = cards();
    let mut d = board(&cards);
    let attacker = put(&mut d, &cards, 0, HOURGLASS, p(3, 4));
    let defender = put(&mut d, &cards, 1, HOURGLASS, p(3, 3));
    let first_ally = put(&mut d, &cards, 0, 0, p(1, 2));
    let second_ally = put(&mut d, &cards, 1, 0, p(5, 2));
    let (after, events) = attack(&d, &cards, attacker);
    assert!(after.unit(attacker).is_none() && after.unit(defender).is_none());
    assert_eq!([after.sides[0].lp, after.sides[1].lp], [START_LP - 250; 2]);
    for ally in [first_ally, second_ally] {
        assert_eq!(after.unit(ally).unwrap().attack_bonus, 600);
    }
    let activations: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            DuelEvent::Activated {
                unit,
                card: HOURGLASS,
                ..
            } => Some(*unit),
            _ => None,
        })
        .collect();
    assert_eq!(activations, [defender, attacker]);
}

#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
struct ObservedUnit {
    side: u8,
    slot: u8,
    card: u16,
    pos: [u8; 2],
    flags: u8,
    bind: u8,
    order: u8,
    attack: i16,
    defense: i16,
}

impl ObservedUnit {
    fn unit(&self) -> DuelUnit {
        DuelUnit {
            id: UnitId(u16::from(self.side) * 11 + u16::from(self.slot)),
            card: self.card,
            side: self.side,
            slot: self.slot,
            position: p(self.pos[0], self.pos[1]),
            face_up: self.flags & 4 != 0,
            defence: self.flags & 8 != 0,
            committed_face_up: self.flags & 16 != 0,
            committed_defence: self.flags & 32 != 0,
            acted: self.flags & 1 != 0,
            stolen: self.flags & 2 != 0,
            spellbind: self.bind,
            spellbind_fresh: false,
            summon_order: self.order,
            attack_bonus: self.attack,
            defense_bonus: self.defense,
            evo_counter: 0,
            discount: 0,
            stolen_by: None,
        }
    }

    fn of(u: &DuelUnit) -> Self {
        Self {
            side: u.side,
            slot: u.slot,
            card: u.card,
            pos: [u.position.column, u.position.row],
            flags: u8::from(u.acted)
                | u8::from(u.stolen) << 1
                | u8::from(u.face_up) << 2
                | u8::from(u.defence) << 3
                | u8::from(u.committed_face_up) << 4
                | u8::from(u.committed_defence) << 5,
            bind: u.spellbind,
            order: u.summon_order,
            attack: u.attack_bonus,
            defense: u.defense_bonus,
        }
    }
}

#[derive(serde::Deserialize)]
struct Probe {
    name: String,
    group: u8,
    slot: u8,
    entry: [u32; 2],
    lp: [u16; 2],
    initial: Vec<ObservedUnit>,
    ret: u32,
    #[serde(rename = "final")]
    expected: Vec<ObservedUnit>,
    final_lp: [u16; 2],
}

#[derive(serde::Deserialize)]
struct Evidence {
    cards: Vec<CardInfo>,
    cases: Vec<Probe>,
}

#[test]
fn private_seto_destruction_instruction_oracle() {
    let Some(path) = std::env::var_os("ROSES_SETO_DESTRUCTION_ORACLE") else {
        return;
    };
    let evidence: Evidence = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(!evidence.cases.is_empty(), "empty destruction oracle");
    let count = evidence.cases.len();
    for probe in evidence.cases {
        let mut d = board(&evidence.cards);
        d.units = probe.initial.iter().map(ObservedUnit::unit).collect();
        for side in 0..2 {
            d.sides[side].lp = probe.lp[side];
        }
        let source = d
            .units
            .iter()
            .find(|u| (u.side, u.slot) == (probe.group, probe.slot))
            .unwrap()
            .clone();
        let mut events = vec![];
        assert!(matches!(
            d.destruction_boost_for_lp(
                &source,
                probe.entry[0],
                probe.entry[1],
                &evidence.cards,
                &mut events
            )
            .unwrap(),
            Aftermath::Continue
        ));
        let mut actual: Vec<_> = d.units.iter().map(ObservedUnit::of).collect();
        actual.sort_by_key(|u| (u.side, u.slot));
        assert_eq!(actual, probe.expected, "{} units", probe.name);
        assert_eq!(
            [d.sides[0].lp, d.sides[1].lp],
            probe.final_lp,
            "{} LP",
            probe.name
        );
        assert_eq!(
            events
                .iter()
                .any(|e| matches!(e, DuelEvent::Activated { .. })),
            probe.ret & 1 != 0,
            "{} activation",
            probe.name
        );
    }
    eprintln!("Seto destruction original-instruction oracle: {count} matched, 0 differ");
}
