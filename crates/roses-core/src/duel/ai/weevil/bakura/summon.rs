//! Bakura's summon planner 0x15AC30 (Mako's 0x14D470: +0xC = 2, monster
//! limit 6, with the hidden strength 0x15D9F0) and its subtree: the
//! category choice 0x15AEC0 (Joey's 0x1517E0 with the ritual 0x343 in
//! place of the pair and graveyard tests) and the monster choice 0x15B3E0
//! are his own; the hand search 0x15B0F0 and the non-monster choice
//! 0x15B1E0 are Pegasus's 0x137160 / 0x137250; the card list 0x15B5D0 is
//! Margaret's 0x149850; the cell 0x15B800 is the generic module's 0x170EF0.

use super::super::margaret::summon::card_list;
use super::super::pegasus::summon::{affordable_monster, find_card, non_monster, zone};
use super::super::search::find_target;
use super::super::shadi::summon::cell_with;
use super::super::summon::{equip, equip_bonus, kind_of, virt, Hand};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{hidden_value, id, ritual_ok};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x15AC30: the depth of the target search 0x278850.
const TARGET_DEPTH: i32 = 3;
/// 0x15AC30: from this many monsters (`slti 6`: never) a target-less
/// monster summon turns into the non-monster choice.
const MONSTERS_FOR_NON_MONSTER: i32 = 6;
/// 0x15B3E0: the ATK (with the bonus) a power-up must keep U below
/// (`slti 0x578`).
const EQUIP_ATK_BELOW: i32 = 0x578;
/// Paralyzing Potion and Cursebreaker (never appended).
const POTION: i32 = 0x31B;
const CURSEBREAKER: i32 = 0x31C;

/// 0x15AEC0(side, U): monster zone full → the non-monster choice;
/// magic/trap zone full → the first affordable monster; the ritual 0x343
/// in hand (0x15B0F0) with [`ritual_ok`] → 2; the first affordable monster
/// → 0; the non-monster choice.
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
    if find_card(w, side, &mut local, u, id::RITUAL_343)? >= 0
        && ritual_ok(w, side, id::RITUAL_343)? == 1
    {
        return Ok(2);
    }
    if let Some(c) = affordable_monster(w, side, &hand)? {
        *u = Some(virt(w, c));
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// 0x15B3E0(side, d, U): d+6 = the LAST hand index holding U's card as a
/// monster card (no affordability test); then, in hand order, every
/// power-up (class 2, not Paralyzing Potion or Cursebreaker) that fits U's
/// card (0x24A880 ≥ 0) while U's ATK (0x1E1570, bonuses included) plus its
/// bonus (0x24ACC0) stays below 1400 is appended from d+7 and adds its
/// bonus to U +0x38 / +0x3A; the first that would reach 1400 ends the scan.
/// 1; −1 without U.
fn monster_choice<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
) -> R<i32> {
    let Some(unit) = u.as_mut() else {
        return Ok(-1);
    };
    let hand = Hand::of(w, side);
    let n = w.next_hand_size(side);
    let card = i32::from(unit.card);
    for i in 0..n {
        let c = hand.get(i)?;
        if c == card && kind_of(w, c) >> 5 == 0 {
            d.hand[0] = i as u8;
        }
    }
    let mut next = 1usize;
    for k in 0..n {
        let c = hand.get(k)?;
        if equip(w, card, c)? < 0 {
            continue;
        }
        if kind_of(w, c) >> 5 != 2 || c == POTION || c == CURSEBREAKER {
            continue;
        }
        let bonus = equip_bonus(w, c) as i16;
        if w.atk_of(unit, false)? + i32::from(bonus) >= EQUIP_ATK_BELOW {
            break;
        }
        unit.atk_mod = unit.atk_mod.wrapping_add(bonus);
        unit.def_mod = unit.def_mod.wrapping_add(bonus);
        let Some(slot) = d.hand.get_mut(next) else {
            return unsupported("0x15B3E0 writes past d+0xA");
        };
        *slot = k as u8;
        next += 1;
    }
    Ok(1)
}

/// 0x15AC30(key, side, d): Mako's 0x14D470 with [`category`],
/// [`monster_choice`] and the hidden strength 0x15D9F0 (+0xC = 2).
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
        None => unsupported("0x15AC30 reads an uninitialised U"),
    };
    let tgt;
    if r == 0 {
        if monster_choice(w, side, d, &mut u)? < 0 {
            return unsupported("0x15AC30 uses an uninitialised target");
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
    let c = cell_with(w, side, &with_u(&u)?, tgt, hidden_value(w, side))?;
    if c < 0 {
        return Ok(-1);
    }
    d.target = c as u16;
    Ok(1)
}
