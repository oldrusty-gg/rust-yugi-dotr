use super::*;

// ------------------------------------------------------------ synthetic
// A synthetic White Rose map with invented coordinates and message ids (no
// retail data): the chain 11 → 12 → 13 → 14 → 15 ⇒ 16 ⇒ 17 → 18 … → 21 → 0
// ⇒ 11, sea routes 4, 5 and 11.
fn white_fixture() -> MapData {
    let mut nodes: Vec<MapNode> = (0..22u8)
        .map(|i| MapNode {
            index: i,
            name: format!("node-{i}"),
            position: [-100.0 + 9.0 * f32::from(i), 0.0, 150.0 - 5.0 * f32::from(i)],
            facing_raw: 0.0,
            base_model: 8,
            castle_model: 11,
            red_paths: [0; 4],
            white_paths: [0; 4],
            path_count: 0,
            path_dirs: [0; 4],
            win_event: 0,
            loss_event: 0,
        })
        .collect();
    let chain: [(u8, u8); 12] = [
        (11, 12),
        (12, 13),
        (13, 14),
        (14, 15),
        (15, 16),
        (16, 17),
        (17, 18),
        (18, 19),
        (19, 20),
        (20, 21),
        (21, 0),
        (0, 11),
    ];
    let mut paths: Vec<MapPath> = (0..32)
        .map(|i| MapPath {
            side: (i / 16) as u8,
            index: (i % 16) as u8,
            waypoints: Vec::new(),
            sea_route: false,
            ends: [0, 0],
            alternate_ends: [0, 0],
            dots: Vec::new(),
        })
        .collect();
    for (k, (a, b)) in chain.iter().enumerate() {
        nodes[usize::from(*a)].white_paths[0] = k as u16;
        let pa = nodes[usize::from(*a)].position;
        let pb = nodes[usize::from(*b)].position;
        let waypoints = vec![[pa[0] as i16, pa[2] as i16], [pb[0] as i16, pb[2] as i16]];
        let sea = matches!(k, 4 | 5 | 11);
        paths[16 + k] = MapPath {
            side: 1,
            index: k as u8,
            dots: spline_dots(&waypoints, sea),
            waypoints,
            sea_route: sea,
            ends: [u16::from(*a), u16::from(*b)],
            alternate_ends: [u16::from(*a), u16::from(*b)],
        };
    }
    for (n, win) in [
        (0u8, 311i16),
        (11, 312),
        (12, 301),
        (13, 302),
        (14, 303),
        (16, 305),
        (18, 307),
        (19, 308),
        (20, 309),
        (21, 310),
    ] {
        nodes[usize::from(n)].win_event = win;
        nodes[usize::from(n)].loss_event = win - 100;
    }
    let mut messages = BTreeMap::new();
    let mut white_texts = Vec::new();
    for n in [0u8, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21] {
        let duel = !matches!(n, 15 | 17);
        let base = 5000 + 40 * u16::from(n);
        white_texts.push(NodeText {
            node: n,
            speaker_message: duel.then_some(200 + u16::from(n)),
            place_message: Some(230 + u16::from(n)),
            opponent: duel.then_some(3 + n),
            encounter_base: duel.then_some(base),
            encounter_groups: duel.then_some(1),
            loss_base: duel.then_some(base + 10),
            win_base: duel.then_some(base + 20),
            loss_groups: duel.then_some(1),
            win_groups: duel.then_some(1),
            conditional: false,
            second_visit: None,
        });
        if duel {
            for b in [base, base + 10, base + 20] {
                for c in 1..=4 {
                    messages.insert(
                        b + c,
                        MessageInfo {
                            blank: c != 1,
                            glyphs: 20,
                            lines: 1,
                            text: "x".into(),
                        },
                    );
                }
            }
        }
    }
    // Mako's pages with their traced line counts (trace-dover: 3 lines
    // reveal in 19 updates, 2 in 13, 1 in 7).
    for (id, lines) in [(3140u16, 3u8), (3144, 3), (3145, 1), (3148, 2), (3152, 3), (3156, 2), (3160, 3), (3161, 2)] {
        messages.insert(
            id,
            MessageInfo {
                blank: false,
                glyphs: 30,
                lines,
                text: "x".into(),
            },
        );
    }
    for id in [6001u16, 6002, 6003, 6004, 6101, 6102, 6103, 6104] {
        messages.insert(
            id,
            MessageInfo {
                blank: id % 100 != 1,
                glyphs: 20,
                lines: 2,
                text: "x".into(),
            },
        );
    }
    MapData {
        nodes,
        paths,
        texts: Vec::new(),
        rose_card_nodes: vec![2, 4, 5, 6, 8, 9, 10, 11],
        messages,
        white_texts,
        white_captions: vec![
            EventCaption {
                event: 105,
                base: 6000,
                groups: 1,
                style: 1,
                speaker: None,
            },
            EventCaption {
                event: 111,
                base: 6100,
                groups: 1,
                style: 2,
                speaker: Some(198),
            },
        ],
    }
}

