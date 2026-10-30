use super::*;

const COPY: u16 = 10;
const SLEEP: u16 = 11;
const PLAIN: u16 = 12;
const SET_CARD: u16 = 13;

fn pos(column: u8, row: u8) -> GridPos {
    GridPos { column, row }
}

fn cards() -> Vec<CardInfo> {
    let mut cards: Vec<_> = (0..40)
        .map(|id| CardInfo {
            id,
            kind: kind::MACHINE,
            level: 1,
            attribute: 0,
            attack: 500,
            defense: 400,
            effects: [false; 5],
            toon: false,
            kind_index: 0,
            entries: [None; 5],
            equip_bits: [0; 7],
            power_up: None,
        })
        .collect();
    cards[COPY as usize].entries[3] = Some([0x385C, 0x1F0000 | u32::from(COPY)]);
    cards[COPY as usize].effects[3] = true;
    cards[COPY as usize].attack = 100;
    cards[COPY as usize].defense = 100;
    cards[SLEEP as usize].entries[3] = Some([0x179F, 1]);
    cards[SLEEP as usize].effects[3] = true;
    cards[SET_CARD as usize].kind = class::MAGIC << 5;
    cards
}

fn board(cards: &Vec<CardInfo>) -> Duel {
    Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [0, 1],
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

fn add(d: &mut Duel, side: u8, card: u16, at: GridPos, cards: &Vec<CardInfo>) -> UnitId {
    d.create(side, card, at, cards, &mut vec![]).unwrap()
}

fn reveal(d: &Duel, id: UnitId, cards: &Vec<CardInfo>) -> (Duel, Vec<DuelEvent>) {
    d.apply(
        0,
        &DuelCommand::Move {
            unit: id,
            path: vec![],
            face_up: true,
            defence: None,
        },
        cards,
    )
    .unwrap()
}

#[test]
fn manual_copy_creates_one_fresh_face_up_unit_without_repeating_flip() {
    let cards = cards();
    let mut d = board(&cards);
    d.terrain[pos(3, 5).index().unwrap()] = terrain::LABYRINTH;
    let source = add(&mut d, 0, COPY, pos(4, 4), &cards);
    let order = d.sides[0].summons;
    let (d, events) = reveal(&d, source, &cards);
    let copies: Vec<_> = d.units.iter().filter(|u| u.card == COPY).collect();
    assert_eq!(copies.len(), 2);
    let copy = copies.into_iter().find(|u| u.id != source).unwrap();
    assert_eq!(copy.position, pos(4, 6));
    assert_eq!(copy.summon_order, order);
    assert!(copy.face_up && copy.committed_face_up);
    assert!(!copy.acted && !copy.defence && !copy.committed_defence);
    assert_eq!(copy.spellbind, 0);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, DuelEvent::Created { .. }))
            .count(),
        1
    );
}

#[test]
fn full_monster_slots_or_surrounded_leader_prevent_flip_summon() {
    let cards = cards();
    for full_slots in [false, true] {
        let mut d = board(&cards);
        let source = add(&mut d, 0, COPY, pos(4, 4), &cards);
        if full_slots {
            for c in 0..4 {
                add(&mut d, 0, PLAIN, pos(c, 2), &cards);
            }
        } else {
            for at in [pos(3, 5), pos(4, 6), pos(2, 6), pos(4, 5), pos(2, 5)] {
                d.terrain[at.index().unwrap()] = terrain::LABYRINTH;
            }
        }
        let (after, events) = reveal(&d, source, &cards);
        assert_eq!(after.units.len(), d.units.len());
        assert!(!events
            .iter()
            .any(|e| matches!(e, DuelEvent::Activated { .. })));
    }
}

