//! `roses import-story-red`: extend the story-to-map import with the story
//! events the campaign map fires on the Lancastrian route (PAL SLES_515.79;
//! private research `story/the project notes §5–6 and the
//! static dispatch summaries): the Bosworth trigger 11120 → 11122 → 11124,
//! the Bosworth win 11140 → 11142 → 11143 → 11144 → 11146 → 11147 → 11148 →
//! 11149, the Seto win 11160 → 11162 and the Manawyddan win 11180 → 11182 →
//! 11184 — and, in the same pass, the white route's 12040..12156
//! (`import_story_white`). Each event becomes a `Stage::Story` named `map-story-<event>`
//! whose pages are the messages the story dispatcher 0x21DD80 shows, with
//! the speaker windows of the dialogue code: layout 2 = the bottom window
//! (name in `bot`, colour selector c1), layout 3 = the top window (`top`,
//! c0), layout 1 = a caption with no speaker. The chains end with
//! `StoryEnd::MapEvent` (the map resumes at 11124, 108 or 112); 11184 ends
//! with `MapEvent(11184)`: the rest of that map event is the ending
//! (letterbox close, memory-card op 2, movie EDMV_A6, title), which the
//! game runs with `roses_client::ending` (research `story/the project notes).
//! Both ending movies, MOVIE/EDMV_A6.PSS and MOVIE/EDMV_C6.PSS, are copied
//! as is (`media/ending-red.pss`, `media/ending-white.pss`, assets
//! `ending-movie-red` / `ending-movie-white`) after a strict full decode
//! with the built-in decoder, as the opening's PSS.
//!
//! Presentation, traced in the EE interpreter (research
//! `story/the project notes): the background drawer 0x22E8B0
//! (TEXEVE paintings bound per event by 0x229570, slid by W+53B0/W+53B2),
//! the big figures (texture slots 6/7: TEXEVE 20/19, 22/19, 18), the
//! letterbox 0x233750 (black bands of 72, 144 or 256 raw rows), the centred
//! map window 0x232C80 (W+5487), the caption origin of layout 1, raw
//! (89, 384), and the speaker panels with the face engine's frame-0
//! portrait (`story_faces`, moods per page). Each page is the settled frame
//! the original waits on for Cross, including the painting-only waits
//! (W+53AE == 2). Panels latch: the bottom panel stays from the first bottom
//! page on, the top panel from the first top page on (0x53B7/0x53B6). Not
//! drawn: the 3D worlds behind 11120/11124/11149 (campaign map) and 11160,
//! 11162, 11180, 11184 (bank 0x1E91B0(3, 0, 0) = MODEL.MRG pack 33,
//! 10 objects), slide/letterbox motion, cross-fades, the 11160 figure shake
//! (W+5483), text reveal and the C-module effects 401/402 (0x1F5B00).
use crate::import_post_name::{name_cells, name_text_for, simon_common};
use crate::import_story::{frame_layers, import_png, message_image, raw_rect, story_layer};
use crate::import_story_map::{label_bar, raw_origin, text_layer, top_panel};
use crate::story_faces;
use crate::{emit_json, open_pal, Result};
use roses_assets::{
    decode_pal_message_font, parse_pal_message_templates, parse_pal_messages, MessageTemplateToken,
    OpeningArchiveKind,
};
use roses_client::campaign_map as map;
use roses_client::face_anim::FaceTables;
use roses_client::{Button, Layer, Loaded, Stage, StoryEnd, StoryPage};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::Path;

/// Where a page's text and speaker window are (dialogue code, FLOW.md §9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PageKind {
    /// Layout 1: caption/narration, no speaker window (text at raw (89, 384)).
    Caption,
    /// Layout 2: bottom window with the speaker's name and colour selector.
    Bottom(u16, u8),
    /// Layout 3: top window with the speaker's name and colour selector.
    Top(u16, u8),
    /// Layout 3 without a speaker (0x53B6 stays off): text at the top, no
    /// top panel, no figure (W+5439 = 0) [emu: 2700].
    TopVoice,
    /// W+5487 != 0: the centred window 0x232C80 over the map (11120, 11124,
    /// 11149), message chosen by the event, not by the dialogue counter.
    Window,
    /// A painting-only Cross wait (W+53AE == 2, thin 72-row letterbox).
    Still(&'static [Painting]),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RedPage {
    /// Message id; 0 for a `Still`.
    pub msg: u16,
    pub kind: PageKind,
}

/// One on-screen quad of the background drawer 0x22E8B0 at the settled
/// slide: TEXEVE record, its full UV extent (= the decoded image size) and
/// the raw 512x512 destination, which may extend past the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Painting {
    pub record: u16,
    pub uv: [u32; 2],
    pub raw: [i32; 4],
}

/// A big figure (0x2306A0) at rest: TEXEVE record, UV extent, raw rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Figure {
    pub record: u16,
    pub uv: [u32; 2],
    pub raw: [u32; 4],
}

