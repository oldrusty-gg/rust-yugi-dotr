//! The duel's result screens and the reward slot machine, drawn as the game
//! draws them with the CPU GS model of `roses_client::gs`: the overlay banner
//! drawer 0x1B0120 / 0x1B1830 (YOU WIN, YOU LOSE, PROMOTION, DEMOTION, GET
//! CARD, GET SPECIAL CARD), the loss darkening 0x1B8DF0 and mode 0xF's draw
//! pass 0x2849A0 (static screen 0x2828C0, reels 0x283910, line flash
//! 0x283340, markers and arrows 0x282040). Bevy-independent; the renderer
//! uploads the frames this module produces.
//!
//! Provenance (private research the project notes,
//! this pass; nothing here is verified against PS2 hardware):
//! - [F] 0x1B1830 (box, plate, portrait/picture, name, icon) and the banner
//!   handlers of 0x1B0120 read from the disassembly; 0x1B8DF0/0x1B8E40 (a
//!   full-screen white quad with ALPHA mode 2 and alpha D3A).
//! - [emu] primitive lists of the game's own drawers run in the R5900
//!   interpreter on saved states: YOU WIN / YOU LOSE at rest and mid-slide,
//!   PROMOTION / DEMOTION, GET CARD / GET SPECIAL CARD, thirteen consecutive
//!   spinning reel frames, the stopped reels of the popup state and screen 2
//!   (the flash) forced by pokes. The reel formulas below reproduce every
//!   one of those lists.
//! - [H] marked where this module goes beyond what was observed.
//!
//! Every texture, glyph and text comes from the user's imported files
//! (TEXSYS.MRG, PICMINI.MRG, MSG_E.DAT, TEXETC.MRG records 3 and 4,
//! PICTURE.MRG and the executable's tables and string table); no game bytes
//! are embedded here.

use roses_client::duel_hud::{
    Descriptor, Element, HUD_Z, HudFrame, HudTables, HudTextures, HudUpdate, PrimKind, Primitive,
    Target, TextureSource, Vertex, render_layer, render_primitives,
};
use roses_client::gs::{self, Blend, Sprite, Target16, Texture, Triangle};
use roses_core::card_block::{AlternateArt, CardBlock, EMPTY_CARD, RECENT_LEN};
use roses_core::duel::RANK_COUNT;
use roses_core::duel_end::{banner, message};
use roses_core::slot_machine::{LINE_ROWS, PrizeKind, PrizePhase, Screen, SlotMachine};
use std::collections::BTreeMap;

/// The GS frame the screens draw into.
pub const FRAME: usize = 512;
/// The message the GET CARD popups draw under the name ("Get!").
pub const MSG_GET: u16 = 154;
/// The banner string table (EE 0x2903B0: 17 pointers to ASCII strings).
const BANNER_STRINGS_EE: u32 = 0x29_03B0;
const BANNER_STRING_COUNT: usize = 17;
const EE_TO_FILE: u32 = 0xF_9680;
/// Descriptor of the first press-icon cel (0x1B1830: 190 + (counter / 5) % 3).
const ICON_DESCRIPTOR: usize = 190;
/// Panel types (descriptor sets `ptype * 9 + 59`) used by the banner.
const BOX_PANEL: usize = 1;
const PLATE_PANEL: usize = 11;
const PORTRAIT_PANEL: usize = 4;
/// Colour set of the box gradient (white, alpha 96: drawn subtractively).
const BOX_COLOUR: usize = 13;
/// Colour set of the prize popups' plate (orange); results and rank banners
/// use the side's set (0 red, 1 teal).
const PRIZE_COLOUR: usize = 2;
const GREY: [u8; 4] = [128, 128, 128, 128];
const WHITE: [u8; 4] = [255, 255, 255, 128];
const CENTRE_X: i32 = 0x100;
/// Box widths passed by the handlers: results and prizes 0x15A, rank
/// banners −0x1CC (negative = squeeze the name to fit).
const RESULT_WIDTH: i32 = 0x15A;
const RANK_WIDTH: i32 = -0x1CC;
/// Minimum inner heights: 0 for results, 0x128 for the popups with a card.
const CARD_MIN_HEIGHT: i32 = 0x128;
/// Box top: results at y + 0x80, popups at y + 0x6C.
const RESULT_TOP: i32 = 0x80;
const POPUP_TOP: i32 = 0x6C;
/// The second text line of the popups (handler: y + 0x156).
const SECOND_LINE_Y: i32 = 0x156;

// ---------------------------------------------------------------- import

/// Schema of `roses import-duel-end` manifests.
pub const DUEL_END_SCHEMA: &str = "roses-duel-end-import-1";
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(serde::Deserialize)]
struct DuelEndManifest {
    schema: String,
    files: Vec<DuelEndFile>,
}

#[derive(serde::Deserialize)]
struct DuelEndFile {
    path: String,
    sha256: String,
}

/// The duel-end import's verbatim disc files, hash-checked.
pub struct DuelEndFiles {
    /// DATA/TEXETC.MRG (records 3 and 4 are the slot machine's textures).
    pub texetc: Vec<u8>,
    /// DATA/PICTURE.MRG (128×128 card pictures, record = picture id).
    pub pictures: Vec<u8>,
}

/// Read a `roses import-duel-end` folder.
pub fn load_duel_end_import(dir: &std::path::Path) -> Result<DuelEndFiles, String> {
    let manifest: DuelEndManifest = serde_json::from_slice(&crate::bounded_read(
        &dir.join("manifest.json"),
        1024 * 1024,
    )?)
    .map_err(|e| format!("{}: {e}", dir.join("manifest.json").display()))?;
    if manifest.schema != DUEL_END_SCHEMA {
        return Err(format!("unsupported duel-end import {}", manifest.schema));
    }
    let read = |name: &str| -> Result<Vec<u8>, String> {
        let entry = manifest
            .files
            .iter()
            .find(|f| f.path == name)
            .ok_or_else(|| format!("duel-end import lacks {name}"))?;
        let bytes = crate::bounded_read(&dir.join(name), MAX_FILE_BYTES)?;
        use sha2::Digest;
        if format!("{:x}", sha2::Sha256::digest(&bytes)) != entry.sha256 {
            return Err(format!("{name} does not match its import hash"));
        }
        Ok(bytes)
    };
    Ok(DuelEndFiles {
        texetc: read("TEXETC.MRG")?,
        pictures: read("PICTURE.MRG")?,
    })
}

// ---------------------------------------------------------------- assets

/// Everything the banner drawer needs from the user's files.
pub struct EndAssets {
    pub tables: HudTables,
    pub textures: HudTextures,
    /// The 17 banner strings (ASCII) of the executable's table.
    pub strings: Vec<String>,
    /// PICTURE.MRG (the aligned 128×128 card pictures).
    pictures: Vec<u8>,
    pub art: Vec<AlternateArt>,
    picture_cache: BTreeMap<u16, Texture>,
}

/// Message ids the end screens compose besides the HUD's own.
pub fn end_message_ids() -> Vec<u16> {
    let mut ids = HudTables::required_messages();
    ids.extend([
        message::PROMOTED,
        message::DEMOTED,
        MSG_GET,
        message::SLOT_MACHINE,
    ]);
    ids
}

