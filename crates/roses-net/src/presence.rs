//! Presses streamed to the other seat ([`crate::Press`]): what the server
//! removes before relaying one, and the check a receiving client runs
//! before showing another seat's input state.
//!
//! In the original's 2-player duel both players watch one screen: the
//! acting player's cursor, menus (hand row, marks, the move selection, the
//! graveyard and unit picks, the card details, the surrender and settings
//! windows) are seen by the other player, while hidden cards stay hidden
//! (Card Display HIDE). A press carries the acting seat's input layer
//! state; the only card ids in it are the details view's card, which the
//! server keeps only when the other seat may see that card.

use crate::filter::PlayerView;
use crate::protocol::Press;
use roses_core::duel::ui::{DuelUi, Mode, DETAILS_PAGES};
use roses_core::duel::Duel;

/// Remove what `viewer` may not see from `press` (the other seat's press):
/// the details view's card unless it is a card `view` (the viewer's own
/// view) shows — the field unit's card, or a graveyard entry while the
/// graveyard pick is open, or the other side's hand card under OPEN.
pub fn sanitize(press: &mut Press, view: &PlayerView) {
    let Some(details) = press.input.details.as_mut() else {
        return;
    };
    let shown = match details.unit {
        Some(unit) => view.board.units.iter().find(|u| u.id == unit).and_then(|u| u.card),
        None if matches!(press.input.mode, Mode::Graveyard(_)) => view
            .graveyards
            .iter()
            .any(|g| g.contains(&details.card))
            .then_some(details.card),
        None => view
            .open_hand
            .as_ref()
            .filter(|h| h.contains(&details.card))
            .map(|_| details.card),
    };
    details.card = shown.unwrap_or(0);
}

/// Whether another seat's input state fits this copy of the duel well
/// enough to be shown (indices in range, units present): a malformed or
/// stale state is never adopted, the receiver keeps its own.
pub fn plausible(input: &DuelUi, duel: &Duel) -> bool {
    let on_board = |p: roses_core::GridPos| p.index().is_some();
    let side = if duel.multi().is_some() { usize::from(duel.active) } else { usize::from(duel.active & 1) };
    let hand = duel.sides[side].hand.len();
    let menu_ok = |m: &roses_core::duel::ui::HandMenu, n: usize| {
        (n == 0 || m.cursor < n)
            && m.marks.len() <= n
            && m.marks.iter().all(|&i| i < n)
            && m.marks.iter().enumerate().all(|(k, i)| !m.marks[..k].contains(i))
    };
    if input.cursors.len() != duel.sides.len() || !input.cursors.iter().all(|&c| on_board(c)) {
        return false;
    }
    let mode_ok = match &input.mode {
        Mode::Field => true,
        Mode::Move(sel) => {
            duel.unit(sel.unit).is_some()
                && !sel.path.is_empty()
                && sel.path.len() <= 64
                && sel.path.iter().all(|&p| on_board(p))
                && (sel.marks.is_empty() || sel.marks.len() == 49)
        }
        Mode::SummonCell { leader, menu } => on_board(*leader) && menu_ok(menu, hand.max(menu.cursor + 1)),
        Mode::Hand(menu) | Mode::Confirm(menu) => hand > 0 && menu_ok(menu, hand),
        Mode::SelectUnit(pick) => {
            !pick.units.is_empty() && pick.index < pick.units.len() && pick.units.iter().all(|&u| duel.unit(u).is_some())
        }
        Mode::Graveyard(g) => {
            usize::from(g.shown) < duel.sides.len()
                && g.lists.len() == duel.sides.len()
                && g.row.len() == duel.sides.len()
                && g.scroll.len() == duel.sides.len()
                && (0..duel.sides.len()).all(|s| {
                    let n = duel.sides[s].graveyard.len();
                    g.lists[s].iter().all(|&i| i < n) && usize::from(g.scroll[s]) <= g.lists[s].len() && g.row[s] < 5
                })
        }
    };
    let details_ok = input
        .details
        .is_none_or(|d| d.page < DETAILS_PAGES && d.unit.is_none_or(|u| duel.unit(u).is_some()));
    let hand_view_ok = input.hand_view.is_none_or(|v| v.cursor < hand.max(1));
    let settings_ok = input.settings.is_none_or(|p| p.row < 2);
    mode_ok && details_ok && hand_view_ok && settings_ok
}
