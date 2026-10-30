//! Per-seat filtering of the authoritative duel: what one player may see.
//!
//! Hidden from a seat: the opponent's hand, every deck's order (a seat sees
//! only deck counts, its own included), the identity of the opponent's
//! face-down cards, the random-generator state, the summon scratch records
//! (`temp_units`, which hold the acting side's hand) and experience
//! bookkeeping. Public: the board positions and face-up cards, LP, stars,
//! hand/deck counts, graveyards (online-profile decision: the pick lists of
//! Monster Reborn-type cards show them), whose turn it is, open selections.
//!
//! Events are filtered too: the opponent's draws become counts, their
//! face-down summons lose the card id, and their hand-chain fusions/equips
//! become [`SeenEvent::Combined`].
//!
//! Spectators (a port addition, `--faithful` hides it) get the view of
//! nobody ([`spectator_view`], [`spectator_events`]): no hand at all, every
//! face-down card without identity, both sides' draws as counts and both
//! sides' hand chains as [`SeenEvent::Combined`], whatever the match's Card
//! Display switches say. Under HIDE that is a subset of each seat's own
//! view, so a spectator never knows anything hidden that a player does not.

pub use roses_core::duel::Viewer;
use roses_core::duel::{Duel, DuelEvent, DuelView, Multi, Pending, UnitRecord};
use roses_core::{GridPos, UnitId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Everything one seat may know about the duel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerView {
    pub seat: u8,
    /// The core's per-player view (board, LP, stars, own hand, counts).
    pub board: DuelView,
    pub first: u8,
    /// Per side (two entries in a 2-player duel: the wire form of the
    /// former `[_; 2]` arrays; N in the port's N-player modes).
    pub can_summon: Vec<bool>,
    /// The acting side's open summon cell.
    pub pending_summon: Option<GridPos>,
    /// The acting side's open selection (its lists are public).
    pub pending: Option<Pending>,
    pub graveyards: Vec<Vec<u16>>,
    pub star_income: Vec<u8>,
    /// The rest of the public state, enough for a client to rebuild a
    /// playable copy of the duel (`crate::mirror::mirror_duel`).
    pub public: PublicDetail,
    /// The other side's hand, when its Card Display is OPEN
    /// ([`crate::MatchOptions`]); never sent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_hand: Option<Vec<u16>>,
    /// The view of nobody (a spectator's, [`spectator_view`]): `seat` is
    /// only the side the presentation is drawn from (0), `board.hand` is
    /// empty and both hands are counts only. Absent (false) in a seat's view,
    /// so a seat's messages are unchanged on the wire.
    #[serde(default, skip_serializing_if = "is_false")]
    pub spectator: bool,
    /// N-player duels (a port addition, `roses_core::duel::multi`): the
    /// mode's public state (rules, teams, turn order, knockouts, targets).
    /// Absent from every 2-player view, so those are unchanged on the wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi: Option<Multi>,
    /// 2v2 (a port addition): the team-mate's hand, by side.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub team_hands: Vec<TeamHand>,
}

/// A hand the seat may see besides its own (its 2v2 team-mate's).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamHand {
    pub side: u8,
    pub hand: Vec<u16>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn per_side<T>(duel: &Duel, f: impl Fn(&roses_core::duel::SideState) -> T) -> Vec<T> {
    duel.sides.iter().map(f).collect()
}

/// A seat's 2v2 team-mates' hands (none outside 2v2).
fn team_hands(duel: &Duel, seat: u8) -> Vec<TeamHand> {
    (0..duel.side_count())
        .filter(|&s| duel.team_mates(seat, s))
        .map(|s| TeamHand { side: s, hand: duel.sides[usize::from(s)].hand.clone() })
        .collect()
}

/// Public state of one unit beyond [`roses_core::duel::UnitView`]. For a
/// unit whose card is hidden from the seat (the opponent's face-down
/// cards) everything that could hint at the card (stat changes, the cost
/// discount, the evolution counter, a rewritten record) is left out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicUnit {
    pub id: UnitId,
    pub committed_face_up: bool,
    pub committed_defence: bool,
    pub spellbind: u8,
    pub spellbind_fresh: bool,
    pub summon_order: u8,
    pub stolen: bool,
    /// N-player duels: who took control (absent in a 2-player duel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stolen_by: Option<u8>,
    pub attack_bonus: i16,
    pub defense_bonus: i16,
    pub evo_counter: u8,
    pub discount: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<UnitRecord>,
}