/// The banner string table of the PAL executable: 17 pointers at EE
/// 0x2903B0 into NUL-terminated ASCII strings (codes 2 YOU WIN .. 0xD GET
/// SPECIAL CARD). Read from the user's executable, never embedded.
pub fn parse_banner_strings(executable: &[u8]) -> Result<Vec<String>, String> {
    let start = (BANNER_STRINGS_EE - EE_TO_FILE) as usize;
    let table = executable
        .get(start..start + 4 * BANNER_STRING_COUNT)
        .ok_or("banner string table lies outside the executable")?;
    table
        .chunks_exact(4)
        .enumerate()
        .map(|(i, p)| {
            let ee = u32::from_le_bytes(p.try_into().unwrap());
            let at = ee
                .checked_sub(EE_TO_FILE)
                .map(|v| v as usize)
                .filter(|&v| v < executable.len())
                .ok_or_else(|| format!("banner string {i} points outside the executable"))?;
            let end = executable[at..]
                .iter()
                .position(|&b| b == 0)
                .ok_or_else(|| format!("banner string {i} is not terminated"))?;
            let text = &executable[at..at + end];
            if end == 0 || end > 32 || !text.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
                return Err(format!("banner string {i} is not ASCII"));
            }
            Ok(String::from_utf8_lossy(text).into_owned())
        })
        .collect()
}

impl EndAssets {
    /// `executable`, `messages` (MSG_E.DAT), `texsys`, `picmini` are the duel
    /// import's verbatim files; `pictures` is the duel-end import's
    /// PICTURE.MRG. `leaders` are the two sides' deck leaders (portraits).
    pub fn new(
        executable: &[u8],
        messages: &[u8],
        texsys: &[u8],
        picmini: &[u8],
        leaders: [u16; 2],
        pictures: Vec<u8>,
    ) -> Result<Self, String> {
        let mut codes = BTreeMap::new();
        for chunk in end_message_ids().chunks(256) {
            let parsed =
                roses_assets::parse_pal_messages(messages, chunk).map_err(|e| e.to_string())?;
            for m in parsed.messages {
                codes.insert(m.message_id, m.expanded_codes);
            }
        }
        let tables = HudTables::from_executable(executable, codes)?;
        let textures = HudTextures::new(texsys, picmini, leaders, messages)?;
        let art = roses_assets::parse_pal_alternate_art(executable)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|r| AlternateArt {
                card: r.card,
                variants: r.variants,
                pictures: r.pictures,
            })
            .collect();
        if pictures.is_empty() || pictures.len() % roses_assets::PICTURE_RECORD_STRIDE != 0 {
            return Err("PICTURE.MRG is not the aligned picture archive".into());
        }
        Ok(Self {
            tables,
            textures,
            strings: parse_banner_strings(executable)?,
            pictures,
            art,
            picture_cache: BTreeMap::new(),
        })
    }

    /// The banner string of a code (2 YOU WIN .. 0xD GET SPECIAL CARD).
    pub fn banner_string(&self, code: u8) -> Result<&str, String> {
        self.strings
            .get(usize::from(code))
            .map(String::as_str)
            .ok_or_else(|| format!("banner code {code:#x} has no string"))
    }

    /// A 128×128 picture record as the GS samples it (5-bit channels
    /// shifted left, alpha 0x80; the pictures are drawn without blending or
    /// alpha test, so the alpha never matters).
    pub fn picture(&mut self, record: u16) -> Result<&Texture, String> {
        if !self.picture_cache.contains_key(&record) {
            let image = roses_assets::decode_picture(&self.pictures, usize::from(record))
                .map_err(|e| format!("picture {record}: {e}"))?;
            let texels = image
                .pixels
                .chunks_exact(4)
                .map(|p| {
                    // Undo the decoder's v·255/31 expansion to the raw 5-bit
                    // value, then expand as the GS does (v << 3).
                    let raw = |v: u8| ((u32::from(v) * 31 + 127) / 255) as u8;
                    [raw(p[0]) << 3, raw(p[1]) << 3, raw(p[2]) << 3, 0x80]
                })
                .collect();
            self.picture_cache.insert(
                record,
                Texture {
                    width: image.width as usize,
                    height: image.height as usize,
                    texels,
                },
            );
        }
        Ok(&self.picture_cache[&record])
    }

    /// The picture record shown for a card: its own unless the block's art
    /// counter selects an alternate (0x215E60).
    pub fn picture_for(&mut self, card: u16, block: &CardBlock) -> Result<Texture, String> {
        let record = block.art_picture(card, &self.art);
        self.picture(record).cloned()
    }
}

// ---------------------------------------------------------------- banners

/// One overlay banner of 0x1B0120.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerKind {
    /// Codes 2..=6 (YOU WIN, YOU LOSE, 1P WIN, 2P WIN, DRAW): the string, the
    /// portrait of `side` (D54; DRAW passes 2 = no portrait) and the side's
    /// colour set. Only YOU WIN / YOU LOSE were observed (one human).
    Result { code: u8, side: u8 },
    /// PROMOTION (9) / DEMOTION (0xA) of `card` owned by `side`: picture,
    /// name (message 323 + card) and "has been Promoted!/Demoted!".
    Rank { demotion: bool, side: u8, card: u16 },
    /// GET CARD (0xC) / GET SPECIAL CARD (0xD): picture, name and "Get!".
    Prize { special: bool, card: u16 },
}

/// The banner of a two-pad result on one screen: the code and D54. Online (a
/// port addition) a seat has its own screen and nothing calls it 1P or 2P,
/// so it reads YOU WIN / YOU LOSE with its own portrait; a draw, a spectator
/// (`own_side` None) and offline stay as they are.
pub fn seat_result(code: u8, winner: u8, own_side: Option<u8>) -> (u8, u8) {
    use roses_core::duel_end::banner;
    match own_side {
        Some(own) if code == banner::P1_WIN || code == banner::P2_WIN => {
            (if winner == own { banner::YOU_WIN } else { banner::YOU_LOSE }, own)
        }
        _ => (code, winner),
    }
}

/// The primitives of one banner, in the drawer's order. `slide` is c9c
/// (results/rank: the box top is y + 0x200 − 8·c9c; prizes ignore it),
/// `icon_cel` the press icon's cel (0x1B1830: (frame counter / 5) % 3; the
/// original's counter is global and not reproduced [H]), `block` the block
/// whose art counters pick the card picture. The card picture is returned
/// separately: it goes into `HudTextures::render_target`, which the
/// `TextureSource::RenderTarget` sprite in the list samples.
pub fn banner_primitives(
    assets: &mut EndAssets,
    kind: BannerKind,
    slide: i16,
    icon_cel: u8,
    block: &CardBlock,
) -> Result<(Vec<Primitive>, Option<Texture>), String> {
    let y_offset = 0x200 - 8 * i32::from(slide);
    let (code, side, card, name, width, min_height, top, second) = match kind {
        BannerKind::Result { code, side } => (
            code,
            Some(side),
            None,
            None,
            RESULT_WIDTH,
            0,
            y_offset + RESULT_TOP,
            None,
        ),
        BannerKind::Rank {
            demotion,
            side,
            card,
        } => (
            if demotion {
                banner::DEMOTION
            } else {
                banner::PROMOTION
            },
            Some(side),
            Some(card),
            Some(name_message(card)),
            RANK_WIDTH,
            CARD_MIN_HEIGHT,
            y_offset + POPUP_TOP,
            Some(if demotion {
                message::DEMOTED
            } else {
                message::PROMOTED
            }),
        ),
        BannerKind::Prize { special, card } => (
            if special {
                banner::GET_SPECIAL_CARD
            } else {
                banner::GET_CARD
            },
            None,
            Some(card),
            Some(name_message(card)),
            RESULT_WIDTH,
            CARD_MIN_HEIGHT,
            POPUP_TOP,
            Some(MSG_GET),
        ),
    };
    let title = assets.banner_string(code)?.as_bytes().to_vec();
    let colour = match kind {
        BannerKind::Prize { .. } => PRIZE_COLOUR,
        _ => usize::from(side.unwrap_or(0)),
    };
    let portrait = match kind {
        BannerKind::Result { side, .. } if side < 2 => Some(side),
        _ => None,
    };
    let mut b = BannerBuilder {
        t: &assets.tables,
        prims: Vec::new(),
    };
    let picture = b.box_0x1b1830(
        colour, portrait, card, name, top, width, min_height, &title, icon_cel,
    )?;
    if let Some(msg) = second {
        // The rank handlers compose their line into canvas line 2, the prize
        // handler re-uses line 1 (after the name was drawn) [F].
        let line = if matches!(kind, BannerKind::Prize { .. }) {
            1
        } else {
            2
        };
        let w = b.message_width(msg)?;
        b.text_line(
            line,
            msg,
            CENTRE_X - (w >> 1),
            top - POPUP_TOP + SECOND_LINE_Y,
            w,
            1.0,
        );
    }
    let prims = b.prims;
    let picture = match (picture, card) {
        (true, Some(card)) => Some(assets.picture_for(card, block)?),
        _ => None,
    };
    Ok((prims, picture))
}

