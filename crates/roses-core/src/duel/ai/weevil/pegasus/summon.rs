//! Pegasus's summon planner 0x136AE0 and its subtree: the category choice
//! 0x136D60, the hand search 0x137160, the non-monster choice 0x137250
//! (with a card filter), the monster choice 0x137450 (Weevil's 0x10EBF0
//! with power-up scoring at every tier and the fodder pass 0x137630), the
//! card list 0x137800 and the summon cell 0x1379F0. The predicted hand,
//! the fusion tree and the tribute solver are the shared ones of
//! [`super::super::summon`].

use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{
    affordable, equip, kind_of, monster_choice, neighbour, ritual_ok_with, virt, FodderFilter,
    Hand, MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{control_target, dragon_jar_useful, id, mimicat_target, RITUAL_ATK_LIMIT};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x137450 / 0x137630: 0x27A850 scoring at every tier, fodder limit
/// 0x3E9, no Cocoon test.
const PEGASUS_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0x3E9,
    fodder_filter: FodderFilter::None,
    skip_root: 999,
};

/// Paralyzing Potion and Cursebreaker: never listed as power-ups
/// (0x137800, as 0x10EE10).
const POTION: i32 = 0x31B;
const CURSEBREAKER: i32 = 0x31C;

/// 0x139860(side, card): 0x110210 without the moth exclusions and with
/// the tribute ATK limit 2600.
fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    ritual_ok_with(w, side, card, RITUAL_ATK_LIMIT, false)
}

/// 0x1DF290(class): the card category of a class: monsters 0, magic and
/// power-ups 1, traps 2, rituals 3, anything else itself.
pub(in super::super) fn category_of(class: i32) -> i32 {
    match class {
        1 | 2 => 1,
        3 | 4 => 2,
        5 => 3,
        c => c,
    }
}

/// 0x27A1D0(side, which): 0x1E1470 (slots 6..=10 of group `side`) for
/// `which` = 1, else 0x1E13D0 (slots 1..=5).
pub(in super::super) fn zone<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    which: i32,
) -> i32 {
    if which == 1 {
        w.others(side)
    } else {
        w.monsters(side)
    }
}

/// 0x137160(side, d, U, card): the first predicted-hand index holding
/// `card` (& 0xFFFF) that is affordable (0x271440): d+6 = index, U =
/// 0x1DDFE0(card), returns the card's class; −1 when none.
pub(in super::super) fn find_card<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
    card: i32,
) -> R<i32> {
    let hand = Hand::of(w, side);
    let n = w.next_hand_size(side);
    if n <= 0 {
        return Ok(-1);
    }
    for i in 0..n {
        let c = hand.get(i)?;
        if c != card & 0xFFFF || !affordable(w, side, c)? {
            continue;
        }
        d.hand[0] = i as u8;
        *u = Some(virt(w, c));
        return Ok(kind_of(w, c) >> 5);
    }
    Ok(-1)
}

/// 0x137250(side, d, U, filter): d+6.. = 0xFF; the first hand index
/// (ascending) of, in this order: the card `filter` (unless 999), a card of
/// a class other than 0 and 2 (magic, trap, ritual), any non-monster.
/// d+6 = index, U = the card, returns 1; −1 when there is none.
pub(in super::super) fn non_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
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
                _ => class != 0,
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

/// 0x137800(side, d, U): d+6.. = 0xFF; d+6 = the LAST hand index holding
/// U's card that is affordable; for a monster, append every other index
/// holding a power-up (class 2, not 0x31B/0x31C) that fits it (0x24A880 ≥
/// 0). Returns 1 (−1 without U). The card index the original reads back
/// through d+6 is refused when none was found (an out-of-buffer read).
pub(in super::super) fn card_list<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &Option<Unit>,
) -> R<i32> {
    card_list_with(w, side, d, u, true)
}

/// [`card_list`] with the affordability test of the card search optional:
/// Tea's copy 0x140590 takes the last index holding U's card whatever it
/// costs.
pub(in super::super) fn card_list_with<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &Option<Unit>,
    affordable_test: bool,
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
        let c = hand.get(i)?;
        if c != i32::from(u.card) {
            continue;
        }
        if !affordable_test || affordable(w, side, c)? {
            d.hand[0] = i as u8;
        }
    }
    let first = hand.get(i32::from(d.hand[0]))?;
    if kind_of(w, first) >> 5 != 0 {
        return Ok(1);
    }
    let mut count = 1usize;
    for k in 0..n {
        if k == i32::from(d.hand[0]) || k == i32::from(d.hand[1]) {
            continue;
        }
        let c = hand.get(k)?;
        if equip(w, first, c)? < 0 {
            continue;
        }
        if kind_of(w, c) >> 5 != 2 || c == POTION || c == CURSEBREAKER {
            continue;
        }
        let Some(slot) = d.hand.get_mut(count) else {
            return unsupported("0x137800 writes past d+0xA");
        };
        *slot = k as u8;
        count += 1;
    }
    Ok(1)
}

/// The first affordable monster of the predicted hand (the loops of
/// 0x136D60), as a card id.
pub(in super::super) fn affordable_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    hand: &Hand,
) -> R<Option<i32>> {
    for i in 0..w.next_hand_size(side) {
        let c = hand.get(i)?;
        if kind_of(w, c) >> 5 == 0 && affordable(w, side, c)? {
            return Ok(Some(c));
        }
    }
    Ok(None)
}

