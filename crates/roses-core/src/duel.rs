//! Duel rules traced from the PAL executable and checked against saved
//! original states (private evidence under `work/research/duel/`).
//!
//! Scope: summoning monsters, setting magic/power-up/trap/ritual cards
//! face-down, movement, monster/leader battles, terrain and attribute rules,
//! summoning power, turns, spellbinding, graveyards, the 100-round limit,
//! surrender, life-point, trapped-leader and Exodia results, the campaign deck
//! shuffle, Block Attack and Negate Attack, limited-range traps, magic
//! activation (field change, LP gain, stat boosts), equipping power-ups by
//! moving onto them, and attacks on set cards. Anything the research has not
//! verified is refused with [`DuelError::Unsupported`] instead of being
//! guessed: monster card effects (all five effect entries), other trap types
//! and trap hooks, rituals, transforming power-ups, moving face-down set cards,
//! fusions, summoning onto own units, leader abilities other than #2, #4, #6,
//! #8, #9, #10, #12 and #14, control changes, the low-LP special draw and the
//! summon-onto-full-field "dust" result. The families traced in
//! `claude-effects/next/` (direct LP damage, Magic Jammer, battle-damage
//! shields, the entry-4 destruction effects of F04/F05, the flip boost by card
//! id) are implemented with their notes' section numbers in the comments.
//!
//! Plain Rust: no engine, clock or I/O. Every transition is a pure function
//! of the state and one command, so a server can run the same rules.

use crate::{GridPos, UnitId, BOARD_SIDE, CARD_COUNT};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub const DUEL_SCHEMA: &str = "roses-duel-1";
pub const START_LP: u16 = 4000;
pub const START_STARS: u8 = 4;
/// Side +0x342, initialised to 3 (0x1AEA28) and added each turn from round 2.
pub const STAR_INCOME: u8 = 3;
pub const MAX_STARS: u8 = 12;
pub const MAX_LP: u16 = 9999;
pub const HAND_SIZE: usize = 5;
pub const DECK_SIZE: usize = 40;
pub const ROUND_LIMIT: u8 = 100;
pub const MONSTER_SLOTS: usize = 5;
/// Unit slots 6..=10 hold set magic, power-up, trap and ritual cards.
pub const OTHER_SLOTS: std::ops::RangeInclusive<u8> = 6..=10;
/// Card byte 0 >> 5 of each card class.
pub mod class {
    pub const MONSTER: u8 = 0;
    pub const MAGIC: u8 = 1;
    pub const POWER_UP: u8 = 2;
    /// Limited-range trap (fires next to the trap, 0x24CC30).
    pub const NEAR_TRAP: u8 = 3;
    /// Full-range trap (fires anywhere, 0x26C5C0).
    pub const FAR_TRAP: u8 = 4;
    pub const RITUAL: u8 = 5;
}

/// Full-range trap effect types (entry 0 word0 >> 8).
pub mod trap_type {
    pub const MIRROR_FORCE: u32 = 0x04;
    /// Shadow of Eyes: a unit confirming in defence is put in attack
    /// position, the trap stays (event 0x47 rec[0xC] = 0,
    /// TRAPS-0x23-0x31-FAKE-TRAP-FIELD.md §1).
    pub const SHADOW_OF_EYES: u32 = 0x23;
    pub const BLOCK_ATTACK: u32 = 0x2C;
    pub const WHITE_HOLE: u32 = 0x2D;
    pub const ROYAL_DECREE: u32 = 0x2E;
    /// Anti-Raigeki: read only by Raigeki's event 5 (0x2593C0), which it
    /// cancels; it stays face-up (E2-536-TRAP-0x4.md §3).
    pub const ANTI_RAIGEKI: u32 = 0x50;
    pub const NEGATE_ATTACK: u32 = 0x30;
    /// Mirror Wall: halves the attacker's ATK, the trap stays and the battle
    /// goes on (event 0x36 type 0x31, TRAPS-0x23-0x31-FAKE-TRAP-FIELD.md §2).
    pub const MIRROR_WALL: u32 = 0x31;
    /// Fake Trap: read only by event 7 (Harpie's Feather Duster), which it
    /// cancels by being destroyed instead (TRAPS-0x23-0x31-FAKE-TRAP-FIELD.md
    /// §3).
    pub const FAKE_TRAP: u32 = 0x4B;
    pub const GORGONS_EYE: u32 = 0x4A;
    pub const MAGIC_JAMMER: u32 = 0x0E;
    /// Goblin Fan: reflects direct LP damage (event 9, F02.md); its equip
    /// hooks (0x26B9D0, 0x272560) are refused.
    pub const GOBLIN_FAN: u32 = 0x29;
    /// Bad Reaction to Simochi: turns LP gains into losses (event 8, F02.md
    /// section 5).
    pub const SIMOCHI: u32 = 0x2A;
    /// Types whose hook this engine checks (fired or explicitly refused).
    /// 0x4C/0x11 (Call of the Grave, Seal of the Ancients) and 0x2B
    /// (power-ups) are refused where their hook would run.
    pub const HOOKED: [u32; 16] = [
        MIRROR_FORCE,
        SHADOW_OF_EYES,
        BLOCK_ATTACK,
        WHITE_HOLE,
        ANTI_RAIGEKI,
        ROYAL_DECREE,
        NEGATE_ATTACK,
        MIRROR_WALL,
        GORGONS_EYE,
        MAGIC_JAMMER,
        0x4C,
        0x11,
        GOBLIN_FAN,
        SIMOCHI,
        0x2B,
        FAKE_TRAP,
    ];
}

/// Limited-range trap thresholds by card byte 1 (6..=10): the mover's
/// effective ATK must not exceed this for the trap to destroy it.
pub const NEAR_TRAP_ATTACK_LIMITS: [u16; 5] = [1000, 1500, 2000, 3000, 4000];
pub const TERRAIN_FAVOUR: i32 = 500;
pub const CRUSH_ATTACK: u16 = 1500;
pub const EXODIA_HEAD: u16 = 0x3A;
pub const EXODIA_LIMBS: [u16; 4] = [0x36, 0x37, 0x38, 0x39];
/// "Summoned Lord Exodia": rejected by the deck insert routine 0x1DFCB0.
pub const REJECTED_DECK_CARD: u16 = 0x29F;
/// Harpie's Feather Duster: the id 0x254FBC binds to event 7.
pub const FEATHER_DUSTER: u16 = 0x2BE;
/// Raigeki: magic type 4 / hi 0x12 bound to event 5 by id (0x254FC8).
pub const RAIGEKI: u16 = 0x2BC;
/// Magic type 4 / hi 0 ids that reach the default branch 0x255408 of
/// builder 0x2550A0 (event 0x15 straight away): Warrior Elimination,
/// Eradicating Aerosol, Breath of Light, Eternal Draught, and 726 Fissure
/// (filter 0x20: the weakest monster, 0x24F324; MORE-RULES-305-128-745-726.md).
/// Magic type 4 / hi 0 ids whose builder record (0x2550A0) goes to a plain
/// mask-destruction handler: the default branch {0x15} (720, 723..726) and
/// the id branches 701 Heavy Storm {6} (0x259DB0), 721 Eternal Rest {0x10},
/// 722 Stain Storm {0x11}, 727 Last Day of Witch {0x17}, 728 Exile of the
/// Wicked {0x12}, 729 Dust Tornado {0x13} (0x25C680 / 0x25C970 / 0x25D9D0 /
/// 0x25CC50 / 0x25CF30): each removes the mask through 0x250C90 (via
/// 0x260F90 or directly), mask order, with no trap search; they differ only
/// in their sub-scripts [F; emu REFUSALS-SWEEP.md ER1, SS1, LW1, LW0, EW1,
/// DT1, HS1]; 730 Cold Wave {0x14} (0x25D220) picks one face-up monster of
/// the mask with 0x2504F0 for its animation only and removes the whole mask
/// through 0x260F90 [F; emu CW1 with the IOP RPC 0x17D760 stubbed]. 699
/// Dark Hole (event 4, countertraps) and 731 are handled apart.
pub const ELIMINATIONS: [u16; 12] = [701, 720, 721, 722, 723, 724, 725, 726, 727, 728, 729, 730];
pub const LEADER_CELLS: [GridPos; 2] =
    [GridPos { column: 3, row: 6 }, GridPos { column: 3, row: 0 }];

pub mod terrain {
    pub const FOREST: u8 = 0;
    pub const WASTELAND: u8 = 1;
    pub const MOUNTAIN: u8 = 2;
    pub const MEADOW: u8 = 3;
    pub const SEA: u8 = 4;
    pub const DARK: u8 = 5;
    pub const TOON: u8 = 6;
    pub const NORMAL: u8 = 7;
    pub const LABYRINTH: u8 = 8;
    pub const CRUSH: u8 = 9;
}

/// Monster kind codes (card byte 0 & 0x1F) used by the terrain table.
pub mod kind {
    pub const DRAGON: u8 = 0;
    pub const SPELLCASTER: u8 = 1;
    pub const ZOMBIE: u8 = 2;
    pub const WARRIOR: u8 = 3;
    pub const BEAST_WARRIOR: u8 = 4;
    pub const BEAST: u8 = 5;
    pub const WINGED_BEAST: u8 = 6;
    pub const FIEND: u8 = 7;
    pub const FAIRY: u8 = 8;
    pub const INSECT: u8 = 9;
    pub const DINOSAUR: u8 = 10;
    pub const REPTILE: u8 = 11;
    pub const FISH: u8 = 12;
    pub const SEA_SERPENT: u8 = 13;
    pub const MACHINE: u8 = 14;
    pub const THUNDER: u8 = 15;
    pub const AQUA: u8 = 16;
    pub const PYRO: u8 = 17;
    pub const ROCK: u8 = 18;
    pub const PLANT: u8 = 19;
    pub const IMMORTAL: u8 = 20;
}

/// Static card data supplied by the importer (never embedded here).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardInfo {
    pub id: u16,
    /// Raw card byte 0: 0..=20 monster kinds; 32..=160 non-monster kinds.
    pub kind: u8,
    pub level: u8,
    /// 0 light, 1 dark, 2 fire, 3 earth, 4 water, 5 wind.
    pub attribute: u8,
    pub attack: u16,
    pub defense: u16,
    /// Whether each of the five effect entries is present (roles: battle
    /// pre-pass, movement, continuous, flip, destroy).
    pub effects: [bool; 5],
    /// Toon terrain favour flag (bit 1 of byte 0x29C976 + id·7).
    #[serde(default)]
    pub toon: bool,
    /// Card byte 1: index within the kind (limited-range trap dispatch).
    #[serde(default)]
    pub kind_index: u8,
    /// Raw effect entries (word0, word1): five for monsters, entry 0 only for
    /// other cards; `None` is the 0xFFFFFFFF sentinel.
    #[serde(default)]
    pub entries: [Option<[u32; 2]>; 5],
    /// Monsters: power-up compatibility row (0x29C970 + id·7).
    #[serde(default)]
    pub equip_bits: [u8; 7],
    /// Power-ups: category byte 0x29C5E0 + id and value u16 0x29C322 + id·2.
    #[serde(default)]
    pub power_up: Option<PowerUp>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerUp {
    /// 0 adds `value` to ATK and DEF; 4 transforms the monster.
    pub category: u8,
    pub value: u16,
}

