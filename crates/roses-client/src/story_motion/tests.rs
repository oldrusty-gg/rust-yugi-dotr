//! Frame timing against the traces (`story/the project notes
//! .json`, `trace-win.json`, `trace-manawyddan.json`, the project notes
//! trace-manawyddan.json`): frame numbers are the traces', the event's C2
//! frame is its first update; Cross is pressed on the traced press frames.
use super::*;

#[derive(Clone, Debug)]
struct Snap {
    lb: i32,
    slide: [i32; 2],
    alpha: i32,
    pan: i32,
    panels: [Option<i32>; 2],
    waiting: bool,
    forward: bool,
    text: bool,
    paintings: bool,
    page: usize,
}

struct Run {
    c2: u32,
    snaps: Vec<Snap>,
    motion: StoryMotion,
}
impl Run {
    fn at(&self, frame: u32) -> &Snap {
        &self.snaps[(frame - self.c2) as usize]
    }
    /// Frames on which `f` holds.
    fn frames(&self, f: impl Fn(&Snap) -> bool) -> Vec<u32> {
        (0..self.snaps.len() as u32)
            .filter(|&i| f(&self.snaps[i as usize]))
            .map(|i| i + self.c2)
            .collect()
    }
}

fn run(event: u16, pages: &[PageInfo], carry: Option<Carry>, c2: u32, presses: &[u32], last: u32) -> Run {
    let spec = event_motion(event).unwrap();
    let mut m = StoryMotion::from_pages(&format!("map-story-{event}"), spec, pages.to_vec(), carry).unwrap();
    let mut snaps = Vec::new();
    for frame in c2..=last {
        let r = m.step();
        snaps.push(Snap {
            lb: m.letterbox.height(),
            slide: m.slide,
            alpha: m.alpha,
            pan: m.drawn_pan,
            panels: m.panels,
            waiting: m.waiting(),
            forward: r.forward.is_some(),
            text: m.text_state().is_some(),
            paintings: m.painting_visible(),
            page: m.page,
        });
        if presses.contains(&frame) {
            assert!(m.waiting(), "event {event}: no wait on press frame {frame}");
            m.press(Button::Cross);
        }
    }
    Run { c2, snaps, motion: m }
}

const fn still() -> PageInfo {
    PageInfo { role: Role::Still, lines: 1, message: 0 }
}
const fn cap(lines: u32) -> PageInfo {
    PageInfo { role: Role::Caption, lines, message: 0 }
}
const fn bot(lines: u32) -> PageInfo {
    PageInfo {
        role: Role::Dialogue { target: 100, latch: Some(0) },
        lines,
        message: 0,
    }
}
const fn top(lines: u32) -> PageInfo {
    PageInfo {
        role: Role::Dialogue { target: -100, latch: Some(1) },
        lines,
        message: 0,
    }
}
const fn voice(lines: u32) -> PageInfo {
    PageInfo {
        role: Role::Dialogue { target: 0, latch: None },
        lines,
        message: 0,
    }
}
fn closed() -> Option<Carry> {
    Some(Carry { bars: LETTERBOX_CLOSED, ..Carry::default() })
}

#[test]
fn bosworth_slide_11122_matches_trace_trigger() {
    // C2 = 11122 at f275 (the map closed the letterbox, 256 rows).
    let r = run(11122, &[still(), cap(1)], closed(), 275, &[379, 395], 420);
    assert_eq!(r.at(275).lb, 256);
    assert!(!r.at(275).paintings, "the C2 update draws no painting");
    assert_eq!((r.at(276).lb, r.at(276).slide), (240, [0, -448]));
    assert!(r.at(276).paintings);
    assert_eq!(r.at(289).lb, 72);
    assert_eq!(r.at(289).slide, [0, -448]);
    assert_eq!(r.at(290).slide, [0, -444]);
    assert_eq!(r.at(378).slide, [0, -92]);
    assert_eq!(r.at(379).slide, [0, -91]);
    assert_eq!(r.frames(|s| s.waiting), vec![379, 395]);
    // Cross read at f380 (page 2652), letterbox mode 2 from f381.
    assert_eq!((r.at(380).page, r.at(380).forward), (1, true));
    assert_eq!((r.at(381).lb, r.at(389).lb), (80, 144));
    assert_eq!(r.frames(|s| s.text), (390..=401).collect::<Vec<_>>());
    assert_eq!((r.at(401).lb, r.at(402).lb, r.at(415).lb), (144, 152, 256));
    // The frame after the close is C2 = 11124's.
    assert_eq!(r.frames(|s| s.forward), vec![380, 415]);
}

