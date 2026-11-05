//! Card activations as the original presents them
//! (the project notes) [F handlers, emu
//! savestates 688-690 / 722-725 / 257 and the modules run in the
//! interpreter]:
//! - a trap (Block Attack, event 0x46, and the other traps' common part):
//!   camera cut to the trap's cell (0x262410(0, trap, 100, 180)), the trap
//!   turns over (8 updates), message box "Trap activated!" (0x104, y 0x50)
//!   with c602 (30 updates), then the box shows the trap's name with c601
//!   (82 updates), then the trap is removed, the box closes, the camera goes
//!   back over 16 updates and 18 idle updates follow;
//! - a magic card (event 0x65): no camera move, no box; c601 (82 updates).
//!
//! The rules engine has already applied the activation; the board shows the
//! state from before it (`frozen`) until the sequence ends.
//!
//! Drawn: the message box, c602's burst on the trap (`duel_c602`), c601's
//! flying card and its burst, the trap's 8-update flip, c544 on a
//! spellbound victim (with the board light) and c557 with it on a stat
//! loss, c603 + c545 on a destroyed victim, and the trap plans
//! (`TrapPlan`, NEWLY-ALLOWED.md 2, 3, 7, 15.2): Mirror Force (c034 on the
//! defender, c555 on every target, c603 on the victim, the others fading
//! and shrinking), Mirror Wall (c034, c555 + c557 on the attacker, then the
//! battle), Shadow of Eyes (c513 on the mover, its upright turn), Widespread
//! Ruin (c546 on the nine cells, the light, the fades).
use crate::duel_equip::EquipFx;
use crate::duel_overlay::OverlayInput;
use crate::duel_versus::CardPose;

/// The reveal flip 0x252FF0: +0x54 by 0x20 per update to 0xFF [F].
pub const FLIP_UPDATES: u32 = 8;
/// c602's length [emu].
pub const C602_UPDATES: u32 = 30;
/// c601's length with param 1 [emu].
pub const C601_UPDATES: u32 = 82;
/// The camera's way back (0x262390(ctx, 16)) and the idle count after it.
pub const RESTORE_UPDATES: u32 = 16;
pub const IDLE_UPDATES: u32 = 18;
/// Field magic: units fade out 16 U before the scripts launch (the 17th).
pub const FIELD_FADE: u32 = 17;
/// Field magic: the updates after the last script (camera back 30 U, 32
/// idle, the handler's end).
pub const FIELD_TAIL: u32 = 33;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Trap,
    Magic,
}

/// A trap with its own handler steps after c602 (no c601), chosen from the
/// card's trap type (`card_effect_type()`) [F NEWLY-ALLOWED.md 2, 3, 7,
/// 15.2]. `t` below is the activation's update (t = E + 1).
///
/// | trap | c602 param | box | steps | trap removed | done |
/// |---|---|---|---|---|---|
/// | Mirror Force (4) | 1 | name t 39..164 | t 39 cut + c034 (defender); t 82 cut / overview + c555 on every target; t 105 cut + c603 (victim), the others fade and shrink; t 165 back 16 | t 39 | 183 |
/// | Mirror Wall (0x31) | 0 | name t 39..138 | t 39 cut + c034 (defender); t 82 cut + c555 (attacker); t 89 c557 (attacker); t 139 ATK halved, back 16 | never | 157, then the battle |
/// | Shadow of Eyes (0x23) | 0 | name t 39..54 | t 39 cut + c513 (mover); t 55 mover upright (8 U), back 16 | never | 73 |
/// | Widespread Ruin (case 11) | 0 | 0x104 t 9..66 | light t 2..7 down; t 39 16-U move to 500 + c546 on 9 cells (+ c603 p1 on a mover on the trap's cell); SE 0x31 t 40 x1, t 44 x4; t 45 listed units fade; t 61 removed; t 62..67 light up, back 16 at t 67 | t 61 | 99 |
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapPlan {
    MirrorForce,
    MirrorWall,
    ShadowOfEyes,
    WidespreadRuin,
}

impl TrapPlan {
    /// c602's param: 1 fades the card (-4 per update), 0 keeps it.
    pub fn c602_param(self) -> u8 {
        u8::from(self == Self::MirrorForce)
    }
}

/// A unit a trap plan shows (the monster effects' target record).
pub type PlanUnit = crate::duel_monster_fx::Target;

/// The default camera before the board camera's first view [H].
const DEFAULT_VIEW: ([f64; 3], [f64; 3]) = ([0.0, 181.4, 241.4], [0.0, 0.0, 100.0]);

/// How the board shows a unit this update (`Activation::unit_looks`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitLook {
    pub unit: (u8, u8),
    /// Removed (graveyard) or replaced by its c603 copy.
    pub hidden: bool,
    /// 0x2519E0's level (card alpha; with `shrink` also the scales: card
    /// level / 1280, model min(1, 2 level / 128)) [F NEWLY-ALLOWED.md 1.3].
    pub level: Option<u8>,
    pub shrink: bool,
    /// The card's defence flag and its +0x55 turn while it turns upright.
    pub defence: Option<bool>,
    pub sideways: Option<u8>,
}

