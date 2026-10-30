//! The White Rose (Yorkist) route's map story events, written by
//! `roses import-story-red` next to the red route's (PAL SLES_515.79;
//! private research `story/claude-story-white/NOTES.md`,
//! `story/claude-campaign-map/WHITE-ROUTE-AUDIT.md` §2–3).
//!
//! Traced in the EE interpreter (`claude-story-white/tools/trace_white.py`,
//! the game's own handlers from the patched post-duel state with SAVE[0] =
//! 1; every 2D draw leaf recorded) [emu]:
//! - 12040 (0x221AD4, Dover) and 12041 (0x221BEC, the Strait): Mako's
//!   bottom panel (face 15, colour selector 0) over the live campaign map
//!   (52/53 model draws per frame), 144-row letterbox, no big figure (no
//!   TEXEVE bound), moods 0. 12040 continues at map event 104 (the
//!   voyage), 12041 at its own tail, which the map plays (node 16 rises,
//!   C2 = 0).
//! - 12130 (0x221DC4): scene-0 world (bank 0x1E91B0(3, 0, 0), W+53B5 3),
//!   Seto's three bottom pages with his figure (slot 6 = TEXEVE 18 at rest,
//!   the 11160 position; slot 7 = TEXEVE 23 off screen); music (36, 1).
//! - 12132 (0x221FFC): scene-0 world (W+53B5 2, level 0x1000 lowered),
//!   Manawyddan (top, **selector 0** on this side; 1 on the red side) and
//!   Seto (bottom, selector 1), no figures, panels latch; moods 0, then
//!   bottom 2 from 3332; effect task 401, SFX 34/35, music (33, 1), then
//!   (4, 0), map bank again and C2 = 0 (map at Stonehenge, state 1).
//! - 12150 (0x22241C): scene 0, Manawyddan's two top pages, moods 2/2.
//! - 12152 (0x222680): scene 0, Seto's 19 bottom pages with the TEXEVE 18
//!   figure, top panel off, mood 2; music (37, 1).
//! - 12154 (0x2227E0): letterbox 72 and the 2x2 mosaic of TEXEVE 14, 16,
//!   13, 15 (412x438 quads, UV 512x384) at B0 = 0, B2 = −470: one
//!   painting-only Cross wait, then Seto's seven bottom pages over the
//!   paintings with the 144-row letterbox (mood 0). Afterwards the mosaic
//!   fades (W+54A4/W+54A6) while the scene-0 camera rises (W+62 = 1, W+64
//!   +16 per update from 0): not a settled page.
//! - 12156 (0x222AB4): 15 captions at raw (89, 384) over the scene-0
//!   world (W+53B5 1, camera level 1040 at entry, W+53F6 = −0x4000, both
//!   carried from 12154), music (38, 1); after 3508 the ending
//!   (`roses_client::ending::RouteEnding::white`).
//!
//! Event 12000 (Seto sends Simon away, the side choice's Yorkists row) is
//! `import_story_map`'s `seto-yorkists` stage; the trace confirms its pages
//! and figures from 3044 on (NOTES.md).
use crate::import_story_red::{
    bot, cap, painting, still, top, Next, Painting, RedEvent, RedPage, Scene, MAKO, MANAWYDDAN,
    SETO, SETO_FIGURE,
};

pub(crate) const EVENT_DOVER_VOYAGE: u16 = 104;
/// 12041 continues at its own tail, played by the map (0x221CFC..: node
/// 16's rise flag W+530C, letterbox mode 0, C2 = 0) [F, emu].
pub(crate) const EVENT_STRAIT_TAIL: u16 = 12041;

/// Pages whose text embeds the player name (message 3 slots).
pub(crate) const NAME_MESSAGES: [u16; 0] = [];

/// 12154: slots 2/3/4/5 = TEXEVE 14/16/13/15 (0x229570) as 412x438 quads
/// at (B0, B2+74), (B0+412, B2+74), (B0, B2+512), (B0+412, B2+512) with
/// B0 = 0, B2 = −470 at the wait and through the pages [emu f1041..f1221].
pub(crate) const MOSAIC_12154: &[Painting] = &[
    painting(14, [512, 384], [0, -396, 412, 438]),
    painting(16, [512, 384], [412, -396, 412, 438]),
    painting(13, [512, 384], [0, 42, 412, 438]),
    painting(15, [512, 384], [412, 42, 412, 438]),
];

