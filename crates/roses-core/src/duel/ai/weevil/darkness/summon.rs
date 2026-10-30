//! Darkness-ruler's summon planner 0x12DB40 and its subtree: the category
//! choice 0x12DDF0, its trap-first hand search 0x12E230 and the summon
//! cell 0x12ECC0 are his own; the hand search 0x12E3B0 (affordability
//! tested before and after the id compare, the same answer), the
//! non-monster choice 0x12E4B0 and the monster choice 0x12E6B0 are
//! Pegasus's 0x137160 / 0x137250 / 0x137450 by opcode shape and shared
//! with it ([`super::super::pegasus::summon`]); the card list 0x12EAC0 is
//! Pegasus's 0x137800 returning U's class instead of 1 (dropped by the
//! caller); the fodder pass 0x12E890 is Weevil's 0x10EF80 with the limit
//! 0x1069 and the card test 0x12EA70.

use super::super::pegasus::summon::{affordable_monster, card_list, find_card, non_monster, zone};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{
    affordable, kind_of, monster_choice, neighbour, virt, FodderFilter, Hand, MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{graveyard_pick, hidden_value, id, ritual_ok};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x12E6B0 / 0x12E890: 0x27A850 scoring at every tier, fodder limit
/// 0x1069, every recorded card refused as fodder (0x12EA70).
const DARKNESS_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0x1069,
    fodder_filter: FodderFilter::Recorded,
    skip_root: 999,
};

/// 0x12ECC0: the route limit of the summon cell's 0x27BA70 search and of
/// its 0x27AC00 search.
const ATTACK_ROUTE_LIMIT: i32 = 5;
const PLAIN_ROUTE_LIMIT: i32 = 2;
/// 0x12ECC0: the distance limit of the cell nearest the enemy leader.
const NEAR_LIMIT: i32 = 0xE;
/// 0x12E230: 0x1DF290's trap category.
const TRAP_CATEGORY: i32 = 2;

/// 0x1DF290(class): the card category of a class: monsters 0, magic and
/// power-ups 1, traps 2, rituals 3, anything else itself.
fn category_of(class: i32) -> i32 {
    match class {
        1 | 2 => 1,
        3 | 4 => 2,
        5 => 3,
        c => c,
    }
}

/// 0x12E230(side, hand, U, d): the first predicted-hand index holding a
/// trap (0x1DF290 = 2, affordable or not), else the first holding card
/// 0x31B, 0x2AC or 0x137 that is affordable (0x271440): d+6 = index, U =
/// 0x1DDFE0(card), returns the card's class; −1 when none.
fn trap_first<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    hand: &Hand,
    u: &mut Option<Unit>,
    d: &mut Decision,
) -> R<i32> {
    let n = w.next_hand_size(side);
    for i in 0..n {
        let c = hand.get(i)?;
        if category_of(kind_of(w, c) >> 5) != TRAP_CATEGORY {
            continue;
        }
        d.hand[0] = i as u8;
        *u = Some(virt(w, c));
        return Ok(kind_of(w, c) >> 5);
    }
    for i in 0..n {
        let c = hand.get(i)?;
        if !affordable(w, side, c)? {
            continue;
        }
        if c != id::POTION && c != id::CARD_2AC && c != id::CARD_137 {
            continue;
        }
        d.hand[0] = i as u8;
        *u = Some(virt(w, c));
        return Ok(kind_of(w, c) >> 5);
    }
    Ok(-1)
}