/// A victim's module after the trap's c601 (kept as a wait).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Victim {
    /// Box text while it plays (0x109 "Spellbound!", 0x108 "Power
    /// decrease!", or the trap's 0x104).
    pub text: u16,
    pub updates: u32,
    /// The victim unit: side, cell, card (the camera's 16-update move to
    /// it, 0x262410(16, victim, 100, 180), and the modules' node).
    pub side: u8,
    pub cell: (u8, u8),
    pub card: u16,
    /// c544 plays (a spellbind); else c603 + c545 (destroyed).
    pub spellbound: bool,
    /// c603 + c545 play (the victim is destroyed).
    pub destroyed: bool,
    /// The camera's frames to the victim: 16 (a move), 0 (Negate Attack's
    /// cut to the attacker).
    pub camera_frames: u32,
    /// c557 plays with c544 (the victim loses ATK / DEF).
    pub lowered: bool,
    /// The camera moves to the victim (not for Remedy's c508).
    pub camera: bool,
    /// Remedy (event 8): c508 with this LP amount, screen anchored.
    pub recovery: Option<i32>,
    /// The victim's card as the board shows it: face up, defence
    /// (sideways), image values.
    pub face_up: bool,
    pub defence: bool,
    pub face: Option<crate::duel_overlay::BoardFace>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Activation {
    pub kind: Kind,
    /// The board before the activation (the trap face down).
    pub frozen: OverlayInput,
    /// The card's unit (side, slot), cell and id.
    pub unit: (u8, u8),
    pub cell: (u8, u8),
    pub card: u16,
    pub victim: Option<Victim>,
    /// Updates played (1 = the handler's init update).
    pub t: u32,
    /// c601 plays after c602 (not for Negate Attack, whose card fades with
    /// c602 param 1 and goes straight to the attacker) [F 3.4].
    pub big: bool,
    /// c601's burst.
    pub fx: Option<EquipFx>,
    /// c602's burst on the trap.
    pub burst: Option<crate::duel_c602::C602>,
    /// c544 on a spellbound victim.
    pub spellbind: Option<crate::duel_c544::C544>,
    /// c557 with c544 when the victim also loses ATK/DEF.
    pub decrease: Option<crate::duel_c557::C557>,
    /// c034, Negate Attack's shield on the attacker.
    pub shield: Option<crate::duel_c034::C034>,
    /// c508, Remedy's LP recovery.
    pub recover: Option<crate::duel_c508::C508>,
    /// Field magic (event 2): the changed cells and their new terrain, the
    /// terrain before, and one c502 per cell once launched.
    pub field: Vec<((u8, u8), u8)>,
    pub field_before: Vec<u8>,
    pub scripts: Vec<crate::duel_c502::C502>,
    /// c603 + c545 on a destroyed victim.
    pub shatter: Option<(crate::duel_victims::C603, crate::duel_victims::C545)>,
    /// The duel before the activation (the HUD's values until it ends).
    pub duel: Option<roses_core::duel::Duel>,
    /// A trap with its own steps after c602 (no c601).
    pub plan: Option<TrapPlan>,
    /// Event 0x36's defender (record +4) and attacker (+8); the mover of
    /// Shadow of Eyes / Widespread Ruin.
    pub defender: Option<PlanUnit>,
    pub attacker: Option<PlanUnit>,
    pub mover: Option<PlanUnit>,
    /// Mirror Force's targets (mask order), Widespread Ruin's listed units.
    pub targets: Vec<PlanUnit>,
    /// Mirror Force's c603 victim (an index of `targets`).
    pub victim_index: usize,
    /// The plans' modules: c555 per target, c513 on the mover, c546 per
    /// cell, c603 on the Mirror Force victim / the Ruin's mover.
    pub c555: Vec<crate::duel_c555::C555>,
    pub c513: Option<crate::duel_monster_mods::C513>,
    pub c546: Vec<crate::duel_c546::C546>,
    pub plan_c603: Option<(crate::duel_victims::C603, PlanUnit)>,
    /// The presentation's random stream (c555 / c546; the victim pick).
    pub rng: crate::duel_fxlib::Rng,
    /// The board camera (eye, target) the world modules draw with.
    pub view: Option<([f64; 3], [f64; 3])>,
    /// The duel the HUD shows once the plan's rule change applied (Mirror
    /// Wall's halved ATK); `None` = the live duel.
    pub applied_duel: Option<roses_core::duel::Duel>,
}

/// The phases of the sequence at update `t`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Flip,
    /// c602, its update.
    Burst(u32),
    /// c601, its update.
    BigCard(u32),
    Victim(u32),
    Restore(u32),
    /// A trap plan's own steps (u = t - 38).
    Plan(u32),
    Done,
}

impl Activation {
    pub fn new(
        kind: Kind,
        frozen: OverlayInput,
        unit: (u8, u8),
        cell: (u8, u8),
        card: u16,
        victim: Option<Victim>,
        duel: Option<roses_core::duel::Duel>,
    ) -> Self {
        Self {
            kind,
            frozen,
            unit,
            cell,
            card,
            victim,
            t: 0,
            big: true,
            fx: None,
            burst: None,
            spellbind: None,
            decrease: None,
            shield: None,
            recover: None,
            field: Vec::new(),
            field_before: Vec::new(),
            scripts: Vec::new(),
            shatter: None,
            duel,
            plan: None,
            defender: None,
            attacker: None,
            mover: None,
            targets: Vec::new(),
            victim_index: 0,
            c555: Vec::new(),
            c513: None,
            c546: Vec::new(),
            plan_c603: None,
            rng: crate::duel_fxlib::Rng::new(0x22B),
            view: None,
            applied_duel: None,
        }
    }

    /// A trap plan's activation (`big` off: no c601). Mirror Force's victim
    /// is drawn here: rand31 % count over the face-up targets, else over all
    /// (the engine does not consume it; the port's stream) [F 2, H stream].
    pub fn with_plan(mut self, plan: TrapPlan, seed: u32) -> Self {
        self.plan = Some(plan);
        self.big = false;
        self.rng = crate::duel_fxlib::Rng::new(seed);
        self
    }

    /// Mirror Force's victim pick (at E+104, 0x2504F0).
    fn pick_victim(&mut self) {
        if self.targets.is_empty() {
            return;
        }
        let up: Vec<usize> = (0..self.targets.len()).filter(|&i| self.targets[i].face_up).collect();
        let pool: Vec<usize> = if up.is_empty() { (0..self.targets.len()).collect() } else { up };
        self.victim_index = pool[self.rng.next() as usize % pool.len()];
    }

    /// The update the plan's handler returns (the phase is `Done` from it).
    fn plan_done(&self, plan: TrapPlan) -> u32 {
        match plan {
            // E+182; an empty mask holds the box 18 U and returns at E+98.
            TrapPlan::MirrorForce if self.targets.is_empty() => 99,
            TrapPlan::MirrorForce => 183,
            TrapPlan::MirrorWall => 157,
            TrapPlan::ShadowOfEyes => 73,
            // c546 ends at u 60 = E+97, the return E+98 [H +-1].
            TrapPlan::WidespreadRuin => 99,
        }
    }

    /// The camera's way back (0x262390(ctx, 16)) of a plan.
    fn plan_back(&self, plan: TrapPlan) -> Option<u32> {
        match plan {
            // No restore for an empty mask: the battle's camera takes over [H].
            TrapPlan::MirrorForce if self.targets.is_empty() => None,
            TrapPlan::MirrorForce => Some(165),
            TrapPlan::MirrorWall => Some(139),
            TrapPlan::ShadowOfEyes => Some(55),
            TrapPlan::WidespreadRuin => Some(67),
        }
    }

    /// c602's param this activation (Negate Attack's 1 without a plan).
    pub fn c602_param(&self) -> u8 {
        self.plan.map_or(u8::from(!self.big), TrapPlan::c602_param)
    }

    fn big_start(&self) -> u32 {
        match self.kind {
            Kind::Trap => FLIP_UPDATES + C602_UPDATES,
            Kind::Magic => 0,
        }
    }

    pub fn phase(&self) -> Phase {
        let t = self.t;
        let big = self.big_start();
        if t == 0 || (self.kind == Kind::Trap && t <= FLIP_UPDATES) {
            return Phase::Flip;
        }
        if t <= big {
            return Phase::Burst(t - FLIP_UPDATES);
        }
        if let Some(plan) = self.plan {
            return if t < self.plan_done(plan) { Phase::Plan(t - big) } else { Phase::Done };
        }
        let big_len = if self.big { C601_UPDATES } else { 0 };
        if t <= big + big_len {
            return Phase::BigCard(t - big);
        }
        let after = big + big_len;
        let victim = self.victim.map_or(0, |v| v.updates);
        if t <= after + victim {
            return Phase::Victim(t - after);
        }
        let rest = match self.kind {
            Kind::Trap => RESTORE_UPDATES.max(IDLE_UPDATES),
            // Field magic: the camera back over 30 U while the units fade
            // in, 32 idle updates [F EFFECT-MODULES-2.md 1.5].
            Kind::Magic if !self.field.is_empty() => FIELD_TAIL,
            Kind::Magic => 1,
        };
        if t <= after + victim + rest {
            return Phase::Restore(t - after - victim);
        }
        Phase::Done
    }

