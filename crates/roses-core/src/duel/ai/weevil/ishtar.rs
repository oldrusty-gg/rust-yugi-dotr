//! Ishtar's controller (opponent 5, control byte 5, driver 0x127380 =
//! Weevil's 0x1117D0 by opcode shape, module 0x1220D0..0x127548, 36
//! functions). The module is Weevil's template with Pegasus's actor
//! choice (0x122760), hand search, non-monster choice and charge test
//! (0x124DF0 / 0x124EE0 / 0x127110), Keith's monster planner shape
//! (0x122B40), retreat and attack executors (0x125830 / 0x125DD0 with
//! radius 2), Weevil's plan-3/6 executor (0x126440), Necromancer's slot
//! clearer (0x127210), Pegasus's ritual test (0x127290) and twelve novel
//! functions; the shared parts run through the framework in [`super`] with
//! the constants and hooks of [`Ishtar`].
//!
//! Own functions, by EE address: the dispatch [`dispatch`] 0x1220D0
//! (Weevil's 0x10D210 with 0x127210 at most clearing sites),
//! [`hidden_value`] 0x125D70, [`assign`] 0x1225B0 with the shared-library
//! leader plan [`leader_plan`] 0x27FB50, [`monster`] 0x122B40 (0x122DA0:
//! 0x27F760 with limit 0x5DC), the patrol plan [`patrol`] 0x122E00 with
//! [`zone_of`] 0x122F50, [`zone_goal`] 0x123240 and [`nearest_goal`]
//! 0x1230E0, [`effect_monster`] 0x1232C0, the class-4 plan [`class4`]
//! 0x123420, the lane goals [`lane_goal`] 0x123F40 / [`lane_goal_free`]
//! 0x123F90 / [`lane_cell`] 0x123DD0, [`reveal`] 0x126C50, and the summon
//! planner in [`summon`] (0x123600 and its subtree).

pub(super) mod summon;

use super::exec;
use super::plans::{self, set_plan, set_target};
use super::search::{self, Pool, Steps, Targets};
use super::summon::{popcount, ritual_ok_with, unit_mask};
use super::world::{Unit, World, R};
use super::{choose_actor, has_effect, stay, Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// Monster 565 (0x1232C0: plan 3 in place with an own committed
    /// face-up card in the slots 6..=10; 0x124010 / 0x126C50: summoned /
    /// revealed by the same count).
    pub const FACE_UP_COUNTER: i32 = 0x235;
    /// Monster 593 (0x124010 / 0x126C50: with three own kind-0x10 units).
    pub const KIND_10_TRIO: i32 = 0x251;
    /// Monsters 600 and 560 (0x124010 / 0x126C50: each with two copies of
    /// the other on the field).
    pub const PAIR_A: i32 = 0x258;
    pub const PAIR_B: i32 = 0x230;
    /// Monster 84 (0x124010: summoned while the leader is threatened).
    pub const GUARD: i32 = 0x54;
    /// Monster 535 (0x1250E0: never the best node of a fusion tree).
    pub const SKIPPED_TOP: i32 = 0x217;
    /// The class-4 cards 829, 818 and 820 (0x123420, 0x124910).
    pub const TRAP_33D: i32 = 0x33D;
    pub const TRAP_332: i32 = 0x332;
    pub const TRAP_334: i32 = 0x334;
    /// The ritual 844 (0x124910, with acceptable tributes).
    pub const RITUAL_34C: i32 = 0x34C;
    /// The magic card 742 (0x124910: with three own attribute-4 units).
    pub const MAGIC_2E6: i32 = 0x2E6;
    /// Cursebreaker (0x124910: with an own unit bound for good).
    pub const CURSEBREAKER: i32 = 0x31C;
}