/// 11140's pages: (role, lines) from the import and the traced waits.
fn pages_11140() -> Vec<PageInfo> {
    vec![
        bot(3), bot(2), bot(3), bot(1), voice(2), bot(1), top(1), bot(3), top(3), top(1),
        top(3), top(2), bot(1), top(3), top(1), bot(1), top(3), top(3), top(2), top(3),
        top(1), top(2), bot(2), top(3), top(3),
    ]
}
const PRESSES_11140: [u32; 25] = [
    123, 143, 169, 183, 212, 235, 284, 329, 375, 389, 415, 435, 468, 514, 528, 561, 607, 633,
    653, 679, 693, 713, 752, 798, 824,
];

#[test]
fn bosworth_dialogue_11140_matches_trace_win() {
    // C2 = update 1 (f49); the letterbox waits 12 updates while the map's
    // return panel slides out (f50..f61), opens f62..f79, U0 f80; pressing
    // on our waits, every page's wait lies at the traced frame.
    let pages = pages_11140();
    let mut m = StoryMotion::from_pages("map-story-11140", event_motion(11140).unwrap(), pages, None).unwrap();
    let mut waits = Vec::new();
    let mut u0 = None;
    let mut frames = Vec::new();
    let mut forward_at = None;
    let mut talking = Vec::new();
    for update in 1..2000u32 {
        let r = m.step();
        talking.push(m.talking());
        if u0.is_none() && m.panels[0] == Some(PANEL_STEP) {
            u0 = Some(update);
        }
        frames.push((update, m.drawn_pan, m.panels, m.letterbox.height()));
        if m.waiting() {
            waits.push(update);
            m.press(Button::Cross);
        }
        if r.forward.is_some() && m.finished() {
            forward_at = Some(update);
            break;
        }
    }
    let u0 = u0.unwrap();
    assert_eq!(u0, 80 - 49 + 1);
    // Kind 3 (talking) on the bottom face f105..f123 (2688: counts
    // f104..f122, wait f123), on the top face f278..f284 (2708) [emu].
    let talk = |frame: u32| talking[(frame - 80 + u0 - 1) as usize];
    assert_eq!((talk(104), talk(105), talk(123), talk(124)), ([false; 2], [true, false], [true, false], [false; 2]));
    assert_eq!((talk(277), talk(278), talk(284), talk(285)), ([false; 2], [false, true], [false, true], [false; 2]));
    let traced: Vec<u32> = PRESSES_11140.iter().map(|p| p - 80).collect();
    let ours: Vec<u32> = waits.iter().map(|w| w - u0).collect();
    assert_eq!(ours, traced);
    let at = |frame: u32| frames[(frame - 80 + u0 - 1) as usize];
    // Bottom panel slide-in f80..f95, pan 0 -> 100 from f96 (figures one
    // update behind), top panel slide-in f242..f257.
    assert_eq!(at(80).2, [Some(64), None]);
    assert_eq!(at(95).2, [Some(1024), None]);
    assert_eq!((at(96).1, at(97).1, at(106).1), (0, 10, 100));
    assert_eq!(at(242).2, [Some(1024), Some(64)]);
    assert_eq!(at(257).2, [Some(1024), Some(1024)]);
    assert_eq!((at(258).1, at(259).1), (100, 90));
    // Close: pan -100 -> 0 f832..f841, panels out f841..f856, letterbox
    // mode 3 f857..f870, C2 = 11142 at f872 (forward on f871).
    assert_eq!((at(832).1, at(833).1), (-100, -90));
    assert_eq!(at(840).2, [Some(1024), Some(1024)]);
    assert_eq!(at(841).2, [Some(960), Some(960)]);
    assert_eq!(at(855).2, [Some(64), Some(64)]);
    assert_eq!(at(856).2, [Some(0), Some(0)]);
    assert_eq!((at(856).3, at(857).3, at(870).3), (144, 152, 256));
    assert_eq!(forward_at.unwrap() - u0 + 80, 871);
    // (The frames vector lags the drawn pan by one entry: index = update.)
}