/// The loss darkening 0x1B8DF0: a full-screen white quad with ALPHA mode 2
/// (Cd − Cs·As/128) and alpha D3A (0..=0x80); nothing at 0.
pub fn darkening_primitive(d3a: u8) -> Option<Primitive> {
    (d3a != 0).then(|| {
        let c = [255, 255, 255, d3a];
        Primitive {
            element: Element::Banner,
            kind: PrimKind::TriStrip,
            texture: TextureSource::None,
            target: Target::Frame,
            blend: Some(2),
            alpha_test: false,
            bilinear: false,
            gouraud: false,
            z: HUD_Z,
            verts: vec![
                vtx(0.0, 0.0, 0.0, 0.0, c),
                vtx(FRAME as f32, 0.0, 0.0, 0.0, c),
                vtx(0.0, FRAME as f32, 0.0, 0.0, c),
                vtx(FRAME as f32, FRAME as f32, 0.0, 0.0, c),
            ],
        }
    })
}

/// Render banner primitives (and the darkening) as a premultiplied RGBA8
/// layer over the duel board (`duel_hud::render_layer`: no dither, coverage
/// from black/white renders — the subtractive box and darkening are
/// approximated as "over" operations, see that function's note).
pub fn render_banner_layer(
    assets: &mut EndAssets,
    prims: Vec<Primitive>,
    picture: Option<Texture>,
) -> (Vec<u8>, Vec<String>) {
    assets.textures.render_target = picture;
    let frame = HudFrame {
        prims,
        unsupported: Vec::new(),
        next: HudUpdate::default(),
    };
    let (layer, skipped) = render_layer(&frame, &assets.tables, &mut assets.textures, FRAME, FRAME);
    let notes = skipped
        .into_iter()
        .map(|s| format!("banner primitive {}: {}", s.index, s.reason))
        .collect();
    (layer, notes)
}

/// Composite a premultiplied layer over another premultiplied layer
/// (`out = top + (1 − a_top)·bottom`), both RGBA8 of the same size.
pub fn composite_over(bottom: &[u8], top: &[u8]) -> Vec<u8> {
    bottom
        .chunks_exact(4)
        .zip(top.chunks_exact(4))
        .flat_map(|(b, t)| {
            let keep = 255 - u32::from(t[3]);
            let mix =
                |i: usize| (u32::from(t[i]) + (u32::from(b[i]) * keep + 127) / 255).min(255) as u8;
            [mix(0), mix(1), mix(2), mix(3)]
        })
        .collect()
}

fn name_message(card: u16) -> u16 {
    // 0x1DF480: names start at message 323.
    323 + card
}

fn vtx(x: f32, y: f32, u: f32, v: f32, rgba: [u8; 4]) -> Vertex {
    Vertex { x, y, u, v, rgba }
}

/// The banner drawer's primitive builder (the subset of 0x1B1830 the six
/// banners use, with the HUD panel routine 0x1B5360 as `duel_hud` ports it).
struct BannerBuilder<'a> {
    t: &'a HudTables,
    prims: Vec<Primitive>,
}

