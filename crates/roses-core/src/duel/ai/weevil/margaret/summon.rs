//! Margaret Mai Beaufort's summon planner 0x1488F0 (Labyrinth-ruler's
//! 0x132D30 with her own hidden strength, a monster limit of 4 and a
//! fourth category) and its subtree: the category choice 0x148BC0, the
//! Elegant Egotist list 0x148F00, the card list 0x149850 and the summon
//! cell 0x149A80 are her own; the hand search 0x149140, the non-monster
//! choice 0x149230 and the monster choice 0x149430 are Pegasus's 0x137160
//! / 0x137250 / 0x137450 (shared); the fodder pass 0x149610 is Weevil's
//! 0x10EF80 with the limit 0x1069 and York's card test 0x1497F0 =
//! 0x13C0B0.

use super::super::pegasus::summon::{
    affordable_monster, category_of, find_card, non_monster, zone,
};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{
    affordable, equip, equip_bonus, kind_of, monster_choice, neighbour, unit_mask, virt,
    FodderFilter, Hand, MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{formation_cell, graveyard_pick, hidden_value, id, ritual_ok, CRUSH};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x149430 / 0x149610: 0x27A850 scoring at every tier, fodder limit
/// 0x1069, non-monsters and monsters with effect entries never fodder.
const MARGARET_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0x1069,
    fodder_filter: FodderFilter::NonMonsterOrEffect,
    skip_root: 999,
};

/// 0x1488F0: the depth of the target search 0x278850.
const TARGET_DEPTH: i32 = 3;
/// 0x1488F0: from this many monsters (0x1E13D0) a target-less monster
/// summon turns into the non-monster choice.
const MONSTERS_FOR_NON_MONSTER: i32 = 4;
/// 0x149A80: the route limit of the target searches and of the
/// formation-cell searches.
const TARGET_ROUTE_LIMIT: i32 = 3;
const GOAL_ROUTE_LIMIT: i32 = 7;
/// 0x149A80: the distance bound of the formation-cell choice.
const GOAL_DISTANCE_LIMIT: i32 = 0xE;
/// The Labyrinth terrain (8).
const LABYRINTH: i32 = 8;

/// 0x148F00(side, d, U): with Elegant Egotist in the predicted hand (d+6 =
/// its LAST index) and an affordable Harpie Lady (d+7 = the LAST such
/// index): U = Harpie Lady Sisters' record (0x1DDFE0(0x111)), then every
/// other index holding a power-up (class 2, not Paralyzing Potion or
/// Cursebreaker) that fits the Sisters (0x24A880 ≥ 0) is appended from
/// d+8 on, each adding its 0x24ACC0 bonus to U's +0x38 and +0x3A. 1; −1
/// without the pair (d+6 may already be written). d+6.. are not reset.
fn egotist<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
) -> R<i32> {
    transform_pair(w, side, d, u, id::EGOTIST, id::HARPIE_LADY, id::HARPIE_SISTERS, true)
}

/// The shape of 0x148F00 with its three card immediates: `lead` in the
/// predicted hand (d+6 = its LAST index), an affordable `base` (d+7 = the
/// LAST such index), U = `result`'s record plus the bonuses of the other
/// power-ups that fit `result` (appended from d+8). Margaret's 0x148F00:
/// Elegant Egotist, Harpie Lady → Harpie Lady Sisters; Joey's 0x151A40:
/// 0x31F, 7 → 0x1F5.
pub(in super::super) fn transform_pair<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
    lead: i32,
    base: i32,
    result: i32,
    append: bool,
) -> R<i32> {
    let hand = Hand::of(w, side);
    let n = w.next_hand_size(side);
    let mut found = false;
    for i in 0..n {
        if hand.get(i)? == lead {
            found = true;
            d.hand[0] = i as u8;
        }
    }
    if !found {
        return Ok(-1);
    }
    let mut harpie = false;
    for i in 0..n {
        let c = hand.get(i)?;
        if c == base && affordable(w, side, c)? {
            d.hand[1] = i as u8;
            harpie = true;
        }
    }
    if !harpie {
        return Ok(-1);
    }
    let mut v = virt(w, result);
    let mut next = 2usize;
    // Yugi's copy 0x1602D0 has no power-up loop.
    for k in 0..if append { n } else { 0 } {
        if k == i32::from(d.hand[0]) || k == i32::from(d.hand[1]) {
            continue;
        }
        let c = hand.get(k)?;
        if equip(w, result, c)? < 0 {
            continue;
        }
        if kind_of(w, c) >> 5 != 2 || c == id::POTION || c == id::CURSEBREAKER {
            continue;
        }
        let Some(slot) = d.hand.get_mut(next) else {
            return unsupported("0x148F00 / 0x151A40 write past d+0xA");
        };
        *slot = k as u8;
        next += 1;
        let bonus = equip_bonus(w, c) as i16;
        v.atk_mod = v.atk_mod.wrapping_add(bonus);
        v.def_mod = v.def_mod.wrapping_add(bonus);
    }
    *u = Some(v);
    Ok(1)
}

