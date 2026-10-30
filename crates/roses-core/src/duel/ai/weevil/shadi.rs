//! The generic computer module (driver 0x172C40, module 0x16BC90..0x172DD4,
//! 37 functions): Shadi's controller (control byte 16, the white route's
//! Bury St. Edmunds duel, reached through the j-stub 0x154AE0) and, in the
//! original, the VS-mode controllers (control 0 and 22..=31). The port
//! registers Shadi's byte and Custom Duel's 22..=25
//! (`weevil::VS_CONTROLLERS`; the module never reads its control byte);
//! 0 and 26..=31 have no retail writer [F] and stay refused. The driver is Weevil's
//! 0x1117D0 with its own watchdog and without the hand-index reset of the
//! action record (d+6..+0xA keep stack residue; the planners that write a
//! summon write all five, [H]).
//!
//! The dispatch 0x16DA10 is Manawyddan's red 0x163A00 with plan 0xD
//! (Pegasus's unit pick 0x139220 → [`plan_d`] 0x1728D0) and plan 0xC (Seto's
//! graveyard action 0x117970 → 0x172630 with [`graveyard_pick`] 0x16D1C0),
//! plans 0xF and 0x10 run as plan 6 in both slots (the patrol's codes), and
//! a leader summon always clears the primary slot; the actor choice
//! 0x16E190 is Pegasus's 0x136240; the executors are York's retreat
//! 0x13C730 (0x171210, static radius 3), Labyrinth-ruler's attack 0x1344B0
//! (0x1717E0, route radius 2) and Seto's move 0x116BC0 (0x171DC0), all
//! calling the reveal [`reveal::reveal`] 0x16BEE0 (Seto's 0x111BF0 in
//! full); the summon planner 0x16FDA0 is Mako's 0x14D470 with this
//! module's hidden strength and a monster limit of 5.
//!
//! Own functions, by EE address: [`hidden_value`] 0x172BE0, [`assign`]
//! 0x16DF70, [`planner`] 0x16E570 with [`rush`] 0x16E670 (0x16ED80:
//! 0x27F760 with limit 0x9C4), [`row_clear`] 0x16E8D0 and [`patrol`]
//! 0x16E9B0 (draws rand()), [`effect_planner`] 0x16EDE0, [`support`]
//! 0x16FB10, [`graveyard_pick`] 0x16D1C0 (draws rand()), [`plan_d`]
//! 0x1728D0, the reveal and its helpers in [`reveal`] and the summon
//! subtree in [`summon`].
//!
//! The Exodia tests of a side led by card 0x3A read its saved deck list
//! (0x16D800 / 0x170B80, [`super::summon::exodia_in_saved_deck`] over
//! [`crate::duel::Duel::saved_decks`]); 0x16D8C0 is Jasper's 0x159230.
//! 0x16D1C0's graveyard read of side −1 always ends in −1. The saved
//! leader rank 0x2154C0 is [`crate::duel::Duel::leader_ranks`].

pub(super) mod reveal;
pub(super) mod summon;

use super::exec;
use super::plans::{self, set_target};
use super::search::{self, Pool, Targets};
use super::seto::executors::plan_c_with;
use super::seto::reveal::{control_target, f112ed0, global_mask};
use super::summon::{exodia_in_saved_deck, kind_of, nth_unit, popcount, virt};
use super::world::{unsupported, World, R};
use super::{has_effect, Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// 0x16FB10: entry type 0x1F → plan 0xD for this card only.
    pub const CHANGE_OF_HEART: i32 = 0x2DC;
    /// 0x16DF70 / 0x16EDE0 / 0x16BEE0 / 0x170B80: the Exodia pieces
    /// 0x36..=0x39 (planned as effect monsters).
    pub const EXODIA_FIRST: i32 = 0x36;
    pub const EXODIA_LAST: i32 = 0x39;
    /// 0x170030: magic cards summoned when 0x16D480 finds a target.
    pub const CROSS_A: i32 = 0x2BC;
    pub const CROSS_B: i32 = 0x2BE;
    /// 0x170030: Cursebreaker and Paralyzing Potion.
    pub const CURSEBREAKER: i32 = 0x31C;
    pub const POTION: i32 = 0x31B;
    /// 0x16EDE0 (entry-3 type 0x3C): the 0x24E360 word it passes (type
    /// 0x26D: no handler, an empty mask).
    pub const WORD_26D: u32 = 0x26D;
}