impl BannerBuilder<'_> {
    fn push(
        &mut self,
        kind: PrimKind,
        texture: TextureSource,
        blend: Option<u8>,
        alpha_test: bool,
        gouraud: bool,
        verts: Vec<Vertex>,
    ) {
        let bilinear = matches!(texture, TextureSource::TextCanvas { .. });
        self.prims.push(Primitive {
            element: Element::Banner,
            kind,
            texture,
            target: Target::Frame,
            blend,
            alpha_test,
            bilinear,
            gouraud,
            z: HUD_Z,
            verts,
        });
    }

    fn descriptor(&self, index: usize) -> Result<Descriptor, String> {
        self.t
            .sprites
            .get(index)
            .copied()
            .ok_or_else(|| format!("sprite descriptor {index} is past the table"))
    }

    fn atlas_sprite(
        &mut self,
        d: &Descriptor,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        uv: [f32; 4],
        rgba: [u8; 4],
    ) {
        self.push(
            PrimKind::Sprite,
            TextureSource::Atlas(d.slot),
            Some(0),
            true,
            false,
            vec![
                vtx(x0, y0, uv[0], uv[1], rgba),
                vtx(x1, y1, uv[2], uv[3], rgba),
            ],
        );
    }

    /// 0x1B5360: gradient (colour set ≥ 0) and 3×3 frame of panel `ptype`
    /// at (x, y) with inner size (wu, hu) given negative (explicit pixels) or
    /// positive (middle-piece units), as `duel_hud::Builder::panel`. The
    /// gradient of a colour set whose alpha is below 128 (set 13, the box)
    /// is drawn with ALPHA mode 2 and the alpha test, as observed [emu];
    /// opaque sets (the plates) without blending.
    #[allow(clippy::too_many_arguments)]
    fn panel(
        &mut self,
        colour: Option<usize>,
        ptype: usize,
        x: i32,
        y: i32,
        wu: i32,
        hu: i32,
    ) -> Result<(), String> {
        let p: Vec<Descriptor> = (0..9)
            .map(|k| self.descriptor(ptype * 9 + 59 + k))
            .collect::<Result<_, _>>()?;
        let m = self
            .t
            .margins
            .get(ptype)
            .copied()
            .ok_or_else(|| format!("panel type {ptype} has no margins"))?
            .map(i32::from);
        let lead_w = p[0].width() - m[0];
        let trail_w = p[2].width() - m[1];
        let lead_h = p[0].height() - m[2];
        let trail_h = p[6].height() - m[3];
        let inner_w = if wu < 0 {
            -wu
        } else {
            lead_w + trail_w + wu * p[1].width()
        };
        let inner_h = if hu < 0 {
            -hu
        } else {
            lead_h + trail_h + hu * p[3].height()
        };
        if let Some(set) = colour {
            let c = self
                .t
                .colours
                .get(set)
                .ok_or_else(|| format!("colour set {set} is past the table"))?;
            let outer = [c[2], c[1], c[0], c[3]];
            let inner = [c[6], c[5], c[4], c[7]];
            let (gx, gy) = ((x + m[0]) as f32, (y + m[2]) as f32);
            let (w, hh) = (inner_w as f32, (inner_h >> 1) as f32);
            let translucent = outer[3] < 128;
            let (blend, test) = if translucent {
                (Some(2), true)
            } else {
                (None, false)
            };
            let g = |xx: f32, yy: f32, c| vtx(xx, yy, 0.0, 0.0, c);
            self.push(
                PrimKind::TriStrip,
                TextureSource::None,
                blend,
                test,
                true,
                vec![
                    g(gx, gy, outer),
                    g(gx + w, gy, outer),
                    g(gx, gy + hh, inner),
                    g(gx + w, gy + hh, inner),
                ],
            );
            self.push(
                PrimKind::TriStrip,
                TextureSource::None,
                blend,
                test,
                true,
                vec![
                    g(gx, gy + hh, inner),
                    g(gx + w, gy + hh, inner),
                    g(gx, gy + 2.0 * hh, outer),
                    g(gx + w, gy + 2.0 * hh, outer),
                ],
            );
        }
        let mid = |n: i32, unit: i32, lead: i32, trail: i32| match n {
            1.. => n * unit,
            0 => 0,
            _ => -n - lead - trail,
        };
        let mut row_y = 0;
        for r in 0..3 {
            let (mut col_x, mut rh) = (0, 0);
            for k in 0..3 {
                let d = p[r * 3 + k];
                let mut w = d.width();
                let mut h = d.height();
                if k == 1 {
                    w = mid(wu, d.width(), lead_w, trail_w);
                }
                if r == 1 {
                    h = mid(hu, d.height(), lead_h, trail_h);
                }
                if r * 3 + k != 4 && w > 0 && h > 0 {
                    let (x0, y0) = ((x + col_x) as f32 - 0.5, (y + row_y) as f32 - 0.5);
                    let uv = [
                        f32::from(d.u0),
                        f32::from(d.v0),
                        f32::from(d.u1),
                        f32::from(d.v1),
                    ];
                    self.atlas_sprite(&d, x0, y0, x0 + w as f32, y0 + h as f32, uv, GREY);
                }
                col_x += w;
                rh = h;
            }
            row_y += rh;
        }
        Ok(())
    }

    /// 0x1B7B50 with flags 3 (proportional, centred on x) at font size `size`.
    fn gold_string(&mut self, x: i32, y: i32, size: usize, text: &[u8]) -> Result<i32, String> {
        let width = self.gold_width(size, text)?;
        let (cw, _) = self.t.font_sizes[size];
        let mut pen = x - (width >> 1);
        for &ch in text {
            let Some(g) = gold_glyph(ch) else {
                pen += i32::from(cw) >> 1;
                continue;
            };
            let d = self.glyph(g, size)?;
            let (gw, gh) = (d.width(), d.height());
            let (x0, y0) = (pen as f32, y as f32);
            let uv = [
                f32::from(d.u0) + 0.5,
                f32::from(d.v0) + 0.5,
                f32::from(d.u1) + 0.5,
                f32::from(d.v1) + 0.5,
            ];
            self.push(
                PrimKind::TriStrip,
                TextureSource::Atlas(d.slot),
                Some(0),
                true,
                false,
                vec![
                    vtx(x0, y0, uv[0], uv[1], GREY),
                    vtx(x0 + gw as f32, y0, uv[2], uv[1], GREY),
                    vtx(x0, y0 + gh as f32, uv[0], uv[3], GREY),
                    vtx(x0 + gw as f32, y0 + gh as f32, uv[2], uv[3], GREY),
                ],
            );
            pen += gw;
        }
        Ok(width)
    }

    fn glyph(&self, g: usize, size: usize) -> Result<Descriptor, String> {
        self.t
            .glyphs
            .get(g + 37 * size)
            .copied()
            .ok_or_else(|| format!("gold glyph {g} of size {size} is past the table"))
    }

    /// 0x1B82C0(size, flags 3): the proportional width of a gold string.
    fn gold_width(&self, size: usize, text: &[u8]) -> Result<i32, String> {
        let (cw, _) = self.t.font_sizes[size];
        let mut width = 0;
        for &ch in text {
            width += match gold_glyph(ch) {
                Some(g) => self.glyph(g, size)?.width(),
                None => i32::from(cw) >> 1,
            };
        }
        Ok(width)
    }

    /// Composed width of a message (12 pixels per code, 0x246FE0 +0xC).
    fn message_width(&self, message: u16) -> Result<i32, String> {
        self.t
            .message(message)
            .map(|codes| 12 * codes.len() as i32)
            .ok_or_else(|| format!("message {message} was not imported"))
    }

    /// 0x246B90 / 0x246D00: a composed canvas line drawn at (x, y), `w`
    /// texels wide, x-scaled by `sx` (0x1A2B80 truncates the scaled width).
    fn text_line(&mut self, line: u8, message: u16, x: i32, y: i32, w: i32, sx: f32) {
        let dw = (w as f32 * sx) as i32;
        let v = 22.0 * f32::from(line);
        self.push(
            PrimKind::Sprite,
            TextureSource::TextCanvas {
                line,
                message: Some(message),
            },
            Some(0),
            true,
            false,
            vec![
                vtx(x as f32, y as f32, 0.5, v + 0.5, WHITE),
                vtx(
                    (x + dw) as f32,
                    (y + 22) as f32,
                    w as f32 + 0.5,
                    v + 22.5,
                    WHITE,
                ),
            ],
        );
    }

    /// 0x1B1830(pkt, side, card | portrait bit, name message, centre 0x100,
    /// top, width, min height, title string, icon = 1). Returns whether a
    /// card picture sprite was emitted (its texture is the caller's).
    #[allow(clippy::too_many_arguments)]
    fn box_0x1b1830(
        &mut self,
        colour: usize,
        portrait: Option<u8>,
        card: Option<u16>,
        name: Option<u16>,
        top: i32,
        width: i32,
        min_height: i32,
        title: &[u8],
        icon_cel: u8,
    ) -> Result<bool, String> {
        const TITLE_SIZE: usize = 2;
        let fit = width < 0;
        let mut width = width.abs();
        let has_title = !title.is_empty();
        // 0x1B82C0: the plate is the title's proportional width + 0x18 wide
        // and its glyph height + 0xC tall.
        let title_w = self.gold_width(TITLE_SIZE, title)?;
        let title_h = i32::from(self.t.font_sizes[TITLE_SIZE].1);
        let (plate_w, plate_h) = if has_title {
            (title_w + 0x18, title_h + 0xC)
        } else {
            (0, 0)
        };
        let name_w = match name {
            Some(m) => Some(self.message_width(m)?),
            None => None,
        };
        let mut scale = 1.0f32;
        let (min_w, mut height) = match name_w {
            Some(nw) => {
                let min_w = if fit {
                    if width - 0x18 < nw {
                        scale = (width - 0x18) as f32 / nw as f32;
                    }
                    width
                } else if nw >= 0x1C9 {
                    scale = 456.0 / nw as f32;
                    0x1E0
                } else {
                    nw.max(plate_w) + 0x18
                };
                let base = if has_title { plate_h + 0x18 } else { 0xC };
                (min_w, base + 22 + 0xC)
            }
            None => (
                if has_title { plate_w + 0x18 } else { 0 },
                if has_title { plate_h + 0x18 } else { 0xC },
            ),
        };
        if portrait.is_some() {
            height += 0x3C;
        } else if card.is_some() {
            height += 0x8C;
        }
        height += 0x1C; // the press icon
        if width < min_w {
            width = min_w;
        }
        let inner_h = min_height.max(height);
        self.panel(
            Some(BOX_COLOUR),
            BOX_PANEL,
            CENTRE_X - (width >> 1) - 4,
            top,
            -width,
            -inner_h,
        )?;
        if has_title {
            self.panel(
                Some(colour),
                PLATE_PANEL,
                CENTRE_X - (plate_w >> 1),
                top + 0xC,
                -plate_w,
                -plate_h,
            )?;
            self.gold_string(CENTRE_X, top + 0x14, TITLE_SIZE, title)?;
        }
        let content_y = top + if has_title { plate_h + 0x18 } else { 0xC };
        let mut picture = false;
        if let Some(side) = portrait {
            let (px, py) = ((CENTRE_X - 0x18) as f32, content_y as f32);
            self.push(
                PrimKind::Sprite,
                TextureSource::Portrait(side),
                None,
                false,
                false,
                vec![
                    vtx(px, py, 0.5, 0.5, GREY),
                    vtx(px + 48.0, py + 48.0, 40.5, 32.5, GREY),
                ],
            );
            self.panel(None, PORTRAIT_PANEL, CENTRE_X - 0x18, content_y, 3, 6)?;
        } else if card.is_some() {
            let (px, py) = ((CENTRE_X - 0x40) as f32, content_y as f32);
            self.push(
                PrimKind::Sprite,
                TextureSource::RenderTarget,
                None,
                false,
                false,
                vec![
                    vtx(px, py, 0.5, 0.5, GREY),
                    vtx(px + 128.0, py + 128.0, 128.5, 128.5, GREY),
                ],
            );
            picture = true;
        }
        if let (Some(m), Some(nw)) = (name, name_w) {
            let half = ((nw as f32 * scale) / 2.0) as i32;
            let ny = top
                + if has_title { plate_h + 0x18 } else { 0x10 }
                + if portrait.is_some() {
                    0x3C
                } else if card.is_some() {
                    0x8C
                } else {
                    0
                };
            self.text_line(1, m, CENTRE_X - half, ny, nw, scale);
        }
        let d = self.descriptor(ICON_DESCRIPTOR + usize::from(icon_cel % 3))?;
        let (ix, iy) = ((CENTRE_X - 8) as f32, (top + inner_h - 0x1C) as f32);
        let uv = [
            f32::from(d.u0) + 0.5,
            f32::from(d.v0) + 0.5,
            f32::from(d.u1) + 0.5,
            f32::from(d.v1) + 0.5,
        ];
        self.atlas_sprite(
            &d,
            ix,
            iy,
            ix + d.width() as f32,
            iy + d.height() as f32,
            uv,
            GREY,
        );
        Ok(picture)
    }
}