#[test]
fn bosworth_11142_11143_11144_match_trace_win() {
    let r = run(11142, &[still(), cap(3), cap(1)], closed(), 872, &[1015, 1031, 1041], 1061);
    assert!(!r.at(872).paintings);
    assert_eq!((r.at(873).lb, r.at(886).lb), (240, 72));
    assert_eq!((r.at(886).slide, r.at(887).slide), ([0, 0], [0, -4]));
    assert_eq!((r.at(1014).slide, r.at(1015).slide), ([0, -512], [0, -512]));
    assert_eq!(r.frames(|s| s.waiting), vec![1015, 1031, 1041]);
    assert_eq!((r.at(1017).lb, r.at(1025).lb), (80, 144));
    assert_eq!(r.at(1026).text, true);
    assert_eq!((r.at(1035).page, r.at(1036).page), (1, 2));
    assert_eq!((r.at(1047).lb, r.at(1048).lb, r.at(1061).lb), (144, 152, 256));
    assert_eq!(r.frames(|s| s.forward), vec![1016, 1036, 1061]);

    let r = run(11143, &[cap(1)], Some(r.motion.carry()), 1062, &[1068], 1074);
    assert_eq!(r.frames(|s| s.text), (1063..=1074).collect::<Vec<_>>());
    assert_eq!(r.frames(|s| s.forward), vec![1074]);

    let r = run(
        11144,
        &[still(), cap(1), cap(1), cap(1), still()],
        Some(r.motion.carry()),
        1075,
        &[1090, 1106, 1116, 1126, 1206],
        1230,
    );
    assert_eq!((r.at(1076).lb, r.at(1089).lb), (240, 72));
    assert_eq!(r.frames(|s| s.waiting), vec![1090, 1106, 1116, 1126, 1206]);
    assert_eq!((r.at(1092).lb, r.at(1100).lb), (80, 144));
    assert_eq!(r.at(1101).text, true);
    assert_eq!((r.at(1132).lb, r.at(1133).lb, r.at(1141).lb), (144, 136, 72));
    // W+54A6: TEXEVE 6 drawn at 128 on f1142, 2 less each update, 0 on f1206.
    assert_eq!((r.at(1142).alpha, r.at(1143).alpha, r.at(1205).alpha, r.at(1206).alpha), (128, 126, 2, 0));
    assert_eq!((r.at(1207).lb, r.at(1208).lb, r.at(1230).lb), (72, 80, 256));
    assert_eq!(r.frames(|s| s.forward), vec![1091, 1111, 1121, 1133, 1230]);
}

#[test]
fn bosworth_11148_matches_trace_win() {
    let pages = [
        bot(3), bot(1), bot(3), bot(2), top(3), bot(3), bot(1), top(2), bot(1), bot(3), bot(1), bot(3), bot(1),
    ];
    let presses = [1384, 1398, 1424, 1444, 1505, 1550, 1564, 1604, 1637, 1663, 1677, 1703, 1717];
    let r = run(11148, &pages, closed(), 1326, &presses, 1749);
    assert_eq!((r.at(1327).lb, r.at(1340).lb), (248, 144));
    assert_eq!(r.at(1341).panels, [Some(64), None]);
    assert_eq!(r.at(1356).panels, [Some(1024), None]);
    assert_eq!((r.at(1357).pan, r.at(1358).pan), (0, 10));
    for p in presses {
        assert!(r.at(p).waiting && !r.at(p - 1).waiting, "wait at f{p}");
    }
    // Close: pan 100 -> 0 f1725..f1734, panels out f1734..f1749, then
    // C2 = 11149 at f1750 with the letterbox kept.
    assert_eq!((r.at(1725).pan, r.at(1726).pan, r.at(1735).pan), (100, 90, 0));
    assert_eq!(r.at(1734).panels, [Some(960), Some(960)]);
    assert_eq!(r.at(1749).panels, [Some(0), Some(0)]);
    assert_eq!(r.at(1749).lb, 144);
    assert_eq!(r.frames(|s| s.forward).last(), Some(&1749));
}

#[test]
fn stonehenge_mosaic_11182_matches_trace_manawyddan() {
    let pages = [still(), cap(1), cap(1), cap(1), cap(1), cap(1), still()];
    let r = run(11182, &pages, closed(), 501, &[594, 610, 620, 630, 640, 650, 744], 768);
    assert_eq!((r.at(502).lb, r.at(515).lb), (240, 72));
    for (f, s) in [(515, [0, -438]), (516, [-2, -436]), (517, [-4, -433]), (521, [-12, -422]), (593, [-156, -219])] {
        assert_eq!(r.at(f).slide, s, "f{f}");
    }
    assert_eq!(r.frames(|s| s.waiting && s.paintings && !s.text).first(), Some(&594));
    assert_eq!(r.at(605).text, true);
    assert_eq!((r.at(657).lb, r.at(665).lb), (136, 72));
    for (f, s) in [(665, [-156, -219]), (666, [-158, -217]), (667, [-160, -214]), (743, [-312, 0])] {
        assert_eq!(r.at(f).slide, s, "f{f}");
    }
    assert!(r.at(744).waiting && !r.at(743).waiting);
    assert_eq!((r.at(746).lb, r.at(768).lb), (80, 256));
    assert_eq!(r.frames(|s| s.forward).last(), Some(&768));
}

