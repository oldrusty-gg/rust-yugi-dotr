//! The generic module's reveal request 0x16BEE0 (Seto's 0x111BF0 with the
//! type-0x21 magic threshold 2500, all its branches live here) and its
//! helpers: Seto's 0x1119A0 / 0x112C90 (threshold 2500 in 0x16CF80) /
//! 0x112ED0 ([`super::super::seto::reveal`]), the ritual test
//! [`ritual_below_result`] 0x16D6D0 (= Seto's 0x113120) and the Exodia
//! tests 0x16D7E0 / 0x16D800 / 0x16D8C0.

use super::super::seto::reveal::{control_target, f112c90_with, f112ed0, global_mask};
use super::super::summon::{
    atk_def, nth_unit, popcount, ritual_spec, tributes, unit_mask, virt,
};
use super::super::world::{World, R};
use super::super::{has_effect, FusionData, Record};
use super::{
    any_spellbound, enemy_leader_rank, exodia_assembled, exodia_piece, face_up_word,
    graveyard_monsters, GRAVE_MONSTERS, RANK_HOLD,
};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x16CF80: the strongest affected enemy's threshold (`slti 0x9C5`).
const MASS_THRESHOLD: i32 = 2500;
/// 0x16C468: the type-0x21 magic's enemy ATK threshold (`slti 0x9C4`).
const STRONG_ENEMY_ATK: i32 = 0x9C4;
/// 0x16C238: 0x27E0B0's minimum and radius.
const OPEN_MIN: i32 = 8;
const OPEN_RADIUS: i32 = 2;
/// Crush terrain (9) and Labyrinth (8).
const CRUSH: i32 = 9;
const LABYRINTH: i32 = 8;
/// 0x16C300: the LP below which the relation-1 type-0xD magic reveals
/// (`slti 0x4B1`).
const LOW_LP: i32 = 0x4B1;
/// 0x16C31C: the type-0xD filter that counts enemy units (0x2500A0 mode 3).
const COUNT_FILTER: u32 = 0x32;
/// 0x16CCEC / 0x16CA4C: the filters entry-3 types 4 / 0x17 / 0x1F refuse
/// (0x1A) and the one type 4 sends to 0x16CF80 (0x30).
const FILTER_1A: u32 = 0x1A;
const FILTER_30: u32 = 0x30;

/// 0x16D6D0(side, card) (= Seto's 0x113120): 1 when the ritual's tributes
/// (0x2561B0 on the 0x2565B0 spec of entry 0's word 1) can be chosen and
/// each one's ATK at its cell (0x252300) is below the printed ATK of the
/// card word 1 names (0x2714E0, unsigned compare); else 0.
pub(super) fn ritual_below_result<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    // 0x2715C0(card, 0): a card without an entry reads word 1 from a null
    // entry pointer.
    let Some([_, word1]) = virt(w, card).entries[0] else {
        return super::super::world::unsupported("0x16D6D0 reads entry 0 of a card without one");
    };
    let limit = atk_def(w, word1 as i32) >> 16;
    let m = tributes(w, &ritual_spec(word1), side)?;
    if m < 0 {
        return Ok(0);
    }
    for i in 0..popcount(m) {
        let k = nth_unit(m, i);
        let (g, s) = (k >> 8, k & 0xFF);
        let p = w.pos(g, s);
        if w.eff_atk(g, s, p & 0xFF, p >> 8)? as u32 >= limit {
            return Ok(0);
        }
    }
    Ok(1)
}

/// 0x24FB40(side, 1): the first neighbour of the side's leader (0x24FD30
/// order) that is on the board, not Labyrinth and empty (0x1E4A10); −1.
fn free_neighbour<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    for i in 0..8 {
        let cell = super::super::summon::neighbour(w, side, i)?;
        if cell < 0 {
            continue;
        }
        let (col, row) = (cell & 0xFF, cell >> 8);
        if w.terrain(col, row) == LABYRINTH || w.at_key(col, row) >= 0 {
            continue;
        }
        return Ok(cell);
    }
    Ok(-1)
}

/// 0x27CF90 ([`super::super::keith::pick_strongest`]).
fn pick_strongest<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    list: &[i32],
) -> R<i32> {
    super::super::keith::pick_strongest(w, list)
}

/// 0x252300 at the unit's own cell (a missing key reads as 0x1E2310's −1
/// cell, which the port's ATK query answers for a missing unit).
fn atk_here<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, k: i32) -> R<i32> {
    let (g, s) = (k >> 8, k & 0xFF);
    let p = w.pos(g, s);
    w.eff_atk(g, s, p & 0xFF, p >> 8)
}

/// The units of `mask` committed face-up (+0x57 bit 4).
fn face_up_count<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, mask: i32) -> i32 {
    (0..popcount(mask))
        .filter(|&i| {
            let k = nth_unit(mask, i);
            w.flag(k >> 8, k & 0xFF, 4)
        })
        .count() as i32
}

