//! Manawyddan fab Llyr's summon planner 0x164CC0 (York's 0x13B440 by
//! opcode shape, with see-all 1 in the target search) and its subtree: the
//! category choice 0x164F50, the trap search 0x1652D0 and the summon cell
//! 0x165BC0 are his own; the hand search 0x1651E0, the non-monster choice
//! 0x1653B0, the monster choice 0x1655B0 and the card list 0x1659D0 are
//! Pegasus's 0x137160 / 0x137250 / 0x137450 / 0x137800 by opcode shape and
//! shared with it ([`super::super::pegasus::summon`]); the fodder pass
//! 0x165790 is Weevil's 0x10EF80 with York's card test 0x165970 (=
//! 0x13C0B0) and a `bnez` on 2·ATK + DEF where Weevil has `sltiu 0xDAD`,
//! i.e. only a card with 2·ATK + DEF = 0 may be fodder.

use super::super::pegasus::summon::{
    affordable_monster, card_list, category_of, find_card, non_monster, zone,
};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{
    kind_of, monster_choice, neighbour, virt, FodderFilter, Hand, MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData, Opponent};
use super::{goal_cell, id, ritual_ok, support_cell, Manawyddan, CRUSH};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x1655B0 / 0x165790: 0x27A850 scoring at every tier, non-monsters and
/// monsters with effect entries never fodder, and a `bnez` on 2·ATK + DEF
/// (0x1658D0) in place of Weevil's `sltiu 0xDAD`: the limit 1.
const MANAWYDDAN_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 1,
    fodder_filter: FodderFilter::NonMonsterOrEffect,
    skip_root: 999,
};

/// 0x164CC0: the monster count (0x1E13D0) from which a target-less monster
/// summon turns into the non-monster choice (`slti 6`: never, the zone
/// holds five).
const MONSTERS_FOR_NON_MONSTER: i32 = 6;
/// 0x164CC0: the depth of the summon target search 0x278850 (3 in
/// Pegasus's 0x136AE0, 2 in York's 0x13B440).
const TARGET_DEPTH: i32 = 2;
/// 0x165BC0: the route limit of the target searches (0x27BA70 / 0x27AC00)
/// and of the goal searches (0x27AC00).
const TARGET_ROUTE_LIMIT: i32 = 5;
const GOAL_ROUTE_LIMIT: i32 = 7;
/// 0x165BC0: the distance below which a neighbour counts for a goal.
const GOAL_DISTANCE_LIMIT: i32 = 14;
/// The Labyrinth terrain (8).
const LABYRINTH: i32 = 8;

/// 0x1652D0(side, D, U): the first predicted-hand index holding a trap
/// (0x1DF290 of its class = 2, no affordability test): D+6 = index, U =
/// 0x1DDFE0(card), returns the card's class; −1 when none.
fn trap_card<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
) -> R<i32> {
    let hand = Hand::of(w, side);
    for i in 0..w.next_hand_size(side) {
        let c = hand.get(i)?;
        if category_of(kind_of(w, c) >> 5) != 2 {
            continue;
        }
        d.hand[0] = i as u8;
        *u = Some(virt(w, c));
        return Ok(kind_of(w, c) >> 5);
    }
    Ok(-1)
}

/// 0x164F50(side, U): what to summon: 0 a monster (U set by the caller's
/// 0x1655B0 unless set here), 1 a non-monster (U set), 2 a particular card
/// (U set by 0x1651E0 / 0x1652D0), −1 nothing. In order: monster zone
/// full → the non-monster choice; magic/trap zone full → the first
/// affordable monster; Curse of Tri-Horned Dragon (0x34A), then Cosmo
/// Queen's Prayer (0x352), in hand with choosable tributes (0x167F40); the
/// first trap in hand ([`trap_card`]); the first affordable monster; the
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
    for ritual in [id::RITUAL_34A, id::RITUAL_352] {
        if find_card(w, side, &mut local, u, ritual)? >= 0 && ritual_ok(w, side, ritual)? == 1 {
            return Ok(2);
        }
    }
    if trap_card(w, side, &mut local, u)? >= 0 {
        return Ok(2);
    }
    if let Some(c) = affordable_monster(w, side, &hand)? {
        *u = Some(virt(w, c));
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// The neighbour of the side's leader (0x24FD30) nearest `goal` by route
/// (0x27AC00, at most 7 moves), among those that are empty (0x1E4A10),
/// not Labyrinth and, on Crush, would not crush U (0x251760): the lowest
/// non-negative length below 14 wins (strict, first wins); −1 when none.
/// The route's allowed occupant is the goal's (0x1E4A10 at 0x165F28) when
/// `goal_occupant` is set, the monster branch; the support branch
/// (0x1660EC) asks 0x1E4A10 about the neighbour again, which it has just
/// found empty, so it always passes 0xFFFF. The step buffer `buf` is
/// shared with the caller's other searches and never reset.
fn nearest_to<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
    buf: &mut Steps,
    goal_occupant: bool,
    goal: impl Fn(i32, i32) -> i32,
) -> R<i32> {
    let mut best = -1;
    let mut best_d = GOAL_DISTANCE_LIMIT;
    for i in 0..8 {
        let cell = neighbour(w, side, i)?;
        if cell < 0 {
            continue;
        }
        let (col, row) = (cell & 0xFF, cell >> 8);
        let t = w.terrain(col, row);
        if t == LABYRINTH || (t == CRUSH && w.crushed(u, col, row)?) || w.at_key(col, row) >= 0 {
            continue;
        }
        let g = goal(col, row);
        let (gc, gr) = (g & 0xFF, g >> 8);
        let occ = match w.at_key(gc, gr) {
            k if k >= 0 && goal_occupant => k & 0xFFFF,
            _ => 0xFFFF,
        };
        let d = path(w, Some(u), buf, col, row, gc, gr, occ, GOAL_ROUTE_LIMIT)?;
        if d < best_d && d >= 0 {
            best = cell;
            best_d = d;
        }
    }
    Ok(if best_d < GOAL_DISTANCE_LIMIT { best } else { -1 })
}

