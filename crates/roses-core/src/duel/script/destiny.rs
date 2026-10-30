//! The Destiny Draw of a scripted side: the low-LP special draw as lesson 3
//! exercises it (research: `claude-effects/SPECIAL-DRAW.md`, tags as in
//! `script.rs`).
//!
//! The rules engine keeps the per-side Destiny byte (`SideState::destiny`,
//! S+0x344: low nibble = situation 1..=7 selecting a card row, bits 4–6 =
//! rank bonus, bit 7 = used) but refuses to arm it at a turn start and does
//! not draw the special card; the practice script presets the byte
//! (`S0+0x344 = 2`, lesson 3 step 240) and the session applies the draw here
//! after the engine's plain refill of `OpenSummon` [F, emu, V+O]:
//!
//! - **Pending** (state 0x3E, 0x1D648C): `deck > 0 && byte != 0 &&
//!   !(byte & 0x80) && hand < 5` when Cross/Square accepts the summon cell;
//!   event 0x72 shows message 0x116 ([`DESTINY_MESSAGE`]) before the refill.
//! - **Refill** 0x1D8F90: `target = min(hand + deck, 5)`, then `min(target
//!   + 1, 5)` for the special draw; every card but the last comes from the
//!   deck front, the last from [`pick`] (which sets bit 7). The engine's
//!   refill draws `min(hand + deck, 5) − hand` from the deck, one more than
//!   the original whenever `deck ≥ 5 − hand`; that card goes back to the deck
//!   front and the special card is appended ([`Duel::script_destiny_draw`]).
//! - **Pick** 0x270FC0 + 0x1D912C: row = byte & 0xF into the `u16[3]` rows
//!   of 0x2B7AF0 (`roses-assets::parse_pal_destiny_rows`), rank bonus rb =
//!   (byte >> 4) & 7, (b0, b1) by the field leader's kind (switch 0x2D3400),
//!   w0 = b0 + 80·rb + 10, w1 = b1 + 40·rb + 20, r = (int)(100.0f ·
//!   ((float)rand31 / 2^31)), ids[r < w0 ? 0 : r < w0 + w1 ? 1 : 2]. The
//!   pick fails (−1, deck draw instead, no bit 7) for a leader kind ≥ 21
//!   [F, emu T6/T7]. In the real game the rand value depends on frame timing
//!   (presentation code consumes the RNG), so the card is reproducible only
//!   for a given [`Tutorial::rng`] state; lesson 3's recording drew 429
//!   Arsenal Bug [O 377].

use super::Tutorial;
use crate::duel::{next_rand31, CardData, Duel, DuelError, DuelEvent, Result, HAND_SIZE};
use serde::{Deserialize, Serialize};

/// Rows 0..=7 of 0x2B7AF0 (row 0 is the 999 filler, never armed).
pub const DESTINY_ROW_COUNT: usize = 8;
pub type DestinyRows = [[u16; 3]; DESTINY_ROW_COUNT];
/// Bit 7 of the byte: the card was drawn (never re-armed in the duel).
pub const DESTINY_USED: u8 = 0x80;
/// Message 0x116 "Destiny Draw!" of event 0x72 (string 278) [F].
pub const DESTINY_MESSAGE: u16 = 278;
/// Leader kinds the pick handles (0x2D3400 has 21 cases).
pub const DESTINY_KINDS: u8 = 21;

/// (b0, b1) of the pick by the leader's kind (switch 0x2D3400; `None` for a
/// kind the switch has no case for) [F, emu T6].
pub fn kind_weights(kind: u8) -> Option<(i32, i32)> {
    Some(match kind & 0x1F {
        0 | 7 => (0, 0),
        1 | 3 | 16 => (2, 1),
        2 | 8 => (12, 6),
        4 => (5, 3),
        5 | 12 => (8, 4),
        6 => (6, 3),
        9 | 15 | 18 | 19 => (10, 5),
        10 | 13 => (14, 7),
        11 => (15, 8),
        14 => (3, 1),
        17 => (13, 6),
        20 => (-5, -2),
        _ => return None,
    })
}

/// 0x270FC0 with the bucket draw of 0x1D912C: the special card for the
/// Destiny byte `byte` of a side whose field leader has `leader_kind`, or
/// `None` when the pick fails (used byte, unknown kind, or a row past the
/// table — arming never produces one). Exactly one rand31 is consumed on a
/// successful pick.
pub fn pick(rows: &DestinyRows, byte: u8, leader_kind: u8, rng: &mut u64) -> Option<u16> {
    if byte & DESTINY_USED != 0 {
        return None;
    }
    let (b0, b1) = kind_weights(leader_kind)?;
    let ids = rows.get(usize::from(byte & 0xF))?;
    let rb = i32::from((byte >> 4) & 7);
    let w0 = b0 + 80 * rb + 10;
    let w1 = b1 + 40 * rb + 20;
    let r = (100.0_f32 * (next_rand31(rng) as f32 / 2_147_483_648.0_f32)) as i32;
    Some(
        ids[if r < w0 {
            0
        } else if r < w0 + w1 {
            1
        } else {
            2
        }],
    )
}