/// Public duel state beyond [`roses_core::duel::DuelView`] (a whitelist:
/// a new engine field is never sent until it is added here).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicDetail {
    /// In the order of `DuelView::units`.
    pub units: Vec<PublicUnit>,
    /// Per side, as [`PlayerView::can_summon`].
    pub summons: Vec<u8>,
    /// Each leader's abilities (the HUD's leader page shows them).
    pub leader_abilities: Vec<Vec<u8>>,
    pub leader_ranks: Vec<u8>,
    pub controllers: Vec<u8>,
    /// Binding Chain binds by unit id (the bind is visible on the board).
    pub chain_bound: BTreeMap<u16, u8>,
    /// The seat's own Destiny Draw byte (the opponent's is not sent).
    pub own_destiny: u8,
}

/// `seat`: the viewer's side (its own Destiny Draw byte is sent); none for
/// a spectator.
fn public_detail(duel: &Duel, seat: Option<u8>, board: &DuelView) -> PublicDetail {
    let units = duel
        .units
        .iter()
        .map(|u| {
            let shown = board
                .units
                .iter()
                .find(|v| v.id == u.id)
                .is_some_and(|v| v.card.is_some());
            PublicUnit {
                id: u.id,
                committed_face_up: u.committed_face_up,
                committed_defence: u.committed_defence,
                spellbind: u.spellbind,
                spellbind_fresh: u.spellbind_fresh,
                summon_order: u.summon_order,
                stolen: u.stolen,
                stolen_by: u.stolen_by,
                attack_bonus: if shown { u.attack_bonus } else { 0 },
                defense_bonus: if shown { u.defense_bonus } else { 0 },
                evo_counter: if shown { u.evo_counter } else { 0 },
                discount: if shown { u.discount } else { 0 },
                record: if shown { duel.records.get(&u.id.0).cloned() } else { None },
            }
        })
        .collect();
    PublicDetail {
        units,
        summons: per_side(duel, |s| s.summons),
        leader_abilities: per_side(duel, |s| s.leader_abilities.clone()),
        leader_ranks: duel.leader_ranks.clone(),
        controllers: duel.controllers.clone(),
        chain_bound: duel.chain_bound.clone(),
        own_destiny: seat.and_then(|s| duel.sides.get(usize::from(s))).map_or(0, |s| s.destiny),
    }
}

/// The seat's view with every Card Display HIDE (nothing of the other
/// side's hidden cards).
pub fn player_view(duel: &Duel, seat: u8) -> PlayerView {
    player_view_with(duel, seat, [false; 2])
}

/// The core's view with the cards of an OPEN side's group shown: the
/// original hides nothing of such a side between two pads (0x1DA6A0 with
/// param 0x1D's bit set needs a computer viewer), so its hand and face-down
/// cards are the other seat's to see.
fn open_view(duel: &Duel, seat: u8, open: [bool; 2]) -> DuelView {
    let mut board = duel.view(seat);
    // N-player duels have no per-side OPEN (a team-mate's cards are in
    // the core's view already).
    if duel.multi().is_some() {
        return board;
    }
    let other = seat ^ 1;
    if open[usize::from(other & 1)] {
        for (v, u) in board.units.iter_mut().zip(&duel.units) {
            if u.side == other {
                v.card = Some(u.card);
            }
        }
    }
    board
}

/// [`player_view`] for a match whose sides' Card Display switches are
/// `open` (bit per side, true = OPEN).
pub fn player_view_with(duel: &Duel, seat: u8, open: [bool; 2]) -> PlayerView {
    let board = open_view(duel, seat, open);
    let other = usize::from((seat ^ 1) & 1);
    let multi = duel.multi().cloned();
    PlayerView {
        open_hand: (multi.is_none() && open[other]).then(|| duel.sides[other].hand.clone()),
        public: public_detail(duel, Some(seat), &board),
        seat,
        board,
        first: duel.first,
        can_summon: per_side(duel, |s| s.can_summon),
        pending_summon: duel.pending_summon,
        pending: duel.pending.clone(),
        graveyards: per_side(duel, |s| s.graveyard.clone()),
        star_income: duel.star_income.clone(),
        spectator: false,
        team_hands: team_hands(duel, seat),
        multi,
    }
}

