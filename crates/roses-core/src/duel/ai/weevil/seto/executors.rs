//! Seto-specific attack (0x1166F0) and graveyard action (0x117970).

use super::super::search::Steps;
use super::super::world::{unsupported, R};
use super::super::{exec, necromancer, search};
use super::{hidden_value, CardData, Decision, FusionData, Record, Seto, W};

/// 0x1166F0: unlike the shared attack executor, reveal is evaluated but
/// discarded, movable exits set reveal/position2, and no-candidate returns-1.
pub(super) fn attack<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<i32> {
    let target = rec.target(which);
    let (tg, ts) = (i32::from(target >> 8), i32::from(target & 0xFF));
    if w.unit(tg, ts).is_none() {
        // 0x116788 always clears the primary slot, even on secondary entry.
        rec.consume(1);
        return Ok(-1);
    }
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let _ = super::reveal::reveal(w, g, s, rec)?;
    let pos = w.pos(g, s);
    let (c, r) = (pos & 0xFF, pos >> 8);
    let tp = w.pos(tg, ts);
    let (tc, tr) = (tp & 0xFF, tp >> 8);
    let Some(u) = w.unit(g, s) else {
        return unsupported("0x1166F0 without its acting unit");
    };
    if w.allowance(u, c, r)? == 0 {
        d.key = rec.key;
        d.kind = 6;
        d.target = pos as u16;
        let danger = w.danger(c, r, side, 3, 1, hidden_value(w, side)? & 0xFF != 0)?;
        d.position = u8::from(danger >= w.atk(g, s, false)?);
        // Reveal is intentionally untouched here (0x1168A0..0x11691C).
        rec.consume(which);
        return Ok(0);
    }
    let mut steps = Steps::new();
    search::attack_path_k(
        w,
        g,
        s,
        &mut steps,
        c,
        r,
        tc,
        tr,
        u32::from(target),
        0,
        30,
        hidden_value(w, side)? & 0xFFFF,
    )?;
    let count = search::step_count(&steps);
    let dist = |b: u8| (i32::from(b & 0xF) - tc).abs() + (i32::from(b >> 4) - tr).abs();
    let (mut best, mut pick) = (0xFFFF, None);
    for i in 0..count.max(0) as usize {
        let b = steps.0[i];
        let score = w.danger(
            i32::from(b & 0xF),
            i32::from(b >> 4),
            side,
            2,
            1,
            hidden_value(w, side)? & 0xFF != 0,
        )?;
        if score < best {
            best = score;
            pick = Some(i);
        } else if score == best {
            let Some(j) = pick else {
                return unsupported("0x1166F0 tie reads past its step buffer");
            };
            if dist(b) < dist(steps.0[j]) {
                pick = Some(i);
            }
        }
    }
    d.key = rec.key;
    d.kind = 6;
    d.reveal = 2;
    d.position = 2;
    if let Some(i) = pick {
        let b = steps.0[i];
        d.target = u16::from(b >> 4) << 8 | u16::from(b & 0xF);
        Ok(0)
    } else {
        d.target = pos as u16;
        rec.consume(which);
        Ok(-1)
    }
}

/// 0x117970: entry3 for monsters, entry0 otherwise. A graveyard pick
/// activates type0xC. Without one, committed face-up cards mark acted;
/// set cards retreat. Non-monsters additionally test Crush and far traps.
pub(super) fn plan_c<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<()> {
    plan_c_with::<Seto, _, _>(w, rec, side, d, which, false, |_| super::f117f50(w, side))
}

/// [`plan_c`] with the module's graveyard pick (given the raw entry the
/// executor read) and retreat: Seto's 0x117970 picks with 0x117F50(side);
/// the generic module's copy 0x172630 with 0x16D1C0(side, entry
/// relation, entry filter), which may draw rand(), and without a pick
/// lets monsters retreat at once (`monster_retreats`, 0x17283C: only a
/// committed face-up NON-monster is marked acted).
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn plan_c_with<O, C, F>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
    monster_retreats: bool,
    pick: impl FnOnce([u32; 2]) -> R<i32>,
) -> R<()>
where
    O: super::super::Opponent,
    C: CardData + ?Sized,
    F: FusionData + ?Sized,
{
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let class = w.kind(g, s) >> 5;
    // [F] 0x1179D0/0x1179EC call the raw accessor 0x1E1DF0, which
    // returns the entry's address even when both words are 0xFFFFFFFF.
    // Unlike 0x252060, it rejects only missing/virtual units here.
    let entry = w
        .unit(g, s)
        .filter(|u| u.slot < 11)
        .map(|u| u.entries[if class == 0 { 3 } else { 0 }].unwrap_or([u32::MAX; 2]));
    if let Some(entry) = entry {
        let mut pick = pick(entry)?;
        if class != 0 {
            let p = w.pos(g, s);
            if w.terrain(p & 0xFF, p >> 8) == 9 {
                pick = -1;
            }
            if w.classify(g, s, 6, false)? == 5 {
                pick = -1;
            }
        }
        if pick >= 0 {
            let gy = (pick >> 16) & 0xFFFF;
            let menu =
                necromancer::graveyard_menu_index(w, gy, pick & 0xFF, (entry[0] & 0x3F) as i32);
            d.key = rec.key;
            d.kind = 0xC;
            d.position = exec::stance(w, g, s, d.target, 1, false)?;
            d.reveal = 1;
            let selection_side = if entry[0] >> 6 & 3 == 3 { gy } else { side };
            d.target = (selection_side << 8 | menu) as u16;
            rec.consume(which);
            return Ok(());
        }
    }
    if !(monster_retreats && class == 0) && w.flag(g, s, 4) {
        d.key = rec.key;
        d.kind = 0xE;
        d.mark_acted = Some(rec.key);
        return Ok(());
    }
    exec::retreat::<O, _, _>(w, rec, side, d, which)
}
