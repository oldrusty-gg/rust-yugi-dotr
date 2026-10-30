//! The summon planner 0x10E620 and its subtree: the category choice
//! 0x10E890, the fusion-tree monster choice 0x10EBF0 (tree helpers
//! 0x27A210..0x27AAB0), the non-monster choice 0x10FD20 with the ritual
//! tribute check 0x110210 (tribute solver 0x2561B0), Cocoon of Evolution
//! (0x10FB60 / 0x10EE10) and the summon cell 0x10F160.
//!
//! The hand the planner reads is the predicted post-draw hand of 0x271660
//! (hand cards in slot order, then the deck top), `n` = 0x271750 entries.
//! Its callers keep that list in an uninitialised stack buffer of five
//! words and the fusion tree reads all five slots: past `n` (hand + deck
//! < 5, which a real turn never reaches) a child frame sees what its
//! parent's 0x27A270 call left there (`Hand::get_in_child`); the words
//! that hold the upper half of a 128-bit register are 0 below a child and
//! refused below a root (see there).
//!
//! The virtual unit U (0x10E620 sp+0x70) is `Option<Unit>`: `None` until
//! 0x1DDFE0 first writes it.

use super::search::{attack_path, find_target, path, step_count, Steps};
use super::world::{unsupported, virtual_unit, Unit, World, R};
use super::{Decision, FusionData, EQUIP_RANDOM};
use crate::duel::{CardData, CardInfo};

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the planner tests as immediates.
mod id {
    /// Cocoon of Evolution.
    pub const COCOON: i32 = 0x31E;
    /// Paralyzing Potion and Cursebreaker (never appended as power-ups).
    pub const POTION: i32 = 0x31B;
    pub const CURSEBREAKER: i32 = 0x31C;
    /// Larva of Moth (the Cocoon's power-up test monster, 0x10EE10).
    pub const LARVA: i32 = 0x1AB;
    /// Hercules Beetle (never fodder, 0x10FAF0).
    pub const HERCULES: i32 = 0x18E;
    /// Summoned Lord Exodia: 0x1DE350 maps it to 999 (no card data).
    pub const EXODIA_LORD: i32 = 0x29F;
}

// ---- card data helpers -------------------------------------------------------

/// 0x1DE350(card): the card record, none for 0x29F (mapped to 999) and ids
/// outside 0..0x356.
pub(super) fn data<'a, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<'a, C, F>,
    card: i32,
) -> Option<&'a CardInfo> {
    let card = if card == id::EXODIA_LORD { 999 } else { card };
    if !(0..0x356).contains(&card) {
        return None;
    }
    w.card(card as u16)
}

/// 0x271610(card): card byte 0 (0 for 999 and cards without a record).
pub(super) fn kind_of<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, card: i32) -> i32 {
    if card == 999 {
        return 0;
    }
    data(w, card).map_or(0, |c| i32::from(c.kind))
}

/// 0x2714E0(card): (ATK & 0x1FFF) << 16 | (DEF & 0x1FFF), 0 for 999 and
/// cards without a record.
pub(super) fn atk_def<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, card: i32) -> u32 {
    if card == 999 {
        return 0;
    }
    data(w, card).map_or(0, |c| {
        u32::from(c.attack & 0x1FFF) << 16 | u32::from(c.defense & 0x1FFF)
    })
}

/// 0x271540(card): 2 for a monster without effect entries (card +4 =
/// 0xFFFF), 1 for one with, 0 otherwise.
fn monster_type<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, card: i32) -> i32 {
    if card == 999 {
        return 0;
    }
    match data(w, card) {
        Some(c) if c.kind >> 5 == 0 => {
            if c.entries.iter().all(Option::is_none) {
                2
            } else {
                1
            }
        }
        _ => 0,
    }
}

/// 0x2713B0(card, side): 0xFF without a card record, else the world cost.
fn cost<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, card: i32, side: i32) -> R<i32> {
    if data(w, card).is_none() {
        return Ok(0xFF);
    }
    w.cost(card, side)
}

/// 0x271440(side, card): stars ≥ cost.
pub(super) fn affordable<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<bool> {
    Ok(w.stars(side) >= cost(w, card, side)?)
}

/// 0x271490(side, card): 0x271440 on card & 0xFFF.
fn affordable_12<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<bool> {
    affordable(w, side, card & 0xFFF)
}

/// 0x1DDFE0(U, card): the virtual unit record for `card` (card 999, kind
/// and attribute 0xFF, no entries when the card has no record).
pub(super) fn virt<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, card: i32) -> Unit {
    match data(w, card) {
        Some(info) => virtual_unit(info),
        None => Unit {
            card: 999,
            kind: 0xFF,
            attribute: 0xFF,
            col: 7,
            row: 7,
            side: 2,
            slot: 0xB,
            order: 0,
            discount: 0,
            spellbind: 0,
            x4a: 0,
            flags: 0,
            atk_mod: 0,
            def_mod: 0,
            entries: [None; 5],
            id: None,
        },
    }
}

/// 0x24A880(monster, card) as u16 arguments. The rand() branch of
/// 0x24A960 (a power-up of value 9, Insect Imitation, on Ryu-Ran 0x10/0x11,
/// Monster Egg 0xA7, 0x14C or Wing Egg Elf 0x17B) draws one rand()
/// (0x19AE88, [`World::rand31`]) and answers the transformed card:
/// 0xD + r % 5, 0xD1 + r % 15, 0x65 + (r & 0x1F), or 0x24ADF0's
/// [0x23, 0x4F, 0x2C, 0x5C, 0x17B, 0x18A][r % 6] [F 0x24AA54..0x24AAE8,
/// 0x24ADF0; code immediates].
pub(super) fn equip<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    monster: i32,
    card: i32,
) -> R<i32> {
    let e = w.fusions.equip(monster as u16, card as u16);
    if e == EQUIP_RANDOM {
        let r = w.rand31() as i32;
        return Ok(match monster & 0xFFFF {
            0x10 | 0x11 => r % 5 + 0xD,
            0xA7 => r % 15 + 0xD1,
            0x14C => (r & 0x1F) + 0x65,
            _ => [0x23, 0x4F, 0x2C, 0x5C, 0x17B, 0x18A][(r % 6) as usize],
        });
    }
    Ok(e)
}

/// 0x24AC40(card): −1 for categories 2 and 3 and for category 4 with value
/// 9, else 1.
fn equip_kind<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, card: i32) -> i32 {
    match w.fusions.category(card as u16) {
        4 => {
            if w.fusions.value(card as u16) == 9 {
                -1
            } else {
                1
            }
        }
        3 | 2 => -1,
        _ => 1,
    }
}

/// 0x24ACC0(card): the power-up bonus: the u16 value for category 0, 1000
/// for categories 1 and 4, else 0.
pub(super) fn equip_bonus<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    card: i32,
) -> u32 {
    match w.fusions.category(card as u16) {
        4 | 1 => 1000,
        0 => u32::from(w.fusions.value(card as u16)),
        _ => 0,
    }
}

/// The 0x271660 list: the predicted hand (`n` = 0x271750 entries).
pub(super) struct Hand(Vec<i32>);

impl Hand {
    /// 0x271660(side, buf).
    pub fn of<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> Self {
        Self(w.predicted_hand(side))
    }
    /// Word `i` of the buffer; slots the original leaves uninitialised are
    /// refused.
    pub fn get(&self, i: i32) -> R<i32> {
        match usize::try_from(i).ok().and_then(|i| self.0.get(i)) {
            Some(&c) => Ok(c),
            None => unsupported("0x271660 buffer slot past the predicted hand"),
        }
    }
    /// Word `i` as a child frame of 0x27A3D0 reads it (its own buffer at
    /// sp+0x90). Past the predicted hand the slots still hold what the
    /// parent frame's 0x27A270 call saved there (its frame sits at the
    /// child's sp+0x70): words 0..3 = the parent's s2 as a 128-bit `sq`
    /// (word 1 = the high half of its hand index = 0), word 4 = that call's
    /// return address (0x27A4DC below a root, 0x27A4A0 below a child),
    /// never an affordable card.
    ///
    /// Words 2..3 are the upper 64 bits of the parent's s2, which only
    /// 128-bit moves write (`paddub`, the EE's register move): s2 is the
    /// parent's a2 at entry. Below a child that a2 is 0: every parent zeroes
    /// a2 in 0x27A6D0 before its loop, and the only 128-bit writes to a2 and
    /// t0 in the subtree copy $zero (0x27A6D0, 0x27A710, 0x24AEE0), copy a2
    /// into itself (0x2520A0), copy t0 (0x27A48C / 0x27A4C8: 0 from the
    /// root call's `move $t0, $zero` at 0x10ECA4 / 0x137504) or are
    /// overwritten on the same path (0x2730AC, then 0x1E1EC4 with the zero
    /// 0x2730B8 passed as a3). Below a root they are the a2 upper half the
    /// summon planner left before 0x10EBF0 / 0x137450, a data-dependent
    /// chain of register copies; it is 0 as well: no function reachable
    /// from the 11 ported drivers (563, no `jalr`) creates a non-zero upper
    /// half (no `lq` off a base other than $sp, no MMI arithmetic, no
    /// `qmfc2`), so every upper half there is a copy of $zero or of a
    /// register's upper half at the driver's entry, and traced through the
    /// drivers (128-bit
    /// provenance tracker, all 102 such oracle cases of the 10 modules) the
    /// chain ends at a `$zero` copy or at the driver's a0, which the duel
    /// step 0x1D00F0 clears with `move $a0, $zero` at 0x1D33B8 (on every
    /// path to the 0x1CFF80 call at 0x1D3570; nothing after it writes a0's
    /// upper half while the side's +0x334 is 0, and only zeroes are stored
    /// there). See `work/research/duel/claude-ai/TIE-0x120A20.md`.
    /// [F static + emu tracker]
    fn get_in_child(&self, i: i32, parent_is_root: bool) -> R<i32> {
        if usize::try_from(i).is_ok_and(|i| i < self.0.len()) {
            return self.get(i);
        }
        match i {
            1..=3 => Ok(0),
            4 => Ok(if parent_is_root { 0x27A4DC } else { 0x27A4A0 }),
            _ => unsupported("0x27A3D0 reads its parent's saved s2 past an empty predicted hand"),
        }
    }
}