/// Gold-font glyph index of an ASCII character (0x1B7B50: letters 0..=25,
/// digits 26..=35, '?' 36; anything else advances half a cell).
fn gold_glyph(ch: u8) -> Option<usize> {
    match ch {
        b'A'..=b'Z' => Some(usize::from(ch - b'A')),
        b'a'..=b'z' => Some(usize::from(ch - b'a')),
        b'0'..=b'9' => Some(usize::from(ch - b'0') + 26),
        b'?' => Some(36),
        _ => None,
    }
}

// ---------------------------------------------------------------- slot machine

/// TEXETC.MRG records 3 (the stone backdrop, reel columns, buttons and
/// icons; GS slot 0x2020) and 4 (the side panels and the title; 0x2120), as
/// the GS samples them (16-bit CLUT, TEXA 0/0x80). The backdrop is tiled to
/// 512×512 because the screen draws it with texel coordinates up to 448 in
/// REPEAT mode, which the CPU model does not wrap.
pub struct SlotTextures {
    pub backdrop: Texture,
    pub panels: Texture,
    /// The title "GRAVEYARDS SLOT MACHINE" (panel texture u 224..256 × v
    /// 0..256, stored rotated) transposed to 256×32 so it can be drawn as a
    /// filtered sprite.
    pub title: Texture,
    /// The bar's golden edge (the cap texels u 234..242 × v 0..3 of the
    /// atlas) repeated over 512 texels: the edge lines are drawn with texel
    /// coordinates 0..340 and the original frame shows the cap's gold along
    /// them, so the texture wraps on that 8-texel block [H, inferred from the
    /// frame; the CLAMP register is not traced].
    pub bar_edge: Texture,
    /// The stone behind everything: the 64×64 tile at the atlas origin
    /// repeated (the two backdrop quads sample texels 0..448 / 0..352 of
    /// it; the wrap is REGION_REPEAT on that tile — inferred from the
    /// original frame, the CLAMP register value itself is not traced [H]).
    pub stone: Texture,
}
/// Side of the stone tile at the atlas origin.
const STONE_TILE: usize = 64;

impl SlotTextures {
    pub fn new(texetc: &[u8]) -> Result<Self, String> {
        let decode = |index: usize| -> Result<Texture, String> {
            let t = roses_assets::decode_pal_opening_indexed_texture(
                texetc,
                roses_assets::OpeningArchiveKind::TexEtc,
                index,
            )
            .map_err(|e| format!("TEXETC #{index}: {e}"))?;
            Ok(Texture::from_c16_clut(
                t.width as usize,
                t.height as usize,
                &t.palette_indices,
                &t.palette_words_raw,
                0,
                0x80,
            ))
        };
        let atlas = decode(3)?;
        let panels = decode(4)?;
        if atlas.width != 256 || atlas.height != 256 || panels.width != 256 || panels.height != 256
        {
            return Err("TEXETC records 3 and 4 are not 256×256".into());
        }
        let mut backdrop = Texture {
            width: FRAME,
            height: FRAME,
            texels: Vec::with_capacity(FRAME * FRAME),
        };
        let mut stone = Texture {
            width: FRAME,
            height: FRAME,
            texels: Vec::with_capacity(FRAME * FRAME),
        };
        for y in 0..FRAME {
            for x in 0..FRAME {
                backdrop
                    .texels
                    .push(atlas.texels[(y % 256) * 256 + x % 256]);
                stone
                    .texels
                    .push(atlas.texels[(y % STONE_TILE) * 256 + x % STONE_TILE]);
            }
        }
        let mut title = Texture {
            width: 256,
            height: 32,
            texels: Vec::with_capacity(256 * 32),
        };
        for y in 0..32 {
            for x in 0..256 {
                title.texels.push(panels.texels[x * 256 + 224 + y]);
            }
        }
        let mut bar_edge = Texture {
            width: FRAME,
            height: 3,
            texels: Vec::with_capacity(FRAME * 3),
        };
        for y in 0..3 {
            for x in 0..FRAME {
                bar_edge.texels.push(atlas.texels[y * 256 + 234 + x % 8]);
            }
        }
        Ok(Self {
            backdrop,
            panels,
            title,
            bar_edge,
            stone,
        })
    }
}

/// The popup shown over the reels (screen 3, prize phase 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrizePopup {
    pub kind: PrizeKind,
    pub card: u16,
}

/// What the machine shows this frame: the popup while a prize is up.
pub fn current_popup(
    machine: &SlotMachine,
    last_prize: Option<(PrizeKind, u16)>,
) -> Option<PrizePopup> {
    if machine.screen != Screen::Prizes || machine.prizes.phase != PrizePhase::Popup {
        return None;
    }
    let (kind, _) = last_prize?;
    Some(PrizePopup {
        kind,
        card: machine.prizes.card,
    })
}

const ALPHA_OVER: Blend = Blend {
    a: 0,
    b: 1,
    c: 0,
    d: 1,
    fix: 0,
};
const ALPHA_SUBTRACT: Blend = Blend {
    a: 2,
    b: 0,
    c: 0,
    d: 1,
    fix: 0,
};
const ALPHA_ADD: Blend = Blend {
    a: 0,
    b: 2,
    c: 0,
    d: 1,
    fix: 0,
};
const ALPHA_BRIGHTEN: Blend = Blend {
    a: 1,
    b: 2,
    c: 0,
    d: 1,
    fix: 0,
};