/// 0x16BEE0(g, s, rec): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. An Exodia piece whose controller's leader is card
/// 0x3A with the four pieces in its saved deck list and all four next to
/// it ([`exodia_assembled`]): 1. By class: monsters → [`monster`];
/// magic → [`magic`]; rituals: 2 under a type-0xE far trap (0x27F170 mode
/// 6, see-all 0), 1 when [`ritual_below_result`] allows it and the own
/// cell is not Crush, else 2; others 2.
pub(super) fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    let pos = w.pos(g, s);
    let ctrl = w.ctrl(g, s);
    // 0x24E230(controller, 2, sp+0xA0): a list later reused as a buffer.
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    if exodia_piece(card) && exodia_assembled(w, ctrl)? {
        return Ok(1);
    }
    match w.kind(g, s) >> 5 {
        0 => monster(w, g, s, ctrl, pos),
        1 => magic(w, g, s, ctrl, card, pos),
        5 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            if ritual_below_result(w, ctrl, card)? != 1
                || w.terrain(pos & 0xFF, pos >> 8) == CRUSH
            {
                return Ok(2);
            }
            Ok(1)
        }
        _ => Ok(2),
    }
}

fn yes(b: bool) -> u32 {
    if b {
        1
    } else {
        2
    }
}

/// The magic branch (0x16C06C): 2 under a type-0xE far trap or without an
/// entry; by entry 0's type: 0x28 → the controller's leader threatened
/// (0x27D500, see-all 0); 0x27 → fewer than 4 stars; 0x26 → summon flag
/// clear; 0x25 → filter 0x21: the card itself threatened, else the leader;
/// 0x36 → 1; 0x21 → the enemy's strongest face-up monster (0x27CF90 over
/// 0x27DC00(other, 0x1D, 0, 0)) has 2500 or more ATK and beats the own
/// strongest (over 0x27D9B0(controller, 0x1D, 1)); 0x12 → 3 or more own
/// units of the filter; 0x20 → a slot of group `g` bound by 0xFF; 0x1F →
/// card 0x2DD with a control target (0x1119A0); 0xD → relation 1: word-1
/// type 4 → LP below 1201, other types 1; filter 0x32 → 4 or more enemy
/// units in slots 1..=10 (0x2500A0 mode 3); other filters 1; 4 → word-1
/// type 0x12: the 0x112ED0 target shares the card's row or column; other
/// types (0x13 included): 0x112C90; 2 → the open-area centre 0x27E0B0(u,
/// c, r, word-1 low byte, 8, 2) is the own cell; 0x18 → 6 or more of the
/// enemy's face-up units (rel 2, filter 0x34); 0x49 / 0x17 → 0x112C90;
/// others 2.
fn magic<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    ctrl: i32,
    card: i32,
    pos: i32,
) -> R<u32> {
    if w.classify(g, s, 6, false)? == 5 {
        return Ok(2);
    }
    let Some([w0, w1]) = w.unit(g, s).and_then(|u| u.entry(0)) else {
        return Ok(2);
    };
    let filter = w0 & 0x3F;
    let relation = (w0 >> 6 & 3) as i32;
    let other = i32::from(ctrl == 0);
    let mass = |relation: i32, filter: u32| -> R<u32> {
        Ok(yes(f112c90_with(w, ctrl, relation, filter, MASS_THRESHOLD)? == 1))
    };
    Ok(match w0 >> 8 {
        0x28 => yes(w.threatened(ctrl, 0, false)?),
        0x27 => yes(w.stars(ctrl) < 4),
        0x26 => yes(!w.can_summon(ctrl)),
        0x25 => {
            if filter == 0x21 {
                yes(w.threatened(g, s, false)?)
            } else {
                yes(w.threatened(ctrl, 0, false)?)
            }
        }
        0x36 => 1,
        0x21 => {
            let enemy = pick_strongest(w, &w.list_seen(other, 0x1D, false, false))?;
            let own = pick_strongest(w, &w.list(ctrl, 0x1D, true))?;
            let e = atk_here(w, enemy)?;
            if e < STRONG_ENEMY_ATK {
                return Ok(2);
            }
            yes(atk_here(w, own)? < e)
        }
        0x12 => yes(popcount(global_mask(w, ctrl, 1, filter)?) >= 3),
        0x20 => yes((0..11).any(|slot| w.spellbind(g, slot) == 0xFF)),
        0x1F => yes(card == 0x2DD && control_target(w, ctrl)? >= 0),
        0xD => {
            if relation == 1 {
                if w1 >> 16 == 4 {
                    yes(w.lp(ctrl) < LOW_LP)
                } else {
                    1
                }
            } else if filter == COUNT_FILTER {
                yes((1..11).filter(|&slot| w.unit(other, slot).is_some()).count() >= 4)
            } else {
                1
            }
        }
        4 => {
            if w1 >> 16 == 0x12 {
                let t = f112ed0(w, g, [w0, w1])?;
                yes(t >= 0 && (pos & 0xFF == t & 0xFF || pos >> 8 == t >> 8))
            } else {
                mass(relation, filter)?
            }
        }
        2 => {
            let Some(u) = w.unit(g, s) else {
                return Ok(2);
            };
            let v = super::super::plans::open_area(
                w,
                u,
                pos & 0xFF,
                pos >> 8,
                (w1 & 0xFF) as i32,
                OPEN_MIN,
                OPEN_RADIUS,
            )?;
            yes(v >= 0 && v == pos)
        }
        0x18 => yes(face_up_count(w, unit_mask(w, 0, 0, ctrl, 2, 0x34, 0xE)?) >= 6),
        0x49 | 0x17 => mass(relation, filter)?,
        _ => 2,
    })
}