#[test]
fn stolen_copy_uses_current_controllers_leader_and_capacity() {
    let cards = cards();
    let mut d = board(&cards);
    let source = add(&mut d, 1, COPY, pos(4, 4), &cards);
    d.units.iter_mut().find(|u| u.id == source).unwrap().stolen = true;
    for c in 0..4 {
        add(&mut d, 1, PLAIN, pos(c, 2), &cards);
    }
    let (d, _) = reveal(&d, source, &cards);
    let copy = d
        .units
        .iter()
        .find(|u| u.card == COPY && u.id != source)
        .unwrap();
    assert_eq!((copy.side, copy.position), (0, pos(3, 5)));
    assert!(!copy.stolen);
}

#[test]
fn first_free_cell_zero_prevents_copy_without_trying_later_cells() {
    let cards = cards();
    let mut d = board(&cards);
    d.units
        .iter_mut()
        .find(|u| u.side == 0 && u.slot == 0)
        .unwrap()
        .position = pos(0, 1);
    let source = add(&mut d, 0, COPY, pos(4, 4), &cards);
    let (d, events) = reveal(&d, source, &cards);
    assert_eq!(d.units.len(), 3);
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Created { .. })));
}

#[test]
fn battle_copy_exists_after_original_is_destroyed() {
    let cards = cards();
    let mut d = board(&cards);
    let attacker = add(&mut d, 0, PLAIN, pos(4, 3), &cards);
    let defender = add(&mut d, 1, COPY, pos(4, 4), &cards);
    let (d, _) = d
        .apply(
            0,
            &DuelCommand::Move {
                unit: attacker,
                path: vec![pos(4, 4)],
                face_up: true,
                defence: None,
            },
            &cards,
        )
        .unwrap();
    assert!(d.unit(defender).is_none());
    assert_eq!(d.sides[1].graveyard, vec![COPY]);
    let copy = d.units.iter().find(|u| u.card == COPY).unwrap();
    assert_eq!((copy.side, copy.position), (1, pos(3, 1)));
    assert!(copy.face_up && copy.committed_face_up);
}

#[test]
fn sleep_binds_set_cards_and_monsters_preserving_longer_binds() {
    let cards = cards();
    let mut d = board(&cards);
    let source = add(&mut d, 0, SLEEP, pos(4, 4), &cards);
    let own = add(&mut d, 0, PLAIN, pos(0, 2), &cards);
    let enemy = add(&mut d, 1, PLAIN, pos(1, 2), &cards);
    let set = add(&mut d, 1, SET_CARD, pos(2, 2), &cards);
    let bound = add(&mut d, 1, PLAIN, pos(3, 2), &cards);
    d.units
        .iter_mut()
        .find(|u| u.id == bound)
        .unwrap()
        .spellbind = 3;
    let (d, events) = reveal(&d, source, &cards);
    for target in [enemy, set] {
        let u = d.unit(target).unwrap();
        assert_eq!(u.spellbind, 1);
        assert!(u.acted && u.spellbind_fresh);
        assert!(!u.face_up);
    }
    let u = d.unit(bound).unwrap();
    assert_eq!(u.spellbind, 3);
    assert!(u.acted && !u.spellbind_fresh);
    assert_eq!(d.unit(own).unwrap().spellbind, 0);
    assert!(d
        .units
        .iter()
        .filter(|u| u.slot == 0)
        .all(|u| u.spellbind == 0));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, DuelEvent::Spellbound { .. }))
            .count(),
        2
    );
}

#[test]
fn sleep_defender_binds_attacker_before_battle_and_prevents_damage() {
    let cards = cards();
    let mut d = board(&cards);
    let attacker = add(&mut d, 0, PLAIN, pos(4, 3), &cards);
    let defender = add(&mut d, 1, SLEEP, pos(4, 4), &cards);
    let (d, _) = d
        .apply(
            0,
            &DuelCommand::Move {
                unit: attacker,
                path: vec![pos(4, 4)],
                face_up: true,
                defence: None,
            },
            &cards,
        )
        .unwrap();
    assert_eq!(d.unit(attacker).unwrap().position, pos(4, 3));
    assert_eq!(d.unit(attacker).unwrap().spellbind, 1);
    assert!(d.unit(defender).is_some());
    assert_eq!([d.sides[0].lp, d.sides[1].lp], [START_LP; 2]);
}