fn run_until(m: &mut MapModel, limit: u32, stop: impl Fn(&MapModel) -> bool) -> Vec<(u64, MapEvent)> {
    let mut log = Vec::new();
    for _ in 0..limit {
        let pad = if m.text_phase() == 4
            || m.caption.as_ref().is_some_and(|c| c.phase == 4)
        {
            Pad::CROSS
        } else {
            Pad::NONE
        };
        for e in m.step(pad) {
            log.push((m.frame, e));
        }
        if stop(m) {
            break;
        }
    }
    log
}

fn at(log: &[(u64, MapEvent)], event: &MapEvent) -> Option<u64> {
    log.iter().find(|(_, e)| e == event).map(|(f, _)| *f)
}

#[test]
fn the_white_start_reveals_windsor_and_hides_stonehenge() {
    let mut m = MapModel::resume(white_fixture(), CampaignSave::white_start()).unwrap();
    assert_eq!(m.save.side, 1);
    assert!(matches!(m.handler, Handler::Reveal { node: 11, .. }));
    let log = run_until(&mut m, 400, |m| m.save.event == 0);
    // The centroid pan ends on the white path's forward end, Windsor.
    assert_eq!(m.camera_target, m.node_position(12));
    let log = [log, run_until(&mut m, 400, |m| m.save.state(12) == STATE_OPEN)].concat();
    assert_eq!(m.save.event, 0);
    assert_eq!(m.save.state(STONEHENGE_NODE), STATE_HIDDEN);
    assert_eq!(m.save.location, STONEHENGE_NODE);
    assert!(at(&log, &MapEvent::PathRevealing(0)).is_some());
    assert!(at(&log, &MapEvent::NodeRevealed(12)).is_some());
    // Stonehenge stays hoverable at state 0 but refuses Cross; Windsor
    // takes a Cross with its taunt.
    assert!(m.eligible(11) && m.eligible(12) && !m.eligible(13) && m.eligible(0));
}

#[test]
fn white_duels_swap_the_identities_and_the_side_moves_first() {
    let data = white_fixture();
    let mut save = CampaignSave::white_start();
    save.event = 0;
    save.node_states[12] = STATE_OPEN;
    let mut m = MapModel::resume(data, save).unwrap();
    m.save.hovered = 12;
    m.save.opponent = 15;
    m.hovered = Some(12);
    m.travel = 3;
    m.figure.state = 3;
    m.save.destination = 12;
    let mut events = Vec::new();
    m.push_check(&mut events);
    assert_eq!(m.duel_request.as_ref().unwrap().params, [15, 1, 1015, 1100]);
}

#[test]
fn white_re_entry_draws_path_zero_and_the_beaten_nodes_paths() {
    let mut save = CampaignSave::white_start();
    save.event = 0;
    for n in [12, 13, 15, 0] {
        save.node_states[n] = STATE_BEATEN;
    }
    save.node_states[14] = STATE_OPEN;
    assert_eq!(save.drawn_paths(&white_fixture()), vec![0, 1, 2, 4, 11]);
    // The rose table of the white side: 0, 12, 13 here (15 is no rose).
    assert_eq!(save.rose_cards(&white_fixture()), 3);
    assert!(save.rose_card_on_win(&white_fixture(), 21));
    assert!(!save.rose_card_on_win(&white_fixture(), 16));
    assert!(!save.rose_card_on_win(&white_fixture(), 11));
}

#[test]
fn a_white_win_reveals_the_next_node_from_the_next_frame() {
    let data = white_fixture();
    let mut save = CampaignSave::white_start();
    save.event = 0;
    save.node_states[12] = STATE_OPEN;
    save.hovered = 12;
    save.location = 12;
    save.destination = 12;
    save.opponent = 15;
    save.apply_duel_end(&data, true);
    assert_eq!(save.event, 301);
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until(&mut m, 800, |m| m.save.state(13) == STATE_OPEN);
    let changed = at(&log, &MapEvent::EventChanged(101)).unwrap();
    assert!(at(&log, &MapEvent::RoseCard { node: 12 }).is_some());
    // The letterbox closes over 13 frames of W+6C falling, the path is
    // marked the frame after, C2 = 0 49 frames after the reveal's start.
    assert_eq!(at(&log, &MapEvent::PathRevealing(1)), Some(changed + 15));
    assert_eq!(at(&log, &MapEvent::EventChanged(0)), Some(changed + 49));
    assert_eq!(m.save.state(12), STATE_BEATEN);
}