/// 0x16E570 / 0x16EDE0 / 0x16FB10: the ATK from which a monster rushes
/// instead of patrolling (`slti 0x9C4`).
const RUSH_ATK: i32 = 0x9C4;
/// 0x16E670: the enemy-leader move limit, the last pass round and the
/// pass radius; 0x16ED80: the first pass's ATK limit.
const RUSH_MOVES: i32 = 5;
const RUSH_ROUNDS: i32 = 5;
const RUSH_RADIUS: i32 = 5;
const PASS_ATK_LIMIT: i32 = 0x9C4;
/// 0x16E9B0: the same for the patrol.
const PATROL_MOVES: i32 = 2;
const PATROL_ROUNDS: i32 = 2;
const PATROL_RADIUS: i32 = 2;
/// 0x16E570's 0x16E8D0 / 0x16E9B0 argument (1): the patrol row 3 (4 for
/// any other argument) and its columns.
const PATROL_ROW: i32 = 3;
const OTHER_ROW: i32 = 4;
const PATROL_LEFT: i32 = 1;
const PATROL_RIGHT: i32 = 5;
const ROW_FIRST_COL: i32 = 1;
const ROW_END_COL: i32 = 6;
/// 0x172BE0's LP test (`slti 0x4B1`), monster test (`slti 4`) and answers.
const LOW_LP: i32 = 0x4B1;
const FEW_MONSTERS: i32 = 4;
const HIDDEN_LOW_LP: i32 = 0x960;
const HIDDEN_FEW_MONSTERS: i32 = 0x7D0;
const HIDDEN_DEFAULT: i32 = 0x3E8;
/// 0x16FB10: 0x27E0B0's minimum and radius (the terrain is the entry's
/// word-1 low byte).
const OPEN_MIN: i32 = 8;
const OPEN_RADIUS: i32 = 2;
/// 0x16EDE0 (entry-2 0x58) / 0x16BEE0: the enemy leader's saved rank from
/// which the unit holds / reveals (`slti 6`).
const RANK_HOLD: i32 = 6;
/// 0x16EDE0 entry-2 0x53 / 0x16BEE0: monsters in both graveyards.
const GRAVE_MONSTERS: i32 = 6;
/// 0x16EDE0 entry-3 0x25: the last round, the pick bound and filter.
const HUNT_ROUNDS: i32 = 5;
const HUNT_BOUND: i32 = 5;
const HUNT_FILTER: i32 = 0x34;
/// 0x16EDE0 entry-3 0x3C: the initial distance bound.
const NEAREST_BOUND: i32 = 7;
/// 0x16D1C0: the starting deck-cost bar and the ATK a revived monster
/// needs (`slti 0x9C4`).
const GRAVEYARD_MIN_COST: i32 = 0x28;
const REVIVE_ATK: i32 = 0x9C4;
/// The revival entry type 0x16D1C0 passes over.
const REVIVAL_TYPE: u32 = 0x14;

/// The generic module, as Shadi's control byte.
pub(super) struct Shadi;

