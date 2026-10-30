//! Jasper Dice's summon planner 0x156040 (Tea's 0x13F7B0 shape with
//! Jasper's hidden strength 0x159050 and a cell function that ignores the
//! target) and its subtree: the category choice 0x1562D0, the Exodia piece
//! list 0x156560 with its sort 0x156880 / swap 0x156B80, the card test
//! 0x157140 and the summon cell 0x157410 with 0x157770 / 0x157840 are his
//! own; the non-monster choice 0x156BB0 is Pegasus's 0x137250; the monster
//! choice 0x156DB0 is Joey's 0x151F70 (affordability before the tree)
//! comparing only the low half of the score ([`ChoiceExtra::low_half_score`])
//! with the fodder pass 0x156FA0 (Weevil's 0x10EF80 without its limit and
//! type tests, the card test 0x157140); the card list 0x1571E0 is
//! Margaret's 0x149850.

use super::super::margaret::summon::card_list;
use super::super::pegasus::summon::{affordable_monster, category_of, non_monster, zone};
use super::super::summon::{
    atk_def, data, equip, equip_bonus, kind_of, monster_choice_with, virt, ChoiceExtra,
    FodderFilter, Hand, MonsterChoice,
};
use super::super::world::{unsupported, Unit, World, R};
use super::super::{Decision, FusionData};
use super::{exodia_ready, first_free, id, is_piece, FALLBACK_CELL, LEADER_CELL};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x157140: 2·ATK + DEF from which a monster is kept (`slti 0x1195`).
const STRONG: u32 = 0x1195;

/// 0x156DB0 / 0x156FA0: 0x27A850 scoring at every tier (no 0x23BCD0
/// call), no fodder limit or type test, the card test 0x157140.
const JASPER_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: true,
    fodder_limit: u32::MAX,
    fodder_filter: FodderFilter::UntypedNonMonsterStrongOr {
        strong: STRONG,
        first: id::FIRST_PIECE,
        last: id::LAST_PIECE,
    },
    skip_root: 999,
};

/// 0x156DB0: 0x271440 before the tree; the low half of the score compared.
const JASPER_EXTRA: ChoiceExtra = ChoiceExtra {
    affordable_first: true,
    low_half_score: true,
    ..ChoiceExtra::NONE
};

/// 0x157410: the starting distance bound of the neighbour scan.
const CELL_DISTANCE_LIMIT: i32 = 7;
/// 0x157770: the cells next to (3, 4) a piece is summoned to, in order.
const PIECE_CELLS: [i32; 4] = [0x303, 0x404, 0x402, 0x503];
/// 0x157840: the cells of cards 0x335 / 0x338 (col 2 / col 4, row 3).
const PAIR_CELLS: [i32; 2] = [0x302, 0x304];
/// Labyrinth (8) and Crush (9) terrain.
const LABYRINTH: i32 = 8;
const CRUSH: i32 = 9;
/// 0x156880's recursion depth past which the port gives up (a sort of at
/// most five entries that does not cycle stays far below it; a cycle is
/// caught exactly by [`sort`]'s ancestor check first).
const SORT_DEPTH: u32 = 64;
/// 0x156560: the u16 an unused list entry holds.
const UNUSED_ENTRY: i32 = 0xFF;

/// 0x157140(card): kept out of the fodder pass and the piece list's
/// ordering: a non-monster, a monster with 2·ATK + DEF (0x2714E0) of 0x1195
/// or more, or an Exodia piece.
fn keeps<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, card: i32) -> bool {
    let v = atk_def(w, card);
    kind_of(w, card) >> 5 != 0 || (v & 0xFFFF) + ((v >> 16) << 1) >= STRONG || is_piece(card)
}

/// 0x2713B0(card, side): 0xFF without a card record, else the cost.
fn cost<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, card: i32, side: i32) -> R<i32> {
    if data(w, card).is_none() {
        return Ok(0xFF);
    }
    w.cost(card, side)
}

/// Who called 0x156560, which decides the stack residue its sort can read
/// (REFUSALS-SWEEP.md §7: [F] + [emu] runs of the module through the duel
/// state machine).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Site {
    /// 0x1562D0 (the category, its local record): `via_actor` when
    /// 0x156040 was called by the actor choice 0x1553A0, else by the
    /// dispatch 0x154AF0.
    Category { via_actor: bool },
    /// 0x156040 itself (the action record).
    Summon,
    /// The reveal request 0x158C30 (a local record).
    Reveal,
}