/// 0x249C90: whether power-up `power_up` fits `monster` (leaders excluded by
/// the caller). Card 0x304 fits Zombies; others use the compatibility bits.
pub fn power_up_fits(monster: &CardInfo, power_up: u16) -> bool {
    if !monster.is_monster() {
        return false;
    }
    if power_up == 0x304 {
        return monster.kind & 0x1F == kind::ZOMBIE;
    }
    let bit = usize::from(power_up.wrapping_sub(0x2F0));
    bit < 56 && monster.equip_bits[bit >> 3] >> (bit & 7) & 1 == 1
}
impl CardInfo {
    pub fn is_monster(&self) -> bool {
        self.kind >> 5 == 0
    }
    /// Card byte 0 >> 5 (see [`class`]).
    pub fn class(&self) -> u8 {
        self.kind >> 5
    }
    /// Entry 0's type (word0 >> 8) for a non-monster card.
    pub fn card_effect_type(&self) -> Option<u32> {
        if self.is_monster() {
            return None;
        }
        self.entries[0].map(|[word0, _]| word0 >> 8)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuelUnit {
    pub id: UnitId,
    pub card: u16,
    pub side: u8,
    /// 0 leader; 1..=5 monsters (0x1E0870 slot rule); 6..=10 other cards.
    pub slot: u8,
    pub position: GridPos,
    pub face_up: bool,
    pub defence: bool,
    pub committed_face_up: bool,
    pub committed_defence: bool,
    /// +0x57 bit0: acted this turn or cannot act.
    pub acted: bool,
    /// +0x49: set by attribute disadvantage; blocks action at turn start.
    pub spellbind: u8,
    pub summon_order: u8,
    /// +0x38/+0x3A: permanent ATK/DEF changes (power-ups, traps, magic).
    #[serde(default)]
    pub attack_bonus: i16,
    #[serde(default)]
    pub defense_bonus: i16,
    /// +0x58 bit3: bound during this turn by 0x251CD0, so the end of this
    /// turn does not count down its spellbind; cleared at every turn end.
    #[serde(default)]
    pub spellbind_fresh: bool,
    /// +0x4B: entry-2 type 0xC countdown, started by the type 0xC flip
    /// effect (Larva/Pupa of Moth); 0 on every new unit.
    #[serde(default)]
    pub evo_counter: u8,
    /// +0x57 bit1: control changed (event 0x1A: Change of Heart, Brain
    /// Control). The unit stays in its owner's group and slot; its
    /// controller is the other side (0x24F790 / 0x251C80).
    #[serde(default)]
    pub stolen: bool,
    /// +0x48: the ability-8 summon discount 0x2703B0(group, card) stored
    /// when the record is created (0x1E0870 for summons, fusion rebuilds
    /// and 0x1E0800; 0x1D9C70 / 0x1D9F50 for group-2 records); 0 for the
    /// leaders. 0x1E1BD0 reads it for the unit's cost.
    #[serde(default)]
    pub discount: u8,
    /// Port addition (4-player modes only, `duel/multi.rs`): the side that
    /// took control of a stolen unit when "the other side" is ambiguous.
    /// Never set in a 2-player duel, where it is left out of the JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stolen_by: Option<u8>,
}

impl DuelUnit {
    /// 0x24F790: the side that controls the unit (`side` is the owning
    /// group; a stolen unit answers to the other side).
    pub fn controller(&self) -> u8 {
        match self.stolen_by {
            Some(by) if self.stolen => by,
            _ => self.side ^ u8::from(self.stolen),
        }
    }
}

/// A written group-2 record ([`Duel::temp_units`]): the cell (+0x40/+0x41)
/// and the card (+0x3C): 999 for the fillers 0x1D9C70 writes, the hand
/// card or fusion result of a hand slot, the lifted field unit's card in
/// slot 0 (state 0x44 copies its whole record there).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TempUnit {
    #[serde(flatten)]
    pub cell: GridPos,
    pub card: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideState {
    pub lp: u16,
    pub stars: u8,
    pub can_summon: bool,
    pub hand: Vec<u16>,
    /// Next card first.
    pub deck: Vec<u16>,
    pub leader_abilities: Vec<u8>,
    pub summons: u8,
    /// Removed cards in removal order (side +0x232, count +0x322).
    #[serde(default)]
    pub graveyard: Vec<u16>,
    /// +0x344: the Destiny Draw byte (SPECIAL-DRAW.md §1; the practice
    /// scripts preset it, `script::Tutorial` draws the card).
    #[serde(default)]
    pub destiny: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    LifePoints,
    Surrender,
    TurnLimit,
    Exodia,
    LeaderTrapped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    /// None is a draw.
    pub winner: Option<u8>,
    pub reason: EndReason,
}

/// Controller class of a pad-driven side (side +0x33C); computer modules are
/// the opponent id (< 0x20: Seto 1, Weevil 2, Rex 3, Keith 4, Pegasus 9,
/// Yugi 0x13), scripts 0x20..=0x63 (ABILITY-3-CPU.md section 1).
pub const PAD_CONTROLLER: u8 = 100;

fn pad_controllers() -> [u8; 2] {
    [PAD_CONTROLLER; 2]
}

// Per-side fields are `Vec`s (two entries in a 2-player duel, up to four in
// the port's 4-player modes). A two-entry `Vec` serialises exactly as the
// former `[_; 2]` arrays did, so 2-player states and their hashes are
// unchanged.
fn pad_controller_list() -> Vec<u8> {
    vec![PAD_CONTROLLER; 2]
}

fn two_experience_maps() -> Vec<BTreeMap<u16, ExperienceChange>> {
    vec![BTreeMap::new(), BTreeMap::new()]
}

fn two_ranks() -> Vec<u8> {
    vec![0; 2]
}

fn two_stranded() -> Vec<Vec<(u8, u16)>> {
    vec![Vec::new(), Vec::new()]
}

fn two_decks() -> Vec<Vec<u16>> {
    vec![Vec::new(), Vec::new()]
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuelSetup {
    pub terrain: Vec<u8>,
    pub leaders: [u16; 2],
    pub decks: [Vec<u16>; 2],
    pub first: u8,
    pub leader_abilities: [Vec<u8>; 2],
    /// Each side's controller byte (see [`PAD_CONTROLLER`]); only the
    /// Destiny Draw arming reads it (ABILITY-3-CPU.md section 4).
    #[serde(default = "pad_controllers")]
    pub controllers: [u8; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Duel {
    pub schema_version: String,
    pub terrain: Vec<u8>,
    pub units: Vec<DuelUnit>,
    pub sides: Vec<SideState>,
    /// 0x2DB8F3: increments at the start of the first player's turn.
    pub round: u8,
    pub first: u8,
    pub active: u8,
    pub pending_summon: Option<GridPos>,
    pub outcome: Option<Outcome>,
    /// Experience changes made during play, per side and card id
    /// (`duel/experience.rs`); the duel-end award is [`Duel::end_award`].
    #[serde(default = "two_experience_maps")]
    pub experience: Vec<BTreeMap<u16, ExperienceChange>>,
    /// Side +0x33C controller bytes ([`DuelSetup::controllers`]).
    #[serde(default = "pad_controller_list")]
    pub controllers: Vec<u8>,
    /// A selection the active side owes (states 0x2D / 0x30).
    #[serde(default)]
    pub pending: Option<Pending>,
    /// The temporary unit records of group 2 (0x2DC178 + slot·0x5C, the
    /// hand display): the cell each record was last written at, `None`
    /// once reset (0x1DDFE0(u, −1): slot 0xB, cell 7/7). Written by the
    /// summon's refill 0x1D8F90 (state 0x3F: slots 0..=5 at the cursor,
    /// slot 0 as card 999, then the hand, then 999 fillers), the chain
    /// (0x250D50 resets a consumed record, 0x1D9F50 re-creates a fusion
    /// result in the older record's slot at the cursor) and the duel start
    /// (all reset). Survivors keep their record. The computer controllers
    /// read these positions through 0x1E2310(column, row) when 0x2800C0
    /// hands a cell to 0x27F170 as if it were a unit key (column 2 = this
    /// group), and slot 0 as "side 2's leader" (0x1E1E80 / 0x273370) when
    /// a hand card's virtual record (owner 2) is evaluated. Slots 6..=10 are
    /// never written.
    #[serde(default)]
    pub temp_units: [Option<TempUnit>; 11],
    /// The shared LCG state 0x19AE88 ([`next_rand31`]) as the duel's effects
    /// consume it (Barrel Dragon's pick, Magical Labyrinth's wall). Seeded by
    /// [`Duel::seed_rng`]; the stream is not claimed to match the original's
    /// (the visual pick of the 0x15 handler draws a second number, not
    /// modelled).
    #[serde(default)]
    pub rng: u64,
    /// The Destiny Draw's card rows (0x2B7AF0), supplied by the duel's
    /// creator ([`Duel::set_destiny_rows`]); without them an armed side's
    /// special draw is refused (scripted sides leave it to the session).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destiny_rows: Option<script::DestinyRows>,
    /// Each side's saved leader rank for the Destiny byte's rank bonus
    /// ([`Duel::set_leader_ranks`]).
    #[serde(default = "two_ranks")]
    pub leader_ranks: Vec<u8>,
    /// Unit records rewritten by duel effects (kind, attribute, effect
    /// entries), keyed by unit id; see [`Duel::unit_card`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub records: BTreeMap<u16, UnitRecord>,
    /// Unit +0x4A, keyed by unit id: the continuous bind of an entry-2
    /// type-0x17 card (Binding Chain 386), 0xFF while set (event 0 with
    /// rec[4] = 1, 0x257C7C). It keeps the unit from acting (turn start
    /// 0x269A74, battle step 2 0x24B740) until a turn end finds it no
    /// longer covered (0x273E20 → 0x2683A0); see `white_rules.rs`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub chain_bound: BTreeMap<u16, u8>,
    /// Deck cards past a gap of the 40-slot deck array (side +0x1E0), as
    /// (slot, card), sorted by slot: The Wicked Worm Beast's return
    /// (0x1DFCB0) can write a group-1 card past the compacted cards, and
    /// only a successful draw of that group (0x1DFFB8 → 0x1DFDA0) closes
    /// the gap; a draw whose slot 0 is empty returns 999 and compacts
    /// nothing. `sides[g].deck` stays the compacted front. See
    /// `sweep_rules.rs` (REFUSALS-SWEEP.md §worm).
    #[serde(default = "two_stranded", skip_serializing_if = "stranded_empty")]
    pub deck_stranded: Vec<Vec<(u8, u16)>>,
    /// Side +0x342, the SP recharge rate added at each turn start from round
    /// 2 (0x269570 reads it through 0x1DF910). 3 in the campaign; Custom
    /// Duel's "SP Recharge Rate" row sets 1..=12 ([`Duel::set_star_income`]).
    #[serde(default = "default_star_income")]
    pub star_income: Vec<u8>,
    /// Each side's saved deck list (the current deck of its card block,
    /// 0x214960 / 0x215040): the 40 cards it brought, which the deck
    /// insert dealt from (the loader 0x214120 writes a static deck record
    /// there for a computer side). Set from the setup decks by
    /// [`Duel::new`]; empty (unknown) for an observed duel. Read by the
    /// generic module's Exodia tests 0x16D800 / 0x16EDE0 / 0x170B80
    /// (REFUSALS-SWEEP.md §7).
    #[serde(default = "two_decks", skip_serializing_if = "decks_empty")]
    pub saved_decks: Vec<Vec<u16>>,
    /// Port addition: the 4-player modes' state (`duel/multi.rs`); `None`
    /// (and absent from the JSON) in every 2-player duel. Only with the
    /// `online` feature.
    #[cfg(feature = "online")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi: Option<Box<multi::Multi>>,
    next_unit: u16,
}

fn default_star_income() -> Vec<u8> {
    vec![STAR_INCOME; 2]
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DuelCommand {
    /// Choose a summon cell (state 0x3E), which refills the hand (0x3F).
    OpenSummon {
        cell: GridPos,
    },
    /// Pick one hand card for the open summon (single card only).
    /// Hand cards in marking order (one card, or a fusion/equip list).
    ChooseSummon {
        hand: Vec<usize>,
    },
    CancelSummon,
    /// Activate a unit and walk `path` (excluding the origin). An empty path
    /// holds position. `face_up` flips a face-down non-leader first (never
    /// back); `defence` is only allowed when holding.
    Move {
        unit: UnitId,
        path: Vec<GridPos>,
        face_up: bool,
        defence: Option<bool>,
    },
    EndTurn,
    Surrender,
    /// Answer the selection a magic activation opened ([`Duel::pending`]):
    /// states 0x30 (a unit) and 0x2D (a graveyard card).
    Pick {
        pick: Pick,
    },
    /// Port addition (N-player duels only, `duel/multi.rs`): aim the acting
    /// side's single-target effects ("the opponent") at `side`.
    Target {
        side: u8,
    },
}

/// A selection the active side owes after a magic activation whose builder
/// returned 2 (0x2547E0): the card has already left for the graveyard, the
/// effect waits for the choice. Opened by [`DuelCommand::Move`], answered by
/// [`DuelCommand::Pick`]; every other command is refused meanwhile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Pending {
    /// Change of Heart (type 0x1F filter 0x33, builder arm 0x2558BC, state
    /// 0x30): `units` are the cards in slots 1..10 of the group opposing
    /// `by` (state 0x2F, 0x1D58B8: by group membership, slots ascending,
    /// whatever their controller), one of which changes controller until
    /// the next turn starts. Circle spends the card for nothing.
    ControlChange { by: u8, units: Vec<UnitId> },
    /// Mimicat / Monster Reborn (type 0x14, builder arm 0x254A18, state
    /// 0x2D): a card of one of `graveyards` (mode 1 the controller's, 2 the
    /// opponent's, 3 both; `monsters` for filter 0x1D) is created for `by`
    /// on `at`, the cell the magic card stood on.
    GraveyardCopy {
        by: u8,
        at: GridPos,
        graveyards: Vec<u8>,
        monsters: bool,
        /// Class code 2 of the list builder 0x251920 (card id >= 0x2AB):
        /// magic filter 0x1E (688 Graverobber, created on its own cell) and
        /// the flip type 0x14 pick of Mask of Darkness / Magician of Faith
        /// (created on a leader neighbour, 0x24FB40).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        non_monsters: bool,
    },
}

/// The answer to a [`Pending`] selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Pick {
    /// A unit of the pending list.
    Unit { unit: UnitId },
    /// The `index`-th card (oldest first) of `side`'s graveyard.
    Graveyard { side: u8, index: usize },
    /// Circle: the selection closes with no effect (the card is spent: the
    /// 0x1A handler sees rec+0xC = -1, the event-1 handler a card of 999).
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DuelEvent {
    Drew {
        side: u8,
        cards: Vec<u16>,
    },
    /// Event 0x72 (message 0x116 "Destiny Draw!") before the refill whose
    /// last card is `card` (the pick of 0x270FC0; `duel/destiny.rs`).
    DestinyDraw {
        side: u8,
        card: u16,
    },
    Summoned {
        unit: UnitId,
        card: u16,
        at: GridPos,
        cost: u8,
    },
    Revealed {
        unit: UnitId,
        card: u16,
    },
    Moved {
        unit: UnitId,
        from: GridPos,
        to: GridPos,
    },
    Battle {
        attacker: UnitId,
        defender: UnitId,
        attack: u16,
        defend: u16,
        defence: bool,
    },
    Damage {
        side: u8,
        amount: u16,
    },
    Destroyed {
        unit: UnitId,
        card: u16,
    },
    Spellbound {
        unit: UnitId,
    },
    /// An effect cleared the unit's spellbind (0x251CD0 with amount 0:
    /// Dragon Piper's flip, Magical Neutralizing Force Field).
    Unbound {
        unit: UnitId,
    },
    /// 0x252FF0(g, s, 0) (Water Element): the card was turned face down.
    Concealed {
        unit: UnitId,
        card: u16,
    },
    /// Event 0x1A: `side` controls the unit until the next turn start.
    Controlled {
        unit: UnitId,
        side: u8,
    },
    /// Event 1 with bit 17 (0x2586A0, Mimicat / Monster Reborn): `card`
    /// left side `from`'s graveyard for a new face-down unit on `at`.
    Revived {
        unit: UnitId,
        card: u16,
        at: GridPos,
        from: u8,
    },
    /// A magic activation opened a selection ([`Duel::pending`]).
    SelectionOpened {
        card: u16,
    },
    /// A trap or magic card took effect; `target` is the unit it hit.
    Activated {
        unit: UnitId,
        card: u16,
        target: Option<UnitId>,
    },
    /// A used card left the field for its owner's graveyard.
    Discarded {
        unit: UnitId,
        card: u16,
    },
    /// A summon or move combined two cards (fusion or transformation).
    Fused {
        cards: [u16; 2],
        result: u16,
    },
    /// A power-up of a summon chain raised the chain's monster (0x271B90;
    /// followed by the power-up's `Dropped`).
    Equipped {
        side: u8,
        monster: u16,
        power_up: u16,
        value: i16,
    },
    /// A card left a summon chain for the graveyard (a failed combination,
    /// a used power-up, or a result with no free slot).
    Dropped {
        side: u8,
        card: u16,
    },
    /// A move or attack was stopped; the unit's turn is used.
    Cancelled {
        unit: UnitId,
    },
    /// Battle step 1's record {0x36, defender, attacker, ..}: the attack an
    /// event-0x36 trap (Negate Attack, Mirror Force, Mirror Wall) answers;
    /// its c034 plays on the defender (NEWLY-ALLOWED.md 1.4).
    Attacked {
        attacker: UnitId,
        defender: UnitId,
    },
    /// Permanent ATK/DEF change (+0x38/+0x3A).
    Modified {
        unit: UnitId,
        attack: i16,
        defense: i16,
    },
    Crushed {
        unit: UnitId,
    },
    /// An effect created a unit (entry-4 type 0x38, entry-2 type 0xC).
    Created {
        unit: UnitId,
        card: u16,
        at: GridPos,
    },
    TurnEnded {
        side: u8,
    },
    TurnStarted {
        side: u8,
        round: u8,
        stars: u8,
    },
    Ended {
        outcome: Outcome,
    },
    /// Port addition (N-player duels only): `side` is out of the duel.
    KnockedOut {
        side: u8,
        reason: EndReason,
    },
    /// Port addition (N-player duels only): `side` aims its single-target
    /// effects at `target`.
    TargetChosen {
        side: u8,
        target: u8,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DuelError {
    Finished,
    NotYourTurn,
    InvalidCommand(&'static str),
    Illegal(&'static str),
    Unsupported(String),
}
impl fmt::Display for DuelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Finished => f.write_str("duel has ended"),
            Self::NotYourTurn => f.write_str("not the acting side"),
            Self::InvalidCommand(m) | Self::Illegal(m) => f.write_str(m),
            Self::Unsupported(m) => write!(f, "not yet supported: {m}"),
        }
    }
}
impl std::error::Error for DuelError {}
type Result<T> = std::result::Result<T, DuelError>;

/// Card data lookup supplied by the caller.
pub trait CardData {
    fn card(&self, id: u16) -> Option<&CardInfo>;
    /// 0x24AEC0: the order-free fusion of two cards. `None` when this data
    /// carries no fusion table (fusions are then refused), `Some(None)` when
    /// the pair does not fuse.
    fn fusion(&self, _a: u16, _b: u16) -> Option<Option<u16>> {
        None
    }
    /// 0x2565B0: the three requirement words for ritual result `result`.
    /// `None` when this data carries no ritual table (rituals are then
    /// refused), `Some(None)` when the id has no requirements (the ritual
    /// card is destroyed).
    fn ritual(&self, _result: u16) -> Option<Option<[u32; 3]>> {
        None
    }
}

/// Imported card data plus the fusion table (pairs stored as (low, high,
/// result), sorted for lookup).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalogue {
    pub cards: Vec<CardInfo>,
    pub fusions: Vec<[u16; 3]>,
    /// Ritual result id and its three requirement words (0x2565B0).
    #[serde(default)]
    pub rituals: Vec<(u16, [u32; 3])>,
}
impl Catalogue {
    pub fn new(cards: Vec<CardInfo>, fusions: impl IntoIterator<Item = [u16; 3]>) -> Self {
        let mut fusions: Vec<[u16; 3]> = fusions
            .into_iter()
            .map(|[a, b, r]| [a.min(b), a.max(b), r])
            .collect();
        fusions.sort_unstable();
        Self {
            cards,
            fusions,
            rituals: Vec::new(),
        }
    }

    /// Adds the ritual requirement table (result id, words).
    pub fn with_rituals(mut self, rituals: impl IntoIterator<Item = (u16, [u32; 3])>) -> Self {
        self.rituals = rituals.into_iter().collect();
        self
    }
}
impl CardData for Catalogue {
    fn card(&self, id: u16) -> Option<&CardInfo> {
        self.cards.card(id)
    }
    fn fusion(&self, a: u16, b: u16) -> Option<Option<u16>> {
        let key = [a.min(b), a.max(b)];
        Some(
            self.fusions
                .binary_search_by(|f| [f[0], f[1]].cmp(&key))
                .ok()
                .map(|i| self.fusions[i][2]),
        )
    }
    fn ritual(&self, result: u16) -> Option<Option<[u32; 3]>> {
        Some(
            self.rituals
                .iter()
                .find(|(r, _)| *r == result)
                .map(|&(_, words)| words),
        )
    }
}

/// 0x24A960 transformations by category-4 power-ups (when the power-up fits
/// the monster): Elegant Egotist → 273, Cocoon of Evolution → 427,
/// Metalmorph 7 → 501 and 342 → 488. Insect Imitation (800) draws from the
/// RNG and is not supported; `None` means "no deterministic result".
pub fn transform(monster: u16, power_up: u16) -> Option<u16> {
    match (power_up, monster) {
        (797, _) => Some(273),
        (798, _) => Some(427),
        (799, 7) => Some(501),
        (799, 342) => Some(488),
        _ => None,
    }
}
impl CardData for [CardInfo] {
    fn card(&self, id: u16) -> Option<&CardInfo> {
        self.get(usize::from(id)).filter(|c| c.id == id)
    }
}
impl CardData for Vec<CardInfo> {
    fn card(&self, id: u16) -> Option<&CardInfo> {
        self.as_slice().card(id)
    }
}

/// An entry-2 stat aura (types 0x12 boost / 0x13 weaken) with a filter
/// 0x2505F0 accepts (types 2, 6, 7, 0xE, 0xF, 0x10, 0x11, 0x13; attributes
/// 0x14, 0x15, 0x19; 0x1D any monster; 0x31 monsters Electro-Whip 0x2FF
/// fits, see [`aura_filter_matches`]).
struct Aura {
    mode: u32,
    filter: u32,
    value: i32,
    /// Type 0x22 (Millennium Shield): DEF only (0x267A00 adds it when its
    /// a3 is 0, the DEF readers) [F E2-0x22.md].
    def_only: bool,
}

fn aura(card: &CardInfo) -> Option<Aura> {
    let [word0, word1] = card.entries[2]?;
    let (sign, def_only) = match word0 >> 8 {
        0x12 => (1, false),
        0x13 => (-1, false),
        0x22 => (1, true),
        _ => return None,
    };
    let filter = word0 & 0x3F;
    const ACCEPTED: [u32; 13] = [
        2, 6, 7, 0xE, 0xF, 0x10, 0x11, 0x13, 0x14, 0x15, 0x19, 0x1D, 0x31,
    ];
    ACCEPTED.contains(&filter).then_some(Aura {
        mode: (word0 & 0xFF) >> 6,
        filter,
        value: sign * (word1 & 0xFFFF) as i32,
        def_only,
    })
}

/// Target filters of 0x24E360/0x2505F0 for monsters: 0x00–0x13 type, 0x14–0x19
/// attribute, 0x1D any monster.
fn filter_matches(filter: u32, card: &CardInfo) -> bool {
    card.is_monster()
        && match filter {
            0..=0x13 => u32::from(card.kind & 0x1F) == filter,
            0x14..=0x19 => u32::from(card.attribute) == filter - 0x14,
            0x1D => true,
            _ => false,
        }
}

/// 0x2505F0's filter test for an aura target (monsters only; +0x3E class
/// checked first). Filter 0x31 (0x250738) is 0x24A0C0(card id, 0x2FF) !=
/// 0x3E7: bit 15 of the card's compatibility row 0x29C970, i.e. the monsters
/// Electro-Whip 767 (category 0, value 500, so 0x24A0C0 never answers 0x3E7
/// once the bit is set) can equip; only The Immortal of Thunder 536 uses it
/// (E2-536-TRAP-0x4.md §1).
fn aura_filter_matches(filter: u32, card: &CardInfo) -> bool {
    match filter {
        0x31 => card.is_monster() && power_up_fits(card, 0x2FF),
        _ => filter_matches(filter, card),
    }
}

/// The card-only filters of the 0x24E360 jump table 0x2D3130 (F05b.md §3):
/// [`filter_matches`] plus 0x1E (non-monster cards) and 0x1F (any card); the
/// stat, id and position filters are refused.
fn select_filter(filter: u32, card: &CardInfo) -> Result<bool> {
    Ok(match filter {
        0..=0x19 | 0x1D => filter_matches(filter, card),
        0x1E => !card.is_monster(),
        0x1F => true,
        _ => return Err(unsupported(format!("target filter {filter:#x}"))),
    })
}

/// Flip-effect outcome for the battle steps (0x265250 return bits).
enum Flip {
    Continue,
    /// Bit 9: the battle opponent was destroyed.
    OpponentDestroyed,
    /// Bit 11 (Ooguchi's 0x801): step 0xE, the battle ends (no damage, no
    /// advance).
    BattleEnds,
    /// Bit 10: the flipping unit destroyed itself (Man Eater on Forest).
    SelfDestroyed,
}

/// Result of the entry-0 battle pre-pass (0x263840): stat deltas and the
/// type-5 damage shields of the defender and the attacker.
struct PrePass {
    def_delta: i32,
    att_delta: i32,
    def_shield: bool,
    att_shield: bool,
    /// Type 4 (0x263B48, result 0x10001): the unit's effect decides the
    /// battle (rec bit 2 for the defender, bit 1 for the attacker).
    def_decides: bool,
    att_decides: bool,
    /// Type 1 (0x263E24, Jirai Gumo; rec+0x10 bit 3 on the attacker, bit 4
    /// on the defender): the OTHER unit's permanent ATK/DEF changes are set
    /// to 0 and its pre-pass delta is dropped before its stats are read
    /// (0x2487DC / 0x24890C / 0x248CE4 / 0x248E18; emu C1b/C2b/C3/C4).
    def_nullified: bool,
    att_nullified: bool,
    /// Type 0x3A (0x263DF4, One Who Hunts Souls 217; both non-leaders):
    /// rec+0x10 bit 2 -> record 0x5E word 3 bit 27 -> battle step flag
    /// 0x400: the removals of steps 0xB/0xF skip the graveyard (0x1E00F0)
    /// for every unit this battle destroys. [F 0x24C518, 0x24C5BC; emu
    /// REFUSALS-SWEEP.md OH1/OH2]
    banish: bool,
}

/// What the battle machine does after an entry-4 destruction effect
/// (0x2530F0 return bits, F04b.md §4).
enum Aftermath {
    /// The normal steps follow (removal, binds, advance).
    Continue,
    /// Bit 9: the effect removed the battle units itself; no advance, no
    /// binds, no further entry-4 effect.
    BattleOver,
    /// Bit 8 (Golgoil, 0x2535F8 result 0x101): the unit is not removed
    /// (step 0xB/0x10 without its flag); the rest of the battle goes on.
    Survives,
    /// Bit 10 (Big Eye, 0x253464 result 0x401) from a dying defender: the
    /// removals run at step 0xF, so no binds, no advance and no crush sweep
    /// (F04a.md §1.2). Not examined for a dying attacker (step 0xA).
    NoAdvance,
}

/// One element of a summon chain: the own unit lifted from the chosen cell,
/// or a card (from the hand or an earlier fusion).
#[derive(Clone, Debug)]
enum ChainItem {
    Field(DuelUnit),
    Hand(u16),
}
impl ChainItem {
    fn card(&self) -> u16 {
        match self {
            Self::Field(u) => u.card,
            Self::Hand(id) => *id,
        }
    }
}

enum ChainStep {
    Fuse(u16),
    /// Event-1 equip record; `value` is the plain category-0 amount (0 for
    /// Riryoku and category 1, see [`EquipSpecial`]).
    Equip { value: i16, special: EquipSpecial },
    Fail,
}

/// 0x2730D0(kind): the terrain leader ability #7 lays under its leader:
/// Immortal → 9 (Crush), Aqua → Sea, Dinosaur → Wasteland, Insect →
/// Forest, Fiend and Spellcaster → Dark, any other kind → Normal. [F]
fn leader_terrain(kind: u8) -> u8 {
    match kind {
        0x14 => 9,
        0x10 => terrain::SEA,
        0xA => terrain::WASTELAND,
        9 => terrain::FOREST,
        7 | 1 => terrain::DARK,
        _ => terrain::NORMAL,
    }
}

fn stranded_empty(s: &[Vec<(u8, u16)>]) -> bool {
    s.iter().all(Vec::is_empty)
}

fn decks_empty(s: &[Vec<u16>]) -> bool {
    s.iter().all(Vec::is_empty)
}

fn unsupported(m: impl Into<String>) -> DuelError {
    DuelError::Unsupported(m.into())
}

/// Terrain affinity 0x24F8A0: +1 favoured, -1 unfavoured, 0 neutral.
pub fn terrain_affinity(terrain_code: u8, card: &CardInfo) -> i32 {
    use kind::*;
    if !card.is_monster() {
        return 0;
    }
    let k = card.kind & 0x1F;
    let (plus, minus): (&[u8], &[u8]) = match terrain_code {
        terrain::FOREST => (&[BEAST_WARRIOR, BEAST, INSECT, PYRO, PLANT], &[FIEND]),
        terrain::WASTELAND => (
            &[ZOMBIE, DINOSAUR, MACHINE, ROCK],
            &[AQUA, FISH, SEA_SERPENT, PLANT],
        ),
        terrain::MOUNTAIN => (&[DRAGON, WINGED_BEAST, FAIRY, THUNDER], &[ZOMBIE]),
        terrain::MEADOW => (&[WARRIOR, BEAST_WARRIOR], &[SPELLCASTER]),
        terrain::SEA => (&[FISH, SEA_SERPENT, THUNDER, AQUA], &[MACHINE, PYRO]),
        terrain::DARK => (&[SPELLCASTER, ZOMBIE, FIEND], &[FAIRY]),
        terrain::TOON => return if card.toon { 1 } else { -1 },
        terrain::CRUSH => (&[IMMORTAL], &[]),
        _ => (&[], &[]),
    };
    if plus.contains(&k) {
        1
    } else if minus.contains(&k) {
        -1
    } else {
        0
    }
}

/// Attribute relation 0x250390 on the cycle Light > Water > Fire > Earth >
/// Wind > Dark > Light.
pub fn attribute_relation(a: u8, b: u8) -> i32 {
    const CYCLE: [u8; 6] = [0, 4, 2, 3, 5, 1];
    let (Some(ia), Some(ib)) = (
        CYCLE.iter().position(|&x| x == a),
        CYCLE.iter().position(|&x| x == b),
    ) else {
        return 0;
    };
    if (ia + 1) % 6 == ib {
        1
    } else if (ib + 1) % 6 == ia {
        -1
    } else {
        0
    }
}

/// Campaign deck build (duel init loop 0x1AE7B0): for i in 0..40 insert
/// source[side][i] for side 0 then side 1 via 0x1DFCB0, which retries
/// idx = (int)(40.0f * ((float)rand31 / 2^31)) until the slot is empty.
/// `rng` is the shared 64-bit LCG state (0x19AE88). Card 0x29F is rejected.
pub fn build_campaign_decks(rng: &mut u64, sources: [&[u16]; 2]) -> [Vec<u16>; 2] {
    let mut decks = [[None::<u16>; DECK_SIZE], [None::<u16>; DECK_SIZE]];
    for i in 0..DECK_SIZE {
        for side in 0..2 {
            let Some(&card) = sources[side].get(i) else {
                continue;
            };
            if card == REJECTED_DECK_CARD || usize::from(card) >= usize::from(CARD_COUNT) {
                continue;
            }
            loop {
                let r = next_rand31(rng);
                let idx = (40.0_f32 * (r as f32 / 2_147_483_648.0_f32)) as usize;
                if idx < DECK_SIZE && decks[side][idx].is_none() {
                    decks[side][idx] = Some(card);
                    break;
                }
            }
        }
    }
    decks.map(|d| d.into_iter().flatten().collect())
}

pub fn next_rand31(state: &mut u64) -> u32 {
    *state = state.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(1);
    ((*state >> 32) & 0x7FFF_FFFF) as u32
}

impl Duel {
    pub fn new(setup: DuelSetup, cards: &impl CardData) -> Result<(Self, Vec<DuelEvent>)> {
        if setup.terrain.len() != 49 || setup.terrain.iter().any(|&t| t > 9) || setup.first > 1 {
            return Err(DuelError::InvalidCommand("invalid duel setup"));
        }
        let mut duel = Self::build(
            setup.terrain,
            &setup.leaders,
            &setup.decks,
            setup.first,
            &setup.leader_abilities,
            setup.controllers.to_vec(),
            &LEADER_CELLS,
            cards,
        )?;
        let mut events = Vec::new();
        duel.start_turn(setup.first, cards, &mut events)?;
        Ok((duel, events))
    }

    /// The state [`Duel::new`] starts from, for any number of sides
    /// (leaders on `cells`), before the first turn starts.
    #[allow(clippy::too_many_arguments)]
    fn build(
        terrain: Vec<u8>,
        leaders: &[u16],
        decks: &[Vec<u16>],
        first: u8,
        leader_abilities: &[Vec<u8>],
        controllers: Vec<u8>,
        cells: &[GridPos],
        cards: &impl CardData,
    ) -> Result<Self> {
        // Leader abilities (0x1E1E80). The HUD lists #4..#16; #0..#3 are not
        // displayed: #0 has no gameplay query, #1 is only read for
        // pad-controlled sides (0x1D61C4, campaign save bookkeeping, not
        // modelled), #2 "Face-down Card Prediction" has no consumer at all
        // (no call site asks 0x273070 for n = 2; claude-effects/next/
        // ABILITY-2.md), #3 permits the low-LP special draw (refused at turn
        // start for the sides that arm it). Of the rank abilities #4 (leader
        // steps), #6 (halved direct damage), #8 (the same-type summon
        // discount 0x273160, `summon_discount`), #9 (leader window 5x5), #10
        // (same-type strength, ABILITY-9-10.md), #12 (same-type movement,
        // ABILITY-12.md) and #14 (the opposing leader's race-weakening aura,
        // the subtract half of 0x273420 in `queries::leader_aura` with the
        // race table 0x2735C0) are implemented.
        for abilities in leader_abilities {
            if let Some(a) = abilities
                .iter()
                .find(|a| !matches!(a, 0..=16))
            {
                return Err(unsupported(format!("leader ability #{a}")));
            }
        }
        for deck in decks {
            if deck.len() > DECK_SIZE || deck.iter().any(|&c| cards.card(c).is_none()) {
                return Err(DuelError::InvalidCommand("deck contains unknown cards"));
            }
            // The deck insert rejects 0x29F (build_campaign_decks drops it);
            // CreateUnit would store it as 999.
            if deck.contains(&REJECTED_DECK_CARD) {
                return Err(DuelError::InvalidCommand("deck contains card 0x29F"));
            }
        }
        let mut units = Vec::new();
        for (side, &position) in cells.iter().enumerate() {
            let leader = cards
                .card(leaders[side])
                .filter(|c| c.is_monster())
                .ok_or(DuelError::InvalidCommand("leader must be a monster"))?;
            units.push(DuelUnit {
                id: UnitId(side as u16),
                card: leader.id,
                side: side as u8,
                slot: 0,
                position,
                face_up: true,
                defence: false,
                committed_face_up: true,
                committed_defence: false,
                acted: false,
                spellbind: 0,
                summon_order: 0,
                attack_bonus: 0,
                defense_bonus: 0,
                spellbind_fresh: false,
                evo_counter: 0,
                stolen: false,
                discount: 0,
                stolen_by: None,
            });
        }
        let side_state = |side: usize| SideState {
            lp: START_LP,
            stars: START_STARS,
            can_summon: true,
            hand: Vec::new(),
            deck: decks[side].clone(),
            leader_abilities: leader_abilities[side].clone(),
            summons: 0,
            graveyard: Vec::new(),
            destiny: 0,
        };
        let n = cells.len();
        Ok(Self {
            schema_version: DUEL_SCHEMA.into(),
            terrain,
            units,
            sides: (0..n).map(side_state).collect(),
            round: 0,
            first,
            active: first,
            pending: None,
            pending_summon: None,
            outcome: None,
            experience: vec![BTreeMap::new(); n],
            controllers,
            temp_units: [None; 11],
            rng: 0,
            destiny_rows: None,
            leader_ranks: vec![0; n],
            records: BTreeMap::new(),
            chain_bound: BTreeMap::new(),
            deck_stranded: vec![Vec::new(); n],
            star_income: vec![STAR_INCOME; n],
            saved_decks: decks.to_vec(),
            #[cfg(feature = "online")]
            multi: None,
            next_unit: n as u16,
        })
    }

    /// Rebuild a duel from an observed position (e.g. a saved original
    /// state) for comparison tests. Validates cells, leaders and cards.
    pub fn from_observed(
        terrain: Vec<u8>,
        units: Vec<DuelUnit>,
        sides: [SideState; 2],
        round: u8,
        first: u8,
        active: u8,
        cards: &impl CardData,
    ) -> Result<Self> {
        let sides = sides.to_vec();
        if terrain.len() != 49 || first > 1 || active > 1 {
            return Err(DuelError::InvalidCommand("invalid observed duel"));
        }
        for side in 0..2 {
            if units
                .iter()
                .filter(|u| u.side == side && u.slot == 0)
                .count()
                != 1
            {
                return Err(DuelError::InvalidCommand(
                    "each side needs exactly one leader",
                ));
            }
        }
        if units
            .iter()
            .any(|u| u.position.index().is_none() || cards.card(u.card).is_none())
        {
            return Err(DuelError::InvalidCommand("observed unit is invalid"));
        }
        let next_unit = units.iter().map(|u| u.id.0).max().unwrap_or(0) + 1;
        Ok(Self {
            schema_version: DUEL_SCHEMA.into(),
            pending: None,
            terrain,
            units,
            sides,
            round,
            first,
            active,
            pending_summon: None,
            outcome: None,
            experience: two_experience_maps(),
            controllers: pad_controller_list(),
            temp_units: [None; 11],
            rng: 0,
            destiny_rows: None,
            leader_ranks: two_ranks(),
            records: BTreeMap::new(),
            chain_bound: BTreeMap::new(),
            deck_stranded: two_stranded(),
            star_income: default_star_income(),
            saved_decks: two_decks(),
            #[cfg(feature = "online")]
            multi: None,
            next_unit,
        })
    }

    pub fn unit(&self, id: UnitId) -> Option<&DuelUnit> {
        self.units.iter().find(|u| u.id == id)
    }
    /// Sets the 0x19AE88 state the duel's effects draw from.
    pub fn seed_rng(&mut self, state: u64) {
        self.rng = state;
    }

    /// Custom Duel's duel words (side +0x33A LP, +0x342 SP recharge rate)
    /// as the set-up screen left them [F 0x1C6BC0 / 0x1C6FC0]: applied
    /// after [`Duel::new`] (the first turn's start reads neither).
    pub fn set_custom_rules(&mut self, lp: [u16; 2], star_income: [u8; 2]) {
        for s in 0..2 {
            self.sides[s].lp = lp[s].min(MAX_LP);
        }
        self.star_income = star_income.to_vec();
    }

    /// One 0x19AE88 draw.
    fn rand31(&mut self) -> u32 {
        next_rand31(&mut self.rng)
    }

    pub fn unit_at(&self, p: GridPos) -> Option<&DuelUnit> {
        self.units.iter().find(|u| u.position == p)
    }
    fn terrain_at(&self, p: GridPos) -> u8 {
        self.terrain[p.index().expect("valid cell")]
    }
    fn leader(&self, side: u8) -> &DuelUnit {
        self.units
            .iter()
            .find(|u| u.side == side && u.slot == 0)
            .expect("leaders are never destroyed")
    }
    /// 0x268710(type): the units of slots 1..10 of both groups in face-up
    /// committed defence (+0x57 bits 4, 3, 5) whose record entry 2 has
    /// `type`.
    fn continuous_count(&self, ty: u32, cards: &impl CardData) -> usize {
        self.units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && u.defence
                    && u.committed_face_up
                    && u.committed_defence
                    && self.unit_card(u, cards).entries[2].is_some_and(|[w, _]| w >> 8 == ty)
            })
            .count()
    }

    fn has_ability(&self, side: u8, ability: u8) -> bool {
        self.sides[usize::from(side)]
            .leader_abilities
            .contains(&ability)
    }

    /// 0x273070(n, side): the ability is listed and the opponent does not
    /// control a face-up committed-defence unit in slots 1..=10 with an
    /// entry-2 type 0x58 (682 Moisture Creature; 0x268890), controller as
    /// 0x24F790 (ABILITY-12.md §2).
    fn leader_has(&self, side: u8, ability: u8, cards: &impl CardData) -> bool {
        let suppressed = self.units.iter().any(|u| {
            u.controller() != side
                && u.slot != 0
                && u.defence
                && u.committed_face_up
                && u.committed_defence
                && cards
                    .card(u.card)
                    .and_then(|c| c.entries[2])
                    .is_some_and(|[w, _]| w >> 8 == 0x58)
        });
        !suppressed && self.has_ability(side, ability)
    }

    /// 0x273370(side, col, row): the cell lies in the clipped square around
    /// `side`'s leader, half-size 2 with ability #9 and 1 otherwise; the
    /// leader's own cell counts (ABILITY-9-10.md §2).
    fn in_window(&self, side: u8, cell: GridPos, cards: &impl CardData) -> bool {
        let half = if self.leader_has(side, 9, cards) {
            2
        } else {
            1
        };
        let leader = self.leader(side).position;
        let (lc, lr) = (i32::from(leader.column), i32::from(leader.row));
        let (c, r) = (i32::from(cell.column), i32::from(cell.row));
        (lc - half).max(0) <= c
            && c <= (lc + half).min(6)
            && (lr - half).max(0) <= r
            && r <= (lr + half).min(6)
    }

    /// 0x273830(cell, side, kind, n): `cell` in `side`'s leader window, the
    /// leader has ability `n`, and the leader card's kind (byte 0 & 0x1F, read
    /// from the card record) equals `kind`. Non-monsters have kind 0.
    fn leader_kind_aura(
        &self,
        cell: GridPos,
        side: u8,
        kind: u8,
        ability: u8,
        cards: &impl CardData,
    ) -> bool {
        self.in_window(side, cell, cards)
            && self.leader_has(side, ability, cards)
            && cards
                .card(self.leader(side).card)
                .is_some_and(|c| c.kind & 0x1F == kind & 0x1F)
    }

    /// 0x273420: #10 adds min(100 * L, 500) for the controller's race in
    /// its leader window; #14 subtracts the opposing leader's amount for
    /// races selected by 0x2735C0 inside that leader's window. L is the
    /// leader's level minus its stored +0x48 discount, clamped to 0..=12.
    fn leader_aura(&self, unit: &DuelUnit, cell: GridPos, cards: &impl CardData) -> i32 {
        let card = self.unit_card(unit, cards);
        let controller = unit.controller();
        let amount = |side| (100 * i32::from(self.unit_cost(self.leader(side), cards))).min(500);
        let mut sum = 0;
        if self.leader_kind_aura(cell, controller, card.kind, 10, cards) {
            sum += amount(controller);
        }
        // N-player duels (a port addition): each enemy leader whose window
        // holds the cell subtracts its amount.
        for opponent in self.enemy_leaders(controller) {
            if self.in_window(opponent, cell, cards) && self.leader_has(opponent, 14, cards) {
                let leader = cards
                    .card(self.leader(opponent).card)
                    .expect("validated card");
                if queries::weakens(i32::from(leader.kind), card.kind & 0x1F) {
                    sum -= amount(opponent);
                }
            }
        }
        sum
    }

    /// Not an original command: the online profile's forfeit (a player who
    /// concedes, or whose reconnect grace or turn-timeout allowance ran out;
    /// ONLINE_PLAN.md case 4). Unlike [`DuelCommand::Surrender`] it is
    /// accepted from either side at any time (the server decides it, not
    /// the acting side's input), and it skips the continuous-effect guard
    /// so a duel stopped at an unsupported interaction can still end. The
    /// result is recorded as a surrender by `side`. A port addition: only
    /// with the `online` feature.
    #[cfg(feature = "online")]
    pub fn forfeit(&self, side: u8) -> Result<(Self, Vec<DuelEvent>)> {
        if self.outcome.is_some() {
            return Err(DuelError::Finished);
        }
        if side > 1 || self.multi().is_some() {
            return Err(DuelError::InvalidCommand("no such side"));
        }
        let mut next = self.clone();
        let mut events = Vec::new();
        next.pending = None;
        next.pending_summon = None;
        next.finish(Some(side ^ 1), EndReason::Surrender, &mut events);
        Ok((next, events))
    }

    /// [`Duel::forfeit`] for any duel, N-player ones included (a port
    /// addition): there only `side` leaves, and when it was acting the
    /// next living side's turn starts.
    #[cfg(feature = "online")]
    pub fn forfeit_with(&self, side: u8, cards: &impl CardData) -> Result<(Self, Vec<DuelEvent>)> {
        if self.multi().is_none() {
            return self.forfeit(side);
        }
        if self.outcome.is_some() {
            return Err(DuelError::Finished);
        }
        if side >= self.side_count() || self.is_out(side) {
            return Err(DuelError::InvalidCommand("no such side"));
        }
        let mut next = self.clone();
        let mut events = Vec::new();
        next.multi_surrender(side, cards, &mut events)?;
        Ok((next, events))
    }

    /// Apply one command for `side`. On error the state is unchanged.
    pub fn apply(
        &self,
        side: u8,
        command: &DuelCommand,
        cards: &impl CardData,
    ) -> Result<(Self, Vec<DuelEvent>)> {
        if self.outcome.is_some() {
            return Err(DuelError::Finished);
        }
        if side != self.active {
            return Err(DuelError::NotYourTurn);
        }
        let mut next = self.clone();
        let mut events = Vec::new();
        if let DuelCommand::Target { side: target } = command {
            // N-player duels (a port addition): no rules state but the
            // target changes, at any point of the own turn.
            next.choose_target(side, *target, &mut events)?;
            return Ok((next, events));
        }
        next.guard_continuous_effects(cards)?;
        let lp_before: Vec<u16> = next.sides.iter().map(|s| s.lp).collect();
        // States 0x30 / 0x2D poll the pad for the selection only.
        if let Some(pending) = next.pending.take() {
            let DuelCommand::Pick { pick } = command else {
                return Err(DuelError::InvalidCommand("answer the open selection first"));
            };
            next.resolve_pick(pending, pick, cards, &mut events)?;
            next.after_command(&lp_before, cards, &mut events)?;
            return Ok((next, events));
        }
        match command {
            DuelCommand::Pick { .. } => {
                return Err(DuelError::InvalidCommand("no selection is open"));
            }
            DuelCommand::Target { .. } => unreachable!("handled above"),
            DuelCommand::Surrender if next.multi().is_some() => {
                next.multi_surrender(side, cards, &mut events)?;
                return Ok((next, events));
            }
            DuelCommand::Surrender => {
                next.finish(Some(side ^ 1), EndReason::Surrender, &mut events);
                return Ok((next, events));
            }
            DuelCommand::OpenSummon { cell } => next.open_summon(*cell, cards, &mut events)?,
            DuelCommand::ChooseSummon { hand } => next.choose_summon(hand, cards, &mut events)?,
            DuelCommand::CancelSummon => {
                if next.pending_summon.take().is_none() {
                    return Err(DuelError::InvalidCommand("no summon is open"));
                }
            }
            DuelCommand::Move {
                unit,
                path,
                face_up,
                defence,
            } => {
                if next.pending_summon.is_some() {
                    return Err(DuelError::InvalidCommand(
                        "finish or cancel the summon first",
                    ));
                }
                next.move_unit(*unit, path, *face_up, *defence, cards, &mut events)?
            }
            DuelCommand::EndTurn => {
                if next.pending_summon.is_some() {
                    return Err(DuelError::InvalidCommand(
                        "finish or cancel the summon first",
                    ));
                }
                next.end_turn(cards, &mut events)?
            }
        }
        next.after_command(&lp_before, cards, &mut events)?;
        Ok((next, events))
    }

    /// The life-point check after a command: the original's (either side at
    /// 0 ends the duel), or the N-player knockouts (`duel/multi.rs`).
    fn after_command(&mut self, lp_before: &[u16], cards: &impl CardData, events: &mut Vec<DuelEvent>) -> Result<()> {
        if self.multi().is_some() {
            return self.multi_after(lp_before, cards, events);
        }
        self.check_life_points(events);
        Ok(())
    }

    fn guard_continuous_effects(&self, cards: &impl CardData) -> Result<()> {
        for u in &self.units {
            let card = cards.card(u.card).expect("validated card");
            if !card.is_monster() {
                // Set cards stay passive until moved, revealed, attacked or
                // triggered; those paths refuse explicitly below. Trap
                // searches ignore the face (0x24CC30, 0x26C4B0), so a trap a
                // limited-range trap exposed keeps working face-up. A magic
                // or ritual card revealed without activating (Patrol Robo,
                // event 0xD mode 4) does nothing until its controller acts
                // with it: state 0x38 then activates it like a card revealed
                // at selection (0x253D60 phase 0 reads +0x57 bit 2 only;
                // TRAPS-0x23-0x31-FAKE-TRAP-FIELD.md §4, emu M1-M8).
                if u.face_up
                    && !matches!(
                        card.class(),
                        class::MAGIC
                            | class::POWER_UP
                            | class::NEAR_TRAP
                            | class::FAR_TRAP
                            | class::RITUAL
                    )
                {
                    return Err(unsupported(format!("face-up card {} on the field", u.card)));
                }
                if card.class() == class::FAR_TRAP
                    && !card
                        .card_effect_type()
                        .is_some_and(|t| trap_type::HOOKED.contains(&t))
                {
                    return Err(unsupported(format!(
                        "trap card {} (its trigger is not traced)",
                        u.card
                    )));
                }
                continue;
            }
            if u.slot != 0
                && u.committed_face_up
                && u.defence
                && u.committed_defence
                && card.entries[2].is_some()
                && aura(card).is_none()
                // 0x3B: the LP-change modifiers read by 0x268A10 (F02.md;
                // `lp_modifier`), consulted by every LP change the engine
                // makes. 0x4E: the turn-start boost (Pumpking 108), run by
                // `turn_start_effects` as 0x4D is.
                && !card.entries[2].is_some_and(|[w, _]| {
                    // 0x56: the star income of `catabolism` (0x269450).
                    // 0xE: Temple of Skulls 113, read by `negation_chain`.
                    matches!(
                        w >> 8,
                        0x39 | 0xC | 0x4D | 0x3B | 0x4E | 6 | 0xD | 0x34 | 0xF | 0x56 | 0xE
                            // 0x47 Ancient Jar 627, 0x55 Timeater 678: the
                            // countdowns and round counter (`continuous_count`).
                            | 0x47 | 0x55
                            // 0x53 Souleater 673 (`turn_start_effects`).
                            | 0x53
                            // 0x52 Shapesnatch 675: no rules code compares
                            // an entry-2 type with 0x52 (only computer and
                            // presentation tables at 0x1768B0 / 0x178E60)
                            // [F REFUSALS-SWEEP.md §deck 0x30]: no effect.
                            | 0x52
                            // 0x58 Moisture Creature 682: its only rules
                            // reader is 0x273070's suppression (`leader_has`).
                            | 0x58
                            // 0x54 Carat Idol 676 (`turn_start_effects`).
                            | 0x54
                            // 0x51: a possessed monster (`flip_possess`).
                            | 0x51
                    )
                        || (w >> 8 == 0xB && w & 0x3F == 0)
                        // 0x17 (Binding Chain 386): the +0x4A binds of
                        // white_rules.rs, for the filters 0x24E360 knows.
                        || (w >> 8 == 0x17
                            && (1..=3).contains(&((w & 0xFF) >> 6))
                            && select_filter(w & 0x3F, card).is_ok())
                })
            {
                return Err(unsupported(format!("continuous effect of card {}", u.card)));
            }
        }
        Ok(())
    }

    fn open_summon(
        &mut self,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let s = self.active;
        if self.pending_summon.is_some() {
            return Err(DuelError::InvalidCommand("a summon is already open"));
        }
        let side = &self.sides[usize::from(s)];
        if !side.can_summon {
            return Err(DuelError::Illegal("already summoned this turn"));
        }
        if side.deck.len() + side.hand.len() == 0 {
            // A deck holding only stranded cards draws 999s: the hand stays
            // empty either way.
            return Err(DuelError::Illegal("no cards left to summon"));
        }
        if !self.summon_area(s, cell) {
            return Err(DuelError::Illegal("not a summon area"));
        }
        // 0x1D8F90: refill the hand to min(5, hand + deck) from the deck
        // front; an armed Destiny Draw makes the last card the special one.
        let drawn = match self.destiny_refill(s, cards, events)? {
            Some(drawn) => drawn,
            None => {
                let mut drawn = Vec::new();
                while self.sides[usize::from(s)].hand.len() < HAND_SIZE
                    && !self.sides[usize::from(s)].deck.is_empty()
                {
                    let card = self.draw_front(s);
                    self.sides[usize::from(s)].hand.push(card);
                    drawn.push(card);
                }
                drawn
            }
        };
        if !drawn.is_empty() {
            events.push(DuelEvent::Drew {
                side: s,
                cards: drawn,
            });
        }
        // 0x1D8F90 → 0x1D9C70(side, slot, card): group-2 slots 0..=5 are
        // (re)created at the side's cursor, which is on `cell`: slot 0 as
        // card 999, slot i + 1 as hand card i, the rest as 999 fillers.
        let hand = self.sides[usize::from(s)].hand.clone();
        for (slot, t) in self.temp_units[..6].iter_mut().enumerate() {
            let card = slot
                .checked_sub(1)
                .and_then(|i| hand.get(i).copied())
                .unwrap_or(999);
            *t = Some(TempUnit { cell, card });
        }
        self.pending_summon = Some(cell);
        Ok(())
    }

    /// 0x273160(side): the ability-8 summon discount by the leader card's
    /// kind (byte 0 & 0x1F) and level L = 0x1E1B70(side, 0, 0): 0 without
    /// the ability (0x273070, suppressed by Moisture Creature); kind 0x14 →
    /// 1; 0xD / 4 → 2 at L ≥ 6 else 1; 0xE / 0xC / 6 / 0 → 2 at L ≥ 7 else
    /// 1; other kinds 0.
    fn leader_discount(&self, side: u8, cards: &impl CardData) -> u8 {
        if !self.leader_has(side, 8, cards) {
            return 0;
        }
        let leader = self.leader(side);
        let Some(info) = cards.card(leader.card) else {
            return 0;
        };
        let level = self.unit_cost(leader, cards);
        match info.kind & 0x1F {
            0x14 => 1,
            0xD | 4 => 1 + u8::from(level >= 6),
            0xE | 0xC | 6 | 0 => 1 + u8::from(level >= 7),
            _ => 0,
        }
    }

    /// 0x2703B0(side, card): the discount stored in a new record's +0x48:
    /// [`Self::leader_discount`] for a monster (class 0) whose kind matches
    /// the leader card's, else 0 (also for a card without a record).
    fn summon_discount(&self, side: u8, card: u16, cards: &impl CardData) -> u8 {
        let Some(info) = cards.card(card) else {
            return 0;
        };
        let Some(leader) = cards.card(self.leader(side).card) else {
            return 0;
        };
        if info.kind >> 5 != 0 || info.kind & 0x1F != leader.kind & 0x1F {
            return 0;
        }
        self.leader_discount(side, cards)
    }

    /// 0x1E1BD0(record, 0) = clamp(level − record +0x48, 0, 12) for a unit
    /// on the field (0x1E1B70(group, slot, 0)).
    fn unit_cost(&self, unit: &DuelUnit, cards: &impl CardData) -> u8 {
        let level = cards.card(unit.card).map_or(0, |c| c.level);
        level.saturating_sub(unit.discount).min(MAX_STARS)
    }

    /// Cost 0x1E1BD0 = clamp(level − discount, 0, 12) of a hand record,
    /// whose +0x48 is 0x2703B0(side, card) (0x1D9C70 at the refill): the
    /// discount is non-zero only for a monster of the leader's kind when
    /// the leader has ability #8 (0x273160 → 0x273070(8)).
    /// 0x1E1BD0: the stars a card costs to summon for side `s`.
    pub fn summon_cost(&self, s: u8, card: &CardInfo, cards: &impl CardData) -> u8 {
        card.level
            .saturating_sub(self.summon_discount(s, card.id, cards))
            .min(MAX_STARS)
    }

    /// States 0x43–0x50: the chain is the own unit on the chosen cell (if
    /// any) followed by the marked hand cards in marking order; each step
    /// fuses, transforms, equips or drops the older element (FUSION.md §3).
    fn choose_summon(
        &mut self,
        hand: &[usize],
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let s = self.active;
        let cell = self
            .pending_summon
            .ok_or(DuelError::InvalidCommand("no summon is open"))?;
        let side = &self.sides[usize::from(s)];
        if hand.is_empty()
            || hand.iter().any(|&i| i >= side.hand.len())
            || (1..hand.len()).any(|i| hand[..i].contains(&hand[i]))
        {
            return Err(DuelError::InvalidCommand("invalid hand selection"));
        }
        let ids: Vec<u16> = hand.iter().map(|&i| side.hand[i]).collect();
        // 0x270280: the highest cost among the marked cards (the field unit
        // adds nothing); non-monsters have level 0.
        let cost = ids
            .iter()
            .map(|&id| self.summon_cost(s, cards.card(id).expect("validated card"), cards))
            .max()
            .unwrap_or(0);
        if cost > side.stars {
            return Err(DuelError::Illegal("not enough summoning power"));
        }
        let field = self
            .unit_at(cell)
            .filter(|u| u.side == s && u.slot != 0)
            .cloned();
        // Run the chain on a scratch copy first so refusals change nothing.
        let mut chain: Vec<ChainItem> = field.iter().cloned().map(ChainItem::Field).collect();
        chain.extend(ids.iter().map(|&id| ChainItem::Hand(id)));
        let mut steps = Vec::new();
        let mut acc = chain[0].clone();
        for next in chain.into_iter().skip(1) {
            let step = self.chain_step(&acc, &next, cards)?;
            acc = match &step {
                ChainStep::Fuse(result) => ChainItem::Hand(*result),
                ChainStep::Equip { .. } => {
                    if cards.card(acc.card()).expect("card").is_monster() {
                        acc.clone()
                    } else {
                        next.clone()
                    }
                }
                ChainStep::Fail => next.clone(),
            };
            steps.push((step, acc.card()));
        }
        // Commit: stars, hand, the lifted field unit, the chain results.
        let side = &mut self.sides[usize::from(s)];
        side.stars -= cost;
        side.can_summon = false;
        let mut sorted: Vec<usize> = hand.to_vec();
        sorted.sort_unstable_by(|a, b| b.cmp(a));
        for i in sorted {
            side.hand.remove(i);
        }
        if let Some(f) = &field {
            self.units.retain(|u| u.id != f.id);
        }
        let mut element = field
            .map(ChainItem::Field)
            .unwrap_or(ChainItem::Hand(ids[0]));
        let mut bonus = 0i16;
        // Category-1 entry writes on the surviving element's record
        // (0x249F3C); the final record is copied into the unit (state 0x4F).
        let mut engraved: Option<[Option<[u32; 2]>; 5]> = None;
        let mut rest = ids
            .iter()
            .skip(usize::from(matches!(element, ChainItem::Hand(_))));
        // Group-2 record of each chain element (0x1D9540: the field unit in
        // slot 0, hand card i in slot i + 1); the chain handlers reset a
        // consumed record (0x250D50 → 0x1E0E30) and 0x1D9F50 re-creates a
        // fusion result in the older element's slot at the cursor.
        let mut elem_slot = if let ChainItem::Field(f) = &element {
            // State 0x44 (0x1D9540) copies the lifted unit's record into
            // slot 0 (its cell is the summon cell).
            self.temp_units[0] = Some(TempUnit { cell, card: f.card });
            0
        } else {
            hand[0] + 1
        };
        let mut rest_slots = hand
            .iter()
            .map(|&i| i + 1)
            .skip(usize::from(matches!(element, ChainItem::Hand(_))));
        let dropped_from = self.sides[usize::from(s)].graveyard.len();
        for (step, survivor) in steps {
            let next = *rest.next().expect("chain element");
            let next_slot = rest_slots.next().expect("chain element");
            let graveyard = &mut self.sides[usize::from(s)].graveyard;
            match step {
                ChainStep::Fuse(result) => {
                    graveyard.extend([element.card(), next]);
                    events.push(DuelEvent::Fused {
                        cards: [element.card(), next],
                        result,
                    });
                    element = ChainItem::Hand(result);
                    bonus = 0;
                    engraved = None;
                    self.temp_units[next_slot] = None;
                    self.temp_units[elem_slot] = Some(TempUnit { cell, card: result });
                }
                ChainStep::Equip { value, special } => {
                    let (monster, power_up) = if survivor == element.card() {
                        (element.clone(), next)
                    } else {
                        (ChainItem::Hand(next), element.card())
                    };
                    graveyard.push(power_up);
                    if monster.card() != element.card() {
                        bonus = 0;
                        engraved = None;
                        self.temp_units[elem_slot] = None;
                        elem_slot = next_slot;
                    } else {
                        self.temp_units[next_slot] = None;
                    }
                    element = monster;
                    // 0x249D80 on the element's record, with a3 = the
                    // active side for Riryoku (0x272D00) and 0 otherwise.
                    let mut drained = 0;
                    let value = match special {
                        EquipSpecial::Plain => value,
                        EquipSpecial::Riryoku => {
                            // Computed when the step runs, so a second
                            // Riryoku halves the already drained LP (emu E).
                            drained = self.riryoku_amount(s, cards);
                            i16::try_from(drained).unwrap_or(i16::MAX)
                        }
                        EquipSpecial::Entries(v) => {
                            let base = engraved.unwrap_or_else(|| match &element {
                                ChainItem::Field(u) => self.unit_card(u, cards).entries,
                                ChainItem::Hand(id) => {
                                    cards.card(*id).expect("validated card").entries
                                }
                            });
                            if let Some(e) = Self::engrave(base, v) {
                                engraved = Some(e);
                            }
                            0
                        }
                    };
                    events.push(DuelEvent::Equipped {
                        side: s,
                        monster: element.card(),
                        power_up,
                        value,
                    });
                    events.push(DuelEvent::Dropped {
                        side: s,
                        card: power_up,
                    });
                    if special == EquipSpecial::Riryoku {
                        // 0x1E0710(opponent, 1, amount): clamped at 0.
                        self.damage(self.opp(s), drained, events);
                    }
                    bonus = bonus.saturating_add(value);
                }
                ChainStep::Fail => {
                    graveyard.push(element.card());
                    events.push(DuelEvent::Dropped {
                        side: s,
                        card: element.card(),
                    });
                    element = ChainItem::Hand(next);
                    bonus = 0;
                    engraved = None;
                    self.temp_units[elem_slot] = None;
                    elem_slot = next_slot;
                }
            }
        }
        // 0x250D50 (from 0x2717A0, 0x271960, 0x271B90): each chain card sent
        // to the graveyard costs 4 on the active side's block.
        let dropped = self.sides[usize::from(s)].graveyard[dropped_from..].to_vec();
        for card in dropped {
            self.record_experience(s, card, -4);
        }
        self.pending_summon = None;
        let card = cards.card(element.card()).expect("validated card").clone();
        // State 0x4D: no free slot of the result's kind → graveyard.
        let used: Vec<u8> = self
            .units
            .iter()
            .filter(|u| u.side == s && u.slot != 0)
            .map(|u| u.slot)
            .collect();
        let mut slots = if card.is_monster() {
            1..=MONSTER_SLOTS as u8
        } else {
            OTHER_SLOTS
        };
        let Some(slot) = slots.find(|n| !used.contains(n)) else {
            self.sides[usize::from(s)].graveyard.push(card.id);
            self.record_experience(s, card.id, -4); // 0x250D50 (interpreter)
            self.temp_units[elem_slot] = None; // 0x272F60 → 0x250D50 → 0x1E0E30
            events.push(DuelEvent::Dropped {
                side: s,
                card: card.id,
            });
            return Ok(());
        };
        let side = &mut self.sides[usize::from(s)];
        let order = side.summons;
        side.summons = side.summons.saturating_add(1);
        // A surviving field unit keeps its record (flags, changes); anything
        // else is created fresh: face-down, attack position, not acted.
        let mut unit = match element {
            ChainItem::Field(u) => u,
            ChainItem::Hand(id) => {
                let discount = self.summon_discount(s, id, cards);
                let id_unit = UnitId(self.next_unit);
                self.next_unit += 1;
                DuelUnit {
                    id: id_unit,
                    card: id,
                    side: s,
                    slot,
                    position: cell,
                    face_up: false,
                    defence: false,
                    committed_face_up: false,
                    committed_defence: false,
                    acted: false,
                    spellbind: 0,
                    summon_order: order,
                    attack_bonus: 0,
                    defense_bonus: 0,
                    spellbind_fresh: false,
                    evo_counter: 0,
                    stolen: false,
                    discount,
                    stolen_by: None,
                }
            }
        };
        unit.slot = slot;
        unit.summon_order = order;
        unit.position = cell;
        unit.attack_bonus = unit.attack_bonus.saturating_add(bonus);
        unit.defense_bonus = unit.defense_bonus.saturating_add(bonus);
        if let Some(e) = engraved {
            self.set_record_entries(unit.id, e);
        }
        events.push(DuelEvent::Summoned {
            unit: unit.id,
            card: unit.card,
            at: cell,
            cost,
        });
        let summoned = unit.id;
        self.units.push(unit);
        self.crush_sweep(cards, events)?;
        // 0x250E40 (after its Crush test): #16 / #15 / #13 of the other
        // side's leader for a unit summoned into its window.
        self.window_summon_abilities(summoned, cards, events)
    }

    /// 0x26FF40: one chain step, accumulator `a` (older) with `b`.
    fn chain_step(&self, a: &ChainItem, b: &ChainItem, cards: &impl CardData) -> Result<ChainStep> {
        // A lifted field unit's chain record is a byte copy of its unit
        // record (0x1D9540), so it keeps rewritten kinds and entries.
        let info = |x: &ChainItem| match x {
            ChainItem::Field(u) => self.unit_card(u, cards),
            ChainItem::Hand(id) => std::borrow::Cow::Borrowed(cards.card(*id).expect("validated card")),
        };
        let (ca, cb) = (info(a), info(b));
        let (ca, cb) = (&*ca, &*cb);
        match (ca.class(), cb.class()) {
            (class::MONSTER, class::MONSTER) => match cards.fusion(ca.id, cb.id) {
                None => Err(unsupported("fusion without an imported fusion table")),
                Some(Some(result)) => Ok(ChainStep::Fuse(result)),
                Some(None) => Ok(ChainStep::Fail),
            },
            (class::MONSTER, class::POWER_UP) | (class::POWER_UP, class::MONSTER) => {
                let (monster, power_up) = if ca.is_monster() { (ca, cb) } else { (cb, ca) };
                if !power_up_fits(monster, power_up.id) {
                    return Ok(ChainStep::Fail);
                }
                // [F/emu] 0x272660..0x2726AC: only an opposing Goblin
                // Fan or either group's Reverse Trap opens the trap path.
                // Royal Decree is queried only AFTER one of those is found;
                // alone it leaves the equip and its own flags unchanged.
                // Actual trap reactions still need the staged handler.
                if self
                    .far_trap_in(&self.foes(self.active), trap_type::GOBLIN_FAN, cards)
                    .is_some()
                    || self.living_sides().into_iter().any(|g| self.far_trap(g, 0x2B, cards).is_some())
                {
                    return Err(unsupported("equip negation traps in a summon list"));
                }
                let data = power_up.power_up.ok_or_else(|| {
                    unsupported(format!("power-up {} without imported data", power_up.id))
                })?;
                match data.category {
                    // 0x249E40: value + the type-0x34 modifier sum
                    // (0x24A7C0, Maha Vailo; white_rules.rs).
                    0 if data.value > 0 => Ok(ChainStep::Equip {
                        value: i16::try_from(i32::from(data.value) + self.equip_modifier(cards))
                            .unwrap_or(i16::MAX),
                        special: EquipSpecial::Plain,
                    }),
                    // Riryoku: 0x24AB20 = 0x219 → handler 0x272560 →
                    // 0x249D80 value-0 branch (FLIP-112-POWERUPS-789-791 §3).
                    0 => Ok(ChainStep::Equip {
                        value: 0,
                        special: EquipSpecial::Riryoku,
                    }),
                    // Category 1 (0x249F3C): entry writes, no stat change.
                    1 => Ok(ChainStep::Equip {
                        value: 0,
                        special: EquipSpecial::Entries(data.value),
                    }),
                    4 => transform(monster.id, power_up.id)
                        .map(ChainStep::Fuse)
                        .ok_or_else(|| unsupported(format!("power-up {}", power_up.id))),
                    _ => Err(unsupported(format!(
                        "power-up {} (category {})",
                        power_up.id, data.category
                    ))),
                }
            }
            _ => Ok(ChainStep::Fail),
        }
    }

    /// Effective ATK/DEF (0x1E1570/0x1E1650 plus terrain 0x24F7D0 and the
    /// leader auras 0x273420 of abilities #10/#14), measured at `cell`.
    fn effective(
        &self,
        unit: &DuelUnit,
        cell: GridPos,
        attack: bool,
        cards: &impl CardData,
    ) -> u16 {
        self.effective_sum(unit, cell, attack, cards)
            .clamp(0, i32::from(MAX_LP)) as u16
    }

    /// `effective` before its final clamp, so battle deltas (0x248130) can
    /// join the same clamp: 0x252300/0x252460 base + permanent change,
    /// clamped, plus the 0x267A00 modifiers.
    fn effective_sum(
        &self,
        unit: &DuelUnit,
        cell: GridPos,
        attack: bool,
        cards: &impl CardData,
    ) -> i32 {
        let card = self.unit_card(unit, cards);
        let printed = i32::from(if attack { card.attack } else { card.defense });
        let change = i32::from(if attack {
            unit.attack_bonus
        } else {
            unit.defense_bonus
        });
        let base = (printed + change).clamp(0, i32::from(MAX_LP));
        let terrain = if unit.slot == 0 {
            0
        } else {
            terrain_affinity(self.terrain_at(cell), &card) * TERRAIN_FAVOUR
        };
        // 0x273420 comes last, for every unit (leaders and non-monsters too).
        let mut modifiers =
            terrain + self.aura_total(unit, attack, cards) + self.leader_aura(unit, cell, cards);
        // 0x267A58-0x267B44: card 0x2A3 takes a leader's printed stat instead
        // of its own (its own leader's in defence, the other leader's
        // otherwise); its own value is capped at 9999 only.
        if unit.card == 0x2A3 {
            let own = (printed + change).min(i32::from(MAX_LP));
            let side = if unit.defence {
                unit.side
            } else {
                self.opp(unit.side)
            };
            let leader = cards.card(self.leader(side).card).expect("validated card");
            let raw = i32::from(if attack {
                leader.attack
            } else {
                leader.defense
            })
            .clamp(0, i32::from(MAX_LP));
            modifiers += raw - own;
        }
        base + modifiers
    }

    /// 0x267A00 auras: every monster-slot source that is face-up in
    /// committed defence with an entry-2 stat aura adds its value to matching
    /// non-leader monsters (itself included; stacking; not permanent).
    fn aura_total(&self, unit: &DuelUnit, attack: bool, cards: &impl CardData) -> i32 {
        let card = self.unit_card(unit, cards);
        if unit.slot == 0 || !card.is_monster() {
            return 0;
        }
        self.units
            .iter()
            .filter(|src| {
                (1..=MONSTER_SLOTS as u8).contains(&src.slot)
                    && src.defence
                    && src.committed_face_up
                    && src.committed_defence
            })
            .filter_map(|src| {
                let a = aura(cards.card(src.card)?)?;
                // 0x24F6A0: mode 1 same side, 2 the other, 3 both, 0 none.
                let side_ok = match a.mode {
                    1 => src.controller() == unit.controller(),
                    2 => self.foe(src.controller(), unit.controller()),
                    3 => true,
                    _ => false,
                };
                (side_ok && !(attack && a.def_only) && aura_filter_matches(a.filter, &card))
                    .then_some(a.value)
            })
            .sum()
    }

    /// Step allowance 0x247E10 at the unit's own cell.
    fn allowance(&self, unit: &DuelUnit, cards: &impl CardData) -> Result<usize> {
        let card = self.unit_card(unit, cards);
        if unit.slot == 0 {
            return Ok(if self.leader_has(unit.side, 4, cards) {
                2
            } else {
                1
            });
        }
        // Entry-1 type 0x4F (Larva/Pupa of Moth): no steps, face-up or down.
        if card.entries[1].is_some_and(|[w, _]| w >> 8 == 0x4F) {
            return Ok(0);
        }
        // Entry-1 types read by 0x247E10: 8/9/0xA set the wall flag, 6 fixes
        // the steps (not traced), 0x4F above. Types 7 (limited-range trap
        // immunity, 0x24CE34), 0x42 (LP loss per entered cell, 0x2513F4) and
        // 2 (terrain on entry, 0x251498) leave the allowance alone; see
        // `near_trap` and `on_entry`.
        if card.effects[1]
            && !card.entries[1].is_some_and(|[w, _]| matches!(w >> 8, 2 | 7..=0xA | 0x42))
        {
            return Err(unsupported(format!(
                "movement effect of card {}",
                unit.card
            )));
        }
        if !unit.face_up {
            return Ok(1);
        }
        // 0x2480EC, the last write: ability #12 sets the steps to 2 for a
        // face-up unit of the owner's leader's kind inside the leader window
        // (ABILITY-12.md §2; the owner's leader, never the controller's).
        if self.leader_kind_aura(unit.position, unit.side, card.kind, 12, cards) {
            return Ok(2);
        }
        let mut steps = if terrain_affinity(self.terrain_at(unit.position), &card) == 1 {
            2
        } else {
            1
        };
        // 0x247F58..0x2480B4: every unit of group 0 then group 1 (slots
        // 0..10) in committed face-up defence (+0x57 bits 4 and 5) with an
        // entry-2 type 6 (539 Thunder Nyan Nyan: filter 0xF, lo 2) sets
        // the steps to lo: for filter 0xF only a monster mover of kind 0xF
        // (record +0x3E), any other filter for every face-up mover; the
        // last such source wins. [F; WHITE-REFUSALS.md §18]
        let mut sources: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| u.committed_face_up && u.committed_defence)
            .collect();
        sources.sort_by_key(|u| (u.side, u.slot));
        for src in sources {
            let Some([w0, w1]) = self.unit_card(src, cards).entries[2] else {
                continue;
            };
            if w0 >> 8 != 6 {
                continue;
            }
            if w0 & 0x3F != 0xF || (card.kind == 0xF) {
                steps = (w1 & 0xFFFF) as usize;
            }
        }
        Ok(steps)
    }

    /// 0x247E10 flag 0x10: entry-1 type 8 (Dungeon Worm, Wall Shadow) lets
    /// a non-leader enter and cross empty Labyrinth cells.
    /// 0x247E10: entry-1 types 8, 9 and 0xA carry the wall flag (face-down
    /// 0x247E90–0x247EA8, face-up 0x247F30–0x247F40).
    fn has_wall_flag(&self, unit: &DuelUnit, cards: &impl CardData) -> bool {
        unit.slot != 0
            && cards
                .card(unit.card)
                .and_then(|c| c.entries[1])
                .is_some_and(|[w, _]| matches!(w >> 8, 8..=0xA))
    }

    /// Reveal in battle (steps 3 and 2). Returns whether the unit was not
    /// yet committed face-up, i.e. whether its flip effect may fire.
    fn battle_reveal(&mut self, id: UnitId, events: &mut Vec<DuelEvent>) -> bool {
        let Some(u) = self.units.iter_mut().find(|u| u.id == id) else {
            return false;
        };
        if u.slot == 0 || u.committed_face_up {
            return false;
        }
        u.face_up = true;
        u.committed_face_up = true;
        events.push(DuelEvent::Revealed {
            unit: id,
            card: u.card,
        });
        true
    }

    /// Flip effects 0x265250 (entry 3), for role 1 (battle, the opponent),
    /// role 0 (a manual flip ending a move) or role 2 (a move onto an own
    /// unit, the partner). Roles and hooks: MONSTER-EFFECTS.md §1b/§1c.
    fn flip(
        &mut self,
        id: UnitId,
        role: FlipRole,
        center: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Flip> {
        let unit = self.unit(id).expect("flipping unit").clone();
        // The record's entries (a category-1 power-up may have written e3).
        let card = self.unit_card(&unit, cards).into_owned();
        let Some([word0, word1]) = card.entries[3] else {
            return Ok(Flip::Continue);
        };
        // Filter 0x1A (the battle opponent) has no target without a battle or
        // against a leader.
        let opponent = role
            .opponent()
            .filter(|o| self.unit(*o).is_some_and(|u| u.slot != 0));
        let (ty, low) = (word0 >> 8, word0 & 0xFF);
        let (hi, lo) = ((word1 >> 16) as u16, (word1 & 0xFFFF) as u16);
        match ty {
            // Key Mace / The Drdek (event 0xD mode 5): the other units of
            // the mask take committed defence (lo 1) or attack (lo 0)
            // (white_rules.rs).
            0x23 => self.flip_positions(id, role, low, lo, cards, events)?,
            // Tongyo 470 (0x265508): the battle opponent -lo (white_rules.rs).
            0x13 => self.flip_lower_opponent(id, opponent, lo, cards, events),
            // Change Slime 570 (0x265F24): takes the battle opponent's stats.
            0x16 => self.flip_copy_stats(id, opponent, hi, cards, events)?,
            // Ooguchi 575 (0x266610): moves beside a leader; ends a battle.
            0x25 => return self.flip_teleport(id, role, hi, events),
            // Mystery Hand 310 (0x2666FC): sends a neighbouring card lo
            // beside the other side's leader.
            0x3C => self.flip_send_neighbour(id, role, lo, events)?,
            // Stat boost (event 0x1D): matching own/other/both non-leader
            // units, itself included; filter 0x25 = ATK at their cell ≤ hi,
            // filter 0x1C = card id == hi (F01.md §1.3: no kind check). The
            // role, the opponent and the cell are not read on this branch.
            0x12 => {
                let (mode, filter) = (low >> 6, low & 0x3F);
                // hi is traced as filter 0x25's ATK limit and filter 0x1C's
                // card id; Mountain Warrior's radius code 0x16 is not.
                // hi 0x16 with filter 0x1D (213 Mountain Warrior, 0x265484):
                // 0x24E360 scope 0x16 = the monsters standing on Mountain
                // (terrain 2) whose controller fits the mode (white_rules
                // notes §15). Its filters 0x34 / 0x1F are not run.
                let mountain = hi == 0x16 && filter == 0x1D;
                if !mountain
                    && !matches!(filter, 0x25 | 0x1C)
                    && (!matches!(filter, 0..=0x19 | 0x1D | 0x3B) || hi != 0)
                {
                    return Err(unsupported(format!("flip filter {filter:#x}")));
                }
                let mut matched: Vec<&DuelUnit> = self
                    .units
                    .iter()
                    .filter(|u| {
                        let c = self.unit_card(u, cards);
                        let side_ok = match mode {
                            1 => u.controller() == unit.controller(),
                            2 => self.foe(u.controller(), unit.controller()),
                            _ => true,
                        };
                        side_ok
                            && u.slot != 0
                            && match filter {
                                _ if mountain => {
                                    c.is_monster()
                                        && self.terrain_at(u.position) == terrain::MOUNTAIN
                                }
                                0x25 => {
                                    c.is_monster()
                                        && self.effective(u, u.position, true, cards) <= hi
                                }
                                0x1C => u.card == hi,
                                // 0x24EF78 (Slate Warrior 674): monsters of
                                // kind 0x14 (Immortal) [F; emu exp_slate SL1].
                                0x3B => c.is_monster() && c.kind & 0x1F == kind::IMMORTAL,
                                _ => filter_matches(filter, &c),
                            }
                    })
                    .collect();
                // The 0x1D event walks the mask g0 s1..s10 then g1 s1..s10
                // (F01.md §1.3), not the units' creation order.
                matched.sort_by_key(|u| (u.side, u.slot));
                let targets: Vec<UnitId> = matched.into_iter().map(|u| u.id).collect();
                // 0x265464: an empty mask writes no event record.
                if targets.is_empty() {
                    return Ok(Flip::Continue);
                }
                events.push(DuelEvent::Activated {
                    unit: id,
                    card: unit.card,
                    target: None,
                });
                for t in targets {
                    self.boost_capped(t, i32::from(lo), cards, events);
                }
            }
            // Nightmare Scorpion (event 0): bind the battle opponent. Any
            // other filter (0x265614) builds a target mask with
            // 0x24E360(0, 0, controller, mode, filter, 0xE): slots 1..10 of
            // both groups whose controller fits the mode; event {0, 0,
            // mask, word1 lo}, none when the mask is empty. Dragon Piper
            // clears binds; Invitation to a Dark Sleep applies a one-turn
            // bind and marks every matching card acted, including set cards.
            0x17 if low & 0x3F != 0x1A => {
                self.flip_bind_all(id, low, word1, events)?;
            }
            // Blue-Eyed Silver Zombie 112 / Mech Mole Zombie 116 / Tao the
            // Chanter 90: kind or attribute rewrite (event 0x42).
            0x36 | 0x37 => {
                return self.flip_retype(id, opponent, word0, word1, cards, events);
            }
            // Kageningen's neighbour summon (0x265E2C / event 1).
            0x38 => {
                self.flip_summon_neighbour(id, word1, cards, events)?;
            }
            0x17 => {
                if let Some(o) = opponent {
                    events.push(DuelEvent::Activated {
                        unit: id,
                        card: unit.card,
                        target: Some(o),
                    });
                    self.bind(o, word1 as u8, events);
                    if let Some(u) = self.units.iter_mut().find(|u| u.id == o) {
                        u.acted = true;
                    }
                }
            }
            // Man-Eater Bug (event 0x15): destroy the battle opponent (a Zombie
            // only when hi == 2), without its destroy effect. Other filters
            // (Hurricail 0x1E, 0x1F with a kind in hi, Barrel Dragon 0x30)
            // are not traced.
            // Barrel Dragon 502 (0x265CA4, filter 0x30, mode 3): one unit
            // drawn at random (0x2504F0: rand31 % count, the n-th bit of
            // the mask from g0 s1) among slots 1..10 of both groups, itself
            // excluded, any class and face; event 0x15 destroys it
            // (0x250C90, no e4). No candidate: nothing. In battle the result
            // is 0x201 when the opponent is drawn (emu H1 Weevil's Hunter
            // Spider, H2 an own Red-Eyes, H3 the own Nekogal). The 0x15
            // handler draws a second number for its animation (0x25051C,
            // f33): not consumed here.
            4 if low & 0x3F == 0x30 => {
                let mut pool: Vec<&DuelUnit> = self
                    .units
                    .iter()
                    .filter(|u| u.slot != 0 && u.id != id)
                    .collect();
                pool.sort_by_key(|u| (u.side, u.slot));
                let pool: Vec<UnitId> = pool.into_iter().map(|u| u.id).collect();
                if pool.is_empty() {
                    return Ok(Flip::Continue);
                }
                let pick = pool[self.rand31() as usize % pool.len()];
                events.push(DuelEvent::Activated {
                    unit: id,
                    card: unit.card,
                    target: Some(pick),
                });
                self.remove_by_effect(pick, cards, events)?;
                if opponent == Some(pick) {
                    return Ok(Flip::OpponentDestroyed);
                }
            }
            // 0x265D0C (filters other than 0x1A / 0x30): the 0x24E360 mask
            // around the unit (hi = the scope; Man Eater's Forest, Flame
            // Ghost's Sea, Hurricail's distance 1) or, with hi 0, of the
            // whole board, destroyed (white_rules.rs).
            4 if low & 0x3F != 0x1A => {
                return self.flip_destroy_on_terrain(id, role, low, hi, cards, events);
            }
            4 => {
                let fits = opponent.is_some_and(|o| {
                    hi != 2 || self.unit_card(self.unit(o).unwrap(), cards).kind & 0x1F == kind::ZOMBIE
                });
                if let (true, Some(o)) = (fits, opponent) {
                    events.push(DuelEvent::Activated {
                        unit: id,
                        card: unit.card,
                        target: Some(o),
                    });
                    // 0x25D770 -> 0x260F90 -> 0x250C90.
                    self.remove_by_effect(o, cards, events)?;
                    return Ok(Flip::OpponentDestroyed);
                }
            }
            // Larva/Pupa of Moth (0x2667D0, event 0x43): start the entry-2
            // countdown at hi + 1, whatever the role; the battle goes on.
            0xC => {
                events.push(DuelEvent::Activated {
                    unit: id,
                    card: unit.card,
                    target: None,
                });
                if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
                    u.evo_counter = (hi as u8).saturating_add(1);
                }
            }
            // Yashinoki 654 (0x265664; mode 1, lo 500): record {8, 0,
            // 0x24F740(ctrl, mode), word1}: event 8, LP + lo for the
            // controller (mode 1), its opponent (2); any role. [F; emu YS1]
            0xD => {
                let (mode, ctrl) = (low >> 6, unit.controller());
                let side = match mode {
                    1 => ctrl,
                    2 => self.opp(ctrl),
                    _ => {
                        return Err(unsupported(format!(
                            "flip effect of card {} (type 0xd mode {mode})",
                            unit.card
                        )))
                    }
                };
                events.push(DuelEvent::Activated {
                    unit: id,
                    card: unit.card,
                    target: None,
                });
                self.lp_gain(side, lo, cards, events);
            }
            // Yamadron (event 2): field change around `center`.
            2 => {
                events.push(DuelEvent::Activated {
                    unit: id,
                    card: unit.card,
                    target: None,
                });
                self.change_terrain(center, hi as u8, lo as u8);
            }
            // Swordstalker (146: 0x266010, filter 0x2F, hi 0x1D): itself
            // gains lo × the monsters (class 0) in its controller's
            // graveyard (0x252260(0x251C80(g, s), 0)) through event {0x1D,
            // 1, own mask, amount}, permanent ATK/DEF as 0x25EF60; no event
            // when the amount is 0 (0x2661E0). Run as a manual flip with 9
            // monsters in the graveyard: +900/+900. Filters 0x22/0x38/0x2E
            // and hi 0x1C are read but not run.
            // Machine King 514 (0x26610C, filter 0x2E, hi 0xE, lo 100):
            // itself gains lo × the monsters of kind hi in slots 1..10 of
            // both groups (0x24E360(0, 0, ctrl, 3, hi, 0xE): any face,
            // itself included) through event {0x1D, 1, own mask, amount};
            // the record is written even for 0 (never: it counts itself)
            // [emu G1 +300 with a face-up enemy and a face-down own
            // Machine, G2 +100 alone, G3 +200 with a face-down enemy].
            0x35 if low & 0x3F == 0x2E => {
                let count = self
                    .units
                    .iter()
                    .filter(|u| {
                        u.slot != 0 && {
                            let c = self.unit_card(u, cards);
                            c.is_monster() && u16::from(c.kind & 0x1F) == hi
                        }
                    })
                    .count() as i32;
                events.push(DuelEvent::Activated {
                    unit: id,
                    card: unit.card,
                    target: None,
                });
                self.boost_capped(id, i32::from(lo) * count, cards, events);
            }
            // Arsenal Bug 429 (0x2660BC: filter 0x38, hi 9, lo 1500) /
            // filter 0x22 (0x266070): record {0x1D, 1, own mask, lo}, the
            // event only when the controller's leader (0x1E1980(ctrl, 0):
            // its +0x3E kind byte; 0x1E22B0(ctrl, 0): its card id for 0x22)
            // matches hi [F; emu REFUSALS-SWEEP.md AB0 (a Fairy leader:
            // nothing), AB1 (an Insect leader: +1500/+1500)].
            0x35 if matches!(low & 0x3F, 0x38 | 0x22) => {
                let leader = self.leader(unit.controller()).clone();
                let fits = if low & 0x3F == 0x38 {
                    u16::from(self.unit_card(&leader, cards).kind & 0x1F) == hi
                } else {
                    leader.card == hi
                };
                if fits {
                    events.push(DuelEvent::Activated {
                        unit: id,
                        card: unit.card,
                        target: None,
                    });
                    self.boost_capped(id, i32::from(lo), cards, events);
                }
            }
            0x35 => {
                let filter = low & 0x3F;
                if filter != 0x2F || !matches!(hi, 0x1C | 0x1D) {
                    return Err(unsupported(format!("flip effect of card {}", unit.card)));
                }
                let monsters = if hi == 0x1C {
                    // 87 Dark Magician Girl (0x2661AC): 0x27F660(group, 3,
                    // id) counts card 0x3C (Dark Magician) and card 0x57
                    // (itself) in both graveyards (each count & 0xFF).
                    // [F; emu DMG1]
                    [0x3C_u16, 0x57]
                        .iter()
                        .map(|&card| {
                            (self
                                .sides
                                .iter()
                                .flat_map(|s| s.graveyard.iter())
                                .filter(|&&c| c == card)
                                .count()
                                & 0xFF) as i32
                        })
                        .sum()
                } else {
                    self.sides[usize::from(unit.controller())]
                        .graveyard
                        .iter()
                        .filter(|&&c| cards.card(c).is_some_and(CardInfo::is_monster))
                        .count() as i32
                };
                let amount = i32::from(lo) * monsters;
                if amount <= 0 {
                    return Ok(Flip::Continue);
                }
                events.push(DuelEvent::Activated {
                    unit: id,
                    card: unit.card,
                    target: None,
                });
                self.boost_capped(id, amount, cards, events);
            }
            // Patrol Robo 523 (0x265B74: mode 2, filter 0x1E, lo 1): the
            // 0x24E360(0, 0, ctrl, mode, filter, 0xE) mask minus itself,
            // event {0xD, 4, mask, lo} → 0x25B980 mode 4: 0x252FF0(g, s, 1)
            // on every target = face-up and committed face-up, nothing
            // else; nothing with an empty mask [emu I1: the enemy's set
            // ritual, power-up and trap turned 0x29 → 0x3D / 0x00 → 0x14
            // (+bit 0 by the commit of that run), own set card and monsters
            // untouched; I2 empty]. Filter 0x2D (0x250A70) is not run. A
            // revealed magic or ritual card then waits face-up until its
            // controller acts with it (`move_unit`'s `fires`, emu M1-M8).
            // Monster Eye 344 (filter 0x2D, mode 2, lo 1): 0x2D3130[0x2D]
            // is filter 0x1F's arm (any card); the mask (minus itself) is
            // narrowed by 0x250A70 to the unit with the highest summon
            // order (0x1E2640, the first in mask order on a tie), which is
            // revealed [F 0x265BBC; emu REFUSALS-SWEEP.md ME1: the enemy's
            // newest set card (order 10) turned 0x29 -> 0x3D, ME2: an
            // already face-up newest unit is picked, nothing changes].
            0x18 => {
                let (mode, filter) = (low >> 6, low & 0x3F);
                if hi != 0 {
                    return Err(unsupported(format!("flip effect of card {}", unit.card)));
                }
                let newest = filter == 0x2D;
                let filter = if newest { 0x1F } else { filter };
                // The filter must be one `select_filter` knows.
                select_filter(filter, &card)?;
                let ctrl = unit.controller();
                let mut matched: Vec<&DuelUnit> = self
                    .units
                    .iter()
                    .filter(|u| {
                        u.slot != 0
                            && u.id != id
                            && match mode {
                                1 => u.controller() == ctrl,
                                2 => self.foe(u.controller(), ctrl),
                                3 => true,
                                _ => false,
                            }
                            && select_filter(filter, &self.unit_card(u, cards)).unwrap_or(false)
                    })
                    .collect();
                matched.sort_by_key(|u| (u.side, u.slot));
                if newest {
                    let mut best: Option<&DuelUnit> = None;
                    for u in matched {
                        if best.is_none_or(|b| u.summon_order > b.summon_order) {
                            best = Some(u);
                        }
                    }
                    matched = best.into_iter().collect();
                }
                let targets: Vec<UnitId> = matched.into_iter().map(|u| u.id).collect();
                if targets.is_empty() {
                    return Ok(Flip::Continue);
                }
                events.push(DuelEvent::Activated {
                    unit: id,
                    card: unit.card,
                    target: None,
                });
                for t in targets {
                    if lo == 0 {
                        // Water Element 565: 0x252FF0(g, s, 0) turns the
                        // target face down and clears committed face-up
                        // [F MORE-EFFECTS.md 1.2.1].
                        self.conceal(t, events);
                    } else {
                        self.expose(t, events);
                    }
                }
            }
            // Mask of Darkness 305 / Magician of Faith 43 (0x2656B0): on the
            // controller's own turn, a non-monster card of a graveyard is
            // created on a leader neighbour (more_rules.rs).
            0x14 => self.flip_graveyard_pick(id, low, cards, events)?,
            // Mega Thunderball 545 (0x266464): control of the battle
            // opponent (white_rules.rs).
            0x1F => return self.flip_take_control(id, role, low, hi, cards, events),
            // Rigras Leever 681 (0x266818; mode, filter, role unread):
            // record {0xE, 0, ctrl ^ 1, 1}: The Eye of Truth's event with
            // rec[0xC] = 1, whose last stage (0x25C16C) sends both hands to
            // their graveyards, group 0 first, hand slots 0..4 (0x1E0540 +
            // 0x1E00F0, no experience) [F; emu REFUSALS-SWEEP.md RL1].
            0x57 => {
                events.push(DuelEvent::Activated {
                    unit: id,
                    card: unit.card,
                    target: None,
                });
                for side in &mut self.sides {
                    let hand = std::mem::take(&mut side.hand);
                    side.graveyard.extend(hand);
                }
            }
            // Berfomet 364 (0x265FC4; mode, filter, role unread): record
            // {0x41, 0, ctrl, lo}; event 0x41 (0x2650B0) sets the
            // controller's stars to lo through 0x1DF860(ctrl, 4, lo),
            // clamped to 0..=12. [F; emu BF1: 6 -> 12]
            0x1A => {
                events.push(DuelEvent::Activated {
                    unit: id,
                    card: unit.card,
                    target: None,
                });
                self.sides[usize::from(unit.controller())].stars = (lo as u8).min(MAX_STARS);
            }
            // Jowls of Dark Demise 672 (0x266848): the battle opponent is
            // possessed (sweep_rules.rs).
            0x51 => return self.flip_possess(id, role, cards, events),
            // Time Wizard 36 (0x26556C): weak Dragons become card lo.
            0x33 => self.flip_transform(id, low, hi, lo, cards, events)?,
            // Wall of Illusion 361 / Hane-Hane 257 (0x266328): back to the
            // deck (sweep_rules.rs).
            0x24 => return self.flip_return_to_deck(id, role, low, cards, events),
            _ => return Err(unsupported(format!("flip effect of card {}", unit.card))),
        }
        Ok(Flip::Continue)
    }

    /// Field change: cells within `radius` of `center`, Labyrinth excepted.
    fn change_terrain(&mut self, center: GridPos, radius: u8, terrain: u8) {
        for (index, t) in self.terrain.iter_mut().enumerate() {
            let (c, r) = ((index % 7) as u8, (index / 7) as u8);
            if c.abs_diff(center.column) + r.abs_diff(center.row) <= radius
                && *t != terrain::LABYRINTH
            {
                *t = terrain;
            }
        }
    }

    /// Entry-0 battle pre-pass 0x263840 (defender first, then attacker),
    /// before stats are computed: terrain changes around the defender's cell,
    /// type-0x1B bonuses and the type-5 damage shields (rec+0x10 bits 0/1,
    /// F03.md §2: only between two non-leader monsters; mode, filter and
    /// word1 unread).
    fn pre_pass(
        &mut self,
        att: UnitId,
        def: UnitId,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<PrePass> {
        let mut deltas = [0i32; 2];
        let mut lp_changes: Vec<(u8, u32, u16)> = Vec::new();
        let mut shields = [false; 2];
        let mut decides = [false; 2];
        let mut nullifies = [false; 2];
        let mut whims: Vec<(usize, u32)> = Vec::new();
        let mut banish = false;
        for (i, (who, other)) in [(def, att), (att, def)].into_iter().enumerate() {
            let u = self.unit(who).expect("combatant").clone();
            let o = self.unit(other).expect("combatant").clone();
            // The record's e0 (category-1 power-ups write it, 0x249F3C).
            let Some([word0, word1]) = self.unit_card(&u, cards).entries[0] else {
                continue;
            };
            let (hi, lo) = ((word1 >> 16) as u8, (word1 & 0xFFFF) as i32);
            match word0 >> 8 {
                2 => self.change_terrain(cell, hi, lo as u8),
                5 => shields[i] = u.slot != 0 && o.slot != 0,
                // 0x263E24: both non-leaders; word0's low byte and word1
                // are not read.
                1 => nullifies[i] = u.slot != 0 && o.slot != 0,
                // 0x263B48 (Rogue Doll 37: mode 2, filter 2): 0x263FA0 tests
                // the OTHER unit's monster type against the filter
                // (0x264010); between two non-leaders the result 0x10001
                // lets the pre-pass decide the battle (rec bit 1 attacker /
                // bit 2 defender). Mode 1 (own type) is read but not run.
                4 => {
                    let (mode, filter) = ((word0 >> 6) & 3, word0 & 0x3F);
                    if mode != 2 {
                        return Err(unsupported(format!(
                            "battle effect type 0x4 mode {mode} of card {}",
                            u.card
                        )));
                    }
                    // 0x264010 reads the other unit's record kind.
                    let oc = self.unit_card(&o, cards);
                    decides[i] = u.slot != 0
                        && o.slot != 0
                        && oc.is_monster()
                        && u32::from(oc.kind & 0x1F) == filter;
                }
                0x1B => {
                    let oc = self.unit_card(&o, cards);
                    if u.slot != 0 && o.slot != 0 && oc.is_monster() && oc.kind & 0x1F == hi {
                        deltas[i] += lo;
                    }
                }
                // 0x263DA8 (Dark Elf 79: hi 1, lo 50): an LP change of the
                // unit's own GROUP (the a0 of 0x263840, not the controller)
                // in the pre-pass, attacker or defender, leaders included
                // (no slot test): hi 1 → 0x1E0710(g, 1, 0x268A10(0x2C, lo))
                // (loss, floor 0), hi 0 → 0x1E0710(g, 0, 0x268A10(0x2B,
                // lo)) (gain); result 0x10000 (no decision).
                // MANAWYDDAN-812-720-79.md §3. Other hi values (an
                // unmodified 0x1E0710(g, hi, lo)) are not used by any card.
                0xD if hi <= 1 => lp_changes.push((u.side, u32::from(hi), lo as u16)),
                // 0x263E78 (Goddess of Whim 382): no slot test (leaders
                // too); n = lo / 100 + 1, two rand() (0x19AE88) draws r1,
                // r2 (unsigned % n) and the unit's delta (rec+4 defender,
                // rec+8 attacker) = (r1 − r2) · 100; result 0x10000. Drawn
                // after the loop (defender first) [F].
                0x3F => whims.push((i, (lo as u32 & 0xFFFF) / 100 + 1)),
                // 0x263DF4 (One Who Hunts Souls 217; mode, filter, word1
                // unread): both non-leaders -> rec+0x10 bit 2, result
                // 0x10000.
                0x3A => banish |= u.slot != 0 && o.slot != 0,
                ty => {
                    return Err(unsupported(format!(
                        "battle effect type {ty:#x} of card {}",
                        u.card
                    )))
                }
            }
        }
        for (i, n) in whims {
            let r1 = self.rand31() % n;
            let r2 = self.rand31() % n;
            deltas[i] = (r1 as i32 - r2 as i32) * 100;
        }
        // Applied after the loop so that a refusal leaves no trace (the
        // order, defender first, is the original's).
        for (side, hi, lo) in lp_changes {
            if hi == 1 {
                let loss = self.lp_modifier(0x2C, lo, cards);
                self.damage(side, loss, events);
            } else {
                let gain = self.lp_modifier(0x2B, lo, cards);
                let lp = &mut self.sides[usize::from(side)].lp;
                *lp = (u32::from(*lp) + u32::from(gain)).min(u32::from(MAX_LP)) as u16;
            }
        }
        Ok(PrePass {
            def_delta: deltas[0],
            att_delta: deltas[1],
            def_shield: shields[0],
            att_shield: shields[1],
            def_nullified: nullifies[1],
            att_nullified: nullifies[0],
            def_decides: decides[0],
            att_decides: decides[1],
            banish,
        })
    }

    fn destroy(
        &mut self,
        id: UnitId,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let index = self
            .units
            .iter()
            .position(|u| u.id == id)
            .expect("unit exists");
        let _ = cards;
        let unit = self.units.remove(index);
        self.sides[usize::from(unit.side)].graveyard.push(unit.card);
        events.push(DuelEvent::Destroyed {
            unit: unit.id,
            card: unit.card,
        });
        Ok(())
    }

    /// Battle steps 0xB/0xF: the removal of a unit the battle destroyed;
    /// with step flag 0x400 (`banish`, One Who Hunts Souls) the card skips
    /// the graveyard (0x24C518: 0x1E00F0 not called before 0x1E0E30).
    fn battle_removal(
        &mut self,
        id: UnitId,
        banish: bool,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        // 0x24C544: a possessed monster whose entry 4 is still in place
        // (no cell for the Jowls) sends card lo to its controller too.
        if let Some(u) = self.unit(id) {
            if let Some([w0, w1]) = self.unit_card(u, cards).entries[4] {
                if w0 >> 8 == 0x51 {
                    let side = usize::from(u.controller());
                    self.sides[side].graveyard.push((w1 & 0xFFFF) as u16);
                }
            }
        }
        if !banish {
            return self.destroy(id, cards, events);
        }
        let index = self.units.iter().position(|u| u.id == id).expect("unit exists");
        let unit = self.units.remove(index);
        events.push(DuelEvent::Destroyed {
            unit: unit.id,
            card: unit.card,
        });
        Ok(())
    }

    fn damage(&mut self, side: u8, amount: u16, events: &mut Vec<DuelEvent>) {
        let lp = &mut self.sides[usize::from(side)].lp;
        *lp = lp.saturating_sub(amount);
        events.push(DuelEvent::Damage { side, amount });
    }

    /// Crush terrain (0x2516F0): a non-leader monster, not Immortal, with
    /// effective ATK >= 1500 on terrain 9 is destroyed.
    fn crushes(&self, unit: &DuelUnit, cards: &impl CardData) -> bool {
        let card = self.unit_card(unit, cards);
        unit.slot != 0
            && card.is_monster()
            && card.kind & 0x1F != kind::IMMORTAL
            && self.terrain_at(unit.position) == terrain::CRUSH
            && self.effective(unit, unit.position, true, cards) >= CRUSH_ATTACK
    }

    fn crush_sweep(&mut self, cards: &impl CardData, events: &mut Vec<DuelEvent>) -> Result<()> {
        let victims: Vec<UnitId> = self
            .units
            .iter()
            .filter(|u| self.crushes(u, cards))
            .map(|u| u.id)
            .collect();
        for id in victims {
            events.push(DuelEvent::Crushed { unit: id });
            // Event 0x70 handler 0x274D90 -> 0x250C90 (0x2750A0).
            self.remove_by_effect(id, cards, events)?;
        }
        Ok(())
    }

    fn move_unit(
        &mut self,
        id: UnitId,
        path: &[GridPos],
        face_up: bool,
        defence: Option<bool>,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let s = self.active;
        let index = self
            .units
            .iter()
            .position(|u| u.id == id)
            .ok_or(DuelError::InvalidCommand("no such unit"))?;
        let unit = self.units[index].clone();
        if unit.controller() != s {
            return Err(DuelError::Illegal("unit belongs to the opponent"));
        }
        if unit.acted {
            return Err(DuelError::Illegal("unit has already acted this turn"));
        }
        // Set cards move one step, face-down or not (MONSTER-EFFECTS.md
        // §3.2; Weevil's Pact moved (2,1) → (3,1) → (4,1)), and commit in
        // place (Weevil's "X R1 X" on 817 and 757). A magic card is activated
        // by revealing it (R2) and confirming, after at most one step
        // (0x253D60, experiment E4); a power-up may be revealed (never
        // activated). Revealing traps or rituals is not traced yet.
        let unit_card = self.unit_card(&unit, cards).into_owned();
        if unit.face_up && matches!(unit_card.class(), class::NEAR_TRAP | class::FAR_TRAP) {
            // State 0x38 runs 0x253D60 for a face-up unit; for traps untraced.
            return Err(unsupported(format!(
                "acting with face-up trap card {}",
                unit.card
            )));
        }
        let activates = !unit_card.is_monster() && face_up && !unit.face_up;
        // State 0x38 → 0x253D60 phase 0 needs only +0x57 bit 2 for kinds 1
        // and 5: a magic or ritual card already face-up (revealed by an
        // effect) activates when its controller ends an action with it, the
        // same as one revealed now; R2 does nothing then (emu M1-M5).
        let fires = activates
            || (unit.face_up && matches!(unit_card.class(), class::MAGIC | class::RITUAL));
        if !unit_card.is_monster() {
            let kind = unit_card.class();
            if face_up && !matches!(kind, class::MAGIC | class::POWER_UP | class::RITUAL) {
                return Err(unsupported(format!("revealing set card {}", unit.card)));
            }
        }
        // Shoulder toggles only for non-leaders before stepping (0x1D46E0).
        if unit.slot == 0 && (face_up || defence.is_some()) {
            return Err(DuelError::Illegal("leaders cannot change position"));
        }
        if defence.is_some() && !path.is_empty() {
            return Err(DuelError::Illegal("a moving monster takes attack position"));
        }
        // A monster flipped at selection runs its flip effect when the move
        // ends without a battle (role 0, 0x253D60 phase 0).
        let manual_flip = face_up
            && !unit.committed_face_up
            && unit.slot != 0
            && unit_card.is_monster()
            && unit_card.entries[3].is_some();
        if face_up && !unit.face_up {
            // Visible flip at selection; committed at the end of the action.
            self.units[index].face_up = true;
        }
        if let Some(d) = defence {
            self.units[index].defence = d;
        }
        let unit = self.units[index].clone();
        let allowance = self.allowance(&unit, cards)?;
        if path.len() > allowance {
            return Err(DuelError::Illegal("path exceeds the unit's movement"));
        }
        // Validate the orthogonal path (0x1E3CE0 DFS rules).
        let mut previous = unit.position;
        let mut visited = vec![unit.position];
        for (step, &cell) in path.iter().enumerate() {
            if cell.index().is_none()
                || previous.column.abs_diff(cell.column) + previous.row.abs_diff(cell.row) != 1
            {
                return Err(DuelError::Illegal(
                    "path must use orthogonal steps on the board",
                ));
            }
            if visited.contains(&cell) {
                return Err(DuelError::Illegal("path revisits a cell"));
            }
            let occupant = self.unit_at(cell);
            let last = step + 1 == path.len();
            match occupant {
                Some(o) if !last => {
                    let _ = o;
                    return Err(DuelError::Illegal("path passes through a unit"));
                }
                Some(o) if o.controller() == s && o.slot == 0 => {
                    return Err(DuelError::Illegal("cannot enter the own leader's cell"))
                }
                // 2v2 (a port addition): a team-mate's units block like
                // walls; they are never attacked, fused with or equipped.
                Some(o) if self.team_mates(o.controller(), s) => {
                    return Err(DuelError::Illegal("cannot enter a team-mate's unit"))
                }
                Some(o) if unit.slot == 0 && o.controller() != s => {
                    return Err(DuelError::Illegal("leaders cannot attack"))
                }
                // RITUAL.md section 7.4: not traced (checked before the
                // own-unit combinations, which would otherwise run first).
                Some(_) if fires && unit_card.class() == class::RITUAL => {
                    return Err(unsupported("a revealed ritual card moving onto a unit"))
                }
                // 0x249860 combinations (FUSION.md §5), in combine().
                Some(o) if o.controller() == s => (),
                // A face-up magic card onto an enemy card: event 0x5F
                // destroys the card, the magic advances and activates on the
                // new cell (0x24CB1C → 0x253D60, bit 4 or bit 2; emu M6/M7).
                // A revealed power-up onto an enemy card is not traced.
                Some(o)
                    if activates
                        && unit_card.class() != class::MAGIC
                        && !cards.card(o.card).expect("card").is_monster() =>
                {
                    return Err(unsupported(
                        "a revealed magic card moving onto an enemy card",
                    ))
                }
                Some(_) => (),
                None if self.terrain_at(cell) == terrain::LABYRINTH
                    && !self.has_wall_flag(&unit, cards) =>
                {
                    return Err(DuelError::Illegal("labyrinth cannot be entered"));
                }
                None => (),
            }
            visited.push(cell);
            previous = cell;
        }
        let target = path.last().and_then(|&c| self.unit_at(c)).map(|u| u.id);
        let walk_to = if target.is_some() {
            path.len() - 1
        } else {
            path.len()
        };
        // 0x1D426C: the confirm press itself checks full-range traps, for
        // moves, attacks and staying in place alike (not for leaders). A unit
        // that moves has left defence by its first step.
        if unit.slot != 0 {
            let in_defence = path.is_empty() && self.units[index].defence;
            if self.confirm_trap(index, in_defence, cards, events)? {
                return Ok(());
            }
        }
        // Moving forces attack position (0x1D4678).
        if !path.is_empty() {
            self.units[index].defence = false;
        }
        for &cell in &path[..walk_to] {
            // Traps on earlier cells may have removed units: look it up again.
            let index = self.units.iter().position(|u| u.id == id).expect("mover");
            let from = self.units[index].position;
            self.units[index].position = cell;
            events.push(DuelEvent::Moved {
                unit: id,
                from,
                to: cell,
            });
            if self.crushes(&self.units[index].clone(), cards) {
                events.push(DuelEvent::Crushed { unit: id });
                // 0x250E40 -> 0x251630: event 0x70, removal by 0x250C90.
                return self.remove_by_effect(id, cards, events);
            }
            // Walker states 0x36/0x37: limited-range traps on each entered
            // cell (crush first is an assumption; no case combines them).
            if self.near_trap(id, cell, cards, events)? {
                return Ok(());
            }
            // 0x1D601C: the on-entry effects of the cell (0x2510A0 →
            // 0x251150) when no trap fired.
            if self.on_entry(id, cell, cards, events)? {
                return Ok(());
            }
        }
        if let Some(mut defender) = target {
            // End of state 0x3C: the attack target cell is checked too (for an
            // own power-up target as well; that path is assumed, not traced).
            let cell = *path.last().unwrap();
            if self.near_trap(id, cell, cards, events)? {
                return Ok(());
            }
            // Own or enemy by controller (0x24F790): a unit of the mover's
            // group possessed by the enemy is fought [emu REFUSALS-SWEEP.md
            // JD3: Blue-Eyes Ultimate Dragon attacked the possessed
            // Red-Eyes of its own group].
            if self.unit(defender).expect("target").controller() == s {
                if manual_flip {
                    // Role 2 (battle step 17, before the combination): the
                    // type-0x12 branch of 0x265250 ignores the role (F01.md
                    // §1.2, emu R3/R4: the mover ends committed face-up);
                    // 0x13 and 0x16 (hi 0x1A) do nothing in role 2, 0x23
                    // leaves the partner out of its mask and 0x3C refuses
                    // when it would move the partner (white_rules.rs).
                    // REFUSALS-SWEEP.md §role 2: the arms that read the
                    // battle opponent (filter 0x1A of 0x17, 0x1F, 0x36/0x37,
                    // type 4) test role 2 and do nothing; 0xD, 0x18, 0x1A
                    // and 0x35 never read the role (result 1, the
                    // combination follows); type 4's other filters build
                    // their mask without the role, and 0x24BBE8 turns
                    // result 0x201 (partner in the mask) into step 0x20B
                    // (the mover advances onto the partner's cell, emu R1)
                    // and 0x401 / 0x601 (itself in the mask) into 0x10F /
                    // 0x30F (the move ends, emu R2/R3). Other flip types
                    // (0x24 filter 0x1B, 0x25, ...) are not traced there.
                    // 0x14 (Mask of Darkness 305) opens its graveyard pick,
                    // whose event 1 creates the copy on the leader's
                    // neighbour; then the combination [emu REFUSALS-SWEEP.md
                    // §7 R16b]: the pick resolves after it here (the cell is
                    // chosen at the flip; the combination only adds a
                    // monster to the graveyard and frees the mover's cell).
                    if unit_card.entries[3].is_none_or(|[w, _]| {
                        !matches!(
                            w >> 8,
                            2 | 4 | 0xC | 0xD | 0x12 | 0x13 | 0x14 | 0x16 | 0x17 | 0x18 | 0x1A | 0x1F | 0x23
                                | 0x24 | 0x25 | 0x33 | 0x35 | 0x36 | 0x37 | 0x38 | 0x3C | 0x57
                        ) || (matches!(w >> 8, 0x1F) && w & 0x3F != 0x1A)
                    }) {
                        return Err(unsupported(format!(
                            "a flip effect of card {} moving onto an own unit",
                            unit.card
                        )));
                    }
                    // 0x24BBE0 passes the target (partner's) cell, read by
                    // type 2 (476 Kairyu-Shin: the field change is centred
                    // there; 0x265400 has no role test).
                    let seat = self.unit(defender).map(|u| (u.side, u.slot, u.position));
                    self.flip(id, FlipRole::OntoOwn(defender), cell, cards, events)?;
                    // A partner replaced in its slot (Time Wizard's event
                    // 0x3F) is combined as the new unit [emu REFUSALS-SWEEP.md
                    // R9: Thousand Dragon went to the graveyard by the failed
                    // fusion]; 0x38 (Kageningen) summons and combines [R10].
                    if self.unit(defender).is_none() {
                        if let Some((side, slot, at)) = seat {
                            if let Some(u) = self
                                .units
                                .iter()
                                .find(|u| u.side == side && u.slot == slot && u.position == at)
                            {
                                defender = u.id;
                            }
                        }
                    }
                    if self.unit(id).is_none() {
                        // 0x10F / 0x30F: the mover left; nothing else.
                        return self.crush_sweep(cards, events);
                    }
                    if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
                        u.committed_face_up = true;
                    }
                    if self.unit(defender).is_none()
                        && unit_card.entries[3].is_some_and(|[w, _]| w >> 8 != 4)
                    {
                        return Err(unsupported(format!(
                            "a flip effect of card {} emptying its partner's slot",
                            unit.card
                        )));
                    }
                    if self.unit(defender).is_none() {
                        // Step 0x20B -> 0xB -> 0xC/0xD: the mover takes the
                        // partner's cell as after a won battle, then commits.
                        self.advance_into(id, cell, cards, events)?;
                        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
                            u.committed_face_up = u.face_up;
                            u.committed_defence = u.defence;
                            u.acted = true;
                        }
                        return Ok(());
                    }
                }
                if !self.combine(id, defender, cards, events)? {
                    return Ok(());
                }
            } else {
                let seat = (unit.side, unit.slot);
                let first_new = self.next_unit;
                self.battle(id, defender, cell, cards, events)?;
                // The battle machine commits the unit in the attacker's
                // slot: one that replaced it in place (Time Wizard's
                // Thousand Dragon, emu REFUSALS-SWEEP.md TW2: 0x15) ends
                // committed and acted like the attacker would have.
                if self.unit(id).is_none() {
                    if let Some(u) = self.units.iter_mut().find(|u| {
                        (u.side, u.slot) == seat && u.id.0 >= first_new
                    }) {
                        u.committed_face_up = u.face_up;
                        u.committed_defence = u.defence;
                        u.acted = true;
                    }
                }
            }
        } else if manual_flip {
            let at = self.unit(id).expect("mover").position;
            self.flip(id, FlipRole::Manual, at, cards, events)?;
        }
        if fires && unit_card.class() == class::MAGIC && self.unit(id).is_some() {
            return self.activate_magic(id, cards, events);
        }
        if fires && unit_card.class() == class::RITUAL && self.unit(id).is_some() {
            return self.activate_ritual(id, cards, events);
        }
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            u.committed_face_up = u.face_up;
            u.committed_defence = u.defence;
            u.acted = true;
        }
        Ok(())
    }