/// The monster branch (0x16C59C): 2 without an effect entry (0x2520F0) or
/// without entry 2; by entry 2 (group `g`): 0x58 → 1 when the enemy
/// leader's saved rank is 6 or more; 0x55 / 0x47 → 1 when an own / enemy
/// unit is bound by a spell other than 0xFF; 0x54 → 1 when the board's
/// strongest monster is another of group `g`'s; 0x53 → 1 with 6 or more
/// monsters in both graveyards (the count stays in s1 for entry 3);
/// 0x17 / 0x13 → bit 6 and an own unit of the filter → 2; 0x27FA90 of the
/// enemy's units ≥ 3 (unsigned) → 1; 6, 0x4D, 0x12, 0x4E, 0xB → the own
/// MASK of the filter ≥ 2 (unsigned) → 1; 0xF, 0x43, 0x56, 0x3B, 0x22,
/// 0xE, 0xD, 0x34, 0xC → 1. Otherwise [`entry3`] (2 without entry 3).
fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    ctrl: i32,
    pos: i32,
) -> R<u32> {
    if has_effect(w, g, s) == 0 {
        return Ok(2);
    }
    let Some(u) = w.unit(g, s) else {
        return Ok(2);
    };
    let Some([w0, _]) = u.entry(2) else {
        return Ok(2);
    };
    let filter = w0 & 0x3F;
    // s0 / s1: registers the entry-3 branches read on (quirks).
    let mut s0 = 0;
    let mut s1 = 0;
    match w0 >> 8 {
        0x58 => {
            if enemy_leader_rank(w, g) >= RANK_HOLD {
                return Ok(1);
            }
        }
        0x55 => {
            s0 = unit_mask(w, 0, 0, g, 1, 0x34, 0xE)?;
            if any_spellbound(w, s0) {
                return Ok(1);
            }
        }
        0x54 => {
            s0 = unit_mask(w, 0, 0, g, 3, 0x21, 0xE)?;
            let k = nth_unit(s0, 0);
            if w.ctrl(k >> 8, k & 0xFF) == g && k != (g << 8 | s) {
                return Ok(1);
            }
        }
        0x53 => {
            s1 += graveyard_monsters(w, g);
            if s1 >= GRAVE_MONSTERS {
                return Ok(1);
            }
        }
        0x47 => {
            s0 = unit_mask(w, 0, 0, g, 2, 0x34, 0xE)?;
            if any_spellbound(w, s0) {
                return Ok(1);
            }
        }
        0x17 | 0x13 => {
            if w0 >> 6 & 1 == 1 && global_mask(w, g, 1, filter)? != 0 {
                return Ok(2);
            }
            s0 = face_up_word(w, global_mask(w, g, 2, filter)?);
            if s0 as u32 >= 3 {
                return Ok(1);
            }
        }
        6 | 0x4D | 0x12 | 0x4E | 0xB => {
            s0 = global_mask(w, g, 1, filter)?;
            if s0 as u32 >= 2 {
                return Ok(1);
            }
        }
        0xF | 0x43 | 0x56 | 0x3B | 0x22 | 0xE | 0xD | 0x34 | 0xC => return Ok(1),
        _ => (),
    }
    entry3(w, g, s, ctrl, pos, s0, s1)
}