// ---- fusion tree (0x27A210..0x27AAB0) -----------------------------------------

/// 0x27A6D0(path): entries before the first 0xFF (at most 5).
fn path_len(path: &[u8]) -> usize {
    path.iter().take(5).position(|&b| b == 0xFF).unwrap_or(5)
}

/// 0x27A740(v, path): `v` is one of the five path bytes.
fn in_path(v: u8, path: &[u8]) -> bool {
    path.iter().take(5).any(|&b| b == v)
}

/// 0x27A660(path, v): insert `v` in front (nothing when the path is full).
fn prepend(path: &mut [u8], v: u8) {
    if path_len(path) == 5 {
        return;
    }
    for i in (0..4).rev() {
        path[i + 1] = path[i];
    }
    path[0] = v;
}

/// A fusion-tree node (0x18 bytes in the pool at 0x1ED0000): +0 card,
/// +2..+6 the path of hand indices (0xFF unused), +7 padding (the sixth
/// byte, which 0x27A850 writes when the path is full), +8 four children.
#[derive(Clone, Copy, Debug)]
struct Node {
    card: u16,
    path: [u8; 6],
    children: [Option<usize>; 4],
}

/// The pool at 0x1ED0000 with 0x10EBF0's two u16 counters (sp+0xEC bytes,
/// sp+0xEE nodes), reset for every root.
#[derive(Default)]
pub(super) struct Tree {
    bytes: u16,
    count: u16,
    nodes: Vec<Node>,
}

impl Tree {
    /// 0x27A210(&bytes, &count): the next node, none once bytes + 0x38
    /// would exceed 0x10000 (after 2729 nodes).
    fn alloc(&mut self) -> Option<usize> {
        if u32::from(self.bytes) + 0x38 >= 0x10001 {
            return None;
        }
        let i = usize::from(self.count);
        self.count = self.count.wrapping_add(1);
        self.bytes = self.bytes.wrapping_add(0x18);
        let blank = Node {
            card: 0,
            path: [0; 6],
            children: [None; 4],
        };
        if i >= self.nodes.len() {
            self.nodes.resize(i + 1, blank);
        }
        Some(i)
    }

    /// 0x27A270(node, card, parent path, idx): path = the parent's path +
    /// `idx` (or just `idx` for a root), no children.
    fn init(&mut self, n: usize, card: u16, parent: Option<usize>, idx: u8) {
        let pad = self.nodes[n].path[5];
        let mut path = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, pad];
        match parent {
            None => path[0] = idx,
            Some(p) => {
                let pp = self.nodes[p].path;
                path[..5].copy_from_slice(&pp[..5]);
                path[path_len(&pp)] = idx;
            }
        }
        self.nodes[n] = Node {
            card,
            path,
            children: [None; 4],
        };
    }

    /// 0x27A3D0(side, _, idx, card, parent, &bytes, &count): the node for
    /// hand index `idx` below `parent` (a root holds `card` itself; a child
    /// combines the parent's card with L[idx], which must be affordable on
    /// its own and combine to something other than 999), then children for
    /// every index not on the path, ascending, until the path has 5 entries.
    fn build<C: CardData + ?Sized, F: FusionData + ?Sized>(
        &mut self,
        w: &W<C, F>,
        side: i32,
        hand: &Hand,
        idx: u8,
        card: u16,
        parent: Option<usize>,
    ) -> R<Option<usize>> {
        let n = if let Some(p) = parent {
            // A root's path holds only its own index.
            let parent_is_root = path_len(&self.nodes[p].path) == 1;
            let c = hand.get_in_child(i32::from(idx), parent_is_root)?;
            if !affordable(w, side, c)? {
                return Ok(None);
            }
            let fused = (combine(w, i32::from(card), c & 0xFFFF)? & 0xFFFF) as u16;
            if fused == 999 {
                return Ok(None);
            }
            let Some(n) = self.alloc() else {
                return Ok(None);
            };
            self.init(n, fused, parent, idx);
            n
        } else {
            let Some(n) = self.alloc() else {
                return Ok(None);
            };
            self.init(n, card, None, idx);
            n
        };
        if path_len(&self.nodes[n].path) == 5 {
            return Ok(Some(n));
        }
        let mut slot = 0;
        for j in 0..5u8 {
            if in_path(j, &self.nodes[n].path) {
                continue;
            }
            let card = self.nodes[n].card;
            if card == 999 {
                continue;
            }
            let child = self.build(w, side, hand, j, card, Some(n))?;
            // The pointer is stored even when null; the slot only advances
            // for a real child.
            self.nodes[n].children[slot] = child;
            if child.is_some() {
                slot += 1;
            }
        }
        Ok(Some(n))
    }

    /// 0x27A5C0(side, _, idx, card, null, &bytes, &count): a root without
    /// children (none when the pool is full).
    fn single(&mut self, idx: u8, card: u16) -> Option<usize> {
        let n = self.alloc()?;
        self.init(n, card, None, idx);
        Some(n)
    }

    /// 0x27A780(node): the node with the highest ATK, pre-order, strict >
    /// (ties keep the earlier one).
    fn best_atk<C: CardData + ?Sized, F: FusionData + ?Sized>(
        &self,
        w: &W<C, F>,
        node: Option<usize>,
    ) -> Option<usize> {
        let n = node?;
        let mut best = n;
        let mut best_v = atk_def(w, i32::from(self.nodes[n].card)) >> 16;
        for k in 0..4 {
            if let Some(s) = self.best_atk(w, self.nodes[n].children[k]) {
                let v = atk_def(w, i32::from(self.nodes[s].card)) >> 16;
                if best_v < v {
                    best_v = v;
                    best = s;
                }
            }
        }
        Some(best)
    }
}

/// 0x27A330(a, b): 999 if either is 999; the 0x24A880 transformation when
/// it is below 0x10000 and 0x24AC40(b) = 1; else the fusion 0x24AEC0 (999
/// when none).
fn combine<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, a: i32, b: i32) -> R<i32> {
    let (a, b) = (a & 0xFFFF, b & 0xFFFF);
    if a == 999 || b == 999 {
        return Ok(999);
    }
    let e = equip(w, a, b)?;
    if e >> 16 == 0 && equip_kind(w, b) == 1 {
        return Ok(e & 0xFFFF);
    }
    Ok(i32::from(w.fusions.fuse(a as u16, b as u16).unwrap_or(999)))
}

/// 0x27AA70(side, node, 0, 0): (ATK << 16) | DEF of the node's card.
fn score_plain<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, card: u16) -> i32 {
    let v = atk_def(w, i32::from(card));
    ((v >> 16) << 16 | (v & 0xFFFF)) as i32
}

/// 0x27A850(side, node, 0, 0) (tier ≥ 3): 0 while a face-up (bit 4) type
/// 0x2B far trap is out (0x26C4B0); else append every unused hand index
/// holding a power-up (class 2, not 0x31B/0x31C) that gives the node's card
/// a plain bonus (0x24A880 >> 16 = 1) with 0x24AC40 = 1 to the node's path
/// and add its 0x24ACC0 bonus to ATK and DEF. (The callers' third argument
/// is always null: with a list the original only calls 0x27AAB0 and appends
/// nothing.)
pub(super) fn score_equips<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    hand: &Hand,
    tree: &mut Tree,
    best: usize,
) -> R<i32> {
    let card = i32::from(tree.nodes[best].card);
    let v = atk_def(w, card);
    let mut atk = v >> 16;
    let mut def = v & 0xFFFF;
    let k = w.far_trap_for(side, 0x2B);
    if k >= 0 && w.flag(k >> 8, k & 0xFF, 4) {
        return Ok(0);
    }
    for j in 0..w.next_hand_size(side) {
        if in_path(j as u8, &tree.nodes[best].path) {
            continue;
        }
        let c = hand.get(j)?;
        if kind_of(w, c) >> 5 != 2 {
            continue;
        }
        if c == id::CURSEBREAKER || c == id::POTION {
            continue;
        }
        if equip(w, card, c)? >> 16 != 1 {
            continue;
        }
        if equip_kind(w, c) != 1 {
            continue;
        }
        let path = &mut tree.nodes[best].path;
        let len = path_len(path);
        path[len] = j as u8;
        atk = atk.wrapping_add(equip_bonus(w, c));
        def = def.wrapping_add(equip_bonus(w, c));
    }
    Ok((atk << 16 | def & 0xFFFF) as i32)
}

