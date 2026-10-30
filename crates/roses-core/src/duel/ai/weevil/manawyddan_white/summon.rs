//! White Manawyddan's summon planner 0x169070 (the red 0x164CC0: target
//! search depth 2 with see-all 1, hidden strength 0, +0xC = 0) with his own
//! category 0x169300 and cell 0x169EE0 (Joey's 0x1525C0 shape: see-all 1,
//! route limit 4, rituals searched like monsters); the hand search, the
//! non-monster and monster choices and the card list are Pegasus's, the
//! fodder pass 0x169AB0 is the red 0x165790 (limit 1).

use super::super::pegasus::summon::{
    affordable_monster, card_list, find_card, non_monster, zone,
};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{monster_choice, neighbour, virt, FodderFilter, Hand, MonsterChoice};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{watch, CARD_16C, CARD_E0};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x1698D0 / 0x169AB0: as the red module's choice.
const CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 1,
    fodder_filter: FodderFilter::NonMonsterOrEffect,
    skip_root: 999,
};
const TARGET_DEPTH: i32 = 2;
const MONSTERS_FOR_NON_MONSTER: i32 = 6;
const TARGET_ROUTE_LIMIT: i32 = 4;
const LABYRINTH: i32 = 8;
const CRUSH: i32 = 9;
/// 0x169300's card immediates.
const CARD_2BB: i32 = 0x2BB;
const CARD_339: i32 = 0x339;
const CARD_2EC: i32 = 0x2EC;

/// 0x169300(side, U): zone rules (monster zone full → the non-monster
/// choice; magic/trap zone full → the first affordable monster); card
/// 0x2BB when [`watch`] answers 1; card 0x339; with fewer than four stars
/// cards 0x2EC, 0xE0, 0x16C; the first affordable monster; the non-monster
/// choice.
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
    if watch(w, side)? == 1 && find_card(w, side, &mut local, u, CARD_2BB)? >= 0 {
        return Ok(2);
    }
    if find_card(w, side, &mut local, u, CARD_339)? >= 0 {
        return Ok(2);
    }
    if w.stars(side) < 4 {
        for card in [CARD_2EC, CARD_E0, CARD_16C] {
            if find_card(w, side, &mut local, u, card)? >= 0 {
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

/// 0x169EE0(side, _, U, tgt): classes other than monsters and rituals →
/// the last empty open neighbour above row 8; with a target: the empty
/// neighbour with the fewest moves to it (0x27BA70 see-all 1 at most 4
/// when 0x279E50 (see-all 1, hidden 0) is positive, else 0x27AC00 at most
/// 4), counted only when the shared step buffer holds a candidate; else
/// the first empty open neighbour; −1.
fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
    tgt: i32,
) -> R<i32> {
    let mut buf = Steps::new();
    let open = |col: i32, row: i32| -> R<bool> {
        let t = w.terrain(col, row);
        if t == LABYRINTH || (t == CRUSH && w.crushed(u, col, row)?) {
            return Ok(false);
        }
        Ok(true)
    };
    let class = i32::from(u.kind >> 5);
    if class != 0 && class != 5 {
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
            let m = w.margin_seen(u, tgt, 0, tc, tr, true)?;
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
                    1,
                    TARGET_ROUTE_LIMIT,
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
    for i in 0..8 {
        let cell = neighbour(w, side, i)?;
        if cell < 0 {
            continue;
        }
        let (col, row) = (cell & 0xFF, cell >> 8);
        if !open(col, row)? || w.at_key(col, row) >= 0 {
            continue;
        }
        return Ok(cell);
    }
    Ok(-1)
}

/// 0x169070(key, side, d): the red 0x164CC0 with [`category`] and
/// [`cell`].
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
            1,
            0,
        )
    };
    let with_u = |u: &Option<Unit>| match u {
        Some(u) => Ok(u.clone()),
        None => unsupported("0x169070 reads an uninitialised U"),
    };
    let tgt;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, CHOICE)? < 0 {
            return unsupported("0x169070 uses an uninitialised target");
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
