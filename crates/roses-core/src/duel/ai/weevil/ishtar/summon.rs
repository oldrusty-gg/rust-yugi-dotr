//! Ishtar's summon planner 0x123600 and its subtree: the category choice
//! 0x124010 with the monster mask 0x124640 and its kind test 0x124860,
//! the special-card search 0x124910, the card list 0x125560 and the
//! summon cell 0x1238E0 are his own; the hand search 0x124DF0 and the
//! non-monster choice 0x124EE0 are Pegasus's 0x137160 / 0x137250 by
//! opcode shape and shared with it ([`super::super::pegasus::summon`]);
//! the monster choice 0x1250E0 is Weevil's 0x10EBF0 with a root's best
//! node 0x217 skipped and plain scoring below 1500 ATK
//! ([`ChoiceExtra`]), the fodder pass 0x125300 Weevil's 0x10EF80 with the
//! limit 0xFA1 and the card test 0x1254E0.

use super::super::pegasus::summon::{find_card, non_monster, zone};
use super::super::search::{attack_path, find_target, path, step_count, Steps};
use super::super::summon::{
    affordable, kind_of, monster_choice_with, neighbour, popcount, virt, ChoiceExtra,
    FodderFilter, Hand, MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{
    committed_face_up_others, hidden_value, id, kind_count, lane_cell, ritual_ok, card_count,
};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x1250E0 / 0x125300: 0x27A850 scoring (no tier test), Weevil's fodder
/// pass with the limit 0xFA1 and the card test 0x1254E0 (any
/// non-monster, or a monster of kind 0, 0xF or 0x12 is never fodder).
const ISHTAR_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: 0xFA1,
    fodder_filter: FodderFilter::NonMonsterOrKinds(&[0, 0xF, 0x12]),
    skip_root: 999,
};

/// 0x1250E0: a root whose best node is 0x217 is skipped; a best node
/// below 1500 printed ATK is scored by 0x2714E0.
const ISHTAR_EXTRA: ChoiceExtra = ChoiceExtra {
    skip_top: id::SKIPPED_TOP,
    plain_below_atk: 0x5DC,
    ..ChoiceExtra::NONE
};

/// 0x123600: the depth of the target search and the route limit of the
/// cell searches of 0x1238E0.
const TARGET_SEARCH_DEPTH: i32 = 2;
const CELL_ROUTE_LIMIT: i32 = 3;
/// 0x1238E0: the route limit of the lane-goal fallback (`slti 0xE`) and
/// the terrain of its last resort.
const LANE_LEN_LIMIT: i32 = 0xE;
const FALLBACK_TERRAIN: i32 = 4;
/// 0x124910: the attribute (+0x3F, 0x1E1AB0) counted for card 0x2E6 and
/// the spellbind value (+0x49) of a unit bound for good.
const COUNTED_ATTRIBUTE: i32 = 4;
const BOUND_FOR_GOOD: i32 = 0xFF;

/// 0x124640(side, hand): a bit mask of the affordable monsters of the
/// predicted hand (bit 4 − index) as written: after every hit the index
/// takes the answer of the hand-size call that follows (0x1246D0 copies
/// its return), so the scan ends at the first hit and the mask holds one
/// bit. Then, with a non-zero mask, the same scan over cards of class 0x12
/// and, when the mask is exactly 2, over class 0xF (card byte 0 >> 5 never
/// exceeds 7, so both add nothing); the mask when it is 2, else 0.
fn monster_mask<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    hand: &Hand,
) -> R<i32> {
    let scan = |mask: &mut i32, class: i32| -> R<()> {
        let mut i = 0;
        while i < w.next_hand_size(side) {
            let c = hand.get(i)?;
            if affordable(w, side, c)? && kind_of(w, c) >> 5 == class {
                *mask = (*mask + (1 << (4 - i))) & 0xFF;
                i = w.next_hand_size(side);
            }
            i += 1;
        }
        Ok(())
    };
    let mut mask = 0;
    scan(&mut mask, 0)?;
    if mask & 0xFF == 0 {
        return Ok(0);
    }
    scan(&mut mask, 0x12)?;
    if mask & 0xFF != 2 {
        return Ok(0);
    }
    scan(&mut mask, 0xF)?;
    Ok(mask & 0xFF)
}