impl Opponent for Shadi {
    const CONTROL: u8 = 16;
    /// 0x171DC0 (= Seto's 0x116BC0) and 0x1717E0: strength 1.
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    /// 0x1717E0 (0x1344B0's 4 → 2) and 0x171DC0 (Seto's 2).
    const DANGER_RADIUS: i32 = 2;
    /// 0x171210 (York's 0x13C730 with 3).
    const STATIC_DANGER_RADIUS: i32 = 3;
    /// York's block in 0x171210 and Seto's in 0x171DC0.
    const FACE_UP_SUPPORT_PASSES: bool = true;
    /// 0x16DA84..: 0x27CAF0(rec, 1) whatever the primary plan.
    const LEADER_SUMMON_SLOT: Option<u8> = Some(1);
    /// 0x16DAF8 / 0x16DD38: 0x10 and 0xF run as plan 6.
    fn plan_alias(code: u8) -> u8 {
        match code {
            0xF | 0x10 => 6,
            c => c,
        }
    }
    fn trap_actor(class: i32) -> bool {
        class == 3
    }
    fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        side: i32,
    ) -> R<i32> {
        Ok(hidden_value(w, side))
    }
    fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        mem: &mut Memory,
        idx: usize,
    ) -> R<()> {
        assign(w, &mut mem.records[idx])
    }
    fn summon<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        key: i32,
        side: i32,
        d: &mut Decision,
    ) -> R<i32> {
        summon::summon(w, key, side, d)
    }
    fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        g: i32,
        s: i32,
        rec: &Record,
    ) -> R<u32> {
        reveal::reveal(w, g, s, rec)
    }
    /// 0x172AE0 = Pegasus's 0x139950.
    fn charge<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        g: i32,
        s: i32,
        c: i32,
        r: i32,
        tc: i32,
        tr: i32,
    ) -> R<bool> {
        super::pegasus::charge(w, g, s, c, r, tc, tr)
    }
    /// 0x16DCA4 / 0x16DEE8: 0xC → 0x172630; 0x16DCBC / 0x16DF00: 0xD →
    /// 0x1728D0 (in the primary slot whether or not the unit has acted).
    fn extra_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        rec: &mut Record,
        side: i32,
        d: &mut Decision,
        which: u8,
        code: u8,
    ) -> R<bool> {
        match code {
            0xC => plan_c_with::<Shadi, _, _>(w, rec, side, d, which, true, |entry| {
                graveyard_pick(
                    w,
                    side,
                    (entry[0] >> 6 & 3) as i32,
                    (entry[0] & 0x3F) as i32,
                )
            })?,
            0xD => plan_d::<Shadi, _, _>(w, rec, side, d, which)?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// 0x172BE0(side): the strength assumed for hidden units: 2400 while the
/// side's LP is below 1201; else 2000 with fewer than four monsters in its
/// slots 1..=5, else 1000.
fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    if w.lp(side) < LOW_LP {
        return HIDDEN_LOW_LP;
    }
    if w.monsters(side) < FEW_MONSTERS {
        HIDDEN_FEW_MONSTERS
    } else {
        HIDDEN_DEFAULT
    }
}

/// An Exodia piece (0x16D7E0: card − 0x36 < 4).
fn exodia_piece(card: i32) -> bool {
    (id::EXODIA_FIRST..=id::EXODIA_LAST).contains(&card)
}

/// 0x16D800(side) == 1 and 0x16D8C0(side) == 4: the side's leader is card
/// 0x3A, its saved deck list holds the four pieces and all four stand
/// next to it ([`super::jasper::pieces_around_leader`]).
fn exodia_assembled<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> R<bool> {
    Ok(exodia_in_saved_deck(w, side)? && super::jasper::pieces_around_leader(w, side)? == 4)
}

/// 0x2154C0(other, its leader card): the saved rank of `side`'s enemy
/// leader (−1 for card 0x29F, mapped to 999, and ids from 0x2AB).
fn enemy_leader_rank<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    let other = i32::from(side == 0);
    let card = w.card_id(other, 0);
    let card = if card == 0x29F { 999 } else { card };
    if !(0..0x2AB).contains(&card) {
        return -1;
    }
    i32::from(w.duel.leader_ranks[other as usize])
}

/// Monster cards (0x271610 class 0) in the graveyards of `side` and of
/// its enemy (0x1E0080 / 0x1E00D0).
fn graveyard_monsters<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> i32 {
    let mut n = 0;
    for gy in [side, i32::from(side == 0)] {
        for i in 0..w.graveyard_count(gy) {
            if kind_of(w, w.graveyard(gy, i)) >> 5 == 0 {
                n += 1;
            }
        }
    }
    n
}

/// A unit of `mask` bound (0x1E2800 > 0) but not by 0xFF.
fn any_spellbound<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, mask: i32) -> bool {
    (0..popcount(mask)).any(|i| {
        let k = nth_unit(mask, i);
        let sb = w.spellbind(k >> 8, k & 0xFF);
        sb > 0 && sb != 0xFF
    })
}

/// 0x24E190(n): 2^n − 1 summed in floating point; 0 for n ≤ 0.
fn ones(n: i32) -> u32 {
    if n <= 0 {
        0
    } else if n >= 32 {
        u32::MAX
    } else {
        (1u32 << n) - 1
    }
}

/// 0x24E0F0(mask, g, s): `mask` without the unit's bit, computed as the
/// original does (also for the swapped arguments 0x27FA90 passes).
fn remove_raw(mask: i32, g: i32, s: i32) -> i32 {
    let m = mask as u32;
    if g == 0 {
        let v = (m >> 11 >> ((10 - s) as u32 & 31)) & !1;
        let high = v << ((21 - s) as u32 & 31);
        (high | (m & ones(21 - s))) as i32
    } else {
        ((m & !ones(11 - s)) | (m & ones(10 - s))) as i32
    }
}

/// 0x27FA90(mask): for every unit of `mask` not committed face-up, the
/// running word becomes 0x24E0F0(g, s, word) (quirk: the arguments are
/// swapped, so a word above 10 collapses to the unit's group); the word
/// (`mask` when every unit is face-up). Callers compare it unsigned.
fn face_up_word<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, mask: i32) -> i32 {
    let mut word = mask;
    for i in 0..popcount(mask) {
        let k = nth_unit(mask, i);
        if w.flag(k >> 8, k & 0xFF, 4) {
            continue;
        }
        word = remove_raw(k >> 8, k & 0xFF, word);
    }
    word
}

