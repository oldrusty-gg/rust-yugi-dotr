//! Select → "Return to Title Screen?", the OPTION panel and the free-duel
//! map (`system.rs`, `free_duel.rs`); the frame numbers are the EE
//! interpreter's (work/research/modes/map-modes/captures).
use super::tests::fixture;
use super::*;

fn menu(f: impl FnOnce(&mut MenuPad)) -> MenuPad {
    let mut m = MenuPad::default();
    f(&mut m);
    m
}

fn sfx(events: &[MapEvent]) -> Vec<u16> {
    events
        .iter()
        .filter_map(|e| match e {
            MapEvent::Sfx(s) => Some(*s),
            _ => None,
        })
        .collect()
}

#[test]
fn select_opens_return_to_title_which_slides_in_eight_updates_and_yes_pops() {
    // [emu select-yes] f2 Select: SE 4, y 512; f3..f10 y 448..0 (asking
    // from f10); Down x3 ignored; Left = YES with SE 3; Cross: SE 4, then
    // the pop every update.
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    let cursor = m.cursor;
    let events = m.step_with(Pad::NONE, menu(|p| p.select = true));
    assert_eq!(sfx(&events), [4]);
    let d = m.title_dialog.clone().unwrap();
    assert_eq!((d.state, d.y(), d.yes), (1, 512, false));
    let mut ys = Vec::new();
    for _ in 0..8 {
        m.step(Pad::NONE);
        ys.push(m.title_dialog.as_ref().unwrap().y());
    }
    assert_eq!(ys, [448, 384, 320, 256, 192, 128, 64, 0]);
    assert_eq!(m.title_dialog.as_ref().unwrap().state, 2);
    // The map takes no input meanwhile: the cursor stays.
    for _ in 0..3 {
        m.step_with(Pad { down: true, ..Pad::NONE }, menu(|p| p.down = true));
    }
    assert_eq!(m.cursor, cursor);
    let events = m.step_with(Pad { left: true, ..Pad::NONE }, menu(|p| p.left = true));
    assert_eq!(sfx(&events), [3]);
    assert!(m.title_dialog.as_ref().unwrap().yes);
    let events = m.step_with(Pad::CROSS, menu(|p| p.cross = true));
    assert_eq!(sfx(&events), [4]);
    assert!(!m.title_request);
    assert_eq!(m.travel, 0);
    let events = m.step(Pad::NONE);
    assert!(events.contains(&MapEvent::ReturnToTitle));
    assert!(m.title_request);
}

#[test]
fn no_or_circle_slides_the_dialog_down_and_circle_while_opening_is_ignored() {
    // [emu select-no] Cross on NO (preselected): SE 4, y 64..448 over seven
    // updates, gone on the eighth. [emu select-circle] Circle while the box
    // slides up does nothing.
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    m.step_with(Pad::NONE, menu(|p| p.select = true));
    for k in 0..8 {
        let circle = k == 4;
        let events = m.step_with(Pad::NONE, menu(|p| p.circle = circle));
        assert!(sfx(&events).is_empty());
    }
    let events = m.step_with(Pad::CROSS, menu(|p| p.cross = true));
    assert_eq!(sfx(&events), [4]);
    let mut ys = Vec::new();
    for _ in 0..8 {
        m.step(Pad::NONE);
        ys.push(m.title_dialog.as_ref().map(|d| d.y()));
    }
    assert_eq!(
        ys,
        [Some(64), Some(128), Some(192), Some(256), Some(320), Some(384), Some(448), None]
    );
    assert!(!m.title_request);
    // Circle on the question is NO with SE 5.
    m.step_with(Pad::NONE, menu(|p| p.select = true));
    for _ in 0..8 {
        m.step(Pad::NONE);
    }
    let events = m.step_with(Pad::NONE, menu(|p| p.circle = true));
    assert_eq!(sfx(&events), [5]);
    assert_eq!(m.title_dialog.as_ref().unwrap().state, 4);
}

