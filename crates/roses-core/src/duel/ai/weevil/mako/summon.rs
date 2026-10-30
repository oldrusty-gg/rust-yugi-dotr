//! Mako's summon planner 0x14D470 (Labyrinth-ruler's 0x132D30 writing
//! +0xC = 2 and with the monster limit 6) and its subtree: the category
//! choice 0x14D700 and the summon cell 0x14E310 (Margaret's 0x149A80 with
//! the formation of [`super::formation_cell`] and no occupant allowed on
//! the route) are his own; the hand search 0x14D9C0, the non-monster
//! choice 0x14DAB0 and the monster choice 0x14DCB0 are Pegasus's 0x137160
//! / 0x137250 / 0x137450; the card list 0x14E0D0 is Margaret's 0x149850;
//! the fodder pass 0x14DE90 is Weevil's 0x10EF80 with the limit 0x1069
//! and York's card test 0x14E070 = 0x13C0B0.

use super::super::margaret::summon::card_list;
use super::super::pegasus::summon::{
    affordable_monster, category_of, find_card, non_monster, zone,
};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{monster_choice, neighbour, virt, FodderFilter, Hand, MonsterChoice};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{formation_cell, formation_full, hidden_value, id, ritual_ok, CRUSH};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x14DCB0 / 0x14DE90: 0x27A850 scoring at every tier, fodder limit
/// 0x1069, non-monsters and monsters with effect entries never fodder.
const MAKO_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0x1069,
    fodder_filter: FodderFilter::NonMonsterOrEffect,
    skip_root: 999,
};

/// 0x14D470: the depth of the target search 0x278850.
const TARGET_DEPTH: i32 = 3;
/// 0x14D470: from this many monsters (0x1E13D0, `slti 6`: never) a
/// target-less monster summon turns into the non-monster choice.
const MONSTERS_FOR_NON_MONSTER: i32 = 6;
/// 0x14E310: the route limits and the distance bound.
const TARGET_ROUTE_LIMIT: i32 = 3;
const GOAL_ROUTE_LIMIT: i32 = 7;
const GOAL_DISTANCE_LIMIT: i32 = 0xE;
/// The Labyrinth terrain (8).
const LABYRINTH: i32 = 8;

/// 0x14D700(side, U): what to summon: 0 a monster (U set), 1 a non-monster
/// (U set), 2 a particular card (U set), −1 nothing. In order: monster
/// zone full → the non-monster choice; magic/trap zone full → the first
/// affordable monster; the ritual 0x354 in hand (affordable) with
/// acceptable tributes (0x150190); card 0x2BD with a full formation and no
/// own magic or trap card (0x1E1470); card 0x2E6 with four or more own
/// monsters; the first affordable monster; the non-monster choice.
fn category<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &mut Option<Unit>,
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
    if find_card(w, side, &mut local, u, id::RITUAL_354)? >= 0
        && ritual_ok(w, side, id::RITUAL_354)? == 1
    {
        return Ok(2);
    }
    if formation_full(w)? == 1
        && w.others(side) == 0
        && find_card(w, side, &mut local, u, id::CARD_2BD)? >= 0
    {
        return Ok(2);
    }
    if w.monsters(side) >= 4 && find_card(w, side, &mut local, u, id::CARD_2E6)? >= 0 {
        return Ok(2);
    }
    if let Some(c) = affordable_monster(w, side, &hand)? {
        *u = Some(virt(w, c));
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// 0x14E310(side, _, U, tgt): Margaret's 0x149A80 with the goal of the
/// target-less search the first formation cell (0x14D2A0) holding no
/// unit (an off-board one counts as free and is searched for as it is;
/// none → −1), the route's allowed occupant key 0 (`move $t2, $zero`;
/// 0x1E4A10's answer is dropped) and the hidden strength 0x150280(side).
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
        let mut goal = -1;
        for k in 0..3 {
            let v = formation_cell(w, k)?;
            if w.at(v & 0xFF, v >> 8).is_none() {
                goal = v;
                break;
            }
        }
        let (gc, gr) = (goal & 0xFF, goal >> 8);
        let d = path(w, Some(u), &mut buf, col, row, gc, gr, 0, GOAL_ROUTE_LIMIT)?;
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

/// 0x14D470(key, side, d): Labyrinth-ruler's 0x132D30: fill `d` with a
/// summon (type 0xA, +0xC = 2) or return −1: summon flag clear, both zones
/// full, no category. The target search 0x278850 (depth 3, filter 0x1F,
/// see-all 0, hidden strength 0x150280(side)) runs on the category's U,
/// after the monster choice for category 0. Category 0: 0x14DCB0 (the
/// non-monster fallback needs six monsters: never). Category 2: 0x14E0D0
/// lists the card and its power-ups (adding their bonuses to U). Category
/// 1: the non-monster choice filtered by U's card. Then 0x14E310 with the
/// target.
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
    d.reveal = 2;
    let pos = w.pos(key >> 8, key & 0xFF);
    let other = i32::from(side == 0);
    // 0x278850(U, col, row, other, 3, 0x1F, 2, 2, [see-all 0, 0x150280(side)]).
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
        None => unsupported("0x14D470 reads an uninitialised U"),
    };
    let tgt;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, MAKO_CHOICE)? < 0 {
            // Unreachable (0x14D700 found an affordable monster).
            return unsupported("0x14D470 uses an uninitialised target");
        }
        tgt = target(&u)?;
        if tgt < 0 && w.monsters(side) >= MONSTERS_FOR_NON_MONSTER {
            let card = i32::from(with_u(&u)?.card);
            if non_monster(w, side, d, &mut u, card)? < 0 {
                return Ok(-1);
            }
        }
    } else {
        tgt = target(&u)?;
        if r == 2 {
            card_list(w, side, d, &mut u)?;
        } else {
            let card = i32::from(with_u(&u)?.card);
            if non_monster(w, side, d, &mut u, card)? < 0 {
                return Ok(-1);
            }
        }
    }
    let c = cell(w, side, &with_u(&u)?, tgt)?;
    if c < 0 {
        return Ok(-1);
    }
    d.target = c as u16;
    Ok(1)
}