/// The 20 bytes from sp+0xD0 of 0x156560's frame that its sort 0x156880
/// can touch: 0..10 the five u16 list entries (initialised to 0x00FF),
/// 10..16 frame padding and 16..20 the caller's frame (the low word of the
/// caller's saved $s0 or of an outgoing argument): stack residue, `None`
/// where the port does not know it.
/// - Padding: at [`Site::Category`] and [`Site::Reveal`] 0x156560 follows
///   [`super::exodia_ready`] 0x1590D0 directly, whose prologue (`sq $ra,
///   16($sp)`, frame 0x20) stored $ra as a quad at the caller's sp - 0x10,
///   the list's bytes 0..16: bytes 10..16 are bits 80..127 of $ra, 0
///   [emu; H for the hardware: no MMI op writes $ra]. At [`Site::Summon`]
///   they are 0x278850's halfwords sp+0x1BA / +0x1BC / +0x1BE (its node
///   pool's byte and node counters and its target count; the port does not
///   run that search): unknown.
/// - Caller word: [`Site::Summon`]: 0 (0x156104 `sd $zero, 0($sp)`);
///   [`Site::Category`]: 0x1562D0's saved $s0 = 0x156040's $s0 at entry,
///   the side from the dispatch (0x154B1C `move $s0, $v0` of 0x1DF7C0) but
///   the action record's address (a stack address) from 0x1553A0
///   (0x1553CC); [`Site::Reveal`]: the $s0 of 0x158320 / 0x157D50 at the
///   call (a target cell, a counter): unknown.
struct SortBuf([Option<u8>; 20]);

impl SortBuf {
    fn new(site: Site, side: i32) -> Self {
        let mut b = [None; 20];
        for (i, slot) in b.iter_mut().take(10).enumerate() {
            *slot = Some(if i % 2 == 0 { 0xFF } else { 0 });
        }
        if matches!(site, Site::Category { .. } | Site::Reveal) {
            for slot in &mut b[10..16] {
                *slot = Some(0);
            }
        }
        let caller = match site {
            Site::Summon => Some(0u32),
            Site::Category { via_actor: false } => Some(side as u32),
            _ => None,
        };
        if let Some(v) = caller {
            for (slot, byte) in b[16..20].iter_mut().zip(v.to_le_bytes()) {
                *slot = Some(byte);
            }
        }
        Self(b)
    }
    /// `sh`: entry `i` (u16).
    fn set(&mut self, i: usize, v: u16) {
        let [lo, hi] = v.to_le_bytes();
        self.0[2 * i] = Some(lo);
        self.0[2 * i + 1] = Some(hi);
    }
    /// `lhu`: entry `i` (u16).
    fn entry(&self, i: i32) -> R<i32> {
        let at = usize::try_from(i).ok().map(|i| 2 * i);
        match at.and_then(|a| Some((*self.0.get(a)?, *self.0.get(a + 1)?))) {
            Some((Some(lo), Some(hi))) => Ok(i32::from(u16::from_le_bytes([lo, hi]))),
            _ => unsupported("0x156880 compares a stack-residue list entry"),
        }
    }
    /// `lbu`: the low byte of entry `i`.
    fn low(&self, i: usize) -> R<u8> {
        match self.0.get(2 * i).copied().flatten() {
            Some(b) => Ok(b),
            None => unsupported("0x156560 copies a stack-residue list entry"),
        }
    }
    /// 0x156B80(list, i, j): swaps the 32-bit WORDS i and j of the u16
    /// list (quirk: entries 2i, 2i+1 with 2j, 2j+1).
    fn swap(&mut self, i: i32, j: i32) -> R<()> {
        let (Ok(i), Ok(j)) = (usize::try_from(i), usize::try_from(j)) else {
            return unsupported("0x156B80 swaps a word before the list");
        };
        if 4 * i.max(j) + 4 > self.0.len() {
            return unsupported("0x156B80 swaps a word past the caller's frame");
        }
        for b in 0..4 {
            self.0.swap(4 * i + b, 4 * j + b);
        }
        Ok(())
    }
}