/// Reel geometry [emu]: columns 90 apart, backgrounds 85 wide from x 124,
/// pictures 77 wide from x 128, the column 55..457, cards 110 tall on a
/// 128 pitch, the background a 2×176-texel strip (u 48..152 then 152..256
/// of the atlas, v 80..256) drawn 201 px per texture and split by the wrap.
const REEL_PITCH: i32 = 90;
const COLUMN_X: i32 = 124;
const COLUMN_W: i32 = 85;
const CARD_X: i32 = 128;
const CARD_W: i32 = 77;
const COLUMN_TOP: i32 = 55;
const COLUMN_BOTTOM: i32 = 457;
const CARD_H: i32 = 110;
const CARD_PITCH: i32 = 128;
const CARD_ROW0_Y: i32 = 73;
const STRIP_PX: i32 = 201;
const STRIP_V0: f32 = 80.0;
const STRIP_V1: f32 = 256.0;
const STRIP_U: [(f32, f32); 2] = [(48.0, 152.0), (152.0, 256.0)];

struct Painter<'a> {
    target: &'a mut Target16,
    dither: Option<u64>,
}

impl Painter<'_> {
    /// A four-vertex strip (two triangles) with the GS state of the screen's
    /// draws: TFX modulate, TCC, nearest sampling.
    fn quad(
        &mut self,
        tex: Option<&Texture>,
        xy: [[f32; 2]; 4],
        uv: [[f32; 2]; 4],
        rgba: [[u8; 4]; 4],
        blend: Option<Blend>,
        alpha_test: bool,
        gouraud: bool,
    ) {
        let strip: Vec<_> = (0..4).map(|i| (xy[i], uv[i], rgba[i])).collect();
        gs::draw_strip(
            self.target,
            &strip,
            &Triangle {
                xy: [[0.0; 2]; 3],
                uv: [[0.0; 2]; 3],
                rgba: [[0; 4]; 3],
                gouraud,
                texture: tex,
                modulate: true,
                tcc: true,
                blend,
                alpha_test_greater: alpha_test.then_some(0),
                dither: self.dither,
            },
        );
    }

    /// An axis-aligned textured rectangle (x0,y0)-(x1,y1) mapped to texels
    /// (u0,v0)-(u1,v1); mirrored when x1 < x0, y1 < y0, u1 < u0 or v1 < v0.
    /// Nearest sampling through the strip path, or bilinear through the
    /// sprite path (the two rasterise axis-aligned quads alike).
    #[allow(clippy::too_many_arguments)]
    fn rect(
        &mut self,
        tex: &Texture,
        x: [f32; 4],
        uv: [f32; 4],
        rgba: [u8; 4],
        blend: Blend,
        alpha_test: bool,
    ) {
        self.rect_filtered(tex, x, uv, rgba, blend, alpha_test, false);
    }

    #[allow(clippy::too_many_arguments)]
    fn rect_filtered(
        &mut self,
        tex: &Texture,
        x: [f32; 4],
        uv: [f32; 4],
        rgba: [u8; 4],
        blend: Blend,
        alpha_test: bool,
        bilinear: bool,
    ) {
        let [mut x0, mut y0, mut x1, mut y1] = x;
        let [mut u0, mut v0, mut u1, mut v1] = uv;
        if !bilinear {
            self.quad(
                Some(tex),
                [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
                [[u0, v0], [u1, v0], [u0, v1], [u1, v1]],
                [rgba; 4],
                Some(blend),
                alpha_test,
                false,
            );
            return;
        }
        if x1 < x0 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut u0, &mut u1);
        }
        if y1 < y0 {
            std::mem::swap(&mut y0, &mut y1);
            std::mem::swap(&mut v0, &mut v1);
        }
        gs::draw(
            self.target,
            &Sprite {
                xy: [x0, y0, x1, y1],
                uv: [u0, v0, u1, v1],
                rgba,
                texture: Some(tex),
                modulate: true,
                tcc: true,
                bilinear: true,
                blend: Some(blend),
                alpha_test_greater: alpha_test.then_some(0),
                dither: self.dither,
            },
        );
    }

    fn atlas(&mut self, tex: &Texture, x: [f32; 4], uv: [f32; 4]) {
        self.rect(tex, x, uv, GREY, ALPHA_OVER, true);
    }

    /// The static screen's stretched textures are drawn filtered (the
    /// original frame shows them smoothed: TEX1 MMAG linear [H, inferred from
    /// the frame; the register is not traced]).
    fn atlas_filtered(&mut self, tex: &Texture, x: [f32; 4], uv: [f32; 4]) {
        self.rect_filtered(tex, x, uv, GREY, ALPHA_OVER, true, STATIC_BILINEAR);
    }
}

/// Whether the static screen (stone, panels, bars, buttons, arrows) is drawn
/// with bilinear filtering; see `Painter::atlas_filtered`.
pub const STATIC_BILINEAR: bool = true;