#[test]
fn the_option_panel_toggles_two_rows_and_cross_writes_the_bits() {
    // [emu options-toggle] f1 Cross on icon 2 (y 512), open at f9; Down:
    // row 1 (SE 3); Right: bit 2 off (7 -> 3, SE 3); Up: row 0; Left: no
    // change; Right: bit 1 off (-> 1); Down x3 (repeat) end on row 1;
    // Cross: bits 1 written (+0xFF9C 0x1C -> 0x04), SE 4; closes over 8.
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    m.system_icons = vec![[0, 0, 8, 8], [20, 0, 28, 8], [240, 230, 272, 262], [60, 0, 68, 8]];
    m.cursor = (256, 240);
    m.step(Pad::NONE);
    assert_eq!(m.hovered_icon, Some(2));
    let events = m.step(Pad::CROSS);
    assert_eq!(sfx(&events), [4]);
    let p = m.options.clone().unwrap();
    assert_eq!((p.state, p.y(), p.bits, p.row), (1, 512, 7, 0));
    for _ in 0..8 {
        m.step(Pad::NONE);
    }
    assert_eq!(m.options.as_ref().unwrap().state, 2);
    let mut sounds = Vec::new();
    let mut press = |m: &mut MapModel, f: &dyn Fn(&mut MenuPad)| {
        let mut p = MenuPad::default();
        f(&mut p);
        sounds.push(sfx(&m.step_with(Pad::NONE, p)));
        let o = m.options.clone().unwrap();
        (o.row, o.bits)
    };
    assert_eq!(press(&mut m, &|p| p.down = true), (1, 7));
    assert_eq!(press(&mut m, &|p| p.right = true), (1, 3));
    assert_eq!(press(&mut m, &|p| p.up = true), (0, 3));
    assert_eq!(press(&mut m, &|p| p.left = true), (0, 3));
    assert_eq!(press(&mut m, &|p| p.right = true), (0, 1));
    for _ in 0..3 {
        press(&mut m, &|p| p.down = true);
    }
    assert_eq!(sounds, [vec![3], vec![3], vec![3], vec![], vec![3], vec![3], vec![3], vec![3]]);
    assert_eq!(m.option_bits, crate::game_options::DEFAULT_BITS);
    let events = m.step_with(Pad::NONE, menu(|p| p.cross = true));
    assert!(events.contains(&MapEvent::OptionsSaved { bits: 1 }));
    assert_eq!(sfx(&events), [4]);
    assert_eq!(m.option_bits, 1);
    let mut ys = Vec::new();
    for _ in 0..8 {
        m.step(Pad::NONE);
        ys.push(m.options.as_ref().map(|p| p.y()));
    }
    assert_eq!(
        ys,
        [Some(64), Some(128), Some(192), Some(256), Some(320), Some(384), Some(448), None]
    );
}

/// The fixture with White Rose texts for nodes 11..=13 and node 13's base
/// model 8 (a side-0 leader).
fn free_fixture() -> MapData {
    let mut data = fixture();
    let white = |node: u8, speaker: u16| NodeText {
        node,
        speaker_message: Some(speaker),
        place_message: Some(speaker + 22),
        opponent: Some((speaker - 197) as u8),
        encounter_base: None,
        encounter_groups: None,
        loss_base: None,
        win_base: None,
        loss_groups: None,
        win_groups: None,
        conditional: false,
        second_visit: None,
    };
    data.white_texts = vec![white(11, 218), white(12, 208), white(13, 209)];
    data.nodes[13].base_model = SIDE0_BASE_MODEL;
    data
}

fn free_save() -> CampaignSave {
    let mut save = CampaignSave::after_landing();
    save.red_route_cleared = true;
    save.white_route_cleared = true;
    // The second ending clears SAVE+0x3C and leaves C2 at its last event.
    save.map_entered = false;
    save.event = 313;
    save
}

fn hover(m: &mut MapModel, node: u8) {
    m.camera_target = m.node_position(node);
    let ((x, y), _) = node_anchor(m.camera_target, m.data.node(node)).unwrap();
    m.cursor = (x.clamp(0, 480) as i16, y.clamp(0, 480) as i16);
    m.step(Pad::NONE);
    assert_eq!(m.hovered, Some(node));
}

#[test]
fn continue_with_both_routes_opens_the_free_duel_map() {
    // [emu free-duel-hover init] 0x21C254: nodes 3, both sides' paths 3,
    // C2 = 0, C6/C7 kept.
    let m = MapModel::resume(free_fixture(), free_save()).unwrap();
    assert!(m.free_duel);
    assert_eq!(m.save.event, 0);
    assert!(m.save.node_states.iter().all(|s| *s == STATE_BEATEN));
    assert!(m.paths.iter().all(|p| p.state == 3));
    assert_eq!((m.save.hovered, m.save.location), (1, 1));
    assert!(matches!(m.handler, Handler::Free));
    assert!(m.hud.shown);
    // Every node is eligible (0x23AD50).
    assert!((0..NODE_COUNT as u8).all(|n| m.eligible(n)));
}

