use super::*;

const MONSTER: u16 = 2;
const LORD: u16 = 3;
const REVERSE: u16 = 4;
const FAN: u16 = 5;
const DECREE: u16 = 6;
const EQUIP: u16 = 752;
const POTION: u16 = 753;
const TRANSFORM: u16 = 754;
const RIRYOKU: u16 = 755;
const CLEANSE: u16 = 756;

fn p(column: u8, row: u8) -> GridPos {
    GridPos { column, row }
}

fn cards() -> Vec<CardInfo> {
    let mut c: Vec<_> = (0..757)
        .map(|id| CardInfo {
            id,
            kind: 0,
            kind_index: 0,
            level: 2,
            attribute: 0,
            attack: 600,
            defense: 350,
            toon: false,
            entries: [None; 5],
            effects: [false; 5],
            equip_bits: [0x1F, 0, 0, 0, 0, 0, 0],
            power_up: None,
        })
        .collect();
    for (id, category, value) in [
        (EQUIP, 0, 400),
        (POTION, 3, 0),
        (TRANSFORM, 4, 7),
        (RIRYOKU, 0, 0),
        (CLEANSE, 2, 123),
    ] {
        c[usize::from(id)].kind = class::POWER_UP << 5;
        c[usize::from(id)].power_up = Some(PowerUp { category, value });
    }
    c[usize::from(LORD)].entries[2] = Some([0xB00, 0]);
    c[usize::from(LORD)].effects[2] = true;
    for (id, ty) in [(REVERSE, 0x2B), (FAN, 0x29), (DECREE, 0x2E)] {
        c[usize::from(id)].kind = class::FAR_TRAP << 5;
        c[usize::from(id)].entries[0] = Some([ty << 8, 0]);
    }
    c
}

fn board(c: &Vec<CardInfo>) -> Duel {
    Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [0, 1],
            decks: [vec![], vec![]],
            first: 0,
            leader_abilities: [vec![], vec![]],
            controllers: [PAD_CONTROLLER; 2],
        },
        c,
    )
    .unwrap()
    .0
}

fn add(d: &mut Duel, c: &Vec<CardInfo>, card: u16, side: u8, at: GridPos) -> UnitId {
    d.create(side, card, at, c, &mut vec![]).unwrap()
}

fn action(
    d: &Duel,
    c: &Vec<CardInfo>,
    id: UnitId,
    at: GridPos,
    face_up: bool,
) -> Result<(Duel, Vec<DuelEvent>)> {
    d.apply(
        d.active,
        &DuelCommand::Move {
            unit: id,
            path: vec![at],
            face_up,
            defence: None,
        },
        c,
    )
}

#[test]
fn attack_equips_and_advances_without_flipping_a_set_monster() {
    let c = cards();
    for side in 0..2 {
        for face in [false, true] {
            let mut d = board(&c);
            d.active = side;
            let a = add(&mut d, &c, MONSTER, side, p(2, 3));
            let target = add(&mut d, &c, EQUIP, side ^ 1, p(3, 3));
            let (d, _) = action(&d, &c, a, p(3, 3), face).unwrap();
            let u = d.unit(a).unwrap();
            assert_eq!(u.position, p(3, 3));
            assert_eq!((u.attack_bonus, u.defense_bonus), (400, 400));
            assert_eq!((u.face_up, u.committed_face_up), (face, face));
            assert!(u.acted);
            assert!(d.unit(target).is_none());
            assert_eq!(d.sides[usize::from(side ^ 1)].graveyard, [EQUIP]);
            assert_eq!(d.experience[usize::from(side)][&MONSTER].delta, 6);
        }
    }
}

#[test]
fn potion_binds_the_attacker_before_it_can_advance() {
    let c = cards();
    let mut d = board(&c);
    let a = add(&mut d, &c, MONSTER, 0, p(2, 3));
    let target = add(&mut d, &c, POTION, 1, p(3, 3));
    let (d, _) = action(&d, &c, a, p(3, 3), false).unwrap();
    let u = d.unit(a).unwrap();
    assert_eq!(u.position, p(2, 3));
    assert_eq!(u.spellbind, 255);
    assert!(u.spellbind_fresh && u.acted);
    assert_eq!((u.attack_bonus, u.defense_bonus), (0, 0));
    assert!(d.unit(target).is_none());
}