/// 0x122DA0: the ATK limit of 0x27F760's first pass (0x5DC).
const PASS_ATK_LIMIT: i32 = 0x5DC;
/// 0x122B40: the move limit of the enemy-leader test (0x27CB40), the last
/// round and the radius of the 0x27F760 passes.
const LEADER_MOVES: i32 = 2;
const PASS_ROUNDS: i32 = 3;
const PASS_RADIUS: i32 = 3;
/// 0x125D70's answers: LP below 1001; else fewer than three monsters; else.
const LOW_LP: i32 = 0x3E9;
const HIDDEN_LOW_LP: i32 = 0x7D0;
const HIDDEN_FEW_MONSTERS: i32 = 0x3E8;
const HIDDEN_DEFAULT: i32 = 0x1F4;
/// 0x127290: the tribute ATK limit (`slti 0xA28`, as Pegasus's 0x139860).
const RITUAL_ATK_LIMIT: i32 = 0xA28;
/// 0x122E00: monsters below this ATK (0x1E1510) walk to a lane goal, the
/// others patrol the zone circuit.
const PATROL_ATK_LIMIT: i32 = 0x5DC;
/// 0x1225B0: the cell 0x27FB50 guards for the leader (col 3, row 2).
const HOLD_COL: i32 = 3;
const HOLD_ROW: i32 = 2;
/// 0x123DD0: the route limit of the lane-goal searches.
const LANE_ROUTE_LIMIT: i32 = 7;
/// 0x123420: the terrain of the cells card 0x334 is planned onto.
const TERRAIN_334: i32 = 1;

/// Ishtar's module.
pub(super) struct Ishtar;

impl Opponent for Ishtar {
    const CONTROL: u8 = 5;
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    /// 0x125DD0 / 0x126440: radius 2 (Keith's 0x1203B0 with 2 for 4,
    /// Weevil's 0x110FC0 unchanged).
    const DANGER_RADIUS: i32 = 2;
    const ATTACK_MOVE_CLEARS: bool = true;
    /// 0x125830 / 0x125DD0: radius 2 where the other modules pass 3.
    const STATIC_DANGER_RADIUS: i32 = 2;
    /// 0x12292C (0x122760 = Pegasus's 0x136240): class 3 only.
    fn trap_actor(class: i32) -> bool {
        class == 3
    }
    fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        side: i32,
    ) -> R<i32> {
        Ok(hidden_value(w, side))
    }
    /// 0x127210 at the executors' exits (0x126440, 0x125830, the immobile
    /// and moved paths of 0x125DD0).
    fn clear_plan(rec: &mut Record, which: u8) {
        clear_plan(rec, which);
    }
    fn dispatch<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        mem: &mut Memory,
        d: &mut Decision,
    ) -> R<i32> {
        dispatch(w, mem, d)
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
        reveal(w, g, s, rec)
    }
    /// 0x127110 = Pegasus's 0x139950.
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
    fn extra_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(
        _w: &W<C, F>,
        _rec: &mut Record,
        _side: i32,
        _d: &mut Decision,
        _which: u8,
        _code: u8,
    ) -> R<bool> {
        Ok(false)
    }
}

/// 0x125D70(side): the strength assumed for hidden units (Keith's 0x120350
/// by opcode shape and constants): 0x7D0 (2000) while the side's LP
/// (0x1E06F0) is below 1001; else 0x3E8 (1000) with fewer than three
/// monsters in its slots 1..=5 (0x1E13D0), else 0x1F4 (500). Passed whole
/// (the plan passes, 0x27E960's search strength, the summon searches),
/// `& 0xFFFF` (the executors' route searches and the retreat's dangers)
/// or `& 0xFF` (the executors' danger see-all byte, inlined in 0x126440,
/// and the power-up planner's strength).
pub(super) fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> i32 {
    if w.lp(side) < LOW_LP {
        return HIDDEN_LOW_LP;
    }
    if w.monsters(side) < 3 {
        HIDDEN_FEW_MONSTERS
    } else {
        HIDDEN_DEFAULT
    }
}

/// 0x127210(rec, which) (= Necromancer's 0x12C100): clear plan slot `which`
/// (1 primary, 0 secondary): its plan code becomes 0xFF and its target
/// 0xFFFF. Other values of `which` do nothing.
fn clear_plan(rec: &mut Record, which: u8) {
    match which {
        1 => {
            rec.plan_a = 0xFF;
            rec.target_a = 0xFFFF;
        }
        0 => {
            rec.plan_b = 0xFF;
            rec.target_b = 0xFFFF;
        }
        _ => (),
    }
}

