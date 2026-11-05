//! Port addition, not in the 2003 game (docs/ADDITIONS.md, MODES): the
//! presentation of the N-player duels (the 4-player free-for-all and 2v2 of
//! `roses_core::duel::multi`), Bevy-free. Only with the `online` feature;
//! `--faithful` never reaches it (Online Play is hidden there).
//!
//! The duel itself is the existing online duel (`duel_online`, `DuelPlay`):
//! the seat's copy of the N-side duel rebuilt from each view, the input
//! layer on it, every intention to the server. What is N-player here:
//!
//! - the board camera: each seat sees the board from its own end (S yaw
//!   0x4000, N 0xC000, E 0x0000, W 0x8000; [`seat_yaw`]), the d-pad turned
//!   to match (`roses_core::duel::ui::board_keys`); on another player's
//!   turn the turn camera moves to that player's end, as the 2-player
//!   online duel does;
//! - the HUD: the original HUD is drawn for a pair of sides ([`hud_pair`]:
//!   the acting player or this seat on the left plate, the acting enemy or
//!   the chosen target on the right) through a 2-side projection of the
//!   duel ([`project_duel`], [`project_input`]); the other players get
//!   compact plates drawn with the same plate pieces and the leaders'
//!   portraits ([`compact_plates`]),
//!   team colour by the panel gradient (this seat's team red, enemies
//!   teal), the acting player's plate lit, knocked-out players OUT;
//! - whose turn and the target ([`turn_lines`]); Target (keyboard T, or L2
//!   while R2 is held) cycles the living enemies ([`next_target`]) and
//!   sends `DuelCommand::Target`;
//! - Card Display: a seat always sees its own and its team-mate's cards
//!   (`DuelPlay::multi_own_sides`), enemies' stay hidden;
//! - knockouts: with "cards leave" the original trap-victim break-up on
//!   every unit of the player ([`KnockoutShow`]), with STAY its units drawn
//!   dim; a "<NAME> OUT" note either way;
//! - the end: a placing list over the board ([`placings`]) instead of the
//!   2-player result screens, then back to the lobby;
//! - [`LocalMulti`]: an offline preview (a test aid, `--multi-preview`)
//!   where the computer players (`MultiComputer`) play every seat but the
//!   local one on this computer.

use roses_core::duel::multi::computer::{CatalogueFusions, MultiComputer};
use roses_core::duel::ui::{DuelUi, Mode};
use roses_core::duel::{Catalogue, Duel, DuelCommand, MultiMode};
use roses_core::GridPos;

/// `DuelPlay::halted` once the placing list was shown and closed (the game
/// returns to Online Play's lobby).
pub const MULTI_DONE: &str = "the match is over";
/// Updates the placing list stays before it closes by itself.
pub const RESULTS_UPDATES: u32 = 25 * 12;
/// Updates the placing list waits before Cross may close it.
pub const RESULTS_MIN_UPDATES: u32 = 25 * 2;
/// A compact plate's portrait (pixels, square): the LP plates' PICMINI
/// leader picture at two thirds of their 48.
pub const PLATE_PORTRAIT: i32 = 32;
/// The seats' letters (S, N, E, W).
pub const SEAT_LETTERS: [&str; 4] = ["S", "N", "E", "W"];

/// The board camera's yaw (16-bit) of a seat's end: S 0x4000 and N 0xC000
/// as the original's two sides; E 0x0000 (the eye at +x, column 6) and W
/// 0x8000 (`duel_board::orbit_eye`: a = 0x4000 − yaw).
pub fn seat_yaw(side: u8) -> i32 {
    match side {
        1 => 0xC000,
        2 => 0x0000,
        3 => 0x8000,
        _ => 0x4000,
    }
}

/// Quarter turns of a board card quad (`duel_overlay::place`, each turn
/// (x, z) -> (-z, x)): the turns that put the card at its side's facing
/// [`model_yaw`] (rotation y), (4 - yaw / 0x4000) mod 4 — side 0's cards 2,
/// side 1's 0 (the original's), E 1 and W 3.
pub fn card_turns(side: u8) -> u8 {
    ((4 - model_yaw(side) / 0x4000) % 4) as u8
}

/// Quarter turns of a leader's emblem: the original's 0 (side 0) and 2
/// (side 1), E 3 and W 1.
pub fn emblem_turns(side: u8) -> u8 {
    (card_turns(side) + 2) % 4
}

/// A side's facing yaw (`duel_models::UnitScene::base_yaw`, the board
/// cards' rotation y and the versus cards' start spin), derived from its
/// camera yaw as the original's two sides are: 0xC000 - [`seat_yaw`] gives
/// side 0 0x8000 and side 1 0 (yaw 0 faces +z), E 0xC000 (facing −x, west)
/// and W 0x4000 (facing +x) — checked on captures of each seat's opening
/// view, where every leader shows its back to its own camera.
pub fn model_yaw(side: u8) -> i32 {
    (0xC000 - seat_yaw(side)) & 0xFFFF
}

/// Where a seat sits on this seat's screen (it looks from its own end).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Near,
    Left,
    Far,
    Right,
}

/// The direction (0 S, 1 W, 2 N, 3 E) of a seat round the table.
fn compass(side: u8) -> u8 {
    match side {
        0 => 0,
        3 => 1,
        1 => 2,
        _ => 3,
    }
}

/// Where `other` sits as `me` sees the board from its end.
pub fn place_of(me: u8, other: u8) -> Place {
    match (compass(other) + 4 - compass(me)) % 4 {
        0 => Place::Near,
        1 => Place::Left,
        2 => Place::Far,
        _ => Place::Right,
    }
}

/// Sides of one team (2v2) or the side itself.
fn mine(duel: &Duel, me: u8, s: u8) -> bool {
    s == me || duel.team_mates(me, s)
}

/// `side`'s single-target enemy: its chosen target while that one plays,
/// else the first living enemy after it in turn order.
pub fn target_of(duel: &Duel, side: u8) -> u8 {
    let Some(m) = duel.multi() else {
        return side ^ 1;
    };
    if let Some(&t) = m.targets.get(usize::from(side)) {
        if duel.alive(t) && duel.foe(side, t) {
            return t;
        }
    }
    let n = m.order.len();
    let at = m.order.iter().position(|&x| x == side).unwrap_or(0);
    (1..n)
        .map(|k| m.order[(at + k) % n])
        .find(|&o| duel.alive(o) && duel.foe(side, o))
        .or_else(|| (0..duel.side_count()).find(|&o| duel.foe(side, o)))
        .unwrap_or(side ^ 1)
}