#[test]
fn cleansing_power_up_resets_deltas_and_bind_but_preserves_fresh_and_evolution() {
    let c = cards();
    for moving_power in [false, true] {
        let mut d = board(&c);
        let a = add(
            &mut d,
            &c,
            if moving_power { CLEANSE } else { MONSTER },
            0,
            p(2, 3),
        );
        let target = add(
            &mut d,
            &c,
            if moving_power { MONSTER } else { CLEANSE },
            1,
            p(3, 3),
        );
        let monster = if moving_power { target } else { a };
        let u = d.units.iter_mut().find(|u| u.id == monster).unwrap();
        u.attack_bonus = 250;
        u.defense_bonus = -100;
        u.spellbind = if moving_power { 255 } else { 0 };
        u.spellbind_fresh = true;
        u.evo_counter = 7;
        let (d, _) = action(&d, &c, a, p(3, 3), false).unwrap();
        let u = d.unit(monster).unwrap();
        assert_eq!((u.attack_bonus, u.defense_bonus, u.spellbind), (0, 0, 0));
        assert!(u.spellbind_fresh);
        assert_eq!(u.evo_counter, 7);
        assert_eq!(u.acted, !moving_power);
        assert_eq!(u.position, p(3, 3));
    }
}

#[test]
fn dragon_protection_discards_even_an_unported_equip_without_exposure() {
    let c = cards();
    for power in [EQUIP, POTION, CLEANSE, TRANSFORM, RIRYOKU] {
        let mut d = board(&c);
        let a = add(&mut d, &c, MONSTER, 0, p(2, 3));
        let target = add(&mut d, &c, power, 1, p(3, 3));
        let lord = add(&mut d, &c, LORD, 1, p(6, 4));
        let source = d.units.iter_mut().find(|u| u.id == lord).unwrap();
        source.committed_face_up = true;
        source.defence = true;
        source.committed_defence = true;
        let (d, _) = action(&d, &c, a, p(3, 3), false).unwrap();
        let u = d.unit(a).unwrap();
        assert_eq!(u.position, p(3, 3));
        assert_eq!((u.attack_bonus, u.defense_bonus, u.spellbind), (0, 0, 0));
        assert!(!u.face_up && !u.committed_face_up);
        assert!(d.unit(target).is_none());
    }
}

#[test]
fn ordinary_field_equips_ignore_fan_and_untriggered_decree() {
    let c = cards();
    for (power, trap) in [
        (EQUIP, FAN),
        (EQUIP, DECREE),
        (POTION, REVERSE),
        (CLEANSE, REVERSE),
    ] {
        for trap_side in 0..2 {
            for moving_power in [false, true] {
                for own in [false, true] {
                    let mut d = board(&c);
                    let a = add(
                        &mut d,
                        &c,
                        if moving_power { power } else { MONSTER },
                        0,
                        p(2, 3),
                    );
                    let t = add(
                        &mut d,
                        &c,
                        if moving_power { MONSTER } else { power },
                        u8::from(!own),
                        p(3, 3),
                    );
                    let trap = add(&mut d, &c, trap, trap_side, p(6, 5));
                    let saved = d.unit(trap).unwrap().clone();
                    let (d, _) = action(&d, &c, a, p(3, 3), false).unwrap();
                    assert_eq!(d.unit(trap), Some(&saved));
                    let m = d.unit(if moving_power { t } else { a }).unwrap();
                    assert_eq!(m.spellbind, if power == POTION { 255 } else { 0 });
                    assert_eq!(m.attack_bonus, if power == EQUIP { 400 } else { 0 });
                }
            }
        }
    }
}

#[test]
fn untraced_transform_riryoku_and_reverse_reactions_refuse_atomically() {
    let c = cards();
    // Riryoku with a Reverse Trap is ported since 7 Oct (refusals_tests.rs).
    for (power, trap) in [(TRANSFORM, None), (EQUIP, Some(REVERSE))] {
        let mut d = board(&c);
        let a = add(&mut d, &c, MONSTER, 0, p(2, 3));
        add(&mut d, &c, power, 1, p(3, 3));
        if let Some(trap) = trap {
            add(&mut d, &c, trap, 1, p(6, 5));
        }
        let original = d.clone();
        assert!(matches!(
            action(&d, &c, a, p(3, 3), false),
            Err(DuelError::Unsupported(_))
        ));
        assert_eq!(d, original);
    }
}

#[test]
fn equip_stat_setters_cap_against_printed_stats() {
    let c = cards();
    let mut d = board(&c);
    let a = add(&mut d, &c, MONSTER, 0, p(2, 3));
    add(&mut d, &c, EQUIP, 1, p(3, 3));
    let u = d.units.iter_mut().find(|u| u.id == a).unwrap();
    u.attack_bonus = 9999;
    u.defense_bonus = -9999;
    let (d, _) = action(&d, &c, a, p(3, 3), false).unwrap();
    let u = d.unit(a).unwrap();
    assert_eq!((u.attack_bonus, u.defense_bonus), (9399, -9599));
}

