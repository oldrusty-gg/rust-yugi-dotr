//! Yugi's summon planner 0x15FBD0 (Margaret's 0x1488F0 with +0xC = 2, the
//! hidden strength 0x162E50 and the monster limit 5) and its subtree: the
//! category choice 0x15FEB0 and the pair 0x1602D0 (Margaret's 0x148F00
//! with 0x316 + 0x129 → 0x129 and no power-up loop) are his own; the hand
//! search 0x160430, the non-monster choice 0x160520 and the monster choice
//! 0x160720 are Pegasus's; the fodder pass 0x160900 is Weevil's 0x10EF80
//! with the limit 0xFA1 and York's card test 0x160AE0; the card list
//! 0x160B40 is Margaret's 0x149850; the cell 0x160D80 is the generic
//! module's 0x170EF0.

use super::super::margaret::summon::{card_list, transform_pair};
use super::super::pegasus::summon::{find_card, non_monster, zone};
use super::super::search::find_target;
use super::super::shadi::summon::cell_with;
use super::super::summon::{
    affordable, kind_of, monster_choice, virt, FodderFilter, Hand, MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{crowded_enemy, hidden_value, ritual_ok, threat};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x160720 / 0x160900: Pegasus's choice with the fodder limit 0xFA1 and
/// York's card test.
const YUGI_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0xFA1,
    fodder_filter: FodderFilter::NonMonsterOrEffect,
    skip_root: 999,
};

const TARGET_DEPTH: i32 = 3;
/// 0x15FBD0: `slti 5`.
const MONSTERS_FOR_NON_MONSTER: i32 = 5;
/// Card immediates of 0x15FEB0 / 0x1602D0.
const RITUAL_341: i32 = 0x341;
const RITUAL_355: i32 = 0x355;
const CARD_2BC: i32 = 0x2BC;
const CARD_2CB: i32 = 0x2CB;
const CARD_2AD: i32 = 0x2AD;
const CARD_2B: i32 = 0x2B;
const PAIR_LEAD: i32 = 0x316;
const CARD_129: i32 = 0x129;
const CURSEBREAKER: i32 = 0x31C;
const POTION: i32 = 0x31B;

/// 0x15FEB0(side, U): zone rules as Margaret's; then the rituals 0x341 and
/// 0x355 in hand with acceptable tributes (0x162D60); 0x2BC with a
/// [`threat`]; 0x2CB (0x162FC0 never negative: always asked); 0x2AD with a
/// non-monster plan-C pick (0x1630A0); 0x2B with a monster plan-C pick
/// (0x163340); the pair 0x1602D0 → 3; Cursebreaker, then Paralyzing
/// Potion, while a slot is non-zero in 0x1E2800; the first affordable
/// monster other than 0x129; the non-monster choice.
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
    let first_monster = |u: &mut Option<Unit>, skip: i32| -> R<bool> {
        for i in 0..w.next_hand_size(side) {
            let c = hand.get(i)?;
            if kind_of(w, c) >> 5 != 0 || !affordable(w, side, c)? || c == skip {
                continue;
            }
            *u = Some(virt(w, c));
            return Ok(true);
        }
        Ok(false)
    };
    if others == 5 {
        return Ok(if first_monster(u, 999)? { 0 } else { -1 });
    }
    for ritual in [RITUAL_341, RITUAL_355] {
        if find_card(w, side, &mut local, u, ritual)? >= 0 && ritual_ok(w, side, ritual)? == 1 {
            return Ok(2);
        }
    }
    if threat(w, side)? >= 0 && find_card(w, side, &mut local, u, CARD_2BC)? >= 0 {
        return Ok(2);
    }
    if crowded_enemy(w, side)? >= 0 && find_card(w, side, &mut local, u, CARD_2CB)? >= 0 {
        return Ok(2);
    }
    if super::super::margaret::graveyard_pick_with(w, side, false, super::id::GRAVE_FOR_SUPPORT)
        >= 0
        && find_card(w, side, &mut local, u, CARD_2AD)? >= 0
    {
        return Ok(2);
    }
    if super::super::margaret::graveyard_pick_with(w, side, true, super::id::GRAVE_FOR_MONSTERS)
        >= 0
        && find_card(w, side, &mut local, u, CARD_2B)? >= 0
    {
        return Ok(2);
    }
    if transform_pair(w, side, &mut local, u, PAIR_LEAD, CARD_129, CARD_129, false)? == 1 {
        return Ok(3);
    }
    for card in [CURSEBREAKER, POTION] {
        for slot in 0..11 {
            if w.spellbind(side, slot) == 0 {
                continue;
            }
            if find_card(w, side, &mut local, u, card)? >= 0 {
                return Ok(2);
            }
        }
    }
    if first_monster(u, CARD_129)? {
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// 0x15FBD0(key, side, d): Margaret's 0x1488F0 with [`category`], +0xC =
/// 2, the hidden strength 0x162E50 and the generic summon cell.
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
        None => unsupported("0x15FBD0 reads an uninitialised U"),
    };
    let tgt;
    let mut non_monster_path = false;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, YUGI_CHOICE)? < 0 {
            return unsupported("0x15FBD0 uses an uninitialised target");
        }
        tgt = target(&u)?;
        if tgt < 0 && w.monsters(side) >= MONSTERS_FOR_NON_MONSTER {
            non_monster_path = true;
        }
    } else {
        tgt = target(&u)?;
        match r {
            2 => {
                card_list(w, side, d, &mut u)?;
            }
            3 => {
                transform_pair(w, side, d, &mut u, PAIR_LEAD, CARD_129, CARD_129, false)?;
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
    let c = cell_with(w, side, &with_u(&u)?, tgt, hidden_value(w, side))?;
    if c < 0 {
        return Ok(-1);
    }
    d.target = c as u16;
    Ok(1)
}
