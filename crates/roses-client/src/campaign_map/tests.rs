use super::*;

// Synthetic map with invented coordinates and message ids; no retail data.
fn node(index: u8, x: f32, z: f32, red: [u16; 4], count: u16, dirs: [u16; 4]) -> MapNode {
    MapNode {
        index,
        name: format!("node-{index}"),
        position: [x, 0.0, z],
        facing_raw: 0.0,
        base_model: 9,
        castle_model: 11 + i16::from(index % 16),
        red_paths: red,
        white_paths: [0; 4],
        path_count: count,
        path_dirs: dirs,
        win_event: 301 + i16::from(index),
        loss_event: 201 + i16::from(index),
    }
}
fn path(side: u8, index: u8, waypoints: Vec<[i16; 2]>, ends: [u16; 2], alt: [u16; 2]) -> MapPath {
    let dots = spline_dots(&waypoints, false);
    MapPath {
        side,
        index,
        waypoints,
        sea_route: false,
        ends,
        alternate_ends: alt,
        dots,
    }
}
fn message(blank: bool, lines: u8) -> MessageInfo {
    MessageInfo {
        blank,
        glyphs: u16::from(lines) * 20,
        lines,
        text: if blank {
            " ".into()
        } else {
            "x".repeat(usize::from(lines) * 20)
        },
    }
}

/// Node 1 at (-80, 60), node 2 at (-38, 40) joined by path 1 (chord 46.5 →
/// 11 dots), node 2 → node 3 by path 3 and node 2 → node 5 by path 4, node 8
/// open elsewhere.
pub(super) fn fixture() -> MapData {
    let mut nodes: Vec<MapNode> = (0..22)
        .map(|i| {
            node(
                i,
                -100.0 + 12.0 * i as f32,
                150.0 - 9.0 * i as f32,
                [0; 4],
                0,
                [0; 4],
            )
        })
        .collect();
    nodes[1] = node(1, -80.0, 60.0, [1, 2, 0, 0], 2, [1, 1, 0, 0]);
    nodes[2] = node(2, -38.0, 40.0, [3, 4, 12, 0], 2, [1, 1, 1, 0]);
    nodes[3] = node(3, -20.0, 8.0, [3, 7, 0, 0], 2, [2, 1, 0, 0]);
    nodes[4] = node(4, -6.0, -30.0, [7, 0, 0, 0], 1, [2, 0, 0, 0]);
    nodes[5] = node(5, 8.0, -4.0, [4, 13, 0, 0], 1, [2, 1, 0, 0]);
    nodes[7] = node(7, -5.0, 55.0, [14, 0, 0, 0], 1, [1, 0, 0, 0]);
    nodes[8] = node(8, -30.0, 80.0, [2, 11, 0, 0], 1, [1, 1, 0, 0]);
    nodes[11] = node(11, -35.0, 105.0, [0; 4], 0, [0; 4]);
    let mut paths: Vec<MapPath> = (0..32)
        .map(|i| path((i / 16) as u8, (i % 16) as u8, Vec::new(), [0, 0], [0, 0]))
        .collect();
    paths[1] = path(0, 1, vec![[-80, 60], [-38, 40]], [1, 2], [1, 2]);
    paths[2] = path(0, 2, vec![[-80, 60], [-30, 80]], [1, 8], [1, 8]);
    paths[3] = path(0, 3, vec![[-38, 40], [-28, 24], [-20, 8]], [2, 3], [3, 2]);
    paths[4] = path(0, 4, vec![[-38, 40], [-20, 26], [8, -4]], [2, 5], [5, 2]);
    paths[7] = path(0, 7, vec![[-20, 8], [-6, -30]], [3, 4], [4, 3]);
    paths[11] = path(0, 11, vec![[-30, 80], [-5, 55]], [8, 7], [8, 7]);
    paths[12] = path(0, 12, vec![[-38, 40], [-5, 55]], [2, 7], [2, 7]);
    paths[13] = path(0, 13, vec![[8, -4], [-5, 55]], [5, 7], [5, 7]);
    paths[14] = path(0, 14, vec![[-5, 55], [-35, 105]], [7, 11], [7, 11]);
    let text = |node: u8, speaker: u16, base: u16, groups: [u16; 3]| NodeText {
        node,
        speaker_message: Some(speaker),
        place_message: Some(speaker + 22),
        opponent: Some((speaker - 197) as u8),
        encounter_base: Some(base),
        encounter_groups: Some(groups[0]),
        loss_base: Some(base + 10),
        win_base: Some(base + 20),
        loss_groups: Some(groups[1]),
        win_groups: Some(groups[2]),
        conditional: false,
        second_visit: None,
    };
    let mut texts = vec![
        text(2, 199, 1000, [1, 1, 1]),
        text(3, 206, 1100, [1, 1, 2]),
        text(4, 205, 1150, [1, 1, 1]),
        text(5, 201, 1250, [1, 1, 1]),
        text(6, 204, 1350, [1, 1, 1]),
        text(7, 207, 1200, [1, 1, 1]),
        text(8, 200, 1450, [1, 1, 1]),
        text(9, 203, 1300, [1, 1, 1]),
        text(10, 202, 1400, [1, 1, 1]),
        text(11, 198, 1500, [1, 1, 1]),
    ];
    texts[9].conditional = true;
    texts[9].second_visit = Some(NodeTextVariant {
        speaker_message: Some(217),
        place_message: Some(239),
        opponent: Some(20),
        encounter_base: Some(1600),
        encounter_groups: Some(1),
        loss_base: Some(1610),
        loss_groups: Some(2),
        win_base: Some(1620),
        win_groups: Some(1),
    });
    // Three-, one-, three- and two-line pages: 19, 7, 19 and 13 reveal frames.
    let mut messages = BTreeMap::from([
        (1001, message(false, 3)),
        (1002, message(false, 1)),
        (1011, message(false, 3)),
        (1021, message(false, 2)),
        (CAPTION_MESSAGE, message(false, 1)),
        // The story windows' messages: two, one, three (the 11149 counter)
        // and one line(s), as the traced reveals (13, 7, 19 updates).
        (2648, message(false, 2)),
        (2656, message(false, 1)),
        (2804, message(false, 3)),
        (2828, message(false, 1)),
    ]);
    // Every other counter of a text base is a blank page; node 3's win
    // text (two groups) has pages 1121, 1125 and 1126.
    let mut fill = |base: u16, groups: u16, pages: &[u16]| {
        for counter in 1..=groups * PAGES_PER_GROUP {
            messages
                .entry(base + counter)
                .or_insert_with(|| message(!pages.contains(&(base + counter)), 1));
        }
    };
    for t in &texts {
        let v = t.first_visit();
        let enc = v.encounter_base.unwrap();
        fill(enc, v.encounter_groups.unwrap(), &[enc + 1]);
        let loss = v.loss_base.unwrap();
        fill(loss, v.loss_groups.unwrap(), &[loss + 1]);
        let win = v.win_base.unwrap();
        fill(win, v.win_groups.unwrap(), &[win + 1, win + 5, win + 6]);
        if t.second_visit.is_some() {
            fill(1600, 1, &[1601]);
            fill(1610, 2, &[1611, 1615]);
            fill(1620, 1, &[1621]);
        }
    }
    MapData {
        nodes,
        paths,
        texts,
        rose_card_nodes: vec![2, 4, 5, 6, 8, 9, 10, 11],
        messages,
        ..MapData::default()
    }
}

/// A save after the duel at `node` (states as given), the duel end applied.
fn after_duel_at(data: &MapData, states: &[(u8, u8)], node: u8, won: bool, c9: u8) -> CampaignSave {
    let mut save = CampaignSave::after_landing();
    for (n, s) in states {
        save.node_states[usize::from(*n)] = *s;
    }
    save.hovered = node;
    save.destination = node;
    save.location = node;
    save.c9 = c9;
    let variant = data.text_variant(node, save.state(node), c9).unwrap();
    save.opponent = i16::from(variant.opponent.unwrap());
    save.apply_duel_end(data, won);
    save
}

/// Run a model with immediate page presses until the handler is free and
/// the map is still, or a story event suspended it.
fn run_until_still(m: &mut MapModel, limit: u64) -> Vec<(u64, MapEvent)> {
    let mut log = Vec::new();
    let mut last_press = 0;
    for _ in 0..limit {
        let pad = if m.text_phase() == 4 && m.frame - last_press > 3 {
            last_press = m.frame + 1;
            Pad::CROSS
        } else {
            Pad::NONE
        };
        for event in m.step(pad) {
            log.push((m.frame, event));
        }
        if matches!(m.handler, Handler::Story { .. }) {
            break;
        }
        if matches!(m.handler, Handler::Free)
            && m.frame > 20
            && m.text.is_none()
            && m.paths.iter().all(|p| p.state != 1 && p.state != 2)
            && m.rises.iter().all(|r| !r.rising)
        {
            break;
        }
    }
    log
}

fn at(log: &[(u64, MapEvent)], event: &MapEvent) -> Option<u64> {
    log.iter().find(|(_, e)| e == event).map(|(f, _)| *f)
}

#[test]
fn dialogue_pages_skip_blanks_and_close_after_four_per_group() {
    let data = fixture();
    // Node 3's win text: 1121, then blanks 1122..1124 skipped, 1125, 1126, blanks to 1128.
    assert_eq!(data.dialogue_pages(1120, 2), (vec![1121, 1125, 1126], None));
    assert_eq!(data.dialogue_pages(1120, 1), (vec![1121], None));
    assert_eq!(data.dialogue_pages(5000, 1), (vec![], Some(5001)));
    let mut m = MapModel::at_event_zero(data).unwrap();
    m.start_text(1120, 2, &mut Vec::new());
    let mut pages = vec![1121];
    let mut last = 0;
    for _ in 0..300 {
        let pad = if m.text_phase() == 4 && m.frame - last > 3 {
            last = m.frame + 1;
            Pad::CROSS
        } else {
            Pad::NONE
        };
        for e in m.step(pad) {
            if let MapEvent::TextPage { message, .. } = e {
                pages.push(message);
            }
        }
        if m.text.is_none() {
            break;
        }
    }
    assert_eq!(pages, [1121, 1125, 1126]);
    assert!(m.text.is_none());
    assert_eq!(m.notes, Vec::<String>::new());
}