/// 0x12DDF0(side, U, mode): what to summon: 0 a monster (U set by the
/// caller's 0x12E6B0 unless set here), 1 a non-monster (U set), 2 a
/// particular card (U set by 0x12E230 / 0x12E3B0), −1 nothing. In order:
/// monster zone full → the non-monster choice; magic/trap zone full → the
/// first affordable monster. Mode 0: [`trap_first`]; the ritual 0x342 in
/// hand with acceptable tributes (0x1310B0); card 0x131 with a graveyard
/// pick (0x130F30, never). Mode 1: the ritual 0x342 as above; card 0x146
/// unless the ritual's tributes were acceptable and it was not in hand
/// (the register tested, s2, then holds that search's −1; otherwise the
/// caller's s2, the action record's address, which is never below 4). A
/// count of enemy-controlled units is computed and dropped. Then the
/// first affordable monster; the non-monster choice.
fn category<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &mut Option<Unit>,
    mode: u8,
) -> R<i32> {
    let monsters = zone(w, side, 0) & 0xFF;
    let others = zone(w, side, 1) & 0xFF;
    let hand = Hand::of(w, side);
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
    if mode == 0 {
        if trap_first(w, side, &hand, u, &mut local)? >= 0 {
            return Ok(2);
        }
        if ritual_ok(w, side, id::RITUAL_342)? == 1
            && find_card(w, side, &mut local, u, id::RITUAL_342)? >= 0
        {
            return Ok(2);
        }
        // 0x12DFA8: the enemy-unit count, dropped; s2 is −1 here, so card
        // 0x146 is never searched in mode 0.
        if graveyard_pick(w, side)? >= 0 && find_card(w, side, &mut local, u, id::CARD_131)? >= 0 {
            return Ok(2);
        }
    } else {
        // s2: the caller's register (the action record's address) until
        // the 0x342 search writes it.
        let mut s2_below_4 = false;
        if ritual_ok(w, side, id::RITUAL_342)? == 1 {
            let f = find_card(w, side, &mut local, u, id::RITUAL_342)?;
            if f >= 0 {
                return Ok(2);
            }
            s2_below_4 = f < 4;
        }
        // 0x12E0CC: the enemy-unit count, dropped.
        if !s2_below_4 && find_card(w, side, &mut local, u, id::CARD_146)? >= 0 {
            return Ok(2);
        }
    }
    if let Some(c) = affordable_monster(w, side, &hand)? {
        *u = Some(virt(w, c));
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// 0x12ECC0(side, _, U, tgt): the summon cell among the leader's
/// neighbours (0x24FD30), by U's class:
/// - monsters, power-ups and traps with a target: the empty neighbour
///   (not Labyrinth, nor Crush when it would crush U) with the fewest
///   moves to the target: 0x27BA70 (see-all 0, at most 5, hidden strength
///   0x131050(side) & 0xFFFF) when 0x279E50 (strength 0x131050(side),
///   see-all 0) is positive, else 0x27AC00 (at most 2), counted only when
///   the step buffer holds a candidate (filled once, never reset); −1 when
///   none has a route;
/// - the same classes without a target: the empty neighbour (as above)
///   nearest the enemy leader's cell (Manhattan below 14, first wins);
/// - magic, rituals and the rest: the last empty neighbour that is neither
///   Labyrinth nor Crush (no crush test).
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
        if t == 8 || (t == 9 && w.crushed(u, col, row)?) {
            return Ok(false);
        }
        Ok(true)
    };
    if !matches!(u.kind >> 5, 0 | 2 | 3) {
        let mut last = -1;
        for i in 0..8 {
            let cell = neighbour(w, side, i)?;
            if cell < 0 {
                continue;
            }
            let (col, row) = (cell & 0xFF, cell >> 8);
            let t = w.terrain(col, row);
            if t == 8 || t == 9 || w.at_key(col, row) >= 0 || row >= 8 {
                continue;
            }
            last = cell;
        }
        return Ok(last);
    }
    if tgt < 0 {
        let other = i32::from(side == 0);
        let lp = w.pos(other, 0);
        let (lr, lc) = (lp >> 8, lp & 0xFF);
        let mut best = -1;
        let mut best_d = NEAR_LIMIT;
        for i in 0..8 {
            let cell = neighbour(w, side, i)?;
            if cell < 0 {
                continue;
            }
            let (col, row) = (cell & 0xFF, cell >> 8);
            if !open(col, row)? || w.at_key(col, row) >= 0 {
                continue;
            }
            let d = (row.max(lr) - row.min(lr)) + (col.max(lc) - col.min(lc));
            if d < best_d && d >= 0 {
                best = cell;
                best_d = d;
            }
        }
        return Ok(if best_d < NEAR_LIMIT { best } else { -1 });
    }
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
        let hidden = hidden_value(w, side)?;
        let m = w.margin_seen(u, tgt, hidden, tc, tr, false)?;
        let d = if m > 0 {
            let hidden = hidden_value(w, side)? & 0xFFFF;
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
                ATTACK_ROUTE_LIMIT,
                hidden,
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
                PLAIN_ROUTE_LIMIT,
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
    Ok(if best == 0xFFFF { -1 } else { best })
}

/// 0x12DB40(key, side, d, mode): fill `d` with a summon (type 0xA) or
/// return −1: summon flag clear, both zones full, no category. The target
/// search 0x278850 (see-all 0, hidden strength 0x131050(side)) runs first
/// on the category's U. Category 0: 0x12E6B0, then the search again on
/// its U; a failed choice (never: the category found an affordable
/// monster) takes the non-monster path with the first search's target.
/// Category 2: 0x12EAC0 lists the card and its power-ups. Category 1:
/// 0x12E4B0 filtered by U's card. Then 0x12ECC0.
pub(super) fn summon<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    key: i32,
    side: i32,
    d: &mut Decision,
    mode: u8,
) -> R<i32> {
    d.key = key as u16;
    if !w.can_summon(side) {
        return Ok(-1);
    }
    if zone(w, side, 0) == 5 && zone(w, side, 1) == 5 {
        return Ok(-1);
    }
    let mut u = None;
    let r = category(w, side, &mut u, mode)?;
    if r < 0 {
        return Ok(-1);
    }
    d.kind = 0xA;
    d.reveal = 0;
    let pos = w.pos(key >> 8, key & 0xFF);
    let other = i32::from(side == 0);
    // 0x278850(U, col, row, other, 3, 0x1F, 2, 2, [see-all 0, 0x131050(side)]).
    let target = |u: &Option<Unit>| -> R<i32> {
        let hidden = hidden_value(w, side)?;
        find_target(
            w,
            u.as_ref(),
            pos & 0xFF,
            pos >> 8,
            other,
            3,
            0x1F,
            2,
            2,
            0,
            hidden,
        )
    };
    let with_u = |u: &Option<Unit>| match u {
        Some(u) => Ok(u.clone()),
        None => unsupported("0x12DB40 reads an uninitialised U"),
    };
    let mut tgt = target(&u)?;
    let non_monster_path = r != 0 && r != 2;
    let mut fallback = non_monster_path;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, DARKNESS_CHOICE)? < 0 {
            fallback = true;
        } else {
            tgt = target(&u)?;
            // 0x12DCB8: 0x1E13D0(side) < 6 is always true, so a monster
            // without a target keeps the monster path.
            if tgt < 0 && w.monsters(side) >= 6 {
                fallback = true;
            }
        }
    } else if r == 2 {
        card_list(w, side, d, &u)?;
    }
    if fallback {
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
