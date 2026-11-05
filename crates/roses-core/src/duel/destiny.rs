//! The low-LP special draw ("Destiny Draw") for every side: arming at the
//! side's turn start (0x270800 / 0x270900) and the special card as the last
//! card of the summon refill (state 0x3E event 0x72, refill 0x1D8F90, pick
//! 0x270FC0) (the project notes §2–4, next/ABILITY-3-CPU.md).
//! The pick and the weights are shared with the practice scripts
//! (`script::destiny`); the card rows of 0x2B7AF0 are supplied by the duel's
//! creator ([`Duel::set_destiny_rows`], from
//! `roses-assets::parse_pal_destiny_rows`).

use super::script::destiny::{pick, DestinyRows, DESTINY_USED};
use super::*;

/// Event 0x72's message 0x116 "Destiny Draw!" (MSG string 278) [F].
pub const DESTINY_MESSAGE_BOX: u16 = 0x116;

impl Duel {
    /// The card rows of 0x2B7AF0 the special card is picked from.
    pub fn set_destiny_rows(&mut self, rows: DestinyRows) {
        self.destiny_rows = Some(rows);
    }

    /// Each side's saved leader rank (0 NCO .. 12 SD; 0x2154C0): the rank
    /// bonus of the Destiny byte is max(rank - 8, 0) [F].
    pub fn set_leader_ranks(&mut self, ranks: [u8; 2]) {
        self.leader_ranks = ranks.to_vec();
    }

    /// "Armed" as every reader tests it: `deck > 0 && v != 0 && !(v & 0x80)`.
    fn destiny_armed(&self, s: u8) -> bool {
        let side = &self.sides[usize::from(s)];
        self.deck_count(s) > 0 && side.destiny != 0 && side.destiny & DESTINY_USED == 0
    }

    /// The end of the turn-start coroutine 0x269570 (0x269CBC–0x269D58):
    /// `if !(byte & 0x80) && (n = 0x270800(s)) > 0: byte = n` [F, emu T2/T12/T17].
    pub(super) fn arm_destiny(&mut self, s: u8, cards: &impl CardData) -> Result<()> {
        if self.sides[usize::from(s)].destiny & DESTINY_USED != 0 {
            return Ok(());
        }
        if let Some(n) = self.destiny_test(s, cards)? {
            self.sides[usize::from(s)].destiny = n;
        }
        Ok(())
    }

    /// 0x270800(s): the controller class first (computer modules other than
    /// Seto 1 / Yugi 0x13 stop before anything else), the rank, ability #3
    /// for pad sides only (0x273070: listed, no opposing effect 0x58), LP <
    /// 1000, then the classifier 0x270900; `None` = −1. Scripts and CPU
    /// sides draw one rand31 (6 + (r & 1)); a human draws one only in the
    /// LP < 250 fallback.
    fn destiny_test(&mut self, s: u8, cards: &impl CardData) -> Result<Option<u8>> {
        let ctrl = self.controllers[usize::from(s)];
        if ctrl < 0x20 && ctrl != 1 && ctrl != 0x13 {
            return Ok(None);
        }
        let rank = self.leader_ranks[usize::from(s)];
        if ctrl >= PAD_CONTROLLER && !self.leader_has(s, 3, cards) {
            return Ok(None);
        }
        if self.sides[usize::from(s)].lp >= 1000 {
            return Ok(None);
        }
        let v = if ctrl < PAD_CONTROLLER {
            6 + (self.rand31() & 1) as u8
        } else {
            match self.destiny_situation(s, cards)? {
                Some(v) => v,
                None if self.sides[usize::from(s)].lp < 250 => 6 + (self.rand31() & 1) as u8,
                None => return Ok(None),
            }
        };
        Ok(Some(v | (rank.saturating_sub(8).min(7) << 4)))
    }

