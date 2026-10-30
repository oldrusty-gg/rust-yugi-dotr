//! A client-side copy of the duel rebuilt from one seat's [`PlayerView`]:
//! everything the seat may see is exact, everything hidden from it is a
//! placeholder ([`hidden_card`]): the opponent's hand and face-down cards,
//! and both decks (counts only). The client runs its unchanged input layer
//! (`roses_core::duel::ui::DuelUi`, which reads only the acting seat's own
//! cards, the board and public counters) and its presentation on this copy;
//! it never applies commands to it. The server stays the only authority:
//! every intention the input layer produces is sent, and the next revision's
//! view replaces the copy.
//!
//! Invariant (tested): `player_view(&mirror_duel(view), seat) == view`, and
//! for a spectator's view `spectator_view(&mirror_duel(view)) == view`.

use crate::filter::{PlayerView, SeenEvent};
use roses_core::duel::{CardData, Duel, DuelEvent, DuelUnit, SideState};

/// The card id that stands for a card this seat may not see: the lowest
/// catalogue id of a monster without effect entries, chosen from the rules
/// data so the copy stays a valid duel. It is never shown as a card: the
/// presentation hides every record the original hides (face-down cards of
/// the other side, its hand), and the identities the server reveals replace
/// it ([`reveal`]).
pub fn hidden_card(cards: &roses_core::duel::Catalogue) -> u16 {
    (0..roses_core::CARD_COUNT)
        .find(|&id| {
            cards
                .card(id)
                .is_some_and(|c| c.is_monster() && c.entries.iter().all(Option::is_none) && !c.effects.iter().any(|&e| e))
        })
        .or_else(|| (0..roses_core::CARD_COUNT).find(|&id| cards.card(id).is_some()))
        .unwrap_or(0)
}

/// The duel as `view.seat` may know it (see the module notes).
pub fn mirror_duel(view: &PlayerView, cards: &roses_core::duel::Catalogue) -> Result<Duel, String> {
    let hidden = hidden_card(cards);
    let b = &view.board;
    let seat = if view.multi.is_some() { usize::from(view.seat) } else { usize::from(view.seat & 1) };
    if view.public.units.len() != b.units.len() {
        return Err("the view's unit lists differ".into());
    }
    let units: Vec<DuelUnit> = b
        .units
        .iter()
        .zip(&view.public.units)
        .map(|(u, p)| {
            if u.id != p.id {
                return Err(format!("unit {} has no public detail", u.id.0));
            }
            Ok(DuelUnit {
                id: u.id,
                card: u.card.unwrap_or(hidden),
                side: u.side,
                slot: u.slot,
                position: u.position,
                face_up: u.face_up,
                defence: u.defence,
                committed_face_up: p.committed_face_up,
                committed_defence: p.committed_defence,
                acted: u.acted,
                spellbind: p.spellbind,
                summon_order: p.summon_order,
                attack_bonus: p.attack_bonus,
                defense_bonus: p.defense_bonus,
                spellbind_fresh: p.spellbind_fresh,
                evo_counter: p.evo_counter,
                stolen: p.stolen,
                discount: p.discount,
                stolen_by: p.stolen_by,
            })
        })
        .collect::<Result<_, String>>()?;
    let side = |s: usize| SideState {
        lp: b.lp[s],
        stars: b.stars[s],
        can_summon: view.can_summon[s],
        // A spectator's view has no hand: both are placeholders.
        hand: if s == seat && !view.spectator {
            b.hand.clone()
        } else if let Some(t) = view.team_hands.iter().find(|t| usize::from(t.side) == s) {
            // 2v2: the team-mate's hand.
            t.hand.clone()
        } else {
            match &view.open_hand {
                Some(hand) if hand.len() == b.hand_counts[s] && view.multi.is_none() => hand.clone(),
                _ => vec![hidden; b.hand_counts[s]],
            }
        },
        deck: vec![hidden; b.deck_counts[s]],
        leader_abilities: view.public.leader_abilities[s].clone(),
        summons: view.public.summons[s],
        graveyard: view.graveyards[s].clone(),
        destiny: if s == seat && !view.spectator { view.public.own_destiny } else { 0 },
    };
    let mut duel = match &view.multi {
        // N-player duels (a port addition).
        Some(multi) => Duel::from_observed_multi(
            b.terrain.clone(),
            units,
            (0..b.lp.len()).map(side).collect(),
            b.round,
            view.first,
            b.active,
            multi.clone(),
            cards,
        ),
        None => Duel::from_observed(b.terrain.clone(), units, [side(0), side(1)], b.round, view.first, b.active, cards),
    }
    .map_err(|e| format!("the view is not a duel: {e}"))?;
    duel.pending_summon = view.pending_summon;
    duel.pending = view.pending.clone();
    duel.outcome = b.outcome;
    duel.controllers = view.public.controllers.clone();
    duel.leader_ranks = view.public.leader_ranks.clone();
    duel.chain_bound = view.public.chain_bound.clone();
    duel.star_income = view.star_income.clone();
    duel.records = view
        .public
        .units
        .iter()
        .filter_map(|p| Some((p.id.0, p.record.clone()?)))
        .collect();
    Ok(duel)
}

/// Engine events for the presentation from one revision's filtered events:
/// hidden cards become [`hidden_card`] placeholders (the opponent's draws,
/// face-down summons and creations); a combination made out of sight
/// ([`SeenEvent::Combined`]) has no engine event and is dropped.
pub fn duel_events(seen: &[SeenEvent], hidden: u16) -> Vec<DuelEvent> {
    seen.iter()
        .filter_map(|e| match e {
            SeenEvent::Public { event } => Some(event.clone()),
            SeenEvent::Drew { side, count } => Some(DuelEvent::Drew {
                side: *side,
                cards: vec![hidden; *count],
            }),
            SeenEvent::DestinyDraw { side } => Some(DuelEvent::DestinyDraw {
                side: *side,
                card: hidden,
            }),
            SeenEvent::Summoned { unit, at, cost } => Some(DuelEvent::Summoned {
                unit: *unit,
                card: hidden,
                at: *at,
                cost: *cost,
            }),
            SeenEvent::Created { unit, at } => Some(DuelEvent::Created {
                unit: *unit,
                card: hidden,
                at: *at,
            }),
            SeenEvent::Combined { .. } => None,
        })
        .collect()
}

/// Put the identities a revision revealed (a flipped, destroyed, activated
/// or discarded card) into the copy from before it, so the presentation of
/// that revision (the battle's versus cards, an activation) shows the real
/// card where the original shows it. Returns how many units changed.
pub fn reveal(duel: &mut Duel, seen: &[SeenEvent]) -> usize {
    let mut changed = 0;
    for e in seen {
        let SeenEvent::Public { event } = e else {
            continue;
        };
        let (unit, card) = match event {
            DuelEvent::Revealed { unit, card }
            | DuelEvent::Destroyed { unit, card }
            | DuelEvent::Activated { unit, card, .. }
            | DuelEvent::Discarded { unit, card }
            | DuelEvent::Concealed { unit, card } => (*unit, *card),
            _ => continue,
        };
        if let Some(u) = duel.units.iter_mut().find(|u| u.id == unit) {
            if u.card != card {
                u.card = card;
                changed += 1;
            }
        }
    }
    changed
}