#[test]
fn a_win_at_another_node_reveals_with_the_generic_handler() {
    // Pegasus beaten at node 3 after Chester: event 304 → 104 reveals path 7
    // (node 4 rises) and path 3 (already drawn, alternate end 2 open).
    let data = fixture();
    let save = after_duel_at(&data, &[(2, 3), (3, 1), (5, 1)], 3, true, 0);
    assert_eq!(save.event, 304);
    assert_eq!(save.wins_by_opponent[9], 1);
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until_still(&mut m, 800);
    assert_eq!(
        at(
            &log,
            &MapEvent::TextPage {
                message: 1121,
                page: 1
            }
        ),
        Some(15)
    );
    // Two groups: 1125 and 1126 follow before the window closes.
    assert!(at(
        &log,
        &MapEvent::TextPage {
            message: 1126,
            page: 6
        }
    )
    .is_some());
    assert!(
        at(&log, &MapEvent::RoseCard { node: 3 }).is_none(),
        "node 3 is not a rose node"
    );
    let reveal = at(&log, &MapEvent::EventChanged(104)).unwrap();
    assert_eq!(at(&log, &MapEvent::NodeBeaten(3)), Some(reveal));
    assert_eq!(at(&log, &MapEvent::PathRevealing(7)), Some(reveal + 15));
    assert!(at(&log, &MapEvent::PathRevealing(3)).is_none());
    assert_eq!(
        at(&log, &MapEvent::EventChanged(0)),
        Some(reveal + 15 + 1 + 32 + 1)
    );
    let n7 = m.data.paths[7].dots.len() as u64;
    assert_eq!(
        at(&log, &MapEvent::NodeRevealed(4)),
        Some(reveal + 15 + 4 * n7 + 15 + 64)
    );
    assert_eq!(
        (m.save.state(3), m.save.state(4)),
        (STATE_BEATEN, STATE_OPEN)
    );
    assert!([1usize, 2, 3, 4, 7].iter().all(|p| m.paths[*p].state == 3));
    assert_eq!(m.notes, Vec::<String>::new());
    // A loss at node 3: state 2, C2 = 0 after the one-group loss text.
    let data = fixture();
    let save = after_duel_at(&data, &[(2, 3), (3, 1), (5, 1)], 3, false, 0);
    assert_eq!(save.event, 204);
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until_still(&mut m, 400);
    let closed = at(&log, &MapEvent::TextClosed).unwrap();
    assert_eq!(at(&log, &MapEvent::NodeLost(3)), Some(closed + 10));
    assert_eq!(m.save.state(3), STATE_LOST);
}

#[test]
fn an_arrival_event_keeps_c2_and_frees_the_cursor_after_two_pans() {
    let data = fixture();
    let save = after_duel_at(&data, &[(2, 3), (8, 3), (9, 1)], 9, true, 0);
    assert_eq!(save.event, 310);
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until_still(&mut m, 600);
    let closed = at(&log, &MapEvent::TextClosed).unwrap();
    let rose = at(&log, &MapEvent::RoseCard { node: 9 }).unwrap();
    assert_eq!(rose, closed + 10);
    let arrive = at(&log, &MapEvent::EventChanged(110)).unwrap();
    assert_eq!(arrive, rose + 128);
    // 17 frames of the close logic's pan, 0x228410, 17 frames more.
    assert_eq!(
        at(&log, &MapEvent::Arrived(9)),
        Some(arrive + u64::from(ARRIVE_FIRST_FRAMES + ARRIVE_PAN_FRAMES))
    );
    assert!(at(&log, &MapEvent::EventChanged(0)).is_none());
    assert_eq!(m.save.event, 110);
    assert!(matches!(m.handler, Handler::Free));
    assert_eq!(m.save.state(9), STATE_BEATEN);
    // The free cursor accepts a press with C2 = 110 (< 200).
    m.camera_target = m.node_position(2);
    m.save.location = 2;
    m.update_anchors();
    hover_node_two(&mut m);
    m.step(Pad::CROSS);
    assert_eq!(m.travel, 1);
    // A rematch win at the beaten node leaves C2 = 0 (0x227F00).
    let data = fixture();
    let mut save = after_duel_at(&data, &[(2, 3), (8, 3), (9, 3)], 9, true, 0);
    assert_eq!(save.event, 0);
    save.apply_duel_end(&data, false);
    assert_eq!((save.event, save.losses), (0, 1));
}

#[test]
fn the_eighth_win_fires_the_bosworth_trigger_and_event_11124_reveals_bosworth() {
    let data = fixture();
    let beaten: Vec<(u8, u8)> = [2u8, 3, 4, 5, 6, 8, 9].iter().map(|n| (*n, 3)).collect();
    let mut states = beaten.clone();
    states.push((10, 1));
    let save = after_duel_at(&data, &states, 10, true, 0);
    assert_eq!(save.event, 311);
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until_still(&mut m, 800);
    let arrive = at(&log, &MapEvent::EventChanged(111)).unwrap();
    // The trigger fires in 0x228410 after the close logic's pan.
    assert!(at(&log, &MapEvent::StoryEvent(11120)).is_none());
    assert_eq!(
        at(&log, &MapEvent::EventChanged(11120)),
        Some(arrive + u64::from(ARRIVE_FIRST_FRAMES))
    );
    assert!(at(&log, &MapEvent::Arrived(10)).is_none());
    // The map plays 11120's window, then hands 11122 (paintings) to the
    // story scene 54 frames after the trigger (f221 → f275 [emu]).
    let trigger = arrive + u64::from(ARRIVE_FIRST_FRAMES);
    assert_eq!(at(&log, &MapEvent::StoryEvent(11122)), Some(trigger + 54));
    assert_eq!(m.save.event, 11122);
    assert_eq!(m.take_story_request(), Some(11122));
    assert!(matches!(m.handler, Handler::Story { event: 11122 }));
    assert!(m.step(Pad::CROSS).is_empty());
    assert_eq!(m.save.beaten_count(), 8);
    // Not fired with seven wins.
    let data = fixture();
    let save = after_duel_at(&data, &beaten[..6], 9, true, 0);
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until_still(&mut m, 800);
    assert!(at(&log, &MapEvent::EventChanged(11120)).is_none());
    assert!(at(&log, &MapEvent::Arrived(9)).is_some());
    // After the story pages: resume at 11124, paths 12/13/11 draw, Bosworth rises.
    let data = fixture();
    let mut save = CampaignSave::after_landing();
    for (n, s) in &states {
        save.node_states[usize::from(*n)] = *s;
    }
    save.node_states[10] = STATE_BEATEN;
    save.location = 10;
    save.event = EVENT_BOSWORTH_REVEAL;
    assert!(!save.drawn_paths(&data).contains(&12));
    let mut m = MapModel::resume(data, save).unwrap();
    assert_eq!(m.camera_target, m.node_position(10));
    let log = run_until_still(&mut m, 800);
    // Camera cut at frame 1, window 2656, letterbox off: paths at f455 −
    // f416 [emu].
    let paths = at(&log, &MapEvent::PathRevealing(12)).unwrap();
    assert_eq!(paths, 39);
    assert_eq!(m.camera_target, m.node_position(7));
    assert_eq!(at(&log, &MapEvent::PathRevealing(13)), Some(paths));
    assert_eq!(at(&log, &MapEvent::PathRevealing(11)), Some(paths));
    let first_done = log
        .iter()
        .filter_map(|(f, e)| matches!(e, MapEvent::PathRevealed(_)).then_some(*f))
        .min()
        .unwrap();
    assert_eq!(at(&log, &MapEvent::NodeRising(7)), Some(first_done));
    assert_eq!(at(&log, &MapEvent::EventChanged(0)), Some(first_done + 1));
    assert_eq!(at(&log, &MapEvent::NodeRevealed(7)), Some(first_done + 64));
    assert_eq!(m.save.state(7), STATE_OPEN);
    assert!([11usize, 12, 13].iter().all(|p| m.paths[*p].state == 3));
    // Re-entry afterwards draws paths 11-13 (eight beaten).
    let mut save = m.save.clone();
    save.event = 0;
    assert!(save.drawn_paths(&m.data).contains(&12));
}