#[test]
fn white_mosaic_12154_matches_trace_manawyddan() {
    let pages = [still(), bot(1), bot(2), bot(3), bot(3), bot(3), bot(3), bot(1)];
    let presses = [1041, 1083, 1103, 1129, 1155, 1181, 1207, 1221];
    let r = run(12154, &pages, closed(), 870, &presses, 1317);
    assert_eq!((r.at(871).lb, r.at(884).lb), (240, 72));
    for (f, s) in [(884, [-312, 0]), (885, [-310, -4]), (886, [-308, -7]), (1036, [-8, -458]), (1040, [0, -470])] {
        assert_eq!(r.at(f).slide, s, "f{f}");
    }
    for p in presses {
        assert!(r.at(p).waiting && !r.at(p - 1).waiting, "wait at f{p}");
    }
    assert_eq!((r.at(1043).lb, r.at(1051).lb), (80, 144));
    assert_eq!(r.at(1052).panels, [Some(64), None]);
    // Tail: pan 100 -> 0 from f1229, panel out f1238..f1253, the mosaic
    // fades f1254 (128) .. f1317 (2), C2 = 12156 at f1318.
    assert_eq!((r.at(1229).pan, r.at(1230).pan), (100, 90));
    assert_eq!((r.at(1238).panels, r.at(1253).panels), ([Some(960), None], [Some(0), None]));
    assert_eq!((r.at(1254).alpha, r.at(1255).alpha, r.at(1317).alpha), (128, 126, 2));
    assert_eq!(r.frames(|s| s.forward).last(), Some(&1317));
}

#[test]
fn figures_follow_the_pan_of_the_previous_update() {
    // Richard (rest 88) and Pegasus (152) [emu trace-win f96..f106, f841].
    assert_eq!(figure_x(88, 0, 0), -336);
    assert_eq!(figure_x(88, 0, 10), -294);
    assert_eq!(figure_x(88, 0, 30), -209);
    assert_eq!(figure_x(88, 0, 90), 45);
    assert_eq!(figure_x(88, 0, 100), 88);
    assert_eq!(figure_x(152, 1, 10), 548);
    assert_eq!(figure_x(152, 1, -10), 476);
    assert_eq!(figure_x(152, 1, -100), 152);
    // Yugi (152, bottom) f1726..f1735.
    assert_eq!(figure_x(152, 0, 90), 116);
    assert_eq!(figure_x(152, 0, 0), -208);
    // Seto at pan 0 (white trace-brest f665: -464).
    assert_eq!(figure_x(24, 0, 0), -464);
}

#[test]
fn layers_move_and_clip() {
    let spec = event_motion(11122).unwrap();
    let mut m = StoryMotion::from_pages("map-story-11122", spec, vec![still(), cap(1)], closed()).unwrap();
    for _ in 0..20 {
        m.step();
    }
    let set = vec![
        black([0, 0, 640, 448]),
        black([0, 0, 640, 63]),
        black([0, 385, 640, 63]),
    ];
    let dims = |_: &str| Some([640, 384]);
    let layers = m.layers(&set, &[], &dims);
    // Black base, TEXEVE 3 low and TEXEVE 2 high (slots 3/5 lie at x 512), bands.
    let paintings: Vec<_> = layers.iter().filter(|l| l.asset.starts_with("texeve")).collect();
    assert_eq!(paintings.len(), 2);
    assert_eq!((paintings[0].asset.as_str(), paintings[1].asset.as_str()), ("texeve-03", "texeve-02"));
    for l in &layers {
        let [x, y, w, h] = l.destination;
        assert!(w > 0 && h > 0 && x + w <= 640 && y + h <= 448);
    }
    let bands: Vec<_> = layers.iter().filter(|l| l.asset == "story-black").collect();
    assert_eq!(bands.len(), 3);
    assert_eq!(bands[1].destination, [0, 0, 640, 63]);
}