/// 0x156880(list, lo, hi, side, hand): a quicksort of the list's hand
/// indices by descending cost (0x2713B0 of the hand card): the middle entry
/// swapped to `lo` is the pivot, entries whose cost exceeds the pivot's are
/// swapped to the front, the pivot swapped into place, then (quirk) the
/// ranges lo..=last (pivot included) and last+1..=hi are sorted. The
/// swaps move 32-bit words of the u16 list ([`SortBuf::swap`]), so a range
/// can come back to a state it already had: a call depends only on (lo,
/// hi, the 20 bytes) and the fixed hand, so an ancestor call in the same
/// state means the original recurses without end and never returns (the
/// game hangs) [emu: REFUSALS-SWEEP.md §7, through the state machine].
fn sort<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    buf: &mut SortBuf,
    lo: i32,
    hi: i32,
    side: i32,
    hand: &Hand,
    ff: Option<i32>,
    path: &mut Vec<(i32, i32, [Option<u8>; 20])>,
) -> R<()> {
    if lo >= hi {
        return Ok(());
    }
    if path.iter().any(|&(l, h, b)| l == lo && h == hi && b == buf.0) {
        return unsupported("0x156880 recurses without end (the original hangs)");
    }
    if path.len() as u32 > SORT_DEPTH {
        return unsupported("0x156880 recursion beyond the port's bound");
    }
    path.push((lo, hi, buf.0));
    let value = |idx: i32| -> R<i32> {
        if idx == UNUSED_ENTRY {
            // hand[0xFF] is the word at sp+0x4AC of 0x156560, in the
            // frames above the decision: `ff` where the port knows it
            // ([`exodia_list`]); elsewhere never a card id below 0x356 in
            // the oracle's runs (0xC33024, 0x1000019E, 0x84A60006), so
            // 0x2713B0 finds no record and answers 0xFF [emu; H for the
            // game's stack].
            return match ff {
                Some(card) => cost(w, card, side),
                None => Ok(0xFF),
            };
        }
        cost(w, hand.get(idx)?, side)
    };
    buf.swap(lo, (lo + hi) >> 1)?;
    let mut last = lo;
    for i in lo + 1..=hi {
        let (ei, elo) = (buf.entry(i)?, buf.entry(lo)?);
        // Equal indices read the same word (also the 0x00FF of an unused
        // entry, which reads past the hand buffer): never "less".
        if ei != elo && value(elo)? < value(ei)? {
            last += 1;
            buf.swap(last, i)?;
        }
    }
    buf.swap(lo, last)?;
    sort(w, buf, lo, last, side, hand, ff, path)?;
    sort(w, buf, last + 1, hi, side, hand, ff, path)?;
    path.pop();
    Ok(())
}