/// 0x124860(side, hand, card): 0 when the [`monster_mask`] has three bits,
/// else 1 for a card of class 0, 0xF or 0x12 and 0 otherwise.
fn mask_kind_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    hand: &Hand,
    card: i32,
) -> R<i32> {
    let class = kind_of(w, card & 0xFFFF) >> 5;
    let mask = monster_mask(w, side, hand)?;
    if popcount(mask & 0xFF) == 3 {
        return Ok(0);
    }
    Ok(i32::from(matches!(class, 0 | 0xF | 0x12)))
}

/// 0x124910(side, d, U): the special cards, in order: 0x33D in hand;
/// 0x34C with acceptable tributes ([`ritual_ok`]); 0x332; 0x2E6 with three
/// or more own units of attribute 4 in the slots 0..=5 of either group;
/// Cursebreaker with an own unit bound for good (+0x49 = 0xFF, slots
/// 0..=10); 0x334 with an own 0x33D in the slots 6..=10. The first hand
/// index holding the card: d+6 = index, U = 0x1DDFE0(card), returns the
/// card's class; −1 when none applies.
fn special<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
) -> R<i32> {
    let hand = Hand::of(w, side);
    let n = w.next_hand_size(side);
    let other = i32::from(side == 0);
    let pick = |d: &mut Decision, u: &mut Option<Unit>, card: i32| -> R<i32> {
        for i in 0..n {
            let c = hand.get(i)?;
            if c != card {
                continue;
            }
            d.hand[0] = i as u8;
            *u = Some(virt(w, c));
            return Ok(kind_of(w, c) >> 5);
        }
        Ok(-1)
    };
    let v = pick(d, u, id::TRAP_33D)?;
    if v >= 0 {
        return Ok(v);
    }
    if ritual_ok(w, side, id::RITUAL_34C)? == 1 {
        let v = pick(d, u, id::RITUAL_34C)?;
        if v >= 0 {
            return Ok(v);
        }
    }
    let v = pick(d, u, id::TRAP_332)?;
    if v >= 0 {
        return Ok(v);
    }
    let attribute = |g: i32, s: i32| w.unit(g, s).map_or(-1, |x| i32::from(x.attribute));
    let mut count = 0;
    for slot in 0..6 {
        for grp in [side, other] {
            if w.ctrl(grp, slot) == side && attribute(grp, slot) == COUNTED_ATTRIBUTE {
                count += 1;
            }
        }
    }
    if count >= 3 {
        let v = pick(d, u, id::MAGIC_2E6)?;
        if v >= 0 {
            return Ok(v);
        }
    }
    let mut bound = 0;
    for slot in 0..11 {
        for grp in [side, other] {
            if w.ctrl(grp, slot) == side && w.spellbind(grp, slot) == BOUND_FOR_GOOD {
                bound += 1;
            }
        }
    }
    if bound > 0 {
        let v = pick(d, u, id::CURSEBREAKER)?;
        if v >= 0 {
            return Ok(v);
        }
    }
    let mut set = 0;
    for slot in 6..11 {
        for grp in [side, other] {
            if w.ctrl(grp, slot) == side && w.card_id(grp, slot) == id::TRAP_33D {
                set += 1;
            }
        }
    }
    if set > 0 {
        let v = pick(d, u, id::TRAP_334)?;
        if v >= 0 {
            return Ok(v);
        }
    }
    Ok(-1)
}

/// The first affordable monster of the predicted hand (the loops of
/// 0x124010), as a card id.
fn affordable_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
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

