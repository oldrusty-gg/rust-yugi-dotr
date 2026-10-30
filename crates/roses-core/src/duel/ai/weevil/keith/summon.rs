//! Keith's summon planner 0x11E880 and its subtree: the category choice
//! 0x11EFB0, the special-card search 0x11F3E0, the non-monster choice
//! 0x11F620 (Pegasus's 0x137250 with one card never taken on its last
//! pass), the monster choice 0x11F830 (Pegasus's 0x137450 with one root
//! skipped) with Weevil's fodder pass 0x11FA20 (filter 0x11FC00), the card
//! list 0x11FC90 and the summon cell 0x11EB40. The predicted hand, the
//! fusion tree and the tribute solver are the shared ones of
//! [`super::super::summon`].

use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{
    affordable, equip, kind_of, monster_choice, neighbour, virt, FodderFilter, Hand, MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{hidden_value, id, lane_cell, machines};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x11F830 / 0x11FA20: 0x27A850 scoring at every tier, root 0x156
/// skipped, Weevil's fodder limit 0xDAD with the filter 0x11FC00 (any
/// non-monster, or 0x1F0, 0x1FD, 0x1FE, 0x202, 0x1F6, 0x20B).
const KEITH_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0xDAD,
    fodder_filter: FodderFilter::NonMonsterOr(&[
        id::DEFENDER_B,
        id::ESCORT,
        id::DEFENDER_A,
        id::MACHINE_KING,
        id::NO_FODDER_A,
        id::NO_FODDER_B,
    ]),
    skip_root: id::SKIPPED_MONSTER,
};

/// 0x11EB40: the depth of its target search and the route limit of its
/// cell searches.
const CELL_SEARCH_DEPTH: i32 = 4;
const CELL_ROUTE_LIMIT: i32 = 4;
/// 0x11EB40: the route limit of the lane-goal fallback (`slti 0xE`).
const LANE_LEN_LIMIT: i32 = 0xE;

/// 0x11F3E0(side, d, U, a3). `a3` = 1: the first hand index holding
/// 0x202 that is affordable (0x271440): d+6 = index, U = the card,
/// returns its class. `a3` = 0: d+6 = the LAST hand index holding 0x31F;
/// with one, d+7 = the FIRST monster index that takes it (0x24A880 ≥ 0),
/// is affordable and is not 0x31B / 0x31C: U = that monster, returns its
/// class (0). −1 otherwise (d+6 may already be written), and for other
/// `a3`.
fn find_special<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
    a3: i32,
) -> R<i32> {
    let hand = Hand::of(w, side);
    let n = w.next_hand_size(side);
    match a3 & 0xFF {
        1 => {
            for i in 0..n {
                let c = hand.get(i)?;
                if c != id::MACHINE_KING || !affordable(w, side, c)? {
                    continue;
                }
                d.hand[0] = i as u8;
                *u = Some(virt(w, c));
                return Ok(kind_of(w, c) >> 5);
            }
            Ok(-1)
        }
        0 => {
            let mut found = false;
            for i in 0..n {
                if hand.get(i)? == id::PIECE_POWER_UP {
                    found = true;
                    d.hand[0] = i as u8;
                }
            }
            if !found {
                return Ok(-1);
            }
            for j in 0..n {
                let c = hand.get(j)?;
                if kind_of(w, c) >> 5 != 0 {
                    continue;
                }
                if equip(w, c, id::PIECE_POWER_UP)? < 0 {
                    continue;
                }
                if !affordable(w, side, c)? {
                    continue;
                }
                if c == id::POTION || c == id::CURSEBREAKER {
                    continue;
                }
                d.hand[1] = j as u8;
                *u = Some(virt(w, c));
                return Ok(kind_of(w, c) >> 5);
            }
            Ok(-1)
        }
        _ => Ok(-1),
    }
}