/// 0x1220D0: Weevil's 0x10D210 with 0x127210 instead of 0x27CAF0 after the
/// primary summon (either way), a failed primary attack and a failed
/// secondary summon (a successful one keeps 0x27CAF0); the secondary plan
/// 0xE ends the turn as in Weevil's. Plans 3 / 6 run 0x126440, 2 0x125DD0,
/// 5 0x125830, 9 0x27E960 with 0x125D70(side) as the hidden strength.
fn dispatch<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    mem: &mut Memory,
    d: &mut Decision,
) -> R<i32> {
    let side = i32::from(w.active);
    let k = choose_actor::<Ishtar, _, _>(w, mem, side, d)?;
    if k < 0 {
        return Ok(0);
    }
    let Some(idx) = mem.find(k as u16) else {
        return Ok(0);
    };
    if k >> 16 == 1 {
        let which = u8::from(mem.records[idx].plan_a == 0xA);
        mem.records[idx].consume(which);
        return Ok(1);
    }
    assign(w, &mut mem.records[idx])?;
    let rec = &mut mem.records[idx];
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let acted = w.flag(g, s, 0);
    let mut done = false;
    match rec.plan_a {
        0xE => {
            stay(w, rec, d);
            done = true;
        }
        9 => {
            exec::equip_move::<Ishtar, _, _>(w, rec, side, d, 1)?;
            done = true;
        }
        3 if !acted => {
            exec::go_to_cell::<Ishtar, _, _>(w, rec, side, d, 1, true)?;
            done = true;
        }
        6 if !acted => {
            exec::go_to_cell::<Ishtar, _, _>(w, rec, side, d, 1, false)?;
            done = true;
        }
        0xA => {
            let r = summon::summon(w, i32::from(rec.key), side, d)?;
            clear_plan(rec, 1);
            if r < 0 {
                d.kind = 0xFF;
                d.reveal = 0;
            } else {
                done = true;
            }
        }
        2 if !acted => {
            let r = exec::attack::<Ishtar, _, _>(w, rec, side, d, 1)?;
            if r < 0 {
                clear_plan(rec, 1);
                return Ok(1);
            }
            done = true;
        }
        5 if !acted => {
            exec::retreat::<Ishtar, _, _>(w, rec, side, d, 1)?;
            done = true;
        }
        _ => (),
    }
    if done {
        return Ok(1);
    }
    match rec.plan_b {
        // Quirk kept from 0x10D210: plan 0xE in the secondary slot acts
        // but ends the turn (0x122558 leaves the acted flag clear).
        0xE => {
            stay(w, rec, d);
            Ok(0)
        }
        9 => {
            exec::equip_move::<Ishtar, _, _>(w, rec, side, d, 0)?;
            Ok(1)
        }
        3 | 6 | 2 | 5 if acted => Ok(0),
        3 => {
            exec::go_to_cell::<Ishtar, _, _>(w, rec, side, d, 0, true)?;
            Ok(1)
        }
        6 => {
            exec::go_to_cell::<Ishtar, _, _>(w, rec, side, d, 0, false)?;
            Ok(1)
        }
        0xA => {
            let r = summon::summon(w, i32::from(rec.key), side, d)?;
            if r < 0 {
                clear_plan(rec, 0);
                d.kind = 0xFF;
                d.reveal = 0;
            } else {
                rec.consume(0);
            }
            Ok(1)
        }
        2 => {
            if exec::attack::<Ishtar, _, _>(w, rec, side, d, 0)? < 0 {
                exec::retreat::<Ishtar, _, _>(w, rec, side, d, 0)?;
            }
            Ok(1)
        }
        5 => {
            exec::retreat::<Ishtar, _, _>(w, rec, side, d, 0)?;
            Ok(1)
        }
        _ => Ok(0),
    }
}

