//! Tea's summon planner 0x13F7B0 and its subtree: the category choice
//! 0x13FA40, the Insect Imitation pair 0x13FC80, the monster choice
//! 0x140120 and the summon cell 0x140780 are her own; the hand search
//! 0x13FE10 is Darkness-ruler's 0x12E3B0 (= Pegasus's 0x137160, shared:
//! [`find_card`]); the non-monster choice 0x13FF10 is Keith's 0x11F620
//! with Insect Imitation (0x320) as the card its last pass skips; the card
//! list 0x140590 is Pegasus's 0x137800 without the affordability test
//! ([`card_list_with`]); the fodder pass 0x140360 is Weevil's 0x10EF80
//! with the limit 0xC1D and Darkness-ruler's card test 0x140540 =
//! 0x12EA70 (every card with a record is kept, so it never adds fodder).

use super::super::pegasus::summon::{affordable_monster, card_list_with, find_card, zone};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{
    affordable, kind_of, monster_choice_with, neighbour, virt, ChoiceExtra, FodderFilter, Hand,
    MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData, EQUIP_RANDOM};
use super::{grass_cell, id, residue_u, weak_trio, GOAL_DISTANCE_LIMIT};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x140120 / 0x140360: 0x27A850 scoring at every tier, fodder limit
/// 0xC1D, every recorded card refused as fodder (0x140540).
const TEA_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0xC1D,
    fodder_filter: FodderFilter::Recorded,
    skip_root: 999,
};

/// 0x140120: the fusion tree (0x27A3D0) only from tier 3 (below it
/// 0x27A5C0, the card alone), the fodder pass only from tier 4 (both
/// through 0x23BCD0).
const TEA_EXTRA: ChoiceExtra = ChoiceExtra {
    tree_from_tier: 3,
    fodder_from_tier: 4,
    ..ChoiceExtra::NONE
};

/// 0x13F7B0: the depth of the target search 0x278850.
const TARGET_DEPTH: i32 = 3;
/// 0x140780: the route limit of its 0x27BA70 / 0x27AC00 searches.
const ROUTE_LIMIT: i32 = 2;
/// 0x13F7B0: from this many monsters (0x1E13D0) a target-less monster
/// summon turns into the non-monster choice.
const MONSTERS_FOR_NON_MONSTER: i32 = 4;
/// The Labyrinth terrain (8) and Crush (9).
const LABYRINTH: i32 = 8;
const CRUSH: i32 = 9;

/// 0x13FF10(side, d, U, filter): d+6.. = 0xFF; the first hand index
/// (ascending) of, in this order: the card `filter` (unless 999), a card
/// of a class other than 0 and 2 (magic, trap, ritual), any non-monster
/// other than Insect Imitation. d+6 = index, U = the card, returns 1; −1
/// when there is none.
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
                _ => class != 0 && c != id::INSECT_IMITATION,
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

/// 0x24A880(monster, Insect Imitation) as 0x13FC80 uses it (only the
/// sign): for the five monsters 0x24A960 transforms at random (Ryu-Ran,
/// Manga Ryu-Ran, Monster Egg, Gorgon Egg, Wing Egg Elf) it draws one
/// rand() (0x19AE88, [`World::rand31`]) and answers a card id, never
/// negative; otherwise the table answer.
fn takes_insect_imitation<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    monster: i32,
) -> i32 {
    let e = w.fusions.equip(monster as u16, id::INSECT_IMITATION as u16);
    if e == EQUIP_RANDOM {
        w.rand31();
        return 0;
    }
    e
}

