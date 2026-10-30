//! York's summon planner 0x13B440 and its subtree: the category choice
//! 0x13B6D0 and the summon cell 0x13C300 are his own; the hand search
//! 0x13BA00, the non-monster choice 0x13BAF0, the monster choice 0x13BCF0
//! and the card list 0x13C110 are Pegasus's 0x137160 / 0x137250 / 0x137450
//! / 0x137800 by opcode shape and shared with it
//! ([`super::super::pegasus::summon`]); the fodder pass 0x13BED0 is
//! Weevil's 0x10EF80 with the limit 0xFA1 and the card test 0x13C0B0.

use super::super::pegasus::summon::{
    affordable_monster, card_list, category_of, find_card, non_monster, zone,
};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{
    kind_of, monster_choice, neighbour, virt, FodderFilter, Hand, MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{id, lane_cell, ritual_ok, strongest_threat, CRUSH, LABYRINTH, SUMMON_ROUTE_LIMIT};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x13BCF0 / 0x13BED0: 0x27A850 scoring at every tier, fodder limit
/// 0xFA1, non-monsters and monsters with effect entries never fodder.
const YORK_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0xFA1,
    fodder_filter: FodderFilter::NonMonsterOrEffect,
    skip_root: 999,
};

/// 0x13B440: the monster count (0x1E13D0) from which a target-less monster
/// summon turns into the non-monster choice (`slti 6`: never, the zone
/// holds five).
const MONSTERS_FOR_NON_MONSTER: i32 = 6;
/// 0x13B440: the depth of the summon target search 0x278850 (3 in
/// Pegasus's 0x136AE0).
const TARGET_DEPTH: i32 = 2;
/// 0x13C300: the distance below which a neighbour counts for the lane
/// goal.
const LANE_DISTANCE_LIMIT: i32 = 14;

/// 0x13B6D0(side, U): what to summon: 0 a monster (U set by the caller's
/// 0x13BCF0 unless set here), 1 a non-monster (U set), 2 a particular card
/// (U set by 0x13BA00), âˆ’1 nothing. In order: monster zone full â†’ the
/// non-monster choice; magic/trap zone full â†’ the first affordable
/// monster; Revival of Sennen Genjin in hand with acceptable tributes
/// (0x13E250); Raigeki while a [`strongest_threat`] exists; any trap in
/// hand; Cursebreaker while any slot 0..=10 of the group has a non-zero
/// 0x1E2800 value (an empty slot answers âˆ’1 and counts); the first
/// affordable monster; the non-monster choice.
pub(super) fn category<C: CardData + ?Sized, F: FusionData + ?Sized>(
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
    if find_card(w, side, &mut local, u, id::REVIVAL_OF_SENNEN_GENJIN)? >= 0
        && ritual_ok(w, side, id::REVIVAL_OF_SENNEN_GENJIN)? == 1
    {
        return Ok(2);
    }
    if strongest_threat(w, side)? >= 0 && find_card(w, side, &mut local, u, id::RAIGEKI)? >= 0 {
        return Ok(2);
    }
    for i in 0..w.next_hand_size(side) {
        let c = hand.get(i)?;
        if category_of(kind_of(w, c) >> 5) == 2 {
            *u = Some(virt(w, c));
            return Ok(1);
        }
    }
    for slot in 0..11 {
        if w.spellbind(side, slot) != 0 && find_card(w, side, &mut local, u, id::CURSEBREAKER)? >= 0
        {
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

/// 0x13C300(side, _, U, tgt): the summon cell among the leader's
/// neighbours (0x24FD30) that are empty, not Labyrinth and, on Crush,
/// would not crush U (0x251760).
/// - Non-monsters: the last such neighbour (0x13C64C), âˆ’1 when none.
/// - A monster with a target: the neighbour with the fewest moves to it:
///   0x27BA70 (see-all 0, at most 2, hidden strength 0x13E340 = 0) when
///   0x279E50 (strength 0, see-all 0) is positive, else 0x27AC00 (at most
///   2), counted only when the step buffer holds a candidate; the buffer
///   is filled once and never reset between neighbours, so a failed search
///   keeps the previous neighbour's candidates and its âˆ’1 wins. When no
///   neighbour has a route it falls through to the lane goal.
/// - Otherwise the neighbour whose Manhattan distance to the side's
///   [`lane_cell`] is the lowest below 14 (strict, first wins; no lane cell
///   reads as (0xFF, âˆ’1) and is never close); âˆ’1 when none.
pub(super) fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
    tgt: i32,
) -> R<i32> {
    // sp+0xD0: the step buffer.
    let mut buf = Steps::new();
    let open = |col: i32, row: i32| -> R<bool> {
        let t = w.terrain(col, row);
        if t == LABYRINTH || (t == CRUSH && w.crushed(u, col, row)?) {
            return Ok(false);
        }
        Ok(true)
    };
    if u.kind >> 5 != 0 {
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
            let m = w.margin_seen(u, tgt, 0, tc, tr, false)?;
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
                    SUMMON_ROUTE_LIMIT,
                    0,
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
                    SUMMON_ROUTE_LIMIT,
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
    let mut best = 0;
    let mut best_d = LANE_DISTANCE_LIMIT;
    for i in 0..8 {
        let cell = neighbour(w, side, i)?;
        if cell < 0 {
            continue;
        }
        let (col, row) = (cell & 0xFF, cell >> 8);
        if !open(col, row)? || w.at_key(col, row) >= 0 {
            continue;
        }
        let v = lane_cell(w, side);
        let (vc, vr) = (v & 0xFF, v >> 8);
        let d = (row.max(vr) - row.min(vr)) + (col.max(vc) - col.min(vc));
        if d < best_d && d >= 0 {
            best = cell;
            best_d = d;
        }
    }
    Ok(if best_d < LANE_DISTANCE_LIMIT {
        best
    } else {
        -1
    })
}

/// 0x13B440(key, side, d): fill `d` with a summon (type 0xA) or return âˆ’1:
/// summon flag clear, both zones full, no category. The target search
/// 0x278850 runs with depth 2, see-all 0 and hidden strength 0.
/// Category 0: 0x13BCF0; without a target and with six or more monsters
/// (never) the non-monster choice (filtered by the chosen card) takes
/// over. Category 2: 0x13C110 lists the card and its power-ups. Category
/// 1: 0x13BAF0 filtered by U's card. Then 0x13C300.
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
    // 0x278850(U, col, row, other, 2, 0x1F, 2, 2, [see-all 0, 0x13E340() = 0]).
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
            0,
        )
    };
    let with_u = |u: &Option<Unit>| match u {
        Some(u) => Ok(u.clone()),
        None => unsupported("0x13B440 reads an uninitialised U"),
    };
    let tgt;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, YORK_CHOICE)? < 0 {
            // Unreachable (0x13B6D0 found an affordable monster): the
            // original falls into the non-monster path with an
            // uninitialised target register.
            return unsupported("0x13B440 uses an uninitialised target");
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
            card_list(w, side, d, &u)?;
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