// ---- monster choice ------------------------------------------------------------

/// What differs between the modules' copies of the monster choice and its
/// fodder pass (0x10EBF0 / 0x10EF80 for Weevil, 0x137450 / 0x137630 for
/// Pegasus).
#[derive(Clone, Copy, Debug)]
pub(super) struct MonsterChoice {
    /// Score with 0x27A850 (power-ups) at every tier (Pegasus) instead of
    /// 0x27AA70 below tier 3 (Weevil).
    pub equips_at_any_tier: bool,
    /// Fodder limit on 2·ATK + DEF (`sltiu`: 0xDAD Weevil, 0x3E9 Pegasus).
    pub fodder_limit: u32,
    /// The monsters the fodder pass skips ([`FodderFilter`]).
    pub fodder_filter: FodderFilter,
    /// A monster never taken as a root (Keith's 0x11F830 skips 0x156);
    /// 999 for none.
    pub skip_root: i32,
}

/// The card test of the modules' fodder passes (the `0x10FAF0` slot of
/// 0x10EF80 and its copies): a card it accepts is never fodder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FodderFilter {
    /// 0x10FAF0: a monster that takes Cocoon of Evolution, or Hercules
    /// Beetle (Weevil).
    Cocoon,
    /// No test (Pegasus's 0x137630).
    None,
    /// 0x133990: any non-monster, or Suijin, Sanga of the Thunder, Kazejin
    /// (Labyrinth-ruler's 0x1337B0).
    GateGuardian,
    /// 0x12EA70: any non-monster, or a monster 0x271540 types (1 or 2),
    /// i.e. every card with a record (Darkness-ruler's 0x12E890, which
    /// therefore never adds fodder).
    Recorded,
    /// Any non-monster, or one of the listed cards (Keith's 0x11FC00).
    NonMonsterOr(&'static [i32]),
    /// Any non-monster, or a monster of one of the listed kinds (card byte
    /// 0 & 0x1F; Ishtar's 0x1254E0).
    NonMonsterOrKinds(&'static [i32]),
    /// 0x13C0B0: any non-monster, or a monster with effect entries
    /// (0x271540 = 1; York's 0x13BED0).
    NonMonsterOrEffect,
    /// 0x170B80 (the generic module of Shadi and the VS controllers): as
    /// [`FodderFilter::NonMonsterOrEffect`], plus the Exodia pieces
    /// 0x36..=0x39 when the active side's (0x1DF7C0) leader is card 0x3A
    /// and its saved deck list holds all four ([`exodia_in_saved_deck`]).
    NonMonsterOrEffectOrExodia,
    /// Jasper's 0x157140 in his fodder pass 0x156FA0, which has neither the
    /// 2·ATK + DEF limit nor the normal-monster test (0x271540 = 2) of
    /// 0x10EF80 (give it `fodder_limit` `u32::MAX`): any non-monster, a
    /// monster whose 2·ATK + DEF (0x2714E0) is at least `strong`, or a card
    /// in `first..=last`.
    UntypedNonMonsterStrongOr { strong: u32, first: i32, last: i32 },
}

/// What Ishtar's monster choice 0x1250E0 does differently from the copies
/// [`monster_choice`] covers: a root whose best node is `skip_top` is passed
/// over without scoring, and a best node whose printed ATK is below
/// `plain_below_atk` is scored by 0x2714E0 (plain ATK / DEF) instead of
/// 0x27A850. [`ChoiceExtra::NONE`] (999, 0) is every other module.
#[derive(Clone, Copy, Debug)]
pub(super) struct ChoiceExtra {
    pub skip_top: i32,
    pub plain_below_atk: i32,
    /// Below this tier a root gets no fusion tree: Tea's 0x140120 builds
    /// it with 0x27A5C0 (one node, the card itself) instead of 0x27A3D0.
    /// `i16::MIN` (every other module): always the full tree.
    pub tree_from_tier: i16,
    /// Below this tier the chosen root gets no fodder pass (Tea's 0x140120
    /// calls 0x140360 only at tier ≥ 4); `i16::MIN`: always.
    pub fodder_from_tier: i16,
    /// Joey's 0x151F70 asks 0x271440 (affordable) before building a root's
    /// tree: an unaffordable root is skipped without a tree and does not
    /// raise the returned best score. False: every other module.
    pub affordable_first: bool,
    /// Jasper's 0x156DB0 compares only the low half (the DEF part,
    /// `andi 0xFFFF`) of a root's 0x27A850 score with the FULL score of the
    /// last chosen root, and returns the best low half. False: every other
    /// module.
    pub low_half_score: bool,
}

impl ChoiceExtra {
    pub const NONE: Self = Self {
        skip_top: 999,
        plain_below_atk: 0,
        tree_from_tier: i16::MIN,
        fodder_from_tier: i16::MIN,
        affordable_first: false,
        low_half_score: false,
    };
}

/// 0x10EBF0 / 0x10EF80.
pub(super) const WEEVIL_CHOICE: MonsterChoice = MonsterChoice {
    equips_at_any_tier: false,
    fodder_limit: 0xDAD,
    fodder_filter: FodderFilter::Cocoon,
    skip_root: 999,
};

/// 0x10EBF0(side, d, U): for every monster in the predicted hand (ascending
/// index) build its fusion tree, take the node with the highest ATK and
/// score it (0x27AA70; tier ≥ 3: 0x27A850 with power-ups). The first root
/// that is affordable and beats the best affordable score so far gets
/// fodder (0x10EF80), writes its path to d+6.. and becomes U (with the
/// power-up bonus in +0x38/+0x3A). Returns the best score over ALL roots,
/// affordable or not, or −1.
pub(super) fn monster_choice<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
    style: MonsterChoice,
) -> R<i32> {
    monster_choice_with(w, side, d, u, style, ChoiceExtra::NONE)
}

/// [`monster_choice`] with Ishtar's variations ([`ChoiceExtra`]).
pub(super) fn monster_choice_with<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
    style: MonsterChoice,
    extra: ChoiceExtra,
) -> R<i32> {
    let hand = Hand::of(w, side);
    let mut best_affordable = -1;
    let mut best = -1;
    for i in 0..w.next_hand_size(side) {
        let c = hand.get(i)?;
        if kind_of(w, c) >> 5 != 0 || c == style.skip_root {
            continue;
        }
        if extra.affordable_first && !affordable(w, side, c)? {
            continue;
        }
        // The pool counters are reset for every root.
        let mut tree = Tree::default();
        let root = if w.tier < extra.tree_from_tier {
            // 0x27A5C0: one node for the card (0x27A210 / 0x27A270).
            tree.single(i as u8, (c & 0xFFFF) as u16)
        } else {
            tree.build(w, side, &hand, i as u8, (c & 0xFFFF) as u16, None)?
        };
        let Some(top) = tree.best_atk(w, root) else {
            return unsupported("0x10EBF0 scores a null node");
        };
        let top_card = tree.nodes[top].card;
        // 0x1251B4: the node's card is read through its record pointer.
        if i32::from(top_card) == extra.skip_top {
            continue;
        }
        let score = if w.tier < 3 && !style.equips_at_any_tier {
            score_plain(w, top_card)
        } else if (atk_def(w, i32::from(top_card)) >> 16) < extra.plain_below_atk as u32 {
            // 0x1251C8: 0x2714E0's word as the score.
            score_plain(w, top_card)
        } else {
            score_equips(w, side, &hand, &mut tree, top)?
        };
        // Jasper's 0x156EA4: `andi $s4, $v0, 0xFFFF` is what is compared.
        let cmp = if extra.low_half_score {
            score & 0xFFFF
        } else {
            score
        };
        if affordable(w, side, c)? {
            if best_affordable < cmp {
                if w.tier >= extra.fodder_from_tier {
                    fodder(w, side, &mut tree, top, &hand, style)?;
                }
                d.hand.copy_from_slice(&tree.nodes[top].path[..5]);
                let card = i32::from(tree.nodes[top].card);
                let mut v = virt(w, card);
                // +0x38/+0x3A = score − printed stat, read through U +0
                // (the card record pointer, null without a record).
                let Some(info) = data(w, i32::from(v.card)) else {
                    return unsupported("0x10EBF0 reads a null card record");
                };
                v.atk_mod = ((score >> 16) - i32::from(info.attack & 0x1FFF)) as i16;
                v.def_mod = ((score & 0xFFFF) - i32::from(info.defense & 0x1FFF)) as i16;
                *u = Some(v);
                best_affordable = score;
                if best < cmp {
                    best = cmp;
                }
            } else if extra.low_half_score && best < cmp {
                // 0x156F2C: the best low half is kept on this path too.
                best = cmp;
            }
        } else if best < score {
            best = score;
        }
    }
    Ok(best)
}