/// What lies behind an event's pages.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Scene {
    /// A 3D world behind the 144-row letterbox: the campaign map (11120,
    /// 11124, 11149) or the "scene 0" bank 0x1E91B0(3, 0, 0) (11184). The
    /// world itself is not drawn here (black).
    World,
    /// Captions over these paintings and the 144-row letterbox.
    Paintings(&'static [Painting]),
    /// Captions over the closed letterbox (mode 3, 256 rows each side).
    Curtain,
    /// Dialogue over settled paintings (12154: Seto's pages over the
    /// TEXEVE 14/16/13/15 mosaic), 144-row letterbox, latched panels [emu].
    PaintedDialogue(&'static [Painting]),
    /// Dialogue: bottom/top speaker figure (none where the original draws
    /// none), 144-row letterbox, latched panels.
    Dialogue {
        bottom: Option<Figure>,
        top: Option<Figure>,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Next {
    Stage(u16),
    Map(u16),
    Ending,
}

pub(crate) struct RedEvent {
    pub event: u16,
    pub scene: Scene,
    pub pages: &'static [RedPage],
    /// Panel moods W+546B (bottom) / W+546C (top) from a message on
    /// (message, bottom, top); the face expression is 0x298DAC[4·speaker +
    /// mood] [emu: trace-*.json]. Empty = 0, 0.
    pub moods: &'static [(u16, u8, u8)],
    pub next: Next,
}

pub(crate) const SETO: u16 = 198;
const PEGASUS: u16 = 206;
const RICHARD: u16 = 207;
const YUGI: u16 = 216;
pub(crate) const MANAWYDDAN: u16 = 217;
/// Mako (white route, 12040/12041): face 15 = TEXANM record 15.
pub(crate) const MAKO: u16 = 211;
pub(crate) const fn cap(msg: u16) -> RedPage {
    RedPage {
        msg,
        kind: PageKind::Caption,
    }
}
pub(crate) const fn bot(msg: u16, speaker: u16, selector: u8) -> RedPage {
    RedPage {
        msg,
        kind: PageKind::Bottom(speaker, selector),
    }
}
const fn top_voice(msg: u16) -> RedPage {
    RedPage {
        msg,
        kind: PageKind::TopVoice,
    }
}
const fn win(msg: u16) -> RedPage {
    RedPage {
        msg,
        kind: PageKind::Window,
    }
}
pub(crate) const fn still(paintings: &'static [Painting]) -> RedPage {
    RedPage {
        msg: 0,
        kind: PageKind::Still(paintings),
    }
}
pub(crate) const fn painting(record: u16, uv: [u32; 2], raw: [i32; 4]) -> Painting {
    Painting { record, uv, raw }
}

// Settled quads of 0x22E8B0 [emu, trace-*.json]. The textures are bound by
// 0x229570: 11122 slots 2/3 = TEXEVE 3, 4/5 = 2; 11142 5, 4; 11144 6/7,
// 6/7; 11146 8. Slots 3/5 lie at raw x 512.. (off screen) and are omitted.
/// 11122: W+53B2 -448 -> -91 (+4/update); quads at y = W+53B2 + 74, + 438.
const SLIDE_11122: &[Painting] = &[
    painting(3, [640, 384], [0, -17, 512, 438]),
    painting(2, [640, 384], [0, 421, 512, 438]),
];
/// 11142: W+53B2 0 -> -512 (-4/update); TEXEVE 5 has left the screen.
const SLIDE_11142: &[Painting] = &[painting(4, [640, 384], [0, 0, 512, 438])];
/// 11144: slot 3 (TEXEVE 7) under slot 2 (TEXEVE 6), both full screen.
const PAINTINGS_11144: &[Painting] = &[
    painting(7, [640, 448], [0, 0, 512, 512]),
    painting(6, [640, 448], [0, 0, 512, 512]),
];
/// 11144 sub-step 5: TEXEVE 6 fades out (alpha = W+54A6, 126 -> 0).
const AFTER_11144: &[Painting] = &[painting(7, [640, 448], [0, 0, 512, 512])];
const PAINTINGS_11146: &[Painting] = &[painting(8, [640, 448], [0, 0, 512, 512])];
// Big figures at rest (p = +100 bottom speaker in slot 6, -100 top in slot 7).
const RICHARD_FIGURE: Figure = Figure {
    record: 20,
    uv: [384, 256],
    raw: [88, 132, 312, 288],
};
const PEGASUS_FIGURE: Figure = Figure {
    record: 19,
    uv: [256, 256],
    raw: [152, 132, 208, 288],
};
const YUGI_FIGURE: Figure = Figure {
    record: 22,
    uv: [256, 256],
    raw: [152, 132, 208, 288],
};
/// 11160 slot 6 (Seto); 11162 and 11180 draw no big figure [emu].
pub(crate) const SETO_FIGURE: Figure = Figure {
    record: 18,
    uv: [512, 256],
    raw: [24, 132, 416, 288],
};
/// 11182: slots 2/3/4/5 = TEXEVE 10/12/9/11, a 2x2 mosaic of 412x438 quads
/// (UV 512x384) at (B0, B2+74), (B0+412, B2+74), (B0, B2+512),
/// (B0+412, B2+512). Captions and the first wait at B0 = -156, B2 = -219;
/// the second wait after the captions at B0 = -312, B2 = 0 [emu].
const MOSAIC_11182: &[Painting] = &[
    painting(10, [512, 384], [-156, -145, 412, 438]),
    painting(12, [512, 384], [256, -145, 412, 438]),
    painting(9, [512, 384], [-156, 293, 412, 438]),
    painting(11, [512, 384], [256, 293, 412, 438]),
];
const AFTER_11182: &[Painting] = &[
    painting(10, [512, 384], [-312, 74, 412, 438]),
    painting(12, [512, 384], [100, 74, 412, 438]),
];
pub(crate) const fn top(msg: u16, speaker: u16, selector: u8) -> RedPage {
    RedPage {
        msg,
        kind: PageKind::Top(speaker, selector),
    }
}

/// The red route's map story events (dispatch summaries `story_pages.json`,
/// handlers 0x21F258..0x221584; RED-ROUTE.md §5–6).
pub(crate) const RED_EVENTS: [RedEvent; 16] = [
    RedEvent {
        event: 11120,
        scene: Scene::World,
        moods: &[],
        pages: &[win(2648)],
        next: Next::Stage(11122),
    },
    RedEvent {
        event: 11122,
        scene: Scene::Paintings(SLIDE_11122),
        moods: &[],
        pages: &[still(SLIDE_11122), cap(2652)],
        next: Next::Stage(11124),
    },
    RedEvent {
        event: 11124,
        scene: Scene::World,
        moods: &[],
        pages: &[win(2656)],
        next: Next::Map(map::EVENT_BOSWORTH_REVEAL as u16),
    },
    RedEvent {
        event: 11140,
        scene: Scene::Dialogue {
            bottom: Some(RICHARD_FIGURE),
            top: Some(PEGASUS_FIGURE),
        },
        moods: &[],
        pages: &[
            bot(2688, RICHARD, 1),
            bot(2692, RICHARD, 1),
            bot(2696, RICHARD, 1),
            bot(2697, RICHARD, 1),
            top_voice(2700),
            bot(2704, RICHARD, 1),
            top(2708, PEGASUS, 1),
            bot(2712, RICHARD, 1),
            top(2716, PEGASUS, 1),
            top(2717, PEGASUS, 1),
            top(2720, PEGASUS, 1),
            top(2721, PEGASUS, 1),
            bot(2724, RICHARD, 1),
            top(2728, PEGASUS, 1),
            top(2729, PEGASUS, 1),
            bot(2732, RICHARD, 1),
            top(2736, PEGASUS, 1),
            top(2740, PEGASUS, 1),
            top(2741, PEGASUS, 1),
            top(2744, PEGASUS, 1),
            top(2745, PEGASUS, 1),
            top(2748, PEGASUS, 1),
            bot(2752, RICHARD, 1),
            top(2756, PEGASUS, 1),
            top(2760, PEGASUS, 1),
        ],
        next: Next::Stage(11142),
    },
    RedEvent {
        event: 11142,
        scene: Scene::Paintings(SLIDE_11142),
        moods: &[],
        pages: &[still(SLIDE_11142), cap(2764), cap(2765)],
        next: Next::Stage(11143),
    },
    RedEvent {
        event: 11143,
        scene: Scene::Curtain,
        moods: &[],
        pages: &[cap(2768)],
        next: Next::Stage(11144),
    },
    RedEvent {
        event: 11144,
        scene: Scene::Paintings(PAINTINGS_11144),
        moods: &[],
        pages: &[
            still(PAINTINGS_11144),
            cap(2772),
            cap(2776),
            cap(2780),
            still(AFTER_11144),
        ],
        next: Next::Stage(11146),
    },
    RedEvent {
        event: 11146,
        scene: Scene::Paintings(PAINTINGS_11146),
        moods: &[],
        pages: &[still(PAINTINGS_11146), cap(2784), cap(2785), cap(2788)],
        next: Next::Stage(11147),
    },
    RedEvent {
        event: 11147,
        scene: Scene::Curtain,
        moods: &[],
        pages: &[cap(2792), cap(2793)],
        next: Next::Stage(11148),
    },
    RedEvent {
        event: 11148,
        scene: Scene::Dialogue {
            bottom: Some(YUGI_FIGURE),
            top: Some(PEGASUS_FIGURE),
        },
        moods: &[],
        pages: &[
            bot(2796, YUGI, 0),
            bot(2797, YUGI, 0),
            bot(2800, YUGI, 0),
            bot(2801, YUGI, 0),
            top(2804, PEGASUS, 1),
            bot(2808, YUGI, 0),
            bot(2809, YUGI, 0),
            top(2812, PEGASUS, 1),
            bot(2816, YUGI, 0),
            bot(2820, YUGI, 0),
            bot(2821, YUGI, 0),
            bot(2824, YUGI, 0),
            bot(2825, YUGI, 0),
        ],
        next: Next::Stage(11149),
    },
    RedEvent {
        event: 11149,
        scene: Scene::World,
        moods: &[],
        // 0x232C74: the window shows 2828 (the counter W+5378 holds 2804).
        pages: &[win(2828)],
        next: Next::Map(map::EVENT_STONEHENGE_REVEAL as u16),
    },
    RedEvent {
        event: 11160,
        scene: Scene::Dialogue {
            bottom: Some(SETO_FIGURE),
            top: None,
        },
        moods: &[(2864, 2, 0), (2872, 1, 0)],
        pages: &[
            bot(2864, SETO, 1),
            bot(2868, SETO, 1),
            bot(2872, SETO, 1),
            bot(2876, SETO, 1),
            bot(2880, SETO, 1),
            bot(2881, SETO, 1),
            bot(2884, SETO, 1),
            bot(2888, SETO, 1),
            bot(2892, SETO, 1),
            bot(2896, SETO, 1),
            bot(2900, SETO, 1),
            bot(2901, SETO, 1),
            bot(2904, SETO, 1),
            bot(2905, SETO, 1),
            bot(2908, SETO, 1),
            bot(2909, SETO, 1),
            bot(2912, SETO, 1),
            bot(2916, SETO, 1),
            bot(2917, SETO, 1),
        ],
        next: Next::Stage(11162),
    },
    RedEvent {
        event: 11162,
        scene: Scene::Dialogue {
            bottom: None,
            top: None,
        },
        moods: &[(2920, 0, 0), (2924, 1, 0)],
        pages: &[
            top(2920, MANAWYDDAN, 1),
            bot(2924, SETO, 1),
            bot(2928, SETO, 1),
            bot(2929, SETO, 1),
            top(2932, MANAWYDDAN, 1),
            top(2936, MANAWYDDAN, 1),
        ],
        next: Next::Map(map::EVENT_STONEHENGE_ARRIVE as u16),
    },
    RedEvent {
        event: 11180,
        scene: Scene::Dialogue {
            bottom: None,
            top: None,
        },
        moods: &[(2956, 2, 2)],
        pages: &[
            top(2956, MANAWYDDAN, 1),
            bot(2960, SETO, 1),
            top(2964, MANAWYDDAN, 1),
        ],
        next: Next::Stage(11182),
    },
    RedEvent {
        event: 11182,
        scene: Scene::Paintings(MOSAIC_11182),
        moods: &[],
        pages: &[
            still(MOSAIC_11182),
            cap(2968),
            cap(2972),
            cap(2976),
            cap(2977),
            cap(2980),
            still(AFTER_11182),
        ],
        next: Next::Stage(11184),
    },
    RedEvent {
        event: 11184,
        scene: Scene::World,
        moods: &[],
        pages: &[
            cap(2984),
            cap(2988),
            cap(2992),
            cap(2993),
            cap(2996),
            cap(2997),
            cap(3000),
            cap(3001),
            cap(3004),
            cap(3008),
            cap(3012),
            cap(3013),
            cap(3016),
            cap(3017),
            cap(3020),
            cap(3021),
            cap(3024),
            cap(3028),
        ],
        next: Next::Ending,
    },
];
/// Pages of the red route whose text embeds the player name (message 3
/// slots; the white route's are `import_story_white::NAME_MESSAGES`).
pub(crate) const NAME_MESSAGES: [u16; 2] = [2796, 3000];

/// Every map story event this import writes: the red route's, then the
/// white route's (`import_story_white`).
pub(crate) fn all_events() -> impl Iterator<Item = &'static RedEvent> {
    RED_EVENTS
        .iter()
        .chain(crate::import_story_white::WHITE_EVENTS.iter())
}
pub(crate) const ENDING_NOTE: &str = "After caption 3028 the stage continues at map event 11184's tail (StoryEnd::MapEvent 11184): letterbox closes, SAVE+0x3C = 0, SAVE[1] = 1, memory-card save op 2, movie MOVIE/EDMV_A6.PSS (asset ending-movie-red, played by the built-in PSS decoder, not skippable), then the title (roses_client::ending). The white route ends the same way after caption 3508 (StoryEnd::MapEvent 12156): SAVE+0x3C = 0, SAVE[2] = 1 (card block bit 5 when SAVE[1] is set too), op 2, movie MOVIE/EDMV_C6.PSS (asset ending-movie-white, not skippable), C2 = 0, title. The free-duel map after both routes is not ported.";
pub(crate) const CAPTION_NOTE: &str = "Captions (layout 1) are drawn at raw (89, 384). They lie over their settled TEXEVE paintings, the closed letterbox, or (11184) the 144-row letterbox over the undrawn scene-0 world.";
pub(crate) const MAP_NOTE: &str = "11120, 11124 and 11149 show the centred window over the campaign map behind the 144-row letterbox; these story pages draw black behind it; the connected game plays the three windows in the campaign map scene over the live map instead (roses_client::campaign_map story_window). The scene-0 world behind the Stonehenge dialogue is drawn by the story scene when the game import has it (black otherwise).";
/// Letterbox band heights (raw rows each side, 0x233750 = W+1E + W+20 + W+22).
const LETTERBOX_STILL: u32 = 72;
const LETTERBOX: u32 = 144;
const LETTERBOX_CLOSED: u32 = 256;
/// Window origin y and text offset of 0x232C80 (raw).
const WINDOW_Y: u32 = 256;

pub(crate) fn stage_id(event: u16) -> String {
    format!("map-story-{event}")
}

fn speaker_name(speaker: u16) -> &'static str {
    match speaker {
        SETO => "seto",
        PEGASUS => "pegasus",
        RICHARD => "richard",
        YUGI => "yugi",
        MANAWYDDAN => "manawyddan",
        MAKO => "mako",
        _ => "unknown",
    }
}

/// The bottom speaker window for `speaker`: the imported Simon panel with
/// the speaker's portrait, bar colour, emblem/ornament selector and name.
fn bottom_panel(
    simon_panel: &[Layer],
    speaker: u16,
    selector: u8,
    name_image: &roses_assets::RgbaImage,
) -> Result<Vec<Layer>> {
    let name = speaker_name(speaker);
    let mut layers = Vec::new();
    let mut replaced = 0;
    for layer in simon_panel {
        let mut l = layer.clone();
        match layer.asset.as_str() {
            "simon-small-portrait" => {
                l.asset = format!("{name}-small-portrait");
                replaced += 1;
            }
            "simon-label-bar" => {
                l.asset = if selector == 0 {
                    "simon-label-bar".into()
                } else {
                    "seto-label-bar".into()
                };
                replaced += 1;
            }
            "simon-speaker" => {
                let origin = [layer.destination[0], layer.destination[1]];
                l = text_layer(&format!("{name}-speaker"), name_image, origin);
                replaced += 1;
            }
            "texsys-07" if layer.source == Some([224, 0, 32, 32]) => {
                l.source = Some([224, 32 * u32::from(selector), 32, 32]);
            }
            "texsys-07" if layer.source == Some([0, 0, 64, 23]) && selector != 0 => {
                l.source = Some([0, 23, 64, 21]);
                l.destination[3] = (21 * 7 + 4) / 8;
            }
            _ => {}
        }
        layers.push(l);
    }
    if replaced != 3 {
        return Err("the Simon panel no longer holds its portrait, bar and name layers".into());
    }
    Ok(layers)
}

/// Raw 512x512 y to the 640x448 canvas (edges, so adjacent quads meet).
fn canvas_y(raw: i32) -> u32 {
    ((raw.clamp(0, 512) * 7 + 4) / 8) as u32
}

/// The visible part of a painting quad: the destination clipped to the
/// screen and the source rows cut in proportion (rounded to whole texels).
pub(crate) fn painting_layer(p: &Painting) -> Option<Layer> {
    // The same cut the story scene's motion uses while the paintings move.
    roses_client::story_motion::raw_quad_layer(&format!("texeve-{:02}", p.record), p.uv, p.raw)
}

/// The two black bands of the letterbox, `rows` raw rows each.
pub(crate) fn letterbox(rows: u32) -> Vec<Layer> {
    let rows = rows.min(256) as i32;
    let top = canvas_y(rows);
    let bottom = canvas_y(512 - rows);
    vec![
        story_layer("story-black", None, [0, 0, 640, top]),
        story_layer("story-black", None, [0, bottom, 640, 448 - bottom]),
    ]
}

fn figure_layer(f: &Figure) -> Layer {
    story_layer(
        &format!("texeve-{:02}", f.record),
        Some([0, 0, f.uv[0], f.uv[1]]),
        raw_rect(f.raw),
    )
}

/// Raw rectangle of the centred window for a text image (0x232CE4..0x232D38):
/// width = text width + 12 (clamped to 480 at x = 16), height + 12, y = 256.
pub(crate) fn window_rect(text: &roses_assets::RgbaImage) -> [u32; 4] {
    let w = text.width + 12;
    let h = text.height + 12;
    if w > 480 {
        [16, WINDOW_Y, 480, h]
    } else {
        [256 - w / 2, WINDOW_Y, w, h]
    }
}

/// Window style 0 through 0x218E70: black fill (GS alpha 64) inset by 2 and
/// the eight TEXSYS 7 border pieces at (64, 0) 16x14.
pub(crate) fn window_layers(rect: [u32; 4]) -> Vec<Layer> {
    let [x, y, w, h] = rect;
    let mut fill = story_layer("story-black", None, raw_rect([x + 2, y + 2, w - 4, h - 4]));
    fill.opacity = 128;
    let mut layers = vec![fill];
    layers.extend(frame_layers(rect, [64, 0, 16, 14]));
    layers
}

/// Paintings over a black base (0x22EEEC/0x22EDB4 fill the screen first).
fn backdrop(paintings: &[Painting]) -> Vec<Layer> {
    let mut layers = vec![story_layer("story-black", None, [0, 0, 640, 448])];
    layers.extend(paintings.iter().filter_map(painting_layer));
    layers
}

/// Which speaker panels are on (0x53B7 bottom, 0x53B6 top): each turns on
/// at its speaker's first page and stays on for the rest of the event [emu].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Latch {
    pub bottom: Option<(u16, u8)>,
    pub top: Option<(u16, u8)>,
}
impl Latch {
    pub(crate) fn see(&mut self, kind: PageKind) {
        match kind {
            PageKind::Bottom(s, c) => self.bottom = Some((s, c)),
            PageKind::Top(s, c) => self.top = Some((s, c)),
            _ => {}
        }
    }
}