    /// [`Self::battle_steps`]; in an N-player duel (a port addition) "the
    /// opponent" of either combatant's controller is the other's for the
    /// whole battle.
    fn battle(
        &mut self,
        att: UnitId,
        def: UnitId,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        if self.multi().is_none() {
            return self.battle_steps(att, def, cell, cards, events);
        }
        let pair = [
            self.unit(att).expect("attacker").controller(),
            self.unit(def).expect("defender").controller(),
        ];
        self.set_pair(Some(pair));
        let result = self.battle_steps(att, def, cell, cards, events);
        self.set_pair(None);
        result
    }

    /// Battle 0x24AFA0 / 0x248130 for monster attackers.
    fn battle_steps(
        &mut self,
        att: UnitId,
        def: UnitId,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        // Step 1 (0x24AFA0/0x24B0D0), before any reveal: full-range traps of
        // the defending side, in this order, for any attack on the other side.
        let attacker = self.unit(att).expect("attacker");
        let attacker_side = attacker.side;
        let attacker_controller = attacker.controller();
        if self.unit(def).expect("defender").controller() != attacker_controller {
            for ty in [
                trap_type::MIRROR_FORCE,
                trap_type::NEGATE_ATTACK,
                trap_type::MIRROR_WALL,
            ] {
                let Some(trap) = self.far_trap_in(&self.foes(attacker_controller), ty, cards) else {
                    continue;
                };
                // 0x24B14C / 0x24B2A8 / 0x24B3D0: a found battle trap first
                // meets leader ability #11 of the attacker at the target cell
                // (0x2738C0): {0xFE, 0, ctrl << 8, 0x10}, result 0x10003 =
                // battle step 3; the trap stays and no other trap is tried.
                if self.ability11_at(self.unit(att).expect("attacker"), cell, cards) {
                    break;
                }
                // 0x24B310: an immune Dragon skips Negate Attack and
                // goes straight to battle step 3, before Royal Decree.
                // Mirror Force checks the Decree first and still fires
                // for an immune attacker (0x24B1F0, below).
                if matches!(ty, trap_type::NEGATE_ATTACK | trap_type::MIRROR_WALL)
                    && self.dragon_immunity(self.unit(att).expect("attacker"), cards)
                {
                    break;
                }
                // All three Decree branches return step 3, skipping the
                // remaining battle-trap priorities (0x24B1E8/B360/B488).
                if self.royal_decree_reacts(self.opp(attacker_side), cards, events) {
                    break;
                }
                if ty == trap_type::MIRROR_WALL {
                    // Mirror Wall (0x24B3A0: record {0x36, defender,
                    // attacker, trap | 0x31 << 16}, result 0x10003 = battle
                    // step 3; handler 0x26CDB0): the trap is revealed and
                    // marked acted and STAYS (only type 0x31 skips the
                    // 0x250C90 of stage 0); stage 3 subtracts half the
                    // attacker's clamped card ATK + ATK change (0x1E1510(g,
                    // s, 0) >> 1) from its stored ATK change (0x2525C0 op 1,
                    // ATK only). The battle then goes on with the weakened
                    // attacker. [F; emu W0-W5]
                    events.push(DuelEvent::Attacked { attacker: att, defender: def });
                    self.react_trap(trap, Some(att), events);
                    self.mirror_wall_halve(att, cards, events);
                    break;
                }
                if ty == trap_type::MIRROR_FORCE {
                    // 0x24B12C: record {0x36, defender, attacker, trap |
                    // 4 << 16}; result 0x1010F (the battle ends, attacker
                    // gone) or, for an attacker under the Dragon immunity
                    // (0x269020, either side's source), 0x10003: the event
                    // still runs and the battle goes on at step 3
                    // (E2-536-TRAP-0x4.md §2; emu MF1-MF7).
                    let immune = self.dragon_immunity(self.unit(att).expect("attacker"), cards);
                    let a = self.unit(att).expect("attacker");
                    if !immune && (a.slot == 0 || a.defence) {
                        // 0x35 spares it, yet 0x1010F clears the attacker
                        // pointer: not traced.
                        return Err(unsupported(
                            "Mirror Force with an attacker it does not destroy",
                        ));
                    }
                    events.push(DuelEvent::Attacked { attacker: att, defender: def });
                    self.mirror_force(trap, att, self.opp(attacker_controller), cards, events)?;
                    if immune {
                        break;
                    }
                    return self.crush_sweep(cards, events);
                }
                if ty != trap_type::NEGATE_ATTACK {
                    return Err(unsupported(format!("trap effect type {ty:#x}")));
                }
                // Negate Attack: no damage, destruction, advance or reveal; the
                // attacker keeps the cell it walked to and commits there.
                events.push(DuelEvent::Attacked { attacker: att, defender: def });
                self.activate(trap, Some(att), events);
                self.discard(trap, events);
                events.push(DuelEvent::Cancelled { unit: att });
                return Ok(());
            }
        }
        let ai = self.units.iter().position(|u| u.id == att).unwrap();
        let di = self.units.iter().position(|u| u.id == def).unwrap();
        let (acard, dcard) = (
            self.unit_card(&self.units[ai], cards).into_owned(),
            self.unit_card(&self.units[di], cards).into_owned(),
        );
        if !acard.is_monster() {
            // A card moving onto an enemy unit (0x248130, MONSTER-EFFECTS.md
            // §3.2). Onto a monster or leader: a power-up that fits a
            // non-leader monster gives it the bonus; anything else is
            // destroyed. Onto a card: that card is destroyed and the mover
            // advances.
            let defender = self.units[di].clone();
            if dcard.is_monster() {
                // 0x248130 takes DEFENDER first, attacker second. This is
                // the 0x2493A0 branch, with no defending-monster XP award.
                // 0x249408: an enemy power-up is destroyed without
                // equipping a Dragon covered by the continuous source.
                let fits = acard.class() == class::POWER_UP
                    && defender.slot != 0
                    // 0x2493BC: leader ability #11 (0x273770) first.
                    && !self.ability11(&defender, cards)
                    && !self.dragon_immunity(&defender, cards)
                    && power_up_fits(&dcard, acard.id);
                if fits {
                    let data = acard.power_up.ok_or_else(|| {
                        unsupported(format!("power-up {} without imported data", acard.id))
                    })?;
                    if data.category == 4 {
                        // Event 0x2B on the enemy monster (sweep_rules.rs).
                        return self.transform_by_power_up(def, att, defender.position, false, cards, events);
                    }
                    if !matches!(data.category, 0..=3) {
                        return Err(unsupported("a transforming power-up"));
                    }
                    // Riryoku 789 moved onto an enemy monster: record
                    // {0x25, monster, power-up, card}, the attacked case's
                    // handler 0x26B9D0 with the roles swapped: the monster
                    // gains half the LP of the Riryoku's opponent group,
                    // which pays it [emu REFUSALS-SWEEP.md RY1/RY2: the
                    // enemy Hunter Spider / Red-Eyes +1700, side 1 3400 ->
                    // 1700 per application (the harness repeats phase 2)].
                    if data.category == 0 && data.value == 0 {
                        // The trap answers (Goblin Fan, Reverse Trap) and
                        // their #11 skip are traced for the attacked case
                        // only.
                        if self.living_sides().into_iter().any(|g| {
                            self.far_trap(g, trap_type::GOBLIN_FAN, cards).is_some()
                                || self.far_trap(g, 0x2B, cards).is_some()
                        }) {
                            return Err(unsupported(
                                "Riryoku moved onto an enemy with an equip trap on the field",
                            ));
                        }
                        self.riryoku_attacked(def, att, cards, events)?;
                        self.discard(att, events);
                        return Ok(());
                    }
                    self.ordinary_field_equip_guard(data.category, cards)?;
                    self.discard(att, events);
                    self.apply_ordinary_field_equip(def, data.category, data.value, cards, events);
                } else {
                    self.discard(att, events);
                }
                return Ok(());
            }
            self.discard(def, events);
            self.advance(att, cell, events);
            return Ok(());
        }
        if !dcard.is_monster() {
            // Step 3 skips a card defender (not revealed, no flip). Step 2
            // (0x24B728): no attack while spellbound or in committed
            // defence; the attacker is not revealed, except one flipped
            // face-up during this move with an e3, whose flip runs with
            // the card as its battle opponent (MONSTER-EFFECTS.md §3.1) [F].
            let a = self.units[ai].clone();
            if a.spellbind > 0 || self.is_chain_bound(att) || (a.defence && a.committed_defence) {
                events.push(DuelEvent::Cancelled { unit: att });
                return Ok(());
            }
            // Filter 0x1A takes the card as its target: Man-Eater Bug's event
            // 0x15 destroys it (no 0x5F record, no +6) and the Bug advances
            // (0x20B); Nightmare Scorpion's bind event {0, 0, card, 1} runs,
            // then 0x5F removes the card as usual [emu claude-effects
            // tools/flip-card FC1/FC3].
            if a.face_up && !a.committed_face_up && acard.entries[3].is_some() {
                self.battle_reveal(att, events);
                let flipped = self.flip(att, FlipRole::Battle(def), cell, cards, events)?;
                if self.unit(att).is_none() {
                    return self.crush_sweep(cards, events);
                }
                // Bit 11 (Ooguchi): step 0xE, the battle ends.
                if let Flip::BattleEnds = flipped {
                    return Ok(());
                }
                if let Flip::OpponentDestroyed = flipped {
                    // Step 0x20B: the card is gone; the attacker may advance.
                    if self.unit(att).is_some_and(|u| u.spellbind == 0) {
                        return self.advance_into(att, cell, cards, events);
                    }
                    return self.crush_sweep(cards, events);
                }
                if self.unit(def).is_none() {
                    return Err(unsupported("a flip effect removing the attacked card"));
                }
            }
            let ai = self.units.iter().position(|u| u.id == att).unwrap();
            let di = self.units.iter().position(|u| u.id == def).unwrap();
            let dcard = self.unit_card(&self.units[di], cards).into_owned();
            // 0x2495AC: immune or incompatible attackers destroy a
            // power-up; a compatible attacker receives event 0x24 first.
            if dcard.class() == class::POWER_UP {
                return self.attack_power_up(att, def, cell, cards, events);
            }
            // 0x2495D4 (a power-up) / 0x249810 (other cards): the attacker's
            // card +6; event 0x5F then removes the card (0x276F10 -> 0x250C90).
            let side = self.units[ai].side;
            self.record_experience(side, acard.id, 6);
            self.discard(def, events);
            // Step 6 (0x24BCF0): a 0x5F record with rec[0xC] = 0 goes to
            // step 0x20B, i.e. step 0xB → 0xC → 0xD as after a won battle:
            // advance, crush sweep, then the attacker's on-entry effect on
            // the target cell [F].
            return self.advance_into(att, cell, cards, events);
        }
        // The battle machine addresses its units by (group, slot): a flip
        // that replaces a combatant in place (Time Wizard 36's event 0x3F:
        // the Dragon to the graveyard, Thousand Dragon created on its cell
        // in the lowest free slot) leaves the new unit fighting [emu
        // REFUSALS-SWEEP.md TW2/TW3: a Blackland Fire Dragon attacking a
        // face-down Time Wizard became Thousand Dragon in g0 s3, which won
        // and advanced].
        let mut att = att;
        let mut def = def;
        let seats = [att, def].map(|id| {
            let u = self.unit(id).expect("combatant");
            (u.side, u.slot, u.position)
        });
        let reseat = |d: &Duel, id: UnitId, (side, slot, at): (u8, u8, GridPos)| {
            if d.unit(id).is_some() {
                return id;
            }
            d.units
                .iter()
                .find(|u| u.side == side && u.slot == slot && u.position == at)
                .map_or(id, |u| u.id)
        };
        // Step 3: the defender is revealed; its flip effect may end the
        // battle (Man-Eater Bug) or disable the attacker (Key Mace, Nightmare
        // Scorpion) for step 2.
        if self.battle_reveal(def, events) {
            match self.flip(def, FlipRole::Battle(att), cell, cards, events)? {
                Flip::OpponentDestroyed => return self.crush_sweep(cards, events),
                // Bit 11 (0x24B65C, Ooguchi): step 0xE, the battle ends.
                Flip::BattleEnds => return Ok(()),
                // Bit 10 at step 3: step 0x20B, the attacker may advance.
                Flip::SelfDestroyed => {
                    if self.unit(att).is_some_and(|u| u.spellbind == 0) {
                        return self.advance_into(att, cell, cards, events);
                    }
                    return self.crush_sweep(cards, events);
                }
                Flip::Continue => (),
            }
            att = reseat(self, att, seats[0]);
            def = reseat(self, def, seats[1]);
        }
        // Step 2 (0x24B728): no attack while spellbound or in committed
        // defence; then the attacker is revealed and may flip.
        // A flip that emptied the attacker's slot (Time Wizard's new
        // Thousand Dragon took a lower free slot) ends the battle: no
        // record, the defender stays revealed [emu REFUSALS-SWEEP.md TW4].
        if self.unit(att).is_none() {
            return Ok(());
        }
        let attacker = self.unit(att).expect("attacker").clone();
        // 0x24B728: +0x49 (0x1E2800) or +0x4A (0x1E28C0) > 0 → step 0xE.
        if attacker.spellbind > 0
            || self.is_chain_bound(att)
            || (attacker.defence && attacker.committed_defence)
        {
            events.push(DuelEvent::Cancelled { unit: att });
            return Ok(());
        }
        if self.battle_reveal(att, events) {
            match self.flip(att, FlipRole::Battle(def), cell, cards, events)? {
                Flip::OpponentDestroyed => {
                    // Step 0x20B: the attacker may advance.
                    if self.unit(att).is_some_and(|u| u.spellbind == 0) {
                        return self.advance_into(att, cell, cards, events);
                    }
                    return self.crush_sweep(cards, events);
                }
                // Bit 11 (0x24BA78, Ooguchi): step 0xE, the battle ends.
                Flip::BattleEnds => return Ok(()),
                // Bit 10 at step 2: step 0x10F, no advance.
                Flip::SelfDestroyed => return self.crush_sweep(cards, events),
                Flip::Continue => (),
            }
            att = reseat(self, att, seats[0]);
            def = reseat(self, def, seats[1]);
            if self.unit(def).is_none() {
                return Err(unsupported(
                    "an attacker's flip effect emptying the defender's slot",
                ));
            }
        }
        // A replaced combatant fights with its own card.
        let (acard, dcard) = (
            self.unit_card(self.unit(att).expect("attacker"), cards).into_owned(),
            self.unit_card(self.unit(def).expect("defender"), cards).into_owned(),
        );
        // Step 6: entry-0 pre-pass, then the battle itself.
        let mut pre = self.pre_pass(att, def, cell, cards, events)?;
        if pre.att_decides || pre.def_decides {
            let attacker = self.unit(att).expect("attacker").clone();
            let defender = self.unit(def).expect("defender").clone();
            return self.decided_battle(&attacker, &defender, &pre, cell, cards, events);
        }
        // Type 1 (Jirai Gumo): the other unit's permanent changes are set to
        // 0 (0x1E16D0/0x1E17F0 op 4, permanent: Red-Eyes kept 0/0 after
        // the battle in emu C3/C4) and its pre-pass delta is skipped.
        if pre.def_nullified {
            self.zero_bonuses(def, events);
            pre.def_delta = 0;
        }
        if pre.att_nullified {
            self.zero_bonuses(att, events);
            pre.att_delta = 0;
        }
        let attacker = self.unit(att).expect("attacker").clone();
        let defender = self.unit(def).expect("defender").clone();
        let clamp = |v: i32| v.clamp(0, i32::from(MAX_LP)) as u16;
        // 0x248130 adds the pre-pass delta inside the one final clamp.
        let a = clamp(self.effective_sum(&attacker, cell, true, cards) + pre.att_delta);
        // 0x248310 / 0x2489EC: the attacker's delta also becomes permanent
        // (0x1E16D0 and 0x1E17F0 op 0, +0x38 and +0x3A), against a leader
        // too [F; emu exp_prepass P1, P4].
        self.add_battle_delta(att, pre.att_delta, events);
        if defender.slot == 0 {
            let mut dmg = a;
            if self.has_ability(defender.side, 6) {
                dmg >>= 1;
            }
            events.push(DuelEvent::Battle {
                attacker: att,
                defender: def,
                attack: a,
                defend: 0,
                defence: false,
            });
            // Event 0x5D handler 0x275790: the LP setter takes
            // 0x268A10(0x2C, damage) (0x275E00). [H] the ability-6 halving
            // and the modifier commute here (no leader with #6 and a 0x3B
            // modifier in the decks in scope).
            let dmg = self.lp_modifier(0x2C, dmg, cards);
            self.damage(defender.side, dmg, events);
            self.leader_hit_experience(&attacker, &defender);
            return Ok(());
        }
        // 0x248894 (defence: DEF) / 0x248DA0 (attack position: ATK = 0x1E1510
        // + rec+4 + 0x267A00 a3 1): the defender's delta is added to the stat
        // it fights with, then made permanent on both stats like the
        // attacker's [F; emu exp_prepass P2 (attack position: 1200 beat 600,
        // kept +500/+500), P3 (defence)].
        let d =
            clamp(self.effective_sum(&defender, cell, !defender.defence, cards) + pre.def_delta);
        self.add_battle_delta(def, pre.def_delta, events);
        events.push(DuelEvent::Battle {
            attacker: att,
            defender: def,
            attack: a,
            defend: d,
            defence: defender.defence,
        });
        let relation = attribute_relation(acard.attribute, dcard.attribute);
        let att_beats = relation > 0 && defender.spellbind == 0;
        let def_beats = relation < 0 && attacker.spellbind == 0;
        let (mut att_dies, mut def_dies) = (false, false);
        // The loser's side pays the difference unless the loser carries the
        // type-5 shield (F03.md §2.3); the winner's shield is never read.
        let mut loss: Option<(u8, u16, bool)> = None;
        if defender.defence {
            if d > a {
                loss = Some((attacker.controller(), d - a, pre.att_shield));
            } else if d < a {
                def_dies = true;
            }
        } else if d > a {
            att_dies = true;
            loss = Some((attacker.controller(), d - a, pre.att_shield));
        } else if d == a {
            att_dies = true;
            def_dies = true;
        } else {
            def_dies = true;
            loss = Some((defender.controller(), a - d, pre.def_shield));
        }
        let mut dealt = 0;
        if let Some((side, amount, shielded)) = loss {
            if shielded {
                // Record bits 20/22 mark the shield on screen (sub-event 0x25C).
                let (shielded_unit, other) = if side == attacker.controller() {
                    (&attacker, def)
                } else {
                    (&defender, att)
                };
                events.push(DuelEvent::Activated {
                    unit: shielded_unit.id,
                    card: shielded_unit.card,
                    target: Some(other),
                });
            } else {
                dealt = amount;
                // Event 0x5E handler 0x2768EC: 0x268A10(0x2C, low16) before
                // the LP setter (F03.md §2.4); the experience test reads the
                // raw difference.
                let amount = self.lp_modifier(0x2C, amount, cards);
                self.damage(side, amount, events);
            }
        }
        self.battle_experience(&attacker, &defender, a, d, dealt, cards);
        // Steps 8–10: entry-4 effects (0x2530F0) of the units destroyed by the
        // enemy, the defender's first, both while the units are still on the
        // board; then the removals (steps 0xB/0xF). Battle deaths cost no
        // experience (see `experience.rs`).
        let mut over = false;
        let mut no_advance = false;
        let (mut def_stays, mut att_stays) = (false, false);
        if def_dies {
            let outcome = self.on_destroyed_in_battle(
                &defender,
                Some(&attacker),
                att_dies,
                cell,
                cards,
                events,
            )?;
            over = matches!(outcome, Aftermath::BattleOver);
            no_advance = matches!(outcome, Aftermath::NoAdvance);
            def_stays = matches!(outcome, Aftermath::Survives);
        }
        if att_dies && !over {
            let other = if def_dies { None } else { Some(&defender) };
            let outcome =
                self.on_destroyed_in_battle(&attacker, other, def_dies, cell, cards, events)?;
            over = matches!(outcome, Aftermath::BattleOver);
            att_stays = matches!(outcome, Aftermath::Survives);
        }
        if def_dies && !def_stays && self.unit(def).is_some() {
            self.battle_removal(def, pre.banish, cards, events)?;
        }
        if att_dies && !att_stays && self.unit(att).is_some() {
            self.battle_removal(att, pre.banish, cards, events)?;
        }
        // A defender's effect that removed the attacker (F04a step 0x30F),
        // ended the battle (F04b) or returned bit 10 (Big Eye, step 0x20F)
        // goes 0xF -> 0xE: no binds, no advance and no crush sweep; the
        // attacker-dies paths sweep at step 0x10.
        if def_dies && (over || no_advance || att_dies || self.unit(att).is_none()) {
            return Ok(());
        }
        if !over {
            if att_beats && !def_dies {
                self.bind(def, 1, events);
            }
            if def_beats && !att_dies {
                self.bind(att, 1, events);
            }
        }
        if def_dies && !att_dies && !over && !def_beats {
            return self.advance_into(att, cell, cards, events);
        }
        self.crush_sweep(cards, events)
    }

