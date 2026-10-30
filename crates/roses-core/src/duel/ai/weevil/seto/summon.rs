//! Seto's PAL summon subtree, 0x114950..0x116120. The category and cell
//! policies are his own; the card lookup and non-monster selector match
//! Pegasus's instruction streams after address relocation. The fusion
//! chooser uses the shared tree with Seto's traced fodder threshold.

use super::super::pegasus::summon::{affordable_monster, find_card, non_monster, zone};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{
    equip, kind_of, monster_choice, neighbour, unit_mask, virt, FodderFilter, Hand, MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{f117cb0, f117e20, f117ed0, f117f50, hidden_value};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x115610 matches Pegasus's 0x137450 with the local fodder call
/// relocated; 0x1157F0 uses 2*ATK+DEF < 0x1389 and 0x1159D0 excludes
/// non-monsters and monsters with effects. No extra root-card exclusion.
const SETO_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0x1389,
    fodder_filter: FodderFilter::NonMonsterOrEffect,
    skip_root: 999,
};

/// 0x115610: shared fusion/equip search with Seto's fodder policy.
pub(super) fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
) -> R<i32> {
    monster_choice(w, side, d, u, SETO_CHOICE)
}

/// 0x115080: try the first affordable predicted-hand index of each of
/// the five countertraps in this instruction-immediate order. Return
/// its class, with D+6 and U filled, or -1.
pub(super) fn priority_trap<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
) -> R<i32> {
    for card in [0x337, 0x335, 0x338, 0x33C, 0x339] {
        let result = find_card(w, side, d, u, card)?;
        if result >= 0 {
            return Ok(result);
        }
    }
    Ok(-1)
}

/// 0x114BE0(side,U): full monster zone -> non-monster, full other zone
/// -> affordable monster. Otherwise the priority is ritual 0x345;
/// 0x53 or a countertrap when 0x117ED0 approves and no own 0x53 exists;
/// cards 0x179, 0x186 and 0x2AD when their respective predicates approve;
/// Cursebreaker when any slot 0..10 has nonzero 0x1E2800 (empty slots
/// return -1 and count); affordable monster; any non-monster.
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
    if find_card(w, side, &mut local, u, 0x345)? >= 0
        && super::reveal::ritual_ok(w, side, 0x345)? == 1
    {
        return Ok(2);
    }
    if f117ed0(w, side)? == 1
        && unit_mask(w, 0, 0, side, 1, 0x53001C, 0xE)? == 0
        && (find_card(w, side, &mut local, u, 0x53)? >= 0
            || priority_trap(w, side, &mut local, u)? >= 0)
    {
        return Ok(2);
    }
    if f117cb0(w, side)? == 1 && find_card(w, side, &mut local, u, 0x179)? >= 0 {
        return Ok(2);
    }
    if f117e20(w, side)? == 1 && find_card(w, side, &mut local, u, 0x186)? >= 0 {
        return Ok(2);
    }
    if f117f50(w, side)? >= 0 && find_card(w, side, &mut local, u, 0x2AD)? >= 0 {
        return Ok(2);
    }
    for slot in 0..11 {
        if w.spellbind(side, slot) != 0 && find_card(w, side, &mut local, u, 0x31C)? >= 0 {
            return Ok(2);
        }
    }
    if let Some(c) = affordable_monster(w, side, &hand)? {
        *u = Some(virt(w, c));
        return Ok(0);
    }
    non_monster(w, side, &mut local, u, 999)
}

/// 0x115A30: list the LAST hand index of U's card without an affordability
/// check (unlike Pegasus's 0x137800), then fitting power-ups for a monster.
/// Paralyzing Potion and Cursebreaker are excluded. Preserve the original
/// skip of the current D+6 and D+7 while scanning.
pub(super) fn card_list<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &Option<Unit>,
) -> R<i32> {
    d.hand = [0xFF; 5];
    let hand = Hand::of(w, side);
    let Some(u) = u else {
        return Ok(-1);
    };
    let n = w.next_hand_size(side);
    for i in 0..n {
        if i == i32::from(d.hand[0]) || i == i32::from(d.hand[1]) {
            continue;
        }
        if hand.get(i)? == i32::from(u.card) {
            d.hand[0] = i as u8;
        }
    }
    let first = hand.get(i32::from(d.hand[0]))?;
    if kind_of(w, first) >> 5 != 0 {
        return Ok(1);
    }
    let mut count = 1usize;
    for i in 0..n {
        if i == i32::from(d.hand[0]) || i == i32::from(d.hand[1]) {
            continue;
        }
        let c = hand.get(i)?;
        if equip(w, first, c)? < 0 || kind_of(w, c) >> 5 != 2 || matches!(c, 0x31B | 0x31C) {
            continue;
        }
        let Some(slot) = d.hand.get_mut(count) else {
            return unsupported("0x115A30 writes past D+0xA");
        };
        *slot = i as u8;
        count += 1;
    }
    Ok(1)
}