/// 0x27FB50(rec, col, row, see_all), the shared-library leader plan this
/// module is the first to call: the leader threatened (0x27D500) or the
/// cell (col, row) threatened by the leader's enemies (0x27D750) → 0x50A /
/// 0x5FF by the summon flag (0x1DF950); else 0x60A / 0x6FF with (col,
/// row) as the primary target (the walk to the held cell).
pub(super) fn leader_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    col: i32,
    row: i32,
    see_all: bool,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.threatened(g, s, see_all)? || w.cell_threatened(g, s, col, row, see_all)? {
        rec.set_plans(if w.can_summon(g) { 0x50A } else { 0x5FF });
        return Ok(());
    }
    rec.set_plans(if w.can_summon(g) { 0x60A } else { 0x6FF });
    set_target(rec, row << 8 | col);
    Ok(())
}

/// 0x1225B0(rec): (re)assign the plans of a record (no mode). Missing,
/// acted or free records are left alone (0x251C80 is called first and
/// dropped). The leader (kept while the +8 word is not −1): 0x27FB50 on
/// (col 3, row 2) with see-all 0. Non-leaders by class: 4 → [`class4`];
/// 2 → 0x27E390(rec, 2, 0, 0x125D70(controller) & 0xFF); 5 and 1 → 0x5FF;
/// 0 → [`effect_monster`] when 0x2520F0 answers 1, else
/// [`monster`](self::monster)(rec, 1); 3, 6 and above → 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    if w.unit(g, s).is_none() || w.flag(g, s, 0) || rec.key == 0xFFFF {
        return Ok(());
    }
    if s == 0 {
        if rec.targets() != 0xFFFF_FFFF {
            return Ok(());
        }
        return leader_plan(w, rec, HOLD_COL, HOLD_ROW, false);
    }
    match w.kind(g, s) >> 5 {
        4 => class4(w, rec),
        2 => plans::power_up(w, rec, 2, 0, hidden_value(w, ctrl) & 0xFF)?,
        0 => {
            if has_effect(w, g, s) == 1 {
                effect_monster(w, rec)?;
            } else {
                monster(w, rec, 1)?;
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x122B40(rec, a1): monsters (Keith's 0x11D9E0 by shape). `a1` ≠ 0 and
/// 0x27CB40 reaches the enemy leader within 2 moves: plan 2 on its key.
/// Else N = 1..=3: 0x122DA0 → 0x27F760 (radius 3, see-all 0, hidden
/// 0x125D70(controller), limit 0x5DC): 1 → planned; 0 → plan 5; 2 → plan
/// 3 at the own cell; 3 → plan 2 with the target left as the pass wrote
/// it (0x122D2C writes the plan word only); else next N (the reachable
/// list and the node pool are shared by all rounds). Exhausted →
/// [`patrol`].
fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    a1: i32,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let ctrl = w.ctrl(g, s);
    let leader = i32::from(ctrl == 0) << 8;
    let my = w.pos(g, s);
    let (lg, ls) = (leader >> 8, leader & 0xFF);
    let lp = w.pos(lg, ls);
    // sp+0x140: the reachable list (key 0xFFFF, level 0xFF), once;
    // 0x2800B0(sp+0xD0): a no-op.
    let mut list = Targets::new();
    if a1 != 0 {
        let v = search::moves_to_k(
            w,
            g,
            s,
            my & 0xFF,
            my >> 8,
            lg,
            ls,
            lp & 0xFF,
            lp >> 8,
            LEADER_MOVES,
        )?;
        if v > 0 {
            set_plan(rec, 2);
            set_target(rec, leader);
            return Ok(());
        }
    }
    // sp+0x19C / sp+0x19E: the pool counters, zeroed once.
    let mut pool = Pool::new();
    for n in 1..=PASS_ROUNDS {
        let side = i32::from(w.ctrl(g, s) == 0);
        let hidden = hidden_value(w, ctrl);
        let v = plans::attack_passes(
            w,
            rec,
            u,
            my & 0xFF,
            my >> 8,
            side,
            n,
            hidden,
            &mut list,
            &mut pool,
            PASS_RADIUS,
            0,
            PASS_ATK_LIMIT,
        )?;
        match v {
            0 => {
                set_plan(rec, 5);
                return Ok(());
            }
            1 => return Ok(()),
            2 => {
                set_plan(rec, 3);
                set_target(rec, my);
                return Ok(());
            }
            3 => {
                set_plan(rec, 2);
                return Ok(());
            }
            _ => (),
        }
    }
    patrol(w, rec)
}

/// 0x123240(i): the goal cell of patrol zone `i` (jump table 0x2BA350),
/// (row << 8) | col: (2, 0), (6, 0), (6, 4), (4, 4), (4, 6), (0, 6),
/// (0, 2), (2, 2); (2, 0) for any other index.
fn zone_goal(i: i32) -> i32 {
    const GOALS: [i32; 8] = [0x002, 0x006, 0x406, 0x404, 0x604, 0x600, 0x200, 0x202];
    usize::try_from(i)
        .ok()
        .and_then(|i| GOALS.get(i))
        .copied()
        .unwrap_or(0x002)
}

/// 0x122F50 (on 0x1E2310's answer): the patrol zone a cell lies in, a
/// clockwise circuit: 0 columns 2..5 of row 0, 1 column 6 rows 0..3, 2
/// columns 5..6 of row 4, 3 column 4 rows 4..5, 4 columns 1..4 of row 6,
/// 5 column 0 rows 3..6, 6 columns 0..1 of row 2, 7 column 2 rows 1..2;
/// −1 elsewhere.
fn zone_of(col: i32, row: i32) -> i32 {
    if (2..6).contains(&col) && row == 0 {
        return 0;
    }
    if col == 6 && (0..4).contains(&row) {
        return 1;
    }
    if (5..7).contains(&col) && row == 4 {
        return 2;
    }
    if col == 4 && (4..6).contains(&row) {
        return 3;
    }
    if (1..5).contains(&col) && row == 6 {
        return 4;
    }
    if col == 0 && (3..7).contains(&row) {
        return 5;
    }
    if (0..2).contains(&col) && row == 2 {
        return 6;
    }
    if col == 2 && (1..3).contains(&row) {
        return 7;
    }
    -1
}

/// 0x1230E0(col, row): the zone goal nearest to (col, row) as written:
/// every goal's Manhattan distance is compared with −1 (`slti −1`), which
/// no distance is below, so each goal replaces the last and the eighth
/// one, (2, 2), always wins.
fn nearest_goal(col: i32, row: i32) -> i32 {
    let mut best = -1;
    for i in 0..8 {
        let goal = zone_goal(i);
        let (gc, gr) = (goal & 0xFF, goal >> 8);
        let d = (row.max(gr) - row.min(gr)) + (col.max(gc) - col.min(gc));
        if d >= -1 {
            best = zone_goal(i);
        }
    }
    best
}

/// 0x123F40(i): the lane goals, (row << 8) | col: (3, 3), (4, 3), (2, 3);
/// (2, 3) for any other index.
fn lane_goal(i: i32) -> i32 {
    match i {
        0 => 0x303,
        1 => 0x304,
        _ => 0x302,
    }
}

/// 0x123F90(i): lane goal `i` holds no unit (0x1E4960).
fn lane_goal_free<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, i: i32) -> i32 {
    let goal = lane_goal(i);
    if goal < 0 {
        return 0;
    }
    i32::from(w.at(goal & 0xFF, goal >> 8).is_none())
}

/// 0x123DD0(u, c, r, &out): the first free lane goal ([`lane_goal_free`],
/// i = 0..=2) with a route from (c, r): 0x27AC00 (at most 7 moves, the
/// goal's occupant enterable, one step buffer for every search), taken
/// when its length is below 0xFF and not negative. (−1, 0xFF) when none.
pub(super) fn lane_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: Option<&Unit>,
    c: i32,
    r: i32,
) -> R<(i32, u8)> {
    let mut buf = Steps::new();
    for i in 0..3 {
        if lane_goal_free(w, i) != 1 {
            continue;
        }
        let goal = lane_goal(i);
        let (gc, gr) = (goal & 0xFF, goal >> 8);
        let occ = match w.at_key(gc, gr) {
            k if k < 0 => 0xFFFF,
            k => k,
        };
        let d = search::path(w, u, &mut buf, c, r, gc, gr, occ & 0xFFFF, LANE_ROUTE_LIMIT)?;
        if (0..0xFF).contains(&d) {
            return Ok((goal, d as u8));
        }
    }
    Ok((-1, 0xFF))
}

/// 0x122E00(rec): the plan of a monster no pass could place: plan 6 (the
/// secondary cleared). A monster whose ATK with bonuses (0x1E1510) is
/// below 1500 walks to the first free lane goal with a route
/// ([`lane_cell`]), or takes plan 5 when none has one. A stronger one
/// patrols: from a zone ([`zone_of`]) it heads for the next zone's goal
/// (i + 1 mod 8), from elsewhere for [`nearest_goal`].
fn patrol<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.unit(g, s).is_none() {
        return Ok(());
    }
    // 0x24F790(g, s) is called and its result dropped.
    let p = w.pos(g, s);
    let (col, row) = (p & 0xFF, p >> 8);
    rec.set_plans(0x6FF);
    if w.atk(g, s, false)? < PATROL_ATK_LIMIT {
        let (goal, _) = lane_cell(w, w.unit(g, s), col, row)?;
        if goal < 0 {
            rec.set_plans(0x5FF);
        } else {
            set_target(rec, goal);
        }
        return Ok(());
    }
    let zone = zone_of(col, row);
    let goal = if zone < 0 {
        nearest_goal(col, row)
    } else {
        zone_goal((zone + 1) & 7)
    };
    set_target(rec, goal);
    Ok(())
}

