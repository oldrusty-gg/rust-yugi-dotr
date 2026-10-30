//! The free-duel map after both routes (W+18C = 1) (Claude, 5 Oct 2026;
//! notes and oracles in `work/research/modes/map-modes/`).
//!
//! Set-up 0x21C254 (story init with SAVE[1] && SAVE[2]) [F, emu
//! captures/free-duel-hover.json]: map bank 0x1E91B0(3, 2, 0), SAVE[1] =
//! SAVE[2] = 1, C2 = 0, all 32 node states 3, the rise words cleared, both
//! sides' 16 paths state 3, W+53B5 = 0, track 4 (mode 0); C6..CA are kept.
//!
//! The W+18C branches the model follows [F; emu where noted]:
//! - dispatcher 0x2263D8: C2 < 200 → the free cursor (C2 is 0 always: the
//!   duel end stores 0 when both flags are set, 0x227F00);
//! - node pass 0x22A888: W+6E = 1 (the HUD is always shown);
//! - hover 0x23AD50: every node is eligible;
//! - hover panel 0x234CC0's switch 0x2D2630: Stonehenge's speaker by
//!   W+53C4: 2 → 218 (C4 21, the White Rose's Manawyddan), 1 → 217 (C4 20,
//!   the red Manawyddan), else 198 (Seto); every other node as on its own
//!   side's map (2..10 red, 12..21 White Rose) [emu: C4 2, 9, 10, 3, 11, 12
//!   on nodes 2, 3, 7, 8, 12, 13; 1, 20, 21 at Stonehenge];
//! - 0x235264: the panel (and the Triangle code 0x238640) for every node
//!   but 1, 15 and 17;
//! - bottom panel 0x234018: the hovered node's leader side (Stonehenge:
//!   W+53C4 < 2 → 1, else 0; other nodes: base model 8 → 0, else 1); when
//!   it equals SAVE[0] while the hover panel is fully in (W+6C = 0x100),
//!   **SAVE[0] ^= 1** — the player always takes the other side [emu: nodes
//!   12/13 and the white Manawyddan turn SAVE[0] to 1];
//! - L1/R1 at Stonehenge cycle C9 over 0..=2 (`system::stonehenge_flip`);
//! - Cross handler 0x239868: the W+5442 countdown is skipped (the model
//!   has none on this path);
//! - close logic 0x230FC8: a return event closes to C2 = 0 (unreachable
//!   while the duel end stores 0).
//! The duel itself: params as on the side's map (`push_check`), the
//! opponent's deck from C4 as everywhere; after it 0x227F00 counts the
//! wins/losses and the map comes back free [F RED-ROUTE §2.1].
use super::*;

/// 0x235264: the nodes without a duel on the free-duel map.
pub const FREE_DUEL_PLACES: [u8; 3] = [1, 15, 17];
/// 0x234078: a node whose base model is 8 has a side-0 leader.
pub const SIDE0_BASE_MODEL: i16 = 8;

impl CampaignSave {
    /// SAVE[1] && SAVE[2]: story init sets W+18C (0x21BFBC).
    pub fn free_duel(&self) -> bool {
        self.red_route_cleared && self.white_route_cleared
    }
}

impl MapModel {
    /// Story init with both routes cleared (0x21C254): the free-duel map,
    /// camera on the location, the free cursor at C2 = 0.
    pub(super) fn free_duel_map(data: MapData, mut save: CampaignSave) -> Result<Self> {
        save.red_route_cleared = true;
        save.white_route_cleared = true;
        save.event = 0;
        save.node_states = vec![STATE_BEATEN; NODE_COUNT];
        let position = data.node(save.location).position;
        let target = [f64::from(position[0]), f64::from(position[2])];
        let mut model = Self::base(data, save, target)?;
        model.free_duel = true;
        model.draw_all_paths();
        model.fade = FADE_FULL;
        model.hud.shown = true;
        model.hud.hidden = false;
        model.hud.slide = HUD_SLIDE_FULL;
        model.handler = Handler::Free;
        model.update_anchors();
        Ok(model)
    }

    /// Both sides' paths are state 3; the dot pass draws SAVE[0]'s.
    pub(super) fn draw_all_paths(&mut self) {
        let side = self.save.side;
        self.paths = (0..PATHS_PER_SIDE)
            .map(|i| PathState {
                state: 0,
                frames: 0,
                alpha: vec![0; self.data.path(side, i as u8).dots.len()],
            })
            .collect();
        for p in 0..PATHS_PER_SIDE {
            self.set_path_drawn(p as u8);
        }
    }

    /// The hover/text constants on the free-duel map (the switch 0x2D2630
    /// with W+18C set).
    pub(super) fn free_duel_text(&self, node: u8) -> Option<NodeTextVariant> {
        if node == STONEHENGE_NODE {
            return match self.w53c4 {
                2 => self.data.side_text(1, node, STATE_BEATEN, 0),
                w => self.data.text_variant(node, STATE_BEATEN, w),
            };
        }
        if usize::from(node) >= 12 {
            return self.data.side_text(1, node, STATE_BEATEN, 0);
        }
        self.data.text_variant(node, STATE_BEATEN, 0)
    }

    /// 0x234030..0x2340A0: the side of `node`'s leader.
    pub fn leader_side(&self, node: u8) -> u8 {
        if node == STONEHENGE_NODE {
            return u8::from(self.w53c4 < 2);
        }
        u8::from(self.data.node(node).base_model != SIDE0_BASE_MODEL)
    }

    /// 0x234140: with the hover panel fully in, a leader of the player's
    /// own side turns the player to the other side (the path set the dot
    /// pass draws follows SAVE[0]).
    pub(super) fn free_duel_side(&mut self) {
        if !self.free_duel || self.hud.panel < HUD_SLIDE_FULL || self.hovered.is_none() {
            return;
        }
        let node = self.save.hovered;
        if self.leader_side(node) == self.save.side {
            self.save.side ^= 1;
            self.draw_all_paths();
        }
    }
}