/// 0x115F0C..0x115F50: corner goal for a monster favoured on Meadow,
/// chosen separately for each candidate neighbour.
fn meadow_goal(col: i32, row: i32) -> i32 {
    if row >= 5 {
        0x602
    } else if col >= 5 {
        0x206
    } else if col < row {
        0x501
    } else {
        0x105
    }
}

/// 0x115C10: choose among 0x24FD30's eight leader neighbours. A
/// non-monster or card 0x53 takes the last empty neighbour excluding
/// Labyrinth AND Crush. Other monsters first seek a route to the target
/// (depth 5, see-all 0, Seto's hidden estimate), retaining the shared step
/// buffer between neighbours. With no route, minimize depth-7 path length
/// to (3,3), or to a corner when favoured on Meadow. Only nonnegative
/// lengths <14 qualify and ties retain the first candidate.
pub(super) fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
    tgt: i32,
) -> R<i32> {
    let mut buf = Steps::new();
    if u.kind >> 5 != 0 || u.card == 0x53 {
        let mut last = -1;
        for i in 0..8 {
            let cell = neighbour(w, side, i)?;
            if cell < 0 {
                continue;
            }
            let (col, row) = (cell & 0xFF, cell >> 8);
            if matches!(w.terrain(col, row), 8 | 9) || w.at_key(col, row) >= 0 || row >= 8 {
                continue;
            }
            last = cell;
        }
        return Ok(last);
    }
    let open = |col: i32, row: i32| -> R<bool> {
        let terrain = w.terrain(col, row);
        Ok(terrain != 8 && (terrain != 9 || !w.crushed(u, col, row)?))
    };
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
            let hidden = hidden_value(w, side)?;
            let margin = w.margin_seen(u, tgt, hidden, tc, tr, false)?;
            let d = if margin > 0 {
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
                    5,
                    hidden,
                )?
            } else {
                path(w, Some(u), &mut buf, col, row, tc, tr, tgt & 0xFFFF, 5)?
            };
            if step_count(&buf) > 0 && d < best_d {
                best_d = d;
                best = cell;
            }
        }
        if best != 0xFFFF {
            return Ok(best);
        }
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
        let goal = if w.terrain_bonus_kind(3, u)? == 1 {
            meadow_goal(col, row)
        } else {
            0x303
        };
        let (gc, gr) = (goal & 0xFF, goal >> 8);
        let occupant = w.at_key(gc, gr) & 0xFFFF;
        let d = path(w, Some(u), &mut buf, col, row, gc, gr, occupant, 7)?;
        if d >= 0 && d < best_d {
            best = cell;
            best_d = d;
        }
    }
    Ok(best)
}

/// 0x114950: fill a summon decision, using depth 3 / see-all 0 target
/// search and 0x118500's hidden estimate. A target-less monster choice
/// switches to non-monster selection when four monsters already exist.
pub(super) fn summon<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    key: i32,
    side: i32,
    d: &mut Decision,
) -> R<i32> {
    d.key = key as u16;
    if !w.can_summon(side) || (zone(w, side, 0) == 5 && zone(w, side, 1) == 5) {
        return Ok(-1);
    }
    let mut u = None;
    let result = category(w, side, &mut u)?;
    if result < 0 {
        return Ok(-1);
    }
    d.kind = 0xA;
    d.reveal = 0;
    let pos = w.pos(key >> 8, key & 0xFF);
    let target = |u: &Option<Unit>| {
        find_target(
            w,
            u.as_ref(),
            pos & 0xFF,
            pos >> 8,
            i32::from(side == 0),
            3,
            0x1F,
            2,
            2,
            0,
            hidden_value(w, side)?,
        )
    };
    let read_u = |u: &Option<Unit>| match u {
        Some(u) => Ok(u.clone()),
        None => unsupported("0x114950 reads an uninitialised U"),
    };
    let tgt;
    if result == 0 {
        if monster(w, side, d, &mut u)? < 0 {
            return unsupported("0x114950 uses an uninitialised target");
        }
        tgt = target(&u)?;
        if tgt < 0 && w.monsters(side) >= 4 {
            let card = i32::from(read_u(&u)?.card);
            if non_monster(w, side, d, &mut u, card)? < 0 {
                return Ok(-1);
            }
        }
    } else {
        tgt = target(&u)?;
        if result == 2 {
            card_list(w, side, d, &u)?;
        } else {
            let card = i32::from(read_u(&u)?.card);
            if non_monster(w, side, d, &mut u, card)? < 0 {
                return Ok(-1);
            }
        }
    }
    let c = cell(w, side, &read_u(&u)?, tgt)?;
    if c < 0 {
        return Ok(-1);
    }
    d.target = c as u16;
    Ok(1)
}
