//! The generic module's summon planner 0x16FDA0 (Mako's 0x14D470: +0xC =
//! 2, with the hidden strength 0x172BE0 and the monster limit 5) and its
//! subtree: the category choice 0x170030 and the summon cell 0x170EF0 are
//! its own; the hand search 0x1704D0, the non-monster choice 0x1705C0 and
//! the monster choice 0x1707C0 are Pegasus's 0x137160 / 0x137250 /
//! 0x137450; the fodder pass 0x1709A0 is Weevil's 0x10EF80 with the limit
//! 0xFA1 and the card test 0x170B80; the card list 0x170CB0 is
//! Margaret's 0x149850.

use super::super::margaret::summon::card_list;
use super::super::pegasus::summon::{affordable_monster, find_card, non_monster, zone};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::seto::reveal::f112ed0;
use super::super::summon::{
    kind_of, monster_choice, neighbour, virt, FodderFilter, Hand, MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::reveal::ritual_below_result;
use super::{hidden_value, id};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x1707C0 / 0x1709A0: 0x27A850 scoring at every tier, fodder limit
/// 0xFA1, non-monsters, monsters with effect entries and (under leader
/// 0x3A) the Exodia pieces never fodder.
const SHADI_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0xFA1,
    fodder_filter: FodderFilter::NonMonsterOrEffectOrExodia,
    skip_root: 999,
};

/// 0x16FDA0: the depth of the target search 0x278850.
const TARGET_DEPTH: i32 = 3;
/// 0x16FDA0: from this many monsters (0x1E13D0, `slti 5`) a target-less
/// monster summon turns into the non-monster choice.
const MONSTERS_FOR_NON_MONSTER: i32 = 5;
/// 0x170EF0: the route limit of the target searches.
const TARGET_ROUTE_LIMIT: i32 = 3;
/// Labyrinth (8) and Crush (9) terrain.
const LABYRINTH: i32 = 8;
const CRUSH: i32 = 9;

/// 0x170030(side, U): what to summon: 0 a monster (U set), 1 a
/// non-monster (U set), 2 a particular card (U set), −1 nothing. In
/// order: monster zone full → the non-monster choice; magic/trap zone full
/// → the first affordable monster; the first ritual in hand whose tributes
/// pass [`ritual_below_result`] (and that 0x1704D0 finds); card 0x2BC or
/// 0x2BE in hand when 0x112ED0 on its entry 0 finds a target; Cursebreaker,
/// then Paralyzing Potion, in hand while one of the side's slots 0..=10
/// holds a spellbound unit or none (0x1E2800 ≠ 0); the first affordable
/// monster; the non-monster choice.
fn category<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &mut Option<Unit>,
) -> R<i32> {
    let monsters = zone(w, side, 0) & 0xFF;
    let others = zone(w, side, 1) & 0xFF;
    let hand = Hand::of(w, side);
    // sp+0xC0: a local action record (written, never read).
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
    let n = w.next_hand_size(side);
    for i in 0..n {
        let c = hand.get(i)?;
        if kind_of(w, c) >> 5 != 5 {
            continue;
        }
        if ritual_below_result(w, side, c)? == 1
            && find_card(w, side, &mut local, u, c & 0xFFFF)? >= 0
        {
            return Ok(2);
        }
    }
    for i in 0..n {
        let c = hand.get(i)?;
        if c != id::CROSS_A && c != id::CROSS_B {
            continue;
        }
        // 0x2715C0(card, 0): the entry, read by 0x16D480 without a null
        // check.
        let Some(entry) = virt(w, c).entries[0] else {
            return unsupported("0x170030 reads entry 0 of a card without one");
        };
        if f112ed0(w, side, entry)? >= 0 && find_card(w, side, &mut local, u, c & 0xFFFF)? >= 0 {
            return Ok(2);
        }
    }
    for card in [id::CURSEBREAKER, id::POTION] {
        for slot in 0..11 {
            if w.spellbind(side, slot) == 0 {
                continue;
            }
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

/// 0x170EF0(side, _, U, tgt): the summon cell among the leader's
/// neighbours (0x24FD30) that are not Labyrinth and, on Crush, would not
/// crush U (0x251760): non-monsters → the first empty (0x1E4A10) one; with
/// a target: the empty (0x1E4960) neighbour with the fewest moves to it
/// (0x27BA70, see-all 0, at most 3, hidden strength 0x172BE0(side) when
/// 0x279E50 with that strength is positive, else 0x27AC00 at most 3),
/// counted only when the shared step buffer holds a candidate; without a
/// target or a route: the first empty one; −1 when none.
fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
    tgt: i32,
) -> R<i32> {
    cell_with(w, side, u, tgt, hidden_value(w, side))
}

/// [`cell`] with the module's hidden strength (Bakura's copy 0x15B800 asks
/// 0x15D9F0).
pub(in super::super) fn cell_with<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
    tgt: i32,
    hidden: i32,
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
            let m = w.margin_seen(u, tgt, hidden, tc, tr, false)?;
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
                    hidden & 0xFFFF,
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

/// 0x16FDA0(key, side, d): fill `d` with a summon (type 0xA, +0xC = 2) or
/// return −1: summon flag clear, both zones full, no category. The target
/// search 0x278850 (depth 3, filter 0x1F, see-all 0, hidden strength
/// 0x172BE0(side)) runs on the category's U, after the monster choice for
/// category 0. Category 0: 0x1707C0; without a target and with five or
/// more monsters, the non-monster choice filtered by the chosen card.
/// Category 2: 0x170CB0 lists the card and its power-ups; 1: the
/// non-monster choice filtered by U's card. Then 0x170EF0 with the target.
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
    // 0x278850(U, col, row, other, 3, 0x1F, 2, 2, [see-all 0, 0x172BE0(side)]).
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
        None => unsupported("0x16FDA0 reads an uninitialised U"),
    };
    let tgt;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, SHADI_CHOICE)? < 0 {
            // Unreachable (0x170030 found an affordable monster).
            return unsupported("0x16FDA0 uses an uninitialised target");
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