/// 0x11F620(side, d, U, filter): d+6.. = 0xFF; the first hand index
/// (ascending) of, in this order: the card `filter` (unless 999), a card
/// of a class other than 0 and 2 (magic, trap, ritual), any non-monster
/// other than 0x31F. d+6 = index, U = the card, returns 1; −1 when there
/// is none.
fn non_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
    filter: i32,
) -> R<i32> {
    let hand = Hand::of(w, side);
    d.hand = [0xFF; 5];
    let n = w.next_hand_size(side);
    for pass in 0..3 {
        for i in 0..n {
            let c = hand.get(i)?;
            let class = kind_of(w, c) >> 5;
            let hit = match pass {
                0 => c == filter && filter != 999,
                1 => class != 0 && class != 2,
                _ => class != 0 && c != id::PIECE_POWER_UP,
            };
            if hit {
                d.hand[0] = i as u8;
                *u = Some(virt(w, c));
                return Ok(1);
            }
        }
    }
    Ok(-1)
}

/// 0x11FC90(side, d, U, a3): d+6.. = 0xFF, then [`find_special`](a3 = 0)
/// for both `a3` = 0 and `a3` = 1 (quirk: the 0x202 choice of the
/// category's answer 3 is not searched again); for `a3` = 0 also d+8 =
/// the LAST other hand index holding a power-up (class 2, not 0x31B /
/// 0x31C) that fits 0x1E8 (0x24A880 ≥ 0). Returns 0 (−1 without U).
fn card_list<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
    a3: i32,
) -> R<i32> {
    if u.is_none() {
        return Ok(-1);
    }
    d.hand = [0xFF; 5];
    let hand = Hand::of(w, side);
    match a3 & 0xFF {
        0 => {
            find_special(w, side, d, u, 0)?;
            for k in 0..w.next_hand_size(side) {
                if k == i32::from(d.hand[0]) || k == i32::from(d.hand[1]) {
                    continue;
                }
                let c = hand.get(k)?;
                if equip(w, id::LIST_MONSTER, c)? < 0 {
                    continue;
                }
                if kind_of(w, c) >> 5 != 2 || c == id::POTION || c == id::CURSEBREAKER {
                    continue;
                }
                d.hand[2] = k as u8;
            }
        }
        1 => {
            find_special(w, side, d, u, 0)?;
        }
        _ => (),
    }
    Ok(0)
}

/// The first affordable monster of the predicted hand, skipping `skip`
/// (999 = none), as a card id (the loops of 0x11EFB0).
fn affordable_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    hand: &Hand,
    skip: i32,
) -> R<Option<i32>> {
    for i in 0..w.next_hand_size(side) {
        let c = hand.get(i)?;
        if kind_of(w, c) >> 5 == 0 && affordable(w, side, c)? && c != skip {
            return Ok(Some(c));
        }
    }
    Ok(None)
}

/// 0x11EFB0's second loop (0x11F214): over slots 1..=10 of group `side`
/// and the other group, hidden units; a 0x337 on the field resets the
/// count and ends the loop. Group `side`: a unit the side controls is
/// tested for 0x337, any other one is counted when its +0x57 bit 4 is
/// clear; the other group the other way round (controlled → counted when
/// hidden, else tested for 0x337). The 0x337 found in group `side` makes
/// the other group's test read slot 11 (no unit).
fn hidden_count<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    let other = i32::from(side == 0);
    let mut count: u8 = 0;
    let mut slot = 1;
    while slot < 11 {
        if w.ctrl(side, slot) == side {
            if w.card_id(side, slot) == id::HIDDEN_TRAP {
                slot = 0xB;
                count = 0;
            }
        } else if !w.flag(side, slot, 4) {
            count = count.wrapping_add(1);
        }
        if w.ctrl(other, slot) == side {
            if !w.flag(other, slot, 4) {
                count = count.wrapping_add(1);
            }
        } else if w.card_id(other, slot) == id::HIDDEN_TRAP {
            slot = 0xB;
            count = 0;
        }
        slot += 1;
    }
    i32::from(count)
}