#[test]
fn bosworth_and_stonehenge_wins_fire_their_story_events() {
    let data = fixture();
    let eight: Vec<(u8, u8)> = [2u8, 3, 4, 5, 6, 8, 9, 10]
        .iter()
        .map(|n| (*n, 3))
        .collect();
    // Richard beaten: 308 → no rose card → 11140, state 3.
    let mut states = eight.clone();
    states.push((7, 1));
    let save = after_duel_at(&data, &states, 7, true, 0);
    assert_eq!(save.event, 308);
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until_still(&mut m, 600);
    assert!(at(&log, &MapEvent::RoseCard { node: 7 }).is_none());
    let closed = at(&log, &MapEvent::TextClosed).unwrap();
    assert_eq!(at(&log, &MapEvent::StoryEvent(11140)), Some(closed + 10));
    assert_eq!(m.save.state(7), STATE_BEATEN);
    // 11149 → 108: node 7's path 14 reveals Stonehenge.
    let data = fixture();
    let mut save = m.save.clone();
    save.event = EVENT_STONEHENGE_REVEAL;
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until_still(&mut m, 800);
    assert_eq!(at(&log, &MapEvent::PathRevealing(14)), Some(16));
    assert_eq!(at(&log, &MapEvent::EventChanged(0)), Some(50));
    assert_eq!(m.save.state(11), STATE_OPEN);
    // The same after the map played 11149's window (108 at f1787 − f1750).
    let data = fixture();
    let mut save = m.save.clone();
    save.node_states[11] = STATE_HIDDEN;
    save.event = EVENT_BOSWORTH_FAREWELL;
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until_still(&mut m, 800);
    assert_eq!(at(&log, &MapEvent::EventChanged(108)), Some(37));
    assert_eq!(at(&log, &MapEvent::PathRevealing(14)), Some(37 + 16));
    assert_eq!(at(&log, &MapEvent::EventChanged(0)), Some(37 + 50));
    assert_eq!(m.save.state(11), STATE_OPEN);
    // Seto: 312 → 8th rose → 11160 (state 3).
    let data = fixture();
    let mut states = eight.clone();
    states.extend([(7, 3), (11, 1)]);
    let save = after_duel_at(&data, &states, 11, true, 0);
    assert_eq!((save.event, save.opponent), (312, 1));
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until_still(&mut m, 600);
    assert_eq!(
        at(
            &log,
            &MapEvent::TextPage {
                message: 1521,
                page: 1
            }
        ),
        Some(15)
    );
    let rose = at(&log, &MapEvent::RoseCard { node: 11 }).unwrap();
    assert_eq!(at(&log, &MapEvent::StoryEvent(11160)), Some(rose + 128));
    assert_eq!(m.save.state(11), STATE_BEATEN);
    assert_eq!(m.save.rose_cards(&m.data), 8);
    // 11162 → 112: C9 = 1, arrival, then the hover shows Manawyddan.
    let data = fixture();
    let mut save = m.save.clone();
    save.event = EVENT_STONEHENGE_ARRIVE;
    let mut m = MapModel::resume(data, save).unwrap();
    assert_eq!((m.save.c9, m.w53c4), (1, 1));
    let log = run_until_still(&mut m, 200);
    assert_eq!(
        at(&log, &MapEvent::Arrived(11)),
        Some(u64::from(ARRIVE_FIRST_FRAMES + ARRIVE_PAN_FRAMES))
    );
    assert_eq!(m.save.event, 112);
    assert_eq!(
        m.data
            .text_variant(11, m.save.state(11), m.w53c4)
            .unwrap()
            .opponent,
        Some(20)
    );
    assert_eq!(
        m.data.text_variant(11, STATE_BEATEN, 0).unwrap().opponent,
        Some(1)
    );
    // Cross on Stonehenge at state 3 with W+53C4 == 1 starts the Manawyddan text.
    let anchor = m.anchors[11].unwrap().0;
    m.cursor = (anchor.0 as i16, anchor.1 as i16);
    m.step(Pad::NONE);
    assert_eq!((m.hovered, m.save.opponent), (Some(11), 20));
    while m.hud.panel < HUD_SLIDE_FULL {
        m.step(Pad::NONE);
    }
    let events = m.step(Pad::CROSS);
    assert!(events.contains(&MapEvent::TextPage {
        message: 1601,
        page: 1
    }));
    m.step(Pad::NONE);
    assert_eq!(m.save.c9, 1);
    // Manawyddan beaten: 313, win text 1621, no rose, 11180, state 5.
    let data = fixture();
    let mut states = eight.clone();
    states.extend([(7, 3), (11, 3)]);
    let save = after_duel_at(&data, &states, 11, true, 1);
    assert_eq!((save.event, save.opponent), (313, 20));
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until_still(&mut m, 600);
    assert_eq!(
        at(
            &log,
            &MapEvent::TextPage {
                message: 1621,
                page: 1
            }
        ),
        Some(15)
    );
    assert!(at(&log, &MapEvent::RoseCard { node: 11 }).is_none());
    let closed = at(&log, &MapEvent::TextClosed).unwrap();
    assert_eq!(at(&log, &MapEvent::StoryEvent(11180)), Some(closed + 10));
    assert_eq!(m.save.state(11), STATE_BEATEN_SECOND);
    // Lost to Manawyddan: 213, two-group loss text, state 4, C2 = 0.
    let data = fixture();
    let save = after_duel_at(&data, &states, 11, false, 1);
    assert_eq!(save.event, 213);
    let mut m = MapModel::resume(data, save).unwrap();
    let log = run_until_still(&mut m, 600);
    assert!(at(
        &log,
        &MapEvent::TextPage {
            message: 1615,
            page: 5
        }
    )
    .is_some());
    assert_eq!(
        at(&log, &MapEvent::NodeLost(11)),
        at(&log, &MapEvent::EventChanged(0))
    );
    assert_eq!((m.save.state(11), m.save.event), (STATE_LOST_SECOND, 0));
    // Lost to Seto: 212, state 2.
    let data = fixture();
    let mut states = eight.clone();
    states.extend([(7, 3), (11, 1)]);
    let save = after_duel_at(&data, &states, 11, false, 0);
    assert_eq!(save.event, 212);
    let mut m = MapModel::resume(data, save).unwrap();
    run_until_still(&mut m, 600);
    assert_eq!((m.save.state(11), m.save.event), (STATE_LOST, 0));
    // A save after the ending (SAVE+0x3C = 0) does not resume the map
    // (Continue takes the route switch 0x21C734); SAVE[1] alone does not
    // stop it (the red route replayed after the switch keeps SAVE[1]).
    let data = fixture();
    let mut save = CampaignSave::after_landing();
    save.red_route_cleared = true;
    assert!(MapModel::resume(data.clone(), save.clone()).is_ok());
    save.map_entered = false;
    assert!(MapModel::resume(data, save).is_err());
}

#[test]
fn natural_spline_is_linear_for_two_points_and_solves_the_tridiagonal_system() {
    let dots = spline_dots(&[[-15, 5], [-5, -35]], false);
    assert_eq!(dots.len(), 10);
    for (i, d) in dots.iter().enumerate() {
        assert!((d[0] - (-15.0 + i as f32)).abs() < 1e-5);
        assert!((d[1] - (5.0 - 4.0 * i as f32)).abs() < 1e-5);
    }
    let points = [[-75.0, 65.0], [-50.0, 60.0], [-30.0, 40.0]];
    let m = natural_spline_tangents(&points);
    let c0 = [25.0, -5.0];
    let c1 = [20.0, -20.0];
    for i in 0..2 {
        assert!((2.0 * m[0][i] + m[1][i] - 3.0 * c0[i]).abs() < 1e-9);
        assert!((m[0][i] + 4.0 * m[1][i] + m[2][i] - 3.0 * (c0[i] + c1[i])).abs() < 1e-9);
        assert!((m[1][i] + 2.0 * m[2][i] - 3.0 * c1[i]).abs() < 1e-9);
    }
    let dots = spline_dots(&[[-75, 65], [-50, 60], [-30, 40]], false);
    assert_eq!(dots.len(), 6 + 7);
    assert_eq!(dots[6], [-50.0, 60.0]);
    let sea = spline_dots(&[[25, 111], [38, 109], [53, 108]], true);
    assert_eq!(sea.len(), 18);
    assert_eq!(sea[9], [38.0, 109.0]);
    assert!(spline_dots(&[[1, 1]], false).is_empty());
    assert!(spline_dots(&[], true).is_empty());
}

#[test]
fn projection_floors_anchors_and_scales_the_hover_radius() {
    let target = [-27.5, 57.5];
    let n = node(2, -30.0, 40.0, [0; 4], 0, [0; 4]);
    let ((x, y), radius) = node_anchor(target, &n).unwrap();
    // Camera space of the anchor: xc = -2.5, zc = (103.35 + 129.5)/√2.
    let zc = (112.0 - ANCHOR_HEIGHT + 129.5) / std::f64::consts::SQRT_2;
    assert_eq!((x, y), (243, 135));
    assert!((radius - PROJECTION_X * 5.0 / zc).abs() < 1e-9);
    assert!((radius - 24.29).abs() < 0.01);
    let behind = node(0, 0.0, 0.0, [0; 4], 0, [0; 4]);
    assert!(node_anchor([0.0, -300.0], &behind).is_none());
    assert_eq!(camera_eye([1.0, 2.0]), [1.0, 112.0, 114.0]);
}

#[test]
fn cursor_moves_sixteen_per_frame_and_hovers_within_the_radius() {
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    assert_eq!(m.cursor, CURSOR_START);
    assert_eq!(m.fade, FADE_FULL);
    let anchor = m.anchors[2].unwrap();
    // Node 2 is eligible (state 1); node 3 is hidden and has no anchor.
    assert!(m.anchors[3].is_none());
    let mut hovered_at = None;
    for frame in 1..=12 {
        let events = m.step(Pad::UP);
        assert_eq!(m.cursor, (256, 222 - 16 * frame));
        if events.contains(&MapEvent::Hover(Some(2))) {
            hovered_at = Some(frame);
        }
    }
    let frame = hovered_at.expect("node 2 hovered");
    let dy = f64::from(222 - 16 * i32::from(frame) - anchor.0 .1);
    let dx = f64::from(256 - anchor.0 .0);
    assert!((dx * dx + dy * dy).sqrt() < anchor.1);
    let before = f64::from(222 - 16 * (i32::from(frame) - 1) - anchor.0 .1);
    assert!((dx * dx + before * before).sqrt() >= anchor.1);
    assert_eq!(m.save.hovered, 2);
    assert_eq!(m.save.opponent, 2);
    // Clamped at the top edge; the camera pans 4.0 per frame there.
    let z = m.camera_target[1];
    for _ in 0..10 {
        m.step(Pad::UP);
    }
    assert_eq!(m.cursor.1, 0);
    assert!(m.camera_target[1] < z);
    // Hidden nodes are never hovered.
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    m.save.node_states[2] = STATE_HIDDEN;
    for _ in 0..12 {
        let events = m.step(Pad::UP);
        assert!(!events.iter().any(|e| matches!(e, MapEvent::Hover(Some(2)))));
    }
}

/// Move the cursor onto node 2, then wait for the hover panel to slide in
/// (Cross needs W+6C = 0x100).
fn hover_node_two(m: &mut MapModel) {
    for _ in 0..40 {
        m.step(Pad::UP);
        if m.hovered == Some(2) {
            for _ in 0..20 {
                if m.hud.panel >= HUD_SLIDE_FULL {
                    return;
                }
                m.step(Pad::NONE);
            }
            panic!("the hover panel did not slide in");
        }
    }
    panic!("node 2 not hovered");
}