/// Eye offsets (W+5490, W+5494) of trace-seto-cam: 11160 f661.. (state 1
/// from amplitude 0, counter 0) and the ramp-down f1158.. of 11162.
#[test]
fn shake_offsets_match_trace_seto_cam() {
    let up = [
        (0.0, 0.0), (-0.2, -0.2), (0.3, 0.3), (0.0, -0.2), (-0.5, 0.25), (0.6, 0.0), (0.0, -0.7),
        (-0.8, 0.8), (0.9, -0.45), (0.0, 0.5), (-1.1, 0.0), (1.2, -1.2), (0.0, 1.3), (-1.4, -0.7),
        (1.5, 0.75), (0.0, 0.0), (-1.7, -1.7), (1.8, 1.8), (0.0, -0.95), (-2.0, 1.0), (2.0, 0.0),
        (0.0, -2.0), (-2.0, 2.0), (2.0, -1.0), (0.0, 1.0),
    ];
    let mut s = Shake { state: 1, ..Shake::default() };
    for (i, want) in up.iter().enumerate() {
        s.step(0);
        let got = (s.offset[0], s.offset[1]);
        assert!((got.0 - want.0).abs() < 1e-4 && (got.1 - want.1).abs() < 1e-4, "f{}: {got:?} vs {want:?}", 661 + i);
    }
    // f680: amplitude 100, state 2.
    assert_eq!((s.state, s.amplitude), (2, 100));
    // 11162: the clock reaches 5 on f1160; the camera sees it on f1161
    // (state 3), ramps down from f1162 and stops (state 4) on f1181.
    let mut s = Shake { state: 2, amplitude: 100, counter: 0, offset: [0.0; 2] };
    // W+5482 is a byte running since f661 (0): 497 updates later = 241
    // (it wraps at f1173: the offsets of f1172 and f1173 are both 0).
    s.counter = ((1158 - 661) % 256) as u8;
    let down = [
        (-2.0, -2.0), (2.0, 2.0), (0.0, -1.0), (-2.0, 1.0), (1.9, 0.0), (0.0, -1.8), (-1.7, 1.7),
        (1.6, -0.8), (0.0, 0.75), (-1.4, 0.0), (1.3, -1.3), (0.0, 1.2), (-1.1, -0.55), (1.0, 0.5),
        (0.0, 0.0), (0.0, 0.0), (-0.7, -0.7), (0.6, 0.6), (0.0, -0.25), (-0.4, 0.2), (0.3, 0.0),
        (0.0, -0.2), (-0.1, 0.1), (0.0, 0.0), (0.0, 0.0),
    ];
    for (i, want) in down.iter().enumerate() {
        let frame = 1158 + i as u32;
        s.step(if frame > 1160 { 5 } else { 4 });
        let got = (s.offset[0], s.offset[1]);
        assert!((got.0 - want.0).abs() < 1e-4 && (got.1 - want.1).abs() < 1e-4, "f{frame}: {got:?} vs {want:?}");
        if frame == 1161 {
            assert_eq!(s.state, 3);
        }
        if frame == 1180 {
            assert_eq!(s.state, 3);
        }
        if frame == 1181 {
            assert_eq!(s.state, 4);
        }
    }
}

fn msg(mut p: PageInfo, message: u16) -> PageInfo {
    p.message = message;
    p
}

#[test]
fn seto_shake_starts_after_page_2912_opens() {
    // A page 2912 opened as the first page: the handler sets W+5483 = 1 on
    // U0+1 (amplitude from 0) and the camera moves the same update [emu
    // f660/f661]; the figure quad follows, floored (f662 (23, 131)).
    let r = run(11160, &[msg(bot(1), 2912)], closed(), 1, &[], 60);
    let u0 = r.frames(|s| s.panels[0] == Some(PANEL_STEP))[0];
    let mut m = StoryMotion::from_pages("map-story-11160", event_motion(11160).unwrap(), vec![msg(bot(1), 2912)], closed()).unwrap();
    let mut states = Vec::new();
    for u in 1..=u0 + 25 {
        m.step();
        states.push((u, m.shake.state, m.shake.amplitude));
    }
    assert_eq!(states[(u0 - 1) as usize].1, 0);
    assert_eq!(states[u0 as usize], (u0 + 1, 1, 5));
    assert_eq!(states[(u0 + 1) as usize].2, 10);
    // At amplitude 100 the shake holds (state 2).
    assert_eq!(states[(u0 + 20) as usize].1, 2);
}