/// The living enemies of `side` in turn order after it.
pub fn living_enemies(duel: &Duel, side: u8) -> Vec<u8> {
    let Some(m) = duel.multi() else {
        return vec![side ^ 1];
    };
    let n = m.order.len();
    let at = m.order.iter().position(|&x| x == side).unwrap_or(0);
    (1..n).map(|k| m.order[(at + k) % n]).filter(|&o| duel.alive(o) && duel.foe(side, o)).collect()
}

/// The Target key: the living enemy after the current target (none with
/// fewer than two living enemies: nothing to choose).
pub fn next_target(duel: &Duel, side: u8) -> Option<u8> {
    let enemies = living_enemies(duel, side);
    if enemies.len() < 2 {
        return None;
    }
    let current = target_of(duel, side);
    let at = enemies.iter().position(|&e| e == current).map_or(0, |i| i + 1);
    Some(enemies[at % enemies.len()])
}

/// The two sides the original HUD shows: `[left, right]`. Left: the acting
/// player when it is this seat or its team-mate, else this seat. Right:
/// the acting enemy, else the left player's target; while a graveyard list
/// of another side is open, that side.
pub fn hud_pair(duel: &Duel, input: &DuelUi, me: u8) -> [u8; 2] {
    let active = duel.active;
    let left = if mine(duel, me, active) { active } else { me };
    let mut right = if mine(duel, me, active) { target_of(duel, left) } else { active };
    if let Mode::Graveyard(pick) = &input.mode {
        if pick.shown != left && pick.shown < duel.side_count() {
            right = pick.shown;
        }
    }
    if right == left {
        right = target_of(duel, left);
    }
    [left, right]
}

/// The projected side of `s` for the pair: the pair's own sides, the left
/// side's team-mates with it, everyone else with the right side.
pub fn projected_side(duel: &Duel, pair: [u8; 2], s: u8) -> u8 {
    if s == pair[0] {
        0
    } else if s == pair[1] {
        1
    } else if mine(duel, pair[0], s) {
        0
    } else {
        1
    }
}

/// A 2-side copy of the N-player duel for the original HUD: the pair's
/// sides as sides 0 and 1, every other unit with the side it is
/// projected to (after the pair's own units, so a side's first leader is
/// the pair's), no N-player state.
pub fn project_duel(duel: &Duel, pair: [u8; 2]) -> Duel {
    fn two<T: Clone>(v: &[T], a: usize, b: usize) -> Vec<T> {
        [a, b].iter().filter_map(|&i| v.get(i).cloned()).collect()
    }
    let mut d = duel.clone();
    let [a, b] = pair.map(usize::from);
    d.sides = [a, b].map(|i| duel.sides[i.min(duel.sides.len() - 1)].clone()).to_vec();
    d.experience = two(&duel.experience, a, b);
    d.controllers = two(&duel.controllers, a, b);
    d.leader_ranks = two(&duel.leader_ranks, a, b);
    d.star_income = two(&duel.star_income, a, b);
    d.saved_decks = two(&duel.saved_decks, a, b);
    d.deck_stranded = two(&duel.deck_stranded, a, b);
    d.multi = None;
    d.pending = None;
    let mut units = duel.units.clone();
    units.sort_by_key(|u| !(usize::from(u.side) == a || usize::from(u.side) == b));
    for u in &mut units {
        u.side = projected_side(duel, pair, u.side);
        u.stolen_by = u.stolen_by.map(|s| projected_side(duel, pair, s));
    }
    d.units = units;
    d.active = projected_side(duel, pair, duel.active);
    d.first = projected_side(duel, pair, duel.first);
    d
}

/// The input state for [`project_duel`]'s copy: the pair's cursors, an
/// open graveyard list as the right side's.
pub fn project_input(input: &DuelUi, pair: [u8; 2]) -> DuelUi {
    let mut i = input.clone();
    let cursor = |s: u8| input.cursors.get(usize::from(s)).copied().unwrap_or(GridPos { column: 3, row: 3 });
    i.cursors = vec![cursor(pair[0]), cursor(pair[1])];
    if let Mode::Graveyard(pick) = &mut i.mode {
        let take = |v: &Vec<Vec<usize>>, s: u8| v.get(usize::from(s)).cloned().unwrap_or_default();
        let byte = |v: &Vec<u8>, s: u8| v.get(usize::from(s)).copied().unwrap_or(0);
        let lists = vec![take(&pick.lists, pair[0]), take(&pick.lists, pair[1])];
        let row = vec![byte(&pick.row, pair[0]), byte(&pick.row, pair[1])];
        let scroll = vec![byte(&pick.scroll, pair[0]), byte(&pick.scroll, pair[1])];
        pick.shown = u8::from(pick.shown != pair[0]);
        pick.lists = lists;
        pick.row = row;
        pick.scroll = scroll;
    }
    i
}

/// Units on the board of `side` in slots `slots` (the HUD's counts).
pub fn count_units(duel: &Duel, side: u8, slots: std::ops::RangeInclusive<u8>) -> u8 {
    duel.units.iter().filter(|u| u.side == side && slots.contains(&u.slot)).count() as u8
}

/// `name` for a line that holds `letters`: whole if it fits, else its first
/// word (the HUD's plates shorten "PEGASUS CRAWFORD" the same way), else its
/// first letters.
pub fn short_name(name: &str, letters: usize) -> String {
    let name = name.trim();
    if name.chars().count() <= letters {
        return name.to_string();
    }
    match name.split_whitespace().next() {
        Some(first) if first.chars().count() <= letters => first.to_string(),
        _ => name.chars().take(letters).collect(),
    }
}

/// Seat `s`'s name; a seat with no name (none sent, or blank) by its letter.
pub fn seat_name(names: &[String], s: u8) -> String {
    names
        .get(usize::from(s))
        .filter(|n| !n.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| format!("SEAT {}", SEAT_LETTERS[usize::from(s) & 3]))
}

/// The compact plate's text, as offsets from the plate's corner. The panel
/// (type 0) fills from 16 across and 21 down for its 180 x 48; the frame
/// and its ornament are above that, so both lines sit in the fill, beside
/// the portrait.
pub const PLATE_FILL_TOP: i32 = 21;
pub const PLATE_FILL_LEFT: i32 = 16;
pub const PLATE_TEXT_X: i32 = 20 + PLATE_PORTRAIT + 6;
pub const PLATE_NAME_Y: i32 = 25;
pub const PLATE_LP_Y: i32 = 45;
/// The width of a plate's text lines.
pub const PLATE_TEXT_W: i32 = 120;
/// Letters of a plate's name (an account name's 12).
pub const PLATE_NAME_LETTERS: usize = 12;

/// The compact plate's first line: the seat letter and the name.
pub fn plate_title(side: u8, name: &str) -> String {
    format!("{} {}", SEAT_LETTERS[usize::from(side) & 3], short_name(name, PLATE_NAME_LETTERS))
}