/// The leader card whose saved deck list the generic module's Exodia tests
/// read (0x113250 / 0x16D800 / 0x170B80).
pub(super) const EXODIA_LEADER: i32 = 0x3A;

/// 0x16D800(side) (inlined in 0x16EDE0 and 0x170B80): 1 when `side`'s
/// leader is card 0x3A and its saved deck list (deck 0x215040(side) of its
/// card block) holds each of the pieces 0x36..=0x39 (0x214960 >= 0: a
/// plain u16 search of the 40 entries), else 0. The list is
/// [`crate::duel::Duel::saved_decks`]; unknown (an observed duel) is
/// refused.
pub(super) fn exodia_in_saved_deck<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> R<bool> {
    if w.card_id(side, 0) != EXODIA_LEADER {
        return Ok(false);
    }
    let Some(deck) = w.duel.saved_decks.get(side as usize).filter(|d| !d.is_empty()) else {
        return unsupported("the generic module reads an unknown saved deck list of leader 0x3A");
    };
    Ok((0x36..=0x39).all(|piece| deck.contains(&piece)))
}

/// 0x10FAF0(card): a monster that takes Cocoon of Evolution (0x24A880 ≥ 0),
/// or Hercules Beetle.
fn cocoon_able<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, card: i32) -> R<bool> {
    if kind_of(w, card) >> 5 != 0 {
        return Ok(false);
    }
    if equip(w, card, id::COCOON)? >= 0 {
        return Ok(true);
    }
    Ok(card == id::HERCULES)
}

/// 0x133990(card): a non-monster, or one of the Gate Guardian pieces
/// Suijin (0x23D), Sanga of the Thunder (0x216) and Kazejin (0x63).
fn gate_guardian_piece<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    card: i32,
) -> bool {
    kind_of(w, card) >> 5 != 0 || matches!(card, 0x23D | 0x216 | 0x63)
}

/// 0x10EF80(side, node, L): prepend to the node's path every unused hand
/// index (ascending) holding a card that is not 999, not 0x10FAF0, costs at
/// most the node card's cost (& 0xFF), has 2·ATK + DEF < 0xDAD, is a normal
/// monster (0x271540 = 2) and does not combine with the node's card.
/// Pegasus's 0x137630 is the same without the 0x10FAF0 test and with the
/// limit 0x3E9 (`style`).
fn fodder<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    tree: &mut Tree,
    best: usize,
    hand: &Hand,
    style: MonsterChoice,
) -> R<()> {
    let card = i32::from(tree.nodes[best].card);
    let limit = cost(w, card, side)? & 0xFF;
    let mut unused = [0xFFu8; 5];
    let mut count = 0;
    for j in 0..w.next_hand_size(side) {
        if !in_path(j as u8, &tree.nodes[best].path) {
            let Some(slot) = unused.get_mut(count) else {
                return unsupported("0x10EF80 index list overflow");
            };
            *slot = j as u8;
            count += 1;
        }
    }
    if count == 0 {
        return Ok(());
    }
    for &k in unused.iter().take(path_len(&unused)) {
        let c = hand.get(i32::from(k))?;
        let kept = match style.fodder_filter {
            FodderFilter::Cocoon => cocoon_able(w, c)?,
            FodderFilter::None => false,
            FodderFilter::GateGuardian => gate_guardian_piece(w, c),
            FodderFilter::Recorded => kind_of(w, c) >> 5 != 0 || monster_type(w, c) != 0,
            FodderFilter::NonMonsterOr(cards) => kind_of(w, c) >> 5 != 0 || cards.contains(&c),
            FodderFilter::NonMonsterOrKinds(kinds) => {
                let k = kind_of(w, c);
                k >> 5 != 0 || kinds.contains(&(k & 0x1F))
            }
            FodderFilter::NonMonsterOrEffect => kind_of(w, c) >> 5 != 0 || monster_type(w, c) == 1,
            FodderFilter::NonMonsterOrEffectOrExodia => {
                if kind_of(w, c) >> 5 != 0 || monster_type(w, c) == 1 {
                    true
                } else {
                    (0x36..=0x39).contains(&c) && exodia_in_saved_deck(w, i32::from(w.active))?
                }
            }
            FodderFilter::UntypedNonMonsterStrongOr {
                strong,
                first,
                last,
            } => {
                let v = atk_def(w, c);
                kind_of(w, c) >> 5 != 0
                    || (v & 0xFFFF) + ((v >> 16) << 1) >= strong
                    || (first..=last).contains(&c)
            }
        };
        if c == 999 || kept || limit < cost(w, c, side)? {
            continue;
        }
        let v = atk_def(w, c);
        if (v & 0xFFFF) + ((v >> 16) << 1) >= style.fodder_limit {
            continue;
        }
        let untyped = matches!(
            style.fodder_filter,
            FodderFilter::UntypedNonMonsterStrongOr { .. }
        );
        if !untyped && monster_type(w, c) != 2 {
            continue;
        }
        if combine(w, card, c & 0xFFFF)? & 0xFFFF != 999 {
            continue;
        }
        prepend(&mut tree.nodes[best].path, k);
    }
    Ok(())
}

// ---- Cocoon of Evolution -------------------------------------------------------

/// 0x10FB60(side, d, U, a3): with cell (0, 0) empty and the leader on
/// (col 1, row 0): d+6 = the LAST hand index holding Cocoon of Evolution,
/// d+7 = the FIRST monster index that takes it (0x24A880 ≥ 0) and is
/// affordable (0x271490); U = that monster; returns its class (0). −1
/// otherwise (d+6 may already be written).
fn cocoon<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
    a3: i32,
) -> R<i32> {
    let hand = Hand::of(w, side);
    if a3 & 0xFF != 0 {
        return Ok(-1);
    }
    if w.at(0, 0).is_some() || w.pos(side, 0) != 1 {
        return Ok(-1);
    }
    let n = w.next_hand_size(side);
    let mut found = false;
    for i in 0..n {
        if hand.get(i)? == id::COCOON {
            found = true;
            d.hand[0] = i as u8;
        }
    }
    if !found {
        return Ok(-1);
    }
    for j in 0..n {
        let c = hand.get(j)?;
        if kind_of(w, c) >> 5 != 0 {
            continue;
        }
        if equip(w, c, id::COCOON)? < 0 {
            continue;
        }
        if !affordable_12(w, side, c)? {
            continue;
        }
        d.hand[1] = j as u8;
        *u = Some(virt(w, c));
        return Ok(kind_of(w, c) >> 5);
    }
    Ok(-1)
}

/// 0x10EE10(side, d, U, a3): d+6.. = 0xFF, then 0x10FB60; at tier ≥ 4 also
/// append every other hand index holding a power-up (class 2, not
/// 0x31B/0x31C) that fits Larva of Moth (0x24A880(0x1AB, c) ≥ 0).
fn cocoon_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
    a3: i32,
) -> R<i32> {
    d.hand = [0xFF; 5];
    let hand = Hand::of(w, side);
    if a3 & 0xFF != 0 {
        return Ok(0);
    }
    cocoon(w, side, d, u, 0)?;
    if w.tier < 4 {
        return Ok(0);
    }
    let mut next = 2;
    for k in 0..w.next_hand_size(side) {
        if k == i32::from(d.hand[0]) || k == i32::from(d.hand[1]) {
            continue;
        }
        let c = hand.get(k)?;
        if equip(w, id::LARVA, c)? < 0 {
            continue;
        }
        if kind_of(w, c) >> 5 != 2 || c == id::POTION || c == id::CURSEBREAKER {
            continue;
        }
        let Some(slot) = d.hand.get_mut(next) else {
            return unsupported("0x10EE10 writes past d+0xA");
        };
        *slot = k as u8;
        next += 1;
    }
    Ok(0)
}

// ---- non-monsters and rituals ----------------------------------------------------

