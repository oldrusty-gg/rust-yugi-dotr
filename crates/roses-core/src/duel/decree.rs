//! Royal Decree's event 0x66 reaction in the movement and battle builders.

use super::*;

impl Duel {
    /// 0x268710(0xF), shared by the far-trap search and near-trap builder.
    pub(super) fn traps_suppressed(&self, cards: &impl CardData) -> bool {
        self.units.iter().any(|u| {
            u.slot != 0
                && u.defence
                && u.committed_face_up
                && u.committed_defence
                && cards
                    .card(u.card)
                    .and_then(|c| c.entries[2])
                    .is_some_and(|[word, _]| word >> 8 == 0xF)
        })
    }

    /// 0x24CE84 scans occupied cells by column then row; 0x24DC50 orders
    /// the resulting list by descending summon age with these exact swaps.
    /// The pivot swaps determine priority even when summon ages are equal.
    pub(super) fn order_near_traps(traps: &mut [DuelUnit]) {
        fn sort(traps: &mut [DuelUnit]) {
            if traps.len() <= 1 {
                return;
            }
            traps.swap(0, (traps.len() - 1) / 2);
            let mut pivot = 0;
            for i in 1..traps.len() {
                if traps[0].summon_order < traps[i].summon_order {
                    pivot += 1;
                    traps.swap(pivot, i);
                }
            }
            traps.swap(0, pivot);
            // The original first recursion includes its pivot; preserve
            // that partition boundary and the resulting equal-age order.
            sort(&mut traps[..=pivot]);
            sort(&mut traps[pivot + 1..]);
        }
        traps.sort_by_key(|u| (u.position.column, u.position.row));
        sort(traps);
    }

    /// Search both physical groups, keeping `first` on equal summon order.
    /// The caller establishes that a trap exists at the source's priority
    /// boundary. Event 0x66 (0x26D880) reveals Decree, marks it acted, and
    /// leaves both cards in place. The trap being cancelled is untouched.
    pub(super) fn royal_decree_reacts(
        &mut self,
        first: u8,
        cards: &impl CardData,
        events: &mut Vec<DuelEvent>,
    ) -> bool {
        let Some(id) = self.far_trap_in(&self.first_then_all(first), trap_type::ROYAL_DECREE, cards) else {
            return false;
        };
        let u = self.unit(id).expect("selected Decree");
        if !u.committed_face_up {
            events.push(DuelEvent::Revealed {
                unit: id,
                card: u.card,
            });
        }
        // 0x26D8CC always calls 0x252FF0(mode 1): set BOTH face bits even
        // when the committed flag was already set. 0x26DA00 sets acted.
        self.activate(id, None, events);
        self.units
            .iter_mut()
            .find(|u| u.id == id)
            .expect("reacting Decree")
            .acted = true;
        true
    }
}

#[cfg(test)]
mod tests;