/// The plates, their OUT and TARGET labels: only with the HUD on screen.
pub fn plates_shown(hud_shown: bool, exodia: bool) -> bool {
    hud_shown && !exodia
}

/// "<NAME> OUT" over the board: not over a scene that has the screen to
/// itself (the Exodia event, a battle's close-ups); it waits for the board.
pub fn out_note_shown(duel_over: bool, animating: bool, exodia: bool, scene_hides_hud: bool) -> bool {
    (!duel_over || animating) && !exodia && !scene_hides_hud
}

/// One compact plate (a player outside the HUD's pair).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plate {
    pub side: u8,
    pub name: String,
    pub lp: u16,
    pub stars: u8,
    pub hand: u8,
    /// Panel gradient: 0 (this seat's team, red) or 1 (enemies, teal).
    pub colour: i32,
    /// Its turn now.
    pub active: bool,
    /// This seat's chosen target.
    pub target: bool,
    pub out: bool,
    /// Its place on the screen (Left / Right; Far goes to a free one).
    pub place: Place,
}

/// The compact plates of the players outside `pair`, left first.
pub fn compact_plates(duel: &Duel, me: u8, pair: [u8; 2], names: &[String]) -> Vec<Plate> {
    let target = duel.multi().map(|_| target_of(duel, me));
    let mut plates: Vec<Plate> = (0..duel.side_count())
        .filter(|s| !pair.contains(s))
        .map(|s| {
            let side = &duel.sides[usize::from(s)];
            Plate {
                side: s,
                name: seat_name(names, s),
                lp: side.lp,
                stars: side.stars,
                hand: side.hand.len().min(99) as u8,
                colour: if mine(duel, me, s) { 0 } else { 1 },
                active: duel.active == s && duel.outcome.is_none(),
                target: target == Some(s) && s != me,
                out: duel.is_out(s),
                place: place_of(me, s),
            }
        })
        .collect();
    // Left and right as they sit; a far player takes the side left free.
    let taken = |p: &[Plate], place: Place| p.iter().any(|q| q.place == place);
    for i in 0..plates.len() {
        if matches!(plates[i].place, Place::Far | Place::Near) {
            plates[i].place = if !taken(&plates, Place::Left) { Place::Left } else { Place::Right };
        }
    }
    plates.sort_by_key(|p| p.place != Place::Left);
    plates
}

/// Letters a turn note's line holds between the two LP plates.
pub const NOTE_LETTERS: usize = 11;

/// The turn note under the round box: "YOUR TURN", or TURN and the acting
/// player's name ([`short_name`] of [`NOTE_LETTERS`] letters).
pub fn turn_lines(duel: &Duel, me: Option<u8>, names: &[String]) -> Vec<String> {
    if duel.outcome.is_some() {
        return vec!["MATCH OVER".into()];
    }
    match me {
        Some(m) if m == duel.active => vec!["YOUR TURN".into()],
        _ => vec!["TURN".into(), short_name(&seat_name(names, duel.active), NOTE_LETTERS)],
    }
}

/// The final placings, best first: (place, side). The winner (or its
/// team) is 1st; at the round limit the other players still in by LP;
/// knocked-out players by the place they were given.
pub fn placings(duel: &Duel) -> Vec<(u8, u8)> {
    let Some(m) = duel.multi() else {
        return Vec::new();
    };
    let n = duel.side_count();
    let winner = duel.outcome.and_then(|o| o.winner);
    let won = |s: u8| winner.is_some_and(|w| w == s || duel.team_mates(w, s));
    let mut alive: Vec<u8> = (0..n).filter(|&s| !duel.is_out(s)).collect();
    alive.sort_by_key(|&s| (!won(s), std::cmp::Reverse(duel.sides[usize::from(s)].lp)));
    let mut out = Vec::new();
    let mut place = 1u8;
    for (i, &s) in alive.iter().enumerate() {
        // Team-mates and draws share a place.
        let tie = i > 0 && {
            let p = alive[i - 1];
            (won(p) && won(s)) || (winner.is_none() && duel.sides[usize::from(p)].lp == duel.sides[usize::from(s)].lp)
        };
        if !tie {
            place = i as u8 + 1;
        }
        out.push((place, s));
    }
    let mut knocked: Vec<(u8, u8)> = (0..n)
        .filter_map(|s| m.out.get(usize::from(s)).copied().flatten().map(|k| (k.place.max(out.len() as u8 + 1), s)))
        .collect();
    knocked.sort();
    out.extend(knocked);
    // 2v2: the losing team shares the place after the winners.
    if m.rules.mode == MultiMode::Teams && winner.is_some() {
        for p in &mut out {
            p.0 = if won(p.1) { 1 } else { 2 };
        }
        out.sort();
    }
    out
}

/// "1ST", "2ND", "3RD", "4TH".
pub fn place_word(place: u8) -> &'static str {
    match place {
        1 => "1ST",
        2 => "2ND",
        3 => "3RD",
        _ => "4TH",
    }
}

/// The result line for this seat (none for a spectator / a preview).
pub fn result_line(duel: &Duel, me: Option<u8>) -> Option<String> {
    let me = me?;
    let place = placings(duel).into_iter().find(|p| p.1 == me)?.0;
    Some(match (place, duel.outcome.and_then(|o| o.winner)) {
        (1, Some(_)) => "YOU WIN".into(),
        (1, None) => "DRAW".into(),
        (p, _) => format!("YOU PLACED {}", place_word(p)),
    })
}

// ------------------------------------------------------------- knockouts

/// Updates the "<NAME> OUT" note stays over the board after a knockout.
pub const OUT_NOTE_UPDATES: u32 = 25 * 3;
/// The knockout's overview camera move and the move back (updates; the
/// monster effects' overview takes 16).
pub const KNOCKOUT_CAMERA_UPDATES: u32 = 16;

/// One unit of a knocked-out player as it leaves the board.
#[derive(Clone, Debug, PartialEq)]
pub struct KnockoutUnit {
    pub cell: (u8, u8),
    /// The card image the pieces show: (card, stars, ATK, DEF), or the
    /// card back for a face-down card.
    pub face: (u16, u8, i32, i32),
    pub c603: crate::duel_victims::C603,
    pub c545: crate::duel_victims::C545,
}

/// A knockout with "cards leave" (room setting LEAVE): every unit of the
/// player breaks up as a destroyed trap victim does in the original —
/// c603's card pieces (60 updates) and c545's two puffs (15) on each unit,
/// with their sounds (bank 1: 0x40, 0x30) — once the camera has moved to
/// the overview (`CameraStep::Overview`), then the camera comes back.
#[derive(Clone, Debug, PartialEq)]
pub struct KnockoutShow {
    pub side: u8,
    pub units: Vec<KnockoutUnit>,
    /// Updates played (1 = the first).
    pub t: u32,
}