#[test]
fn effect_wait_11162_matches_trace_seto_cam() {
    // C2 = 11162 at f773 with 11160's closed letterbox and its shake
    // (state 2); letterbox done f787, W+64 3840 on f788 .. 0 on f803; the
    // pan -4 from f937 to -100 on f961 (clock 1), 5 on f1160, shake state 3
    // f1161, 4 f1181; the first page's U0 on f1182.
    let carry = Carry {
        bars: LETTERBOX_CLOSED,
        shake: Shake { state: 2, amplitude: 100, counter: 0, offset: [0.0; 2] },
        ..Carry::default()
    };
    let pages = [msg(top(1), 2920), msg(bot(1), 2924)];
    let spec = event_motion(11162).unwrap();
    let mut m = StoryMotion::from_pages("map-story-11162", spec, pages.to_vec(), Some(carry)).unwrap();
    let mut at = std::collections::BTreeMap::new();
    for frame in 773..=1200u32 {
        m.step();
        at.insert(frame, (m.letterbox.height(), m.level, m.pan, m.shake.state, m.effect_stage(m.update), m.panels));
    }
    assert_eq!(at[&787].0, 144);
    assert_eq!(at[&786].0 > 144, true);
    assert_eq!((at[&787].1, at[&788].1, at[&802].1, at[&803].1), (4096, 3840, 256, 0));
    assert_eq!((at[&936].2, at[&937].2, at[&960].2, at[&961].2), (0, -4, -96, -100));
    assert_eq!((at[&960].4, at[&961].4, at[&1009].4, at[&1010].4, at[&1159].4, at[&1160].4), (0, 1, 1, 2, 4, 5));
    assert_eq!((at[&1160].3, at[&1161].3, at[&1180].3, at[&1181].3), (2, 3, 3, 4));
    assert_eq!((at[&1181].5, at[&1182].5), ([None, None], [None, Some(PANEL_STEP)]));
}

#[test]
fn stonehenge_tails_end_closed_for_the_map_to_open() {
    // 11162: mode 3 from f1409 (152) to 256 on f1422; the map bank is back
    // on f1423 under the closed letterbox and the map opens it from f1424
    // (`campaign_map::Handler::StoryExit`), so the stage ends on f1423
    // [emu trace-seto-cam; white 12132 f1666/f1679/f1680 alike].
    for event in [11162u16, 12132] {
        let carry = Carry { bars: LETTERBOX_CLOSED, ..Carry::default() };
        let pages = [msg(top(1), 0), msg(bot(1), 0)];
        let spec = event_motion(event).unwrap();
        let mut m = StoryMotion::from_pages(&format!("map-story-{event}"), spec, pages.to_vec(), Some(carry)).unwrap();
        let mut heights = Vec::new();
        let mut forward = None;
        for update in 1..5000u32 {
            let r = m.step();
            heights.push(m.letterbox.height());
            if m.waiting() {
                m.press(Button::Cross);
            }
            if r.forward.is_some() && m.finished() {
                forward = Some(update as usize);
                break;
            }
        }
        let f = forward.expect("the tail ends");
        let h = |u: usize| heights[u - 1];
        assert_eq!((h(f - 15), h(f - 14), h(f - 1), h(f)), (144, 152, 256, 256), "{event}");
        let layers = m.layers(&[], &[], &|_| None);
        assert!(layers.is_empty() || m.letterbox.height() == 256);
    }
}

#[test]
fn white_12150_waits_for_the_effect_and_hands_its_panel_to_12152() {
    // 12150 (story start f122 after the map's close and the scene load):
    // mode 2 f123..f136, U0 f137; page 3388 U0 f175, the clock and the
    // shake from f176; phase 11 f189, 12 f190; W+5470 = 5 f375, shake 3
    // f376, 4 f396 -> C2 = 12152 f397 [emu white trace-manawyddan].
    let pages = [msg(top(1), 3384), msg(top(1), 3388)];
    let r = run(12150, &pages, closed(), 122, &[169, 183], 396);
    assert_eq!(r.frames(|s| s.waiting), vec![169, 183]);
    assert_eq!(r.at(137).panels, [None, Some(PANEL_STEP)]);
    assert_eq!(r.frames(|s| s.forward), vec![175, 396]);
    let m = &r.motion;
    let u = |frame: u32| frame - 122 + 1;
    assert_eq!(m.effect_stage(u(175)), 0);
    assert_eq!((m.effect_stage(u(176)), m.effect_stage(u(224)), m.effect_stage(u(225)), m.effect_stage(u(375))), (1, 1, 2, 5));
    assert!(m.finished());
    let carry = m.carry();
    assert_eq!((carry.dialogue, carry.panels, carry.pan, carry.shake.state), (true, [None, Some(PANEL_FULL)], -100, 0));
    // 12152: U0 f398, the bottom panel in f398..f413 while the top one
    // slides out; the pan leaves -100 on f414.
    let pages = [msg(bot(1), 3392), msg(bot(1), 3396)];
    let r2 = run(12152, &pages, Some(carry), 397, &[439], 450);
    assert_eq!((r2.at(397).panels, r2.at(398).panels), ([None, None], [Some(PANEL_STEP), None]));
    assert_eq!(r2.at(413).panels, [Some(PANEL_FULL), None]);
    assert_eq!((r2.at(413).pan, r2.at(414).pan, r2.at(415).pan), (-100, -100, -90));
    assert_eq!(r2.at(397).lb, 144);
}