/// 0x124010(side, U): what to summon: 0 a monster (U set by the caller's
/// 0x1250E0 unless set here), 1 a non-monster (U set), 2 a particular
/// card (U set by 0x124910 / 0x124DF0), 3 the three-kind list (never: the
/// [`monster_mask`] has no three bits), −1 nothing. In order: monster
/// zone full → the non-monster choice; magic/trap zone full → the first
/// affordable monster; the three-kind list; a special card ([`special`]);
/// with no monster out, the first affordable monster whose
/// [`mask_kind_ok`] answers 0 (never for a monster); 0x235 with an own
/// committed face-up card in the slots 6..=10; 0x251 with three own
/// kind-0x10 units; 0x230 with two own 0x258; 0x258 with two own 0x230;
/// 0x54 while the leader is threatened (0x27D500, see-all 0); the first
/// affordable monster; the non-monster choice.
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
    let mask = monster_mask(w, side, &hand)?;
    if popcount(mask & 0xFF) == 3 && card_list(w, side, &mut local, u, 1)? >= 0 {
        return Ok(3);
    }
    if special(w, side, &mut local, u)? >= 0 {
        return Ok(2);
    }
    if monsters == 0 {
        for i in 0..w.next_hand_size(side) {
            let c = hand.get(i)?;
            if kind_of(w, c) >> 5 != 0 || !affordable(w, side, c)? {
                continue;
            }
            if mask_kind_ok(w, side, &hand, c)? == 0 {
                *u = Some(virt(w, c));
                return Ok(0);
            }
        }
    }
    if committed_face_up_others(w, side) > 0
        && find_card(w, side, &mut local, u, id::FACE_UP_COUNTER)? >= 0
    {
        return Ok(2);
    }
    if kind_count(w, side, 0x10) >= 3 && find_card(w, side, &mut local, u, id::KIND_10_TRIO)? >= 0
    {
        return Ok(2);
    }
    if card_count(w, side, id::PAIR_A) >= 2 && find_card(w, side, &mut local, u, id::PAIR_B)? >= 0
    {
        return Ok(2);
    }
    if card_count(w, side, id::PAIR_B) >= 2 && find_card(w, side, &mut local, u, id::PAIR_A)? >= 0
    {
        return Ok(2);
    }
    if w.threatened(side, 0, false)? && find_card(w, side, &mut local, u, id::GUARD)? >= 0 {
        return Ok(2);
    }
    if let Some(c) = affordable_monster(w, side, &hand)? {
        *u = Some(virt(w, c));
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// 0x125560(side, d, U, a3): d+6.. = 0xFF (−1 without U). `a3` = 0: d+6 =
/// the LAST hand index holding U's card that is affordable. Otherwise
/// (the [`monster_mask`] is computed and dropped): d+6 / d+7 / d+8 = the
/// LAST affordable monster of kind 0 / 0x12 / 0xF. Returns 0.
fn card_list<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &Option<Unit>,
    a3: i32,
) -> R<i32> {
    d.hand = [0xFF; 5];
    let hand = Hand::of(w, side);
    let Some(u) = u else {
        return Ok(-1);
    };
    let n = w.next_hand_size(side);
    if a3 & 0xFF == 0 {
        for i in 0..n {
            if i == i32::from(d.hand[0]) || i == i32::from(d.hand[1]) {
                continue;
            }
            let c = hand.get(i)?;
            if c == i32::from(u.card) && affordable(w, side, c)? {
                d.hand[0] = i as u8;
            }
        }
        return Ok(0);
    }
    monster_mask(w, side, &hand)?;
    for (slot, kind) in [(0usize, 0), (1, 0x12), (2, 0xF)] {
        for i in 0..n {
            let c = hand.get(i)?;
            let k = kind_of(w, c);
            if k >> 5 == 0 && k & 0x1F == kind && affordable(w, side, c)? {
                d.hand[slot] = i as u8;
            }
        }
    }
    Ok(0)
}