/// 0x149850(side, d, U): Pegasus's 0x137800 (d+6.. = 0xFF; d+6 = the LAST
/// hand index holding U's card that is affordable; for a monster, append
/// every other index holding a power-up that fits it) that also adds each
/// appended power-up's 0x24ACC0 bonus to U's +0x38 and +0x3A. 1 (−1
/// without U).
pub(in super::super) fn card_list<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
) -> R<i32> {
    d.hand = [0xFF; 5];
    let hand = Hand::of(w, side);
    let Some(unit) = u.as_mut() else {
        return Ok(-1);
    };
    let n = w.next_hand_size(side);
    for i in 0..n {
        if i == i32::from(d.hand[0]) || i == i32::from(d.hand[1]) {
            continue;
        }
        let c = hand.get(i)?;
        if c != i32::from(unit.card) {
            continue;
        }
        if affordable(w, side, c)? {
            d.hand[0] = i as u8;
        }
    }
    let first = hand.get(i32::from(d.hand[0]))?;
    if kind_of(w, first) >> 5 != 0 {
        return Ok(1);
    }
    let mut count = 1usize;
    for k in 0..n {
        if k == i32::from(d.hand[0]) || k == i32::from(d.hand[1]) {
            continue;
        }
        let c = hand.get(k)?;
        if equip(w, first, c)? < 0 {
            continue;
        }
        if kind_of(w, c) >> 5 != 2 || c == id::POTION || c == id::CURSEBREAKER {
            continue;
        }
        let Some(slot) = d.hand.get_mut(count) else {
            return unsupported("0x149850 writes past d+0xA");
        };
        *slot = k as u8;
        count += 1;
        let bonus = equip_bonus(w, c) as i16;
        unit.atk_mod = unit.atk_mod.wrapping_add(bonus);
        unit.def_mod = unit.def_mod.wrapping_add(bonus);
    }
    Ok(1)
}