#[test]
fn cross_on_an_open_node_shows_the_taunt_walks_and_requests_the_duel() {
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    hover_node_two(&mut m);
    let press = m.frame + 1;
    let events = m.step(Pad::CROSS);
    assert!(events.contains(&MapEvent::Sfx(SFX_SELECT)));
    assert!(events.contains(&MapEvent::TextPage {
        message: 1001,
        page: 1
    }));
    assert_eq!(m.travel, 1);
    assert_eq!(m.text_phase(), 2);
    assert_eq!(
        m.cursor,
        (
            m.anchors[2].unwrap().0 .0 as i16,
            m.anchors[2].unwrap().0 .1 as i16
        )
    );
    let events = m.step(Pad::NONE);
    assert!(events.contains(&MapEvent::TravelStarted {
        from: 1,
        to: 2,
        path: Some((1, true))
    }));
    assert_eq!((m.travel, m.figure.state, m.save.destination), (2, 1, 2));
    assert_eq!(m.window_openness(), 20);
    assert!(m.lerp.as_ref().is_some_and(|l| l.t == 0));
    // Window opens over five frames, then the three-line page reveals in 19.
    let mut pages = Vec::new();
    let mut last_press = press;
    let mut request = None;
    let mut closed = None;
    let mut arrived = None;
    for _ in 0..200 {
        let pad = if m.text_phase() == 4 && m.frame - last_press > 3 {
            last_press = m.frame + 1;
            Pad::CROSS
        } else {
            Pad::NONE
        };
        let events = m.step(pad);
        for event in events {
            match event {
                MapEvent::TextPage { message, .. } => pages.push((m.frame, message)),
                MapEvent::DuelRequest(r) => request = Some((m.frame, r)),
                MapEvent::TextClosed => closed = Some(m.frame),
                _ => {}
            }
        }
        if m.figure.state == 3 && arrived.is_none() {
            arrived = Some(m.frame);
        }
        if m.frame == press + 5 {
            assert_eq!((m.window_openness(), m.text_phase()), (100, 3));
        }
        if m.frame == press + 5 + 19 {
            assert_eq!(m.text_phase(), 4);
        }
        if request.is_some() {
            break;
        }
    }
    // Camera lerp: 32 frames from the frame after the travel start.
    assert_eq!(m.camera_target, [-38.0, 40.0]);
    assert_eq!(pages, [(press + 30, 1002)]);
    let (frame, request) = request.expect("duel requested");
    assert_eq!(request.params, [2, 0, DUEL_PARAM_MODE, 1002]);
    assert_eq!(request.node, 2);
    // Walk: 11 dots × 4 frames from the travel-start frame; here the window
    // (two pages, 5 + 19 + 5 + 1 + 5 + 7 + 5 + 1 + 5 frames) closes later and
    // the request follows the close.
    assert_eq!(m.data.paths[1].dots.len(), 11);
    assert_eq!(arrived, Some(press + 1 + 11 * 4 - 1));
    assert_eq!(closed, Some(press + 50));
    assert_eq!(frame, press + 51);
    assert!(m.text.is_none());
    assert_eq!(m.travel, 4);
    assert_eq!(m.figure.position, [-38.0, 40.0]);
    assert_eq!(m.notes, Vec::<String>::new());
}

#[test]
fn rematch_after_a_loss_travels_without_text() {
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    m.save.node_states[2] = STATE_LOST;
    m.save.location = 2;
    m.camera_target = [-38.0, 40.0];
    hover_node_two(&mut m);
    m.step(Pad::CROSS);
    assert!(m.text.is_none());
    assert_eq!(m.travel, 1);
    let press = m.frame;
    m.step(Pad::NONE);
    assert_eq!((m.travel, m.figure.state), (2, 3));
    let events = m.step(Pad::NONE);
    assert!(events.iter().any(|e| matches!(e, MapEvent::DuelRequest(_))));
    assert_eq!(m.frame, press + 2);
    // Over DC: SFX 6 and nothing else.
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    m.player_dc = Some(900);
    m.opponent_dc = Some(854);
    hover_node_two(&mut m);
    let events = m.step(Pad::CROSS);
    assert!(events.contains(&MapEvent::OverDc));
    assert_eq!(m.travel, 0);
    assert!(m.over_dc);
}

fn run_return(won: bool) -> (MapModel, Vec<(u64, MapEvent)>) {
    let mut m = MapModel::after_duel(fixture(), won).unwrap();
    let mut log = Vec::new();
    let mut last_press = 0;
    for _ in 0..600 {
        let pad = if m.text_phase() == 4 && m.frame - last_press > 3 {
            last_press = m.frame + 1;
            Pad::CROSS
        } else {
            Pad::NONE
        };
        for event in m.step(pad) {
            log.push((m.frame, event));
        }
        if m.save.event == 0
            && m.frame > 20
            && m.camera_phase() == 0
            && m.text.is_none()
            && m.paths.iter().all(|p| p.state != 1 && p.state != 2)
            && m.rises.iter().all(|r| !r.rising)
        {
            break;
        }
    }
    (m, log)
}

#[test]
fn a_win_reveals_the_next_nodes_and_counts_a_rose_card() {
    let (m, log) = run_return(true);
    let at = |event: &MapEvent| log.iter().find(|(_, e)| e == event).map(|(f, _)| *f);
    assert_eq!(m.save.wins, 1);
    assert_eq!(m.save.wins_by_opponent[2], 1);
    assert_eq!(m.save.duel_count, 1);
    assert_eq!((m.save.location, m.save.destination), (2, 2));
    // Text after the 13-frame fade-in (fade restarts on the handler's first frame).
    assert_eq!(
        at(&MapEvent::TextPage {
            message: 1021,
            page: 1
        }),
        Some(15)
    );
    let closed = at(&MapEvent::TextClosed).unwrap();
    let rose = at(&MapEvent::RoseCard { node: 2 }).unwrap();
    // Countdown 100 → 0 by 10 after the window closed, then 128 frames of animation.
    assert_eq!(rose, closed + 10);
    assert_eq!(at(&MapEvent::EventChanged(103)), Some(rose + 128));
    assert_eq!(at(&MapEvent::NodeBeaten(2)), Some(rose + 128));
    let reveal = rose + 128;
    assert_eq!(at(&MapEvent::PathRevealing(3)), Some(reveal + 15));
    assert_eq!(at(&MapEvent::PathRevealing(4)), Some(reveal + 15));
    assert_eq!(
        at(&MapEvent::EventChanged(0)),
        Some(reveal + 15 + 1 + 32 + 1)
    );
    // Dots: 4 frames apart, 16 frames each.
    let n3 = m.data.paths[3].dots.len() as u64;
    let n4 = m.data.paths[4].dots.len() as u64;
    assert_eq!((n3, n4), (8, 15));
    assert_eq!(
        at(&MapEvent::PathRevealed(3)),
        Some(reveal + 15 + 4 * n3 + 15)
    );
    assert_eq!(
        at(&MapEvent::PathRevealed(4)),
        Some(reveal + 15 + 4 * n4 + 15)
    );
    assert_eq!(
        at(&MapEvent::NodeRevealed(3)),
        Some(reveal + 15 + 4 * n3 + 15 + 64)
    );
    assert_eq!(
        at(&MapEvent::NodeRevealed(5)),
        Some(reveal + 15 + 4 * n4 + 15 + 64)
    );
    assert_eq!(m.save.state(2), STATE_BEATEN);
    assert_eq!((m.save.state(3), m.save.state(5)), (STATE_OPEN, STATE_OPEN));
    assert_eq!(m.save.rose_cards(&m.data), 1);
    assert_eq!(m.camera_target, [(-20.0 + 8.0) / 2.0, (8.0 - 4.0) / 2.0]);
    assert_eq!(m.save.event, 0);
    assert!(matches!(m.handler, Handler::Free));
    // Paths 1 and 2 (Milford Haven beaten) and 3/4 are drawn.
    assert!([1usize, 2, 3, 4].iter().all(|p| m.paths[*p].state == 3));
    assert!(m.paths[3].alpha.iter().all(|a| *a == DOT_FULL_ALPHA));
    assert_eq!(m.notes, Vec::<String>::new());
}

#[test]
fn a_loss_marks_the_node_lost_and_returns_to_the_free_map() {
    let (m, log) = run_return(false);
    let at = |event: &MapEvent| log.iter().find(|(_, e)| e == event).map(|(f, _)| *f);
    assert_eq!(m.save.losses, 1);
    assert_eq!(m.save.losses_by_opponent[2], 1);
    assert_eq!(
        at(&MapEvent::TextPage {
            message: 1011,
            page: 1
        }),
        Some(15)
    );
    let closed = at(&MapEvent::TextClosed).unwrap();
    assert_eq!(at(&MapEvent::NodeLost(2)), Some(closed + 10));
    assert_eq!(at(&MapEvent::EventChanged(0)), Some(closed + 10));
    assert!(at(&MapEvent::RoseCard { node: 2 }).is_none());
    assert_eq!(m.save.state(2), STATE_LOST);
    assert_eq!(m.save.rose_cards(&m.data), 0);
    assert_eq!(m.paths[3].state, 0);
    assert_eq!(m.save.event, 0);
    // The free map accepts a rematch press afterwards (node state 2, no text).
    let mut m = m;
    hover_node_two(&mut m);
    m.step(Pad::CROSS);
    assert_eq!(m.travel, 1);
    assert!(m.text.is_none());
}

#[test]
fn save_helpers_and_validation() {
    let data = fixture();
    let mut save = CampaignSave::after_landing();
    assert_eq!(save.drawn_paths(&data), [1, 2]);
    save.node_states[2] = STATE_BEATEN;
    assert_eq!(save.drawn_paths(&data), [1, 2, 3, 4]);
    assert_eq!(save.rose_cards(&data), 1);
    assert_eq!(data.pages(1000), (vec![1001, 1002], None));
    assert_eq!(data.pages(1020), (vec![1021], None));
    assert_eq!(data.pages(5000), (vec![], Some(5001)));
    assert_eq!(data.path_between(1, 2), Some((1, true)));
    assert_eq!(data.path_between(2, 1), Some((1, false)));
    assert_eq!(data.path_between(2, 8), None);
    assert_eq!(data.reveal_end(3, 1), 3);
    assert_eq!(data.reveal_end(3, 2), 2);
    let mut bad = fixture();
    bad.nodes.pop();
    assert!(bad.validate().is_err());
    let mut bad = fixture();
    bad.paths[1].ends = [22, 0];
    assert!(bad.validate().is_err());
    let mut resumed = CampaignSave::after_landing();
    resumed.event = 500;
    assert!(MapModel::resume(fixture(), resumed).is_err());
}

#[cfg(test)]
mod private_tests {
    use super::*;

    fn data() -> MapData {
        let path = std::env::var_os("ROSES_CAMPAIGN_MAP_DATA").expect("ROSES_CAMPAIGN_MAP_DATA");
        load_map_data(std::path::Path::new(&path)).unwrap()
    }