impl KnockoutShow {
    /// `units`: (cell, face up, defence, slot, image) of each unit the
    /// player had on the board before the knockout.
    pub fn new(side: u8, units: &[((u8, u8), bool, bool, u8, (u16, u8, i32, i32))], seed: u32) -> Self {
        let units = units
            .iter()
            .enumerate()
            .map(|(i, &(cell, face_up, defence, slot, image))| {
                let [x, z] = crate::duel_board::cell_center(cell.0, cell.1);
                // The card object's rotation (0x4000, Y, Z) as the victim's
                // [emu FLIP-AND-VICTIMS.md 1.1/1.3]; a leader shows its card.
                let up = face_up || slot == 0;
                let y = crate::duel_play::side_model_base(side) - if defence && slot != 0 { 0x4000 } else { 0 };
                let rot = [0x4000, y, if up { 0x8000 } else { 0 }];
                let s = seed.wrapping_add(i as u32 * 7);
                KnockoutUnit {
                    cell,
                    face: if up { image } else { (roses_client::card_face::BACK_CARD, 0, 0, 0) },
                    c603: crate::duel_victims::C603::new([x, 0.0, z], rot, 0x25B + s),
                    c545: crate::duel_victims::C545::new([x, 0.0, z], 0x221 + s),
                }
            })
            .collect();
        Self { side, units, t: 0 }
    }

    /// The whole presentation's length: the camera out, the pieces, the
    /// camera back.
    pub fn length(&self) -> u32 {
        crate::duel_victims::C603_UPDATES + 2 * KNOCKOUT_CAMERA_UPDATES
    }

    /// The modules' update (1 = their first; 0 while the camera moves out).
    pub fn u(&self) -> u32 {
        self.t.saturating_sub(KNOCKOUT_CAMERA_UPDATES)
    }

    /// One update: the sound ids (bank 1) it starts.
    pub fn tick(&mut self) -> Vec<u8> {
        self.t += 1;
        let u = self.u();
        if u == 0 {
            return Vec::new();
        }
        for k in &mut self.units {
            if u <= crate::duel_victims::C603_UPDATES {
                k.c603.update(u);
            }
            if u <= crate::duel_victims::C545_UPDATES {
                k.c545.update(u);
            }
        }
        if u == 1 && !self.units.is_empty() { vec![0x40, 0x30] } else { Vec::new() }
    }

    /// The camera step this update asks for: the overview at the start,
    /// back to the cursor once the pieces are gone.
    pub fn camera(&self) -> Option<crate::duel_monster_fx::CameraStep> {
        use crate::duel_monster_fx::CameraStep;
        match self.t {
            1 => Some(CameraStep::Overview(KNOCKOUT_CAMERA_UPDATES)),
            t if t == KNOCKOUT_CAMERA_UPDATES + crate::duel_victims::C603_UPDATES => Some(CameraStep::Back(KNOCKOUT_CAMERA_UPDATES)),
            _ => None,
        }
    }

    pub fn done(&self) -> bool {
        self.t >= self.length()
    }

    /// The pieces still flying: (c603, the image).
    pub fn pieces(&self) -> Vec<(&crate::duel_victims::C603, (u16, u8, i32, i32))> {
        if !(1..=crate::duel_victims::C603_UPDATES).contains(&self.u()) {
            return Vec::new();
        }
        self.units.iter().map(|k| (&k.c603, k.face)).collect()
    }

    /// c545's puffs (additive, TEXEFF).
    pub fn smoke_tris(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<crate::duel_c621::FxTri> {
        if !(1..=crate::duel_victims::C545_UPDATES).contains(&self.u()) {
            return Vec::new();
        }
        self.units.iter().flat_map(|k| k.c545.draw(eye, target)).collect()
    }
}

/// The units a knockout takes off the board (LEAVE): every unit of `side`
/// in the duel before the step, leader first, with its image.
pub fn knockout_units(
    before: &Duel,
    side: u8,
    faces: &[(roses_core::UnitId, (u16, u8, i32, i32), bool)],
) -> Vec<((u8, u8), bool, bool, u8, (u16, u8, i32, i32))> {
    let mut units: Vec<_> = before.units.iter().filter(|u| u.side == side).collect();
    units.sort_by_key(|u| u.slot);
    units
        .into_iter()
        .map(|u| {
            let image = faces.iter().find(|f| f.0 == u.id).map_or((u.card, 0, 0, 0), |f| f.1);
            ((u.position.column, u.position.row), u.committed_face_up || u.face_up, u.defence, u.slot, image)
        })
        .collect()
}

/// The note over the board while a knockout shows: "<NAME> OUT" in the
/// gold font's letters (others as spaces), at most 14 of them.
pub fn out_note(name: &str) -> Vec<u8> {
    format!("{} OUT", short_name(name, PLATE_NAME_LETTERS))
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() { b.to_ascii_uppercase() } else { b' ' })
        .collect()
}

// ------------------------------------------------------------ local play

/// The offline N-player preview (a test aid): the computer players play
/// every seat in `computers`, one command per `pace` updates; the local
/// seat (if any) plays through the pad on this computer.
pub struct LocalMulti {
    pub computers: Vec<MultiComputer>,
    pub pace: u32,
    wait: u32,
    stalls: u32,
}

impl LocalMulti {
    pub fn new(duel: &Duel, computer_sides: &[u8], pace: u32) -> Self {
        Self {
            computers: computer_sides.iter().map(|&s| MultiComputer::new(duel, s)).collect(),
            pace,
            wait: pace,
            stalls: 0,
        }
    }

    /// Whether a computer plays `side`.
    pub fn plays(&self, side: u8) -> bool {
        self.computers.iter().any(|c| c.side == side)
    }

    /// The next computer command when its pace allows: (side, command).
    pub fn decide(&mut self, duel: &Duel, cards: &Catalogue) -> Option<(u8, DuelCommand)> {
        if duel.outcome.is_some() {
            return None;
        }
        if self.wait > 0 {
            self.wait -= 1;
            return None;
        }
        let side = duel.active;
        let c = self.computers.iter_mut().find(|c| c.side == side)?;
        let command = c.decide(duel, cards, &CatalogueFusions(cards))?;
        self.wait = self.pace;
        Some((side, command))
    }

    /// The duel refused the computer's command: set it aside; after many in
    /// a row the seat forfeits (`Some`: the duel after it).
    pub fn refused(&mut self, duel: &Duel, side: u8, command: &DuelCommand, cards: &Catalogue) -> Option<(Duel, Vec<roses_core::duel::DuelEvent>)> {
        if let Some(c) = self.computers.iter_mut().find(|c| c.side == side) {
            c.refused(command);
        }
        self.stalls += 1;
        self.wait = 0;
        if self.stalls > 64 {
            self.stalls = 0;
            return duel.forfeit_with(side, cards).ok();
        }
        None
    }