#[test]
fn white_cross_rules_refuse_boulogne_and_dover_after_the_strait() {
    let data = white_fixture();
    let mut save = CampaignSave::white_start();
    save.event = 0;
    for n in [12, 13, 14] {
        save.node_states[n] = STATE_BEATEN;
    }
    save.node_states[15] = STATE_OPEN;
    save.node_states[17] = STATE_BEATEN;
    save.location = 14;
    let mut m = MapModel::resume(data.clone(), save.clone()).unwrap();
    // Dover with the Strait hidden: travel, no text, no DC check, then
    // story 12040 at the lerp's end.
    m.player_dc = Some(500);
    m.opponent_dc = Some(1);
    let anchor = node_anchor(m.camera_target, m.data.node(15)).unwrap().0;
    m.cursor = (anchor.0 as i16, anchor.1 as i16);
    m.hud.panel = HUD_SLIDE_FULL;
    let log = run_until(&mut m, 200, |m| m.save.event == EVENT_DOVER_STORY);
    assert!(at(&log, &MapEvent::Sfx(SFX_SELECT)).is_none(), "Cross was not pressed yet");
    let mut log = Vec::new();
    let anchor = m.anchors[15].unwrap().0;
    m.cursor = (anchor.0 as i16, anchor.1 as i16);
    for _ in 0..3 {
        for e in m.step(Pad::NONE) {
            log.push((m.frame, e));
        }
    }
    for e in m.step(Pad::CROSS) {
        log.push((m.frame, e));
    }
    log.extend(run_until(&mut m, 200, |m| m.save.event == EVENT_DOVER_STORY));
    assert!(at(&log, &MapEvent::Sfx(SFX_SELECT)).is_some());
    assert!(!log.iter().any(|(_, e)| matches!(e, MapEvent::TextPage { .. })));
    // 12040 is played by the map (Mako's panel), not handed to the story scene.
    assert!(at(&log, &MapEvent::EventChanged(EVENT_DOVER_STORY)).is_some());
    assert!(at(&log, &MapEvent::StoryEvent(EVENT_DOVER_STORY)).is_none());
    assert!(matches!(m.handler, Handler::MapStory { event: EVENT_DOVER_STORY, .. }));
    assert!(m.duel_request.is_none());
    // No duel either once the leader's walk reaches Dover while Mako's
    // page plays (the story replaces the push).
    let later = run_until(&mut m, 400, |m| m.save.event != EVENT_DOVER_STORY || m.duel_request.is_some());
    assert!(!later.iter().any(|(_, e)| matches!(e, MapEvent::DuelRequest(_))));
    assert!(m.duel_request.is_none());
    assert_eq!(white_story_continuation(EVENT_DOVER_STORY), Some(EVENT_DOVER_VOYAGE));
    // Boulogne never; Dover not once the Strait is open.
    let mut save = save;
    save.node_states[16] = STATE_OPEN;
    for node in [17u8, 15] {
        let mut m = MapModel::resume(data.clone(), save.clone()).unwrap();
        m.hud.panel = HUD_SLIDE_FULL;
        m.update_anchors();
        let anchor = m.anchors[usize::from(node)].unwrap().0;
        m.cursor = (anchor.0 as i16, anchor.1 as i16);
        m.step(Pad::NONE);
        m.step(Pad::NONE);
        let events = m.step(Pad::CROSS);
        assert!(!events.contains(&MapEvent::Sfx(SFX_SELECT)), "node {node}");
        assert_eq!(m.travel, 0);
    }
}

#[test]
fn the_dover_voyage_sails_kind_two_and_hands_over_to_12041() {
    let data = white_fixture();
    let mut save = CampaignSave::white_start();
    for n in [11, 12, 13, 14] {
        save.node_states[n] = STATE_BEATEN;
    }
    save.node_states[15] = STATE_OPEN;
    save.event = EVENT_DOVER_VOYAGE;
    save.location = 14;
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until(&mut m, 400, |m| m.save.event == EVENT_STRAIT_STORY);
    assert_eq!(m.ship.kind, 2);
    assert_eq!((m.ship_side(), m.ship_path(), m.ship_model()), (1, Some(4), 30));
    assert_eq!(m.save.state(15), STATE_BEATEN);
    assert_eq!(m.save.location, WHITE_STRAIT_NODE);
    let story = at(&log, &MapEvent::EventChanged(EVENT_STRAIT_STORY)).unwrap();
    assert!(matches!(m.handler, Handler::MapStory { event: EVENT_STRAIT_STORY, .. }));
    // Sub-step 0, 32 HUD updates (the last also pans), 15 more pan
    // updates: sailing from update 48, four per sample, then the hand-over.
    let samples = m.data.path(1, 4).dots.len() as u64;
    assert_eq!(story, 48 + 4 * samples);
    // 12041's tail: the Strait rises, the HUD comes back, C2 = 0.
    let mut save = m.save.clone();
    save.event = EVENT_STRAIT_STORY;
    let mut m = MapModel::resume(m.data.clone(), save).unwrap();
    let log = run_until(&mut m, 400, |m| m.save.event == 0);
    assert_eq!(at(&log, &MapEvent::NodeRevealed(16)), Some(64));
    assert_eq!(at(&log, &MapEvent::EventChanged(0)), Some(64 + 32 + 9));
    assert_eq!(m.save.state(16), STATE_OPEN);
}