    /// One update; returns the sound cues (bank 1) of the modules'
    /// schedules: c602 0x3F at 1, c601 0x3D at 1 and 0x3E at 30 [emu].
    pub fn tick(&mut self) -> Vec<u8> {
        self.t += 1;
        let mut se = Vec::new();
        if let Phase::Burst(u) = self.phase() {
            if u == 1 {
                let [x, z] = crate::duel_board::cell_center(self.cell.0, self.cell.1);
                self.burst = Some(crate::duel_c602::C602::new([x, 0.0, z], 0x25A + self.t));
            }
            if let Some(b) = &mut self.burst {
                b.update(u);
            }
        } else {
            self.burst = None;
        }
        match (self.phase(), self.victim) {
            (Phase::Victim(u), Some(v)) if v.spellbound && u <= crate::duel_c544::UPDATES => {
                if u == 1 {
                    let [x, z] = crate::duel_board::cell_center(v.cell.0, v.cell.1);
                    self.spellbind = Some(crate::duel_c544::C544::new([x, 0.0, z], 0x220 + self.t));
                    // SECTL.MRG: c544 (1, 0x2F) [F EFFECT-MODULES-2.md 5].
                    se.push(0x2F);
                }
                if let Some(c) = &mut self.spellbind {
                    c.update();
                }
            }
            _ => self.spellbind = None,
        }
        match (self.phase(), self.victim) {
            (Phase::Victim(u), Some(v)) if v.spellbound && v.lowered && u <= crate::duel_c557::UPDATES => {
                if u == 1 {
                    let [x, z] = crate::duel_board::cell_center(v.cell.0, v.cell.1);
                    self.decrease = Some(crate::duel_c557::C557::new([x, 0.0, z], 0x22D + self.t));
                    se.push(0x35);
                }
                if let Some(c) = &mut self.decrease {
                    c.update(u);
                }
            }
            _ if self.plan.is_some() => {}
            _ => self.decrease = None,
        }
        match (self.phase(), self.victim) {
            (Phase::Victim(u), Some(v)) if v.destroyed && u <= crate::duel_victims::C603_UPDATES => {
                if u == 1 {
                    let [x, z] = crate::duel_board::cell_center(v.cell.0, v.cell.1);
                    // The card object's rotation (0x4000, Y, Z) [emu
                    // FLIP-AND-VICTIMS.md 1.1/1.3].
                    let y = crate::duel_play::side_model_base(v.side) - if v.defence { 0x4000 } else { 0 };
                    let rot = [0x4000, y, if v.face_up { 0x8000 } else { 0 }];
                    self.shatter = Some((
                        crate::duel_victims::C603::new([x, 0.0, z], rot, 0x25B + self.t),
                        crate::duel_victims::C545::new([x, 0.0, z], 0x221 + self.t),
                    ));
                    // c603 (1, 0x40), c545 (1, 0x30).
                    se.extend([0x40, 0x30]);
                }
                if let Some((c603, c545)) = &mut self.shatter {
                    c603.update(u);
                    if u <= crate::duel_victims::C545_UPDATES {
                        c545.update(u);
                    }
                }
            }
            _ => self.shatter = None,
        }
        match self.field_u() {
            Some(u) if u <= crate::duel_c502::UPDATES => {
                if u == 1 {
                    self.scripts = self
                        .field
                        .iter()
                        .enumerate()
                        .map(|(n, &(cell, t))| crate::duel_c502::C502::new(cell, t, n == 0, 0x1F6 + n as u32))
                        .collect();
                }
                for s in &mut self.scripts {
                    s.update(u);
                }
                // SECTL: c502 (1, 0x03) at 1, 0x04 at 35, 0x05 at 100 (per
                // script; one cue here [H mixer]).
                match u {
                    1 => se.push(0x03),
                    35 => se.push(0x04),
                    100 => se.push(0x05),
                    _ => {}
                }
            }
            _ => self.scripts.clear(),
        }
        match (self.phase(), self.victim.and_then(|v| v.recovery)) {
            (Phase::Victim(u), Some(amount)) if u <= crate::duel_c508::UPDATES => {
                if u == 1 {
                    self.recover = Some(crate::duel_c508::C508::new(amount, 0x1FC + self.t));
                    se.push(0x0A);
                }
                if u == 42 {
                    se.push(0x0B);
                }
                if let Some(c) = &mut self.recover {
                    c.update(u);
                }
            }
            _ => self.recover = None,
        }
        match (self.phase(), self.victim) {
            (Phase::Victim(u), Some(v)) if !self.big && !v.spellbound && !v.destroyed && u <= crate::duel_c034::UPDATES => {
                if u == 1 {
                    let [x, z] = crate::duel_board::cell_center(v.cell.0, v.cell.1);
                    self.shield = Some(crate::duel_c034::C034::new([x, 0.0, z], v.side));
                    se.push(0x51);
                }
                if let Some(c) = &mut self.shield {
                    c.update(u);
                }
            }
            _ if self.plan.is_some() => {}
            _ => self.shield = None,
        }
        match self.phase() {
            Phase::Burst(1) => se.push(0x3F),
            Phase::BigCard(u) => {
                if u == 1 {
                    se.push(0x3D);
                    self.fx = Some(EquipFx::c601(0x259 + self.t));
                }
                if u == 30 {
                    se.push(0x3E);
                }
                if let Some(fx) = &mut self.fx {
                    fx.update(u);
                }
            }
            _ => self.fx = None,
        }
        if let (Some(plan), Phase::Plan(_)) = (self.plan, self.phase()) {
            se.extend(self.tick_plan(plan));
        }
        se
    }

    fn centre(u: &PlanUnit) -> [f64; 3] {
        let [x, z] = crate::duel_board::cell_center(u.cell.0, u.cell.1);
        [x, 0.0, z]
    }

    /// The card object's rotation (0x4000, Y, Z) for c603 [emu FV 1.1].
    fn card_rot(u: &PlanUnit) -> [i32; 3] {
        let y = crate::duel_play::side_model_base(u.unit.0) - if u.defence { 0x4000 } else { 0 };
        [0x4000, y, if u.face_up { 0x8000 } else { 0 }]
    }