struct Panels<'a> {
    simon_panel: &'a [Layer],
    name_images: &'a BTreeMap<u16, roses_assets::RgbaImage>,
    faces: &'a FaceTables,
}
impl Panels<'_> {
    fn bottom(&self, speaker: u16, selector: u8, mood: u8) -> Result<Vec<Layer>> {
        let name_image = self
            .name_images
            .get(&speaker)
            .ok_or("speaker name image missing")?;
        let mut layers = bottom_panel(self.simon_panel, speaker, selector, name_image)?;
        story_faces::rewrite(&mut layers, self.faces, [mood, 0])?;
        Ok(layers)
    }
    fn top(&self, speaker: u16, selector: u8, mood: u8) -> Result<Vec<Layer>> {
        let name_image = self
            .name_images
            .get(&speaker)
            .ok_or("speaker name image missing")?;
        let name = speaker_name(speaker);
        let bar = if selector == 0 {
            "simon-label-bar"
        } else {
            "seto-label-bar"
        };
        let mut layers = top_panel(
            selector,
            &format!("{name}-small-portrait"),
            bar,
            &format!("{name}-speaker"),
            name_image,
        );
        story_faces::rewrite(&mut layers, self.faces, [0, mood])?;
        Ok(layers)
    }
}

fn kind_name(kind: PageKind) -> String {
    match kind {
        PageKind::Still(_) => "Still".into(),
        other => format!("{other:?}"),
    }
}