const fn seto(msg: u16) -> RedPage {
    bot(msg, SETO, 1)
}
const fn mako(msg: u16) -> RedPage {
    bot(msg, MAKO, 0)
}
const fn manawyddan(msg: u16) -> RedPage {
    top(msg, MANAWYDDAN, 0)
}

pub(crate) const WHITE_EVENTS: [RedEvent; 8] = [
    RedEvent {
        event: 12040,
        scene: Scene::Dialogue {
            bottom: None,
            top: None,
        },
        moods: &[],
        pages: &[mako(3140)],
        next: Next::Map(EVENT_DOVER_VOYAGE),
    },
    RedEvent {
        event: 12041,
        scene: Scene::Dialogue {
            bottom: None,
            top: None,
        },
        moods: &[],
        pages: &[
            mako(3144),
            mako(3145),
            mako(3148),
            mako(3152),
            mako(3156),
            mako(3160),
            mako(3161),
        ],
        next: Next::Map(EVENT_STRAIT_TAIL),
    },
    RedEvent {
        event: 12130,
        scene: Scene::Dialogue {
            bottom: Some(SETO_FIGURE),
            top: None,
        },
        moods: &[],
        pages: &[seto(3304), seto(3305), seto(3308)],
        next: Next::Stage(12132),
    },
    RedEvent {
        event: 12132,
        scene: Scene::Dialogue {
            bottom: None,
            top: None,
        },
        moods: &[(3312, 0, 0), (3332, 2, 0)],
        pages: &[
            manawyddan(3312),
            manawyddan(3316),
            manawyddan(3320),
            manawyddan(3324),
            manawyddan(3325),
            manawyddan(3328),
            seto(3332),
            manawyddan(3336),
            manawyddan(3340),
            manawyddan(3341),
            seto(3344),
            seto(3348),
            seto(3349),
            seto(3352),
            manawyddan(3356),
        ],
        // C2 = 0 (0x2223D0): the free map at Stonehenge (state 1).
        next: Next::Map(0),
    },
    RedEvent {
        event: 12150,
        scene: Scene::Dialogue {
            bottom: None,
            top: None,
        },
        moods: &[(3384, 2, 2)],
        pages: &[manawyddan(3384), manawyddan(3388)],
        next: Next::Stage(12152),
    },
    RedEvent {
        event: 12152,
        scene: Scene::Dialogue {
            bottom: Some(SETO_FIGURE),
            top: None,
        },
        moods: &[(3392, 2, 2)],
        pages: &[
            seto(3392),
            seto(3396),
            seto(3400),
            seto(3401),
            seto(3404),
            seto(3405),
            seto(3408),
            seto(3409),
            seto(3412),
            seto(3416),
            seto(3417),
            seto(3420),
            seto(3421),
            seto(3424),
            seto(3428),
            seto(3432),
            seto(3436),
            seto(3437),
            seto(3440),
        ],
        next: Next::Stage(12154),
    },
    RedEvent {
        event: 12154,
        scene: Scene::PaintedDialogue(MOSAIC_12154),
        moods: &[],
        pages: &[
            still(MOSAIC_12154),
            seto(3444),
            seto(3448),
            seto(3452),
            seto(3456),
            seto(3460),
            seto(3461),
            seto(3462),
        ],
        next: Next::Stage(12156),
    },
    RedEvent {
        event: 12156,
        scene: Scene::World,
        moods: &[],
        pages: &[
            cap(3464),
            cap(3465),
            cap(3468),
            cap(3469),
            cap(3472),
            cap(3476),
            cap(3480),
            cap(3484),
            cap(3488),
            cap(3489),
            cap(3492),
            cap(3496),
            cap(3500),
            cap(3504),
            cap(3508),
        ],
        next: Next::Ending,
    },
];