/// 0x16DF70(rec): (re)assign the plans of a record (no mode). Missing,
/// acted or free records are left alone. The leader (kept while the +8
/// word is not −1): threatened (0x27D500, see-all 0) → 0x50A / 0x5FF by
/// the summon flag, else 0xA05 / 0x5FF. Non-leaders by class: 4, 3 and 1 →
/// [`support`]; 2 → 0x27E390(rec, 5, 0, 0x172BE0(controller) & 0xFF); 0 →
/// [`effect_planner`] with an effect entry (0x2520F0 = 1) or for an Exodia
/// piece, else [`planner`]; 5 and above → 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let card = w.card_id(g, s);
    let _ = w.pos(g, s);
    let ctrl = w.ctrl(g, s);
    if w.unit(g, s).is_none() || w.flag(g, s, 0) || rec.key == 0xFFFF {
        return Ok(());
    }
    if s == 0 {
        if rec.targets() != 0xFFFF_FFFF {
            return Ok(());
        }
        let summon = w.can_summon(g);
        if w.threatened(g, 0, false)? {
            rec.set_plans(if summon { 0x50A } else { 0x5FF });
        } else {
            rec.set_plans(if summon { 0xA05 } else { 0x5FF });
        }
        return Ok(());
    }
    match w.kind(g, s) >> 5 {
        4 | 3 | 1 => support(w, rec)?,
        2 => plans::power_up(w, rec, 5, 0, hidden_value(w, ctrl) & 0xFF)?,
        0 => {
            if has_effect(w, g, s) == 1 || exodia_piece(card) {
                effect_planner(w, rec)?;
            } else {
                planner(w, rec)?;
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x16E570(rec) (inlined at 0x16F170 / 0x16FA0C / 0x16FC94 too): a
/// unit whose ATK (0x1E1510, with bonuses) is 2500 or more [`rush`]es;
/// else it [`patrol`]s while [`row_clear`] (row 3) answers 1, otherwise
/// rushes. Missing units are left alone.
fn planner<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.unit(g, s).is_none() {
        return Ok(());
    }
    if w.atk(g, s, false)? >= RUSH_ATK {
        return rush(w, rec);
    }
    if row_clear(w, g, s, 1) == 1 {
        patrol(w, rec)
    } else {
        rush(w, rec)
    }
}

/// 0x16E670(rec): Keith's planner shape: 0x27CB40 reaches the enemy
/// leader within 5 moves → plan 2 on its key. Else N = 1..=5: 0x16ED80 →
/// 0x27F760 (radius 5, see-all 0, hidden 0x172BE0(controller), limit
/// 0x9C4; list and pool shared): 0 and 2 → plan 3 at the own cell; 1 →
/// planned; 3 → plan 6 at the own cell; else next N. Exhausted → plan 6
/// to the enemy leader's cell.
fn rush<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    passes(w, rec, RUSH_MOVES, RUSH_ROUNDS, RUSH_RADIUS, hidden_value::<C, F>, |_w, rec, _my, lp| {
        rec.set_plans(0x6FF);
        set_target(rec, lp);
        Ok(())
    })
}

/// The shared head of [`rush`] and [`patrol`]: the enemy-leader test and
/// the pass rounds; `exhausted` gets the own and the enemy leader's cells.
pub(super) fn passes<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    moves: i32,
    rounds: i32,
    radius: i32,
    hidden: fn(&W<C, F>, i32) -> i32,
    exhausted: impl FnOnce(&W<C, F>, &mut Record, i32, i32) -> R<()>,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let ctrl = w.ctrl(g, s);
    let leader = i32::from(ctrl == 0) << 8;
    let my = w.pos(g, s);
    let (c, r) = (my & 0xFF, my >> 8);
    let lp = w.pos(leader >> 8, leader & 0xFF);
    // sp+0x130 / 0x140: the reachable list, once; 0x2800B0: a no-op.
    let mut list = Targets::new();
    let v = search::moves_to_k(
        w,
        g,
        s,
        c,
        r,
        leader >> 8,
        leader & 0xFF,
        lp & 0xFF,
        lp >> 8,
        moves,
    )?;
    if v > 0 {
        rec.set_plans(0x2FF);
        set_target(rec, leader);
        return Ok(());
    }
    let mut pool = Pool::new();
    for n in 1..=rounds {
        let side = i32::from(w.ctrl(g, s) == 0);
        let v = plans::attack_passes(
            w,
            rec,
            u,
            c,
            r,
            side,
            n,
            hidden(w, ctrl),
            &mut list,
            &mut pool,
            radius,
            0,
            PASS_ATK_LIMIT,
        )?;
        match v {
            0 | 2 => {
                rec.set_plans(0x3FF);
                set_target(rec, my);
                return Ok(());
            }
            1 => return Ok(()),
            3 => {
                rec.set_plans(0x6FF);
                set_target(rec, my);
                return Ok(());
            }
            _ => (),
        }
    }
    exhausted(w, rec, my, lp)
}