    pub fn accepted(&mut self) {
        self.stalls = 0;
    }
}

/// The N-player state of a `DuelPlay`.
pub struct MultiPlay {
    /// This screen's seat (none: a preview of computer players only, seen
    /// from seat 0's end).
    pub seat: Option<u8>,
    /// Every seat's name.
    pub names: Vec<String>,
    /// Each seat's board camera setting (distance, yaw, pitch).
    pub cameras: Vec<crate::duel_play::SideCamera>,
    /// Each seat's leader card (kept after a knockout takes it away).
    pub leaders: Vec<u16>,
    /// The offline preview's computer players.
    pub local: Option<LocalMulti>,
    /// Updates the placing list has been shown.
    pub results: u32,
    /// The HUD pair of the last update (portraits are set on a change).
    pub pair: Option<[u8; 2]>,
    /// The Target key was pressed (keyboard T; taken by the next update).
    pub target_key: bool,
    /// A knockout presentation playing, and those waiting for the board
    /// to be free (LEAVE).
    pub knockout: Option<KnockoutShow>,
    pub knockouts_waiting: Vec<KnockoutShow>,
    /// The "<NAME> OUT" note: the side and the updates it has left.
    pub out_note: Option<(u8, u32)>,
}

impl MultiPlay {
    pub fn new(duel: &Duel, seat: Option<u8>, names: Vec<String>) -> Self {
        let n = duel.side_count();
        let mut cameras: Vec<crate::duel_play::SideCamera> = Vec::new();
        for s in 0..n {
            let mut c = crate::duel_play::SideCamera::default();
            c.yaw = seat_yaw(s);
            cameras.push(c);
        }
        Self {
            seat,
            names,
            cameras,
            leaders: (0..n).map(|s| duel.units.iter().find(|u| u.side == s && u.slot == 0).map_or(0, |u| u.card)).collect(),
            local: None,
            results: 0,
            pair: None,
            target_key: false,
            knockout: None,
            knockouts_waiting: Vec::new(),
            out_note: None,
        }
    }

    /// The side the HUD is drawn for.
    pub fn viewer(&self) -> u8 {
        self.seat.unwrap_or(0)
    }

    pub fn name(&self, side: u8) -> String {
        seat_name(&self.names, side)
    }

    /// Keep each seat's leader card while it is on the board.
    pub fn note_leaders(&mut self, duel: &Duel) {
        for u in duel.units.iter().filter(|u| u.slot == 0) {
            if let Some(l) = self.leaders.get_mut(usize::from(u.side)) {
                *l = u.card;
            }
        }
    }
}

/// An N-player duel for the preview from the duel import's decks: the
/// player's starter deck and the import's other decks dealt round the
/// seats (`decks`: leader and cards per seat), the import's board.
pub fn preview_duel(
    terrain: Vec<u8>,
    decks: &[(u16, Vec<u16>)],
    mode: MultiMode,
    seed: u64,
    cards: &Catalogue,
) -> Result<Duel, String> {
    use roses_core::duel::{MultiRules, MultiSetup};
    let n = decks.len();
    let mut rng = seed;
    let sources: Vec<&[u16]> = decks.iter().map(|d| d.1.as_slice()).collect();
    let dealt = roses_core::duel::build_decks(&mut rng, &sources);
    let setup = MultiSetup {
        // `ROSES_MULTI_PREVIEW_STAY=1` (a test aid): knocked-out players'
        // cards stay on the board (the room setting STAY).
        rules: MultiRules { mode, cards_leave: std::env::var_os("ROSES_MULTI_PREVIEW_STAY").is_none(), ..MultiRules::default() },
        terrain,
        leaders: decks.iter().map(|d| d.0).collect(),
        decks: dealt,
        // `ROSES_MULTI_PREVIEW_FIRST` (a test aid): the seat that starts.
        first: std::env::var("ROSES_MULTI_PREVIEW_FIRST").ok().and_then(|v| v.parse::<u8>().ok()).filter(|&f| usize::from(f) < n).unwrap_or(0),
        leader_abilities: vec![Vec::new(); n],
        controllers: vec![roses_core::duel::PAD_CONTROLLER; n],
    };
    let (mut duel, _) = Duel::new_multi(setup, cards).map_err(|e| format!("the N-player preview duel: {e:?}"))?;
    duel.rng = seed;
    Ok(duel)
}

