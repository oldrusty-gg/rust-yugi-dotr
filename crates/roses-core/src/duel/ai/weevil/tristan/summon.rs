//! T. Tristan Grey's summon planner 0x144470 (Tea's 0x13F7B0 without its
//! Insect Imitation category) and its subtree: the category choice
//! 0x1446B0 and the summon cell 0x145330 are his own; the hand search
//! 0x144A20 is Darkness-ruler's 0x12E3B0 (= Pegasus's 0x137160), the
//! non-monster choice 0x144B20 and the monster choice 0x144D20 are
//! Pegasus's 0x137250 / 0x137450, the card list 0x145130 is
//! Darkness-ruler's 0x12EAC0 (Pegasus's 0x137800 returning U's class),
//! all shared; the fodder pass 0x144F00 is Weevil's 0x10EF80 with the
//! limit 0xAF1 and Darkness-ruler's card test 0x1450E0 = 0x12EA70 (every
//! card with a record is kept, so it never adds fodder).

use super::super::pegasus::summon::{affordable_monster, card_list, find_card, non_monster, zone};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{monster_choice, neighbour, virt, FodderFilter, Hand, MonsterChoice};
use super::super::tea::residue_u;
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{crowded_front, favoured_row, id, ritual_ok, row_card};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x144D20 / 0x144F00: 0x27A850 scoring at every tier, fodder limit
/// 0xAF1, every recorded card refused as fodder (0x1450E0).
const TRISTAN_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0xAF1,
    fodder_filter: FodderFilter::Recorded,
    skip_root: 999,
};

/// 0x144470: the depth of the target search 0x278850.
const TARGET_DEPTH: i32 = 3;
/// 0x145330: the route limit of its 0x27BA70 / 0x27AC00 searches.
const ROUTE_LIMIT: i32 = 3;
/// 0x145330: the distance bound of the no-target choice (`0xE`).
const ROW_DISTANCE_LIMIT: i32 = 0xE;
/// 0x144470: from this many monsters (0x1E13D0) a target-less monster
/// summon turns into the non-monster choice.
const MONSTERS_FOR_NON_MONSTER: i32 = 4;
/// The Labyrinth terrain (8) and Crush (9).
const LABYRINTH: i32 = 8;
const CRUSH: i32 = 9;

/// 0x1446B0(side, U): what to summon: 0 a monster, 1 a non-monster (U
/// set), 2 a particular card (U set), −1 nothing. In order: monster zone
/// full → the non-monster choice; magic/trap zone full → the first
/// affordable monster (U set); the ritual 0x34E in hand (affordable) with
/// acceptable tributes (0x146F70); with [`crowded_front`] card 0x2DB, then
/// 0x2EF in hand; the predicted-hand monster with the highest printed ATK
/// (0x1E1570 of U) among those that [`row_card`] places on one of rows
/// 1..=5 and that are affordable (0x144A20); the first affordable monster
/// — U NOT set (0x1449A4 returns before 0x1DDFE0); the non-monster choice.
fn category<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &mut Option<Unit>,
) -> R<i32> {
    let monsters = zone(w, side, 0) & 0xFF;
    let others = zone(w, side, 1) & 0xFF;
    let hand = Hand::of(w, side);
    // sp+0xA0: a local action record (written, never read).
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
    if find_card(w, side, &mut local, u, id::RITUAL_34E)? >= 0
        && ritual_ok(w, side, id::RITUAL_34E)? == 1
    {
        return Ok(2);
    }
    if crowded_front(w, side)? == 1 {
        for card in [id::CARD_2DB, id::CARD_2EF] {
            if find_card(w, side, &mut local, u, card)? >= 0 {
                return Ok(2);
            }
        }
    }
    let (mut best_atk, mut best) = (-1, 0);
    for i in 0..w.next_hand_size(side) {
        let c = hand.get(i)?;
        for row in 1..6 {
            if !row_card(w, c, row)? {
                continue;
            }
            if find_card(w, side, &mut local, u, c & 0xFFFF)? < 0 {
                continue;
            }
            let Some(found) = u.as_ref() else {
                return unsupported("0x1446B0 reads a null U");
            };
            let atk = w.atk_of(found, false)?;
            if best_atk < atk {
                best = c;
                best_atk = atk;
            }
        }
    }
    if best_atk >= 0 {
        *u = Some(virt(w, best));
        return Ok(2);
    }
    if affordable_monster(w, side, &hand)?.is_some() {
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// The leader's neighbour (col, row) is open for U: not Labyrinth, not
/// Crush when it would crush U (0x251760).
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

/// 0x145330(side, _, U, tgt): Tea's 0x140780 with the route limit 3 and,
/// without a target, the neighbour whose ROW is nearest (below 14, first
/// wins) the [`favoured_row`] of U (−1 when none: then the nearest to row
/// −1, i.e. the lowest row).
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
    let mut best = -1;
    let mut best_d = ROW_DISTANCE_LIMIT;
    for i in 0..8 {
        let cell = neighbour(w, side, i)?;
        if cell < 0 {
            continue;
        }
        let (col, row) = (cell & 0xFF, cell >> 8);
        if !open(w, u, col, row)? || w.at_key(col, row) >= 0 {
            continue;
        }
        let v = favoured_row(w, u)?;
        let d = row.max(v) - row.min(v);
        if d < best_d && d >= 0 {
            best = cell;
            best_d = d;
        }
    }
    Ok(if best_d < ROW_DISTANCE_LIMIT {
        best
    } else {
        -1
    })
}

/// 0x144470(key, side, d): fill `d` with a summon (type 0xA) or return −1:
/// summon flag clear, both zones full, no category. As Tea's 0x13F7B0:
/// the target search 0x278850 (depth 3, filter 0x1F, see-all 0, hidden
/// strength 0) runs once on the category's U ([`residue_u`] when the
/// category left it unwritten). Category 0: 0x144D20; with a target or
/// fewer than four monsters the cell for the chosen monster, else (or
/// when no root was found) the non-monster choice filtered by U's card.
/// Category 2: 0x145130 lists the card and its power-ups. Category 1: the
/// non-monster choice filtered by U's card, then the cell without a
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
    d.reveal = 0;
    let pos = w.pos(key >> 8, key & 0xFF);
    let other = i32::from(side == 0);
    let searched = match &u {
        Some(u) => u.clone(),
        None => residue_u(w)?,
    };
    // 0x278850(U, col, row, other, 3, 0x1F, 2, 2, [see-all 0, 0x146A90 = 0]).
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
        None => unsupported("0x144470 reads an uninitialised U"),
    };
    let mut non_monster_path = r != 0 && r != 2;
    let mut cell_tgt = tgt;
    match r {
        0 => {
            if monster_choice(w, side, d, &mut u, TRISTAN_CHOICE)? < 0
                || (tgt < 0 && w.monsters(side) >= MONSTERS_FOR_NON_MONSTER)
            {
                non_monster_path = true;
            }
        }
        2 => {
            card_list(w, side, d, &u)?;
        }
        _ => (),
    }
    if non_monster_path {
        // 0x144644: filter = U +0x3C.
        let card = i32::from(match &u {
            Some(u) => u.card,
            None => searched.card,
        });
        if non_monster(w, side, d, &mut u, card)? < 0 {
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