/// 0x16E8D0(g, s, a2): along row 3 (a2 = 1, else row 4), columns 1..=5:
/// the unit itself first → 1; another unit with the same controller
/// (0x251C80) first → 0; a unit of key 0 (group 0's leader) is passed
/// over; none → 1.
pub(super) fn row_clear<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, g: i32, s: i32, a2: i32) -> i32 {
    let row = if a2 & 0xFF == 1 { PATROL_ROW } else { OTHER_ROW };
    let me = g << 8 | s;
    let ctrl = w.ctrl(g, s);
    for col in ROW_FIRST_COL..ROW_END_COL {
        let k = w.at_key(col, row);
        if k == 0 {
            continue;
        }
        if k == me {
            return 1;
        }
        if w.ctrl(k >> 8, k & 0xFF) == ctrl {
            return 0;
        }
    }
    1
}

/// 0x16E9B0(rec) (row 3): as [`rush`] with 2 moves, rounds 1..=2 and
/// radius 2. Exhausted: off row 3 → plan 6 to (own column, row 3); on it,
/// by the primary plan: 0xF → target (row, 5) while the column is below 5
/// (plans kept), else 0x10FF to (row, 1); 0x10 → 0xFFF to (row, 5) while
/// the column is below 2, else target (row, 0) (plans kept); any other →
/// rand() (0x19AE88): even 0x10FF to (row, 1), odd 0xFFF to (row, 5).
fn patrol<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    patrol_with(w, rec, 1, hidden_value::<C, F>)
}

/// [`patrol`] with 0x16E9B0's row argument (1: row 3, else row 4; Yugi's
/// 0x15EDF0 passes 2 too) and the module's hidden strength.
pub(super) fn patrol_with<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    a1: i32,
    hidden: fn(&W<C, F>, i32) -> i32,
) -> R<()> {
    let patrol_row = if a1 & 0xFF == 1 { PATROL_ROW } else { OTHER_ROW };
    passes(w, rec, PATROL_MOVES, PATROL_ROUNDS, PATROL_RADIUS, hidden, |w, rec, my, _lp| {
        let (col, row) = (my & 0xFF, my >> 8);
        if row != patrol_row {
            rec.set_plans(0x6FF);
            set_target(rec, patrol_row << 8 | col);
            return Ok(());
        }
        match rec.plan_a {
            0xF => {
                if col < PATROL_RIGHT {
                    set_target(rec, row << 8 | PATROL_RIGHT);
                } else {
                    rec.set_plans(0x10FF);
                    set_target(rec, row << 8 | PATROL_LEFT);
                }
            }
            0x10 => {
                if col < 2 {
                    rec.set_plans(0xFFF);
                    set_target(rec, row << 8 | PATROL_RIGHT);
                } else {
                    set_target(rec, row << 8);
                }
            }
            _ => {
                if w.rand31() & 1 == 0 {
                    rec.set_plans(0x10FF);
                    set_target(rec, row << 8 | PATROL_LEFT);
                } else {
                    rec.set_plans(0xFFF);
                    set_target(rec, row << 8 | PATROL_RIGHT);
                }
            }
        }
        Ok(())
    })
}

/// Plan 3 at the own cell.
fn hold(rec: &mut Record, my: i32) {
    rec.set_plans(0x3FF);
    set_target(rec, my);
}