/// 0x10FD20(side, d, U): d+6.. = 0xFF; the first hand index (ascending) of,
/// in this order: a ritual (class 5) whose tributes pass 0x110210, a trap
/// (class 3/4), any class other than 0 and 2 (magic, or a ritual that
/// failed), a power-up (any non-monster). d+6 = index, U = the card,
/// returns 1; −1 when there is none.
fn non_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
    u: &mut Option<Unit>,
) -> R<i32> {
    let hand = Hand::of(w, side);
    d.hand = [0xFF; 5];
    let n = w.next_hand_size(side);
    for pass in 0..4 {
        for i in 0..n {
            let c = hand.get(i)?;
            let class = kind_of(w, c) >> 5;
            let hit = match pass {
                0 => class == 5 && ritual_ok(w, side, c)? == 1,
                1 => class == 3 || class == 4,
                2 => class != 0 && class != 2,
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

/// 0x110210(side, card): 1 when the ritual's tributes can be chosen
/// (0x2561B0 on the 0x2565B0 spec of its entry 0 word 1) and none of them
/// is Larva/Pupa of Moth, Great Moth, Perfectly Ultimate Great Moth or has
/// an effective ATK (0x252300 at its own cell) of 3000 or more; else 0.
pub(super) fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    ritual_ok_with(w, side, card, 0xBB8, true)
}

/// [`ritual_ok`] with the module's tribute limits: `atk_limit` is the
/// `slti` immediate (0xBB8 in 0x110210, 0xA28 in Pegasus's 0x139860 and
/// 0x139A50), `moths` whether the moth-line cards are refused (0x110210
/// only).
pub(super) fn ritual_ok_with<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
    atk_limit: i32,
    moths: bool,
) -> R<i32> {
    // 0x2715C0(card, 0): entry 0 of a virtual record (null for 999).
    if card == 999 {
        return unsupported("0x110210 reads entry 0 of card 999");
    }
    let word1 = virt(w, card).entries[0].map_or(u32::MAX, |e| e[1]);
    let spec = ritual_spec(word1);
    let m = tributes(w, &spec, side)?;
    if m < 0 {
        return Ok(0);
    }
    let mut i = 0;
    while i < popcount(m) {
        let k = nth_unit(m, i);
        let (g, s) = (k >> 8, k & 0xFF);
        let c = w.card_id(g, s);
        if moths && ((c.wrapping_sub(0x1AB) as u32) < 2 || (c.wrapping_sub(0x192) as u32) < 2) {
            return Ok(0);
        }
        let p = w.pos(g, s);
        if w.eff_atk(g, s, p & 0xFF, p >> 8)? >= atk_limit {
            return Ok(0);
        }
        i += 1;
    }
    Ok(1)
}

/// 0x2565B0(spec, id): the three tribute words of ritual `id` (immediates of
/// the original switch): (value << 16) | type, where type 0..0x12 is a
/// monster kind, 0x1C a card id, 0x25 / 0x39 stat filters of 0x24E360;
/// all −1 for other ids.
pub(super) fn ritual_spec(id: u32) -> [u32; 3] {
    match id {
        0x4E => [0x3C_001C, 0x5DC_0025, 0x5DC_0025],
        0x1D8 => [0x214_001C, 0xC, 0xC],
        0x80 => [2, 0xE, 9],
        0x4C => [0x29B_001C, 0xCC_001C, 0x4F_001C],
        0x94 => [0x92_001C, 3, 3],
        0x1A6 => [0x18E_001C, 9, 9],
        0x161 => [0xAD_001C, 0xE, 0xE],
        0x160 => [0x14A_001C, 0x3E8_0025, 0x3E8_0025],
        0x4D => [0x131_001C, 1, 1],
        0x254 => [0x248_001C, 0x10, 0x10],
        0xB => [0x8_001C, 3, 6],
        0xA => [0x5_001C, 0x125_001C, 0x6_001C],
        0x87 => [0x23_001C, 3, 3],
        0xDC => [0x279_001C, 4, 4],
        0x93 => [0xD2_001C, 0x150_001C, 0x295_001C],
        0xC3 => [0x236_001C, 6, 3],
        0x2 => [0x1C, 0x1C, 0x1C],
        0x152 => [0x149_001C, 0x121_001C, 7],
        0xF8 => [0xF9_001C, 5, 5],
        0x151 => [0x12C_001C, 7, 7],
        0x8F => [0x9B_001C, 0x5DC_0025, 0x5DC_0025],
        0x91 => [0x63_001C, 0x23D_001C, 0x216_001C],
        0xD => [4, 0, 0x11],
        0xBD => [0x7D0_0039, 0x7D0_0039, 0x7D0_0039],
        _ => [u32::MAX; 3],
    }
}

/// Unit masks of the tribute solver: 22 bits, bit 21 − slot for group 0 and
/// bit 10 − slot for group 1 (any other group counts as group 1).
fn mask_bit(g: i32, s: i32) -> u32 {
    if g == 0 {
        (21 - s) as u32
    } else {
        (10 - s) as u32
    }
}

/// 0x250230(mask): set bits among the low 22.
pub(super) fn popcount(mask: i32) -> i32 {
    (mask as u32 & 0x3F_FFFF).count_ones() as i32
}

/// 0x250570(mask, i): key of the `i`-th unit (group 0 slots 0..10, then
/// group 1), −1 past the end.
pub(super) fn nth_unit(mask: i32, i: i32) -> i32 {
    let mut seen = 0;
    for v in 0..22 {
        if (mask >> (21 - v)) & 1 != 1 {
            continue;
        }
        if seen == i {
            return if v < 11 { v } else { (v - 11) | 0x100 };
        }
        seen += 1;
    }
    -1
}

/// 0x250330(key, mask): the unit's bit.
fn has_unit(key: i32, mask: i32) -> bool {
    (mask as u32) >> (mask_bit(key >> 8, key & 0xFF) & 31) & 1 == 1
}

/// 0x24E0A0(mask, g, s): add a unit.
fn add_unit(mask: i32, g: i32, s: i32) -> i32 {
    (mask as u32 | 1 << (mask_bit(g, s) & 31)) as i32
}

/// 0x24E0F0(mask, g, s): remove a unit (rebuilt from 0x24E190 = 2^n − 1).
pub(super) fn remove_unit(mask: i32, g: i32, s: i32) -> i32 {
    (mask as u32 & !(1 << (mask_bit(g, s) & 31))) as i32
}

/// 0x250C10(g, s): 0x252300 at (col, col) (sic) + 0x252460 at (col, row).
fn strength<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, g: i32, s: i32) -> R<i32> {
    let p = w.pos(g, s);
    let col = p & 0xFF;
    Ok(w.eff_atk(g, s, col, col)? + w.eff_def(g, s, col, p >> 8)?)
}

/// 0x250B20(mask): the units with the lowest 0x250C10 (below 0xFFFF).
fn weakest<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, mask: i32) -> R<i32> {
    let mut best = 0xFFFF;
    let mut out = 0;
    for i in 0..popcount(mask) {
        let k = nth_unit(mask, i);
        let (g, s) = (k >> 8, k & 0xFF);
        let v = strength(w, g, s)?;
        if v < best {
            out = add_unit(0, g, s);
            best = v;
        } else if v == best {
            out = add_unit(out, g, s);
        }
    }
    Ok(out)
}

/// 0x2508D0(mask): the unit with the lowest summon order (below 0xFF,
/// first wins), −1 if none.
fn oldest<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, mask: i32) -> i32 {
    let mut best = 0xFF;
    let mut out = -1;
    for i in 0..popcount(mask) {
        let k = nth_unit(mask, i);
        let o = w.order(k >> 8, k & 0xFF);
        if o < best {
            out = k;
            best = o;
        }
    }
    out
}

/// 0x2561B0(spec, side): tries the six orders of the spec words (table
/// 0x2D3300), picks tributes with 0x256440 and returns the set whose total
/// 0x250C10 is lowest (≤ 0xFFFF; ties: the earlier order), or −1.
pub(super) fn tributes<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    spec: &[u32; 3],
    side: i32,
) -> R<i32> {
    const ORDERS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut masks = [-1i32; 6];
    let mut scores = [-1i32; 6];
    for (p, o) in ORDERS.iter().enumerate() {
        let words = [spec[o[0]], spec[o[1]], spec[o[2]]];
        let m = tribute_pick(w, &words, side)?;
        masks[p] = m;
        if m < 0 {
            continue;
        }
        scores[p] = 0;
        let n = popcount(m);
        if n != 3 {
            scores[p] = -1;
            continue;
        }
        for i in 0..n {
            let k = nth_unit(m, i);
            scores[p] += strength(w, k >> 8, k & 0xFF)?;
        }
    }
    let mut pick = 0xFF;
    let mut low = 0xFFFF;
    for a in (0..6).rev() {
        let s = scores[a];
        if s < 0 {
            continue;
        }
        if low >= s {
            low = s;
            pick = a;
        }
    }
    Ok(if pick == 0xFF { -1 } else { masks[pick] })
}