#[test]
fn all_field_collision_directions_use_the_original_capped_setters() {
    let c = cards();
    for own in [false, true] {
        for moving_power in [false, true] {
            for bonuses in [(9999, -9999), (-7000, 9800)] {
                let mut d = board(&c);
                let a = add(
                    &mut d,
                    &c,
                    if moving_power { EQUIP } else { MONSTER },
                    0,
                    p(2, 3),
                );
                let target = add(
                    &mut d,
                    &c,
                    if moving_power { MONSTER } else { EQUIP },
                    u8::from(!own),
                    p(3, 3),
                );
                let monster = if moving_power { target } else { a };
                let u = d.units.iter_mut().find(|u| u.id == monster).unwrap();
                (u.attack_bonus, u.defense_bonus) = bonuses;
                let (d, _) = action(&d, &c, a, p(3, 3), false).unwrap();
                let u = d.unit(monster).unwrap();
                let expected = if bonuses.0 == 9999 {
                    (9399, -9599)
                } else {
                    (-6600, 9649)
                };
                assert_eq!(
                    (u.attack_bonus, u.defense_bonus),
                    expected,
                    "own={own}, moving_power={moving_power}, initial={bonuses:?}"
                );
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct OriginalState {
    flags: u8,
    bound: u8,
    fresh: bool,
    attack: i16,
    defense: i16,
    power_gone: bool,
}

#[derive(serde::Deserialize)]
struct OracleBoard {
    units: Vec<DuelUnit>,
}

#[derive(serde::Deserialize)]
struct OracleCase {
    att: [u8; 2],
    power: [u8; 2],
    duel: OracleBoard,
    branch: String,
    expected: OriginalState,
}

#[derive(serde::Deserialize)]
struct Evidence {
    cards: Vec<CardInfo>,
    cases: Vec<OracleCase>,
}

#[test]
fn private_attacked_power_up_matches_original_instructions() {
    let Some(path) = std::env::var_os("ROSES_POWERUP_ATTACK_ORACLE") else {
        return;
    };
    let text = std::fs::read_to_string(path).expect("configured power-up evidence must exist");
    let evidence: Evidence = serde_json::from_str(&text).expect("valid power-up evidence");
    assert!(!evidence.cases.is_empty());
    let mut matched = 0;
    let mut refused = 0;
    for (i, case) in evidence.cases.iter().enumerate() {
        let mut d = board(&evidence.cards);
        d.units = case.duel.units.clone();
        let a = d
            .units
            .iter()
            .find(|u| [u.side, u.slot] == case.att)
            .unwrap()
            .clone();
        let power = d
            .units
            .iter()
            .find(|u| [u.side, u.slot] == case.power)
            .unwrap()
            .clone();
        let result =
            d.attack_power_up(a.id, power.id, power.position, &evidence.cards, &mut vec![]);
        if case.branch == "reverse" {
            assert!(matches!(result, Err(DuelError::Unsupported(_))), "case {i}");
            refused += 1;
            continue;
        }
        result.unwrap_or_else(|e| panic!("case {i}: {e}"));
        let u = d.unit(a.id).unwrap();
        let flags = u8::from(u.acted)
            | (u8::from(u.stolen) << 1)
            | (u8::from(u.face_up) << 2)
            | (u8::from(u.defence) << 3)
            | (u8::from(u.committed_face_up) << 4)
            | (u8::from(u.committed_defence) << 5);
        assert_eq!(
            (
                flags,
                u.spellbind,
                u.spellbind_fresh,
                u.attack_bonus,
                u.defense_bonus
            ),
            (
                case.expected.flags,
                case.expected.bound,
                case.expected.fresh,
                case.expected.attack,
                case.expected.defense
            ),
            "case {i}"
        );
        assert_eq!(
            d.unit(power.id).is_none(),
            case.expected.power_gone,
            "case {i}"
        );
        assert_eq!(
            d.experience[usize::from(a.side)][&a.card].delta,
            6,
            "case {i}"
        );
        matched += 1;
    }
    eprintln!("Attacked power-up: {matched} original instruction cases match, {refused} explicit Reverse refusals");
}

#[test]
fn an_attacked_riryoku_drains_the_attackers_own_side() {
    // Event 0x25 (WHITE-REFUSALS.md §22, emu RR1): the amount is half the
    // LP of the Riryoku's opponent group, i.e. the attacker's own side;
    // the attacker gains it and advances.
    let c = cards();
    let mut d = board(&c);
    let a = add(&mut d, &c, MONSTER, 0, p(2, 3));
    add(&mut d, &c, RIRYOKU, 1, p(3, 3));
    let (d, _) = action(&d, &c, a, p(3, 3), false).unwrap();
    let u = d.unit(a).unwrap();
    assert_eq!(u.attack_bonus, 2000);
    assert_eq!(d.sides[0].lp, 2000);
    assert_eq!(d.sides[1].lp, START_LP);
}
