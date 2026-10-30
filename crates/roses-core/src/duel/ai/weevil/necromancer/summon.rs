//! Necromancer's summon planner 0x128700 (Labyrinth-ruler's 0x132D30 by
//! opcode shape, with the hidden strength 0x12BA80) and its subtree: the
//! category choice 0x128990 and the summon cell 0x129610 are his own; the
//! hand search 0x128D00, the non-monster choice 0x128DF0, the monster
//! choice 0x128FF0 and the card list 0x129420 are Pegasus's 0x137160 /
//! 0x137250 / 0x137450 / 0x137800 by opcode shape and shared with it
//! ([`super::super::pegasus::summon`]); the fodder pass 0x1291D0 is
//! Weevil's 0x10EF80 with the limit 0xCE5 and the card test 0x1293B0.

use super::super::pegasus::summon::{affordable_monster, card_list, find_card, non_monster, zone};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{
    affordable, kind_of, monster_choice, neighbour, virt, FodderFilter, Hand, MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{
    goal_cell, graveyard_best, hidden_value, id, own_strongest_holds, ritual_ok, REBORN_ATK_LIMIT,
};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x128FF0 / 0x1291D0: 0x27A850 scoring at every tier, fodder limit
/// 0xCE5, non-monsters and the cards 0x6C, 0x6F and 0x68 never fodder
/// (0x1293B0).
const NECROMANCER_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0xCE5,
    fodder_filter: FodderFilter::NonMonsterOr(&[id::PUMPKING, id::ESCORT, id::NO_FODDER]),
    skip_root: 999,
};

/// 0x129610: the route limit of the summon cell's 0x27BA70 and 0x27AC00
/// searches.
const SUMMON_ROUTE_LIMIT: i32 = 5;
/// 0x129610: the distance limit of the cell nearest the goal.
const NEAR_LIMIT: i32 = 0xE;
/// The Labyrinth terrain (8) and Crush (9).
const LABYRINTH: i32 = 8;
const CRUSH: i32 = 9;

/// 0x128990(side, U): what to summon: 0 a monster (U set by the caller's
/// 0x128FF0 unless set here), 1 a non-monster (U set), 2 a particular card
/// (U set by 0x128D00), −1 nothing. In order: monster zone full → the
/// non-monster choice; magic/trap zone full → the first affordable
/// monster; card 0x2BB in hand while the leader is threatened (0x27D500,
/// see-all 0) or the own strongest monster holds (0x12B8D0); Pumpking in
/// hand; card 0x2AD in hand while a graveyard monster has ATK 2400 or more
/// (0x27F380 mode 3); the ritual 0x353 in hand with acceptable tributes
/// (0x12C180); with an empty monster zone the first affordable monster
/// (without writing U); the first affordable monster; the non-monster
/// choice. A found card that a later test rejects leaves U and the local
/// hand index as the search wrote them.
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
    if find_card(w, side, &mut local, u, id::CARD_2BB)? >= 0
        && (w.threatened(side, 0, false)? || own_strongest_holds(w, side)? == 1)
    {
        return Ok(2);
    }
    if find_card(w, side, &mut local, u, id::PUMPKING)? >= 0 {
        return Ok(2);
    }
    let v = graveyard_best(w, side, 3);
    if v >= 0
        && v & 0xFFFF >= REBORN_ATK_LIMIT
        && find_card(w, side, &mut local, u, id::REBORN)? >= 0
    {
        return Ok(2);
    }
    if find_card(w, side, &mut local, u, id::RITUAL_353)? >= 0
        && ritual_ok(w, side, id::RITUAL_353)? == 1
    {
        return Ok(2);
    }
    if monsters == 0 {
        // 0x128BC4: the first affordable monster, without 0x1DDFE0 (the
        // caller's 0x128FF0 writes U).
        for i in 0..w.next_hand_size(side) {
            let c = hand.get(i)?;
            if kind_of(w, c) >> 5 == 0 && affordable(w, side, c)? {
                return Ok(0);
            }
        }
    }
    if let Some(c) = affordable_monster(w, side, &hand)? {
        *u = Some(virt(w, c));
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// 0x129610(side, _, U, tgt): the summon cell among the leader's
/// neighbours (0x24FD30), by U's class:
/// - a monster with a target: the empty neighbour (not Labyrinth, nor
///   Crush when it would crush U, 0x251760) with the fewest moves to the
///   target: 0x27BA70 (see-all 0, at most 5, hidden strength
///   0x12BA80(side) & 0xFFFF) when 0x279E50 (strength 0x12BA80(side),
///   see-all 0) is positive, else 0x27AC00 (at most 5), counted only when
///   the step buffer holds a candidate (filled once, never reset); −1 when
///   none has a route;
/// - a monster without a target: the empty neighbour (as above) nearest
///   its own [`goal_cell`] (Manhattan below 14, first wins);
/// - non-monsters: the last empty neighbour that is neither Labyrinth nor
///   a Crush that would crush U (its row is always below 8).
fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
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
    if tgt < 0 {
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
            let goal = goal_cell(row);
            let (gc, gr) = (goal & 0xFF, goal >> 8);
            let d = (row.max(gr) - row.min(gr)) + (col.max(gc) - col.min(gc));
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
        let hidden = hidden_value(w, side);
        let m = w.margin_seen(u, tgt, hidden, tc, tr, false)?;
        let d = if m > 0 {
            let hidden = hidden_value(w, side) & 0xFFFF;
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
    Ok(if best == 0xFFFF { -1 } else { best })
}

/// 0x128700(key, side, d): fill `d` with a summon (type 0xA) or return −1:
/// summon flag clear, both zones full, no category. The target search
/// 0x278850 runs with see-all 0 and the hidden strength 0x12BA80(side).
/// Category 0: 0x128FF0; without a target and with five monsters (never,
/// as the category then is 1) the non-monster choice (filtered by the
/// chosen card) takes over. Category 2: 0x129420 lists the card and its
/// power-ups. Category 1: 0x128DF0 filtered by U's card. Then 0x129610.
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
    // 0x278850(U, col, row, other, 3, 0x1F, 2, 2, [see-all 0, 0x12BA80(side)]).
    let target = |u: &Option<Unit>| -> R<i32> {
        let hidden = hidden_value(w, side);
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
        None => unsupported("0x128700 reads an uninitialised U"),
    };
    let tgt;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, NECROMANCER_CHOICE)? < 0 {
            // Unreachable (0x128990 found an affordable monster): the
            // original falls into the non-monster path with an
            // uninitialised target register.
            return unsupported("0x128700 uses an uninitialised target");
        }
        tgt = target(&u)?;
        if tgt < 0 && w.monsters(side) >= 5 {
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