/// 0x256440(words, side): for each word in turn, the candidates of
/// 0x24E360 minus the units already taken, their weakest (0x250B20) and of
/// those the oldest (0x2508D0) is taken. The set when it holds exactly 3
/// units, else −1.
fn tribute_pick<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    words: &[u32; 3],
    side: i32,
) -> R<i32> {
    let mut taken = 0;
    for &word in words {
        let mut cand = unit_mask(w, 0, 0, side, 1, word, 0xE)?;
        if cand <= 0 {
            continue;
        }
        for j in 0..popcount(taken) {
            let k = nth_unit(taken, j);
            if has_unit(k, cand) {
                cand = remove_unit(cand, k >> 8, k & 0xFF);
            }
        }
        let v = weakest(w, cand)?;
        if v <= 0 {
            continue;
        }
        let k = oldest(w, v);
        if k < 0 {
            continue;
        }
        taken = add_unit(taken, k >> 8, k & 0xFF);
    }
    Ok(if popcount(taken) == 3 { taken } else { -1 })
}

/// 0x24E230(side, rel, buf): non-leader units (group `side` slots 1..10,
/// then the other group) whose controller has relation `rel` to `side`
/// (0x24F6A0: 1 same, 2 opposite, 3 any).
pub(super) fn unit_list<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    rel: i32,
) -> Vec<i32> {
    let related = |ctrl: i32| match rel {
        3 => true,
        2 => side == i32::from(ctrl == 0),
        1 => side == ctrl,
        _ => false,
    };
    let mut out = Vec::new();
    for g in [side, i32::from(side == 0)] {
        for s in 1..11 {
            if w.unit(g, s).is_some() && related(w.ctrl(g, s)) {
                out.push((g & 0xFF) << 8 | s);
            }
        }
    }
    out
}

/// 0x24E360(c, r, side, rel, word, mode) for the default modes (not
/// 0x12/0x14/0x16/0x17/0x18/0x1D): units of 0x24E230(side, rel) within
/// Manhattan distance `mode` of (c, r) that match the word's type (table
/// 0x2D3130): 0..0x12 a monster of that kind, 0x1C card id = value,
/// 0x1D any monster, 0x1E any non-monster, 0x1F/0x2D/0x34/0x3D any unit,
/// monsters with 0x252300 at their cell ≥ (0x24) / ≤ (0x25) value, with
/// 0x252460 ≥ (0x39) / ≤ (0x3A) value, 0x21 the single strongest monster
/// (handler 0x24F1C4: highest (ATK << 16 | DEF) at its own cell from 0,
/// ties to the lower summon order, an exact tie replaced only when the
/// held unit's group equals `rel`). Types without a handler (and ≥ 0x3E)
/// match nothing; the remaining handlers (never in a ritual spec) are
/// refused.
pub(super) fn unit_mask<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    c: i32,
    r: i32,
    side: i32,
    rel: i32,
    word: u32,
    mode: i32,
) -> R<i32> {
    let ty = word & 0xFFFF;
    // 0x24E44C..0x24EAD0: the terrain modes (0x14 → terrain 0, 0x16 → 2,
    // 0x17 → 3, 0x18 → 4, 0x1D → 9: units standing on it, whatever their
    // distance; types 0x34 / 0x1F any, 0x1D monsters whose controller
    // relates to `side`) and the cross mode 0x12 (same column or row as
    // (c, r); types 0x1F any, 0x1E non-monsters / 0x1D monsters of the
    // relation). Other types match nothing in these modes.
    let terrain_of = |m: i32| match m {
        0x14 => Some(0),
        0x16 => Some(2),
        0x17 => Some(3),
        0x18 => Some(4),
        0x1D => Some(9),
        _ => None,
    };
    if terrain_of(mode).is_some() || mode == 0x12 {
        let mut out = 0;
        for k in unit_list(w, side, rel) {
            let (g, s) = (k >> 8, k & 0xFF);
            let p = w.pos(g, s);
            let here = match terrain_of(mode) {
                Some(t) => w.terrain(p & 0xFF, p >> 8) == t,
                None => p & 0xFF == c || p >> 8 == r,
            };
            if !here {
                continue;
            }
            let monster = w.kind(g, s) >> 5 == 0;
            let related = |g: i32, s: i32| {
                let ctrl = w.ctrl(g, s);
                match rel {
                    3 => true,
                    2 => side == i32::from(ctrl == 0),
                    1 => side == ctrl,
                    _ => false,
                }
            };
            let hit = match (mode, ty) {
                (0x12, 0x1F) => true,
                (0x12, 0x1E) => !monster && related(g, s),
                (0x12, 0x1D) | (_, 0x1D) => monster && related(g, s),
                (0x12, _) => false,
                (_, 0x34 | 0x1F) => true,
                _ => false,
            };
            if hit {
                out = add_unit(out, g, s);
            }
        }
        return Ok(out);
    }
    let list = unit_list(w, side, rel);
    let value = (word as i32) >> 16;
    let mut out = 0;
    // 0x24F1C4's state: sp+0xAC best score (0), s2 order (9999), s7 group (−1).
    let (mut best, mut best_order, mut best_group) = (0u32, 9999, -1);
    for k in list {
        let (g, s) = (k >> 8, k & 0xFF);
        let p = w.pos(g, s);
        let (col, row) = (p & 0xFF, p >> 8);
        let dist = (col.max(c) - col.min(c)) + (row.max(r) - row.min(r));
        if mode < dist {
            continue;
        }
        let kind = w.kind(g, s);
        let monster = kind >> 5 == 0;
        if ty == 0x21 {
            if !monster {
                continue;
            }
            let v = (w.eff_atk(g, s, col, row)? as u32) << 16
                | (w.eff_def(g, s, col, row)? as u32) & 0xFFFF;
            let replace = if best >> 16 != v >> 16 {
                best >> 16 < v >> 16
            } else if best & 0xFFFF != v & 0xFFFF {
                best & 0xFFFF < v & 0xFFFF
            } else {
                let o = w.order(g, s);
                if best_order != o {
                    o < best_order
                } else {
                    best_group == rel
                }
            };
            if replace {
                best_order = w.order(g, s);
                best = v;
                out = add_unit(0, g, s);
                best_group = g;
            }
            continue;
        }
        // The stat filters compare with value & 0xFFFF (sp+0xD0).
        let limit = value & 0xFFFF;
        let hit = match ty {
            0..=0x12 => monster && kind & 0x1F == ty as i32,
            0x1C => w.card_id(g, s) == value,
            0x1D => monster,
            0x1E => !monster,
            0x1F | 0x2D | 0x34 | 0x3D => true,
            // 0x24EFA4..0x24F0CC: monsters of attribute 0..=5 (+0x3F,
            // 0x1E1AB0); Ishtar's 0x126C50 reads the type from a magic
            // card's entry (0x2E6: type 0x18).
            0x14..=0x19 => {
                monster
                    && w.unit(g, s)
                        .is_some_and(|u| u32::from(u.attribute) == ty - 0x14)
            }
            0x24 => monster && w.eff_atk(g, s, col, row)? >= limit,
            0x25 => monster && w.eff_atk(g, s, col, row)? <= limit,
            // 0x24F5A0: a kind-0 monster with 0x252300 ≤ value.
            0x37 => monster && kind & 0x1F == 0 && w.eff_atk(g, s, col, row)? <= limit,
            0x39 => monster && w.eff_def(g, s, col, row)? >= limit,
            0x3A => monster && w.eff_def(g, s, col, row)? <= limit,
            0x1A | 0x1B | 0x23 | 0x26..=0x2C | 0x2E..=0x33 | 0x38 => false,
            0x3E.. => false,
            _ => return unsupported(format!("0x24E360 type {ty:#x}")),
        };
        if hit {
            out = add_unit(out, g, s);
        }
    }
    Ok(out)
}

// ---- formation counts ------------------------------------------------------------

/// 0x110120(kind, i): Weevil's formation cells (row << 8) | col (jump
/// table 0x2BA310 for kind 1).
fn formation_cell(kind: i32, i: i32) -> i32 {
    const KIND1: [i32; 7] = [0x203, 0x302, 0x301, 0x300, 0x104, 0x203, 0x004];
    let i = i & 0xFF;
    if kind & 0xFF == 1 {
        return KIND1.get(i as usize).copied().unwrap_or(0x203);
    }
    match i {
        3 => 0x002,
        2 => 0x200,
        1 => 0x201,
        _ => 0x102,
    }
}

/// 0x10FFC0(kind): own (controller = the active side, 0x1DF7C0) non-leader
/// units on the formation cells: kind 1 over the first six cells (0x203
/// counted twice), 1 when at least 2; kind 2 over the four trap cells, 1
/// when at least 1.
fn formation_count<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, kind: i32) -> i32 {
    let (cells, need) = if kind & 0xFF == 1 { (6, 2) } else { (4, 1) };
    let mut count = 0;
    for i in 0..cells {
        let cell = formation_cell(kind, i) & 0xFFFF;
        let k = w.at_key(cell & 0xFF, cell >> 8);
        if k < 0 {
            continue;
        }
        if w.ctrl(k >> 8, k & 0xFF) == i32::from(w.active) && k & 0xFF != 0 {
            count += 1;
        }
    }
    i32::from(count >= need)
}

// ---- summon cell ---------------------------------------------------------------------