    /// 0x270900's situations 1–5 for a human side (first match wins; counts
    /// over slots 1..10 of both groups, "strongest" = 0x24E360(0, 0, s, 3,
    /// 0x21, 0xE), controller 0x251C80, effective ATK at the unit's own cell)
    /// [F; emu T9/T10/T13: 0 mismatches on 376 boards].
    pub fn destiny_situation(&self, s: u8, cards: &impl CardData) -> Result<Option<u8>> {
        let o = self.opp(s);
        let field: Vec<&DuelUnit> = self.units.iter().filter(|u| (1..=10).contains(&u.slot)).collect();
        let kind = |u: &DuelUnit| self.unit_card(u, cards).kind;
        let monster = |u: &DuelUnit| kind(u) >> 5 == 0;
        let strongest = self.strongest_monster(s, 3, cards)?.and_then(|id| self.unit(id));
        let strongest_ctrl = strongest.map(|u| u.controller());
        let opp_monsters = field.iter().filter(|u| u.controller() == o && monster(u)).count();
        let leader = self.leader(s);
        // 1 (0x27094C): a Fiend strongest of the opponent's within Manhattan
        // distance 3 of s's leader, the opponent controlling >= 3 Fiends.
        if let Some(st) = strongest {
            let dist = st.position.column.abs_diff(leader.position.column)
                + st.position.row.abs_diff(leader.position.row);
            let fiends = field.iter().filter(|u| u.controller() == o && kind(u) == kind::FIEND).count();
            if st.controller() == o && kind(st) == kind::FIEND && dist < 3 && fiends >= 3 {
                return Ok(Some(1));
            }
        }
        // 2 (0x270A4C): an Insect leader against >= 3 opposing monsters, >= 3
        // of them with effective ATK in [3000, 4000) and not favoured by the
        // Forest: 0x24F820(0, g, s) is the terrain affinity 0x24F8A0 of
        // terrain 0 (Beast-Warrior, Beast, Insect, Pyro, Plant skipped; the
        // earlier reading "non-Immortal" was case 9) [F; emu T18].
        if opp_monsters >= 3 && kind(leader) == kind::INSECT {
            let strong = field
                .iter()
                .filter(|u| {
                    u.controller() == o
                        && monster(u)
                        && terrain_affinity(terrain::FOREST, &self.unit_card(u, cards)) != 1
                })
                .filter(|u| (3000..4000).contains(&self.effective(u, u.position, true, cards)))
                .count();
            if strong >= 3 {
                return Ok(Some(2));
            }
        }
        // 3 (0x270B6C): >= 3 monsters on the board, >= 31 Labyrinth cells,
        // and among the monsters whose movement entry has type 8/9/10
        // (wall-walkers) none is s's and >= 3 are the opponent's.
        let monsters = field.iter().filter(|u| monster(u)).count();
        let walls = self.terrain.iter().filter(|&&t| t == terrain::LABYRINTH).count();
        if monsters >= 3 && walls >= 31 {
            let walker = |u: &DuelUnit| {
                monster(u)
                    && self
                        .unit_card(u, cards)
                        .entries[1]
                        .is_some_and(|[w, _]| matches!(w >> 8, 8..=10))
            };
            let own = field.iter().filter(|u| u.controller() == s && walker(u)).count();
            let opp = field.iter().filter(|u| u.controller() != s && walker(u)).count();
            if own == 0 && opp >= 3 {
                return Ok(Some(3));
            }
        }
        // 4 (0x270CB8): the strongest is not s's (or there is none: A = 0) and
        // the hand's best base ATK (-1 for an empty hand) trails it by less
        // than half the opponent's LP.
        if strongest_ctrl != Some(s) {
            let best = self.sides[usize::from(s)]
                .hand
                .iter()
                .filter(|&&c| c != 999)
                .filter_map(|&c| cards.card(c))
                .map(|c| i32::from(c.attack & 0x1FFF))
                .max()
                .unwrap_or(-1);
            let a = strongest.map_or(0, |u| i32::from(self.effective(u, u.position, true, cards)));
            if best < a && a - best < i32::from(self.sides[usize::from(o)].lp >> 1) {
                return Ok(Some(4));
            }
        }
        // 5 (0x270DB8): the strongest is s's and the opponent controls >= 4
        // trap-class cards (kind class 3 / 4).
        if strongest_ctrl == Some(s) {
            let traps = field
                .iter()
                .filter(|u| u.controller() == o && matches!(kind(u) >> 5, 3 | 4))
                .count();
            if traps >= 4 {
                return Ok(Some(5));
            }
        }
        Ok(None)
    }

    /// The refill 0x1D8F90 with the special card (state 0x3E decided it on
    /// the accepted cell: event 0x72 first): `target = min(hand + deck, 5)`,
    /// + 1 for the special card (max 5); the last card comes from the pick
    /// (bit 7 set, one rand31), the others from the deck front. Returns the
    /// drawn cards, or `None` when the side is not armed (the plain refill
    /// runs) or a scripted side's draw is left to the practice session (no
    /// rows on the duel).
    pub(super) fn destiny_refill(
        &mut self,
        s: u8,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> Result<Option<Vec<u16>>> {
        if !self.destiny_armed(s) || self.sides[usize::from(s)].hand.len() >= HAND_SIZE {
            return Ok(None);
        }
        let Some(rows) = self.destiny_rows else {
            if script::scripted(self.controllers[usize::from(s)]) {
                return Ok(None);
            }
            return Err(unsupported(
                "the Destiny Draw needs the card rows of 0x2B7AF0 (`Duel::set_destiny_rows`)",
            ));
        };
        let leader_kind = self.unit_card(self.leader(s), cards).kind;
        let byte = self.sides[usize::from(s)].destiny;
        let (hand, deck) = {
            let side = &self.sides[usize::from(s)];
            (side.hand.len(), self.deck_count(s))
        };
        let target = ((hand + deck).min(HAND_SIZE) + 1).min(HAND_SIZE);
        let mut drawn = Vec::new();
        let mut special = None;
        for i in hand..target {
            let mut card = None;
            if i + 1 >= target {
                let mut rng = self.rng;
                if let Some(c) = pick(&rows, byte, leader_kind, &mut rng) {
                    if cards.card(c).is_none() {
                        return Err(DuelError::InvalidCommand(
                            "Destiny Draw row names an unknown card",
                        ));
                    }
                    self.rng = rng;
                    self.sides[usize::from(s)].destiny |= DESTINY_USED;
                    special = Some(c);
                    card = Some(c);
                }
            }
            let card = match card {
                Some(c) => c,
                None => {
                    // 0x1DFFB8 on an empty slot 0 (an empty deck, or one
                    // holding only stranded cards) returns 999, which the
                    // hand insert 0x1E03B0 rejects (ids >= 0x356): nothing
                    // is drawn [F 0x1DFFD4, 0x1E03F4].
                    if self.sides[usize::from(s)].deck.is_empty() {
                        continue;
                    }
                    self.draw_front(s)
                }
            };
            self.sides[usize::from(s)].hand.push(card);
            drawn.push(card);
        }
        if let Some(card) = special {
            // Event 0x72 (message 0x116) plays before the refill.
            let at = events.len();
            events.insert(at, DuelEvent::DestinyDraw { side: s, card });
        }
        Ok(Some(drawn))
    }
}
