//! Rex Raptor's summon planner 0x119BD0 and its subtree: the category
//! choice 0x11A310, the non-monster choice 0x11A600 (= Pegasus's
//! 0x137250), the monster choice 0x11A9A0 (= Weevil's 0x10EBF0) with the
//! fodder pass 0x11ABC0 (Weevil's 0x10EF80 without the Cocoon test), the
//! counter-card search 0x11A800 ([`super::type_card`]) and the summon cell
//! 0x119DF0. The predicted hand, the fusion tree and the tribute solver
//! are the shared ones of [`super::super::summon`].

use super::super::pegasus::summon::{affordable_monster, non_monster, zone};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{monster_choice, neighbour, virt, FodderFilter, Hand, MonsterChoice};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{hidden_value, lane_free, lane_goal, type_card, GOAL_ROW_A};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x11A9A0 / 0x11ABC0: Weevil's 0x10EBF0 (0x27AA70 below tier 3) with
/// the fodder limit 0xDAD and no card test in the fodder pass.
const REX_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: false,
    fodder_limit: 0xDAD,
    fodder_filter: FodderFilter::None,
    skip_root: 999,
};

/// 0x119DF0: the depth of its target search and the route limit of its
/// cell searches.
const CELL_SEARCH_DEPTH: i32 = 3;
const CELL_ROUTE_LIMIT: i32 = 3;
/// 0x119DF0: the route limit of the lane-goal search (0x27AC00) and the
/// bound of its best length (`slti 0xE`).
const GOAL_ROUTE_LIMIT: i32 = 7;
const GOAL_LEN_LIMIT: i32 = 0xE;
/// 0x119BD0: the monster count that diverts a target-less monster to the
/// non-monster choice (`slti 6`, never reached with five monster slots).
const DIVERT_MONSTERS: i32 = 6;

/// 0x11A310(side, U): what to summon: 0 a monster (U set by the caller's
/// 0x11A9A0, or here), 1 a non-monster (U set), −1 nothing. In order:
/// monster zone full → the non-monster choice; magic/trap zone full → the
/// first affordable monster; the counter card of the strongest enemy
/// ([`type_card`]) → the non-monster choice filtered by it, whatever it
/// finds; with a lane HOLDING an own monster (the 0x11A474 loop stops at
/// the first [`lane_free`] = 0, `bnel` at 0x11A480) the first affordable
/// monster (U left to 0x11A9A0), else the non-monster choice as is; with
/// every lane free the first affordable monster (U set), else the
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
    let counter = type_card(w, side, &mut local)?;
    if counter >= 0 {
        return non_monster(w, side, &mut local, u, counter);
    }
    if (0..4).any(|i| lane_free(w, side, i) == 0) {
        if affordable_monster(w, side, &hand)?.is_some() {
            return Ok(0);
        }
        return non_monster(w, side, &mut local, u, 999);
    }
    if let Some(c) = affordable_monster(w, side, &hand)? {
        *u = Some(virt(w, c));
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// 0x119DF0(side, _, U): the summon cell among the leader's neighbours
/// (0x24FD30), or −1.
/// - Non-monsters: the LAST empty neighbour whose terrain is neither 8 nor
///   9 (no crush test).
/// - Monsters with a target (0x278850 from the leader's cell, depth 3,
///   filter 0x1F, see-all 0, hidden 0x11B2E0 = 0): among the empty
///   neighbours (terrain 8 excluded, Crush only when U would be crushed)
///   the one with the fewest moves to the target: 0x27BA70 (see-all 0, at
///   most 3) when the 0x279E50 margin is positive, else 0x27AC00 (at most
///   3); counted only when the step buffer (shared, never reset) holds a
///   candidate; strict < from 0xFF, negative lengths skipped.
/// - Monsters without one: the row-1 lane goal (0x1196A0 for the
///   summoning side; (255, −1) when there is none) and, among the same
///   neighbours (own occupant test repeated: the enterable key is the
///   neighbour's own, always 0xFFFF), the one with the fewest moves to it
///   (0x27AC00, at most 7; strict < from 14, negative skipped).
fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32, u: &Unit) -> R<i32> {
    let mut buf = Steps::new();
    if u.kind >> 5 != 0 {
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
    let lp = w.pos(side, 0);
    let other = i32::from(side == 0);
    let hidden = hidden_value(w, side);
    let tgt = find_target(
        w,
        Some(u),
        lp & 0xFF,
        lp >> 8,
        other,
        CELL_SEARCH_DEPTH,
        0x1F,
        2,
        2,
        0,
        hidden,
    )?;
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
            if t == 8 || (t == 9 && w.crushed(u, col, row)?) || w.at(col, row).is_some() {
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
                    CELL_ROUTE_LIMIT,
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
                    CELL_ROUTE_LIMIT,
                )?
            };
            if step_count(&buf) <= 0 {
                continue;
            }
            if d < best_d && d >= 0 {
                best_d = d;
                best = cell;
            }
        }
        return Ok(if best == 0xFFFF { -1 } else { best });
    }
    let goal = lane_goal(w, side, GOAL_ROW_A, true);
    let (gc, gr) = (goal & 0xFF, goal >> 8);
    let mut best_len = GOAL_LEN_LIMIT;
    let mut best = -1;
    for i in 0..8 {
        let cell = neighbour(w, side, i)?;
        if cell < 0 {
            continue;
        }
        let (col, row) = (cell & 0xFF, cell >> 8);
        let t = w.terrain(col, row);
        if t == 8 || (t == 9 && w.crushed(u, col, row)?) || w.at_key(col, row) >= 0 {
            continue;
        }
        // 0x11A1BC: the enterable occupant is read from the neighbour
        // itself (just found empty), not from the goal.
        let occ = match w.at_key(col, row) {
            k if k < 0 => 0xFFFF,
            k => k,
        };
        let d = path(
            w,
            Some(u),
            &mut buf,
            col,
            row,
            gc,
            gr,
            occ & 0xFFFF,
            GOAL_ROUTE_LIMIT,
        )?;
        if d < best_len && d >= 0 {
            best = cell;
            best_len = d;
        }
    }
    Ok(if best_len < GOAL_LEN_LIMIT { best } else { -1 })
}

/// 0x119BD0(key, side, d): fill `d` with a summon (type 0xA) or return
/// −1: summon flag clear, both zones full, no category. Category 0:
/// 0x11A9A0, then the target search 0x278850 (see-all 0, hidden 0x11B2E0)
/// from the key's cell; without a target and with six or more monsters
/// (never), or when no root was affordable, the non-monster choice
/// (filtered by U's card) takes over. Category 1: the target search (its
/// result dropped) and 0x11A600 filtered by U's card. Then 0x119DF0.
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
    // 0x278850(U, col, row, other, 3, 0x1F, 2, 2, [0, 0x11B2E0(side)]).
    let target = |u: &Option<Unit>| {
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
        None => unsupported("0x119BD0 reads an uninitialised U"),
    };
    let mut non_monster_path = true;
    if r == 0 && monster_choice(w, side, d, &mut u, REX_CHOICE)? >= 0 {
        let tgt = target(&u)?;
        non_monster_path = tgt < 0 && w.monsters(side) >= DIVERT_MONSTERS;
    } else if r != 0 {
        target(&u)?;
    }
    if non_monster_path {
        let card = i32::from(with_u(&u)?.card);
        if non_monster(w, side, d, &mut u, card)? < 0 {
            return Ok(-1);
        }
    }
    let c = cell(w, side, &with_u(&u)?)?;
    if c < 0 {
        return Ok(-1);
    }
    d.target = c as u16;
    Ok(1)
}