    #[test]
    #[ignore = "Requires the private campaign-map import (ROSES_CAMPAIGN_MAP_DATA = map-data.json)"]
    fn private_hover_and_chester_match_the_emulator_logs() {
        // logs/hover-622.txt: Up held from (256,222), Chester hovered at frame 5.
        let mut m = MapModel::at_event_zero(data()).unwrap();
        assert_eq!(m.camera_target, [-27.5, 57.5]);
        let anchors: Vec<_> = m.anchors.iter().map(|a| a.map(|(p, _)| p)).collect();
        assert_eq!(anchors[0], Some((-567, 1573)));
        assert_eq!(anchors[1], Some((-3, 250)));
        assert_eq!(anchors[2], Some((243, 135)));
        assert_eq!(anchors[8], Some((270, 304)));
        assert_eq!(anchors[11], Some((205, 504)));
        assert!(anchors[3].is_none() && anchors[9].is_none());
        let mut hover = Vec::new();
        for _ in 0..12 {
            for event in m.step(Pad::UP) {
                if let MapEvent::Hover(h) = event {
                    hover.push((m.frame, h));
                }
            }
        }
        assert_eq!(hover, [(5, Some(2)), (7, None)]);
        assert_eq!((m.save.hovered, m.save.opponent), (2, 2));
        // logs/pre-duel-652.txt: press at frame 3 (cursor (256,126) over Chester),
        // pages 2524/2525, blank 2526, push at frame 56 with (2, 0, 1100, 1002).
        let mut m = MapModel::at_event_zero(data()).unwrap();
        for _ in 0..6 {
            m.step(Pad::UP);
        }
        assert_eq!(m.cursor, (256, 126));
        // The 652 savestate had the hover panel in (W+6C = 256).
        m.hud.panel = HUD_SLIDE_FULL;
        m.frame = 2;
        let mut presses = vec![3u64];
        let mut log = Vec::new();
        m.step(Pad::CROSS);
        assert_eq!((m.travel, m.text_phase(), m.cursor), (1, 2, (243, 135)));
        loop {
            let pad = if m.text_phase() == 4 && m.frame + 1 - presses.last().unwrap() > 3 {
                presses.push(m.frame + 1);
                Pad::CROSS
            } else {
                Pad::NONE
            };
            let events = m.step(pad);
            // cur=(243,135) on every frame f003..f055 while the target lerps.
            if m.frame <= 55 {
                assert_eq!(m.cursor, (243, 135), "frame {}", m.frame);
            }
            for e in events {
                log.push((m.frame, e));
            }
            if m.duel_request.is_some() || m.frame > 200 {
                break;
            }
        }
        assert_eq!(presses, [3, 28, 42]);
        let at = |event: &MapEvent| log.iter().find(|(_, e)| e == event).map(|(f, _)| *f);
        assert_eq!(
            at(&MapEvent::TextPage {
                message: 2525,
                page: 2
            }),
            Some(33)
        );
        assert_eq!(at(&MapEvent::TextClosed), Some(53));
        let request = m.duel_request.clone().unwrap();
        assert_eq!((m.frame, request.params), (56, [2, 0, 1100, 1002]));
        assert_eq!(m.camera_target, [-30.0, 40.0]);
    }

    #[test]
    #[ignore = "Requires the private campaign-map import (ROSES_CAMPAIGN_MAP_DATA = map-data.json)"]
    fn private_return_matches_the_post_duel_logs() {
        // logs/post-win-767.txt and post-loss-767.txt (immediate page presses).
        for won in [true, false] {
            let mut m = MapModel::after_duel(data(), won).unwrap();
            assert_eq!(m.save.event, if won { 303 } else { 203 });
            let mut log = Vec::new();
            let mut last = 0;
            for _ in 0..400 {
                let pad = if m.text_phase() == 4 && m.frame + 1 - last > 3 {
                    last = m.frame + 1;
                    Pad::CROSS
                } else {
                    Pad::NONE
                };
                for e in m.step(pad) {
                    log.push((m.frame, e));
                }
            }
            let at = |event: &MapEvent| log.iter().find(|(_, e)| e == event).map(|(f, _)| *f);
            if won {
                assert_eq!(
                    at(&MapEvent::TextPage {
                        message: 2532,
                        page: 1
                    }),
                    Some(15)
                );
                assert_eq!(at(&MapEvent::TextClosed), Some(45));
                assert_eq!(at(&MapEvent::RoseCard { node: 2 }), Some(55));
                assert_eq!(at(&MapEvent::EventChanged(103)), Some(183));
                assert_eq!(at(&MapEvent::PathRevealing(3)), Some(198));
                assert_eq!(at(&MapEvent::EventChanged(0)), Some(232));
                assert_eq!(at(&MapEvent::PathRevealed(3)), Some(245));
                assert_eq!(at(&MapEvent::PathRevealed(4)), Some(269));
                assert_eq!(at(&MapEvent::NodeRevealed(3)), Some(309));
                assert_eq!(at(&MapEvent::NodeRevealed(5)), Some(333));
                assert_eq!(m.camera_target, [-5.0, 0.0]);
                assert_eq!(m.save.state(2), STATE_BEATEN);
                assert_eq!(m.save.rose_cards(&m.data), 1);
            } else {
                assert_eq!(
                    at(&MapEvent::TextPage {
                        message: 2528,
                        page: 1
                    }),
                    Some(15)
                );
                assert_eq!(at(&MapEvent::TextClosed), Some(51));
                assert_eq!(at(&MapEvent::EventChanged(0)), Some(61));
                assert_eq!(m.save.state(2), STATE_LOST);
            }
        }
    }

    /// The re-entry after a duel at `node` with the given node states.
    fn after(states: &[(u8, u8)], node: u8, won: bool, c9: u8) -> MapModel {
        let data = data();
        let mut save = CampaignSave::after_landing();
        for (n, st) in states {
            save.node_states[usize::from(*n)] = *st;
        }
        save.hovered = node;
        save.destination = node;
        save.location = node;
        save.c9 = c9;
        let variant = data.text_variant(node, save.state(node), c9).unwrap();
        save.opponent = i16::from(variant.opponent.unwrap());
        save.apply_duel_end(&data, won);
        MapModel::resume(data, save).unwrap()
    }

    fn run_log(m: &mut MapModel, limit: u64) -> Vec<(u64, MapEvent)> {
        let mut log = Vec::new();
        let mut last = 0;
        for _ in 0..limit {
            let pad = if m.text_phase() == 4 && m.frame + 1 - last > 3 {
                last = m.frame + 1;
                Pad::CROSS
            } else {
                Pad::NONE
            };
            for e in m.step(pad) {
                log.push((m.frame, e));
            }
            if matches!(m.handler, Handler::Story { .. }) {
                break;
            }
            if matches!(m.handler, Handler::Free)
                && m.frame > 20
                && m.text.is_none()
                && m.paths.iter().all(|p| p.state != 1 && p.state != 2)
                && m.rises.iter().all(|r| !r.rising)
            {
                break;
            }
        }
        log
    }

    fn at(log: &[(u64, MapEvent)], event: &MapEvent) -> Option<u64> {
        log.iter().find(|(_, e)| e == event).map(|(f, _)| *f)
    }

    fn page(message: u16, page: u16) -> MapEvent {
        MapEvent::TextPage { message, page }
    }