    /// Steps 0xC/0xD after a won battle (0x24C3E4 with the attacker present
    /// and unbound): the attacker takes the target cell unless it is an
    /// empty Labyrinth cell and the attacker has no wall flag (d66 bit 5),
    /// the crush sweep (0x2517D0) runs, then the attacker's entry-1 on-entry
    /// effect (0x24C804 → 0x251150) on the TARGET cell, advanced or not
    /// [emu C6: Jirai Gumo lost 100 LP without entering the wall cell].
    fn advance_into(
        &mut self,
        att: UnitId,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let Some(attacker) = self.unit(att).cloned() else {
            return Ok(());
        };
        if self.terrain_at(cell) != terrain::LABYRINTH || self.has_wall_flag(&attacker, cards) {
            self.advance(att, cell, events);
        }
        self.crush_sweep(cards, events)?;
        if self.unit(att).is_some() {
            self.on_entry(att, cell, cards, events)?;
        }
        Ok(())
    }

    /// 0x1E16D0/0x1E17F0 op 0 with a battle pre-pass delta (0x248130): both
    /// permanent changes grow by `delta`, each clamped to ±9999 (0x1E1740).
    fn add_battle_delta(&mut self, id: UnitId, delta: i32, events: &mut Vec<DuelEvent>) {
        if delta == 0 {
            return;
        }
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            let max = i32::from(MAX_LP);
            u.attack_bonus = (i32::from(u.attack_bonus) + delta).clamp(-max, max) as i16;
            u.defense_bonus = (i32::from(u.defense_bonus) + delta).clamp(-max, max) as i16;
            events.push(DuelEvent::Modified {
                unit: id,
                attack: u.attack_bonus,
                defense: u.defense_bonus,
            });
        }
    }

    /// 0x1E16D0/0x1E17F0 op 4 with 0: both permanent changes become 0.
    fn zero_bonuses(&mut self, id: UnitId, events: &mut Vec<DuelEvent>) {
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            if u.attack_bonus == 0 && u.defense_bonus == 0 {
                return;
            }
            u.attack_bonus = 0;
            u.defense_bonus = 0;
            events.push(DuelEvent::Modified {
                unit: id,
                attack: 0,
                defense: 0,
            });
        }
    }

    /// Entry-1 on-entry effects 0x251150 (through 0x2510A0 for a cell that
    /// is not Crush) for a non-leader that has just entered `cell`: the
    /// walker states 0x36/0x37 (after the limited-range traps) and battle
    /// step 0xD. Returns true when the unit left the board (replaced), so
    /// the action ends there (no commit: emu B1–B5 end 0x38 → 0x16).
    fn on_entry(
        &mut self,
        id: UnitId,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<bool> {
        let Some(unit) = self.unit(id).cloned() else {
            return Ok(true);
        };
        if unit.slot == 0 {
            // 0x251150's leader branch: ability #7 (0x251194: 0x273070(7,
            // g)) turns the entered cell into the leader kind's terrain
            // (0x2730D0 via 0x1E34A0, Labyrinth and Crush cells included);
            // then #16 / #15 / #13 sweep the leader's window around the
            // entered cell (white_rules.rs). [F; emu REFUSALS-SWEEP.md
            // L0-L3: Sea and Crush cells became 7 under a Fairy leader;
            // staying in place changes nothing]
            if self.leader_has(unit.side, 7, cards) {
                let kind = self.unit_card(&unit, cards).kind & 0x1F;
                if let Some(t) = cell.index().and_then(|i| self.terrain.get_mut(i)) {
                    *t = leader_terrain(kind);
                }
            }
            self.window_sweep(unit.controller(), cell, cards, events)?;
            return Ok(false);
        }
        let card = cards.card(unit.card).expect("validated card").clone();
        let Some([word0, word1]) = card.entries[1] else {
            return Ok(false);
        };
        let (ty, mode, lo) = (word0 >> 8, (word0 >> 6) & 3, (word1 & 0xFFFF) as u16);
        match ty {
            // 0x25159C (Shadow Ghoul 123, lo 144 Wall Shadow): on a
            // Labyrinth cell, event 0x3C (handler 0x26ABF0, MOTH-EFFECTS.md
            // §1): the unit goes to its owner's graveyard (0x250C90) and the
            // card `lo` is created on the cell for its group: face-down,
            // attack position, not acted, the lowest free monster slot, a
            // fresh summon order. The move ends without a commit. Any
            // other terrain: nothing [emu B1–B6].
            9 => {
                if self.terrain_at(cell) != terrain::LABYRINTH {
                    return Ok(false);
                }
                events.push(DuelEvent::Activated {
                    unit: id,
                    card: unit.card,
                    target: None,
                });
                let side = unit.side;
                self.remove_by_effect(id, cards, events)?;
                self.create(side, lo, cell, cards, events);
                Ok(true)
            }
            // 0x251548: a Labyrinth cell under the unit becomes terrain 7
            // (code only; no card in the decks in scope).
            0xA => {
                if self.terrain_at(cell) == terrain::LABYRINTH {
                    events.push(DuelEvent::Activated {
                        unit: id,
                        card: unit.card,
                        target: None,
                    });
                    if let Some(t) = cell.index().and_then(|i| self.terrain.get_mut(i)) {
                        *t = terrain::NORMAL;
                    }
                }
                Ok(false)
            }
            // 0x2513F4 (Jirai Gumo 408: mode 1, lo 100): the controller's
            // side (mode 1) or the other side (mode 2) loses 0x268A10(0x2C,
            // lo) (0x1E0710 op 1), once per entered cell, the battle
            // advance included, face-down too; no trap check [emu D1–D5,
            // C1b/C5: −100, −200 with a Midnight Fiend]. Other modes: no LP
            // change (0x251474).
            0x42 => {
                let victim = match mode {
                    1 => unit.controller(),
                    2 => self.opp(unit.controller()),
                    _ => return Ok(false),
                };
                let loss = self.lp_modifier(0x2C, lo, cards);
                events.push(DuelEvent::Activated {
                    unit: id,
                    card: unit.card,
                    target: None,
                });
                self.damage(victim, loss, events);
                Ok(false)
            }
            // Dokurorider 128 (0x251498): terrain lo within hi of the cell.
            2 => {
                self.terrain_on_entry(id, cell, word1, events)?;
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    /// A battle the entry-0 pre-pass decided (0x248130 result low byte 1,
    /// branches 0x2484FC–0x2487B0; Rogue Doll against a Zombie, run in the
    /// interpreter as attacker and defender, face-down or face-up, either
    /// position): no stats are computed, the record 0x5E carries damage
    /// 0xFFFF (its handler skips the LP setter, 0x27690C), the other unit is
    /// destroyed through the normal steps 8–10 (its entry 4 fires: Fiend's
    /// Hand took the Doll with it), the winner stays on its cell without a
    /// bind and its side gets 2·(cost(loser) + 2) for the winner's card
    /// (0x2485C4 / 0x248788). Both effects applying at once (0x24851C) is
    /// read but not run.
    fn decided_battle(
        &mut self,
        attacker: &DuelUnit,
        defender: &DuelUnit,
        pre: &PrePass,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        if pre.att_decides && pre.def_decides {
            return Err(unsupported(
                "a battle between two units whose battle effects both decide it",
            ));
        }
        let (winner, loser) = if pre.att_decides {
            (attacker, defender)
        } else {
            (defender, attacker)
        };
        events.push(DuelEvent::Activated {
            unit: winner.id,
            card: winner.card,
            target: Some(loser.id),
        });
        let cost = i32::from(self.unit_cost(loser, cards));
        self.record_experience(winner.side, winner.card, 2 * (cost + 2));
        // Steps 8–10 as for any battle death, with the winner as the other
        // unit; then the removal.
        self.on_destroyed_in_battle(loser, Some(winner), false, cell, cards, events)?;
        if self.unit(loser.id).is_some() {
            self.battle_removal(loser.id, pre.banish, cards, events)?;
        }
        if pre.att_decides {
            // The attacker's path ends through step 12's sweep (no advance).
            return self.crush_sweep(cards, events);
        }
        Ok(())
    }

    /// Entry 4 (0x2530F0) of `unit`, destroyed in battle by the enemy (battle
    /// steps 8/9/0xA only; removals by 0x250C90 never run it). `other` is the
    /// other battle unit (the attacker for a dead defender; the surviving
    /// defender for a dead attacker; `None` for the attacker of a mutual
    /// destruction) and `other_dies` whether it dies too (t3); `target` is
    /// the attack target cell. Effects distinguish physical ownership from
    /// current control where their original callers do.
    fn on_destroyed_in_battle(
        &mut self,
        unit: &DuelUnit,
        other: Option<&DuelUnit>,
        other_dies: bool,
        target: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Aftermath> {
        let Some([word0, word1]) = self.unit_card(unit, cards).entries[4] else {
            return Ok(Aftermath::Continue);
        };
        let (ty, mode, filter) = (word0 >> 8, (word0 >> 6) & 3, word0 & 0x3F);
        let (hi, lo) = (word1 >> 16, word1 & 0xFFFF);
        // Filter 0x1A: the battle opponent, unless it is a leader or dies too.
        let opponent = other.filter(|o| o.slot != 0 && !other_dies);
        let refuse = || {
            Err(unsupported(format!(
                "destruction effect of card {}",
                unit.card
            )))
        };
        match ty {
            0x38 => {
                self.summon_beside_leader(unit, word1, cards, events)?;
                Ok(Aftermath::Continue)
            }
            0x41 => self.destruction_boost_for_lp(unit, word0, word1, cards, events),
            // The Wicked Worm Beast 228 (0x25346C): back into the deck
            // (white_rules.rs).
            0x24 => self.return_to_deck(unit, cards, events),
            // 0x253A6C: a monster Jowls of Dark Demise possessed
            // (sweep_rules.rs).
            0x51 => {
                self.possessed_destroyed(unit, word1, cards, events);
                Ok(Aftermath::Continue)
            }
            // Sinister Serpent 456 (0x253B48; mode/filter unread): record
            // {0x41, 0, ctrl ^ 1, lo}: Berfomet's event 0x41 sets the stars
            // of the dying unit's controller's opponent to lo (0..=12).
            // [F; emu REFUSALS-SWEEP.md SS1/SS2: 5 -> 0 for the attacker's
            // side, and for the defender's side when the Serpent attacked]
            0x1A => {
                events.push(DuelEvent::Activated {
                    unit: unit.id,
                    card: unit.card,
                    target: None,
                });
                let opponent = self.opp(unit.controller());
                self.sides[usize::from(opponent)].stars = lo.min(u32::from(MAX_STARS)) as u8;
                Ok(Aftermath::Continue)
            }
            // Dark Plant 647 (0x253240, mode/filter unread): record {2,
            // target cell, hi, lo} = the field change of Yamadron's event 2
            // around the battle's target cell. [F; emu DP1]
            2 => {
                events.push(DuelEvent::Activated {
                    unit: unit.id,
                    card: unit.card,
                    target: None,
                });
                self.change_terrain(target, hi as u8, lo as u8);
                Ok(Aftermath::Continue)
            }
            // Skelengel 384 (0x253904): one more summon for the controller.
            0x26 => {
                self.skelengel_summon(unit, events);
                Ok(Aftermath::Continue)
            }
            // Hourglass of Life 377 (0x253650): graveyard monsters revived
            // beside the controller's leader for LP (white_rules.rs).
            0x3E => self.hourglass_revive(unit, word1, cards, events),
            // Big Eye 316 (0x2533C8, filter 0x1A): event {0, 0, killer, lo}
            // = Nightmare Scorpion's handler 0x2579E0: the killer is
            // spellbound for lo and marked acted; the result 0x401 (bit 10)
            // sends a dying defender to step 0xF (no advance, emu L1), a
            // dying attacker through the normal step 0x10 (emu L2). Nothing
            // when the killer dies too (emu L3) or is a leader. Other
            // filters build a 0x24E360 mask (0x253408): not run.
            0x17 => {
                if filter != 0x1A {
                    return refuse();
                }
                let Some(o) = opponent else {
                    return Ok(Aftermath::Continue);
                };
                events.push(DuelEvent::Activated {
                    unit: unit.id,
                    card: unit.card,
                    target: Some(o.id),
                });
                self.bind(o.id, lo as u8, events);
                if let Some(u) = self.units.iter_mut().find(|u| u.id == o.id) {
                    u.acted = true;
                }
                Ok(Aftermath::NoAdvance)
            }
            // Golgoil 519 (0x253560, mode 1 filter 0x1B): with no trap 0x4C
            // and no Royal Decree in either group (mask-3 searches), the first
            // free non-Labyrinth neighbour of its owner's leader
            // (0x24FB40(g, 1)) receives it through event 0x1F (0x25FB20
            // stage 1: 0x1E0FD0 raw move, flags untouched) and the battle
            // machine skips its removal (result 0x101; emu J1/J2: the
            // defender keeps its position and is still not acted, the
            // attacker advances; the attacker commits as usual). No free
            // cell: 0x24FB40 < 0, nothing, an ordinary death (emu J3b).
            // The 0x4C trap path (event 0x66) is not traced.
            0x14 => {
                if mode != 1 || filter != 0x1B {
                    return refuse();
                }
                // 0x26C4B0(g ^ 1, 0x4C) and 0x26C4B0(0, 0x2E) both search
                // with mask 3 (0x26C594: the named group first, then the
                // other), so either group's trap counts.
                if self
                    .far_trap_in(&self.foes_then_all(unit.side), 0x4C, cards)
                    .is_some()
                {
                    return Err(unsupported("trap effect type 0x4c"));
                }
                // 0x25356C jumps over the Decree query when no type0x4C
                // trap exists. Decree alone cannot prevent Golgoil returning.
                let Some(cell) = self.leader_neighbour_cell(unit.side, true) else {
                    return Ok(Aftermath::Continue);
                };
                events.push(DuelEvent::Activated {
                    unit: unit.id,
                    card: unit.card,
                    target: None,
                });
                self.advance(unit.id, cell, events);
                Ok(Aftermath::Survives)
            }
            // Needle Ball / Yaiba Robo: event 9 as for the damage magic cards
            // (F02.md §4, MONSTER-EFFECTS.md §1c).
            0xD if mode == 2 && hi == 1 => {
                events.push(DuelEvent::Activated {
                    unit: unit.id,
                    card: unit.card,
                    target: None,
                });
                self.direct_damage(unit.side, lo as u16, cards, events);
                Ok(Aftermath::Continue)
            }
            4 => match filter {
                // F04a.md: destroy the enemy that destroyed this monster
                // (mode bits and word1 unread; no protection checks).
                0x1A => {
                    if let Some(o) = opponent {
                        events.push(DuelEvent::Activated {
                            unit: unit.id,
                            card: unit.card,
                            target: Some(o.id),
                        });
                        self.remove_by_effect(o.id, cards, events)?;
                    }
                    Ok(Aftermath::Continue)
                }
                // F04b.md §2: Blast Juggler / Blast Sphere, event 0x2E with
                // the record overwritten to {0x2E, self, 0, 1}: the dying unit
                // and every non-leader unit of the 3x3 square around its own
                // cell die, centre first then column by column. Only cards
                // 506/509 get the 0x2E code (another id would raise event
                // 0x15 with an empty mask; no such card exists).
                0x1F if matches!(unit.card, 506 | 509) => {
                    events.push(DuelEvent::Activated {
                        unit: unit.id,
                        card: unit.card,
                        target: None,
                    });
                    let mut victims = vec![unit.id];
                    for dc in -1..=1_i32 {
                        for dr in -1..=1_i32 {
                            if dc == 0 && dr == 0 {
                                continue;
                            }
                            let (c, r) = (
                                i32::from(unit.position.column) + dc,
                                i32::from(unit.position.row) + dr,
                            );
                            if !(0..7).contains(&c) || !(0..7).contains(&r) {
                                continue;
                            }
                            let cell = GridPos {
                                column: c as u8,
                                row: r as u8,
                            };
                            if let Some(v) = self.unit_at(cell).filter(|v| v.slot != 0) {
                                victims.push(v.id);
                            }
                        }
                    }
                    for v in victims {
                        self.remove_by_effect(v, cards, events)?;
                    }
                    Ok(Aftermath::BattleOver)
                }
                // F04b.md §3: Hinotama Soul, event 0x17: every non-monster
                // card of either side within Manhattan distance `hi` of the
                // attack target cell, in mask order; nothing without targets.
                0x1E if mode == 3 && !matches!(hi, 0x12 | 0x14 | 0x16 | 0x17 | 0x18 | 0x1D) => {
                    let mut targets: Vec<&DuelUnit> = self
                        .units
                        .iter()
                        .filter(|u| {
                            u.slot != 0
                                && cards.card(u.card).is_some_and(|c| !c.is_monster())
                                && u32::from(
                                    u.position.column.abs_diff(target.column)
                                        + u.position.row.abs_diff(target.row),
                                ) <= hi
                        })
                        .collect();
                    targets.sort_by_key(|u| (u.side, u.slot));
                    let targets: Vec<UnitId> = targets.iter().map(|u| u.id).collect();
                    if targets.is_empty() {
                        return Ok(Aftermath::Continue);
                    }
                    events.push(DuelEvent::Activated {
                        unit: unit.id,
                        card: unit.card,
                        target: None,
                    });
                    for t in targets {
                        self.remove_by_effect(t, cards, events)?;
                    }
                    Ok(Aftermath::Continue)
                }
                _ => refuse(),
            },
            // F05a.md / F05b.md: permanent -lo ATK/DEF (floored at 0) on the
            // killer (filter 0x1A) or on the 0x24E360 selection from the
            // dying unit's own cell (hi = scope code, event 0x39).
            0x13 => {
                let targets: Vec<UnitId> = if filter == 0x1A {
                    opponent.map(|o| o.id).into_iter().collect()
                } else {
                    let mut targets: Vec<UnitId> = Vec::new();
                    for u in &self.units {
                        if u.slot == 0 || u.id == unit.id {
                            continue;
                        }
                        if other_dies && other.is_some_and(|o| o.id == u.id) {
                            continue;
                        }
                        let side_ok = match mode {
                            1 => u.controller() == unit.controller(),
                            2 => self.foe(u.controller(), unit.controller()),
                            3 => true,
                            _ => false,
                        };
                        if !side_ok {
                            continue;
                        }
                        let c = &*self.unit_card(u, cards);
                        let (p, q) = (u.position, unit.position);
                        let (in_scope, filter_ok) = match hi {
                            // The plain list: Manhattan radius 14 from (0,0).
                            0 => (true, select_filter(filter, c)?),
                            // Same column or row; only 0x1D/0x1E/0x1F match.
                            0x12 => (
                                p.column == q.column || p.row == q.row,
                                match filter {
                                    0x1D => c.is_monster(),
                                    0x1E => !c.is_monster(),
                                    0x1F => true,
                                    _ => false,
                                },
                            ),
                            0x14 | 0x16 | 0x17 | 0x18 | 0x1D => return refuse(),
                            radius => (
                                u32::from(p.column.abs_diff(q.column) + p.row.abs_diff(q.row))
                                    <= radius,
                                select_filter(filter, c)?,
                            ),
                        };
                        if in_scope && filter_ok {
                            targets.push(u.id);
                        }
                    }
                    targets
                };
                if targets.is_empty() || lo == 0 {
                    return Ok(Aftermath::Continue);
                }
                events.push(DuelEvent::Activated {
                    unit: unit.id,
                    card: unit.card,
                    target: (filter == 0x1A).then(|| targets[0]),
                });
                for t in targets {
                    self.lower_stats(t, lo as i32, cards, events);
                }
                Ok(Aftermath::Continue)
            }
            _ => refuse(),
        }
    }

    /// Entry-4 type 0x38 (0x2539BC, event 1 0x2586A0): card `word1 & 0xFFFF`
    /// appears on the first free neighbour of the dead unit's controller's
    /// leader (hi 0x1F), unless that side has five monsters, the dying one
    /// included (MOTH-EFFECTS.md section 3).
    /// 0x24FB40(side, flag): the first free cell among the eight neighbours
    /// of `side`'s leader in the 0x24FD30 order (side 1 turned 180
    /// degrees); with `skip_labyrinth` (flag 1) a Labyrinth cell never
    /// counts. None when all are taken.
    fn leader_neighbour_cell(&self, side: u8, skip_labyrinth: bool) -> Option<GridPos> {
        self.leader_neighbour_cells(side, skip_labyrinth)
            .first()
            .copied()
    }

    /// 0x24FC20(side, n, flag) for n = 0..: the free neighbours of `side`'s
    /// leader in the 0x24FD30 order; element n is 0x24FC20's n-th answer
    /// (its first is 0x24FB40's).
    fn leader_neighbour_cells(&self, side: u8, skip_labyrinth: bool) -> Vec<GridPos> {
        let leader = self.leader(side).position;
        const ORDER: [(i8, i8); 8] = [
            (0, -1),
            (0, 1),
            (1, 0),
            (-1, 0),
            (1, -1),
            (-1, -1),
            (1, 1),
            (-1, 1),
        ];
        let turn = if side == 0 { 1 } else { -1 };
        ORDER
            .iter()
            .filter_map(|&(dc, dr)| {
                let c = leader.column as i8 + dc * turn;
                let r = leader.row as i8 + dr * turn;
                let at = GridPos {
                    column: u8::try_from(c).ok()?,
                    row: u8::try_from(r).ok()?,
                };
                (at.index().is_some()
                    && self.unit_at(at).is_none()
                    && (!skip_labyrinth || self.terrain_at(at) != terrain::LABYRINTH))
                    .then_some(at)
            })
            .collect()
    }

    fn summon_beside_leader(
        &mut self,
        dead: &DuelUnit,
        word1: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        if word1 >> 16 != 0x1F {
            return Err(unsupported(format!(
                "destruction effect of card {}",
                dead.card
            )));
        }
        let card = (word1 & 0xFFFF) as u16;
        let side = dead.side;
        let Some(cell) = self.leader_neighbour_cell(side, true) else {
            return Ok(());
        };
        let monsters = self
            .units
            .iter()
            .filter(|u| u.side == side && (1..=MONSTER_SLOTS as u8).contains(&u.slot))
            .count();
        if cards.card(card).is_some_and(|c| c.is_monster()) && monsters >= MONSTER_SLOTS {
            return Ok(());
        }
        events.push(DuelEvent::Activated {
            unit: dead.id,
            card: dead.card,
            target: None,
        });
        self.create(side, card, cell, cards, events);
        Ok(())
    }

    /// 0x1E0800 -> 0x1DDFE0: a fresh unit for `side` on `cell` in the lowest
    /// free slot of its zone: face-down, attack position, not acted, the
    /// next summon order. None when the zone is full.
    fn create(
        &mut self,
        side: u8,
        card: u16,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Option<UnitId> {
        let info = cards.card(card)?;
        let used: Vec<u8> = self
            .units
            .iter()
            .filter(|u| u.side == side && u.slot != 0)
            .map(|u| u.slot)
            .collect();
        let mut slots = if info.is_monster() {
            1..=MONSTER_SLOTS as u8
        } else {
            OTHER_SLOTS
        };
        let slot = slots.find(|n| !used.contains(n))?;
        let discount = self.summon_discount(side, card, cards);
        let s = &mut self.sides[usize::from(side)];
        let order = s.summons;
        s.summons = s.summons.saturating_add(1);
        let id = UnitId(self.next_unit);
        self.next_unit += 1;
        self.units.push(DuelUnit {
            id,
            card,
            side,
            slot,
            position: cell,
            face_up: false,
            defence: false,
            committed_face_up: false,
            committed_defence: false,
            acted: false,
            spellbind: 0,
            summon_order: order,
            attack_bonus: 0,
            defense_bonus: 0,
            spellbind_fresh: false,
            evo_counter: 0,
            stolen: false,
            discount,
            stolen_by: None,
        });
        events.push(DuelEvent::Created {
            unit: id,
            card,
            at: cell,
        });
        Some(id)
    }

    /// Turn start 0x269570 for the active side (MOTH-EFFECTS.md sections 1
    /// and 4): phase 0 counts down the entry-2 type 0xC counters of its
    /// face-up defence units in slots 1..5 (never below 1); phases
    /// 0x21-0x25 then run each such unit's entry 2 in slot order: type 0xC
    /// at counter 1 replaces it with its next form on the same cell (old card
    /// to the graveyard, no entry 4; the new unit is fresh, in the lowest
    /// free slot); type 0x4D lowers every enemy non-leader monster by the
    /// value, not below zero.
    fn turn_start_effects(
        &mut self,
        s: u8,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let ready = |u: &DuelUnit| {
            u.side == s
                && (1..=MONSTER_SLOTS as u8).contains(&u.slot)
                && u.committed_face_up
                && u.committed_defence
        };
        // 0x2696D0: a Timeater makes it 0x252180(1, n - 2) (MOTH-EFFECTS.md
        // emu J: 6 -> 4; K: 2 -> 1).
        let step = if self.continuous_count(0x55, cards) > 0 { 2 } else { 1 };
        for u in self.units.iter_mut() {
            if ready(u) && u.evo_counter > 0 {
                u.evo_counter = u.evo_counter.saturating_sub(step).max(1);
            }
        }
        for slot in 1..=MONSTER_SLOTS as u8 {
            let Some(u) = self
                .units
                .iter()
                .find(|u| ready(u) && u.slot == slot)
                .cloned()
            else {
                continue;
            };
            let Some([word0, word1]) = cards.card(u.card).and_then(|c| c.entries[2]) else {
                continue;
            };
            match word0 >> 8 {
                0xC if u.evo_counter == 1 => {
                    events.push(DuelEvent::Activated {
                        unit: u.id,
                        card: u.card,
                        target: None,
                    });
                    // Event 0x3C handler 0x26ABF0 -> 0x250C90 (0x26ADC0).
                    self.remove_by_effect(u.id, cards, events)?;
                    self.create(s, (word1 & 0xFFFF) as u16, u.position, cards, events);
                }
                // Type 0x4E (0x267F64; 108 Pumpking the King of Ghosts: mode
                // 3, filter 2 Zombie, lo 100): the same turn-start trigger as
                // 0x4D; mask = 0x24E360(0, 0, ctrl, mode, filter, 0xE) over
                // the non-leader monsters of the filter's kind in slots 1..10
                // of both groups whose controller fits the mode (any face,
                // itself included, no 0x273C60 immunity filter); empty →
                // nothing; else event {0x1D, 1, mask, lo}: permanent ATK/DEF
                // +lo on every target (0x25EF60, as Hoshiningen's flip).
                0x4E => {
                    let (mode, filter) = ((word0 & 0xFF) >> 6, word0 & 0x3F);
                    let gain = (word1 & 0xFFFF) as i32;
                    let mut matched: Vec<&DuelUnit> = self
                        .units
                        .iter()
                        .filter(|t| {
                            t.slot != 0
                                && filter_matches(filter, &self.unit_card(t, cards))
                                && match mode {
                                    1 => t.controller() == s,
                                    2 => self.foe(t.controller(), s),
                                    _ => true,
                                }
                        })
                        .collect();
                    matched.sort_by_key(|t| (t.side, t.slot));
                    let targets: Vec<UnitId> = matched.into_iter().map(|t| t.id).collect();
                    if targets.is_empty() {
                        continue;
                    }
                    events.push(DuelEvent::Activated {
                        unit: u.id,
                        card: u.card,
                        target: None,
                    });
                    for id in targets {
                        self.boost_capped(id, gain, cards, events);
                    }
                }
                // Type 0x54 (676 Carat Idol): the pull of event 0x48
                // (sweep_rules.rs).
                0x54 => self.carat_idol_pull(&u, word0, cards, events)?,
                // Type 0x53 (0x268080; 673 Souleater, lo 200; mode and
                // filter unread): every monster card leaves both graveyards
                // (group 0 then 1, 0x1E0280 then the compaction 0x1E01A0)
                // and record {0x1D, 1, its own bit, lo * count}: the
                // Souleater's permanent ATK/DEF + lo per card, the record
                // written even for 0 [F; emu REFUSALS-SWEEP.md SE1: ten
                // monsters gone, +2000/+2000; SE2 attack position: nothing].
                0x53 => {
                    let mut count = 0i32;
                    for side in &mut self.sides {
                        let before = side.graveyard.len();
                        side.graveyard
                            .retain(|&c| !cards.card(c).is_some_and(CardInfo::is_monster));
                        count += (before - side.graveyard.len()) as i32;
                    }
                    events.push(DuelEvent::Activated {
                        unit: u.id,
                        card: u.card,
                        target: None,
                    });
                    let gain = ((word1 & 0xFFFF) as i32 * count) & 0xFFFF;
                    self.boost_capped(u.id, gain, cards, events);
                }
                // Type 0xD (0x267EB4; 95 Injection Fairy Lily, lo 50):
                // record {8, 0, ctrl, word1}: event 8, the LP gain of the
                // magic cards (0x25A740), lo for the controller. [F; emu
                // L1: 2700 -> 2750 at its own turn start]
                0xD => {
                    events.push(DuelEvent::Activated {
                        unit: u.id,
                        card: u.card,
                        target: None,
                    });
                    self.lp_gain(u.controller(), (word1 & 0xFFFF) as u16, cards, events);
                }
                0x4D => {
                    let loss = (word1 & 0xFFFF) as i32;
                    let targets: Vec<UnitId> = self
                        .units
                        .iter()
                        .filter(|t| {
                            t.controller() != s
                                && t.slot != 0
                                && cards.card(t.card).is_some_and(|c| c.is_monster())
                        })
                        .map(|t| t.id)
                        .collect();
                    if targets.is_empty() {
                        continue;
                    }
                    events.push(DuelEvent::Activated {
                        unit: u.id,
                        card: u.card,
                        target: None,
                    });
                    for id in targets {
                        let t = self.units.iter_mut().find(|t| t.id == id).unwrap();
                        let c = cards.card(t.card).expect("validated card");
                        // [H] floored at the card's base value, as event 0x2C.
                        let floor = |base: u16, bonus: i16| {
                            (i32::from(bonus) - loss).max(-i32::from(base)) as i16
                        };
                        t.attack_bonus = floor(c.attack, t.attack_bonus);
                        t.defense_bonus = floor(c.defense, t.defense_bonus);
                        events.push(DuelEvent::Modified {
                            unit: id,
                            attack: t.attack_bonus,
                            defense: t.defense_bonus,
                        });
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }

    /// A unit moving onto an own unit that is not a monster pair (0x249860,
    /// experiments E2/E3, MONSTER-EFFECTS.md §2.3 and §3.2):
    /// - monster onto a power-up that fits: +value on ATK and DEF, the card is
    ///   used and the monster takes its cell (its action is used);
    /// - power-up onto a monster it fits: the monster gains the value and
    ///   stays; the power-up is used;
    /// - a pairing that fails: the stationary unit is destroyed and the mover
    ///   takes its cell (a monster onto an unfitting power-up, a power-up onto
    ///   a monster it does not fit, any card onto a card, a leader onto a
    ///   card).
    ///
    /// A monster not committed face-up stays face-down.
    fn combine(
        &mut self,
        mover: UnitId,
        target: UnitId,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<bool> {
        let m = self.unit(mover).expect("mover").clone();
        let t = self.unit(target).expect("target").clone();
        let mcard = self.unit_card(&m, cards).into_owned();
        let tcard = self.unit_card(&t, cards).into_owned();
        if m.slot == 0 {
            // A leader destroys the own unit and moves in (emu).
            if tcard.is_monster() {
                // Event 0x5F (0x276F10 -> 0x250C90).
                self.remove_by_effect(target, cards, events)?;
            } else {
                self.discard(target, events);
            }
            self.advance(mover, t.position, events);
            self.crush_sweep(cards, events)?;
            return Ok(true);
        }
        if mcard.is_monster() && tcard.is_monster() {
            return self.field_fusion(mover, target, cards, events);
        }
        let (monster, power_up) = match (mcard.class(), tcard.class()) {
            (class::MONSTER, class::POWER_UP) if m.slot != 0 => (Some(&mcard), Some(&tcard)),
            (class::POWER_UP, class::MONSTER) if t.slot != 0 => (Some(&tcard), Some(&mcard)),
            _ => (None, None),
        };
        if let (Some(monster), Some(power_up)) = (monster, power_up) {
            let data = power_up.power_up.ok_or_else(|| {
                unsupported(format!("power-up {} without imported data", power_up.id))
            })?;
            if power_up_fits(monster, power_up.id) {
                let (monster_id, power_id) =
                    if mcard.is_monster() { (mover, target) } else { (target, mover) };
                // Category 4 on an own monster: event 0x2B as for the
                // enemy (sweep_rules.rs), the result in the monster's slot
                // on the target cell, acted only when the monster moved
                // [emu REFUSALS-SWEEP.md OC1 (Cocoon onto own Acid Crawler:
                // Larva not acted), OC2 (Acid Crawler onto own Cocoon:
                // acted)].
                if data.category == 4 {
                    self.transform_by_power_up(monster_id, power_id, t.position, mcard.is_monster(), cards, events)?;
                    self.crush_sweep(cards, events)?;
                    return Ok(false);
                }
                // Riryoku on an own monster: event 0x25 as when attacked; the
                // Riryoku owner's opponent pays [emu OR1 (Riryoku onto own
                // Red-Eyes: +1700, side 1 3400 -> 1700 per application),
                // OR2 (Red-Eyes onto own Riryoku: the same, it moves in)].
                // The trap answers are traced for the attacked case only.
                if data.category == 0 && data.value == 0 {
                    if self.living_sides().into_iter().any(|g| {
                        self.far_trap(g, trap_type::GOBLIN_FAN, cards).is_some()
                            || self.far_trap(g, 0x2B, cards).is_some()
                    }) {
                        return Err(unsupported("Riryoku on an own monster with an equip trap on the field"));
                    }
                    self.riryoku_attacked(monster_id, power_id, cards, events)?;
                    self.discard(power_id, events);
                    if mcard.is_monster() {
                        self.advance(mover, t.position, events);
                    }
                    self.crush_sweep(cards, events)?;
                    return Ok(true);
                }
                if !matches!(data.category, 0..=3) {
                    return Err(unsupported(format!(
                        "power-up {} category {} on the field",
                        power_up.id, data.category
                    )));
                }
                self.ordinary_field_equip_guard(data.category, cards)?;
                if mcard.is_monster() {
                    self.discard(target, events);
                    self.advance(mover, t.position, events);
                    self.apply_ordinary_field_equip(
                        mover,
                        data.category,
                        data.value,
                        cards,
                        events,
                    );
                    self.keep_face_down(mover);
                } else {
                    // The monster does not move and keeps its turn.
                    self.discard(mover, events);
                    self.apply_ordinary_field_equip(
                        target,
                        data.category,
                        data.value,
                        cards,
                        events,
                    );
                }
                self.crush_sweep(cards, events)?;
                return Ok(true);
            }
        }
        // Failed pairing (0x3E7 / event 0x5F): the stationary unit goes.
        if tcard.is_monster() {
            // Events 0x24 (0x26B4D8) / 0x5F (0x277054): 0x250C90.
            self.remove_by_effect(target, cards, events)?;
        } else {
            self.discard(target, events);
        }
        self.advance(mover, t.position, events);
        self.keep_face_down(mover);
        self.crush_sweep(cards, events)?;
        Ok(true)
    }

    /// Monster onto own monster (0x249860 → event 0x64, 0x264500; emu): with
    /// a fusion the mover's card goes to the graveyard, the result is created
    /// fresh in the lowest free slot, and the target is destroyed. The result
    /// moves into the target cell and is marked acted, except that the
    /// original looks the attacker up by its old slot: a result in a lower
    /// slot stays on the mover's cell and is not marked acted. Without a
    /// fusion the target is destroyed and the mover moves in.
    fn field_fusion(
        &mut self,
        mover: UnitId,
        target: UnitId,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<bool> {
        let m = self.unit(mover).expect("mover").clone();
        let t = self.unit(target).expect("target").clone();
        let result = cards
            .fusion(m.card, t.card)
            .ok_or_else(|| unsupported("fusion without an imported fusion table"))?;
        let Some(result) = result else {
            // Event 0x64 handler 0x264500 -> 0x250C90 (0x264854).
            self.remove_by_effect(target, cards, events)?;
            self.advance(mover, t.position, events);
            self.crush_sweep(cards, events)?;
            return Ok(true);
        };
        // 0x1E00F0 and the rebuild 0x1E0D70: no experience for the mover.
        self.units.retain(|u| u.id != mover);
        self.sides[usize::from(m.side)].graveyard.push(m.card);
        let used: Vec<u8> = self
            .units
            .iter()
            .filter(|u| u.side == m.side && u.slot != 0)
            .map(|u| u.slot)
            .collect();
        let slot = (1..=MONSTER_SLOTS as u8)
            .find(|n| !used.contains(n))
            .expect("the mover's slot is free");
        // 0x1E0D70 -> 0x1E0870: the rebuilt record gets a fresh +0x48.
        let discount = self.summon_discount(m.side, result, cards);
        let side = &mut self.sides[usize::from(m.side)];
        let order = side.summons;
        side.summons = side.summons.saturating_add(1);
        let found = slot == m.slot;
        self.units.push(DuelUnit {
            id: mover,
            card: result,
            side: m.side,
            slot,
            position: m.position,
            face_up: false,
            defence: false,
            committed_face_up: false,
            committed_defence: false,
            acted: false,
            spellbind: 0,
            summon_order: order,
            attack_bonus: 0,
            defense_bonus: 0,
            spellbind_fresh: false,
            evo_counter: 0,
            stolen: false,
            discount,
            stolen_by: None,
        });
        events.push(DuelEvent::Fused {
            cards: [m.card, t.card],
            result,
        });
        // 0x250C90 (0x2649D0).
        self.remove_by_effect(target, cards, events)?;
        // Not found by (group, slot): no advance and no commit.
        if found {
            self.advance(mover, t.position, events);
        }
        self.crush_sweep(cards, events)?;
        Ok(found)
    }

    fn advance(&mut self, id: UnitId, to: GridPos, events: &mut Vec<DuelEvent>) {
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            let from = u.position;
            u.position = to;
            events.push(DuelEvent::Moved { unit: id, from, to });
        }
    }

    /// 0x2527A0/0x2525C0 op 0 (event 0x1D, F01.md §1.4): permanent +value
    /// on DEF then ATK; when clamp(base + delta) + value would reach 10000
    /// the delta becomes 9999 - base instead (deltas stored within ±9999).
    fn boost_capped(
        &mut self,
        id: UnitId,
        value: i32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        let Some(u) = self.units.iter_mut().find(|u| u.id == id) else {
            return;
        };
        let card = cards.card(u.card).expect("validated card");
        let raise = |base: u16, delta: i16| -> i16 {
            let (base, delta) = (i32::from(base), i32::from(delta));
            let current = (base + delta).clamp(0, i32::from(MAX_LP));
            if current + value >= 10000 {
                i32::from(MAX_LP) - base
            } else {
                delta + value
            }
            .clamp(-i32::from(MAX_LP), i32::from(MAX_LP)) as i16
        };
        u.defense_bonus = raise(card.defense, u.defense_bonus);
        u.attack_bonus = raise(card.attack, u.attack_bonus);
        events.push(DuelEvent::Modified {
            unit: id,
            attack: u.attack_bonus,
            defense: u.defense_bonus,
        });
    }

    /// 0x2527A0/0x2525C0 op 1 (event 0x39, F05b.md §4): permanent -amount
    /// on DEF then ATK; when clamp(base + delta) - amount would go below 0
    /// the delta becomes -base (the stat reads exactly 0).
    fn lower_stats(
        &mut self,
        id: UnitId,
        amount: i32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        let Some(u) = self.units.iter_mut().find(|u| u.id == id) else {
            return;
        };
        let card = cards.card(u.card).expect("validated card");
        let lower = |base: u16, delta: i16| -> i16 {
            let (base, delta) = (i32::from(base), i32::from(delta));
            let current = (base + delta).clamp(0, i32::from(MAX_LP));
            if current - amount >= 0 {
                delta - amount
            } else {
                -base
            }
            .clamp(-i32::from(MAX_LP), i32::from(MAX_LP)) as i16
        };
        u.defense_bonus = lower(card.defense, u.defense_bonus);
        u.attack_bonus = lower(card.attack, u.attack_bonus);
        events.push(DuelEvent::Modified {
            unit: id,
            attack: u.attack_bonus,
            defense: u.defense_bonus,
        });
    }

    /// Mirror Force, event 0x36 type 4 (handler 0x26CDB0): init reveals
    /// the trap (0x252FF0); stage 0 marks it acted and sends it to the
    /// graveyard (0x250C90, every type but 0x31); stage 1 takes
    /// 0x24E360(0, 0, defender's controller, mode 2, filter 0x35, 0xE) =
    /// every unit in slots 1..10 controlled by the other side whose +0x57
    /// bit 3 (defence) is clear, any class and face (face-down attack
    /// monsters and set cards left in attack position included; leaders
    /// never), then 0x273C60(mask, trap's controller) drops the Dragons the
    /// immunity keeps (ability #11 is refused at setup); stage 3 (0x260F90)
    /// sends each through 0x250C90, group 0 then 1, by slot (no entry-4
    /// effect). Stage 2 draws one rand() for the camera (0x260D20 ->
    /// 0x2504F0); not consumed here, like Barrel Dragon's animation draw.
    fn mirror_force(
        &mut self,
        trap: UnitId,
        att: UnitId,
        defender_ctrl: u8,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let trap_ctrl = self.unit(trap).expect("trap").controller();
        self.activate(trap, Some(att), events);
        self.discard(trap, events);
        let mut mask: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| (1..=10).contains(&u.slot) && self.foe(u.controller(), defender_ctrl) && !u.defence)
            .collect();
        mask.sort_by_key(|u| (u.side, u.slot));
        let targets: Vec<UnitId> = mask
            .into_iter()
            .filter(|u| self.magic_target_admitted(u, trap_ctrl, cards))
            .map(|u| u.id)
            .collect();
        for t in targets {
            self.remove_by_effect(t, cards, events)?;
        }
        Ok(())
    }

    /// Mirror Wall's stage 3 (0x26D330): x = clamp(card ATK + ATK change,
    /// 0, 9999) >> 1, then 0x2525C0(g, s, 1, x): the ATK change drops by x
    /// (0x1E16D0 op 1, stored within ±9999). The fallback of 0x2525C0 for a
    /// result below 0 cannot apply. DEF is untouched. [F; emu W1/W3/W4]
    fn mirror_wall_halve(
        &mut self,
        id: UnitId,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        let Some(u) = self.units.iter_mut().find(|u| u.id == id) else {
            return;
        };
        let card = cards.card(u.card).expect("validated card");
        let current =
            (i32::from(card.attack) + i32::from(u.attack_bonus)).clamp(0, i32::from(MAX_LP));
        let max = i32::from(MAX_LP);
        u.attack_bonus = (i32::from(u.attack_bonus) - (current >> 1)).clamp(-max, max) as i16;
        events.push(DuelEvent::Modified {
            unit: id,
            attack: u.attack_bonus,
            defense: u.defense_bonus,
        });
    }

    /// 0x26AEF0: a monster not committed face-up stays face-down.
    fn keep_face_down(&mut self, id: UnitId) {
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            if !u.committed_face_up && u.slot != 0 {
                u.face_up = false;
            }
        }
    }

    /// A revealed ritual card at the end of its action (0x253D60 phase 7 ->
    /// builder 0x2560B8; event 0x38 -> 0x26D4B0 and script c059; RITUAL.md):
    /// after the negation chain the card goes to the graveyard, then (if
    /// three sacrifices are found) they go to their graveyards in slot order
    /// and the result appears on the ritual's cell for the active side,
    /// fresh, in the lowest free monster slot. No cost; the new monster may
    /// act this turn. The Seal of the Ancients is refused rather than modelled.
    fn activate_ritual(
        &mut self,
        id: UnitId,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let unit = self.unit(id).expect("ritual card").clone();
        let card = cards.card(unit.card).expect("validated card").clone();
        let [word0, word1] = card.entries[0]
            .ok_or_else(|| unsupported(format!("ritual card {} without an effect", unit.card)))?;
        if self.negation_chain(id, word0 >> 8, cards, events)? {
            return Ok(());
        }
        let s = unit.side;
        let result = (word1 & 0xFFFF) as u16;
        let words = cards
            .ritual(result)
            .ok_or_else(|| unsupported("rituals without an imported requirement table"))?;
        let sacrifices = match words {
            Some(words) => self.ritual_sacrifices(s, words, cards)?,
            None => None,
        };
        self.activate(id, None, events);
        self.discard(id, events);
        let Some(mut sacrifices) = sacrifices else {
            return Ok(());
        };
        sacrifices.sort_by_key(|&u| {
            let u = self.unit(u).expect("sacrifice");
            (u.side, u.slot)
        });
        for u in sacrifices {
            // Event 0x38 handler 0x26D4B0 -> 0x250C90 (0x26D694).
            self.remove_by_effect(u, cards, events)?;
        }
        // c059 frame 0x87 -> 0x1E0800: no crush check on this path.
        let active = self.active;
        self.create(active, result, unit.position, cards, events);
        Ok(())
    }

    /// 0x2561B0: three distinct units `side` controls (slots 1..10) meeting
    /// the three words, cheapest by score = ATK read at cell (col, col) plus
    /// DEF at the unit's cell (the original's quirk), over the six orders of
    /// table 0x2D3300; each word takes the lowest score, then the lowest
    /// summon order (0xFF never), then group and slot order. Ties between
    /// orders go to the first. `None` when no order finds three.
    fn ritual_sacrifices(
        &self,
        side: u8,
        words: [u32; 3],
        cards: &impl CardData,
    ) -> Result<Option<Vec<UnitId>>> {
        let mut candidates: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| u.side == side && (1..=10).contains(&u.slot))
            .collect();
        candidates.sort_by_key(|u| (u.side, u.slot));
        let fits = |u: &DuelUnit, word: u32| -> Result<bool> {
            let (f, p) = (word & 0xFFFF, (word >> 16) as u16);
            let c = self.unit_card(u, cards);
            Ok(match f {
                0..=0x13 => c.is_monster() && u32::from(c.kind & 0x1F) == f,
                0x1C => u.card == p,
                0x25 => c.is_monster() && self.effective(u, u.position, true, cards) <= p,
                0x39 => c.is_monster() && self.effective(u, u.position, false, cards) >= p,
                _ => return Err(unsupported(format!("ritual requirement filter {f:#x}"))),
            })
        };
        let score = |u: &DuelUnit| {
            let quirk = GridPos {
                column: u.position.column,
                row: u.position.column,
            };
            u32::from(self.effective(u, quirk, true, cards))
                + u32::from(self.effective(u, u.position, false, cards))
        };
        const ORDERS: [[usize; 3]; 6] = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let mut best: Option<(u32, Vec<UnitId>)> = None;
        // The original tries k = 5..0 and keeps a total that is <= the best.
        for order in ORDERS.iter().rev() {
            let mut chosen: Vec<UnitId> = Vec::new();
            for &i in order {
                let mut pool = Vec::new();
                for &u in &candidates {
                    if !chosen.contains(&u.id) && fits(u, words[i])? {
                        pool.push(u);
                    }
                }
                let Some(low) = pool.iter().map(|u| score(u)).min() else {
                    break;
                };
                let mut pick: Option<(u8, UnitId)> = None;
                for u in pool.iter().filter(|u| score(u) == low) {
                    if u.summon_order < pick.map_or(0xFF, |(o, _)| o) {
                        pick = Some((u.summon_order, u.id));
                    }
                }
                let Some((_, id)) = pick else {
                    break;
                };
                chosen.push(id);
            }
            if chosen.len() == 3 {
                let total = chosen
                    .iter()
                    .map(|&id| score(self.unit(id).expect("candidate")))
                    .sum();
                if best.as_ref().is_none_or(|(t, _)| total <= *t) {
                    best = Some((total, chosen));
                }
            }
        }
        Ok(best.map(|(_, chosen)| chosen))
    }

    /// A revealed magic card at the end of its action (state 0x38 →
    /// 0x253D60): the negation chain, then the card goes to the graveyard
    /// before its effect (0x2547E0) applies.
    fn activate_magic(
        &mut self,
        id: UnitId,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let unit = self.unit(id).expect("magic card").clone();
        let card = cards.card(unit.card).expect("validated card").clone();
        let s = unit.side;
        let [word0, word1] = card.entries[0]
            .ok_or_else(|| unsupported(format!("magic card {} without an effect", unit.card)))?;
        let (ty, low) = (word0 >> 8, word0 & 0xFF);
        let (mode, filter) = (low >> 6, low & 0x3F);
        let (hi, lo) = (word1 >> 16, word1 & 0xFFFF);
        // The chain runs before the builder, so a card of an unsupported
        // effect type still resolves into a negation (F06.md §6.7).
        if self.negation_chain(id, ty, cards, events)? {
            return Ok(());
        }
        // Checks before anything changes, so refusals leave no trace.
        match ty {
            2 => (),
            // Type 0xD (F02.md §1): mode 1 hi 0 own LP gain, mode 1 hi 4
            // Dian Keto (LP to at least lo), mode 2 hi 1 direct damage.
            0xD => match (mode, hi) {
                (1, 0) | (2, 1) => (),
                (1, 4) => {
                    // Dian Keto's builder path with a Simochi is not traced.
                    if self.far_trap_in(&self.foes(s), trap_type::SIMOCHI, cards).is_some() {
                        return Err(unsupported("Dian Keto with Bad Reaction to Simochi"));
                    }
                }
                _ => {
                    return Err(unsupported(format!(
                        "magic card {} (effect type 0xd variant)",
                        unit.card
                    )))
                }
            },
            // 739 Rain of Mercy's id gives record 0x5A (0x255A14) instead of
            // 0x1D; handler 0x263080 applies the same op-0 ATK/DEF boost as
            // 0x25EF60 (code only). 740 Windstorm of Etaqua's record 0x5C
            // (0x255A08, handler 0x263490) does the same with its own
            // presentation (0x1E5540) [F; emu REFUSALS-SWEEP.md WE1: both
            // sides' Winged Beasts +600/+600, any face].
            0x12 if filter < 0x1A => (),
            // Type 0x17 (0x254974; Dragon Capture Jar 683: mode 3, filter 0
            // Dragon, lo 0xFF): the mode/kind mask through 0x24E360, the
            // immunity filter 0x273C60 (ability #11 and Lord of D. are
            // refused elsewhere), event {0, 0, mask, lo} as Nightmare
            // Scorpion's: +0x49 := lo and the acted flag (0x1E30F0 bit 0)
            // on every target; nothing (0xFE) with an empty mask.
            0x17 if (1..=3).contains(&mode) && filter < 0x14 && (1..=0xFF).contains(&lo) => (),
            // Time Seal 684 (mode 3, filter 0x21, lo 0xFF): the same arm
            // with the "strongest monster" selection of 0x24E360 (crit
            // 0x21, 0x24F1C4) over the mode's controllers; the immunity
            // filter 0x273C60 runs on the mask [emu N1–N3: the strongest by
            // effective ATK at its own cell on either side, any face, gets
            // +0x49 = 255 and the acted flag; a shorter bind is replaced].
            0x17 if (1..=3).contains(&mode) && filter == 0x21 && (1..=0xFF).contains(&lo) => {
                self.guard_control_immunity(unit.controller(), cards)?;
            }
            // Magical Labyrinth 698 (0x254E08): nothing to check.
            0x48 => (),
            // Solomon's Lawbook 747 (0x255FFC; mode and filter unread).
            0x26 => (),
            // Shift 746 (0x255E0C: mode 1, filter 0x21, hi 0): the activator's
            // strongest monster moves onto the card's cell. hi 0x21 and
            // filter 0x22 take other branches (0x255E28 / 0x255EF4): not run.
            0x25 if mode == 1 && filter == 0x21 && hi == 0 => (),
            // Creature Swap 751 (0x255E28, hi 0x21; mode 3, filter 0x34 any
            // card): every unit mirrored across the diagonal (white_rules.rs).
            0x25 if hi == 0x21 && (1..=3).contains(&mode) && filter == 0x34 => (),
            // Type 0x20 (0x255994; Magical Neutralizing Force Field 734).
            0x20 => (),
            // Type 0x1F (0x2557D0): control change. Filter 0x33
            // (0x2558BC, 732 Change of Heart) opens the selection of state
            // 0x30 over the enemy group; filter 0x21 (0x255818, 733 Brain
            // Control) takes the strongest enemy-controlled monster
            // automatically; any other filter (0x255984) only discards the
            // card. The candidate mask is 0x24E360(0, 0, ctrl, mode, ...)
            // with the card's mode for 0x21 (both cards: mode 2); other
            // modes of 0x21 are not traced.
            // The Celtic Guardian test (0x268710(0x39)) comes first in the
            // builder: with one standing the card is wasted before the
            // immunity filter could matter.
            0x1F if filter == 0x33 || (filter == 0x21 && mode == 2) => {
                if !self.defender_with(0x39, cards) {
                    self.guard_control_immunity(unit.controller(), cards)?;
                }
            }
            // Type 0x14 (0x254A18; 687 Mimicat mode 3 filter 0x1F, 685
            // Monster Reborn mode 3 filter 0x1D, 688 Graverobber mode 3
            // filter 0x1E): a graveyard pick (state 0x2D). Mode 0
            // (0x254AAC: discarded for nothing) and other filters
            // (0x254CC0: discarded for nothing) are not run.
            0x14 if (1..=3).contains(&mode) && matches!(filter, 0x1D..=0x1F) => (),
            // Type 4 with hi 0 (0x2550A0; 699: mode 3, filter 0x1F): the
            // mask of every card in slots 1..10 of both groups minus the
            // card itself (0x24E0F0) and the immunity filter 0x273C60
            // (ability #11 remains refused; Lord of D. filters below), then by
            // card id: 699 → event {4, ctrl, mask, 0} (0x25515C); the other
            // ids of this branch (701, 721, 722, 727..730) and hi 0x13 /
            // 0x19 (0x255070 / 0x254F3C) emit other events, not traced.
            // Event 4's White Hole/Decree reactions are traced separately
            // in countertraps.rs. White Hole is not consumed. [F, emu]
            4 if hi == 0 && unit.card == 699 => {}
            // The same builder's default id branch (0x255408; 720 Warrior
            // Elimination, 723 Eradicating Aerosol, 724 Breath of Light,
            // 725 Eternal Draught): {0x15, 0, mask, 0} straight to the
            // destruction handler 0x25D770 (MANAWYDDAN-812-720-79.md §2,
            // emu E1-E3). 726 Fissure's filter 0x20 is the weakest-monster
            // criterion 0x24F324 (MORE-RULES-305-128-745-726.md §4).
            4 if hi == 0
                && ELIMINATIONS.contains(&unit.card)
                && (1..=3).contains(&mode)
                && (filter == 0x20 || select_filter(filter, &card).is_ok()) => {}
            // Type 0x36 (0x255CC8; 745 Call of the Haunted): a kind rewrite
            // of the mode/filter mask through event 0x42 (more_rules.rs).
            0x36 if select_filter(filter, &card).is_ok() => {}
            // Type 4 with hi 0x12 (0x254F3C; the compare at 0x254F2C is
            // against a1 = 0x12, not 0x19): the cross mask of the card's
            // cell, then by id: 702 (0x2BE, Harpie's Feather Duster: mode 3,
            // filter 0x1E) → event 7, the only reader of Fake Trap. 700
            // (0x2BC, Raigeki: mode 3, filter 0x1D) → event 5 with its
            // Anti-Raigeki / Decree reactions (E2-536-TRAP-0x4.md §3, emu
            // RG0-RG8); other ids are discarded for nothing (0x255060), not
            // run. TRAPS-0x23-0x31-FAKE-TRAP-FIELD.md §3. [F, emu F0-F9]
            4 if hi == 0x12
                && (unit.card == FEATHER_DUSTER || unit.card == RAIGEKI)
                && (1..=3).contains(&mode)
                && matches!(filter, 0x1D..=0x1F) => {}
            // Type 4 with hi 0x13 (0x254F20 → 0x255070; 731 Fairy Meteor
            // Crush): record 0x19 with ten random cells (white_rules.rs);
            // mode and filter unread.
            4 if hi == 0x13 => {}
            // Type 0x22 (0x255B7C; 737 Yellow Luster Shield: mode 1, filter
            // 0x1D, lo 900): DEF +lo on the mask (white_rules.rs).
            0x22 if (1..=3).contains(&mode) && select_filter(filter, &card).is_ok() => {}
            // Type 0x18 (0x25564C; 716 Dark-Piercing Light: mode 2, filter
            // 0x1F, lo 1): Patrol Robo's event {0xD, 4, mask, lo}.
            0x18 if (1..=3).contains(&mode) && select_filter(filter, &card).is_ok() => {}
            // Type 0x27 (0x25602C; 748 Magic Drain): the opponent's stars
            // move to the activator (white_rules.rs).
            0x27 => {}
            // Type 0x28 (0x256094; 750 Earthshaker, mode and filter unread):
            // event {0x23, ctrl, 0, 0}, the random shuffle (white_rules.rs).
            0x28 => {}
            // Type 0x1D (0x2556F4; 718 The Eye of Truth, mode and filter
            // unread): record {0xE, 0, ctrl ^ 1, 0}; handler 0x25BDE0 shows
            // the opponent's hand card by card (0x25C260 writes only its own
            // record) and changes nothing [F; emu REFUSALS-SWEEP.md ET1/ET2:
            // board, hands and graveyards unchanged but for the card].
            0x1D => {}
            // Type 0x24 (0x255D60; 744 Monster Recovery: mode 1, filter
            // 0x1D): the magic mask (minus the card, 0x273C60), record
            // {0x1E, 0, mask, ctrl}: The Wicked Worm Beast's return to the
            // deck per unit, mask order [F; emu REFUSALS-SWEEP.md MR1: both
            // own monsters, any face, appended to deck 0 after 8 draws].
            0x24 if (1..=3).contains(&mode) && select_filter(filter, &card).is_ok() => {}
            // Type 0x49 (0x2555A8; 715 Swords of Revealing Light: mode 2,
            // filter 0x1F, lo 3): reveal, acted, bind lo (white_rules.rs).
            0x49 if (1..=3).contains(&mode) && select_filter(filter, &card).is_ok() => {}
            // Type 0x25 filter 0x22 hi 0 (0x255EE8; 749 Dimensionhole): the
            // activator's leader moves onto the card's cell.
            0x25 if filter == 0x22 && hi == 0 => {}
            // Type 0x21 (0x255ADC; 736 Shield & Sword: mode 3, filter
            // 0x1D): ATK and DEF swapped on the mask (white_rules.rs).
            0x21 if (1..=3).contains(&mode) && select_filter(filter, &card).is_ok() => {}
            _ => {
                return Err(unsupported(format!(
                    "magic card {} (effect type {ty:#x})",
                    unit.card
                )))
            }
        }
        self.activate(id, None, events);
        self.discard(id, events);
        match ty {
            // Magical Labyrinth (0x254E08): the Labyrinth cells are listed
            // column by column (col 0..6 outer, row inner, 0x254E50); none →
            // the card is spent for nothing (no draw, emu E3); else one is
            // drawn (0x19AE88 % count) and event 3 (0x2583D0) runs the
            // terrain sub-scripts 0x1F6 {cell, 7} and 0x1F7 {card's cell,
            // 8} (c502/c503 → 0x1E34A0, MONSTER-EFFECTS.md §2.1): the wall
            // moves to the card's cell [emu E1/E2/E4, a wall under a unit
            // counts].
            0x48 => {
                let walls: Vec<GridPos> = (0..7u8)
                    .flat_map(|column| (0..7u8).map(move |row| GridPos { column, row }))
                    .filter(|&p| self.terrain_at(p) == terrain::LABYRINTH)
                    .collect();
                if walls.is_empty() {
                    return Ok(());
                }
                let wall = walls[self.rand31() as usize % walls.len()];
                if let Some(t) = wall.index().and_then(|i| self.terrain.get_mut(i)) {
                    *t = terrain::NORMAL;
                }
                if let Some(t) = unit.position.index().and_then(|i| self.terrain.get_mut(i)) {
                    *t = terrain::LABYRINTH;
                }
            }
            // Shift (0x255F0C): the mask 0x24E360(0, 0, ctrl, 1, 0x21, 0xE)
            // (the activator's strongest monster, 0x24F1C4) minus itself
            // and the immunity filter (a no-op over own units); event
            // {0x1F, ref, mask, pos} → 0x25FB20 stage 1: 0x1E0FD0 puts the
            // unit on the card's cell (free: the card left first), flags
            // and position untouched, no trap or Crush check; an empty mask
            // raises 0xFE (nothing) [emu F1–F4: the lower summon order wins
            // an equal pair].
            0x25 if hi == 0x21 => self.creature_swap(&unit, mode, cards, events),
            0x25 if filter == 0x22 => {
                // 0x255EF4: mask = the activator's leader (0x24E0A0(0, ctrl,
                // 0)), no immunity filter; event {0x1F, leader, mask, card
                // cell}: Shift's raw move. [F; emu DH1]
                let leader = self.leader(unit.controller()).id;
                self.advance(leader, unit.position, events);
            }
            0x25 => {
                if let Some(t) = self.strongest_monster(s, 1, cards)? {
                    self.advance(t, unit.position, events);
                }
            }
            // Solomon's Lawbook 747 (builder 0x255FFC → event {0x21, 0,
            // ctrl, 0}, handler 0x260760): once sub-script 0x215 is done,
            // 0x1DF980(ctrl, 1) sets S+0x346 bit 0 (`can_summon`) of the
            // card's controller: one more summon this turn. No reaction
            // reads event 0x21. [F; emu L0/L1, MANAWYDDAN-812-720-79.md §4]
            0x26 => self.sides[usize::from(unit.controller())].can_summon = true,
            // Field change (0x257E40, E4): every cell within the radius,
            // Labyrinth excepted, takes the terrain.
            2 => {
                let radius = ((word1 >> 16) & 0xFF) as u8;
                let terrain = (word1 & 0xFF) as u8;
                for (index, t) in self.terrain.iter_mut().enumerate() {
                    let (c, r) = ((index % 7) as u8, (index / 7) as u8);
                    let d = c.abs_diff(unit.position.column) + r.abs_diff(unit.position.row);
                    if d <= radius && *t != terrain::LABYRINTH {
                        *t = terrain;
                    }
                }
            }
            // Mode 2: event 9 (0x25ADF0), F02.md §2–§3; Just Desserts
            // (filter 0x32) multiplies by the opponent's units in slots
            // 1..=10, and an amount of 0 raises no event at all.
            0xD if mode == 2 => {
                let amount = if filter == 0x32 {
                    let n = self
                        .units
                        .iter()
                        .filter(|u| u.side != s && u.slot != 0)
                        .count() as u32;
                    (lo * n) & 0xFFFF
                } else {
                    lo
                };
                if amount != 0 {
                    self.direct_damage(s, amount as u16, cards, events);
                }
            }
            // Dian Keto (F02.md §5): the LP becomes lo unless already higher,
            // without the 0x2B modifier.
            0xD if hi == 4 => {
                let lp = &mut self.sides[usize::from(s)].lp;
                *lp = (*lp).max(lo.min(u32::from(MAX_LP)) as u16);
            }
            // LP gain (0x25A740, event 8), clamped to 9999.
            0xD => self.lp_gain(s, lo as u16, cards, events),
            // Stat boost (0x25EF60): non-leader monsters matching the filter
            // on the chosen sides gain the value on ATK and DEF.
            0x12 => {
                let (mode, filter) = (low >> 6, (low & 0x3F) as u8);
                let value = i16::try_from(word1 & 0xFFFF).unwrap_or(i16::MAX);
                let targets: Vec<UnitId> = self
                    .units
                    .iter()
                    .filter(|u| {
                        let own = u.controller() == s;
                        let side_ok = match mode {
                            1 => own,
                            2 => self.foe(u.controller(), s),
                            3 => true,
                            _ => false,
                        };
                        let c = self.unit_card(u, cards);
                        side_ok
                            // 0x255A74: enemy protected Dragons are removed
                            // from the magic boost's target mask.
                            && self.magic_target_admitted(u, unit.controller(), cards)
                            && u.slot != 0
                            && c.is_monster()
                            && if filter < 0x14 {
                                c.kind & 0x1F == filter
                            } else {
                                c.attribute == filter - 0x14
                            }
                    })
                    .map(|u| u.id)
                    .collect();
                for target in targets {
                    let u = self.units.iter_mut().find(|u| u.id == target).unwrap();
                    u.attack_bonus = u.attack_bonus.saturating_add(value);
                    u.defense_bonus = u.defense_bonus.saturating_add(value);
                    events.push(DuelEvent::Modified {
                        unit: target,
                        attack: u.attack_bonus,
                        defense: u.defense_bonus,
                    });
                }
            }
            // Card 699 (event 4, handler 0x258C60): stage 2 launches
            // {0x15, 0, mask, 0} through 0x257350 = 0x1F5B00(0, 0x15, ..),
            // the CMODULE launcher: record 0x15 is c021, the vortex
            // (DARK-HOLE-BLOCKER-GRAVEROBBER.md), not event 0x15's handler.
            // When c021 ends the handler's own pass (0x259154) removes every
            // unit of the mask through 0x250C90 (owner's graveyard, no entry
            // 4), group 0 then group 1 by slot (bit 21 - i). An empty mask
            // is record 0xFE (nothing; the card is spent, 0x255194).
            4 if hi == 0x13 => self.meteor_strikes(&unit, cards, events)?,
            0x22 => self.magic_defence_boost(&unit, mode, filter, lo, cards, events),
            // 0x25564C: the magic mask (minus the card, 0x273C60); event 0xD
            // mode 4: lo != 0 reveals (0x252FF0(g, s, 1)), 0 conceals.
            // [F; emu DL1]
            0x18 => {
                for t in self.magic_mask(&unit, mode, filter, cards) {
                    if lo == 0 {
                        self.conceal(t, events);
                    } else {
                        self.expose(t, events);
                    }
                }
            }
            0x27 => self.magic_drain(&unit, events),
            0x28 => self.earthshaker(events),
            0x49 => self.magic_reveal_bind(&unit, mode, filter, lo, cards, events),
            0x21 => self.magic_stat_swap(&unit, mode, filter, cards, events),
            4 if hi == 0x12 && unit.card == RAIGEKI => {
                self.raigeki(&unit, mode, filter, cards, events)?;
            }
            4 if hi == 0x12 => {
                self.feather_duster(&unit, mode, filter, cards, events)?;
            }
            4 if hi == 0 && ELIMINATIONS.contains(&unit.card) => {
                self.elimination(&unit, mode, filter, cards, events)?;
            }
            0x36 => self.magic_retype(&unit, mode, filter, word1, cards)?,
            4 => {
                // 0x2550DC: freeze the filtered mask before any source is
                // removed. Destroying Lord of D. cannot add new targets.
                let mut all: Vec<&DuelUnit> = self
                    .units
                    .iter()
                    .filter(|u| {
                        u.slot != 0 && self.magic_target_admitted(u, unit.controller(), cards)
                    })
                    .collect();
                all.sort_by_key(|u| (u.side, u.slot));
                let mut targets: Vec<UnitId> = all.into_iter().map(|u| u.id).collect();
                self.dark_hole_countertraps(unit.controller(), &mut targets, cards, events);
                for t in targets {
                    self.remove_by_effect(t, cards, events)?;
                }
            }
            // Dragon Capture Jar, run in the interpreter with a set own
            // Baby Dragon and a face-up enemy Blackland Fire Dragon: both got
            // +0x49 = 255 (fresh) and the acted flag; the Jar itself leaves
            // first. Targets in mask order (group 0 then 1, by slot).
            0x17 => {
                let targets: Vec<UnitId> = if filter == 0x21 {
                    // Time Seal: the single strongest monster (0x24F1C4).
                    self.strongest_monster(s, mode, cards)?
                        .into_iter()
                        .collect()
                } else {
                    let mut matched: Vec<&DuelUnit> = self
                        .units
                        .iter()
                        .filter(|u| {
                            let c = self.unit_card(u, cards);
                            u.slot != 0
                                && c.is_monster()
                                && u32::from(c.kind & 0x1F) == filter
                                && match mode {
                                    1 => u.controller() == s,
                                    2 => self.foe(u.controller(), s),
                                    _ => true,
                                }
                        })
                        .collect();
                    matched.sort_by_key(|u| (u.side, u.slot));
                    matched.into_iter().map(|u| u.id).collect()
                };
                // 0x2549B0: filtering follows strongest-target selection;
                // a protected winner is removed, not replaced by runner-up.
                let targets: Vec<UnitId> = targets
                    .into_iter()
                    .filter(|&t| {
                        self.magic_target_admitted(
                            self.unit(t).expect("target"),
                            unit.controller(),
                            cards,
                        )
                    })
                    .collect();
                for t in targets {
                    self.bind(t, lo as u8, events);
                    if let Some(u) = self.units.iter_mut().find(|u| u.id == t) {
                        u.acted = true;
                    }
                }
            }
            // Magical Neutralizing Force Field (builder 0x255994, handler
            // 0x25F2F0; run in the interpreter with spellbound and boosted
            // units on both sides and face-up traps on both sides): rec+8 =
            // the committed-face-up near/far traps of both groups (filter
            // 0x36), the card itself excluded. Phase 1: every unit in slots
            // 1..10 of both groups (filter 0x34, minus the immunity filter)
            // gets 0x251CD0(0) (spellbind cleared), +0x4A := 0 and its
            // permanent ATK/DEF deltas set to 0 (0x2525C0/0x2527A0 op 4).
            // Phase 3: the traps go to their owners' graveyards (0x250C90),
            // group 0 first. Set traps, leaders and monsters stay.
            0x20 => {
                let all: Vec<UnitId> = {
                    // 0x25F4A0: the reset handler filters its own mask.
                    let mut v: Vec<&DuelUnit> = self
                        .units
                        .iter()
                        .filter(|u| {
                            u.slot != 0 && self.magic_target_admitted(u, unit.controller(), cards)
                        })
                        .collect();
                    v.sort_by_key(|u| (u.side, u.slot));
                    v.into_iter().map(|u| u.id).collect()
                };
                for t in all {
                    self.unbind(t, events);
                    // 0x1E2860(g, s, 0): +0x4A := 0.
                    self.chain_bound.remove(&t.0);
                    if let Some(u) = self.units.iter_mut().find(|u| u.id == t) {
                        if u.attack_bonus != 0 || u.defense_bonus != 0 {
                            u.attack_bonus = 0;
                            u.defense_bonus = 0;
                            events.push(DuelEvent::Modified {
                                unit: t,
                                attack: 0,
                                defense: 0,
                            });
                        }
                    }
                }
                let traps: Vec<UnitId> = {
                    let mut v: Vec<&DuelUnit> = self
                        .units
                        .iter()
                        .filter(|u| {
                            let c = cards.card(u.card).expect("validated card");
                            u.slot != 0
                                && u.committed_face_up
                                && matches!(c.class(), class::NEAR_TRAP | class::FAR_TRAP)
                        })
                        .collect();
                    v.sort_by_key(|u| (u.side, u.slot));
                    v.into_iter().map(|u| u.id).collect()
                };
                for t in traps {
                    self.remove_by_effect(t, cards, events)?;
                }
            }
            // Celtic Guardian (entry-2 type 0x39, 0x268710 > 0 at 0x2557D0)
            // destroys the card without effect (MONSTER-EFFECTS.md).
            0x1F if self.defender_with(0x39, cards) => (),
            // Change of Heart: the mask is every enemy-controlled unit in
            // slots 1..10 (0x24E360 sel 2, crit 0x34 any card, minus the
            // immunity filter); an empty mask is "nothing" (record 0xFE,
            // the card is spent). Otherwise the record {0x1A, ctrl, mask,
            // 0xFFFE} returns 2 and state 0x2F lists the opposing group's
            // slots 1..10 (0x1D58B8, d36 = enemy group | 0x80 from the
            // builder's byte 1 = (ctrl == 0) << 4, 0x255920) for state 0x30.
            // [emu out_magic.txt: coh0/coh2/coh_up/coh_down8/coh_circle]
            0x1F if filter == 0x33 => {
                let by = unit.controller();
                let candidates = self
                    .units
                    .iter()
                    .filter(|u| {
                        u.slot != 0 && self.foe(u.controller(), by)
                        // 0x2558F4: the candidate mask is filtered, while
                        // state 0x2F's group-based list stays unchanged.
                        && self.magic_target_admitted(u, by, cards)
                    })
                    .count();
                if candidates > 0 {
                    let opponent = self.opp(by);
                    let mut listed: Vec<&DuelUnit> = self
                        .units
                        .iter()
                        .filter(|u| u.slot != 0 && u.side == opponent)
                        .collect();
                    listed.sort_by_key(|u| u.slot);
                    if listed.is_empty() {
                        // State 0x2F with an empty list skips to 0x39 and the
                        // handler takes its automatic path (rec+0xC stays
                        // 0xFFFE): not traced.
                        return Err(unsupported(
                            "Change of Heart with the opposing group empty but an own unit under enemy control",
                        ));
                    }
                    let units = listed.into_iter().map(|u| u.id).collect();
                    self.pending = Some(Pending::ControlChange { by, units });
                    events.push(DuelEvent::SelectionOpened { card: unit.card });
                }
            }
            // Brain Control: the single strongest enemy-controlled monster
            // (0x24E360 crit 0x21), rec+0xC = 0xFFFE, so the handler's
            // automatic path (0x25E564) toggles every masked unit; nothing
            // with an empty mask. [emu out_magic.txt: brain]
            0x1F => {
                // 0x255818: 0x24E360(0, 0, ctrl, mode 2, 0x21, 0xE): the
                // activator's view (list order and the exact-tie rule).
                if let Some(target) = self.strongest_monster(unit.controller(), mode, cards)? {
                    // 0x255854: immunity filters after strongest selection.
                    if self.magic_target_admitted(
                        self.unit(target).expect("target"),
                        unit.controller(),
                        cards,
                    ) {
                        self.change_control(target, unit.controller(), events);
                    }
                }
            }
            // Graveyard pick: mode 1 the controller's graveyard, 2 the
            // opponent's, 3 both (0x254A4C: s1 = ctrl / ctrl ^ 1 / 2); the
            // builder counts the eligible cards (0x1E00D0 for 0x1F any,
            // 0x252260(side, 0) monsters for 0x1D) and discards the card for
            // nothing when there are none (0x254CA4); otherwise it returns 2
            // with byte 1 = s1 << 4 | class code, and state 0x2B builds the
            // lists (0x1D5184). [emu out_magic2/3.txt]
            0x14 => {
                let by = unit.controller();
                let graveyards = match mode {
                    1 => vec![by],
                    2 => vec![self.opp(by)],
                    _ => self.all_sides(),
                };
                let monsters = filter == 0x1D;
                // Filter 0x1E (0x254B4C; Graverobber): Σ 0x252260(g, class
                // 1..5), the non-monster cards of the offered graveyards;
                // state 0x2B then lists class code 2 (0x251920: id >=
                // 0x2AB) and the card is created on the magic card's own
                // cell as Monster Reborn's [emu DARK-HOLE-BLOCKER-
                // GRAVEROBBER.md G1-G4].
                let non_monsters = filter == 0x1E;
                let mut eligible = graveyards
                    .iter()
                    .flat_map(|&g| self.sides[usize::from(g)].graveyard.iter())
                    .filter(|&&c| {
                        let monster = cards.card(c).is_some_and(|c| c.is_monster());
                        (!monsters || monster) && (!non_monsters || !monster)
                    })
                    .count();
                // The arm counts (0x254C74..0x254C98) before its own discard
                // (0x254CE4): the card just played is not a candidate for
                // the count, although state 0x2B lists it afterwards [emu
                // out_magic3.txt mimicat_d13: Mimicat copies itself].
                if !monsters && graveyards.contains(&unit.side) {
                    eligible -= 1;
                }
                if eligible > 0 {
                    self.pending = Some(Pending::GraveyardCopy {
                        by,
                        at: unit.position,
                        graveyards,
                        monsters,
                        non_monsters,
                    });
                    events.push(DuelEvent::SelectionOpened { card: unit.card });
                }
            }
            // 718 The Eye of Truth: the opponent's hand is shown only.
            0x1D => {}
            0x24 => {
                for t in self.magic_mask(&unit, mode, filter, cards) {
                    if let Some(u) = self.unit(t).cloned() {
                        self.return_to_deck(&u, cards, events)?;
                    }
                }
            }
            _ => unreachable!(),
        }
        // No crush check follows a magic card; a boosted monster on Crush
        // terrain is crushed at the next turn start [F MORE-EFFECTS.md 1.1].
        Ok(())
    }

    /// 0x268710(ty) > 0: a face-up committed-defence non-leader with an
    /// entry-2 effect of type `ty` stands on either side (the Ancient Tree
    /// test of [`Self::far_trap_in`], the Moisture Creature test of
    /// [`Self::leader_has`]).
    /// 0x269450(side): over the continuous type-0x56 units (0x268710 /
    /// 0x268590: slots 1.., face-up committed defence), entry 2's mode 2
    /// counts −1 when the unit's controller is not `side`, mode 3 counts +1
    /// whoever controls it, mode 1 nothing; the sign of the sum (−1, 0, 1).
    /// [F; emu exp_catabolism C0-C5: +1 for an own or enemy Servant 680
    /// (mode 3), none in attack position, two count once, capped at 12]
    fn catabolism(&self, side: u8, cards: &impl CardData) -> i32 {
        let mut sum = 0i32;
        for u in &self.units {
            if !(u.slot != 0 && u.defence && u.committed_face_up && u.committed_defence) {
                continue;
            }
            let Some([w, _]) = cards.card(u.card).and_then(|c| c.entries[2]) else {
                continue;
            };
            if w >> 8 != 0x56 {
                continue;
            }
            match (w >> 6) & 3 {
                2 if u.controller() != side => sum -= 1,
                3 => sum += 1,
                _ => (),
            }
        }
        sum.signum()
    }

    fn defender_with(&self, ty: u32, cards: &impl CardData) -> bool {
        self.units.iter().any(|u| {
            u.slot != 0
                && u.defence
                && u.committed_face_up
                && u.committed_defence
                && cards
                    .card(u.card)
                    .and_then(|c| c.entries[2])
                    .is_some_and(|[w, _]| w >> 8 == ty)
        })
    }

    /// Formerly refused leader ability #11; the 0x273C60 filter
    /// (`magic_target_admitted`) and the handler's 0x273770 test
    /// (`resolve_pick`) now model it (immunity.rs).
    fn guard_control_immunity(&self, _by: u8, _cards: &impl CardData) -> Result<()> {
        Ok(())
    }

    /// 0x24E360(0, 0, ctrl, 2, 0x21, 0xE) → 0x24F1C4 (jump table 0x2D3130[0x21]; 0x24F324 is
    /// criterion 0x20, the weakest, see more_rules.rs): among the monsters
    /// (class 0, any face) in slots 1..10 of both groups whose controller
    /// is `side` (0x24E230), the highest effective ATK at the unit's own
    /// cell (0x252300); ties: the higher DEF (0x252460), then the lower
    /// summon order (0x1E2640). An exact tie falls to a group comparison
    /// that looks like a bug (0x24F28C; 0x24F3E8 in the 0x20 twin, HELPERS.md
    /// §2), modelled below as read; a 0/0 monster beats the initial key by
    /// its order.
    /// The 0x21 selection over the controllers `mode` admits from `side`
    /// (0x24F6A0: 1 own, 2 other, 3 both), as 0x24E360 builds it for filter 0x21.
    fn strongest_monster(
        &self,
        side: u8,
        mode: u32,
        cards: &impl CardData,
    ) -> Result<Option<UnitId>> {
        // 0x24F1C4 over the 0x24E230(side, mode) list: `side`'s group
        // slots 1..10 first, then the other group, units whose controller
        // the mode admits. Best starts as key 0 (ATK << 16 | DEF), order
        // 9999 (0x270F), group -1. A candidate wins with a higher ATK; with
        // an equal ATK, a higher DEF; with equal ATK and DEF, a lower summon
        // order; with the same order too (two groups' counters), only when
        // the best's GROUP equals the mode number (0x24F28C `bne s7, s0`,
        // s0 = the mode argument) [F REFUSALS-SWEEP.md §strongest]. A 0/0
        // monster wins over the initial key by its order (< 9999).
        let mut list: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && match mode {
                        1 => u.controller() == side,
                        2 => self.foe(u.controller(), side),
                        3 => true,
                        _ => false,
                    }
            })
            .collect();
        list.sort_by_key(|u| (u.side != side, u.slot));
        let (mut key, mut order, mut group): ((u16, u16), u16, i32) = ((0, 0), 0x270F, -1);
        let mut best: Option<UnitId> = None;
        for u in list {
            if !cards.card(u.card).is_some_and(|c| c.is_monster()) {
                continue;
            }
            let k = (
                self.effective(u, u.position, true, cards),
                self.effective(u, u.position, false, cards),
            );
            let o = u16::from(u.summon_order);
            let take = if k.0 != key.0 {
                key.0 < k.0
            } else if k.1 != key.1 {
                key.1 < k.1
            } else if o == order {
                group == mode as i32
            } else {
                o < order
            };
            if take {
                (key, order, group, best) = (k, o, i32::from(u.side), Some(u.id));
            }
        }
        Ok(best)
    }

    /// Event 0x1A, one target (0x25E9DC / 0x25E63C): the control-changed
    /// bit toggles (0x1E30F0 bit 1), then the acted bit clears unless the
    /// unit is spellbound (0x1E2800) or its +0x4A is set (0x1E28C0, Binding
    /// Chain's `chain_bound`). [emu: Hunter Spider 0x15 → 0x17 keeps bit 0, Giant
    /// Flea 0x29 → 0x2A loses it]
    fn change_control(&mut self, id: UnitId, to: u8, events: &mut Vec<DuelEvent>) {
        let chained = self.is_chain_bound(id);
        let multi = self.multi().is_some();
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            if multi {
                // N-player duels (a port addition): control goes to `to`,
                // or back to the owner.
                u.stolen = u.side != to;
                u.stolen_by = u.stolen.then_some(to);
            } else {
                u.stolen = !u.stolen;
            }
            debug_assert_eq!(u.controller(), to);
            if u.spellbind == 0 && !chained {
                u.acted = false;
            }
            events.push(DuelEvent::Controlled { unit: id, side: to });
        }
    }

    /// The answer to [`Duel::pending`]: Circle (rec+0xC = −1 at 0x25E344,
    /// 999 at 0x2586EC) ends the effect with nothing; a unit (state 0x31,
    /// c4c = group << 8 | slot) goes through the chosen-target path of the
    /// 0x1A handler; a graveyard card (state 0x2E, c4c = grave << 16 | id)
    /// through event 1 (0x2586A0).
    fn resolve_pick(
        &mut self,
        pending: Pending,
        pick: &Pick,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        match (pending, pick) {
            (_, Pick::Cancel) => Ok(()),
            (Pending::ControlChange { by, units }, Pick::Unit { unit }) => {
                if !units.contains(unit) {
                    return Err(DuelError::InvalidCommand(
                        "the unit is not in the selection",
                    ));
                }
                let u = self
                    .unit(*unit)
                    .ok_or(DuelError::InvalidCommand("selected unit is gone"))?;
                // 0x25E9D4: a target the activator already controls is left
                // alone (the mask and the list differ: the list is the
                // enemy group, the mask the enemy-controlled units).
                // 0x25E950: the displayed group list may include a
                // protected Dragon; selecting it spends the card for nothing.
                // 0x25E8C8: leader ability #11 first (0x273770: box 0x10F
                // "Leader Ability activated!", no change), then the Lord.
                if self.ability11(u, cards)
                    || u.controller() == by
                    || self.dragon_immunity(u, cards)
                {
                    return Ok(());
                }
                self.change_control(*unit, by, events);
                Ok(())
            }
            (
                Pending::GraveyardCopy {
                    by,
                    at,
                    graveyards,
                    monsters,
                    non_monsters,
                },
                Pick::Graveyard { side, index },
            ) => {
                if !graveyards.contains(side) {
                    return Err(DuelError::InvalidCommand("that graveyard is not offered"));
                }
                let grave = &self.sides[usize::from(*side)].graveyard;
                let card_id = *grave
                    .get(*index)
                    .ok_or(DuelError::InvalidCommand("graveyard index out of range"))?;
                let card = cards
                    .card(card_id)
                    .ok_or(DuelError::InvalidCommand("unknown graveyard card"))?
                    .clone();
                if monsters && !card.is_monster() {
                    return Err(DuelError::InvalidCommand("the pick must be a monster"));
                }
                if non_monsters && card_id < 0x2AB {
                    return Err(DuelError::InvalidCommand("the pick must not be a monster"));
                }
                if self.unit_at(at).is_some() {
                    return Err(unsupported("a graveyard copy onto an occupied cell"));
                }
                // 0x25872C / 0x258758: five cards already in the class's
                // slot range of the controller's group → rec+0xC := 999,
                // nothing happens (the card stays in the graveyard).
                let range = if card.is_monster() {
                    1..=MONSTER_SLOTS as u8
                } else {
                    MONSTER_SLOTS as u8 + 1..=2 * MONSTER_SLOTS as u8
                };
                let used: Vec<u8> = self
                    .units
                    .iter()
                    .filter(|u| u.side == by && range.contains(&u.slot))
                    .map(|u| u.slot)
                    .collect();
                if used.len() >= MONSTER_SLOTS {
                    return Ok(());
                }
                let slot = range
                    .clone()
                    .find(|n| !used.contains(n))
                    .expect("a free slot in the range");
                // 0x1E0800(ctrl, col, row, card, 0): a fresh unit of the
                // controller's group, face-down in attack position; a
                // non-monster card gets the acted bit (0x2589CC).
                let discount = self.summon_discount(by, card_id, cards);
                let owner = &mut self.sides[usize::from(by)];
                let order = owner.summons;
                owner.summons = owner.summons.saturating_add(1);
                let id = UnitId(self.next_unit);
                self.next_unit += 1;
                self.units.push(DuelUnit {
                    id,
                    card: card_id,
                    side: by,
                    slot,
                    position: at,
                    face_up: false,
                    defence: false,
                    committed_face_up: false,
                    committed_defence: false,
                    acted: !card.is_monster(),
                    spellbind: 0,
                    summon_order: order,
                    attack_bonus: 0,
                    defense_bonus: 0,
                    spellbind_fresh: false,
                    evo_counter: 0,
                    stolen: false,
                    discount,
                    stolen_by: None,
                });
                // 0x258AF8: the first entry of that graveyard with the
                // card's id leaves (by id, not by position).
                let grave = &mut self.sides[usize::from(*side)].graveyard;
                if let Some(first) = grave.iter().position(|&c| c == card_id) {
                    grave.remove(first);
                }
                events.push(DuelEvent::Revived {
                    unit: id,
                    card: card_id,
                    at,
                    from: *side,
                });
                // The engine sweeps Crush cells after every placement [H:
                // the handler itself does not].
                self.crush_sweep(cards, events)
            }
            _ => Err(DuelError::InvalidCommand(
                "the answer does not fit the open selection",
            )),
        }
    }

    /// 0x26C5C0: the oldest (lowest summon order) full-range trap of `group`
    /// with effect `ty`, anywhere on the field and face-up or not.
    fn far_trap(&self, group: u8, ty: u32, cards: &impl CardData) -> Option<UnitId> {
        self.far_trap_in(&[group], ty, cards)
    }

    /// 0x26C5C0 over `groups` in order (mask 3 = the own group first): slots
    /// 6..=10, strictly lower summon order wins, ties keep the first found.
    /// Every search fails while an Ancient Tree of Enlightenment (entry-2
    /// type 0xF, 655) stands face-up in committed defence on either side
    /// (0x268710; F06.md §3, run for the Jammer, code-only for the rest).
    fn far_trap_in(&self, groups: &[u8], ty: u32, cards: &impl CardData) -> Option<UnitId> {
        if self.traps_suppressed(cards) {
            return None;
        }
        let mut best: Option<(u8, UnitId)> = None;
        for &group in groups {
            let mut traps: Vec<&DuelUnit> = self
                .units
                .iter()
                .filter(|u| u.side == group && OTHER_SLOTS.contains(&u.slot))
                .filter(|u| {
                    cards.card(u.card).is_some_and(|c| {
                        c.class() == class::FAR_TRAP && c.card_effect_type() == Some(ty)
                    })
                })
                .collect();
            traps.sort_by_key(|u| u.slot);
            for u in traps {
                // [F] 0x26C5C0 starts its best-order byte at 0xFF and
                // accepts only a strictly earlier summon order.
                if u.summon_order < best.map_or(0xFF, |(order, _)| order) {
                    best = Some((u.summon_order, u.id));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// A trap or continuous unit that reacted without leaving the field
    /// (events 0x66 and the event-9 stages 3–4, F02.md §3.2, F06.md §4):
    /// revealed (0x252FF0) if not yet committed face-up, then marked acted.
    fn react_trap(&mut self, id: UnitId, target: Option<UnitId>, events: &mut Vec<DuelEvent>) {
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            if !u.committed_face_up {
                u.face_up = true;
                u.committed_face_up = true;
                events.push(DuelEvent::Revealed {
                    unit: id,
                    card: u.card,
                });
            }
            u.acted = true;
            events.push(DuelEvent::Activated {
                unit: id,
                card: u.card,
                target,
            });
        }
    }

    /// 0x253D60 phases 1–6 for a face-up magic or ritual card `id` of
    /// effect type `ty` (F06.md §2): first match wins. Returns true when the
    /// card was consumed without effect.
    /// (a) Temple of Skulls (entry-2 type 0xE face-up in committed defence,
    ///     either side): the Temple acts, the card goes to the graveyard;
    /// (c) Magic Jammer (type 0x0E) of the opposite group, unless
    /// (d) a Royal Decree (either group, group 0 first) reacts instead, in
    ///     which case the card resolves as if no trap were set; else the
    ///     Jammer and then the card go to their graveyards (event 0x33);
    /// (e) Call of the Grave (only magic type 0x14) and the Seal of the
    ///     Ancients (only ritual type 0x32) are refused (keep = 0, code only).
    fn negation_chain(
        &mut self,
        id: UnitId,
        ty: u32,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<bool> {
        let s = self.unit(id).expect("activating card").side;
        // 0x268590(0xE, 0): the first such unit in slot order, group 0
        // first [F; emu REFUSALS-SWEEP.md TS1/TS2: an own or enemy Temple in
        // face-up defence took Dark Hole and Windstorm of Etaqua, the
        // Temple got the acted bit; TS3: in attack position nothing].
        let mut sources: Vec<&DuelUnit> = self.units.iter().collect();
        sources.sort_by_key(|u| (u.side, u.slot));
        let temple = sources
            .into_iter()
            .find(|u| {
                u.slot != 0
                    && u.defence
                    && u.committed_face_up
                    && u.committed_defence
                    && cards
                        .card(u.card)
                        .and_then(|c| c.entries[2])
                        .is_some_and(|[w, _]| w >> 8 == 0xE)
            })
            .map(|u| u.id);
        if let Some(temple) = temple {
            self.react_trap(temple, Some(id), events);
            self.discard(id, events);
            return Ok(true);
        }
        if let Some(jammer) = self.far_trap_in(&self.foes(s), trap_type::MAGIC_JAMMER, cards) {
            if let Some(decree) = self.far_trap_in(&self.living_sides(), trap_type::ROYAL_DECREE, cards) {
                self.react_trap(decree, Some(id), events);
            } else {
                if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
                    u.face_up = true;
                    u.committed_face_up = true;
                }
                self.activate(jammer, Some(id), events);
                self.discard(jammer, events);
                self.discard(id, events);
                return Ok(true);
            }
        }
        if ty == 0x14 && self.far_trap_in(&self.first_then_all(s), 0x4C, cards).is_some() {
            return Err(unsupported("trap effect type 0x4c"));
        }
        if ty == 0x32 && self.far_trap_in(&self.first_then_all(s), 0x11, cards).is_some() {
            return Err(unsupported("trap effect type 0x11"));
        }
        Ok(false)
    }

    /// 0x268A10(filter, amount): face-up committed-defence units of slots
    /// 1..=10 (side 0 first, slot order) with an entry-2 type 0x3B of that
    /// filter (0x2C losses, 0x2B gains) apply op = word1 >> 16 with x =
    /// word1 & 0xFFFF: 0 add, 1 subtract, 2 multiply, 3 subtract amount / x;
    /// clamped to 0..=9999 (F02.md §3.3). 0 stays 0.
    fn lp_modifier(&self, filter: u32, amount: u16, cards: &impl CardData) -> u16 {
        if amount == 0 {
            return 0;
        }
        let mut sources: Vec<&DuelUnit> = self
            .units
            .iter()
            .filter(|u| u.slot != 0 && u.committed_face_up && u.defence && u.committed_defence)
            .collect();
        sources.sort_by_key(|u| (u.side, u.slot));
        let mut v = i32::from(amount);
        for u in sources {
            let Some([w0, w1]) = cards.card(u.card).and_then(|c| c.entries[2]) else {
                continue;
            };
            if w0 >> 8 != 0x3B || w0 & 0x3F != filter {
                continue;
            }
            let x = (w1 & 0xFFFF) as i32;
            match w1 >> 16 {
                0 => v += x,
                1 => v -= x,
                2 => v *= x,
                3 if x != 0 => v -= v / x,
                _ => (),
            }
        }
        v.clamp(0, i32::from(MAX_LP)) as u16
    }

    /// Event 9 (0x25ADF0, F02.md §3): side `s` inflicts `amount` on the
    /// other side. A Goblin Fan (0x29) in the victim's group reflects it onto
    /// `s` unless a Royal Decree (either group, the victim's first) reacts
    /// instead; the reacting trap is revealed and marked acted and stays.
    /// The 0x2C modifiers apply to the final victim's loss.
    fn direct_damage(
        &mut self,
        s: u8,
        amount: u16,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) {
        let t = self.opp(s);
        let mut victim = t;
        if let Some(fan) = self.far_trap(t, trap_type::GOBLIN_FAN, cards) {
            if let Some(decree) = self.far_trap_in(&self.first_then_all(t), trap_type::ROYAL_DECREE, cards) {
                self.react_trap(decree, None, events);
            } else {
                self.react_trap(fan, None, events);
                victim = s;
            }
        }
        let loss = self.lp_modifier(0x2C, amount, cards);
        self.damage(victim, loss, events);
    }

    /// Event 8 (0x25A740, F02.md §5): side `s` gains `amount` with the 0x2B
    /// modifiers, unless a Bad Reaction to Simochi (0x2A) in the other group
    /// turns it into a loss of the same modified amount; a Royal Decree
    /// (either group, `s` first) reacts instead and the gain stands.
    fn lp_gain(&mut self, s: u8, amount: u16, cards: &impl CardData, events: &mut Vec<DuelEvent>) {
        if let Some(simochi) = self.far_trap_in(&self.foes(s), trap_type::SIMOCHI, cards) {
            if let Some(decree) = self.far_trap_in(&self.first_then_all(s), trap_type::ROYAL_DECREE, cards) {
                self.react_trap(decree, None, events);
            } else {
                self.react_trap(simochi, None, events);
                let loss = self.lp_modifier(0x2B, amount, cards);
                self.damage(s, loss, events);
                return;
            }
        }
        let gain = self.lp_modifier(0x2B, amount, cards);
        let lp = &mut self.sides[usize::from(s)].lp;
        *lp = (u32::from(*lp) + u32::from(gain)).min(u32::from(MAX_LP)) as u16;
    }

    /// Reveal a trap or magic card as it takes effect.
    fn activate(&mut self, id: UnitId, target: Option<UnitId>, events: &mut Vec<DuelEvent>) {
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            u.face_up = true;
            u.committed_face_up = true;
            events.push(DuelEvent::Activated {
                unit: id,
                card: u.card,
                target,
            });
        }
    }

    /// 0x250C90 for a used card: off the field into its owner's graveyard,
    /// charging the card 4 experience first.
    fn discard(&mut self, id: UnitId, events: &mut Vec<DuelEvent>) {
        if let Some(index) = self.units.iter().position(|u| u.id == id) {
            let unit = self.units.remove(index);
            self.record_experience(unit.side, unit.card, -4);
            self.sides[usize::from(unit.side)].graveyard.push(unit.card);
            events.push(DuelEvent::Discarded {
                unit: id,
                card: unit.card,
            });
        }
    }

    /// Reveal a trap victim without its flip effect (0x266D60/0x266970).
    /// 0x252FF0(g, s, 0): face down, committed face-up cleared.
    fn conceal(&mut self, id: UnitId, events: &mut Vec<DuelEvent>) {
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            if u.face_up || u.committed_face_up {
                u.face_up = false;
                u.committed_face_up = false;
                events.push(DuelEvent::Concealed { unit: id, card: u.card });
            }
        }
    }

    fn expose(&mut self, id: UnitId, events: &mut Vec<DuelEvent>) {
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            if !u.committed_face_up && u.slot != 0 {
                u.face_up = true;
                u.committed_face_up = true;
                events.push(DuelEvent::Revealed {
                    unit: id,
                    card: u.card,
                });
            }
        }
    }

    /// Move confirm hook 0x26C7A0 (state 0x1D). Returns true when a trap
    /// cancelled the action.
    fn confirm_trap(
        &mut self,
        index: usize,
        in_defence: bool,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<bool> {
        let mover = self.units[index].clone();
        // 0x26C80C: leader ability #11 at the mover's cell (0x2738C0) is
        // noted first; 0x26C824 (the 0x269020 immunity) precedes the
        // full-range trap search and Decree check.
        let resists = self.ability11(&mover, cards);
        if self.dragon_immunity(&mover, cards) {
            return Ok(false);
        }
        let types: &[u32] = if in_defence {
            &[trap_type::GORGONS_EYE, trap_type::SHADOW_OF_EYES]
        } else {
            &[trap_type::BLOCK_ATTACK]
        };
        // 0x26C4B0(0x24F790(g, slot), ty) with mask 2 (0x26C580): the
        // other group of the mover's CONTROLLER.
        for &ty in types {
            let Some(trap) = self.far_trap_in(&self.foes(mover.controller()), ty, cards) else {
                continue;
            };
            // 0x26C864: a found trap meets the #11 resist before the Decree
            // query: record {0xFE, 0, ctrl << 8, 0x10}, result 1 (the
            // action goes on, the trap stays set).
            if resists {
                return Ok(false);
            }
            // 0x26C8D0/C9B4/CA94: opposite PHYSICAL group first, even
            // for a stolen mover. Decree leaves the requested action intact.
            if self.royal_decree_reacts(self.opp(mover.side), cards, events) {
                return Ok(false);
            }
            if ty == trap_type::SHADOW_OF_EYES {
                // Shadow of Eyes (record {0x47, trap, mover, 0}, builder
                // result 1: the action goes on; handler 0x277310): the trap
                // is revealed (0x252FF0) and marked acted (0x1E30F0 bit 0)
                // and STAYS; stage 2 clears the mover's current and
                // committed defence bits (0x1E30F0 bits 3 and 5) instead of
                // binding it. No face change: a face-down unit stays
                // face-down. The action then ends as usual (state 0x1E →
                // 0x38: commit, acted, a manual flip if any). [F; emu S0-S6]
                self.react_trap(trap, Some(mover.id), events);
                let u = self
                    .units
                    .iter_mut()
                    .find(|u| u.id == mover.id)
                    .expect("mover");
                u.defence = false;
                u.committed_defence = false;
                return Ok(false);
            }
            if ty == trap_type::GORGONS_EYE {
                // Gorgon's Eye (event 0x47 {trap, mover, 1}, handler
                // 0x277310; emu Q1): the trap is revealed (0x252FF0) and
                // STAYS on the field; the mover keeps its committed position
                // (the defence request is undone), is spellbound for good
                // (0x251CD0 with 0xFF) and its turn is used. Only a position
                // change in place reaches this hook (a moving unit takes
                // attack position: emu Q2 nothing).
                let u = &mut self.units[index];
                u.face_up = u.committed_face_up;
                u.defence = u.committed_defence;
                u.acted = true;
                self.activate(trap, Some(mover.id), events);
                if let Some(t) = self.units.iter_mut().find(|u| u.id == trap) {
                    t.acted = true;
                }
                self.bind(mover.id, 0xFF, events);
                events.push(DuelEvent::Cancelled { unit: mover.id });
                return Ok(true);
            }
            // Block Attack (event 0x46; savestates 688-690): the action is
            // cancelled, the unit stays, its selection-time flip and position
            // change are undone, and its turn is used. It is not revealed.
            let u = &mut self.units[index];
            u.face_up = u.committed_face_up;
            u.defence = u.committed_defence;
            u.acted = true;
            self.activate(trap, Some(mover.id), events);
            self.discard(trap, events);
            events.push(DuelEvent::Cancelled { unit: mover.id });
            return Ok(true);
        }
        Ok(false)
    }

    /// Limited-range traps 0x24CC30 for a non-leader mover at `cell` (the
    /// cell just entered, or the attack target). Returns true when a trap
    /// stopped the mover there (or destroyed it).
    fn near_trap(
        &mut self,
        mover: UnitId,
        cell: GridPos,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<bool> {
        let Some(unit) = self.unit(mover).cloned() else {
            return Ok(true);
        };
        if unit.slot == 0 {
            return Ok(false);
        }
        // 0x24CC9C: leader abilities #16 / #15 / #13 of the other side's
        // leader when the cell lies in its window (white_rules.rs).
        if let Some(stop) = self.window_entry_abilities(mover, cell, cards, events)? {
            return Ok(stop);
        }
        // 0x24CE34: a mover whose entry 1 is type 7 (299 Reaper of the
        // Cards) is immune to limited-range traps [emu K1/K2 against the
        // controls K0/K3].
        if cards
            .card(unit.card)
            .and_then(|c| c.entries[1])
            .is_some_and(|[w, _]| w >> 8 == 7)
        {
            return Ok(false);
        }
        // 0x24CE68 precedes candidate collection and Royal Decree.
        if self.traps_suppressed(cards) {
            return Ok(false);
        }
        let mut traps: Vec<DuelUnit> = self
            .units
            .iter()
            .filter(|u| {
                u.controller() != unit.controller()
                    && u.position.column.abs_diff(cell.column) + u.position.row.abs_diff(cell.row)
                        <= 1
                    && cards
                        .card(u.card)
                        .is_some_and(|c| c.class() == class::NEAR_TRAP)
            })
            .cloned()
            .collect();
        Self::order_near_traps(&mut traps);
        if traps.is_empty() {
            return Ok(false);
        }
        // 0x24D014 searches group0 first before checking a nearby trap's
        // specific applicability, and before Dragon immunity.
        if self.royal_decree_reacts(0, cards, events) {
            return Ok(false);
        }
        let mover_card = cards.card(unit.card).expect("validated card").clone();
        for trap in traps {
            let card = cards.card(trap.card).expect("validated card");
            let [_, word1] = card.entries[0].unwrap_or([0, 0]);
            let destroys = match card.kind_index {
                0..=5 => false,
                6..=10 => {
                    let limit = NEAR_TRAP_ATTACK_LIMITS[usize::from(card.kind_index - 6)];
                    if !mover_card.is_monster() || self.effective(&unit, cell, true, cards) > limit
                    {
                        continue;
                    }
                    true
                }
                12 => {
                    if mover_card.is_monster() {
                        continue;
                    }
                    true
                }
                // Widespread Ruin 812 (0x24DA54): no applicability test, any
                // mover class; ability #11 (refused at setup), the 0x269020
                // immunity, then event 0x2E {0x2E, trap, mover, 0}
                // (MANAWYDDAN-812-720-79.md §1, emu WR0-WR6).
                11 => {
                    if self.ability11_at(&unit, cell, cards) || self.dragon_immunity(&unit, cards) {
                        return Ok(false);
                    }
                    self.widespread_ruin(trap.id, mover, cards, events)?;
                    return Ok(true);
                }
                _ => {
                    return Err(unsupported(format!(
                        "limited-range trap {} (case {})",
                        trap.card, card.kind_index
                    )))
                }
            };
            // 0x24D0F0 / 0x24D158 and the equivalent checks in each
            // near-trap case: after applicability and Royal Decree, leader
            // ability #11 at the entered cell (0x2738C0: {0xFE, 0, ctrl <<
            // 8, 0x10}, result 1, the trap stays) and then the 0x269020
            // immunity (result 0) skip the hook.
            if self.ability11_at(&unit, cell, cards) || self.dragon_immunity(&unit, cards) {
                return Ok(false);
            }
            self.activate(trap.id, Some(mover), events);
            self.expose(mover, events);
            self.discard(trap.id, events);
            if destroys {
                // Event 0x2D (0x266970): trap and victim both by 0x250C90.
                self.remove_by_effect(mover, cards, events)?;
                return Ok(true);
            }
            // Event 0x2C (0x266D60): bind for (word1 >> 16) & 0xFF own turns
            // and lower ATK/DEF by word1 & 0xFFFF, not below zero.
            let turns = ((word1 >> 16) & 0xFF) as u8;
            let loss = (word1 & 0xFFFF) as i32;
            if loss != 0 && mover_card.is_monster() {
                let u = self.units.iter_mut().find(|u| u.id == mover).unwrap();
                let floor =
                    |base: u16, bonus: i16| (i32::from(bonus) - loss).max(-i32::from(base)) as i16;
                u.attack_bonus = floor(mover_card.attack, u.attack_bonus);
                u.defense_bonus = floor(mover_card.defense, u.defense_bonus);
                events.push(DuelEvent::Modified {
                    unit: mover,
                    attack: u.attack_bonus,
                    defense: u.defense_bonus,
                });
            }
            self.bind(mover, turns, events);
            // State 0x39: the mover stops and commits on this cell.
            let u = self.units.iter_mut().find(|u| u.id == mover).unwrap();
            u.committed_face_up = u.face_up;
            u.committed_defence = u.defence;
            u.acted = true;
            return Ok(true);
        }
        Ok(false)
    }

    /// 0x251CD0: spellbind for `turns` of the unit's own turns (0xFF is
    /// permanent); a shorter bind never replaces a longer one.
    fn bind(&mut self, id: UnitId, turns: u8, events: &mut Vec<DuelEvent>) {
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            if turns >= u.spellbind {
                u.spellbind = turns;
                u.spellbind_fresh = true;
                events.push(DuelEvent::Spellbound { unit: id });
            }
        }
    }

    /// 0x251CD0 with amount 0 (0x251D30): +0x49 := 0 only; the fresh flag
    /// and the acted flag are untouched.
    fn unbind(&mut self, id: UnitId, events: &mut Vec<DuelEvent>) {
        if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
            if u.spellbind != 0 {
                u.spellbind = 0;
                events.push(DuelEvent::Unbound { unit: id });
            }
        }
    }

    fn end_turn(&mut self, cards: &impl CardData, events: &mut Vec<DuelEvent>) -> Result<()> {
        let s = self.active;
        // 0x273E20 phase 0 (0x273E8C): leader ability #5 of the ending side
        // → record {8, 0, side, 0x32}: event 8, LP + 50. [F]
        if self.leader_has(s, 5, cards) {
            self.lp_gain(s, 50, cards, events);
        }
        self.crush_sweep(cards, events)?;
        // 0x273E20 phase 2: the ending side's leader window sweep (#16,
        // #15, #13; white_rules.rs).
        let at = self.leader(s).position;
        self.window_sweep(s, at, cards, events)?;
        self.end_turn_experience(s);
        // 0x273E20: the ending side's slots 1..10 count down unless bound
        // during this turn; then every fresh flag clears. An Ancient Jar
        // (entry-2 type 0x47, either side, 0x2741B0) stops the countdown;
        // a Timeater (0x55, 0x274190) makes it 0x252180(0, n - 2) [F; emu
        // REFUSALS-SWEEP.md J0-J3, T1/T2].
        let frozen = self.continuous_count(0x47, cards) > 0;
        let step = if self.continuous_count(0x55, cards) > 0 { 2 } else { 1 };
        for u in self.units.iter_mut() {
            if !frozen
                && u.side == s
                && u.slot != 0
                && !u.spellbind_fresh
                && u.spellbind > 0
                && u.spellbind < 0xFF
            {
                u.spellbind = u.spellbind.saturating_sub(step);
            }
            u.spellbind_fresh = false;
        }
        // The same loop clears +0x4A where no type-0x17 card covers it.
        self.chain_bind_turn_end(cards);
        events.push(DuelEvent::TurnEnded { side: s });
        if self.multi().is_some() {
            return self.multi_end_turn(s, cards, events);
        }
        if self.leader_trapped(s) {
            self.finish(Some(s ^ 1), EndReason::LeaderTrapped, events);
            return Ok(());
        }
        if s != self.first && self.round >= ROUND_LIMIT {
            let winner = self.lp_winner();
            self.finish(winner, EndReason::TurnLimit, events);
            return Ok(());
        }
        self.start_turn(s ^ 1, cards, events)
    }

    /// 0x2744C0: no free summon area around the leader of `s`.
    fn leader_trapped(&self, s: u8) -> bool {
        let leader = self.leader(s).position;
        for dr in -1_i32..=1 {
            for dc in -1_i32..=1 {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let (c, r) = (i32::from(leader.column) + dc, i32::from(leader.row) + dr);
                if !(0..i32::from(BOARD_SIDE)).contains(&c)
                    || !(0..i32::from(BOARD_SIDE)).contains(&r)
                {
                    continue;
                }
                let cell = GridPos {
                    column: c as u8,
                    row: r as u8,
                };
                let wall = self.terrain_at(cell) == terrain::LABYRINTH;
                match self.unit_at(cell) {
                    Some(u) if u.controller() == s && !wall => return false,
                    None if !wall => return false,
                    _ => (),
                }
            }
        }
        true
    }

    fn start_turn(
        &mut self,
        s: u8,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<()> {
        let prev = self.active;
        self.active = s;
        // N-player duels (a port addition): a round starts when play
        // reaches or passes the first player's seat.
        let new_round = if self.multi().is_some() {
            self.round == 0 || self.wraps(prev, s)
        } else {
            s == self.first
        };
        // 0x2695B0: with a Timeater (entry-2 type 0x55, either side) the
        // round counter grows at every turn start, not only the first
        // side's [F; emu REFUSALS-SWEEP.md RT0-RT4].
        if new_round || self.continuous_count(0x55, cards) > 0 {
            self.round = self.round.saturating_add(1);
        }
        // Every control change ends when the next turn starts, for both
        // groups: +0x57 bit 1 is cleared on every unit as the active side
        // switches (state 0x17), before the turn-start effects (MOTH-EFFECTS.md
        // §6.3: "after the round counter and bit1 clearing") and the
        // per-group acted reset [emu pegasus-fx/out_turn.txt: Flea 0x2B →
        // 0x29 at f7, the acted reset at f24; an own stolen Nekogal 0x02 →
        // 0x00 likewise].
        // 0x2695EC: a unit whose record carries an entry-2 type 0x51 (the
        // monster Jowls of Dark Demise possessed) keeps it [F; emu
        // REFUSALS-SWEEP.md JD4].
        let possessed: Vec<UnitId> = self
            .units
            .iter()
            .filter(|u| {
                u.slot != 0
                    && self.unit_card(u, cards).entries[2].is_some_and(|[w, _]| w >> 8 == 0x51)
            })
            .map(|u| u.id)
            .collect();
        for u in &mut self.units {
            if !possessed.contains(&u.id) {
                u.stolen = false;
                u.stolen_by = None;
            }
        }
        // 0x273E20 phase 1: the crush check (event 0x70) [F MORE-EFFECTS.md
        // 1.1; H: its order against the turn-start effects].
        self.crush_sweep(cards, events)?;
        self.turn_start_effects(s, cards, events)?;
        // Phases 0x26..0x2A: the other group's continuous type-0x17 binds
        // (Binding Chain, white_rules.rs).
        self.chain_bind_turn_start(s, cards, events);
        // Phase 0x40: +0x49 or +0x4A keeps a unit from acting (0x269A60).
        let chained: Vec<bool> = self
            .units
            .iter()
            .map(|u| self.is_chain_bound(u.id))
            .collect();
        for (u, chained) in self.units.iter_mut().zip(chained) {
            u.acted = u.side != s || u.spellbind > 0 || chained;
        }
        // A scripted side gets the Exodia event only, no win (turn-verified.md §7 D).
        if self.exodia_complete(s, cards) && !script::scripted(self.controllers[usize::from(s)]) {
            self.finish(Some(s), EndReason::Exodia, events);
            return Ok(());
        }
        // 0x269CBC: from round 2, 0x1DF860(side, 0, rate + 0x269450(side))
        // when a continuous type-0x56 unit stands (Servant of Catabolism).
        let catabolism = self.catabolism(s, cards);
        let side = &mut self.sides[usize::from(s)];
        if self.round >= 2 {
            let gain = i32::from(self.star_income[usize::from(s)]) + catabolism;
            side.stars = (i32::from(side.stars) + gain).clamp(0, i32::from(MAX_STARS)) as u8;
        }
        // Destiny Draw arming 0x270800 after the stars (0x269CBC–0x269D58;
        // claude-effects/SPECIAL-DRAW.md §2, ABILITY-3-CPU.md §2).
        self.arm_destiny(s, cards)?;
        let side = &mut self.sides[usize::from(s)];
        side.can_summon = true;
        events.push(DuelEvent::TurnStarted {
            side: s,
            round: self.round,
            stars: side.stars,
        });
        Ok(())
    }

    /// 0x269D80: Exodia leader with all four limbs face-up, ready and
    /// unbound in the eight neighbouring cells.
    fn exodia_complete(&self, s: u8, cards: &impl CardData) -> bool {
        let leader = self.leader(s);
        if leader.card != EXODIA_HEAD {
            return false;
        }
        let _ = cards;
        EXODIA_LIMBS.iter().all(|&limb| {
            self.units.iter().any(|u| {
                u.card == limb
                    && u.side == s
                    && u.committed_face_up
                    && !u.acted
                    && u.spellbind == 0
                    && !self.is_chain_bound(u.id)
                    && u.position.column.abs_diff(leader.position.column) <= 1
                    && u.position.row.abs_diff(leader.position.row) <= 1
                    && u.position != leader.position
            })
        })
    }

    fn lp_winner(&self) -> Option<u8> {
        let (a, b) = (self.sides[0].lp, self.sides[1].lp);
        match a.cmp(&b) {
            std::cmp::Ordering::Greater => Some(0),
            std::cmp::Ordering::Less => Some(1),
            std::cmp::Ordering::Equal => None,
        }
    }

    fn check_life_points(&mut self, events: &mut Vec<DuelEvent>) {
        if self.outcome.is_none() && (self.sides[0].lp == 0 || self.sides[1].lp == 0) {
            let winner = self.lp_winner();
            self.finish(winner, EndReason::LifePoints, events);
        }
    }

    fn finish(&mut self, winner: Option<u8>, reason: EndReason, events: &mut Vec<DuelEvent>) {
        let outcome = Outcome { winner, reason };
        self.outcome = Some(outcome);
        events.push(DuelEvent::Ended { outcome });
    }

    /// Cells a unit could legally end on this action (for UI and AI),
    /// following 0x1E3CE0: orthogonal steps within the allowance, through
    /// empty non-Labyrinth cells, ending on an empty cell or a target.
    pub fn reachable(&self, id: UnitId, cards: &impl CardData) -> Result<Vec<Vec<GridPos>>> {
        let unit = self
            .unit(id)
            .ok_or(DuelError::InvalidCommand("no such unit"))?
            .clone();
        let allowance = self.allowance(&unit, cards)?;
        let mut out = Vec::new();
        let mut stack = vec![vec![]];
        while let Some(path) = stack.pop() {
            let here = path.last().copied().unwrap_or(unit.position);
            if path.len() == allowance {
                continue;
            }
            for (dc, dr) in [(1_i32, 0_i32), (-1, 0), (0, 1), (0, -1)] {
                let (c, r) = (i32::from(here.column) + dc, i32::from(here.row) + dr);
                if !(0..7).contains(&c) || !(0..7).contains(&r) {
                    continue;
                }
                let cell = GridPos {
                    column: c as u8,
                    row: r as u8,
                };
                if cell == unit.position || path.contains(&cell) {
                    continue;
                }
                let mut next = path.clone();
                next.push(cell);
                match self.unit_at(cell) {
                    Some(o) if o.controller() == unit.controller() && o.slot == 0 => (),
                    Some(o) if self.team_mates(o.controller(), unit.controller()) => (),
                    Some(o) if unit.slot == 0 && o.controller() != unit.controller() => (),
                    Some(_) => out.push(next),
                    None if self.terrain_at(cell) == terrain::LABYRINTH
                        && !self.has_wall_flag(&unit, cards) => {}
                    None => {
                        out.push(next.clone());
                        stack.push(next);
                    }
                }
            }
        }
        Ok(out)
    }

    /// 0x1E37F0: the eight neighbours of `side`'s leader, on the board, not
    /// Labyrinth and not holding a unit controlled by the opponent.
    pub fn summon_area(&self, side: u8, cell: GridPos) -> bool {
        let leader = self.leader(side).position;
        cell.index().is_some()
            && cell != leader
            && cell.column.abs_diff(leader.column) <= 1
            && cell.row.abs_diff(leader.row) <= 1
            && self.terrain_at(cell) != terrain::LABYRINTH
            && self.unit_at(cell).is_none_or(|u| u.side == side)
    }

    /// Stars needed to summon `card` alone from `side`'s hand (0x1E1B70).
    pub fn hand_cost(&self, side: u8, card: u16, cards: &impl CardData) -> Option<u8> {
        cards.card(card).map(|c| self.summon_cost(side, c, cards))
    }

    /// Step allowance and wall flag of `unit` (0x247E10 through 0x1E29E0 and
    /// 0x1E2A50). Callers may pass a copy with a pending face-up toggle, as
    /// the move selection does when R2/L2 change it.
    pub fn move_range(&self, unit: &DuelUnit, cards: &impl CardData) -> Result<(u8, bool)> {
        let steps = self.allowance(unit, cards)?;
        Ok((steps as u8, self.has_wall_flag(unit, cards)))
    }

    /// Terrain code of a cell.
    pub fn terrain_of(&self, cell: GridPos) -> Option<u8> {
        cell.index().map(|i| self.terrain[i])
    }

    /// Per-player view: the opponent's hand, deck order and face-down card
    /// identities are withheld (online play must never send them). A port
    /// addition for online play: only with the `online` feature.
    ///
    /// A face-down card is known to its owner (who set it) and to its
    /// controller: a unit taken by Change of Heart / Brain Control (+0x57
    /// bit 1) is shown to its new controller, as the original's card box
    /// does — 0x1DA6A0 and the legend 0x268CA0 decide by the controller
    /// 0x251C90, not the owning group [F queries.rs `hidden`/`legend`].
    #[cfg(feature = "online")]
    pub fn view(&self, viewer: u8) -> DuelView {
        self.view_as(Viewer::Seat(viewer))
    }

    /// [`Self::view`] for a seat or for nobody (an online spectator, a port
    /// addition): with [`Viewer::Nobody`] no hand is included and every
    /// face-down card is withheld, whoever owns or controls it.
    #[cfg(feature = "online")]
    pub fn view_as(&self, viewer: Viewer) -> DuelView {
        // In 2v2 (a port addition) a team-mate's cards are known too.
        let knows = |u: &DuelUnit| match viewer {
            Viewer::Seat(s) => {
                u.side == s || u.controller() == s || self.team_mates(s, u.side) || self.team_mates(s, u.controller())
            }
            Viewer::Nobody => false,
        };
        DuelView {
            terrain: self.terrain.clone(),
            units: self
                .units
                .iter()
                .map(|u| UnitView {
                    id: u.id,
                    card: (knows(u) || u.committed_face_up).then_some(u.card),
                    side: u.side,
                    slot: u.slot,
                    position: u.position,
                    face_up: u.committed_face_up || (knows(u) && u.face_up),
                    defence: u.defence,
                    acted: u.acted,
                    spellbound: u.spellbind > 0,
                })
                .collect(),
            lp: self.sides.iter().map(|s| s.lp).collect(),
            stars: self.sides.iter().map(|s| s.stars).collect(),
            hand: match viewer {
                Viewer::Seat(s) => self
                    .sides
                    .get(usize::from(s))
                    .map_or_else(Vec::new, |side| side.hand.clone()),
                Viewer::Nobody => Vec::new(),
            },
            hand_counts: self.sides.iter().map(|s| s.hand.len()).collect(),
            deck_counts: (0..self.sides.len() as u8).map(|s| self.deck_count(s)).collect(),
            round: self.round,
            active: self.active,
            outcome: self.outcome,
        }
    }
}

/// Who a [`Duel::view_as`] is for: a seat (side 0 or 1), or nobody (an
/// online spectator: no hand, no face-down identity). A port addition for
/// online play.
#[cfg(feature = "online")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viewer {
    Seat(u8),
    Nobody,
}

#[cfg(feature = "online")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitView {
    pub id: UnitId,
    pub card: Option<u16>,
    pub side: u8,
    pub slot: u8,
    pub position: GridPos,
    pub face_up: bool,
    pub defence: bool,
    pub acted: bool,
    pub spellbound: bool,
}

#[cfg(feature = "online")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuelView {
    pub terrain: Vec<u8>,
    pub units: Vec<UnitView>,
    /// Per side (two entries in a 2-player duel: the wire form of the
    /// former `[_; 2]` arrays).
    pub lp: Vec<u16>,
    pub stars: Vec<u8>,
    pub hand: Vec<u16>,
    pub hand_counts: Vec<usize>,
    pub deck_counts: Vec<usize>,
    pub round: u8,
    pub active: u8,
    pub outcome: Option<Outcome>,
}

pub mod ai;
pub mod multi;
pub use multi::{build_decks, Knockout, Multi, MultiMode, MultiRules, MultiSetup, MULTI_LEADER_CELLS, MULTI_ROUND_LIMIT, MULTI_TURN_ORDER};
mod countertraps;
mod decree;
mod destiny;
pub use destiny::DESTINY_MESSAGE_BOX;
mod destruction_seto;
mod experience;
mod feather_duster;
mod flip_seto;
mod immunity;
mod more_rules;
mod powerup_attack;
mod white_rules;
use white_rules::FlipRole;
mod sweep_rules;
mod ruin_elimination;
mod unit_records;
pub use unit_records::UnitRecord;
use unit_records::EquipSpecial;
pub mod queries;
pub mod script;
pub mod session;
pub mod ui;
pub use experience::{recompute_rank, ExperienceAward, ExperienceChange, RANKED_CARDS, RANK_COUNT};

#[cfg(test)]
mod effects_next_tests;
#[cfg(test)]
mod experience_tests;
#[cfg(test)]
mod retype_equip_tests;
#[cfg(test)]
mod leader_ability_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod traps_field_tests;
#[cfg(test)]
mod immortal_mirror_force_tests;
#[cfg(test)]
mod manawyddan_tests;
#[cfg(test)]
mod more_rules_tests;
#[cfg(test)]
mod white_rules_tests;
#[cfg(test)]
mod refusals_tests;
#[cfg(test)]
mod graverobber_tests;
#[cfg(test)]
mod destiny_tests;
