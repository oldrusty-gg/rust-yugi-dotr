//! Seto reveal queries. Addresses refer to PAL SLES_515.79.
//!
//! 0x111BF0 has a single JAL caller, 0x11839C, reached only for magic
//! cards whose first entry is not type 2, 0xD or 0x14. Its unrelated
//! monster/ritual branches are not part of the reachable controller path.

use super::super::pegasus::protected_filter;
use super::super::summon::{
    kind_of, nth_unit, popcount, ritual_spec, tributes, unit_list, unit_mask, virt,
};
use super::super::world::{unsupported, World, R};
use super::super::{FusionData, Record};
use crate::duel::CardData;

/// 0x1184A0: existence of a ritual tribute set, without an ATK ceiling.
pub(super) fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    if card == 999 {
        return unsupported("Seto 0x1184A0 reads entry 0 of card 999");
    }
    let word = virt(w, card).entries[0].map_or(u32::MAX, |e| e[1]);
    Ok(i32::from(tributes(w, &ritual_spec(word), side)? >= 0))
}

/// 0x1183E0(position): the centre always succeeds; its four orthogonal
/// neighbours succeed only when an enemy of the active side occupies it.
pub(super) fn center_reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    pos: i32,
) -> i32 {
    if pos == 0x303 {
        return 1;
    }
    let Some(center) = w.at(3, 3) else {
        return 0;
    };
    i32::from(
        center.ctrl() != i32::from(w.active)
            && ((pos & 0xFF) - 3).abs() + ((pos >> 8) - 3).abs() == 1,
    )
}

/// 0x118140(g, s, rec): the controller's reveal request.
pub(super) fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    g: i32,
    s: i32,
    rec: &Record,
) -> R<u32> {
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    let card = w.card_id(g, s);
    let ctrl = w.ctrl(g, s);
    match w.kind(g, s) >> 5 {
        0 => match card {
            0x53 => {
                let two = unit_mask(w, 0, 0, ctrl, 1, 0x2001C, 0xE)?;
                let zero = if two == 0 {
                    unit_mask(w, 0, 0, ctrl, 1, 0x1C, 0xE)?
                } else {
                    0
                };
                Ok(if two != 0 || zero != 0 { 1 } else { 2 })
            }
            0x92 => {
                // 0x252260(controller, 0) counts graveyard monsters.
                let n = (0..w.graveyard_count(ctrl))
                    .filter(|&i| kind_of(w, w.graveyard(ctrl, i)) >> 5 == 0)
                    .count();
                Ok(if n >= 6 { 1 } else { 2 })
            }
            _ => Ok(2),
        },
        5 => {
            if w.classify(g, s, 6, false)? == 5 || ritual_ok(w, ctrl, card)? != 1 {
                return Ok(2);
            }
            // 0x1182E4/EC passes group and slot as cell coordinates to
            // 0x1E3530. The earlier position result was discarded.
            Ok(if w.terrain(g, s) == 9 { 2 } else { 1 })
        }
        1 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            // 0x118330 dereferences its entry pointer without a null check.
            let Some(entry) = w.unit(g, s).and_then(|u| u.entry(0)) else {
                return unsupported("Seto 0x118330 reads a null magic entry");
            };
            match entry[0] >> 8 {
                0x14 => Ok(2),
                0xD => Ok(1),
                2 => Ok(if center_reveal(w, w.pos(g, s)) == 1 {
                    1
                } else {
                    2
                }),
                _ => special_reveal(w, g, s, rec),
            }
        }
        _ => Ok(2),
    }
}

/// 0x1119A0(side): the two control targets packed in high/low halves.
/// Unlike Pegasus's otherwise matching helper, 0x27D9B0 see-all is 0.
pub(in super::super) fn control_target<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    side: i32,
) -> R<i32> {
    let other = i32::from(side == 0);
    let lp = w.pos(other, 0);
    if w.continuous_count(0x39) > 0 {
        return Ok(-1);
    }
    let strongest = protected_filter(w, unit_mask(w, 0, 0, side, 2, 0x21, 0xE)?, side)?;
    let mut last = -1;
    let mut best = -1;
    let mut threshold = 1;
    for k in w.list(other, 0x34, false) {
        let (g, s) = (k >> 8, k & 0xFF);
        let p = w.pos(g, s);
        if ((p & 0xFF) - (lp & 0xFF)).abs() + ((p >> 8) - (lp >> 8)).abs() != 1
            || w.flag(g, s, 1)
            || w.spellbind(g, s) != 0
            || w.x4a(g, s) != 0
        {
            continue;
        }
        let atk = w.eff_atk(g, s, p & 0xFF, p >> 8)?;
        if atk < threshold {
            continue;
        }
        last = k;
        threshold = atk;
        if nth_unit(strongest, 0) == k {
            best = k;
        }
    }
    Ok(if last < 0 {
        -1
    } else {
        (last & 0xFFFF) << 16 | (best & 0xFFFF)
    })
}