#[test]
fn the_strait_voyage_shows_the_caption_and_reveals_boulogne() {
    let data = white_fixture();
    let mut save = CampaignSave::white_start();
    save.event = 0;
    for n in [11, 12, 13, 14, 15] {
        save.node_states[n] = STATE_BEATEN;
    }
    save.node_states[16] = STATE_OPEN;
    save.hovered = 16;
    save.location = 16;
    save.destination = 16;
    save.opponent = 14;
    save.apply_duel_end(&data, true);
    assert_eq!(save.event, 305);
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until(&mut m, 1200, |m| m.save.state(18) == STATE_OPEN);
    assert!(at(&log, &MapEvent::RoseCard { node: 16 }).is_none());
    let voyage = at(&log, &MapEvent::EventChanged(EVENT_STRAIT_VOYAGE)).unwrap();
    assert!(at(&log, &MapEvent::Music { track: 35, keep: true }).unwrap() > voyage);
    let caption = at(&log, &MapEvent::TextPage { message: 6001, page: 1 }).unwrap();
    let reveal = at(&log, &MapEvent::EventChanged(EVENT_BOULOGNE_REVEAL)).unwrap();
    assert!(caption < reveal);
    assert_eq!(at(&log, &MapEvent::PathRevealing(6)), Some(reveal + 9));
    assert_eq!(m.save.state(16), STATE_BEATEN);
    assert_eq!(m.save.state(17), STATE_BEATEN);
    // The sea route 5 keeps state 1 and does not count as animating.
    assert_eq!(m.paths[5].state, 1);
    assert!(m.paths_idle() || m.paths[6].state != 3);
}

#[test]
fn the_brest_win_sails_home_with_setos_window_and_starts_12130() {
    let data = white_fixture();
    let mut save = CampaignSave::white_start();
    save.event = 0;
    for n in 12..=21 {
        save.node_states[n] = STATE_BEATEN;
    }
    save.node_states[0] = STATE_OPEN;
    save.hovered = 0;
    save.location = 0;
    save.destination = 0;
    save.opponent = 19;
    save.apply_duel_end(&data, true);
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until(&mut m, 2000, |m| matches!(m.handler, Handler::Story { .. }));
    assert!(at(&log, &MapEvent::RoseCard { node: 0 }).is_some());
    assert!(at(&log, &MapEvent::TextPage { message: 6101, page: 1 }).is_some());
    assert!(at(&log, &MapEvent::StoryEvent(EVENT_WHITE_FINAL)).is_some());
    assert_eq!((m.ship.kind, m.ship_model()), (4, 27));
    assert_eq!(m.save.state(STONEHENGE_NODE), STATE_OPEN);
    assert_eq!(m.save.location, STONEHENGE_NODE);
    let n11 = m.data.node(11).position;
    assert!((m.camera_target[0] - f64::from(n11[0])).abs() < 1e-3);
}

#[test]
fn the_manawyddan_win_closes_with_12150_and_older_imports_refuse_white_texts() {
    let data = white_fixture();
    let mut save = CampaignSave::white_start();
    save.event = 0;
    for n in 12..=21 {
        save.node_states[n] = STATE_BEATEN;
    }
    save.node_states[0] = STATE_BEATEN;
    save.node_states[11] = STATE_OPEN;
    save.c9 = 1;
    save.hovered = 11;
    save.location = 11;
    save.destination = 11;
    save.opponent = 21;
    let mut won = save.clone();
    won.apply_duel_end(&data, true);
    // No red C9 exception on the white side: 312.
    assert_eq!(won.event, 312);
    let mut m = MapModel::resume(data.clone(), won).unwrap();
    let log = run_until(&mut m, 600, |m| matches!(m.handler, Handler::Story { .. }));
    assert!(at(&log, &MapEvent::RoseCard { node: 11 }).is_none());
    assert_eq!(m.take_story_request(), Some(EVENT_WHITE_WIN));
    assert_eq!(m.save.state(11), STATE_BEATEN);
    let mut lost = save;
    lost.apply_duel_end(&data, false);
    assert_eq!(lost.event, 212);
    let mut m = MapModel::resume(data.clone(), lost).unwrap();
    run_until(&mut m, 600, |m| matches!(m.handler, Handler::Free));
    assert_eq!(m.save.state(11), STATE_LOST);
    // An import without the white texts: the return names the boundary.
    let old = MapData {
        white_texts: Vec::new(),
        white_captions: Vec::new(),
        ..data
    };
    let mut save = CampaignSave::white_start();
    save.event = 0;
    save.node_states[12] = STATE_OPEN;
    save.hovered = 12;
    save.location = 12;
    save.destination = 12;
    save.opponent = 15;
    save.apply_duel_end(&old, false);
    let mut m = MapModel::resume(old, save).unwrap();
    run_until(&mut m, 200, |m| matches!(m.handler, Handler::Free));
    assert!(m.notes.iter().any(|n| n == WHITE_TEXTS_MISSING));
}

// ------------------------------------------------------------ emulator
mod private {
    use super::*;

    fn data() -> MapData {
        let path = std::env::var_os("ROSES_WHITE_MAP_DATA").expect("ROSES_WHITE_MAP_DATA");
        load_map_data(std::path::Path::new(&path)).unwrap()
    }