    /// A plan's modules for this update (t >= 39); SE bank 1 ids.
    fn tick_plan(&mut self, plan: TrapPlan) -> Vec<u8> {
        let t = self.t;
        let mut se = Vec::new();
        let (eye, target) = self.view.unwrap_or(DEFAULT_VIEW);
        // c034 on the defender, E+38 .. E+80 (event 0x36, 1.4.1).
        if matches!(plan, TrapPlan::MirrorForce | TrapPlan::MirrorWall) {
            match self.defender {
                Some(d) if (39..39 + crate::duel_c034::UPDATES).contains(&t) => {
                    if t == 39 {
                        self.shield = Some(crate::duel_c034::C034::new(Self::centre(&d), d.unit.0));
                        se.push(0x51);
                    }
                    if let Some(c) = &mut self.shield {
                        c.update(t - 38);
                    }
                }
                _ => self.shield = None,
            }
            // c555 at E+81 on every target (Mirror Wall: the attacker).
            if t == 82 {
                let on: Vec<PlanUnit> = match plan {
                    TrapPlan::MirrorWall => self.attacker.into_iter().collect(),
                    _ => self.targets.clone(),
                };
                let rng = &mut self.rng;
                self.c555 = on
                    .iter()
                    .map(|u| {
                        let [x, z] = crate::duel_board::cell_center(u.cell.0, u.cell.1);
                        crate::duel_c555::C555::new([x as f32, 0.0, z as f32], &mut || rng.next())
                    })
                    .collect();
                // SE (1, 0x52) per copy.
                se.extend(std::iter::repeat_n(crate::duel_c555::SE_ID as u8, self.c555.len()));
            }
            if (82..82 + crate::duel_c555::UPDATES).contains(&t) {
                let rng = &mut self.rng;
                for c in &mut self.c555 {
                    c.update(t - 81, &mut || rng.next(), eye, target);
                }
            } else {
                self.c555.clear();
            }
        }
        match plan {
            TrapPlan::MirrorForce => {
                // E+104: the victim's c603 (60 U).
                if t == 105 && !self.targets.is_empty() {
                    self.pick_victim();
                    let v = self.targets[self.victim_index];
                    self.plan_c603 = Some((crate::duel_victims::C603::new(Self::centre(&v), Self::card_rot(&v), 0x25B + t), v));
                    se.push(0x40);
                }
                if (105..165).contains(&t) {
                    if let Some((c, _)) = &mut self.plan_c603 {
                        c.update(t - 104);
                    }
                } else {
                    self.plan_c603 = None;
                }
            }
            TrapPlan::MirrorWall => {
                // c555's 2 (u 7) seen at E+88: c557 on the attacker (50 U).
                match self.attacker {
                    Some(a) if (89..89 + crate::duel_c557::UPDATES).contains(&t) => {
                        if t == 89 {
                            self.decrease = Some(crate::duel_c557::C557::new(Self::centre(&a), 0x22D + t));
                            se.push(0x35);
                        }
                        if let Some(c) = &mut self.decrease {
                            c.update(t - 88);
                        }
                    }
                    _ => self.decrease = None,
                }
            }
            TrapPlan::ShadowOfEyes => match self.mover {
                // E+38: c513 on the mover (16 U, SE 0x10).
                Some(m) if (39..39 + crate::duel_monster_mods::C513::UPDATES).contains(&t) => {
                    if t == 39 {
                        self.c513 = Some(crate::duel_monster_mods::C513::new(Self::centre(&m), 0x201 + t));
                        se.push(0x10);
                    }
                    if let Some(c) = &mut self.c513 {
                        c.update();
                    }
                }
                _ => self.c513 = None,
            },
            TrapPlan::WidespreadRuin => {
                // E+38: c546 on the nine cells (the trap's with param 1).
                if t == 39 {
                    let rng = &mut self.rng;
                    self.c546 = crate::duel_c546::launches(self.cell)
                        .into_iter()
                        .map(|(cell, param)| crate::duel_c546::C546::new(cell, param, &mut || rng.next()))
                        .collect();
                    // A mover on the trap's cell is not listed: c603 param 1.
                    if let Some(m) = self.mover.filter(|m| m.cell == self.cell) {
                        self.plan_c603 = Some((crate::duel_victims::C603::with_param(Self::centre(&m), Self::card_rot(&m), 0x25B + t, 1), m));
                    }
                }
                if (39..39 + crate::duel_c546::UPDATES).contains(&t) {
                    let rng = &mut self.rng;
                    for c in &mut self.c546 {
                        c.update(t - 38, &mut || rng.next(), eye, target);
                    }
                } else {
                    self.c546.clear();
                }
                if (39..39 + crate::duel_victims::C603_UPDATES).contains(&t) {
                    if let Some((c, _)) = &mut self.plan_c603 {
                        c.update(t - 38);
                    }
                } else {
                    self.plan_c603 = None;
                }
                // The handler's own SE (1, 0x31): once at its counter 1,
                // four times at 5 [F 0x26775C..0x267798].
                match t {
                    40 => se.push(crate::duel_c546::HANDLER_SE as u8),
                    44 => se.extend([crate::duel_c546::HANDLER_SE as u8; 4]),
                    _ => {}
                }
            }
        }
        se
    }

    /// The camera step a plan starts this update (`Unit` = 0x262410(frames,
    /// unit, 100, 180), `Overview(0)` = 0x262060(0), `Cell(_, 16, 500)` =
    /// 0x262410(16, trap, 500, 500), `Back` = 0x262390).
    pub fn plan_camera(&self) -> Option<crate::duel_monster_fx::CameraStep> {
        use crate::duel_monster_fx::CameraStep;
        let plan = self.plan?;
        let t = self.t;
        if !matches!(self.phase(), Phase::Plan(_)) {
            return None;
        }
        if Some(t) == self.plan_back(plan) {
            return Some(CameraStep::Back(RESTORE_UPDATES));
        }
        match (plan, t) {
            (TrapPlan::MirrorForce | TrapPlan::MirrorWall, 39) => self.defender.map(|d| CameraStep::Unit(d, 0)),
            (TrapPlan::MirrorForce, 82) => match self.targets.len() {
                0 => None,
                1 => Some(CameraStep::Unit(self.targets[0], 0)),
                _ => Some(CameraStep::Overview(0)),
            },
            (TrapPlan::MirrorForce, 105) => self.targets.get(self.victim_index).map(|v| CameraStep::Unit(*v, 0)),
            (TrapPlan::MirrorWall, 82) => self.attacker.map(|a| CameraStep::Unit(a, 0)),
            (TrapPlan::ShadowOfEyes, 39) => self.mover.map(|m| CameraStep::Unit(m, 0)),
            (TrapPlan::WidespreadRuin, 39) => Some(CameraStep::Cell(
                PlanUnit { unit: self.unit, cell: self.cell, card: self.card, face_up: false, defence: false, face: None },
                16,
                500,
            )),
            _ => None,
        }
    }

    /// The units a plan changes on the frozen board this update.
    pub fn unit_looks(&self) -> Vec<UnitLook> {
        let Some(plan) = self.plan else {
            return Vec::new();
        };
        let t = self.t;
        let look = |u: &PlanUnit| UnitLook { unit: u.unit, hidden: false, level: None, shrink: false, defence: None, sideways: None };
        let mut out = Vec::new();
        match plan {
            TrapPlan::MirrorForce => {
                for (i, u) in self.targets.iter().enumerate() {
                    let mut l = look(u);
                    if t >= 165 || (t >= 105 && i == self.victim_index) {
                        // Every target to the graveyard at E+164; the victim's
                        // card replaced by its c603 from E+104.
                        l.hidden = true;
                    } else if t >= 105 {
                        // 0x2519E0(g, s, -1, 7, 3): -8 per update from 128,
                        // fading and shrinking.
                        l.level = Some(128u32.saturating_sub(8 * (t - 104)) as u8);
                        l.shrink = true;
                    }
                    if l.hidden || l.level.is_some() {
                        out.push(l);
                    }
                }
            }
            TrapPlan::MirrorWall => {}
            TrapPlan::ShadowOfEyes => {
                if let Some(m) = &self.mover {
                    let mut l = look(m);
                    if t < 55 {
                        // The frozen board keeps the requested defence.
                        l.defence = Some(true);
                    } else {
                        // Bits 3 / 5 cleared at E+54: +0x55 -0x20 per update
                        // (8 U) [H count, FV 1.3].
                        l.defence = Some(false);
                        let p = 0xFFi32 - 0x20 * (t as i32 - 54);
                        l.sideways = (p > 0).then_some(p as u8);
                    }
                    out.push(l);
                }
            }
            TrapPlan::WidespreadRuin => {
                let on_trap = self.mover.filter(|m| m.cell == self.cell);
                for u in &self.targets {
                    let mut l = look(u);
                    if t >= 61 {
                        l.hidden = true;
                    } else if t >= 45 {
                        // 0x2519E0(g, s, -1, 1, 3): fade only.
                        l.level = Some(128u32.saturating_sub(8 * (t - 44)) as u8);
                    }
                    if l.hidden || l.level.is_some() {
                        out.push(l);
                    }
                }
                if let Some(m) = &self.mover {
                    if (on_trap.is_some() && t >= 39) || t >= 61 {
                        out.push(UnitLook { hidden: true, ..look(m) });
                    }
                }
            }
        }
        out
    }