/// The monster branch's entry-3 tests (0x16C9DC; controller `c`): 0x57 →
/// 1 with an empty hand (0x1E0390); 0xC, 0xD → 1; 0x46 → the leader
/// threatened; 0x3C → a unit within 1 of the own cell matching the word
/// (word-1 low half << 16 | filter, relation 1) and a free leader
/// neighbour of the enemy (0x24FB40); 0x1F → no continuous 0x39 effect,
/// filter not 0x1A, 3 or more units of the filter and relation; 0x26 →
/// summon flag clear; 0x1A → group `g` has fewer than 4 stars; 4 → filter
/// 0x1A → 2; 0x30 → 0x112C90, else the units of s0 (quirk: the entry-2
/// register) counted; other filters: the units of the filter and relation
/// (around the own cell with word-1 mode when it is not 0): 2 when one is
/// own, else 2 or more face-up → 1; 0x18 → 3 or more own face-up units of
/// the filter (counted on from s1) → 2, else 6 or more enemy ones → 1;
/// 0x23 → 0x112C90; 0x17 → filter not 0x1A and 0x112C90; 0x33 → 3 or more
/// own units of the word; 0x12 → 3 or more own units of the filter (word-1
/// type 0x16: mode 0x16; else the word in mode 0xE); others 2.
#[allow(clippy::too_many_arguments)]
fn entry3<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    ctrl: i32,
    pos: i32,
    s0: i32,
    mut s1: i32,
) -> R<u32> {
    let Some([w0, w1]) = w.unit(g, s).and_then(|u| u.entry(3)) else {
        return Ok(2);
    };
    let filter = w0 & 0x3F;
    let relation = (w0 >> 6 & 3) as i32;
    let other = i32::from(ctrl == 0);
    let mass = |relation: i32, filter: u32| -> R<bool> {
        Ok(f112c90_with(w, ctrl, relation, filter, MASS_THRESHOLD)? == 1)
    };
    // Every unit of the mask: 2 when one is the controller's, else
    // 1 with two or more of them committed face-up (counted on from s1).
    let enemies_face_up = |mask: i32, mut s1: i32| -> u32 {
        for i in 0..popcount(mask) {
            let k = nth_unit(mask, i);
            if w.ctrl(k >> 8, k & 0xFF) == ctrl {
                return 2;
            }
            if w.flag(k >> 8, k & 0xFF, 4) {
                s1 += 1;
            }
        }
        yes(s1 >= 2)
    };
    Ok(match w0 >> 8 {
        0x57 => yes(w.hand_count(ctrl) == 0),
        0xC | 0xD => 1,
        0x46 => yes(w.threatened(ctrl, 0, false)?),
        0x3C => {
            let word = (w1 & 0xFFFF) << 16 | filter;
            let m = unit_mask(w, pos & 0xFF, pos >> 8, ctrl, 1, word, 1)?;
            yes(m != 0 && free_neighbour(w, other)? >= 0)
        }
        0x1F => {
            if w.continuous_count(0x39) > 0 || filter == FILTER_1A {
                2
            } else {
                yes(popcount(global_mask(w, ctrl, relation, filter)?) >= 3)
            }
        }
        0x26 => yes(!w.can_summon(ctrl)),
        0x1A => yes(w.stars(g) < 4),
        4 => {
            if filter == FILTER_1A {
                2
            } else if filter == FILTER_30 {
                if mass(relation, FILTER_30)? {
                    1
                } else {
                    enemies_face_up(s0, s1)
                }
            } else {
                let mode = (w1 >> 16) as i32;
                let mask = if mode == 0 {
                    global_mask(w, ctrl, relation, filter)?
                } else {
                    unit_mask(w, pos & 0xFF, pos >> 8, ctrl, relation, filter, mode)?
                };
                enemies_face_up(mask, s1)
            }
        }
        0x18 => {
            s1 += face_up_count(w, global_mask(w, ctrl, 1, filter)?);
            if s1 >= 3 {
                2
            } else {
                yes(face_up_count(w, global_mask(w, ctrl, 2, filter)?) >= 6)
            }
        }
        0x23 => yes(mass(relation, filter)?),
        0x17 => {
            if filter == FILTER_1A {
                2
            } else {
                yes(mass(relation, filter)?)
            }
        }
        0x33 => {
            let word = (w1 >> 16) << 16 | filter;
            yes(popcount(global_mask(w, ctrl, 1, word)?) >= 3)
        }
        0x12 => {
            let mask = if w1 >> 16 == 0x16 {
                unit_mask(w, 0, 0, ctrl, 1, filter, 0x16)?
            } else {
                global_mask(w, ctrl, 1, (w1 >> 16) << 16 | filter)?
            };
            yes(popcount(mask) >= 3)
        }
        _ => 2,
    })
}

#[cfg(test)]
mod tests {
    use super::super::{ones, remove_raw};

    /// 0x24E0F0 with ordinary arguments removes the unit's bit.
    #[test]
    fn remove_raw_clears_the_bit() {
        assert_eq!(remove_raw(0x3F_FFFF, 0, 3), 0x3F_FFFF & !(1 << 18));
        assert_eq!(remove_raw(0x3F_FFFF, 1, 4), 0x3F_FFFF & !(1 << 6));
        assert_eq!(ones(0), 0);
        assert_eq!(ones(3), 7);
    }
}