    #[test]
    #[ignore = "Requires the private campaign-map import (ROSES_CAMPAIGN_MAP_DATA = map-data.json)"]
    fn private_red_route_returns_match_the_emulator_logs() {
        // logs/lancashire-win.txt: 304, win pages 2632/2636/2640/2641/2644,
        // 104 at f141, paths 7/8 at f156, C2 = 0 at f190.
        let mut m = after(&[(2, 3), (3, 1), (5, 1)], 3, true, 0);
        assert_eq!(m.save.event, 304);
        let log = run_log(&mut m, 700);
        for (message, p, frame) in [
            (2632, 1, 15),
            (2636, 5, 39),
            (2640, 9, 59),
            (2641, 10, 85),
            (2644, 13, 99),
        ] {
            assert_eq!(at(&log, &page(message, p)), Some(frame), "page {message}");
        }
        assert_eq!(at(&log, &MapEvent::TextClosed), Some(131));
        assert_eq!(at(&log, &MapEvent::EventChanged(104)), Some(141));
        assert_eq!(at(&log, &MapEvent::PathRevealing(7)), Some(156));
        assert_eq!(at(&log, &MapEvent::PathRevealing(8)), Some(156));
        assert_eq!(at(&log, &MapEvent::EventChanged(0)), Some(190));
        assert_eq!(
            (m.save.state(3), m.save.state(4), m.save.state(10)),
            (3, 1, 1)
        );
        // logs/lancashire-loss.txt: 204, pages 2628/2629, C2 = 0 at f81, state 2.
        let mut m = after(&[(2, 3), (3, 1), (5, 1)], 3, false, 0);
        let log = run_log(&mut m, 400);
        assert_eq!(at(&log, &page(2628, 1)), Some(15));
        assert_eq!(at(&log, &page(2629, 2)), Some(45));
        assert_eq!(at(&log, &MapEvent::NodeLost(3)), Some(81));
        assert_eq!(at(&log, &MapEvent::EventChanged(0)), Some(81));
        // logs/exeter-win.txt: 310, page 2568, rose, 110 at f189, free at f224.
        let mut m = after(&[(2, 3), (8, 3), (9, 1), (6, 1)], 9, true, 0);
        let log = run_log(&mut m, 500);
        assert_eq!(at(&log, &page(2568, 1)), Some(15));
        assert_eq!(at(&log, &MapEvent::RoseCard { node: 9 }), Some(61));
        assert_eq!(at(&log, &MapEvent::EventChanged(110)), Some(189));
        assert_eq!(at(&log, &MapEvent::Arrived(9)), Some(224));
        assert_eq!(m.save.event, 110);
        // logs/bosworth-trigger.txt: the eighth win (Isle of Man) → 311 at
        // f203, 11120 at f221 (W+5370 = 111).
        let eight: Vec<(u8, u8)> = [2u8, 3, 4, 5, 6, 8, 9].iter().map(|n| (*n, 3)).collect();
        let mut states = eight.clone();
        states.push((10, 1));
        let mut m = after(&states, 10, true, 0);
        let log = run_log(&mut m, 500);
        assert_eq!(at(&log, &page(2604, 1)), Some(15));
        assert_eq!(at(&log, &page(2605, 2)), Some(45));
        assert_eq!(at(&log, &MapEvent::EventChanged(111)), Some(203));
        assert_eq!(at(&log, &MapEvent::EventChanged(11120)), Some(221));
        // 11124 from f416: paths 12/13/11 at f455, path 12 (7 dots) done
        // 43 frames later, Bosworth rising and C2 = 0 the frame after
        // (f498 → f499).
        let mut save = m.save.clone();
        save.event = EVENT_BOSWORTH_REVEAL;
        let mut m = MapModel::resume(data(), save).unwrap();
        let log = run_log(&mut m, 400);
        assert_eq!(at(&log, &MapEvent::PathRevealing(12)), Some(39));
        assert_eq!(m.data.paths[12].dots.len(), 7);
        assert_eq!(at(&log, &MapEvent::PathRevealed(12)), Some(82));
        assert_eq!(at(&log, &MapEvent::NodeRising(7)), Some(82));
        assert_eq!(at(&log, &MapEvent::EventChanged(0)), Some(83));
        assert_eq!(at(&log, &MapEvent::NodeRevealed(7)), Some(146));
        assert_eq!(m.save.state(7), STATE_OPEN);
        // logs/bosworth-win.txt: 308, page 2684, no rose, 11140 at f49; then
        // 108 (f1787): path 14 at +16, C2 = 0 at +50.
        let mut states: Vec<(u8, u8)> = [2u8, 3, 4, 5, 6, 8, 9, 10]
            .iter()
            .map(|n| (*n, 3))
            .collect();
        states.push((7, 1));
        let mut m = after(&states, 7, true, 0);
        let log = run_log(&mut m, 400);
        assert_eq!(at(&log, &page(2684, 1)), Some(15));
        assert!(at(&log, &MapEvent::RoseCard { node: 7 }).is_none());
        assert_eq!(at(&log, &MapEvent::StoryEvent(11140)), Some(49));
        let mut save = m.save.clone();
        save.event = EVENT_STONEHENGE_REVEAL;
        let mut m = MapModel::resume(data(), save).unwrap();
        let log = run_log(&mut m, 400);
        assert_eq!(at(&log, &MapEvent::PathRevealing(14)), Some(16));
        assert_eq!(at(&log, &MapEvent::EventChanged(0)), Some(50));
        assert_eq!(m.data.paths[14].dots.len(), 13);
        assert_eq!(
            at(&log, &MapEvent::NodeRevealed(11)),
            Some(16 + 52 + 15 + 64)
        );
        assert_eq!(m.save.state(11), STATE_OPEN);
        // logs/seto-win.txt: 312, pages 2860/2861, the 8th rose, 11160 at f203.
        let mut states: Vec<(u8, u8)> = [2u8, 3, 4, 5, 6, 7, 8, 9, 10]
            .iter()
            .map(|n| (*n, 3))
            .collect();
        states.push((11, 1));
        let mut m = after(&states, 11, true, 0);
        assert_eq!((m.save.event, m.save.opponent), (312, 1));
        let log = run_log(&mut m, 400);
        assert_eq!(at(&log, &page(2860, 1)), Some(15));
        assert_eq!(at(&log, &page(2861, 2)), Some(45));
        assert_eq!(at(&log, &MapEvent::RoseCard { node: 11 }), Some(75));
        assert_eq!(at(&log, &MapEvent::StoryEvent(11160)), Some(203));
        assert_eq!(m.save.rose_cards(&m.data), 8);
        // 11162 → 112 (f1438): C9 = 1, W+53C4 = 1 on the first map frame,
        // the two pans, free at +35 with the Manawyddan hover.
        let mut save = m.save.clone();
        save.event = EVENT_STONEHENGE_ARRIVE;
        let mut m = MapModel::resume(data(), save).unwrap();
        assert_eq!((m.save.c9, m.w53c4), (1, 1));
        let log = run_log(&mut m, 200);
        assert_eq!(at(&log, &MapEvent::Arrived(11)), Some(35));
        assert_eq!(
            m.data.text_variant(11, 3, 1).unwrap().speaker_message,
            Some(217)
        );
        // logs/seto-loss.txt: 212, pages 2856/2857, C2 = 0 at f75, state 2.
        let mut m = after(&states, 11, false, 0);
        let log = run_log(&mut m, 400);
        assert_eq!(at(&log, &page(2857, 2)), Some(45));
        assert_eq!(at(&log, &MapEvent::EventChanged(0)), Some(75));
        assert_eq!(m.save.state(11), STATE_LOST);
        // logs/manawyddan-win.txt: 313 (C9 = 1), page 2952, no rose, 11180 at f61, state 5.
        states.pop();
        states.push((11, 3));
        let mut m = after(&states, 11, true, 1);
        assert_eq!((m.save.event, m.save.opponent), (313, 20));
        let log = run_log(&mut m, 400);
        assert_eq!(at(&log, &page(2952, 1)), Some(15));
        assert!(at(&log, &MapEvent::RoseCard { node: 11 }).is_none());
        assert_eq!(at(&log, &MapEvent::StoryEvent(11180)), Some(61));
        assert_eq!(m.save.state(11), STATE_BEATEN_SECOND);
        // logs/manawyddan-loss.txt: 213, pages 2944/2948 (two groups), C2 = 0 at f87, state 4.
        let mut m = after(&states, 11, false, 1);
        let log = run_log(&mut m, 400);
        assert_eq!(at(&log, &page(2944, 1)), Some(15));
        assert_eq!(at(&log, &page(2948, 5)), Some(45));
        assert_eq!(at(&log, &MapEvent::EventChanged(0)), Some(87));
        assert_eq!(m.save.state(11), STATE_LOST_SECOND);
    }
}

#[test]
fn system_icon_hover_takes_precedence_and_icon_zero_pushes_the_deck_editor() {
    // Synthetic icon rectangles (the real ones come from the executable).
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    m.system_icons = vec![
        [240, 190, 272, 222],
        [280, 190, 312, 222],
        [240, 230, 272, 262],
        [280, 230, 312, 262],
    ];
    // The cursor starts at (256, 222): inside icon 0's margin already, so
    // the first update reports the hover; two presses down leave it for
    // icon 2 (y 254 within 230..262).
    let events = m.step(Pad::NONE);
    assert!(events.contains(&MapEvent::SystemIcon(Some(0))));
    assert_eq!(m.cursor_mode, 2);
    let mut events = Vec::new();
    for _ in 0..2 {
        events.extend(m.step(Pad {
            down: true,
            ..Pad::NONE
        }));
    }
    assert_eq!(m.cursor, (256, 254));
    assert!(events.contains(&MapEvent::SystemIcon(Some(2))));
    // × on icon 2 opens the OPTION panel; Circle closes it unchanged.
    let events = m.step(Pad::CROSS);
    assert!(events.contains(&MapEvent::Sfx(SFX_SYSTEM_ICON)));
    assert!(m.options.is_some());
    assert!(!m.editor_request);
    for _ in 0..8 {
        m.step(Pad::NONE);
    }
    m.step_with(Pad::NONE, MenuPad { circle: true, ..MenuPad::default() });
    for _ in 0..8 {
        m.step(Pad::NONE);
    }
    assert!(m.options.is_none());
    // Back on icon 0, × pushes the editor with the save's side.
    for _ in 0..2 {
        m.step(Pad::UP);
    }
    let events = m.step(Pad::CROSS);
    assert!(events.contains(&MapEvent::Sfx(SFX_SYSTEM_ICON)));
    assert!(events.contains(&MapEvent::DeckEditor { side: 0 }));
    assert!(m.editor_request);
    assert_eq!(m.travel, 0);
    // Without rectangles the icons are a boundary: nothing is hovered.
    let mut plain = MapModel::at_event_zero(fixture()).unwrap();
    let events = plain.step(Pad::CROSS);
    assert!(!events.iter().any(|e| matches!(e, MapEvent::SystemIcon(_))));
    assert!(!plain.editor_request);
}