    /// The trap's own card level during a plan (Widespread Ruin fades it
    /// with the listed units from E+44).
    pub fn plan_card_level(&self) -> Option<u8> {
        match (self.plan, self.t) {
            (Some(TrapPlan::WidespreadRuin), t @ 45..) => Some(128u32.saturating_sub(8 * (t - 44)) as u8),
            _ => None,
        }
    }

    /// The HUD hides while a trap plays, until the camera's way back
    /// (689 / 690 [emu]; the slide moments H).
    pub fn hides_hud(&self) -> bool {
        if self.kind != Kind::Trap {
            return false;
        }
        match self.plan {
            Some(plan) => self.phase() != Phase::Done && self.plan_back(plan).is_none_or(|b| self.t < b),
            None => !matches!(self.phase(), Phase::Restore(_) | Phase::Done),
        }
    }

    /// The duel the HUD shows: the one before the activation until the
    /// plan's rule change (Mirror Wall's halved ATK at E+138, Mirror Force's
    /// removal at E+164, the Ruin's at ~E+60, Shadow of Eyes' turn at
    /// E+54), then `applied_duel` (`None`: the live duel).
    pub fn shown_duel(&self) -> Option<&roses_core::duel::Duel> {
        let applied = match self.plan {
            Some(TrapPlan::MirrorForce) => 165,
            Some(TrapPlan::MirrorWall) => 139,
            Some(TrapPlan::ShadowOfEyes) => 55,
            Some(TrapPlan::WidespreadRuin) => 61,
            None => return self.duel.as_ref(),
        };
        if self.t >= applied { self.applied_duel.as_ref() } else { self.duel.as_ref() }
    }

    /// A plan's world modules: [additive, subtractive (0x42), Cs * As
    /// (0x88), alpha blend (0x44), full-screen flashes (c555's arrivals,
    /// over the HUD)].
    pub fn plan_tris(&self, eye: [f64; 3], target: [f64; 3]) -> [Vec<crate::duel_c621::FxTri>; 5] {
        let mut out: [Vec<crate::duel_c621::FxTri>; 5] = Default::default();
        for c in &self.c555 {
            out[0].extend_from_slice(&c.additive);
            out[1].extend_from_slice(&c.subtractive);
            out[2].extend_from_slice(&c.opaque);
            out[3].extend_from_slice(&c.alpha_blend);
            out[4].extend_from_slice(&c.flash);
        }
        for c in &self.c546 {
            out[0].extend_from_slice(&c.additive);
            out[1].extend_from_slice(&c.subtractive);
            out[2].extend_from_slice(&c.opaque);
            out[3].extend_from_slice(&c.alpha_blend);
        }
        if let Some(c) = &self.c513 {
            out[0].extend(c.draw(eye, target));
        }
        out
    }

    /// The plan's light 0x2DF5B1 (Widespread Ruin: -15 per update to 48 from
    /// E+1, +15 from ~E+61 to 128 at ~E+66) [F 7].
    fn plan_light(&self) -> Option<u8> {
        if self.plan != Some(TrapPlan::WidespreadRuin) {
            return None;
        }
        let t = self.t;
        Some(if t <= 61 { (128 - 15 * (t.saturating_sub(1)).min(6) as i32).max(48) as u8 } else { (48 + 15 * (t - 61)).min(128) as u8 })
    }

    pub fn done(&self) -> bool {
        self.phase() == Phase::Done
    }

    /// The message box text this update (kind 0 at y 0x50).
    pub fn box_text(&self) -> Option<u16> {
        if !self.field.is_empty() {
            return None;
        }
        if self.kind != Kind::Trap {
            return match self.phase() {
                Phase::Victim(_) => self.victim.map(|v| v.text),
                _ => None,
            };
        }
        if let (Some(plan), Phase::Plan(_)) = (self.plan, self.phase()) {
            // The card's name from E+38 until the box closes (Widespread
            // Ruin keeps "Trap activated!" until ~E+65) [F 2, 3, 7].
            let t = self.t;
            let close = match plan {
                TrapPlan::MirrorForce if self.targets.is_empty() => 99,
                TrapPlan::MirrorForce => 165,
                TrapPlan::MirrorWall => 139,
                TrapPlan::ShadowOfEyes => 55,
                TrapPlan::WidespreadRuin => 67,
            };
            return (t < close).then_some(if plan == TrapPlan::WidespreadRuin { 0x104 } else { 323 + self.card });
        }
        match self.phase() {
            Phase::Burst(_) => Some(0x104),
            Phase::BigCard(_) => Some(323 + self.card),
            Phase::Victim(_) => self.victim.map(|v| v.text),
            _ => None,
        }
    }

    /// The card shows face up (the flip has ended).
    pub fn revealed(&self) -> bool {
        self.kind == Kind::Magic || self.t >= FLIP_UPDATES
    }

    /// The trap's +0x54 while it turns over (0x20 per update from the
    /// update after the handler's init) [F/emu FLIP-AND-VICTIMS.md 1.1].
    pub fn flip(&self) -> Option<u8> {
        (self.kind == Kind::Trap && (1..FLIP_UPDATES).contains(&self.t)).then(|| (0x20 * self.t) as u8)
    }

    /// The card's alpha during c602 param 1 (Negate Attack: -4 per update,
    /// 128 -> 8) [F/emu C602-BURST.md 4; the one-update offset H].
    pub fn card_alpha(&self) -> Option<u8> {
        match self.phase() {
            Phase::Burst(u) if self.c602_param() == 1 => Some((128 - 4 * (u - 1).min(30)) as u8),
            _ => None,
        }
    }

    /// The board card is hidden: c601 hides it, and a trap is removed after.
    pub fn board_card_hidden(&self) -> bool {
        match self.plan {
            // Mirror Force's trap goes at E+38, the Ruin's with its listed
            // units (~E+60); Mirror Wall and Shadow of Eyes stay face up.
            Some(TrapPlan::MirrorForce) => return self.t >= 39,
            Some(TrapPlan::WidespreadRuin) => return self.t >= 61,
            Some(_) => return false,
            None => {}
        }
        !matches!(self.phase(), Phase::Flip | Phase::Burst(_))
    }