/// The counting loop of 0x1232C0 (0x12334C) and 0x124010 (0x12423C): over
/// the slots 6..=10 of group `side` and of the other group, the units with
/// +0x57 bit 4 set (committed face-up) whose controller 0x251C80 is `side`.
pub(super) fn committed_face_up_others<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> i32 {
    let other = i32::from(side == 0);
    let mut count = 0u8;
    for slot in 6..11 {
        for grp in [side, other] {
            if w.flag(grp, slot, 4) && w.ctrl(grp, slot) == side {
                count = count.wrapping_add(1);
            }
        }
    }
    i32::from(count)
}

/// The counting loops of 0x124010 (0x1242FC) and 0x126C50 (0x126EA0): over
/// the slots 1..=5 of group `side` and of the other group, the units of
/// kind `kind` (card byte 0 & 0x1F, any class) whose controller is `side`.
pub(super) fn kind_count<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    kind: i32,
) -> i32 {
    let other = i32::from(side == 0);
    let mut count = 0u8;
    for slot in 1..6 {
        for grp in [side, other] {
            if w.kind(grp, slot) & 0x1F == kind && w.ctrl(grp, slot) == side {
                count = count.wrapping_add(1);
            }
        }
    }
    i32::from(count)
}

/// The counting loops of 0x124010 (0x1243C0, 0x124480) and 0x126C50
/// (0x126D60, 0x126E00): over the slots 1..=5 of group `side` and of the
/// other group, the units holding `card` whose controller is `side`.
pub(super) fn card_count<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> i32 {
    let other = i32::from(side == 0);
    let mut count = 0u8;
    for slot in 1..6 {
        for grp in [side, other] {
            if w.card_id(grp, slot) == card && w.ctrl(grp, slot) == side {
                count = count.wrapping_add(1);
            }
        }
    }
    i32::from(count)
}