/// 0x165BC0(side, _, U, tgt): the summon cell among the leader's
/// neighbours (0x24FD30) that are empty, not Labyrinth and, on Crush,
/// would not crush U (0x251760); −1 without U.
/// - A monster or ritual (class 0 or 5) with a target: the neighbour with
///   the fewest moves to it: 0x27BA70 (see-all 1, at most 5, hidden
///   strength 0x167FA0 = 0) when 0x279E50 (strength 0, see-all 1) is
///   positive, else 0x27AC00 (at most 5), counted only when the step
///   buffer holds a candidate; the buffer is filled once and never reset
///   between neighbours, so a failed search keeps the previous
///   neighbour's candidates and its −1 wins. When no neighbour has a
///   route it falls through to the goal search.
/// - A monster or ritual without a target: the neighbour nearest
///   ([`nearest_to`]) its own [`goal_cell`].
/// - Other classes: for each of the five [`support_cell`]s in turn, the
///   neighbour nearest it with no occupant allowed (0xFFFF); the first
///   support cell with one wins, else −1.
fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
    tgt: i32,
) -> R<i32> {
    // sp+0x100: the step buffer.
    let mut buf = Steps::new();
    if !matches!(u.kind >> 5, 0 | 5) {
        for i in 0..5 {
            let goal = support_cell(i) & 0xFFFF;
            let v = nearest_to(w, side, u, &mut buf, false, |_, _| goal)?;
            if v >= 0 {
                return Ok(v);
            }
        }
        return Ok(-1);
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
            let t = w.terrain(col, row);
            if t == LABYRINTH || (t == CRUSH && w.crushed(u, col, row)?) || w.at(col, row).is_some()
            {
                continue;
            }
            let hidden = Manawyddan::hidden_value(w, side)?;
            let m = w.margin_seen(u, tgt, hidden, tc, tr, true)?;
            let d = if m > 0 {
                let hidden = Manawyddan::hidden_value(w, side)? & 0xFFFF;
                attack_path(
                    w,
                    Some(u),
                    &mut buf,
                    col,
                    row,
                    tc,
                    tr,
                    tgt as u32,
                    1,
                    TARGET_ROUTE_LIMIT,
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
    nearest_to(w, side, u, &mut buf, true, goal_cell)
}

/// 0x164CC0(key, side, d): fill `d` with a summon (type 0xA) or return −1:
/// summon flag clear, both zones full, no category. The target search
/// 0x278850 runs with depth 2, see-all 1 and the hidden strength
/// 0x167FA0(side) = 0. Category 0: 0x1655B0; without a target and with six
/// or more monsters (never) the non-monster choice (filtered by the chosen
/// card) takes over. Category 2: 0x1659D0 lists the card and its
/// power-ups. Category 1: 0x1653B0 filtered by U's card. Then 0x165BC0.
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
    // 0x278850(U, col, row, other, 2, 0x1F, 2, 2, [see-all 1, 0x167FA0(side)]).
    let target = |u: &Option<Unit>| -> R<i32> {
        let hidden = Manawyddan::hidden_value(w, side)?;
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
            1,
            hidden,
        )
    };
    let with_u = |u: &Option<Unit>| match u {
        Some(u) => Ok(u.clone()),
        None => unsupported("0x164CC0 reads an uninitialised U"),
    };
    let tgt;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, MANAWYDDAN_CHOICE)? < 0 {
            // Unreachable (0x164F50 found an affordable monster): the
            // original falls into the non-monster path with an
            // uninitialised target register.
            return unsupported("0x164CC0 uses an uninitialised target");
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