    /// The victim's camera move starts (0x262410(16, victim, 100, 180)).
    pub fn victim_camera_starts(&self) -> Option<Victim> {
        (self.phase() == Phase::Victim(1)).then_some(self.victim).flatten().filter(|v| v.camera)
    }

    /// The board light 0x2DF5B1 (c544 dims the board), 128 = full.
    pub fn light(&self) -> u8 {
        self.plan_light().unwrap_or_else(|| self.spellbind.as_ref().map_or(128, |c| c.light_out))
    }

    /// The camera goes back (the update after c601 / the victim's module).
    pub fn restore_starts(&self) -> bool {
        (self.kind == Kind::Trap || !self.field.is_empty()) && self.phase() == Phase::Restore(1)
    }

    /// The camera's way back (16 U, field magic 30 U).
    pub fn restore_frames(&self) -> u32 {
        if self.field.is_empty() { RESTORE_UPDATES } else { 30 }
    }

    /// Field magic's camera starts (the update after c601): the number of
    /// changed cells (1: a cut to it, else the overview track).
    pub fn field_camera_starts(&self) -> Option<usize> {
        (!self.field.is_empty() && self.phase() == Phase::Victim(1)).then_some(self.field.len())
    }

    /// Field magic's script update (c502 u) this update.
    fn field_u(&self) -> Option<u32> {
        match self.phase() {
            Phase::Victim(u) if !self.field.is_empty() && u > FIELD_FADE - 1 => Some(u - (FIELD_FADE - 1)),
            _ => None,
        }
    }

    /// Field magic's unit level (cards and models): -8 per update to 0
    /// before the scripts, +8 from the camera's way back [F 1.5].
    pub fn unit_level(&self) -> Option<u8> {
        if self.field.is_empty() {
            return None;
        }
        match self.phase() {
            Phase::Victim(u) => Some(128u32.saturating_sub(8 * u) as u8),
            Phase::Restore(r) => Some((8 * r).min(128) as u8),
            _ => None,
        }
    }

    /// The terrain the board shows during field magic (the new byte from
    /// c502's u 68) and the cells' heights (cell +0x14).
    pub fn field_board(&self) -> Option<(Vec<u8>, [i16; 49])> {
        if self.field.is_empty() || self.field_before.len() != 49 {
            return None;
        }
        let mut terrain = self.field_before.clone();
        let mut heights = [0i16; 49];
        let u = self.field_u();
        let after = matches!(self.phase(), Phase::Restore(_) | Phase::Done);
        for &((c, r), t) in &self.field {
            let i = usize::from(r) * 7 + usize::from(c);
            if after || u.is_some_and(|u| u >= crate::duel_c502::TERRAIN_WRITE) {
                terrain[i] = t;
            }
            if let Some(u) = u {
                heights[i] = crate::duel_c502::height(u);
            }
        }
        Some((terrain, heights))
    }

    /// c502's frames / walls (additive, subtractive) and the flash.
    pub fn field_tris(&self, eye: [f64; 3], target: [f64; 3]) -> (Vec<crate::duel_c621::FxTri>, Vec<crate::duel_c621::FxTri>, Vec<crate::duel_c621::FxTri>) {
        let (mut add, mut sub, mut flash) = (Vec::new(), Vec::new(), Vec::new());
        for s in &self.scripts {
            let (tris, subtract) = s.draw(eye, target);
            if subtract { sub.extend(tris) } else { add.extend(tris) }
            flash.extend(s.flash());
        }
        (add, sub, flash)
    }

    /// c601's copy this update: (pose fraction k / 20 of the flight, scale,
    /// alpha) [emu 2.6]: flight u 1..20 (scale 0.1 + 0.045 u), hold, from
    /// u 66 scale +0.1 and alpha -8 per update.
    pub fn big_card(&self) -> Option<(f32, f32, u8)> {
        let Phase::BigCard(u) = self.phase() else {
            return None;
        };
        let k = (u.min(20) as f32) / 20.0;
        let (scale, alpha) = if u >= 66 {
            (1.0 + 0.1 * (u - 65) as f32, 128u32.saturating_sub(8 * (u - 65)) as u8)
        } else {
            (0.1 + 0.9 * k, 128)
        };
        Some((k, scale, alpha))
    }

    /// The victim is hidden (c603 replaces its card) [F 3: board card alpha 0].
    pub fn victim_hidden(&self) -> Option<(u8, (u8, u8))> {
        let v = self.victim?;
        (v.destroyed && matches!(self.phase(), Phase::Victim(_) | Phase::Restore(_) | Phase::Done)).then_some((v.side, v.cell))
    }

    /// c557's (additive, subtractive) triangles while it plays.
    pub fn decrease_tris(&self, eye: [f64; 3], target: [f64; 3]) -> (Vec<crate::duel_c621::FxTri>, Vec<crate::duel_c621::FxTri>) {
        self.decrease.as_ref().map_or_else(|| (Vec::new(), Vec::new()), |c| c.draw(eye, target))
    }

    /// c508's triangles while it plays.
    pub fn recovery_tris(&self) -> &[crate::duel_c621::FxTri] {
        self.recover.as_ref().map_or(&[], |c| &c.tris)
    }