/// 0x16EDE0(rec): monsters with an effect entry and the Exodia pieces.
/// An Exodia piece whose controller's leader is card 0x3A with all four
/// pieces in its saved deck list (0x16EED8..0x16EF7C, the 0x16D800 test
/// inlined) gets plan 6 toward the leader's cell (0x16EF84: the loop over
/// the leader's neighbours 0x170EE0 writes the same plan word and its own
/// cell as the target, then the leader's cell overwrites it) and nothing
/// else. By entry 2 (0x252060; controller
/// `c`): 0x58 → hold (plan 3 in place) when the enemy leader's saved rank
/// is 6 or more; 0x55 / 0x47 → hold when an own (rel 1, filter 0x34) /
/// enemy (rel 2) unit is bound by a spell other than 0xFF; 0x54 → hold when
/// the strongest monster on the board (rel 3, type 0x21) is another of
/// the controller's; 0x53 → hold with six or more monsters in both
/// graveyards; 0x17 / 0x13 → with bit 6 and an own unit of the filter:
/// [`planner`] and done; else hold when 0x27FA90 of the enemy's units of
/// the filter is 3 or more (unsigned); 6, 0x4D, 0x12, 0x4E, 0xB → hold
/// when the own mask of the filter is 2 or more (the MASK, unsigned);
/// 0xF, 0x43, 0x56, 0x3B, 0x22, 0xE, 0xD, 0x34, 0xC → hold. Otherwise by
/// entry 3 (not for committed face-up units): 0x51 → kept when 0x27F060
/// planned the strongest enemy; 0x46 → 0xBFF when it did, done either
/// way; 0xC → hold; 0x3C → the nearest unit of the (empty) type-0x26D mask
/// as 0xBFF; 0x25 → rounds 1..=5 of 0x278A20 (the tree, list and count
/// kept) and 0x278AA0 (bound 5, filter 0x34, face 0, def 2, see-all 0):
/// plan 2 on a pick; 0x1F (no continuous 0x39 effect, filter 0x1A), 0x24
/// (filter 0x1A / 0x1B), 0x16, 4 (filter 0x1A), 0x17 (filter 0x1A) →
/// 0xBFF when 0x27F060 planned (0x1F with word-1 type 0xE and 4 with 2 then
/// require the target to be a monster of that kind); 0x14 → 0xCFF.
/// Otherwise by entry 1: type 0x42 with relation 1 → plan 6 in place;
/// then [`planner`].
fn effect_planner<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    let card = w.card_id(g, s);
    let my = w.pos(g, s);
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let (c, r) = (my & 0xFF, my >> 8);
    if exodia_piece(card) && exodia_in_saved_deck(w, ctrl)? {
        rec.set_plans(0x6FF);
        set_target(rec, w.pos(ctrl, 0));
        return Ok(());
    }
    if let Some([w0, _]) = u.entry(2) {
        let filter = w0 & 0x3F;
        let held = match w0 >> 8 {
            0x58 => enemy_leader_rank(w, ctrl) >= RANK_HOLD,
            0x55 => any_spellbound(w, global_mask(w, ctrl, 1, 0x34)?),
            0x54 => {
                let k = nth_unit(global_mask(w, ctrl, 3, 0x21)?, 0);
                w.ctrl(k >> 8, k & 0xFF) == ctrl && k != i32::from(rec.key)
            }
            0x53 => graveyard_monsters(w, ctrl) >= GRAVE_MONSTERS,
            0x47 => any_spellbound(w, global_mask(w, ctrl, 2, 0x34)?),
            0x17 | 0x13 => {
                if w0 >> 6 & 1 == 1 && global_mask(w, ctrl, 1, filter)? != 0 {
                    return planner(w, rec);
                }
                face_up_word(w, global_mask(w, ctrl, 2, filter)?) as u32 >= 3
            }
            6 | 0x4D | 0x12 | 0x4E | 0xB => global_mask(w, ctrl, 1, filter)? as u32 >= 2,
            0xF | 0x43 | 0x56 | 0x3B | 0x22 | 0xE | 0xD | 0x34 | 0xC => true,
            _ => false,
        };
        if held {
            hold(rec, my);
            return Ok(());
        }
    }
    if !w.flag(g, s, 4) {
        if let Some([w0, w1]) = u.entry(3) {
            let filter = w0 & 0x3F;
            let planned = |rec: &mut Record| plans::strongest_enemy(w, rec, 0).map(|v| v == 1);
            // The target's kind must be a monster of `kind` (0x1E1980 of
            // the primary target).
            let target_kind = |rec: &Record, kind: i32| {
                let t = i32::from(rec.target_a);
                let k = w.kind(t >> 8, t & 0xFF) & 0xFFFF;
                k >> 5 == 0 && k & 0x1F == kind
            };
            match w0 >> 8 {
                0x51 => {
                    if planned(rec)? {
                        return Ok(());
                    }
                }
                0x46 => {
                    if planned(rec)? {
                        rec.set_plans(0xBFF);
                    }
                    return Ok(());
                }
                0xC => {
                    hold(rec, my);
                    return Ok(());
                }
                0x3C => {
                    let mask = global_mask(w, ctrl, 3, id::WORD_26D)?;
                    if mask != 0 {
                        let (mut best, mut pick) = (NEAREST_BOUND, -1);
                        for i in 0..popcount(mask) {
                            let k = nth_unit(mask, i);
                            let p = w.pos(k >> 8, k & 0xFF);
                            let d = ((p >> 8) - r).abs() + ((p & 0xFF) - c).abs();
                            if d < best {
                                best = d;
                                pick = k;
                            }
                        }
                        if pick >= 0 {
                            rec.set_plans(0xBFF);
                            set_target(rec, pick);
                            return Ok(());
                        }
                    }
                }
                0x25 => {
                    // sp+0x140: the list; sp+0x19E: its count; sp+0x19C /
                    // sp+0x19A: the pool counters; fp: the tree's root.
                    let mut list = Targets::new();
                    let mut pool = Pool::new();
                    let mut root = None;
                    let other = i32::from(ctrl == 0);
                    for n in 1..=HUNT_ROUNDS {
                        let hidden = hidden_value(w, ctrl);
                        root = search::reach(
                            w, &mut pool, &mut list, root, Some(u), c, r, other, n, 0, hidden,
                        )?;
                        let hidden = hidden_value(w, ctrl);
                        let k = search::pick_target(
                            w,
                            u,
                            c,
                            r,
                            &list,
                            HUNT_BOUND,
                            HUNT_FILTER,
                            0,
                            2,
                            0,
                            hidden,
                            i32::from(list.count),
                        )?;
                        if k >= 0 {
                            rec.set_plans(0x2FF);
                            set_target(rec, k);
                            return Ok(());
                        }
                    }
                }
                0x1F => {
                    if w.continuous_count(0x39) <= 0 && filter == 0x1A && planned(rec)? {
                        rec.set_plans(0xBFF);
                        if w1 >> 16 != 0xE || target_kind(rec, 0xE) {
                            return Ok(());
                        }
                    }
                }
                0x24 => {
                    if (filter == 0x1A || filter == 0x1B) && planned(rec)? {
                        rec.set_plans(0xBFF);
                        return Ok(());
                    }
                }
                0x16 => {
                    if planned(rec)? {
                        rec.set_plans(0xBFF);
                        return Ok(());
                    }
                }
                4 => {
                    if filter == 0x1A && planned(rec)? {
                        rec.set_plans(0xBFF);
                        if w1 >> 16 != 2 || target_kind(rec, 2) {
                            return Ok(());
                        }
                    }
                }
                0x14 => {
                    rec.set_plans(0xCFF);
                    return Ok(());
                }
                0x17 => {
                    if filter == 0x1A && planned(rec)? {
                        rec.set_plans(0xBFF);
                        return Ok(());
                    }
                }
                _ => (),
            }
        }
    }
    if let Some([w0, _]) = u.entry(1) {
        if w0 >> 8 == 0x42 && w0 >> 6 & 3 == 1 {
            rec.set_plans(0x6FF);
            set_target(rec, my);
        }
    }
    planner(w, rec)
}