/// Neighbour offsets (dcol, drow) of 0x24FD30 around the leader, as its
/// jump tables 0x2D32A0 (side 0) and 0x2D3280 (side 1) compute them. The
/// original bounds-checks only the +1 offsets; a −1 off the board yields a
/// negative value, which is what the callers test.
const NEIGHBOURS: [[(i32, i32); 8]; 2] = [
    [
        (0, -1),
        (0, 1),
        (1, 0),
        (-1, 0),
        (1, -1),
        (-1, -1),
        (1, 1),
        (-1, 1),
    ],
    [
        (0, 1),
        (0, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (1, 1),
        (-1, -1),
        (1, -1),
    ],
];

/// 0x24FD30(side, i) (= 0x10F660): neighbour `i` of the side's leader as
/// (row << 8) | col, negative off the board or for other sides / i ≥ 8.
pub(super) fn neighbour<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    i: i32,
) -> R<i32> {
    let p = w.pos(side, 0);
    if !(0..2).contains(&side) || !(0..8).contains(&i) {
        return Ok(-1);
    }
    if p < 0 {
        return unsupported("0x24FD30 without a leader");
    }
    let (dc, dr) = NEIGHBOURS[side as usize][i as usize];
    let (c, r) = ((p & 0xFF) + dc, (p >> 8) + dr);
    Ok(if (0..7).contains(&c) && (0..7).contains(&r) {
        r << 8 | c
    } else {
        -1
    })
}

/// 0x10F160(side, _, U, tgt): the summon cell among the leader's
/// neighbours (0x24FD30, empty, terrain not 8), or −1.
/// - Monsters and traps (class 0/3/4) with a target: Crush (9) only when
///   U would be crushed there; the fewest moves to the target (0x27BA70
///   when 0x279E50 ≥ 0, else 0x27AC00, both at most 2) among cells whose
///   step buffer has a candidate (0x27C900 > 0); first wins ties.
/// - Without a target: for each formation cell in turn (0x110120 kind 1
///   for monsters, kind 2 for traps) the cell (terrain 9 excluded) with the
///   fewest moves (0x27AC00, at most 7), strict < from 14; the first
///   formation cell with a route wins.
/// - Other classes: the LAST neighbour that is empty and not terrain 8/9.
fn cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &Unit,
    tgt: i32,
) -> R<i32> {
    let mut buf = Steps::new();
    let class = i32::from(u.kind >> 5);
    if class != 0 && class != 3 && class != 4 {
        let mut last = 0xFFFF;
        for i in 0..8 {
            let cell = neighbour(w, side, i)?;
            if cell < 0 {
                continue;
            }
            let (col, row) = (cell & 0xFF, cell >> 8);
            let t = w.terrain(col, row);
            if t == 8 || t == 9 || w.at_key(col, row) >= 0 {
                continue;
            }
            last = cell;
        }
        return Ok(if last == 0xFFFF { -1 } else { last });
    }
    if tgt >= 0 {
        let mut best_d = 0xFF;
        let mut best = 0xFFFF;
        let tp = w.pos(tgt >> 8, tgt & 0xFF);
        let (tc, tr) = (tp & 0xFF, tp >> 8);
        // 0x10F670 (the hidden-strength argument) is always 0.
        let m = w.margin_seen(u, tgt, 0, tc, tr, false)?;
        for i in 0..8 {
            let cell = neighbour(w, side, i)?;
            if cell < 0 {
                continue;
            }
            let (col, row) = (cell & 0xFF, cell >> 8);
            let t = w.terrain(col, row);
            if t == 8 || w.at(col, row).is_some() {
                continue;
            }
            if t == 9 && w.crushed(u, col, row)? {
                continue;
            }
            buf = Steps::new();
            let d = if m >= 0 {
                attack_path(w, Some(u), &mut buf, col, row, tc, tr, tgt as u32, 0, 2, 0)?
            } else {
                path(w, Some(u), &mut buf, col, row, tc, tr, tgt & 0xFFFF, 2)?
            };
            if step_count(&buf) <= 0 {
                continue;
            }
            if d < best_d && d >= 0 {
                best_d = d;
                best = cell;
            }
        }
        return Ok(if best == 0xFFFF { -1 } else { best });
    }
    let (kind, goals) = if class != 0 { (2, 4) } else { (1, 6) };
    for g in 0..goals {
        let goal = formation_cell(kind, g) & 0xFFFF;
        let (gc, gr) = (goal & 0xFF, goal >> 8);
        let mut best = 0xFFFF;
        let mut best_d = 14;
        for i in 0..8 {
            let cell = neighbour(w, side, i)?;
            if cell < 0 {
                continue;
            }
            let (col, row) = (cell & 0xFF, cell >> 8);
            let t = w.terrain(col, row);
            if t == 8 || t == 9 || w.at_key(col, row) >= 0 {
                continue;
            }
            // The step buffer is not reset between these searches.
            let d = path(w, Some(u), &mut buf, col, row, gc, gr, 0xFFFF, 7)?;
            if d < best_d && d >= 0 {
                best = cell;
                best_d = d;
            }
        }
        if best_d < 14 {
            return Ok(best);
        }
    }
    Ok(-1)
}

// ---- category and entry point ------------------------------------------------------

/// The first affordable monster of the predicted hand (the loops of
/// 0x10E890), as a hand index.
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

/// 0x10E890(side, U): what to summon: 0 a monster, 1 a non-monster, 2
/// Cocoon of Evolution, −1 nothing. 0x10FD20/0x10FB60 write a local
/// action record here.
fn category<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    u: &mut Option<Unit>,
) -> R<i32> {
    let monsters = w.monsters(side) & 0xFF;
    let others = w.others(side) & 0xFF;
    let hand = Hand::of(w, side);
    let mut local = Decision::new();
    // 1. Monster zone full.
    if monsters == 5 {
        return Ok(if non_monster(w, side, &mut local, u)? < 0 {
            -1
        } else {
            1
        });
    }
    // 2. Magic/trap zone full.
    if others == 5 {
        if let Some(c) = affordable_monster(w, side, &hand)? {
            *u = Some(virt(w, c));
            return Ok(0);
        }
        return Ok(-1);
    }
    // 3. Cocoon of Evolution.
    if cocoon(w, side, &mut local, u, 0)? >= 0 {
        return Ok(2);
    }
    // 4. No monster on the field: a monster (U left to 0x10EBF0), else
    //    0x10FD20's result as is.
    if monsters == 0 {
        if affordable_monster(w, side, &hand)?.is_some() {
            return Ok(0);
        }
        return non_monster(w, side, &mut local, u);
    }
    // 5. Fewer than two units on the monster formation cells.
    if formation_count(w, 1) == 0 && affordable_monster(w, side, &hand)?.is_some() {
        return Ok(0);
    }
    // 6. No unit on the trap formation cells.
    if formation_count(w, 2) == 0 {
        let v = non_monster(w, side, &mut local, u)?;
        if v >= 0 {
            return Ok(v);
        }
    }
    // 7. A monster, else 8. a non-monster.
    if let Some(c) = affordable_monster(w, side, &hand)? {
        *u = Some(virt(w, c));
        return Ok(0);
    }
    let v = non_monster(w, side, &mut local, u)?;
    Ok(if v < 0 { -1 } else { v })
}

/// 0x10E620(key, side, d): fill `d` with a summon (type 0xA) or return −1.
/// d+0 = key always; d+2 = 0xA and d+0xC = 0 once a category is found; the
/// hand indices go to d+6.., the cell to d+4. `d` may be partly written on
/// −1.
pub(super) fn summon<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    key: i32,
    side: i32,
    d: &mut Decision,
) -> R<i32> {
    d.key = key as u16;
    if !w.can_summon(side) {
        return Ok(-1);
    }
    if w.monsters(side) == 5 && w.others(side) == 5 {
        return Ok(-1);
    }
    // U: the virtual unit record at sp+0x70 (uninitialised until written).
    let mut u = None;
    let r = category(w, side, &mut u)?;
    if r == -1 {
        return Ok(-1);
    }
    d.kind = 0xA;
    let pos = w.pos(key >> 8, key & 0xFF);
    d.reveal = 0;
    // 0x278850(U, col, row, other side, 3, 0x1F, 2, 2, 0, 0x10F670 = 0).
    let target = |u: &Option<Unit>| {
        find_target(
            w,
            u.as_ref(),
            pos & 0xFF,
            pos >> 8,
            i32::from(side == 0),
            3,
            0x1F,
            2,
            2,
            0,
            0,
        )
    };
    let with_u = |u: &Option<Unit>| match u {
        Some(u) => Ok(u.clone()),
        None => unsupported("0x10E620 reads an uninitialised U"),
    };
    let tgt;
    if r == 0 {
        if monster_choice(w, side, d, &mut u, WEEVIL_CHOICE)? < 0 {
            // Unreachable (0x10E890 found an affordable monster): the
            // original falls into the non-monster path with an
            // uninitialised target register.
            if non_monster(w, side, d, &mut u)? < 0 {
                return Ok(-1);
            }
            return unsupported("0x10E620 uses an uninitialised target");
        }
        tgt = target(&u)?;
        // The test 0x1E13D0 < 6 before the cell is always true; otherwise
        // the non-monster path would run.
        if tgt < 0 && w.monsters(side) >= 6 && non_monster(w, side, d, &mut u)? < 0 {
            return Ok(-1);
        }
    } else {
        if r == 2 {
            // The target search runs here too but its result is unused
            // (and it has no side effects).
            cocoon_plan(w, side, d, &mut u, 0)?;
            d.target = 0;
            return Ok(1);
        }
        tgt = target(&u)?;
        if non_monster(w, side, d, &mut u)? < 0 {
            return Ok(-1);
        }
    }
    let c = cell(w, side, &with_u(&u)?, tgt)?;
    if c < 0 {
        return Ok(-1);
    }
    d.target = c as u16;
    Ok(1)
}