    /// c034's triangles while it plays.
    pub fn shield_tris(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<crate::duel_c621::FxTri> {
        self.shield.as_ref().map_or_else(Vec::new, |c| c.draw(eye, target))
    }

    /// c545's triangles while it plays.
    pub fn smoke_tris(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<crate::duel_c621::FxTri> {
        match &self.shatter {
            Some((_, c545)) if matches!(self.phase(), Phase::Victim(u) if u <= crate::duel_victims::C545_UPDATES) => c545.draw(eye, target),
            _ => Vec::new(),
        }
    }

    /// c544's triangles (none when no spellbind plays).
    pub fn spellbind_tris(&self, eye: [f64; 3], target: [f64; 3]) -> Vec<crate::duel_c621::FxTri> {
        self.spellbind.as_ref().map_or_else(Vec::new, |c| c.draw(eye, target))
    }

    /// c601's burst triangles.
    pub fn effects(&self) -> &[crate::duel_c621::FxTri] {
        self.fx.as_ref().map_or(&[], |f| &f.tris)
    }
}

/// c601's copy pose: from the face-up board card (rot (0x4000, side yaw,
/// 0x8000), scale 0.1) to view (0, 0) at depth 800 facing the camera (scale
/// 1.0), rot y +0x1000 per update (1.25 turns over the flight) [emu 2.6;
/// the board card's own rotation H].
pub fn big_card_pose(side: u8, cell: (u8, u8), centre: &CardPose, k: f32, scale: f32) -> CardPose {
    let [x, z] = crate::duel_board::cell_center(cell.0, cell.1);
    let _ = side;
    // Ends at the 0x46 hold's facing (0, 0x10000, 0x10000) after 1.25
    // turns of rot y (+0x1000 per update) [emu; the start's y H].
    let from = CardPose {
        position: [x as f32, 0.0, z as f32],
        spin: [0x4000, 0x10000 - 0x14000, 0x8000],
        tilt: [0, 0, 0],
        scale: [0.1, 0.1],
    };
    let f = |a: f32, b: f32| a * (1.0 - k) + b * k;
    let i = |a: i32, b: i32| (f(a as f32, b as f32) as i32) & 0xFFFF;
    let spin_to = [0, 0x10000, 0x10000];
    CardPose {
        position: std::array::from_fn(|n| f(from.position[n], centre.position[n])),
        spin: std::array::from_fn(|n| i(from.spin[n], spin_to[n])),
        tilt: std::array::from_fn(|n| i(from.tilt[n], centre.tilt[n])),
        scale: [scale, scale * 0.3913 / 0.375],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> OverlayInput {
        serde_json::from_str("{}").unwrap()
    }

    #[test]
    fn trap_sequence_follows_the_handler_timeline() {
        let mut a = Activation::new(Kind::Trap, input(), (1, 6), (2, 1), 0x2F0, None, None);
        let mut log = Vec::new();
        while !a.done() {
            let se = a.tick();
            log.push((a.t, a.phase(), a.box_text(), se));
        }
        // Flip 8, c602 30 (box 0x104), c601 82 (box = name), 18 more.
        assert_eq!(log[8].1, Phase::Burst(1));
        assert_eq!((log[8].2, log[8].3.clone()), (Some(0x104), vec![0x3F]));
        assert_eq!((log[38].1, log[38].2), (Phase::BigCard(1), Some(323 + 0x2F0)));
        assert_eq!(log[38 + 29].3, vec![0x3E]);
        assert_eq!((log[120].1, log[120].2), (Phase::Restore(1), None));
        assert_eq!(log.len() as u32, 8 + 30 + 82 + 18 + 1);
    }

    #[test]
    fn negate_attack_skips_c601_and_waits_c034() {
        let v = Victim { text: 323 + 828, updates: 43, side: 0, cell: (3, 4), card: 1, spellbound: false, destroyed: false, camera_frames: 0, lowered: false, camera: true, recovery: None, face_up: true, defence: false, face: None };
        let mut a = Activation::new(Kind::Trap, input(), (1, 7), (1, 2), 828, Some(v), None);
        a.big = false;
        let mut log = Vec::new();
        while !a.done() {
            a.tick();
            log.push((a.phase(), a.box_text()));
        }
        assert_eq!(log[8], (Phase::Burst(1), Some(0x104)));
        assert_eq!(log[38], (Phase::Victim(1), Some(323 + 828)));
        assert_eq!(log[38 + 43], (Phase::Restore(1), None));
        assert_eq!(log.len(), 8 + 30 + 43 + 18 + 1);
    }

    #[test]
    fn big_card_flies_holds_and_fades() {
        let mut a = Activation::new(Kind::Magic, input(), (0, 6), (3, 5), 691, None, None);
        a.tick();
        assert_eq!(a.big_card(), Some((0.05, 0.1 + 0.9 * 0.05, 128)));
        while a.t < 66 {
            a.tick();
        }
        let (_, s, alpha) = a.big_card().unwrap();
        assert!((s - 1.1).abs() < 1e-6 && alpha == 120);
        while a.t < 81 {
            a.tick();
        }
        assert_eq!(a.big_card().unwrap().2, 0);
        while !a.done() {
            a.tick();
        }
        assert_eq!(a.t, 84);
        assert_eq!(a.box_text(), None);
    }
    fn unit(unit: (u8, u8), cell: (u8, u8), face_up: bool) -> PlanUnit {
        PlanUnit { unit, cell, card: 1, face_up, defence: false, face: None }
    }

    /// (t, phase, box, SE, camera, looks) per update until done.
    #[allow(clippy::type_complexity)]
    fn run(a: &mut Activation) -> Vec<(u32, Phase, Option<u16>, Vec<u8>, Option<crate::duel_monster_fx::CameraStep>, Vec<UnitLook>)> {
        let mut log = Vec::new();
        while !a.done() {
            let se = a.tick();
            log.push((a.t, a.phase(), a.box_text(), se, a.plan_camera(), a.unit_looks()));
        }
        log
    }

    #[test]
    fn mirror_force_plan_follows_handler_0x26cdb0() {
        use crate::duel_monster_fx::CameraStep;
        let defender = unit((1, 2), (3, 2), false);
        let mut a = Activation::new(Kind::Trap, input(), (1, 7), (6, 0), 827, None, None);
        a.defender = Some(defender);
        a.targets = vec![unit((0, 1), (3, 3), false), unit((0, 2), (4, 5), true)];
        let mut a = a.with_plan(TrapPlan::MirrorForce, 5);
        assert!(!a.big);
        let log = run(&mut a);
        let at = |t: u32| log[(t - 1) as usize].clone();
        // 183 U; c602 param 1 fades the card, "Trap activated!" t 9..38.
        assert_eq!(log.len(), 183);
        assert_eq!((at(9).1, at(9).2, at(9).3.clone()), (Phase::Burst(1), Some(0x104), vec![0x3F]));
        // E+38: the name, the trap removed, the cut to the defender, c034.
        assert_eq!((at(39).1, at(39).2, at(39).4), (Phase::Plan(1), Some(323 + 827), Some(CameraStep::Unit(defender, 0))));
        assert_eq!(at(39).3, vec![0x51]);
        // E+81: the overview cut (two targets), c555 on each (SE 0x52 x 2).
        assert_eq!((at(82).4, at(82).3.clone()), (Some(CameraStep::Overview(0)), vec![0x52, 0x52]));
        // E+104: the face-up target is the victim (c603, SE 0x40), the
        // other fades and shrinks -8 per update.
        assert_eq!((at(105).4, at(105).3.clone()), (Some(CameraStep::Unit(a.targets[1], 0)), vec![0x40]));
        assert_eq!(a.victim_index, 1);
        let looks = at(105).5;
        assert!(looks.iter().any(|l| l.unit == (0, 2) && l.hidden));
        assert!(looks.iter().any(|l| l.unit == (0, 1) && l.level == Some(120) && l.shrink));
        assert_eq!(at(120).5.iter().find(|l| l.unit == (0, 1)).and_then(|l| l.level), Some(0));
        // E+164: every target removed, box closed, camera back 16.
        assert_eq!((at(164).2, at(165).2, at(165).4), (Some(323 + 827), None, Some(CameraStep::Back(16))));
        assert!(at(165).5.iter().all(|l| l.hidden) && at(165).5.len() == 2);
    }

    #[test]
    fn mirror_force_plan_draws_c555_and_its_flashes() {
        let mut a = Activation::new(Kind::Trap, input(), (1, 7), (6, 0), 827, None, None);
        a.defender = Some(unit((1, 2), (3, 2), false));
        a.targets = vec![unit((0, 1), (3, 3), true)];
        let mut a = a.with_plan(TrapPlan::MirrorForce, 5);
        // The overview camera (500, 90 deg, 33.75 deg) over the board.
        let pitch = f64::from(0x17FF) * std::f64::consts::PI / 32768.0;
        a.view = Some(([0.0, 500.0 * pitch.sin(), 500.0 * pitch.cos()], [0.0; 3]));
        let mut flashes = Vec::new();
        let mut drawn = 0;
        while !a.done() {
            a.tick();
            let [add, _, _, _, flash] = a.plan_tris([0.0; 3], [0.0; 3]);
            drawn = drawn.max(add.len());
            if !flash.is_empty() {
                flashes.push((a.t, flash.len()));
            }
            if a.t == 82 {
                // One target: a cut to it.
                assert!(matches!(a.plan_camera(), Some(crate::duel_monster_fx::CameraStep::Unit(..))));
            }
        }
        // Four orbs arrive at c555's u 7 = E+87: four full-screen quads.
        assert_eq!(flashes, vec![(88, 8)]);
        assert!(drawn > 100, "c555 draws {drawn}");
    }

    #[test]
    fn mirror_force_with_an_empty_mask_holds_the_box() {
        let mut a = Activation::new(Kind::Trap, input(), (1, 7), (6, 0), 827, None, None);
        a.defender = Some(unit((1, 2), (3, 2), false));
        let mut a = a.with_plan(TrapPlan::MirrorForce, 5);
        let log = run(&mut a);
        // Return at E+98 without a camera restore.
        assert_eq!(log.len(), 99);
        assert_eq!(log[97].2, Some(323 + 827));
        assert!(log.iter().all(|l| !matches!(l.4, Some(crate::duel_monster_fx::CameraStep::Back(_)))));
        assert!(!a.hides_hud());
    }

    #[test]
    fn mirror_wall_plan_halves_at_e138_and_keeps_the_trap() {
        use crate::duel_monster_fx::CameraStep;
        let (defender, attacker) = (unit((1, 2), (3, 2), true), unit((0, 3), (3, 3), true));
        let mut a = Activation::new(Kind::Trap, input(), (1, 7), (6, 0), 829, None, None);
        a.defender = Some(defender);
        a.attacker = Some(attacker);
        let mut a = a.with_plan(TrapPlan::MirrorWall, 3);
        let mut log = Vec::new();
        while !a.done() {
            let se = a.tick();
            log.push((a.phase(), a.box_text(), se, a.plan_camera(), a.card_alpha(), a.board_card_hidden(), a.decrease.is_some(), a.hides_hud()));
        }
        let at = |t: u32| log[(t - 1) as usize].clone();
        assert_eq!(log.len(), 157);
        // c602 param 0: the card keeps its alpha; it never leaves.
        assert_eq!(at(20).4, None);
        assert!(log.iter().all(|l| !l.5));
        assert_eq!((at(39).1, at(39).3), (Some(323 + 829), Some(CameraStep::Unit(defender, 0))));
        assert_eq!((at(82).3, at(82).2.clone()), (Some(CameraStep::Unit(attacker, 0)), vec![0x52]));
        // c555's 2 seen at E+88: c557 on the attacker (SE 0x35, 50 U).
        assert_eq!((at(88).6, at(89).6, at(89).2.clone(), at(138).6, at(139).6), (false, true, vec![0x35], true, false));
        // E+138: box closed, camera back 16, the HUD back.
        assert_eq!((at(138).1, at(139).1, at(139).3), (Some(323 + 829), None, Some(CameraStep::Back(16))));
        assert_eq!((at(138).7, at(139).7), (true, false));
    }

    #[test]
    fn shadow_of_eyes_plan_turns_the_mover_upright_at_e54() {
        use crate::duel_monster_fx::CameraStep;
        let mover = unit((0, 4), (2, 5), true);
        let mut a = Activation::new(Kind::Trap, input(), (1, 8), (2, 3), 818, None, None);
        a.mover = Some(mover);
        let mut a = a.with_plan(TrapPlan::ShadowOfEyes, 1);
        let log = run(&mut a);
        let at = |t: u32| log[(t - 1) as usize].clone();
        assert_eq!(log.len(), 73);
        assert_eq!((at(39).2, at(39).3.clone(), at(39).4), (Some(323 + 818), vec![0x10], Some(CameraStep::Unit(mover, 0))));
        assert_eq!((at(54).2, at(55).2, at(55).4), (Some(323 + 818), None, Some(CameraStep::Back(16))));
        let look = |t: u32| at(t).5[0];
        assert_eq!((look(54).defence, look(54).sideways), (Some(true), None));
        assert_eq!((look(55).defence, look(55).sideways), (Some(false), Some(0xDF)));
        assert_eq!((look(61).sideways, look(62).sideways), (Some(0x1F), None));
        assert!(!a.board_card_hidden());
    }

    #[test]
    fn widespread_ruin_plan_follows_handler_0x2671b0() {
        use crate::duel_monster_fx::CameraStep;
        // The mover attacked the trap's own cell: its c603 param 1.
        let mover = unit((0, 2), (3, 3), true);
        let mut a = Activation::new(Kind::Trap, input(), (1, 6), (3, 3), 812, None, None);
        a.mover = Some(mover);
        a.targets = vec![unit((1, 1), (2, 3), true), unit((0, 3), (4, 4), false)];
        let mut a = a.with_plan(TrapPlan::WidespreadRuin, 2);
        let mut log = Vec::new();
        while !a.done() {
            let se = a.tick();
            log.push((a.light(), a.box_text(), se, a.plan_camera(), a.unit_looks(), a.c546.len(), a.plan_c603.is_some(), a.board_card_hidden(), a.plan_card_level()));
        }
        let at = |t: u32| log[(t - 1) as usize].clone();
        assert_eq!(log.len(), 99);
        assert_eq!((at(1).0, at(2).0, at(7).0, at(61).0, at(62).0, at(67).0), (128, 113, 48, 48, 63, 128));
        assert_eq!((at(9).1, at(66).1, at(67).1), (Some(0x104), Some(0x104), None));
        assert_eq!(at(39).3, Some(CameraStep::Cell(PlanUnit { unit: (1, 6), cell: (3, 3), card: 812, face_up: false, defence: false, face: None }, 16, 500)));
        // Nine cells at E+38; the mover on the trap's cell shatters.
        assert_eq!((at(38).5, at(39).5, at(39).6), (0, 9, true));
        assert!(at(39).4.iter().any(|l| l.unit == (0, 2) && l.hidden));
        assert_eq!((at(40).2.clone(), at(44).2.clone()), (vec![0x31], vec![0x31; 4]));
        // E+44: the listed units and the trap fade; ~E+60 all removed.
        assert_eq!(at(45).4.iter().find(|l| l.unit == (1, 1)).and_then(|l| l.level), Some(120));
        assert_eq!((at(45).8, at(60).7, at(61).7), (Some(120), false, true));
        assert_eq!(at(61).4.iter().filter(|l| l.hidden).count(), 3);
        assert_eq!(at(67).3, Some(CameraStep::Back(16)));
        assert_eq!(at(98).5, 9);
    }

    #[test]
    fn negate_attack_keeps_c602_param_1_and_hides_the_hud() {
        let v = Victim { text: 323 + 828, updates: 43, side: 0, cell: (3, 4), card: 1, spellbound: false, destroyed: false, camera_frames: 0, lowered: false, camera: true, recovery: None, face_up: true, defence: false, face: None };
        let mut a = Activation::new(Kind::Trap, input(), (1, 7), (1, 2), 828, Some(v), None);
        a.big = false;
        while a.t < 9 {
            a.tick();
        }
        assert_eq!((a.c602_param(), a.card_alpha()), (1, Some(128)));
        assert!(a.hides_hud());
    }
}