/// The moods (bottom, top) in force at page `msg` of `event`.
pub(crate) fn moods_at(event: &RedEvent, msg: u16) -> (u8, u8) {
    let mut at = (0, 0);
    for page in event.pages {
        if let Some(&(_, b, t)) = event.moods.iter().find(|m| m.0 == page.msg) {
            at = (b, t);
        }
        if page.msg == msg {
            break;
        }
    }
    at
}

/// TEXEVE records (and their traced sizes) the pages draw.
pub(crate) fn used_textures() -> BTreeMap<u16, [u32; 2]> {
    let mut used = BTreeMap::new();
    for event in all_events() {
        let mut paintings: Vec<Painting> = Vec::new();
        match event.scene {
            Scene::Paintings(p) | Scene::PaintedDialogue(p) => paintings.extend_from_slice(p),
            Scene::Dialogue { bottom, top } => {
                for f in [bottom, top].into_iter().flatten() {
                    used.insert(f.record, f.uv);
                }
            }
            _ => {}
        }
        for page in event.pages {
            if let PageKind::Still(p) = page.kind {
                paintings.extend_from_slice(p);
            }
        }
        for q in paintings {
            used.insert(q.record, q.uv);
        }
    }
    used
}

/// The shared layer set behind one page: its name and layers.
fn page_set(
    event: &RedEvent,
    kind: PageKind,
    latch: Latch,
    moods: (u8, u8),
    still_index: usize,
    panels: &Panels,
) -> Result<(String, Vec<Layer>)> {
    let e = event.event;
    Ok(match (event.scene, kind) {
        (_, PageKind::Still(paintings)) => {
            let mut layers = backdrop(paintings);
            layers.extend(letterbox(LETTERBOX_STILL));
            (format!("map-story-{e}-painting-{still_index}"), layers)
        }
        (Scene::World, PageKind::Window | PageKind::Caption) => {
            ("map-story-world-letterbox".into(), letterbox(LETTERBOX))
        }
        (Scene::Curtain, PageKind::Caption) => (
            "map-story-closed-letterbox".into(),
            letterbox(LETTERBOX_CLOSED),
        ),
        (Scene::Paintings(paintings), PageKind::Caption) => {
            let mut layers = backdrop(paintings);
            layers.extend(letterbox(LETTERBOX));
            (format!("map-story-{e}-caption"), layers)
        }
        (
            Scene::Dialogue { .. } | Scene::PaintedDialogue(_),
            PageKind::Bottom(..) | PageKind::Top(..) | PageKind::TopVoice,
        ) => {
            let (bottom, top, paintings) = match event.scene {
                Scene::Dialogue { bottom, top } => (bottom, top, None),
                Scene::PaintedDialogue(p) => (None, None, Some(p)),
                _ => unreachable!(),
            };
            let figure = match kind {
                PageKind::Bottom(..) => bottom,
                PageKind::Top(..) => top,
                _ => None,
            };
            let mut layers: Vec<Layer> = match paintings {
                Some(p) => backdrop(p),
                None => figure.iter().map(figure_layer).collect(),
            };
            layers.extend(letterbox(LETTERBOX));
            let mut name = match (paintings, figure) {
                (Some(_), _) => format!("map-story-{e}-paintings"),
                (None, Some(f)) => format!("map-story-{e}-figure-{}", f.record),
                (None, None) => format!("map-story-{e}-no-figure"),
            };
            if let Some((s, c)) = latch.bottom {
                layers.extend(panels.bottom(s, c, moods.0)?);
                name += &format!("-bottom-{}-{}", speaker_name(s), moods.0);
            }
            if let Some((s, c)) = latch.top {
                layers.extend(panels.top(s, c, moods.1)?);
                name += &format!("-top-{}-{}", speaker_name(s), moods.1);
            }
            (name, layers)
        }
        _ => return Err(format!("event {e}: page {kind:?} does not fit its scene").into()),
    })
}

/// Simon's bottom mood on 12000's page 3044 [emu trace-start].
const SIMON_3044_MOOD: u8 = 2;