#[cfg(test)]
mod probes {
    //! Function-level oracle probes (private): set `ROSES_WEEVIL_PROBES`
    //! (any value) and `ROSES_WEEVIL_ORACLE` to the port directory holding
    //! `probes/c/*.json` (written by `probes/c/gen.py` from the emulator).
    use super::super::tests::{load_json, Case, Oracle};
    use super::*;
    use crate::duel::DuelError;
    use serde_json::Value;
    use std::collections::BTreeMap;

    /// The action record as the EE lays it out (bytes the probes compare).
    fn d_bytes(d: &Decision) -> Vec<(usize, u8)> {
        let mut out = vec![
            (0, d.key as u8),
            (1, (d.key >> 8) as u8),
            (2, d.kind),
            (4, d.target as u8),
            (5, (d.target >> 8) as u8),
        ];
        for (i, &b) in d.hand.iter().enumerate() {
            out.push((6 + i, b));
        }
        for (i, b) in d.reveal.to_le_bytes().iter().enumerate() {
            out.push((0xC + i, *b));
        }
        out.push((0x10, d.position));
        out
    }

    fn d_diff(d: &Decision, want: &Value) -> Option<String> {
        let want: Vec<u8> = serde_json::from_value(want.clone()).unwrap();
        let bad: Vec<String> = d_bytes(d)
            .into_iter()
            .filter(|&(i, b)| want[i] != b)
            .map(|(i, b)| format!("+{i:#x} {b:#x} vs {:#x}", want[i]))
            .collect();
        (!bad.is_empty()).then(|| bad.join(", "))
    }

    fn u_diff(u: &Option<Unit>, want: &Value) -> Option<String> {
        let mine = u.as_ref().map_or(Value::Null, |u| {
            serde_json::json!({"card": u.card, "kind": u.kind, "atk": u.atk_mod, "def": u.def_mod})
        });
        (mine != *want).then(|| format!("U {mine} vs {want}"))
    }

    #[derive(Default)]
    struct Tally {
        ok: usize,
        bad: Vec<String>,
        refused: Vec<String>,
    }

    impl Tally {
        fn add(&mut self, case: &str, r: R<Option<String>>) {
            match r {
                Ok(None) => self.ok += 1,
                Ok(Some(e)) => self.bad.push(format!("{case}: {e}")),
                Err(DuelError::Unsupported(m)) => self.refused.push(format!("{case}: {m}")),
                Err(e) => self.bad.push(format!("{case}: error {e}")),
            }
        }
    }

    fn ret_diff(mine: i32, want: &Value) -> Option<String> {
        let want = want.as_i64().unwrap() as i32;
        (mine != want).then(|| format!("ret {mine} vs {want}"))
    }

    fn join(parts: Vec<Option<String>>) -> Option<String> {
        let v: Vec<String> = parts.into_iter().flatten().collect();
        (!v.is_empty()).then(|| v.join("; "))
    }

    #[test]
    fn private_summon_probes_match_the_original() {
        if std::env::var_os("ROSES_WEEVIL_PROBES").is_none() {
            return;
        }
        let Some(o) = Oracle::load() else {
            return;
        };
        let mut t: BTreeMap<&str, Tally> = BTreeMap::new();
        for path in o.files("probes/c") {
            let p: Value = load_json(&path);
            let Some(calls) = p.get("calls") else {
                continue;
            };
            let name = p["case"].as_str().unwrap().to_string();
            // Synthetic cases live next to the probes.
            let own = o.root.join("probes/c/cases").join(format!("{name}.json"));
            let shared = o.root.join("cases").join(format!("{name}.json"));
            let case: Case = if own.exists() {
                load_json(&own)
            } else if shared.exists() {
                load_json(&shared)
            } else {
                continue;
            };
            let duel = o.duel(&case.duel);
            let side = i32::from(case.side);
            let w = match World::new(&duel, &o.cards, &o.tables, case.tier) {
                Ok(w) => w,
                Err(e) => {
                    t.entry("world").or_default().add(&name, Err(e));
                    continue;
                }
            };
            // 0x10E620.
            let c = &calls["summon"];
            let mut d = Decision::new();
            let r = summon(&w, side << 8, side, &mut d)
                .map(|r| join(vec![ret_diff(r, &c["ret"]), d_diff(&d, &c["d"])]));
            t.entry("summon").or_default().add(&name, r);
            // 0x10E890.
            let c = &calls["category"];
            let mut u = None;
            let r = category(&w, side, &mut u).map(|r| {
                join(vec![
                    ret_diff(r, &c["ret"]),
                    if r >= 0 { u_diff(&u, &c["u"]) } else { None },
                ])
            });
            t.entry("category").or_default().add(&name, r);
            // 0x10EBF0 / 0x10FD20, then the cell 0x10F160 with the
            // original's target.
            for (key, a1) in [("monster", 0), ("nonmon", 1)] {
                let c = &calls[key];
                let mut d = Decision::new();
                let mut u = None;
                let r = if a1 == 0 {
                    monster_choice(&w, side, &mut d, &mut u, WEEVIL_CHOICE)
                } else {
                    non_monster(&w, side, &mut d, &mut u)
                };
                let same_u = r.is_ok() && u_diff(&u, &c["u"]).is_none();
                let r = r.map(|r| {
                    join(vec![
                        ret_diff(r, &c["ret"]),
                        d_diff(&d, &c["d"]),
                        u_diff(&u, &c["u"]),
                    ])
                });
                t.entry(key).or_default().add(&name, r);
                let (Some(tgt), Some(want), true, Some(u)) =
                    (c["tgt"].as_i64(), c["cell"].as_i64(), same_u, u.as_ref())
                else {
                    continue;
                };
                let r = cell(&w, side, u, tgt as i32).map(|r| ret_diff(r, &Value::from(want)));
                t.entry("cell").or_default().add(&name, r);
                let lp = w.pos(side, 0);
                let r = find_target(
                    &w,
                    Some(u),
                    lp & 0xFF,
                    lp >> 8,
                    i32::from(side == 0),
                    3,
                    0x1F,
                    2,
                    2,
                    0,
                    0,
                )
                .map(|r| ret_diff(r, &Value::from(tgt)));
                t.entry("target (search.rs)").or_default().add(&name, r);
            }
            for (key, plan) in [("cocoon", false), ("cocoonplan", true)] {
                let c = &calls[key];
                let mut d = Decision::new();
                let mut u = None;
                let r = if plan {
                    cocoon_plan(&w, side, &mut d, &mut u, 0)
                } else {
                    cocoon(&w, side, &mut d, &mut u, 0)
                };
                let r = r.map(|r| {
                    join(vec![
                        ret_diff(r, &c["ret"]),
                        d_diff(&d, &c["d"]),
                        u_diff(&u, &c["u"]),
                    ])
                });
                t.entry(key).or_default().add(&name, r);
            }
            for (key, kind) in [("f1", 1), ("f2", 2)] {
                let r = Ok(ret_diff(formation_count(&w, kind), &calls[key]));
                t.entry(key).or_default().add(&name, r);
            }
            for e in calls["ritual"].as_array().unwrap() {
                let card = e[0].as_i64().unwrap() as i32;
                let r = ritual_ok(&w, side, card).map(|r| ret_diff(r, &e[1]));
                t.entry("ritual").or_default().add(&name, r);
            }
        }
        let mut bad = 0;
        for (k, v) in &t {
            eprintln!(
                "summon probes {k}: {} match, {} differ, {} refused",
                v.ok,
                v.bad.len(),
                v.refused.len()
            );
            if !k.contains("search.rs") {
                bad += v.bad.len();
            }
        }
        for (k, v) in &t {
            for b in v.bad.iter().take(40) {
                eprintln!("DIFF {k} {b}");
            }
            let mut reasons: BTreeMap<&str, usize> = BTreeMap::new();
            for r in &v.refused {
                *reasons
                    .entry(r.split(": ").nth(1).unwrap_or(r))
                    .or_default() += 1;
            }
            for (r, n) in reasons {
                eprintln!("REFUSED {k} x{n}: {r}");
            }
        }
        assert_eq!(bad, 0, "summon probes differ");
    }
}