    /// First update (1-based) at which `pred` holds after the update.
    fn first(m: &mut MapModel, limit: u32, pad: impl Fn(&MapModel) -> Pad, pred: impl Fn(&MapModel) -> bool) -> Option<u64> {
        for _ in 0..limit {
            let p = pad(m);
            m.step(p);
            if pred(m) {
                return Some(m.frame);
            }
        }
        None
    }

    fn cross_on_page(m: &MapModel) -> Pad {
        if m.text_phase() == 4 || m.caption.as_ref().is_some_and(|c| c.phase == 4) {
            Pad::CROSS
        } else {
            Pad::NONE
        }
    }

    fn after_win(states: &[(u8, u8)], node: u8, opponent: i16, won: bool) -> MapModel {
        let data = data();
        let mut save = CampaignSave::white_start();
        save.event = 0;
        save.c9 = 0;
        for (n, s) in states {
            save.node_states[usize::from(*n)] = *s;
        }
        save.hovered = node;
        save.location = node;
        save.destination = node;
        save.opponent = opponent;
        save.apply_duel_end(&data, won);
        MapModel::resume(data, save).unwrap()
    }

    #[test]
    #[ignore = "Requires the private White Rose map import (ROSES_WHITE_MAP_DATA)"]
    fn private_event_100_matches_start_12000() {
        // logs/white-start-12000: C2 = 100 at f694 (sub-step 0), path 0
        // state 1 at f708, C2 = 0 and path 0 drawn at f743, Windsor 1 at
        // f807, Stonehenge 0 throughout. Update n = f(694 + n).
        let mut m = MapModel::resume(data(), CampaignSave::white_start()).unwrap();
        let mark = first(&mut m, 200, |_| Pad::NONE, |m| m.paths[0].state == 1).unwrap();
        assert_eq!(694 + mark, 708);
        let free = first(&mut m, 200, |_| Pad::NONE, |m| m.save.event == 0).unwrap();
        assert_eq!(694 + free, 743);
        assert_eq!(m.paths[0].state, 3);
        let windsor = first(&mut m, 200, |_| Pad::NONE, |m| m.save.state(12) == 1).unwrap();
        assert_eq!(694 + windsor, 807);
        assert_eq!(m.save.state(11), 0);
    }