/// What a spectator may see (the view of nobody): the board with every
/// face-down card hidden whoever owns it, no hand (counts only), no Destiny
/// Draw byte, no open hand even when a side's Card Display is OPEN. The
/// public rest (LP, stars, counts, graveyards, the open selection) is as a
/// seat sees it. `seat` is 0: the side the presentation is drawn from.
pub fn spectator_view(duel: &Duel) -> PlayerView {
    let board = duel.view_as(Viewer::Nobody);
    PlayerView {
        open_hand: None,
        public: public_detail(duel, None, &board),
        seat: 0,
        board,
        first: duel.first,
        can_summon: per_side(duel, |s| s.can_summon),
        pending_summon: duel.pending_summon,
        pending: duel.pending.clone(),
        graveyards: per_side(duel, |s| s.graveyard.clone()),
        star_income: duel.star_income.clone(),
        spectator: true,
        multi: duel.multi().cloned(),
        team_hands: Vec::new(),
    }
}

/// [`player_view_with`] or [`spectator_view`] by viewer (a spectator never
/// gets an OPEN side's cards).
pub fn view_for(duel: &Duel, viewer: Viewer, open: [bool; 2]) -> PlayerView {
    match viewer {
        Viewer::Seat(seat) => player_view_with(duel, seat, open),
        Viewer::Nobody => spectator_view(duel),
    }
}

/// A [`DuelEvent`] as one seat sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "seen", rename_all = "snake_case")]
pub enum SeenEvent {
    /// Nothing in it is hidden from this seat.
    Public { event: DuelEvent },
    /// The opponent drew `count` cards.
    Drew { side: u8, count: usize },
    /// The opponent's refill ended with a Destiny Draw.
    DestinyDraw { side: u8 },
    /// A face-down unit appeared (summoned).
    Summoned {
        unit: UnitId,
        at: GridPos,
        cost: u8,
    },
    /// A face-down unit appeared (created by an effect).
    Created { unit: UnitId, at: GridPos },
    /// The opponent combined cards (fusion or equip) out of sight.
    Combined { side: Option<u8> },
}

/// Filter the events of one change for `viewer`. `post` is the state after
/// the change; `actor` the side whose command caused it (`None` for a
/// server decision).
pub fn seen_events(
    post: &Duel,
    actor: Option<u8>,
    viewer: u8,
    events: &[DuelEvent],
) -> Vec<SeenEvent> {
    seen_events_with(post, actor, viewer, events, [false; 2])
}

/// [`seen_events`] with the sides' Card Display switches: an OPEN side's
/// draws, face-down summons and hand chains are public to the other seat.
pub fn seen_events_with(
    post: &Duel,
    actor: Option<u8>,
    viewer: u8,
    events: &[DuelEvent],
    open: [bool; 2],
) -> Vec<SeenEvent> {
    seen_events_for(post, actor, Viewer::Seat(viewer), events, open)
}

/// The events of one change as a spectator sees them: every side's draws
/// are counts, every face-down summon or creation loses its card, every
/// hand chain is [`SeenEvent::Combined`] (Card Display OPEN is ignored).
pub fn spectator_events(post: &Duel, actor: Option<u8>, events: &[DuelEvent]) -> Vec<SeenEvent> {
    seen_events_for(post, actor, Viewer::Nobody, events, [false; 2])
}