/// 0x148BC0(side, U): what to summon: 0 a monster (U set), 1 a non-monster
/// (U set), 2 a particular card (U set), 3 Harpie Lady with Elegant
/// Egotist (U set), −1 nothing. In order: monster zone full → the
/// non-monster choice; magic/trap zone full → the first affordable
/// monster; the ritual 0x34B in hand (affordable) with acceptable tributes
/// (0x14BB10); the Egotist pair; card 0x1E in hand while the side controls
/// one; card 0x2AD in hand with a [`graveyard_pick`]; Cursebreaker in hand
/// while one of the side's slots 0..=10 holds a spellbound unit or none
/// (0x1E2800 ≠ 0, −1 for an empty slot); the first affordable monster
/// other than Harpie Lady; the non-monster choice.
fn category<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &mut Option<Unit>,
) -> R<i32> {
    let monsters = zone(w, side, 0) & 0xFF;
    let others = zone(w, side, 1) & 0xFF;
    let hand = Hand::of(w, side);
    // sp+0x70: a local action record (written, never read).
    let mut local = Decision::new();
    if monsters == 5 {
        return Ok(if non_monster(w, side, &mut local, u, 999)? < 0 {
            -1
        } else {
            1
        });
    }
    if others == 5 {
        if let Some(c) = affordable_monster(w, side, &hand)? {
            *u = Some(virt(w, c));
            return Ok(0);
        }
        return Ok(-1);
    }
    if find_card(w, side, &mut local, u, id::RITUAL_34B)? >= 0
        && ritual_ok(w, side, id::RITUAL_34B)? == 1
    {
        return Ok(2);
    }
    if egotist(w, side, &mut local, u)? == 1 {
        return Ok(3);
    }
    if unit_mask(w, 0, 0, side, 1, (id::CARD_1E as u32) << 16 | 0x1C, 0xE)? != 0
        && find_card(w, side, &mut local, u, id::CARD_1E)? >= 0
    {
        return Ok(2);
    }
    if graveyard_pick(w, side) >= 0 && find_card(w, side, &mut local, u, id::CARD_2AD)? >= 0 {
        return Ok(2);
    }
    for slot in 0..11 {
        if w.spellbind(side, slot) == 0 {
            continue;
        }
        if find_card(w, side, &mut local, u, id::CURSEBREAKER)? >= 0 {
            return Ok(2);
        }
    }
    for i in 0..w.next_hand_size(side) {
        let c = hand.get(i)?;
        if kind_of(w, c) >> 5 != 0 || !affordable(w, side, c)? || c == id::HARPIE_LADY {
            continue;
        }
        *u = Some(virt(w, c));
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// 0x149A80(side, _, U, tgt): the summon cell among the leader's
/// neighbours (0x24FD30) that are empty, not Labyrinth and, on Crush,
/// would not crush U (0x251760):
/// - classes other than monsters and traps (0x1DF290 category 2): the
///   last such neighbour, −1 when none;
/// - with a target: the neighbour with the fewest moves to it (0x27BA70,
///   see-all 0, at most 3, hidden strength 0x14BC00(side) when 0x279E50
///   with that strength is positive, else 0x27AC00 at most 3), counted
///   only when the step buffer (shared, never reset) holds a candidate;
///   without one it falls through to
/// - the neighbour with the shortest route (0x27AC00, at most 7) to the
///   first formation cell (0x148780) holding no unit (strict, below 14,
///   first wins); −1 when none.
fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
    tgt: i32,
) -> R<i32> {
    // sp+0xE0: the step buffer.
    let mut buf = Steps::new();
    let open = |col: i32, row: i32| -> R<bool> {
        let t = w.terrain(col, row);
        if t == LABYRINTH || (t == CRUSH && w.crushed(u, col, row)?) {
            return Ok(false);
        }
        Ok(true)
    };
    let class = i32::from(u.kind >> 5);
    if class != 0 && category_of(class) != 2 {
        let mut last = -1;
        for i in 0..8 {
            let cell = neighbour(w, side, i)?;
            if cell < 0 {
                continue;
            }
            let (col, row) = (cell & 0xFF, cell >> 8);
            if !open(col, row)? || w.at_key(col, row) >= 0 || row >= 8 {
                continue;
            }
            last = cell;
        }
        return Ok(last);
    }
    if tgt >= 0 {
        let mut best_d = 0xFF;
        let mut best = 0xFFFF;
        let tp = w.pos(tgt >> 8, tgt & 0xFF);
        let (tc, tr) = (tp & 0xFF, tp >> 8);
        for i in 0..8 {
            let cell = neighbour(w, side, i)?;
            if cell < 0 {
                continue;
            }
            let (col, row) = (cell & 0xFF, cell >> 8);
            if !open(col, row)? || w.at(col, row).is_some() {
                continue;
            }
            let m = w.margin_seen(u, tgt, hidden_value(w, side), tc, tr, false)?;
            let d = if m > 0 {
                attack_path(
                    w,
                    Some(u),
                    &mut buf,
                    col,
                    row,
                    tc,
                    tr,
                    tgt as u32,
                    0,
                    TARGET_ROUTE_LIMIT,
                    hidden_value(w, side) & 0xFFFF,
                )?
            } else {
                path(
                    w,
                    Some(u),
                    &mut buf,
                    col,
                    row,
                    tc,
                    tr,
                    tgt & 0xFFFF,
                    TARGET_ROUTE_LIMIT,
                )?
            };
            if step_count(&buf) <= 0 {
                continue;
            }
            if d < best_d {
                best_d = d;
                best = cell;
            }
        }
        if best != 0xFFFF {
            return Ok(best);
        }
    }
    let mut best = -1;
    let mut best_d = GOAL_DISTANCE_LIMIT;
    for i in 0..8 {
        let cell = neighbour(w, side, i)?;
        if cell < 0 {
            continue;
        }
        let (col, row) = (cell & 0xFF, cell >> 8);
        if !open(col, row)? || w.at_key(col, row) >= 0 {
            continue;
        }
        let goal = (0..5)
            .map(formation_cell)
            .find(|&v| w.at(v & 0xFF, v >> 8).is_none())
            .unwrap_or(-1);
        let (gc, gr) = (goal & 0xFF, goal >> 8);
        let occ = match w.at_key(gc, gr) {
            k if k >= 0 => k & 0xFFFF,
            _ => 0xFFFF,
        };
        let d = path(
            w,
            Some(u),
            &mut buf,
            col,
            row,
            gc,
            gr,
            occ,
            GOAL_ROUTE_LIMIT,
        )?;
        if d < best_d && d >= 0 {
            best = cell;
            best_d = d;
        }
    }
    Ok(if best_d < GOAL_DISTANCE_LIMIT {
        best
    } else {
        -1
    })
}

/// 0x1488F0(key, side, d): fill `d` with a summon (type 0xA) or return −1:
/// summon flag clear, both zones full, no category. The target search
/// 0x278850 (depth 3, filter 0x1F, see-all 0, hidden strength
/// 0x14BC00(side)) runs on the category's U, after the monster choice for
/// category 0. Category 0: 0x149430; without a target and with four or
/// more monsters the non-monster choice filtered by the chosen card.
/// Category 2: 0x149850 lists the card and its power-ups; 3: 0x148F00 on
/// the action record; 1: the non-monster choice filtered by U's card. Then
/// 0x149A80 with the target.
pub(super) fn summon<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    key: i32,
    side: i32,
    d: &mut Decision,
) -> R<i32> {
    d.key = key as u16;
    if !w.can_summon(side) {
        return Ok(-1);
    }
    if zone(w, side, 0) == 5 && zone(w, side, 1) == 5 {
        return Ok(-1);
    }
    let mut u = None;
    let r = category(w, side, &mut u)?;
    if r < 0 {
        return Ok(-1);
    }
    d.kind = 0xA;
    d.reveal = 0;
    let pos = w.pos(key >> 8, key & 0xFF);
    let other = i32::from(side == 0);
    // 0x278850(U, col, row, other, 3, 0x1F, 2, 2, [see-all 0, 0x14BC00(side)]).
    let target = |u: &Option<Unit>| -> R<i32> {
        find_target(
            w,
            u.as_ref(),
            pos & 0xFF,
            pos >> 8,
            other,
            TARGET_DEPTH,
            0x1F,
            2,
            2,
            0,
            hidden_value(w, side),
        )
    };
    let with_u = |u: &Option<Unit>| match u {
        Some(u) => Ok(u.clone()),
        None => unsupported("0x1488F0 reads an uninitialised U"),
    };
    let tgt;
    let mut non_monster_path = false;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, MARGARET_CHOICE)? < 0 {
            // Unreachable (0x148BC0 found an affordable monster): the
            // original falls into the non-monster path with an
            // uninitialised target register.
            return unsupported("0x1488F0 uses an uninitialised target");
        }
        tgt = target(&u)?;
        if tgt < 0 && w.monsters(side) >= MONSTERS_FOR_NON_MONSTER {
            non_monster_path = true;
        }
    } else {
        tgt = target(&u)?;
        match r {
            2 => {
                card_list(w, side, d, &mut u)?;
            }
            3 => {
                egotist(w, side, d, &mut u)?;
            }
            _ => non_monster_path = true,
        }
    }
    if non_monster_path {
        // 0x148B50: filter = U +0x3C.
        let card = i32::from(with_u(&u)?.card);
        if non_monster(w, side, d, &mut u, card)? < 0 {
            return Ok(-1);
        }
    }
    let c = cell(w, side, &with_u(&u)?, tgt)?;
    if c < 0 {
        return Ok(-1);
    }
    d.target = c as u16;
    Ok(1)
}