/// Draw the slot machine screen of one frame into `target` (cleared to
/// black first): the static screen, the three reels as the physics left
/// them, the line flash, the live-line markers, the arrows, and the prize
/// popup when one is up. `icon_cel` is the popup's press-icon cel, `block`
/// the winner's block (art counters), `dither` the DIMX matrix to apply
/// (None: no dithering). Returns notes about parts drawn from the model
/// only or left out.
#[allow(clippy::too_many_arguments)]
pub fn render_slot_screen(
    machine: &SlotMachine,
    popup: Option<PrizePopup>,
    assets: &mut EndAssets,
    slot: &SlotTextures,
    block: &CardBlock,
    icon_cel: u8,
    dither: Option<u64>,
    target: &mut Target16,
) -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    target.pixels.iter_mut().for_each(|p| *p = 0);
    let mut p = Painter { target, dither };
    let atlas = &slot.backdrop;
    let panels = &slot.panels;
    // ---- 0x2828C0: the static screen [emu spinning/stopped lists 0..20]
    p.atlas_filtered(
        &slot.stone,
        [77.0, 0.0, 435.0, 512.0],
        [0.0, 0.0, 448.0, 448.0],
    );
    p.atlas_filtered(
        &slot.stone,
        [115.0, 55.0, 397.0, 457.0],
        [0.0, 0.0, 352.0, 352.0],
    );
    // The title is stored rotated in the panel texture (u 224..256 across
    // the screen's x, u 256 at the top): drawn from its transposed copy.
    p.atlas_filtered(
        &slot.title,
        [154.0, 9.0, 358.0, 46.0],
        [0.0, 32.0, 256.0, 0.0],
    );
    let panel_uv = [126.0, 0.0, 222.0, 224.0];
    p.atlas_filtered(panels, [0.0, 0.0, 77.0, 256.0], panel_uv);
    p.atlas_filtered(panels, [77.0, 512.0, 0.0, 256.0], panel_uv);
    p.atlas_filtered(panels, [435.0, 0.0, 512.0, 256.0], panel_uv);
    p.atlas_filtered(panels, [512.0, 512.0, 435.0, 256.0], panel_uv);
    p.atlas_filtered(
        atlas,
        [114.0, 55.0, 120.0, 457.0],
        [132.0, 48.0, 140.0, 80.0],
    );
    p.atlas_filtered(
        atlas,
        [395.0, 55.0, 402.0, 457.0],
        [152.0, 48.0, 160.0, 80.0],
    );
    p.atlas_filtered(
        atlas,
        [120.0, 457.0, 256.0, 512.0],
        [64.0, 0.0, 234.0, 48.0],
    );
    p.atlas_filtered(
        atlas,
        [256.0, 457.0, 392.0, 512.0],
        [234.0, 0.0, 64.0, 48.0],
    );
    p.atlas_filtered(
        atlas,
        [120.0, 457.0, 126.0, 512.0],
        [234.0, 0.0, 242.0, 48.0],
    );
    p.atlas_filtered(
        atlas,
        [386.0, 457.0, 392.0, 512.0],
        [242.0, 0.0, 234.0, 48.0],
    );
    p.atlas_filtered(
        &slot.bar_edge,
        [120.0, 457.0, 392.0, 461.0],
        [0.0, 0.0, 340.0, 3.0],
    );
    p.atlas_filtered(
        &slot.bar_edge,
        [120.0, 509.0, 392.0, 512.0],
        [0.0, 0.0, 340.0, 3.0],
    );
    for bx in [138.0, 227.0, 317.0] {
        let bw = if bx == 138.0 { 54.0 } else { 55.0 };
        p.atlas_filtered(
            atlas,
            [bx, 462.0, bx + bw, 485.0],
            [64.0, 48.0, 132.0, 68.0],
        );
        p.atlas_filtered(
            atlas,
            [bx, 507.0, bx + bw, 485.0],
            [64.0, 48.0, 132.0, 68.0],
        );
    }
    if machine.screen == Screen::Spinning {
        // The □ △ ○ symbols and their glow while a stop key is accepted.
        p.atlas(
            atlas,
            [155.0, 470.0, 175.0, 499.0],
            [22.0, 176.0, 44.0, 198.0],
        );
        p.atlas(
            atlas,
            [245.0, 470.0, 265.0, 499.0],
            [0.0, 176.0, 22.0, 198.0],
        );
        p.atlas(
            atlas,
            [334.0, 470.0, 354.0, 499.0],
            [66.0, 176.0, 88.0, 198.0],
        );
        for gx in [152.0, 242.0, 331.0] {
            p.rect(
                atlas,
                [gx, 466.0, gx + 27.0, 504.0],
                [22.0, 96.0, 46.0, 120.0],
                [96, 128, 128, 36],
                ALPHA_ADD,
                true,
            );
        }
    }
    // ---- 0x283910: one reel each [emu, thirteen spinning frames + stopped]
    let count = machine.reels[0].strip.len().max(1) as i32;
    for (r, reel) in machine.reels.iter().enumerate() {
        let x0 = (COLUMN_X + REEL_PITCH * r as i32) as f32;
        let x1 = x0 + COLUMN_W as f32;
        let scroll = i32::from(reel.scroll).rem_euclid(0x15F);
        let (main, q) = if scroll < STRIP_PX {
            (0, scroll)
        } else {
            (1, scroll - STRIP_PX)
        };
        let other = 1 - main;
        let (mu0, mu1) = STRIP_U[main];
        let (ou0, ou1) = STRIP_U[other];
        let top = (COLUMN_TOP + q) as f32;
        p.atlas(
            atlas,
            [x0, top, x1, top + STRIP_PX as f32],
            [mu0, STRIP_V0, mu1, STRIP_V1],
        );
        let split_v = STRIP_V1 - q as f32;
        p.atlas(
            atlas,
            [x0, COLUMN_TOP as f32, x1, top],
            [ou0, split_v, ou1, STRIP_V1],
        );
        p.atlas(
            atlas,
            [x0, top + STRIP_PX as f32, x1, COLUMN_BOTTOM as f32],
            [ou0, STRIP_V0, ou1, split_v],
        );
        let strip = &reel.strip;
        let pos = i32::from(reel.pos).max(0);
        let i0 = pos / CARD_PITCH;
        let offset = pos % CARD_PITCH;
        let cx0 = (CARD_X + REEL_PITCH * r as i32) as f32;
        for j in 0..4i32 {
            let card = strip[((i0 + j).rem_euclid(count)) as usize];
            let y0 = CARD_ROW0_Y + offset + CARD_PITCH * (2 - j);
            let (vis0, vis1) = (y0.max(COLUMN_TOP), (y0 + CARD_H).min(COLUMN_BOTTOM));
            if vis1 <= vis0 {
                continue;
            }
            let texel = |dy: i32| (dy as f32 * 128.0 / CARD_H as f32) as i32 as f32;
            let (v0, v1) = (texel(vis0 - y0), texel(vis1 - y0));
            let picture = assets.picture_for(card, block)?;
            p.rect(
                &picture,
                [cx0, vis0 as f32, cx0 + CARD_W as f32, vis1 as f32],
                [0.0, v0, 128.0, v1],
                GREY,
                ALPHA_OVER,
                false,
            );
        }
        // The column shading: subtractive gradients, dark at both ends.
        let shade = |p: &mut Painter, y0: f32, y1: f32, c0: u8, c1: u8| {
            p.quad(
                None,
                [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
                [[0.0; 2]; 4],
                [
                    [c0, c0, c0, 128],
                    [c0, c0, c0, 128],
                    [c1, c1, c1, 128],
                    [c1, c1, c1, 128],
                ],
                Some(ALPHA_SUBTRACT),
                true,
                true,
            );
        };
        shade(&mut p, 55.0, 125.0, 128, 32);
        shade(&mut p, 125.0, 256.0, 32, 0);
        shade(&mut p, 256.0, 387.0, 0, 32);
        shade(&mut p, 387.0, 457.0, 32, 128);
    }
    // ---- 0x283340 + 0x282040(2): the flash of paying lines [emu, forced
    // screen 2 with line 0: cells of the middle row, the four markers of the
    // middle row and the arrows' glow; other rows/lines are not observed and
    // are drawn by the same rule (H)].
    if machine.flash.started {
        let alpha = |c: u8| -> u8 { 64 + 8 * (16u8.saturating_sub(c)).min(c.saturating_sub(1)) };
        let glow = |p: &mut Painter, x: [f32; 4], uv: [f32; 4], a: u8, tex: &Texture| {
            let c = [2 * a, 2 * a, 2 * a, a];
            p.rect(tex, x, uv, c, ALPHA_OVER, true);
            p.rect(tex, x, uv, c, ALPHA_BRIGHTEN, true);
        };
        for (r, cells) in machine.flash.cells.iter().enumerate() {
            let cx = 166.0 + REEL_PITCH as f32 * r as f32;
            for (row, &c) in cells.iter().enumerate() {
                if c == 0 {
                    continue;
                }
                let a = alpha(c);
                let cy = 384.0 - 128.0 * row as f32;
                if row != 1 {
                    notes.push(format!("flash of reel {r} row {row}: geometry not observed, drawn by the middle-row rule"));
                }
                let uv = [68.0, 57.0, 125.0, 114.0];
                glow(&mut p, [cx - 47.0, cy - 64.0, cx, cy + 1.0], uv, a, atlas);
                glow(&mut p, [cx + 47.0, cy - 64.0, cx, cy + 1.0], uv, a, atlas);
                glow(&mut p, [cx - 47.0, cy + 65.0, cx, cy], uv, a, atlas);
                glow(&mut p, [cx + 47.0, cy + 65.0, cx, cy], uv, a, atlas);
            }
        }
        for (k, &c) in machine.flash.lines.iter().enumerate() {
            if c == 0 {
                continue;
            }
            let a = alpha(c);
            if k != 0 {
                notes.push(format!(
                    "flash markers of line {k}: not observed, not drawn"
                ));
                continue;
            }
            for mx in [114.0, 203.0, 293.0, 382.0] {
                glow(
                    &mut p,
                    [mx, 251.0, mx + 16.0, 261.0],
                    [90.0, 4.0, 110.0, 12.0],
                    a,
                    panels,
                );
            }
            glow(
                &mut p,
                [98.0, 241.0, 108.0, 271.0],
                [13.0, 193.0, 26.0, 219.0],
                a,
                panels,
            );
            glow(
                &mut p,
                [417.0, 241.0, 406.0, 271.0],
                [13.0, 193.0, 26.0, 219.0],
                a,
                panels,
            );
        }
    }
    // ---- 0x282040(1): markers of the live lines while spinning [emu: line 0
    // only, the other lines were never enabled in the first duel] and the arrows.
    if machine.screen == Screen::Spinning {
        for k in 0..LINE_ROWS.len() {
            if machine.live_lines & (1 << k) == 0 {
                continue;
            }
            if k != 0 {
                notes.push(format!(
                    "live-line marker of line {k}: not observed, not drawn"
                ));
                continue;
            }
            for mx in [114.0, 203.0, 293.0, 382.0] {
                p.rect(
                    atlas,
                    [mx, 251.0, mx + 16.0, 261.0],
                    [28.0, 120.0, 48.0, 128.0],
                    [112, 112, 112, 52],
                    ALPHA_OVER,
                    true,
                );
            }
        }
    }
    p.atlas_filtered(
        atlas,
        [93.0, 230.0, 118.0, 282.0],
        [0.0, 154.0, 32.0, 200.0],
    );
    p.atlas_filtered(
        atlas,
        [395.0, 230.0, 421.0, 282.0],
        [32.0, 154.0, 0.0, 200.0],
    );
    // ---- the prize popup (overlay drawer, z above the reels)
    if let Some(popup) = popup {
        let kind = BannerKind::Prize {
            special: popup.kind == PrizeKind::GetSpecialCard,
            card: popup.card,
        };
        let (prims, picture) = banner_primitives(assets, kind, 0, icon_cel, block)?;
        assets.textures.render_target = picture;
        let saved = assets.tables.dimx;
        assets.tables.dimx = dither.unwrap_or(0);
        let skipped = render_primitives(&prims, &assets.tables, &mut assets.textures, p.target);
        assets.tables.dimx = saved;
        for s in skipped {
            notes.push(format!("popup primitive {}: {}", s.index, s.reason));
        }
    }
    Ok(notes)
}