/// [`seen_events_with`] for a seat or for nobody.
pub fn seen_events_for(
    post: &Duel,
    actor: Option<u8>,
    viewer: Viewer,
    events: &[DuelEvent],
    open: [bool; 2],
) -> Vec<SeenEvent> {
    let view = match viewer {
        Viewer::Seat(seat) => open_view(post, seat, open),
        Viewer::Nobody => post.view_as(Viewer::Nobody),
    };
    // N-player duels: hidden unless own or a 2v2 team-mate's (no OPEN).
    let multi = post.multi().is_some();
    let hidden_side = |side: u8| match viewer {
        Viewer::Seat(seat) if multi => side != seat && !post.team_mates(seat, side),
        Viewer::Seat(seat) => side != seat && !open[usize::from(side & 1)],
        Viewer::Nobody => true,
    };
    // Units whose identity this change revealed to everyone.
    let revealed: BTreeSet<UnitId> = events
        .iter()
        .filter_map(|e| match e {
            DuelEvent::Revealed { unit, .. }
            | DuelEvent::Destroyed { unit, .. }
            | DuelEvent::Discarded { unit, .. }
            | DuelEvent::Activated { unit, .. } => Some(*unit),
            _ => None,
        })
        .collect();
    let visible = |unit: UnitId| {
        revealed.contains(&unit)
            || view
                .units
                .iter()
                .find(|u| u.id == unit)
                .is_some_and(|u| u.card.is_some())
    };
    let public = |event: &DuelEvent| SeenEvent::Public {
        event: event.clone(),
    };
    events
        .iter()
        .map(|event| match event {
            DuelEvent::Drew { side, cards } if hidden_side(*side) => SeenEvent::Drew {
                side: *side,
                count: cards.len(),
            },
            DuelEvent::DestinyDraw { side, .. } if hidden_side(*side) => {
                SeenEvent::DestinyDraw { side: *side }
            }
            DuelEvent::Summoned { unit, at, cost, .. } if !visible(*unit) => SeenEvent::Summoned {
                unit: *unit,
                at: *at,
                cost: *cost,
            },
            DuelEvent::Created { unit, at, .. } if !visible(*unit) => SeenEvent::Created {
                unit: *unit,
                at: *at,
            },
            DuelEvent::Fused { .. } if actor.is_none_or(hidden_side) => SeenEvent::Combined { side: actor },
            DuelEvent::Equipped { side, .. } if hidden_side(*side) => {
                SeenEvent::Combined { side: Some(*side) }
            }
            other => public(other),
        })
        .collect()
}

/// Test support: a copy of `duel` in which everything hidden from `viewer`
/// differs (the opponent's hand and face-down card ids, every deck's order
/// and the opponent's deck contents, the random state, the scratch
/// records). `player_view` of the copy must equal that of the original.
#[doc(hidden)]
pub fn scramble_hidden(duel: &Duel, viewer: u8, replacement: u16) -> Duel {
    let mut d = duel.clone();
    let others: Vec<usize> = (0..duel.side_count())
        .filter(|&s| s != viewer && !duel.team_mates(viewer, s))
        .map(usize::from)
        .collect();
    for &other in &others {
        for card in &mut d.sides[other].hand {
            *card = if *card == replacement { replacement ^ 1 } else { replacement };
        }
    }
    for side in 0..d.sides.len() {
        d.sides[side].deck.reverse();
        if d.sides[side].deck.len() > 1 {
            d.sides[side].deck.rotate_left(1);
        }
    }
    for &other in &others {
        for card in &mut d.sides[other].deck {
            *card = replacement;
        }
    }
    let knows = |u: &roses_core::duel::DuelUnit| {
        u.side == viewer
            || u.controller() == viewer
            || duel.team_mates(viewer, u.side)
            || duel.team_mates(viewer, u.controller())
    };
    for u in &mut d.units {
        if !knows(u) && !u.committed_face_up {
            u.card = if u.card == replacement { replacement ^ 1 } else { replacement };
        }
    }
    d.rng ^= 0xDEAD_BEEF_F00D;
    d.temp_units = [None; 11];
    d
}

/// Test support: a copy of `duel` in which everything hidden from a
/// spectator is `replacement` (both hands, every face-down card's id, both
/// decks, both Destiny Draw bytes) or otherwise different (deck order, the
/// random state, the scratch records). `spectator_view` of the copy must
/// equal that of the original.
#[doc(hidden)]
pub fn scramble_hidden_from_all(duel: &Duel, replacement: u16) -> Duel {
    let mut d = duel.clone();
    for side in 0..d.sides.len() {
        for card in &mut d.sides[side].hand {
            *card = replacement;
        }
        for card in &mut d.sides[side].deck {
            *card = replacement;
        }
        d.sides[side].deck.push(replacement);
        d.sides[side].deck.pop();
        d.sides[side].destiny ^= 0x5A;
    }
    for u in &mut d.units {
        if !u.committed_face_up {
            u.card = replacement;
            u.face_up = !u.face_up;
        }
    }
    d.rng ^= 0xDEAD_BEEF_F00D;
    d.temp_units = [None; 11];
    d
}