/// The preview's seats from the duel import: the player's starter deck,
/// then computer opponents' decks (Seto, Weevil, Rex Raptor ...), named.
pub fn preview_seats(data: &crate::duel_play::DuelData, n: usize) -> (Vec<String>, Vec<(u16, Vec<u16>)>) {
    let starter = &data.first_duel.sides[0];
    let mut names = vec!["PLAYER".to_string()];
    let mut decks = vec![(starter.leader, starter.cards.clone())];
    let opponents = roses_net::deck::opponent_decks(&data.executable, Some(&data.messages)).unwrap_or_default();
    for o in opponents.into_iter().filter(|o| o.deck.cards.len() == 40).take(n.saturating_sub(1)) {
        names.push(o.name.to_ascii_uppercase());
        decks.push((o.deck.leader, o.deck.cards));
    }
    while decks.len() < n {
        let s = &data.first_duel.sides[decks.len() & 1];
        names.push(format!("SEAT {}", SEAT_LETTERS[decks.len() & 3]));
        decks.push((s.leader, s.cards.clone()));
    }
    (names, decks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seat_yaws_put_each_seat_at_its_own_end() {
        use crate::duel_board::{cell_center, orbit_eye};
        // The eye stands behind the seat's leader cell, looking across.
        for (side, cell) in roses_core::duel::MULTI_LEADER_CELLS.iter().enumerate() {
            let eye = orbit_eye([0.0, 40.0, 0.0], 200.0, seat_yaw(side as u8) as u16, 0x2000);
            let [x, z] = cell_center(cell.column, cell.row);
            let dot = eye[0] * x + eye[2] * z;
            assert!(dot > 0.0, "side {side}: eye {eye:?} leader at ({x}, {z})");
            // And straight behind it (the other axis is 0).
            let across = if x == 0.0 { eye[0] } else { eye[2] };
            assert!(across.abs() < 1e-6, "side {side}: {eye:?}");
        }
        // The original's two sides are unchanged.
        assert_eq!((seat_yaw(0), seat_yaw(1)), (0x4000, 0xC000));
    }

    #[test]
    fn the_d_pad_turn_matches_the_seat_camera() {
        use roses_core::duel::ai::pad;
        // The screen's right-hand direction for a camera at the seat's end
        // (y up, looking at the board's centre) is where the turned d-pad's
        // Right moves the cursor (ui.rs `board_keys`): E's Right is Up (row
        // − 1), W's Right is Down (row + 1).
        for (side, right) in [(0u8, (1i32, 0i32)), (1, (-1, 0)), (2, (0, -1)), (3, (0, 1))] {
            let eye = crate::duel_board::orbit_eye([0.0, 0.0, 0.0], 200.0, seat_yaw(side) as u16, 0x2000);
            let forward = [-eye[0], -eye[2]];
            // right = forward × up = (−f.z, f.x) in (x, z).
            let r = [-forward[1], forward[0]];
            let got = (r[0].signum() as i32 * i32::from(r[0].abs() > 1e-6), r[1].signum() as i32 * i32::from(r[1].abs() > 1e-6));
            assert_eq!(got, right, "side {side}");
        }
        let _ = pad::RIGHT;
    }

    #[test]
    fn places_round_the_table_follow_the_viewer() {
        assert_eq!(place_of(0, 3), Place::Left);
        assert_eq!(place_of(0, 2), Place::Right);
        assert_eq!(place_of(0, 1), Place::Far);
        assert_eq!(place_of(1, 2), Place::Left);
        assert_eq!(place_of(2, 0), Place::Left);
        assert_eq!(place_of(3, 1), Place::Left);
        assert_eq!(place_of(3, 3), Place::Near);
    }

    #[test]
    fn card_and_emblem_turns_keep_the_original_two_sides() {
        assert_eq!((card_turns(0), card_turns(1)), (2, 0));
        assert_eq!((emblem_turns(0), emblem_turns(1)), (0, 2));
        assert_eq!((model_yaw(0), model_yaw(1)), (0x8000, 0));
        // Each further seat a quarter turn from its neighbours.
        for s in 2..4u8 {
            assert_eq!(card_turns(s) % 2, 1);
            assert_eq!(model_yaw(s) % 0x8000, 0x4000);
        }
        assert_eq!((model_yaw(2), model_yaw(3)), (0xC000, 0x4000));
        assert_eq!((card_turns(2), card_turns(3)), (1, 3));
        assert_eq!((emblem_turns(2), emblem_turns(3)), (3, 1));
    }

    /// The battle presentation's angles for every seat come from the seat
    /// yaw as the original's sides 0 and 1 do: the versus cards' start spin
    /// and the summon landing pose are the side's facing (0xC000 − camera
    /// yaw), and their card quad is the board quad (`card_turns`).
    #[test]
    fn battle_card_angles_follow_the_seat_yaw() {
        use crate::duel_versus::board_card_pose;
        // The original's two sides, unchanged.
        assert_eq!(board_card_pose(0, (3, 6), false).spin, [0x4000, 0x8000, 0x8000]);
        assert_eq!(board_card_pose(1, (3, 0), false).spin, [0x4000, 0, 0x8000]);
        assert_eq!(board_card_pose(0, (3, 6), true).spin, [0x4000, 0x4000, 0x8000]);
        assert_eq!(board_card_pose(1, (3, 0), true).spin[1], 0xC000);
        for side in 0..4u8 {
            let y = board_card_pose(side, (3, 3), false).spin[1];
            assert_eq!(y, (0xC000 - seat_yaw(side)) & 0xFFFF, "side {side}");
            assert_eq!(crate::duel_summon::landing_pose(side, (3, 3), false, false).spin[1], y);
            // The quad's long axis: c603's card plane rotated by y keeps the
            // card's local +y on the board quad's local +z turned
            // `card_turns` times ((x, z) -> (-z, x)).
            let r = f64::from(y) * std::f64::consts::TAU / 65536.0;
            let (wx, wz) = (r.sin(), r.cos());
            let (mut lx, mut lz) = (0.0f64, 1.0f64);
            for _ in 0..card_turns(side) {
                (lx, lz) = (-lz, lx);
            }
            assert!((wx - lx).abs() < 1e-9 && (wz - lz).abs() < 1e-9, "side {side}: ({wx}, {wz}) vs ({lx}, {lz})");
        }
    }

    #[test]
    fn place_words() {
        assert_eq!([1, 2, 3, 4].map(place_word), ["1ST", "2ND", "3RD", "4TH"]);
    }

    fn synthetic_preview(mode: MultiMode, seat: Option<u8>) -> crate::duel_play::DuelPlay {
        let cards = roses_net::synthetic::catalogue();
        let data = crate::duel_online::tests::bare_data(cards.clone(), roses_net::synthetic::board());
        let decks: Vec<(u16, Vec<u16>)> = (0..4).map(|i| {
            let d = &roses_net::synthetic::submissions()[i % 2];
            (d.leader, d.cards.clone())
        }).collect();
        let duel = preview_duel(roses_net::synthetic::board(), &decks, mode, 3, &cards).unwrap();
        let names = (0..4).map(|s| format!("P{s}")).collect();
        crate::duel_play::DuelPlay::multi_preview(data, duel, seat, names, 0).unwrap()
    }

    #[test]
    fn the_target_key_cycles_the_living_enemies_on_the_own_turn() {
        let mut play = synthetic_preview(MultiMode::FreeForAll, Some(0));
        // Seat 0 moves first (setup.first = 0): its own turn.
        assert_eq!(play.session.duel.active, 0);
        let first = target_of(&play.session.duel, 0);
        let mut seen = vec![first];
        for _ in 0..3 {
            play.multi.as_mut().unwrap().target_key = true;
            play.press(0);
            seen.push(target_of(&play.session.duel, 0));
        }
        // Three enemies in turn order (E, N, W), back to the first.
        assert_eq!(seen, [2, 1, 3, 2]);
        assert_eq!(living_enemies(&play.session.duel, 0), vec![2, 1, 3]);
        // In 2v2 the team-mate is never a target.
        let play = synthetic_preview(MultiMode::Teams, Some(0));
        assert_eq!(living_enemies(&play.session.duel, 0), vec![2, 3]);
        assert_eq!(next_target(&play.session.duel, 0), Some(3));
    }

    #[test]
    fn the_hud_pair_and_projection_keep_the_viewer_left_and_the_acting_enemy_right() {
        let play = synthetic_preview(MultiMode::FreeForAll, Some(0));
        let duel = &play.session.duel;
        let input = &play.session.input;
        assert_eq!(hud_pair(duel, input, 0), [0, 2], "own turn: the target on the right");
        // Seat 2 (E) watching seat 0's turn: itself left, the acting enemy right.
        assert_eq!(hud_pair(duel, input, 2), [2, 0]);
        let p = project_duel(duel, [2, 0]);
        assert_eq!((p.sides.len(), p.multi.is_none(), p.active), (2, true, 1));
        assert_eq!(p.sides[0].lp, duel.sides[2].lp);
        // Every unit is on side 0 or 1; the pair's leaders come first.
        assert!(p.units.iter().all(|u| u.side < 2));
        let leader = |s: u8| p.units.iter().find(|u| u.side == s && u.slot == 0).unwrap().position;
        assert_eq!(leader(0), duel.leader_cell(2).unwrap());
        assert_eq!(leader(1), duel.leader_cell(0).unwrap());
        let i = project_input(input, [2, 0]);
        assert_eq!(i.cursors, vec![input.cursors[2], input.cursors[0]]);
        // The compact plates: the two other players, W on seat 2's left?
        let plates = compact_plates(duel, 2, [2, 0], &play.multi.as_ref().unwrap().names);
        assert_eq!(plates.iter().map(|p| p.side).collect::<Vec<_>>().len(), 2);
        assert!(plates.iter().all(|p| p.colour == 1 && !p.out));
        // Turn lines.
        assert_eq!(turn_lines(duel, Some(0), &["A".into(), "B".into(), "C".into(), "D".into()]), vec!["YOUR TURN".to_string()]);
        assert_eq!(turn_lines(duel, Some(2), &["ALEXANDRAWOOD".into()]), vec!["TURN".to_string(), "ALEXANDRAWO".to_string()]);
    }

    /// A player sees its own cards and its 2v2 team-mate's (Card Display
    /// OPEN for those sides), never an enemy's (HIDE, R2 shows nothing);
    /// the HUD's projected pair keeps the viewer's side OPEN only.
    #[test]
    fn own_and_team_mate_cards_are_shown_and_enemies_hidden() {
        use roses_core::duel::queries::{Query, Record};
        for (mode, mate) in [(MultiMode::FreeForAll, None), (MultiMode::Teams, Some(1u8))] {
            let mut play = synthetic_preview(mode, Some(0));
            // R2 held changes nothing.
            for held in [0, roses_core::duel::ai::pad::R2] {
                play.held_pads[0] = held;
                let ctx = play.hud_context();
                assert_eq!(ctx.option_29, 1 | mate.map_or(0, |m| 1 << m), "{mode:?}");
                assert_eq!(ctx.r2_held, [false; 2]);
                let duel = &play.session.duel;
                let q = Query::new(duel, &play.data.catalogue, &ctx);
                for s in 0..4u8 {
                    let card = duel.sides[usize::from(s)].hand.first().copied().unwrap_or(1);
                    let hidden = q.hidden(Record::Hand { side: s, slot: 1, card }, 0);
                    assert_eq!(hidden, !(s == 0 || Some(s) == mate), "{mode:?} side {s} R2 {held}");
                }
            }
        }
        // The preview of computer players only shows its viewer's (seat 0's).
        let play = synthetic_preview(MultiMode::FreeForAll, None);
        assert_eq!(play.multi_own_sides(), Some(1));
    }

    #[test]
    fn a_knockout_breaks_up_every_unit_then_brings_the_camera_back() {
        use crate::duel_monster_fx::CameraStep;
        let units = [((3, 6), true, false, 0, (5, 0, 0, 0)), ((2, 5), false, true, 1, (7, 3, 1000, 900)), ((4, 5), true, false, 2, (9, 4, 1200, 800))];
        let mut show = KnockoutShow::new(2, &units, 1);
        assert_eq!(show.units.len(), 3);
        // The leader shows its card, a face-down card the back.
        assert_eq!(show.units[0].face, (5, 0, 0, 0));
        assert_eq!(show.units[1].face.0, roses_client::card_face::BACK_CARD);
        assert_eq!(show.units[2].face, (9, 4, 1200, 800));
        // E's cards: rotation y 0xC000 (0x8000 in defence).
        assert_eq!(show.units[2].c603.rot, [0x4000, 0xC000, 0x8000]);
        assert_eq!(show.units[1].c603.rot, [0x4000, 0x8000, 0]);
        // The camera moves out first, then the pieces and their sounds.
        assert!(show.tick().is_empty());
        assert_eq!(show.camera(), Some(CameraStep::Overview(KNOCKOUT_CAMERA_UPDATES)));
        assert!(show.pieces().is_empty());
        for _ in 1..KNOCKOUT_CAMERA_UPDATES {
            assert!(show.tick().is_empty());
        }
        assert_eq!(show.tick(), vec![0x40, 0x30]);
        assert_eq!(show.pieces().len(), 3);
        let mut backs = 0;
        while !show.done() {
            assert!(show.tick().is_empty());
            backs += usize::from(matches!(show.camera(), Some(CameraStep::Back(_))));
        }
        assert_eq!((backs, show.t, show.pieces().len()), (1, crate::duel_victims::C603_UPDATES + 2 * KNOCKOUT_CAMERA_UPDATES, 0));
        assert_eq!(out_note("Rex Raptor"), b"REX RAPTOR OUT".to_vec());
    }

    /// Long names keep a whole word on every line that cuts them, and a
    /// seat without a name shows its letter; a 12-letter account name stays
    /// whole wherever the room allows.
    #[test]
    fn long_and_missing_names_read_properly_on_every_line() {
        assert_eq!(short_name("Pegasus Crawford", 11), "Pegasus");
        assert_eq!(short_name("Yugi", 11), "Yugi");
        assert_eq!(short_name("Christopher1", 11), "Christopher");
        assert_eq!(short_name("  Rex Raptor  ", 12), "Rex Raptor");
        assert_eq!(out_note("Pegasus Crawford"), b"PEGASUS OUT".to_vec());
        assert_eq!(out_note("Christopher1"), b"CHRISTOPHER1 OUT".to_vec());
        assert_eq!(plate_title(2, "Pegasus Crawford"), "E Pegasus");
        assert_eq!(plate_title(1, "Christopher1"), "N Christopher1");
        let names = vec!["Yugi".to_string(), "".to_string(), "   ".to_string()];
        assert_eq!(seat_name(&names, 0), "Yugi");
        assert_eq!(seat_name(&names, 1), "SEAT N");
        assert_eq!(seat_name(&names, 2), "SEAT E");
        assert_eq!(seat_name(&names, 3), "SEAT W", "no name sent at all");
    }

    /// The plates' text sits in the panel's fill, not on its frame: the
    /// name on the top ornament is what the 30 Oct takes showed.
    #[test]
    fn compact_plate_text_stays_off_the_frame_and_beside_the_portrait() {
        assert!(PLATE_NAME_Y >= PLATE_FILL_TOP, "the name is below the top ornament");
        assert!(PLATE_LP_Y >= PLATE_NAME_Y + 18, "two lines of the message font");
        assert!(PLATE_LP_Y + 18 <= PLATE_FILL_TOP + 48, "both in the fill");
        assert!(PLATE_TEXT_X >= 20 + PLATE_PORTRAIT, "beside the portrait");
        assert!(PLATE_TEXT_X + PLATE_TEXT_W <= PLATE_FILL_LEFT + 180, "inside the fill");
    }

    /// What stays off a scene that has the screen to itself.
    #[test]
    fn plates_and_the_out_note_wait_for_the_exodia_event_and_battle_scenes() {
        assert!(plates_shown(true, false));
        assert!(!plates_shown(false, false), "the HUD is away");
        assert!(!plates_shown(true, true), "the Exodia event, while the HUD slides out too");
        assert!(out_note_shown(false, false, false, false));
        assert!(out_note_shown(true, true, false, false), "the last knockout's pieces");
        assert!(!out_note_shown(true, false, false, false), "the placing list has it");
        assert!(!out_note_shown(true, true, true, false), "the Exodia event");
        assert!(!out_note_shown(false, true, false, true), "a battle's close-ups");
    }

    /// A knockout's presentation follows the room setting: LEAVE queues the
    /// pieces of every unit the player had (and the OUT note), STAY keeps
    /// the units on the board drawn dim (acted, brightness 64).
    #[test]
    fn a_knockout_leaves_or_stays_as_the_room_says() {
        for leave in [true, false] {
            let mut play = synthetic_preview(MultiMode::FreeForAll, Some(0));
            play.session.duel.multi.as_mut().unwrap().rules.cards_leave = leave;
            let before = play.session.duel.clone();
            let units = before.units.iter().filter(|u| u.side == 2).count();
            let (next, events) = before.forfeit_with(2, &play.data.catalogue).unwrap();
            play.session.duel = next;
            play.queue_knockouts(&events, &before, &[]);
            let m = play.multi.as_ref().unwrap();
            assert_eq!(m.out_note.map(|n| n.0), Some(2));
            let dim: Vec<bool> = crate::duel_play::overlay_input(&play.session).units.iter().filter(|u| u.side == 2).map(|u| u.acted).collect();
            if leave {
                assert_eq!(m.knockouts_waiting.len(), 1);
                assert_eq!(m.knockouts_waiting[0].units.len(), units);
                assert!(dim.is_empty());
                assert!(play.animating());
            } else {
                assert!(m.knockouts_waiting.is_empty());
                assert_eq!(dim, vec![true; units]);
            }
            // The other players' units are drawn as before.
            let shown = crate::duel_play::overlay_input(&play.session);
            for (o, u) in shown.units.iter().zip(&play.session.duel.units).filter(|(o, _)| o.side != 2) {
                assert_eq!(o.acted, u.acted);
            }
        }
    }

    #[test]
    fn four_computers_play_a_synthetic_preview_to_its_placing_list() {
        let mut play = synthetic_preview(MultiMode::Teams, None);
        let (mut shows, mut frozen) = (0, true);
        for _ in 0..200_000 {
            let playing = play.multi.as_ref().unwrap().knockout.is_some();
            let before = playing.then(|| play.session.duel.clone());
            play.press(0);
            assert_eq!(play.halted, None);
            let m = play.multi.as_ref().unwrap();
            shows += usize::from(m.knockout.as_ref().is_some_and(|k| k.t == 1));
            // Nothing moves on the board while a knockout plays.
            if let Some(b) = before {
                frozen &= b == play.session.duel;
            }
            if m.results > 3 {
                break;
            }
        }
        let knocked = play.session.duel.multi().map_or(0, |m| m.out.iter().flatten().count());
        assert!(frozen);
        // "Cards leave" (the default): one presentation per knockout that
        // left units on the board.
        assert!(shows <= knocked && (knocked == 0 || shows > 0), "{shows} shows for {knocked} knockouts");
        let duel = &play.session.duel;
        assert!(duel.outcome.is_some());
        let list = placings(duel);
        assert_eq!(list.len(), 4);
        // 2v2: a team shares its place.
        let winners: Vec<u8> = list.iter().filter(|p| p.0 == 1).map(|p| p.1).collect();
        if duel.outcome.unwrap().winner.is_some() {
            assert_eq!(winners.len(), 2);
            assert!(duel.team_mates(winners[0], winners[1]));
        }
    }

    /// The offline preview with the user's own import: four computer
    /// players (FFA, then 2v2) through `DuelPlay` with the original HUD and
    /// the N-player plates drawn, the board camera driven; every update
    /// presents without a panic and the match ends with a placing list.
    #[test]
    #[ignore = "Requires a private `roses import-duel` folder (ROSES_DUEL_IMPORT)"]
    fn private_four_computer_players_play_through_the_presentation() {
        use crate::duel_play::{drive_camera, DuelHud, DuelPlay};
        let Some(dir) = std::env::var_os("ROSES_DUEL_IMPORT").map(std::path::PathBuf::from) else { return };
        for (mode, seed) in [(MultiMode::FreeForAll, 7u64), (MultiMode::Teams, 11)] {
            let data = crate::duel_play::load(&dir).unwrap();
            let (names, decks) = preview_seats(&data, 4);
            let duel = preview_duel(data.first_duel.terrain.clone(), &decks, mode, seed, &data.catalogue).unwrap();
            let mut play = DuelPlay::multi_preview(data, duel, None, names, 1).unwrap();
            let mut hud = DuelHud::new(&play).unwrap();
            let mut camera = crate::duel_board::BoardCamera::snapped((3, 6));
            let (mut knockouts, mut turns_seen) = (0, std::collections::BTreeSet::new());
            let mut updates = 0u64;
            while updates < 400_000 {
                updates += 1;
                play.press(0);
                turns_seen.insert(play.session.duel.active);
                let cursor = play.cursor();
                drive_camera(&mut camera, &mut play, cursor, &|_| None);
                camera.tick();
                hud.update(&play);
                if updates % 40 == 0 {
                    hud.render();
                }
                assert_eq!(play.halted, None, "{mode:?}");
                if play.multi.as_ref().is_some_and(|m| m.results > 5) {
                    break;
                }
            }
            let d = &play.session.duel;
            knockouts += d.multi().map_or(0, |m| m.out.iter().flatten().count());
            eprintln!("{mode:?}: {updates} updates, round {}, outcome {:?}, {knockouts} knocked out, turns of {turns_seen:?}, placings {:?}", d.round, d.outcome, placings(d));
            assert_eq!(turns_seen.len(), 4, "{mode:?}");
            assert!(d.outcome.is_some(), "{mode:?}: the match did not end in {updates} updates");
            assert_eq!(placings(d).len(), 4);
            hud.render();
            assert!(hud.layer.iter().any(|&b| b != 0));
        }
    }
}

#[cfg(test)]
#[path = "duel_multi_online_tests.rs"]
mod online_tests;