/// 0x16FB10(rec): non-monsters (classes 1, 3, 4), by entry 0's type
/// (0x252060; a missing entry is read through a null pointer: refused):
/// 0x1F → 0xDFF for Change of Heart, else 0x5FF; 0x25 with filter 0x21 →
/// [`planner`], else 0x5FF; 2 → plan 6 to the open-area centre
/// 0x27E0B0(u, c, r, word-1 low byte, 8, 2), 0x5FF without one; 4 → plan 6
/// to 0x16D480(controller, entry)'s cell (filter 0: the cell's column on
/// the own row), 0x5FF without one; 0x14 → 0xCFF; others 0x5FF.
fn support<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let card = w.card_id(g, s);
    let ctrl = w.ctrl(g, s);
    let entry = w.unit(g, s).and_then(|u| u.entry(0));
    let my = w.pos(g, s);
    let Some([w0, w1]) = entry else {
        return unsupported("0x16FB10 reads entry 0 through a null pointer");
    };
    match w0 >> 8 {
        0x1F => rec.set_plans(if card == id::CHANGE_OF_HEART { 0xDFF } else { 0x5FF }),
        0x25 => {
            if w0 & 0x3F == 0x21 {
                return planner(w, rec);
            }
            rec.set_plans(0x5FF);
        }
        2 => {
            let Some(u) = w.unit(g, s) else {
                return Ok(());
            };
            let v = plans::open_area(
                w,
                u,
                my & 0xFF,
                my >> 8,
                (w1 & 0xFF) as i32,
                OPEN_MIN,
                OPEN_RADIUS,
            )?;
            if v < 0 {
                rec.set_plans(0x5FF);
            } else {
                rec.set_plans(0x6FF);
                set_target(rec, v);
            }
        }
        4 => {
            let v = f112ed0(w, ctrl, [w0, w1])?;
            if v < 0 {
                rec.set_plans(0x5FF);
            } else {
                rec.set_plans(0x6FF);
                if w0 & 0x3F == 0 {
                    set_target(rec, (my >> 8) << 8 | (v & 0xFF));
                } else {
                    set_target(rec, v);
                }
            }
        }
        0x14 => rec.set_plans(0xCFF),
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x16D1C0(side, relation, filter): the graveyard card plan 0xC revives.
/// A filter other than 0x1D / 0x1E becomes one of them by rand() (odd
/// 0x1E). 0x1D (monsters): −1 with five monsters; else 0x27F380(side,
/// relation)'s strongest graveyard monster when its ATK (0x1E1570 on its
/// record) is 2500 or more, as graveyard side | index (quirk: OR-ed, not
/// shifted); −1 otherwise. 0x1E (non-monsters): −1 with five cards in the
/// side's slots 6..=10; else over the own graveyard (relation bit 0) then
/// the enemy's (bit 1) every non-monster whose record's deck cost reaches
/// the best so far (from 40) and whose entry 0 is not of type 0x14, as
/// (graveyard side << 16) | index.
fn graveyard_pick<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    relation: i32,
    filter: i32,
) -> R<i32> {
    let filter = if filter == 0x1D || filter == 0x1E {
        filter
    } else if w.rand31() & 1 != 0 {
        0x1E
    } else {
        0x1D
    };
    if filter == 0x1D {
        if w.monsters(side) >= 5 {
            return Ok(-1);
        }
        let v = super::necromancer::graveyard_best(w, side, relation);
        if v < 0 {
            // No graveyard monster: 0x27F380 answers -1, so gy = -1 and the
            // card read is a word before side 0's record (0x2DAAA0, runtime
            // data: 1002 in the 757/765 states, 32 in 008-activate); every
            // path from there returns -1 (the class test, the ATK test, or
            // the final `or` of gy = -1) and 0x1DDFE0 / 0x1E1570 only touch
            // the stack [F 0x16D264..0x16D2BC; REFUSALS-SWEEP.md §AI].
            return Ok(-1);
        }
        let (gy, hi) = (v >> 24, v >> 16);
        let card = w.graveyard(gy, hi & 0xF);
        if kind_of(w, card) >> 5 != 0 {
            return Ok(-1);
        }
        let u = virt(w, card);
        let atk = if super::summon::data(w, card).is_some() {
            w.atk_of(&u, false)?
        } else {
            -1
        };
        if atk < REVIVE_ATK {
            return Ok(-1);
        }
        return Ok(gy | (hi & 0xFF));
    }
    if w.others(side) >= 5 {
        return Ok(-1);
    }
    let mut best = GRAVEYARD_MIN_COST;
    let mut pick = -1;
    for (bit, gy) in [(1, side), (2, i32::from(side == 0))] {
        if relation & bit == 0 {
            continue;
        }
        for i in 0..w.graveyard_count(gy) {
            let card = w.graveyard(gy, i);
            if kind_of(w, card) >> 5 == 0 {
                continue;
            }
            let u = virt(w, card);
            let Some(cost) = w.fusions.deck_cost(card as u16) else {
                return unsupported("0x16D1C0 reads a card record's deck cost");
            };
            if i32::from(cost) < best {
                continue;
            }
            let Some([w0, _]) = u.entries[0] else {
                return unsupported("0x16D1C0 reads a null entry 0");
            };
            if w0 >> 8 == REVIVAL_TYPE {
                continue;
            }
            best = i32::from(cost);
            pick = gy << 16 | i;
        }
    }
    Ok(pick)
}