/// 0x156560(side, d, U): the Exodia piece summon. The first predicted-hand
/// index holding a piece (0x1590B0; −1 without one) starts the piece list;
/// U = its record; every hand index holding a power-up (class 2, not
/// 0x31B / 0x31C) that fits the piece (0x24A880 ≥ 0) is appended, its
/// 0x24ACC0 bonus added to U's +0x38 and +0x3A. The hand indices whose
/// card [`keeps`] rejects go into a u16 list sorted by [`sort`]. d+6.. =
/// the low bytes of the first (hand size − piece list length) list
/// entries, then the piece list (bytes past the hand size keep what they
/// held). Returns 1. `site`: the caller ([`SortBuf`]).
pub(super) fn exodia_list<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
    site: Site,
) -> R<i32> {
    let hand = Hand::of(w, side);
    let n = w.next_hand_size(side);
    let mut pieces = [0xFFu8; 5];
    let mut count = 0usize;
    for i in 0..n {
        if is_piece(hand.get(i)?) {
            pieces[0] = i as u8;
            count = 1;
            // 0x156630: the index is set to the hand size, ending the scan.
            break;
        }
    }
    if count == 0 {
        return Ok(-1);
    }
    let first = hand.get(i32::from(pieces[0]))?;
    let mut v = virt(w, first);
    for i in 0..n {
        let c = hand.get(i)?;
        if equip(w, first, c)? < 0 {
            continue;
        }
        if kind_of(w, c) >> 5 != 2 || c == id::POTION || c == id::CURSEBREAKER {
            continue;
        }
        let bonus = equip_bonus(w, c) as i16;
        v.atk_mod = v.atk_mod.wrapping_add(bonus);
        v.def_mod = v.def_mod.wrapping_add(bonus);
        let Some(slot) = pieces.get_mut(count) else {
            return unsupported("0x156560 writes past its piece list");
        };
        *slot = i as u8;
        count += 1;
    }
    *u = Some(v);
    let mut buf = SortBuf::new(site, side);
    let mut listed = 0usize;
    for i in 0..n {
        if !keeps(w, hand.get(i)?) {
            buf.set(listed, i as u16);
            listed += 1;
        }
    }
    // hand[0xFF]: from the actor choice the decision runs at 0x1D00F0's
    // sp - 0xA0 (0x1D00F0 -> 0x1CFF80 -> the driver 0x159380 -> 0x154AF0 ->
    // 0x1553A0 -> 0x156040 -> 0x1562D0 -> 0x156560: 0x340 bytes of frames),
    // so the word is 0x1D00F0's sp+0xCC, where 0x1D0170 stores side 1's LP
    // (0x1E06F0(1)) at every call [F; emu through the state machine:
    // REFUSALS-SWEEP.md §7]: an LP below 0x356 is a card id. The reveal
    // request from 0x158320 is at the same depth (the same word) but the
    // hook does not know its caller (0x157D50 is 0xA0 deeper).
    let ff = matches!(site, Site::Category { via_actor: true }).then(|| w.lp(1));
    sort(w, &mut buf, 0, listed as i32 - 1, side, &hand, ff, &mut Vec::new()).map_err(|e| {
        match e {
            crate::duel::DuelError::Unsupported(m) => {
                crate::duel::DuelError::Unsupported(format!("{m} ({site:?})"))
            }
            e => e,
        }
    })?;
    let m = (n as usize).saturating_sub(count);
    for j in 0..m {
        d.hand[j] = buf.low(j)?;
    }
    for (k, &p) in pieces.iter().take(count).enumerate() {
        let Some(slot) = d.hand.get_mut(m + k) else {
            return unsupported("0x156560 writes past d+0xA");
        };
        *slot = p;
    }
    Ok(1)
}

/// 0x1562D0(side, U): what to summon: 0 a monster (U NOT set), 1 a
/// non-monster (U set), 2 a trap (U set), 3 the Exodia piece list (U set),
/// −1 nothing. In order: monster zone full → the non-monster choice;
/// magic/trap zone full → the first affordable monster (U set); the first
/// predicted-hand card of category 2 (0x1DF290: classes 3 and 4,
/// affordable or not); [`exodia_ready`] with a piece in hand
/// ([`exodia_list`] on a local record); the first affordable monster —
/// returning before 0x1DDFE0; the non-monster choice.
fn category<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &mut Option<Unit>,
    via_actor: bool,
) -> R<i32> {
    let monsters = zone(w, side, 0) & 0xFF;
    let others = zone(w, side, 1) & 0xFF;
    let hand = Hand::of(w, side);
    // sp+0x70: a local action record (written, never read).
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
    for i in 0..w.next_hand_size(side) {
        let c = hand.get(i)?;
        if category_of(kind_of(w, c) >> 5) == 2 {
            *u = Some(virt(w, c));
            return Ok(2);
        }
    }
    if exodia_ready(w, side)?
        && exodia_list(w, side, &mut local, u, Site::Category { via_actor })? >= 0
    {
        return Ok(3);
    }
    if affordable_monster(w, side, &hand)?.is_some() {
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u, 999)?;
    Ok(if v < 0 { -1 } else { v })
}

/// 0x157770(): the first of (3, 3), (4, 4), (2, 4), (3, 5) (col, row)
/// holding no unit, −1 when all do.
fn piece_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> i32 {
    PIECE_CELLS
        .into_iter()
        .find(|&c| w.at(c & 0xFF, c >> 8).is_none())
        .unwrap_or(-1)
}

/// 0x157840(): (col 2, row 3) when empty, else (col 4, row 3) when empty,
/// else −1.
fn pair_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> i32 {
    PAIR_CELLS
        .into_iter()
        .find(|&c| w.at(c & 0xFF, c >> 8).is_none())
        .unwrap_or(-1)
}