#[test]
fn system_icon_rects_reject_short_or_malformed_tables() {
    assert!(system_icon_rects(&[0u8; 16]).is_err());
    let start = (SYSTEM_ICON_TABLE_EE - EE_TO_FILE) as usize;
    let mut image = vec![0u8; start + SYSTEM_ICONS * SYSTEM_ICON_RECORD_BYTES];
    // x0 >= x1 is refused.
    assert!(system_icon_rects(&image).is_err());
    for i in 0..SYSTEM_ICONS {
        let at = start + i * SYSTEM_ICON_RECORD_BYTES;
        let rect: [i16; 4] = [10 + i as i16 * 40, 20, 42 + i as i16 * 40, 52];
        for (k, v) in rect.iter().enumerate() {
            image[at + 2 * k..at + 2 * k + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
    let rects = system_icon_rects(&image).unwrap();
    assert_eq!(rects.len(), 4);
    assert_eq!(rects[1], [50, 20, 82, 52]);
}

#[test]
fn back_from_a_duel_the_map_keeps_its_save() {
    // The save the map left with (Chester beaten, Pegasus hovered at node
    // 3) comes back with the result applied, not a fresh landing save.
    let data = fixture();
    let mut save = CampaignSave::after_landing();
    for (n, s) in [(2u8, 3u8), (3, 1), (5, 1)] {
        save.node_states[usize::from(n)] = s;
    }
    save.hovered = 3;
    save.destination = 3;
    save.location = 3;
    let variant = data.text_variant(3, save.state(3), 0).unwrap();
    save.opponent = i16::from(variant.opponent.unwrap());
    let m = MapModel::after_duel_from(data.clone(), save, true).unwrap();
    let expected = after_duel_at(&data, &[(2, 3), (3, 1), (5, 1)], 3, true, 0);
    assert_eq!(m.save.event, 304);
    assert_eq!(m.save.node_states, expected.node_states);
    assert_eq!(m.save.wins_by_opponent, expected.wins_by_opponent);
    assert_eq!(m.save.node_states[2], 3);
}

#[test]
fn over_dc_shows_while_cross_is_held_and_sounds_once() {
    // 0x239E28: W+5448 = 1 on every frame Cross is held, SFX 6 only on the
    // press frame; the hover sets W+5446 from the opponent's deck cost.
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    m.player_dc = Some(900);
    m.opponent_costs.insert(2, 854);
    hover_node_two(&mut m);
    assert_eq!(m.opponent_dc, Some(854));
    let events = m.step(Pad::CROSS);
    assert!(events.contains(&MapEvent::Sfx(SFX_OVER_DC)));
    assert!(m.over_dc);
    let held = Pad {
        cross_held: true,
        ..Pad::NONE
    };
    let events = m.step(held);
    assert!(m.over_dc);
    assert!(!events.contains(&MapEvent::Sfx(SFX_OVER_DC)));
    m.step(Pad::NONE);
    assert!(!m.over_dc);
    assert_eq!(m.travel, 0);
    assert!(m.text.is_none());
    // Within the DC the same press selects the node.
    m.player_dc = Some(854);
    let events = m.step(Pad::CROSS);
    assert!(events.contains(&MapEvent::Sfx(SFX_SELECT)));
    assert_eq!(m.travel, 1);
}

#[test]
fn a_walk_with_no_path_goes_straight_in_64_frames() {
    // 0x22BB58: Lancashire (3) → Newcastle (4) has no 0x2D2420 entry.
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    // Path 7 joins them, but the walk switch has no entry for the pair.
    assert_eq!(m.data.path_between(3, 4), Some((7, true)));
    assert_eq!(m.data.walk_path(3, 4), None);
    // A switch entry walks its path where the fixture has it (the real
    // map data matches all 15 entries and their reverse flags).
    for (a, b, index) in WALK_PATHS {
        if m.data.path_between(a, b).map(|p| p.0) == Some(index) {
            assert_eq!(m.data.walk_path(a, b), m.data.path_between(a, b), "{a}→{b}");
        }
    }
    assert_eq!(m.data.walk_path(1, 2), Some((1, true)));
    let (a, b) = (m.node_position(3), m.node_position(4));
    m.start_walk(3, 4, None);
    m.walk_update(); // the start frame's own update does not advance again
    assert_eq!(m.figure.state, 1);
    assert_eq!(m.figure.sub, STRAIGHT_WALK_STEP);
    let mut frames = 1;
    while m.figure.state != 3 {
        m.walk_update();
        frames += 1;
        if frames == 32 {
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            assert!((m.figure.position[0] - mid[0]).abs() < 1e-9);
            assert!((m.figure.position[1] - mid[1]).abs() < 1e-9);
        }
        assert!(frames <= 64);
    }
    assert_eq!(frames, 64);
    assert_eq!(m.figure.position, b);
    assert!(m.notes.is_empty());
}

#[test]
fn the_hover_panel_shows_the_place_until_cross_is_held_over_dc() {
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    assert_eq!(m.place_tooltip(), None);
    hover_node_two(&mut m);
    let place = m.data.text_variant(2, m.save.state(2), 0).unwrap().place_message;
    assert!(place.is_some());
    assert_eq!(m.place_tooltip(), place);
    m.player_dc = Some(900);
    m.opponent_dc = Some(854);
    m.step(Pad::CROSS);
    assert!(m.over_dc);
    assert_eq!(m.place_tooltip(), None);
    m.step(Pad::NONE);
    assert_eq!(m.place_tooltip(), place);
}

/// Step from the current frame with immediate Cross; per frame: (frame,
/// C2, bars, story window phase).
fn story_frames(m: &mut MapModel, frames: u64) -> Vec<(u64, i16, [i32; 3], Option<u8>)> {
    let mut out = Vec::new();
    for _ in 0..frames {
        let pad = if m.text_phase() == 4 { Pad::CROSS } else { Pad::NONE };
        m.step(pad);
        out.push((
            m.frame,
            m.save.event,
            m.letterbox.bars,
            m.story_window.as_ref().map(|w| w.phase),
        ));
        if matches!(m.handler, Handler::Story { .. }) {
            break;
        }
    }
    out
}

#[test]
fn story_windows_follow_the_traced_frames() {
    // 11120 from its first frame (resume = the trigger frame f221).
    let mut save = CampaignSave::after_landing();
    for n in [2u8, 3, 4, 5, 6, 8, 9, 10] {
        save.node_states[usize::from(n)] = STATE_BEATEN;
    }
    save.location = 10;
    save.event = EVENT_BOSWORTH_TRIGGER;
    let mut m = MapModel::resume(fixture(), save.clone()).unwrap();
    assert_eq!(m.paths[12].state, 0, "path 12 undrawn before 11124");
    let f = story_frames(&mut m, 80);
    // [emu trace-trigger: f222 8, f230 72, f239 72+72, f240 window phase
    // 3, f253 phase 4, f254 Cross, f259 11, f260 12, f261 mode 3 (+8),
    // f274 72+72+112, f275 C2 = 11122]
    let row = |k: u64| f[(k - 1) as usize];
    assert_eq!(row(1).2, [8, 0, 0]);
    assert_eq!(row(9).2, [72, 0, 0]);
    assert_eq!(row(18).2, [72, 72, 0]);
    assert_eq!(row(18).3, None);
    assert_eq!(row(19).3, Some(3));
    assert_eq!(row(31).3, Some(3));
    assert_eq!(row(32).3, Some(4));
    assert_eq!(row(33).3, Some(5));
    assert_eq!(row(38).3, Some(11));
    assert_eq!(row(39).3, Some(12));
    assert_eq!((row(40).2, row(40).3), ([72, 72, 8], None));
    assert_eq!((row(53).1, row(53).2), (EVENT_BOSWORTH_TRIGGER, [72, 72, 112]));
    assert_eq!(row(54).1, EVENT_BOSWORTH_PAINTINGS);
    assert_eq!(f.len(), 54);
    assert_eq!(m.story_request, Some(EVENT_BOSWORTH_PAINTINGS));
    assert!(!m.hud.shown);
    // 11124 from f416: closed letterbox, camera cut at once, window 2656.
    let mut save = save.clone();
    save.event = EVENT_BOSWORTH_REVEAL;
    let mut m = MapModel::resume(fixture(), save.clone()).unwrap();
    assert_eq!(m.letterbox.height(), 256);
    let before = m.camera_target;
    m.step(Pad::NONE);
    assert_ne!(before, m.camera_target);
    assert_eq!(m.camera_target, m.node_position(BOSWORTH_NODE));
    assert_eq!(m.letterbox.bars, [72, 72, 104]);
    for _ in 1..14 {
        m.step(Pad::NONE);
    }
    assert_eq!((m.letterbox.bars, m.letterbox.active), ([72, 72, 0], 2));
    assert!(m.story_window.is_none());
    m.step(Pad::NONE);
    let w = m.story_window.clone().unwrap();
    assert_eq!((w.message, w.phase, w.reveal_frames), (2656, 3, 7));
    // The fixture's messages are 20 glyphs per line.
    assert_eq!(m.story_window_rect(), Some([130, 256, 252, 34]));
    // 11149 from f1750: the window on the first frame, 2828 drawn, the
    // reveal of 2804 (three lines).
    let mut save = save.clone();
    save.node_states[7] = STATE_BEATEN;
    save.location = 7;
    save.event = EVENT_BOSWORTH_FAREWELL;
    let mut m = MapModel::resume(fixture(), save).unwrap();
    assert_eq!(m.letterbox.height(), 144);
    // Node 7's paths towards hidden ends wait for 108 (path 14 at f1803).
    let node = m.data.node(7).clone();
    for k in 0..usize::from(node.path_count.min(4)) {
        let (p, dir) = (node.red_paths[k] as u8, node.path_dirs[k]);
        if m.save.state(m.data.reveal_end(p, dir)) == STATE_HIDDEN {
            assert_eq!(m.paths[usize::from(p)].state, 0, "path {p}");
        }
    }
    m.step(Pad::NONE);
    let w = m.story_window.clone().unwrap();
    assert_eq!((w.message, w.counter, w.reveal_frames), (2828, 2804, 19));
    let f = story_frames(&mut m, 60);
    // phase 4 at f1770, 11 at f1776, letterbox off f1778..f1786, 108 f1787.
    let row = |k: u64| f.iter().find(|r| r.0 == k).copied().unwrap();
    assert_eq!(row(20).3, Some(4));
    assert_eq!(row(26).3, Some(11));
    assert_eq!((row(28).2, row(28).3), ([64, 64, 0], None));
    assert_eq!((row(36).1, row(36).2), (EVENT_BOSWORTH_FAREWELL, [0, 0, 0]));
    assert_eq!(row(37).1, EVENT_STONEHENGE_REVEAL);
    assert!(m.notes.is_empty(), "{:?}", m.notes);
}

#[test]
fn story_window_rects_match_the_traced_windows() {
    // [emu] 2648 (two lines of 27 glyphs) (88,256) 336x56, 2656 (202,256)
    // 108x34, 2828 (136,256) 240x34; wider than 480 → x = 16.
    assert_eq!(story_window_rect(324, 2), [88, 256, 336, 56]);
    assert_eq!(story_window_rect(96, 1), [202, 256, 108, 34]);
    assert_eq!(story_window_rect(228, 1), [136, 256, 240, 34]);
    assert_eq!(story_window_rect(480, 1), [16, 256, 480, 34]);
    let info = MessageInfo::from_codes(&[0x10, 0x11, 0x2000, 0x12, 0x8013]);
    assert_eq!((info.lines, info.glyphs, info.widest_line(), info.blank), (2, 4, 2, false));
    assert!(MessageInfo::from_codes(&[0x2001]).blank);
}

#[cfg(test)]
mod private_window_tests {
    use super::*;
    use serde_json::Value;

    fn data() -> MapData {
        let path = std::env::var_os("ROSES_CAMPAIGN_MAP_DATA").expect("ROSES_CAMPAIGN_MAP_DATA");
        let mut data = load_map_data(std::path::Path::new(&path)).unwrap();
        // Imports made before the window messages were listed: their sizes
        // as traced (2648 two lines of 27, 2656 one of 8, 2804 three, 2828
        // one of 19).
        for (id, lines, width) in [(2648u16, 2u8, 27usize), (2656, 1, 8), (2804, 3, 20), (2828, 1, 19)] {
            data.messages.entry(id).or_insert(MessageInfo {
                blank: false,
                glyphs: (width * usize::from(lines)) as u16,
                lines,
                text: vec!["?".repeat(width); usize::from(lines)].join("\n"),
            });
        }
        data
    }

    fn oracle() -> Value {
        let dir = std::env::var_os("ROSES_MAP_WINDOWS_ORACLE").expect("ROSES_MAP_WINDOWS_ORACLE");
        let path = std::path::Path::new(&dir).join("oracle-windows.json");
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn i(v: &Value) -> i64 {
        v.as_i64().unwrap()
    }

    /// Model frame vs one oracle row: C2, letterbox (mode, active, bars),
    /// window rect, page phase, advance mark, camera target, HUD slide.
    fn compare(m: &MapModel, row: &Value, errors: &mut Vec<String>) {
        let f = i(&row["frame"]);
        let mut check = |what: &str, ok: bool, detail: String| {
            if !ok {
                errors.push(format!("f{f} {what}: {detail}"));
            }
        };
        check(
            "C2",
            i64::from(m.save.event) == i(&row["C2"]),
            format!("{} vs {}", m.save.event, row["C2"]),
        );
        let lb: Vec<i64> = row["lb"].as_array().unwrap().iter().map(i).collect();
        let mine = vec![
            i64::from(m.letterbox.mode),
            i64::from(m.letterbox.active),
            i64::from(m.letterbox.bars[0]),
            i64::from(m.letterbox.bars[1]),
            i64::from(m.letterbox.bars[2]),
        ];
        check("letterbox", lb == mine, format!("{mine:?} vs {lb:?}"));
        let window = m.story_window_rect().map(|r| r.map(i64::from).to_vec());
        let want = row["window"]
            .as_array()
            .map(|a| a.iter().map(i).collect::<Vec<_>>());
        check("window", window == want, format!("{window:?} vs {want:?}"));
        if let Some(w) = &m.story_window {
            check(
                "phase",
                i64::from(w.phase) == i(&row["disp"]),
                format!("{} vs {}", w.phase, row["disp"]),
            );
        }
        let mark = m.story_window.as_ref().is_some_and(|w| w.phase == 4);
        check("mark", Some(mark) == row["mark"].as_bool(), format!("{mark}"));
        if let Some(t) = row["tgt"].as_array() {
            let want = [t[0].as_f64().unwrap(), t[1].as_f64().unwrap()];
            check(
                "camera",
                m.camera_target == want,
                format!("{:?} vs {want:?}", m.camera_target),
            );
        }
        if let Some(dy) = row["hud_dy"].as_i64() {
            check(
                "hud",
                i64::from(320 - m.hud.slide) == dy,
                format!("{} vs {dy}", 320 - m.hud.slide),
            );
        }
    }

    fn rows<'a>(o: &'a Value, key: &str) -> Vec<&'a Value> {
        o[key].as_array().unwrap().iter().collect()
    }

    #[test]
    #[ignore = "Requires ROSES_CAMPAIGN_MAP_DATA and ROSES_MAP_WINDOWS_ORACLE (story/claude-map-windows)"]
    fn private_story_windows_match_the_emulated_frames() {
        let o = oracle();
        let trigger = rows(&o, "trigger");
        let at = |f: i64| {
            trigger
                .iter()
                .find(|r| i(&r["frame"]) == f)
                .copied()
                .unwrap()
        };
        // The eighth win at Isle of Man, as run_red.py patched it.
        let data = data();
        let mut save = CampaignSave::after_landing();
        for n in [2u8, 3, 4, 5, 6, 8, 9] {
            save.node_states[usize::from(n)] = STATE_BEATEN;
        }
        save.node_states[10] = STATE_OPEN;
        save.hovered = 10;
        save.destination = 10;
        save.location = 10;
        let variant = data.text_variant(10, 1, 0).unwrap();
        save.opponent = i16::from(variant.opponent.unwrap());
        save.apply_duel_end(&data, true);
        let mut m = MapModel::resume(data, save).unwrap();
        let mut errors = Vec::new();
        while m.frame < 500 {
            let row = trigger.iter().find(|r| i(&r["frame"]) == m.frame as i64 + 1);
            let pad = match row {
                Some(r) if i(&r["pad"]) != 0 => Pad::CROSS,
                Some(_) => Pad::NONE,
                None if m.text_phase() == 4 => Pad::CROSS,
                None => Pad::NONE,
            };
            m.step(pad);
            if (221..=275).contains(&m.frame) {
                compare(&m, at(m.frame as i64), &mut errors);
            }
            if matches!(m.handler, Handler::Story { .. }) {
                break;
            }
        }
        assert_eq!(m.frame, 275);
        assert_eq!(m.story_request, Some(EVENT_BOSWORTH_PAINTINGS));
        // 11124: the map re-enters at f416 (C2 = 11124, letterbox closed).
        let mut save = m.save.clone();
        save.event = EVENT_BOSWORTH_REVEAL;
        let mut m = MapModel::resume(super::private_window_tests::data(), save).unwrap();
        compare(&m, at(416), &mut errors);
        for f in 417..=499 {
            let pad = if i(&at(f)["pad"]) != 0 { Pad::CROSS } else { Pad::NONE };
            m.step(pad);
            compare(&m, at(f), &mut errors);
        }
        // 11149: from f1750 (C2 = 11149, 144-row letterbox).
        let farewell = rows(&o, "farewell");
        let at = |f: i64| {
            farewell
                .iter()
                .find(|r| i(&r["frame"]) == f)
                .copied()
                .unwrap()
        };
        let mut save = m.save.clone();
        save.node_states[7] = STATE_BEATEN;
        save.location = 7;
        save.event = EVENT_BOSWORTH_FAREWELL;
        let mut m = MapModel::resume(super::private_window_tests::data(), save).unwrap();
        compare(&m, at(1750), &mut errors);
        for f in 1751..=1787 {
            let pad = if i(&at(f)["pad"]) != 0 { Pad::CROSS } else { Pad::NONE };
            m.step(pad);
            compare(&m, at(f), &mut errors);
        }
        assert!(
            errors.is_empty(),
            "{} mismatches:\n{}",
            errors.len(),
            errors.join("\n")
        );
    }
}

#[test]
fn the_map_runs_under_the_bosworth_story_and_plays_11149_after_it() {
    // Richard beaten: C2 = 11140 in the return's close (f49). The map keeps
    // drawing under the story: its HUD slides out 20 per update from C2+1
    // (system box dy 20 at f50 .. 240 at f61, 256 at f62; cursor gone from
    // f50) [emu trace-win3].
    let data = fixture();
    let mut states: Vec<(u8, u8)> = [2u8, 3, 4, 5, 6, 8, 9, 10].iter().map(|n| (*n, 3)).collect();
    states.push((7, 1));
    let save = after_duel_at(&data, &states, 7, true, 0);
    let mut m = MapModel::resume(data, save).unwrap();
    run_until_still(&mut m, 600);
    assert!(matches!(m.handler, Handler::Story { event: EVENT_BOSWORTH_WIN }));
    assert_eq!((m.hud.slide, m.hud.hidden), (HUD_SLIDE_FULL, false), "f49 as before the event");
    assert!(!m.hud.shown, "the draw pass decided W+6E = 0 on the C2 update");
    let mut dy = Vec::new();
    for _ in 0..14 {
        m.step(Pad::NONE);
        dy.push((HUD_SLIDE_FULL - m.hud.slide, m.hud.hidden));
    }
    assert_eq!(dy[0], (20, true));
    assert_eq!(dy[11], (240, true));
    assert_eq!(dy[12], (256, true));
    assert_eq!(dy[13], (256, true));
    // The frontend's story chain moves C2 along (11142 .. 11148) and ends
    // at 11149: the map plays its window from the frame C2 became 11149,
    // as a resume at 11149 does (108 37 updates later).
    for e in [11142, 11143, 11144, 11146, 11147, 11148] {
        m.story_progress(e);
        m.step(Pad::NONE);
        assert_eq!(m.save.event, e);
    }
    assert!(m.end_story(108).is_err(), "only window events continue in place");
    m.end_story(EVENT_BOSWORTH_FAREWELL).unwrap();
    assert_eq!(m.save.event, EVENT_BOSWORTH_FAREWELL);
    assert_eq!((m.letterbox.height(), m.letterbox.active), (144, 2));
    let log = run_until_still(&mut m, 800);
    let base = log.first().map(|(f, _)| *f).unwrap() - 1;
    assert_eq!(at(&log, &MapEvent::EventChanged(108)).map(|f| f - base), Some(37));
    assert!(m.end_story(EVENT_BOSWORTH_FAREWELL).is_err(), "not in a story any more");
}

#[test]
fn stonehenge_tails_open_the_letterbox_over_the_map() {
    // 11162's tail: the story scene ends on the update after its letterbox
    // closed (f1423); the map then opens it (mode 0: 64+64+104 on f1424
    // .. 0 on f1437, done) with C2 still 11162, and C2 = 112 on f1438
    // with the HUD out [emu trace-seto-cam f1423..f1438; white 12132 ->
    // 0 the same, trace-brest f1680..f1695].
    let data = fixture();
    let mut save = CampaignSave::after_landing();
    for n in [2u8, 3, 4, 5, 6, 7, 8, 9, 10, 11] {
        save.node_states[usize::from(n)] = 3;
    }
    save.location = 11;
    save.event = EVENT_STONEHENGE_ARRIVE;
    let resumed = MapModel::resume(data.clone(), save.clone()).unwrap();
    let mut m = MapModel::after_story(data.clone(), save.clone(), 11162).unwrap();
    assert_eq!((m.save.event, m.letterbox.height()), (11162, 256));
    let mut log = Vec::new();
    for f in 1424..=1438u32 {
        let events = m.step(Pad::NONE);
        log.push((f, m.save.event, m.letterbox.bars, m.letterbox.active, events));
        assert_eq!(m.hud.slide, 0, "f{f}: the HUD stays out");
    }
    assert_eq!((log[0].1, log[0].2, log[0].3), (11162, [64, 64, 104], 1));
    assert_eq!((log[13].1, log[13].2, log[13].3), (11162, [0, 0, 0], 2));
    assert!(log[13].4.is_empty());
    assert_eq!(log[14].1, EVENT_STONEHENGE_ARRIVE);
    assert!(log[14].4.contains(&MapEvent::EventChanged(EVENT_STONEHENGE_ARRIVE)));
    assert!(matches!(m.handler, Handler::Arrive { node: 11, .. }));
    assert_eq!(m.save.c9, resumed.save.c9);
    // Other story events resume as before.
    let plain = MapModel::after_story(data, save, 11160).unwrap();
    assert_eq!(plain.handler, resumed.handler);
}

#[test]
fn scene0_story_entry_closes_the_letterbox_over_the_map() {
    // 11160 (and 11180, 12150): C2 = the event at once, mode 3 from C2+1
    // waiting 12 updates for the return panel, bars from C2+13, closed on
    // C2+44, the story scene requested then [emu trace-seto-cam f203..f247].
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    let mut events = Vec::new();
    m.start_story(EVENT_SETO_WIN, &mut events);
    assert!(events.contains(&MapEvent::StoryEvent(EVENT_SETO_WIN)));
    assert_eq!(m.save.event, EVENT_SETO_WIN);
    let mut heights = Vec::new();
    let mut request = None;
    for t in 1..=50u32 {
        m.step(Pad::NONE);
        heights.push(m.letterbox.height());
        if let Some(e) = m.take_story_request() {
            request.get_or_insert((t, e));
        }
    }
    assert_eq!((heights[0], heights[11], heights[12], heights[43]), (0, 0, 8, 256));
    assert_eq!((m.letterbox.mode, m.letterbox.active), (3, 2));
    assert_eq!(request, Some((44, EVENT_SETO_WIN)));
    // 11140 is not a scene-0 entry: requested at once.
    let mut m = MapModel::at_event_zero(fixture()).unwrap();
    m.start_story(EVENT_BOSWORTH_WIN, &mut Vec::new());
    assert_eq!(m.take_story_request(), Some(EVENT_BOSWORTH_WIN));
}