#[test]
fn reveal_rows_finish_in_7_13_19_updates() {
    for (rows, n) in [(1, 7), (2, 13), (3, 19)] {
        assert!(reveal_alphas(rows, n)[rows - 1][3] >= 0x80);
        assert!(reveal_alphas(rows, n - 1)[rows - 1][3] < 0x80);
    }
}

/// The motion over the user's real story import (ROSES_STORY_MOTION_IMPORT =
/// its manifest.json): every event chain plays to its end with Cross on each
/// wait, every update's layers pass the bounds checks, and the chained events
/// carry the letterbox. With ROSES_STORY_MOTION_PNG = a new directory, a few
/// mid-motion frames are written there.
#[test]
fn private_story_motion_plays_the_imported_events() {
    let Some(path) = std::env::var_os("ROSES_STORY_MOTION_IMPORT") else {
        eprintln!("skipped: ROSES_STORY_MOTION_IMPORT is not set");
        return;
    };
    let png_dir = std::env::var_os("ROSES_STORY_MOTION_PNG").map(std::path::PathBuf::from);
    if let Some(dir) = &png_dir {
        std::fs::create_dir_all(dir).unwrap();
    }
    let loaded = crate::Loaded::open(std::path::Path::new(&path)).unwrap();
    // With ROSES_FACE_EXE the panels' faces animate too (and the PNGs show
    // talking frames and Seto's shake).
    let mut faces = std::env::var_os("ROSES_FACE_EXE").map(|exe| {
        crate::face_anim::StoryFaces::new(
            crate::face_anim::FaceTables::from_executable(&std::fs::read(exe).unwrap()).unwrap(),
        )
    });
    let mut extra_shots: Vec<String> = Vec::new();
    // (chain, updates of the first event to write as PNG)
    let chains: [(&[u16], &[u32]); 6] = [
        (&[11122], &[8, 60, 100, 118]),
        (&[11140, 11142, 11143, 11144, 11146, 11147, 11148], &[12, 26, 30, 40, 55]),
        (&[11160, 11162], &[]),
        (&[11180, 11182], &[]),
        (&[12130, 12132], &[]),
        (&[12150, 12152, 12154], &[]),
    ];
    // ROSES_MAP_BEHIND_ORACLE = `story/the project notes
    // (extract_oracle.py): per chain and traced frame what the original shows
    // behind the 2D layers (M map, S scene 0, . covered). Each update's
    // overlay must leave the world visible exactly where the trace does, and
    // the stage's `behind` world must be the traced one.
    let oracle: Option<serde_json::Value> = std::env::var_os("ROSES_MAP_BEHIND_ORACLE")
        .map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap());
    let mut behind_errors = Vec::new();
    let mut behind_checked = 0usize;
    let mut report = Vec::new();
    for (chain, shots) in chains {
        let mut model = crate::Model::new(loaded.manifest.clone()).unwrap();
        model.jump_to(&format!("map-story-{}", chain[0])).unwrap();
        let mut motion = StoryMotion::new(&loaded, &model, None).expect("first event moves");
        let traced = oracle.as_ref().map(|o| {
            let c = &o[chain[0].to_string()];
            (c["start"].as_u64().unwrap() as u32, c["frames"].as_str().unwrap().as_bytes().to_vec())
        });
        let mut index = 0;
        let mut updates = 0u32;
        loop {
            let r = motion.step();
            updates += 1;
            assert!(updates < 20_000, "chain {chain:?} does not end");
            if let Some(f) = faces.as_mut() {
                f.step(&loaded.story_page_layers(&model).unwrap(), motion.talking());
            }
            let layers = loaded
                .active_layers_faces(&model, Some(&motion), faces.as_ref())
                .unwrap_or_else(|e| panic!("{} update {}: {e}", motion.stage(), motion.update));
            if let Some((start, frames)) = &traced {
                let frame = start + updates - 1;
                match frames.get((updates - 1) as usize) {
                    Some(&want) => {
                        // (The overlay without the runtime name text,
                        // which never covers the screen.)
                        let c = loaded.manifest.canvas;
                        let mut rgba = vec![0u8; (c.width * c.height * 4) as usize];
                        for layer in &layers {
                            let image = loaded.assets[&layer.asset].image.as_ref().unwrap();
                            crate::composite(&mut rgba, c.width, image, layer);
                        }
                        let shows = rgba.chunks_exact(4).any(|p| p[3] < 255);
                        let event: u16 = motion.stage().trim_start_matches("map-story-").parse().unwrap();
                        let world = match behind(event) {
                            Some(Behind::Map) => b'M',
                            Some(Behind::Scene0) => b'S',
                            None => b'?',
                        };
                        let ours = if shows { world } else { b'.' };
                        behind_checked += 1;
                        if ours != want {
                            behind_errors.push(format!(
                                "{} update {} (f{frame}): ours {} traced {}",
                                motion.stage(),
                                motion.update,
                                ours as char,
                                want as char
                            ));
                        }
                    }
                    None => behind_errors.push(format!("{} f{frame}: past the traced chain", motion.stage())),
                }
            }
            // Seto shaking (figure moved by the floored offsets) and a
            // talking face, once each per stage.
            let tag = if motion.camera().shake != [0.0; 2] && motion.camera().shake[0] <= -2.0 {
                Some("shake")
            } else if motion.talking() != [false; 2] && motion.update % 7 == 0 {
                Some("talk")
            } else {
                None
            };
            let key = tag.map(|t| format!("{}-{t}", motion.stage()));
            let extra = key.as_ref().is_some_and(|k| !extra_shots.contains(k));
            if let Some(k) = key.filter(|_| extra) {
                extra_shots.push(k);
            }
            if (index == 0 && shots.contains(&motion.update)) || extra {
                if let Some(dir) = &png_dir {
                    let c = loaded.manifest.canvas;
                    let mut rgba = vec![0u8; (c.width * c.height * 4) as usize];
                    for p in rgba.chunks_exact_mut(4) {
                        p[3] = 255;
                    }
                    for layer in &layers {
                        let image = loaded.assets[&layer.asset].image.as_ref().unwrap();
                        crate::composite(&mut rgba, c.width, image, layer);
                    }
                    let file = std::fs::File::create(dir.join(format!("{}-u{:03}.png", motion.stage(), motion.update))).unwrap();
                    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), c.width, c.height);
                    enc.set_color(png::ColorType::Rgba);
                    enc.set_depth(png::BitDepth::Eight);
                    enc.write_header().unwrap().write_image_data(&rgba).unwrap();
                }
            }
            if motion.waiting() {
                motion.press(Button::Cross);
            }
            if r.forward.is_some() {
                let stage = model.stage().id().to_owned();
                let _ = model.input(Button::Cross);
                if motion.finished() {
                    report.push(format!("{} {} updates", stage, motion.update));
                    index += 1;
                    let Some(&next) = chain.get(index) else { break };
                    let id = format!("map-story-{next}");
                    if model.stage().id() != id {
                        // A chain the import ends elsewhere (the map windows).
                        model.jump_to(&id).unwrap();
                    }
                    motion = StoryMotion::new(&loaded, &model, Some(motion.carry()))
                        .unwrap_or_else(|| panic!("{id} does not move"));
                } else {
                    assert_eq!(model.page_index(), motion.page, "{stage}: page out of step");
                }
            }
        }
    }
    eprintln!("{}", report.join("\n"));
    if oracle.is_some() {
        eprintln!("behind: {behind_checked} updates checked against the traces");
        assert!(behind_checked > 4000);
        assert!(
            behind_errors.is_empty(),
            "{} updates differ:\n{}",
            behind_errors.len(),
            behind_errors.iter().take(40).cloned().collect::<Vec<_>>().join("\n")
        );
    }
    // Updates from C2 to the next C2 with Cross on every wait, as traced;
    // 11160/11180/12150 start 45 updates after their C2: the map closes
    // its letterbox over itself first (`Handler::StoryEntry`); 11162/12132
    // end 14 updates before their next C2: the map opens the letterbox
    // over itself (`Handler::StoryExit`).
    for (stage, updates) in [
        (11122, 141),
        (11140, 823),
        (11160, 570 - 45),
        (11162, 665 - 14),
        (11180, 440 - 45),
        (12130, 146),
        (12132, 884 - 14),
        (12150, 320 - 45),
        (12152, 473),
        (11142, 190),
        (11143, 13),
        (11144, 156),
        (11146, 72),
        (11147, 23),
        (11148, 424),
        (11182, 268),
        (12154, 448),
    ] {
        let line = format!("map-story-{stage} {updates} updates");
        assert!(report.contains(&line), "{line} not in {report:?}");
    }
}