/// 0x112C90(side, relation, filter): use a mass effect when the strongest
/// affected enemy beats every affected friend and exceeds 3000 ATK;
/// otherwise refuse any affected friend, then require four affected enemies.
pub(in super::super) fn f112c90<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    side: i32,
    relation: i32,
    filter: u32,
) -> R<i32> {
    f112c90_with(w, side, relation, filter, 3000)
}

/// [`f112c90`] with the strongest affected enemy's threshold: 3000
/// (`slti 0xBB9`) in Seto's 0x112C90, 2500 (`slti 0x9C5`) in the generic
/// module's copy 0x16CF80.
pub(in super::super) fn f112c90_with<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    side: i32,
    relation: i32,
    filter: u32,
    threshold: i32,
) -> R<i32> {
    let mask = global_mask(w, side, relation, filter)?;
    let (mut own, mut enemy) = (-1, -1);
    for i in 0..popcount(mask) {
        let k = nth_unit(mask, i);
        let (g, s) = (k >> 8, k & 0xFF);
        let p = w.pos(g, s);
        let a = w.eff_atk(g, s, p & 0xFF, p >> 8)?;
        if w.ctrl(g, s) == side {
            own = own.max(a);
        } else {
            enemy = enemy.max(a);
        }
    }
    if own < enemy && enemy > threshold {
        return Ok(1);
    }
    if relation & 1 != 0 && popcount(global_mask(w, side, 1, filter)?) > 0 {
        return Ok(0);
    }
    Ok(i32::from(
        relation >> 1 != 0 && popcount(global_mask(w, side, 2, filter)?) >= 4,
    ))
}

/// Seto's global 0x24E360(0,0,side,relation,filter,0xE) calls also use
/// filters0x13 and0x20, which the shared subset does not implement.
/// 0x24EF4C is monster kind19. 0x24F324 chooses the weakest monster,
/// initialized to ATK10000/DEF0; preserve its unusual 0x24F3C0 comparison
/// of the ENTIRE previous packed score against the next ATK on a tie.
pub(in super::super) fn global_mask<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    side: i32,
    relation: i32,
    filter: u32,
) -> R<i32> {
    if !matches!(filter & 0xFFFF, 0x13 | 0x20) {
        return unit_mask(w, 0, 0, side, relation, filter, 0xE);
    }
    let (mut mask, mut best, mut best_order, mut best_group) = (0, 0x2710_0000u32, 9999, -1);
    for k in unit_list(w, side, relation) {
        let (g, s) = (k >> 8, k & 0xFF);
        let kind = w.kind(g, s);
        if kind >> 5 != 0 {
            continue;
        }
        let bit = 1 << if g == 0 { 21 - s } else { 10 - s };
        if filter & 0xFFFF == 0x13 {
            if kind & 0x1F == 0x13 {
                mask |= bit;
            }
            continue;
        }
        let p = w.pos(g, s);
        let value = (w.eff_atk(g, s, p & 0xFF, p >> 8)? as u32) << 16
            | w.eff_def(g, s, p & 0xFF, p >> 8)? as u32;
        let replace = if value >> 16 < best >> 16 {
            true
        } else if best != value >> 16 {
            false
        } else if value & 0xFFFF != best & 0xFFFF {
            value & 0xFFFF < best & 0xFFFF
        } else if w.order(g, s) != best_order {
            w.order(g, s) < best_order
        } else {
            best_group == relation
        };
        if replace {
            best = value;
            best_order = w.order(g, s);
            best_group = g;
            mask = bit;
        }
    }
    Ok(mask)
}