/// 0x13FC80(side, d, U): with Insect Imitation in the predicted hand
/// (each copy: d+7 = its index, U = its record), the first monster index
/// that takes it (0x24A880 ≥ 0), is affordable (0x271490) and is neither
/// Paralyzing Potion nor Cursebreaker: d+7 = that index (quirk: the same
/// byte, so d+6 keeps what it held), U = that monster, returns its class
/// (0). −1 otherwise (d+7 and U may already be written).
fn insect_pair<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
) -> R<i32> {
    let hand = Hand::of(w, side);
    let n = w.next_hand_size(side);
    let mut found = false;
    for i in 0..n {
        let c = hand.get(i)?;
        if c == id::INSECT_IMITATION {
            d.hand[1] = i as u8;
            *u = Some(virt(w, c));
            found = true;
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
        if takes_insect_imitation(w, c) < 0 {
            continue;
        }
        if !affordable(w, side, c & 0xFFF)? {
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

/// 0x13FA40(side, U): what to summon: 0 a monster, 1 a non-monster (U
/// set), 2 card 0x170 (U set), 3 a monster for Insect Imitation (U set),
/// −1 nothing. In order: monster zone full → the non-monster choice;
/// magic/trap zone full → the first affordable monster (U set); card 0x170
/// in hand (affordable) with [`weak_trio`]; the Insect Imitation pair; the
/// first affordable monster — U NOT set (0x13FC14 returns before
/// 0x1DDFE0); the non-monster choice.
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
    if weak_trio(w, side)? == 1 && find_card(w, side, &mut local, u, id::CARD_170)? >= 0 {
        return Ok(2);
    }
    if insect_pair(w, side, &mut local, u)? >= 0 {
        return Ok(3);
    }
    if affordable_monster(w, side, &hand)?.is_some() {
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// The leader's neighbour `cell` is open for U: not Labyrinth, not Crush
/// when it would crush U (0x251760).
fn open<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: &Unit,
    col: i32,
    row: i32,
) -> R<bool> {
    let t = w.terrain(col, row);
    if t == LABYRINTH || (t == CRUSH && w.crushed(u, col, row)?) {
        return Ok(false);
    }
    Ok(true)
}

/// 0x140780(side, _, U, tgt): the summon cell among the leader's
/// neighbours (0x24FD30) that are [`open`] and empty:
/// - classes other than 0 and 3: the last such neighbour, −1 when none;
/// - a monster or trap with a target: the neighbour with the fewest moves
///   to it: 0x27BA70 (see-all 0, at most 2, hidden strength 0) when
///   0x279E50 (strength 0, see-all 0) is positive, else 0x27AC00 (at most
///   2), counted only when the step buffer holds a candidate (the buffer
///   is filled once and never reset between neighbours); −1 when none;
/// - without a target: the neighbour nearest (Manhattan, below 14, first
///   wins) its own [`grass_cell`] for U; −1 when none.
fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
    tgt: i32,
) -> R<i32> {
    // sp+0xD0: the step buffer.
    let mut buf = Steps::new();
    let class = i32::from(u.kind >> 5);
    if class != 0 && class != 3 {
        let mut last = -1;
        for i in 0..8 {
            let cell = neighbour(w, side, i)?;
            if cell < 0 {
                continue;
            }
            let (col, row) = (cell & 0xFF, cell >> 8);
            if !open(w, u, col, row)? || w.at_key(col, row) >= 0 || row >= 8 {
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
            if !open(w, u, col, row)? || w.at(col, row).is_some() {
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
                    ROUTE_LIMIT,
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
                    ROUTE_LIMIT,
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
    let mut best = 0;
    let mut best_d = GOAL_DISTANCE_LIMIT;
    for i in 0..8 {
        let cell = neighbour(w, side, i)?;
        if cell < 0 {
            continue;
        }
        let (col, row) = (cell & 0xFF, cell >> 8);
        if !open(w, u, col, row)? || w.at_key(col, row) >= 0 {
            continue;
        }
        // Without a grass cell 0x13F3F0 returns the residue 0x1409E4 (the
        // return address 0x24FD30 saved there), never within 14.
        let Some(v) = grass_cell(w, u, col, row)? else {
            continue;
        };
        let (vc, vr) = (v & 0xFF, v >> 8);
        let d = (row.max(vr) - row.min(vr)) + (col.max(vc) - col.min(vc));
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

/// 0x13F7B0(key, side, d): fill `d` with a summon (type 0xA) or return −1:
/// summon flag clear, both zones full, no category. The target search
/// 0x278850 (depth 3, filter 0x1F, see-all 0, hidden strength 0) runs
/// once, right after the category, on the category's U — on its
/// first-affordable-monster answer an unwritten buffer ([`residue_u`]).
/// Category 0: 0x140120; with a target (from that search) or fewer than
/// four monsters the cell for the chosen monster, else (or when no root
/// was found) the non-monster choice filtered by U's card. Category 2:
/// 0x140590 lists card 0x170 and its power-ups. Category 3: 0x13FC80
/// again on the action record. Category 1: the non-monster choice
/// filtered by U's card, then the cell without a target.
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
    // 0x278850(U, col, row, other, 3, 0x1F, 2, 2, [see-all 0, 0x142400 = 0]).
    let searched = match &u {
        Some(u) => u.clone(),
        None => residue_u(w)?,
    };
    let tgt = find_target(
        w,
        Some(&searched),
        pos & 0xFF,
        pos >> 8,
        other,
        TARGET_DEPTH,
        0x1F,
        2,
        2,
        0,
        0,
    )?;
    let with_u = |u: &Option<Unit>| match u {
        Some(u) => Ok(u.clone()),
        None => unsupported("0x13F7B0 reads an uninitialised U"),
    };
    let mut non_monster_path = r != 0 && r != 2 && r != 3;
    let mut cell_tgt = tgt;
    match r {
        0 => {
            if monster_choice_with(w, side, d, &mut u, TEA_CHOICE, TEA_EXTRA)? < 0
                || (tgt < 0 && w.monsters(side) >= MONSTERS_FOR_NON_MONSTER)
            {
                non_monster_path = true;
            }
        }
        2 => {
            card_list_with(w, side, d, &u, false)?;
        }
        3 => {
            insect_pair(w, side, d, &mut u)?;
        }
        _ => (),
    }
    if non_monster_path {
        // 0x13F9D0: filter = U +0x3C.
        let card = i32::from(match &u {
            Some(u) => u.card,
            None => searched.card,
        });
        let v = non_monster(w, side, d, &mut u, card)?;
        if v < 0 {
            return Ok(-1);
        }
        cell_tgt = -1;
    }
    let c = cell(w, side, &with_u(&u)?, cell_tgt)?;
    if c < 0 {
        return Ok(-1);
    }
    d.target = c as u16;
    Ok(1)
}
