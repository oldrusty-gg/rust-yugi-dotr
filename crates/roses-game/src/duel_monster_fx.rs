//! Monster flip effects as the original presents them
//! (the project notes 2) [F handlers]: the
//! revealed card turns over (8 updates) and its model grows in (16), then
//! the effect's handler runs on the board before any battle scene: the
//! board dims (0x80 -> 0x30 by 15 per update), the units that are not
//! targets fade (-8 per update), the camera goes to the target (or the
//! overview), the effect's module plays on every target, the rule change
//! shows, then the light, the units and the camera come back and 17
//! updates pass.
//!
//! | effect | card | box | module | camera | total after E |
//! |---|---|---|---|---|---|
//! | Boost | Hoshiningen (Trumpeter) | 0x107 | c529 30 U | 16 U / overview 16 | 61 |
//! | Defence | Key Mace | none | c513 16 U | cut / overview 16 | 80 |
//! | Destroy | Man-Eater Bug | none | c031 44 U, then c603 on the victim | overview 16, cut | 144 |
//! | Bind | Nightmare Scorpion | 0x109 | c500 30 U | 32 U / overview 32 | 144 |
//! | Countdown | Larva / Pupa of Moth (event 0x43, 0x26F5F0) | 0x114 | c513 16 U | 30 U | 66, no dim |
//!
//! | Evolve | Larva / Pupa of Moth at turn start (event 0x3C, 0x26ABF0) | none | c560 90 U | 30 U | 144 |
//!
//! After a magic card's c601 (NEWLY-ALLOWED.md 4, 6, 8.1; e = E + 1):
//!
//! | effect | card | box | modules | camera | length |
//! |---|---|---|---|---|---|
//! | Duster | Harpie's Feather Duster (event 7) | none | c507 e 2, c603 p1 e 19 | overview 30, back e 79 | 97 |
//! | FakeTrap | Fake Trap reacting | name e 9..38 | c602 e 9, c603 e 40 | cut (0,0,0) 180, back e 101 | 119 |
//! | Raigeki | Raigeki (event 5) | none | c505 e 2 + 12k per ring cell | cell 200, overview 30 e 19, back e 38+12K | 62+12K |
//! | AntiRaigeki | Anti-Raigeki reacting | 0x104 e 9..38, name e 40..94 | c602 e 9, c505 x10 e 40+6i | cell 180, 500 over 30 e 39, back e 95 | 113 |
//! | Lawbook | Solomon's Lawbook (event 0x21) | none | c533 e 2 (the board light) | none | 95 |
//! | DarkHole | Dark Hole (event 4, 0x258C60) | a reacting White Hole's / Royal Decree's name e 1..38 | c602 e 9 per reaction; c021 e c+1 (c = 1, or 39 after a reaction) | cut to the reacting trap e 2; overview 20 e c+1; 90-degree orbits (60 U) e c+23+60j; back e c+L+1 | c+L+18 (L = c021's length) |
//!
//! The moth's countdown and evolution: the project notes
//! 1-2 [F]; the evolution swaps the old card for the new face-down one at
//! E+105 under c560's flash (`C560-EVOLUTION.md`).
use crate::duel_overlay::OverlayInput;

/// The reveal before the handler: the 8-update flip and the 16-update
/// model grow-in the battle coroutine waits for [F 0x1D2470].
pub const REVEAL_UPDATES: u32 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Boost,
    Defence,
    Destroy,
    Bind,
    /// Larva / Pupa of Moth: the evolution countdown starts (the moth is
    /// its own target).
    Countdown,
    /// The moth evolves at its owner's turn start into the next form.
    Evolve,
    /// Event 9 damage after a magic card's c601 (Tremendous Fire and the
    /// other burn cards): handler 0x25ADF0's plain path with c509
    /// [F PEGASUS-MAGIC.md 2.2, C509-DAMAGE.md]. Target 0 is the victim's
    /// leader; `burn` holds the card's cell and the amount.
    Burn,
    /// Magical Neutralizing Force Field (event 0x1C, 0x25F2F0): overview,
    /// c022, unbind and reset at E+75, c603 param 1 on every face-up trap
    /// (`targets`) [F C022-FORCE-FIELD.md, PEGASUS-MAGIC.md 2.4].
    Field,
    /// Change of Heart after its selection (event 0x1A, 0x25E300): overview,
    /// c026, cut to the target with c027, control at E+173
    /// [F C026-C027-CHANGE-OF-HEART.md].
    Heart,
    /// A ritual (event 0x38, 0x26D4B0): sacrifices (`targets`) revealed,
    /// overview, box 0x106 and c059, the result created at its u 135
    /// [F C059-RITUAL.md 8].
    Ritual,
    /// Mimicat / Monster Reborn after the graveyard pick (event 1 bit 17,
    /// 0x2586A0): a 16-update move to the empty cell, c501 at E+2, the
    /// revived unit (target 0, face down) from E+34 [F MIMICAT.md].
    Revive,
    /// Patrol Robo / Water Element (event 0xD mode 2/4, 0x25B980): Key
    /// Mace's timeline (c513 per target), the targets turned face up
    /// (Reveal) or face down (Conceal) at E+27 [F MORE-EFFECTS.md].
    Reveal,
    Conceal,
    /// Jirai Gumo (event 0xFF, 0x261B20): box 0x105 and c513 on the card,
    /// 18 updates, no camera or light [F MORE-EFFECTS.md].
    Notice,
    /// Shift (magic type 0x25, event 0x1F, 0x25FB20): c531 on the unit at
    /// E+8, the move at E+25, a 30-update camera move to the new cell, c532
    /// at E+50 [F MORE-EFFECTS.md 2.3]; target 0 at its old cell,
    /// `shift_to` the new.
    Shift,
    /// The crush at turn start (event 0x70, 0x274D90): overview 30, box
    /// 0x115 "Virus infection!", c612 on every victim at E+53, removal at
    /// E+118 [F MORE-EFFECTS.md 2.2].
    Crush,
    /// Blue-Eyed Silver Zombie / Tao (event 0x42, 0x26F220): c513 on every
    /// retyped unit at E+30 with "Changed Type!" (0x111) or "Changed
    /// Attribute!" (0x112), the record rewritten at E+34, 70 updates
    /// [F NEWLY-ALLOWED.md].
    Retype,
    /// Harpie's Feather Duster (event 7, 0x25A080) after c601: the cross
    /// cells tinted, overview 30, c507 on every target at E+1, c603 param 1
    /// on each at its c507's 2 (E+18), removal at E+78 [F NEWLY-ALLOWED.md
    /// 4.1]. `card_cell` = the Duster's cell.
    Duster,
    /// Fake Trap reacting to the Duster (event 7's 0x20 stages): the cut to
    /// (0, 0, 0) (the original's swapped argument), its reveal, its name
    /// with c602 at E+8, c603 at E+39, removed at E+100 [F 4.2]; target 0 is
    /// the Fake Trap.
    FakeTrap,
    /// Raigeki (event 5, 0x2593C0) after c601: c505 on the card's cell at
    /// E+1, then on the ring-k cells every 12 U, the struck monsters fade,
    /// all removed at E+37+12K [F 6.1].
    Raigeki,
    /// Anti-Raigeki reacting (event 5's 0x80 stages): reveal, c602 with
    /// "Trap activated!", a 30-U move to it, ten c505 on its own cell every
    /// 6 U; nothing destroyed [F 6.2]; target 0 is the Anti-Raigeki.
    AntiRaigeki,
    /// Solomon's Lawbook (event 0x21, 0x260760) after c601: c533 at E+1
    /// (screen anchored, it drives the board light), the extra summon at
    /// E+94 [F 8.1].
    Lawbook,
    /// Dark Hole (event 4, 0x258C60) after c601: a White Hole / Royal Decree
    /// reacting first (its name box from E, its 8-update reveal, the cut to
    /// it at E+1, c602 at E+8, the box closed at E+38), then c021 (the
    /// vortex: it fades every other card, shrinks the models, carries the
    /// targets' cards off and dims the board) launched at E+c (c = 1, or 39
    /// after a reaction) with the overview(20) camera, 90-degree orbits
    /// every 60 U from E+c+22 and SE 0x49 every 28 U; when c021 ends (seen
    /// at E+c+L) every target is removed and the camera goes back; return
    /// at E+c+L+17 [F/emu DARK-HOLE-BLOCKER-GRAVEROBBER.md].
    DarkHole,
}

/// A target unit: (side, slot), cell, card, face up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub unit: (u8, u8),
    pub cell: (u8, u8),
    pub card: u16,
    pub face_up: bool,
    pub defence: bool,
    pub face: Option<crate::duel_overlay::BoardFace>,
}