/// 0x1238E0(side, _, U, tgt): the summon cell among the leader's
/// neighbours (0x24FD30) that are not Labyrinth and, on Crush, would not
/// crush U (0x251760).
/// - A monster or trap (class 0 / 3) with a target: the empty neighbour
///   with the fewest moves to it: 0x27BA70 (see-all 0, at most 3, hidden
///   strength 0x125D70(side) & 0xFFFF) when 0x279E50 (that strength,
///   see-all 0) is positive, else 0x27AC00 (at most 3), counted only when
///   the step buffer (filled once, never reset) holds a candidate; strict
///   < from 0xFF. −1 when no neighbour has a route.
/// - A monster or trap without a target: the empty neighbour whose lane
///   goal ([`lane_cell`]) is the fewest moves away (strict < from 14);
///   else the first empty neighbour on terrain 4; else −1.
/// - Other classes: the LAST empty neighbour.
fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
    tgt: i32,
) -> R<i32> {
    // sp+0xD8: the step buffer.
    let mut buf = Steps::new();
    let open = |col: i32, row: i32| -> R<bool> {
        let t = w.terrain(col, row);
        if t == 8 || (t == 9 && w.crushed(u, col, row)?) {
            return Ok(false);
        }
        Ok(true)
    };
    let class = i32::from(u.kind >> 5);
    if class != 0 && class != 3 {
        let mut last = -1;
        for i in 0..8 {
            let cell = neighbour(w, side, i)?;
            if cell < 0 {
                continue;
            }
            let (col, row) = (cell & 0xFF, cell >> 8);
            if !open(col, row)? || w.at_key(col, row) >= 0 {
                continue;
            }
            if row < 8 {
                last = cell;
            }
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
    for i in 0..8 {
        let cell = neighbour(w, side, i)?;
        if cell < 0 {
            continue;
        }
        let (col, row) = (cell & 0xFF, cell >> 8);
        if !open(col, row)? || w.at_key(col, row) >= 0 {
            continue;
        }
        let (_, len) = lane_cell(w, Some(u), col, row)?;
        if i32::from(len) < best_len {
            best = cell;
            best_len = i32::from(len);
        }
    }
    if best_len < LANE_LEN_LIMIT {
        return Ok(best);
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
        if w.terrain(col, row) == FALLBACK_TERRAIN {
            return Ok(cell);
        }
    }
    Ok(-1)
}

/// 0x123600(key, side, d): fill `d` with a summon (type 0xA) or return −1:
/// summon flag clear, both zones full, no category. The target search
/// 0x278850 runs from the key's cell with depth 2, see-all 0 and the
/// hidden strength 0x125D70(side). Category 0: 0x1250E0; with a target
/// the cell at once; without one and with fewer than four monsters out
/// the cell without a target; else the non-monster choice (filtered by
/// U's card) takes over. Category 3: 0x125560(1); category 2:
/// 0x125560(0), both with the target; other categories: the non-monster
/// choice and no target. Then 0x1238E0.
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
    // 0x278850(U, col, row, other, 2, 0x1F, 2, 2, [0, 0x125D70(side)]).
    let target = |u: &Option<Unit>| {
        let hidden = hidden_value(w, side);
        find_target(
            w,
            u.as_ref(),
            pos & 0xFF,
            pos >> 8,
            other,
            TARGET_SEARCH_DEPTH,
            0x1F,
            2,
            2,
            0,
            hidden,
        )
    };
    let with_u = |u: &Option<Unit>| match u {
        Some(u) => Ok(u.clone()),
        None => unsupported("0x123600 reads an uninitialised U"),
    };
    let mut tail = true;
    let mut tgt = if r == 0 {
        if monster_choice_with(w, side, d, &mut u, ISHTAR_CHOICE, ISHTAR_EXTRA)? < 0 {
            // 0x1236C8: no root scored (every best node was 0x217): the
            // non-monster path takes U's card (the category's first
            // affordable monster) with no target search and no target.
            -1
        } else {
            let tgt = target(&u)?;
            if tgt >= 0 || w.monsters(side) < 4 {
                tail = false;
            }
            tgt
        }
    } else {
        target(&u)?
    };
    if tail {
        match r {
            3 => {
                card_list(w, side, d, &u, 1)?;
            }
            2 => {
                card_list(w, side, d, &u, 0)?;
            }
            _ => {
                let card = i32::from(with_u(&u)?.card);
                if non_monster(w, side, d, &mut u, card)? < 0 {
                    return Ok(-1);
                }
                tgt = -1;
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