/// 0x11EFB0(side, U): what to summon: 0 a monster (U set), 1 a
/// non-monster (U set), 2 the 0x31F pair (U set by 0x11F3E0), 3 card
/// 0x202 (U set), −1 nothing. In order: monster zone full → the
/// non-monster choice; magic/trap zone full → the first affordable
/// monster; the 0x31F pair; with three or more own kind-0xE units
/// ([`machines`]) 0x202 in hand, else 0x2E2 in hand; 0x337 in hand with
/// six or more hidden units ([`hidden_count`]); the first affordable
/// monster other than 0x156; the non-monster choice.
fn category<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &mut Option<Unit>,
) -> R<i32> {
    let monsters = w.monsters(side) & 0xFF;
    let others = w.others(side) & 0xFF;
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
        if let Some(c) = affordable_monster(w, side, &hand, 999)? {
            *u = Some(virt(w, c));
            return Ok(0);
        }
        return Ok(-1);
    }
    if find_special(w, side, &mut local, u, 0)? >= 0 {
        return Ok(2);
    }
    if machines(w, side) >= 3 {
        if find_special(w, side, &mut local, u, 1)? >= 0 {
            return Ok(3);
        }
        let v = non_monster(w, side, &mut local, u, id::MACHINE_MAGIC)?;
        if v >= 0 {
            return Ok(v);
        }
    }
    let v = non_monster(w, side, &mut local, u, id::HIDDEN_TRAP)?;
    if v >= 0 && hidden_count(w, side) >= 6 {
        return Ok(v);
    }
    if let Some(c) = affordable_monster(w, side, &hand, id::SKIPPED_MONSTER)? {
        *u = Some(virt(w, c));
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// The leader's neighbours (0x24FD30) that are empty, not Labyrinth and,
/// on Crush, would not crush U (0x251760), with their cells.
fn free_neighbours<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
) -> R<Vec<i32>> {
    let mut out = Vec::new();
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
        out.push(cell);
    }
    Ok(out)
}

/// 0x11EB40(side, _, U): the summon cell. Non-monsters take the last free
/// neighbour (0x11EEC0). A monster searches its own target from the
/// leader's cell (0x278850 depth 4, filter 0x1F, see-all 0, hidden
/// 0x120350(side)): with one, the free neighbour with the fewest moves to
/// it (0x27BA70, see-all 0, at most 4, when the 0x279E50 margin with that
/// hidden strength is positive, else 0x27AC00 at most 4; counted only
/// when the step buffer, shared and never reset, holds a candidate;
/// strict < from 0xFF, a negative length wins); without one, the free
/// neighbour whose nearest lane goal ([`lane_cell`]) is the fewest moves
/// away (strict < from 14). −1 when none.
fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32, u: &Unit) -> R<i32> {
    let mut buf = Steps::new();
    if u.kind >> 5 != 0 {
        let mut last = -1;
        for cell in free_neighbours(w, side, u)? {
            if cell >> 8 < 8 {
                last = cell;
            }
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
        for cell in free_neighbours(w, side, u)? {
            let (col, row) = (cell & 0xFF, cell >> 8);
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
            if d < best_d {
                best_d = d;
                best = cell;
            }
        }
        return Ok(if best == 0xFFFF { -1 } else { best });
    }
    let mut best_len = LANE_LEN_LIMIT;
    let mut best = -1;
    for cell in free_neighbours(w, side, u)? {
        let (col, row) = (cell & 0xFF, cell >> 8);
        let (_, len) = lane_cell(w, Some(u), col, row)?;
        if i32::from(len) < best_len {
            best = cell;
            best_len = i32::from(len);
        }
    }
    Ok(if best_len < LANE_LEN_LIMIT { best } else { -1 })
}

/// 0x11E880(key, side, d): fill `d` with a summon (type 0xA) or return
/// −1: summon flag clear, both zones full, no category. Category 0:
/// 0x11F830, then the target search 0x278850 (see-all 0, hidden
/// 0x120350(side)) from the key's cell: without a target and with four or
/// more monsters, or when no root was affordable, the non-monster choice
/// (filtered by U's card) takes over. Categories 2 and 3: 0x11FC90 with
/// 0 / 1. Category 1: 0x11F620 filtered by U's card. Then 0x11EB40.
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
    if w.monsters(side) == 5 && w.others(side) == 5 {
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
    // 0x278850(U, col, row, other, 3, 0x1F, 2, 2, [0, 0x120350(side)]).
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
        None => unsupported("0x11E880 reads an uninitialised U"),
    };
    let mut non_monster_path = false;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, KEITH_CHOICE)? < 0 {
            non_monster_path = true;
        } else {
            let tgt = target(&u)?;
            if tgt < 0 && w.monsters(side) >= 4 {
                non_monster_path = true;
            }
        }
    } else {
        target(&u)?;
        match r {
            2 => {
                card_list(w, side, d, &mut u, 0)?;
            }
            3 => {
                card_list(w, side, d, &mut u, 1)?;
            }
            _ => non_monster_path = true,
        }
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