/// The special card a frame drew (event 0x72 with [`DESTINY_MESSAGE`], then
/// the card as the last one of the refill).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestinyDraw {
    pub side: u8,
    pub card: u16,
}

impl Duel {
    /// `S0+0x344 = v` 0x1DF9E0(side, v).
    pub fn script_set_destiny(&mut self, side: u8, value: u8) {
        self.sides[usize::from(side)].destiny = value;
    }

    /// The state-0x3E test: `side`'s next accepted summon cell draws the
    /// special card [F].
    pub fn script_destiny_pending(&self, side: u8) -> bool {
        let s = &self.sides[usize::from(side)];
        !s.deck.is_empty()
            && s.destiny != 0
            && s.destiny & DESTINY_USED == 0
            && s.hand.len() < HAND_SIZE
    }

    /// After the engine's refill of `OpenSummon` for `side`, whose hand and
    /// deck held `before` = (hand, deck) cards: the original's refill loop
    /// with the special card. Returns the draw, or `None` when the pick
    /// failed and the original would have drawn the same deck cards as the
    /// engine.
    pub fn script_destiny_draw(
        &mut self,
        side: u8,
        before: (usize, usize),
        rows: &DestinyRows,
        cards: &impl CardData,
        rng: &mut u64,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Option<DestinyDraw>> {
        let (hand_before, deck_before) = before;
        let leader = self.leader(side).card;
        let kind = cards
            .card(leader)
            .ok_or(DuelError::InvalidCommand("leader card unknown"))?
            .kind;
        let byte = self.sides[usize::from(side)].destiny;
        let Some(card) = pick(rows, byte, kind, rng) else {
            // 0x270FC0 = −1: the last card comes from the deck like the
            // others. With hand + deck < 5 the original pops one card more
            // than the deck holds (0x1DFFB0 on an empty deck), not traced.
            if hand_before + deck_before < HAND_SIZE {
                return Err(DuelError::Unsupported(format!(
                    "a Destiny Draw whose pick fails (leader kind {kind}, byte {byte:#x}) with hand + deck < 5"
                )));
            }
            return Ok(None);
        };
        if cards.card(card).is_none() {
            return Err(DuelError::InvalidCommand(
                "Destiny Draw row names an unknown card",
            ));
        }
        let s = &mut self.sides[usize::from(side)];
        let drew = events.iter_mut().rev().find_map(|e| match e {
            DuelEvent::Drew { side: d, cards } if *d == side => Some(cards),
            _ => None,
        });
        let mut drawn = drew.map(std::mem::take).unwrap_or_default();
        if deck_before >= HAND_SIZE - hand_before {
            // The engine took the original's special slot from the deck.
            let returned = s.hand.pop().expect("the refill drew a card");
            s.deck.insert(0, returned);
            if drawn.last() == Some(&returned) {
                drawn.pop();
            }
        }
        s.hand.push(card);
        s.destiny |= DESTINY_USED;
        drawn.push(card);
        match events.iter_mut().rev().find_map(|e| match e {
            DuelEvent::Drew { side: d, cards } if *d == side => Some(cards),
            _ => None,
        }) {
            Some(cards) => *cards = drawn,
            None => events.push(DuelEvent::Drew { side, cards: drawn }),
        }
        Ok(Some(DestinyDraw { side, card }))
    }
}

impl Tutorial {
    /// Whether the active side's next `OpenSummon` draws the special card
    /// (checked before the command is applied).
    pub fn destiny_pending(&self, duel: &Duel) -> bool {
        duel.script_destiny_pending(duel.active)
    }

    /// The special draw after the engine refilled the active side's hand
    /// ([`Duel::script_destiny_draw`] with this state's rows and RNG).
    pub fn destiny_draw(
        &mut self,
        duel: &mut Duel,
        cards: &impl CardData,
        hand_before: usize,
        deck_before: usize,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Option<DestinyDraw>> {
        let Some(rows) = self.destiny_rows.as_ref() else {
            return Err(DuelError::Unsupported(
                "the Destiny Draw needs the card rows of 0x2B7AF0 (`Tutorial::destiny_rows`, from `roses-assets::parse_pal_destiny_rows`)".into(),
            ));
        };
        let rows = *rows;
        duel.script_destiny_draw(
            duel.active,
            (hand_before, deck_before),
            &rows,
            cards,
            &mut self.rng,
            events,
        )
    }
}