/// 0x1728D0(rec, side, d, which): plan 0xD (Pegasus's 0x139220): with a
/// target (Seto's control target 0x1119A0 = 0x16BC90, the high half not
/// 0xFFFF) and no type-0xE far trap (0x27F170 mode 6, see-all 0): type
/// 0xD, +0x10 from 0x27FCA0(defflag 1, see-all 0) on the stale +4, +0xC =
/// 1, +4 = the units of the enemy group in slots 1.. up to the target's
/// KEY minus one; the slot cleared. Else a committed face-up card is
/// marked acted by the module (empty type-0xE record) and a set one
/// retreats.
pub(super) fn plan_d<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let _ = w.pos(g, s);
    let mut t = control_target(w, side)?;
    if w.classify(g, s, 6, false)? == 5 {
        t = -1;
    }
    let key = (t >> 16) & 0xFFFF;
    if t < 0 || key == 0xFFFF {
        if w.flag(g, s, 4) {
            d.mark_acted = Some(rec.key);
            d.key = rec.key;
            d.kind = 0xE;
            return Ok(());
        }
        return exec::retreat::<O, _, _>(w, rec, side, d, which);
    }
    let other = i32::from(side == 0);
    let mut count: i32 = 0;
    let mut slot = 1;
    while slot < 11 {
        if w.unit(other, slot).is_some() {
            count += 1;
        }
        if slot == key {
            break;
        }
        slot += 1;
    }
    d.key = rec.key;
    d.kind = 0xD;
    d.position = exec::stance(w, g, s, d.target, 1, false)?;
    d.reveal = 1;
    d.target = (count - 1) as u16;
    rec.consume(which);
    Ok(())
}

#[cfg(test)]
mod tests;