    #[test]
    #[ignore = "Requires the private White Rose map import (ROSES_WHITE_MAP_DATA)"]
    fn private_returns_match_the_white_logs() {
        // windsor-win: pages 3112 f15, 3113 f45, 101 at f203, mark f218,
        // C2 = 0 f252, London 1 f309.
        let mut m = after_win(&[(11, 3), (12, 1)], 12, 11, true);
        let text = first(&mut m, 100, cross_on_page, |m| m.text.as_ref().is_some_and(|t| t.message == 3112)).unwrap();
        assert_eq!(text, 15);
        let reveal = first(&mut m, 400, cross_on_page, |m| m.save.event == 101).unwrap();
        assert_eq!(reveal, 203);
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.paths[1].state == 1), Some(218));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.save.event == 0), Some(252));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.save.state(13) == 1), Some(309));
        // windsor-loss: page 3108 f15, C2 = 0 f55, state 2.
        let mut m = after_win(&[(12, 1)], 12, 11, false);
        assert_eq!(first(&mut m, 100, cross_on_page, |m| m.save.event == 0), Some(55));
        assert_eq!(m.save.state(12), STATE_LOST);
        // canterbury-win: 103 at f215, Dover rises.
        let mut m = after_win(&[(11, 3), (12, 3), (13, 3), (14, 1)], 14, 13, true);
        assert_eq!(first(&mut m, 400, cross_on_page, |m| m.save.event == 103), Some(215));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.save.event == 0), Some(264));
    }

    #[test]
    #[ignore = "Requires the private White Rose map import (ROSES_WHITE_MAP_DATA)"]
    fn private_dover_voyage_matches_dover_12040() {
        // dover-12040: 104 sub-step 0 at f070 (update 1 = f071), ship
        // sailing f118, arrived f189, C2 = 12041 f190; the tail from f369
        // (update 1): Strait 1 f432, letterbox mode 0 f464, C2 = 0 f473.
        let data = data();
        let mut save = CampaignSave::white_start();
        for n in [11, 12, 13, 14] {
            save.node_states[n] = 3;
        }
        save.node_states[15] = 1;
        save.event = EVENT_DOVER_VOYAGE;
        save.location = 15;
        save.hovered = 15;
        let mut m = MapModel::resume(data.clone(), save).unwrap();
        let sail = first(&mut m, 200, |_| Pad::NONE, |m| m.ship.state == 2).unwrap();
        assert_eq!(70 + sail, 118);
        let arrived = first(&mut m, 200, |_| Pad::NONE, |m| m.ship.state == 3).unwrap();
        assert_eq!(70 + arrived, 189);
        let story = first(&mut m, 5, |_| Pad::NONE, |m| m.save.event == EVENT_STRAIT_STORY).unwrap();
        assert_eq!(70 + story, 190);
        assert_eq!((m.save.location, m.save.state(15)), (16, 3));
        let mut save = m.save.clone();
        save.event = EVENT_STRAIT_STORY;
        let mut m = MapModel::resume(data, save).unwrap();
        let rise = first(&mut m, 200, |_| Pad::NONE, |m| m.save.state(16) == 1).unwrap();
        assert_eq!(368 + rise, 432);
        let lb = first(&mut m, 100, |_| Pad::NONE, |m| m.letterbox.mode == 0).unwrap();
        assert_eq!(368 + lb, 464);
        let free = first(&mut m, 100, |_| Pad::NONE, |m| m.save.event == 0).unwrap();
        assert_eq!(368 + free, 473);
    }

    #[test]
    #[ignore = "Requires the private White Rose map import (ROSES_WHITE_MAP_DATA)"]
    fn private_mako_pages_over_the_map_match_trace_dover() {
        // trace-dover (claude-story-white): 12040 entered at f0; panel f19,
        // phase 4 f62, C2 = 104 f70, ship sailing f118, arrived f189, C2 =
        // 12041 f190, pages 3144..3161 (phase 4 f210 .. f336), close f344,
        // panel gone f368, Strait 1 f432, C2 = 0 f473. Line counts as traced
        // (the map import does not list the pages; the frontend fills them).
        let mut data = data();
        for (id, lines) in [(3140u16, 3u8), (3144, 3), (3145, 1), (3148, 2), (3152, 3), (3156, 2), (3160, 3), (3161, 2)] {
            data.messages.insert(id, MessageInfo { blank: false, glyphs: 30, lines, text: String::new() });
        }
        let mut save = CampaignSave::white_start();
        for n in [11, 12, 13, 14] {
            save.node_states[n] = 3;
        }
        save.node_states[15] = 1;
        save.event = EVENT_DOVER_STORY;
        save.location = 15;
        save.hovered = 15;
        let mut m = MapModel::resume(data, save).unwrap();
        let cross = |m: &MapModel| if m.text_phase() == 4 { Pad::CROSS } else { Pad::NONE };
        // Update n = f(n - 1): the resumed sub-step 0 is the entry frame.
        let f = |m: &mut MapModel, limit: u32, pred: &dyn Fn(&MapModel) -> bool| {
            first(m, limit, cross, pred).map(|n| n - 1)
        };
        assert_eq!(f(&mut m, 40, &|m| m.story_dialogue.is_some()), Some(19));
        assert_eq!(f(&mut m, 80, &|m| m.text_phase() == 4), Some(62));
        assert_eq!(f(&mut m, 20, &|m| m.save.event == EVENT_DOVER_VOYAGE), Some(70));
        assert_eq!(f(&mut m, 200, &|m| m.ship.state == 2), Some(118));
        assert_eq!(f(&mut m, 200, &|m| m.ship.state == 3), Some(189));
        assert_eq!(f(&mut m, 5, &|m| m.save.event == EVENT_STRAIT_STORY), Some(190));
        for wait in [210, 224, 244, 270, 290, 316, 336] {
            assert_eq!(f(&mut m, 40, &|m| m.text_phase() == 4), Some(wait));
        }
        assert_eq!(f(&mut m, 20, &|m| m.text_phase() == 13), Some(344));
        assert_eq!(f(&mut m, 40, &|m| m.story_dialogue.is_none()), Some(368));
        assert_eq!(f(&mut m, 100, &|m| m.save.state(16) == 1), Some(432));
        assert_eq!(f(&mut m, 100, &|m| m.letterbox.mode == 0), Some(464));
        assert_eq!(f(&mut m, 100, &|m| m.save.event == 0), Some(473));
    }

    #[test]
    #[ignore = "Requires the private White Rose map import (ROSES_WHITE_MAP_DATA)"]
    fn private_strait_voyage_matches_mako_win() {
        // mako-win: 105 at f087, sub 2 f101, sailing f148, arrived f219,
        // 3180 f220 (phase 4 f225, Cross f226, closed f231), Boulogne 1
        // f283, 106 f315, Boulogne 3 f316, path 6 f324, C2 = 0 f364,
        // path 6 drawn f367, Amiens 1 f431; path 5 stays 1.
        let mut m = after_win(&[(11, 3), (12, 3), (13, 3), (14, 3), (15, 3), (16, 1)], 16, 14, true);
        assert_eq!(first(&mut m, 200, cross_on_page, |m| m.save.event == 105), Some(87));
        assert_eq!(first(&mut m, 200, |_| Pad::NONE, |m| m.letterbox.active == 1), Some(101));
        assert_eq!(first(&mut m, 200, |_| Pad::NONE, |m| m.ship.state == 2), Some(148));
        assert_eq!(first(&mut m, 200, |_| Pad::NONE, |m| m.ship.state == 3), Some(219));
        assert_eq!(first(&mut m, 5, |_| Pad::NONE, |m| m.caption.as_ref().is_some_and(|c| c.message == 3180)), Some(220));
        assert_eq!(first(&mut m, 20, |_| Pad::NONE, |m| m.caption.as_ref().is_some_and(|c| c.phase == 4)), Some(225));
        m.step(Pad::CROSS);
        assert_eq!(first(&mut m, 20, |_| Pad::NONE, |m| m.caption.as_ref().is_none_or(|c| c.phase == 12)), Some(231));
        assert_eq!(first(&mut m, 200, |_| Pad::NONE, |m| m.save.state(17) == 1), Some(283));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.save.event == 106), Some(315));
        assert_eq!(first(&mut m, 5, |_| Pad::NONE, |m| m.save.state(17) == 3), Some(316));
        assert_eq!(first(&mut m, 20, |_| Pad::NONE, |m| m.paths[6].state == 1), Some(324));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.save.event == 0), Some(364));
        assert_eq!(first(&mut m, 10, |_| Pad::NONE, |m| m.paths[6].state == 3), Some(367));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.save.state(18) == 1), Some(431));
        assert_eq!(m.paths[5].state, 1);
    }

    #[test]
    #[ignore = "Requires the private White Rose map import (ROSES_WHITE_MAP_DATA)"]
    fn private_home_voyage_matches_brest_win() {
        // brest-win: 111 f257, 3300 f289 (phase 4 f332, Cross f333, closed
        // f364), sailing f412, arrived f519, C7 = 11 f520, Stonehenge 1
        // f583, letterbox mode 0 f615, mode 3 f633, C2 = 12130 f665.
        let states: Vec<(u8, u8)> = (12..=21).map(|n| (n, 3)).chain([(0, 1)]).collect();
        let mut m = after_win(&states, 0, 19, true);
        assert_eq!(first(&mut m, 400, cross_on_page, |m| m.save.event == 111), Some(257));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.text.as_ref().is_some_and(|t| t.message == 3300)), Some(289));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.text_phase() == 4), Some(332));
        m.step(Pad::CROSS);
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.text.is_none()), Some(364));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.ship.state == 2), Some(412));
        assert_eq!(first(&mut m, 200, |_| Pad::NONE, |m| m.ship.state == 3), Some(519));
        assert_eq!(first(&mut m, 5, |_| Pad::NONE, |m| m.save.location == 11), Some(520));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.save.state(11) == 1), Some(583));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.letterbox.mode == 0), Some(615));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.letterbox.mode == 3), Some(633));
        assert_eq!(first(&mut m, 100, |_| Pad::NONE, |m| m.save.event == EVENT_WHITE_FINAL), Some(665));
    }

    #[test]
    #[ignore = "Requires the private White Rose map import (ROSES_WHITE_MAP_DATA)"]
    fn private_manawyddan_win_starts_12150() {
        // manawyddan-win: 312 (no rose) → 12150 at f77.
        let states: Vec<(u8, u8)> = (12..=21).map(|n| (n, 3)).chain([(0, 3), (11, 1)]).collect();
        let mut m = after_win(&states, 11, 21, true);
        assert_eq!(m.save.event, 312);
        assert_eq!(first(&mut m, 200, cross_on_page, |m| m.save.event == EVENT_WHITE_WIN), Some(77));
        assert_eq!(m.save.state(11), 3);
    }
}