/// 0x1232C0(rec): monsters with an effect entry: card 0x235 with an own
/// committed face-up card in the slots 6..=10
/// ([`committed_face_up_others`]) gets plan 3 at its own cell first; then
/// every card runs [`monster`](self::monster)(rec, 0), whose passes
/// overwrite that plan (a target it wrote survives only when the passes
/// write none).
fn effect_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.unit(g, s).is_none() {
        return Ok(());
    }
    let ctrl = w.ctrl(g, s);
    let my = w.pos(g, s);
    if w.card_id(g, s) == id::FACE_UP_COUNTER && committed_face_up_others(w, ctrl) > 0 {
        rec.set_plans(0x3FF);
        set_target(rec, my);
    }
    monster(w, rec, 0)
}

/// 0x123420(rec): class-4 cards. Every path ends in plan 5 (0x1235C8 is
/// reached by all of them after the plan-3 writes), but the target writes
/// stay: card 0x334 → the cell (col 0, row 1) when an empty terrain-1 cell
/// exists (the scan over the columns picks the LAST one, 0x1E3530 = 1 and
/// no unit, the distance compare against a best of −1 that is never
/// updated accepting every cell, but 0x1235B0 writes the constant 0x100
/// and the pick only decides between that and plan 5); 0x332 → the cell
/// (col 0, row 1); 0x33D → the cell (0, 0); others → nothing.
fn class4<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.unit(g, s).is_none() {
        return;
    }
    // 0x24F790(g, s) and 0x1E2310(g, s) are called; the position only
    // feeds the dead distance compare.
    let _ = w.ctrl(g, s);
    let _ = w.pos(g, s);
    match w.card_id(g, s) {
        id::TRAP_334 => {
            rec.set_plans(0x3FF);
            let mut pick = -1;
            for col in 0..7 {
                for row in 0..7 {
                    if w.terrain(col, row) == TERRAIN_334 && w.at(col, row).is_none() {
                        pick = row << 8 | col;
                    }
                }
            }
            if pick >= 0 {
                set_target(rec, 0x100);
            }
        }
        id::TRAP_332 => {
            rec.set_plans(0x3FF);
            set_target(rec, 0x100);
        }
        id::TRAP_33D => {
            rec.set_plans(0x3FF);
            set_target(rec, 0);
        }
        _ => (),
    }
    rec.set_plans(0x5FF);
}

