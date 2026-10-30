//! Labyrinth-ruler's summon planner 0x132D30 and its subtree: the category
//! choice 0x132FC0 and the summon cell 0x133BF0 are his own; the hand
//! search 0x1332E0, the non-monster choice 0x1333D0, the monster choice
//! 0x1335D0 and the card list 0x133A00 are Pegasus's 0x137160 / 0x137250
//! / 0x137450 / 0x137800 by opcode shape and shared with it
//! ([`super::super::pegasus::summon`]); the fodder pass 0x1337B0 is
//! Weevil's 0x10EF80 with the limit 0x10CD and the card test 0x133990.

use super::super::pegasus::summon::{affordable_monster, card_list, find_card, non_monster, zone};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{monster_choice, neighbour, virt, FodderFilter, Hand, MonsterChoice};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{goal, hidden_value, id, ritual_ok, CRUSH, LABYRINTH, SUMMON_ROUTE_LIMIT};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x1335D0 / 0x1337B0: 0x27A850 scoring at every tier, fodder limit
/// 0x10CD, the Gate Guardian pieces and non-monsters never fodder.
const LABYRINTH_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0x10CD,
    fodder_filter: FodderFilter::GateGuardian,
    skip_root: 999,
};

/// 0x132FC0(side, U): what to summon: 0 a monster (U set by the caller's
/// 0x1335D0 unless set here), 1 a non-monster (U set), 2 a particular card
/// (U set by 0x1332E0), −1 nothing. In order: monster zone full → the
/// non-monster choice; magic/trap zone full → the first affordable
/// monster; Gate Guardian Ritual in hand with choosable tributes
/// (0x1358D0); Shift while a Gate Guardian the side controls stands in
/// the slots 1..=5 of either group; the first affordable monster; the
/// non-monster choice.
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
    if find_card(w, side, &mut local, u, id::GATE_GUARDIAN_RITUAL)? >= 0
        && ritual_ok(w, side, id::GATE_GUARDIAN_RITUAL)? == 1
    {
        return Ok(2);
    }
    let other = i32::from(side == 0);
    for slot in 1..6 {
        for grp in [side, other] {
            if w.unit(grp, slot).is_some()
                && w.card_id(grp, slot) == id::GATE_GUARDIAN
                && w.ctrl(grp, slot) == side
                && find_card(w, side, &mut local, u, id::SHIFT)? >= 0
            {
                return Ok(2);
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

/// 0x133BF0(side, _, U, tgt): the summon cell among the leader's
/// neighbours (0x24FD30) that are empty, not Labyrinth and, on Crush,
/// would not crush U (0x251760).
/// - A monster with a target: the neighbour with the fewest moves to it:
///   0x27BA70 (see-all 0, at most 4, hidden strength 0x1353B0(side) &
///   0xFFFF) when 0x279E50 (strength 0x1353B0(side), see-all 0) is
///   positive, else 0x27AC00 (at most 4), counted only when the step
///   buffer holds a candidate; the buffer is filled once and never reset
///   between neighbours, so a failed search keeps the previous neighbour's
///   candidates and its −1 wins. −1 when no neighbour has a route.
/// - Otherwise (non-monsters, monsters without a target): the neighbour
///   whose Manhattan distance to its own [`goal`] is the lowest below 14
///   (strict, first wins; a goal in the enemy leader's row with no leader
///   gives a negative row); −1 when none.
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
    if u.kind >> 5 == 0 && tgt >= 0 {
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
        return Ok(if best == 0xFFFF { -1 } else { best });
    }
    let mut best = -1;
    let mut best_d = 14;
    for i in 0..8 {
        let cell = neighbour(w, side, i)?;
        if cell < 0 {
            continue;
        }
        let (col, row) = (cell & 0xFF, cell >> 8);
        if !open(col, row)? || w.at_key(col, row) >= 0 {
            continue;
        }
        let v = goal(w, u, col, row)?;
        let (vc, vr) = (v & 0xFF, v >> 8);
        let d = (row.max(vr) - row.min(vr)) + (col.max(vc) - col.min(vc));
        if d < best_d && d >= 0 {
            best = cell;
            best_d = d;
        }
    }
    Ok(if best_d < 14 { best } else { -1 })
}

/// 0x132D30(key, side, d): fill `d` with a summon (type 0xA) or return −1:
/// summon flag clear, both zones full, no category. The target search
/// 0x278850 runs with see-all 0 and the hidden strength 0x1353B0(side).
/// Category 0: 0x1335D0; without a target and with five monsters (never,
/// as the category then is 1) the non-monster choice (filtered by the
/// chosen card) takes over. Category 2: 0x133A00 lists the card and its
/// power-ups. Category 1: 0x1333D0 filtered by U's card. Then 0x133BF0.
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
    // 0x278850(U, col, row, other, 3, 0x1F, 2, 2, [see-all 0, 0x1353B0(side)]).
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
        None => unsupported("0x132D30 reads an uninitialised U"),
    };
    let tgt;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, LABYRINTH_CHOICE)? < 0 {
            // Unreachable (0x132FC0 found an affordable monster): the
            // original falls into the non-monster path with an
            // uninitialised target register.
            return unsupported("0x132D30 uses an uninitialised target");
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