pub(crate) const REPORT_NOTES: [&str; 4] = [
    "White route events 12040, 12041 (Mako over the campaign map: the story stages draw black behind them when played as stages), 12130, 12132, 12150, 12152 (scene-0 world: drawn by the game's story scene), 12154 (Seto over the TEXEVE 14/16/13/15 mosaic) and 12156 (captions over the rising scene-0 camera, then the white ending). Event 12000 is import-story-map's seto-yorkists.",
    "Speaker colour selectors traced per side: Mako 0, Seto 1, Manawyddan 0 on the white side (1 on the red side).",
    "Not drawn: the mosaic's fade after 12154's pages (W+54A4/W+54A6), the C-module effects 401 (12132) and 402 (12156), the overlays W+53A4 30-33 and 4, the 12130 figure shake (W+5483) and the 3D map behind 12040/12041.",
    "Music of these events (35, 36, 33, 37, 38) is cued by the game at stage entry (opening::stage_music).",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_story_red::{moods_at, painting_layer, Latch, PageKind};

    fn event(id: u16) -> &'static RedEvent {
        WHITE_EVENTS.iter().find(|e| e.event == id).unwrap()
    }

    #[test]
    fn white_events_chain_and_count_the_traced_pages() {
        let counts: Vec<(u16, usize)> = WHITE_EVENTS
            .iter()
            .map(|e| (e.event, e.pages.iter().filter(|p| p.msg != 0).count()))
            .collect();
        assert_eq!(
            counts,
            [
                (12040, 1),
                (12041, 7),
                (12130, 3),
                (12132, 15),
                (12150, 2),
                (12152, 19),
                (12154, 7),
                (12156, 15)
            ]
        );
        // With 12000's 27 pages: the 96 white pages of story_sizing.json.
        assert_eq!(counts.iter().map(|c| c.1).sum::<usize>() + 27, 96);
        let ids: Vec<u16> = WHITE_EVENTS.iter().map(|e| e.event).collect();
        let mut maps = Vec::new();
        for e in &WHITE_EVENTS {
            match e.next {
                Next::Stage(n) => assert!(ids.contains(&n), "{} -> {n}", e.event),
                Next::Map(m) => maps.push((e.event, m)),
                Next::Ending => assert_eq!(e.event, 12156),
            }
        }
        assert_eq!(maps, [(12040, 104), (12041, 12041), (12132, 0)]);
        assert_eq!(
            WHITE_EVENTS.last().unwrap().pages.last().unwrap().msg,
            3508
        );
    }

    #[test]
    fn white_panels_moods_and_mosaic() {
        assert_eq!(moods_at(event(12132), 3328), (0, 0));
        assert_eq!(moods_at(event(12132), 3356), (2, 0));
        assert_eq!(moods_at(event(12152), 3440), (2, 2));
        assert_eq!(moods_at(event(12154), 3462), (0, 0));
        // 12132: Manawyddan's top panel alone until Seto's first page.
        let mut latch = Latch::default();
        let mut bottoms = Vec::new();
        for p in event(12132).pages {
            latch.see(p.kind);
            bottoms.push(latch.bottom.is_some());
        }
        assert_eq!(bottoms.iter().position(|b| *b), Some(6));
        assert!(bottoms[6..].iter().all(|b| *b));
        // The mosaic: the quads cover the whole screen above the letterbox.
        let layers: Vec<_> = MOSAIC_12154.iter().filter_map(painting_layer).collect();
        assert_eq!(layers.len(), 4);
        assert_eq!(layers[0].destination, [0, 0, 515, 37]);
        assert_eq!(layers[2].destination, [0, 37, 515, 383]);
        assert_eq!(layers[1].source.map(|s| s[2]), Some(124));
        assert!(matches!(event(12154).pages[0].kind, PageKind::Still(_)));
    }

    /// Private oracle: `ROSES_STORY_WHITE_ORACLE` = the research directory
    /// `story/claude-story-white` with `trace-{dover,brest,manawyddan}.json`
    /// (tools/trace_white.py). Every page: letterbox, pan W+5439, top-panel
    /// latch, figure / painting quads, moods, panel colour selectors.
    #[test]
    fn private_white_pages_match_the_traced_frames() {
        let Some(dir) = std::env::var_os("ROSES_STORY_WHITE_ORACLE") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let traces: Vec<serde_json::Value> = ["trace-dover.json", "trace-brest.json", "trace-manawyddan.json"]
            .iter()
            .map(|n| serde_json::from_slice(&std::fs::read(dir.join(n)).unwrap()).unwrap())
            .collect();
        let rows: Vec<&serde_json::Value> = traces
            .iter()
            .flat_map(|t| t["rows"].as_array().unwrap())
            .collect();
        let int = |v: &serde_json::Value| v.as_i64().unwrap() as i32;
        let slot_record = |event: u16, slot: i32| -> u16 {
            match (event, slot) {
                (12154, 2) => 14,
                (12154, 3) => 16,
                (12154, 4) => 13,
                (12154, 5) => 15,
                (_, 6) => 18,
                (_, 7) => 23,
                _ => panic!("no slot {slot} in {event}"),
            }
        };
        let quads = |row: &serde_json::Value, lo: i32, hi: i32| {
            let event = int(&row["C2"]) as u16;
            row["calls"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["fn"] == "quad" && (lo..=hi).contains(&int(&c["slot"])))
                .filter(|c| c["rgba"][3].as_i64() != Some(0))
                .filter_map(|c| {
                    let d: Vec<i32> = c["dest"].as_array().unwrap().iter().map(int).collect();
                    let uv: Vec<i32> = c["uv"].as_array().unwrap().iter().map(int).collect();
                    (d[2] > 0 && d[3] > 0 && d[0] < 512 && d[1] < 512).then(|| {
                        (
                            slot_record(event, int(&c["slot"])),
                            [uv[2] as u32, uv[3] as u32],
                            [d[0], d[1], d[2] - d[0], d[3] - d[1]],
                        )
                    })
                })
                .collect::<Vec<_>>()
        };
        // The panel emblem (TEXSYS 7 (224, 32·selector)) at raw (16, 16)
        // (top) or (136, 368) (bottom).
        let emblem = |row: &serde_json::Value, at: [i32; 2]| -> Option<u8> {
            row["calls"].as_array().unwrap().iter().find_map(|c| {
                let d: Vec<i32> = c["dest"].as_array()?.iter().map(int).collect();
                let uv: Vec<i32> = c["uv"].as_array()?.iter().map(int).collect();
                (c["fn"] == "quad" && d[..2] == at && uv[0] == 224).then_some((uv[1] / 32) as u8)
            })
        };
        let lb = |row: &serde_json::Value| {
            row["lb"][2].as_i64().unwrap() + row["lb"][3].as_i64().unwrap() + row["lb"][4].as_i64().unwrap()
        };
        let expect = |p: &[Painting]| p.iter().map(|q| (q.record, q.uv, q.raw)).collect::<Vec<_>>();
        for e in &WHITE_EVENTS {
            let mut latch = Latch::default();
            for p in e.pages {
                latch.see(p.kind);
                let row = *rows
                    .iter()
                    .find(|r| {
                        int(&r["C2"]) == i32::from(e.event)
                            && if p.msg == 0 { r["ae"] == 2 } else { r["msg"] == p.msg && r["disp"] == 4 }
                    })
                    .unwrap_or_else(|| panic!("no traced frame for {} {}", e.event, p.msg));
                match (e.scene, p.kind) {
                    (_, PageKind::Still(paintings)) => {
                        assert_eq!(lb(row), 72);
                        assert_eq!(quads(row, 2, 5), expect(paintings));
                    }
                    (Scene::World, PageKind::Caption) => {
                        assert_eq!(lb(row), 144);
                        assert!(quads(row, 2, 7).is_empty());
                    }
                    (scene, PageKind::Bottom(_, sel) | PageKind::Top(_, sel)) => {
                        assert_eq!(lb(row), 144, "{}", p.msg);
                        let bottom = matches!(p.kind, PageKind::Bottom(..));
                        assert_eq!(row["f5439"], if bottom { 100 } else { -100 }, "{}", p.msg);
                        assert_eq!(row["b6"] == 1, latch.top.is_some(), "{}", p.msg);
                        let at = if bottom { [136, 368] } else { [16, 16] };
                        assert_eq!(emblem(row, at), Some(sel), "selector {}", p.msg);
                        let (b, t) = moods_at(e, p.msg);
                        assert_eq!(row["mood"], serde_json::json!([b, t]), "mood {}", p.msg);
                        match scene {
                            Scene::PaintedDialogue(paintings) => {
                                assert_eq!(quads(row, 2, 5), expect(paintings));
                                assert!(quads(row, 6, 7).is_empty());
                            }
                            Scene::Dialogue { bottom: fb, top: ft } => {
                                let want: Vec<_> = if bottom { fb } else { ft }
                                    .map(|f| (f.record, f.uv, f.raw.map(|v| v as i32)))
                                    .into_iter()
                                    .collect();
                                let got = quads(row, 6, 7);
                                assert_eq!(got.len(), want.len(), "figure {}", p.msg);
                                // W+5483 (12130 from 3308): a shake of a pixel or two.
                                for (g, w) in got.iter().zip(&want) {
                                    assert_eq!((g.0, g.1), (w.0, w.1));
                                    assert!((0..4).all(|i| (g.2[i] - w.2[i]).abs() <= 4), "{}", p.msg);
                                }
                            }
                            _ => panic!("{} {}", e.event, p.msg),
                        }
                    }
                    other => panic!("unexpected {other:?}"),
                }
            }
        }
    }
}