/// 0x127290(side, card) = Pegasus's 0x139860: 0x110210 without the moth
/// exclusions and with the tribute ATK limit 2600.
pub(super) fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    ritual_ok_with(w, side, card, RITUAL_ATK_LIMIT, false)
}

/// 0x126C50(g, s, rec): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. By class (jump table 0x2BA370): monsters 0x235 → 1
/// when, with the OTHER group's unit in the own slot number committed
/// face-up (0x126F84 tests slot `s`, not the loop's), the other group's
/// slots 6..=10 hold a unit the own side controls (never while control
/// does not change hands), 0x251 → 1 with three own kind-0x10 units
/// ([`kind_count`]), 0x258 / 0x230 → 1 with two copies of the other
/// ([`card_count`]), others 2; magic: 2 without an entry 0 or under a
/// type-0xE far trap (0x27F170 mode 6, see-all 0), else 1 with three or
/// more own units matching the entry's low byte & 0x3F as a 0x24E360 type
/// (rel 1, distance 0xE); rituals: 2 under the far trap, when
/// [`ritual_ok`] refuses or on Crush, else 1; power-ups, traps and
/// class-4 cards 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    let pos = w.pos(g, s);
    let ctrl = w.ctrl(g, s);
    let entry0 = w.unit(g, s).and_then(|u| u.entry(0));
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    let at_least = |n: i32, need: i32| if n < need { 2 } else { 1 };
    match w.kind(g, s) >> 5 {
        0 => Ok(match card {
            id::FACE_UP_COUNTER => {
                let other = i32::from(g == 0);
                let mut count = 0u8;
                for slot in 6..11 {
                    for grp in [g, other] {
                        if w.flag(grp, s, 4) && w.ctrl(grp, slot) == g {
                            count = count.wrapping_add(1);
                        }
                    }
                }
                at_least(i32::from(count), 1)
            }
            id::KIND_10_TRIO => at_least(kind_count(w, g, 0x10), 3),
            id::PAIR_A => at_least(card_count(w, g, id::PAIR_B), 2),
            id::PAIR_B => at_least(card_count(w, g, id::PAIR_A), 2),
            _ => 2,
        }),
        1 => {
            let Some([w0, _]) = entry0 else {
                return Ok(2);
            };
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            let mask = unit_mask(w, 0, 0, ctrl, 1, w0 & 0x3F, 0xE)?;
            Ok(at_least(popcount(mask), 3))
        }
        5 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            if ritual_ok(w, w.ctrl(g, s), card)? != 1 {
                return Ok(2);
            }
            Ok(if w.terrain(pos & 0xFF, pos >> 8) == 9 {
                2
            } else {
                1
            })
        }
        _ => Ok(2),
    }
}

#[cfg(test)]
mod tests;