/// One row per update of the dialogue's traced quantities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DialogueRow {
    event: i16,
    phase: u8,
    slide: i32,
    message: u16,
    kind: u8,
}

fn dialogue_row(m: &MapModel) -> DialogueRow {
    let d = m.story_dialogue.as_ref();
    DialogueRow {
        event: m.save.event,
        phase: d.map_or(0, |d| d.phase),
        slide: d.map_or(0, |d| d.slide),
        message: d.map_or(0, |d| d.message),
        kind: d.and_then(|d| d.face_request()).map_or(0, |f| f.1),
    }
}

/// The first frame (1-based from `rows[0]`) whose row satisfies `p`.
fn first_row(rows: &[DialogueRow], p: impl Fn(&DialogueRow) -> bool) -> Option<usize> {
    rows.iter().position(p).map(|i| i + 1)
}

#[test]
fn mako_pages_play_over_the_map_with_the_traced_frames() {
    // trace-dover (claude-story-white): f1 = the first update of 12040.
    let data = white_fixture();
    let mut save = CampaignSave::white_start();
    save.event = 0;
    for n in [11, 12, 13, 14] {
        save.node_states[n] = STATE_BEATEN;
    }
    save.node_states[15] = STATE_OPEN;
    save.location = 14;
    let mut m = MapModel::resume(data, save).unwrap();
    m.player_dc = Some(500);
    m.opponent_dc = Some(1);
    m.hud.panel = HUD_SLIDE_FULL;
    run_until(&mut m, 200, |m| m.save.event == EVENT_DOVER_STORY);
    let anchor = m.anchors[15].unwrap().0;
    m.cursor = (anchor.0 as i16, anchor.1 as i16);
    for _ in 0..3 {
        m.step(Pad::NONE);
    }
    m.step(Pad::CROSS);
    run_until(&mut m, 300, |m| m.save.event == EVENT_DOVER_STORY);
    assert!(
        matches!(m.handler, Handler::MapStory { event: EVENT_DOVER_STORY, sub: 1 }),
        "{:?} event {} travel {} hovered {:?}",
        m.handler,
        m.save.event,
        m.travel,
        m.hovered
    );
    // Drive frame by frame (Cross on the update after a page waits).
    let mut rows = Vec::new();
    let mut log = Vec::new();
    let start = m.frame;
    for _ in 0..700 {
        let pad = if m.text_phase() == 4 { Pad::CROSS } else { Pad::NONE };
        for e in m.step(pad) {
            log.push((m.frame - start, e));
        }
        rows.push(dialogue_row(&m));
        // The HUD stays out under Mako's panel [emu: drawn 320 rows down].
        if m.story_dialogue.is_some() {
            assert_eq!(m.hud.slide, 0, "HUD slide at update {}", rows.len());
        }
        if m.save.event == 0 {
            break;
        }
    }
    // 12040: letterbox and music from f1, done f18; panel from f19
    // (W+53BA 64), full and phase 3 at f34; talking f44; waits f62; Cross
    // f63 (phase 5); 11 f68, 12 f69; C2 = 104 at f70, the panel stays.
    assert_eq!(at(&log, &MapEvent::Music { track: 35, keep: true }), Some(1));
    assert_eq!(first_row(&rows, |r| r.slide > 0), Some(19));
    assert_eq!(rows[18].slide, 64);
    assert_eq!(rows[18].phase, 2);
    assert_eq!(rows[18].message, 3140);
    assert_eq!(first_row(&rows, |r| r.slide == DIALOGUE_PANEL_FULL), Some(34));
    assert_eq!(first_row(&rows, |r| r.phase == 3), Some(34));
    assert_eq!(first_row(&rows, |r| r.kind == 3), Some(44));
    assert_eq!(first_row(&rows, |r| r.phase == 4), Some(62));
    assert_eq!(first_row(&rows, |r| r.phase == 5), Some(63));
    assert_eq!(rows[62].kind, 1);
    assert_eq!(first_row(&rows, |r| r.phase == 11), Some(68));
    assert_eq!(first_row(&rows, |r| r.phase == 12), Some(69));
    assert_eq!(first_row(&rows, |r| r.event == EVENT_DOVER_VOYAGE), Some(70));
    // The voyage under Mako's panel (no text, face kind 1).
    let strait = first_row(&rows, |r| r.event == EVENT_STRAIT_STORY).unwrap();
    for r in &rows[70..strait - 1] {
        assert_eq!((r.slide, r.phase, r.kind), (DIALOGUE_PANEL_FULL, 12, 1));
    }
    // 12041 (trace C2 = 12041 at f190): relative frames of every page.
    let rel = |f: usize| strait + f - 190;
    let page_frames: [(u16, usize, usize); 7] = [
        (3144, 191, 210),
        (3145, 217, 224),
        (3148, 231, 244),
        (3152, 251, 270),
        (3156, 277, 290),
        (3160, 297, 316),
        (3161, 323, 336),
    ];
    for (k, (message, reveal, wait)) in page_frames.into_iter().enumerate() {
        let shown = first_row(&rows, |r| r.message == message).unwrap();
        // Page 1 opens in phase 3; the others in phase 2 for one update.
        assert_eq!(shown, rel(if k == 0 { reveal } else { reveal - 1 }), "{message}");
        assert_eq!(rows[rel(reveal) - 1].phase, 3, "{message}");
        assert_eq!(rows[rel(reveal + 1) - 1].kind, 3, "{message}");
        assert_eq!(rows[rel(wait) - 2].phase, 3, "{message}");
        assert_eq!(rows[rel(wait) - 1].phase, 4, "{message}");
        assert_eq!(rows[rel(wait)].phase, 5, "{message}");
    }
    let phase_at = |f: usize| rows[rel(f) - 1].phase;
    assert_eq!((phase_at(342), phase_at(343), phase_at(344)), (11, 12, 13));
    // The close: page box 100 -> 0 f344..f353, the panel 960 at f353 and
    // gone at f368 (face y 384 at f353 .. 608 at f367).
    assert_eq!(rows[rel(352) - 1].slide, DIALOGUE_PANEL_FULL);
    assert_eq!(rows[rel(353) - 1].slide, 960);
    assert_eq!(rows[rel(367) - 1].slide, 64);
    assert_eq!(
        rows[rel(368) - 1],
        DialogueRow { event: EVENT_STRAIT_STORY, phase: 0, slide: 0, message: 0, kind: 0 }
    );
    // The tail from f369 (as resumed at 12041): C2 = 0 at f473.
    assert_eq!(rows.len(), rel(473));
    assert_eq!(rows.last().unwrap().event, 0);
    assert_eq!(m.save.state(16), STATE_OPEN);
    assert!(!log.iter().any(|(_, e)| matches!(e, MapEvent::StoryEvent(_))));
}