/// What the presentation asks of the board this update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraStep {
    /// 0x262410(frames, target, 100, 180).
    Unit(Target, u32),
    /// 0x262060(frames): target 0, polar (500, yaw, 0x17FF).
    Overview(u32),
    /// 0x262390(ctx, frames): back to the cursor camera.
    Back(u32),
    /// 0x262DE0(frames, cell, distance): the (empty) cell at a distance.
    Cell(Target, u32, u32),
    /// 0x2622B0(frames, dyaw): the polar yaw turns by dyaw (16-bit units)
    /// over `frames` around the current target (tracks cleared first).
    Orbit(u32, i32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MonsterShow {
    pub effect: Effect,
    /// The board before the step (the revealed unit face down).
    pub frozen: OverlayInput,
    /// The duel before the step (the HUD's values until the end).
    pub duel: Option<roses_core::duel::Duel>,
    /// The revealed unit (side, slot) and its card.
    pub revealed: Option<(u8, u8)>,
    pub targets: Vec<Target>,
    /// Updates played (1 = the reveal's first).
    pub t: u32,
    /// The modules on the targets (`duel_monster_mods`) and the Bug
    /// victim's c603.
    pub c529: Vec<crate::duel_monster_mods::C529>,
    pub c513: Vec<crate::duel_monster_mods::C513>,
    pub c031: Vec<crate::duel_monster_mods::C031>,
    pub c500: Vec<crate::duel_c500::C500>,
    pub c603: Option<crate::duel_victims::C603>,
    pub c560: Option<crate::duel_c560::C560>,
    /// The unit an evolution creates (side, slot), shown from the swap.
    pub spawned: Option<(u8, u8)>,
    /// A burn's card cell and amount, and its module.
    pub burn: Option<((u8, u8), u32)>,
    pub c509: Option<crate::duel_c509::C509>,
    /// The camera (eye, target) the burn's module draws with.
    pub view: Option<([f64; 3], [f64; 3])>,
    pub c022: Option<crate::duel_c022::C022>,
    pub c026: Option<crate::duel_c026::C026>,
    pub c027: Option<crate::duel_c027::C027>,
    /// The Force Field's trap shatters (param 1), one per target.
    pub c603s: Vec<crate::duel_victims::C603>,
    /// The ritual's module input (taken at its launch), the module, and its
    /// launch update E+9 (E+1 when every sacrifice was face up).
    pub ritual: Option<crate::duel_c059::RitualInput>,
    pub c059: Option<crate::duel_c059::C059>,
    pub ritual_start: u32,
    pub c501: Option<crate::duel_c501::C501>,
    pub shift_to: Option<(u8, u8)>,
    pub c531: Option<crate::duel_c531::C531>,
    pub c612: Vec<crate::duel_c612::C612>,
    /// Retype: the attribute (0x37) rather than the kind (0x36).
    pub retype_attribute: bool,
    /// The magic card's cell (Duster, Raigeki).
    pub card_cell: Option<(u8, u8)>,
    /// The side whose turn it is (the camera's yaw: c505 / c507's node).
    pub active_side: u8,
    /// The reacting trap was face down (its 8-update reveal at E).
    pub trap_was_down: bool,
    /// The newly ported modules: c505 bolts (with their launch e), c507
    /// feathers, c533, c602 on a reacting trap.
    pub c505: Vec<(u32, crate::duel_c505::C505)>,
    pub c507: Vec<crate::duel_c507::C507>,
    pub c533: Option<crate::duel_c533::C533>,
    pub c602: Option<crate::duel_c602::C602>,
    /// Dark Hole: the reacting White Hole / Royal Decree (one entry per
    /// search: the same trap twice when both searches pick it), their c602s,
    /// c021's input (mask, units, seed), c021 and the e of its last update.
    pub reacting: Vec<Target>,
    pub c602s: Vec<crate::duel_c602::C602>,
    pub c021_input: Option<(u32, Vec<crate::duel_c021::UnitIn>, u64)>,
    pub c021: Option<crate::duel_c021::C021>,
    pub c021_end: Option<u32>,
    /// Their random stream (the port's; call order and counts as the
    /// original's) [H values].
    pub rng: crate::duel_fxlib::Rng,
}

impl MonsterShow {
    pub fn new(effect: Effect, frozen: OverlayInput, duel: Option<roses_core::duel::Duel>, revealed: Option<(u8, u8)>, targets: Vec<Target>) -> Self {
        Self { effect, frozen, duel, revealed, targets, t: 0, c529: Vec::new(), c513: Vec::new(), c031: Vec::new(), c500: Vec::new(), c603: None, c560: None, spawned: None, burn: None, c509: None, view: None, c022: None, c026: None, c027: None, c603s: Vec::new(), ritual: None, c059: None, ritual_start: 10, c501: None, shift_to: None, c531: None, c612: Vec::new(), retype_attribute: false, card_cell: None, active_side: 0, trap_was_down: false, c505: Vec::new(), c507: Vec::new(), c533: None, c602: None, reacting: Vec::new(), c602s: Vec::new(), c021_input: None, c021: None, c021_end: None, rng: crate::duel_fxlib::Rng::new(0x1F9) }
    }

    /// The reveal's length (0 when the card was already face up).
    fn reveal(&self) -> u32 {
        if self.revealed.is_some() { REVEAL_UPDATES } else { 0 }
    }

    /// The handler's update (1 = E, its init), `None` during the reveal.
    pub fn e(&self) -> Option<u32> {
        self.t.checked_sub(self.reveal()).filter(|&e| e >= 1)
    }

    /// Updates after E until the handler returns.
    pub fn length(&self) -> u32 {
        match self.effect {
            Effect::Boost => 61,
            Effect::Defence | Effect::Reveal | Effect::Conceal => 80,
            Effect::Notice => 18,
            Effect::Destroy | Effect::Bind => 144,
            // E .. E+65 [F MOTH-EVOLUTION.md 1.2].
            Effect::Countdown => 66,
            // E .. E+143 [F MOTH-EVOLUTION.md 2.3].
            Effect::Evolve => 144,
            // Seen end + 23 (E+102 for u_a 24).
            Effect::Burn => self.apply_at() + 23,
            // E+103 without traps, E+162 with.
            Effect::Field => if self.targets.is_empty() { 104 } else { 163 },
            // E+191.
            Effect::Heart => 192,
            // E+243 (E+235 with every sacrifice face up).
            Effect::Ritual => self.ritual_start + 234,
            // E+82.
            Effect::Revive => 83,
            // E+76 at the earliest [F]; with the units' fade-back ~E+86 [H].
            Effect::Shift => 87,
            Effect::Crush => 144,
            Effect::Retype => 70,
            // E+96 (97 U) [F 4.1].
            Effect::Duster => 97,
            // E+118 [F 4.2].
            Effect::FakeTrap => 119,
            // E+61+12K [F 6.1].
            Effect::Raigeki => 62 + 12 * self.raigeki_k(),
            // E+112 [F 6.2].
            Effect::AntiRaigeki => 113,
            // E+94 [F 8.1].
            Effect::Lawbook => 95,
            // Return at E+c+L+17 (L unknown until c021 ends).
            Effect::DarkHole => self.dark_hole_c() + self.c021_len().unwrap_or(400) + 18,
        }
    }

    /// Dark Hole: c021's launch offset c (E+c): 1, or 39 after a reaction.
    pub fn dark_hole_c(&self) -> u32 {
        if self.reacting.is_empty() { 1 } else { 39 }
    }

    /// c021's length L once it has ended.
    pub fn c021_len(&self) -> Option<u32> {
        self.c021_end.map(|e| e - self.dark_hole_c())
    }

    /// Dark Hole: the update c021's end is seen (every target removed, the
    /// camera back): e = c + L + 1.
    pub fn dark_hole_seen(&self) -> Option<u32> {
        self.c021_end.map(|e| e + 1)
    }

    /// Dark Hole: the targets are gone (removed when c021's end is seen).
    pub fn dark_hole_removed(&self) -> bool {
        self.effect == Effect::DarkHole && self.dark_hole_seen().is_some_and(|s| self.e().is_some_and(|e| e >= s))
    }

    /// Dark Hole: c021 runs (its units and light apply).
    pub fn dark_hole_running(&self) -> bool {
        self.effect == Effect::DarkHole && self.c021.is_some()
    }

    /// Raigeki's farthest ring K = max(col, 6 - col, row, 6 - row) of the
    /// card's cell.
    pub fn raigeki_k(&self) -> u32 {
        let (c, r) = self.card_cell.unwrap_or((3, 3));
        u32::from(c.max(6 - c).max(r).max(6 - r))
    }

    /// The c505 launches (e, cell): Raigeki's card cell at e 2, then for k
    /// = 1..K the ring cells left, up, right, down (on the board) at e 2 +
    /// 12k [F 6.1, 0x250270]; Anti-Raigeki's ten on its own cell at e 40 +
    /// 6i [F 6.2].
    pub fn bolt_launches(&self) -> Vec<(u32, (u8, u8))> {
        match self.effect {
            Effect::Raigeki => {
                let (c, r) = self.card_cell.unwrap_or((3, 3));
                let mut out = vec![(2, (c, r))];
                for k in 1..=self.raigeki_k() as i32 {
                    let e = 2 + 12 * k as u32;
                    for (dc, dr) in [(-k, 0), (0, -k), (k, 0), (0, k)] {
                        let (cc, rr) = (i32::from(c) + dc, i32::from(r) + dr);
                        if (0..7).contains(&cc) && (0..7).contains(&rr) {
                            out.push((e, (cc as u8, rr as u8)));
                        }
                    }
                }
                out
            }
            Effect::AntiRaigeki => match self.targets.first() {
                Some(t) => (0..10).map(|i| (40 + 6 * i, t.cell)).collect(),
                None => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    /// Raigeki: the e from which a target's card fades (its cell's bolt's
    /// 2 seen: launch + 6), `None` for a target no bolt strikes.
    pub fn struck_at(&self, cell: (u8, u8)) -> Option<u32> {
        (self.effect == Effect::Raigeki).then(|| self.bolt_launches().into_iter().find(|b| b.1 == cell).map(|b| b.0 + 6)).flatten()
    }

    /// The module's first update (E + n) and length.
    pub fn module(&self) -> (u32, u32) {
        match self.effect {
            Effect::Boost => (7, 30),
            Effect::Defence | Effect::Reveal | Effect::Conceal => (23, 16),
            Effect::Notice => (2, 16),
            Effect::Shift => (9, crate::duel_c531::UPDATES),
            Effect::Retype => (31, 16),
            // c507 on every target at E+1 (60 U).
            Effect::Duster => (2, crate::duel_c507::UPDATES),
            // c602 on the reacting trap at E+8 (30 U).
            Effect::FakeTrap | Effect::AntiRaigeki => (9, crate::duel_activate::C602_UPDATES),
            // The bolts: the first at E+1, the last ends at E+1+12K+29.
            Effect::Raigeki => (2, 12 * self.raigeki_k() + crate::duel_c505::UPDATES),
            // c533 at E+1 (93 U).
            Effect::Lawbook => (2, crate::duel_c533::UPDATES),
            // c021 at E+c (its length once known).
            Effect::DarkHole => (self.dark_hole_c() + 1, self.c021_len().unwrap_or(400)),
            Effect::Crush => (54, crate::duel_c612::UPDATES),
            Effect::Destroy => (6, 44),
            Effect::Bind => (56, 30),
            // E+31: after the 30-update camera move.
            Effect::Countdown => (32, 16),
            // E+30, after the 30-update camera move.
            Effect::Evolve => (31, crate::duel_c560::UPDATES),
            // E+7, after the light ramp; c509 returns 8 at u_a + 48.
            Effect::Burn => (8, self.burn_updates()),
            Effect::Field => (8, crate::duel_c022::END_U),
            Effect::Heart => (2, crate::duel_c026::END_U),
            Effect::Ritual => (self.ritual_start, 216),
            Effect::Revive => (3, crate::duel_c501::UPDATES),
        }
    }

    /// The Force Field's camera return: E+77 without traps, E+136 after the
    /// trap shatters.
    fn field_back(&self) -> u32 {
        if self.targets.is_empty() { 78 } else { 137 }
    }

    /// c509's length (u_a + 48 once known; the r_w run's 72 before).
    fn burn_updates(&self) -> u32 {
        self.c509.as_ref().and_then(|c| c.end_u()).unwrap_or(72)
    }

    /// The victim leader's model point (object +0x80): the cell centre at
    /// y 50 [H: the bob between 46 and 53 is not followed].
    fn burn_point(&self) -> [f64; 3] {
        let t = self.targets.first().map_or((3, 3), |t| t.cell);
        let [x, z] = crate::duel_board::cell_center(t.0, t.1);
        [x, 50.0, z]
    }

    /// The module's update this update.
    pub fn module_u(&self) -> Option<u32> {
        let (start, len) = self.module();
        let e = self.e()?;
        (start..start + len).contains(&e).then(|| e - start + 1)
    }

    /// c603 on the Bug's victim (E+60 .. E+119).
    pub fn shatter_u(&self) -> Option<u32> {
        let e = self.e()?;
        (self.effect == Effect::Destroy && (60..120).contains(&e)).then(|| e - 59)
    }

    /// The update the rule change shows and the restore starts.
    fn apply_at(&self) -> u32 {
        match self.effect {
            Effect::Boost => 37,
            Effect::Defence | Effect::Reveal | Effect::Conceal => 56,
            Effect::Notice => 18,
            // The restore: light, units and camera come back.
            Effect::Shift => 71,
            Effect::Crush => 121,
            // E+46: box closed, camera back, restore.
            Effect::Retype => 47,
            // E+78: every target to the graveyard, camera back.
            Effect::Duster => 79,
            // E+100: the Fake Trap to the graveyard, camera back.
            Effect::FakeTrap => 101,
            // E+37+12K: every struck unit to the graveyard, camera back.
            Effect::Raigeki => 38 + 12 * self.raigeki_k(),
            // E+94: box closed, camera back (nothing changes).
            Effect::AntiRaigeki => 95,
            // E+94: one more summon (0x1DF980(ctrl, 1)).
            Effect::Lawbook => 95,
            // c021's end seen: the targets removed.
            Effect::DarkHole => self.dark_hole_seen().unwrap_or(u32::MAX),
            Effect::Destroy => 120,
            Effect::Bind => 86,
            // E+47: box closed, camera back, units fade back.
            Effect::Countdown => 48,
            // E+120: c560 seen ended; light, units and camera come back.
            Effect::Evolve => 121,
            // c509 seen ended: box closed, LP, camera back.
            Effect::Burn => 8 + self.burn_updates(),
            // E+75: every unbind and stat reset at once.
            Effect::Field => 76,
            // E+173: the control change.
            Effect::Heart => 174,
            // u 135: sacrifices removed, the result created.
            Effect::Ritual => self.ritual_start + 134,
            // E+34: the unit created after c501's 2 at u 32.
            Effect::Revive => 35,
        }
    }

    /// The rule change shows (from this update on).
    pub fn applied(&self) -> bool {
        let at = match self.effect {
            Effect::Defence | Effect::Reveal | Effect::Conceal => 27,
            // E+34: 0x1E19E0 / 0x1E1B10 after c513's 2.
            Effect::Retype => 35,
            _ => self.apply_at(),
        };
        self.e().is_some_and(|e| e >= at)
    }

    pub fn done(&self) -> bool {
        self.e().is_some_and(|e| e > self.length())
    }

    pub fn tick(&mut self) {
        use crate::duel_monster_mods::{C031, C513, C529};
        self.t += 1;
        let centre = |t: &Target| {
            let [x, z] = crate::duel_board::cell_center(t.cell.0, t.cell.1);
            [x, 0.0, z]
        };
        match self.module_u() {
            Some(u) => {
                if u == 1 {
                    let seed = self.t;
                    match self.effect {
                        Effect::Boost => self.c529 = self.targets.iter().enumerate().map(|(i, t)| C529::new(centre(t), 0x211 + seed + i as u32)).collect(),
                        Effect::Defence | Effect::Reveal | Effect::Conceal | Effect::Notice | Effect::Retype => self.c513 = self.targets.iter().enumerate().map(|(i, t)| C513::new(centre(t), 0x201 + seed + i as u32)).collect(),
                        Effect::Destroy => self.c031 = self.targets.iter().enumerate().map(|(i, t)| C031::new(centre(t), 0x1F + seed + i as u32)).collect(),
                        Effect::Evolve => self.c560 = Some(crate::duel_c560::C560::new(0x230 + seed)),
                        // Launched below (`tick_new`).
                        Effect::Burn | Effect::Heart | Effect::Duster | Effect::FakeTrap | Effect::Raigeki | Effect::AntiRaigeki | Effect::Lawbook | Effect::DarkHole => {}
                        Effect::Shift => {
                            self.c531 = self.targets.first().map(|t| {
                                crate::duel_c531::C531::new((i32::from(t.cell.0), i32::from(t.cell.1)), t.unit.1, false, 0x213 + seed)
                            })
                        }
                        Effect::Crush => {
                            self.c612 = self
                                .targets
                                .iter()
                                .enumerate()
                                .map(|(i, t)| {
                                    let [x, z] = crate::duel_board::cell_center(t.cell.0, t.cell.1);
                                    // The victim's model point [H: y 50].
                                    crate::duel_c612::C612::new([x, 50.0, z], 0x264 + seed + i as u32)
                                })
                                .collect()
                        }
                        Effect::Revive => self.c501 = self.targets.first().map(|t| crate::duel_c501::C501::new(t.cell)),
                        Effect::Ritual => {
                            if let Some(input) = self.ritual.take() {
                                self.c059 = Some(crate::duel_c059::C059::new(input, u64::from(0x3B + seed) | 1));
                            }
                        }
                        Effect::Field => self.c022 = Some(crate::duel_c022::C022::new(0, 0x16 + seed)),
                        Effect::Countdown => self.c513 = self.targets.iter().enumerate().map(|(i, t)| C513::new(centre(t), 0x201 + seed + i as u32)).collect(),
                        Effect::Bind => self.c500 = self.targets.iter().enumerate().map(|(i, t)| crate::duel_c500::C500::new(centre(t), 0x1F4 + seed + i as u32)).collect(),
                    }
                }
                for m in &mut self.c529 {
                    m.update(u);
                }
                for m in &mut self.c513 {
                    m.update();
                }
                for m in &mut self.c031 {
                    m.update();
                }
                for m in &mut self.c500 {
                    m.update(u);
                }
                if let Some(m) = &mut self.c560 {
                    m.update();
                }
                if let (Some(m), Some((eye, target))) = (&mut self.c022, self.view) {
                    m.update(u, eye, target);
                }
                if let Some(m) = &mut self.c501 {
                    m.update(u);
                }
                if let Some((eye, target)) = self.view {
                    if self.effect == Effect::Shift {
                        if let Some(m) = &mut self.c531 {
                            m.update(u, eye, target);
                        }
                    }
                    for m in &mut self.c612 {
                        m.update(u, eye, target);
                    }
                }
                if let Some(m) = &mut self.c059 {
                    let (eye, target) = self.view.unwrap_or(([0.0, 181.4, 241.4], [0.0, 0.0, 100.0]));
                    m.update(u, eye, target);
                }
                if self.effect == Effect::Burn {
                    let point = self.burn_point();
                    if u == 1 {
                        if let Some(((c, r), amount)) = self.burn {
                            self.c509 = Some(crate::duel_c509::C509::new((i32::from(c), i32::from(r)), amount, point, 0x1FD + self.t));
                        }
                    }
                    let (eye, target) = self.view.unwrap_or(([0.0, 181.4, 241.4], [0.0, 0.0, 100.0]));
                    if let Some(m) = &mut self.c509 {
                        m.update(u, point, eye, target);
                    }
                }
            }
            None => {
                self.c529.clear();
                self.c513.clear();
                self.c031.clear();
                self.c500.clear();
                self.c560 = None;
                self.c509 = None;
                self.c022 = None;
                self.c059 = None;
                self.c501 = None;
                self.c612.clear();
                if self.effect == Effect::Shift && self.e().is_some_and(|e| e < 51) {
                    // c531 ended; its card alpha (0) holds until c532.
                } else if self.effect != Effect::Shift {
                    self.c531 = None;
                }
            }
        }
        if self.effect == Effect::Shift {
            let e = self.e().unwrap_or(0);
            // c532 at the new cell, E+50 .. E+69.
            if (51..71).contains(&e) {
                if e == 51 {
                    let slot = self.targets.first().map_or(1, |t| t.unit.1);
                    self.c531 = self.shift_to.map(|c| crate::duel_c531::C531::new((i32::from(c.0), i32::from(c.1)), slot, true, 0x214 + self.t));
                }
                if let (Some(m), Some((eye, target))) = (&mut self.c531, self.view) {
                    m.update(e - 50, eye, target);
                }
            } else if e >= 71 {
                self.c531 = None;
            }
        }
        let e = self.e().unwrap_or(0);
        let (eye, target) = self.view.unwrap_or(([0.0, 181.4, 241.4], [0.0, 0.0, 100.0]));
        if self.effect == Effect::Field {
            // c603 param 1 on each face-up trap from E+76 (60 U).
            if (77..137).contains(&e) && !self.targets.is_empty() {
                let u = e - 76;
                if u == 1 {
                    let seed = self.t;
                    self.c603s = self
                        .targets
                        .iter()
                        .enumerate()
                        .map(|(i, t)| {
                            let [x, z] = crate::duel_board::cell_center(t.cell.0, t.cell.1);
                            let y = crate::duel_play::side_model_base(t.unit.0) - if t.defence { 0x4000 } else { 0 };
                            crate::duel_victims::C603::with_param([x, 0.0, z], [0x4000, y, 0x8000], 0x25B + seed + i as u32, 1)
                        })
                        .collect();
                }
                for c in &mut self.c603s {
                    c.update(u);
                }
            } else {
                self.c603s.clear();
            }
        }
        if self.effect == Effect::Heart {
            if (2..128).contains(&e) {
                let c = self.c026.get_or_insert_with(|| crate::duel_c026::C026::new(0x1A + e));
                c.update(e - 1, eye, target);
            } else {
                self.c026 = None;
            }
            if (129..173).contains(&e) {
                let unit = self.targets.first().map_or([0.0; 3], |t| {
                    let [x, z] = crate::duel_board::cell_center(t.cell.0, t.cell.1);
                    [x, 0.0, z]
                });
                let c = self.c027.get_or_insert_with(|| crate::duel_c027::C027::new(0x1B + e));
                c.update(e - 128, unit, eye, target);
            } else {
                self.c027 = None;
            }
        }
        self.tick_new(eye, target);
        match self.shatter_u() {
            Some(u) => {
                if u == 1 {
                    if let Some(t) = self.targets.first() {
                        // The card object's rotation (0x4000, Y, Z) [emu FV 1.1].
                        let y = crate::duel_play::side_model_base(t.unit.0) - if t.defence { 0x4000 } else { 0 };
                        self.c603 = Some(crate::duel_victims::C603::new(centre(t), [0x4000, y, if t.face_up { 0x8000 } else { 0 }], 0x25B + self.t));
                    }
                }
                if let Some(c) = &mut self.c603 {
                    c.update(u);
                }
            }
            None => self.c603 = None,
        }
    }

    /// The camera yaw c505 / c507 turn with (0x205580: 0 side 0, 0x8000
    /// side 1).
    fn yaw16(&self) -> u16 {
        if self.active_side == 0 { 0 } else { 0x8000 }
    }

    /// The modules of Duster, Fake Trap, Raigeki, Anti-Raigeki and Lawbook.
    fn tick_new(&mut self, eye: [f64; 3], target: [f64; 3]) {
        let Some(e) = self.e() else {
            return;
        };
        let centre = |t: &Target| {
            let [x, z] = crate::duel_board::cell_center(t.cell.0, t.cell.1);
            [x, 0.0, z]
        };
        let rot = |t: &Target| {
            let y = crate::duel_play::side_model_base(t.unit.0) - if t.defence { 0x4000 } else { 0 };
            [0x4000, y, if t.face_up { 0x8000 } else { 0 }]
        };
        let yaw = self.yaw16();
        match self.effect {
            Effect::Duster => {
                // c507 on every target at E+1 (param 0: additive).
                if e == 2 {
                    let rng = &mut self.rng;
                    self.c507 = self
                        .targets
                        .iter()
                        .map(|t| {
                            let [x, _, z] = centre(t);
                            crate::duel_c507::C507::new([x as f32, 0.0, z as f32], 0, &mut || rng.next())
                        })
                        .collect();
                }
                if (2..2 + crate::duel_c507::UPDATES).contains(&e) {
                    let rng = &mut self.rng;
                    for m in &mut self.c507 {
                        m.update(e - 1, yaw, &mut || rng.next(), eye, target);
                    }
                } else {
                    self.c507.clear();
                }
                // Each c507's 2 (u 17) seen at u 18 = E+18: c603 param 1
                // (60 U) on its card.
                let start = 1 + crate::duel_c507::C603_U;
                if e == start {
                    let seed = self.t;
                    self.c603s = self
                        .targets
                        .iter()
                        .enumerate()
                        .map(|(i, t)| crate::duel_victims::C603::with_param(centre(t), rot(t), 0x25B + seed + i as u32, 1))
                        .collect();
                }
                if (start..start + crate::duel_victims::C603_UPDATES).contains(&e) {
                    for c in &mut self.c603s {
                        c.update(e - start + 1);
                    }
                } else {
                    self.c603s.clear();
                }
            }
            Effect::FakeTrap | Effect::AntiRaigeki => {
                // c602 (param 0) on the trap at E+8.
                match (self.module_u(), self.targets.first().copied()) {
                    (Some(u), Some(t)) => {
                        if u == 1 {
                            self.c602 = Some(crate::duel_c602::C602::new(centre(&t), 0x25A + self.t));
                        }
                        if let Some(c) = &mut self.c602 {
                            c.update(u);
                        }
                    }
                    _ => self.c602 = None,
                }
                // Fake Trap: c603 (param 0) on it at E+39 (60 U).
                if self.effect == Effect::FakeTrap {
                    if (40..100).contains(&e) {
                        if e == 40 {
                            self.c603s = self.targets.iter().take(1).map(|t| crate::duel_victims::C603::new(centre(t), rot(t), 0x25B + self.t)).collect();
                        }
                        for c in &mut self.c603s {
                            c.update(e - 39);
                        }
                    } else {
                        self.c603s.clear();
                    }
                }
            }
            Effect::Lawbook => match self.module_u() {
                Some(u) => {
                    let c = self.c533.get_or_insert_with(crate::duel_c533::C533::new);
                    c.update(u);
                }
                None => self.c533 = None,
            },
            Effect::DarkHole => {
                // The reaction's c602 (param 0) on each reacting entry at E+8.
                if (9..9 + crate::duel_activate::C602_UPDATES).contains(&e) && !self.reacting.is_empty() {
                    if e == 9 {
                        let seed = self.t;
                        self.c602s = self.reacting.iter().enumerate().map(|(i, t)| crate::duel_c602::C602::new(centre(t), 0x25A + seed + i as u32)).collect();
                    }
                    for c in &mut self.c602s {
                        c.update(e - 8);
                    }
                } else {
                    self.c602s.clear();
                }
                // c021 from E+c until it returns 8 (its last state kept).
                let start = self.dark_hole_c() + 1;
                if e >= start && self.c021_end.is_none() {
                    if self.c021.is_none() {
                        if let Some((mask, units, seed)) = self.c021_input.clone() {
                            self.c021 = Some(crate::duel_c021::C021::new(mask, &units, seed));
                        }
                    }
                    match &mut self.c021 {
                        Some(c) => {
                            if c.update(eye, target) == 8 {
                                self.c021_end = Some(e);
                            }
                        }
                        None => self.c021_end = Some(e),
                    }
                }
            }
            _ => {}
        }
        if matches!(self.effect, Effect::Raigeki | Effect::AntiRaigeki) {
            for (at, cell) in self.bolt_launches() {
                if at == e {
                    let rng = &mut self.rng;
                    self.c505.push((at, crate::duel_c505::C505::new(cell, &mut || rng.next())));
                }
            }
            let rng = &mut self.rng;
            for (at, m) in &mut self.c505 {
                m.update(e - *at + 1, yaw, &mut || rng.next(), eye, target);
            }
            self.c505.retain(|(at, _)| e < at + crate::duel_c505::UPDATES);
        }
    }

    /// A target is hidden on the board this update: replaced by its c603
    /// copy (Duster from E+18, Fake Trap from E+39) or removed (Raigeki at
    /// E+37+12K).
    pub fn new_hidden(&self, unit: (u8, u8)) -> bool {
        let Some(e) = self.e() else {
            return false;
        };
        let target = self.targets.iter().any(|t| t.unit == unit);
        target
            && match self.effect {
                Effect::Duster => e >= 1 + crate::duel_c507::C603_U,
                Effect::FakeTrap => e >= 40,
                Effect::Raigeki => e >= self.apply_at(),
                _ => false,
            }
    }

    /// Raigeki: a struck target's level (0x2519E0 mode 1, -8 per update
    /// from its bolt's 2 seen).
    pub fn struck_level(&self, t: &Target) -> Option<u8> {
        let e = self.e()?;
        let from = self.struck_at(t.cell)?;
        (e >= from).then(|| 128u32.saturating_sub(8 * (e - from + 1)) as u8)
    }

    /// The reacting trap's reveal (Fake Trap, Anti-Raigeki): its unit,
    /// +0x54 while it turns (0x20 per update from E+1) and whether it shows
    /// face up (from E+8).
    pub fn trap_reveal(&self) -> Option<((u8, u8), Option<u8>, bool)> {
        let t = match self.effect {
            Effect::FakeTrap | Effect::AntiRaigeki => self.targets.first()?,
            // Dark Hole: the reacting trap (revealed at E by both searches).
            Effect::DarkHole => self.reacting.last()?,
            _ => return None,
        };
        let e = self.e()?;
        if !self.trap_was_down {
            return Some((t.unit, None, true));
        }
        let flip = (2..9).contains(&e).then(|| (0x20 * (e - 1)) as u8);
        Some((t.unit, flip, e >= 9))
    }

    /// Duster: the cross cells 0x261DB0(col, row, 0, 0, 1, 0x12, 0xFF)
    /// tints (255, 255, 255, 128) from E [F 1.2, 4.1].
    pub fn tint_cells(&self) -> Vec<(u8, u8)> {
        let (Effect::Duster, Some((c, r)), Some(e)) = (self.effect, self.card_cell, self.e()) else {
            return Vec::new();
        };
        if e >= self.apply_at() {
            // [H: the tint's fade is not traced; cleared with the removal.]
            return Vec::new();
        }
        let mut cells: Vec<(u8, u8)> = (0..7).map(|k| (k, r)).collect();
        cells.extend((0..7).filter(|&k| k != r).map(|k| (c, k)));
        cells
    }

    /// The tinted cells as alpha-blended white quads on the board plane
    /// [H: the board's panel tint drawing is not traced; alpha 32].
    pub fn tint_tris(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<crate::duel_c621::FxTri> {
        let cells = self.tint_cells();
        let Some(project) = crate::duel_c602::projector(eye, target) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let c = [128, 128, 128, 32];
        for (col, row) in cells {
            let [x, z] = crate::duel_board::cell_center(col, row);
            let p = [[x - 25.0, 0.0, z - 25.0], [x + 25.0, 0.0, z - 25.0], [x - 25.0, 0.0, z + 25.0], [x + 25.0, 0.0, z + 25.0]].map(|w| project(w));
            if let [Some(a), Some(b), Some(cc), Some(d)] = p {
                out.push(crate::duel_c621::FxTri { screen: [a, b, cc], uv: [[0.0; 2]; 3], colour: [c; 3], textured: false, sheet: 0 });
                out.push(crate::duel_c621::FxTri { screen: [b, d, cc], uv: [[0.0; 2]; 3], colour: [c; 3], textured: false, sheet: 0 });
            }
        }
        out
    }

    /// Dark Hole: the targets' card objects c021 carries off (card, pose,
    /// colour), until they are removed.
    pub fn dark_hole_cards(&self) -> Vec<(u16, crate::duel_versus::CardPose, [u8; 4])> {
        let Some(c) = &self.c021 else {
            return Vec::new();
        };
        if self.dark_hole_removed() {
            return Vec::new();
        }
        self.targets
            .iter()
            .filter_map(|t| {
                let k = c.card((u16::from(t.unit.0) << 8) | u16::from(t.unit.1))?;
                let pose = crate::duel_versus::CardPose { position: k.position, spin: k.rotation, tilt: [0; 3], scale: [k.scale, k.scale] };
                Some((t.card, pose, k.colour))
            })
            .collect()
    }

    /// c533's screen-space layers (Z max, key 0: over the HUD).
    pub fn top_tris(&self) -> Vec<crate::duel_c621::FxTri> {
        self.c533.as_ref().map_or_else(Vec::new, |m| m.additive.clone())
    }

    /// The modules' triangles through the board camera.
    pub fn tris(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<crate::duel_c621::FxTri> {
        let mut out = Vec::new();
        for m in &self.c529 {
            out.extend(m.draw(eye, target));
        }
        for m in &self.c513 {
            out.extend(m.draw(eye, target));
        }
        for m in &self.c031 {
            out.extend(m.draw(eye, target));
        }
        for m in &self.c500 {
            out.extend(m.draw(eye, target).0);
        }
        // Screen space, over the scene (key 0).
        if let Some(m) = &self.c560 {
            out.extend_from_slice(m.draw());
        }
        if let Some(m) = &self.c509 {
            out.extend_from_slice(&m.additive);
        }
        if let Some(m) = &self.c059 {
            out.extend_from_slice(&m.additive);
        }
        if let Some(m) = &self.c501 {
            out.extend(m.draw(eye, target));
        }
        if let Some(m) = &self.c531 {
            out.extend_from_slice(&m.additive);
        }
        for m in &self.c612 {
            out.extend_from_slice(&m.additive);
        }
        for m in [self.c022.as_ref().map(|m| &m.additive), self.c026.as_ref().map(|m| &m.additive), self.c027.as_ref().map(|m| &m.additive)].into_iter().flatten() {
            out.extend_from_slice(m);
        }
        for (_, m) in &self.c505 {
            out.extend_from_slice(&m.additive);
        }
        for m in &self.c507 {
            out.extend_from_slice(&m.additive);
        }
        if let Some(m) = &self.c602 {
            out.extend(m.draw(eye, target));
        }
        for m in &self.c602s {
            out.extend(m.draw(eye, target));
        }
        if let (Some(m), None) = (&self.c021, self.c021_end) {
            out.extend_from_slice(&m.additive);
        }
        out
    }

    /// c509's subtractive (explosion B) and Cs * As (cores, number)
    /// triangles.
    pub fn blended_tris(&self) -> (Vec<crate::duel_c621::FxTri>, Vec<crate::duel_c621::FxTri>) {
        let mut out = self.c509.as_ref().map_or((Vec::new(), Vec::new()), |m| (m.subtractive.clone(), m.opaque.clone()));
        if let Some(m) = &self.c059 {
            out.0.extend_from_slice(&m.subtractive);
        }
        for (_, m) in &self.c505 {
            out.0.extend_from_slice(&m.subtractive);
            out.1.extend_from_slice(&m.opaque);
        }
        // c021's shadow annuli (Cd - Cs As / 128).
        if let (Some(m), None) = (&self.c021, self.c021_end) {
            out.0.extend_from_slice(&m.subtractive);
        }
        for m in &self.c507 {
            out.0.extend_from_slice(&m.subtractive);
            out.1.extend_from_slice(&m.opaque);
        }
        out
    }

    /// c612's smoke: ALPHA 0x44, (Cs - Cd) As + Cd, drawn over the scene.
    pub fn alpha_blend_tris(&self) -> Vec<crate::duel_c621::FxTri> {
        let mut out: Vec<crate::duel_c621::FxTri> = self.c612.iter().flat_map(|m| m.alpha_blend.iter().copied()).collect();
        for (_, m) in &self.c505 {
            out.extend_from_slice(&m.alpha_blend);
        }
        for m in &self.c507 {
            out.extend_from_slice(&m.alpha_blend);
        }
        out
    }

    /// Shift: the unit's card alpha and cell this update (`None` outside
    /// the module runs: as on the board).
    pub fn shift_card(&self) -> Option<(u8, (u8, u8))> {
        let e = self.e()?;
        let from = self.targets.first()?.cell;
        let to = self.shift_to?;
        let cell = if e >= 26 { to } else { from };
        let alpha = match &self.c531 {
            Some(m) => m.card_alpha(),
            None if (26..51).contains(&e) => 0,
            None => 128,
        };
        Some((alpha, cell))
    }

    /// The crushed units are removed at E+118.
    pub fn crushed_removed(&self) -> bool {
        self.effect == Effect::Crush && self.e().is_some_and(|e| e >= 119)
    }

    /// The ritual's card objects c059 moves: each sacrifice (card, pose,
    /// colour) until removed, then the result (black, from u 135).
    pub fn ritual_cards(&self) -> Vec<(u16, crate::duel_versus::CardPose, [u8; 4])> {
        if self.effect == Effect::DarkHole {
            return self.dark_hole_cards();
        }
        let Some(c) = &self.c059 else {
            return Vec::new();
        };
        let pose = |o: &crate::duel_c059::CardObj| crate::duel_versus::CardPose { position: o.position, spin: o.rotation, tilt: [0; 3], scale: [0.1, 0.1] };
        let mut out: Vec<_> = (0..3)
            .filter_map(|k| Some((self.targets.get(k)?.card, c.sacrifice_pose(k)?)))
            .map(|(card, o)| (card, pose(&o), o.colour))
            .collect();
        if let (Some((_, o)), Some(card)) = (c.result_pose(), c.result_card()) {
            out.push((card, pose(&o), o.colour));
        }
        out
    }

    /// c500's landing flash (full screen, over the HUD).
    pub fn flash_tris(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<crate::duel_c621::FxTri> {
        let mut out: Vec<crate::duel_c621::FxTri> = self.c500.iter().take(1).flat_map(|m| m.draw(eye, target).1).collect();
        if let Some(m) = &self.c501 {
            out.extend(m.flash());
        }
        // c505's full-screen flashes (u 2, 4, 6 of every bolt).
        for (_, m) in &self.c505 {
            out.extend_from_slice(&m.flash);
        }
        out
    }

    /// The board light 0x2DF5B1: -15 per update to 0x30 from E+1, +15
    /// from the restore (Bind: after the box closes) [F].
    pub fn light(&self) -> u8 {
        let Some(e) = self.e() else {
            return 128;
        };
        if self.effect == Effect::Countdown {
            // No light variable in 0x26F5F0 [F].
            return 128;
        }
        if self.effect == Effect::Lawbook {
            // c533 writes the board light every update (0x1F7F00) [F 13.3].
            return self.c533.as_ref().map_or(128, |c| c.board_light());
        }
        if self.effect == Effect::DarkHole {
            // c021 writes the board light (0x1F7F00); 0x258C60 never does [F].
            return self.c021.as_ref().map_or(128, |c| c.light());
        }
        if matches!(self.effect, Effect::Duster | Effect::FakeTrap | Effect::Raigeki | Effect::AntiRaigeki) {
            // No 0x1F7F00 call in 0x25A080 / 0x2593C0 [F 4, 6].
            return 128;
        }
        if self.effect == Effect::Burn && e > self.apply_at() + 6 {
            return 128;
        }
        if matches!(self.effect, Effect::Heart | Effect::Revive | Effect::Notice) {
            // 0x25E300 and 0x2586A0 never touch the light [F].
            return 128;
        }
        if self.effect == Effect::Ritual {
            // c059 writes the board light (0x1F7F00).
            return self.c059.as_ref().map_or(128, |c| c.light());
        }
        if self.effect == Effect::Field {
            // Down 15 per update to 48; after the camera returns the light
            // variable (cleared at E+76) restarts at 15, +15 per update
            // [F quirk, PEGASUS-MAGIC.md 2.4].
            let back = self.field_back();
            return if e <= back { (128 - (15 * e.min(6)) as i32).max(48) as u8 } else { (15 * (e - back) as i32).min(128) as u8 };
        }
        let up = self.apply_at();
        if e <= up {
            (128 - (15 * e.min(6)) as i32).max(48) as u8
        } else {
            (48 + 15 * (e - up) as i32).min(128) as u8
        }
    }

    /// The non-target units' level: -8 per update from E, +8 from the
    /// restore (0x261940) [F].
    pub fn others_level(&self) -> u8 {
        let Some(e) = self.e() else {
            return 128;
        };
        if matches!(self.effect, Effect::Burn | Effect::Field | Effect::Heart | Effect::Ritual | Effect::Revive | Effect::Notice | Effect::Duster | Effect::FakeTrap | Effect::Raigeki | Effect::AntiRaigeki | Effect::Lawbook | Effect::DarkHole) {
            // These handlers fade no unit (Burn highlights both leaders [H look]).
            return 128;
        }
        let up = self.apply_at();
        if e < up {
            128u32.saturating_sub(8 * e) as u8
        } else if self.effect == Effect::Countdown {
            // +8 from the update after the close, full at E+63 [F 1.2].
            (8 * (e - up)).min(128) as u8
        } else {
            (8 * (e - up + 1)).min(128) as u8
        }
    }

    /// The message box text this update (y 0x50).
    pub fn box_text(&self) -> Option<u16> {
        let e = self.e()?;
        let (start, _) = self.module();
        match self.effect {
            Effect::Boost if (start..self.apply_at()).contains(&e) => Some(0x107),
            Effect::Bind if (start..self.apply_at()).contains(&e) => Some(0x109),
            // "Damage!" from c509's launch to its seen end.
            Effect::Burn if (start..self.apply_at()).contains(&e) => Some(0x10C),
            Effect::Retype if (start..self.apply_at()).contains(&e) => Some(if self.retype_attribute { 0x112 } else { 0x111 }),
            // "Virus infection!" with c612.
            Effect::Crush if (54..121).contains(&e) => Some(0x115),
            // "Effect activated!" with c513.
            Effect::Notice if (2..18).contains(&e) => Some(0x105),
            // "Ritual activated!" from c059's launch to its seen end.
            Effect::Ritual if (start..start + 217).contains(&e) => Some(0x106),
            // "Turn Count for effect ready!" at y 0x50.
            Effect::Countdown if (start..self.apply_at()).contains(&e) => Some(0x114),
            // The Fake Trap's name from its c602 to its end (E+8..E+37).
            Effect::FakeTrap if (9..39).contains(&e) => self.targets.first().map(|t| 323 + t.card),
            // "Trap activated!" with c602, then the name with the bolts.
            Effect::AntiRaigeki if (9..39).contains(&e) => Some(0x104),
            Effect::AntiRaigeki if (40..95).contains(&e) => self.targets.first().map(|t| 323 + t.card),
            // The reacting trap's name from E (set at the init, the second
            // search's last) to its close at E+38.
            Effect::DarkHole if (1..39).contains(&e) => self.reacting.last().map(|t| 323 + t.card),
            _ => None,
        }
    }

    /// The camera step starting this update.
    pub fn camera(&self) -> Option<CameraStep> {
        let e = self.e()?;
        let single = (self.targets.len() == 1).then(|| self.targets[0]);
        match (self.effect, e) {
            (Effect::Boost, 1) => Some(single.map_or(CameraStep::Overview(16), |t| CameraStep::Unit(t, 16))),
            (Effect::Defence | Effect::Reveal | Effect::Conceal, 1) => Some(single.map_or(CameraStep::Overview(16), |t| CameraStep::Unit(t, 0))),
            (Effect::Destroy, 1) => Some(CameraStep::Overview(16)),
            // The c603 victim (target 0) [F MEP 2.3, NEWLY-ALLOWED.md 5].
            (Effect::Destroy, 60) => self.targets.first().map(|t| CameraStep::Unit(*t, 0)),
            (Effect::Countdown, 1) => single.map(|t| CameraStep::Unit(t, 30)),
            (Effect::Countdown, 48) => Some(CameraStep::Back(16)),
            (Effect::Burn, 1) => self.burn.map(|(cell, _)| {
                let mut t = self.targets.first().copied().unwrap_or(Target { unit: (0, 0), cell, card: 0, face_up: false, defence: false, face: None });
                t.cell = cell;
                t.face_up = false;
                CameraStep::Cell(t, 0, 250)
            }),
            (Effect::Burn, 8) => single.map(|t| CameraStep::Unit(t, 22)),
            (Effect::Field | Effect::Heart, 1) => Some(CameraStep::Overview(16)),
            (Effect::Field, e) if e == self.field_back() => Some(CameraStep::Back(16)),
            (Effect::Heart, 129) => single.map(|t| CameraStep::Unit(t, 0)),
            (Effect::Heart, 173) => Some(CameraStep::Back(16)),
            (Effect::Ritual, 1) => Some(CameraStep::Overview(16)),
            (Effect::Shift, 1) => single.map(|t| CameraStep::Unit(t, 0)),
            (Effect::Shift, 27) => match (single, self.shift_to) {
                (Some(mut t), Some(to)) => {
                    t.cell = to;
                    Some(CameraStep::Unit(t, 30))
                }
                _ => Some(CameraStep::Overview(16)),
            },
            (Effect::Shift, 71) | (Effect::Crush, 121) => Some(CameraStep::Back(16)),
            (Effect::Crush, 1) => Some(CameraStep::Overview(30)),
            (Effect::Retype, 1) => Some(single.map_or(CameraStep::Overview(30), |t| CameraStep::Unit(t, 30))),
            (Effect::Retype, 47) => Some(CameraStep::Back(16)),
            (Effect::Crush, 54) => single.map(|t| CameraStep::Unit(t, 0)),
            (Effect::Revive, 1) => single.map(|t| CameraStep::Cell(t, 16, 180)),
            (Effect::Revive, 65) => Some(CameraStep::Back(16)),
            (Effect::Ritual, e) if e == self.ritual_start + 216 => Some(CameraStep::Back(16)),
            (Effect::Burn, e) if e == self.apply_at() => Some(CameraStep::Back(16)),
            (Effect::Evolve, 1) => single.map(|t| CameraStep::Unit(t, 30)),
            (Effect::Evolve, 121) => Some(CameraStep::Back(16)),
            (Effect::Bind, 1) => Some(single.map_or(CameraStep::Overview(32), |t| CameraStep::Unit(t, 32))),
            (Effect::Boost, 37) | (Effect::Defence | Effect::Reveal | Effect::Conceal, 56) | (Effect::Destroy, 120) => Some(CameraStep::Back(16)),
            (Effect::Bind, 109) => Some(CameraStep::Back(32)),
            (Effect::Duster, 1) => Some(CameraStep::Overview(30)),
            // 0x262410(0, -1, 180, 180): the garbage record lands on (0, 0, 0),
            // the centre cell, polar (180, 90 deg, 22.5 deg) [emu 765, H elsewhere].
            (Effect::FakeTrap, 1) => Some(CameraStep::Cell(Target { unit: (0, 0), cell: (3, 3), card: 0, face_up: false, defence: false, face: None }, 0, 180)),
            // 0x262DE0(0, card cell, 200), then the overview at E+18.
            (Effect::Raigeki, 1) => self.card_cell.map(|cell| CameraStep::Cell(Target { unit: (0, 0), cell, card: 0, face_up: false, defence: false, face: None }, 0, 200)),
            (Effect::Raigeki, 19) => Some(CameraStep::Overview(30)),
            // 0x262410(0, anti, 180, 180) (a set card: the cell branch).
            (Effect::AntiRaigeki, 1) => self.targets.first().map(|t| CameraStep::Cell(*t, 0, 180)),
            // 0x262410(30, anti, 500, 500).
            (Effect::AntiRaigeki, 39) => self.targets.first().map(|t| CameraStep::Cell(*t, 30, 500)),
            (Effect::Duster | Effect::FakeTrap | Effect::Raigeki | Effect::AntiRaigeki, e) if e == self.apply_at() => Some(CameraStep::Back(16)),
            // 0x262410(0, reacting trap, 180, 180) at E+1 (camera state 0).
            (Effect::DarkHole, 2) if !self.reacting.is_empty() => self.reacting.last().map(|t| CameraStep::Cell(*t, 0, 180)),
            (Effect::DarkHole, e) => {
                let c = self.dark_hole_c();
                match self.dark_hole_seen() {
                    Some(seen) if e == seen => Some(CameraStep::Back(16)),
                    Some(seen) if e > seen => None,
                    _ if e == c + 1 => Some(CameraStep::Overview(20)),
                    // State 4 from E+c+22: 0x2622B0(60, 0x3FFF) every 60 U.
                    _ if e >= c + 23 && (e - c - 23) % 60 == 0 => Some(CameraStep::Orbit(60, 0x3FFF)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Sound cues (bank, id) this update: the reveal (0, 0x26), the
    /// modules' SECTL schedules (bank 1) [F].
    pub fn sounds(&self) -> Vec<(u8, u8)> {
        let mut out = Vec::new();
        if self.revealed.is_some() && self.t == 1 {
            out.push((0, 0x26));
        }
        if let Some(u) = self.module_u() {
            let ids: &[(u32, u8)] = match self.effect {
                Effect::Boost => &[(1, 0x24), (23, 0x25)],
                Effect::Defence | Effect::Reveal | Effect::Conceal | Effect::Countdown | Effect::Notice | Effect::Retype => &[(1, 0x10)],
                Effect::Evolve => &[(1, 0x36), (79, 0x3B)],
                Effect::Burn => &[(1, 0x0C), (22, 0x0D)],
                Effect::Field => &[(1, 0x23)],
                Effect::Heart => &[(1, 0x21)],
                Effect::Revive => &[(1, 0x02)],
                Effect::Shift => &[(1, 0x27)],
                Effect::Crush => &[(1, 0x50)],
                Effect::Ritual => &[(1, 0x46), (45, 0x46), (90, 0x46), (100, 0x47), (120, 0x48)],
                Effect::Destroy => &[(1, 0x1B), (40, 0x1C)],
                Effect::Bind => &[(1, 0x00), (10, 0x01)],
                Effect::Duster => &[(17, 0x09), (28, 0x09), (34, 0x09), (41, 0x09)],
                Effect::FakeTrap | Effect::AntiRaigeki => &[(1, 0x3F)],
                Effect::Lawbook => &[(1, 0x29), (20, 0x2A), (30, 0x2A), (40, 0x2A), (50, 0x2A), (60, 0x2A), (70, 0x2A), (77, 0x2B)],
                Effect::Raigeki => &[],
                // c021 has no SE of its own; the handler's 0x49 is below.
                Effect::DarkHole => &[],
            };
            out.extend(ids.iter().filter(|(at, _)| *at == u).map(|&(_, id)| (1, id)));
        }
        if self.shatter_u() == Some(1) {
            out.push((1, 0x40));
        }
        let e = self.e().unwrap_or(0);
        if self.effect == Effect::Field && e == 77 && !self.targets.is_empty() {
            out.push((1, 0x40));
        }
        if self.effect == Effect::Shift && e == 51 {
            out.push((1, 0x28));
        }
        if self.effect == Effect::Heart && e == 129 {
            out.push((1, 0x22));
        }
        // c603's (1, 0x40): the Duster's per target at E+18, the Fake Trap's
        // at E+39.
        if self.effect == Effect::Duster && e == 1 + crate::duel_c507::C603_U {
            out.extend(self.targets.iter().map(|_| (1, 0x40)));
        }
        if self.effect == Effect::FakeTrap && e == 40 {
            out.push((1, 0x40));
        }
        // Dark Hole: c602's (1, 0x3F) per reacting entry at E+8; the
        // handler's SE 0x49 at E+c+1 + 28k while c021 runs.
        if self.effect == Effect::DarkHole {
            if e == 9 {
                out.extend(self.reacting.iter().map(|_| (1, 0x3F)));
            }
            let c = self.dark_hole_c();
            let running = self.dark_hole_seen().is_none_or(|s| e < s);
            if running && e >= c + 2 && (e - c - 2) % 28 == 0 {
                out.push((1, crate::duel_c021::SE_ID as u8));
            }
        }
        // c505's (1, 0x07) at every bolt's u 1.
        for (at, _) in self.bolt_launches() {
            if at == e {
                out.push((1, crate::duel_c505::SE_ID as u8));
            }
        }
        out
    }

    /// The revealed card's +0x54 during its flip (0x20 per update).
    pub fn flip(&self) -> Option<u8> {
        (self.revealed.is_some() && (1..8).contains(&self.t)).then(|| (0x20 * self.t) as u8)
    }

    /// The revealed card shows face up.
    pub fn revealed_face_up(&self) -> bool {
        self.t >= 8
    }

    /// The evolution's swap has happened (E+105, the update after c560's 2):
    /// the old unit is gone and the new card shows.
    pub fn swapped(&self) -> bool {
        let (start, _) = self.module();
        self.effect == Effect::Evolve && self.e().is_some_and(|e| e >= start + crate::duel_c560::SWAP_U)
    }

    /// The Bug's victim is hidden (c603 replaces its card, removed after).
    pub fn victim_hidden(&self) -> Option<Target> {
        (self.effect == Effect::Destroy && self.e().is_some_and(|e| e >= 60)).then(|| self.targets.first().copied()).flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> OverlayInput {
        serde_json::from_str("{}").unwrap()
    }

    fn target() -> Target {
        Target { unit: (1, 2), cell: (3, 2), card: 1, face_up: true, defence: false, face: None }
    }

    #[test]
    fn boost_timeline_follows_the_handler() {
        let mut s = MonsterShow::new(Effect::Boost, input(), None, Some((0, 3)), vec![target()]);
        let mut log = Vec::new();
        while !s.done() {
            s.tick();
            log.push((s.e(), s.light(), s.box_text(), s.module_u(), s.camera()));
        }
        // 24 reveal updates, then E = 1 .. 61, done at E 62.
        assert_eq!(log.len() as u32, REVEAL_UPDATES + 62);
        let at = |e: u32| log[(REVEAL_UPDATES + e - 1) as usize].clone();
        assert_eq!(at(1).4, Some(CameraStep::Unit(target(), 16)));
        assert_eq!((at(1).1, at(6).1, at(7).1), (113, 48, 48));
        assert_eq!((at(7).2, at(7).3), (Some(0x107), Some(1)));
        assert_eq!((at(36).3, at(37).2, at(37).4), (Some(30), None, Some(CameraStep::Back(16))));
        assert_eq!((at(38).1, at(43).1), (63, 128));
    }

    #[test]
    fn countdown_follows_the_moth_handler() {
        let mut s = MonsterShow::new(Effect::Countdown, input(), None, Some((1, 4)), vec![target()]);
        let mut log = Vec::new();
        while !s.done() {
            s.tick();
            log.push((s.e(), s.light(), s.box_text(), s.module_u(), s.camera(), s.others_level(), s.sounds()));
        }
        assert_eq!(log.len() as u32, REVEAL_UPDATES + 67);
        let at = |e: u32| log[(REVEAL_UPDATES + e - 1) as usize].clone();
        assert_eq!(at(1).4, Some(CameraStep::Unit(target(), 30)));
        assert!(log.iter().all(|l| l.1 == 128), "no dim");
        assert_eq!((at(31).2, at(32).2, at(32).3, at(32).6.clone()), (None, Some(0x114), Some(1), vec![(1, 0x10)]));
        assert_eq!((at(47).3, at(48).2, at(48).4), (Some(16), None, Some(CameraStep::Back(16))));
        assert_eq!((at(16).5, at(48).5, at(64).5), (0, 0, 128));
    }

    #[test]
    fn evolve_follows_the_moth_handler() {
        let mut s = MonsterShow::new(Effect::Evolve, input(), None, None, vec![target()]);
        let mut log = Vec::new();
        while !s.done() {
            s.tick();
            log.push((s.e(), s.light(), s.box_text(), s.module_u(), s.camera(), s.sounds(), s.swapped()));
        }
        assert_eq!(log.len(), 145);
        let at = |e: u32| log[(e - 1) as usize].clone();
        assert_eq!(at(1).4, Some(CameraStep::Unit(target(), 30)));
        assert_eq!((at(6).1, at(30).1), (48, 48));
        assert!(log.iter().all(|l| l.2.is_none()), "no box");
        assert_eq!((at(31).3, at(31).5.clone()), (Some(1), vec![(1, 0x36)]));
        assert_eq!(at(109).5, vec![(1, 0x3B)]);
        assert_eq!((at(105).6, at(106).6), (false, true));
        assert_eq!((at(121).4, at(122).1, at(127).1), (Some(CameraStep::Back(16)), 63, 128));
    }

    #[test]
    fn field_and_heart_follow_their_handlers() {
        let mut f = MonsterShow::new(Effect::Field, input(), None, None, Vec::new());
        let mut log = Vec::new();
        while !f.done() {
            f.tick();
            log.push((f.e().unwrap(), f.light(), f.camera(), f.sounds(), f.module_u()));
        }
        assert_eq!(log.len(), 105);
        assert_eq!((log[5].1, log[77].1, log[78].1, log[86].1), (48, 48, 15, 128));
        assert_eq!((log[0].2, log[77].2), (Some(CameraStep::Overview(16)), Some(CameraStep::Back(16))));
        assert_eq!((log[7].4, log[7].3.clone(), log[74].4), (Some(1), vec![(1, 0x23)], Some(68)));
        let mut h = MonsterShow::new(Effect::Heart, input(), None, None, vec![target()]);
        let mut log = Vec::new();
        while !h.done() {
            h.tick();
            log.push((h.camera(), h.sounds(), h.light(), h.applied()));
        }
        assert_eq!(log.len(), 193);
        assert_eq!((log[1].1.clone(), log[128].1.clone()), (vec![(1, 0x21)], vec![(1, 0x22)]));
        assert_eq!((log[128].0, log[172].0), (Some(CameraStep::Unit(target(), 0)), Some(CameraStep::Back(16))));
        assert!(log.iter().all(|l| l.2 == 128));
        assert_eq!((log[172].3, log[173].3), (false, true));
    }

    #[test]
    fn bind_and_destroy_lengths() {
        let mut s = MonsterShow::new(Effect::Bind, input(), None, None, vec![target()]);
        s.t = 56;
        assert_eq!((s.module_u(), s.box_text()), (Some(1), Some(0x109)));
        s.t = 86;
        assert_eq!((s.box_text(), s.light()), (None, 48));
        s.t = 109;
        assert_eq!(s.camera(), Some(CameraStep::Back(32)));
        let mut d = MonsterShow::new(Effect::Destroy, input(), None, None, vec![target()]);
        d.t = 60;
        assert_eq!((d.shatter_u(), d.camera()), (Some(1), Some(CameraStep::Unit(target(), 0))));
        d.t = 145;
        assert!(d.done());
    }
}

#[cfg(test)]
mod draw_tests {
    use super::*;

    #[test]
    fn field_and_ritual_modules_draw_under_the_overview() {
        let input: OverlayInput = serde_json::from_str("{}").unwrap();
        let mut f = MonsterShow::new(Effect::Field, input, None, None, Vec::new());
        // The overview: target 0, distance 500, pitch 0x17FF, side 0 yaw.
        let pitch = f64::from(0x17FF) * std::f64::consts::PI / 32768.0;
        f.view = Some(([0.0, 500.0 * pitch.sin(), 500.0 * pitch.cos()], [0.0; 3]));
        let mut drawn = 0;
        for _ in 0..40 {
            f.tick();
            drawn = drawn.max(f.tris([0.0; 3], [0.0; 3]).len());
        }
        assert!(drawn > 100, "c022 draws {drawn}");
    }
    fn input() -> OverlayInput {
        serde_json::from_str("{}").unwrap()
    }

    fn at_cell(unit: (u8, u8), cell: (u8, u8), face_up: bool) -> Target {
        Target { unit, cell, card: 820, face_up, defence: false, face: None }
    }

    /// The overview camera (500, 90 deg, 33.75 deg).
    fn overview() -> ([f64; 3], [f64; 3]) {
        let pitch = f64::from(0x17FF) * std::f64::consts::PI / 32768.0;
        ([0.0, 500.0 * pitch.sin(), 500.0 * pitch.cos()], [0.0; 3])
    }

    #[allow(clippy::type_complexity)]
    fn run(s: &mut MonsterShow) -> Vec<(u32, Option<u16>, Option<CameraStep>, Vec<(u8, u8)>, u8)> {
        let mut log = Vec::new();
        s.view = Some(overview());
        while !s.done() {
            s.tick();
            log.push((s.e().unwrap(), s.box_text(), s.camera(), s.sounds(), s.light()));
        }
        log
    }

    #[test]
    fn duster_follows_handler_0x25a080() {
        let targets = vec![at_cell((1, 6), (3, 0), false), at_cell((1, 7), (5, 4), false)];
        let mut s = MonsterShow::new(Effect::Duster, input(), None, None, targets.clone());
        s.card_cell = Some((3, 4));
        let mut log = Vec::new();
        s.view = Some(overview());
        while !s.done() {
            s.tick();
            log.push((s.camera(), s.sounds(), s.c507.len(), s.c603s.len(), s.new_hidden((1, 6)), s.tint_cells().len(), s.box_text(), s.tris([0.0; 3], [0.0; 3]).len(), s.light(), s.others_level()));
        }
        let at = |e: u32| log[(e - 1) as usize].clone();
        assert_eq!(log.len(), 98);
        assert_eq!((at(1).0, at(79).0), (Some(CameraStep::Overview(30)), Some(CameraStep::Back(16))));
        // c507 on every target at E+1, its SE 0x09 at u 17 / 28 / 34 / 41.
        assert_eq!((at(1).2, at(2).2), (0, 2));
        assert_eq!((at(18).1.clone(), at(29).1.clone(), at(42).1.clone()), (vec![(1, 0x09)], vec![(1, 0x09)], vec![(1, 0x09)]));
        // E+18: c603 param 1 on each (SE 0x40 per target); the cards go.
        assert_eq!((at(18).3, at(19).3, at(19).1.clone()), (0, 2, vec![(1, 0x40), (1, 0x40)]));
        assert_eq!((at(18).4, at(19).4), (false, true));
        // The row and the column of the card's cell, until the removal.
        assert_eq!((at(1).5, at(78).5, at(79).5), (13, 13, 0));
        assert!(log.iter().all(|l| l.6.is_none() && l.8 == 128 && l.9 == 128));
        // Two feathers (one quad each) and the dust puffs.
        assert!(at(20).7 >= 4, "feathers draw {}", at(20).7);
        assert!(!s.tint_tris([0.0; 3], [0.0; 3]).is_empty() || s.tint_cells().is_empty());
    }

    #[test]
    fn fake_trap_reaction_follows_event_7_stages_0x20() {
        let fake = at_cell((0, 8), (1, 6), true);
        let mut s = MonsterShow::new(Effect::FakeTrap, input(), None, None, vec![fake]);
        s.trap_was_down = true;
        let log = run(&mut s);
        let at = |e: u32| log[(e - 1) as usize].clone();
        assert_eq!(log.len(), 120);
        // The cut to the centre cell (the swapped argument), 180 away.
        assert_eq!(at(1).2, Some(CameraStep::Cell(Target { unit: (0, 0), cell: (3, 3), card: 0, face_up: false, defence: false, face: None }, 0, 180)));
        assert_eq!((at(8).1, at(9).1, at(38).1, at(39).1), (None, Some(323 + 820), Some(323 + 820), None));
        assert_eq!((at(9).3.clone(), at(40).3.clone()), (vec![(1, 0x3F)], vec![(1, 0x40)]));
        assert_eq!(at(101).2, Some(CameraStep::Back(16)));
        s.t = 2;
        assert_eq!(s.trap_reveal(), Some(((0, 8), Some(0x20), false)));
        s.t = 9;
        assert_eq!(s.trap_reveal(), Some(((0, 8), None, true)));
        s.t = 40;
        assert!(s.new_hidden((0, 8)));
    }

    #[test]
    fn raigeki_bolts_every_12_updates_from_the_card_cell() {
        // Card at (3, 4): K = 4, 110 U, destruction at E+85.
        let struck = at_cell((1, 1), (3, 3), true);
        let far = at_cell((1, 2), (3, 0), true);
        let mut s = MonsterShow::new(Effect::Raigeki, input(), None, None, vec![struck, far]);
        s.card_cell = Some((3, 4));
        assert_eq!((s.raigeki_k(), s.length(), s.apply_at()), (4, 110, 86));
        let launches = s.bolt_launches();
        assert_eq!(launches[0], (2, (3, 4)));
        assert_eq!(launches.iter().filter(|b| b.0 == 14).map(|b| b.1).collect::<Vec<_>>(), vec![(2, 4), (3, 3), (4, 4), (3, 5)]);
        assert_eq!(launches.iter().filter(|b| b.0 == 50).map(|b| b.1).collect::<Vec<_>>(), vec![(3, 0)]);
        let mut log = Vec::new();
        s.view = Some(overview());
        while !s.done() {
            s.tick();
            log.push((s.camera(), s.sounds(), s.struck_level(&struck), s.struck_level(&far), s.new_hidden((1, 1)), s.c505.len(), s.flash_tris([0.0; 3], [0.0; 3]).len()));
        }
        let at = |e: u32| log[(e - 1) as usize].clone();
        assert_eq!(log.len(), 111);
        assert_eq!(at(1).0, Some(CameraStep::Cell(Target { unit: (0, 0), cell: (3, 4), card: 0, face_up: false, defence: false, face: None }, 0, 200)));
        assert_eq!((at(19).0, at(86).0), (Some(CameraStep::Overview(30)), Some(CameraStep::Back(16))));
        assert_eq!((at(2).1.clone(), at(14).1.len()), (vec![(1, 0x07)], 4));
        // Struck at E+13 (ring 1): fades from E+19; ring 4's (3, 0) at E+55.
        assert_eq!((at(19).2, at(20).2, at(35).2), (None, Some(120), Some(0)));
        assert_eq!((at(55).3, at(56).3), (None, Some(120)));
        assert_eq!((at(85).4, at(86).4), (false, true));
        // c505's flashes at its u 2, 4, 6.
        assert_eq!((at(3).6, at(4).6, at(5).6), (2, 0, 2));
        assert_eq!(at(2).5, 1);
    }

    #[test]
    fn anti_raigeki_reaction_bolts_its_own_cell() {
        let anti = at_cell((0, 9), (5, 6), true);
        let mut s = MonsterShow::new(Effect::AntiRaigeki, input(), None, None, vec![anti]);
        s.card_cell = Some((2, 1));
        s.trap_was_down = true;
        let log = run(&mut s);
        let at = |e: u32| log[(e - 1) as usize].clone();
        assert_eq!(log.len(), 114);
        assert_eq!((at(1).2, at(39).2, at(95).2), (Some(CameraStep::Cell(anti, 0, 180)), Some(CameraStep::Cell(anti, 30, 500)), Some(CameraStep::Back(16))));
        assert_eq!((at(9).1, at(38).1, at(39).1, at(40).1, at(94).1, at(95).1), (Some(0x104), Some(0x104), None, Some(323 + 820), Some(323 + 820), None));
        assert_eq!((at(9).3.clone(), at(40).3.clone(), at(46).3.clone()), (vec![(1, 0x3F)], vec![(1, 0x07)], vec![(1, 0x07)]));
        assert_eq!(log.iter().filter(|l| l.3.contains(&(1, 0x07))).count(), 10);
        assert!(!s.new_hidden((0, 9)));
    }

    #[test]
    fn lawbook_runs_c533_on_the_board_light() {
        let mut s = MonsterShow::new(Effect::Lawbook, input(), None, None, Vec::new());
        let mut log = Vec::new();
        while !s.done() {
            s.tick();
            log.push((s.light(), s.sounds(), s.camera(), s.box_text(), s.top_tris().len(), s.applied()));
        }
        let at = |e: u32| log[(e - 1) as usize].clone();
        assert_eq!(log.len(), 96);
        // c533's light: 128 - 8 (u - 1) to 32 at u 13, back to 128 at u 93.
        assert_eq!((at(2).0, at(14).0, at(83).0, at(94).0, at(95).0), (128, 32, 40, 128, 128));
        assert_eq!((at(2).1.clone(), at(21).1.clone(), at(78).1.clone()), (vec![(1, 0x29)], vec![(1, 0x2A)], vec![(1, 0x2B)]));
        assert!(log.iter().all(|l| l.2.is_none() && l.3.is_none()));
        assert_eq!((at(2).4 / 2, at(19).4 / 2, at(78).4 / 2), (6, 66, 194));
        assert_eq!((at(94).5, at(95).5), (false, true));
    }

    fn dark_hole(reacting: bool) -> MonsterShow {
        let t = |side: u8, slot: u8, cell: (u8, u8)| Target { unit: (side, slot), cell, card: 400 + u16::from(slot), face_up: true, defence: false, face: None };
        let targets = vec![t(0, 1, (2, 6)), t(1, 1, (2, 4)), t(1, 2, (3, 2))];
        let units: Vec<crate::duel_c021::UnitIn> = targets
            .iter()
            .map(|t| crate::duel_c021::UnitIn {
                handle: (u16::from(t.unit.0) << 8) | u16::from(t.unit.1),
                position: crate::duel_c059::cell_centre(t.cell),
                rotation: [0x4000, 0, 0x8000],
                colour: [128; 4],
                card: true,
                model: true,
            })
            .collect();
        let mask = targets.iter().fold(0, |m, t| m | crate::duel_c021::mask_bit(t.unit.0, t.unit.1));
        let mut s = MonsterShow::new(Effect::DarkHole, input(), None, None, targets);
        if reacting {
            let white = Target { unit: (1, 6), cell: (5, 1), card: 824, face_up: true, defence: false, face: None };
            s.reacting = vec![white, white];
            s.trap_was_down = true;
        }
        s.c021_input = Some((mask, units, 0x2B));
        s
    }

    #[test]
    fn dark_hole_runs_c021_after_c601_and_ends_when_it_is_seen() {
        let mut s = dark_hole(false);
        let mut log = Vec::new();
        while !s.done() {
            s.tick();
            log.push((s.e().unwrap(), s.light(), s.sounds(), s.camera(), s.box_text(), s.dark_hole_removed(), s.dark_hole_cards().len()));
        }
        let l = s.c021_len().expect("c021 ended");
        let at = |e: u32| log[(e - 1) as usize].clone();
        // c021 at E+1 = e 2: its light 116 and the overview(20) [emu D1].
        assert_eq!((at(1).1, at(2).1, at(8).1), (128, 116, 44));
        assert_eq!((at(1).3, at(2).3), (None, Some(CameraStep::Overview(20))));
        // 0x2622B0(60, 0x3FFF) at E+23 and every 60 U [emu D1].
        assert_eq!((at(24).3, at(84).3), (Some(CameraStep::Orbit(60, 0x3FFF)), Some(CameraStep::Orbit(60, 0x3FFF))));
        // SE 0x49 at E+2 and every 28 U while c021 runs.
        assert_eq!((at(3).2.clone(), at(31).2.clone(), at(4).2.clone()), (vec![(1, 0x49)], vec![(1, 0x49)], vec![]));
        // Seen at E+1+L: the targets removed and the camera back; return
        // at E+L+18: length 1 + L + 18 (the log holds the tick past it).
        let seen = 1 + l + 1;
        assert_eq!(at(seen).3, Some(CameraStep::Back(16)));
        assert!(at(seen).5 && !at(seen - 1).5);
        assert_eq!((log.len() as u32, s.length()), (1 + l + 19, 1 + l + 18));
        assert!(at(seen - 1).6 == 3 && at(seen).6 == 0);
        assert!(log.iter().all(|x| x.4.is_none()), "no box without a reaction");
        assert_eq!(at(seen + 1).1, 128);
    }

    #[test]
    fn dark_hole_shows_a_reacting_white_hole_first() {
        let mut s = dark_hole(true);
        let mut log = Vec::new();
        while !s.done() {
            s.tick();
            log.push((s.e().unwrap(), s.light(), s.sounds(), s.camera(), s.box_text(), s.trap_reveal(), s.c602s.len()));
        }
        let at = |e: u32| log[(e - 1) as usize].clone();
        // The name box (323 + 824) from E to the close at E+38.
        assert_eq!((at(1).4, at(38).4, at(39).4), (Some(323 + 824), Some(323 + 824), None));
        // The cut to the trap at E+1, c602 x 2 (both searches) at E+8.
        assert!(matches!(at(2).3, Some(CameraStep::Cell(t, 0, 180)) if t.card == 824));
        assert_eq!((at(9).2.clone(), at(9).6), (vec![(1, 0x3F), (1, 0x3F)], 2));
        assert_eq!(at(5).5.map(|r| r.2), Some(false));
        assert_eq!(at(9).5.map(|r| r.2), Some(true));
        // c021 at E+39 = e 40.
        assert_eq!((at(39).1, at(40).1, at(40).3), (128, 116, Some(CameraStep::Overview(20))));
        assert_eq!(at(41).2, vec![(1, 0x49)]);
        assert_eq!(log.len() as u32, 39 + s.c021_len().unwrap() + 19);
    }
}