/// 0x157410(side, _, U, _): the summon cell (the category and target
/// arguments are not read). An Exodia piece → [`piece_cell`] and cards
/// 0x335 / 0x338 → [`pair_cell`], both only while the leader stands on
/// (3, 4) (−1 otherwise). Others: the goal is the first free cell of the
/// formation of U's class, else of the other formation (mode 1 for
/// monsters, 0 for the rest), else (3, 5); of the leader's neighbours
/// (0x24FD30) that are empty (0x1E4A10) and neither Labyrinth nor Crush,
/// the one with the smallest Manhattan distance to the goal (at most 7,
/// later ties win) whose row is not above the row of the last accepted one
/// (starting with the leader's); −1 when none.
fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32, u: &Unit) -> R<i32> {
    let leader = w.pos(side, 0);
    let class = i32::from(u.kind >> 5);
    let card = i32::from(u.card);
    if is_piece(card) {
        return Ok(if leader == LEADER_CELL { piece_cell(w) } else { -1 });
    }
    if card == id::CARD_335 || card == id::CARD_338 {
        return Ok(if leader == LEADER_CELL { pair_cell(w) } else { -1 });
    }
    let goal = first_free(w, class)
        .or_else(|| first_free(w, i32::from(class == 0)))
        .unwrap_or(FALLBACK_CELL);
    let (gc, gr) = (goal & 0xFF, goal >> 8);
    let mut best = CELL_DISTANCE_LIMIT;
    let mut min_row = leader >> 8;
    let mut pick = -1;
    for i in 0..8 {
        let cell = super::neighbour(w, side, i)?;
        if cell < 0 {
            continue;
        }
        let (col, row) = (cell & 0xFF, cell >> 8);
        let t = w.terrain(col, row);
        if t == LABYRINTH || t == CRUSH || w.at_key(col, row) >= 0 {
            continue;
        }
        let dist = (row - gr).abs() + (col - gc).abs();
        if best < dist || row < min_row {
            continue;
        }
        pick = cell;
        best = dist;
        min_row = row;
    }
    Ok(pick)
}

/// 0x156040(key, side, d): fill `d` with a summon (type 0xA, +0xC = 0) or
/// return −1: summon flag clear, both zones full, no category. The target
/// search 0x278850 (depth 3, filter 0x1F, see-all 0, hidden strength
/// 0x159050(side)) runs on the category's U — on its first-affordable-
/// monster answer an unwritten buffer — but its result is only passed to
/// the cell function, which does not read it, and to a test for six or
/// more monsters (0x1E13D0 counts at most five), so the port does not run
/// it [F]. Category 0: the monster choice 0x156DB0 (always finds the
/// category's monster); 2: the card list 0x1571E0; 3: [`exodia_list`] on
/// the action record; 1 (or a failed monster choice): the non-monster
/// choice filtered by U's card. Then [`cell`]. `via_actor`: called by the
/// actor choice 0x1553A0 (else by the dispatch 0x154AF0; the stack residue
/// of [`exodia_list`] differs).
pub(super) fn summon<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    key: i32,
    side: i32,
    d: &mut Decision,
    via_actor: bool,
) -> R<i32> {
    d.key = key as u16;
    if !w.can_summon(side) {
        return Ok(-1);
    }
    if zone(w, side, 0) == 5 && zone(w, side, 1) == 5 {
        return Ok(-1);
    }
    let mut u = None;
    let r = category(w, side, &mut u, via_actor)?;
    if r < 0 {
        return Ok(-1);
    }
    d.kind = 0xA;
    d.reveal = 0;
    let non_monster_path = match r {
        0 => monster_choice_with(w, side, d, &mut u, JASPER_CHOICE, JASPER_EXTRA)? < 0,
        2 => {
            card_list(w, side, d, &mut u)?;
            false
        }
        3 => {
            exodia_list(w, side, d, &mut u, Site::Summon)?;
            false
        }
        _ => true,
    };
    if non_monster_path {
        // 0x156260: filter = U +0x3C.
        let Some(card) = u.as_ref().map(|u| i32::from(u.card)) else {
            return unsupported("0x156040 reads an uninitialised U");
        };
        if non_monster(w, side, d, &mut u, card)? < 0 {
            return Ok(-1);
        }
    }
    let Some(unit) = u.as_ref() else {
        return unsupported("0x157410 reads an uninitialised U");
    };
    let c = cell(w, side, unit)?;
    if c < 0 {
        return Ok(-1);
    }
    d.target = c as u16;
    Ok(1)
}