/// The (not yet face-rewritten) layer set of 12000's page 3044.
fn simon_3044_set(manifest: &roses_client::Manifest) -> Option<(String, Vec<Layer>)> {
    manifest.stages.iter().find_map(|stage| match stage {
        Stage::Story { id, pages, .. } if id == crate::import_story_map::WHITE_STAGE => pages
            .iter()
            .find(|p| p.id == "message-3044")
            .and_then(|p| p.layer_set.clone())
            .and_then(|set| manifest.layer_sets.get(&set).map(|l| (set, l.clone()))),
        _ => None,
    })
}

pub fn run(iso: &Path, story: &Path, output: &Path) -> Result<()> {
    if output.exists() {
        return Err(
            "story-red output must be a new directory; imports are never overwritten".into(),
        );
    }
    let loaded = Loaded::open(story)?;
    let (mut disc, report) = open_pal(iso)?;
    if loaded.manifest.source.disc_sha256 != report.iso_sha256
        || Some(&loaded.manifest.source.executable_sha256) != report.executable_sha256.as_ref()
    {
        return Err("story import does not match the supplied ISO".into());
    }
    let mut manifest = loaded.manifest.clone();
    // Older imports end in the map placeholder stage; current ones end
    // 11020 at map event 101 (the voyage).
    let reaches_map = manifest.stages.iter().any(|s| {
        s.id() == "map-events-101-102"
            || matches!(s, Stage::Story { id, on_end: StoryEnd::MapEvent { .. }, .. } if id == "yugi-at-brest")
    });
    if !reaches_map {
        return Err("import does not reach the campaign map (run import-story-map first)".into());
    }
    if manifest.stages.iter().any(|s| s.id() == stage_id(11120)) {
        return Err("import already holds the red route story events".into());
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(output)?;
    for asset in &manifest.assets {
        let destination = output.join(&asset.path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let source = File::open(&loaded.assets[&asset.id].path)?;
        let mut target = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        if std::io::copy(&mut source.take(asset.bytes + 1), &mut target)? != asset.bytes {
            return Err("source asset changed size while copying the import".into());
        }
        target.sync_all()?;
    }
    let bytes = disc.read_file("MSG/MSG_E.DAT", 300_000)?;
    let texanm = disc.read_file(
        "DATA/TEXANM.MRG",
        roses_assets::MAX_OPENING_ARCHIVE_BYTES as u64,
    )?;

    let page_ids: Vec<u16> = all_events()
        .flat_map(|e| e.pages.iter().map(|p| p.msg))
        .filter(|&id| id != 0)
        .collect();
    let mut unique = page_ids.clone();
    unique.sort_unstable();
    unique.dedup();
    let templates = parse_pal_message_templates(&bytes, &unique)?;
    let name_ids: Vec<u16> = templates
        .messages
        .iter()
        .filter(|m| {
            m.tokens
                .iter()
                .any(|t| matches!(t, MessageTemplateToken::PlayerNameSlot { .. }))
        })
        .map(|m| m.message_id)
        .collect();
    let mut expected_names = NAME_MESSAGES.to_vec();
    expected_names.extend(crate::import_story_white::NAME_MESSAGES);
    expected_names.sort_unstable();
    if name_ids != expected_names {
        return Err(format!("unexpected player-name pages {name_ids:?}").into());
    }
    let mut static_ids: Vec<u16> = unique
        .iter()
        .copied()
        .filter(|id| !expected_names.contains(id))
        .collect();
    static_ids.extend([PEGASUS, RICHARD, MANAWYDDAN, MAKO]);
    let messages = parse_pal_messages(&bytes, &static_ids)?;
    let message = |id: u16| {
        messages
            .messages
            .iter()
            .find(|m| m.message_id == id)
            .ok_or("story message missing")
    };
    let font = decode_pal_message_font(&bytes)?;
    let (common, text_origin, _) = simon_common(&manifest)?;
    if common.len() < 3 || common[1].asset != "story-black" {
        return Err("Simon's settled layers no longer start with figure and mask".into());
    }
    let simon_panel = common[2..].to_vec();
    let cells = name_cells(&manifest, &font)?;
    let image = |id: u16| message_image(&font, message(id)?);

    // Portraits and names of the speakers the opening did not import.
    for (speaker, record) in [
        (PEGASUS, 10usize),
        (RICHARD, 11),
        (MANAWYDDAN, 21),
        (MAKO, 15),
    ] {
        let portrait =
            roses_assets::decode_pal_opening_texture(&texanm, OpeningArchiveKind::TexAnm, record)?;
        import_png(
            output,
            &mut manifest,
            &format!("{}-small-portrait", speaker_name(speaker)),
            &portrait,
        )?;
        let name = image(speaker)?;
        import_png(
            output,
            &mut manifest,
            &format!("{}-speaker", speaker_name(speaker)),
            &name,
        )?;
    }
    if !manifest.assets.iter().any(|a| a.id == "seto-label-bar") {
        import_png(output, &mut manifest, "seto-label-bar", &label_bar(1))?;
    }
    let name_images: BTreeMap<u16, roses_assets::RgbaImage> = {
        let mut m = BTreeMap::new();
        for speaker in [SETO, PEGASUS, RICHARD, YUGI, MANAWYDDAN, MAKO] {
            // The opening imported Seto's and Yugi's name images already.
            let id = format!("{}-speaker", speaker_name(speaker));
            if manifest.assets.iter().any(|a| a.id == id) {
                m.insert(
                    speaker,
                    image(speaker).or_else(|_| {
                        // Seto/Yugi name messages were not selected: render them now.
                        let one = parse_pal_messages(&bytes, &[speaker])?;
                        message_image(&font, &one.messages[0])
                    })?,
                );
            }
        }
        m
    };

    // The TEXEVE records the Bosworth pages draw must have the traced sizes
    // (the UV extents of 0x22E8B0/0x2306A0 are the whole textures).
    let texeve = disc.read_file(
        "DATA/TEXEVE.MRG",
        roses_assets::MAX_OPENING_ARCHIVE_BYTES as u64,
    )?;
    for (record, uv) in used_textures() {
        let id = format!("texeve-{record:02}");
        if !manifest.assets.iter().any(|a| a.id == id) {
            return Err(format!("story import lacks the opening texture {id}").into());
        }
        let image = roses_assets::decode_pal_opening_texture(
            &texeve,
            OpeningArchiveKind::TexEve,
            usize::from(record),
        )?;
        if [image.width, image.height] != uv {
            return Err(format!("{id} is not the traced {}x{} texture", uv[0], uv[1]).into());
        }
    }
    let executable = disc.read_file("SLES_515.79", 8 * 1024 * 1024)?;
    let faces = FaceTables::from_executable(&executable)?;
    let panels = Panels {
        simon_panel: &simon_panel,
        name_images: &name_images,
        faces: &faces,
    };

    let mut sets: BTreeMap<String, Vec<Layer>> = BTreeMap::new();
    let mut records = Vec::new();
    let mut stages = Vec::new();
    for event in all_events() {
        let mut pages = Vec::new();
        let mut latch = Latch::default();
        let mut stills = 0;
        for page in event.pages {
            latch.see(page.kind);
            let moods = moods_at(event, page.msg);
            let (set, layers) = page_set(event, page.kind, latch, moods, stills, &panels)?;
            sets.entry(set.clone()).or_insert(layers);
            let origin = match page.kind {
                PageKind::Bottom(..) => text_origin,
                PageKind::Top(..) | PageKind::TopVoice => raw_origin(16, 48),
                _ => raw_origin(89, 384),
            };
            let (layers, name_text) = if page.msg == 0 {
                (vec![], None)
            } else if expected_names.contains(&page.msg) {
                let template = templates
                    .messages
                    .iter()
                    .find(|m| m.message_id == page.msg)
                    .ok_or("name template missing")?;
                (
                    vec![],
                    Some(name_text_for(template, &font, origin, &cells)?),
                )
            } else {
                let text = image(page.msg)?;
                let asset = format!("story-text-{}", page.msg);
                if !manifest.assets.iter().any(|a| a.id == asset) {
                    import_png(output, &mut manifest, &asset, &text)?;
                }
                if page.kind == PageKind::Window {
                    let rect = window_rect(&text);
                    let mut layers = window_layers(rect);
                    layers.push(text_layer(
                        &asset,
                        &text,
                        raw_origin(rect[0] + 6, WINDOW_Y + 6),
                    ));
                    (layers, None)
                } else {
                    (vec![text_layer(&asset, &text, origin)], None)
                }
            };
            let pan = match page.kind {
                PageKind::Bottom(..) => 100,
                PageKind::Top(..) => -100,
                _ => 0,
            };
            let id = if page.msg == 0 {
                stills += 1;
                format!("map-story-{}-painting-{}", event.event, stills - 1)
            } else {
                format!("map-story-{}-message-{}", event.event, page.msg)
            };
            records.push(
                serde_json::json!({"stage": stage_id(event.event), "event": event.event,
                "message_id": (page.msg != 0).then_some(page.msg), "kind": kind_name(page.kind),
                "layer_set": set, "runtime_name": name_text.is_some()}),
            );
            pages.push(StoryPage {
                id,
                layer_set: Some(set),
                layers,
                name_text,
                pan: Some(pan),
            });
        }
        let on_end = match event.next {
            Next::Stage(next) => StoryEnd::Stage {
                target: stage_id(next),
            },
            Next::Map(map_event) => StoryEnd::MapEvent { event: map_event },
            // The route's last map event: its tail is the ending
            // (11184 red, 12156 white; `roses_client::ending`).
            Next::Ending => StoryEnd::MapEvent { event: event.event },
        };
        stages.push(Stage::Story {
            id: stage_id(event.event),
            pages,
            music: None,
            advance_buttons: vec![Button::Cross],
            on_end,
        });
    }
    // 12000 (seto-yorkists, import-story-map): Simon's page 3044 shows his
    // bottom mood 2 (W+546B = 2, expression 1) [emu the project notes
    // trace-start]; its panel set is copied before the mood-0 pass.
    let simon_3044 = simon_3044_set(&manifest);
    // Every earlier story panel (opening, Simon, Seto, Yugi): face at mood 0
    // (before the red/white sets join: their faces carry their own moods).
    let earlier_faces = story_faces::apply(&mut manifest, &faces)?;
    if let Some((set, mut layers)) = simon_3044 {
        story_faces::rewrite(&mut layers, &faces, [SIMON_3044_MOOD, 0])?;
        let name = format!("{set}-simon-mood-{SIMON_3044_MOOD}");
        for stage in &mut manifest.stages {
            if let Stage::Story { id, pages, .. } = stage {
                if id == crate::import_story_map::WHITE_STAGE {
                    for page in pages.iter_mut().filter(|p| p.id == "message-3044") {
                        page.layer_set = Some(name.clone());
                    }
                }
            }
        }
        manifest.layer_sets.insert(name, layers);
    }
    manifest.layer_sets.extend(sets);
    manifest.stages.extend(stages);
    // The two ending movies, kept as the disc's PSS for the built-in decoder
    // (the red one plays at the end of 11184; the white route is not ported
    // yet, its movie is copied so a later import need not change).
    let mut ending_movies = Vec::new();
    for (movie, expected) in roses_client::ending::ENDING_MOVIES.iter().zip([
        &crate::import_opening::ENDING_RED_MOVIE,
        &crate::import_opening::ENDING_WHITE_MOVIE,
    ]) {
        debug_assert_eq!(movie.disc_path, expected.disc_path);
        let (asset, check) = crate::import_opening::import_disc_movie(
            &mut disc,
            expected,
            output,
            movie.asset,
            movie.media_path,
        )?;
        ending_movies.push(serde_json::json!({
            "asset": movie.asset, "disc_path": movie.disc_path, "index": movie.index,
            "skippable": movie.skippable(), "sha256": asset.sha256, "bytes": asset.bytes,
            "native_movie_check": check,
        }));
        manifest.assets.push(asset);
    }

    emit_json(&report, Some(&output.join("disc.json")))?;
    emit_json(
        &serde_json::json!({
            "schema_version": "roses-story-red-extension-1",
            "story_manifest_sha256": loaded.manifest_sha256,
            "message_sha256": messages.message_sha256,
            "pages": records,
            "runtime_name_message_ids": expected_names,
            "earlier_panel_faces_at_mood_0": earlier_faces,
            "map_continuations": {"11124": map::EVENT_BOSWORTH_REVEAL, "11149": map::EVENT_STONEHENGE_REVEAL, "11162": map::EVENT_STONEHENGE_ARRIVE,
                "12040": crate::import_story_white::EVENT_DOVER_VOYAGE, "12041": crate::import_story_white::EVENT_STRAIT_TAIL, "12132": 0},
            "white_route": crate::import_story_white::REPORT_NOTES,
            "ending_movies": ending_movies,
            "boundaries": [ENDING_NOTE, CAPTION_NOTE, MAP_NOTE],
            "limitations": [
                "Pages are settled states: letterbox motion, text reveal, page fades, background slides (W+53B2), the 11144 cross-fade (W+54A6), figure and panel slides are not reproduced; painting-only Cross waits are pages of their own.",
                "Drawn: TEXEVE paintings 2-12 (11182 = 2x2 mosaic of 9-12), figures 20/19, 22/19 and 18 (11162 and 11180 draw none), letterbox, latched speaker panels with frame-0 faces at the traced moods. Not drawn: the 3D worlds behind 11120/11124/11149 (campaign map) and 11160/11162/11180/11184 (MODEL.MRG pack 33, 'scene 0'), the C-module effects 401/402 and the overlay W+53A4 30-33 / W+54A4 4 (no 2D output in the traced frames).",
                "Panels of the earlier story stages get faces at mood 0 (their moods are not traced).",
                "Music 28-34 of these events is cued by the game at stage entry (opening::stage_music), not by this manifest.",
                "Event 11124's camera pan to Bosworth and events 11162's and 11184's effects happen in the map model or are boundaries."]
        }),
        Some(&output.join("import-report.json")),
    )?;
    let pending = output.join("manifest.pending.json");
    emit_json(&manifest, Some(&pending))?;
    Loaded::open(&pending)?;
    emit_json(&manifest, Some(&output.join("manifest.json")))?;
    eprintln!(
        "Red route story events imported: {}",
        output.join("manifest.json").display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The story scene's motion (`roses_client::story_motion`) ends every
    /// slide on the settled paintings this import draws.
    #[test]
    fn motion_slides_end_on_the_settled_paintings() {
        use roses_client::story_motion::{event_motion, StillMotion};
        let at = |event: u16, still: usize| {
            let spec = event_motion(event).unwrap();
            let StillMotion::Slide { to, .. } = spec.stills[still] else {
                panic!("{event} still {still} does not slide");
            };
            spec.quads
                .iter()
                .filter_map(|q| {
                    painting_layer(&painting(
                        q.record,
                        q.uv,
                        [to[0] + q.offset[0], to[1] + q.offset[1], q.size[0], q.size[1]],
                    ))
                })
                .collect::<Vec<_>>()
        };
        let settled = |p: &[Painting]| p.iter().filter_map(painting_layer).collect::<Vec<_>>();
        let eq = |a: Vec<Layer>, b: Vec<Layer>| {
            let key = |v: Vec<Layer>| v.into_iter().map(|l| (l.asset, l.source, l.destination)).collect::<Vec<_>>();
            assert_eq!(key(a), key(b));
        };
        eq(at(11122, 0), settled(SLIDE_11122));
        eq(at(11142, 0), settled(SLIDE_11142));
        eq(at(11182, 0), settled(MOSAIC_11182));
        eq(at(11182, 1), settled(AFTER_11182));
        eq(at(12154, 0), settled(crate::import_story_white::MOSAIC_12154));
    }

    fn event(id: u16) -> &'static RedEvent {
        RED_EVENTS.iter().find(|e| e.event == id).unwrap()
    }

    #[test]
    fn paintings_are_clipped_to_the_screen_without_gaps() {
        let [above, below] = [SLIDE_11122[0], SLIDE_11122[1]];
        let a = painting_layer(&above).unwrap();
        assert_eq!(a.asset, "texeve-03");
        assert_eq!(a.source, Some([0, 15, 640, 369]));
        assert_eq!(a.destination, [0, 0, 640, 368]);
        let b = painting_layer(&below).unwrap();
        assert_eq!(b.asset, "texeve-02");
        assert_eq!(b.source, Some([0, 0, 640, 80]));
        assert_eq!(b.destination, [0, 368, 640, 80]);
        let full = painting_layer(&PAINTINGS_11146[0]).unwrap();
        assert_eq!(full.source, Some([0, 0, 640, 448]));
        assert_eq!(full.destination, [0, 0, 640, 448]);
        let slid = painting_layer(&SLIDE_11142[0]).unwrap();
        assert_eq!(slid.destination, [0, 0, 640, 383]);
        assert!(painting_layer(&painting(5, [640, 384], [0, -438, 512, 438])).is_none());
    }

    #[test]
    fn letterbox_bands_match_the_masks() {
        let d = |rows| {
            letterbox(rows)
                .iter()
                .map(|l| l.destination)
                .collect::<Vec<_>>()
        };
        // 144 rows: the bands 0x233750 draws for dialogue (Brest masks too).
        assert_eq!(d(144), [[0, 0, 640, 126], [0, 322, 640, 126]]);
        assert_eq!(d(72), [[0, 0, 640, 63], [0, 385, 640, 63]]);
        assert_eq!(d(256), [[0, 0, 640, 224], [0, 224, 640, 224]]);
    }

    #[test]
    fn map_window_follows_its_text() {
        let text = |width, height| roses_assets::RgbaImage {
            width,
            height,
            pixels: vec![],
        };
        // Traced windows: 2648 (two lines), 2656, 2828 [emu].
        assert_eq!(window_rect(&text(324, 44)), [88, 256, 336, 56]);
        assert_eq!(window_rect(&text(96, 22)), [202, 256, 108, 34]);
        assert_eq!(window_rect(&text(228, 22)), [136, 256, 240, 34]);
        assert_eq!(window_rect(&text(480, 22)), [16, 256, 480, 34]);
        let layers = window_layers([88, 256, 336, 56]);
        assert_eq!(layers.len(), 9);
        assert_eq!(layers[0].opacity, 128);
        assert_eq!(layers[0].destination, raw_rect([90, 258, 332, 52]));
        assert_eq!(layers[1].source, Some([64, 0, 16, 14]));
    }

    #[test]
    fn bosworth_scenes_pages_and_panels() {
        assert!(matches!(event(11143).scene, Scene::Curtain));
        assert!(matches!(event(11147).scene, Scene::Curtain));
        for id in [11120, 11124, 11149] {
            assert!(matches!(event(id).scene, Scene::World));
            assert_eq!(event(id).pages.len(), 1);
            assert_eq!(event(id).pages[0].kind, PageKind::Window);
        }
        assert_eq!(event(11149).pages[0].msg, 2828);
        // Painting-only waits before the captions (and after them in 11144).
        let stills = |id: u16| event(id).pages.iter().filter(|p| p.msg == 0).count();
        assert_eq!(
            [11122, 11142, 11144, 11146, 11182].map(stills),
            [1, 1, 2, 1, 2]
        );
        assert!(matches!(event(11184).scene, Scene::World));
        assert_eq!(moods_at(event(11160), 2868), (2, 0));
        assert_eq!(moods_at(event(11160), 2917), (1, 0));
        assert_eq!(moods_at(event(11140), 2760), (0, 0));
        assert_eq!(
            event(11144).pages.last().unwrap().kind,
            PageKind::Still(AFTER_11144)
        );
        // Panels latch: Richard's alone until Pegasus first speaks (2708).
        let mut latch = Latch::default();
        let mut both = Vec::new();
        for page in event(11140).pages {
            latch.see(page.kind);
            both.push(latch.top.is_some() && latch.bottom.is_some());
        }
        let first = event(11140)
            .pages
            .iter()
            .position(|p| p.msg == 2708)
            .unwrap();
        assert!(both[..first].iter().all(|b| !b) && both[first..].iter().all(|b| *b));
        let used: Vec<(u16, [u32; 2])> = used_textures().into_iter().collect();
        assert_eq!(
            used,
            [
                (2, [640, 384]),
                (3, [640, 384]),
                (4, [640, 384]),
                (6, [640, 448]),
                (7, [640, 448]),
                (8, [640, 448]),
                (9, [512, 384]),
                (10, [512, 384]),
                (11, [512, 384]),
                (12, [512, 384]),
                // The white route's 12154 mosaic.
                (13, [512, 384]),
                (14, [512, 384]),
                (15, [512, 384]),
                (16, [512, 384]),
                (18, [512, 256]),
                (19, [256, 256]),
                (20, [384, 256]),
                (22, [256, 256])
            ]
        );
    }

    /// Private oracle: `ROSES_STORY_RED_ORACLE` = the research directory with
    /// `trace-trigger.json` / `trace-win.json` (tools/trace_story.py). Checks
    /// every constant against the quads the original drawer emitted.
    #[test]
    fn private_constants_match_the_traced_quads() {
        let Some(dir) = std::env::var_os("ROSES_STORY_RED_ORACLE") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let load = |name: &str| -> serde_json::Value {
            serde_json::from_slice(&std::fs::read(dir.join(name)).unwrap()).unwrap()
        };
        let traces = [
            load("trace-trigger.json"),
            load("trace-win3.json"),
            load("trace-seto.json"),
            load("trace-manawyddan.json"),
        ];
        let rows: Vec<&serde_json::Value> = traces
            .iter()
            .flat_map(|t| t["rows"].as_array().unwrap())
            .collect();
        let int = |v: &serde_json::Value| v.as_i64().unwrap() as i32;
        // Texture slots bound by 0x229570 per event [F].
        let slot_record = |event: u16, slot: i32| -> u16 {
            match (event, slot) {
                (11122, 2 | 3) => 3,
                (11122, _) => 2,
                (11142, 2 | 3) => 5,
                (11142, _) => 4,
                (11144, 2 | 4) => 6,
                (11144, _) => 7,
                (11146, _) => 8,
                (11140 | 11148, 7) => 19,
                (11140, 6) => 20,
                (11148, 6) => 22,
                (11160 | 11162 | 11180, 6) => 18,
                (11160 | 11162 | 11180, 7) => 23,
                (11182, 2) => 10,
                (11182, 3) => 12,
                (11182, 4) => 9,
                (11182, 5) => 11,
                _ => panic!("no slot {slot} in {event}"),
            }
        };
        let quads = |row: &serde_json::Value, slots: std::ops::RangeInclusive<i32>| {
            let event = int(&row["C2"]) as u16;
            row["calls"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["fn"] == "quad" && slots.contains(&int(&c["slot"])))
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
        let find = |event: u16, pred: &dyn Fn(&serde_json::Value) -> bool| {
            *rows
                .iter()
                .find(|r| int(&r["C2"]) == i32::from(event) && pred(r))
                .unwrap_or_else(|| panic!("no traced frame for {event}"))
        };
        let expect = |p: &[Painting]| {
            p.iter()
                .map(|q| (q.record, q.uv, q.raw))
                .collect::<Vec<_>>()
        };
        for e in &RED_EVENTS {
            let mut stills = e.pages.iter().filter_map(|p| match p.kind {
                PageKind::Still(p) => Some(p),
                _ => None,
            });
            match e.scene {
                Scene::Paintings(p) => {
                    let row = find(e.event, &|r| r["ae"] == 3 && r["disp"] == 4);
                    assert_eq!(quads(row, 2..=5), expect(p), "{} caption", e.event);
                    assert_eq!(
                        row["lb"][2].as_i64().unwrap() + row["lb"][3].as_i64().unwrap(),
                        144
                    );
                    let first = find(e.event, &|r| r["ae"] == 2);
                    assert_eq!(quads(first, 2..=5), expect(stills.next().unwrap()));
                    assert_eq!(
                        first["lb"][2].as_i64().unwrap() + first["lb"][3].as_i64().unwrap(),
                        72
                    );
                    if let Some(after) = stills.next() {
                        let last = rows
                            .iter()
                            .filter(|r| int(&r["C2"]) == i32::from(e.event) && r["ae"] == 2)
                            .last()
                            .unwrap();
                        assert_eq!(quads(last, 2..=5), expect(after));
                    }
                }
                Scene::Dialogue { bottom, top } => {
                    let fig = |f: Option<Figure>| {
                        f.map(|f| (f.record, f.uv, f.raw.map(|v| v as i32)))
                            .into_iter()
                            .collect::<Vec<_>>()
                    };
                    // Every page: figure, pan W+5439, top-panel latch 0x53B6, moods.
                    let mut latch = Latch::default();
                    for p in e.pages {
                        latch.see(p.kind);
                        let row = find(e.event, &|r| r["msg"] == p.msg && r["disp"] == 4);
                        let (pan, figure) = match p.kind {
                            PageKind::Bottom(..) => (100, bottom),
                            PageKind::Top(..) => (-100, top),
                            _ => (0, None),
                        };
                        assert_eq!(row["f5439"], pan, "{} {}", e.event, p.msg);
                        assert_eq!(row["b6"] == 1, latch.top.is_some(), "{}", p.msg);
                        let got = quads(row, 6..=7);
                        if row["w5483"] == 0 {
                            assert_eq!(got, fig(figure), "{}", p.msg);
                        } else {
                            // W+5483 != 0 (11160 from 2912): the figure shakes
                            // a few pixels around its rest position [emu].
                            let want = fig(figure);
                            assert_eq!(got.len(), want.len());
                            for (g, w) in got.iter().zip(&want) {
                                assert!((0..4).all(|i| (g.2[i] - w.2[i]).abs() <= 4), "{}", p.msg);
                            }
                        }
                        assert_eq!(
                            row["lb"][2].as_i64().unwrap() + row["lb"][3].as_i64().unwrap(),
                            144
                        );
                        if let Some(m) = row.get("mood") {
                            let (b, t) = moods_at(e, p.msg);
                            assert_eq!(m, &serde_json::json!([b, t]), "mood {}", p.msg);
                        }
                    }
                }
                Scene::Curtain => {
                    let row = find(e.event, &|r| r["disp"] == 4);
                    assert_eq!(row["lb"][4], 112);
                    assert!(quads(row, 2..=7).is_empty());
                }
                Scene::World if e.pages[0].kind == PageKind::Caption => {
                    for p in e.pages {
                        let row = find(e.event, &|r| r["msg"] == p.msg && r["disp"] == 4);
                        assert_eq!(
                            row["lb"][2].as_i64().unwrap() + row["lb"][3].as_i64().unwrap(),
                            144
                        );
                        assert!(quads(row, 2..=7).is_empty());
                    }
                }
                // White only (import_story_white's oracle test).
                Scene::PaintedDialogue(_) => unreachable!(),
                Scene::World => {
                    let row = find(e.event, &|r| r["disp"] == 4);
                    assert_eq!(row["e5487"], 1);
                    let window = row["calls"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|c| c["fn"] == "window" && c["ra"] == "0x232d68")
                        .unwrap();
                    let x = int(&window["args"][3]) as u32;
                    let w = int(&window["record"][2]) as u32;
                    let h = int(&window["record"][3]) as u32;
                    assert_eq!(int(&window["args"][4]), 256);
                    assert_eq!(x, 256 - w / 2, "{}", e.event);
                    let msg = e.pages[0].msg;
                    // Text sizes of 2648/2656/2828 (12 px glyphs, 22 px lines).
                    let size = match msg {
                        2648 => [324, 44],
                        2656 => [96, 22],
                        2828 => [228, 22],
                        _ => unreachable!(),
                    };
                    assert_eq!([w - 12, h - 12], size);
                }
            }
        }
    }

    #[test]
    fn red_events_chain_to_the_map_continuations() {
        let ids: Vec<u16> = RED_EVENTS.iter().map(|e| e.event).collect();
        assert_eq!(ids[0], 11120);
        assert_eq!(ids.last(), Some(&11184));
        for event in &RED_EVENTS {
            assert!(!event.pages.is_empty());
            if let Next::Stage(next) = event.next {
                assert!(ids.contains(&next), "{} -> {next}", event.event);
            }
        }
        let map_next: Vec<(u16, u16)> = RED_EVENTS
            .iter()
            .filter_map(|e| match e.next {
                Next::Map(m) => Some((e.event, m)),
                _ => None,
            })
            .collect();
        assert_eq!(map_next, [(11124, 11124), (11149, 108), (11162, 112)]);
        // Page counts of RED-ROUTE.md §5–6.
        let count = |event: u16| {
            RED_EVENTS
                .iter()
                .find(|e| e.event == event)
                .unwrap()
                .pages
                .len()
        };
        assert_eq!(
            (
                count(11140),
                count(11148),
                count(11160),
                count(11162),
                count(11184)
            ),
            (25, 13, 19, 6, 18)
        );
        assert!(
            RED_EVENTS
                .iter()
                .flat_map(|e| e.pages)
                .filter(|p| NAME_MESSAGES.contains(&p.msg))
                .count()
                == 2
        );
    }

    #[test]
    fn bottom_panel_swaps_portrait_bar_and_name() {
        let simon = vec![
            story_layer(
                "simon-small-portrait",
                Some([0, 0, 128, 128]),
                [20, 322, 128, 126],
            ),
            story_layer("simon-label-bar", None, [210, 322, 420, 28]),
            story_layer("texsys-07", Some([224, 0, 32, 32]), [170, 322, 40, 28]),
            story_layer("texsys-07", Some([0, 0, 64, 23]), [55, 305, 60, 20]),
            story_layer("simon-speaker", None, [216, 326, 210, 19]),
        ];
        let name = roses_assets::RgbaImage {
            width: 48,
            height: 22,
            pixels: vec![0; 48 * 22 * 4],
        };
        let out = bottom_panel(&simon, RICHARD, 1, &name).unwrap();
        assert_eq!(out[0].asset, "richard-small-portrait");
        assert_eq!(out[1].asset, "seto-label-bar");
        assert_eq!(out[2].source, Some([224, 32, 32, 32]));
        assert_eq!(out[3].source, Some([0, 23, 64, 21]));
        assert_eq!(out[4].asset, "richard-speaker");
        assert_eq!(out[4].destination, [216, 326, 60, 19]);
        let out = bottom_panel(&simon, YUGI, 0, &name).unwrap();
        assert_eq!(out[1].asset, "simon-label-bar");
        assert_eq!(out[3].source, Some([0, 0, 64, 23]));
        assert!(bottom_panel(&simon[1..], YUGI, 0, &name).is_err());
    }
}