#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
struct ObservedUnit {
    side: u8,
    slot: u8,
    card: u16,
    pos: [u8; 2],
    flags: u8,
    bind: u8,
    fresh: bool,
    order: u8,
}

impl ObservedUnit {
    fn unit(&self) -> DuelUnit {
        DuelUnit {
            id: UnitId(u16::from(self.side) * 11 + u16::from(self.slot)),
            card: self.card,
            side: self.side,
            slot: self.slot,
            position: pos(self.pos[0], self.pos[1]),
            face_up: self.flags & 4 != 0,
            defence: self.flags & 8 != 0,
            committed_face_up: self.flags & 0x10 != 0,
            committed_defence: self.flags & 0x20 != 0,
            acted: self.flags & 1 != 0,
            spellbind: self.bind,
            summon_order: self.order,
            attack_bonus: 0,
            defense_bonus: 0,
            spellbind_fresh: self.fresh,
            evo_counter: 0,
            stolen: self.flags & 2 != 0,
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
            fresh: u.spellbind_fresh,
            order: u.summon_order,
        }
    }
}

#[derive(serde::Deserialize)]
struct Probe {
    name: String,
    card: u16,
    group: u8,
    initial: Vec<ObservedUnit>,
    terrain: Vec<u8>,
    summons: [u8; 2],
    entry: Option<[u32; 2]>,
    ret: u32,
    #[serde(rename = "final")]
    expected: Vec<ObservedUnit>,
}

#[derive(serde::Deserialize)]
struct Evidence {
    cards: Vec<CardInfo>,
    cases: Vec<Probe>,
}

/// Opt-in comparison against private original 0x265250 plus event-handler
/// runs. A configured path must exist and contain nonempty valid evidence.
#[test]
fn private_seto_flip_instruction_oracle() {
    let Some(path) = std::env::var_os("ROSES_SETO_FLIP_ORACLE") else {
        return;
    };
    let evidence: Evidence = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(!evidence.cases.is_empty(), "empty flip oracle");
    let count = evidence.cases.len();
    for probe in evidence.cases {
        let mut cards = evidence.cards.clone();
        if let Some(entry) = probe.entry {
            cards[probe.card as usize].entries[3] = Some(entry);
        }
        let source = probe
            .initial
            .iter()
            .find(|u| u.side == probe.group && u.card == probe.card)
            .unwrap()
            .unit();
        let sides = std::array::from_fn(|i| SideState {
            lp: START_LP,
            stars: START_STARS,
            can_summon: true,
            hand: vec![],
            deck: vec![],
            leader_abilities: vec![],
            summons: probe.summons[i],
            graveyard: vec![],
            destiny: 0,
        });
        let mut d = Duel::from_observed(
            probe.terrain,
            probe.initial.iter().map(ObservedUnit::unit).collect(),
            sides,
            1,
            0,
            0,
            &cards,
        )
        .unwrap();
        let mut events = vec![];
        assert!(matches!(
            d.flip(
                source.id,
                FlipRole::Manual,
                source.position,
                &cards,
                &mut events
            )
            .unwrap(),
            Flip::Continue
        ));
        let mut actual: Vec<_> = d.units.iter().map(ObservedUnit::of).collect();
        actual.sort_by_key(|u| (u.side, u.slot));
        assert_eq!(actual, probe.expected, "{}", probe.name);
        assert_eq!(
            events
                .iter()
                .any(|e| matches!(e, DuelEvent::Activated { .. })),
            probe.ret & 1 != 0,
            "{} activation",
            probe.name
        );
    }
    eprintln!("Seto flip original-instruction oracle: {count} matched, 0 differ");
}