#[test]
fn free_duel_hover_takes_each_leader_and_turns_the_player_to_the_other_side() {
    let mut m = MapModel::resume(free_fixture(), free_save()).unwrap();
    hover(&mut m, 2);
    assert_eq!((m.save.opponent, m.save.side), (2, 0));
    for _ in 0..14 {
        m.step(Pad::NONE);
    }
    assert_eq!(m.save.side, 0);
    hover(&mut m, 13);
    // C4 12; base model 8 = a side-0 leader: SAVE[0] becomes 1 when the
    // panel is fully in [emu: nodes 12/13 at W+6C = 0x100].
    assert_eq!(m.save.opponent, 12);
    let mut frames = 0;
    while m.save.side == 0 {
        m.step(Pad::NONE);
        frames += 1;
        assert!(frames < 40);
    }
    assert_eq!(m.hud.panel, HUD_SLIDE_FULL);
    assert!(m.paths.iter().all(|p| p.state == 3));
}

#[test]
fn l1_r1_at_stonehenge_cycle_seto_and_both_manawyddans() {
    // [emu free-duel-oracle] R1: SE 2, C9 1, W+53C3 1, the panel slides out
    // (-20), at 0 W+53C4 = C9 and C4 = 20; again: C4 21 and SAVE[0] = 1.
    let mut m = MapModel::resume(free_fixture(), free_save()).unwrap();
    hover(&mut m, STONEHENGE_NODE);
    for _ in 0..14 {
        m.step(Pad::NONE);
    }
    assert_eq!((m.save.opponent, m.hud.panel), (1, HUD_SLIDE_FULL));
    let events = m.step_with(Pad::NONE, menu(|p| p.r1 = true));
    assert_eq!(sfx(&events), [2]);
    assert_eq!((m.save.c9, m.w53c4, m.flip), (1, 0, true));
    let mut frames = 0;
    while m.flip {
        m.step(Pad::NONE);
        frames += 1;
        assert!(frames < 40);
    }
    assert_eq!(m.w53c4, 1);
    m.step(Pad::NONE);
    assert_eq!(m.save.opponent, 20);
    for _ in 0..14 {
        m.step(Pad::NONE);
    }
    assert_eq!(m.save.side, 0);
    m.step_with(Pad::NONE, menu(|p| p.r1 = true));
    while m.flip {
        m.step(Pad::NONE);
    }
    m.step(Pad::NONE);
    assert_eq!(m.save.opponent, 21);
    for _ in 0..14 {
        m.step(Pad::NONE);
    }
    assert_eq!(m.save.side, 1);
    // L1 from 0 wraps to 2 on this map.
    m.save.c9 = 0;
    m.step_with(Pad::NONE, menu(|p| p.l1 = true));
    assert_eq!(m.save.c9, 2);
}

#[test]
fn a_free_duel_counts_the_result_and_comes_back_free() {
    let data = free_fixture();
    let mut save = free_save();
    save.node_states = vec![STATE_BEATEN; NODE_COUNT];
    save.hovered = 2;
    save.opponent = 2;
    let m = MapModel::after_duel_from(data, save, true).unwrap();
    assert_eq!(m.save.event, 0);
    assert_eq!((m.save.wins, m.save.wins_by_opponent[2]), (1, 1));
    assert!(m.free_duel && matches!(m.handler, Handler::Free));
    assert_eq!(m.save.location, 2);
}

#[test]
fn a_map_save_keeps_save_2_and_load_reads_it_into_the_map() {
    let mut save = free_save();
    save.map_entered = true;
    let s = crate::memory_card::save_record(None, &save, &[]);
    assert_eq!(&s[0..3], &[0, 1, 1]);
    let back = crate::memory_card::load_record(&s);
    assert!(back.map.white_route_cleared && back.map.free_duel());
    assert_eq!(
        crate::ending::continue_target(&back),
        crate::ending::ContinueTarget::FreeDuel
    );
}