/// 0x27CF90: skip list[0], consider only committed face-up cards, strict
/// ATK improvement from -1. Used with explicitly cleared lists below.
fn pick_strongest<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    list: &[i32],
) -> R<i32> {
    let (mut pick, mut best) = (-1, -1);
    for &k in list.iter().skip(1) {
        let (g, s) = (k >> 8, k & 0xFF);
        if !w.flag(g, s, 4) {
            continue;
        }
        let p = w.pos(g, s);
        let a = w.eff_atk(g, s, p & 0xFF, p >> 8)?;
        if best < a {
            best = a;
            pick = k;
        }
    }
    Ok(pick)
}

/// 0x112ED0(side, entry): cross-spell target. The non-monster branch
/// calls 0x24E360(mode 0x12, filter 0x34) at every cell, but that handler
/// accepts only 0x1D/0x1E/0x1F (0x24E9D0..0x24E9F4); its mask is always
/// zero, so this branch returns -1. Preserve that original mismatch.
pub(in super::super) fn f112ed0<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    side: i32,
    entry: [u32; 2],
) -> R<i32> {
    if entry[0] & 0x3F != 0x1D {
        return Ok(-1);
    }
    let enemy = pick_strongest(w, &w.list_seen(i32::from(side == 0), 0x1D, false, false))?;
    let own = pick_strongest(w, &w.list(side, 0x1D, true))?;
    if enemy < 0 || own < 0 {
        return Ok(-1);
    }
    let pos = w.pos(enemy >> 8, enemy & 0xFF);
    Ok(if w.margin_k(enemy, own, pos & 0xFF, pos >> 8)? < 0 {
        -1
    } else {
        pos
    })
}

/// The live magic branch of 0x111BF0. Its third argument is not read.
pub(super) fn special_reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    if w.kind(g, s) >> 5 != 1 {
        return unsupported("Seto 0x111BF0 non-magic branches are unreachable from 0x118140");
    }
    if w.classify(g, s, 6, false)? == 5 {
        return Ok(2);
    }
    let Some([word, word1]) = w.unit(g, s).and_then(|u| u.entry(0)) else {
        return Ok(2);
    };
    let ctrl = w.ctrl(g, s);
    let filter = word & 0x3F;
    let relation = ((word >> 6) & 3) as i32;
    let yes = |b: bool| if b { 1 } else { 2 };
    match word >> 8 {
        0x17 | 0x49 => Ok(yes(f112c90(w, ctrl, relation, filter)? == 1)),
        0x18 => {
            let mask = unit_mask(w, 0, 0, ctrl, 2, 0x34, 0xE)?;
            let n = (0..popcount(mask))
                .filter(|&i| {
                    let k = nth_unit(mask, i);
                    w.flag(k >> 8, k & 0xFF, 4)
                })
                .count();
            Ok(yes(n >= 6))
        }
        4 => {
            if word1 >> 16 == 0x12 {
                let target = f112ed0(w, g, [word, word1])?;
                let pos = w.pos(g, s);
                Ok(yes(target >= 0
                    && (pos & 0xFF == target & 0xFF
                        || pos >> 8 == target >> 8)))
            } else {
                Ok(yes(f112c90(w, ctrl, relation, filter)? == 1))
            }
        }
        0x1F => Ok(yes(
            w.card_id(g, s) == 0x2DD && control_target(w, ctrl)? >= 0
        )),
        0x20 => Ok(yes((0..11).any(|slot| w.spellbind(g, slot) == 0xFF))),
        0x12 => Ok(yes(popcount(global_mask(w, ctrl, 1, filter)?) >= 3)),
        0x21 => unsupported("Seto 0x1120E0 reads an unterminated stack list from 0x111C70"),
        0x36 => Ok(1),
        0x25 => Ok(yes(if filter == 0x21 {
            w.threatened(g, s, false)?
        } else {
            w.threatened(ctrl, 0, false)?
        })),
        0x26 => Ok(yes(!w.can_summon(ctrl))),
        0x27 => Ok(yes(w.stars(ctrl) < 4)),
        0x28 => Ok(yes(w.threatened(ctrl, 0, false)?)),
        // These return before the 0x11839C call. Refuse direct misuse
        // rather than silently substitute their dead 0x111BF0 branches.
        2 | 0xD | 0x14 => unsupported("Seto 0x111BF0 entry type is handled by 0x118140"),
        _ => Ok(2),
    }
}