// ---------------------------------------------------------------- card blocks

/// A side's persistent card block at the start of a new game, as far as the
/// capture of the first duel shows it (the project notes
/// t15`, blocks read before the duel's changes were applied) [H, the new-game
/// writer is untraced]: one instance per deck card (owner key, random id from
/// the shared LCG, owned bit; the recent list left empty as the capture
/// shows it), the leader's stored rank from its deck record and its
/// experience at that rank's threshold (player: rank 1, 500; Weevil: rank
/// 2, 1000 in the capture). The computer's key in the capture
/// was 0x40000000 | its deck record (0x4000001C); the player's looked random
/// (0x6D921E28), so it is drawn from the LCG here.
pub fn new_game_block(
    owner_key: u32,
    deck: &[u16],
    leader: u16,
    leader_rank: u16,
    thresholds: &[u16; RANK_COUNT],
    rng: &mut u64,
) -> CardBlock {
    let mut block = CardBlock::empty(owner_key);
    for &card in deck.iter().chain(std::iter::once(&leader)) {
        block.grant(card, None, rng);
    }
    // The grants above move each card to the front of the recent list;
    // the capture's new-game block has an empty one (all 999), so the
    // deck editor's NEWCOMER sort starts without newcomers.
    block.recent = [EMPTY_CARD; RECENT_LEN];
    let rank = leader_rank.min(RANK_COUNT as u16 - 1);
    if let Some(record) = block.records.get_mut(usize::from(leader)) {
        record.rank_word = (record.rank_word & !roses_core::card_block::RANK_MASK) | rank;
        record.experience = thresholds[usize::from(rank)];
    }
    block
}

/// The computer side's owner key as the capture shows it: 0x40000000 | deck record.
pub fn computer_owner_key(deck_record: usize) -> u32 {
    0x4000_0000 | (deck_record as u32 & 0xFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_online_seat_reads_you_win_or_you_lose() {
        use roses_core::duel_end::banner;
        assert_eq!(seat_result(banner::P1_WIN, 0, Some(0)), (banner::YOU_WIN, 0));
        assert_eq!(seat_result(banner::P1_WIN, 0, Some(1)), (banner::YOU_LOSE, 1));
        assert_eq!(seat_result(banner::P2_WIN, 1, Some(1)), (banner::YOU_WIN, 1));
        assert_eq!(seat_result(banner::P2_WIN, 1, Some(0)), (banner::YOU_LOSE, 0));
        // A draw, a spectator and an offline shared screen are unchanged.
        assert_eq!(seat_result(banner::DRAW, 2, Some(0)), (banner::DRAW, 2));
        assert_eq!(seat_result(banner::P2_WIN, 1, None), (banner::P2_WIN, 1));
    }
    use roses_core::slot_machine::{ReelData, SlotMachine};

    #[test]
    fn reel_geometry_matches_the_interpreter_frames() {
        // scroll 21 -> main A at 55+21, top piece v 235..256; scroll 213 ->
        // main B at 55+12; pos 752 -> rows at 57/185/313/441.
        let cases: [(i32, usize, i32); 3] = [(21, 0, 21), (213, 1, 12), (340, 1, 139)];
        for (scroll, main, q) in cases {
            let (m, qq) = if scroll < STRIP_PX {
                (0, scroll)
            } else {
                (1, scroll - STRIP_PX)
            };
            assert_eq!((m, qq), (main, q));
        }
        let pos = 752;
        let rows: Vec<i32> = (0..4)
            .map(|j| CARD_ROW0_Y + pos % CARD_PITCH + CARD_PITCH * (2 - j))
            .collect();
        assert_eq!(rows, vec![441, 313, 185, 57]);
        let texel = |dy: i32| (dy as f32 * 128.0 / CARD_H as f32) as i32;
        assert_eq!(texel(16), 18);
        assert_eq!(texel(94), 109);
        assert_eq!(texel(80), 93);
    }

    #[test]
    fn flash_alpha_follows_the_counter() {
        let alpha = |c: u8| -> u8 { 64 + 8 * (16u8.saturating_sub(c)).min(c.saturating_sub(1)) };
        assert_eq!(
            [
                alpha(16),
                alpha(15),
                alpha(13),
                alpha(9),
                alpha(8),
                alpha(5),
                alpha(2),
                alpha(1)
            ],
            [64, 72, 88, 120, 120, 96, 72, 64]
        );
    }

    #[test]
    fn gold_glyphs_and_composite() {
        assert_eq!(gold_glyph(b'Y'), Some(24));
        assert_eq!(gold_glyph(b'7'), Some(33));
        assert_eq!(gold_glyph(b' '), None);
        let bottom = [10, 20, 30, 255, 0, 0, 0, 0];
        let top = [100, 100, 100, 255, 50, 0, 0, 128];
        let out = composite_over(&bottom, &top);
        assert_eq!(&out[..4], &[100, 100, 100, 255]);
        assert_eq!(&out[4..], &[50, 0, 0, 128]);
    }

    #[test]
    fn new_game_block_marks_the_deck_and_the_leader() {
        let mut rng = 1u64;
        let thresholds = [
            0, 500, 1000, 2000, 4000, 7000, 11000, 16000, 22000, 30000, 40000, 52000, 65535,
        ];
        let block = new_game_block(0x11, &[5, 6, 6], 9, 1, &thresholds, &mut rng);
        assert_eq!(block.record(6).unwrap().copies(), 2);
        assert!(block.record(5).unwrap().owned());
        assert_eq!(block.stored_rank(9), Some(1));
        assert_eq!(block.experience(9), Some(500));
        // A new game's recent list is empty (the capture's block).
        assert!(block.recent.iter().all(|&c| c == EMPTY_CARD));
        assert_eq!(computer_owner_key(0x1C), 0x4000_001C);
        // The slot machine model accepts the block's grants through PrizeGrant.
        let data = ReelData {
            strip: vec![1, 2, 3, 4, 5, 6, 7, 8],
            random_slip: false,
            enabled_lines: [true, false, false, false, false],
        };
        let mut rng = 7u64;
        let (machine, _) = SlotMachine::new(&data, vec![], &mut rng).unwrap();
        assert_eq!(machine.reels[0].strip.len(), 8);
    }
}