/// 0x136D60(side, U): what to summon: 0 a monster (U set by the caller's
/// 0x137450 unless set here), 1 a non-monster (U set), 2 a particular card
/// (U set by 0x137160), −1 nothing. In order: monster zone full → the
/// non-monster choice; magic/trap zone full → the first affordable
/// monster; Curse of Millennium Shield, then Garma Sword Oath, in hand with
/// acceptable tributes (0x139860); Change of Heart with a target next to
/// the enemy leader, Brain Control with the strongest one (0x139430; a
/// missing Change of Heart also skips Brain Control, quirk of 0x136F50);
/// Dragon Capture Jar when useful (0x139680); any trap in hand; Tremendous
/// Fire; Mimicat with a Tremendous Fire in a graveyard (0x139770); the
/// first affordable monster; the non-monster choice.
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
    for ritual in [id::CURSE_OF_MILLENNIUM_SHIELD, id::GARMA_SWORD_OATH] {
        if find_card(w, side, &mut local, u, ritual)? >= 0 && ritual_ok(w, side, ritual)? == 1 {
            return Ok(2);
        }
    }
    let v = control_target(w, side)?;
    if v >= 0 {
        let mut low = v & 0xFFFF;
        if (v >> 16) & 0xFFFF != 0xFFFF {
            let f = find_card(w, side, &mut local, u, id::CHANGE_OF_HEART)?;
            if f >= 0 {
                return Ok(2);
            }
            // 0x136F50 masks the search's result (−1) instead of the low half.
            low = f & 0xFFFF;
        }
        if low != 0xFFFF && find_card(w, side, &mut local, u, id::BRAIN_CONTROL)? >= 0 {
            return Ok(2);
        }
    }
    if dragon_jar_useful(w, side)?
        && find_card(w, side, &mut local, u, id::DRAGON_CAPTURE_JAR)? >= 0
    {
        return Ok(2);
    }
    for i in 0..w.next_hand_size(side) {
        let c = hand.get(i)?;
        if category_of(kind_of(w, c) >> 5) == 2 {
            *u = Some(virt(w, c));
            return Ok(1);
        }
    }
    if find_card(w, side, &mut local, u, id::TREMENDOUS_FIRE)? >= 0 {
        return Ok(2);
    }
    if mimicat_target(w, side) >= 0 && find_card(w, side, &mut local, u, id::MIMICAT)? >= 0 {
        return Ok(2);
    }
    if let Some(c) = affordable_monster(w, side, &hand)? {
        *u = Some(virt(w, c));
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// The last of the leader's neighbours (0x24FD30) that is empty, not
/// Labyrinth and, on Crush, would not crush U (0x251760); −1 when none
/// (0x137C24 / 0x137CCC).
fn last_free_neighbour<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
) -> R<i32> {
    let mut last = -1;
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
        last = cell;
    }
    Ok(last)
}

/// 0x1379F0(side, _, U, tgt): the summon cell. Non-monsters, and monsters
/// without a target, take the last free neighbour. A monster with a target
/// takes the neighbour with the fewest moves to it: 0x27BA70 (see-all 1,
/// at most 2) when 0x279E50 (see-all 1) is positive, else 0x27AC00 (at
/// most 2), counted only when the step buffer holds a candidate; the
/// buffer is filled once and never reset between neighbours, so a failed
/// search keeps the previous neighbour's candidates and its −1 wins. When
/// no neighbour has a route it falls through to the last free neighbour
/// (0x137C10 → 0x137C20).
fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
    tgt: i32,
) -> R<i32> {
    let mut buf = Steps::new();
    if u.kind >> 5 != 0 || tgt < 0 {
        return last_free_neighbour(w, side, u);
    }
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
        let m = w.margin_seen(u, tgt, 0, tc, tr, true)?;
        let d = if m > 0 {
            attack_path(w, Some(u), &mut buf, col, row, tc, tr, tgt as u32, 1, 2, 0)?
        } else {
            path(w, Some(u), &mut buf, col, row, tc, tr, tgt & 0xFFFF, 2)?
        };
        if step_count(&buf) <= 0 {
            continue;
        }
        if d < best_d {
            best_d = d;
            best = cell;
        }
    }
    if best == 0xFFFF {
        return last_free_neighbour(w, side, u);
    }
    Ok(best)
}

/// 0x136AE0(key, side, d): fill `d` with a summon (type 0xA) or return −1:
/// summon flag clear, both zones full, no category. The target search
/// 0x278850 runs with see-all 1. Category 0: 0x137450; without a target
/// and with four or more monsters the non-monster choice (filtered by the
/// chosen card) takes over. Category 2: 0x137800 lists the card and its
/// power-ups. Category 1: 0x137250 filtered by U's card. Then 0x1379F0.
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
    // 0x278850(U, col, row, other, 3, 0x1F, 2, 2, [see-all 1, hidden 0]).
    let target = |u: &Option<Unit>| {
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
            1,
            0,
        )
    };
    let with_u = |u: &Option<Unit>| match u {
        Some(u) => Ok(u.clone()),
        None => unsupported("0x136AE0 reads an uninitialised U"),
    };
    let tgt;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, PEGASUS_CHOICE)? < 0 {
            // Unreachable (0x136D60 found an affordable monster): the
            // original falls into the non-monster path with an
            // uninitialised target register.
            return unsupported("0x136AE0 uses an uninitialised target");
        }
        tgt = target(&u)?;
        if tgt < 0 && w.monsters(side) >= 4 {
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
